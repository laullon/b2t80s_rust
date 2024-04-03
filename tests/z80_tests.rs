use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use b2t80s_rust::z80;

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
    start: u16,
    data: [u8; 0],
}

#[test]
fn test_opcodes() {
    let path = env::current_dir().unwrap().join("tests");
    let tests = read_tests(path.join("tests.in"));
    let results = read_tests(path.join("tests.out"));
    assert_eq!(tests.len(), results.len());

    for t in 0..tests.len() {
        let cpu = z80::CPU::new(TestBus);
        cpu.tick();
    }
}

impl z80::Bus for TestBus {
    fn SetAddr(&self, addr: u16) {
        todo!()
    }

    fn ReadMemory(&self) {
        todo!()
    }

    fn GetData(&self) -> u8 {
        todo!()
    }

    fn Release(&self) {
        todo!()
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
                println!("{:?}", lines);
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
            TestMemory {
                start: addr.unwrap(),
                data: [],
            }
        })
        .collect()
}

fn parse_regs(regs: String) -> [u16; 12] {
    let res: Vec<u16> = regs
        .split_whitespace()
        .map(|i| u16::from_str_radix(i, 16).unwrap())
        .collect();
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
