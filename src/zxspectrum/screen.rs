use std::sync::{
    mpsc::{Receiver, Sender},
    Arc, Mutex,
};

use minifb::{Key, Window, WindowOptions};

use super::ula::{HEIGHT, WIDTH};

pub struct Screen {
    window: Window,
    bitmaps: [Arc<Mutex<Vec<u32>>>; 2],
    keyboard_sender: Sender<Vec<Key>>,
    redraw_receiver: Receiver<usize>,
}

impl Screen {
    pub fn new(
        bitmaps: [Arc<Mutex<Vec<u32>>>; 2],
        keyboard_sender: Sender<Vec<Key>>,
        redraw_receiver: Receiver<usize>,
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
            bitmaps,
            keyboard_sender,
            redraw_receiver,
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.redraw_receiver.try_recv() {
                Ok(buffer) => {
                    self.window
                        .update_with_buffer(&self.bitmaps[buffer].lock().unwrap(), WIDTH, HEIGHT)
                        .unwrap();
                    self.window.update();
                }
                Err(_) => {}
            }
            let keys = self.window.get_keys();
            self.keyboard_sender
                .send(keys)
                .expect("Something went wrong");
        }
    }
}
