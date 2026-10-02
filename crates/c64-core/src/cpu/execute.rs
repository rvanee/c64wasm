// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Instruction decoding: one `match` arm per opcode (all 256, including
//! the undocumented ones), plus the instructions with their own bus
//! sequences (branches, jumps, stack operations, BRK, JAM).

use super::mos6502::{FLAG_B, FLAG_C, FLAG_D, FLAG_I, FLAG_N, FLAG_U, FLAG_V, FLAG_Z};
use super::{Bus, Mos6502};

impl Mos6502 {
    #[rustfmt::skip]
    pub(super) fn execute(&mut self, bus: &mut impl Bus, opcode: u8) {
        use super::ops::*;
        match opcode {
            // ---------------------------------------------------- 0x0_
            0x00 => self.brk(bus),
            0x01 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); ora(self, v); }
            0x02 => self.jam(bus),
            0x03 => { let a = self.addr_indexed_indirect(bus); self.rmw(bus, a, slo); }
            0x04 => { let a = self.addr_zp(bus); let _ = self.rd(bus, a); }
            0x05 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); ora(self, v); }
            0x06 => { let a = self.addr_zp(bus); self.rmw(bus, a, asl); }
            0x07 => { let a = self.addr_zp(bus); self.rmw(bus, a, slo); }
            0x08 => self.php(bus),
            0x09 => { let v = self.fetch(bus); ora(self, v); }
            0x0A => { self.implied_filler(bus); self.a = asl(self, self.a); }
            0x0B => { let v = self.fetch(bus); anc(self, v); }
            0x0C => { let a = self.addr_abs(bus); let _ = self.rd(bus, a); }
            0x0D => { let a = self.addr_abs(bus); let v = self.rd(bus, a); ora(self, v); }
            0x0E => { let a = self.addr_abs(bus); self.rmw(bus, a, asl); }
            0x0F => { let a = self.addr_abs(bus); self.rmw(bus, a, slo); }

            // ---------------------------------------------------- 0x1_
            0x10 => self.branch(bus, !self.flag(FLAG_N)),
            0x11 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); ora(self, v); }
            0x12 => self.jam(bus),
            0x13 => { let a = self.addr_indirect_indexed_rw(bus); self.rmw(bus, a, slo); }
            0x14 => { let a = self.addr_zp_indexed(bus, self.x); let _ = self.rd(bus, a); }
            0x15 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); ora(self, v); }
            0x16 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, asl); }
            0x17 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, slo); }
            0x18 => { self.implied_filler(bus); self.set_flag(FLAG_C, false); }
            0x19 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); ora(self, v); }
            0x1A => { self.implied_filler(bus); }
            0x1B => { let a = self.addr_abs_indexed_rw(bus, self.y); self.rmw(bus, a, slo); }
            0x1C => { let a = self.addr_abs_indexed_read(bus, self.x); let _ = self.rd(bus, a); }
            0x1D => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); ora(self, v); }
            0x1E => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, asl); }
            0x1F => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, slo); }

            // ---------------------------------------------------- 0x2_
            0x20 => self.jsr(bus),
            0x21 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); and(self, v); }
            0x22 => self.jam(bus),
            0x23 => { let a = self.addr_indexed_indirect(bus); self.rmw(bus, a, rla); }
            0x24 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); bit(self, v); }
            0x25 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); and(self, v); }
            0x26 => { let a = self.addr_zp(bus); self.rmw(bus, a, rol); }
            0x27 => { let a = self.addr_zp(bus); self.rmw(bus, a, rla); }
            0x28 => self.plp(bus),
            0x29 => { let v = self.fetch(bus); and(self, v); }
            0x2A => { self.implied_filler(bus); self.a = rol(self, self.a); }
            0x2B => { let v = self.fetch(bus); anc(self, v); }
            0x2C => { let a = self.addr_abs(bus); let v = self.rd(bus, a); bit(self, v); }
            0x2D => { let a = self.addr_abs(bus); let v = self.rd(bus, a); and(self, v); }
            0x2E => { let a = self.addr_abs(bus); self.rmw(bus, a, rol); }
            0x2F => { let a = self.addr_abs(bus); self.rmw(bus, a, rla); }

            // ---------------------------------------------------- 0x3_
            0x30 => self.branch(bus, self.flag(FLAG_N)),
            0x31 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); and(self, v); }
            0x32 => self.jam(bus),
            0x33 => { let a = self.addr_indirect_indexed_rw(bus); self.rmw(bus, a, rla); }
            0x34 => { let a = self.addr_zp_indexed(bus, self.x); let _ = self.rd(bus, a); }
            0x35 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); and(self, v); }
            0x36 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, rol); }
            0x37 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, rla); }
            0x38 => { self.implied_filler(bus); self.set_flag(FLAG_C, true); }
            0x39 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); and(self, v); }
            0x3A => { self.implied_filler(bus); }
            0x3B => { let a = self.addr_abs_indexed_rw(bus, self.y); self.rmw(bus, a, rla); }
            0x3C => { let a = self.addr_abs_indexed_read(bus, self.x); let _ = self.rd(bus, a); }
            0x3D => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); and(self, v); }
            0x3E => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, rol); }
            0x3F => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, rla); }

            // ---------------------------------------------------- 0x4_
            0x40 => self.rti(bus),
            0x41 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); eor(self, v); }
            0x42 => self.jam(bus),
            0x43 => { let a = self.addr_indexed_indirect(bus); self.rmw(bus, a, sre); }
            0x44 => { let a = self.addr_zp(bus); let _ = self.rd(bus, a); }
            0x45 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); eor(self, v); }
            0x46 => { let a = self.addr_zp(bus); self.rmw(bus, a, lsr); }
            0x47 => { let a = self.addr_zp(bus); self.rmw(bus, a, sre); }
            0x48 => self.pha(bus),
            0x49 => { let v = self.fetch(bus); eor(self, v); }
            0x4A => { self.implied_filler(bus); self.a = lsr(self, self.a); }
            0x4B => { let v = self.fetch(bus); alr(self, v); }
            0x4C => { self.pc = self.addr_abs(bus); }
            0x4D => { let a = self.addr_abs(bus); let v = self.rd(bus, a); eor(self, v); }
            0x4E => { let a = self.addr_abs(bus); self.rmw(bus, a, lsr); }
            0x4F => { let a = self.addr_abs(bus); self.rmw(bus, a, sre); }

            // ---------------------------------------------------- 0x5_
            0x50 => self.branch(bus, !self.flag(FLAG_V)),
            0x51 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); eor(self, v); }
            0x52 => self.jam(bus),
            0x53 => { let a = self.addr_indirect_indexed_rw(bus); self.rmw(bus, a, sre); }
            0x54 => { let a = self.addr_zp_indexed(bus, self.x); let _ = self.rd(bus, a); }
            0x55 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); eor(self, v); }
            0x56 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, lsr); }
            0x57 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, sre); }
            0x58 => { self.implied_filler(bus); self.set_flag(FLAG_I, false); }
            0x59 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); eor(self, v); }
            0x5A => { self.implied_filler(bus); }
            0x5B => { let a = self.addr_abs_indexed_rw(bus, self.y); self.rmw(bus, a, sre); }
            0x5C => { let a = self.addr_abs_indexed_read(bus, self.x); let _ = self.rd(bus, a); }
            0x5D => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); eor(self, v); }
            0x5E => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, lsr); }
            0x5F => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, sre); }

            // ---------------------------------------------------- 0x6_
            0x60 => self.rts(bus),
            0x61 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); self.adc(v); }
            0x62 => self.jam(bus),
            0x63 => { let a = self.addr_indexed_indirect(bus); self.rmw_cpu(bus, a, Self::rra); }
            0x64 => { let a = self.addr_zp(bus); let _ = self.rd(bus, a); }
            0x65 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); self.adc(v); }
            0x66 => { let a = self.addr_zp(bus); self.rmw(bus, a, ror); }
            0x67 => { let a = self.addr_zp(bus); self.rmw_cpu(bus, a, Self::rra); }
            0x68 => self.pla(bus),
            0x69 => { let v = self.fetch(bus); self.adc(v); }
            0x6A => { self.implied_filler(bus); self.a = ror(self, self.a); }
            0x6B => { let v = self.fetch(bus); self.arr(v); }
            0x6C => self.jmp_indirect(bus),
            0x6D => { let a = self.addr_abs(bus); let v = self.rd(bus, a); self.adc(v); }
            0x6E => { let a = self.addr_abs(bus); self.rmw(bus, a, ror); }
            0x6F => { let a = self.addr_abs(bus); self.rmw_cpu(bus, a, Self::rra); }

            // ---------------------------------------------------- 0x7_
            0x70 => self.branch(bus, self.flag(FLAG_V)),
            0x71 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); self.adc(v); }
            0x72 => self.jam(bus),
            0x73 => { let a = self.addr_indirect_indexed_rw(bus); self.rmw_cpu(bus, a, Self::rra); }
            0x74 => { let a = self.addr_zp_indexed(bus, self.x); let _ = self.rd(bus, a); }
            0x75 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); self.adc(v); }
            0x76 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, ror); }
            0x77 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw_cpu(bus, a, Self::rra); }
            0x78 => { self.implied_filler(bus); self.set_flag(FLAG_I, true); }
            0x79 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); self.adc(v); }
            0x7A => { self.implied_filler(bus); }
            0x7B => { let a = self.addr_abs_indexed_rw(bus, self.y); self.rmw_cpu(bus, a, Self::rra); }
            0x7C => { let a = self.addr_abs_indexed_read(bus, self.x); let _ = self.rd(bus, a); }
            0x7D => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); self.adc(v); }
            0x7E => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, ror); }
            0x7F => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw_cpu(bus, a, Self::rra); }

            // ---------------------------------------------------- 0x8_
            0x80 => { let _ = self.fetch(bus); }
            0x81 => { let a = self.addr_indexed_indirect(bus); let v = self.a; self.wr(bus, a, v); }
            0x82 => { let _ = self.fetch(bus); }
            0x83 => { let a = self.addr_indexed_indirect(bus); let v = self.a & self.x; self.wr(bus, a, v); }
            0x84 => { let a = self.addr_zp(bus); let v = self.y; self.wr(bus, a, v); }
            0x85 => { let a = self.addr_zp(bus); let v = self.a; self.wr(bus, a, v); }
            0x86 => { let a = self.addr_zp(bus); let v = self.x; self.wr(bus, a, v); }
            0x87 => { let a = self.addr_zp(bus); let v = self.a & self.x; self.wr(bus, a, v); }
            0x88 => { self.implied_filler(bus); self.y = self.y.wrapping_sub(1); self.set_nz(self.y); }
            0x89 => { let _ = self.fetch(bus); }
            0x8A => { self.implied_filler(bus); self.a = self.x; self.set_nz(self.a); }
            0x8B => { let v = self.fetch(bus); self.ane(v); }
            0x8C => { let a = self.addr_abs(bus); let v = self.y; self.wr(bus, a, v); }
            0x8D => { let a = self.addr_abs(bus); let v = self.a; self.wr(bus, a, v); }
            0x8E => { let a = self.addr_abs(bus); let v = self.x; self.wr(bus, a, v); }
            0x8F => { let a = self.addr_abs(bus); let v = self.a & self.x; self.wr(bus, a, v); }

            // ---------------------------------------------------- 0x9_
            0x90 => self.branch(bus, !self.flag(FLAG_C)),
            0x91 => { let a = self.addr_indirect_indexed_rw(bus); let v = self.a; self.wr(bus, a, v); }
            0x92 => self.jam(bus),
            0x93 => { let (a, c) = self.addr_indirect_indexed_rw_c(bus); self.sha(bus, a, c); }
            0x94 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.y; self.wr(bus, a, v); }
            0x95 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.a; self.wr(bus, a, v); }
            0x96 => { let a = self.addr_zp_indexed(bus, self.y); let v = self.x; self.wr(bus, a, v); }
            0x97 => { let a = self.addr_zp_indexed(bus, self.y); let v = self.a & self.x; self.wr(bus, a, v); }
            0x98 => { self.implied_filler(bus); self.a = self.y; self.set_nz(self.a); }
            0x99 => { let a = self.addr_abs_indexed_rw(bus, self.y); let v = self.a; self.wr(bus, a, v); }
            0x9A => { self.implied_filler(bus); self.s = self.x; }
            0x9B => { let (a, c) = self.addr_abs_indexed_rw_c(bus, self.y); self.tas(bus, a, c); }
            0x9C => { let (a, c) = self.addr_abs_indexed_rw_c(bus, self.x); self.shy(bus, a, c); }
            0x9D => { let a = self.addr_abs_indexed_rw(bus, self.x); let v = self.a; self.wr(bus, a, v); }
            0x9E => { let (a, c) = self.addr_abs_indexed_rw_c(bus, self.y); self.shx(bus, a, c); }
            0x9F => { let (a, c) = self.addr_abs_indexed_rw_c(bus, self.y); self.sha(bus, a, c); }

            // ---------------------------------------------------- 0xA_
            0xA0 => { let v = self.fetch(bus); self.y = v; self.set_nz(v); }
            0xA1 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xA2 => { let v = self.fetch(bus); self.x = v; self.set_nz(v); }
            0xA3 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); self.a = v; self.x = v; self.set_nz(v); }
            0xA4 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); self.y = v; self.set_nz(v); }
            0xA5 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xA6 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); self.x = v; self.set_nz(v); }
            0xA7 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); self.a = v; self.x = v; self.set_nz(v); }
            0xA8 => { self.implied_filler(bus); self.y = self.a; self.set_nz(self.y); }
            0xA9 => { let v = self.fetch(bus); self.a = v; self.set_nz(v); }
            0xAA => { self.implied_filler(bus); self.x = self.a; self.set_nz(self.x); }
            0xAB => { let v = self.fetch(bus); self.lxa(v); }
            0xAC => { let a = self.addr_abs(bus); let v = self.rd(bus, a); self.y = v; self.set_nz(v); }
            0xAD => { let a = self.addr_abs(bus); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xAE => { let a = self.addr_abs(bus); let v = self.rd(bus, a); self.x = v; self.set_nz(v); }
            0xAF => { let a = self.addr_abs(bus); let v = self.rd(bus, a); self.a = v; self.x = v; self.set_nz(v); }

            // ---------------------------------------------------- 0xB_
            0xB0 => self.branch(bus, self.flag(FLAG_C)),
            0xB1 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xB2 => self.jam(bus),
            0xB3 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); self.a = v; self.x = v; self.set_nz(v); }
            0xB4 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); self.y = v; self.set_nz(v); }
            0xB5 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xB6 => { let a = self.addr_zp_indexed(bus, self.y); let v = self.rd(bus, a); self.x = v; self.set_nz(v); }
            0xB7 => { let a = self.addr_zp_indexed(bus, self.y); let v = self.rd(bus, a); self.a = v; self.x = v; self.set_nz(v); }
            0xB8 => { self.implied_filler(bus); self.set_flag(FLAG_V, false); }
            0xB9 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xBA => { self.implied_filler(bus); self.x = self.s; self.set_nz(self.x); }
            0xBB => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); self.las(v); }
            0xBC => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); self.y = v; self.set_nz(v); }
            0xBD => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); self.a = v; self.set_nz(v); }
            0xBE => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); self.x = v; self.set_nz(v); }
            0xBF => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); self.a = v; self.x = v; self.set_nz(v); }

            // ---------------------------------------------------- 0xC_
            0xC0 => { let v = self.fetch(bus); cmp(self, self.y, v); }
            0xC1 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xC2 => { let _ = self.fetch(bus); }
            0xC3 => { let a = self.addr_indexed_indirect(bus); self.rmw_cpu(bus, a, Self::dcp); }
            0xC4 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); cmp(self, self.y, v); }
            0xC5 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xC6 => { let a = self.addr_zp(bus); self.rmw(bus, a, dec); }
            0xC7 => { let a = self.addr_zp(bus); self.rmw_cpu(bus, a, Self::dcp); }
            0xC8 => { self.implied_filler(bus); self.y = self.y.wrapping_add(1); self.set_nz(self.y); }
            0xC9 => { let v = self.fetch(bus); cmp(self, self.a, v); }
            0xCA => { self.implied_filler(bus); self.x = self.x.wrapping_sub(1); self.set_nz(self.x); }
            0xCB => { let v = self.fetch(bus); self.sbx(v); }
            0xCC => { let a = self.addr_abs(bus); let v = self.rd(bus, a); cmp(self, self.y, v); }
            0xCD => { let a = self.addr_abs(bus); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xCE => { let a = self.addr_abs(bus); self.rmw(bus, a, dec); }
            0xCF => { let a = self.addr_abs(bus); self.rmw_cpu(bus, a, Self::dcp); }

            // ---------------------------------------------------- 0xD_
            0xD0 => self.branch(bus, !self.flag(FLAG_Z)),
            0xD1 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xD2 => self.jam(bus),
            0xD3 => { let a = self.addr_indirect_indexed_rw(bus); self.rmw_cpu(bus, a, Self::dcp); }
            0xD4 => { let a = self.addr_zp_indexed(bus, self.x); let _ = self.rd(bus, a); }
            0xD5 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xD6 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, dec); }
            0xD7 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw_cpu(bus, a, Self::dcp); }
            0xD8 => { self.implied_filler(bus); self.set_flag(FLAG_D, false); }
            0xD9 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xDA => { self.implied_filler(bus); }
            0xDB => { let a = self.addr_abs_indexed_rw(bus, self.y); self.rmw_cpu(bus, a, Self::dcp); }
            0xDC => { let a = self.addr_abs_indexed_read(bus, self.x); let _ = self.rd(bus, a); }
            0xDD => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); cmp(self, self.a, v); }
            0xDE => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, dec); }
            0xDF => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw_cpu(bus, a, Self::dcp); }

            // ---------------------------------------------------- 0xE_
            0xE0 => { let v = self.fetch(bus); cmp(self, self.x, v); }
            0xE1 => { let a = self.addr_indexed_indirect(bus); let v = self.rd(bus, a); self.sbc(v); }
            0xE2 => { let _ = self.fetch(bus); }
            0xE3 => { let a = self.addr_indexed_indirect(bus); self.rmw_cpu(bus, a, Self::isc); }
            0xE4 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); cmp(self, self.x, v); }
            0xE5 => { let a = self.addr_zp(bus); let v = self.rd(bus, a); self.sbc(v); }
            0xE6 => { let a = self.addr_zp(bus); self.rmw(bus, a, inc); }
            0xE7 => { let a = self.addr_zp(bus); self.rmw_cpu(bus, a, Self::isc); }
            0xE8 => { self.implied_filler(bus); self.x = self.x.wrapping_add(1); self.set_nz(self.x); }
            0xE9 => { let v = self.fetch(bus); self.sbc(v); }
            0xEA => { self.implied_filler(bus); }
            0xEB => { let v = self.fetch(bus); self.sbc(v); }
            0xEC => { let a = self.addr_abs(bus); let v = self.rd(bus, a); cmp(self, self.x, v); }
            0xED => { let a = self.addr_abs(bus); let v = self.rd(bus, a); self.sbc(v); }
            0xEE => { let a = self.addr_abs(bus); self.rmw(bus, a, inc); }
            0xEF => { let a = self.addr_abs(bus); self.rmw_cpu(bus, a, Self::isc); }

            // ---------------------------------------------------- 0xF_
            0xF0 => self.branch(bus, self.flag(FLAG_Z)),
            0xF1 => { let a = self.addr_indirect_indexed_read(bus); let v = self.rd(bus, a); self.sbc(v); }
            0xF2 => self.jam(bus),
            0xF3 => { let a = self.addr_indirect_indexed_rw(bus); self.rmw_cpu(bus, a, Self::isc); }
            0xF4 => { let a = self.addr_zp_indexed(bus, self.x); let _ = self.rd(bus, a); }
            0xF5 => { let a = self.addr_zp_indexed(bus, self.x); let v = self.rd(bus, a); self.sbc(v); }
            0xF6 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw(bus, a, inc); }
            0xF7 => { let a = self.addr_zp_indexed(bus, self.x); self.rmw_cpu(bus, a, Self::isc); }
            0xF8 => { self.implied_filler(bus); self.set_flag(FLAG_D, true); }
            0xF9 => { let a = self.addr_abs_indexed_read(bus, self.y); let v = self.rd(bus, a); self.sbc(v); }
            0xFA => { self.implied_filler(bus); }
            0xFB => { let a = self.addr_abs_indexed_rw(bus, self.y); self.rmw_cpu(bus, a, Self::isc); }
            0xFC => { let a = self.addr_abs_indexed_read(bus, self.x); let _ = self.rd(bus, a); }
            0xFD => { let a = self.addr_abs_indexed_read(bus, self.x); let v = self.rd(bus, a); self.sbc(v); }
            0xFE => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw(bus, a, inc); }
            0xFF => { let a = self.addr_abs_indexed_rw(bus, self.x); self.rmw_cpu(bus, a, Self::isc); }
        }
    }

    /// Generic read-modify-write helper for ops that don't need extra CPU
    /// state beyond the value (ASL/LSR/ROL/ROR/INC/DEC and the illegal
    /// SLO/RLA/SRE combos, all of which are "pure" value -> value
    /// transforms plus flags). Performs read, dummy write-back of the old
    /// value, then write of the new value -- exactly what real RMW
    /// instructions do on the bus.
    fn rmw(&mut self, bus: &mut impl Bus, addr: u16, f: fn(&mut Mos6502, u8) -> u8) {
        let old = self.rd(bus, addr);
        self.wr(bus, addr, old);
        let new = f(self, old);
        self.wr(bus, addr, new);
    }

    /// Same bus pattern as `rmw`, but for ops that also need to fold the
    /// result into another register (DCP compares with A, ISC/RRA add into
    /// A) via a method on `Mos6502` rather than a free function.
    fn rmw_cpu(&mut self, bus: &mut impl Bus, addr: u16, f: fn(&mut Mos6502, u8) -> u8) {
        let old = self.rd(bus, addr);
        self.wr(bus, addr, old);
        let new = f(self, old);
        self.wr(bus, addr, new);
    }

    fn branch(&mut self, bus: &mut impl Bus, take: bool) {
        let offset = self.fetch(bus) as i8;
        if take {
            let _ = self.rd(bus, self.pc); // dummy fetch of the following opcode
            let old_pc = self.pc;
            let new_pc = old_pc.wrapping_add(offset as u16);
            if (new_pc & 0xFF00) != (old_pc & 0xFF00) {
                let wrong = (old_pc & 0xFF00) | (new_pc & 0x00FF);
                let _ = self.rd(bus, wrong);
            }
            self.pc = new_pc;
        }
    }

    fn jmp_indirect(&mut self, bus: &mut impl Bus) {
        let ptr_lo = self.fetch(bus);
        let ptr_hi = self.fetch(bus);
        let ptr = ((ptr_hi as u16) << 8) | ptr_lo as u16;
        let lo = self.rd(bus, ptr);
        // Famous 6502 bug: the high byte is fetched from (ptr & 0xFF00) |
        // ((ptr+1) & 0x00FF) -- it does NOT cross a page boundary. Faithful
        // emulation must reproduce this.
        let hi_addr = (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF);
        let hi = self.rd(bus, hi_addr);
        self.pc = ((hi as u16) << 8) | lo as u16;
    }

    fn jsr(&mut self, bus: &mut impl Bus) {
        let lo = self.fetch(bus);
        let _ = self.rd(bus, 0x0100 | self.s as u16); // internal delay cycle
        let pc_before_hi_fetch = self.pc;
        self.push(bus, (pc_before_hi_fetch >> 8) as u8);
        self.push(bus, (pc_before_hi_fetch & 0xFF) as u8);
        let hi = self.fetch(bus);
        self.pc = ((hi as u16) << 8) | lo as u16;
    }

    fn rts(&mut self, bus: &mut impl Bus) {
        self.implied_filler(bus);
        let _ = self.pop_peek(bus);
        let lo = self.pop(bus);
        let hi = self.pop(bus);
        let addr = ((hi as u16) << 8) | lo as u16;
        let _ = self.rd(bus, addr); // dummy read at the pulled address
        self.pc = addr.wrapping_add(1);
    }

    fn rti(&mut self, bus: &mut impl Bus) {
        self.implied_filler(bus);
        let _ = self.pop_peek(bus);
        let p = self.pop(bus);
        let lo = self.pop(bus);
        let hi = self.pop(bus);
        // There is no physical latch for the B flag; pulling P (via PLP or
        // RTI) always yields B=0, U=1, regardless of what was on the stack.
        self.p = (p & !FLAG_B) | FLAG_U;
        self.pc = ((hi as u16) << 8) | lo as u16;
    }

    fn brk(&mut self, bus: &mut impl Bus) {
        let _ = self.fetch(bus); // padding byte
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, (self.pc & 0xFF) as u8);
        self.push(bus, self.p | FLAG_B | FLAG_U);
        // I is set as the vector is fetched, so no IRQ is polled in.
        self.set_flag(FLAG_I, true);
        let lo = self.rd(bus, 0xFFFE);
        let hi = self.rd(bus, 0xFFFF);
        self.pc = ((hi as u16) << 8) | lo as u16;
    }

    fn pha(&mut self, bus: &mut impl Bus) {
        self.implied_filler(bus);
        let v = self.a;
        self.push(bus, v);
    }

    fn php(&mut self, bus: &mut impl Bus) {
        self.implied_filler(bus);
        let v = self.p | FLAG_B | FLAG_U;
        self.push(bus, v);
    }

    fn pla(&mut self, bus: &mut impl Bus) {
        self.implied_filler(bus);
        let _ = self.pop_peek(bus);
        let v = self.pop(bus);
        self.a = v;
        self.set_nz(v);
    }

    fn plp(&mut self, bus: &mut impl Bus) {
        self.implied_filler(bus);
        let _ = self.pop_peek(bus);
        let v = self.pop(bus);
        self.p = (v & !FLAG_B) | FLAG_U;
    }

    /// JAM/KIL/HLT: the instruction decode PLA never finds a valid next
    /// micro-op, so the CPU locks up permanently, driving the address bus
    /// through a fixed, reproducible pattern centred on the IRQ/BRK vector
    /// ($FFFE/$FFFF) rather than fetching further instructions. This exact
    /// sequence (and its length) is empirically verified against real
    /// silicon (SingleStepTests/65x02) and is identical for all twelve JAM
    /// opcodes. `step()` never advances past this -- callers must check
    /// `jammed` and stop scheduling this CPU (a real C64 that executes a
    /// JAM just hangs until reset).
    fn jam(&mut self, bus: &mut impl Bus) {
        self.jammed = true;
        let _ = self.rd(bus, self.pc); // dummy fetch of the following byte
        let _ = self.rd(bus, 0xFFFF);
        let _ = self.rd(bus, 0xFFFE);
        let _ = self.rd(bus, 0xFFFE);
        for _ in 0..6 {
            let _ = self.rd(bus, 0xFFFF);
        }
    }
}
