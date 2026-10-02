// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Boot, put a PRG file into RAM, jump to `start`, run for `cycles`, then
//! hex-dump RAM.
//!
//! Usage: `run_prg <file.prg> <start-hex> <cycles> <dump-from-hex> <length>`
//! Environment: `PCS=0850,0860` prints the raster position the first
//! times those addresses are reached (for interrupt timing).

use c64_core::tools::RomFiles;
use c64_core::{VideoStandard, C64};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 6 {
        eprintln!("usage: run_prg <file.prg> <start-hex> <cycles> <dump-from-hex> <length>");
        std::process::exit(2);
    }
    let hex = |s: &str| u16::from_str_radix(s, 16).unwrap();
    let roms = RomFiles::from_repository().expect("ROMs in roms/");
    let mut c64 = C64::new(roms.system(), VideoStandard::Pal).unwrap();
    c64.run_until(2_500_000);

    let prg = std::fs::read(&args[1]).unwrap();
    let load = u16::from_le_bytes([prg[0], prg[1]]);
    for (i, &b) in prg[2..].iter().enumerate() {
        c64.write(load + i as u16, b);
    }
    c64.cpu_mut().pc = hex(&args[2]);
    let end = c64.cycles() + args[3].parse::<u64>().unwrap();

    let watch: Vec<u16> = std::env::var("PCS").map(|v| v.split(',').map(hex).collect()).unwrap_or_default();
    let mut shown = 0;
    while c64.cycles() < end && !c64.jammed() {
        let pc = c64.cpu().pc;
        if shown < 12 && watch.contains(&pc) {
            println!("{pc:04x} line {} cycle {}", c64.vic().raster_line(), c64.vic().cycle());
            shown += 1;
        }
        c64.step();
    }
    if c64.jammed() {
        println!("JAMMED at {:04X}", c64.cpu().pc);
    }
    let from = hex(&args[4]);
    let length: u16 = args[5].parse().unwrap();
    let dump: Vec<String> = (0..length).map(|i| format!("{:02x}", c64.peek_ram(from + i))).collect();
    println!("{}", dump.join(" "));
}
