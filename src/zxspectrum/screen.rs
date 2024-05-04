use std::sync::{
    mpsc::{self, Sender},
    Arc, Mutex,
};

use minifb::{Key, Window, WindowOptions};

use super::ula::{HEIGHT, WIDTH};

pub struct Screen {
    window: Window,
    bitmap: Arc<Mutex<Vec<u32>>>,
    keyboard_sender: Sender<Vec<Key>>,
}

impl Screen {
    pub fn new(bitmap: Arc<Mutex<Vec<u32>>>, keyboard_sender: Sender<Vec<Key>>) -> Self {
        let window = Window::new(
            "Test - ESC to exit",
            WIDTH * 3,
            HEIGHT * 3,
            WindowOptions::default(),
        )
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });
        Self {
            window,
            bitmap,
            keyboard_sender,
        }
    }

    pub fn run(&mut self) {
        loop {
            self.window
                .update_with_buffer(&self.bitmap.lock().unwrap(), WIDTH, HEIGHT)
                .unwrap();
            self.window.update();
            let keys = self.window.get_keys();
            self.keyboard_sender.send(keys).unwrap();
        }
    }
}
