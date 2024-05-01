use std::sync::{mpsc::Sender, Arc, Mutex};

use minifb::{Key, KeyRepeat, Window, WindowOptions};

use crate::signals::{SignalReq, Signals};

pub const SRC_SIZE: usize = WIDTH * WIDTH + 1;
pub const WIDTH: usize = 448;
pub const HEIGHT: usize = 312;
// pub const SCREEN_WIDTH: usize = 352;
// pub const SCREEN_HEIGHT: usize = 296;

const PALETTE: [u32; 16] = [
    0x00000000, 0x002030c0, 0x00c04010, 0x00c040c0, 0x0040b010, 0x0050c0b0, 0x00e0c010, 0x00c0c0c0,
    0x00000000, 0x003040ff, 0x00ff4030, 0x00ff70f0, 0x0050e010, 0x0050e0ff, 0x00ffe850, 0x00ffffff,
];

// trait ULAListener {
//     fn frame_done(&mut self, bitmap: &Bitmap);
// }

pub struct ULA {
    // listener: Option<Box<dyn ULAListener>>,
    // cpu: Z80,
    keyboard_row: [u8; 8],
    border_colour: u32,
    frame: u8,
    col: usize,
    row: usize,
    ts_per_row: usize,
    scanlines: usize,
    floating_bus: u8,
    ear: bool,
    ear_active: bool,
    buzzer: bool,
    // sound: SoundEngine,
    sound_frame: u8,
    screen_data: u8,
    attr_data: u8,
    screen_data_2: u8,
    attr_data_2: u8,
    content: bool,
    ts: usize,

    pub signals: Signals,

    window: Window,
    bitmap: Vec<u32>,
    data: Vec<u32>,
}

pub enum ULASignal {
    REDRAW,
}

impl ULA {
    pub fn new() -> Self {
        let bitmap: Vec<u32> = vec![0; WIDTH * HEIGHT];
        let window = Window::new(
            "Test - ESC to exit",
            WIDTH * 3,
            HEIGHT * 3,
            WindowOptions::default(),
        )
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

        ULA {
            // listener: None,
            // cpu,
            keyboard_row: [0; 8],
            border_colour: 0,
            frame: 0,
            col: 0,
            row: 0,
            ts_per_row: WIDTH / 2,
            scanlines: HEIGHT,
            floating_bus: 0,
            ear: false,
            ear_active: false,
            buzzer: false,
            // sound: SoundEngine::new(),
            sound_frame: 0,
            screen_data: 0,
            attr_data: 0,
            screen_data_2: 0,
            attr_data_2: 0,
            content: false,
            ts: 0,

            signals: Signals::default(),

            window,
            bitmap,
            data: vec![0; 8],
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
        if self.sound_frame == 50 {
            self.sound_frame = 0;
            // self.sound.tick(self.buzzer);
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

                9 | 10 | 11 | 12 | 13 => (),
                14 | 15 => {
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

        let (x, y) = self.get_xy(self.col, self.row);
        self.bitmap[x + (y * WIDTH)] = self.data.remove(0);

        self.col += 1;
        if self.col == WIDTH {
            self.col = 0;
            self.row += 1;
            if self.row == HEIGHT {
                self.row = 0;
                self.ts = 0;
                self.frame_done();
            }
        }

        if self.row == (HEIGHT - 64) && self.col < 64 {
            self.signals.interrupt = true;
        } else {
            self.signals.interrupt = false;
        }
    }

    fn get_xy(&self, col: usize, row: usize) -> (usize, usize) {
        let mut x = col + 24;
        let mut y = row + 48;
        if x >= WIDTH {
            x -= WIDTH;
            y += 1;
        }
        if y >= HEIGHT {
            y -= HEIGHT;
        }
        (x, y)
    }

    fn frame_done(&mut self) {
        self.window
            .update_with_buffer(&self.bitmap, WIDTH, HEIGHT)
            .unwrap();
        self.window.update();
        // let keys = self.window.get_keys_pressed(KeyRepeat::Yes);
        let keys = self.window.get_keys();
        // if !keys.is_empty() {
        //     println!("key: {:?} {}", keys, keys.len());
        // }
        self.keyboard_row = [0; 8];
        self.on_key(keys);
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
            self.buzzer = (data & 16) >> 4 != 0;
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

    fn on_key(&mut self, keys: Vec<Key>) {
        for key in keys {
            match key {
                Key::Key1 => self.set_bit(3, 1),
                Key::Key2 => self.set_bit(3, 2),
                Key::Key3 => self.set_bit(3, 3),
                Key::Key4 => self.set_bit(3, 4),
                Key::Key5 => self.set_bit(3, 5),

                Key::Key0 => self.set_bit(4, 1),
                Key::Key9 => self.set_bit(4, 2),
                Key::Key8 => self.set_bit(4, 3),
                Key::Key7 => self.set_bit(4, 4),
                Key::Key6 => self.set_bit(4, 5),

                Key::Q => self.set_bit(2, 1),
                Key::W => self.set_bit(2, 2),
                Key::E => self.set_bit(2, 3),
                Key::R => self.set_bit(2, 4),
                Key::T => self.set_bit(2, 5),

                Key::P => self.set_bit(5, 1),
                Key::O => self.set_bit(5, 2),
                Key::I => self.set_bit(5, 3),
                Key::U => self.set_bit(5, 4),
                Key::Y => self.set_bit(5, 5),

                Key::A => self.set_bit(1, 1),
                Key::S => self.set_bit(1, 2),
                Key::D => self.set_bit(1, 3),
                Key::F => self.set_bit(1, 4),
                Key::G => self.set_bit(1, 5),

                Key::Enter => self.set_bit(6, 1),
                Key::L => self.set_bit(6, 2),
                Key::K => self.set_bit(6, 3),
                Key::J => self.set_bit(6, 4),
                Key::H => self.set_bit(6, 5),

                Key::LeftShift | Key::RightShift => self.set_bit(0, 1),
                Key::Z => self.set_bit(0, 2),
                Key::X => self.set_bit(0, 3),
                Key::C => self.set_bit(0, 4),
                Key::V => self.set_bit(0, 5),

                Key::Space => self.set_bit(7, 1),
                Key::LeftAlt | Key::RightAlt => self.set_bit(7, 2),
                Key::M => self.set_bit(7, 3),
                Key::N => self.set_bit(7, 4),
                Key::B => self.set_bit(7, 5),

                Key::Up => {
                    self.set_bit(0, 1);
                    self.set_bit(4, 4);
                }
                Key::Down => {
                    self.set_bit(0, 1);
                    self.set_bit(4, 5);
                }
                Key::Left => {
                    self.set_bit(0, 1);
                    self.set_bit(3, 5);
                }
                Key::Right => {
                    self.set_bit(0, 1);
                    self.set_bit(4, 3);
                }
                Key::Backspace => {
                    self.set_bit(0, 1);
                    self.set_bit(4, 1);
                }
                _ => (),
            }
        }
    }

    fn set_bit(&mut self, row: usize, bit: usize) {
        let b = 1 << (bit - 1);
        self.keyboard_row[row] |= b;
    }
}

// fn getXY( col: usize, row:usize) -> (usize,usize) {
//     var x = col+24
//     var y = row+48
//     if x >= width {
//         x -= width
//         y += 1
//     }
//     if y >= height {
//         y -= height
//     }
//     return (x,y)
// }
