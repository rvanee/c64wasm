// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Regression tests for VIC-II hardware sprites (see `src/vic/mod.rs`'s module
//! doc comment for exactly what's modelled and what's simplified). Unlike
//! `tests/boot.rs`, these need no ROM images and never skip themselves: they
//! drive the VIC directly via `Board::tick` with no CPU running at all, so
//! they're a test of the sprite hardware in isolation, not of anything a
//! KERNAL/BASIC program does with it.
//!
//! General approach: build a `Memory` with dummy (all-zero) ROMs -- fine,
//! since nothing here ever reads ROM content -- program the VIC into a
//! static standard-bitmap display (so "foreground" vs "background" pixels
//! are under this test's own direct control rather than depending on
//! chargen ROM glyph shapes it doesn't have anyway), set up sprite
//! registers and data by poking memory directly (`C64::write`, bypassing
//! the CPU entirely), run a couple of full frames so the VIC's own
//! bad-line-driven fetch/render machinery settles, then read pixels back
//! out of the framebuffer and the collision registers.
//!
//! Rather than assert exact RGB values (which would mean duplicating
//! the private `vic::PALETTE` table here and coupling every test to its
//! exact numbers), these tests compare pixels *to each other*: "this pixel
//! equals that reference pixel" / "these two pixels differ" / "this pixel
//! equals the plain background colour". That proves the actual logic under
//! test (bounding boxes, priority, collision, multicolour decoding) without
//! caring what the 16 palette entries actually render as.

mod common;

use c64_core::{VideoStandard, C64};
use common::{blank_c64, tick};

const CYCLES_PER_LINE: u32 = 63;
const LINES_PER_FRAME: u32 = 312;
const ONE_FRAME: u32 = CYCLES_PER_LINE * LINES_PER_FRAME;
// Matches `Vic::RENDER_X_SHIFT` / `Vic::WIDTH` in src/vic -- not exported
// (purely a rendering-time cosmetic detail), so duplicated here with this
// note pointing back at the source of truth.
const RENDER_X_SHIFT: u16 = 68;
const WIDTH: u16 = 504;

fn new_machine() -> C64 {
    // Sprite logic doesn't depend on the video standard (position/size/
    // priority/collision are all standard-independent), so these tests just
    // fix PAL -- see tests/ntsc.rs for the standard-selection tests.
    blank_c64(VideoStandard::Pal)
}

fn run_frames(mem: &mut C64, frames: u32) {
    tick(mem, ONE_FRAME * frames);
}

/// Pixel colour at raw VIC raster coordinates (the same coordinate space
/// sprite X/Y registers use), applying the same horizontal cosmetic shift
/// rendering does.
fn pixel(mem: &C64, x: u16, y: u16) -> (u8, u8, u8) {
    let out_x = (x + RENDER_X_SHIFT) % WIDTH;
    let fb = mem.framebuffer();
    let idx = (y as usize * WIDTH as usize + out_x as usize) * 4;
    (fb[idx], fb[idx + 1], fb[idx + 2])
}

/// Programs a plain, static standard bitmap-mode (hi-res) display: DEN on,
/// 25 rows, 40 columns, video matrix at $0400, bitmap data at $2000, and
/// every bitmap byte + screen colour byte cleared to 0 (i.e. the whole
/// picture is background colour everywhere until the test pokes specific
/// bytes to make parts of it "foreground").
fn setup_plain_bitmap_display(mem: &mut C64) {
    mem.write(0xD011, 0x38); // DEN|BMM|RSEL, Y-scroll 0
    mem.write(0xD016, 0x08); // CSEL (40 columns), hi-res (MCM off)
    mem.write(0xD018, 0x18); // video matrix @ $0400, bitmap @ $2000
    for addr in 0x0400..0x0400 + 40u16 {
        mem.write(addr, 0x10); // colour nibbles: hi=1 (foreground), lo=0 (background)
    }
    for addr in 0x2000..0x2000 + 8000u32 {
        mem.write(addr as u16, 0x00); // every pixel starts as "background"
    }
}

