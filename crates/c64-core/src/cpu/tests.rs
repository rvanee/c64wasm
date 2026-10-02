// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Quick CPU smoke tests that don't need the single-step test vectors
//! (the full check is `src/bin/tomharte.rs`).

use super::mos6502::*;
use super::*;

struct FlatBus {
    mem: [u8; 65536],
}
impl FlatBus {
    fn new() -> Self {
        FlatBus { mem: [0; 65536] }
    }
}
impl Bus for FlatBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }
    fn write(&mut self, addr: u16, val: u8) {
        self.mem[addr as usize] = val;
    }
}

#[test]
fn lda_immediate_sets_a_and_flags() {
    let mut bus = FlatBus::new();
    bus.mem[0x0200] = 0xA9; // LDA #$00
    bus.mem[0x0201] = 0x00;
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 2);
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.flag(FLAG_Z));
    assert!(!cpu.flag(FLAG_N));
}

#[test]
fn lda_sta_round_trip() {
    let mut bus = FlatBus::new();
    // LDA #$42 ; STA $10
    bus.mem[0x0200] = 0xA9;
    bus.mem[0x0201] = 0x42;
    bus.mem[0x0202] = 0x85;
    bus.mem[0x0203] = 0x10;
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!(bus.mem[0x10], 0x42);
}

#[test]
fn branch_taken_across_page_costs_four_cycles() {
    let mut bus = FlatBus::new();
    // Opcode+operand occupy $01FD/$01FE, so PC is $01FF right after the
    // fetch; a +2 offset lands at $0201, crossing from page 1 to page 2.
    bus.mem[0x01FD] = 0xF0; // BEQ +2
    bus.mem[0x01FE] = 0x02;
    let mut cpu = Mos6502::new();
    cpu.pc = 0x01FD;
    cpu.set_flag(FLAG_Z, true);
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 4);
    assert_eq!(cpu.pc, 0x0201);
}

#[test]
fn jmp_indirect_page_wrap_bug() {
    // The classic 6502 bug: JMP ($30FF) fetches the high byte from
    // $3000, not $3100.
    let mut bus = FlatBus::new();
    bus.mem[0x0200] = 0x6C; // JMP ($30FF)
    bus.mem[0x0201] = 0xFF;
    bus.mem[0x0202] = 0x30;
    bus.mem[0x30FF] = 0x80;
    bus.mem[0x3000] = 0x12; // wrongly-wrapped high byte
    bus.mem[0x3100] = 0x34; // what a non-buggy fetch would read
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x1280);
}

#[test]
fn jsr_rts_round_trip() {
    let mut bus = FlatBus::new();
    bus.mem[0x0200] = 0x20; // JSR $0300
    bus.mem[0x0201] = 0x00;
    bus.mem[0x0202] = 0x03;
    bus.mem[0x0300] = 0x60; // RTS
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    cpu.s = 0xFD;
    cpu.step(&mut bus); // JSR
    assert_eq!(cpu.pc, 0x0300);
    cpu.step(&mut bus); // RTS
    assert_eq!(cpu.pc, 0x0203);
    assert_eq!(cpu.s, 0xFD);
}

#[test]
fn jam_sets_flag_and_freezes_pc() {
    let mut bus = FlatBus::new();
    bus.mem[0x0200] = 0x02; // JAM
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    let cycles = cpu.step(&mut bus);
    assert_eq!(cycles, 11);
    assert!(cpu.jammed);
    assert_eq!(cpu.pc, 0x0201);
}

/// A `FlatBus` that additionally reports one SO edge, exactly once,
/// after a chosen number of cycles have elapsed -- enough to test the
/// SO-pin -> V-flag wiring without needing a real VIA.
struct SoEdgeBus {
    inner: FlatBus,
    cycles_until_edge: u32,
    edge_pending: bool,
}
impl Bus for SoEdgeBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.inner.read(addr)
    }
    fn write(&mut self, addr: u16, val: u8) {
        self.inner.write(addr, val)
    }
    fn tick(&mut self) -> bool {
        if self.cycles_until_edge > 0 {
            self.cycles_until_edge -= 1;
            if self.cycles_until_edge == 0 {
                self.edge_pending = true;
            }
        }
        true
    }
    fn take_so_edge(&mut self) -> bool {
        std::mem::take(&mut self.edge_pending)
    }
}

#[test]
fn so_edge_sets_v_flag_mid_instruction_without_an_opcode() {
    // Each NOP is 2 cycles (opcode fetch + a throwaway implied-mode
    // read), so the 3rd tick overall falls on NOP #2's first cycle.
    let mut bus = SoEdgeBus { inner: FlatBus::new(), cycles_until_edge: 3, edge_pending: false };
    // Three NOPs: the SO edge falls during the second one's first bus
    // cycle, with no instruction anywhere near a real overflow
    // computation -- the V flag must still end up set, purely from the
    // pin, exactly like real 6502/6510 hardware.
    bus.inner.mem[0x0200] = 0xEA;
    bus.inner.mem[0x0201] = 0xEA;
    bus.inner.mem[0x0202] = 0xEA;
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    assert!(!cpu.flag(FLAG_V));
    cpu.step(&mut bus); // NOP #1: tick #1, edge armed but not yet due
    assert!(!cpu.flag(FLAG_V));
    cpu.step(&mut bus); // NOP #2: tick #2, edge fires
    assert!(cpu.flag(FLAG_V));
}

#[test]
fn default_take_so_edge_never_fires_for_ordinary_buses() {
    // FlatBus doesn't override take_so_edge, so the default (false) is
    // exercised on every single cycle already run by every other test
    // in this file -- this just makes the "never sets V on its own"
    // property explicit for one direct case.
    let mut bus = FlatBus::new();
    bus.mem[0x0200] = 0xEA; // NOP
    let mut cpu = Mos6502::new();
    cpu.pc = 0x0200;
    cpu.step(&mut bus);
    assert!(!cpu.flag(FLAG_V));
}
