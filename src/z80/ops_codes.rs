use super::{cpu::CPU, registers::IndexMode};
use crate::z80::cpu::Operation;

const OVERFLOW_ADD_TABLE: [bool; 8] = [false, false, false, true, true, false, false, false];
const OVERFLOW_SUB_TABLE: [bool; 8] = [false, true, false, false, false, false, true, false];
const HALFCARRY_ADD_TABLE: [bool; 8] = [false, true, true, true, false, false, false, true];
const HALFCARRY_SUB_TABLE: [bool; 8] = [false, false, true, false, true, false, true, true];
pub const PARITY_TABLE: [bool; 256] = [
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
    match cpu.fetched.nn {
        Some(v) => cpu.regs.set_rr(r, v),
        None => {
            cpu.scheduler.push(Operation::MrPcN);
            cpu.scheduler.push(Operation::MrPcN);
        }
    }
}

pub fn halt(cpu: &mut CPU) {
    cpu.halt = true;
    cpu.regs.pc -= 1
}

pub fn ld_r_n(cpu: &mut CPU, y: u8) {
    match (y, cpu.regs.index_mode, cpu.fetched.d, cpu.fetched.n) {
        (6, IndexMode::Ix | IndexMode::Iy, None, None) => cpu.scheduler.push(Operation::MrPcD),
        (6, IndexMode::Ix | IndexMode::Iy, Some(_), None) => cpu.scheduler.push(Operation::MrPcN),
        (6, IndexMode::Ix | IndexMode::Iy, Some(d), Some(n)) => {
            cpu.fetched.op_code = None;
            cpu.scheduler.push(Operation::Delay(2));
            cpu.scheduler.push(Operation::Mw8(cpu.regs.get_idx(d), n));
        }
        (_, _, _, None) => cpu.scheduler.push(Operation::MrPcN),
        (6, _, _, Some(v)) => {
            cpu.fetched.op_code = None;
            cpu.scheduler.push(Operation::Mw8(cpu.regs.get_rr(2), v))
        }
        (_, _, _, Some(v)) => cpu.regs.set_r(y, v),
    }
}

pub fn ld_r_r(cpu: &mut CPU, y: u8, z: u8) {
    println!(
        "-> {} {} {:?} {:?} {:?}",
        y, z, cpu.regs.index_mode, cpu.fetched.d, cpu.fetched.n
    );
    match (y, z, cpu.regs.index_mode, cpu.fetched.d, cpu.fetched.n) {
        (6, _, IndexMode::Ix | IndexMode::Iy, None, None) => cpu.scheduler.push(Operation::MrPcD),
        (6, _, IndexMode::Ix | IndexMode::Iy, Some(d), None) => {
            let addr = cpu.regs.get_idx(d);
            cpu.regs.index_mode = IndexMode::Hl;
            cpu.scheduler.push(Operation::Mw8(addr, cpu.regs.get_r(z)));
            cpu.fetched.op_code = None;
            cpu.scheduler.push(Operation::Delay(5));
        }

        (_, 6, IndexMode::Ix | IndexMode::Iy, None, None) => cpu.scheduler.push(Operation::MrPcD),
        (_, 6, IndexMode::Ix | IndexMode::Iy, Some(d), None) => {
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_idx(d)))
        }
        (_, 6, IndexMode::Ix | IndexMode::Iy, Some(_), Some(n)) => {
            cpu.scheduler.push(Operation::Delay(5));
            cpu.fetched.op_code = None;
            cpu.regs.index_mode = IndexMode::Hl;
            cpu.regs.set_r(y, n);
        }

        (6, _, _, _, _) => {
            cpu.fetched.op_code = None;
            cpu.scheduler
                .push(Operation::Mw8(cpu.regs.get_rr(2), cpu.regs.get_r(z)));
        }
        (_, 6, _, _, _) => match cpu.fetched.n {
            None => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
            Some(n) => cpu.regs.set_r(y, n),
        },
        _ => {
            let v = cpu.regs.get_r(z);
            cpu.regs.set_r(y, v);
        }
    };
}