/// Writes a sprite pointer (into the last 8 bytes of the $0400 video
/// matrix set up by `setup_plain_bitmap_display`) and 63 bytes of shape
/// data at the block it points to.
fn set_sprite_data(mem: &mut C64, sprite: u8, block: u8, rows: &[[u8; 3]; 21]) {
    mem.write(0x0400 + 0x03F8 + sprite as u16, block);
    let base = block as u16 * 64;
    for (row, bytes) in rows.iter().enumerate() {
        for (b, byte) in bytes.iter().enumerate() {
            mem.write(base + (row as u16) * 3 + b as u16, *byte);
        }
    }
}

fn solid_sprite_rows() -> [[u8; 3]; 21] {
    [[0xFF, 0xFF, 0xFF]; 21]
}

fn set_sprite_xy(mem: &mut C64, sprite: u8, x: u16, y: u8) {
    mem.write(0xD000 + 2 * sprite as u16, (x & 0xFF) as u8);
    mem.write(0xD001 + 2 * sprite as u16, y);
    let msb = mem.read(0xD010);
    let bit = if x > 0xFF { 1u8 << sprite } else { 0 };
    mem.write(0xD010, (msb & !(1 << sprite)) | bit);
}

#[test]
fn sprite_appears_only_inside_its_bounding_box() {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    set_sprite_data(&mut mem, 0, 4, &solid_sprite_rows());
    set_sprite_xy(&mut mem, 0, 100, 99); // Y reg 99 -> first visible line 100
    mem.write(0xD027, 2); // sprite 0 colour
    mem.write(0xD015, 0x01); // enable sprite 0

    run_frames(&mut mem, 2);

    let bg = pixel(&mem, 200, 150); // far from the sprite: plain background
    let sprite_center = pixel(&mem, 100 + 12, 100 + 10); // middle of the 24x21 box

    assert_ne!(sprite_center, bg, "sprite pixel should differ from plain background");

    // Precise boundary check: last visible column (x=123, col 23) opaque,
    // first invisible column just past it (x=124) is not; same for Y.
    assert_eq!(pixel(&mem, 100 + 23, 105), sprite_center, "sprite's last column (23) should still be opaque");
    assert_eq!(pixel(&mem, 100 + 24, 105), bg, "one pixel past the sprite's 24-pixel width should be background again");
    assert_eq!(pixel(&mem, 110, 100 + 20), sprite_center, "sprite's last row (20) should still be opaque");
    assert_eq!(pixel(&mem, 110, 100 + 21), bg, "one line past the sprite's 21-line height should be background again");
    // And one pixel *before* the box on both axes should also be plain background.
    assert_eq!(pixel(&mem, 99, 105), bg, "one pixel left of the sprite should be background");
    assert_eq!(pixel(&mem, 110, 99), bg, "one line above the sprite should be background");
}

#[test]
fn x_and_y_expansion_double_the_sprite() {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    set_sprite_data(&mut mem, 0, 4, &solid_sprite_rows());
    set_sprite_xy(&mut mem, 0, 100, 99);
    mem.write(0xD027, 2);
    mem.write(0xD015, 0x01);
    mem.write(0xD01D, 0x01); // X-expand sprite 0
    mem.write(0xD017, 0x01); // Y-expand sprite 0

    run_frames(&mut mem, 2);

    let bg = pixel(&mem, 200, 150);
    let sprite_center = pixel(&mem, 100 + 12, 100 + 10);
    assert_ne!(sprite_center, bg);

    // Un-expanded width/height would end at x=123 (col 23) / y=120 (row 20);
    // expanded, the box now runs to x=147 / y=141.
    assert_eq!(pixel(&mem, 100 + 47, 105), sprite_center, "X-expanded sprite should still be opaque at column 47");
    assert_eq!(pixel(&mem, 100 + 48, 105), bg, "X-expanded sprite should end exactly at column 48");
    assert_eq!(pixel(&mem, 110, 100 + 41), sprite_center, "Y-expanded sprite should still be opaque at row 41");
    assert_eq!(pixel(&mem, 110, 100 + 42), bg, "Y-expanded sprite should end exactly at row 42");
}

