// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Cycle timing of the CIA timers, interrupt recognition and the raster
//! interrupt, checked against VICE (x64sc 3.7.1). These need no Commodore
//! ROMs: the test programs bank the ROMs out and install their own vectors,
//! so they run in CI.
//!
//! - `cia_timing.prg` (from `data/make_cia_timing_prg.py`) starts timer A
//!   and reads it back after 0-3 NOPs, force-loads a running timer, then
//!   measures how many 2-cycle `INX`s run between starting a one-shot timer
//!   of 8..15 cycles and its IRQ (CIA1) / NMI (CIA2) handler.
//! - `raster_irq.prg` (from `data/make_raster_irq_prg.py`) takes a raster
//!   interrupt on line $80 from a `JMP *` loop with the screen blanked.
//!
//! The expected values are what VICE produced for the same programs (read
//! with its monitor). Getting the timer start delay wrong by two cycles
//! made the "Next Level" demo's NMI-driven raster effects land early.

mod common;

use c64_core::{VideoStandard, C64};

fn machine_with(prg: &[u8]) -> C64 {
    // Blank ROMs: the programs never touch them.
    let mut c64 = common::blank_c64(VideoStandard::Pal);
    c64.write(0x0000, 0x2F); // CPU port directions, as the KERNAL sets them
    let load = u16::from_le_bytes([prg[0], prg[1]]);
    for (i, &b) in prg[2..].iter().enumerate() {
        c64.write(load + i as u16, b);
    }
    c64.cpu_mut().pc = 0x0810;
    c64
}

#[test]
fn cia_timer_start_load_and_interrupt_latency_match_vice() {
    let mut c64 = machine_with(include_bytes!("data/cia_timing.prg"));
    c64.run_cycles(1_000_000);
    assert!(!c64.jammed(), "an interrupt never arrived (JAM at ${:04X})", c64.cpu().pc);
    let got: Vec<u8> = (0..21).map(|i| c64.peek_ram(0x0400 + i)).collect();
    let vice: [u8; 21] = [
        0x7e, 0x7c, 0x7a, 0x78, // timer read 0..3 NOPs after start ($80)
        0x3f, // force load of $40 into a running timer, read 4 cycles later
        6, 6, 7, 7, 8, 8, 9, 9, // CIA1 IRQ after 8..15 cycles: INX count
        6, 6, 7, 7, 8, 8, 9, 9, // CIA2 NMI
    ];
    assert_eq!(got, vice);
}

#[test]
fn raster_irq_entry_cycles_match_vice() {
    let mut c64 = machine_with(include_bytes!("data/raster_irq.prg"));
    let mut seen = std::collections::BTreeSet::new();
    while c64.cycles() < 400_000 {
        if c64.cpu().pc == 0x0850 {
            assert_eq!(c64.vic().raster_line(), 0x80);
            seen.insert(c64.vic().cycle());
        }
        c64.step();
    }
    // VICE shows the handler starting at cycles 9, 10 and 11 of the line
    // (the JMP * loop gives 3 possible phases).
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![9, 10, 11]);
}
