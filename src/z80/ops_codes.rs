use super::cpu::CPU;
use crate::z80::cpu::Operation;

const overflowAddTable: [bool; 8] = [false, false, false, true, true, false, false, false];
const overflowSubTable: [bool; 8] = [false, true, false, false, false, false, true, false];
const halfcarryAddTable: [bool; 8] = [false, true, true, true, false, false, false, true];
const halfcarrySubTable: [bool; 8] = [false, false, true, false, true, false, true, true];
const parityTable: [bool; 256] = [
    true, false, false, true, false, true, true, false, false, true, true, false, true, false,
    false, true, false, true, true, false, true, false, false, true, true, false, false, true,
    false, true, true, false, false, true, true, false, true, false, false, true, true, false,
    false, true, false, true, true, false, true, false, false, true, false, true, true, false,
    false, true, true, false, true, false, false, true, false, true, true, false, true, false,
    false, true, true, false, false, true, false, true, true, false, true, false, false, true,
    false, true, true, false, false, true, true, false, true, false, false, true, true, false,
    false, true, false, true, true, false, false, true, true, false, true, false, false, true,
    false, true, true, false, true, false, false, true, true, false, false, true, false, true,
    true, false, false, true, true, false, true, false, false, true, true, false, false, true,
    false, true, true, false, true, false, false, true, false, true, true, false, false, true,
    true, false, true, false, false, true, true, false, false, true, false, true, true, false,
    false, true, true, false, true, false, false, true, false, true, true, false, true, false,
    false, true, true, false, false, true, false, true, true, false, true, false, false, true,
    false, true, true, false, false, true, true, false, true, false, false, true, false, true,
    true, false, true, false, false, true, true, false, false, true, false, true, true, false,
    false, true, true, false, true, false, false, true, true, false, false, true, false, true,
    true, false, true, false, false, true, false, true, true, false, false, true, true, false,
    true, false, false, true,
];

pub fn ld_rr_mm(cpu: &mut CPU, r: u8) {
    println!("ld_rr_mm r:{} nn:{:?}", r, cpu.fetched.nn);
    match cpu.fetched.nn {
        Some(v) => cpu.regs.set_rr(r, v),
        None => {
            cpu.scheduler.push(Operation::MR_N);
            cpu.scheduler.push(Operation::MR_N);
        }
    }
}

pub fn exafaf(cpu: &mut CPU) {
    (cpu.regs.a, cpu.regs.a_) = (cpu.regs.a_, cpu.regs.a);
    (cpu.regs.f, cpu.regs.f_) = (cpu.regs.f_, cpu.regs.f);
}

pub fn halt(cpu: &mut CPU) {
    cpu.halt = true;
    cpu.regs.pc -= 1
}

pub fn ld_r_n(cpu: &mut CPU, y: u8) {
    match cpu.fetched.n {
        None => cpu.scheduler.push(Operation::MR_N),
        Some(v) => match y {
            6 => {
                cpu.current_ops = None;
                cpu.scheduler.push(Operation::MW_8(cpu.regs.hl(), v));
            }
            _ => cpu.regs.set_r(y, v),
        },
    }
}

pub fn ld_r_r(cpu: &mut CPU, y: u8, z: u8) {
    match (y, z) {
        (6, _) => {
            cpu.current_ops = None;
            cpu.scheduler
                .push(Operation::MW_8(cpu.regs.hl(), cpu.regs.get_r(z)));
        }
        (_, 6) => match cpu.fetched.n {
            None => cpu.scheduler.push(Operation::MR_ADDR_N(cpu.regs.hl())),
            Some(n) => cpu.regs.set_r(y, n),
        },
        _ => {
            let v = cpu.regs.get_r(z);
            cpu.regs.set_r(y, v);
        }
    };
}

pub fn inc_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rp(p) + 1;
    cpu.regs.set_rr(p, v);
}

pub fn dec_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rp(p).wrapping_sub(1);
    cpu.regs.set_rr(p, v);
}

pub fn inc_r(cpu: &mut CPU, r: u8) {
    match r {
        6 => match cpu.fetched.n {
            None => cpu.scheduler.push(Operation::MR_ADDR_N(cpu.regs.hl())),
            Some(n) => {
                let v = inc(cpu, n);
                cpu.current_ops = None;
                cpu.scheduler.push(Operation::MW_8(cpu.regs.hl(), v));
            }
        },
        _ => {
            let mut v = cpu.regs.get_r(r);
            v = inc(cpu, v);
            cpu.regs.set_r(r, v);
        }
    }
}