#[test]
fn multicolor_sprite_decodes_all_three_colours() {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    // Row 0's three bytes, as 2-bit pairs: 01 01 01 01 | 10 10 10 10 | 11 11 11 11
    // -> first 4 double-wide pixels use shared multicolour 0, next 4 use
    // this sprite's own colour, last 4 use shared multicolour 1.
    let mut rows = [[0u8; 3]; 21];
    rows[0] = [0b01010101, 0b10101010, 0b11111111];
    set_sprite_data(&mut mem, 0, 4, &rows);
    set_sprite_xy(&mut mem, 0, 100, 99);
    mem.write(0xD01C, 0x01); // multicolour mode, sprite 0
    mem.write(0xD025, 1); // shared multicolour 0
    mem.write(0xD026, 3); // shared multicolour 1
    mem.write(0xD027, 5); // sprite 0's own colour
    mem.write(0xD015, 0x01);

    run_frames(&mut mem, 2);

    let bg = pixel(&mem, 200, 150);
    let y = 100; // row 0 of the sprite
    let mc0_px = pixel(&mem, 100 + 1, y); // within the first 4 double-wide pixels
    let own_px = pixel(&mem, 100 + 9, y); // within the middle 4
    let mc1_px = pixel(&mem, 100 + 17, y); // within the last 4

    assert_ne!(mc0_px, bg, "01 pair should be opaque (shared multicolour 0)");
    assert_ne!(own_px, bg, "10 pair should be opaque (sprite's own colour)");
    assert_ne!(mc1_px, bg, "11 pair should be opaque (shared multicolour 1)");
    assert_ne!(mc0_px, own_px, "shared multicolour 0 and the sprite's own colour must differ (colours 1 vs 5)");
    assert_ne!(mc0_px, mc1_px, "the two shared multicolours must differ (colours 1 vs 3)");
    assert_ne!(own_px, mc1_px, "the sprite's own colour and shared multicolour 1 must differ (colours 5 vs 3)");
}

#[test]
fn lower_numbered_sprite_wins_display_priority_and_sets_collision() {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    set_sprite_data(&mut mem, 0, 4, &solid_sprite_rows());
    set_sprite_data(&mut mem, 1, 5, &solid_sprite_rows());
    set_sprite_xy(&mut mem, 0, 100, 99);
    set_sprite_xy(&mut mem, 1, 110, 99); // overlaps sprite 0's right side
    mem.write(0xD027, 2); // sprite 0 colour
    mem.write(0xD028, 6); // sprite 1 colour (different)
    mem.write(0xD015, 0x03); // enable both

    run_frames(&mut mem, 2);

    let sprite0_only = pixel(&mem, 102, 105); // inside sprite 0, left of the overlap
    let sprite1_only = pixel(&mem, 130, 105); // inside sprite 1, right of the overlap
    let overlap = pixel(&mem, 115, 105); // inside both bounding boxes

    assert_ne!(sprite0_only, sprite1_only, "the two sprites must render in different colours");
    assert_eq!(overlap, sprite0_only, "sprite 0 (lower-numbered) must win the overlap");

    let collisions = mem.read(0xD01E);
    assert_eq!(
        collisions & 0x03,
        0x03,
        "both sprite 0 and sprite 1 should be flagged as collided, got {collisions:#04x}"
    );
    let collisions_again = mem.read(0xD01E);
    assert_eq!(collisions_again, 0, "$D01E must read as 0 immediately after being read once (clear-on-read)");
}

#[test]
fn display_priority_and_background_collision_respect_mxdp() {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    // Make the *entire* bitmap solid foreground (all 8000 bytes, i.e. every
    // character row), so a sprite placed anywhere in the display window
    // overlaps genuine "foreground" pixels rather than plain background
    // colour, without having to work out which character row a given
    // raster line falls in.
    for addr in 0x2000..0x2000 + 8000u32 {
        mem.write(addr as u16, 0xFF);
    }
    set_sprite_data(&mut mem, 0, 4, &solid_sprite_rows());
    set_sprite_xy(&mut mem, 0, 100, 99); // first sprite line = raster 100, inside that foreground row
    mem.write(0xD027, 2);
    mem.write(0xD015, 0x01);
    mem.write(0xD01B, 0x01); // MxDP: sprite 0 behind foreground

    run_frames(&mut mem, 2);

    let foreground_only = pixel(&mem, 200, 100); // foreground pixel, no sprite here (well inside the display window)
    let behind_sprite_over_fg = pixel(&mem, 105, 100); // sprite overlapping foreground, MxDP=1
    assert_eq!(behind_sprite_over_fg, foreground_only, "with MxDP=1 the foreground pixel should win over the sprite");

    let bg_collisions = mem.read(0xD01F);
    assert_eq!(
        bg_collisions & 0x01,
        0x01,
        "sprite 0 overlapping a foreground pixel should set its $D01F bit, got {bg_collisions:#04x}"
    );
    assert_eq!(mem.read(0xD01F), 0, "$D01F must clear on read");

    // Now flip MxDP to "in front" and confirm the sprite wins instead.
    mem.write(0xD01B, 0x00);
    run_frames(&mut mem, 2);
    let front_sprite_over_fg = pixel(&mem, 105, 100);
    assert_ne!(
        front_sprite_over_fg, foreground_only,
        "with MxDP=0 the sprite should be drawn in front of the foreground pixel"
    );
}

