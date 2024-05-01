use std::fs::File;
use std::io::Read;
use std::path::Path;

struct LoopBlock {
    id: u8,
    count: i32,
    blocks: Vec<Box<dyn Block>>,
}

struct LoopEndBlock {
    id: u8,
}

struct DataBlock {
    id: u8,
    flag: u8,
    range: std::ops::Range<usize>,
    pilot: u32,
    pilot_len: u32,
    sync1: u32,
    sync2: u32,
    zero: u32,
    one: u32,
    pause: u32,
    last_byte_len: i8,
}

struct PulseSeqBlock {
    id: u8,
    pulses: Vec<u32>,
}

trait Block {}

impl Block for DataBlock {}
impl Block for PulseSeqBlock {}

pub struct Tap {
    blocks: Vec<DataBlock>,
    actual_block: usize,
    data: Vec<u8>,
    pub name: String,
}

impl Tap {
    pub fn new(url: &Path) -> Result<Self, std::io::Error> {
        let mut file = File::open(url)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let header = String::from_utf8_lossy(&data[0..7]);
        if header == "ZXTape!" {
            unimplemented!();
        } else {
            let mut start = 0;
            let mut blocks = Vec::new();
            loop {
                let block = Self::read_default_block(&data, start);
                start = block.range.end;
                blocks.push(block);
                if data.len() <= start {
                    break;
                }
            }
            Ok(Self {
                blocks,
                actual_block: 0,
                data,
                name: String::from(url.to_str().unwrap()),
            })
        }
    }

    pub fn next_block(&mut self) -> Option<Vec<u8>> {
        if self.actual_block >= self.blocks.len() {
            return None;
        }
        let block = &self.blocks[self.actual_block];
        self.actual_block += 1;
        Some(self.data[block.range.clone()].to_vec())
    }

    fn read_default_block(data: &[u8], start: usize) -> DataBlock {
        let length = (data[start] as usize) | ((data[start + 1] as usize) << 8);
        let flag = data[start + 0x02];
        let pilot_len = if flag > 128 { 3223 } else { 8063 };

        DataBlock {
            id: data[start],
            flag,
            range: start + 2..start + length + 2,
            pilot: 2168,
            pilot_len,
            sync1: 667,
            sync2: 735,
            zero: 855,
            one: 1710,
            pause: 3000,
            last_byte_len: 8,
        }
    }

    fn read_tzx_block(data: &[u8]) -> (Box<dyn Block>, usize) {
        unimplemented!(); // Implement this method
    }
}
