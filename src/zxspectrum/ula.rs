use iced::keyboard::{key::Named, Event as KeyEvent, Key};

use crate::signals::{SignalReq, Signals};
use iced::futures::channel::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};

use super::zx48k::UICommands;

pub const SRC_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT + 1;

const WIDTH: usize = 448;
const HEIGHT: usize = 312;

pub const SCREEN_WIDTH: usize = 256 + (SCREEN_BORDER * 2);
pub const SCREEN_HEIGHT: usize = 192 + (SCREEN_BORDER * 2);
const SCREEN_BORDER: usize = 48;

#[derive(Debug)]
pub struct SomeError; // No fields.

const PALETTE: [u32; 16] = [
    0x000000ff, 0x2030c0ff, 0xc04010ff, 0xc040c0ff, 0x40b010ff, 0x50c0b0ff, 0xe0c010ff, 0xc0c0c0ff,
    0x000000ff, 0x3040ffff, 0xff4030ff, 0xff70f0ff, 0x50e010ff, 0x50e0ffff, 0xffe850ff, 0xffffffff,
];

pub struct ULA {
    keyboard_row: [u8; 8],
    border_colour: u32,
    frame: u8,
    col: usize,
    row: usize,
    floating_bus: u8,
    ear: bool,
    ear_active: bool,
    buzzer: u8,
    sound: mpsc::Sender<f32>,
    sound_frame: u8,
    screen_data: u8,
    attr_data: u8,
    screen_data_2: u8,
    attr_data_2: u8,
    pub content: bool,
    ts: usize,

    pub signals: Signals,

    data: Vec<u32>,
    bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
    buffer: usize,
    event_rx: Receiver<KeyEvent>,
    ui_ctl_tx: Sender<UICommands>,
}

pub enum ULASignal {
    REDRAW,
}

impl ULA {
    pub fn new(
        bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
        event_rx: Receiver<KeyEvent>,
        ui_ctl_tx: Sender<UICommands>,
        sound_tx: mpsc::Sender<f32>,
    ) -> Self {
        ULA {
            // listener: None,
            // cpu,
            keyboard_row: [0; 8],
            border_colour: 0,
            frame: 0,
            col: 0,
            row: 0,
            floating_bus: 0,
            ear: false,
            ear_active: false,
            buzzer: 0,
            sound: sound_tx,
            sound_frame: 0,
            screen_data: 0,
            attr_data: 0,
            screen_data_2: 0,
            attr_data_2: 0,
            content: false,
            ts: 0,

            signals: Signals::default(),

            bitmaps,
            data: vec![0; 8],
            buffer: 0,
            event_rx,
            ui_ctl_tx,
        }
    }

    fn get_attr_addr(&self) -> u16 {
        let mut attr_addr = 0x5800;
        attr_addr |= (self.row & 0b11111000) << 2;
        attr_addr |= (self.col & 0b11111000) >> 3;
        attr_addr.try_into().unwrap()
    }

    fn get_screen_addr(&self) -> u16 {
        let mut addr = 0x4000;
        addr |= (self.row & 0b11000000) << 5;
        addr |= (self.row & 0b00000111) << 8;
        addr |= (self.row & 0b00111000) << 2;
        addr |= (self.col & 0b11111000) >> 3;
        addr.try_into().unwrap()
    }

