use std::sync::{mpsc::Sender, Arc, Mutex};

use minifb::{Window, WindowOptions};

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
        match self.signals.port {
            SignalReq::Read => todo!(),
            SignalReq::Write => self.write_port(self.signals.addr, self.signals.data),
            SignalReq::None => (),
        }

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

        self.bitmap[self.col + (self.row * WIDTH)] = self.data.remove(0);

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
        // }
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
    }

    fn read_port(&self, port: u16) -> (u8, bool) {
        if port & 0xff == 0xfe {
            let mut data = 0b00011111;
            let read_row = port >> 8;
            for row in 0..8 {
                if (read_row & (1 << row)) == 0 {
                    data &= self.keyboard_row[row];
                }
            }
            if self.ear_active && self.ear {
                data |= 0b11100000;
            } else {
                data |= 0b10100000;
            }
            return (data, false);
        }
        (self.floating_bus, false)
    }

    fn write_port(&mut self, port: u16, data: u8) {
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
