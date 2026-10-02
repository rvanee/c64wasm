// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! MOS 65xx processors.
//!
//! - [`Mos6502`]: the core, used as is by the 1541.
//! - [`Mos6510`]: the C64's variant, the core plus an I/O port at $00/$01.
//! - [`Bus`]: what a processor is wired to.

mod addressing;
mod bus;
mod execute;
mod mos6502;
mod mos6510;
mod ops;
#[cfg(test)]
mod tests;

pub use bus::Bus;
pub use mos6502::{Mos6502, FLAG_B, FLAG_C, FLAG_D, FLAG_I, FLAG_N, FLAG_U, FLAG_V, FLAG_Z};
pub use mos6510::{Mos6510, ProcessorPort};
