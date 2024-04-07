use self::{ops_codes::*, registers::Registers};

pub fn hello() -> String {
    "Hello!".to_string()
}

mod ops_codes;
pub mod registers;

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
    fetched: Fetched,
    scheduler: Vec<Operation>,
    wait: bool,
    do_interrupt: bool,
    halt: bool,
    current_ops: Option<Operation>,
    current_ops_ts: u8,
}

struct Fetched {
    op_code: Option<u8>,
    prefix: u16,
    n: Option<u8>,
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
    Delay,
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
                    Operation::Delay => self.delay(),
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
            4 => (),
            5 => (),
            6 => (),
            7 => (),
            _ => panic!(),
        }
    }

    fn x0_z0_ops(&mut self, y: u8) {
        match y {
            0 => (), // NOP
            1 => exafaf(self),
            _ => todo!("x0_z0_ops y:{}", y),
        }
    }

    fn x1_ops(&mut self, z: u8, y: u8) {
        match z {
            6 => halt(self),
            _ => panic!(),
        }
    }

    fn x0_z1_ops(&mut self, q: u8, p: u8) {
        match q {
            0 => ld_rr_mm(self, p),
            1 => {
                add_hl_rr(self, p);
                self.fetched.op_code = None;
                self.scheduler.push(Operation::Delay);
                self.scheduler.push(Operation::Delay);
                self.scheduler.push(Operation::Delay);
                self.scheduler.push(Operation::Delay);
                self.scheduler.push(Operation::Delay);
                self.scheduler.push(Operation::Delay);
                self.scheduler.push(Operation::Delay);
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
                _ => panic!("x0_z2_ops q:{} p:{}", q, p),
            },
            1 => match p {
                0 => match self.fetched.n {
                    None => {
                        self.scheduler.push(Operation::MR_ADDR_N(self.regs.bc()));
                    }
                    Some(n) => self.regs.a = n,
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
        self.scheduler.push(Operation::Delay);
        self.scheduler.push(Operation::Delay);
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

    fn delay(self: &mut Self) -> bool {
        true
    }
}
