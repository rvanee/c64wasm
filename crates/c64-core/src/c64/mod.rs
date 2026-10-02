// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The Commodore 64: a 6510 on the [`Board`].
//!
//! - [`board`]: memory, the I/O chips and how they are clocked.
//! - [`pla`]: the memory configurations.
//! - [`keyboard`]: the key matrix on CIA1.
//! - [`serial_port`]: CIA2's wiring to the serial bus (disk drives).

pub mod board;
pub mod keyboard;
pub mod pla;
pub mod serial_port;

pub use board::Board;

use crate::cbm_iec::IecLines;
use crate::clock::VideoStandard;
use crate::cpu::{Bus, Mos6510};
use crate::drive1541::{Drive1541, DriveError};
use crate::media::Disk;
use crate::memory::{Rom, RomError};
use crate::sid::{Sid, SidModel};
use crate::vic::Vic;

/// Keyboard buffer and its length, used by [`C64::type_text`].
const KEYBOARD_BUFFER: u16 = 0x0277;
const KEYBOARD_BUFFER_LEN: u16 = 0x00C6;
const KEYBOARD_BUFFER_SIZE: usize = 10;

/// The three system ROMs.
pub struct SystemRoms<'a> {
    pub basic: &'a [u8],
    pub kernal: &'a [u8],
    pub chargen: &'a [u8],
}

#[derive(Debug)]
pub struct C64 {
    cpu: Mos6510,
    board: Board,
    /// Cycles run by [`C64::step`] since power-on.
    cycles: u64,
}

impl C64 {
    /// Power on: the CPU starts at the KERNAL's reset vector.
    pub fn new(roms: SystemRoms, standard: VideoStandard) -> Result<Self, RomError> {
        let board = Board::new(
            Rom::new("BASIC", roms.basic)?,
            Rom::new("KERNAL", roms.kernal)?,
            Rom::new("character", roms.chargen)?,
            standard,
        );
        let mut c64 = C64 { cpu: Mos6510::new(), board, cycles: 0 };
        let lo = c64.read(0xFFFC) as u16;
        let hi = c64.read(0xFFFD) as u16;
        c64.cpu.pc = hi << 8 | lo;
        Ok(c64)
    }

    /// Run one instruction; returns the cycles it took (including cycles
    /// the VIC-II stole).
    pub fn step(&mut self) -> u32 {
        let n = self.cpu.step(&mut self.board);
        self.cycles += n as u64;
        n
    }

    /// Cycles since power-on.
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// Run until the cycle count reaches `cycle` (whole instructions, so
    /// it can pass it by a few) or the CPU jams.
    pub fn run_until(&mut self, cycle: u64) {
        while self.cycles < cycle && !self.cpu.jammed {
            self.step();
        }
    }

    /// Run at least `cycles` cycles, or until the CPU jams. Returns the
    /// cycles run.
    pub fn run_cycles(&mut self, cycles: u64) -> u64 {
        let start = self.cycles;
        self.run_until(start + cycles);
        self.cycles - start
    }

