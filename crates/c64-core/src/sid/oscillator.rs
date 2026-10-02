// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! A voice's oscillator: the 24-bit phase accumulator, the noise LFSR and
//! the four waveforms (after reSID).

/// Control register bits used by the oscillator.
pub(super) const CTRL_SYNC: u8 = 0x02;
pub(super) const CTRL_RING: u8 = 0x04;
pub(super) const CTRL_TEST: u8 = 0x08;

const LFSR_INIT: u32 = 0x7F_FFF8;

#[derive(Clone, Debug)]
pub(super) struct Oscillator {
    pub freq: u16,
    /// 12-bit pulse width.
    pub pw: u16,
    pub acc: u32,
    /// The accumulator's MSB rose in the last cycle (drives hard sync).
    pub msb_rising: bool,
    lfsr: u32,
}

impl Oscillator {
    pub fn new() -> Self {
        Oscillator { freq: 0, pw: 0, acc: 0, msb_rising: false, lfsr: LFSR_INIT }
    }

    /// TEST bit set: accumulator held at zero, noise LFSR reset.
    pub fn test_reset(&mut self) {
        self.acc = 0;
        self.lfsr = LFSR_INIT;
    }

    /// One cycle of the phase accumulator. Hard sync is applied by the
    /// chip afterwards, once every voice's `msb_rising` is known.
    #[inline]
    pub fn clock(&mut self, control: u8) {
        if control & CTRL_TEST != 0 {
            self.msb_rising = false;
            return;
        }
        let prev = self.acc;
        self.acc = (self.acc + self.freq as u32) & 0xFF_FFFF;
        self.msb_rising = prev & 0x80_0000 == 0 && self.acc & 0x80_0000 != 0;
        // The noise LFSR shifts on a rising edge of accumulator bit 19.
        if prev & 0x08_0000 == 0 && self.acc & 0x08_0000 != 0 {
            let bit0 = ((self.lfsr >> 22) ^ (self.lfsr >> 17)) & 1;
            self.lfsr = ((self.lfsr << 1) | bit0) & 0x7F_FFFF;
        }
    }

    /// 12-bit waveform output for the waveform bits in `control`.
    /// `ring_msb` is the previous voice's accumulator MSB (ring
    /// modulation). Combined waveforms are ANDed (a simplification).
    #[inline]
    pub fn waveform(&self, control: u8, ring_msb: bool) -> u16 {
        let wave = control >> 4;
        if wave == 0 {
            return 0;
        }
        let acc = self.acc;
        let mut out: u16 = 0xFFF;
        if wave & 0x1 != 0 {
            // triangle: the accumulator folded by its MSB
            let mut msb = acc & 0x80_0000 != 0;
            if control & CTRL_RING != 0 {
                msb ^= ring_msb;
            }
            let folded = if msb { !acc } else { acc };
            out &= ((folded >> 11) & 0xFFF) as u16;
        }
        if wave & 0x2 != 0 {
            // sawtooth
            out &= (acc >> 12) as u16;
        }
        if wave & 0x4 != 0 {
            // pulse
            let high = control & CTRL_TEST != 0 || (acc >> 12) as u16 >= self.pw;
            out &= if high { 0xFFF } else { 0 };
        }
        if wave & 0x8 != 0 {
            // noise: eight LFSR bits
            let r = self.lfsr;
            let n = ((r >> 9) & 0x800)
                | ((r >> 8) & 0x400)
                | ((r >> 5) & 0x200)
                | ((r >> 3) & 0x100)
                | ((r >> 2) & 0x080)
                | ((r << 1) & 0x040)
                | ((r << 3) & 0x020)
                | ((r << 4) & 0x010);
            out &= n as u16;
        }
        out
    }
}
