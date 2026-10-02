// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! # c64-core
//!
//! A cycle-exact Commodore 64 with a 1541 disk drive, in plain Rust with
//! no dependencies, so it runs natively (tests, tools) as well as in the
//! browser through the `c64-wasm` crate.
//!
//! Author: R.F. van Ee. Written with the help of Claude (Anthropic), an
//! AI assistant, used for code generation, refactoring and testing; the
//! design, review and testing on real software are the author's.
//!
//! ## Organisation
//!
//! Each chip or unit has its own module, and the two machines are built
//! from them:
//!
//! | Module          | What                                                  |
//! |-----------------|-------------------------------------------------------|
//! | [`cpu`]         | MOS 6502 core, and the 6510 (6502 + I/O port)         |
//! | [`vic`]         | VIC-II video chip                                     |
//! | [`sid`]         | SID sound chip                                        |
//! | [`cia`]         | 6526 CIA                                              |
//! | [`via`]         | 6522 VIA                                              |
//! | [`port`]        | the parallel port shared by the CIA and the VIA       |
//! | [`memory`]      | RAM and ROM                                           |
//! | [`clock`]       | system clocks and the bridge between two clock domains|
//! | [`led`]         | indicator LEDs                                        |
//! | [`cbm_iec`]     | the Commodore serial bus                              |
//! | [`media`]       | floppy disks and the D64/G64 image formats            |
//! | [`c64`]         | the C64: board, PLA, keyboard, serial port            |
//! | [`drive1541`]   | the 1541: mechanics and electronics                   |
//! | [`tools`]       | helpers for tests and examples (BASIC tokenizer)      |
//!
//! Chips that came in several versions ("flavours") are one type with a
//! model enum chosen at construction: [`vic::VicModel`] (6569 PAL /
//! 6567R8 NTSC), [`sid::SidModel`] (6581 / 8580), [`clock::VideoStandard`]
//! and [`led::LedColor`]. Memories of different sizes are one generic
//! type, [`memory::Ram`]`<N>` and [`memory::Rom`]`<N>`. Everything a chip
//! needs from the rest of the machine goes through the [`cpu::Bus`] trait
//! (for the CPUs) or plain method calls from the board that owns it, so
//! the 6502 core is shared by the C64's 6510 and the 1541.
//!
//! ## Example
//!
//! ```no_run
//! use c64_core::c64::{C64, SystemRoms};
//! use c64_core::clock::VideoStandard;
//!
//! let rom = |name: &str| std::fs::read(format!("roms/{name}")).unwrap();
//! let (basic, kernal, chargen) = (rom("basic.rom"), rom("kernal.rom"), rom("chargen.rom"));
//! let roms = SystemRoms { basic: &basic, kernal: &kernal, chargen: &chargen };
//! let mut c64 = C64::new(roms, VideoStandard::Pal).unwrap();
//! c64.run_cycles(3_000_000); // to READY.
//! let picture: &[u8] = c64.framebuffer(); // RGBA
//! ```

pub mod c64;
pub mod cbm_iec;
pub mod cia;
pub mod clock;
pub mod cpu;
pub mod drive1541;
pub mod led;
pub mod media;
pub mod memory;
pub mod port;
pub mod sid;
pub mod tools;
pub mod via;
pub mod vic;

pub use c64::{SystemRoms, C64};
pub use clock::VideoStandard;
pub use drive1541::Drive1541;
