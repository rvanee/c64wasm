// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Regression tests for VIC-II sprite-sprite and sprite-background collision
//! (`$D01E`/`$D01F`) derived from a real user-authored program: the "two
//! crabs" listing (`SpriteCollision.bas` in this crate's root, which
//! `tests/disk_basic_roundtrip.rs` types, saves and loads). The crab shape below is the exact
//! 63-byte hi-res sprite data from that listing's `DATA` statements, not a
//! synthetic placeholder, so a regression here is a regression on the same
//! shape the real program uses.
//!
//! `tests/sprites.rs` already covers sprite-sprite priority/collision and
//! sprite-vs-*bitmap*-foreground collision (`display_priority_and_
//! background_collision_respect_mxdp`); this file fills the specific gap
//! the user's program exercises and that one doesn't: collision against a
//! *standard text mode* foreground pixel (their program prints a reverse-
//! video space as a wall for the crab to bump into), plus sprite-sprite
//! collision using the crabs' actual (non-rectangular) shape rather than a
//! solid test block.
//!
//! Same approach as `tests/sprites.rs`/`tests/ecm.rs`: dummy (all-zero)
//! ROMs, `Board::tick` with no CPU running, registers and memory poked
//! directly, pixels/registers read back afterward.

mod common;

use c64_core::{VideoStandard, C64};
use common::{blank_c64, tick};

const CYCLES_PER_LINE: u32 = 63;
const LINES_PER_FRAME: u32 = 312;
const ONE_FRAME: u32 = CYCLES_PER_LINE * LINES_PER_FRAME;

fn new_machine() -> C64 {
    blank_c64(VideoStandard::Pal)
}

fn run_frames(mem: &mut C64, frames: u32) {
    tick(mem, ONE_FRAME * frames);
}

/// The crab's 63-byte hi-res sprite shape, transcribed verbatim from
/// `SpriteCollision.bas`'s (corrected) `DATA` statements at lines 270-370 --
/// the same bytes the real program loads via `READ`/`POKE`.
const CRAB_SHAPE: [u8; 63] = [
    3, 231, 192, 15, 0, 240, 30, 102, 120, 31, 195, 248, 63, 0, 252, 62, 126, 124, 108, 153, 54, 97, 255, 134, 99, 255,
    198, 55, 255, 236, 31, 255, 248, 15, 255, 240, 63, 255, 252, 127, 255, 254, 143, 255, 241, 31, 255, 248, 103, 255,
    230, 139, 255, 209, 50, 126, 76, 34, 0, 68, 3, 0, 192,
];

/// Programs a bare-minimum standard bitmap display (DEN on, no text/bitmap
/// content needed -- these tests only care about sprites and, in the text
/// test, a hand-placed character) and writes the crab shape once, at sprite
/// data block `block` (address `block * 64`), pointed to by sprite `sprite`
/// via the video matrix's default location ($0400).
fn setup(mem: &mut C64) {
    mem.write(0xD011, 0x1B); // DEN, BMM, RSEL, Y-scroll 0
    mem.write(0xD016, 0x08); // CSEL (40 columns)
    mem.write(0xD018, 0x18); // video matrix @ $0400, bitmap-style base @ $2000
}

fn load_crab(mem: &mut C64, sprite: u8, block: u8) {
    mem.write(0x0400 + 0x03F8 + sprite as u16, block);
    let base = (block as u16) * 64;
    for (i, b) in CRAB_SHAPE.iter().enumerate() {
        mem.write(base + i as u16, *b);
    }
}

fn set_xy(mem: &mut C64, sprite: u8, x: u16, y: u8) {
    mem.write(0xD000 + 2 * sprite as u16, (x & 0xFF) as u8);
    mem.write(0xD001 + 2 * sprite as u16, y);
    let msb = mem.read(0xD010);
    let bit = if x > 0xFF { 1u8 << sprite } else { 0 };
    mem.write(0xD010, (msb & !(1 << sprite)) | bit);
}