pub fn inc_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rr(p) + 1;
    cpu.regs.set_rr(p, v);
}

pub fn dec_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rr(p).wrapping_sub(1);
    cpu.regs.set_rr(p, v);
}

pub fn dec_r(cpu: &mut CPU, r: u8) {
    inc_dec_r(cpu, r, false);
}

pub fn inc_r(cpu: &mut CPU, r: u8) {
    inc_dec_r(cpu, r, true);
}

fn inc_dec_r(cpu: &mut CPU, r: u8, is_inc: bool) {
    match (r, cpu.regs.index_mode) {
        (6, IndexMode::Hl) => match cpu.fetched.n {
            None => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
            Some(n) => {
                let mut v = n;
                if is_inc {
                    v = inc(cpu, v);
                } else {
                    v = dec(cpu, v);
                }
                cpu.fetched.op_code = None;
                cpu.scheduler.push(Operation::Delay(1));
                cpu.scheduler.push(Operation::Mw8(cpu.regs.get_rr(2), v));
            }
        },
        (6, _) => match (cpu.fetched.n, cpu.fetched.d) {
            (None, None) => cpu.scheduler.push(Operation::MrPcD),
            (None, Some(d)) => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_idx(d))),
            (Some(n), Some(d)) => {
                let mut v = n;
                if is_inc {
                    v = inc(cpu, v);
                } else {
                    v = dec(cpu, v);
                }

                cpu.fetched.op_code = None;
                cpu.scheduler.push(Operation::Delay(6));
                cpu.scheduler.push(Operation::Mw8(cpu.regs.get_idx(d), v));
            }
            _ => unreachable!("Invalid inc_r instruction"),
        },
        _ => {
            let mut v = cpu.regs.get_r(r);
            if is_inc {
                v = inc(cpu, v);
            } else {
                v = dec(cpu, v);
            }
            cpu.regs.set_r(r, v);
        }
    }
}

pub fn inc(cpu: &mut CPU, v: u8) -> u8 {
    let r = v.wrapping_add(1);
    cpu.regs.f.s = r & 0x80 != 0;
    cpu.regs.f.z = r == 0;
    cpu.regs.f.h = r & 0x0f == 0;
    cpu.regs.f.p = r == 0x80;
    cpu.regs.f.n = false;
    r
}

pub fn dec(cpu: &mut CPU, v: u8) -> u8 {
    cpu.regs.f.h = v & 0x0f == 0;
    let r = v.wrapping_sub(1);
    cpu.regs.f.s = r & 0x80 != 0;
    cpu.regs.f.z = r == 0;
    cpu.regs.f.p = r == 0x7f;
    cpu.regs.f.n = true;
    r
}

pub fn rlca(cpu: &mut CPU) {
    cpu.regs.a = (cpu.regs.a << 1) | (cpu.regs.a >> 7);
    cpu.regs.f.c = cpu.regs.a & 0x01 != 0;
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
}

pub fn rla(cpu: &mut CPU) {
    let c = cpu.regs.f.c;
    cpu.regs.f.c = cpu.regs.a & 0b10000000 != 0;
    cpu.regs.a = cpu.regs.a << 1;
    if c {
        cpu.regs.a |= 1;
    }
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
}

pub fn rrca(cpu: &mut CPU) {
    cpu.regs.f.c = cpu.regs.a & 0x01 != 0;
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
    cpu.regs.a = (cpu.regs.a >> 1) | (cpu.regs.a << 7);
}

pub fn rra(cpu: &mut CPU) {
    let c = cpu.regs.f.c;
    cpu.regs.f.c = cpu.regs.a & 1 != 0;
    cpu.regs.a = cpu.regs.a >> 1;
    if c {
        cpu.regs.a |= 0b10000000;
    }
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
}

