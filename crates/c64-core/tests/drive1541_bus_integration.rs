// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! A C64 and a 1541, each with its own 6502 and clock, running side by
//! side: the drive boots while the C64 does, and ATN from CIA2 reaches the
//! drive's VIA1.

mod common;

use c64_core::{VideoStandard, C64};

#[test]
fn c64_and_a_real_drive_run_side_by_side_and_atn_reaches_via1() {
    let Some(roms) = common::roms("c64_and_a_real_drive_run_side_by_side", true) else { return };
    let mut c64 = C64::new(roms.system(), VideoStandard::Pal).unwrap();
    let drive = c64.attach_drive(roms.dos1541.as_ref().unwrap(), 8).unwrap();
    assert_eq!(drive, 0);

    // Long enough for the drive's VIA1 setup at $FF10.
    for _ in 0..200_000 {
        assert!(!c64.jammed(), "C64 jammed at ${:04X}", c64.cpu().pc);
        c64.step();
        assert!(!c64.drive(drive).unwrap().jammed(), "drive jammed");
    }
    assert_eq!(c64.drive(drive).unwrap().board().via1.ddrb(), 0x1A);

    // The KERNAL has set CIA2 DDRA to $3F by now: assert ATN directly.
    let dd00 = c64.read(0xDD00);
    c64.write(0xDD00, dd00 | 0b0000_1000);
    for _ in 0..10 {
        c64.step();
    }
    let pb = c64.drive(drive).unwrap().board().via1.port_b_pins();
    assert_eq!(pb & 0x80, 0x80, "ATN IN (PB7) must read asserted on the drive");
}
