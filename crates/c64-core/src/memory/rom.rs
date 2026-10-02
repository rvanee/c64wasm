// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Read-only memory of exactly `N` bytes.

use std::fmt;

/// A ROM image that doesn't fit its socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomError {
    pub name: &'static str,
    pub expected: usize,
    pub got: usize,
}

impl fmt::Display for RomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ROM must be {} bytes, got {}", self.name, self.expected, self.got)
    }
}

impl std::error::Error for RomError {}

/// `N` bytes of ROM. Writes to a ROM's address range are ignored by the
/// boards that contain one.
#[derive(Debug, Clone)]
pub struct Rom<const N: usize> {
    bytes: Box<[u8; N]>,
}

impl<const N: usize> Rom<N> {
    /// A ROM from an image; `name` is used in the error for a wrong size.
    pub fn new(name: &'static str, image: &[u8]) -> Result<Self, RomError> {
        let bytes: Box<[u8; N]> = image.to_vec().into_boxed_slice().try_into().map_err(|_| RomError {
            name,
            expected: N,
            got: image.len(),
        })?;
        Ok(Rom { bytes })
    }

    #[inline]
    pub fn read(&self, offset: u16) -> u8 {
        self.bytes[offset as usize % N]
    }

    pub fn as_array(&self) -> &[u8; N] {
        &self.bytes
    }
}
