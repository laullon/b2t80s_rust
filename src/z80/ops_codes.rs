use super::{cpu::CPU, registers::IndexMode};
use crate::z80::cpu::Operation;

pub const IM: [u8; 8] = [0, 0, 1, 2, 0, 0, 1, 2];
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
    match cpu.regs.index_mode {
        IndexMode::Hl => match (y, z, cpu.fetched.decode_step) {
            (6, 6, _) => todo!(),

            (_, 6, 0) => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
            (_, 6, 1) => {
                cpu.regs.set_r(y, cpu.fetched.n.unwrap());
                cpu.fetched.op_code = None;
            }

            (6, _, 0) => cpu
                .scheduler
                .push(Operation::Mw8(cpu.regs.get_rr(2), cpu.regs.get_r(z))),
            (6, _, 1) => cpu.fetched.op_code = None,

            _ => {
                let v = cpu.regs.get_r(z);
                cpu.regs.set_r(y, v);
            }
        },
        _ => match (y, z, cpu.fetched.decode_step) {
            (6, _, 0) | (_, 6, 0) => cpu.scheduler.push(Operation::MrPcD),

            // LD r[z], (ix+d)
            (_, 6, 1) => cpu
                .scheduler
                .push(Operation::MrAddrN(cpu.regs.get_idx(cpu.fetched.d.unwrap()))),
            (_, 6, 2) => {
                if y == 4 || y == 5 {
                    cpu.regs.index_mode = IndexMode::Hl;
                }
                cpu.regs.set_r(y, cpu.fetched.n.unwrap());
                cpu.scheduler.push(Operation::Delay(5));
                cpu.fetched.op_code = None;
            }

            // LD (ix+d), r[z]
            (6, _, 1) => {
                let get_idx = cpu.regs.get_idx(cpu.fetched.d.unwrap());
                if z == 4 || z == 5 {
                    cpu.regs.index_mode = IndexMode::Hl;
                }
                cpu.scheduler
                    .push(Operation::Mw8(get_idx, cpu.regs.get_r(z)));
                cpu.scheduler.push(Operation::Delay(5));
                cpu.fetched.op_code = None;
            }
            (6, _, 2) => cpu.fetched.op_code = None,

            _ => {
                let v = cpu.regs.get_r(z);
                cpu.regs.set_r(y, v);
            }
        },
    }

    // match (y, z, cpu.regs.index_mode, cpu.fetched.d, cpu.fetched.n) {
    //     (6, _, IndexMode::Ix | IndexMode::Iy, None, None) => cpu.scheduler.push(Operation::MrPcD),
    //     (6, _, IndexMode::Ix | IndexMode::Iy, Some(d), None) => {
    //         let addr = cpu.regs.get_idx(d);
    //         cpu.regs.index_mode = IndexMode::Hl;
    //         cpu.scheduler.push(Operation::Mw8(addr, cpu.regs.get_r(z)));
    //         cpu.fetched.op_code = None;
    //         cpu.scheduler.push(Operation::Delay(5));
    //     }

    //     (_, 6, IndexMode::Ix | IndexMode::Iy, None, None) => cpu.scheduler.push(Operation::MrPcD),
    //     (_, 6, IndexMode::Ix | IndexMode::Iy, Some(d), None) => {
    //         cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_idx(d)))
    //     }
    //     (_, 6, IndexMode::Ix | IndexMode::Iy, Some(_), Some(n)) => {
    //         cpu.scheduler.push(Operation::Delay(5));
    //         cpu.fetched.op_code = None;
    //         cpu.regs.index_mode = IndexMode::Hl;
    //         cpu.regs.set_r(y, n);
    //     }

    //     (6, _, _, _, _) => {
    //         cpu.fetched.op_code = None;
    //         cpu.scheduler
    //             .push(Operation::Mw8(cpu.regs.get_rr(2), cpu.regs.get_r(z)));
    //     }
    //     (_, 6, _, _, _) => match cpu.fetched.n {
    //         None => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
    //         Some(n) => cpu.regs.set_r(y, n),
    //     },
    //     _ => {
    //         let v = cpu.regs.get_r(z);
    //         cpu.regs.set_r(y, v);
    //     }
    // };
}

