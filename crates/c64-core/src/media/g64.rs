// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! G64 images: the raw GCR bytes of up to 84 half-tracks, as written by
//! disk-copying hardware and other emulators.

use super::{Disk, HALF_TRACK_SLOTS};

impl Disk {
    /// Parse a G64 image (single-sided, `"GCR-1541"` signature), per
    /// Schepers' G64.TXT: an 8-byte signature, version byte, track-count
    /// byte, max-track-size u16 LE, a track-count-sized table of 4-byte LE
    /// absolute offsets (0 = no data for that half-track), then an
    /// identically-sized table of 4-byte LE speed-zone values, then each
    /// track's data at its offset as a 2-byte LE length prefix followed by
    /// that many raw GCR bytes.
    pub fn from_g64(bytes: &[u8]) -> Result<Disk, String> {
        if bytes.len() < 12 || &bytes[0..8] != b"GCR-1541" {
            return Err("not a GCR-1541 G64 image (bad signature)".to_string());
        }
        let n_tracks = bytes[9] as usize;
        if n_tracks > HALF_TRACK_SLOTS {
            return Err(format!(
                "G64 image declares {n_tracks} half-tracks, more than the {HALF_TRACK_SLOTS} this project supports"
            ));
        }
        let offset_table = 12;
        let zone_table = offset_table + n_tracks * 4;
        let read_u32 = |at: usize| -> Result<u32, String> {
            let b = bytes.get(at..at + 4).ok_or("G64 image truncated inside a table")?;
            Ok(u32::from_le_bytes(b.try_into().unwrap()))
        };

        let mut disk = Disk::blank();
        for slot in 0..n_tracks {
            let track_offset = read_u32(offset_table + slot * 4)? as usize;
            if track_offset == 0 {
                continue; // no data stored for this half-track
            }
            let zone_raw = read_u32(zone_table + slot * 4)?;
            let zone = (zone_raw & 0b11) as u8;
            let len_bytes = bytes.get(track_offset..track_offset + 2).ok_or("G64 track data offset out of range")?;
            let len = u16::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
            let data = bytes
                .get(track_offset + 2..track_offset + 2 + len)
                .ok_or("G64 track data length runs past end of file")?
                .to_vec();
            disk.tracks[slot] = Some(data);
            disk.zones[slot] = Some(zone);
        }
        Ok(disk)
    }

    /// Serialize this disk into a G64 image: the same container format
    /// `from_g64` parses (signature, version, half-track count, max track
    /// size, an offset table, a speed-zone table, then each track's data as
    /// a 2-byte length prefix followed by its raw GCR bytes), the exact
    /// inverse of `from_g64`. A half-track with no data gets a zero offset
    /// and a zero zone entry, matching how a real drive's G64 dump
    /// represents "nothing captured here".
    pub fn to_g64(&self) -> Vec<u8> {
        let n_slots = HALF_TRACK_SLOTS;
        let max_track_len = self.tracks.iter().flatten().map(|t| t.len()).max().unwrap_or(0) as u16;

        let mut out = Vec::new();
        out.extend_from_slice(b"GCR-1541");
        out.push(0x00); // version
        out.push(n_slots as u8);
        out.extend_from_slice(&max_track_len.to_le_bytes());

        let header_len = 12 + n_slots * 4 * 2;
        let offset_table = 12;
        let zone_table = offset_table + n_slots * 4;
        out.resize(header_len, 0); // offset/zone tables, filled in below

        for slot in 0..n_slots {
            if let Some(track) = &self.tracks[slot] {
                let file_offset = out.len() as u32;
                out[offset_table + slot * 4..offset_table + slot * 4 + 4].copy_from_slice(&file_offset.to_le_bytes());
                let zone = self.zones[slot].unwrap_or(0) as u32;
                out[zone_table + slot * 4..zone_table + slot * 4 + 4].copy_from_slice(&zone.to_le_bytes());
                out.extend_from_slice(&(track.len() as u16).to_le_bytes());
                out.extend_from_slice(track);
            }
            // else: offset and zone entries stay 0, already zero-filled above.
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::gcr;

    #[test]
    fn from_g64_parses_signature_and_a_single_track() {
        // Build a minimal, hand-rolled G64 with data at half-track 1 (a
        // true half-track -- whole tracks live at the *even* half-track
        // numbers, per `HALF_TRACK_SLOTS`'s doc comment; half-track 1 is
        // simply the lowest addressable slot, convenient for a minimal
        // fixture) so the parser is exercised against the actual container
        // format, not just against `from_d64`'s own output.
        let track_bytes = gcr::SYNC_MARK.to_vec();
        let n_tracks = 2usize; // just enough slots to hold half-track 1
        let mut g64 = Vec::new();
        g64.extend_from_slice(b"GCR-1541");
        g64.push(0x00); // version
        g64.push(n_tracks as u8);
        g64.extend_from_slice(&7928u16.to_le_bytes());
        let header_len = 12 + n_tracks * 4 * 2;
        let track_data_offset = header_len as u32;
        // offset table: slot 0 (half-track 1) has data, slot 1 doesn't.
        g64.extend_from_slice(&track_data_offset.to_le_bytes());
        g64.extend_from_slice(&0u32.to_le_bytes());
        // speed-zone table: slot 0 = zone 3.
        g64.extend_from_slice(&3u32.to_le_bytes());
        g64.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(g64.len(), header_len);
        g64.extend_from_slice(&(track_bytes.len() as u16).to_le_bytes());
        g64.extend_from_slice(&track_bytes);

        let disk = Disk::from_g64(&g64).unwrap();
        assert_eq!(disk.half_track_data(1), Some(track_bytes.as_slice()));
        assert_eq!(disk.half_track_zone(1), Some(3));
        assert_eq!(disk.half_track_data(2), None);
    }

    #[test]
    fn from_g64_rejects_bad_signature() {
        assert!(Disk::from_g64(b"not-a-g64-file-at-all-but-long-enough").is_err());
    }

    #[test]
    fn to_g64_round_trips_through_from_g64() {
        let original = Disk::from_d64(&std::fs::read("test-disks/hello.d64").unwrap()).unwrap();
        let reloaded = Disk::from_g64(&original.to_g64()).unwrap();
        for track in 1..=35u8 {
            assert_eq!(
                reloaded.track_data(track),
                original.track_data(track),
                "track {track} GCR bytes must survive the G64 round trip"
            );
            let half_track = 2 * track - 1;
            assert_eq!(reloaded.half_track_zone(half_track), original.half_track_zone(half_track));
        }
        // A true (odd-numbered) half-track -- never populated by
        // `from_d64`, which only ever writes whole tracks at half-track
        // `2*t` -- must round-trip as "no data", not as a zero-length
        // buffer.
        assert_eq!(reloaded.half_track_data(1), None);
    }
}
