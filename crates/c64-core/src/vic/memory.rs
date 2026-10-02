// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! What the VIC-II sees of memory.

/// The VIC-II's view of memory for one cycle: its 16 KB bank of RAM (with
/// the character ROM at $1000-$1FFF in banks 0 and 2), the colour RAM,
/// and the data bus while the CPU still owns it.
pub struct VicMem<'a> {
    pub ram: &'a [u8; 65536],
    pub chargen: &'a [u8; 4096],
    pub color: &'a [u8; 1024],
    /// Bank base address: $0000, $4000, $8000 or $C000.
    pub bank: u16,
    /// Low nibble of the byte at the CPU's program counter: what a
    /// c-access reads as colour during the first 3 cycles of BA low, when
    /// the CPU still owns the bus (VICE models it the same way).
    pub cpu_nibble: u8,
}

impl VicMem<'_> {
    #[inline]
    pub(super) fn read(&self, addr14: u16) -> u8 {
        let a = addr14 & 0x3FFF;
        if (self.bank == 0x0000 || self.bank == 0x8000) && (0x1000..0x2000).contains(&a) {
            self.chargen[(a - 0x1000) as usize]
        } else {
            self.ram[(self.bank | a) as usize]
        }
    }

    #[inline]
    pub(super) fn color(&self, vc: u16) -> u8 {
        self.color[(vc & 0x3FF) as usize] & 0x0F
    }
}