pub fn inc_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rr(p).wrapping_add(1);
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
        (2, 6, IndexMode::Iy | IndexMode::Ix, Some(_), Some(n)) => v = Some(n),

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

pub fn sub_a(cpu: &mut CPU, v: u8) {
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

pub fn bit(cpu: &mut CPU, bit: u8, v: u8) -> u8 {
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
            let port = (n as u16) | (cpu.regs.a as u16) << 8;
            cpu.scheduler.push(Operation::Delay(1));
            cpu.scheduler.push(Operation::Pw8(port, cpu.regs.a));
            cpu.fetched.op_code = None;
        }
    }
}

pub fn in_na(cpu: &mut CPU) {
    match cpu.fetched.n {
        None => cpu.scheduler.push(Operation::MrPcN),
        Some(n) => {
            let port = (cpu.regs.a as u16) << 8 | n as u16;
            cpu.scheduler.push(Operation::Delay(1));
            cpu.scheduler.push(Operation::PrR(port, Some(7), false));
            cpu.fetched.op_code = None;
        }
    }
}

pub fn ex_sp_hl(cpu: &mut CPU) {
    match cpu.fetched.nn {
        None => {
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.sp));
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.sp + 1));
        }
        Some(nn) => {
            let hl = cpu.regs.get_rr(2);
            cpu.regs.set_rr(2, nn);
            cpu.fetched.op_code = None;
            cpu.scheduler.push(Operation::Delay(3));
            cpu.scheduler.push(Operation::Mw16(cpu.regs.sp, hl));
        }
    }
}

pub fn in_c(cpu: &mut CPU) {
    cpu.scheduler
        .push(Operation::PrR(cpu.regs.get_rr(0), None, true));
    cpu.scheduler.push(Operation::Delay(1));
    cpu.fetched.op_code = None;
}

pub fn in_r_c(cpu: &mut CPU, r: u8) {
    cpu.scheduler
        .push(Operation::PrR(cpu.regs.get_rr(0), Some(r), true));
    cpu.scheduler.push(Operation::Delay(1));
    cpu.fetched.op_code = None;
}

pub fn out_c(cpu: &mut CPU) {
    cpu.scheduler.push(Operation::Pw8(cpu.regs.get_rr(0), 0));
    cpu.scheduler.push(Operation::Delay(1));
    cpu.fetched.op_code = None;
}

pub fn out_c_r(cpu: &mut CPU, r: u8) {
    cpu.scheduler
        .push(Operation::Pw8(cpu.regs.get_rr(0), cpu.regs.get_r(r)));
    cpu.scheduler.push(Operation::Delay(1));
    cpu.fetched.op_code = None;
}

pub fn sbc_hl(cpu: &mut CPU, ss: u16) {
    let hl = cpu.regs.get_rr(2);

    let (result, carry1) = hl.overflowing_sub(ss);
    let (result_with_carry, carry2) = result.overflowing_sub(if cpu.regs.f.c { 1 } else { 0 });

    cpu.regs.set_rr(2, result_with_carry as u16);

    let lookup =
        ((hl & 0x8800) >> 11) | ((ss & 0x8800) >> 10) | ((result_with_carry as u16 & 0x8800) >> 9);
    cpu.regs.f.n = true;
    cpu.regs.f.s = cpu.regs.get_r(4) & 0x80 != 0; // negative
    cpu.regs.f.z = result_with_carry == 0;
    cpu.regs.f.p = OVERFLOW_SUB_TABLE[(lookup >> 4) as usize];
    cpu.regs.f.h = (hl & 0xFFF) < (ss & 0xFFF) + (if cpu.regs.f.c { 1 } else { 0 }); // Set half-carry flag based on lower 12 bits
    cpu.regs.f.c = carry1 || carry2; // Set carry flag based on subtraction overflow

    cpu.fetched.op_code = None;
    cpu.scheduler.push(Operation::Delay(7));
}

