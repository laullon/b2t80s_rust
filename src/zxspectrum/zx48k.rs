use iced::futures::channel::mpsc::{Receiver, Sender};
use rfd::FileDialog;
use tokio::task;

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

use iced::keyboard::Event as KeyEvent;

#[derive(Debug)]
pub enum MachineMessage {
    CPUWait,
    CPUResume,
    CPUSetRegisters(Registers),
    Reset,
    TapLoad(std::path::PathBuf),
}

#[derive(Debug)]
enum TapState {
    Empty,
    Loading,
    Ready,
}

pub struct Zx48k {
    memory: [[u8; 0x4000]; 4],

    cpu: CPU,
    ula: ULA,

    tap: Option<Tap>,
    tap_state: TapState,

    contention_remaining: u8,
    io_cycle_precontended: bool,

    machine_ctl_rx: Receiver<MachineMessage>,
    machine_ctl_tx: Sender<MachineMessage>,
}

// todo: review, and move out
#[derive(Debug)]
pub enum UICommands {
    DrawBuffer(usize),
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

            match self.machine_ctl_rx.try_recv() {
                Ok(msg) => match msg {
                    MachineMessage::CPUWait => self.cpu.wait = true,
                    MachineMessage::CPUResume => self.cpu.wait = false,
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
                },
                Err(_) => {}
            }
            // println!("t: {}ms", t.elapsed().as_millis());
        }
    }

    fn tick_tstate(&mut self) {
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
        self.tap = None;
        self.tap_state = TapState::Empty;
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
