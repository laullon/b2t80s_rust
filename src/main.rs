use b2t80s_rust::zxspectrum::{
    ula::{SCREEN_HEIGHT, SCREEN_WIDTH, SRC_SIZE},
    zx48k::{DebugSnapshot, MachineMessage, UICommands, Zx48k},
};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, Stream, StreamConfig,
};
use iced::{
    event,
    futures::{
        channel::mpsc::{self, channel, Sender},
        SinkExt, StreamExt,
    },
    keyboard::Event as KeyEvent,
    stream,
    widget::{
        button, column, container, image, row, rule, scrollable, slider, text, toggler, Image,
        Space,
    },
    Alignment, ContentFit, Element, Event, Font, Length, Size, Subscription, Task, Theme,
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

    set_macos_dock_icon();

    iced::application(UI::default, UI::update, UI::view)
        .title("b2t80s · ZX Spectrum Debugger")
        .theme(UI::theme)
        .window(iced::window::Settings {
            size: Size::new(1280.0, 780.0),
            min_size: Some(Size::new(980.0, 620.0)),
            icon: window_icon(),
            ..iced::window::Settings::default()
        })
        .centered()
        .subscription(UI::subscription)
        .run()
}

#[cfg(target_os = "macos")]
fn set_macos_dock_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(include_bytes!("../assets/app-icon.png"));
    let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return;
    };
    let application = NSApplication::sharedApplication(main_thread);

    // SAFETY: This runs on the main thread and `icon` is a valid NSImage.
    unsafe { application.setApplicationIconImage(Some(&icon)) };
}

#[cfg(target_os = "macos")]
fn macos_is_fullscreen() -> Option<bool> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSWindowStyleMask};

    let main_thread = MainThreadMarker::new()?;
    let application = NSApplication::sharedApplication(main_thread);
    let window = application
        .mainWindow()
        .or_else(|| application.keyWindow())?;

    Some(window.styleMask().contains(NSWindowStyleMask::FullScreen))
}

#[cfg(not(target_os = "macos"))]
fn macos_is_fullscreen() -> Option<bool> {
    None
}

#[cfg(not(target_os = "macos"))]
fn set_macos_dock_icon() {}

fn window_icon() -> Option<iced::window::Icon> {
    let icon = ::image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .ok()?
        .into_rgba8();
    let (width, height) = icon.dimensions();
    iced::window::icon::from_rgba(icon.into_raw(), width, height).ok()
}

/* ********************************************* */

#[derive(Debug, Clone)]

enum Message {
    Ready(Sender<UICommands>),
    SetBuffer(usize),
    DebugSnapshot(DebugSnapshot),
    KeyEvent(KeyEvent),
    SetVolume(f32),
    SetFastTapeLoading(bool),
    WindowChanged(iced::window::Id),
    WindowModeChanged(iced::window::Mode),
    TogglePause,
    StepInstruction,
    LoadGame,
    Reset,
}

enum State {
    Starting,
    Ready(mpsc::Receiver<UICommands>),
}

struct UI {
    app_icon: image::Handle,
    bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
    buffer: usize,
    machine_ctl_tx: Option<Sender<MachineMessage>>,
    event_tx: Option<Sender<KeyEvent>>,
    fps: FPSCounter,
    debug: Option<DebugSnapshot>,
    paused: bool,
    fast_tape_loading: bool,
    fullscreen: bool,
    stream: Option<Stream>,
    volume: Arc<Mutex<f32>>,
    error: Option<String>,
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
            app_icon: image::Handle::from_bytes(
                include_bytes!("../assets/app-icon.png").as_slice(),
            ),
            bitmaps: [scr_bitmap, scr_bitmap_2],
            buffer: 0,
            machine_ctl_tx: None,
            event_tx: None,
            fps: FPSCounter::new(),
            debug: None,
            paused: false,
            fast_tape_loading: true,
            fullscreen: false,
            stream: None,
            volume: Arc::new(Mutex::new(0.5)),
            error: None,
        }
    }
}

