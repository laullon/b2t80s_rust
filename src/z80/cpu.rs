use super::{ops_codes::*, registers::Registers};

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
    op_code: Option<u8>,
    prefix: u16,
    pub n: Option<u8>,
    pub nn: Option<u16>,
    // op: Option<&'a OpCode<T, O>>,
}

#[derive(Copy, Clone)]
pub enum Operation {
    Fetch,
    MR_N,
    MW_8(u16, u8),
    MW_16(u16, u16),
    MR_ADDR_N(u16),
    MR_ADDR_R(u16, u8),
    Delay(u8),
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
                    Operation::Fetch => self.fectch(),
                    Operation::MR_N => self.mr(),
                    Operation::MW_8(addr, data) => self.mw_8(addr, data),
                    Operation::MW_16(addr, data) => self.mw_16(addr, data),
                    Operation::MR_ADDR_N(addr) => self.mr_addr_n(addr),
                    Operation::MR_ADDR_R(addr, r) => self.mr_addr_r(addr, r),
                    Operation::Delay(delay) => self.delay(delay),
                };
                if done {
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
        if matches!(self.fetched.op_code, None) {
            return;
        };

        let op_code = self.fetched.op_code.unwrap();
        let x = (op_code & 0b11000000) >> 6;
        let y = (op_code & 0b00111000) >> 3;
        let z = (op_code & 0b00000111) >> 0;
        let p = y >> 1;
        let q = y & 0b00000001;

        println!(
            "opc:{:02x} x:{} y:{} z:{} p:{} q:{}",
            op_code, x, y, z, p, q
        );
        match x {
            0 => self.x0_ops(z, y, q, p),
            1 => self.x1_ops(y, z),
            2 => self.alu(y, z),
            _ => todo!("decode_and_run x:{}", x),
        }
        // let opc = &self.opsCodes[self.fetched.opCode as usize];
        // println!("-> {}", opc.name);
        // match opc.on_fetch {
        //     Some(f) => f(self),
        //     None => (),
        // }
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
            1 => exafaf(self),
            2..=7 => match self.fetched.n {
                None => self.scheduler.push(Operation::MR_N),
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

    fn alu(&mut self, y: u8, z: u8) {
        match y {
            0 => ula(self, ULA::AddA, z),
            1 => ula(self, ULA::AdcA, z),
            2 => ula(self, ULA::Sub, z),
            3 => ula(self, ULA::SbcA, z),
            _ => panic!("alu y:{}", y),
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
                0 => {
                    self.scheduler
                        .push(Operation::MW_8(self.regs.bc(), self.regs.a));
                }
                1 => {
                    self.scheduler
                        .push(Operation::MW_8(self.regs.de(), self.regs.a));
                }
                2 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_N);
                        self.scheduler.push(Operation::MR_N);
                    }
                    Some(nn) => {
                        self.scheduler.push(Operation::MW_16(self.regs.hl(), nn));
                    }
                },
                3 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_N);
                        self.scheduler.push(Operation::MR_N);
                    }
                    Some(nn) => {
                        self.scheduler.push(Operation::MW_8(nn, self.regs.a));
                    }
                },
                _ => panic!("x0_z2_ops q:{} p:{}", q, p),
            },
            1 => match p {
                0 => match self.fetched.n {
                    None => {
                        self.scheduler.push(Operation::MR_ADDR_N(self.regs.bc()));
                    }
                    Some(n) => self.regs.a = n,
                },
                1 => match self.fetched.n {
                    None => {
                        self.scheduler.push(Operation::MR_ADDR_N(self.regs.de()));
                    }
                    Some(n) => self.regs.a = n,
                },
                2 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_N);
                        self.scheduler.push(Operation::MR_N);
                    }
                    Some(nn) => {
                        self.fetched.op_code = None;
                        self.scheduler.push(Operation::MR_ADDR_R(nn, 5));
                        self.scheduler.push(Operation::MR_ADDR_R(nn + 1, 4));
                    }
                },
                3 => match self.fetched.nn {
                    None => {
                        self.scheduler.push(Operation::MR_N);
                        self.scheduler.push(Operation::MR_N);
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

    fn fectch(self: &mut Self) -> bool {
        self.current_ops_ts += 1;
        // println!("> [fetch] {}", self.current_ops_ts);
        // println("> [fetch]", ops.t, "pc:", fmt.Sprintf("0x%04X", cpu.regs.PC))
        match self.current_ops_ts {
            1 => {
                self.fetched.n = None;
                self.fetched.nn = None;
                self.fetched.op_code = None;

                self.regs.m1 = true;
                self.signals.addr = self.regs.pc;
                self.signals.mem = SignalReq::Read;
                self.regs.pc += 1;
                self.regs.r = self.regs.r & 0x80 | ((self.regs.r + 1) & 0x7f);
            }
            2 => {
                self.regs.m1 = false;
                self.signals.mem = SignalReq::None;
                // self.fetched.prefix = self.fetched.prefix << 8;
                // self.fetched.prefix |= self.fetched.op_code as u16;
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
                self.fetched.n = Some(self.signals.data);
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

    fn mw_16(self: &mut Self, addr: u16, data: u16) -> bool {
        self.current_ops_ts += 1;
        match self.current_ops_ts {
            1 => {
                self.signals.addr = addr;
                self.signals.data = (data >> 8) as u8;
            }
            2 => self.signals.mem = SignalReq::Write,
            3 => {
                self.signals.mem = SignalReq::None;
                return true;
            }
            4 => {
                self.signals.addr = addr + 1;
                self.signals.data = data as u8;
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
