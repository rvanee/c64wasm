// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The MOS 6502 core shared by the C64's 6510 and the 1541's 6502.
//!
//! Every cycle of an NMOS 6502 performs exactly one bus access (read or
//! write), including the dummy accesses of the real chip. All accesses go
//! through `rd`/`wr`, which also clock the rest of the machine, so getting
//! the access sequence right makes the cycle count right -- and that
//! sequence is what Tom Harte's single-step tests check, cycle by cycle
//! (`src/bin/tomharte.rs`).
//!
//! `step` runs one whole instruction. That is enough for RDY stalling as
//! well: RDY only holds up reads, and a held read simply repeats in the
//! next cycle, which is what `rd` does.

use super::Bus;

pub const FLAG_C: u8 = 0x01;
pub const FLAG_Z: u8 = 0x02;
pub const FLAG_I: u8 = 0x04;
pub const FLAG_D: u8 = 0x08;
pub const FLAG_B: u8 = 0x10;
pub const FLAG_U: u8 = 0x20; // unused, always reads as 1
pub const FLAG_V: u8 = 0x40;
pub const FLAG_N: u8 = 0x80;

/// Registers and interrupt state of a 6502.
#[derive(Debug, Clone)]
pub struct Mos6502 {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub pc: u16,
    pub p: u8,
    /// True once a JAM/KIL (illegal, halting) opcode has been executed.
    pub jammed: bool,
    /// Cycles consumed by the *current* `step()` call. Reset at the start
    /// of every `step()`.
    pub(super) step_cycles: u32,
    /// Interrupt polling: what the CPU saw on its NMI (edge latched) and
    /// IRQ (line asserted and I clear) inputs in the latest cycle.
    pub(super) poll_cur: (bool, bool),
}

impl Default for Mos6502 {
    fn default() -> Self {
        Self::new()
    }
}

impl Mos6502 {
    pub fn new() -> Self {
        Mos6502 {
            a: 0,
            x: 0,
            y: 0,
            s: 0xFD,
            pc: 0,
            p: FLAG_U | FLAG_I,
            jammed: false,
            step_cycles: 0,
            poll_cur: (false, false),
        }
    }

    #[inline]
    pub fn flag(&self, f: u8) -> bool {
        self.p & f != 0
    }

    #[inline]
    pub fn set_flag(&mut self, f: u8, v: bool) {
        if v {
            self.p |= f;
        } else {
            self.p &= !f;
        }
    }

    #[inline]
    pub(super) fn set_nz(&mut self, v: u8) {
        self.set_flag(FLAG_Z, v == 0);
        self.set_flag(FLAG_N, v & 0x80 != 0);
    }

    #[inline]
    pub(super) fn rd(&mut self, bus: &mut impl Bus, addr: u16) -> u8 {
        loop {
            let rdy = bus.tick();
            if bus.take_so_edge() {
                self.set_flag(FLAG_V, true);
            }
            self.step_cycles += 1;
            self.poll(bus);
            if rdy {
                return bus.read(addr);
            }
            // RDY low: this cycle was stolen (a VIC-II bad line or sprite
            // DMA fetch used the bus instead). The CPU makes no progress --
            // it'll re-try the same read next cycle -- but the cycle still
            // elapsed, so it's still counted.
        }
    }

    #[inline]
    pub(super) fn wr(&mut self, bus: &mut impl Bus, addr: u16, val: u8) {
        // Real 6510s never stall a *write* cycle on RDY -- but the master
        // clock (and hence the VIC) still advances by one cycle here, so we
        // still call tick().
        let _ = bus.tick();
        if bus.take_so_edge() {
            self.set_flag(FLAG_V, true);
        }
        self.step_cycles += 1;
        self.poll(bus);
        bus.write(addr, val);
    }

    #[inline]
    pub(super) fn poll(&mut self, bus: &mut impl Bus) {
        self.poll_cur = (bus.nmi_edge_pending(), bus.irq_pending() && !self.flag(FLAG_I));
    }

