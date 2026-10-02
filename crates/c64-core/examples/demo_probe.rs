// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Boot with a disk in drive 8, type commands, then run and report every
//! second what both CPUs are doing (busiest code areas, serial-bus
//! activity, drive head and motor). Saves the screen as raw RGBA frames
//! and both RAMs at the end, for comparing builds.
//!
//! Usage: `demo_probe <disk.d64> <seconds> <out-prefix> [command]...`
//! Environment: `NTSC=1` for an NTSC machine, `EVERY=n` to save a frame
//! every n seconds (default 5).
//!
//! Example: `demo_probe demo.d64 60 /tmp/demo 'LOAD"*",8,1' RUN`

use std::collections::HashMap;

use c64_core::tools::RomFiles;
use c64_core::{VideoStandard, C64};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: demo_probe <disk.d64> <seconds> <out-prefix> [command]...");
        std::process::exit(2);
    }
    let roms = RomFiles::from_repository().expect("ROMs in roms/");
    let standard = if std::env::var("NTSC").is_ok() { VideoStandard::Ntsc } else { VideoStandard::Pal };
    let mut c64 = C64::new(roms.system(), standard).unwrap();
    let drive = c64.attach_drive(roms.dos1541.as_ref().expect("roms/1541.rom"), 8).unwrap();
    c64.insert_disk(drive, &std::fs::read(&args[1]).unwrap()).unwrap();
    let seconds: u64 = args[2].parse().unwrap();
    let prefix = &args[3];
    let every: u64 = std::env::var("EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(5);

    c64.run_until(3_000_000);
    for command in &args[4..] {
        let mut line = command.clone().into_bytes();
        line.push(b'\r');
        assert!(c64.type_text(&line), "the screen editor isn't taking keys");
        // Let the screen editor take the line before typing the next one.
        c64.run_cycles(200_000);
    }

    let start = c64.cycles();
    let mut last_lines = c64.serial_bus();
    for second in 0..seconds {
        let mut c64_pcs: HashMap<u16, u64> = HashMap::new();
        let mut drive_pcs: HashMap<u16, u64> = HashMap::new();
        let mut transitions = 0u64;
        let end = c64.cycles() + 985_248;
        while c64.cycles() < end {
            if c64.jammed() {
                println!("C64 JAMMED at ${:04X}", c64.cpu().pc);
                break;
            }
            *c64_pcs.entry(c64.cpu().pc & 0xFFF0).or_default() += 1;
            *drive_pcs.entry(c64.drive(drive).unwrap().pc() & 0xFFF0).or_default() += 1;
            c64.step();
            let lines = c64.serial_bus();
            if lines != last_lines {
                transitions += 1;
                last_lines = lines;
            }
        }
        let d = c64.drive(drive).unwrap();
        println!(
            "t={:>3}s c64[{}] drive[{}] iec_transitions={} ht={} motor={} jammed={} $01=${:02X} d011=${:02X}",
            (c64.cycles() - start) / 985_248,
            busiest(&c64_pcs),
            busiest(&drive_pcs),
            transitions,
            d.current_half_track(),
            d.motor_on(),
            d.jammed(),
            c64.peek_ram(0x01),
            c64.read(0xD011),
        );
        if second % every == every - 1 || second + 1 == seconds {
            std::fs::write(format!("{prefix}-{:03}.rgba", second + 1), c64.framebuffer()).unwrap();
        }
        if c64.jammed() {
            break;
        }
    }
    let ram: Vec<u8> = (0..=0xFFFF).map(|a| c64.peek_ram(a)).collect();
    std::fs::write(format!("{prefix}-c64ram.bin"), ram).unwrap();
    let d = c64.drive(drive).unwrap();
    let drive_ram: Vec<u8> = (0..0x800).map(|a| d.peek_ram(a)).collect();
    std::fs::write(format!("{prefix}-driveram.bin"), drive_ram).unwrap();
    println!(
        "final c64 pc ${:04X} drive pc ${:04X} iec {:?} via1_pb ${:02X}",
        c64.cpu().pc,
        d.pc(),
        c64.serial_bus(),
        d.board().via1.port_b_pins()
    );
}

/// The four busiest 16-byte code areas.
fn busiest(pcs: &HashMap<u16, u64>) -> String {
    let mut v: Vec<_> = pcs.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    v.iter().take(4).map(|(pc, n)| format!("${pc:04X}:{n}")).collect::<Vec<_>>().join(" ")
}
