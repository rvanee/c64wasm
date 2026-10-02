// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Operation semantics: everything that isn't addressing-mode bus mechanics
//! lives here. Functions that don't need to fold their result into a
//! register (pure "old byte -> new byte, set some flags" transforms) are
//! free functions of the shape `fn(&mut Mos6502, u8) -> u8` or `fn(&mut Mos6502,
//! u8)`, so they can be handed straight to `Mos6502::rmw` as a callback. A few
//! (ADC/SBC and the illegal opcodes that build on them, plus the "unstable"
//! high-byte-AND family that need bus access) are inherent `Mos6502` methods
//! instead.

use super::mos6502::{FLAG_C, FLAG_D, FLAG_N, FLAG_V, FLAG_Z};
use super::{Bus, Mos6502};

/// The constant OR'd into A before the AND in the ANE/LXA "unstable"
/// opcodes. This is genuinely chip- (and even temperature-) dependent on
/// real hardware; 0xEE is the value most commonly cited as representative
/// and is what we start from. If validation against a specific reference
/// dataset shows a different constant fits better, change it here -- it's
/// the only thing about these two opcodes worth tuning.
const UNSTABLE_MAGIC: u8 = 0xEE;

// ---- simple accumulator logic ops -------------------------------------

pub(super) fn ora(cpu: &mut Mos6502, v: u8) {
    cpu.a |= v;
    cpu.set_nz(cpu.a);
}

pub(super) fn and(cpu: &mut Mos6502, v: u8) {
    cpu.a &= v;
    cpu.set_nz(cpu.a);
}

pub(super) fn eor(cpu: &mut Mos6502, v: u8) {
    cpu.a ^= v;
    cpu.set_nz(cpu.a);
}

pub(super) fn bit(cpu: &mut Mos6502, v: u8) {
    cpu.set_flag(FLAG_Z, (cpu.a & v) == 0);
    cpu.set_flag(FLAG_N, v & 0x80 != 0);
    cpu.set_flag(FLAG_V, v & 0x40 != 0);
}

pub(super) fn cmp(cpu: &mut Mos6502, reg: u8, v: u8) {
    let result = reg.wrapping_sub(v);
    cpu.set_flag(FLAG_C, reg >= v);
    cpu.set_flag(FLAG_Z, reg == v);
    cpu.set_flag(FLAG_N, result & 0x80 != 0);
}

// ---- read-modify-write primitives (value -> value) --------------------

pub(super) fn asl(cpu: &mut Mos6502, v: u8) -> u8 {
    cpu.set_flag(FLAG_C, v & 0x80 != 0);
    let r = v << 1;
    cpu.set_nz(r);
    r
}

pub(super) fn lsr(cpu: &mut Mos6502, v: u8) -> u8 {
    cpu.set_flag(FLAG_C, v & 0x01 != 0);
    let r = v >> 1;
    cpu.set_nz(r);
    r
}

pub(super) fn rol(cpu: &mut Mos6502, v: u8) -> u8 {
    let c_in: u8 = cpu.flag(FLAG_C) as u8;
    cpu.set_flag(FLAG_C, v & 0x80 != 0);
    let r = (v << 1) | c_in;
    cpu.set_nz(r);
    r
}

pub(super) fn ror(cpu: &mut Mos6502, v: u8) -> u8 {
    let c_in: u8 = cpu.flag(FLAG_C) as u8;
    cpu.set_flag(FLAG_C, v & 0x01 != 0);
    let r = (v >> 1) | (c_in << 7);
    cpu.set_nz(r);
    r
}

pub(super) fn inc(cpu: &mut Mos6502, v: u8) -> u8 {
    let r = v.wrapping_add(1);
    cpu.set_nz(r);
    r
}

pub(super) fn dec(cpu: &mut Mos6502, v: u8) -> u8 {
    let r = v.wrapping_sub(1);
    cpu.set_nz(r);
    r
}

// ---- illegal RMW-plus-logic combos --------------------------------------
// Each performs its shift/rotate (setting C from THAT operation), then
// folds the shifted/rotated value into A via the paired logic op (setting
// N/Z from the final A) -- exactly what the two fused 6502 micro-ops do on
// real silicon.

pub(super) fn slo(cpu: &mut Mos6502, v: u8) -> u8 {
    cpu.set_flag(FLAG_C, v & 0x80 != 0);
    let shifted = v << 1;
    cpu.a |= shifted;
    cpu.set_nz(cpu.a);
    shifted
}

pub(super) fn rla(cpu: &mut Mos6502, v: u8) -> u8 {
    let c_in: u8 = cpu.flag(FLAG_C) as u8;
    cpu.set_flag(FLAG_C, v & 0x80 != 0);
    let rotated = (v << 1) | c_in;
    cpu.a &= rotated;
    cpu.set_nz(cpu.a);
    rotated
}

