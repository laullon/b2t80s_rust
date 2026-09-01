use iced::futures::channel::mpsc::{Receiver, Sender};
use iced::keyboard::Event as KeyEvent;
use rfd::FileDialog;
use tokio::task;

use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::{env, fs::File, io::Read, path::PathBuf};

use crate::signals::SignalReq;
use crate::z80::cpu::{Operation, CPU};
use crate::z80::registers::Registers;

use super::tap::Tap;
use super::ula::{TSTATES_PER_FRAME, TSTATES_PER_LINE, ULA};

const CPU_CLOCK_HZ: u64 = 3_500_000;
const FRAME_DURATION: Duration =
    Duration::from_nanos((TSTATES_PER_FRAME as u64 * 1_000_000_000) / CPU_CLOCK_HZ);
const FIRST_CONTENDED_TSTATE: usize = 14_335;
const CONTENDED_LINES: usize = 192;
const CONTENDED_TSTATES_PER_LINE: usize = 128;
const CONTENTION_PATTERN: [u8; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

#[derive(Debug)]
pub enum MachineMessage {
    CPUWait,
    CPUResume,
    CPUStep,
    CPUSetRegisters(Registers),
    TypeLoadCommand,
    Reset,
    TapLoad(std::path::PathBuf),
}

#[derive(Debug)]
enum TapState {
    Empty,
    Loading,
    Ready,
}

#[derive(Debug)]
enum KeyboardStep {
    Wait(u8),
    SetKey {
        row: usize,
        bit: usize,
        pressed: bool,
    },
}

#[derive(Debug)]
struct KeyboardScript {
    steps: VecDeque<KeyboardStep>,
    wait_frames: u8,
    wait_for_basic_prompt: bool,
}

impl KeyboardScript {
    fn load_command() -> Self {
        let mut steps = VecDeque::new();

        // J enters the LOAD keyword in K mode.
        push_key(&mut steps, 6, 4);

        // A quote is SYMBOL SHIFT + P on the Spectrum keyboard.
        push_quote(&mut steps);
        push_quote(&mut steps);

        // ENTER executes LOAD "".
        push_key(&mut steps, 6, 1);

        Self {
            steps,
            wait_frames: 0,
            wait_for_basic_prompt: true,
        }
    }

    fn observe_pc(&mut self, pc: u16) {
        // 0x0F38 is ED-LOOP, the editor's own call site for WAIT-KEY.
        // WAIT-KEY (0x15D4) is shared by several non-editor ROM paths and
        // can be reached during startup before the BASIC K cursor is ready.
        if self.wait_for_basic_prompt && pc == 0x0f38 {
            self.wait_for_basic_prompt = false;
        }
    }
}

fn push_key(steps: &mut VecDeque<KeyboardStep>, row: usize, bit: usize) {
    steps.push_back(KeyboardStep::SetKey {
        row,
        bit,
        pressed: true,
    });
    steps.push_back(KeyboardStep::Wait(4));
    steps.push_back(KeyboardStep::SetKey {
        row,
        bit,
        pressed: false,
    });
    steps.push_back(KeyboardStep::Wait(4));
}

fn push_quote(steps: &mut VecDeque<KeyboardStep>) {
    steps.push_back(KeyboardStep::SetKey {
        row: 7,
        bit: 2,
        pressed: true,
    });
    steps.push_back(KeyboardStep::Wait(2));
    steps.push_back(KeyboardStep::SetKey {
        row: 5,
        bit: 1,
        pressed: true,
    });
    steps.push_back(KeyboardStep::Wait(4));
    steps.push_back(KeyboardStep::SetKey {
        row: 5,
        bit: 1,
        pressed: false,
    });
    steps.push_back(KeyboardStep::SetKey {
        row: 7,
        bit: 2,
        pressed: false,
    });
    steps.push_back(KeyboardStep::Wait(4));
}

pub struct Zx48k {
    memory: [[u8; 0x4000]; 4],

    cpu: CPU,
    ula: ULA,

    tap: Option<Tap>,
    tap_state: TapState,

    contention_remaining: u8,
    io_cycle_precontended: bool,
    paused: bool,
    step_instruction: bool,
    keyboard_script: Option<KeyboardScript>,

    machine_ctl_rx: Receiver<MachineMessage>,
    machine_ctl_tx: Sender<MachineMessage>,
    ui_ctl_tx: Sender<UICommands>,
}

// todo: review, and move out
#[derive(Debug)]
pub enum UICommands {
    DrawBuffer(usize),
    DebugSnapshot(DebugSnapshot),
}

#[derive(Debug, Clone)]
pub struct DebugSnapshot {
    pub registers: Registers,
    pub halted: bool,
    pub paused: bool,
    pub frame_tstate: usize,
    pub recent_instructions: Vec<String>,
}

impl Zx48k {
    pub fn new(
        bitmaps: [Arc<Mutex<Vec<u8>>>; 2],
        event_rx: Receiver<KeyEvent>,
        machine_ctl_rx: Receiver<MachineMessage>,
        machine_ctl_tx: Sender<MachineMessage>,
        ui_ctl_tx: Sender<UICommands>,
        sound_tx: mpsc::Sender<f32>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            memory: [load_rom()?, [0; 0x4000], [0; 0x4000], [0; 0x4000]],
            cpu: CPU::new(),
            ula: ULA::new(bitmaps, event_rx, ui_ctl_tx.clone(), sound_tx),
            machine_ctl_rx,
            machine_ctl_tx,
            tap: None,
            tap_state: TapState::Empty,
            contention_remaining: 0,
            io_cycle_precontended: false,
            paused: false,
            step_instruction: false,
            keyboard_script: None,
            ui_ctl_tx,
        })
    }

    pub async fn run(self: &mut Self) -> ! {
        println!("Zx48k::run()");
        let mut interval = tokio::time::interval(FRAME_DURATION);
        loop {
            interval.tick().await;
            for _ in 0..TSTATES_PER_FRAME {
                self.tick_tstate();
            }

            while let Ok(msg) = self.machine_ctl_rx.try_recv() {
                match msg {
                    MachineMessage::CPUWait => {
                        self.paused = true;
                        self.step_instruction = false;
                    }
                    MachineMessage::CPUResume => {
                        self.paused = false;
                        self.step_instruction = false;
                    }
                    MachineMessage::CPUStep => {
                        self.paused = true;
                        self.step_instruction = true;
                    }
                    MachineMessage::TypeLoadCommand => self.start_load_command(),
                    MachineMessage::Reset => self.reset(),
                    MachineMessage::CPUSetRegisters(_) => todo!(),
                    MachineMessage::TapLoad(file) => match Tap::new(&file) {
                        Ok(tap) => {
                            self.tap = Some(tap);
                            self.tap_state = TapState::Ready;
                        }
                        Err(error) => {
                            eprintln!("Unable to load {}: {error:#}", file.display());
                            self.reset();
                        }
                    },
                }
            }
            self.advance_keyboard_script();
            self.send_debug_snapshot();
            // println!("t: {}ms", t.elapsed().as_millis());
        }
    }

    fn tick_tstate(&mut self) {
        if self.paused && !self.step_instruction {
            return;
        }

        if let Some(script) = self.keyboard_script.as_mut() {
            script.observe_pc(self.cpu.regs.pc);
        }

        let frame_tstate = self.ula.frame_tstate();

        // The ULA pixel clock is twice the Z80 clock.
        self.ula.tick();
        self.bus_tick();
        self.ula.tick();
        self.bus_tick();

        if self.contention_remaining != 0 {
            self.contention_remaining -= 1;
            return;
        }

        let precontended_io = pending_io_cycle(&self.cpu);
        let trap = self.cpu.tick();
        self.bus_tick();

        if self.step_instruction && trap.is_some() {
            self.step_instruction = false;
        }

        if let Some(port) = precontended_io {
            self.contention_remaining = io_contention_delay(frame_tstate, port);
            self.io_cycle_precontended = true;
        } else if let Some(addr) = memory_cycle_started(&self.cpu) {
            if is_contended_address(addr) {
                self.contention_remaining = memory_contention_delay(frame_tstate);
            }
        } else if let Some(port) = port_cycle_started(&self.cpu) {
            if self.io_cycle_precontended {
                self.io_cycle_precontended = false;
            } else {
                self.contention_remaining = io_contention_delay(frame_tstate, port);
            }
        }

        if trap == Some(0x056B) {
            self.ula.clean_keyboard();

            match self.tap_state {
                TapState::Empty => {
                    self.tap_state = TapState::Loading;
                    load_tap_file(self.machine_ctl_tx.clone());
                }
                TapState::Loading => (),
                TapState::Ready => self.load_tap_block(),
            }
        }
    }

    fn reset(self: &mut Self) {
        self.cpu.do_reset = true;
        self.paused = false;
        self.step_instruction = false;
        self.keyboard_script = None;
        self.ula.clean_keyboard();
        self.tap = None;
        self.tap_state = TapState::Empty;
    }

    fn start_load_command(&mut self) {
        self.reset();
        self.keyboard_script = Some(KeyboardScript::load_command());
    }

    fn advance_keyboard_script(&mut self) {
        let Some(mut script) = self.keyboard_script.take() else {
            return;
        };

        if script.wait_for_basic_prompt {
            self.keyboard_script = Some(script);
            return;
        }

        if script.wait_frames != 0 {
            script.wait_frames -= 1;
            self.keyboard_script = Some(script);
            return;
        }

        loop {
            match script.steps.pop_front() {
                Some(KeyboardStep::Wait(frames)) => {
                    script.wait_frames = frames;
                    self.keyboard_script = Some(script);
                    return;
                }
                Some(KeyboardStep::SetKey { row, bit, pressed }) => {
                    self.ula.set_matrix_key(row, bit, pressed)
                }
                None => return,
            }
        }
    }

    fn send_debug_snapshot(&mut self) {
        let snapshot = DebugSnapshot {
            registers: self.cpu.regs,
            halted: self.cpu.halt,
            paused: self.paused,
            frame_tstate: self.ula.frame_tstate(),
            recent_instructions: self.cpu.log.clone(),
        };
        let _ = self
            .ui_ctl_tx
            .start_send(UICommands::DebugSnapshot(snapshot));
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
        let data: Vec<u8> = match self.tap.as_mut() {
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
            return;
        }

        let requested_length = self.cpu.regs.de();
        let start_address = self.cpu.regs.ix();
        println!("Loading block to {:04x} ({})", start_address, data.len());

        self.cpu.wait = true;
        let a = data[0];
        if self.cpu.regs.a_alt == a {
            if self.cpu.regs.f_alt.c {
                let required_length = requested_length as usize + 2;
                if data.len() < required_length {
                    eprintln!(
                        "Tape block is too short: requested {requested_length} bytes, block contains {}",
                        data.len().saturating_sub(2)
                    );
                    self.cpu.regs.f.c = false;
                    self.cpu.regs.pc = 0x05e2;
                    self.cpu.wait = false;
                    return;
                }

                let mut checksum = data[0];
                for i in 0..(requested_length as usize) {
                    let loaded_byte = data[i + 1];
                    self.mem_write(start_address.wrapping_add(i as u16), loaded_byte);
                    checksum ^= loaded_byte;
                }

                let expected_checksum = data[requested_length as usize + 1];
                self.cpu.regs.f.c = checksum == expected_checksum;
                println!("{checksum} == {expected_checksum} : {}", self.cpu.regs.f.c);
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
    }
}

fn is_contended_address(addr: u16) -> bool {
    addr & 0xc000 == 0x4000
}

fn memory_contention_delay(frame_tstate: usize) -> u8 {
    let Some(display_tstate) = frame_tstate.checked_sub(FIRST_CONTENDED_TSTATE) else {
        return 0;
    };
    let line = display_tstate / TSTATES_PER_LINE;
    let line_tstate = display_tstate % TSTATES_PER_LINE;

    if line < CONTENDED_LINES && line_tstate < CONTENDED_TSTATES_PER_LINE {
        CONTENTION_PATTERN[line_tstate % CONTENTION_PATTERN.len()]
    } else {
        0
    }
}

fn io_contention_delay(frame_tstate: usize, port: u16) -> u8 {
    let high_byte_contended = is_contended_address(port);
    let low_byte_even = port & 1 == 0;

    // The documented sequences are C:1,C:3 / N:1,C:3 / C:1,N:3.
    // Each C segment samples the contention table once, then consumes the
    // stated number of ordinary T-states.
    let segments = match (high_byte_contended, low_byte_even) {
        (true, true) => [(true, 1usize), (true, 3usize)],
        (false, true) => [(false, 1), (true, 3)],
        (true, false) => [(true, 1), (false, 3)],
        (false, false) => [(false, 1), (false, 3)],
    };

    let mut elapsed = 0usize;
    for (contended, duration) in segments {
        if contended {
            elapsed +=
                memory_contention_delay((frame_tstate + elapsed) % TSTATES_PER_FRAME) as usize;
        }
        elapsed += duration;
    }

    (elapsed - 4) as u8
}

fn memory_cycle_started(cpu: &CPU) -> Option<u16> {
    match (cpu.current_ops, cpu.current_ops_ts) {
        (Some(Operation::Fetch), 1) | (Some(Operation::MrPcN), 1) | (Some(Operation::MrPcD), 1) => {
            Some(cpu.signals.addr)
        }
        (Some(Operation::MrAddrN(addr)), 1)
        | (Some(Operation::MrAddrR(addr, _)), 1)
        | (Some(Operation::Mw8(addr, _)), 1)
        | (Some(Operation::Mw16(addr, _)), 1) => Some(addr),
        (Some(Operation::Mw16(addr, _)), 4) => Some(addr.wrapping_add(1)),
        _ => None,
    }
}

fn port_cycle_started(cpu: &CPU) -> Option<u16> {
    match (cpu.current_ops, cpu.current_ops_ts) {
        (Some(Operation::Pw8(port, _)), 1) | (Some(Operation::PrR(port, _, _)), 1) => Some(port),
        _ => None,
    }
}

// Immediate I/O instructions model their first I/O T-state as a Delay(1)
// immediately before the three-state port operation. Detect that boundary so
// contention is applied to the complete four-state I/O cycle.
fn pending_io_cycle(cpu: &CPU) -> Option<u16> {
    let port_operation = match (cpu.current_ops, cpu.current_ops_ts) {
        (Some(Operation::Delay(1)), 0) => cpu.scheduler.first(),
        // Delay(1) is selected and completed in one CPU tick, so it normally
        // remains at the front of the scheduler when this boundary is checked.
        (None, 0) if matches!(cpu.scheduler.first(), Some(Operation::Delay(1))) => {
            cpu.scheduler.get(1)
        }
        _ => return None,
    };

    match port_operation {
        Some(Operation::Pw8(port, _)) | Some(Operation::PrR(port, _, _)) => Some(*port),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_command_types_the_spectrum_keyboard_sequence() {
        let script = KeyboardScript::load_command();
        assert!(script.wait_for_basic_prompt);

        let keys: Vec<(usize, usize, bool)> = script
            .steps
            .iter()
            .filter_map(|step| match step {
                KeyboardStep::SetKey { row, bit, pressed } => Some((*row, *bit, *pressed)),
                KeyboardStep::Wait(_) => None,
            })
            .collect();

        assert_eq!(
            keys,
            vec![
                (6, 4, true),
                (6, 4, false),
                (7, 2, true),
                (5, 1, true),
                (5, 1, false),
                (7, 2, false),
                (7, 2, true),
                (5, 1, true),
                (5, 1, false),
                (7, 2, false),
                (6, 1, true),
                (6, 1, false),
            ]
        );
    }

    #[test]
    fn load_command_starts_when_the_rom_reaches_the_basic_prompt() {
        let mut script = KeyboardScript::load_command();

        script.observe_pc(0x15d4);
        assert!(script.wait_for_basic_prompt);

        script.observe_pc(0x0f38);
        assert!(!script.wait_for_basic_prompt);
    }

    #[test]
    fn memory_contention_uses_the_48k_delay_pattern() {
        let delays: Vec<u8> = (0..8)
            .map(|offset| memory_contention_delay(FIRST_CONTENDED_TSTATE + offset))
            .collect();

        assert_eq!(delays, CONTENTION_PATTERN);
        assert_eq!(memory_contention_delay(FIRST_CONTENDED_TSTATE - 1), 0);
        assert_eq!(
            memory_contention_delay(FIRST_CONTENDED_TSTATE + CONTENDED_TSTATES_PER_LINE),
            0
        );
    }

    #[test]
    fn contention_is_limited_to_the_192_display_lines() {
        let last_line = FIRST_CONTENDED_TSTATE + (CONTENDED_LINES - 1) * TSTATES_PER_LINE;
        let line_after_display = FIRST_CONTENDED_TSTATE + CONTENDED_LINES * TSTATES_PER_LINE;

        assert_eq!(memory_contention_delay(last_line), 6);
        assert_eq!(memory_contention_delay(line_after_display), 0);
    }

    #[test]
    fn io_contention_depends_on_both_halves_of_the_port_address() {
        let tstate = FIRST_CONTENDED_TSTATE;

        assert_eq!(io_contention_delay(tstate, 0x00ff), 0);
        assert_eq!(io_contention_delay(tstate, 0x00fe), 5);
        assert_eq!(io_contention_delay(tstate, 0x40ff), 6);
        assert_eq!(io_contention_delay(tstate, 0x40fe), 6);
    }

    #[test]
    fn immediate_io_contention_starts_at_the_leading_delay() {
        let mut cpu = CPU::new();
        cpu.current_ops = None;
        cpu.scheduler = vec![Operation::Delay(1), Operation::Pw8(0x00fe, 1)];

        assert_eq!(pending_io_cycle(&cpu), Some(0x00fe));
    }
}

fn load_tap_file(mut machine_ctl_tx: Sender<MachineMessage>) {
    let _ = task::spawn(async move {
        let path: std::path::PathBuf = env::current_dir().unwrap();
        let file: Option<_> = FileDialog::new()
            .add_filter("tap", &["tap"])
            .set_directory(path)
            .pick_file();
        match file {
            Some(f) => machine_ctl_tx
                .start_send(MachineMessage::TapLoad(f))
                .unwrap(),
            None => machine_ctl_tx.start_send(MachineMessage::Reset).unwrap(),
        }
    });
}

fn load_rom() -> anyhow::Result<[u8; 0x4000]> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("B2T80S_ROM") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(path) = env::current_dir() {
        candidates.push(path.join("bin/48.rom"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/48.rom"));

    let path = candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            anyhow::anyhow!("ZX Spectrum ROM not found; set B2T80S_ROM or place 48.rom in bin/")
        })?;

    let mut f = File::open(&path)?;
    let mut rom = [0; 0x4000];
    f.read_exact(&mut rom)?;

    Ok(rom)
}
