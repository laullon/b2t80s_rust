use b2t80s_rust::zxspectrum::{
    ula::{SCREEN_HEIGHT, SCREEN_WIDTH, SRC_SIZE},
    zx48k::{MachineMessage, UICommands, Zx48k},
};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, FromSample, Sample, Stream, StreamConfig,
};
use iced::{
    event,
    futures::{
        channel::mpsc::{self, channel, Sender},
        SinkExt, StreamExt,
    },
    keyboard::Event as KeyEvent,
    subscription,
    widget::{button, column, container, image, row, text, tooltip, Image},
    Alignment, Command, ContentFit, Element, Event, Length, Subscription,
};
use std::{
    panic, process,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::task;

fn main() -> iced::Result {
    // take_hook() returns the default hook in case when a custom one is not set
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    iced::program("ZX Spectrum 48K", UI::update, UI::view)
        .subscription(UI::subscription)
        .run()
}

/* ********************************************* */

#[derive(Debug, Clone)]

enum Message {
    Ready(Sender<UICommands>),
    SetBuffer(usize),
    KeyEvent(KeyEvent),
}

enum State {
    Starting,
    Ready(mpsc::Receiver<UICommands>),
}

struct UI {
    bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
    buffer: usize,
    machine_ctl_tx: Option<Sender<MachineMessage>>,
    event_tx: Option<Sender<KeyEvent>>,
    fps: FPSCounter,
    stream: Option<Stream>,
}

struct FPSCounter {
    last_frame: Instant,
    frame_count: u32,
    fps: f32,
}

impl FPSCounter {
    fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            frame_count: 0,
            fps: 0.0,
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.frame_count += 1;
        let duration = now.duration_since(self.last_frame).as_secs_f32();

        if duration >= 1.0 {
            self.fps = self.frame_count as f32 / duration;
            self.frame_count = 0;
            self.last_frame = now;
        }
    }
}

impl Default for UI {
    fn default() -> Self {
        let bitmap: Vec<u8> = vec![0; SRC_SIZE * 4];
        let scr_bitmap = Arc::new(Mutex::new(bitmap));

        let bitmap_2: Vec<u8> = vec![0; SRC_SIZE * 4];
        let scr_bitmap_2 = Arc::new(Mutex::new(bitmap_2));

        Self {
            bitmaps: [scr_bitmap, scr_bitmap_2],
            buffer: 0,
            machine_ctl_tx: None,
            event_tx: None,
            fps: FPSCounter::new(),
            stream: None,
        }
    }
}

impl UI {
    pub fn update(&mut self, msg: Message) -> Command<Message> {
        self.fps.tick();
        match (msg, self.event_tx.as_mut()) {
            (Message::Ready(sender), _) => {
                let (event_tx, event_rx) = channel::<KeyEvent>(10);
                let (machine_ctl_tx, machine_ctl_rx) = channel::<MachineMessage>(0);

                let (stream, sound_tx) = match SoundEngine::init_engine() {
                    Ok((stream, sound_tx)) => match stream.play() {
                        Ok(_) => (stream, sound_tx),
                        Err(e) => panic!("Failed to init sound: {}", e),
                    },
                    Err(e) => panic!("Failed to init sound: {}", e),
                };
                self.stream = Some(stream);

                let mut zx = Zx48k::new(
                    [self.bitmaps[0].clone(), self.bitmaps[1].clone()],
                    event_rx,
                    machine_ctl_rx,
                    machine_ctl_tx.clone(),
                    sender.clone(),
                    sound_tx,
                );

                self.machine_ctl_tx = Some(machine_ctl_tx.clone());
                self.event_tx = Some(event_tx.clone());

                task::spawn(async move {
                    zx.run().await;
                });
            }
            (Message::SetBuffer(b), _) => {
                self.buffer = b;
            }
            (Message::KeyEvent(e), Some(tx)) => tx.start_send(e).unwrap(),
            _ => (),
        }

        Command::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let screen = image::Handle::from_rgba(
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
            self.bitmaps[0].lock().unwrap().clone(),
        );

        let screen = Image::<image::Handle>::new(screen)
            .filter_method(image::FilterMethod::Nearest)
            .content_fit(ContentFit::Cover)
            .width(Length::Fill)
            .height(Length::Fill);

        let controls = row![action(text("Reset"), "Reset", None),]
            .spacing(10)
            .align_items(Alignment::Center);

        let content = column![controls, screen, text(format!("FPS: {:.2}", self.fps.fps))]
            .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            self.some_worker(),
            event::listen_with(|event, _| match event {
                Event::Keyboard(e) => Some(e),
                _ => None,
            })
            .map(Message::KeyEvent),
        ])
    }

    fn some_worker(&self) -> Subscription<Message> {
        struct SomeWorker;
        subscription::channel(
            std::any::TypeId::of::<SomeWorker>(),
            100,
            |mut output| async move {
                let mut state = State::Starting;
                loop {
                    match &mut state {
                        State::Starting => {
                            let (sender, receiver) = mpsc::channel(100);
                            let _ = output.send(Message::Ready(sender)).await;
                            state = State::Ready(receiver);
                        }
                        State::Ready(receiver) => {
                            let input = receiver.next().await;
                            match input {
                                Some(UICommands::DrawBuffer(b)) => {
                                    let _ = output.send(Message::SetBuffer(b)).await;
                                }
                                None => unreachable!(),
                            }
                        }
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

struct SoundEngine {}

impl SoundEngine {
    fn init_engine() -> anyhow::Result<(Stream, std::sync::mpsc::Sender<f32>)> {
        let host: cpal::Host = cpal::default_host();
        let device: cpal::Device = host
            .default_output_device()
            .expect("failed to find output device");
        println!("Output device: {}", device.name()?);

        let config = device.default_output_config().unwrap();
        println!("Default output config: {:?}", config);
        println!("Default sample_format {:?}", config.sample_format());

        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        let channels = config.channels() as usize;

        let (tx, rx) = std::sync::mpsc::channel::<f32>();
        let mut next_value = move || {
            let recv = rx.recv();
            recv.ok().unwrap()
        };

        let mut config: StreamConfig = StreamConfig::from(config);
        config.sample_rate = cpal::SampleRate(35000);
        config.buffer_size = BufferSize::Fixed(1024);
        println!("config: {:?}", config);

        let stream = device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                Self::write_data(data, channels, &mut next_value)
            },
            err_fn,
            None,
        )?;
        Ok((stream, tx))
    }

    fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
    where
        T: Sample + FromSample<f32>,
    {
        for frame in output.chunks_mut(channels) {
            let value = next_sample();
            let value: T = T::from_sample(value);
            for sample in frame.iter_mut() {
                *sample = value;
            }
        }
    }
}
