// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The C64 motherboard as the CPU sees it: memory, the PLA, the I/O chips
//! and the serial port, with every chip clocked once per CPU bus cycle.
//!
//! ## I/O area ($D000-$DFFF when I/O is visible)
//! - `$D000`: VIC-II (64 registers, mirrored)
//! - `$D400`: SID (32 registers, mirrored)
//! - `$D800`: colour RAM, 1024 x 4 bits; the upper nibble reads as 1s
//! - `$DC00`: CIA1 (keyboard, IRQ)
//! - `$DD00`: CIA2 (serial bus, VIC-II bank, NMI)
//! - `$DE00-$DFFF`: cartridge I/O, nothing connected (reads $FF)
//!
//! ## One cycle ([`Bus::tick`])
//! 1. The VIC-II does its memory accesses and says whether it needs the
//!    bus (RDY).
//! 2. Both CIAs count; CIA2's interrupt output is the NMI line.
//! 3. The SID runs.
//! 4. With drives attached, the serial bus lines are recomputed from both
//!    sides and each drive runs the instructions it is owed.

use crate::cbm_iec::{IecBus, IecLines, IecOutput};
use crate::cia::Cia6526;
use crate::clock::{ClockBridge, VideoStandard};
use crate::cpu::Bus;
use crate::drive1541::{self, Drive1541, DriveError};
use crate::memory::{Ram, Rom};
use crate::sid::{Sid, SidModel};
use crate::vic::{Vic, VicMem, VicModel};

use super::keyboard::Keyboard;
use super::pla::{self, Region};
use super::serial_port;

/// Most drives the serial bus is wired for here (device numbers 8-11).
pub const MAX_DRIVES: usize = 4;

/// A drive on the serial bus, with the bridge from the C64's clock to its
/// own.
#[derive(Debug)]
struct SerialDevice {
    drive: Drive1541,
    clock: ClockBridge,
}

#[derive(Debug)]
pub struct Board {
    standard: VideoStandard,
    ram: Ram<65536>,
    basic: Rom<8192>,
    kernal: Rom<8192>,
    chargen: Rom<4096>,
    color_ram: Ram<1024>,
    vic: Vic,
    sid: Sid,
    cia1: Cia6526,
    cia2: Cia6526,
    keyboard: Keyboard,
    /// The 6510's port pins (LORAM, HIRAM, CHAREN for the PLA).
    port_pins: u8,
    serial_bus: IecBus,
    drives: Vec<SerialDevice>,
    /// CIA2's interrupt output, last cycle; RESTORE only makes an edge
    /// while it is released.
    nmi_line: bool,
    /// An NMI edge waiting for the CPU.
    nmi_edge: bool,
    /// The instruction the CPU is executing (for the VIC-II's c-accesses
    /// while the CPU still owns the bus).
    cpu_pc: u16,
}

impl Board {
    pub fn new(basic: Rom<8192>, kernal: Rom<8192>, chargen: Rom<4096>, standard: VideoStandard) -> Self {
        let vic_model = match standard {
            VideoStandard::Pal => VicModel::Pal,
            VideoStandard::Ntsc => VicModel::Ntsc,
        };
        Board {
            standard,
            ram: Ram::new(),
            basic,
            kernal,
            chargen,
            color_ram: Ram::new(),
            vic: Vic::new(vic_model),
            sid: Sid::new(SidModel::Mos6581, standard.clock_hz()),
            cia1: Cia6526::new(),
            cia2: Cia6526::new(),
            keyboard: Keyboard::new(),
            // All port bits are inputs at power-on and read high.
            port_pins: 0xFF,
            serial_bus: IecBus::new(),
            drives: Vec::new(),
            nmi_line: false,
            nmi_edge: false,
            cpu_pc: 0,
        }
    }

    pub fn standard(&self) -> VideoStandard {
        self.standard
    }

    /// RAM, regardless of what the CPU currently sees there.
    pub fn peek_ram(&self, addr: u16) -> u8 {
        self.ram.read(addr)
    }

    pub fn vic(&self) -> &Vic {
        &self.vic
    }

    pub fn sid(&self) -> &Sid {
        &self.sid
    }

    pub fn sid_mut(&mut self) -> &mut Sid {
        &mut self.sid
    }

    /// Replace the SID by another model, keeping the audio sample rate.
    pub fn set_sid_model(&mut self, model: SidModel) {
        let rate = self.sid.sample_rate();
        self.sid = Sid::new(model, self.standard.clock_hz());
        self.sid.set_sample_rate(rate);
    }

    pub fn cia1(&self) -> &Cia6526 {
        &self.cia1
    }

