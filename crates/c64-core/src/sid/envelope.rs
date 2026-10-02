// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! A voice's ADSR envelope generator (after reSID): an 8-bit counter
//! stepped by a 15-bit rate counter, with the piecewise-exponential
//! decay/release. The rate counter is compared for equality and wraps at
//! 2^15, so lowering the rate below the current count produces the real
//! "ADSR delay bug".

/// Envelope rate periods in cycles, indexed by the 4-bit A/D/R value
/// (reSID `rate_counter_period`).
pub const RATE_PERIODS: [u16; 16] =
    [9, 32, 63, 95, 149, 220, 267, 313, 392, 977, 1954, 3126, 3907, 11720, 19532, 31251];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Attack,
    DecaySustain,
    Release,
}

#[derive(Clone, Debug)]
pub(super) struct Envelope {
    attack_decay: u8,
    sustain_release: u8,
    level: u8,
    state: State,
    rate_counter: u16,
    rate_period: u16,
    exp_counter: u8,
    exp_period: u8,
    hold_zero: bool,
}

impl Envelope {
    pub fn new() -> Self {
        Envelope {
            attack_decay: 0,
            sustain_release: 0,
            level: 0,
            state: State::Release,
            rate_counter: 0,
            rate_period: RATE_PERIODS[0],
            exp_counter: 0,
            exp_period: 1,
            hold_zero: true,
        }
    }

    pub fn level(&self) -> u8 {
        self.level
    }

    /// The gate bit went from 0 to 1 (`true`) or 1 to 0 (`false`).
    pub fn gate(&mut self, on: bool) {
        if on {
            self.state = State::Attack;
            self.rate_period = RATE_PERIODS[(self.attack_decay >> 4) as usize];
            self.hold_zero = false;
        } else {
            self.state = State::Release;
            self.rate_period = RATE_PERIODS[(self.sustain_release & 0x0F) as usize];
        }
    }

    pub fn write_attack_decay(&mut self, val: u8) {
        self.attack_decay = val;
        match self.state {
            State::Attack => self.rate_period = RATE_PERIODS[(val >> 4) as usize],
            State::DecaySustain => self.rate_period = RATE_PERIODS[(val & 0x0F) as usize],
            State::Release => {}
        }
    }

    pub fn write_sustain_release(&mut self, val: u8) {
        self.sustain_release = val;
        if self.state == State::Release {
            self.rate_period = RATE_PERIODS[(val & 0x0F) as usize];
        }
    }

    #[inline]
    pub fn clock(&mut self) {
        self.rate_counter = (self.rate_counter + 1) & 0x7FFF;
        if self.rate_counter != self.rate_period {
            return;
        }
        self.rate_counter = 0;
        if self.state == State::Attack {
            self.exp_counter = 0;
            self.level = self.level.wrapping_add(1);
            if self.level == 0xFF {
                self.state = State::DecaySustain;
                self.rate_period = RATE_PERIODS[(self.attack_decay & 0x0F) as usize];
            }
        } else {
            self.exp_counter = self.exp_counter.wrapping_add(1);
            if self.exp_counter != self.exp_period {
                return;
            }
            self.exp_counter = 0;
            if self.hold_zero {
                return;
            }
            match self.state {
                State::DecaySustain => {
                    let sustain = (self.sustain_release >> 4) * 0x11;
                    if self.level != sustain {
                        self.level -= 1;
                    }
                }
                State::Release => self.level = self.level.wrapping_sub(1),
                State::Attack => {}
            }
        }
        self.exp_period = match self.level {
            0xFF => 1,
            0x5D => 2,
            0x36 => 4,
            0x1A => 8,
            0x0E => 16,
            0x06 => 30,
            0x00 => {
                self.hold_zero = true;
                1
            }
            _ => self.exp_period,
        };
    }
}