pub fn add_hl_rr(cpu: &mut CPU, p: u8) {
    // rIdx := cpu.fetched.opCode >> 4 & 0b11
    // reg := cpu.getRRptr(rIdx)

    let v = cpu.regs.get_rr(p);
    let hl = cpu.regs.get_rr(2);
    let result = (hl as u32) + (v as u32);
    let lookup =
        (((hl & 0x0800) >> 11) | ((v & 0x0800) >> 10) | (((result as u16) & 0x0800) >> 9)) as u8;
    cpu.regs.set_rr(2, result as u16);

    cpu.regs.f.n = false;
    cpu.regs.f.h = HALFCARRY_ADD_TABLE[lookup as usize];
    cpu.regs.f.c = (result & 0x10000) != 0;
}

pub fn daa(cpu: &mut CPU) {
    let mut c = cpu.regs.f.c;
    let mut add = 0u8;
    if cpu.regs.f.h || ((cpu.regs.a & 0x0f) > 9) {
        add = 6;
    }
    if c || (cpu.regs.a > 0x99) {
        add |= 0x60;
    }
    if cpu.regs.a > 0x99 {
        c = true;
    }
    if cpu.regs.f.n {
        sub_a(cpu, add);
    } else {
        add_a(cpu, add);
    }
    cpu.regs.f.s = (cpu.regs.a as i8) < 0;
    cpu.regs.f.z = cpu.regs.a == 0;
    cpu.regs.f.p = PARITY_TABLE[cpu.regs.a as usize];
    cpu.regs.f.c = c
}

pub fn cpl(cpu: &mut CPU) {
    cpu.regs.a = !cpu.regs.a;
    cpu.regs.f.h = true;
    cpu.regs.f.n = true;
}

pub fn scf(cpu: &mut CPU) {
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
    cpu.regs.f.c = true;
}

pub fn ccf(cpu: &mut CPU) {
    cpu.regs.f.h = cpu.regs.f.c;
    cpu.regs.f.n = false;
    cpu.regs.f.c = !cpu.regs.f.c;
}

pub fn alu(cpu: &mut CPU, x: u8, y: u8, z: u8) {
    let mut v: Option<u8> = None;
    match (x, z, cpu.regs.index_mode, cpu.fetched.d, cpu.fetched.n) {
        (2, 6, IndexMode::Iy | IndexMode::Ix, None, None) => {
            cpu.scheduler.push(Operation::MrPcD);
        }
        (2, 6, IndexMode::Iy | IndexMode::Ix, Some(d), None) => {
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_idx(d)));
            cpu.scheduler.push(Operation::Delay(5));
        }
        (2, 6, IndexMode::Iy | IndexMode::Ix, Some(d), Some(n)) => v = Some(n),

        (2, 6, _, _, None) => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
        (2, 6, _, _, Some(n)) => v = Some(n),
        (3, 6, _, _, None) => cpu.scheduler.push(Operation::MrPcN),
        (3, 6, _, _, Some(n)) => v = Some(n),

        _ => v = Some(cpu.regs.get_r(z)),
    }
    match v {
        Some(v) => match y {
            0 => add_a(cpu, v),
            1 => adc_a(cpu, v),
            2 => sub_a(cpu, v),
            3 => sbc_a(cpu, v),
            4 => and(cpu, v),
            5 => xor(cpu, v),
            6 => or(cpu, v),
            7 => cp(cpu, v),
            _ => unreachable!("Invalid ALU instruction"),
        },
        None => (),
    }
}

fn cp(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as u16;
    let result = a.wrapping_sub(v as u16);
    update_flags_ula(cpu, v, result as u16, true);
    cpu.regs.a = a as u8;
}

fn sub_a(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as u16;
    let result = a.wrapping_sub(v as u16);
    update_flags_ula(cpu, v, result as u16, true);
}

fn sbc_a(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as u16;
    let mut result = a.wrapping_sub(v as u16);
    if cpu.regs.f.c {
        result = result.wrapping_sub(1);
    }
    update_flags_ula(cpu, v, result, true);
}

fn adc_a(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as i16;
    let mut result = a.wrapping_add(v as i16);
    if cpu.regs.f.c {
        result = result.wrapping_add(1);
    }
    update_flags_ula(cpu, v, result as u16, false);
}

