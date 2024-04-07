use super::cpu::CPU;
use crate::z80::cpu::Operation;

const overflowAddTable: [bool; 8] = [false, false, false, true, true, false, false, false];
const overflowSubTable: [bool; 8] = [false, true, false, false, false, false, true, false];
const halfcarryAddTable: [bool; 8] = [false, true, true, true, false, false, false, true];
const halfcarrySubTable: [bool; 8] = [false, false, true, false, true, false, true, true];

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
        Some(v) => cpu.regs.set_r(y, v),
        None => cpu.scheduler.push(Operation::MR_N),
    }
}

pub fn ld_r_r(cpu: &mut CPU, y: u8, z: u8) {
    if y == 6 {
        panic!("needs MW");
    } else if z == 6 {
        panic!("needs MR");
    }
    let v = cpu.regs.get_r(y);
    cpu.regs.set_r(y, v);
}

pub fn inc_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rp(p) + 1;
    cpu.regs.set_rr(p, v);
}

pub fn dec_rr(cpu: &mut CPU, p: u8) {
    let v = cpu.regs.get_rp(p).wrapping_sub(1);
    cpu.regs.set_rr(p, v);
}

pub fn inc_r(cpu: &mut CPU, y: u8) {
    let mut v = cpu.regs.get_r(y);
    v = v.wrapping_add(1);
    cpu.regs.set_r(y, v);
    cpu.regs.f.S = v & 0x80 != 0;
    cpu.regs.f.Z = v == 0;
    cpu.regs.f.H = v & 0x0f == 0;
    cpu.regs.f.P = v == 0x80;
    cpu.regs.f.N = false;
}

pub fn dec_r(cpu: &mut CPU, y: u8) {
    let mut v = cpu.regs.get_r(y);
    cpu.regs.f.H = v & 0x0f == 0;
    v = v.wrapping_sub(1);
    cpu.regs.set_r(y, v);
    cpu.regs.f.S = v & 0x80 != 0;
    cpu.regs.f.Z = v == 0;
    cpu.regs.f.P = v == 0x7f;
    cpu.regs.f.N = true;
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
