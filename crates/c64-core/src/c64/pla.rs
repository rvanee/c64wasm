// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The PLA (906114-01): decides what the CPU sees at each address, from the
//! 6510 port's LORAM, HIRAM and CHAREN lines. Cartridge lines (GAME,
//! EXROM) are not modelled: they are always high, as with no cartridge.
//!
//! | Range         | Visible                                    |
//! |---------------|--------------------------------------------|
//! | `$A000-$BFFF` | BASIC if LORAM and HIRAM, else RAM          |
//! | `$D000-$DFFF` | RAM if LORAM and HIRAM are both low; else the character ROM if CHAREN is low; else I/O |
//! | `$E000-$FFFF` | KERNAL if HIRAM, else RAM                   |
//! | the rest      | RAM                                         |
//!
//! Writes always go to RAM, except to I/O when I/O is visible: the ROMs
//! have RAM underneath.

/// 6510 port bits wired to the PLA.
pub const LORAM: u8 = 0x01;
pub const HIRAM: u8 = 0x02;
pub const CHAREN: u8 = 0x04;

/// What answers a CPU access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Ram,
    Basic,
    Kernal,
    CharRom,
    Io,
}

/// The region the CPU reads at `addr`, given the 6510 port's pin levels.
#[inline]
pub fn cpu_read(port: u8, addr: u16) -> Region {
    let loram = port & LORAM != 0;
    let hiram = port & HIRAM != 0;
    let charen = port & CHAREN != 0;
    match addr {
        0xA000..=0xBFFF if loram && hiram => Region::Basic,
        0xD000..=0xDFFF if loram || hiram => {
            if charen {
                Region::Io
            } else {
                Region::CharRom
            }
        }
        0xE000..=0xFFFF if hiram => Region::Kernal,
        _ => Region::Ram,
    }
}

/// Whether a CPU write to `addr` goes to I/O (otherwise it goes to RAM).
#[inline]
pub fn cpu_write_is_io(port: u8, addr: u16) -> bool {
    cpu_read(port, addr) == Region::Io
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_on_shows_basic_io_and_kernal() {
        assert_eq!(cpu_read(0xFF, 0xA000), Region::Basic);
        assert_eq!(cpu_read(0xFF, 0xD020), Region::Io);
        assert_eq!(cpu_read(0xFF, 0xFFFC), Region::Kernal);
        assert_eq!(cpu_read(0xFF, 0xC000), Region::Ram);
    }

    #[test]
    fn the_usual_configurations() {
        // $36: BASIC out, KERNAL and I/O in.
        assert_eq!(cpu_read(0x36, 0xA000), Region::Ram);
        assert_eq!(cpu_read(0x36, 0xD000), Region::Io);
        assert_eq!(cpu_read(0x36, 0xE000), Region::Kernal);
        // $33: character ROM instead of I/O.
        assert_eq!(cpu_read(0x33, 0xD000), Region::CharRom);
        // $34: everything RAM.
        assert_eq!(cpu_read(0x34, 0xD000), Region::Ram);
        assert_eq!(cpu_read(0x30, 0xD000), Region::Ram);
        // $35: only I/O.
        assert_eq!(cpu_read(0x35, 0xD000), Region::Io);
        assert_eq!(cpu_read(0x35, 0xE000), Region::Ram);
    }

    #[test]
    fn writes_under_rom_go_to_ram() {
        assert!(!cpu_write_is_io(0x37, 0xA000));
        assert!(!cpu_write_is_io(0x33, 0xD000));
        assert!(cpu_write_is_io(0x37, 0xD000));
    }
}