#[test]
fn two_crabs_collide_with_each_other() {
    let mut mem = new_machine();
    setup(&mut mem);
    load_crab(&mut mem, 0, 4);
    load_crab(&mut mem, 1, 4); // same shape block -- both crabs are identical, as in the real program
    mem.write(0xD027, 3); // sprite 0 colour
    mem.write(0xD028, 6); // sprite 1 colour
    mem.write(0xD015, 0x03); // enable sprites 0 and 1

    // Same position, same shape: wherever crab 0 has an opaque pixel, crab
    // 1 has one too, so this is a guaranteed real (not just bounding-box)
    // pixel-level overlap -- exactly what real hardware's collision circuit
    // detects, per Christian Bauer's VIC-II article (see `src/vic/`).
    set_xy(&mut mem, 0, 150, 100);
    set_xy(&mut mem, 1, 150, 100);
    run_frames(&mut mem, 2);

    let ss = mem.read(0xD01E);
    assert_eq!(
        ss & 0x03,
        0x03,
        "overlapping crabs should set both sprites' bits in the sprite-sprite collision register, got {ss:#04x}"
    );

    // Reading $D01E clears it (real hardware's "cannot be written and are
    // automatically cleared on reading" behaviour) -- confirm that, then
    // separate the crabs and confirm no *new* collision is latched.
    assert_eq!(
        mem.read(0xD01E),
        0,
        "sprite-sprite collision register should read as cleared immediately after the first read"
    );

    set_xy(&mut mem, 0, 50, 100);
    set_xy(&mut mem, 1, 220, 100);
    run_frames(&mut mem, 2);
    assert_eq!(mem.read(0xD01E), 0, "crabs far apart should not collide");
}

#[test]
fn crab_collides_with_text_mode_foreground_but_not_background() {
    let mut mem = new_machine();
    // Standard (non-bitmap, non-ECM) text mode this time, matching the real
    // program: it never sets BMM, and prints its "wall" as an ordinary
    // (reverse-video) character in the default 40-column text screen.
    mem.write(0xD011, 0x1B & !0x20); // DEN, RSEL, BMM cleared -- plain text mode
    mem.write(0xD016, 0x08); // CSEL (40 columns), MCM off
                             // Video matrix @ $0400, character generator @ $0000 -- deliberately not
                             // $1000-$1FFF, which in VIC bank 0 is hardwired to the character ROM
                             // overlay (see `vic::VicMem::read`); with dummy all-zero ROMs that
                             // overlay would make every character render with no foreground pixels
                             // at all regardless of what this test wrote to screen RAM. Same trick
                             // `tests/ecm.rs` uses.
    mem.write(0xD018, 0x10);

    // One character cell, screen column 5, row 6 (raster ~= 51 + 6*8 = 99,
    // comfortably inside the display window), holding a fully-opaque
    // "reverse space"-style glyph -- stand-in for the real program's
    // `PRINT CHR$(18);"...'..."` reverse-video wall, which is (on real
    // hardware) just a character whose ROM glyph happens to be solid.
    let row = 6u16;
    let col = 5u16;
    mem.write(0x0400 + row * 40 + col, 1); // screen code 1 -- arbitrary, RAM-backed here
    mem.write(0xD800 + row * 40 + col, 5); // colour RAM nibble -- arbitrary
    for r in 0..8u16 {
        mem.write(8 + r, 0xFF); // fully solid glyph: every pixel foreground
    }

    load_crab(&mut mem, 0, 4);
    mem.write(0xD027, 3);
    mem.write(0xD015, 0x01); // enable sprite 0 only

    // Character cell (col 5, row 6) occupies pixels x=24+5*8=64..71,
    // y=51+6*8=99..106. Center the crab's bounding box on it.
    set_xy(&mut mem, 0, 56, 88);
    run_frames(&mut mem, 2);

    let sb = mem.read(0xD01F);
    assert_eq!(sb & 0x01, 0x01, "crab overlapping a solid text-mode character should set sprite 0's bit in the sprite-background collision register, got {sb:#04x}");
    assert_eq!(mem.read(0xD01F), 0, "sprite-background collision register should clear on read");

    // Move the crab somewhere over plain (blank, screen-code-0, all-zero
    // glyph) background text and confirm no collision is latched.
    set_xy(&mut mem, 0, 250, 88);
    run_frames(&mut mem, 2);
    assert_eq!(mem.read(0xD01F), 0, "crab over blank background text should not collide");
}