pub fn adc_hl(cpu: &mut CPU, ss: u16) {
    let hl = cpu.regs.get_rr(2);

    let (result, carry1) = hl.overflowing_add(ss);
    let (result_with_carry, carry2) = result.overflowing_add(if cpu.regs.f.c { 1 } else { 0 });

    cpu.regs.set_rr(2, result_with_carry as u16);

    let lookup =
        ((hl & 0x8800) >> 11) | ((ss & 0x8800) >> 10) | ((result_with_carry as u16 & 0x8800) >> 9);
    cpu.regs.f.n = false;
    cpu.regs.f.s = cpu.regs.get_r(4) & 0x80 != 0; // negative
    cpu.regs.f.z = result_with_carry == 0;
    cpu.regs.f.c = carry1 || carry2;
    cpu.regs.f.p = OVERFLOW_ADD_TABLE[(lookup >> 4) as usize];
    cpu.regs.f.h = (hl & 0xFFF) + (ss & 0xFFF) > 0xFFF; // Set half-carry flag based on lower 12 bits

    cpu.fetched.op_code = None;
    cpu.scheduler.push(Operation::Delay(7));
}

pub fn ld_nn_rr(cpu: &mut CPU, p: u8) {
    match cpu.fetched.nn {
        None => {
            cpu.scheduler.push(Operation::MrPcN);
            cpu.scheduler.push(Operation::MrPcN);
        }
        Some(nn) => {
            cpu.fetched.op_code = None;
            cpu.scheduler.push(Operation::Mw16(nn, cpu.regs.get_rr(p)));
        }
    }
}

pub fn ld_rr_nn(cpu: &mut CPU, p: u8) {
    match cpu.fetched.decode_step {
        0 => {
            cpu.scheduler.push(Operation::MrPcN);
            cpu.scheduler.push(Operation::MrPcN);
        }
        1 => {
            cpu.scheduler
                .push(Operation::MrAddrN(cpu.fetched.nn.unwrap()));
            cpu.scheduler
                .push(Operation::MrAddrN(cpu.fetched.nn.unwrap().wrapping_add(1)));
            cpu.fetched.n = None;
            cpu.fetched.nn = None;
        }
        2 => {
            cpu.regs.set_rr(p, cpu.fetched.nn.unwrap());
        }
        _ => unreachable!("Invalid ld_rr_nn instruction"),
    }
}

pub fn rdd(cpu: &mut CPU) {
    match cpu.fetched.n {
        None => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
        Some(n) => {
            let al = cpu.regs.a & 0x0f;
            let nh = n >> 4;
            let nl = n & 0x0f;

            let new_a = (cpu.regs.a & 0xf0) | nl;
            let new_n = (al << 4) | nh;

            cpu.scheduler.push(Operation::Delay(4));
            cpu.scheduler
                .push(Operation::Mw8(cpu.regs.get_rr(2), new_n));
            cpu.fetched.op_code = None;
            cpu.regs.a = new_a;
            update_flags_after_rdd_rld(cpu);
        }
    }
}

pub fn rld(cpu: &mut CPU) {
    match cpu.fetched.n {
        None => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
        Some(n) => {
            let al = cpu.regs.a & 0x0f;
            let nh = n >> 4;
            let nl = n & 0x0f;

            let new_a = (cpu.regs.a & 0xf0) | nh;
            let new_n = (nl << 4) | al;

            cpu.scheduler.push(Operation::Delay(4));
            cpu.scheduler
                .push(Operation::Mw8(cpu.regs.get_rr(2), new_n));
            cpu.fetched.op_code = None;
            cpu.regs.a = new_a;
            update_flags_after_rdd_rld(cpu);
        }
    }
}