fn add_a(cpu: &mut CPU, v: u8) {
    let a = cpu.regs.a as i16;
    let result = a.wrapping_add(v as i16);
    update_flags_ula(cpu, v, result as u16, false);
}

fn update_flags_ula(cpu: &mut CPU, v: u8, result: u16, is_subtraction: bool) {
    let lookup = ((cpu.regs.a & 0x88) >> 3) | ((v & 0x88) >> 2) | ((result as u8) & 0x88) >> 1;
    let half_carry_table = if is_subtraction {
        &HALFCARRY_SUB_TABLE
    } else {
        &HALFCARRY_ADD_TABLE
    };
    let overflow_table = if is_subtraction {
        &OVERFLOW_SUB_TABLE
    } else {
        &OVERFLOW_ADD_TABLE
    };

    cpu.regs.a = (result & 0x00ff) as u8;
    cpu.regs.f.s = (cpu.regs.a & 0x80) != 0;
    cpu.regs.f.z = cpu.regs.a == 0;
    cpu.regs.f.h = half_carry_table[(lookup & 0x07) as usize];
    cpu.regs.f.p = overflow_table[(lookup >> 4) as usize];
    cpu.regs.f.n = is_subtraction;
    cpu.regs.f.c = (result & 0x100) != 0;
}

fn xor(cpu: &mut CPU, s: u8) {
    cpu.regs.a ^= s;
    cpu.regs.f.h = false;
    update_flags_ula_logic(cpu);
}

fn and(cpu: &mut CPU, s: u8) {
    cpu.regs.a &= s;
    cpu.regs.f.h = true;
    update_flags_ula_logic(cpu);
}

fn or(cpu: &mut CPU, s: u8) {
    cpu.regs.a |= s;
    cpu.regs.f.h = false;
    update_flags_ula_logic(cpu);
}

fn update_flags_ula_logic(cpu: &mut CPU) {
    cpu.regs.f.s = (cpu.regs.a as i8) < 0;
    cpu.regs.f.z = cpu.regs.a == 0;
    cpu.regs.f.p = PARITY_TABLE[cpu.regs.a as usize];
    cpu.regs.f.n = false;
    cpu.regs.f.c = false;
}

pub fn ret_cc(cpu: &mut CPU, y: u8) {
    match cpu.fetched.decode_step {
        0 => cpu.scheduler.push(Operation::Delay(1)),
        1 | 2 => {
            if cpu.if_cc(y) {
                ret(cpu)
            }
        }
        _ => unreachable!("Invalid ret_cc instruction"),
    }
}

pub fn ret(cpu: &mut CPU) {
    match cpu.fetched.nn {
        None => {
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.sp));
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.sp + 1));
        }
        Some(nn) => {
            cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
            cpu.regs.pc = nn;
        }
    }
}

pub fn jp(cpu: &mut CPU, y: Option<u8>) {
    match cpu.fetched.nn {
        None => {
            cpu.scheduler.push(Operation::MrPcN);
            cpu.scheduler.push(Operation::MrPcN);
        }
        Some(nn) => {
            let mut jump = true;
            match y {
                Some(y) => jump = cpu.if_cc(y),
                None => (),
            }
            if jump {
                cpu.regs.pc = nn;
            }
        }
    }
}

pub fn call(cpu: &mut CPU, y: Option<u8>) {
    match cpu.fetched.nn {
        None => {
            cpu.scheduler.push(Operation::MrPcN);
            cpu.scheduler.push(Operation::MrPcN);
        }
        Some(nn) => {
            let mut jump = true;
            match y {
                Some(y) => jump = cpu.if_cc(y),
                None => (),
            }
            if jump {
                cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
                cpu.scheduler.push(Operation::Delay(1));
                cpu.scheduler
                    .push(Operation::Mw16(cpu.regs.sp, cpu.regs.pc));
                cpu.regs.pc = nn;
                cpu.fetched.op_code = None;
            }
        }
    }
}

pub fn push(cpu: &mut CPU, r: u8) {
    cpu.fetched.op_code = None;
    cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
    cpu.scheduler.push(Operation::Delay(1));
    cpu.scheduler
        .push(Operation::Mw16(cpu.regs.sp, cpu.regs.get_rr2(r)));
}

