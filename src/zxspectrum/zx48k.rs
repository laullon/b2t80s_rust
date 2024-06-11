use iced::futures::channel::mpsc::{Receiver, Sender};
use rfd::FileDialog;
use tokio::task;

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{env, fs::File, io::Read};

use crate::signals::SignalReq;
use crate::z80::cpu::CPU;
use crate::z80::registers::Registers;

use super::tap::Tap;
use super::ula::ULA;

use iced::keyboard::Event as KeyEvent;

#[derive(Debug)]
pub enum MachineMessage {
    CPUWait,
    CPUResume,
    CPUSetRegisters(Registers),
    Reset,
    TapLoad(std::path::PathBuf),
}

#[derive(Debug)]
enum TapState {
    Empty,
    Loading,
    Ready,
}

pub struct Zx48k {
    memory: [[u8; 0x4000]; 4],

    cpu: CPU,
    ula: ULA,

    tap: Option<Tap>,
    tap_state: TapState,

    machine_ctl_rx: Receiver<MachineMessage>,
    machine_ctl_tx: Sender<MachineMessage>,
}

// todo: review, and move out
#[derive(Debug)]
pub enum UICommands {
    DrawBuffer(usize),
}

impl Zx48k {
    pub fn new(
        bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
        event_rx: Receiver<KeyEvent>,
        machine_ctl_rx: Receiver<MachineMessage>,
        machine_ctl_tx: Sender<MachineMessage>,
        ui_ctl_tx: Sender<UICommands>,
        sound_tx: mpsc::Sender<f32>,
    ) -> Self {
        Self {
            memory: [load_rom(), [0; 0x4000], [0; 0x4000], [0; 0x4000]],
            cpu: CPU::new(),
            ula: ULA::new(bitmaps, event_rx, ui_ctl_tx.clone(), sound_tx),
            machine_ctl_rx,
            machine_ctl_tx,
            tap: None,
            tap_state: TapState::Empty,
        }
    }

    pub async fn run(self: &mut Self) -> ! {
        println!("Zx48k::run()");
        let mut interval = tokio::time::interval(Duration::from_millis(20));
        loop {
            interval.tick().await;
            // let t = std::time::Instant::now();
            for _ in 0..(3_500_000 / 50) {
                self.ula.tick();
                self.bus_tick();
                self.ula.tick();
                self.bus_tick();
                if !(self.ula.content && (self.cpu.signals.addr & 0xc000 == 0x4000)) {
                    let trap = self.cpu.tick();
                    self.bus_tick();

                    match trap {
                        Some(0x056B) => {
                            // println!("Trap 0x056B - load tap block - {:?}", self.tap_state);
                            self.ula.clean_keyboard();

                            match self.tap_state {
                                TapState::Empty => {
                                    self.tap_state = TapState::Loading;
                                    load_tap_file(self.machine_ctl_tx.clone());
                                }
                                TapState::Loading => (),
                                TapState::Ready => self.load_tap_block(),
                            }
                        }
                        _ => {}
                    }
                }
            }

            match self.machine_ctl_rx.try_next() {
                Ok(msg) => match msg {
                    Some(MachineMessage::CPUWait) => self.cpu.wait = true,
                    Some(MachineMessage::CPUResume) => self.cpu.wait = false,
                    Some(MachineMessage::Reset) => self.reset(),
                    Some(MachineMessage::CPUSetRegisters(_)) => todo!(),
                    Some(MachineMessage::TapLoad(file)) => {
                        self.tap = Some(Tap::new(&file).unwrap());
                        self.tap_state = TapState::Ready;
                    }
                    None => (),
                    // _ => unreachable!("Invalid machine message"),
                },
                Err(_) => {}
            }
            // println!("t: {}ms", t.elapsed().as_millis());
        }
    }

    fn reset(self: &mut Self) {
        self.cpu.do_reset = true;
        self.tap = None;
        self.tap_state = TapState::Empty;
    }

    fn mem_read(self: &mut Self, addr: u16) -> u8 {
        let bank: usize = (addr >> 14) as usize;
        let addr = (addr & 0x3fff) as usize;
        let data = self.memory[bank][addr];
        // println!("\tMR {:04x} {:02x}", signals.addr, signals.data)
        data
    }

    fn mem_write(self: &mut Self, addr: u16, data: u8) {
        let bank = (addr >> 14) as usize;
        let addr = (addr & 0x3fff) as usize;
        if bank != 0 {
            self.memory[bank][addr] = data;
            // println!("\tMW {:04x} {:02x}", signals.addr, signals.data)
        }
    }

    fn bus_tick(self: &mut Self) {
        match self.cpu.signals.mem {
            SignalReq::Read => self.cpu.signals.data = self.mem_read(self.cpu.signals.addr),
            SignalReq::Write => self.mem_write(self.cpu.signals.addr, self.cpu.signals.data),
            SignalReq::None => (),
        }

        match self.ula.signals.mem {
            SignalReq::Read => self.ula.signals.data = self.mem_read(self.ula.signals.addr),
            SignalReq::Write => self.mem_write(self.ula.signals.addr, self.ula.signals.data),
            SignalReq::None => (),
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

    fn load_tap_block(&mut self) {
        let data: Vec<u8> = match self.tap.as_mut() {
            Some(tap) => tap
                .next_block()
                .map(|block| block.to_vec())
                .unwrap_or_else(Vec::new),
            None => {
                println!("TAP file not loaded, returning empty vector");
                Vec::new()
            }
        };
        if data.is_empty() {
            return;
        }

        let requested_length = self.cpu.regs.de();
        let start_address = self.cpu.regs.ix();
        println!("Loading block to {:04x} ({})", start_address, data.len());

        self.cpu.wait = true;
        let a = data[0];
        if self.cpu.regs.a_alt == a {
            if self.cpu.regs.f_alt.c {
                let mut checksum = data[0];
                for i in 0..(requested_length as usize) {
                    let loaded_byte = data[i + 1];
                    self.mem_write(start_address.wrapping_add(i as u16), loaded_byte);
                    checksum ^= loaded_byte;
                }

                if start_address == 0x4000 {}

                println!(
                    "{} == {} : {}",
                    checksum,
                    data[requested_length as usize + 1],
                    checksum == data[requested_length as usize + 1]
                );
                self.cpu.regs.f.c = true;
            } else {
                self.cpu.regs.f.c = true;
            }
            println!("done");
        } else {
            self.cpu.regs.f.c = false;
            println!("BAD Block");
        }

        self.cpu.regs.pc = 0x05e2;
        self.cpu.wait = false;
    }
}

fn load_tap_file(mut machine_ctl_tx: Sender<MachineMessage>) {
    let _ = task::spawn(async move {
        let path: std::path::PathBuf = env::current_dir().unwrap();
        let file: Option<_> = FileDialog::new()
            .add_filter("tap", &["tap"])
            .set_directory(path)
            .pick_file();
        match file {
            Some(f) => machine_ctl_tx
                .start_send(MachineMessage::TapLoad(f))
                .unwrap(),
            None => machine_ctl_tx.start_send(MachineMessage::Reset).unwrap(),
        }
    });
}

fn load_rom() -> [u8; 0x4000] {
    let mut path = env::current_dir().unwrap().join("bin");
    // path = path.join("DiagROMv.171.rom");
    path = path.join("48.rom");

    let mut f = File::open(&path).expect("Failed to open ROM file");
    let mut rom = [0; 0x4000];
    f.read_exact(&mut rom).expect("Failed to read ROM file");

    rom
}
