// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Validation runner against the SingleStepTests/65x02 "Tom Harte" JSON test
//! vectors (https://github.com/SingleStepTests/65x02, `6502/v1/*.json`).
//!
//! Each file is named after one opcode byte (e.g. `a9.json` for LDA #imm)
//! and contains ~10,000 single-instruction test cases: full CPU state
//! before, full CPU state after, and the exact ordered list of bus
//! transactions (address, value, read/write) the reference implementation
//! performed. This is exactly the shape our `Mos6502::step` produces, since its
//! cycle count *is* its bus-transaction count -- see the module docs on
//! `c64_core::cpu`.
//!
//! Usage:
//!   cargo run --release --features harness --bin tomharte -- <dir> [opcode_hex] [--max N] [--verbose N]
//!
//! <dir> should contain files like `00.json`, `a9.json`, ... (i.e. point it
//! at the `6502/v1` directory of a checkout/download of that repo).

use c64_core::cpu::{Bus, Mos6502};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct CpuState {
    pc: u16,
    s: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    ram: Vec<(u16, u8)>,
}

#[derive(Deserialize)]
struct TestCase {
    name: String,
    initial: CpuState,
    #[serde(rename = "final")]
    final_state: CpuState,
    cycles: Vec<(u32, Option<u8>, String)>,
}

struct SimpleBus {
    mem: Box<[u8; 65536]>,
    trace: Vec<(u16, u8, bool)>, // (addr, value, is_write)
}

impl SimpleBus {
    fn new() -> Self {
        SimpleBus { mem: Box::new([0u8; 65536]), trace: Vec::with_capacity(16) }
    }
}

impl Bus for SimpleBus {
    fn read(&mut self, addr: u16) -> u8 {
        let v = self.mem[addr as usize];
        self.trace.push((addr, v, false));
        v
    }
    fn write(&mut self, addr: u16, val: u8) {
        self.mem[addr as usize] = val;
        self.trace.push((addr, val, true));
    }
}

struct OpcodeReport {
    total: usize,
    passed: usize,
    failures: Vec<String>,
}

fn run_case(tc: &TestCase) -> Result<(), String> {
    let mut cpu = Mos6502::new();
    cpu.pc = tc.initial.pc;
    cpu.s = tc.initial.s;
    cpu.a = tc.initial.a;
    cpu.x = tc.initial.x;
    cpu.y = tc.initial.y;
    cpu.p = tc.initial.p;

    let mut bus = SimpleBus::new();
    for &(addr, val) in &tc.initial.ram {
        bus.mem[addr as usize] = val;
    }

    let cycles = cpu.step(&mut bus);

    let mut problems = Vec::new();

    if cpu.a != tc.final_state.a {
        problems.push(format!("a: got {:#04x} want {:#04x}", cpu.a, tc.final_state.a));
    }
    if cpu.x != tc.final_state.x {
        problems.push(format!("x: got {:#04x} want {:#04x}", cpu.x, tc.final_state.x));
    }
    if cpu.y != tc.final_state.y {
        problems.push(format!("y: got {:#04x} want {:#04x}", cpu.y, tc.final_state.y));
    }
    if cpu.s != tc.final_state.s {
        problems.push(format!("s: got {:#04x} want {:#04x}", cpu.s, tc.final_state.s));
    }
    if cpu.pc != tc.final_state.pc {
        problems.push(format!("pc: got {:#06x} want {:#06x}", cpu.pc, tc.final_state.pc));
    }
    if cpu.p != tc.final_state.p {
        problems.push(format!(
            "p: got {:#04x} ({:08b}) want {:#04x} ({:08b})",
            cpu.p, cpu.p, tc.final_state.p, tc.final_state.p
        ));
    }

    for &(addr, val) in &tc.final_state.ram {
        let got = bus.mem[addr as usize];
        if got != val {
            problems.push(format!("ram[{:#06x}]: got {:#04x} want {:#04x}", addr, got, val));
        }
    }

    if cycles as usize != tc.cycles.len() {
        problems.push(format!("cycle count: got {} want {}", cycles, tc.cycles.len()));
    } else {
        for (i, (exp_addr, exp_val, exp_kind)) in tc.cycles.iter().enumerate() {
            let (got_addr, got_val, got_write) = bus.trace[i];
            let exp_write = exp_kind == "write";
            let addr_ok = got_addr as u32 == *exp_addr;
            let val_ok = exp_val.map_or(true, |v| v == got_val);
            if !addr_ok || !val_ok || got_write != exp_write {
                problems.push(format!(
                    "cycle[{}]: got ({:#06x},{:#04x},{}) want ({:#06x},{:?},{})",
                    i,
                    got_addr,
                    got_val,
                    if got_write { "write" } else { "read" },
                    exp_addr,
                    exp_val,
                    exp_kind
                ));
            }
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("test '{}': {}", tc.name, problems.join("; ")))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} <dir-of-opcode-json-files> [opcode_hex] [--max N] [--verbose N]", args[0]);
        std::process::exit(1);
    }
    let dir = PathBuf::from(&args[1]);
    let mut opcode_filter: Option<String> = None;
    let mut max_per_file: usize = usize::MAX;
    let mut verbose: usize = 3;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--max" => {
                i += 1;
                max_per_file = args[i].parse().expect("--max needs a number");
            }
            "--verbose" => {
                i += 1;
                verbose = args[i].parse().expect("--verbose needs a number");
            }
            other => opcode_filter = Some(other.to_lowercase()),
        }
        i += 1;
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("can't read {}: {}", dir.display(), e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |e| e == "json"))
        .collect();
    files.sort();

    if let Some(f) = &opcode_filter {
        files.retain(|p| p.file_stem().and_then(|s| s.to_str()).map_or(false, |s| s.eq_ignore_ascii_case(f)));
        if files.is_empty() {
            eprintln!("no file matching opcode {:?} in {}", f, dir.display());
            std::process::exit(1);
        }
    }

    let mut reports: BTreeMap<String, OpcodeReport> = BTreeMap::new();
    let mut total_pass = 0usize;
    let mut total_all = 0usize;

    for path in &files {
        let opcode = path.file_stem().and_then(|s| s.to_str()).unwrap_or("??").to_string();
        let data = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {:?}: {}", path, e));
        let cases: Vec<TestCase> = match serde_json::from_str(&data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skip {} (parse error: {})", opcode, e);
                continue;
            }
        };

        let mut report = OpcodeReport { total: 0, passed: 0, failures: Vec::new() };

        for tc in cases.iter().take(max_per_file) {
            report.total += 1;
            match run_case(tc) {
                Ok(()) => report.passed += 1,
                Err(msg) => {
                    if report.failures.len() < verbose {
                        report.failures.push(msg);
                    }
                }
            }
        }

        total_pass += report.passed;
        total_all += report.total;
        reports.insert(opcode, report);
    }

    println!("{:<6}{:>8}{:>8}{:>8}  sample failures", "op", "total", "passed", "failed");
    for (opcode, r) in &reports {
        let failed = r.total - r.passed;
        let marker = if failed == 0 { "  " } else { "**" };
        println!("{}{:<4}{:>8}{:>8}{:>8}", marker, opcode, r.total, r.passed, failed);
        for f in &r.failures {
            println!("      - {}", f);
        }
    }

    println!(
        "\nTOTAL: {}/{} passed ({:.2}%) across {} opcode(s)",
        total_pass,
        total_all,
        100.0 * total_pass as f64 / total_all.max(1) as f64,
        reports.len()
    );

    if total_pass != total_all {
        std::process::exit(1);
    }
}

#[allow(dead_code)]
fn _unused(_p: &Path) {}
