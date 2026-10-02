// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Regression tests for `VideoStandard`-selectable timing (see
//! `src/clock.rs` and `src/vic/model.rs` for the confirmed
//! figures and what's assumed rather than independently re-verified for
//! NTSC's bad-line/display-window geometry).
//!
//! `tests/boot.rs` already confirms a full KERNAL/BASIC boot reaches READY
//! under both standards; these tests instead check the VIC-II's own raw
//! geometry and rendering directly, the same way `tests/sprites.rs` and
//! `tests/ecm.rs` do (dummy ROMs, `Board::tick` with no CPU running).

mod common;

use c64_core::VideoStandard;
use common::{blank_c64, tick};

#[test]
fn framebuffer_dimensions_match_video_standard() {
    let pal = blank_c64(VideoStandard::Pal);
    assert_eq!(pal.framebuffer_size(), (504, 312), "PAL: 63 cycles/line * 8 = 504 wide, 312 lines/frame tall");

    let ntsc = blank_c64(VideoStandard::Ntsc);
    assert_eq!(
        ntsc.framebuffer_size(),
        (520, 263),
        "NTSC (6567R8): 65 cycles/line * 8 = 520 wide, 263 lines/frame tall"
    );
}

#[test]
fn clock_hz_matches_video_standard() {
    let pal = blank_c64(VideoStandard::Pal);
    assert_eq!(pal.clock_hz(), 985_248);
    assert_eq!(pal.clock_hz(), VideoStandard::Pal.clock_hz());

    let ntsc = blank_c64(VideoStandard::Ntsc);
    assert_eq!(ntsc.clock_hz(), 1_022_727);
    assert_eq!(ntsc.clock_hz(), VideoStandard::Ntsc.clock_hz());
}

/// Ticking exactly one full frame's worth of cycles (cycles-per-line *
/// lines-per-frame) should bring the raster back to the same rendering
/// state it started in -- observable indirectly, since `Vic`'s raster
/// counters aren't public, by checking that the picture rendered at the
/// end of frame N+1 is pixel-identical to the picture rendered at the end
/// of frame N for a perfectly static display (nothing here changes any
/// register mid-run). This exercises the frame-wrap arithmetic
/// (`cycle_in_line`/`raster_line` wraparound) for each standard's own
/// cycle/line counts, not just PAL's.
fn assert_frame_wraps_cleanly(standard: VideoStandard) {
    let mut mem = blank_c64(standard);
    mem.write(0xD011, 0x1B); // DEN, BMM, RSEL, Y-scroll 0 -- a static bitmap display
    mem.write(0xD016, 0x08); // CSEL (40 columns)
    mem.write(0xD018, 0x18); // video matrix @ $0400, bitmap @ $2000
    for addr in 0x0400..0x0400 + 40u16 {
        mem.write(addr, 0x51); // arbitrary non-zero foreground/background nibbles
    }
    for addr in 0x2000..0x2000 + 8000u32 {
        mem.write(addr as u16, 0xAA); // arbitrary non-zero, non-uniform bitmap pattern
    }

    let cycles_per_frame = standard.cycles_per_line() as u32 * standard.lines_per_frame() as u32;
    tick(&mut mem, cycles_per_frame);
    let frame1 = mem.framebuffer().to_vec();
    tick(&mut mem, cycles_per_frame);
    let frame2 = mem.framebuffer().to_vec();

    assert_eq!(frame1, frame2, "{standard:?}: a static display should render pixel-identically frame over frame once the raster has wrapped exactly once");
}

#[test]
fn pal_frame_wraps_cleanly() {
    assert_frame_wraps_cleanly(VideoStandard::Pal);
}

#[test]
fn ntsc_frame_wraps_cleanly() {
    assert_frame_wraps_cleanly(VideoStandard::Ntsc);
}
