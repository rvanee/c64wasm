// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! An 8-bit parallel I/O port with a data-direction register -- the same
//! circuit on the CIA (6526) and the VIA (6522).

/// Output latch, data direction (1 = output) and the levels the outside
/// world puts on the pins.
#[derive(Debug, Clone, Copy)]
pub struct Port {
    /// The output register (ORA/ORB, PRA/PRB).
    pub data: u8,
    /// The data-direction register: 1 = output.
    pub ddr: u8,
    /// External levels on the pins; only input bits use them.
    input: u8,
}

impl Default for Port {
    fn default() -> Self {
        // Nothing connected: inputs float high.
        Port { data: 0, ddr: 0, input: 0xFF }
    }
}

impl Port {
    /// Pin levels: the latch on output bits, the external level on inputs.
    #[inline]
    pub fn pins(&self) -> u8 {
        (self.data & self.ddr) | (self.input & !self.ddr)
    }

    /// Set the external levels of all eight pins.
    #[inline]
    pub fn set_input(&mut self, levels: u8) {
        self.input = levels;
    }

    /// Output bits currently driven low (what a keyboard matrix scans with).
    #[inline]
    pub fn driven_low(&self) -> u8 {
        self.ddr & !self.data
    }
}
