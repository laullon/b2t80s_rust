use super::*;

pub trait z80op {
    fn tick(&mut self);
    fn isDone(&self) -> bool;
}

struct Fetch<'a, BUS, OPS> {
    cpu: CPU<'a, BUS, OPS>,
    t: u8,
    table: Vec<&'a OpCode<BUS, OPS>>,
    done: bool,
}

impl<'a, BUS, OPS> z80op for Fetch<'a, BUS, OPS>
where
    BUS: Bus,
    OPS: z80op,
{
    fn tick(self: &mut Self) {
        self.t += 1;

        match self.t {
            1 => {
                self.cpu.regs.M1 = true;
                self.cpu.bus.SetAddr(self.cpu.regs.PC);
                self.cpu.regs.PC += 1;
                self.cpu.regs.R = self.cpu.regs.R & 0x80 | ((self.cpu.regs.R + 1) & 0x7f);
            }
            2 => {}
            3 => {
                self.cpu.regs.M1 = false;
                self.cpu.bus.ReadMemory();
                let d = self.cpu.bus.GetData();
                self.cpu.bus.Release();
                self.cpu.fetched.prefix = self.cpu.fetched.prefix << 8;
                self.cpu.fetched.prefix |= (self.cpu.fetched.opCode as u16);
                self.cpu.fetched.opCode = d;
            }
            4 => {
                let op = self.table[self.cpu.fetched.opCode as usize];
                for o in &op.ops {
                    self.cpu.scheduler.push(o);
                }
                self.cpu.fetched.op = Some(op);
                match self.cpu.fetched.op.unwrap().onFetch {
                    Some(onFetch) => onFetch(&self.cpu),
                    None => (),
                }
                self.done = true;
            }
            _ => (),
        }
        // println("> [fetch]", self.t, "pc:", fmt.Sprintf("0x%04X", cpu.regs.PC))
    }

    fn isDone(&self) -> bool {
        todo!()
    }
}
