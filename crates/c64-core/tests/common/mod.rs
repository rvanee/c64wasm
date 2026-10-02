// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Shared helpers for the integration tests.
#![allow(dead_code)]

use c64_core::cpu::Bus;
use c64_core::tools::RomFiles;
use c64_core::{SystemRoms, VideoStandard, C64};

/// A C64 with all-zero ROMs, for tests that drive the chips directly
/// (nothing runs on the CPU; registers and memory are written as the CPU
/// would).
pub fn blank_c64(standard: VideoStandard) -> C64 {
    let roms = SystemRoms { basic: &[0; 8192], kernal: &[0; 8192], chargen: &[0; 4096] };
    C64::new(roms, standard).unwrap()
}

/// Clock the board `cycles` times without running the CPU.
pub fn tick(c64: &mut C64, cycles: u32) {
    for _ in 0..cycles {
        c64.board_mut().tick();
    }
}

/// The ROMs in `roms/`, or `None` (and a note) when they are missing --
/// they are copyrighted and not in the repository, so CI runs without.
pub fn roms(test: &str, need_1541: bool) -> Option<RomFiles> {
    let roms = RomFiles::from_repository().filter(|r| !need_1541 || r.dos1541.is_some());
    if roms.is_none() {
        eprintln!("skipping {test}: needs the Commodore ROMs in roms/ (see roms/README.md)");
    }
    roms
}

/// The text screen at $0400 as ASCII, trailing spaces trimmed.
pub fn screen_text(c64: &C64) -> String {
    let mut out = String::new();
    for row in 0..25u16 {
        let line: String = (0..40u16)
            .map(|col| {
                let c = c64.peek_ram(0x0400 + row * 40 + col) & 0x7F;
                match c {
                    0 => '@',
                    1..=26 => (b'A' + c - 1) as char,
                    32..=63 => c as char,
                    _ => '.',
                }
            })
            .collect();
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}