    pub fn cia2(&self) -> &Cia6526 {
        &self.cia2
    }

    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }

    /// The RESTORE key: a one-shot pulse on the NMI line.
    pub fn press_restore(&mut self) {
        if !self.nmi_line {
            self.nmi_edge = true;
        }
    }

    // ---- serial bus ----

    /// Connect a 1541 as device `device_number`; returns its index.
    pub fn attach_drive(&mut self, dos_rom: &[u8], device_number: u8) -> Result<usize, DriveError> {
        assert!(self.drives.len() < MAX_DRIVES, "at most {MAX_DRIVES} drives");
        let drive = Drive1541::new(dos_rom, device_number)?;
        let clock = ClockBridge::new(drive1541::CLOCK_HZ, self.standard.clock_hz());
        self.drives.push(SerialDevice { drive, clock });
        Ok(self.drives.len() - 1)
    }

    pub fn drive_count(&self) -> usize {
        self.drives.len()
    }

    pub fn drive(&self, index: usize) -> Option<&Drive1541> {
        self.drives.get(index).map(|d| &d.drive)
    }

    pub fn drive_mut(&mut self, index: usize) -> Option<&mut Drive1541> {
        self.drives.get_mut(index).map(|d| &mut d.drive)
    }

    /// The serial bus lines as of the last cycle.
    pub fn serial_bus(&self) -> IecLines {
        self.serial_bus.lines()
    }

    /// Recompute the bus from everyone's outputs and present it to everyone,
    /// then let each drive catch up with the C64's clock.
    fn clock_serial_bus(&mut self) {
        let computer = serial_port::bus_output(self.cia2.port_a.pins());
        let mut outputs = [IecOutput::default(); MAX_DRIVES];
        for (out, d) in outputs.iter_mut().zip(&self.drives) {
            *out = d.drive.bus_output(computer.atn);
        }
        self.serial_bus.update(computer, &outputs[..self.drives.len()]);
        let lines = self.serial_bus.lines();
        self.cia2.port_a.set_input(serial_port::port_input(lines));
        for d in &mut self.drives {
            d.drive.receive_bus(lines);
        }
        for d in &mut self.drives {
            d.drive.run_from_host(&mut d.clock);
        }
    }

    // ---- video ----

    /// Advance the VIC-II one cycle; returns RDY.
    fn clock_vic(&mut self) -> bool {
        // CIA2 PA0-1 select the bank, inverted.
        let bank = (3 - (self.cia2.port_a.pins() & 0x03) as u16) * 0x4000;
        let view = VicMem {
            ram: self.ram.as_array(),
            chargen: self.chargen.as_array(),
            color: self.color_ram.as_array(),
            bank,
            cpu_nibble: self.ram.read(self.cpu_pc) & 0x0F,
        };
        self.vic.tick(&view)
    }

    // ---- I/O ----

    /// CIA1's ports see the keyboard as it is wired to their outputs.
    fn scan_keyboard(&mut self) {
        let rows = self.keyboard.rows(self.cia1.port_b.driven_low());
        let columns = self.keyboard.columns(self.cia1.port_a.driven_low());
        self.cia1.port_a.set_input(rows);
        self.cia1.port_b.set_input(columns);
    }

    fn read_io(&mut self, addr: u16) -> u8 {
        match addr {
            0xD000..=0xD3FF => self.vic.read((addr & 0x3F) as usize),
            0xD400..=0xD7FF => self.sid.read((addr & 0x1F) as u8),
            0xD800..=0xDBFF => self.color_ram.read(addr) | 0xF0,
            0xDC00..=0xDCFF => {
                self.scan_keyboard();
                self.cia1.read((addr & 0x0F) as u8)
            }
            0xDD00..=0xDDFF => self.cia2.read((addr & 0x0F) as u8),
            _ => 0xFF,
        }
    }

    fn write_io(&mut self, addr: u16, val: u8) {
        match addr {
            0xD000..=0xD3FF => self.vic.write((addr & 0x3F) as usize, val),
            0xD400..=0xD7FF => self.sid.write((addr & 0x1F) as u8, val),
            0xD800..=0xDBFF => self.color_ram.write(addr, val & 0x0F),
            0xDC00..=0xDCFF => self.cia1.write((addr & 0x0F) as u8, val),
            0xDD00..=0xDDFF => self.cia2.write((addr & 0x0F) as u8, val),
            _ => {}
        }
    }
}

impl Bus for Board {
    fn read(&mut self, addr: u16) -> u8 {
        match pla::cpu_read(self.port_pins, addr) {
            Region::Ram => self.ram.read(addr),
            Region::Basic => self.basic.read(addr - 0xA000),
            Region::Kernal => self.kernal.read(addr - 0xE000),
            Region::CharRom => self.chargen.read(addr - 0xD000),
            Region::Io => self.read_io(addr),
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        if pla::cpu_write_is_io(self.port_pins, addr) {
            self.write_io(addr, val);
        } else {
            self.ram.write(addr, val);
        }
    }

    fn tick(&mut self) -> bool {
        let rdy = self.clock_vic();
        self.cia1.clock();
        self.cia2.clock();
        let nmi = self.cia2.interrupt();
        if nmi && !self.nmi_line {
            self.nmi_edge = true;
        }
        self.nmi_line = nmi;
        self.sid.clock();
        if !self.drives.is_empty() {
            self.clock_serial_bus();
        }
        rdy
    }

    fn note_pc(&mut self, pc: u16) {
        self.cpu_pc = pc;
    }

    fn processor_port_changed(&mut self, pins: u8) {
        self.port_pins = pins;
    }

    /// CIA1 and the VIC-II share the IRQ line.
    fn irq_pending(&self) -> bool {
        self.cia1.interrupt() || self.vic.irq_line()
    }

    fn nmi_edge_pending(&self) -> bool {
        self.nmi_edge
    }

    fn take_nmi_edge(&mut self) -> bool {
        std::mem::take(&mut self.nmi_edge)
    }
}