    pub fn tick(&mut self) {
        self.sound_frame += 1;
        if self.sound_frame == 200 {
            self.sound_frame = 0;
            let t = -0.05 + ((self.buzzer as f32) * 0.1);
            match self.sound.send(t) {
                Ok(_) => (),
                Err(e) => println!("send error: {}", e),
            }
        }

        let in_screen = (0..256).contains(&self.col) && (0..192).contains(&self.row);
        self.content = in_screen;

        if in_screen {
            self.floating_bus = self.signals.data;
            match self.ts % 16 {
                0 => {
                    self.signals.addr = self.get_screen_addr();
                    self.signals.mem = SignalReq::Read
                }
                1 => {
                    self.screen_data = self.signals.data;
                    self.signals.mem = SignalReq::None
                }

                2 => {
                    self.signals.addr = self.get_attr_addr();
                    self.signals.mem = SignalReq::Read
                }
                3 => {
                    self.attr_data = self.signals.data;
                    self.signals.mem = SignalReq::None
                }

                4 => {
                    self.signals.addr = self.get_screen_addr() + 1;
                    self.signals.mem = SignalReq::Read
                }
                5 => {
                    self.screen_data_2 = self.signals.data;
                    self.signals.mem = SignalReq::None
                }

                6 => {
                    self.signals.addr = self.get_attr_addr() + 1;
                    self.signals.mem = SignalReq::Read
                }
                7 => {
                    self.attr_data_2 = self.signals.data;
                    self.signals.mem = SignalReq::None
                }

                8 => {
                    let colors = self.get_pixels_colors(self.attr_data, self.screen_data);
                    self.data.append(colors.to_vec().as_mut());
                    let colors = self.get_pixels_colors(self.attr_data_2, self.screen_data_2);
                    self.data.append(colors.to_vec().as_mut());
                }

                9 | 10 | 11 => {}

                12 | 13 | 14 | 15 => {
                    self.content = false;
                }
                _ => panic!("{} {}", self.ts, self.ts % 16),
            }
        } else {
            if self.ts % 16 == 6 || self.ts % 16 == 14 {
                // for i in 0..8 {
                // self.bitmap[self.col + i + (self.row * WIDTH)] = self.border_colour;
                self.data.append([self.border_colour; 8].to_vec().as_mut());
                // }
            }
        }

        self.ts += 1;

        let d = self.data.remove(0).to_be_bytes();
        if let Ok((x, y)) = self.get_xy(self.col, self.row) {
            let mut bm = self.bitmaps[self.buffer].lock().unwrap();
            let idx = (x + (y * SCREEN_WIDTH)) * 4;
            bm[idx + 0] = d[0];
            bm[idx + 1] = d[1];
            bm[idx + 2] = d[2];
            bm[idx + 3] = d[3];
            drop(bm);
        }

        self.col += 1;
        if self.col == WIDTH {
            self.col = 0;
            self.row += 1;
            if self.row == HEIGHT {
                self.row = 0;
                self.frame_done();
                self.ts = 0;
            }
        }

        if self.row == (HEIGHT - 64) && self.col < 64 {
            self.signals.interrupt = true;
        } else {
            self.signals.interrupt = false;
        }

        match self.event_rx.try_next() {
            Ok(Some(e)) => self.on_key(e),
            _ => (),
            // Ok(Some(event)) => self.on_key(event),
            // Err(_) => (),
        }
    }

    fn get_xy(&self, col: usize, row: usize) -> Result<(usize, usize), SomeError> {
        let mut x = col + SCREEN_BORDER - 8;
        let mut y = row + SCREEN_BORDER;
        if x >= WIDTH {
            x -= WIDTH;
            y += 1;
        }
        if y >= HEIGHT {
            y -= HEIGHT;
        }

        if (x < SCREEN_WIDTH) && (y < SCREEN_HEIGHT) {
            return Ok((x, y));
        }
        Err(SomeError)
    }

    fn frame_done(&mut self) {
        self.ui_ctl_tx
            .start_send(UICommands::DrawBuffer(self.buffer))
            .unwrap();
        self.buffer = 1 - self.buffer;
    }

    pub fn read_port(&self, port: u16) -> u8 {
        if port & 0xff == 0xfe {
            let mut data = 0b00011111;
            let read_row = port >> 8;
            for row in 0..8 {
                if (read_row & (1 << row)) == 0 {
                    data ^= self.keyboard_row[row];
                    // println!(
                    //     "{:08b} - {:08b} - row:{}",
                    //     data, self.keyboard_row[row], row
                    // );
                }
            }
            if self.ear_active && self.ear {
                data |= 0b11100000;
            } else {
                data |= 0b10100000;
            }
            return data;
        }
        self.floating_bus
    }

    pub fn write_port(&mut self, port: u16, data: u8) {
        if port & 0xff == 0xfe {
            self.border_colour = PALETTE[data as usize & 0x07];
            self.buzzer = (data & 16) >> 4;
            self.ear_active = (data & 24) != 0;
        }
    }

    fn get_pixels_colors(&self, attr: u8, pixels: u8) -> [u32; 8] {
        let flash = attr & 0x80 == 0x80;
        let brg = (attr & 0x40) >> 6;
        let paper = PALETTE[((attr & 0x38) >> 3) as usize + (brg * 8) as usize];
        let ink = PALETTE[(attr & 0x07) as usize + (brg * 8) as usize];

        let mut colors = [0; 8];
        for b in 0..8 {
            let mut data = pixels;
            data <<= b;
            data &= 0b10000000;
            if data != 0 {
                colors[b] = PALETTE[0];
            } else {
                colors[b] = PALETTE[2];
            }
            if flash && (self.frame & 0x10 != 0) {
                if data != 0 {
                    colors[b] = paper;
                } else {
                    colors[b] = ink;
                }
            } else if data != 0 {
                colors[b] = ink;
            } else {
                colors[b] = paper;
            }
        }
        colors
    }

