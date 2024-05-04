use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use minifb::{Key, Window, WindowOptions};

use super::ula::{Redraw, HEIGHT, WIDTH};

pub struct Screen {
    window: Window,
    bitmap: Arc<Mutex<Vec<u32>>>,
    keyboard_sender: Sender<Vec<Key>>,
    redraw_receiver: Receiver<Redraw>,
}

impl Screen {
    pub fn new(
        bitmap: Arc<Mutex<Vec<u32>>>,
        keyboard_sender: Sender<Vec<Key>>,
        redraw_receiver: Receiver<Redraw>,
    ) -> Self {
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
            redraw_receiver,
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.redraw_receiver.try_recv() {
                Ok(_) => {
                    self.window
                        .update_with_buffer(&self.bitmap.lock().unwrap(), WIDTH, HEIGHT)
                        .unwrap();
                }
                Err(_) => {}
            }
            self.window.update();
            let keys = self.window.get_keys();
            self.keyboard_sender.send(keys).unwrap();
        }
    }
}
