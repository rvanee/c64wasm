// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Static or dynamic RAM of `N` bytes.

/// `N` bytes of RAM, zero at power-on. Addresses wrap at `N`, like a chip
/// with fewer address lines than the bus it sits on (the 1541's 2 KB RAM
/// appears several times in its address space).
#[derive(Debug, Clone)]
pub struct Ram<const N: usize> {
    bytes: Box<[u8; N]>,
}

impl<const N: usize> Default for Ram<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Ram<N> {
    pub fn new() -> Self {
        Ram { bytes: vec![0u8; N].into_boxed_slice().try_into().unwrap() }
    }

    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        self.bytes[addr as usize % N]
    }

    #[inline]
    pub fn write(&mut self, addr: u16, val: u8) {
        self.bytes[addr as usize % N] = val;
    }

    pub fn as_array(&self) -> &[u8; N] {
        &self.bytes
    }
}
