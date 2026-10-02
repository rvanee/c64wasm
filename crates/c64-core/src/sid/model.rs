// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! SID chip revisions.

/// The two SID revisions. They differ in the analog parts: the filter's
/// cutoff curve and the DC offset in the mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidModel {
    /// The original (breadbin C64): non-linear filter, large DC offset that
    /// makes volume-register "digis" audible.
    Mos6581,
    /// The later C64C chip: linear filter, almost no DC offset.
    Mos8580,
}

/// Piecewise-linear approximation of the 6581 cutoff curve (FC register
/// value -> Hz). Approximate: real 6581s vary considerably chip to chip.
const CUTOFF_6581: [(f32, f32); 12] = [
    (0.0, 220.0),
    (256.0, 260.0),
    (512.0, 450.0),
    (640.0, 800.0),
    (768.0, 1600.0),
    (896.0, 3000.0),
    (1024.0, 4300.0),
    (1280.0, 6900.0),
    (1536.0, 9400.0),
    (1792.0, 11400.0),
    (1920.0, 12000.0),
    (2047.0, 12300.0),
];

impl SidModel {
    /// Filter cutoff frequency in Hz for the 11-bit FC register value.
    pub fn cutoff_hz(self, fc: u16) -> f32 {
        let fc = fc as f32;
        match self {
            SidModel::Mos8580 => 30.0 + fc * (12_000.0 - 30.0) / 2047.0,
            SidModel::Mos6581 => {
                for w in CUTOFF_6581.windows(2) {
                    let ((x0, y0), (x1, y1)) = (w[0], w[1]);
                    if fc <= x1 {
                        return y0 + (y1 - y0) * (fc - x0) / (x1 - x0);
                    }
                }
                CUTOFF_6581[CUTOFF_6581.len() - 1].1
            }
        }
    }

    /// DC offset in the mixer, scaled by the volume register.
    pub fn mixer_dc(self) -> f32 {
        match self {
            SidModel::Mos6581 => 0.35,
            SidModel::Mos8580 => 0.02,
        }
    }
}
