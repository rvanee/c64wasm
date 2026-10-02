// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The SID's filter, approximated by an ideal Chamberlin state-variable
//! filter run every cycle (no distortion, no chip-to-chip spread).

#[derive(Clone, Debug)]
pub(super) struct Filter {
    lp: f32,
    bp: f32,
    f_coef: f32,
    q_inv: f32,
}

/// Outputs of one filter step.
pub(super) struct FilterOut {
    pub lp: f32,
    pub bp: f32,
    pub hp: f32,
}

impl Filter {
    pub fn new() -> Self {
        Filter { lp: 0.0, bp: 0.0, f_coef: 0.0, q_inv: 1.0 }
    }

    /// Set the cutoff (Hz, at a chip clock of `clock_hz`) and the 4-bit
    /// resonance.
    pub fn configure(&mut self, cutoff_hz: f32, resonance: u8, clock_hz: u32) {
        self.f_coef = 2.0 * (std::f32::consts::PI * cutoff_hz / clock_hz as f32).sin();
        let q = 0.707 + resonance as f32 / 8.0;
        self.q_inv = 1.0 / q;
    }

    #[inline]
    pub fn step(&mut self, input: f32) -> FilterOut {
        let hp = input - self.lp - self.q_inv * self.bp;
        self.bp += self.f_coef * hp;
        self.lp += self.f_coef * self.bp;
        FilterOut { lp: self.lp, bp: self.bp, hp }
    }
}
