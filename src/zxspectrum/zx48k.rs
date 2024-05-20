use std::borrow::BorrowMut;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};

use std::sync::{Arc, Mutex};
use std::thread::{self};
use std::time::{Duration, Instant};
use std::{env, fs::File, io::Read};

use iced::futures::SinkExt;
use iced::widget::{button, column, container, image, row, text, tooltip, Image};
use iced::{event, subscription, Alignment, ContentFit, Element, Event, Length, Subscription};
use rfd::FileDialog;

use crate::{signals::SignalReq, z80::cpu::CPU};

use super::tap::Tap;
use super::ula::{SCREEN_HEIGHT, SCREEN_WIDTH, SRC_SIZE, ULA};

use iced::keyboard::Event as KeyEvent;

/* ********************************************* */

#[derive(Default, Debug)]
pub struct UISignals {
    pub active_buffer: AtomicUsize,
    pub do_reset: AtomicBool,
    pub frame_done: AtomicBool,
}

pub struct Zx48k {
    bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
    event_tx: Sender<KeyEvent>,
    ui_signals: Arc<UISignals>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick(),
    KeyEvent(KeyEvent),
    Reset,
}

impl Default for Zx48k {
    fn default() -> Self {
        let bitmap: Vec<u8> = vec![0; SRC_SIZE * 4];
        let ula_bitmap = Arc::new(Mutex::new(bitmap));
        let scr_bitmap = Arc::clone(&ula_bitmap);

        let bitmap_2: Vec<u8> = vec![0; SRC_SIZE * 4];
        let ula_bitmap_2 = Arc::new(Mutex::new(bitmap_2));
        let scr_bitmap_2 = Arc::clone(&ula_bitmap_2);

        let ui_signals = Arc::new(UISignals::default());
        let ui_signals_cloned = ui_signals.clone();

        let (event_tx, event_rx) = channel::<KeyEvent>();

        thread::spawn(move || {
            Bus::new([ula_bitmap, ula_bitmap_2], event_rx, ui_signals).run();
        });

        Self {
            bitmaps: [scr_bitmap, scr_bitmap_2],
            event_tx,
            ui_signals: ui_signals_cloned,
        }
    }
}

impl Zx48k {
    pub fn view(&self) -> Element<'_, Message> {
        let screen = image::Handle::from_rgba(
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
            self.bitmaps[self.ui_signals.active_buffer.load(Ordering::Relaxed)]
                .lock()
                .unwrap()
                .clone(),
        );

        let screen = Image::<image::Handle>::new(screen)
            .filter_method(image::FilterMethod::Nearest)
            .content_fit(ContentFit::Cover)
            .width(Length::Fill)
            .height(Length::Fill);

        let controls = row![action(text("Reset"), "Reset", Some(Message::Reset)),]
            .spacing(10)
            .align_items(Alignment::Center);

        let content = column![controls, screen].height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::Tick() => (),
            Message::KeyEvent(e) => self.event_tx.send(e).unwrap(),
            Message::Reset => self.ui_signals.do_reset.store(true, Ordering::Relaxed),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            self.ula_events(),
            // iced::time::every(std::time::Duration::from_millis(10)).map(|_| Message::Tick()),
            event::listen_with(|event, _| match event {
                Event::Keyboard(e) => Some(e),
                _ => None,
            })
            .map(Message::KeyEvent),
        ])
    }

    fn ula_events(&self) -> Subscription<Message> {
        struct SomeWorker;
        let signals = self.ui_signals.clone();
        subscription::channel(
            std::any::TypeId::of::<SomeWorker>(),
            0,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                loop {
                    if signals.frame_done.load(Ordering::Relaxed) {
                        signals.frame_done.store(false, Ordering::Relaxed);
                        let _ = output.send(Message::Tick()).await;
                    }
                }
            },
        )
    }
}

fn action<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    label: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let action = button(container(content));
    if let Some(on_press) = on_press {
        tooltip(
            action.on_press(on_press),
            label,
            tooltip::Position::FollowCursor,
        )
        .style(container::rounded_box)
        .into()
    } else {
        action.style(button::secondary).into()
    }
}

/* ********************************************* */

struct Bus {
    memory: [[u8; 0x4000]; 4],

    cpu: CPU,
    ula: ULA,

    tap: Option<Tap>,

    ui_signals: Arc<UISignals>,
}

impl Bus {
    pub fn new(
        bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
        event_rx: Receiver<KeyEvent>,
        ui_signals: Arc<UISignals>,
    ) -> Self {
        Self {
            memory: [load_rom(), [0; 0x4000], [0; 0x4000], [0; 0x4000]],
            cpu: CPU::new(),
            ula: ULA::new(bitmaps, event_rx, ui_signals.clone()),
            tap: None,
            ui_signals,
        }
    }

    fn run(self: &mut Self) {
        loop {
            let start_35: Instant = Instant::now();
            let mut total = Duration::new(0, 0);
            let chunks = 100;
            let max_duration = Duration::from_millis(1000 / chunks) - Duration::from_millis(2);
            for _ in 0..chunks {
                let start: Instant = Instant::now();
                for _ in 0..35000 {
                    self.ula.tick();
                    self.bus_tick();
                    self.ula.tick();
                    self.bus_tick();
                    if !(self.ula.content && (self.cpu.signals.addr & 0xc000 == 0x4000)) {
                        let trap = self.cpu.tick();
                        self.bus_tick();

                        match trap {
                            Some(0x056B) => {
                                self.ula.clean_keyboard();
                                match self.tap {
                                    Some(_) => self.load_tap_block(),
                                    None => self.load_tap_file(),
                                }
                            }
                            _ => {}
                        }
                    }

                    if self.ui_signals.do_reset.load(Ordering::Relaxed) {
                        self.cpu.do_reset = true;
                        self.tap = None;
                        self.ui_signals.do_reset.store(false, Ordering::Relaxed);
                    }
                }
                let used = start.elapsed();
                total += used;
                if used < max_duration {
                    thread::sleep(max_duration - used);
                }
            }
            println!("3.5MHz: {:?} ({:?})", total, start_35.elapsed());
        }
    }

    fn load_tap_file(self: &mut Self) {
        let path: std::path::PathBuf = env::current_dir().unwrap();
        let file = FileDialog::new()
            .add_filter("tap", &["tap"])
            .set_directory(path)
            .pick_file();

        match file {
            Some(path) => {
                self.tap = match Tap::new(&path) {
                    Ok(tap) => {
                        println!("Successfully loaded TAP file: {}", tap.name);
                        Some(tap)
                    }
                    Err(err) => {
                        eprintln!("Error loading TAP file: {}", err);
                        None
                    }
                };
            }
            None => self.cpu.do_reset = true,
        };
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
