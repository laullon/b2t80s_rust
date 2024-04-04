use super::{Operation, CPU};

type z80f = fn(&mut CPU);

pub struct OpCode {
    pub name: String,
    mask: u8,
    code: u8,
    len: u8,
    pub ops: Vec<Operation>,
    pub on_fetch: Option<z80f>,
}

impl OpCode {
    fn new(
        name: String,
        mask: u8,
        code: u8,
        len: u8,
        ops: Vec<Operation>,
        on_fetch: Option<z80f>,
    ) -> Self {
        Self {
            name,
            mask,
            code,
            len,
            ops,
            on_fetch,
        }
    }
}

pub fn ops_codes() -> [OpCode; 256] {
    let mut res: [OpCode; 256] =
        core::array::from_fn(|i| OpCode::new(String::from("bad"), 0, 0, 1, vec![], Some(crash)));

    res[0] = OpCode::new(String::from("NOP"), 0xff, 0, 1, vec![], None);
    res[1] = OpCode::new(
        String::from("LD dd, mm"),
        0xff,
        0,
        1,
        vec![Operation::MRnnPC],
        Some(ldDDmm),
    );

    res
}

fn crash(cpu: &mut CPU) {
    panic!("bad opCode")
}

// static OPSCODES: [OpCode<Bus,z80op>;1] = [
//     OpCode::new("LD dd, mm", 0b11001111, 0b00000001, 3, []z80op{&mrNNpc{f: ldDDmm}}, nil),
//     ];

//     // OpCode{"LD dd, mm", 0b11001111, 0b00000001, 3, []z80op{&mrNNpc{f: ldDDmm}}, nil},
// 	{"NOP", 0xFF, 0x00, 1, []z80op{}, nil},

fn ldDDmm(cpu: &mut CPU) {
    let t = cpu.fetched.opCode >> 4 & 0b11;
    match t {
        0b00 => {
            cpu.regs.B = cpu.fetched.n2;
            cpu.regs.C = cpu.fetched.n;
        }
        0b01 => {
            cpu.regs.D = cpu.fetched.n2;
            cpu.regs.E = cpu.fetched.n;
        }
        0b10 => {
            cpu.regs.H = cpu.fetched.n2;
            cpu.regs.L = cpu.fetched.n;
        }
        0b11 => {
            cpu.regs.SP = cpu.fetched.nn;
        }
        _ => panic!("!!!!"),
    }
}
