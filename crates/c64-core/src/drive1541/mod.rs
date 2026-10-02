// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The Commodore 1541 disk drive: a complete computer of its own, with a
//! 6502 running the DOS ROM, connected to the C64 only through the serial
//! bus.
//!
//! - [`mechanics`]: what moves: spindle motor, stepper and head, the
//!   write-protect sensor and the disk.
//! - [`electronics`]: the logic board: RAM, ROM, the two VIAs, the
//!   read/write channel, the activity LED and the serial-bus interface.
//!
//! The drive has its own 1 MHz crystal. The C64 runs it through a
//! [`ClockBridge`], so the two clocks keep their exact ratio.

pub mod electronics;
pub mod mechanics;

use std::fmt;

use crate::cbm_iec::{IecLines, IecOutput};
use crate::clock::ClockBridge;
use crate::cpu::{Bus, Mos6502};
use crate::media::Disk;
use crate::memory::{Rom, RomError};
use electronics::serial_interface::{MAX_DEVICE, MIN_DEVICE};
use electronics::LogicBoard;

pub use electronics::DOS_ROM_SIZE;

/// The drive's clock (a 16 MHz crystal divided by 16).
pub const CLOCK_HZ: u32 = 1_000_000;

/// Why a drive couldn't be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveError {
    Rom(RomError),
    /// Device numbers are set by two solder pads: 8 to 11.
    DeviceNumber(u8),
}

impl fmt::Display for DriveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DriveError::Rom(e) => e.fmt(f),
            DriveError::DeviceNumber(n) => {
                write!(f, "1541 device number must be {MIN_DEVICE}-{MAX_DEVICE}, got {n}")
            }
        }
    }
}

impl std::error::Error for DriveError {}

impl From<RomError> for DriveError {
    fn from(e: RomError) -> Self {
        DriveError::Rom(e)
    }
}

/// A 1541: its CPU and its logic board (which holds the mechanism).
#[derive(Debug)]
pub struct Drive1541 {
    cpu: Mos6502,
    board: LogicBoard,
    device_number: u8,
}

impl Drive1541 {
    /// Power on a drive with a 16 KB DOS ROM image as device 8-11.
    pub fn new(dos_rom: &[u8], device_number: u8) -> Result<Self, DriveError> {
        if !(MIN_DEVICE..=MAX_DEVICE).contains(&device_number) {
            return Err(DriveError::DeviceNumber(device_number));
        }
        let rom = Rom::new("1541 DOS", dos_rom)?;
        let mut board = LogicBoard::new(rom, device_number);
        let mut cpu = Mos6502::new();
        let lo = board.read(0xFFFC) as u16;
        let hi = board.read(0xFFFD) as u16;
        cpu.pc = hi << 8 | lo;
        Ok(Drive1541 { cpu, board, device_number })
    }

    pub fn device_number(&self) -> u8 {
        self.device_number
    }

    /// Run one instruction; returns the cycles it took.
    pub fn step(&mut self) -> u32 {
        self.cpu.step(&mut self.board)
    }

    /// One cycle of the host passed: run the instructions the drive is owed.
    /// A jammed CPU just hangs, as on the real drive.
    pub fn run_from_host(&mut self, clock: &mut ClockBridge) {
        clock.host_tick();
        while clock.device_due() {
            if self.cpu.jammed {
                clock.clear();
                break;
            }
            clock.device_ran(self.step());
        }
    }

    /// The drive's CPU (registers, for diagnostics).
    pub fn cpu(&self) -> &Mos6502 {
        &self.cpu
    }