    pub fn cpu(&self) -> &Mos6510 {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Mos6510 {
        &mut self.cpu
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    pub fn jammed(&self) -> bool {
        self.cpu.jammed
    }

    /// Read as the CPU would (I/O reads can have side effects).
    pub fn read(&mut self, addr: u16) -> u8 {
        if addr < 2 {
            self.cpu.read_port(addr)
        } else {
            self.board.read(addr)
        }
    }

    /// Write as the CPU would.
    pub fn write(&mut self, addr: u16, val: u8) {
        if addr < 2 {
            self.cpu.write_port(&mut self.board, addr, val);
        } else {
            self.board.write(addr, val);
        }
    }

    /// RAM, regardless of the memory configuration.
    pub fn peek_ram(&self, addr: u16) -> u8 {
        self.board.peek_ram(addr)
    }

    /// Type PETSCII text (`\r` is RETURN) through the KERNAL's keyboard
    /// buffer, as fast as the screen editor takes it: 10 characters at a
    /// time, running the machine until the buffer is empty again. Needs the
    /// KERNAL's interrupt handler running (after boot). Returns `false` if
    /// the buffer didn't empty within 30 seconds of emulated time.
    pub fn type_text(&mut self, petscii: &[u8]) -> bool {
        let limit = 30 * self.clock_hz() as u64;
        for chunk in petscii.chunks(KEYBOARD_BUFFER_SIZE) {
            let start = self.cycles;
            while self.peek_ram(KEYBOARD_BUFFER_LEN) != 0 {
                if self.jammed() || self.cycles - start > limit {
                    return false;
                }
                self.run_cycles(2000);
            }
            for (i, &c) in chunk.iter().enumerate() {
                self.write(KEYBOARD_BUFFER + i as u16, c);
            }
            self.write(KEYBOARD_BUFFER_LEN, chunk.len() as u8);
        }
        true
    }

    // ---- keyboard ----

    /// Press or release the key at `row` (CIA1 port A line) and `col` (port
    /// B line); see [`keyboard`] for the matrix.
    pub fn set_key(&mut self, row: usize, col: usize, pressed: bool) {
        self.board.keyboard_mut().set_key(row, col, pressed);
    }

    pub fn release_all_keys(&mut self) {
        self.board.keyboard_mut().release_all();
    }

    pub fn press_restore(&mut self) {
        self.board.press_restore();
    }

    // ---- video and sound ----

    pub fn standard(&self) -> VideoStandard {
        self.board.standard()
    }

    pub fn clock_hz(&self) -> u32 {
        self.board.standard().clock_hz()
    }

    pub fn vic(&self) -> &Vic {
        self.board.vic()
    }

    /// The picture as RGBA, the whole raster (see [`Self::framebuffer_size`]).
    pub fn framebuffer(&self) -> &[u8] {
        self.board.vic().framebuffer()
    }

    pub fn framebuffer_size(&self) -> (u32, u32) {
        let model = self.board.vic().model();
        (model.width() as u32, model.height() as u32)
    }

    pub fn sid(&self) -> &Sid {
        self.board.sid()
    }

    pub fn sid_mut(&mut self) -> &mut Sid {
        self.board.sid_mut()
    }

    pub fn set_sid_model(&mut self, model: SidModel) {
        self.board.set_sid_model(model);
    }

    /// Append the audio since the last call (mono, about -1..1).
    pub fn take_audio(&mut self, out: &mut Vec<f32>) {
        self.board.sid_mut().take_samples(out);
    }

    // ---- disk drives ----

    /// Connect a 1541 as device 8-11; returns its index.
    pub fn attach_drive(&mut self, dos_rom: &[u8], device_number: u8) -> Result<usize, DriveError> {
        self.board.attach_drive(dos_rom, device_number)
    }

    pub fn drive_count(&self) -> usize {
        self.board.drive_count()
    }

    pub fn drive(&self, index: usize) -> Option<&Drive1541> {
        self.board.drive(index)
    }

    pub fn drive_mut(&mut self, index: usize) -> Option<&mut Drive1541> {
        self.board.drive_mut(index)
    }

    /// Insert a D64 or G64 image into a drive.
    pub fn insert_disk(&mut self, drive: usize, image: &[u8]) -> Result<(), String> {
        let disk = Disk::from_image(image)?;
        self.drive_mut(drive).ok_or_else(|| format!("no drive {drive}"))?.insert_disk(disk);
        Ok(())
    }

    pub fn eject_disk(&mut self, drive: usize) -> Option<Disk> {
        self.drive_mut(drive)?.eject_disk()
    }

    /// The disk in a drive as a D64 image, without ejecting it.
    pub fn extract_disk(&self, drive: usize) -> Option<Vec<u8>> {
        Some(self.drive(drive)?.disk()?.to_d64())
    }

    pub fn serial_bus(&self) -> IecLines {
        self.board.serial_bus()
    }
}
