// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Regression tests for VIC-II extended colour mode (ECM, `$D011` bit 6) --
//! see `src/vic/mod.rs`'s module doc comment for exactly what's modelled.
//!
//! Same general approach as `tests/sprites.rs`: dummy (all-zero) ROMs, drive
//! the VIC directly via `Board::tick` with no CPU running, poke registers and
//! memory directly, then read pixels back out of the framebuffer. Pixels are
//! compared *to each other* rather than to hardcoded RGB constants, for the
//! same reason `tests/sprites.rs` does: it proves the actual logic under
//! test without duplicating the private `vic::PALETTE` table here -- except
//! for the "invalid combination renders solid black" checks, where asserting
//! literal (0,0,0) is asserting a real hardware invariant (not an arbitrary
//! palette entry) and so is fair game.

mod common;

use c64_core::{VideoStandard, C64};
use common::{blank_c64, tick};

const CYCLES_PER_LINE: u32 = 63;
const LINES_PER_FRAME: u32 = 312;
const ONE_FRAME: u32 = CYCLES_PER_LINE * LINES_PER_FRAME;
// Matches `Vic::render_x_shift()` / `Vic::width()` in src/vic for PAL --
// not exported (purely a rendering-time cosmetic detail), so duplicated here
// with this note pointing back at the source of truth.
const RENDER_X_SHIFT: u16 = 68;
const WIDTH: u16 = 504;

fn new_machine() -> C64 {
    blank_c64(VideoStandard::Pal)
}

fn run_frames(mem: &mut C64, frames: u32) {
    tick(mem, ONE_FRAME * frames);
}

fn pixel(mem: &C64, x: u16, y: u16) -> (u8, u8, u8) {
    let out_x = (x + RENDER_X_SHIFT) % WIDTH;
    let fb = mem.framebuffer();
    let idx = (y as usize * WIDTH as usize + out_x as usize) * 4;
    (fb[idx], fb[idx + 1], fb[idx + 2])
}

/// Programs a plain text-mode display with the video matrix at $0400 and
/// the character generator at $0000 -- deliberately *not* $1000-$1FFF,
/// which in VIC bank 0 is hardwired to the character ROM overlay (see
/// `vic::VicMem::read`); with dummy all-zero ROMs that overlay would
/// make every character render as blank regardless of what this test wrote
/// to the underlying RAM. Putting the character generator at $0000 instead
/// keeps it plain RAM, so each test can write exact glyph bytes.
fn setup_text_display(mem: &mut C64, den_extra_bits: u8) {
    mem.write(0xD011, 0x10 | den_extra_bits); // DEN, plus caller's mode bits
    mem.write(0xD016, 0x08); // CSEL (40 columns); MCM left to the caller
    mem.write(0xD018, 0x10); // video matrix @ $0400, char generator @ $0000
}

/// Writes one column's (screen code, colour RAM nibble) pair to *every*
/// character row's video-matrix slot (not just row 0) -- because which of
/// the 25 character rows a given visible raster line belongs to depends on
/// VC, which advances by 40 per row, this sidesteps having to compute that
/// mapping (and its row-0-is-partially-clipped special case, since row 0's
/// first 3 sub-lines fall above `y_top` and render as border) by simply
/// making every row identical, the same trick `tests/sprites.rs` uses for
/// its own bitmap-mode MxDP test. Also writes, at the character index the
/// screen code's low 6 bits select, one glyph byte applied to every one of
/// its 8 raster rows (rows are identical here since these tests only ever
/// check a single raster line's worth of pixels).
fn set_column(mem: &mut C64, col: u16, screen_code: u8, color_nibble: u8, glyph_byte: u8) {
    for row_idx in 0..25u16 {
        mem.write(0x0400 + row_idx * 40 + col, screen_code);
        mem.write(0xD800 + row_idx * 40 + col, color_nibble);
    }
    let char_index = (screen_code & 0x3F) as u16;
    for row in 0..8u16 {
        mem.write(char_index * 8 + row, glyph_byte);
    }
}

// Any raster line inside the 25-row display window (y_top=$33=51 to
// y_bot=$fa=250) works, now that `set_column` populates every character
// row identically.
const RASTER: u16 = 100;
const X_LEFT: u16 = 24;

