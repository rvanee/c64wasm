// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Floppy disk media: the raw GCR bit stream of every half-track, as the
//! drive's read/write head sees it. Images are converted to and from this
//! form by `d64` (plain sector images) and `g64` (raw GCR captures).
//!
//! Track layout and the four speed zones are standard community facts
//! (see `docs/1541.md`). At 300 rpm a track holds `bit_rate * 0.2 / 8`
//! bytes, e.g. 7692 for zone 3; D64 tracks are padded to that.

mod d64;
mod g64;
pub mod gcr;

/// Number of (half-)track slots kept, matching the G64 format's own slot
/// count (42 whole tracks x 2). Index `h` (0-based) holds half-track
/// `h + 1`; a whole track `t` (1-35 for a standard disk) lives at index
/// `2 * t - 1`.
pub const HALF_TRACK_SLOTS: usize = 84;

/// Sectors per track for the standard 35-track layout, or `None` outside
/// 1-35 (see `docs/1541.md`: tracks beyond 35 are physically
/// reachable but outside what any stock DOS formats or reads, so this
/// project doesn't synthesize or interpret data for them).
pub fn sectors_per_track(track: u8) -> Option<u8> {
    match track {
        1..=17 => Some(21),
        18..=24 => Some(19),
        25..=30 => Some(18),
        31..=35 => Some(17),
        _ => None,
    }
}

/// The 2-bit density/speed-zone code (VIA2 PB6-5) for a track, per the
/// confirmed zone table.
pub fn speed_zone(track: u8) -> Option<u8> {
    match track {
        1..=17 => Some(0b11),
        18..=24 => Some(0b10),
        25..=30 => Some(0b01),
        31..=35 => Some(0b00),
        _ => None,
    }
}

/// Bit rate in bits/second for each of the 4 speed zones, indexed by the
/// same 2-bit code `speed_zone` returns.
pub fn zone_bit_rate(zone: u8) -> u32 {
    match zone & 0b11 {
        0b11 => 307_692,
        0b10 => 285_714,
        0b01 => 266_667,
        0b00 => 250_000,
        _ => unreachable!(),
    }
}

fn whole_track_index(track: u8) -> usize {
    2 * track as usize - 1
}

#[derive(Debug, Clone)]
pub struct Disk {
    /// Raw GCR byte buffer for each half-track slot (`None` = no data --
    /// either never formatted, an unformatted half-track between real
    /// tracks, or simply not part of this image).
    tracks: Vec<Option<Vec<u8>>>,
    /// Speed zone (0-3) recorded per slot, alongside its data.
    zones: Vec<Option<u8>>,
    pub write_protected: bool,
}

impl Disk {
    /// An unformatted disk.
    pub fn blank() -> Disk {
        Disk { tracks: vec![None; HALF_TRACK_SLOTS], zones: vec![None; HALF_TRACK_SLOTS], write_protected: false }
    }

    /// A disk from an image file: G64 if it starts with the `GCR-1541`
    /// signature, D64 otherwise.
    pub fn from_image(bytes: &[u8]) -> Result<Disk, String> {
        if bytes.starts_with(b"GCR-1541") {
            Disk::from_g64(bytes)
        } else {
            Disk::from_d64(bytes)
        }
    }

    /// The raw GCR bytes for whole track `track` (1-based), if present.
    pub fn track_data(&self, track: u8) -> Option<&[u8]> {
        self.tracks.get(whole_track_index(track))?.as_deref()
    }

    /// The raw GCR bytes for a half-track slot (1-based half-track number,
    /// so `2*track - 1` is whole track `track`), if present.
    pub fn half_track_data(&self, half_track: u8) -> Option<&[u8]> {
        self.tracks.get(half_track as usize - 1)?.as_deref()
    }

    pub fn half_track_zone(&self, half_track: u8) -> Option<u8> {
        *self.zones.get(half_track as usize - 1)?
    }

    /// Mutable access to a half-track's raw GCR bytes, for the drive
    /// mechanics' write path. `None` if that half-track has no data at all:
    /// the head only writes where the disk has a track buffer (every D64
    /// has one for tracks 1-35, so formatting and saving work; see
    /// `docs/1541.md`). The returned buffer is the same bytes
    /// `track_data`/`half_track_data` return.
    pub fn half_track_data_mut(&mut self, half_track: u8) -> Option<&mut Vec<u8>> {
        self.tracks.get_mut(half_track as usize - 1)?.as_mut()
    }

    fn set_whole_track(&mut self, track: u8, data: Vec<u8>, zone: u8) {
        let idx = whole_track_index(track);
        self.tracks[idx] = Some(data);
        self.zones[idx] = Some(zone);
    }

    // ---- D64 -------------------------------------------------------------
}