    /// The logic board (VIAs, mechanism, LED), for diagnostics.
    pub fn board(&self) -> &LogicBoard {
        &self.board
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn jammed(&self) -> bool {
        self.cpu.jammed
    }

    pub fn peek_ram(&self, addr: u16) -> u8 {
        self.board.peek_ram(addr)
    }

    // ---- serial bus ----

    /// What the drive drives onto the bus, given the bus's ATN line.
    pub fn bus_output(&self, atn: bool) -> IecOutput {
        self.board.bus_output(atn)
    }

    /// Present the bus lines to the drive. Call every host cycle: ATN
    /// reaches VIA1 asynchronously to the drive's clock.
    pub fn receive_bus(&mut self, lines: IecLines) {
        self.board.receive_bus(lines);
    }

    // ---- disk and front panel ----

    pub fn insert_disk(&mut self, disk: Disk) {
        self.board.mechanics.insert_disk(disk);
    }

    pub fn eject_disk(&mut self) -> Option<Disk> {
        self.board.mechanics.eject_disk()
    }

    pub fn disk(&self) -> Option<&Disk> {
        self.board.mechanics.disk()
    }

    pub fn motor_on(&self) -> bool {
        self.board.mechanics.motor_on()
    }

    /// Head position in physical half-tracks (1 = track 1).
    pub fn current_half_track(&self) -> u8 {
        self.board.mechanics.current_half_track()
    }

    /// Drive cycles since power-on.
    pub fn cycles(&self) -> u64 {
        self.board.mechanics.cycles()
    }

    /// Head steps since the last call: (cycle, bumped against the stop).
    pub fn take_head_events(&mut self, out: &mut Vec<(u64, bool)>) {
        self.board.mechanics.take_head_events(out);
    }

    /// Fraction of the time the activity LED was lit since the last call.
    pub fn take_led_duty(&mut self) -> f32 {
        self.board.activity_led.take_duty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cbm_iec::IecLines;

    /// NOPs, with the reset vector at $C000.
    fn nop_rom() -> Vec<u8> {
        let mut rom = vec![0xEA; DOS_ROM_SIZE];
        rom[0x3FFC] = 0x00;
        rom[0x3FFD] = 0xC0;
        rom
    }

    fn drive() -> Drive1541 {
        Drive1541::new(&nop_rom(), 8).unwrap()
    }

    #[test]
    fn powers_on_at_the_reset_vector() {
        assert_eq!(drive().pc(), 0xC000);
    }

    #[test]
    fn rejects_a_wrong_rom_or_device_number() {
        assert!(matches!(Drive1541::new(&[0; 100], 8), Err(DriveError::Rom(_))));
        assert_eq!(Drive1541::new(&nop_rom(), 7).unwrap_err(), DriveError::DeviceNumber(7));
        assert_eq!(Drive1541::new(&nop_rom(), 12).unwrap_err(), DriveError::DeviceNumber(12));
    }

    #[test]
    fn ram_is_mirrored_up_to_via1() {
        let mut d = drive();
        d.board.write(0x0042, 0x99);
        assert_eq!(d.board.read(0x0842), 0x99);
        assert_eq!(d.board.read(0x1042), 0x99);
    }

    #[test]
    fn via_registers_are_mirrored_every_16_bytes() {
        let mut d = drive();
        d.board.write(0x1802, 0x1A);
        assert_eq!(d.board.read(0x1812), 0x1A);
        assert_eq!(d.board.read(0x1B92), 0x1A);
    }

    #[test]
    fn step_runs_one_instruction() {
        let mut d = drive();
        assert_eq!(d.step(), 2);
        assert_eq!(d.pc(), 0xC001);
    }

    #[test]
    fn runs_at_its_own_rate_from_a_pal_c64() {
        let mut d = drive();
        let mut clock = ClockBridge::new(CLOCK_HZ, 985_248);
        for _ in 0..4 {
            d.run_from_host(&mut clock);
        }
        assert!(d.pc() > 0xC000);
        for _ in 0..100_000 {
            d.run_from_host(&mut clock);
        }
        // At most one instruction (7 cycles) ahead, never a cycle behind.
        let owed = clock.owed();
        assert!(owed > -7 * 985_248 && owed < 985_248, "owed {owed}");
    }

    #[test]
    fn atn_acknowledge_gate_pulls_data() {
        let mut d = drive();
        d.board.write(0x1802, 0b0001_1010); // DATA OUT, CLOCK OUT, ATNA are outputs
        d.board.write(0x1800, 0);
        assert!(d.bus_output(true).data, "ATN not yet acknowledged");
        assert!(!d.bus_output(false).data);
        d.board.write(0x1800, 0b0001_0000);
        assert!(!d.bus_output(true).data, "acknowledged");
        d.board.write(0x1800, 0b0001_0010);
        assert!(d.bus_output(true).data, "DATA OUT");
    }

    #[test]
    fn atn_edge_interrupts_the_drive() {
        let mut d = drive();
        d.board.write(0x180C, 0x01); // CA1 rising edge, as the DOS sets it
        d.board.write(0x180E, 0x82); // enable CA1
        d.receive_bus(IecLines::default());
        assert!(!d.board.irq_pending());
        d.receive_bus(IecLines { atn: true, ..Default::default() });
        assert!(d.board.irq_pending());
        assert_eq!(d.board.read(0x1800) & 0x80, 0x80, "ATN IN reads 1");
    }

    #[test]
    fn device_number_reaches_via1() {
        for (n, bits) in [(8, 0x00), (9, 0x20), (10, 0x40), (11, 0x60)] {
            let mut d = Drive1541::new(&nop_rom(), n).unwrap();
            d.receive_bus(IecLines::default());
            assert_eq!(d.board.read(0x1800) & 0x60, bits, "device {n}");
        }
    }

    #[test]
    fn insert_and_eject() {
        let mut d = drive();
        assert!(d.disk().is_none());
        d.insert_disk(Disk::blank());
        assert!(d.disk().is_some());
        assert!(d.eject_disk().is_some());
        assert!(d.disk().is_none());
    }
}
