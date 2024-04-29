use std::default;

use crate::signals::{SignalReq, Signals};

use super::{
    ops_codes::*,
    registers::{IndexMode, Registers},
};

pub struct CPU {
    pub regs: Registers,
    pub signals: Signals,
    pub fetched: Fetched,
    pub scheduler: Vec<Operation>,
    pub wait: bool,
    pub halt: bool,
    pub current_ops: Option<Operation>,
    pub current_ops_ts: u8,
}

#[derive(Default)]
pub struct Fetched {
    pub op_code: Option<u8>,
    prefix: u16,
    pub n: Option<u8>,
    pub nn: Option<u16>,
    pub d: Option<u8>,
    pub decode_step: u8,
}

#[derive(Copy, Clone, Debug)]
pub enum Operation {
    Fetch,
    MrPcN,
    Mw8(u16, u8),
    Mw16(u16, u16),
    MrAddrN(u16),
    MrAddrR(u16, u8),
    Delay(u8),
    Pw8(u16, u8),
    PrR(u16, Option<u8>, bool),
    MrPcD,
    Int01,
    Int02,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            signals: Signals {
                addr: 0,
                data: 0,
                interrupt: false,
                mem: SignalReq::None,
                port: SignalReq::None,
            },
            fetched: Fetched::default(),
            scheduler: Vec::new(),
            wait: false,
            halt: false,
            current_ops: Some(Operation::Fetch),
            current_ops_ts: 0,
        }
    }

    pub fn tick(self: &mut Self) {
        if self.wait {
            return;
        }

        if self.halt {
            if self.signals.interrupt {
                self.halt = false;
                self.regs.pc += 1;
            } else {
                return;
            }
        }

        if matches!(self.current_ops, None) {
            if self.scheduler.is_empty() {
                // println!("{:#06x}", self.regs.pc);
                // if self.log != nil {
                //     self.log.AppendLastOP(self.fetched.getInstruction())
                // }
                self.fetched = Fetched::default();
                self.regs.index_mode = IndexMode::Hl;

                if self.signals.interrupt && self.regs.iff1 {
                    match self.regs.im {
                        0 | 1 => self.current_ops = Some(Operation::Int01),
                        2 => self.current_ops = Some(Operation::Int02),
                        _ => unreachable!("Invalid interrupt mode"),
                    }
                } else {
                    self.current_ops = Some(Operation::Fetch);
                }
            } else {
                self.current_ops = Some(self.scheduler.remove(0));
            }
        }

        let c = self.current_ops;
        match c {
            Some(op) => {
                let done = match op {
                    Operation::Fetch => self.fetch(),
                    Operation::MrPcN => self.mr(),
                    Operation::MrPcD => self.mr_d(),
                    Operation::Mw8(addr, data) => self.mw_8(addr, data),
                    Operation::Mw16(addr, data) => self.mw_16(addr, data),
                    Operation::MrAddrN(addr) => self.mr_addr_n(addr),
                    Operation::MrAddrR(addr, r) => self.mr_addr_r(addr, r),
                    Operation::Delay(delay) => self.delay(delay),
                    Operation::Pw8(addr, data) => self.pw_8(addr, data),
                    Operation::PrR(addr, r, flags) => self.pr_r(addr, r, flags),
                    Operation::Int01 => self.int01(),
                    Operation::Int02 => self.int02(),
                };
                if done {
                    // println!(
                    //     "-- done -- op: {:?} - {}",
                    //     self.current_ops, self.current_ops_ts
                    // );
                    self.current_ops = None;
                    self.current_ops_ts = 0;
                    if self.scheduler.is_empty() {
                        self.decode_and_run();
                    }
                }
            }
            None => todo!(),
        };
    }

    fn decode_and_run(&mut self) {
        let mut fetch_done = false;
        match (self.fetched.prefix, self.fetched.op_code, self.fetched.n) {
            (0, Some(0xcb) | Some(0xed), None) => self.scheduler.push(Operation::Fetch),
            (0, Some(0xdd), None) => {
                self.scheduler.push(Operation::Fetch);
                self.regs.index_mode = IndexMode::Ix;
            }
            (0, Some(0xfd), None) => {
                self.scheduler.push(Operation::Fetch);
                self.regs.index_mode = IndexMode::Iy;
            }
            (0xdd | 0xfd, Some(0xcb), None) => {
                self.scheduler.push(Operation::MrPcD);
                self.scheduler.push(Operation::Delay(2));
                self.scheduler.push(Operation::MrPcN);
            }

            (0xdd | 0xfd, Some(0xdd) | Some(0xfd), None) => {
                self.fetched.prefix = self.fetched.op_code.unwrap() as u16;
            }

            (0xdd | 0xfd, Some(0xcb), Some(n)) => {
                self.fetched.prefix = (self.fetched.prefix << 8) | 0xcb;
                self.fetched.op_code = Some(n);
                self.fetched.n = None;
                fetch_done = true;
            }
            _ => fetch_done = true,
        }

        if !fetch_done {
            return;
        };

        let op_code = match self.fetched.op_code {
            Some(op_code) => op_code,
            None => return,
        };
        let x = (op_code & 0b11000000) >> 6;
        let y = (op_code & 0b00111000) >> 3;
        let z = (op_code & 0b00000111) >> 0;
        let p = y >> 1;
        let q = y & 0b00000001;

        // println!(
        //     "<<< pfx: {:04x} opc:{:02x} x:{} y:{} z:{} p:{} q:{} >>>",
        //     self.fetched.prefix, op_code, x, y, z, p, q
        // );

        match (self.fetched.prefix, x) {
            (0xcb | 0xddcb | 0xfdcb, _) => self.cb_ops(x, y, z),

            (0 | 0xdd | 0xfd, 0) => self.x0_ops(z, y, q, p),
            (0 | 0xdd | 0xfd, 1) => self.x1_ops(y, z),
            (0 | 0xdd | 0xfd, 2) => alu(self, x, y, z),
            (0 | 0xdd | 0xfd, 3) => self.x3_ops(z, y, q, p),
            (0xed, _) => self.ed_ops(x, y, z, q, p),
            _ => todo!("decode_and_run x:{}", x),
        }
        self.fetched.decode_step += 1;
    }

    fn ed_ops(&mut self, x: u8, y: u8, z: u8, q: u8, p: u8) {
        match (x, z, y, q) {
            (1, 0, 6, _) => in_c(self),
            (1, 0, _, _) => in_r_c(self, y),
            (1, 1, 6, _) => out_c(self),
            (1, 1, _, _) => out_c_r(self, y),
            (1, 2, _, 0) => sbc_hl(self, self.regs.get_rr(p)),
            (1, 2, _, 1) => adc_hl(self, self.regs.get_rr(p)),
            (1, 3, _, 0) => ld_nn_rr(self, p),
            (1, 3, _, 1) => ld_rr_nn(self, p),
            (1, 4, _, _) => {
                let n = self.regs.a;
                self.regs.a = 0;
                sub_a(self, n);
            }
            (1, 5, _, _) => {
                self.regs.iff1 = self.regs.iff2;
                ret(self)
            }
            (1, 6, _, _) => self.regs.im = IM[y as usize],
            (1, 7, 0 | 1 | 2 | 3, _) => {
                match y {
                    0 => self.regs.i = self.regs.a,
                    1 => self.regs.r = self.regs.a,
                    2 => {
                        self.regs.a = self.regs.i;
                        ld_a_ir_flags(self);
                    }
                    3 => {
                        self.regs.a = self.regs.r;
                        ld_a_ir_flags(self);
                    }
                    _ => unreachable!("Invalid ed instruction y={}", y),
                }

                self.fetched.op_code = None;
                self.scheduler.push(Operation::Delay(1));
            }
            (1, 7, 4, _) => rdd(self),
            (1, 7, 5, _) => rld(self),
            (1, 7, 6 | 7, _) => {}
            (2, _, _, _) => bli(self, y, z),
            _ => todo!("ed_ops x:{} y:{} z:{} q:{} p:{}", x, y, z, q, p),
        }
    }

    fn cb_ops(&mut self, x: u8, y: u8, z: u8) {
        match x {
            0 => self.rot(y, z),
            1 => self.bit_ops(x, y, z),
            2 => self.bit_ops(x, y, z),
            3 => self.bit_ops(x, y, z),
            _ => unreachable!("Invalid cb instruction"),
        }
    }

    fn bit_ops(&mut self, x: u8, y: u8, z: u8) {
        let mut v = None;
        match (z, self.fetched.n, self.regs.index_mode) {
            (6, None, IndexMode::Hl) => {
                self.scheduler.push(Operation::MrAddrN(self.regs.get_rr(2)));
                self.scheduler.push(Operation::Delay(1));
            }
            (_, None, IndexMode::Ix | IndexMode::Iy) => {
                self.scheduler.push(Operation::MrAddrN(
                    self.regs.get_idx(self.fetched.d.unwrap()),
                ));
                self.scheduler.push(Operation::Delay(1));
            }
            (_, Some(n), _) => v = Some(n),
            (_, None, _) => v = Some(self.regs.get_r(z)),
        }

        let mut r = None;
        match (x, v) {
            (1, Some(v)) => _ = bit(self, y, v),
            (2, Some(v)) => r = Some(res(y, v)),
            (3, Some(v)) => r = Some(set(y, v)),
            _ => (),
        }

        match (z, r, self.regs.index_mode) {
            (6, Some(r), IndexMode::Hl) => {
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Mw8(self.regs.get_rr(2), r));
            }
            (_, Some(r), IndexMode::Ix | IndexMode::Iy) => {
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Mw8(
                    self.regs.get_idx(self.fetched.d.unwrap()),
                    r,
                ));
                if z != 6 {
                    self.regs.index_mode = IndexMode::Hl;
                    self.regs.set_r(z, r);
                }
            }
            (_, Some(r), _) => {
                self.regs.set_r(z, r);
            }
            _ => (),
        }
    }

    fn rot(&mut self, y: u8, z: u8) {
        let mut v = None;
        match (z, self.fetched.n, self.regs.index_mode) {
            (6, None, IndexMode::Hl) => {
                self.scheduler.push(Operation::MrAddrN(self.regs.get_rr(2)))
            }
            (_, None, IndexMode::Ix | IndexMode::Iy) => {
                self.scheduler.push(Operation::MrAddrN(
                    self.regs.get_idx(self.fetched.d.unwrap()),
                ));
                self.scheduler.push(Operation::Delay(1));
            }
            (_, Some(n), _) => v = Some(n),
            (_, None, _) => v = Some(self.regs.get_r(z)),
        }

        let mut res = None;
        match (y, v) {
            (0, Some(v)) => res = Some(rlc(self, z, v)),
            (1, Some(v)) => res = Some(rrc(self, z, v)),
            (2, Some(v)) => res = Some(rl(self, z, v)),
            (3, Some(v)) => res = Some(rr(self, z, v)),
            (4, Some(v)) => res = Some(sla(self, z, v)),
            (5, Some(v)) => res = Some(sra(self, z, v)),
            (6, Some(v)) => res = Some(sll(self, z, v)),
            (7, Some(v)) => res = Some(srl(self, z, v)),
            (_, None) => (),
            _ => unreachable!("Invalid rot instruction"),
        }
        match (z, res, self.regs.index_mode) {
            (6, Some(r), IndexMode::Hl) => {
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Delay(1));
                self.scheduler.push(Operation::Mw8(self.regs.get_rr(2), r))
            }
            (_, Some(r), IndexMode::Ix | IndexMode::Iy) => {
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Mw8(
                    self.regs.get_idx(self.fetched.d.unwrap()),
                    r,
                ));

                if z != 6 {
                    self.regs.index_mode = IndexMode::Hl;
                    self.regs.set_r(z, r);
                }
            }
            (_, Some(r), _) => self.regs.set_r(z, r),
            _ => (),
        }
    }

    fn x3_ops(&mut self, z: u8, y: u8, q: u8, p: u8) {
        match z {
            0 => ret_cc(self, y),
            1 => self.x3_z1_ops(q, p),
            2 => jp(self, Some(y)),
            3 => self.x3_z3_ops(y),
            4 => call(self, Some(y)),
            5 => self.x3_z5_ops(q, p),
            6 => alu(self, 3, y, z),
            7 => rst(self, y),
            _ => unreachable!("Invalid x3 instruction"),
        }
    }

    fn x3_z3_ops(&mut self, y: u8) {
        match y {
            0 => jp(self, None),
            1 => self.scheduler.push(Operation::Fetch),
            2 => out_na(self),
            3 => in_na(self),
            4 => ex_sp_hl(self),
            5 => {
                let hl = self.regs.get_rr(2);
                self.regs.set_rr(2, self.regs.get_rr(1));
                self.regs.set_rr(1, hl);
            }
            6 => {
                self.regs.iff1 = false;
                self.regs.iff2 = false;
            }
            7 => {
                self.regs.iff1 = true;
                self.regs.iff2 = true;
            }
            _ => unreachable!("Invalid x3_z3 instruction y={}", y),
        }
    }

    fn x3_z5_ops(&mut self, q: u8, p: u8) {
        match (q, p) {
            (0, _) => push(self, p),
            (1, 0) => call(self, None),
            (1, 1 | 2 | 3) => self.scheduler.push(Operation::Fetch),
            _ => unreachable!("Invalid x3_z5 instruction ({}, {})", q, p),
        }
    }

    fn x3_z1_ops(&mut self, q: u8, p: u8) {
        match (q, p) {
            (0, _) => pop(self, p),
            (1, 0) => ret(self),
            (1, 1) => self.regs.exx(),
            (1, 2) => self.regs.pc = self.regs.get_rr(2),
            (1, 3) => {
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Delay(2));
                self.regs.sp = self.regs.get_rr(2)
            }
            _ => unreachable!("Invalid x3_z1 instruction ({}, {})", q, p),
        }
    }

    pub fn if_cc(&self, y: u8) -> bool {
        match y {
            0 => self.regs.f.z == false,
            1 => self.regs.f.z == true,
            2 => self.regs.f.c == false,
            3 => self.regs.f.c == true,
            4 => self.regs.f.p == false,
            5 => self.regs.f.p == true,
            6 => self.regs.f.s == false,
            7 => self.regs.f.s == true,
            _ => unreachable!(),
        }
    }

    fn x0_ops(&mut self, z: u8, y: u8, q: u8, p: u8) {
        match z {
            0 => self.x0_z0_ops(y),
            1 => self.x0_z1_ops(q, p),
            2 => self.x0_z2_ops(q, p),
            3 => self.x0_z3_ops(q, p),
            4 => inc_r(self, y),
            5 => dec_r(self, y),
            6 => ld_r_n(self, y),
            7 => self.x0_z7_ops(y),
            _ => todo!("x0_ops z:{}", z),
        }
    }

    fn x0_z7_ops(&mut self, y: u8) {
        match y {
            0 => rlca(self),
            1 => rrca(self),
            2 => rla(self),
            3 => rra(self),
            4 => daa(self),
            5 => cpl(self),
            6 => scf(self),
            7 => ccf(self),
            _ => panic!(),
        }
    }

    fn x0_z0_ops(&mut self, y: u8) {
        match y {
            0 => (), // NOP
            1 => self.regs.exafaf(),
            2..=7 => match self.fetched.n {
                None => self.scheduler.push(Operation::MrPcN),
                Some(_) => {
                    let mut jump = true;
                    match y {
                        2 => {
                            self.regs.b = self.regs.b.wrapping_sub(1);
                            jump = self.regs.b != 0;
                            self.scheduler.push(Operation::Delay(1));
                        }
                        3 => {}
                        4 => jump = self.regs.f.z == false,
                        5 => jump = self.regs.f.z == true,
                        6 => jump = self.regs.f.c == false,
                        7 => jump = self.regs.f.c == true,
                        _ => panic!(),
                    }
                    if jump {
                        let jump = self.fetched.n.unwrap() as i8;
                        self.regs.pc = self.regs.pc.wrapping_add(jump as u16);
                        self.scheduler.push(Operation::Delay(5));
                    }
                    self.fetched.op_code = None;
                }
            },
            _ => todo!("x0_z0_ops y:{}", y),
        }
    }

    fn x1_ops(&mut self, y: u8, z: u8) {
        match (y, z) {
            (6, 6) => halt(self),
            _ => ld_r_r(self, y, z),
        }
    }

    fn x0_z1_ops(&mut self, q: u8, p: u8) {
        match q {
            0 => ld_rr_mm(self, p),
            1 => {
                add_hl_rr(self, p);
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Delay(7));
            }
            _ => panic!(),
        }
    }

    fn x0_z2_ops(&mut self, q: u8, p: u8) {
        match q {
            0 => match p {
                0 | 1 => {
                    self.fetched.op_code = None;
                    self.scheduler
                        .push(Operation::Mw8(self.regs.get_rr(p), self.regs.a));
                }
                2 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MrPcN);
                        self.scheduler.push(Operation::MrPcN);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler
                            .push(Operation::Mw16(nn, self.regs.get_rr(p)));
                    }
                },
                3 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MrPcN);
                        self.scheduler.push(Operation::MrPcN);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler.push(Operation::Mw8(nn, self.regs.a));
                    }
                },
                _ => panic!("x0_z2_ops q:{} p:{}", q, p),
            },
            1 => match p {
                0 | 1 => match self.fetched.n {
                    None => {
                        self.scheduler.push(Operation::MrAddrN(self.regs.get_rr(p)));
                    }
                    Some(n) => self.regs.a = n,
                },
                2 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MrPcN);
                        self.scheduler.push(Operation::MrPcN);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler.push(Operation::MrAddrR(nn, 5));
                        self.scheduler.push(Operation::MrAddrR(nn + 1, 4));
                    }
                },
                3 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MrPcN);
                        self.scheduler.push(Operation::MrPcN);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler.push(Operation::MrAddrR(nn, 7));
                    }
                },
                _ => panic!("x0_z2_ops q:{} p:{}", q, p),
            },
            _ => panic!("x0_z2_ops q:{}", q),
        }
    }

    fn x0_z3_ops(&mut self, q: u8, p: u8) {
        match q {
            0 => inc_rr(self, p),
            1 => dec_rr(self, p),
            _ => panic!(),
        }
        self.fetched.op_code = None;
        self.scheduler.push(Operation::Delay(2));
    }

    fn fetch(self: &mut Self) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => {
                self.regs.m1 = true;
                self.signals.addr = self.regs.pc;
                self.signals.mem = SignalReq::Read;
                self.regs.pc = self.regs.pc.wrapping_add(1);
                self.regs.r = (self.regs.r & 0x80) | ((self.regs.r.wrapping_add(1)) & 0x7f);
            }
            2 => {
                self.regs.m1 = false;
                self.signals.mem = SignalReq::None;
                match self.fetched.op_code {
                    Some(opc) => {
                        self.fetched.prefix = self.fetched.prefix << 8;
                        self.fetched.prefix |= opc as u16;
                    }
                    None => (),
                }
                self.fetched.op_code = Some(self.signals.data);
            }
            3 => {}
            4 => {
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mr_addr_n(self: &mut Self, addr: u16) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => self.signals.addr = addr,
            2 => self.signals.mem = SignalReq::Read,
            3 => {
                match self.fetched.n {
                    None => self.fetched.n = Some(self.signals.data),
                    Some(n) => {
                        let nn = ((self.signals.data as u16) << 8) | (n as u16);
                        self.fetched.nn = Some(nn);
                    }
                }
                self.signals.mem = SignalReq::None;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mr_addr_r(self: &mut Self, addr: u16, r: u8) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => self.signals.addr = addr,
            2 => self.signals.mem = SignalReq::Read,
            3 => {
                self.regs.set_r(r, self.signals.data);
                self.signals.mem = SignalReq::None;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn pr_r(self: &mut Self, addr: u16, r: Option<u8>, flags: bool) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => self.signals.addr = addr,
            2 => self.signals.port = SignalReq::Read,
            3 => {
                match r {
                    Some(r) => self.regs.set_r(r, self.signals.data),
                    _ => self.fetched.n = Some(self.signals.data),
                }
                self.signals.port = SignalReq::None;

                if flags {
                    self.regs.f.n = false;
                    self.regs.f.h = false;
                    self.regs.f.p = PARITY_TABLE[self.signals.data as usize];
                    self.regs.f.z = self.signals.data == 0;
                    self.regs.f.s = self.signals.data & 0x0080 != 0;
                }

                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mr(self: &mut Self) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => self.signals.addr = self.regs.pc,
            2 => self.signals.mem = SignalReq::Read,
            3 => {
                match self.fetched.n {
                    None => self.fetched.n = Some(self.signals.data),
                    Some(n) => {
                        let nn = ((self.signals.data as u16) << 8) | (n as u16);
                        self.fetched.nn = Some(nn);
                    }
                }
                self.signals.mem = SignalReq::None;
                self.regs.pc += 1;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mr_d(self: &mut Self) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => self.signals.addr = self.regs.pc,
            2 => self.signals.mem = SignalReq::Read,
            3 => {
                self.fetched.d = Some(self.signals.data);
                self.signals.mem = SignalReq::None;
                self.regs.pc += 1;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mw_8(self: &mut Self, addr: u16, data: u8) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => {
                self.signals.addr = addr;
                self.signals.data = data;
            }
            2 => self.signals.mem = SignalReq::Write,
            3 => {
                self.signals.mem = SignalReq::None;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn pw_8(self: &mut Self, addr: u16, data: u8) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => {
                self.signals.addr = addr;
                self.signals.data = data;
            }
            2 => self.signals.port = SignalReq::Write,
            3 => {
                self.signals.port = SignalReq::None;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mw_16(self: &mut Self, addr: u16, data: u16) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => {
                self.signals.addr = addr;
                self.signals.data = data as u8;
            }
            2 => self.signals.mem = SignalReq::Write,
            3 => {
                self.signals.mem = SignalReq::None;
            }
            4 => {
                self.signals.addr = addr + 1;
                self.signals.data = (data >> 8) as u8;
            }
            5 => self.signals.mem = SignalReq::Write,
            6 => {
                self.signals.mem = SignalReq::None;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn delay(self: &mut Self, delay: u8) -> bool {
        self.current_ops_ts += 1;
        self.current_ops_ts == delay
    }

    pub fn dump_registers_aux(&self) -> String {
        format!(
            "i: {}, r: {}, iff1: {}, iff2: {}, im: {}, halt: {}",
            self.regs.i, self.regs.r, self.regs.iff1, self.regs.iff2, self.regs.im, self.halt
        )
    }

    fn int01(&mut self) -> bool {
        self.regs.iff1 = false;
        self.regs.sp = self.regs.sp.wrapping_sub(2);
        self.scheduler.push(Operation::Delay(1));
        self.scheduler
            .push(Operation::Mw16(self.regs.sp, self.regs.pc));
        self.regs.pc = 0x0038;
        return true;
    }

    fn int02(&self) -> bool {
        todo!()
    }
}
