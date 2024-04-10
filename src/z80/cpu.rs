use super::{
    ops_codes::*,
    registers::{IndexMode, Registers},
};

pub fn hello() -> String {
    "Hello!".to_string()
}

pub enum SignalReq {
    Read,
    Write,
    None,
}

pub struct Signals {
    pub addr: u16,
    pub data: u8,
    pub mem: SignalReq,
    pub port: SignalReq,
}

pub struct CPU {
    pub regs: Registers,
    pub signals: Signals,
    pub fetched: Fetched,
    pub scheduler: Vec<Operation>,
    pub wait: bool,
    pub do_interrupt: bool,
    pub halt: bool,
    pub current_ops: Option<Operation>,
    pub current_ops_ts: u8,
}

pub struct Fetched {
    pub op_code: Option<u8>,
    prefix: u16,
    pub n: Option<u8>,
    pub nn: Option<u16>,
    pub d: Option<u8>,
}

#[derive(Copy, Clone, Debug)]
pub enum Operation {
    Fetch,
    MR_PC_N,
    MW_8(u16, u8),
    MW_16(u16, u16),
    MR_ADDR_N(u16),
    MR_ADDR_R(u16, u8),
    Delay(u8),
    PW_8(u16, u8),
    PR_R(u16, u8),
    MR_PC_D,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            signals: Signals {
                addr: 0,
                data: 0,
                mem: SignalReq::None,
                port: SignalReq::None,
            },
            fetched: Fetched {
                op_code: None,
                prefix: 0,
                n: None,
                nn: None,
                d: None,
            },
            scheduler: Vec::new(),
            wait: false,
            do_interrupt: false,
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
            if self.do_interrupt {
                self.halt = false;
                self.regs.pc += 1;
                self.exec_interrupt()
            } else {
                return;
            }
        }

        if matches!(self.current_ops, None) {
            if self.scheduler.is_empty() {
                // if self.log != nil {
                //     self.log.AppendLastOP(self.fetched.getInstruction())
                // }
                if self.do_interrupt {
                    self.exec_interrupt()
                } else {
                    self.fetched.op_code = None;
                    self.fetched.n = None;
                    self.fetched.nn = None;
                    self.fetched.d = None;
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
                    Operation::MR_PC_N => self.mr(),
                    Operation::MR_PC_D => self.mr_d(),
                    Operation::MW_8(addr, data) => self.mw_8(addr, data),
                    Operation::MW_16(addr, data) => self.mw_16(addr, data),
                    Operation::MR_ADDR_N(addr) => self.mr_addr_n(addr),
                    Operation::MR_ADDR_R(addr, r) => self.mr_addr_r(addr, r),
                    Operation::Delay(delay) => self.delay(delay),
                    Operation::PW_8(addr, data) => self.pw_8(addr, data),
                    Operation::PR_R(addr, r) => self.pr_r(addr, r),
                };
                if done {
                    println!("{} {:?}", self.current_ops_ts, self.current_ops);
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
        let op_code = match self.fetched.op_code {
            Some(op_code) => op_code,
            None => return,
        };
        let x = (op_code & 0b11000000) >> 6;
        let y = (op_code & 0b00111000) >> 3;
        let z = (op_code & 0b00000111) >> 0;
        let p = y >> 1;
        let q = y & 0b00000001;

        println!(
            "pfx: {:04x} opc:{:02x} x:{} y:{} z:{} p:{} q:{}",
            self.fetched.prefix, op_code, x, y, z, p, q
        );
        match (self.fetched.prefix, x) {
            (0 | 0xdd, 0) => self.x0_ops(z, y, q, p),
            (0 | 0xdd, 1) => self.x1_ops(y, z),
            (0 | 0xdd, 2) => alu(self, x, y, z),
            (0 | 0xdd, 3) => self.x3_ops(z, y, q, p),
            (0xcb, _) => self.cb_ops(x, y, z),
            _ => todo!("decode_and_run x:{}", x),
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
        match (z, self.fetched.n) {
            (6, None) => {
                self.scheduler
                    .push(Operation::MR_ADDR_N(self.regs.get_rr(2)));
                self.scheduler.push(Operation::Delay(1));
            }
            (6, Some(n)) => v = Some(n),
            (_, None) => v = Some(self.regs.get_r(z)),
            _ => unreachable!("Invalid bit instruction (r)"),
        }

        let mut r = None;
        match (x, v) {
            (1, Some(v)) => _ = bit(self, y, v),
            (2, Some(v)) => r = Some(res(self, y, v)),
            (3, Some(v)) => r = Some(set(self, y, v)),
            _ => (),
        }

        match (z, r) {
            (6, Some(r)) => {
                self.scheduler.push(Operation::MW_8(self.regs.get_rr(2), r));
            }
            (_, Some(r)) => {
                self.regs.set_r(z, r);
            }
            _ => (),
        }
    }

    fn rot(&mut self, y: u8, z: u8) {
        let mut v = None;
        match (z, self.fetched.n) {
            (6, None) => self
                .scheduler
                .push(Operation::MR_ADDR_N(self.regs.get_rr(2))),
            (6, Some(n)) => v = Some(n),
            (_, None) => v = Some(self.regs.get_r(z)),
            _ => unreachable!("Invalid rot instruction (r)"),
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
        match (z, res) {
            (6, Some(r)) => {
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Delay(1));
                self.scheduler.push(Operation::MW_8(self.regs.get_rr(2), r))
            }
            (_, Some(r)) => self.regs.set_r(z, r),
            _ => (),
        }
    }

    fn x3_ops(&mut self, z: u8, y: u8, q: u8, p: u8) {
        match z {
            0 => {
                self.scheduler.push(Operation::Delay(1));
                if self.if_cc(y) {
                    ret(self)
                }
            }
            1 => self.x3_z1_ops(y, q, p),
            2 => jp(self, Some(y)),
            3 => self.x3_z3_ops(y),
            4 => call(self, Some(y)),
            5 => self.x3_z5_ops(y, q, p),
            6 => alu(self, 3, y, z),
            7 => rst(self, y),
            _ => unreachable!("Invalid x3 instruction"),
        }
    }

    fn x3_z3_ops(&mut self, y: u8) {
        match y {
            0 => jp(self, None),
            1 => self.scheduler.push(Operation::Fetch),
            2 => outNa(self),
            3 => inNa(self),
            _ => unreachable!("Invalid x3_z3 instruction"),
        }
    }

    fn x3_z5_ops(&mut self, y: u8, q: u8, p: u8) {
        match (q, p) {
            (0, _) => push(self, p),
            (1, 0) => call(self, None),
            (1, 1) => self.scheduler.push(Operation::Fetch),
            _ => unreachable!(),
        }
    }

    fn x3_z1_ops(&mut self, y: u8, q: u8, p: u8) {
        match (q, p) {
            (0, _) => pop(self, p),
            (1, 0) => ret(self),
            (1, 1) => self.regs.exx(),
            _ => unreachable!("Invalid x3_z1 instruction"),
        }
    }

    pub fn if_cc(&self, y: u8) -> bool {
        match y {
            0 => self.regs.f.Z == false,
            1 => self.regs.f.Z == true,
            2 => self.regs.f.C == false,
            3 => self.regs.f.C == true,
            4 => self.regs.f.P == false,
            5 => self.regs.f.P == true,
            6 => self.regs.f.N == false,
            7 => self.regs.f.C == true,
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
                None => self.scheduler.push(Operation::MR_PC_N),
                Some(_) => {
                    let mut jump = true;
                    match y {
                        2 => {
                            self.regs.b = self.regs.b.wrapping_sub(1);
                            jump = self.regs.b != 0
                        }
                        3 => {}
                        4 => jump = self.regs.f.Z == false,
                        5 => jump = self.regs.f.Z == true,
                        6 => jump = self.regs.f.C == false,
                        7 => jump = self.regs.f.C == true,
                        _ => panic!(),
                    }
                    if jump {
                        let jump = self.fetched.n.unwrap() as i8;
                        println!("pc:{} jump{}", self.regs.pc, jump);
                        self.regs.pc = self.regs.pc.wrapping_add(jump as u16);
                        self.scheduler.push(Operation::Delay(6));
                    } else {
                        self.scheduler.push(Operation::Delay(1));
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
                    self.scheduler
                        .push(Operation::MW_8(self.regs.get_rr(p), self.regs.a));
                }
                2 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_PC_N);
                        self.scheduler.push(Operation::MR_PC_N);
                    }
                    Some(nn) => {
                        self.scheduler
                            .push(Operation::MW_16(nn, self.regs.get_rr(p)));
                    }
                },
                3 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_PC_N);
                        self.scheduler.push(Operation::MR_PC_N);
                    }
                    Some(nn) => {
                        self.scheduler.push(Operation::MW_8(nn, self.regs.a));
                    }
                },
                _ => panic!("x0_z2_ops q:{} p:{}", q, p),
            },
            1 => match p {
                0 | 1 => match self.fetched.n {
                    None => {
                        self.scheduler
                            .push(Operation::MR_ADDR_N(self.regs.get_rr(p)));
                    }
                    Some(n) => self.regs.a = n,
                },
                2 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_PC_N);
                        self.scheduler.push(Operation::MR_PC_N);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler.push(Operation::MR_ADDR_R(nn, 5));
                        self.scheduler.push(Operation::MR_ADDR_R(nn + 1, 4));
                    }
                },
                3 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_PC_N);
                        self.scheduler.push(Operation::MR_PC_N);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler.push(Operation::MR_ADDR_R(nn, 7));
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

    fn exec_interrupt(&self) {
        todo!()
    }

    // fn new_instruction(&self) {
    //     todo!()
    // }

    fn fetch(self: &mut Self) -> bool {
        self.current_ops_ts += 1;
        // println!("> [fetch] {}", self.current_ops_ts);
        match self.current_ops_ts {
            1 => {
                self.regs.m1 = true;
                self.signals.addr = self.regs.pc;
                self.signals.mem = SignalReq::Read;
                self.regs.pc += 1;
                self.regs.r = self.regs.r & 0x80 | ((self.regs.r + 1) & 0x7f);
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
                match self.fetched.prefix {
                    0xdd => self.regs.index_mode = IndexMode::Ix,
                    0xfd => self.regs.index_mode = IndexMode::Iy,
                    _ => self.regs.index_mode = IndexMode::Hl,
                }
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

    fn pr_r(self: &mut Self, addr: u16, r: u8) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => self.signals.addr = addr,
            2 => self.signals.port = SignalReq::Read,
            3 => {
                self.regs.set_r(r, self.signals.data);
                self.signals.port = SignalReq::None;
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
        println!("[mw_16] {}", self.current_ops_ts);
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
}
