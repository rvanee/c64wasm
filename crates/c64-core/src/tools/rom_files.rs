// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The Commodore ROM images, read from a directory (`roms/` in this
//! repository; see `roms/README.md`). They are copyrighted and not part of
//! the repository, so tests skip themselves when they are missing.

use std::io;
use std::path::{Path, PathBuf};

use crate::c64::SystemRoms;

/// `basic.rom`, `kernal.rom`, `chargen.rom` and, if present, `1541.rom`.
#[derive(Debug, Clone)]
pub struct RomFiles {
    pub basic: Vec<u8>,
    pub kernal: Vec<u8>,
    pub chargen: Vec<u8>,
    pub dos1541: Option<Vec<u8>>,
}

impl RomFiles {
    pub fn load(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref();
        let read = |name: &str| std::fs::read(dir.join(name));
        Ok(RomFiles {
            basic: read("basic.rom")?,
            kernal: read("kernal.rom")?,
            chargen: read("chargen.rom")?,
            dos1541: read("1541.rom").ok(),
        })
    }

    /// The repository's `roms/` directory, found from the crate directory
    /// or the current directory; `None` if the system ROMs aren't there.
    pub fn from_repository() -> Option<Self> {
        let mut candidates = vec![PathBuf::from("roms")];
        if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
            candidates.push(Path::new(manifest).join("../../roms"));
        }
        candidates.into_iter().find_map(|dir| Self::load(dir).ok())
    }

    pub fn system(&self) -> SystemRoms<'_> {
        SystemRoms { basic: &self.basic, kernal: &self.kernal, chargen: &self.chargen }
    }
}
