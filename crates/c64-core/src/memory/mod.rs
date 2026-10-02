// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! RAM and ROM chips.

mod ram;
mod rom;

pub use ram::Ram;
pub use rom::{Rom, RomError};