pub fn inc(cpu: &mut CPU, v: u8) -> u8 {
    let r = v.wrapping_add(1);
    cpu.regs.f.S = r & 0x80 != 0;
    cpu.regs.f.Z = r == 0;
    cpu.regs.f.H = r & 0x0f == 0;
    cpu.regs.f.P = r == 0x80;
    cpu.regs.f.N = false;
    r
}

pub fn dec_r(cpu: &mut CPU, r: u8) {
    match r {
        6 => match cpu.fetched.n {
            None => cpu.scheduler.push(Operation::MR_ADDR_N(cpu.regs.hl())),
            Some(n) => {
                let v = dec(cpu, n);
                cpu.current_ops = None;
                cpu.scheduler.push(Operation::MW_8(cpu.regs.hl(), v));
            }
        },
        _ => {
            let mut v = cpu.regs.get_r(r);
            v = dec(cpu, v);
            cpu.regs.set_r(r, v);
        }
    }
}

pub fn dec(cpu: &mut CPU, v: u8) -> u8 {
    cpu.regs.f.H = v & 0x0f == 0;
    let r = v.wrapping_sub(1);
    cpu.regs.f.S = r & 0x80 != 0;
    cpu.regs.f.Z = r == 0;
    cpu.regs.f.P = r == 0x7f;
    cpu.regs.f.N = true;
    r
}

pub fn rlca(cpu: &mut CPU) {
    cpu.regs.a = (cpu.regs.a << 1) | (cpu.regs.a >> 7);
    cpu.regs.f.C = cpu.regs.a & 0x01 != 0;
    cpu.regs.f.H = false;
    cpu.regs.f.N = false;
}

pub fn rla(cpu: &mut CPU) {
    let c = cpu.regs.f.C;
    cpu.regs.f.C = cpu.regs.a & 0b10000000 != 0;
    cpu.regs.a = cpu.regs.a << 1;
    if c {
        cpu.regs.a |= 1;
    }
    cpu.regs.f.H = false;
    cpu.regs.f.N = false;
}

pub fn rrca(cpu: &mut CPU) {
    cpu.regs.f.C = cpu.regs.a & 0x01 != 0;
    cpu.regs.f.H = false;
    cpu.regs.f.N = false;
    cpu.regs.a = (cpu.regs.a >> 1) | (cpu.regs.a << 7);
}

pub fn rra(cpu: &mut CPU) {
    let c = cpu.regs.f.C;
    cpu.regs.f.C = cpu.regs.a & 1 != 0;
    cpu.regs.a = cpu.regs.a >> 1;
    if c {
        cpu.regs.a |= 0b10000000;
    }
    cpu.regs.f.H = false;
    cpu.regs.f.N = false;
}

pub fn add_hl_rr(cpu: &mut CPU, p: u8) {
    // rIdx := cpu.fetched.opCode >> 4 & 0b11
    // reg := cpu.getRRptr(rIdx)

    let v = cpu.regs.get_rp(p);
    let hl = cpu.regs.hl();
    let result = (hl as u32) + (v as u32);
    let lookup =
        (((hl & 0x0800) >> 11) | ((v & 0x0800) >> 10) | (((result as u16) & 0x0800) >> 9)) as u8;
    cpu.regs.set_hl(result as u16);

    cpu.regs.f.N = false;
    cpu.regs.f.H = halfcarryAddTable[lookup as usize];
    cpu.regs.f.C = (result & 0x10000) != 0;
}

pub fn daa(cpu: &mut CPU) {
    let mut c = cpu.regs.f.C;
    let mut add = 0u8;
    if cpu.regs.f.H || ((cpu.regs.a & 0x0f) > 9) {
        add = 6;
    }
    if c || (cpu.regs.a > 0x99) {
        add |= 0x60;
    }
    if cpu.regs.a > 0x99 {
        c = true;
    }
    if cpu.regs.f.N {
        sub_a(cpu, add);
    } else {
        add_a(cpu, add);
    }
    cpu.regs.f.S = (cpu.regs.a as i8) < 0;
    cpu.regs.f.Z = cpu.regs.a == 0;
    cpu.regs.f.P = parityTable[cpu.regs.a as usize];
    cpu.regs.f.C = c
}

pub fn cpl(cpu: &mut CPU) {
    cpu.regs.a = !cpu.regs.a;
    cpu.regs.f.H = true;
    cpu.regs.f.N = true;
}

pub fn scf(cpu: &mut CPU) {
    cpu.regs.f.H = false;
    cpu.regs.f.N = false;
    cpu.regs.f.C = true;
}

pub fn ccf(cpu: &mut CPU) {
    cpu.regs.f.H = cpu.regs.f.C;
    cpu.regs.f.N = false;
    cpu.regs.f.C = !cpu.regs.f.C;
}

