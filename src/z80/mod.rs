use self::ops_codes::*;

pub fn hello() -> String {
    "Hello!".to_string()
}

mod ops_codes;

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
    doInterrupt: bool,
    halt: bool,
    current_ops: Option<Operation>,
    current_ops_ts: u8,
    opsCodes: [OpCode; 256],
}

struct Fetched {
    opCode: u8,
    prefix: u16,
    n: u8,
    n2: u8,
    nn: u16,
    // op: Option<&'a OpCode<T, O>>,
}

#[derive(Debug, Default)]
pub struct Registers {
    A: u8,
    F: u8,

    B: u8,
    C: u8,

    D: u8,
    E: u8,

    H: u8,
    L: u8,

    SP: u16,
    PC: u16,
    M1: bool,
    R: u8,
}

#[derive(Copy, Clone)]
pub enum Operation {
    Fetch,
    MRnnPC,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            regs: Default::default(),
            signals: Signals {
                addr: 0,
                data: 0,
                mem: SignalReq::None,
                port: SignalReq::None,
            },
            fetched: Fetched {
                opCode: 0,
                prefix: 0,
                n: 0,
                n2: 0,
                nn: 0,
            },
            scheduler: Vec::new(),
            wait: false,
            doInterrupt: false,
            halt: false,
            current_ops: Some(Operation::Fetch),
            current_ops_ts: 0,
            opsCodes: ops_codes(),
        }
    }

    pub fn tick(self: &mut Self) {
        if self.wait {
            return;
        }

        if self.halt {
            if self.doInterrupt {
                self.halt = false;
                self.regs.PC += 1;
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
                if self.doInterrupt {
                    self.exec_interrupt()
                } else {
                    self.current_ops = Some(Operation::Fetch);
                }
            } else {
                self.current_ops = Some(self.scheduler.remove(0));
            }
        }

        match &self.current_ops {
            Some(op) => {
                let done = match op {
                    Operation::Fetch => self.fectch(),
                    Operation::MRnnPC => self.mrNNpc(),
                };
                if done {
                    self.current_ops = None;
                    self.current_ops_ts = 0;
                    if self.scheduler.is_empty() {
                        let opc = &self.opsCodes[self.fetched.opCode as usize];
                        println!("-> {}", opc.name);
                        match opc.on_fetch {
                            Some(f) => f(self),
                            None => (),
                        }
                    }
                }
            }
            None => todo!(),
        };
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
                self.regs.M1 = true;
                self.signals.addr = self.regs.PC;
                self.signals.mem = SignalReq::Read;
                self.regs.PC += 1;
                self.regs.R = self.regs.R & 0x80 | ((self.regs.R + 1) & 0x7f);
            }
            2 => {}
            3 => {
                self.regs.M1 = false;
                self.signals.mem = SignalReq::None;
                self.fetched.prefix = self.fetched.prefix << 8;
                self.fetched.prefix |= self.fetched.opCode as u16;
                self.fetched.opCode = self.signals.data;
            }
            4 => {
                let opc = &self.opsCodes[self.fetched.opCode as usize];
                for o in &opc.ops {
                    self.scheduler.push(*o);
                }
                // let op = &self.table[self.fetched.opCode as usize];
                // for o in &op.ops {
                //     self.scheduler.push(o);
                // }
                // self.fetched.op = Some(op);
                // match op.on_fetch {
                //     Some(onFetch) => onFetch(&cpu),
                //     None => (),
                // }
                // self.done = true;
                return true;
            }
            _ => panic!(),
        }
        false
    }

    fn mrNNpc(self: &mut Self) -> bool {
        self.current_ops_ts += 1;
        // println!("> [fetch] {}", self.current_ops_ts);
        // println("> [fetch]", ops.t, "pc:", fmt.Sprintf("0x%04X", cpu.regs.PC))
        match self.current_ops_ts {
            1 => {
                self.signals.addr = self.regs.PC;
                self.signals.mem = SignalReq::Read;
                self.regs.PC += 1;
            }
            2 => {}
            3 => {
                self.fetched.n = self.signals.data;
                self.signals.mem = SignalReq::None;
            }
            4 => {
                self.signals.addr = self.regs.PC;
                self.signals.mem = SignalReq::Read;
                self.regs.PC += 1;
            }
            5 => {}
            6 => {
                self.fetched.n2 = self.signals.data;
                self.signals.mem = SignalReq::None;
                self.fetched.nn = (self.fetched.n as u16) | ((self.fetched.n2 as u16) << 8);
                return true;
            }
            _ => panic!(),
        }
        false
    }
}