pub(super) fn sre(cpu: &mut Mos6502, v: u8) -> u8 {
    cpu.set_flag(FLAG_C, v & 0x01 != 0);
    let shifted = v >> 1;
    cpu.a ^= shifted;
    cpu.set_nz(cpu.a);
    shifted
}

// ---- immediate-mode illegal ops ----------------------------------------

pub(super) fn anc(cpu: &mut Mos6502, v: u8) {
    cpu.a &= v;
    cpu.set_nz(cpu.a);
    cpu.set_flag(FLAG_C, cpu.a & 0x80 != 0);
}

pub(super) fn alr(cpu: &mut Mos6502, v: u8) {
    cpu.a &= v;
    let c = cpu.a & 0x01 != 0;
    cpu.a >>= 1;
    cpu.set_flag(FLAG_C, c);
    cpu.set_nz(cpu.a);
}

impl Mos6502 {
    // ---- decimal-mode-aware arithmetic ---------------------------------
    // NMOS 6502/6510 quirk (see Bruce Clark's "6502 Decimal Mode"): in BCD
    // (D=1) mode, ADC's N/V flags come from an intermediate result computed
    // before the high-nibble decimal correction, and Z comes from a *plain
    // binary* add -- only C and the stored accumulator value reflect the
    // full decimal correction. SBC is the opposite: N/V/Z/C always match a
    // plain binary subtraction; only the stored value is decimal-corrected.

    pub(super) fn adc(&mut self, v: u8) {
        let c_in: i32 = self.flag(FLAG_C) as i32;
        let a = self.a as i32;
        let m = v as i32;

        if self.flag(FLAG_D) {
            let mut al = (a & 0x0F) + (m & 0x0F) + c_in;
            if al > 0x09 {
                al += 0x06;
            }
            let carry_from_lo = if al > 0x0F { 1 } else { 0 };
            let mut ah = (a >> 4) + (m >> 4) + carry_from_lo;

            // Intermediate result before the final ">9 => +6" correction on
            // the high nibble -- this is what N/V are actually based on.
            let pre = (((ah & 0x0F) << 4) | (al & 0x0F)) as u8;
            self.set_flag(FLAG_N, pre & 0x80 != 0);
            self.set_flag(FLAG_V, (self.a ^ pre) & (v ^ pre) & 0x80 != 0);

            let bin_sum = (a + m + c_in) & 0xFF;
            self.set_flag(FLAG_Z, bin_sum == 0);

            if ah > 0x09 {
                ah += 0x06;
            }
            self.set_flag(FLAG_C, ah > 0x0F);
            self.a = (((ah & 0x0F) << 4) | (al & 0x0F)) as u8;
        } else {
            let sum = a + m + c_in;
            let result = (sum & 0xFF) as u8;
            self.set_flag(FLAG_C, sum > 0xFF);
            self.set_flag(FLAG_V, (self.a ^ result) & (v ^ result) & 0x80 != 0);
            self.a = result;
            self.set_nz(result);
        }
    }

    pub(super) fn sbc(&mut self, v: u8) {
        let c_in: i32 = self.flag(FLAG_C) as i32;
        let a_u8 = self.a;
        let a = a_u8 as i32;
        let m = v as i32;

        // Flags always match a plain binary subtraction, decimal mode or not.
        let diff = a - m - (1 - c_in);
        let result = (diff & 0xFF) as u8;
        self.set_flag(FLAG_C, diff >= 0);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, result & 0x80 != 0);
        self.set_flag(FLAG_V, (a_u8 ^ v) & (a_u8 ^ result) & 0x80 != 0);