pub enum ULA {
    AddA,
    AdcA,
    Sub,
    SbcA,
}

pub fn ula(cpu: &mut CPU, f: ULA, r: u8) {
    let mut v: Option<u8> = None;
    match r {
        6 => match cpu.fetched.n {
            None => cpu.scheduler.push(Operation::MR_ADDR_N(cpu.regs.hl())),
            Some(n) => v = Some(n),
        },
        _ => v = Some(cpu.regs.get_r(r)),
    }
    match v {
        Some(v) => match f {
            ULA::AddA => add_a(cpu, v),
            ULA::AdcA => adc_a(cpu, v),
            ULA::Sub => sub_a(cpu, v),
            ULA::SbcA => sbc_a(cpu, v),
        },
        None => (),
    }
}

fn sub_a(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as i16;
    let result = a - (v as i16);
    let lookup = ((cpu.regs.a & 0x88) >> 3) | (((v) & 0x88) >> 2) | (((result as u8) & 0x88) >> 1);
    cpu.regs.a = result as u8;

    cpu.regs.f.S = cpu.regs.a & 0x80 != 0;
    cpu.regs.f.Z = cpu.regs.a == 0;
    cpu.regs.f.H = halfcarrySubTable[(lookup & 0x07) as usize];
    cpu.regs.f.P = overflowSubTable[(lookup >> 4) as usize];
    cpu.regs.f.N = true;
    cpu.regs.f.C = ((result) & 0x100) == 0x100;
}

fn sbc_a(cpu: &mut CPU, s: u8) {
    let mut result = (cpu.regs.a as u16) - (s as u16);
    if cpu.regs.f.C {
        result -= 1;
    }
    let lookup = ((cpu.regs.a & 0x88) >> 3) | ((s & 0x88) >> 2) | (((result as u8) & 0x88) >> 1);
    cpu.regs.a = result as u8;
    cpu.regs.f.S = cpu.regs.a & 0x0080 != 0;
    cpu.regs.f.Z = cpu.regs.a == 0;
    cpu.regs.f.H = halfcarrySubTable[(lookup & 0x07) as usize];
    cpu.regs.f.P = overflowSubTable[(lookup >> 4) as usize];
    cpu.regs.f.N = true;
    cpu.regs.f.C = (result & 0x100) == 0x100;
}

fn adc_a(cpu: &mut CPU, v: u8) {
    let mut res = (cpu.regs.a as i16) + (v as i16);
    if cpu.regs.f.C {
        res += 1;
    }
    let lookup = ((cpu.regs.a & 0x88) >> 3) | ((v & 0x88) >> 2) | (((res as u8) & 0x88) >> 1);
    cpu.regs.a = res as u8;
    cpu.regs.f.S = cpu.regs.a & 0x80 != 0;
    cpu.regs.f.Z = cpu.regs.a == 0;
    cpu.regs.f.H = halfcarryAddTable[(lookup & 0x07) as usize];
    cpu.regs.f.P = overflowAddTable[(lookup >> 4) as usize];
    cpu.regs.f.N = false;
    cpu.regs.f.C = (res & 0x100) == 0x100;
}

fn add_a(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as i16;
    let result = a + (v as i16);
    let lookup = ((cpu.regs.a & 0x88) >> 3) | (((v) & 0x88) >> 2) | (((result as u8) & 0x88) >> 1);
    cpu.regs.a = (result & 0x00ff) as u8;

    cpu.regs.f.S = cpu.regs.a & 0x80 != 0;
    cpu.regs.f.Z = cpu.regs.a == 0;
    cpu.regs.f.H = halfcarryAddTable[(lookup & 0x07) as usize];
    cpu.regs.f.P = overflowAddTable[(lookup >> 4) as usize];
    cpu.regs.f.N = false;
    cpu.regs.f.C = ((result) & 0x100) != 0;
}

pub fn subA(cpu: &mut CPU, r: u8) {
    let a = cpu.regs.a as i16;
    let result = a - (r as i16);
    let lookup = ((cpu.regs.a & 0x88) >> 3) | (((r) & 0x88) >> 2) | (((result as u8) & 0x88) >> 1);
    cpu.regs.a = (result & 0x00ff) as u8;

    cpu.regs.f.S = cpu.regs.a & 0x80 != 0;
    cpu.regs.f.Z = cpu.regs.a == 0;
    cpu.regs.f.H = halfcarrySubTable[(lookup & 0x07) as usize];
    cpu.regs.f.P = overflowSubTable[(lookup >> 4) as usize];
    cpu.regs.f.N = true;
    cpu.regs.f.C = ((result) & 0x100) == 0x100;
}
