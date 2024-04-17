#[cfg(test)]
use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};
use std::{io::Read, iter::zip};

use crate::z80::{cpu::SignalReq, cpu::CPU};

use super::registers::Registers;

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
impl AuxRegs {
    fn to_string(&self) -> String {
        format!(
            "i: {}, r: {}, iff1: {}, iff2: {}, im: {}, halt: {}",
            self.i, self.r, self.iff1, self.iff2, self.im, self.halt
        )
    }
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
    let total = tests.len();

    zip(&tests, &results)
        .enumerate()
        .for_each(|(i, (test, result))| {
            let mut mem = [0 as u8; 0x010000];
            println!(
                "\n---- {} ---- {}/{} ({}%) ----",
                test.name,
                i,
                total,
                (i * 100) / total
            );

            for test_mem in &test.memory {
                let mut start = test_mem.start;
                for d in &test_mem.data {
                    mem[start as usize] = *d;
                    start += 1;
                }
            }

            let mut cpu = CPU::new();

            cpu.regs.set_all_regs(test.registers);

            cpu.regs.iff1 = test.aux_rgs.iff1;
            cpu.regs.iff2 = test.aux_rgs.iff2;
            cpu.regs.i = test.aux_rgs.i;
            cpu.regs.im = test.aux_rgs.im;
            cpu.regs.r = test.aux_rgs.r;
            cpu.halt = test.aux_rgs.halt;

            for _ in 0..result.aux_rgs.ts {
                match cpu.signals.mem {
                    SignalReq::Read => {
                        cpu.signals.data = mem[cpu.signals.addr as usize];
                        println!("\tMR {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                    }
                    SignalReq::Write => {
                        mem[cpu.signals.addr as usize] = cpu.signals.data;
                        println!("\tMW {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                    }
                    SignalReq::None => (),
                }
                match cpu.signals.port {
                    SignalReq::Read => {
                        cpu.signals.data = (cpu.signals.addr >> 8) as u8;
                        println!("\tPR {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                    }
                    SignalReq::Write => {
                        println!("\tPW {:04x} {:02x}", cpu.signals.addr, cpu.signals.data)
                    }
                    SignalReq::None => (),
                }
                cpu.tick();
            }
            println!("------------");
            let cpu_regs = cpu.regs.dump_registers();
            let res_regs = result.registers.map(|d| format!("{:04x}", d)).join(" ");
            let cpu_f = format!("{:08b}", cpu.regs.f.get() & 0b11010111);
            let res_f = format!("{:08b}", result.registers[0] as u8 & 0b11010111);
            assert_eq!(cpu_f, res_f, "flags fail !!!");
            assert_eq!(cpu_regs, res_regs, "regs fail !!!");
            assert_eq!(
                cpu.dump_registers_aux(),
                result.aux_rgs.to_string(),
                "aux_rgs fail !!!"
            );
            for m in result.memory.iter() {
                let cpu_mem = &mem[(m.start)..(m.start + m.data.len())];
                assert_eq!(cpu_mem, m.data, "mem '{:04x}' fail !!!", m.start);
            }
            assert!(
                matches!(cpu.current_ops, None),
                "current_ops not None !!! {:?}",
                cpu.current_ops
            );
            assert!(
                cpu.scheduler.is_empty(),
                "scheduler not empty !!! {:?}",
                cpu.scheduler
            );
            println!("------------\n");
        });
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

#[test]
fn test_zexdoc() {
    let path = env::current_dir()
        .unwrap()
        .join("tests")
        .join("zexdocsmall.cim");
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            panic!("error!! {}", err);
        }
    };
    let mut zexdoc = Vec::new();
    match f.read_to_end(&mut zexdoc) {
        Ok(_) => (),
        Err(err) => {
            panic!("error!! {}", err);
        }
    };

    let mut mem = vec![0; 0x0100];
    mem.extend_from_slice(&zexdoc);
    mem.extend(vec![0; 0x10000 - mem.len()]);

    let mut screen: Vec<u8> = Vec::new();

    let mut cpu = CPU::new();

    cpu.regs.pc = 0x0100;

    while cpu.regs.pc != 0x0000 {
        // for _ in 0..200 {

        println!("## pc: {:04x}", cpu.regs.pc);
        cpu.tick();

        if cpu.regs.pc == 0x0005 && cpu.current_ops.is_none() && cpu.scheduler.is_empty() {
            print_char(cpu.regs, &mem, &mut screen);
            let mut new_pc = mem[cpu.regs.sp as usize] as u16;
            new_pc |= (mem[(cpu.regs.sp + 1) as usize] as u16) << 8;
            cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
            cpu.regs.pc = new_pc;
            println!("############## pc: {:04x}", cpu.regs.pc);
        }

        match cpu.signals.mem {
            SignalReq::Read => {
                cpu.signals.data = mem[cpu.signals.addr as usize];
                println!("\tMR {:04x} {:02x}", cpu.signals.addr, cpu.signals.data);
            }
            SignalReq::Write => {
                mem[cpu.signals.addr as usize] = cpu.signals.data;
                println!("\tMW {:04x} {:02x}", cpu.signals.addr, cpu.signals.data);
                assert!(cpu.signals.addr > (zexdoc.len() + 100) as u16, "eee");
            }
            SignalReq::None => (),
        }
        match cpu.signals.port {
            SignalReq::Read => {
                // println!("\tPR {:04x} {:02x}", cpu.signals.addr, cpu.signals.data);
                panic!("port read")
            }
            SignalReq::Write => {
                // println!("\tPW {:04x} {:02x}", cpu.signals.addr, cpu.signals.data);
            }
            SignalReq::None => (),
        }
    }
    assert!(false);
}

// Emulate CP/M call 5; function is in register C.
// Function 2: print char in register E
// Function 9: print $ terminated string pointer in DE
fn print_char(regs: Registers, memory: &[u8], cpm_screen: &mut Vec<u8>) {
    match regs.c {
        2 => {
            cpm_screen.push(regs.get_r(3));
            print!("{}", regs.get_r(3) as char);
        }
        9 => {
            let de = regs.get_rr(1) as usize;
            for addr in de..memory.len() {
                let ch = memory[addr];
                if ch == b'$' {
                    break;
                }
                cpm_screen.push(ch);
                print!("{}", ch as char);
            }
        }
        _ => {}
    }
}
