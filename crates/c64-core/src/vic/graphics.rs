// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The graphics data sequencer: turns fetched character/bitmap bytes into
//! pixels for the five display modes (plus the three invalid ones).

/// One g-access waiting to be loaded into the shifter at `load_x`.
#[derive(Clone, Copy, Default)]
pub(super) struct GFetch {
    pub load_x: u16,
    pub gdata: u8,
    pub char_code: u8,
    pub color: u8,
    pub valid: bool,
}

/// The shift register and the attributes of the byte being shifted out.
#[derive(Default)]
pub(super) struct Sequencer {
    pending: [GFetch; 2],
    shift: u8,
    char_code: u8,
    color: u8,
    /// ECM<<2 | BMM<<1 | MCM at load time.
    mode: u8,
    mc_phase: u8,
    mc_bits: u8,
    active: bool,
}

impl Sequencer {
    pub fn queue(&mut self, f: GFetch) {
        if !self.pending[0].valid {
            self.pending[0] = f;
        } else {
            self.pending[1] = f;
        }
    }

    /// Load the next byte when the beam reaches its X position (XSCROLL
    /// delays this); a mode change takes effect at the load.
    #[inline]
    pub fn load_if_due(&mut self, x: u16, mode_now: u8) {
        if self.pending[0].valid && self.pending[0].load_x == x {
            let f = self.pending[0];
            self.pending[0] = self.pending[1];
            self.pending[1].valid = false;
            self.shift = f.gdata;
            self.char_code = f.char_code;
            self.color = f.color;
            self.mode = mode_now;
            self.mc_phase = 0;
            self.active = true;
        }
    }

    /// The next pixel: (colour index, is foreground). `regs` are the VIC
    /// registers (background colours $D021-$D024).
    #[inline]
    pub fn pixel(&mut self, regs: &[u8; 64]) -> (u8, bool) {
        if !self.active {
            return (regs[0x21] & 0x0F, false);
        }
        let m = self.mode;
        let mc_char = m & 1 != 0 && (m & 2 != 0 || self.color & 8 != 0);
        let (bits, two) = if mc_char {
            if self.mc_phase == 0 {
                self.mc_bits = self.shift >> 6;
            }
            (self.mc_bits, true)
        } else {
            (self.shift >> 7, false)
        };
        self.shift <<= 1;
        self.mc_phase ^= 1;
        let ch = self.char_code;
        let col = self.color;
        match m {
            // standard text
            0 => {
                if bits != 0 {
                    (col, true)
                } else {
                    (regs[0x21] & 15, false)
                }
            }
            // multicolour text
            1 => {
                if two {
                    match bits {
                        0 => (regs[0x21] & 15, false),
                        1 => (regs[0x22] & 15, false),
                        2 => (regs[0x23] & 15, true),
                        _ => (col & 7, true),
                    }
                } else if bits != 0 {
                    (col & 7, true)
                } else {
                    (regs[0x21] & 15, false)
                }
            }
            // standard bitmap
            2 => {
                if bits != 0 {
                    (ch >> 4, true)
                } else {
                    (ch & 15, false)
                }
            }
            // multicolour bitmap
            3 => match bits {
                0 => (regs[0x21] & 15, false),
                1 => (ch >> 4, false),
                2 => (ch & 15, true),
                _ => (col, true),
            },
            // extended background colour text
            4 => {
                if bits != 0 {
                    (col, true)
                } else {
                    (regs[0x21 + (ch >> 6) as usize] & 15, false)
                }
            }
            // invalid modes: black, but foreground decoding still runs
            5 => (0, if two { bits >= 2 } else { bits != 0 }),
            6 => (0, bits != 0),
            _ => (0, bits >= 2),
        }
    }

    /// After the last load of a line the shifter outputs background.
    pub fn end_of_line(&mut self) {
        self.active = false;
    }
}
