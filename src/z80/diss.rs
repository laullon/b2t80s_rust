use crate::z80::cpu::decode;

use super::cpu::Fetched;

static ALU: [&str; 8] = [
    "ADD A,", "ADC A,", "SUB A,", "SBC", "AND", "XOR", "OR", "CP",
];

static R: [&str; 8] = ["B", "C", "D", "E", "H", "L", "(HL)", "A"];
static RP: [&str; 4] = ["BC", "DE", "HL", "SP"];
static RP2: [&str; 4] = ["BC", "DE", "HL", "AF"];

static EDX1Z7: [&str; 8] = [
    "LD I, A", "LD R, A", "LD A, I", "LD A, R", "RRD", "RLD", "NOP", "NOP",
];
static CC: [&str; 8] = ["NZ", "Z", "NC", "C", "PO", "PE", "P", "M"];

pub fn disassemble(fetched: Fetched) -> String {
    let (x, y, z, p, q) = decode(fetched.op_code);

    let mut res: String = match (
        fetched.prefix,
        x,
        y as usize,
        z as usize,
        p as usize,
        q as usize,
    ) {
        (0x00 | 0xDD | 0xFD, 0, 0, 0, _, _) => "NOP".to_string(),
        (0x00 | 0xDD | 0xFD, 0, 1, 0, _, _) => "EX AF, AF'".to_string(),
        (0x00 | 0xDD | 0xFD, 0, 2 | 3, 0, _, _) => format!(
            "{} 0x{:04x}",
            ["DJNZ", "JR"][y as usize - 2],
            to_abs_adrr(fetched.pc, fetched.n.unwrap())
        ),
        (0x00 | 0xDD | 0xFD, 0, 4..=7, 0, _, _) => format!(
            "JR {}, 0x{:04x}",
            CC[y as usize - 4],
            to_abs_adrr(fetched.pc, fetched.n.unwrap())
        ),

        (0x00 | 0xDD | 0xFD, 0, _, 1, p, 0) => {
            format!("LD {}, 0x{:04x}", RP[p], fetched.nn.unwrap())
        }
        (0x00 | 0xDD | 0xFD, 0, _, 1, p, 1) => format!("ADD HL, {}", RP[p]),

        (0x00 | 0xDD | 0xFD, 0, _, 2, 0 | 1, 0) => format!("LD {}, A", RP[p as usize]),
        (0x00 | 0xDD | 0xFD, 0, _, 2, 0 | 1, 1) => format!("LD A, {}", RP[p as usize]),
        (0x00 | 0xDD | 0xFD, 0, _, 2, 2 | 3, 0) => format!(
            "LD (0x{:04x}), {}",
            fetched.nn.unwrap(),
            ["HL", "A"][p as usize - 2]
        ),
        (0x00 | 0xDD | 0xFD, 0, _, 2, 2 | 3, 1) => format!(
            "LD {}, (0x{:04x})",
            ["HL", "A"][p as usize - 2],
            fetched.nn.unwrap()
        ),

        (0x00 | 0xDD | 0xFD, 0, _, 3, p, q) => format!("{} {}", ["INC", "DEC"][q], RP[p]),

        (0x00 | 0xDD | 0xFD, 0, y, 4, _, _) => format!("INC {}", R[y]),
        (0x00 | 0xDD | 0xFD, 0, y, 5, _, _) => format!("DEC {}", R[y]),
        (0x00 | 0xDD | 0xFD, 0, y, 6, _, _) => format!("LD {}, 0x{:02x}", R[y], fetched.n.unwrap()),
        (0x00 | 0xDD | 0xFD, 0, y, 7, _, _) => format!(
            "{}",
            ["RLCA", "RRCA", "RLA", "RRA", "DAA", "CPL", "SCF", "CCF"][y]
        ),

        (0x00 | 0xDD | 0xFD, 1, 0, 6, _, _) => "HALT".to_string(),
        (0x00 | 0xDD | 0xFD, 1, y, z, _, _) => format!("LD {}, {}", R[y], R[z]),

        (0x00 | 0xDD | 0xFD, 2, y, z, _, _) => format!("{} {}", ALU[y], R[z]),

        (0x00 | 0xDD | 0xFD, 3, y, 0, _, _) => format!("RET {}", CC[y]),

        (0x00 | 0xDD | 0xFD, 3, _, 1, p, 0) => format!("POP {}", RP2[p]),
        (0x00 | 0xDD | 0xFD, 3, _, 1, p, 1) => {
            ["RET", "EXX", "JP HL", "LD SP, HL"][p as usize].to_string()
        }
        (0x00 | 0xDD | 0xFD, 3, y, 2, _, _) => {
            format!("JP {}, 0x{:04x}", CC[y], fetched.nn.unwrap())
        }

        (0x00 | 0xDD | 0xFD, 3, 0, 3, _, _) => format!("JP 0x{:04x}", fetched.nn.unwrap()),
        (0x00 | 0xDD | 0xFD, 3, 2, 3, _, _) => format!("OUT (0x{:02x}), A", fetched.n.unwrap()),
        (0x00 | 0xDD | 0xFD, 3, 3, 3, _, _) => format!("IN A, (0x{:02x})", fetched.n.unwrap()),
        (0x00 | 0xDD | 0xFD, 3, 4..=7, 3, _, _) => {
            ["EX (SP), HL", "EX DE, HL", "DI", "EI"][y as usize - 4].to_string()
        }

        (0x00 | 0xDD | 0xFD, 3, y, 4, _, _) => {
            format!("CALL {}, 0x{:04x}", CC[y], fetched.nn.unwrap())
        }

        (0x00 | 0xDD | 0xFD, 3, _, 5, p, 0) => format!("PUSH {}", RP2[p]),
        (0x00 | 0xDD | 0xFD, 3, _, 5, 0, 1) => format!("CALL 0x{:04x}", fetched.nn.unwrap()),

        (0x00 | 0xDD | 0xFD, 3, y, 6, _, _) => format!("{} {}", ALU[y], fetched.n.unwrap()),
        (0x00 | 0xDD | 0xFD, 3, y, 7, _, _) => format!("RST 0x{:02x}", y * 8),

        /* CB */
        (0xCB, 0, y, z, _, _) => format!(
            "{} {}",
            ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL"][y],
            R[z]
        ),
        (0xCB, 1, y, z, _, _) => format!("BIT {}, {}", y, R[z]),
        (0xCB, 2, y, z, _, _) => format!("RES {}, {}", y, R[z]),
        (0xCB, 3, y, z, _, _) => format!("SET {}, {}", y, R[z]),

        /* DDCB/FDCB */
        (0xDDCB, 0, y, _, _, _) => format!(
            "{} (IX+{})",
            ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL"][y],
            fetched.d.unwrap()
        ),
        (0xDDCB, 1, y, _, _, _) => format!("BIT {}, (IX+{})", y, fetched.d.unwrap()),
        (0xDDCB, 2, y, _, _, _) => format!("RES {}, (IX+{})", y, fetched.d.unwrap()),
        (0xDDCB, 3, y, _, _, _) => format!("SET {}, (IX+{})", y, fetched.d.unwrap()),
        (0xFDCB, 0, y, _, _, _) => format!(
            "{} (IY+{})",
            ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SLL", "SRL"][y],
            fetched.d.unwrap()
        ),
        (0xFDCB, 1, y, _, _, _) => format!("BIT {}, (IY+{})", y, fetched.d.unwrap()),
        (0xFDCB, 2, y, _, _, _) => format!("RES {}, (IY+{})", y, fetched.d.unwrap()),
        (0xFDCB, 3, y, _, _, _) => format!("SET {}, (IY+{})", y, fetched.d.unwrap()),

        /* ED */
        (0xED, 1, 6, 0, _, _) => "IN A, (C)".to_string(),
        (0xED, 1, y, 0, _, _) => format!("IN {}, (C)", R[y]),

        (0xED, 1, 6, 1, _, _) => "OUT (C), 0".to_string(),
        (0xED, 1, y, 1, _, _) => format!("OUT (C), {}", R[y]),

        (0xED, 1, _, 2, p, 0) => format!("SBC HL, {}", RP[p]),
        (0xED, 1, _, 2, p, 1) => format!("ADC HL, {}", RP[p]),

        (0xED, 1, _, 3, p, 0) => format!("LD ({}), {}", fetched.nn.unwrap(), RP[p]),
        (0xED, 1, _, 3, p, 1) => format!("LD {}, ({})", RP[p], fetched.nn.unwrap()),

        (0xED, 1, _, 4, _, _) => "NEG".to_string(),

        (0xED, 1, 1, 5, _, _) => "RETI".to_string(),
        (0xED, 1, _, 5, _, _) => "RETN".to_string(),

        (0xED, 1, y, 6, _, _) => format!("IM {}", y),

        (0xED, 1, y, 7, _, _) => EDX1Z7[y].to_string(),

        (0xED, 2, y, z, _, _) => [
            ["LDI", "CPI", "INI", "OUTI"],
            ["LDD", "CPD", "IND", "OUTD"],
            ["LDIR", "CPIR", "INIR", "OTIR"],
            ["LDDR", "CPDR", "INDR", "OTDR"],
        ][y - 4][z]
            .to_string(),
        _ => panic!(
            "Unknown instruction: pc:{:04x} prefix:{:04x} opCode:{:02x}",
            fetched.pc, fetched.prefix, fetched.op_code,
        ),
    };
    if fetched.prefix == 0xDD {
        if res.contains("(HL)") {
            res = res.replace(
                " (HL)",
                format!(" (IX+{})", fetched.d.unwrap_or(0)).as_str(),
            );
        } else {
            res = res.replace(" HL", " IX");
            res = res.replace(" L", " IXL");
            res = res.replace(" H", " IXH");
        }
    } else if fetched.prefix == 0xFD {
        if res.contains("(HL)") {
            res = res.replace(" (HL)", format!(" (IY+{}", fetched.d.unwrap_or(0)).as_str());
        } else {
            res = res.replace(" HL", " IY");
            res = res.replace(" L", " IYL");
            res = res.replace(" H", " IYH");
        }
    }
    format!("{:04x} {}", fetched.pc, res)
}

fn to_abs_adrr(pc: u16, n: u8) -> u16 {
    let jump: i8 = n as i8;
    pc.wrapping_add(2).wrapping_add(jump as u16)
}
