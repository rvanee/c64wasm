// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The MOS 6510: the 6502 core plus a 6-bit I/O port at $00 (data
//! direction) and $01 (data). In the C64 the port's low three bits
//! (LORAM, HIRAM, CHAREN) select the memory configuration, and bits 3-5
//! drive the datasette.
//!
//! The port sits between the core and the bus: accesses to $00/$01 are
//! answered by the port, and every port write tells the bus the new pin
//! levels through `Bus::processor_port_changed`.

use std::ops::{Deref, DerefMut};

use super::{Bus, Mos6502};

/// The 6510's on-chip I/O port.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessorPort {
    /// $00: 1 = output.
    ddr: u8,
    /// $01: the output latch.
    data: u8,
}

impl ProcessorPort {
    /// Pin levels: output bits follow the latch, input bits float high
    /// (pulled up). After reset everything is input, so all bits read 1 --
    /// which is why BASIC, KERNAL and I/O are visible at power-on.
    pub fn pins(&self) -> u8 {
        (self.data & self.ddr) | !self.ddr
    }

    fn read(&self, addr: u16) -> u8 {
        if addr == 0 {
            self.ddr
        } else {
            self.pins()
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        if addr == 0 {
            self.ddr = val;
        } else {
            self.data = val;
        }
    }
}

/// A 6510: a [`Mos6502`] (reachable through `Deref` for registers and
/// flags) with its I/O port.
#[derive(Debug, Clone, Default)]
pub struct Mos6510 {
    core: Mos6502,
    port: ProcessorPort,
}

impl Mos6510 {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute one instruction (see [`Mos6502::step`]), with the I/O port
    /// in front of the bus.
    pub fn step(&mut self, bus: &mut impl Bus) -> u32 {
        let mut with_port = PortBus { port: &mut self.port, bus };
        self.core.step(&mut with_port)
    }

    pub fn port(&self) -> &ProcessorPort {
        &self.port
    }

    /// A CPU-side access to $00/$01 from outside the CPU (a debugger or a
    /// test poking the port), with the same effect on the bus as an
    /// instruction would have.
    pub fn write_port(&mut self, bus: &mut impl Bus, addr: u16, val: u8) {
        self.port.write(addr, val);
        bus.processor_port_changed(self.port.pins());
    }

    pub fn read_port(&self, addr: u16) -> u8 {
        self.port.read(addr)
    }
}

impl Deref for Mos6510 {
    type Target = Mos6502;
    fn deref(&self) -> &Mos6502 {
        &self.core
    }
}

impl DerefMut for Mos6510 {
    fn deref_mut(&mut self) -> &mut Mos6502 {
        &mut self.core
    }
}

/// The bus as the 6502 core inside a 6510 sees it.
struct PortBus<'a, B: Bus> {
    port: &'a mut ProcessorPort,
    bus: &'a mut B,
}

impl<B: Bus> Bus for PortBus<'_, B> {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        if addr < 2 {
            self.port.read(addr)
        } else {
            self.bus.read(addr)
        }
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        if addr < 2 {
            self.port.write(addr, val);
            self.bus.processor_port_changed(self.port.pins());
        } else {
            self.bus.write(addr, val);
        }
    }

    #[inline]
    fn tick(&mut self) -> bool {
        self.bus.tick()
    }

    #[inline]
    fn note_pc(&mut self, pc: u16) {
        self.bus.note_pc(pc);
    }

    #[inline]
    fn processor_port_changed(&mut self, pins: u8) {
        self.bus.processor_port_changed(pins);
    }

    #[inline]
    fn nmi_edge_pending(&self) -> bool {
        self.bus.nmi_edge_pending()
    }

    #[inline]
    fn irq_pending(&self) -> bool {
        self.bus.irq_pending()
    }

    #[inline]
    fn take_so_edge(&mut self) -> bool {
        self.bus.take_so_edge()
    }

    #[inline]
    fn take_nmi_edge(&mut self) -> bool {
        self.bus.take_nmi_edge()
    }
}