impl UI {
    pub fn theme(&self) -> Theme {
        Theme::TokyoNightStorm
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        if let Message::WindowChanged(id) = msg {
            return iced::window::mode(id).map(Message::WindowModeChanged);
        }

        match (msg, self.event_tx.as_mut()) {
            (Message::Ready(sender), _) => {
                let (event_tx, event_rx) = channel::<KeyEvent>(10);
                let (machine_ctl_tx, machine_ctl_rx) = channel::<MachineMessage>(16);

                let sound_tx = match SoundEngine::init_engine(self.volume.clone()) {
                    Ok((stream, sound_tx)) => match stream.play() {
                        Ok(()) => {
                            self.stream = Some(stream);
                            sound_tx
                        }
                        Err(error) => {
                            eprintln!("Audio disabled: {error}");
                            SoundEngine::silent_sender()
                        }
                    },
                    Err(error) => {
                        eprintln!("Audio disabled: {error:#}");
                        SoundEngine::silent_sender()
                    }
                };

                let mut zx = match Zx48k::new(
                    [self.bitmaps[0].clone(), self.bitmaps[1].clone()],
                    event_rx,
                    machine_ctl_rx,
                    machine_ctl_tx.clone(),
                    sender.clone(),
                    sound_tx,
                ) {
                    Ok(zx) => zx,
                    Err(error) => {
                        let message = format!("Unable to start emulator: {error:#}");
                        eprintln!("{message}");
                        self.error = Some(message);
                        return Task::none();
                    }
                };

                self.error = None;
                self.machine_ctl_tx = Some(machine_ctl_tx.clone());
                self.event_tx = Some(event_tx.clone());

                task::spawn(async move {
                    zx.run().await;
                });
            }
            (Message::SetBuffer(b), _) => {
                self.buffer = b;
                self.fps.tick();
                if let Some(fullscreen) = macos_is_fullscreen() {
                    self.fullscreen = fullscreen;
                }
            }
            (Message::DebugSnapshot(snapshot), _) => {
                self.paused = snapshot.paused;
                self.debug = Some(snapshot);
            }
            (Message::SetVolume(b), _) => {
                *self.volume.lock().unwrap() = b;
                println!("SetVolume: {}", b);
            }
            (Message::SetFastTapeLoading(enabled), _) => {
                self.fast_tape_loading = enabled;
                if let Some(tx) = self.machine_ctl_tx.as_mut() {
                    let _ = tx.start_send(MachineMessage::SetFastTapeLoading(enabled));
                }
            }
            (Message::WindowModeChanged(mode), _) => {
                self.fullscreen = mode == iced::window::Mode::Fullscreen;
            }
            (Message::Reset, _) => {
                if let Some(tx) = self.machine_ctl_tx.as_mut() {
                    let _ = tx.start_send(MachineMessage::Reset);
                }
            }
            (Message::TogglePause, _) => {
                self.paused = !self.paused;
                if let Some(tx) = self.machine_ctl_tx.as_mut() {
                    let command = if self.paused {
                        MachineMessage::CPUWait
                    } else {
                        MachineMessage::CPUResume
                    };
                    let _ = tx.start_send(command);
                }
            }
            (Message::StepInstruction, _) => {
                self.paused = true;
                if let Some(tx) = self.machine_ctl_tx.as_mut() {
                    let _ = tx.start_send(MachineMessage::CPUStep);
                }
            }
            (Message::LoadGame, _) => {
                if let Some(machine_tx) = self.machine_ctl_tx.as_mut() {
                    let _ = machine_tx.start_send(MachineMessage::TypeLoadCommand);
                }
                self.paused = false;
            }
            (Message::KeyEvent(e), Some(tx)) => {
                let _ = tx.start_send(e);
            }
            _ => (),
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let screen = image::Handle::from_rgba(
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
            self.bitmaps[self.buffer].lock().unwrap().clone(),
        );

        let screen = Image::<image::Handle>::new(screen)
            .filter_method(image::FilterMethod::Nearest)
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill);

        if self.fullscreen {
            return container(screen)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(container::dark)
                .into();
        }

        let run_label = if self.paused {
            "▶  Run"
        } else {
            "Ⅱ  Pause"
        };
        let controls = row![
            Image::<image::Handle>::new(self.app_icon.clone())
                .width(Length::Fixed(30.0))
                .height(Length::Fixed(30.0)),
            text("b2t80s").size(22),
            text("ZX Spectrum 48K").size(14),
            Space::new().width(Length::Fill),
            toggler(self.fast_tape_loading)
                .label("FAST TAPE")
                .text_size(12)
                .size(16)
                .on_toggle(Message::SetFastTapeLoading),
            action(text("Load Game…"), Some(Message::LoadGame)),
            action(text(run_label), Some(Message::TogglePause)),
            action(
                text("↦  Step"),
                self.paused.then_some(Message::StepInstruction),
            ),
            action(text("↻  Reset"), Some(Message::Reset)),
            text("VOL").size(12),
            slider::Slider::new(0.0..=1.0, *self.volume.lock().unwrap(), Message::SetVolume)
                .step(0.1)
                .width(Length::Fixed(90.0)),
        ]
        .spacing(12)
        .padding([10, 14])
        .align_y(Alignment::Center);

        let screen_panel = container(screen)
            .padding(12)
            .width(Length::FillPortion(7))
            .height(Length::Fill)
            .style(container::bordered_box);

        let debugger = self.debugger_panel();
        let workspace = row![screen_panel, debugger]
            .spacing(12)
            .padding([0, 12])
            .height(Length::Fill);

        let state = if self.paused { "PAUSED" } else { "RUNNING" };
        let status = row![
            text(format!("● {state}")).size(12),
            text(format!("FPS {:05.2}", self.fps.fps))
                .size(12)
                .font(Font::MONOSPACE),
            Space::new().width(Length::Fill),
            text(self.error.as_deref().unwrap_or("Ready")).size(12),
        ]
        .spacing(18)
        .padding([8, 14]);

        let content = column![controls, rule::horizontal(1), workspace, status]
            .spacing(8)
            .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn debugger_panel(&self) -> Element<'_, Message> {
        let Some(debug) = self.debug.as_ref() else {
            return container(column![
                text("DEBUGGER").size(18),
                text("Waiting for machine state…"),
            ])
            .padding(16)
            .width(Length::FillPortion(5))
            .height(Length::Fill)
            .style(container::rounded_box)
            .into();
        };

        let regs = debug.registers;
        let register_rows = column![
            register_row("AF", regs.af(), "BC", regs.bc()),
            register_row("DE", regs.de(), "HL", regs.hl()),
            register_row("IX", regs.ix(), "IY", regs.iy()),
            register_row("SP", regs.sp, "PC", regs.pc),
            register_row(
                "AF′",
                regs.af_aux(),
                "IR",
                ((regs.i as u16) << 8) | regs.r as u16
            ),
        ]
        .spacing(7);

        let flags = format!(
            "{}{}{}{}{}{}{}{}",
            flag('S', regs.f.s),
            flag('Z', regs.f.z),
            flag('5', regs.f.f5),
            flag('H', regs.f.h),
            flag('3', regs.f.f3),
            flag('P', regs.f.p),
            flag('N', regs.f.n),
            flag('C', regs.f.c),
        );

        let trace = debug
            .recent_instructions
            .iter()
            .rev()
            .fold(column![].spacing(5), |trace, instruction| {
                trace.push(text(instruction).size(13).font(Font::MONOSPACE))
            });

        let panel = column![
            row![
                text("CPU DEBUGGER").size(18),
                Space::new().width(Length::Fill),
                text(if debug.halted { "HALT" } else { "Z80" }).size(12),
            ]
            .align_y(Alignment::Center),
            text(format!(
                "FRAME {:05}  LINE {:03}  T {:03}",
                debug.frame_tstate,
                debug.frame_tstate / 224,
                debug.frame_tstate % 224
            ))
            .size(12)
            .font(Font::MONOSPACE),
            rule::horizontal(1),
            text("REGISTERS").size(12),
            register_rows,
            text(format!("FLAGS  {flags}    IM {}", regs.im))
                .size(13)
                .font(Font::MONOSPACE),
            rule::horizontal(1),
            row![
                text("RECENT INSTRUCTIONS").size(12),
                Space::new().width(Length::Fill),
                text("newest first").size(11),
            ],
            scrollable(trace).height(Length::Fill),
            rule::horizontal(1),
            text("NEXT: breakpoints · memory · watches").size(11),
        ]
        .spacing(11);

        container(panel)
            .padding(16)
            .width(Length::FillPortion(5))
            .height(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            self.some_worker(),
            event::listen_with(|event, _, window| match event {
                Event::Keyboard(e) => Some(Message::KeyEvent(e)),
                Event::Window(
                    iced::window::Event::Opened { .. }
                    | iced::window::Event::Resized(_)
                    | iced::window::Event::Focused,
                ) => Some(Message::WindowChanged(window)),
                _ => None,
            }),
        ])
    }

    fn some_worker(&self) -> Subscription<Message> {
        Subscription::run(worker_stream)
    }
}