/// The border has priority over sprites on the real VIC-II: a sprite in
/// the border area is hidden unless the border is "opened" (the classic
/// demo trick of preventing the border flip-flops from being set). An
/// earlier version of this test asserted the opposite, matching the first,
/// non-cycle-exact renderer rather than the hardware.
#[test]
fn sprite_is_hidden_by_the_border() {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    set_sprite_data(&mut mem, 0, 4, &solid_sprite_rows());
    // X=5 is well inside the left border (display window starts at x=24).
    set_sprite_xy(&mut mem, 0, 5, 99);
    mem.write(0xD027, 2);
    mem.write(0xD015, 0x01);

    run_frames(&mut mem, 2);

    let border = pixel(&mem, 400, 105); // in the border, no sprite there
    let sprite_over_border = pixel(&mem, 12, 105); // inside the sprite, over the border
    assert_eq!(sprite_over_border, border, "the border covers sprites");
    // The part of the same sprite that reaches into the display window is visible.
    assert_ne!(pixel(&mem, 26, 105), pixel(&mem, 200, 150), "the sprite shows inside the display window");
}

/// Lines with sprite 0's DMA on in one frame, optionally clearing its Y
/// expansion in cycle 15 of the line after it starts (a "sprite crunch").
fn sprite_dma_lines(crunch: bool) -> u32 {
    let mut mem = new_machine();
    setup_plain_bitmap_display(&mut mem);
    set_sprite_data(&mut mem, 0, 4, &solid_sprite_rows());
    set_sprite_xy(&mut mem, 0, 100, 100);
    mem.write(0xD017, 0x01);
    mem.write(0xD015, 0x01);
    // Start from the top of a frame.
    while !(mem.vic().raster_line() == 0 && mem.vic().cycle() == 1) {
        common::tick(&mut mem, 1);
    }
    let mut lines = 0;
    let mut last_line = u16::MAX;
    loop {
        let (line, cycle) = (mem.vic().raster_line(), mem.vic().cycle());
        if crunch && line == 101 && cycle == 16 {
            // A CPU write lands after the VIC's work for its cycle: this is
            // a write in cycle 15.
            mem.write(0xD017, 0x00);
            mem.write(0xD017, 0x01);
        }
        if line != last_line && cycle == 20 {
            last_line = line;
            if mem.vic().sprite_state()[0].0 {
                lines += 1;
            }
        }
        common::tick(&mut mem, 1);
        if mem.vic().raster_line() == 0 && mem.vic().cycle() == 1 {
            return lines;
        }
    }
}

#[test]
fn sprite_crunch_lengthens_an_expanded_sprite() {
    // Without the trick, a Y-expanded sprite's DMA runs from cycle 55 of
    // its first line to cycle 16 of the 43rd: 41 lines sampled at cycle 20.
    assert_eq!(sprite_dma_lines(false), 41);
    // Clearing the expansion in cycle 15 while the flip-flop is reset mixes
    // MC into MCBASE (0 and 3 give 1), so MCBASE no longer steps in threes
    // to 63 and the sprite runs on (demos use this to stretch sprites; the
    // "Next Level" demo's face picture depends on it).
    assert!(sprite_dma_lines(true) > 41, "{}", sprite_dma_lines(true));
}