    #[inline]
    pub(super) fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        let v = self.rd(bus, self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    #[inline]
    pub(super) fn push(&mut self, bus: &mut impl Bus, val: u8) {
        self.wr(bus, 0x0100 | self.s as u16, val);
        self.s = self.s.wrapping_sub(1);
    }

    #[inline]
    pub(super) fn pop_peek(&mut self, bus: &mut impl Bus) -> u8 {
        // dummy read at the *current* (pre-increment) stack address; real
        // hardware does this before bumping S. Discarded by the caller.
        self.rd(bus, 0x0100 | self.s as u16)
    }

    #[inline]
    pub(super) fn pop(&mut self, bus: &mut impl Bus) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.rd(bus, 0x0100 | self.s as u16)
    }

    /// The "read next byte at PC and throw it away" filler cycle that real
    /// 2-cycle implied/accumulator instructions perform.
    #[inline]
    pub(super) fn implied_filler(&mut self, bus: &mut impl Bus) {
        let _ = self.rd(bus, self.pc);
    }

    // ---- top level ----------------------------------------------------

    /// Execute exactly one instruction, returning the number of cycles it
    /// took (equivalently: the number of bus transactions it performed).
    pub fn step(&mut self, bus: &mut impl Bus) -> u32 {
        self.step_cycles = 0;
        // NMI first: it ignores the I flag and wins over a simultaneous IRQ.
        // Interrupts are judged on what the inputs showed in the previous
        // instruction's last cycle -- with the chips' state after that
        // cycle's clock, which matches a real 6502 polling in its second-
        // to-last cycle (the CIAs' outputs change on the opposite clock
        // phase), checked against VICE with a CIA timer test program -- but
        // with the I flag as it was during that cycle, so an IRQ gets in
        // only after the instruction following a CLI, and still right after
        // a SEI, as on the real CPU.
        let (nmi, irq) = self.poll_cur;
        if !self.jammed && nmi && bus.take_nmi_edge() {
            self.poll_cur.0 = false;
            self.service_interrupt(bus, 0xFFFA);
            bus.note_pc(self.pc);
            return self.step_cycles;
        }
        // A pending IRQ, with interrupts not masked, hijacks what would have
        // been the next opcode fetch -- exactly like a real 6510 -- rather
        // than being polled at some coarser granularity. A JAMmed CPU never
        // recovers on real hardware either way, so it's excluded here too.
        if !self.jammed && irq {
            self.service_interrupt(bus, 0xFFFE);
            bus.note_pc(self.pc);
            return self.step_cycles;
        }
        let opcode = self.fetch(bus);
        self.execute(bus, opcode);
        bus.note_pc(self.pc);
        self.step_cycles
    }

    /// Service a maskable interrupt (IRQ): the same 7-cycle bus sequence as
    /// `BRK` (see below), but with no opcode/padding byte to fetch first --
    /// real hardware instead spends those two cycles on throwaway reads of
    /// the instruction that *would* have been fetched next, before
    /// hijacking the sequence for the interrupt -- and with B=0 in the
    /// pushed status (vs. BRK's B=1), which is the only way a shared
    /// IRQ/BRK handler can tell a real hardware interrupt from a `BRK`
    /// instruction after the fact.
    fn service_interrupt(&mut self, bus: &mut impl Bus, vector: u16) {
        let _ = self.rd(bus, self.pc);
        let _ = self.rd(bus, self.pc);
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, (self.pc & 0xFF) as u8);
        let p = (self.p & !FLAG_B) | FLAG_U;
        self.push(bus, p);
        self.set_flag(FLAG_I, true);
        let lo = self.rd(bus, vector);
        let hi = self.rd(bus, vector.wrapping_add(1));
        self.pc = ((hi as u16) << 8) | lo as u16;
    }
}