pub fn pop(cpu: &mut CPU, r: u8) {
    match cpu.fetched.nn {
        None => {
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.sp));
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.sp + 1));
        }
        Some(nn) => {
            cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
            cpu.regs.set_rr2(r, nn);
        }
    }
}

pub fn rst(cpu: &mut CPU, y: u8) {
    cpu.fetched.op_code = None;
    cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
    cpu.scheduler.push(Operation::Delay(1));
    cpu.scheduler
        .push(Operation::Mw16(cpu.regs.sp, cpu.regs.pc));
    cpu.regs.pc = (y * 8) as u16;
}

fn set_flags_rot(cpu: &mut CPU, res: u8) {
    cpu.regs.f.z = res == 0;
    cpu.regs.f.n = false;
    cpu.regs.f.h = false;
    cpu.regs.f.s = res & 0x80 != 0;
    cpu.regs.f.p = PARITY_TABLE[res as usize];
}

pub fn rlc(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = (v << 1) | (v >> 7);
    cpu.regs.f.c = (v & 0b10000000) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn rrc(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = (v >> 1) | (v << 7);
    cpu.regs.f.c = (v & 0b00000001) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn rl(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = (v << 1) | cpu.regs.f.c as u8;
    cpu.regs.f.c = (v & 0b10000000) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn rr(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = (v >> 1) | ((cpu.regs.f.c as u8) << 7);
    cpu.regs.f.c = (v & 0b00000001) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn sla(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = v << 1;
    cpu.regs.f.c = (v & 0b10000000) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn sra(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = (v >> 1) | (v & 0b10000000);
    cpu.regs.f.c = (v & 0b00000001) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn sll(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = (v << 1) | 1;
    cpu.regs.f.c = (v & 0b10000000) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn srl(cpu: &mut CPU, _z: u8, v: u8) -> u8 {
    let res = v >> 1;
    cpu.regs.f.c = (v & 0b00000001) != 0;
    set_flags_rot(cpu, res);
    res
}

pub fn bit(cpu: &mut CPU, y: u8, v: u8) -> u8 {
    let bit = y as u8;
    let v = v & 1 << bit;
    cpu.regs.f.n = false;
    cpu.regs.f.h = true;
    cpu.regs.f.p = PARITY_TABLE[v as usize];
    cpu.regs.f.z = (v & (1 << bit)) == 0;
    cpu.regs.f.s = v & 0x0080 != 0;
    v
}

pub fn res(y: u8, v: u8) -> u8 {
    let bit = y as u8;
    let b = 1 << bit;
    v & (!b)
}

pub fn set(y: u8, v: u8) -> u8 {
    let bit = y as u8;
    let b = 1 << bit;
    v | b
}

pub fn out_na(cpu: &mut CPU) {
    match cpu.fetched.n {
        None => cpu.scheduler.push(Operation::MrPcN),
        Some(n) => {
            let port = (n as u16) << 8 | cpu.regs.a as u16;
            cpu.scheduler.push(Operation::Delay(1));
            cpu.scheduler.push(Operation::Pw8(port, cpu.regs.a));
            cpu.fetched.op_code = None;
        }
    }
}

// func inAn(cpu *z80) {
// 	inAn_f = cpu.regs.F.GetByte()
// 	port := toWord(cpu.fetched.n, cpu.regs.A)
// 	cpu.scheduler.append(&in{from: port, f: inAn_m1})
// }

// func inAn_m1(cpu *z80, data uint8) {
// 	cpu.regs.A = data
// 	cpu.regs.F.SetByte(inAn_f)
// }

pub fn in_na(cpu: &mut CPU) {
    match cpu.fetched.n {
        None => cpu.scheduler.push(Operation::MrPcN),
        Some(n) => {
            let port = (n as u16) << 8 | cpu.regs.a as u16;
            cpu.scheduler.push(Operation::Delay(1));
            cpu.scheduler.push(Operation::PrR(port, 7));
            cpu.fetched.op_code = None;
        }
    }
}
