// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! One of the eight sprite (MOB) units: DMA state and the pixel shifter.

#[derive(Clone, Copy, Default, Debug)]
pub(super) struct Sprite {
    /// Sprite DMA is on (data is fetched each line).
    pub dma: bool,
    /// The sprite is displayed on this line.
    pub display: bool,
    /// Y-expansion flip-flop: false on the first of the two lines an
    /// expanded sprite shows each row for.
    pub exp_ff: bool,
    pub mc: u8,
    pub mcbase: u8,
    pub pointer: u8,
    /// 24 bits fetched by the s-accesses.
    pub data: u32,
    shift: u32,
    shifting: bool,
    bits_left: u8,
    /// Repetition counter for X expansion and multicolour pixels.
    sub: u8,
    /// Current output: 0 = transparent, else the 1-3 colour code.
    cur: u8,
}

impl Sprite {
    /// Next pixel at sprite coordinate `x` for a sprite at `sx`. Returns
    /// 0 (transparent) or the colour code: 1 = multicolour 0 ($D025),
    /// 2 = the sprite's own colour, 3 = multicolour 1 ($D026).
    #[inline]
    pub fn pixel(&mut self, x: u16, sx: u16, x_expand: bool, multicolor: bool) -> u8 {
        if !self.shifting && x == sx {
            self.shifting = true;
            self.shift = self.data;
            self.bits_left = 24;
            self.sub = 0;
        }
        if !self.shifting {
            return 0;
        }
        if self.sub == 0 {
            self.cur = if multicolor {
                ((self.shift >> 22) & 3) as u8
            } else if self.shift & 0x80_0000 != 0 {
                2
            } else {
                0
            };
        }
        let cur = self.cur;
        let period = (if multicolor { 2 } else { 1 }) * (if x_expand { 2 } else { 1 });
        self.sub += 1;
        if self.sub >= period {
            self.sub = 0;
            let n = if multicolor { 2 } else { 1 };
            self.shift = (self.shift << n) & 0xFF_FFFF;
            self.bits_left = self.bits_left.saturating_sub(n);
            if self.bits_left == 0 {
                self.shifting = false;
            }
        }
        cur
    }

    /// The shifter stops at the end of the line.
    pub fn end_of_line(&mut self) {
        self.shifting = false;
    }
}
