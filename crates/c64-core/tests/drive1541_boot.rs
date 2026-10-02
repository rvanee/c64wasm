// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The real 1541 DOS ROM boots on its own and sets up both VIAs as the
//! disassembly says: VIA1 DDRB = $1A at $FF10 and, after the ROM checksum
//! (about a million cycles), VIA2 DDRB = $6F at $F259.

mod common;

use c64_core::Drive1541;

#[test]
fn dos_rom_boots_and_programs_both_vias_as_documented() {
    let Some(roms) = common::roms("dos_rom_boots_and_programs_both_vias_as_documented", true) else { return };
    let mut drive = Drive1541::new(roms.dos1541.as_ref().unwrap(), 8).unwrap();
    let mut cycles = 0;
    while cycles < 2_000_000 {
        assert!(!drive.jammed(), "drive CPU jammed at ${:04X}", drive.pc());
        cycles += drive.step();
    }
    assert_eq!(drive.board().via1.ddrb(), 0x1A);
    assert_eq!(drive.board().via2.ddrb(), 0x6F);
}
