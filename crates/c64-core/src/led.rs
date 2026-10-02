// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Indicator LEDs. A LED driven by firmware can flicker faster than a
//! video frame, so it records how much of the time it was lit; a front
//! end draws that average as its brightness.

/// The LED's colour (the C64's power LED and the 1541's power LED are
/// green on most units, the 1541's activity LED is red).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedColor {
    Red,
    Green,
}

/// One LED with its duty-cycle bookkeeping.
#[derive(Debug, Clone)]
pub struct Led {
    color: LedColor,
    lit: bool,
    lit_cycles: u32,
    window: u32,
}

impl Led {
    pub fn new(color: LedColor) -> Self {
        Led { color, lit: false, lit_cycles: 0, window: 0 }
    }

    pub fn color(&self) -> LedColor {
        self.color
    }

    /// Advance one clock cycle with the LED driven `lit` or not.
    #[inline]
    pub fn clock(&mut self, lit: bool) {
        self.lit = lit;
        self.window = self.window.saturating_add(1);
        if lit {
            self.lit_cycles = self.lit_cycles.saturating_add(1);
        }
    }

    /// Whether it is lit right now.
    pub fn is_lit(&self) -> bool {
        self.lit
    }

    /// Fraction of cycles (0.0-1.0) it was lit since the previous call;
    /// the instantaneous state if no cycles have passed.
    pub fn take_duty(&mut self) -> f32 {
        let duty = if self.window == 0 {
            if self.lit {
                1.0
            } else {
                0.0
            }
        } else {
            self.lit_cycles as f32 / self.window as f32
        };
        self.lit_cycles = 0;
        self.window = 0;
        duty
    }
}