        if self.flag(FLAG_D) {
            let mut al = (a & 0x0F) - (m & 0x0F) + c_in - 1;
            if al < 0 {
                al = ((al - 0x06) & 0x0F) - 0x10;
            }
            let mut full = (a & 0xF0) - (m & 0xF0) + al;
            if full < 0 {
                full -= 0x60;
            }
            self.a = (full & 0xFF) as u8;
        } else {
            self.a = result;
        }
    }

    // ---- illegal opcodes that need CPU methods (adc/sbc, or bus access) --

    /// ARR: AND #imm, then ROR A -- but on NMOS hardware in decimal mode the
    /// familiar per-nibble BCD correction is applied to the *stored* value
    /// only, after N/Z/C/V have already been latched from the plain
    /// (non-decimal) AND-then-ROR result. Verified against
    /// SingleStepTests/65x02's `6b.json`.
    pub(super) fn arr(&mut self, v: u8) {
        let t = self.a & v;
        let c_in = self.flag(FLAG_C) as u8;
        let mut result = (t >> 1) | (c_in << 7);

        self.set_nz(result);
        self.set_flag(FLAG_C, result & 0x40 != 0);
        self.set_flag(FLAG_V, ((result >> 6) ^ (result >> 5)) & 0x01 != 0);

        if self.flag(FLAG_D) {
            // NB: do this arithmetic in u16 -- `(t & 0xF0) + (t & 0x10)` can
            // reach 0x100 and silently wraps if left as u8.
            let t16 = t as u16;
            if (t16 & 0x0F) + (t16 & 0x01) > 0x05 {
                result = (result & 0xF0) | ((result.wrapping_add(0x06)) & 0x0F);
            }
            if (t16 & 0xF0) + (t16 & 0x10) > 0x50 {
                result = result.wrapping_add(0x60);
                self.set_flag(FLAG_C, true);
            }
        }
        self.a = result;
    }

    pub(super) fn sbx(&mut self, v: u8) {
        let t = (self.a & self.x) as i32;
        let m = v as i32;
        let diff = t - m;
        self.set_flag(FLAG_C, diff >= 0);
        let result = (diff & 0xFF) as u8;
        self.x = result;
        self.set_nz(result);
    }

    pub(super) fn ane(&mut self, v: u8) {
        self.a = (self.a | UNSTABLE_MAGIC) & self.x & v;
        self.set_nz(self.a);
    }

    pub(super) fn lxa(&mut self, v: u8) {
        self.a = (self.a | UNSTABLE_MAGIC) & v;
        self.x = self.a;
        self.set_nz(self.a);
    }

    pub(super) fn las(&mut self, v: u8) {
        let r = v & self.s;
        self.a = r;
        self.x = r;
        self.s = r;
        self.set_nz(r);
    }

    /// SHA/AHX, SHX, SHY and TAS/SHS are the "high-byte-AND" family of
    /// unstable store opcodes: the value stored is reg(s) & (high byte of
    /// the effective address + 1), which is itself a documented real-chip
    /// approximation. But there's a second, independently-verified quirk
    /// (SingleStepTests/65x02 `93.json`/`9b.json`/`9c.json`/`9e.json`): when
    /// the index addition actually carries into the high byte, the *address
    /// bus itself* gets corrupted too -- the high byte that's really driven
    /// during the write is replaced by the stored value, not the
    /// mathematically correct effective address. `crossed` tells us whether
    /// that happened.
    fn unstable_store_addr(addr: u16, value: u8, crossed: bool) -> u16 {
        if crossed {
            ((value as u16) << 8) | (addr & 0x00FF)
        } else {
            addr
        }
    }

    /// The AND mask is always (pre-index base address high byte) + 1. When
    /// the index addition didn't carry, that's just (effective address high
    /// byte) + 1; when it did carry, the base's high byte is one less than
    /// the effective address's, so the "+1" cancels back out to the
    /// effective address's high byte unchanged. Verified against
    /// SingleStepTests/65x02.
    fn unstable_and_mask(addr: u16, crossed: bool) -> u8 {
        let hi = (addr >> 8) as u8;
        if crossed {
            hi
        } else {
            hi.wrapping_add(1)
        }
    }

    pub(super) fn sha(&mut self, bus: &mut impl Bus, addr: u16, crossed: bool) {
        let hi = Self::unstable_and_mask(addr, crossed);
        let v = self.a & self.x & hi;
        let real_addr = Self::unstable_store_addr(addr, v, crossed);
        self.wr(bus, real_addr, v);
    }

    pub(super) fn shx(&mut self, bus: &mut impl Bus, addr: u16, crossed: bool) {
        let hi = Self::unstable_and_mask(addr, crossed);
        let v = self.x & hi;
        let real_addr = Self::unstable_store_addr(addr, v, crossed);
        self.wr(bus, real_addr, v);
    }

    pub(super) fn shy(&mut self, bus: &mut impl Bus, addr: u16, crossed: bool) {
        let hi = Self::unstable_and_mask(addr, crossed);
        let v = self.y & hi;
        let real_addr = Self::unstable_store_addr(addr, v, crossed);
        self.wr(bus, real_addr, v);
    }

    pub(super) fn tas(&mut self, bus: &mut impl Bus, addr: u16, crossed: bool) {
        self.s = self.a & self.x;
        let hi = Self::unstable_and_mask(addr, crossed);
        let v = self.s & hi;
        let real_addr = Self::unstable_store_addr(addr, v, crossed);
        self.wr(bus, real_addr, v);
    }

    // ---- illegal RMW combos that need a Mos6502 method (fold into ADC/SBC/CMP) -

    pub(super) fn rra(cpu: &mut Mos6502, v: u8) -> u8 {
        let c_old = cpu.flag(FLAG_C) as u8;
        let c_new = v & 0x01 != 0;
        let rotated = (v >> 1) | (c_old << 7);
        cpu.set_flag(FLAG_C, c_new);
        cpu.adc(rotated);
        rotated
    }

    pub(super) fn dcp(cpu: &mut Mos6502, v: u8) -> u8 {
        let result = v.wrapping_sub(1);
        let a = cpu.a;
        cmp(cpu, a, result);
        result
    }

    pub(super) fn isc(cpu: &mut Mos6502, v: u8) -> u8 {
        let result = v.wrapping_add(1);
        cpu.sbc(result);
        result
    }
}
