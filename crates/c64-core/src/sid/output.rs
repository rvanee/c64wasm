// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The audio output: averaging the per-cycle output down to the host's
//! sample rate, then the C64's output capacitor (a ~16 Hz high-pass that
//! removes the mixer's DC offset).

/// Older samples are dropped beyond this (~2 s at 48 kHz).
const MAX_BUFFERED_SAMPLES: usize = 96_000;

#[derive(Clone, Debug)]
pub(super) struct AudioOutput {
    clock_hz: u32,
    sample_rate: u32,
    frac: u64,
    acc_sum: f32,
    acc_n: u32,
    hp_state: f32,
    hp_prev_in: f32,
    samples: Vec<f32>,
}

impl AudioOutput {
    pub fn new(clock_hz: u32) -> Self {
        AudioOutput {
            clock_hz,
            sample_rate: 44_100,
            frac: 0,
            acc_sum: 0.0,
            acc_n: 0,
            hp_state: 0.0,
            hp_prev_in: 0.0,
            samples: Vec::new(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn set_sample_rate(&mut self, hz: u32) {
        self.sample_rate = hz;
        self.frac = 0;
        self.acc_sum = 0.0;
        self.acc_n = 0;
        self.samples.clear();
    }

    pub fn take(&mut self, out: &mut Vec<f32>) {
        out.append(&mut self.samples);
    }

    /// Feed one chip cycle's output.
    #[inline]
    pub fn push(&mut self, out: f32) {
        if self.sample_rate == 0 {
            return;
        }
        self.acc_sum += out;
        self.acc_n += 1;
        self.frac += self.sample_rate as u64;
        if self.frac >= self.clock_hz as u64 {
            self.frac -= self.clock_hz as u64;
            let x = self.acc_sum / self.acc_n as f32;
            self.acc_sum = 0.0;
            self.acc_n = 0;
            let a = 1.0 - 2.0 * std::f32::consts::PI * 16.0 / self.sample_rate as f32;
            let y = a * (self.hp_state + x - self.hp_prev_in);
            self.hp_prev_in = x;
            self.hp_state = y;
            if self.samples.len() >= MAX_BUFFERED_SAMPLES {
                self.samples.drain(0..MAX_BUFFERED_SAMPLES / 2);
            }
            self.samples.push(y);
        }
    }
}
