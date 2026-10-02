// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The MOS 6526 CIA (Complex Interface Adapter): two 8-bit ports, two
//! 16-bit timers and an interrupt control register, mirrored every 16
//! bytes of its I/O window. The C64 has two: CIA1 (keyboard, joysticks,
//! IRQ) and CIA2 (serial bus, VIC-II bank, NMI).
//!
//! Not modelled: the time-of-day clock and the serial shift register
//! (their registers just store what is written), counting CNT pulses
//! (approximated as counting cycles) and timer output on PB6/PB7.

mod timer;

use crate::port::Port;
use timer::Timer;

/// Interrupt sources in the ICR.
pub const ICR_TIMER_A: u8 = 0x01;
pub const ICR_TIMER_B: u8 = 0x02;

#[derive(Debug, Clone, Default)]
pub struct Cia6526 {
    pub port_a: Port,
    pub port_b: Port,
    timer_a: Timer,
    timer_b: Timer,
    /// Sources allowed to assert the interrupt output.
    icr_mask: u8,
    /// Sources that fired and haven't been acknowledged (bit 7 is
    /// computed on read).
    icr_pending: u8,
    /// Time-of-day and serial registers ($8-$C): stored, not running.
    tod_sdr: [u8; 5],
}

impl Cia6526 {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register read (`reg` 0-15). Reading the ICR acknowledges it.
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x0F {
            0x0 => self.port_a.pins(),
            0x1 => self.port_b.pins(),
            0x2 => self.port_a.ddr,
            0x3 => self.port_b.ddr,
            0x4 => self.timer_a.counter() as u8,
            0x5 => (self.timer_a.counter() >> 8) as u8,
            0x6 => self.timer_b.counter() as u8,
            0x7 => (self.timer_b.counter() >> 8) as u8,
            0xD => {
                let fired = self.icr_pending & self.icr_mask != 0;
                let v = self.icr_pending | if fired { 0x80 } else { 0 };
                self.icr_pending = 0;
                v
            }
            0xE => self.timer_a.control(),
            0xF => self.timer_b.control(),
            r => self.tod_sdr[(r - 8) as usize],
        }
    }

    pub fn write(&mut self, reg: u8, val: u8) {
        match reg & 0x0F {
            0x0 => self.port_a.data = val,
            0x1 => self.port_b.data = val,
            0x2 => self.port_a.ddr = val,
            0x3 => self.port_b.ddr = val,
            0x4 => self.timer_a.write_latch_lo(val),
            0x5 => self.timer_a.write_latch_hi(val),
            0x6 => self.timer_b.write_latch_lo(val),
            0x7 => self.timer_b.write_latch_hi(val),
            0xD => {
                // Bit 7 selects whether the named bits are set or cleared.
                if val & 0x80 != 0 {
                    self.icr_mask |= val & 0x1F;
                } else {
                    self.icr_mask &= !(val & 0x1F);
                }
            }
            0xE => self.timer_a.write_control(val),
            0xF => self.timer_b.write_control(val),
            r => self.tod_sdr[(r - 8) as usize] = val,
        }
    }

    /// One phi2 cycle.
    pub fn clock(&mut self) {
        let a_underflow = self.timer_a.clock(false);
        if a_underflow {
            self.icr_pending |= ICR_TIMER_A;
        }
        // CRB INMODE 2: timer B counts timer A underflows.
        let inmode = (self.timer_b.control() >> 5) & 0x03;
        let gate = inmode == 2 && !a_underflow;
        if self.timer_b.clock(gate) {
            self.icr_pending |= ICR_TIMER_B;
        }
    }

    /// The interrupt output (/IRQ): CIA1 drives the CPU's IRQ, CIA2 its NMI.
    #[inline]
    pub fn interrupt(&self) -> bool {
        self.icr_pending & self.icr_mask != 0
    }

    /// Live timer counters, for diagnostics.
    pub fn timers(&self) -> (u16, u16) {
        (self.timer_a.counter(), self.timer_b.counter())
    }
}
