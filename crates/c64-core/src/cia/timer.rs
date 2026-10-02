// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! One 16-bit CIA timer (A or B), with the 6526's pipeline delays.
//!
//! Measured against VICE with a test program (`tests/cia_timing.rs`):
//! after a write that starts a stopped timer, the first decrement comes
//! three cycles later; after a write that stops it, it still counts for
//! three more cycles; a force-load lands two cycles after the write, and
//! the cycle after the load doesn't count. An underflow reloads the latch,
//! so the period is latch + 1.

/// Control register bits shared by CRA and CRB.
pub(super) const CR_START: u8 = 0x01;
pub(super) const CR_ONE_SHOT: u8 = 0x08;
pub(super) const CR_FORCE_LOAD: u8 = 0x10;

#[derive(Debug, Clone, Default)]
pub(super) struct Timer {
    counter: u16,
    latch: u16,
    /// CRA/CRB as stored (force load is a strobe and is not stored).
    control: u8,
    /// Cycles left before a freshly started timer counts.
    skip: u8,
    /// Cycles a stopped timer keeps counting.
    run_on: u8,
    /// Cycles until a pending force-load happens.
    load_in: u8,
}

impl Timer {
    pub fn counter(&self) -> u16 {
        self.counter
    }

    pub fn control(&self) -> u8 {
        self.control
    }

    pub fn write_latch_lo(&mut self, val: u8) {
        self.latch = (self.latch & 0xFF00) | val as u16;
    }

    /// Writing the high byte of a stopped timer also loads the counter, so
    /// software can set a period before starting it.
    pub fn write_latch_hi(&mut self, val: u8) {
        self.latch = (self.latch & 0x00FF) | ((val as u16) << 8);
        if self.control & CR_START == 0 {
            self.counter = self.latch;
        }
    }

    pub fn write_control(&mut self, val: u8) {
        let was_running = self.control & CR_START != 0;
        self.control = val & !CR_FORCE_LOAD;
        let running = val & CR_START != 0;
        if running && !was_running {
            self.skip = 2;
            self.run_on = 0;
        } else if !running && was_running {
            self.run_on = 3;
        }
        if val & CR_FORCE_LOAD != 0 {
            self.load_in = 2;
        }
    }

    /// One cycle; `gate` holds the count (timer B waiting for timer A
    /// underflows). Returns true on underflow.
    pub fn clock(&mut self, gate: bool) -> bool {
        let started = self.control & CR_START != 0;
        let counting = (started && self.skip == 0) || self.run_on > 0;
        if self.skip > 0 {
            self.skip -= 1;
        }
        if self.run_on > 0 {
            self.run_on -= 1;
        }
        if self.load_in > 0 {
            self.load_in -= 1;
            if self.load_in == 0 {
                self.counter = self.latch;
                self.skip = self.skip.max(1);
                return false;
            }
        }
        if !counting || gate {
            return false;
        }
        if self.counter == 0 {
            self.counter = self.latch;
            if self.control & CR_ONE_SHOT != 0 {
                self.control &= !CR_START;
                self.run_on = 0;
            }
            true
        } else {
            self.counter -= 1;
            false
        }
    }
}
