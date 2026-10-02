// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The CPU's view of the outside world.
//!
//! Every clock cycle of an NMOS 6502 is exactly one bus access, so the
//! core calls `tick` once per access and lets the bus advance the rest of
//! the machine; that is also how RDY (bus stealing by the VIC-II) and the
//! interrupt and SO inputs reach the CPU.

/// The address/data bus plus the CPU's control inputs. Only `read` and
/// `write` are required; the defaults describe a plain memory with nothing
/// else attached (as used by the CPU's unit tests and the single-step test
/// harness).
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);

    /// Advance every other chip on the bus by exactly one master clock
    /// cycle, and report whether the CPU may actually use the bus this
    /// cycle (the real 6510's RDY input). Called exactly once per elapsed
    /// CPU cycle -- including ones the CPU ends up not getting, which is
    /// how VIC-II bad lines and sprite DMA steal cycles from the CPU on
    /// real hardware: RDY goes low, the CPU's current read just keeps
    /// re-issuing (never advancing) until it goes high again, while
    /// whatever's holding it low gets the bus to itself in the meantime.
    ///
    /// The default never stalls.
    #[inline]
    fn tick(&mut self) -> bool {
        true
    }

    /// Told the address of the instruction the CPU is about to execute
    /// (the VIC-II's DMA-delay quirk reads colour data from it).
    #[inline]
    fn note_pc(&mut self, _pc: u16) {}

    /// The 6510's I/O port pins changed (see `cpu::Mos6510`). The C64 uses
    /// the low three bits for memory banking.
    #[inline]
    fn processor_port_changed(&mut self, _pins: u8) {}

    /// Whether an NMI edge has been latched and not yet taken (without
    /// consuming it; see `take_nmi_edge`).
    #[inline]
    fn nmi_edge_pending(&self) -> bool {
        false
    }

    /// Whether something is pulling the CPU's IRQ line low right now.
    /// Level-sensitive: it stays asserted until the source is acknowledged.
    #[inline]
    fn irq_pending(&self) -> bool {
        false
    }

    /// Whether a fresh falling edge has occurred on the CPU's SO (Set
    /// Overflow) input since the last time this was called -- real 6502/
    /// 6510 hardware sets the V flag asynchronously, the instant SO falls,
    /// with no instruction involved. The 1541 wires its byte-ready
    /// signal directly to this pin, which is how its tight GCR read loop notices a fresh byte
    /// within a couple of cycles using `BVC`/`CLV` rather than a slower
    /// register poll. Checked every single bus cycle (from `rd`/`wr`,
    /// alongside `tick`) rather than once per instruction like
    /// `irq_pending`, since a real edge can fall and be tested again
    /// (`BVC`) within the same instruction's execution. A `Bus` that reports
    /// an edge is responsible for not reporting the same edge twice.
    #[inline]
    fn take_so_edge(&mut self) -> bool {
        false
    }

    /// Whether a falling (asserting) edge has occurred on the CPU's NMI
    /// input since the last call. NMI is *edge*-triggered on real hardware:
    /// the 6510 latches the edge internally, so an NMI source that asserts
    /// and is acknowledged again within one instruction (e.g. a CIA2 timer
    /// interrupt whose ICR gets read right away) is still serviced. The bus
    /// does the edge detection every cycle and latches it; the CPU consumes
    /// it at the next instruction boundary. Defaults to `false`.
    #[inline]
    fn take_nmi_edge(&mut self) -> bool {
        false
    }
}
