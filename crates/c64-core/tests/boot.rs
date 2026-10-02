// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The KERNAL and BASIC boot to READY under PAL and NTSC. The startup
//! banner's free-memory count is a check of RAM sizing and banking.

mod common;

use c64_core::{VideoStandard, C64};
use common::{roms, screen_text};

fn assert_boots_to_ready(standard: VideoStandard) {
    let Some(roms) = roms("boots_to_ready", false) else { return };
    let mut c64 = C64::new(roms.system(), standard).unwrap();
    // The KERNAL's RAM test takes most of the ~2.5 million cycles to READY.
    c64.run_cycles(3_000_000);
    assert!(!c64.jammed(), "CPU jammed at ${:04X} ({standard:?})", c64.cpu().pc);
    let screen = screen_text(&c64);
    assert!(screen.contains("COMMODORE 64 BASIC"), "banner missing ({standard:?}):\n{screen}");
    assert!(screen.contains("38911 BASIC BYTES FREE"), "wrong free memory ({standard:?}):\n{screen}");
    assert!(screen.contains("READY."), "no READY. ({standard:?}):\n{screen}");
}

#[test]
fn boots_to_ready_prompt() {
    assert_boots_to_ready(VideoStandard::Pal);
}

#[test]
fn boots_to_ready_prompt_ntsc() {
    assert_boots_to_ready(VideoStandard::Ntsc);
}

/// Typing through the CIA1 keyboard matrix (not the keyboard buffer): the
/// KERNAL's own scan routine has to see the keys.
#[test]
fn typing_through_the_keyboard_matrix() {
    let Some(roms) = roms("typing_through_the_keyboard_matrix", false) else { return };
    let mut c64 = C64::new(roms.system(), VideoStandard::Pal).unwrap();
    c64.run_cycles(3_000_000);
    // (row, column) of P, R, I, N, T, space, 7, RETURN.
    let keys = [(5, 1), (2, 1), (4, 1), (4, 7), (2, 6), (7, 4), (3, 0), (0, 1)];
    for (row, col) in keys {
        c64.set_key(row, col, true);
        c64.run_cycles(40_000); // two keyboard scans
        c64.set_key(row, col, false);
        c64.run_cycles(40_000);
    }
    let screen = screen_text(&c64);
    assert!(screen.contains("PRINT 7\n 7\n"), "screen:\n{screen}");
}
