use std::{env, fs::File, io::Read};

use crate::{signals::SignalReq, z80::cpu::CPU};

use super::ula::ULA;

pub struct Machine {
    memory: [[u8; 0x4000]; 4],

    cpu: CPU,
    ula: ULA,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            memory: [load_rom(), [0; 0x4000], [0; 0x4000], [0; 0x4000]],
            cpu: CPU::new(),
            ula: ULA::new(),
        }
    }

    pub fn run(self: &mut Self) {
        loop {
            self.ula.tick();
            self.bus_tick();
            self.ula.tick();
            self.bus_tick();
            self.cpu.tick();
            self.bus_tick();
        }
    }

    fn bus_tick(self: &mut Self) {
        for (signals, idx) in [(&mut self.ula.signals, 0), (&mut self.cpu.signals, 1)] {
            let bank = (signals.addr >> 14) as usize;
            let addr = (signals.addr & 0x3fff) as usize;
            match signals.mem {
                SignalReq::Read => {
                    signals.data = self.memory[bank][addr];
                    // println!("\tMR {:04x} {:02x}", signals.addr, signals.data)
                }
                SignalReq::Write => {
                    // assert_ne!(
                    //     bank, 0,
                    //     "bank 0 write - {:04x} {:02x} - {} - pc: {:04x}",
                    //     signals.addr, signals.data, idx, self.cpu.regs.pc
                    // );
                    if bank != 0 {
                        self.memory[bank][addr] = signals.data;
                        // println!("\tMW {:04x} {:02x}", signals.addr, signals.data)
                    }
                }
                SignalReq::None => (),
            }
        }

        match self.cpu.signals.port {
            SignalReq::Read => {
                if self.cpu.signals.addr & 0x00e0 == 0x0000 {
                    //  Kempston joystick
                    self.cpu.signals.data = 0x00;
                } else if self.cpu.signals.addr & 0x0001 == 0x0000 {
                    // ULA
                    self.cpu.signals.data = self.ula.read_port(self.cpu.signals.addr);
                } else {
                    self.cpu.signals.data = 0xff;
                    // println!(
                    //     "port read - {:04x} ({:016b}) - pc: {:04x}",
                    //     self.cpu.signals.addr, self.cpu.signals.addr, self.cpu.regs.pc
                    // );
                }
            }
            SignalReq::Write => {
                self.ula.signals.addr = self.cpu.signals.addr;
                self.ula.signals.data = self.cpu.signals.data;
                self.ula.signals.port = SignalReq::Write;

                if self.cpu.signals.addr & 0x0001 == 0x0000 {
                    // ULA
                    self.ula
                        .write_port(self.cpu.signals.addr, self.cpu.signals.data);
                } else {
                    // println!(
                    //     "port write - {:04x} ({:016b}) - pc: {:04x}",
                    //     self.cpu.signals.addr, self.cpu.signals.addr, self.cpu.regs.pc
                    // );
                }
            }
            SignalReq::None => (),
        }
        self.cpu.signals.interrupt = self.ula.signals.interrupt;
    }
}

fn load_rom() -> [u8; 0x4000] {
    let mut path = env::current_dir().unwrap().join("bin");
    // path = path.join("DiagROMv.171.rom");
    path = path.join("48.rom");

    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            panic!("error!! {}", err);
        }
    };
    let mut zexdoc = Vec::new();
    match f.read_to_end(&mut zexdoc) {
        Ok(_) => (),
        Err(err) => {
            panic!("error!! {}", err);
        }
    };

    let mut rom = vec![0; 0];
    rom.extend_from_slice(&zexdoc);
    rom.try_into().unwrap()
}