fn worker_stream() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(100, async move |mut output: Sender<Message>| {
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
                        Some(UICommands::DebugSnapshot(snapshot)) => {
                            let _ = output.send(Message::DebugSnapshot(snapshot)).await;
                        }
                        None => unreachable!(),
                    }
                }
            }
        }
    })
}

fn register_row<'a>(
    left_name: &'a str,
    left_value: u16,
    right_name: &'a str,
    right_value: u16,
) -> Element<'a, Message> {
    row![
        text(format!("{left_name:<3} {left_value:04X}"))
            .font(Font::MONOSPACE)
            .width(Length::FillPortion(1)),
        text(format!("{right_name:<3} {right_value:04X}"))
            .font(Font::MONOSPACE)
            .width(Length::FillPortion(1)),
    ]
    .spacing(16)
    .into()
}

fn flag(name: char, enabled: bool) -> char {
    if enabled {
        name
    } else {
        '·'
    }
}

fn action<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let action = button(container(content));
    if let Some(on_press) = on_press {
        action.on_press(on_press).into()
    } else {
        action.style(button::secondary).into()
    }
}

struct SoundEngine {}

impl SoundEngine {
    fn init_engine(
        volume: Arc<Mutex<f32>>,
    ) -> anyhow::Result<(Stream, std::sync::mpsc::Sender<f32>)> {
        let host: cpal::Host = cpal::default_host();
        let device: cpal::Device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no output device is available"))?;
        println!("Output device: {}", device.description()?.name());

        let def_config = device.default_output_config()?;
        println!("Default output config: {:?}", def_config);
        println!("Default sample_format {:?}", def_config.sample_format());

        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        let channels = def_config.channels() as usize;
        let output_sample_rate = def_config.sample_rate() as f64;

        let (tx, rx) = std::sync::mpsc::channel::<f32>();
        let source_step = 35_000.0 / output_sample_rate;
        let mut source_phase = 1.0;
        let mut current_sample = 0.0;
        let mut next_value = move || {
            source_phase += source_step;
            while source_phase >= 1.0 {
                match rx.try_recv() {
                    Ok(sample) => {
                        current_sample = sample;
                        source_phase -= 1.0;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        source_phase = 1.0;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        current_sample = 0.0;
                        source_phase = 0.0;
                        break;
                    }
                }
            }
            current_sample * *volume.lock().unwrap()
        };

        let config: StreamConfig = StreamConfig::from(def_config);
        println!("config: {:?}", config);

        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                Self::write_data(data, channels, &mut next_value)
            },
            err_fn,
            None,
        )?;
        Ok((stream, tx))
    }

    fn silent_sender() -> std::sync::mpsc::Sender<f32> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || while rx.recv().is_ok() {});
        tx
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
