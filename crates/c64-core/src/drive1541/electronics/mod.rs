// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The 1541's logic board: 2 KB RAM, the 16 KB DOS ROM, two VIAs, the
//! read/write electronics and the activity LED, on the drive CPU's bus.
//!
//! ## Address map
//! - `$0000-$17FF`: 2 KB RAM, mirrored.
//! - `$1800-$1BFF`: VIA1 (serial bus), mirrored every 16 bytes.
//! - `$1C00-$1FFF`: VIA2 (drive control), mirrored every 16 bytes.
//! - `$C000-$FFFF`: DOS ROM. The rest is open bus ($FF).
//!
//! ## VIA2 (drive control)
//! Port B: bits 0-1 stepper phase, 2 spindle motor, 3 activity LED,
//! 4 write-protect sensor (input, 1 = writable), 5-6 density (bit rate),
//! 7 sync (input, 0 = sync mark under the head). Port A: data to and from
//! the read/write electronics. CA1 and the CPU's SO pin: byte ready. CB2:
//! read (high) or write (low).
//!
//! Every cycle the board first applies what the CPU last wrote (stepper,
//! motor, density, write data and mode), then clocks the mechanism and the
//! read/write electronics, and then presents their outputs (the new byte,
//! sync, byte ready) to VIA2 -- so a byte completed in this cycle is on
//! Port A before byte ready latches it.

pub mod read_write;
pub mod serial_interface;

use crate::cbm_iec::{IecLines, IecOutput};
use crate::cpu::Bus;
use crate::drive1541::mechanics::Mechanics;
use crate::led::{Led, LedColor};
use crate::memory::{Ram, Rom};
use crate::via::Via6522;
use read_write::ReadWrite;

const PB_STEPPER: u8 = 0b0000_0011;
const PB_MOTOR: u8 = 0b0000_0100;
const PB_LED: u8 = 0b0000_1000;
const PB_WRITABLE: u8 = 0b0001_0000;
const PB_DENSITY_SHIFT: u8 = 5;
const PB_NO_SYNC: u8 = 0b1000_0000;

pub const DOS_ROM_SIZE: usize = 16384;

#[derive(Debug)]
pub struct LogicBoard {
    ram: Ram<2048>,
    rom: Rom<DOS_ROM_SIZE>,
    pub via1: Via6522,
    pub via2: Via6522,
    read_write: ReadWrite,
    pub mechanics: Mechanics,
    pub activity_led: Led,
    /// Device number pads (VIA1 PB5-6).
    device_bits: u8,
    /// Byte ready fired; consumed by the CPU's SO input.
    so_edge: bool,
}

impl LogicBoard {
    pub fn new(rom: Rom<DOS_ROM_SIZE>, device_number: u8) -> Self {
        LogicBoard {
            ram: Ram::new(),
            rom,
            via1: Via6522::new(),
            via2: Via6522::new(),
            read_write: ReadWrite::new(),
            mechanics: Mechanics::new(),
            activity_led: Led::new(LedColor::Red),
            device_bits: serial_interface::device_number_bits(device_number),
            so_edge: false,
        }
    }

    pub fn peek_ram(&self, addr: u16) -> u8 {
        self.ram.read(addr)
    }

    /// What this drive drives onto the serial bus.
    pub fn bus_output(&self, atn: bool) -> IecOutput {
        serial_interface::bus_output(self.via1.port_b_pins(), atn)
    }

    /// Present the bus lines to VIA1. ATN also goes to CA1, asynchronously
    /// to the drive's own clock.
    pub fn receive_bus(&mut self, lines: IecLines) {
        self.via1.set_pb_input(serial_interface::port_input(lines) | self.device_bits);
        self.via1.set_ca1(lines.atn);
    }
}

impl Bus for LogicBoard {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x17FF => self.ram.read(addr),
            0x1800..=0x1BFF => self.via1.read((addr & 0x0F) as u8),
            0x1C00..=0x1FFF => self.via2.read((addr & 0x0F) as u8),
            0xC000..=0xFFFF => self.rom.read(addr - 0xC000),
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x17FF => self.ram.write(addr, val),
            0x1800..=0x1BFF => self.via1.write((addr & 0x0F) as u8, val),
            0x1C00..=0x1FFF => self.via2.write((addr & 0x0F) as u8, val),
            _ => {}
        }
    }

    /// One drive cycle (1 MHz).
    fn tick(&mut self) -> bool {
        self.via1.tick();
        self.via2.tick();

        let pb = self.via2.port_b_pins();
        self.mechanics.set_motor(pb & PB_MOTOR != 0);
        self.read_write.set_density((pb >> PB_DENSITY_SHIFT) & 0b11);
        if self.mechanics.set_stepper_phase(pb & PB_STEPPER) {
            self.read_write.head_moved();
        }
        self.read_write.set_write_data(self.via2.port_a_pins());
        self.read_write.set_write_mode(!self.via2.cb2_output());

        self.mechanics.clock();
        self.read_write.clock(&mut self.mechanics);
        self.activity_led.clock(pb & PB_LED != 0);

        self.via2.set_pa_input(self.read_write.last_byte());
        let mut pb_in = 0;
        if self.read_write.no_sync() {
            pb_in |= PB_NO_SYNC;
        }
        if self.mechanics.writable() {
            pb_in |= PB_WRITABLE;
        }
        self.via2.set_pb_input(pb_in);
        let byte_ready = self.read_write.byte_ready();
        self.via2.set_ca1(!byte_ready);
        if byte_ready {
            self.so_edge = true;
        }
        true // nothing on this bus steals cycles
    }

    fn irq_pending(&self) -> bool {
        self.via1.irq_pending() || self.via2.irq_pending()
    }

    fn take_so_edge(&mut self) -> bool {
        std::mem::take(&mut self.so_edge)
    }
}
