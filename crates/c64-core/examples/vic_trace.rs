// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Boot, `LOAD"*",8,1` and `RUN` a disk, run for `seconds`, then trace one
//! frame and save it as raw RGBA.
//!
//! Usage: `vic_trace <disk.d64> <seconds> <out-prefix>`
//!
//! - Default: per raster line, the sprite registers and sprite units
//!   whenever they change.
//! - `PCS=1234,1240`: the raster position and CIA2 timers each time the
//!   CPU reaches those addresses (40 times), with the instructions before.
//! - `WRITES=1`: every change of the main VIC-II registers in one frame.

use c64_core::tools::RomFiles;
use c64_core::{VideoStandard, C64};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: vic_trace <disk.d64> <seconds> <out-prefix>");
        std::process::exit(2);
    }
    let roms = RomFiles::from_repository().expect("ROMs in roms/");
    let mut c64 = C64::new(roms.system(), VideoStandard::Pal).unwrap();
    let drive = c64.attach_drive(roms.dos1541.as_ref().expect("roms/1541.rom"), 8).unwrap();
    c64.insert_disk(drive, &std::fs::read(&args[1]).unwrap()).unwrap();
    let seconds: f64 = args[2].parse().unwrap();

    c64.run_until(3_000_000);
    for command in ["LOAD\"*\",8,1\r", "RUN\r"] {
        assert!(c64.type_text(command.as_bytes()), "the screen editor isn't taking keys");
        c64.run_cycles(200_000);
    }
    let start = c64.cycles();
    c64.run_cycles((seconds * c64.clock_hz() as f64) as u64);
    while c64.vic().raster_line() != 0 {
        c64.step();
    }

    if let Ok(pcs) = std::env::var("PCS") {
        trace_pcs(&mut c64, &pcs);
    } else if std::env::var("WRITES").is_ok() {
        trace_writes(&mut c64);
    } else {
        trace_sprites(&mut c64);
        std::fs::write(format!("{}.rgba", args[3]), c64.framebuffer()).unwrap();
    }
    eprintln!("traced at {:.1}s", (c64.cycles() - start) as f64 / c64.clock_hz() as f64);
}

fn trace_pcs(c64: &mut C64, pcs: &str) {
    let pcs: Vec<u16> = pcs.split(',').map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
    let mut history: Vec<(u16, u16, u16)> = Vec::new();
    let mut hits = 0;
    while hits < 40 {
        let pc = c64.cpu().pc;
        let v = c64.vic();
        if pcs.contains(&pc) {
            let before: Vec<String> =
                history.iter().rev().take(4).rev().map(|(p, l, c)| format!("{p:04x}@{l}/{}", c - 1)).collect();
            let (ta, tb) = c64.board().cia2().timers();
            println!(
                "{pc:04x} {} {}  (0-based) TA={ta:04x} TB={tb:04x}  before: {}",
                v.raster_line(),
                v.cycle() - 1,
                before.join(" ")
            );
            hits += 1;
        }
        history.push((pc, v.raster_line(), v.cycle()));
        if history.len() > 8 {
            history.remove(0);
        }
        c64.step();
    }
}

fn trace_writes(c64: &mut C64) {
    const WATCHED: [usize; 12] = [0x11, 0x16, 0x18, 0x15, 0x10, 0x1B, 0x1C, 0x17, 0x1D, 0x21, 0x22, 0x23];
    let mut prev = c64.vic().regs;
    loop {
        c64.step();
        let v = c64.vic();
        for r in WATCHED {
            if v.regs[r] != prev[r] {
                println!(
                    "line {:3} cyc {:2}  D0{r:02X} {:02X} -> {:02X}  pc={:04X}",
                    v.raster_line(),
                    v.cycle(),
                    prev[r],
                    v.regs[r],
                    c64.cpu().pc
                );
            }
        }
        prev = v.regs;
        if v.raster_line() == 311 && v.cycle() > 50 {
            break;
        }
    }
}

fn trace_sprites(c64: &mut C64) {
    let mut last = String::new();
    let mut line = 0u16;
    loop {
        let v = c64.vic();
        let r = &v.regs;
        let mut s = format!(
            "en={:02X} msb={:02X} yx={:02X} xx={:02X} mc={:02X} pr={:02X} d016={:02X} d011={:02X} |",
            r[0x15], r[0x10], r[0x17], r[0x1D], r[0x1C], r[0x1B], r[0x16], r[0x11]
        );
        for (i, &(dma, display, mc, mcbase, _)) in v.sprite_state().iter().enumerate() {
            let x = r[2 * i] as u16 | (((r[0x10] >> i) & 1) as u16) << 8;
            s += &format!(
                " {i}:{:02X},{x:03X}{}{}{:02}",
                r[1 + 2 * i],
                if dma { 'D' } else { '.' },
                if display { 'S' } else { '.' },
                mcbase.max(mc) % 64
            );
        }
        if s != last {
            println!("line {line:3} {s}");
            last = s;
        }
        while c64.vic().raster_line() == line {
            c64.step();
        }
        line = c64.vic().raster_line();
        if line == 0 {
            break;
        }
    }
}
