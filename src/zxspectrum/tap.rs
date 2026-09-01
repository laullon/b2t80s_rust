use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{bail, ensure};

#[derive(Debug, Clone)]
struct DataBlock {
    range: std::ops::Range<usize>,
}

#[derive(Debug, Clone)]
pub struct Tap {
    blocks: Vec<DataBlock>,
    actual_block: usize,
    data: Vec<u8>,
    pub name: String,
}

const PILOT_PULSE: u32 = 2_168;
const HEADER_PILOT_PULSES: usize = 8_063;
const DATA_PILOT_PULSES: usize = 3_223;
const SYNC_PULSE_1: u32 = 667;
const SYNC_PULSE_2: u32 = 735;
const ZERO_PULSE: u32 = 855;
const ONE_PULSE: u32 = 1_710;
const BLOCK_PAUSE: u32 = 3_500_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TapeSegment {
    Pulse(u32),
    Pause(u32),
}

#[derive(Debug)]
pub struct TapePlayer {
    segments: VecDeque<TapeSegment>,
    remaining: u32,
    ear: bool,
    playing: bool,
}

impl Tap {
    pub fn new(url: &Path) -> anyhow::Result<Self> {
        let mut file = File::open(url)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        if data.starts_with(b"ZXTape!") {
            bail!("TZX tape images are not supported yet");
        }

        ensure!(!data.is_empty(), "Tape image is empty");

        let mut start = 0;
        let mut blocks = Vec::new();
        while start < data.len() {
            let block = Self::read_default_block(&data, start)?;
            start = block.range.end;
            blocks.push(block);
        }

        Ok(Self {
            blocks,
            actual_block: 0,
            data,
            name: url.to_string_lossy().into_owned(),
        })
    }

    pub fn next_block(&mut self) -> Option<Vec<u8>> {
        if self.actual_block >= self.blocks.len() {
            return None;
        }
        let block = &self.blocks[self.actual_block];
        self.actual_block += 1;
        Some(self.data[block.range.clone()].to_vec())
    }

    pub fn into_player(self) -> TapePlayer {
        let blocks = self
            .blocks
            .iter()
            .map(|block| self.data[block.range.clone()].to_vec());
        TapePlayer::from_blocks(blocks)
    }

    fn read_default_block(data: &[u8], start: usize) -> anyhow::Result<DataBlock> {
        ensure!(
            data.len().saturating_sub(start) >= 2,
            "Truncated TAP block header at offset {start}"
        );
        let length = (data[start] as usize) | ((data[start + 1] as usize) << 8);
        ensure!(
            length >= 2,
            "Invalid TAP block length {length} at offset {start}"
        );
        let end = start
            .checked_add(length + 2)
            .ok_or_else(|| anyhow::anyhow!("TAP block length overflow at offset {start}"))?;
        ensure!(
            end <= data.len(),
            "TAP block at offset {start} declares {length} bytes, but only {} remain",
            data.len() - start - 2
        );
        Ok(DataBlock {
            range: start + 2..end,
        })
    }
}

impl TapePlayer {
    fn from_blocks(blocks: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let mut segments = VecDeque::new();

        for block in blocks {
            let pilot_pulses = if block.first().copied() == Some(0) {
                HEADER_PILOT_PULSES
            } else {
                DATA_PILOT_PULSES
            };
            segments.extend((0..pilot_pulses).map(|_| TapeSegment::Pulse(PILOT_PULSE)));
            segments.push_back(TapeSegment::Pulse(SYNC_PULSE_1));
            segments.push_back(TapeSegment::Pulse(SYNC_PULSE_2));

            for byte in block {
                for bit in (0..8).rev() {
                    let duration = if byte & (1 << bit) == 0 {
                        ZERO_PULSE
                    } else {
                        ONE_PULSE
                    };
                    segments.push_back(TapeSegment::Pulse(duration));
                    segments.push_back(TapeSegment::Pulse(duration));
                }
            }
            segments.push_back(TapeSegment::Pause(BLOCK_PAUSE));
        }

        Self {
            playing: !segments.is_empty(),
            segments,
            remaining: 0,
            ear: false,
        }
    }

    pub fn tick(&mut self) -> bool {
        if self.remaining == 0 {
            match self.segments.pop_front() {
                Some(TapeSegment::Pulse(duration)) => {
                    self.ear = !self.ear;
                    self.remaining = duration;
                }
                Some(TapeSegment::Pause(duration)) => {
                    self.ear = false;
                    self.remaining = duration;
                }
                None => {
                    self.ear = false;
                    self.playing = false;
                    return false;
                }
            }
        }

        self.remaining -= 1;
        self.ear
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{Tap, TapePlayer, TapeSegment, DATA_PILOT_PULSES, HEADER_PILOT_PULSES};

    #[test]
    fn rejects_truncated_block_headers() {
        assert!(Tap::read_default_block(&[], 0).is_err());
        assert!(Tap::read_default_block(&[2], 0).is_err());
    }

    #[test]
    fn rejects_blocks_larger_than_the_file() {
        assert!(Tap::read_default_block(&[4, 0, 0, 0], 0).is_err());
    }

    #[test]
    fn accepts_a_complete_block() {
        let block = Tap::read_default_block(&[2, 0, 0, 0], 0).unwrap();
        assert_eq!(block.range, 2..4);
    }

    #[test]
    fn tape_player_uses_header_and_data_pilot_lengths() {
        let header = TapePlayer::from_blocks([vec![0, 0]]);
        let data = TapePlayer::from_blocks([vec![0xff, 0]]);

        assert_eq!(
            header
                .segments
                .iter()
                .take_while(|segment| **segment == TapeSegment::Pulse(2_168))
                .count(),
            HEADER_PILOT_PULSES
        );
        assert_eq!(
            data.segments
                .iter()
                .take_while(|segment| **segment == TapeSegment::Pulse(2_168))
                .count(),
            DATA_PILOT_PULSES
        );
    }

    #[test]
    fn tape_player_holds_each_pulse_for_its_tstate_duration() {
        let mut player = TapePlayer {
            segments: VecDeque::from([TapeSegment::Pulse(3), TapeSegment::Pulse(2)]),
            remaining: 0,
            ear: false,
            playing: true,
        };

        assert_eq!(
            (0..6).map(|_| player.tick()).collect::<Vec<_>>(),
            vec![true, true, true, false, false, false]
        );
        assert!(!player.is_playing());
    }
}