    fn on_key(&mut self, event: KeyEvent) {
        // println!("event: {:?}", event);
        let (key, pressed) = match event {
            KeyEvent::KeyPressed { key, .. } => (key, true),
            KeyEvent::KeyReleased { key, .. } => (key, false),
            KeyEvent::ModifiersChanged(_) => return,
        };

        match key.as_ref() {
            // Key::Unidentified => todo!(),
            Key::Character("1") => self.set_bit(3, 1, pressed),
            Key::Character("2") => self.set_bit(3, 2, pressed),
            Key::Character("3") => self.set_bit(3, 3, pressed),
            Key::Character("4") => self.set_bit(3, 4, pressed),
            Key::Character("5") => self.set_bit(3, 5, pressed),

            Key::Character("0") => self.set_bit(4, 1, pressed),
            Key::Character("9") => self.set_bit(4, 2, pressed),
            Key::Character("8") => self.set_bit(4, 3, pressed),
            Key::Character("7") => self.set_bit(4, 4, pressed),
            Key::Character("6") => self.set_bit(4, 5, pressed),

            Key::Character("q") => self.set_bit(2, 1, pressed),
            Key::Character("w") => self.set_bit(2, 2, pressed),
            Key::Character("e") => self.set_bit(2, 3, pressed),
            Key::Character("r") => self.set_bit(2, 4, pressed),
            Key::Character("t") => self.set_bit(2, 5, pressed),

            Key::Character("p") => self.set_bit(5, 1, pressed),
            Key::Character("o") => self.set_bit(5, 2, pressed),
            Key::Character("i") => self.set_bit(5, 3, pressed),
            Key::Character("u") => self.set_bit(5, 4, pressed),
            Key::Character("y") => self.set_bit(5, 5, pressed),

            Key::Character("a") => self.set_bit(1, 1, pressed),
            Key::Character("s") => self.set_bit(1, 2, pressed),
            Key::Character("d") => self.set_bit(1, 3, pressed),
            Key::Character("f") => self.set_bit(1, 4, pressed),
            Key::Character("g") => self.set_bit(1, 5, pressed),

            Key::Named(Named::Enter) => self.set_bit(6, 1, pressed),
            Key::Character("l") => self.set_bit(6, 2, pressed),
            Key::Character("k") => self.set_bit(6, 3, pressed),
            Key::Character("j") => self.set_bit(6, 4, pressed),
            Key::Character("h") => self.set_bit(6, 5, pressed),

            Key::Named(Named::Shift) => self.set_bit(0, 1, pressed),
            Key::Character("z") => self.set_bit(0, 2, pressed),
            Key::Character("x") => self.set_bit(0, 3, pressed),
            Key::Character("c") => self.set_bit(0, 4, pressed),
            Key::Character("v") => self.set_bit(0, 5, pressed),

            Key::Named(Named::Space) => self.set_bit(7, 1, pressed),
            Key::Named(Named::Alt) => self.set_bit(7, 2, pressed),
            Key::Character("m") => self.set_bit(7, 3, pressed),
            Key::Character("n") => self.set_bit(7, 4, pressed),
            Key::Character("b") => self.set_bit(7, 5, pressed),

            Key::Named(Named::ArrowUp) => {
                self.set_bit(0, 1, pressed);
                self.set_bit(4, 4, pressed);
            }
            Key::Named(Named::ArrowDown) => {
                self.set_bit(0, 1, pressed);
                self.set_bit(4, 5, pressed);
            }
            Key::Named(Named::ArrowLeft) => {
                self.set_bit(0, 1, pressed);
                self.set_bit(3, 5, pressed);
            }
            Key::Named(Named::ArrowRight) => {
                self.set_bit(0, 1, pressed);
                self.set_bit(4, 3, pressed);
            }
            Key::Named(Named::Backspace) => {
                self.set_bit(0, 1, pressed);
                self.set_bit(4, 1, pressed);
            }

            Key::Character(c) => println!("Unknown key: {}", c),
            _ => (),
        }
    }

    fn set_bit(&mut self, row: usize, bit: usize, set: bool) {
        let b = 1 << (bit - 1);
        if set {
            self.keyboard_row[row] |= b;
        } else {
            self.keyboard_row[row] &= !b;
        }
    }

    pub(crate) fn clean_keyboard(&mut self) {
        self.keyboard_row = [0; 8];
    }
}