#[test]
fn ecm_text_mode_picks_one_of_four_background_registers() {
    let mut mem = new_machine();
    setup_text_display(&mut mem, 0x40 | 0x08); // ECM, RSEL (25 rows)

    // Four distinct background colours, one per $D021-$D024, plus one
    // foreground colour (from colour RAM) shared by every test column.
    mem.write(0xD021, 1); // background 0: white
    mem.write(0xD022, 2); // background 1: red
    mem.write(0xD023, 3); // background 2: cyan
    mem.write(0xD024, 4); // background 3: purple
    let foreground_nibble = 5; // green

    // Same character index (2) in every column -- only the top two bits
    // (the background-register select) differ -- with a glyph whose top bit
    // is set (foreground) and second bit clear (background), so each column
    // shows one foreground pixel and one background pixel side by side.
    for bg_select in 0..4u8 {
        let screen_code = (bg_select << 6) | 2;
        set_column(&mut mem, bg_select as u16, screen_code, foreground_nibble, 0b1000_0000);
    }

    run_frames(&mut mem, 2);

    let fg: Vec<_> = (0..4u16).map(|col| pixel(&mem, X_LEFT + col * 8, RASTER)).collect();
    let bg: Vec<_> = (0..4u16).map(|col| pixel(&mem, X_LEFT + col * 8 + 1, RASTER)).collect();

    // Every column used the identical character index and colour-RAM
    // nibble, so if ECM correctly masks the screen code to its low 6 bits
    // before addressing the character generator (rather than letting the
    // background-select bits leak into the address), every column fetches
    // the *same* glyph byte -- meaning every column's foreground pixel is
    // present and identical.
    for (i, &p) in fg.iter().enumerate() {
        assert_eq!(p, fg[0], "column {i}'s foreground pixel should match column 0's (same char index, same colour-RAM nibble) -- a mismatch suggests the background-select bits leaked into the character-generator address");
    }

    // But the four background pixels must all be *different* -- each
    // column's top-two screen-code bits should have selected a genuinely
    // different one of $D021/$D022/$D023/$D024.
    for i in 0..4 {
        for j in (i + 1)..4 {
            assert_ne!(
                bg[i], bg[j],
                "background pixels for bg_select={i} and bg_select={j} should differ (different $D021-$D024 registers)"
            );
        }
    }
    // And the foreground colour must differ from every background -- this
    // is colour-RAM-driven, entirely independent of the $D021-$D024 quartet.
    for (i, &b) in bg.iter().enumerate() {
        assert_ne!(fg[0], b, "foreground pixel should differ from bg_select={i}'s background");
    }
}

#[test]
fn ecm_plus_mcm_without_bmm_is_solid_black() {
    let mut mem = new_machine();
    // ECM (0x40) + MCM (0x16 bit 4) with BMM clear: one of the three
    // combinations Christian Bauer's VIC-II article documents as producing
    // "only black pixels" on real hardware.
    setup_text_display(&mut mem, 0x40 | 0x08);
    mem.write(0xD016, 0x08 | 0x10); // CSEL, MCM
    mem.write(0xD021, 1); // background colour 0: deliberately non-black,
                          // so a passing test proves the invalid-mode path
                          // overrides it rather than coincidentally landing
                          // on black some other way.
    set_column(&mut mem, 0, 0b11_000010, 5, 0xFF); // would be fully "foreground" in any valid mode

    run_frames(&mut mem, 2);

    let p = pixel(&mem, X_LEFT, RASTER);
    assert_eq!(p, (0, 0, 0), "ECM+MCM (BMM clear) should render solid black, got {p:?}");
}

#[test]
fn ecm_plus_bmm_is_solid_black_regardless_of_mcm() {
    for mcm in [false, true] {
        let mut mem = new_machine();
        // ECM (0x40) + BMM (0x20): invalid regardless of MCM.
        setup_text_display(&mut mem, 0x40 | 0x20 | 0x08);
        let d016 = if mcm { 0x08 | 0x10 } else { 0x08 };
        mem.write(0xD016, d016);
        mem.write(0xD018, 0x18); // video matrix @ $0400, bitmap-style base @ $2000
        mem.write(0xD021, 1); // non-black background, same reasoning as above
        for addr in 0x2000..0x2000 + 8000u32 {
            mem.write(addr as u16, 0xFF); // would be fully "foreground" in valid bitmap mode
        }
        for addr in 0x0400..0x0400 + 40u16 {
            mem.write(addr, 0x51); // arbitrary non-zero colour/foreground nibbles
        }

        run_frames(&mut mem, 2);

        let p = pixel(&mem, X_LEFT, RASTER);
        assert_eq!(p, (0, 0, 0), "ECM+BMM (MCM={mcm}) should render solid black, got {p:?}");
    }
}