fn update_flags_after_rdd_rld(cpu: &mut CPU) {
    let a = cpu.regs.a;
    cpu.regs.f.s = a & 0x80 != 0;
    cpu.regs.f.z = a == 0;
    cpu.regs.f.p = PARITY_TABLE[a as usize];
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
}

pub fn ld_a_ir_flags(cpu: &mut CPU) {
    let a = cpu.regs.a;
    cpu.regs.f.s = a & 0x80 != 0;
    cpu.regs.f.z = a == 0;
    cpu.regs.f.p = cpu.regs.iff2;
    cpu.regs.f.h = false;
    cpu.regs.f.n = false;
}

pub fn bli(cpu: &mut CPU, a: u8, b: u8) {
    match (a, b) {
        (4, 0) => ldi_ldd(cpu, false),
        (4, 1) => cpi_cpd(cpu, false),
        (4, 2) => ini_ind(cpu, false),
        (4, 3) => outi_outd(cpu, false),
        (5, 0) => ldi_ldd(cpu, true),
        (5, 1) => cpi_cpd(cpu, true),
        (5, 2) => ini_ind(cpu, true),
        (5, 3) => outi_outd(cpu, true),
        (6, 0) => ldir_lddr(cpu, false),
        (6, 1) => cpir_cpdr(cpu, false),
        (6, 2) => inir_indr(cpu, false),
        (6, 3) => otir_otdr(cpu, false),
        (7, 0) => ldir_lddr(cpu, true),
        (7, 1) => cpir_cpdr(cpu, true),
        (7, 2) => inir_indr(cpu, true),
        (7, 3) => otir_otdr(cpu, true),
        _ => unreachable!("Invalid bli instruction"),
    }
}

fn otir_otdr(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 | 1 => {
            outi_outd(cpu, sub);
        }
        2 => {
            outi_outd(cpu, sub);
            if !cpu.regs.f.z {
                cpu.scheduler.push(Operation::Delay(5))
            }
        }
        3 => {
            cpu.regs.pc = cpu.regs.pc.wrapping_sub(2);
        }
        _ => unreachable!("Invalid inir_indr instruction"),
    }
}

fn inir_indr(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 | 1 => {
            ini_ind(cpu, sub);
        }
        2 => {
            ini_ind(cpu, sub);
            if !cpu.regs.f.z {
                cpu.scheduler.push(Operation::Delay(5))
            }
        }
        3 => {
            cpu.regs.pc = cpu.regs.pc.wrapping_sub(2);
        }
        _ => unreachable!("Invalid inir_indr instruction"),
    }
}

fn cpir_cpdr(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 | 1 => {
            cpi_cpd(cpu, sub);
        }
        2 => {
            cpi_cpd(cpu, sub);
            if cpu.regs.f.p && !cpu.regs.f.z {
                cpu.scheduler.push(Operation::Delay(5))
            }
        }
        3 => {
            cpu.regs.pc = cpu.regs.pc.wrapping_sub(2);
        }
        _ => unreachable!("Invalid cpir_cpdr instruction"),
    }
}

fn ldir_lddr(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 | 1 => {
            ldi_ldd(cpu, sub);
        }
        2 => {
            ldi_ldd(cpu, sub);
            if cpu.regs.f.p {
                cpu.scheduler.push(Operation::Delay(5))
            }
        }
        3 => {
            cpu.regs.pc = cpu.regs.pc.wrapping_sub(2);
        }
        _ => unreachable!("Invalid ldir instruction"),
    }
}

fn outi_outd(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 => {
            cpu.scheduler.push(Operation::Delay(1));
            cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2)))
        }
        1 => {
            cpu.regs.set_r(0, cpu.regs.get_r(0).wrapping_sub(1));
            cpu.scheduler
                .push(Operation::Pw8(cpu.regs.get_rr(0), cpu.fetched.n.unwrap()));
            cpu.scheduler.push(Operation::Delay(1));
        }
        2 => {
            if sub {
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_sub(1));
            } else {
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_add(1));
            }

            let b = cpu.regs.get_r(0);
            let value = cpu.fetched.n.unwrap();
            let aux = cpu.regs.get_r(5).wrapping_add(value);
            let p = (aux & 0x07) ^ b;

            cpu.regs.f.z = b == 0;
            cpu.regs.f.s = b & 0x80 != 0;

            cpu.regs.f.h = aux < value;
            cpu.regs.f.c = aux < value;

            cpu.regs.f.n = value & 0x80 != 0;

            cpu.regs.f.p = PARITY_TABLE[p as usize];
        }
        _ => unreachable!("Invalid outi instruction"),
    }
}

