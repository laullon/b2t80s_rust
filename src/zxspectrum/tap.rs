use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::Sender;

use rfd::FileDialog;

use crate::z80::registers::Registers;

use super::zx48k::MachineMessage;

struct LoopBlock {
    id: u8,
    count: i32,
    blocks: Vec<Box<dyn Block>>,
}

struct LoopEndBlock {
    id: u8,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Tap {
    blocks: Vec<DataBlock>,
    actual_block: usize,
    data: Vec<u8>,
    pub name: String,
}

impl Tap {
    pub(crate) async fn load() -> Result<Tap, &'static str> {
        let path: std::path::PathBuf = env::current_dir().unwrap();
        let file = FileDialog::new()
            .add_filter("tap", &["tap"])
            .set_directory(path)
            .pick_file();

        match file {
            Some(path) => match Tap::new(&path) {
                Ok(tap) => {
                    println!("Successfully loaded TAP file: {}", tap.name);
                    Ok(tap)
                }
                Err(err) => {
                    format!("Error loading TAP file: {}", err);
                    Err("Error loading TAP file")
                }
            },
            None => Err("No TAP file selected"),
        }
    }

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

    // fn read_tzx_block(data: &[u8]) -> (Box<dyn Block>, usize) {
    //     unimplemented!(); // Implement this method
    // }

    pub fn load_tap_block(&mut self, regs: Registers, machine_ctl_tx: Sender<MachineMessage>) {
        let data = self
            .next_block()
            .map(|block| block.to_vec())
            .unwrap_or_else(Vec::new);
        if data.is_empty() {
            return;
        }

        let requested_length = regs.de();
        let start_address = regs.ix();
        println!("Loading block to {:04x} ({})", start_address, data.len());

        let a = data[0];
        println!("{} == {} : {}", regs.a_alt, a, regs.a_alt == a);
        println!("requestedLength: {}", requested_length);
        if regs.a_alt == a {
            if regs.f_alt.c {
                let mut checksum = data[0];
                for i in 0..(requested_length as usize) {
                    let loaded_byte = data[i + 1];
                    // self.mem_write(start_address.wrapping_add(i as u16), loaded_byte);
                    checksum ^= loaded_byte;
                }

                if start_address == 0x4000 {}

                println!(
                    "{} == {} : {}",
                    checksum,
                    data[requested_length as usize + 1],
                    checksum == data[requested_length as usize + 1]
                );
                // regs.f.c = true;
            } else {
                // regs.f.c = true;
            }
            println!("done");
        } else {
            // regs.f.c = false;
            println!("BAD Block");
        }

        // regs.pc = 0x05e2;
        machine_ctl_tx
            .send(MachineMessage::CPUSetRegisters(regs))
            .unwrap();
        machine_ctl_tx.send(MachineMessage::CPUResume).unwrap();
        println!("Done\n--------");
    }
}
