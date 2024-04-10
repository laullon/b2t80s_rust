#[cfg(test)]
use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use crate::z80::{cpu::SignalReq, cpu::CPU};

// use b2t80s_rust::z80::registers::Registers;
// use b2t80s_rust::z80::{self};

#[derive(Debug)]
struct TestDefinition {
    name: String,
    registers: [u16; 12],
    aux_rgs: AuxRegs,
    memory: Vec<TestMemory>,
}

#[derive(Debug)]
struct AuxRegs {
    i: u8,
    r: u8,
    iff1: bool,
    iff2: bool,
    im: u8,
    halt: bool,
    ts: u16,
}

#[derive(Debug)]
struct TestMemory {
    start: usize,
    data: Vec<u8>,
}

#[test]
fn test_opcodes() {
    let path = env::current_dir().unwrap().join("tests");
    let tests = read_tests(path.join("tests.in"));
    let results = read_tests(path.join("tests.out"));
    assert_eq!(tests.len(), results.len());

    for t in 0..results.len() {
        let test = &tests[t];
        let result = &results[t];
        let mut mem = [0 as u8; 0x010000];
        println!("\n---- {} ----", test.name);

        for test_mem in &test.memory {
            let mut start = test_mem.start;
            for d in &test_mem.data {
                mem[start as usize] = *d;
                start += 1;
            }
        }

        let mut cpu = CPU::new();

        cpu.regs.set_all_regs(test.registers);

        for _ in 0..result.aux_rgs.ts {
            match cpu.signals.mem {
                SignalReq::Read => {
                    cpu.signals.data = mem[cpu.signals.addr as usize];
                    println!("    MR {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                }
                SignalReq::Write => {
                    mem[cpu.signals.addr as usize] = cpu.signals.data;
                    println!("    MW {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                }
                SignalReq::None => (),
            }
            match cpu.signals.port {
                SignalReq::Read => {
                    cpu.signals.data = cpu.signals.addr as u8;
                    println!("    PR {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                }
                SignalReq::Write => {
                    println!("    PW {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                }
                SignalReq::None => (),
            }
            cpu.tick();
        }
        println!("------------");
        let cpu_regs = cpu.regs.dump_registers();
        let res_regs = result.registers.map(|d| format!("{:04x}", d)).join(" ");
        let cpu_f = format!("{:08b}", cpu.regs.f.get());
        let res_f = format!("{:08b}", result.registers[0] as u8);
        assert_eq!(cpu_f, res_f, "flags fail !!!");
        assert_eq!(cpu_regs, res_regs, "regs fail !!!");
        for m in result.memory.iter() {
            let cpu_mem = &mem[(m.start)..(m.start + m.data.len())];
            assert_eq!(cpu_mem, m.data, "mem '{:04x}' fail !!!", m.start);
        }
        println!("------------\n");
    }
}

fn read_tests(path: PathBuf) -> Vec<TestDefinition> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);

    let mut results: Vec<TestDefinition> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for l in reader.lines() {
        let line = l.unwrap();
        match line.as_str() {
            "" => {
                // println!("{:?}", lines);
                let test = TestDefinition {
                    name: lines.remove(0),
                    registers: parse_regs(lines.remove(0)),
                    aux_rgs: parse_aux_regs(lines.remove(0)),
                    memory: parse_memory(&lines),
                };
                // println!("{:?}", test);
                results.push(test);
                lines.clear();
            }
            _ => {
                if !line.starts_with("  ") {
                    lines.push(line);
                }
            }
        }
    }
    return results;
}

fn parse_memory(lines: &Vec<String>) -> Vec<TestMemory> {
    lines
        .iter()
        .map(|line| -> TestMemory {
            let addr = u16::from_str_radix(&line[0..4], 16);
            let data: Vec<u8> = line[line.find(" ").unwrap()..line.rfind(" ").unwrap()]
                .split_whitespace()
                .map(|i| u8::from_str_radix(i, 16).unwrap())
                .collect();

            TestMemory {
                start: addr.unwrap() as usize,
                data: data,
            }
        })
        .collect()
}

fn parse_regs(regs: String) -> [u16; 12] {
    let mut res: Vec<u16> = regs
        .split_whitespace()
        .map(|i| u16::from_str_radix(i, 16).unwrap())
        .collect();
    res[0] = res[0] & 0b11111111_11010111; // flags 3&5 removed
    res[4] = res[4] & 0b11111111_11010111; // flags 3&5 removed
    return res.as_slice().try_into().expect("ERRRRRRR");
}

fn parse_aux_regs(aux: String) -> AuxRegs {
    let res: Vec<u8> = aux[0..aux.rfind(" ").unwrap()]
        .split_whitespace()
        .map(|i| u8::from_str_radix(i, 16).unwrap())
        .collect();
    return AuxRegs {
        i: res[0],
        r: res[1],
        iff1: res[2] == 1,
        iff2: res[3] == 1,
        im: res[4],
        halt: res[5] == 1,
        ts: u16::from_str_radix(aux.split_whitespace().last().unwrap(), 10).unwrap(),
    };
}