fn ini_ind(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 => {
            cpu.scheduler.push(Operation::Delay(1));
            cpu.scheduler
                .push(Operation::PrR(cpu.regs.get_rr(0), None, true))
        }
        1 => {
            cpu.scheduler
                .push(Operation::Mw8(cpu.regs.get_rr(2), cpu.fetched.n.unwrap()));
            cpu.scheduler.push(Operation::Delay(1));
        }
        2 => {
            cpu.regs.set_r(0, cpu.regs.get_r(0).wrapping_sub(1));
            if sub {
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_sub(1));
            } else {
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_add(1));
            }

            let value = cpu.fetched.n.unwrap();
            let c = cpu.regs.get_r(1);
            let b = cpu.regs.get_r(0);
            let aux;
            if sub {
                aux = value.wrapping_add(c.wrapping_sub(1));
            } else {
                aux = value.wrapping_add(c.wrapping_add(1));
            }
            cpu.regs.f.h = aux < c;
            cpu.regs.f.c = aux < c;
            cpu.regs.f.n = value & 0x80 != 0;
            cpu.regs.f.z = cpu.regs.get_r(0) == 0;

            let p = (aux & 0x07) ^ b;
            cpu.regs.f.p = PARITY_TABLE[p as usize];
        }
        _ => unreachable!("Invalid ini_ind instruction"),
    }
}

fn cpi_cpd(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
        1 => {
            cpu.scheduler.push(Operation::Delay(5));
        }
        2 => {
            let data = cpu.fetched.n.unwrap();
            let result = cpu.regs.a.wrapping_sub(data);

            cpu.regs.set_rr(0, cpu.regs.get_rr(0).wrapping_sub(1));
            if sub {
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_sub(1));
            } else {
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_add(1));
            }

            let lookup = (cpu.regs.a & 0x08) >> 3 | (data & 0x08) >> 2 | (result & 0x08) >> 1;
            cpu.regs.f.h = HALFCARRY_SUB_TABLE[lookup as usize];
            cpu.regs.f.s = result & 0x80 != 0;
            cpu.regs.f.z = result == 0;
            cpu.regs.f.p = cpu.regs.get_rr(0) != 0;
            cpu.regs.f.n = true;
        }
        _ => unreachable!("Invalid cpi instruction"),
    }
}

fn ldi_ldd(cpu: &mut CPU, sub: bool) {
    match cpu.fetched.decode_step {
        0 => cpu.scheduler.push(Operation::MrAddrN(cpu.regs.get_rr(2))),
        1 => {
            cpu.scheduler.push(Operation::Delay(2));
            cpu.scheduler
                .push(Operation::Mw8(cpu.regs.get_rr(1), cpu.fetched.n.unwrap()));
        }
        2 => {
            if sub {
                cpu.regs.set_rr(1, cpu.regs.get_rr(1).wrapping_sub(1));
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_sub(1));
            } else {
                cpu.regs.set_rr(1, cpu.regs.get_rr(1).wrapping_add(1));
                cpu.regs.set_rr(2, cpu.regs.get_rr(2).wrapping_add(1));
            }
            cpu.regs.set_rr(0, cpu.regs.get_rr(0).wrapping_sub(1));

            cpu.regs.f.p = cpu.regs.get_rr(0) != 0;
            cpu.regs.f.h = false;
            cpu.regs.f.n = false;

            // }
        }
        _ => unreachable!("Invalid ldi instruction"),
    }
}
