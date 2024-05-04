use std::borrow::BorrowMut;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;
use std::{env, fs::File, io::Read};

use minifb::Key;

use crate::{signals::SignalReq, z80::cpu::CPU};

use super::screen::Screen;
use super::tap::Tap;
use super::ula::{HEIGHT, ULA, WIDTH};

pub fn run() {
    let bitmap: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let ula_bitmap = Arc::new(Mutex::new(bitmap));
    let scr_bitmap = Arc::clone(&ula_bitmap);

    let bitmap_2: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let ula_bitmap_2 = Arc::new(Mutex::new(bitmap_2));
    let scr_bitmap_2 = Arc::clone(&ula_bitmap_2);

    let (keyboard_sender, keyboard_receiver) = channel::<Vec<Key>>();
    let (redraw_sender, redraw_receiver) = channel::<usize>();

    thread::spawn(move || {
        Bus::new([ula_bitmap, ula_bitmap_2], keyboard_receiver, redraw_sender).run();
    });

    Screen::new([scr_bitmap, scr_bitmap_2], keyboard_sender, redraw_receiver).run();
}

struct Bus {
    memory: [[u8; 0x4000]; 4],

    cpu: CPU,
    ula: ULA,

    tap: Option<Tap>,
}

impl Bus {
    pub fn new(
        bitmaps: [Arc<Mutex<Vec<u32>>>; 2],
        keyboard_receiver: Receiver<Vec<Key>>,
        redraw_sender: Sender<usize>,
    ) -> Self {
        let mut path: std::path::PathBuf = env::current_dir().unwrap().join("bin");
        // path = path.join("ulatest3.tap");
        path = path.join("ManicMiner.tap");

        let tap = match Tap::new(&path) {
            Ok(tap) => {
                println!("Successfully loaded TAP file: {}", tap.name);
                Some(tap)
            }
            Err(err) => {
                eprintln!("Error loading TAP file: {}", err);
                None
            }
        };

        Self {
            memory: [load_rom(), [0; 0x4000], [0; 0x4000], [0; 0x4000]],
            cpu: CPU::new(),
            ula: ULA::new(bitmaps, keyboard_receiver, redraw_sender),
            tap,
            // screen: Screen::new(scr_bitmap),
        }
    }

    pub fn run(self: &mut Self) {
        loop {
            let start = Instant::now();
            for _ in 0..3_500_000 {
                self.ula.tick();
                self.bus_tick();
                self.ula.tick();
                self.bus_tick();
                let trap = self.cpu.tick();
                self.bus_tick();

                match trap {
                    Some(0x056B) => self.load_data_block(),
                    _ => {}
                }
            }
            println!("3.5MHz: {:?}", start.elapsed());
        }
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

    fn load_data_block(&mut self) {
        let data: Vec<u8> = match self.tap.borrow_mut() {
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
            return; //emulator::CONTINUE
        }

        let requested_length = self.cpu.regs.de();
        let start_address = self.cpu.regs.ix();
        println!("Loading block to {:04x} ({})", start_address, data.len());

        self.cpu.wait = true;
        let a = data[0];
        println!(
            "{} == {} : {}",
            self.cpu.regs.a_alt,
            a,
            self.cpu.regs.a_alt == a
        );
        println!("requestedLength: {}", requested_length);
        if self.cpu.regs.a_alt == a {
            if self.cpu.regs.f_alt.c {
                let mut checksum = data[0];
                for i in 0..(requested_length as usize) {
                    let loaded_byte = data[i + 1];
                    self.mem_write(start_address.wrapping_add(i as u16), loaded_byte);
                    checksum ^= loaded_byte;
                }
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
        println!("Done\n--------");

        return;
    }
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
