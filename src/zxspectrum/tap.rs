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

#[cfg(test)]
mod tests {
    use super::Tap;

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
}
