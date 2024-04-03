use self::logic::z80op;

pub fn hello() -> String {
    "Hello!".to_string()
}

mod logic;
pub struct CPU<'a, BUS, OPS> {
    regs: Registers,
    bus: BUS,
    fetched: Fetched<'a, BUS, OPS>,
    scheduler: Vec<&'a OPS>,
    wait: bool,
    doInterrupt: bool,
    halt: bool,
}

type z80f<T, O> = fn(&CPU<T, O>);

struct OpCode<BUS, OPS> {
    name: String,
    mask: u8,
    code: u8,
    len: u8,
    ops: Vec<OPS>,
    onFetch: Option<z80f<BUS, OPS>>,
}

struct Fetched<'a, T, O> {
    opCode: u8,
    prefix: u16,
    op: Option<&'a OpCode<T, O>>,
}

struct Registers {
    PC: u16,
    M1: bool,
    R: u8,
}

pub trait Bus {
    fn SetAddr(&self, addr: u16);
    fn ReadMemory(&self);
    fn GetData(&self) -> u8;
    fn Release(&self);
}

impl<'a, BUS, OPS> CPU<'a, BUS, OPS>
where
    BUS: Bus,
    OPS: z80op,
{
    pub fn new(bus: BUS) -> Self {
        Self {
            regs: Registers {
                PC: 0,
                M1: false,
                R: 0,
            },
            bus,
            fetched: Fetched {
                opCode: 0,
                prefix: 0,
                op: None,
            },
            scheduler: Vec::new(),
            wait: false,
            doInterrupt: false,
            halt: false,
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

        if self.scheduler.first().unwrap().isDone() {
            self.scheduler.remove(0);
            if self.scheduler.is_empty() {
                // if self.log != nil {
                //     self.log.AppendLastOP(self.fetched.getInstruction())
                // }
                if self.doInterrupt {
                    self.exec_interrupt()
                } else {
                    self.new_instruction()
                }
            }
        }
        match self.scheduler.first() {
            Some(op) => **op.tick(),
            None => todo!(),
        }
    }

    fn exec_interrupt(&self) {
        todo!()
    }

    fn new_instruction(&self) {
        todo!()
    }
}
