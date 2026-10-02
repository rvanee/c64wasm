// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! One of the three voices: oscillator, envelope and the control register
//! that ties them together.

use super::envelope::Envelope;
use super::oscillator::{Oscillator, CTRL_TEST};

const CTRL_GATE: u8 = 0x01;

#[derive(Clone, Debug)]
pub(super) struct Voice {
    pub osc: Oscillator,
    pub env: Envelope,
    pub control: u8,
}

impl Voice {
    pub fn new() -> Self {
        Voice { osc: Oscillator::new(), env: Envelope::new(), control: 0 }
    }

    /// Write one of the voice's seven registers.
    pub fn write(&mut self, reg: u8, val: u8) {
        let o = &mut self.osc;
        match reg {
            0 => o.freq = (o.freq & 0xFF00) | val as u16,
            1 => o.freq = (o.freq & 0x00FF) | ((val as u16) << 8),
            2 => o.pw = (o.pw & 0x0F00) | val as u16,
            3 => o.pw = (o.pw & 0x00FF) | (((val & 0x0F) as u16) << 8),
            4 => self.write_control(val),
            5 => self.env.write_attack_decay(val),
            _ => self.env.write_sustain_release(val),
        }
    }

    fn write_control(&mut self, val: u8) {
        let gate_was = self.control & CTRL_GATE != 0;
        let gate = val & CTRL_GATE != 0;
        if gate != gate_was {
            self.env.gate(gate);
        }
        if val & CTRL_TEST != 0 {
            self.osc.test_reset();
        }
        self.control = val;
    }

    #[inline]
    pub fn waveform(&self, ring_msb: bool) -> u16 {
        self.osc.waveform(self.control, ring_msb)
    }
}
