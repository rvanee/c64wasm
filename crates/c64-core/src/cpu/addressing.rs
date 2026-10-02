// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Addressing modes: the bus-access sequences that compute an operand
//! address, including the dummy reads of page-crossing fix-ups.

use super::{Bus, Mos6502};

impl Mos6502 {
    // Each returns the effective address (or reads the value directly for
    // immediate/accumulator), performing exactly the bus transactions real
    // hardware performs for that category, in order.

    #[inline]
    pub(super) fn addr_zp(&mut self, bus: &mut impl Bus) -> u16 {
        self.fetch(bus) as u16
    }

    #[inline]
    pub(super) fn addr_zp_indexed(&mut self, bus: &mut impl Bus, index: u8) -> u16 {
        let base = self.fetch(bus);
        let _ = self.rd(bus, base as u16); // dummy read before the index is applied
        base.wrapping_add(index) as u16
    }

    #[inline]
    pub(super) fn addr_abs(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        ((hi as u16) << 8) | lo as u16
    }

    /// Absolute,X / Absolute,Y for *read* instructions: only pay the extra
    /// cycle if the index causes a page crossing.
    #[inline]
    pub(super) fn addr_abs_indexed_read(&mut self, bus: &mut impl Bus, index: u8) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        let base = ((hi as u16) << 8) | lo as u16;
        let addr = base.wrapping_add(index as u16);
        if (addr & 0xFF00) != (base & 0xFF00) {
            let wrong = (base & 0xFF00) | (addr & 0x00FF);
            let _ = self.rd(bus, wrong);
        }
        addr
    }

    /// Absolute,X / Absolute,Y for *write/RMW* instructions: the fix-up
    /// cycle always happens, whether or not a page was actually crossed.
    #[inline]
    pub(super) fn addr_abs_indexed_rw(&mut self, bus: &mut impl Bus, index: u8) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        let base = ((hi as u16) << 8) | lo as u16;
        let addr = base.wrapping_add(index as u16);
        let wrong = (base & 0xFF00) | (addr & 0x00FF);
        let _ = self.rd(bus, wrong);
        addr
    }

    /// Same bus pattern as `addr_abs_indexed_rw`, but also reports whether a
    /// page boundary was actually crossed. Needed only by the "unstable"
    /// SHA/SHX/SHY/TAS family, whose *target address* (not just the stored
    /// value) gets corrupted by a real bus-conflict quirk when the index
    /// addition carries into the high byte.
    #[inline]
    pub(super) fn addr_abs_indexed_rw_c(&mut self, bus: &mut impl Bus, index: u8) -> (u16, bool) {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        let base = ((hi as u16) << 8) | lo as u16;
        let addr = base.wrapping_add(index as u16);
        let wrong = (base & 0xFF00) | (addr & 0x00FF);
        let _ = self.rd(bus, wrong);
        (addr, (addr & 0xFF00) != (base & 0xFF00))
    }

    /// As `addr_indirect_indexed_rw`, but also reports whether the `+Y` step
    /// crossed a page (see `addr_abs_indexed_rw_c`).
    #[inline]
    pub(super) fn addr_indirect_indexed_rw_c(&mut self, bus: &mut impl Bus) -> (u16, bool) {
        let zp = self.fetch(bus);
        let lo = self.rd(bus, zp as u16);
        let hi = self.rd(bus, zp.wrapping_add(1) as u16);
        let base = ((hi as u16) << 8) | lo as u16;
        let addr = base.wrapping_add(self.y as u16);
        let wrong = (base & 0xFF00) | (addr & 0x00FF);
        let _ = self.rd(bus, wrong);
        (addr, (addr & 0xFF00) != (base & 0xFF00))
    }

    #[inline]
    pub(super) fn addr_indexed_indirect(&mut self, bus: &mut impl Bus) -> u16 {
        // (zp,X)
        let zp = self.fetch(bus);
        let _ = self.rd(bus, zp as u16); // dummy, pre-index
        let ptr = zp.wrapping_add(self.x);
        let lo = self.rd(bus, ptr as u16);
        let hi = self.rd(bus, ptr.wrapping_add(1) as u16);
        ((hi as u16) << 8) | lo as u16
    }

    #[inline]
    pub(super) fn addr_indirect_indexed_read(&mut self, bus: &mut impl Bus) -> u16 {
        // (zp),Y, read variant: conditional extra cycle
        let zp = self.fetch(bus);
        let lo = self.rd(bus, zp as u16);
        let hi = self.rd(bus, zp.wrapping_add(1) as u16);
        let base = ((hi as u16) << 8) | lo as u16;
        let addr = base.wrapping_add(self.y as u16);
        if (addr & 0xFF00) != (base & 0xFF00) {
            let wrong = (base & 0xFF00) | (addr & 0x00FF);
            let _ = self.rd(bus, wrong);
        }
        addr
    }

    #[inline]
    pub(super) fn addr_indirect_indexed_rw(&mut self, bus: &mut impl Bus) -> u16 {
        // (zp),Y, write/RMW variant: unconditional extra cycle
        let zp = self.fetch(bus);
        let lo = self.rd(bus, zp as u16);
        let hi = self.rd(bus, zp.wrapping_add(1) as u16);
        let base = ((hi as u16) << 8) | lo as u16;
        let addr = base.wrapping_add(self.y as u16);
        let wrong = (base & 0xFF00) | (addr & 0x00FF);
        let _ = self.rd(bus, wrong);
        addr
    }
}
