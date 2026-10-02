// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! VIC-II chip variants and their raster geometry.

/// The VIC-II variant, which fixes the TV system.
///
/// |                 | PAL 6569 | NTSC 6567R8 |
/// |-----------------|---------:|------------:|
/// | cycles per line |       63 |          65 |
/// | lines per frame |      312 |         263 |
///
/// The older 64-cycle NTSC 6567R56A is not modelled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VicModel {
    /// MOS 6569 (PAL).
    Pal,
    /// MOS 6567R8 (NTSC).
    Ntsc,
}

impl VicModel {
    pub fn cycles_per_line(self) -> u16 {
        match self {
            VicModel::Pal => 63,
            VicModel::Ntsc => 65,
        }
    }

    pub fn lines_per_frame(self) -> u16 {
        match self {
            VicModel::Pal => 312,
            VicModel::Ntsc => 263,
        }
    }

    /// Framebuffer width: the whole raster line, 8 pixels per cycle.
    pub fn width(self) -> u16 {
        self.cycles_per_line() * 8
    }

    pub fn height(self) -> u16 {
        self.lines_per_frame()
    }

    /// Sprite X coordinate of pixel `raw` of the line (pixel 0 is the
    /// first pixel of cycle 1). PAL: cycle 1 starts at $194 and the
    /// coordinate wraps from $1F7 to 0.
    #[inline]
    pub fn xpos(self, raw: u16) -> u16 {
        match self {
            VicModel::Pal => (0x194 + raw) % 504,
            VicModel::Ntsc => (0x19C + raw) % 512,
        }
    }

    /// Framebuffer column of pixel `raw`, chosen so the display window
    /// (X $18) lands at column $18 + 68 (PAL) / + 76 (NTSC).
    #[inline]
    pub fn fb_x(self, raw: u16) -> u16 {
        let w = self.width();
        let shift = match self {
            VicModel::Pal => 32,
            VicModel::Ntsc => 24,
        };
        (raw + w - shift) % w
    }

    /// Cycle of sprite `n`'s pointer access (Bauer: 58 + 2n, wrapping into
    /// the next line).
    #[inline]
    pub fn sprite_p_cycle(self, n: usize) -> u16 {
        let cpl = self.cycles_per_line();
        let c = 58 + 2 * n as u16;
        if c > cpl {
            c - cpl
        } else {
            c
        }
    }
}
