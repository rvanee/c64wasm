// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! D64 images: 683 sectors of 256 bytes (optionally followed by 683
//! error bytes), converted to GCR tracks with synthesized headers and gaps,
//! and back (by finding and decoding the sectors on each track).

use super::gcr;
use super::{sectors_per_track, speed_zone, zone_bit_rate, Disk};

const GAP1_LEN: usize = 9;
const GAP2_LEN: usize = 4;
const GAP_FILL: u8 = 0x55;

impl Disk {
    /// Build a sector-address -> byte-offset table for a standard 35-track
    /// D64, per Schepers' D64.TXT.
    fn d64_offset(track: u8, sector: u8) -> usize {
        let mut sectors_before = 0usize;
        for t in 1..track {
            sectors_before += sectors_per_track(t).unwrap() as usize;
        }
        (sectors_before + sector as usize) * 256
    }

    /// Parse a D64 image (35-track, with or without the trailing 683-byte
    /// error-info block; 40/42-track extensions are rejected with an error
    /// rather than silently truncated, since this project doesn't
    /// implement anything beyond the standard 35-track layout).
    pub fn from_d64(bytes: &[u8]) -> Result<Disk, String> {
        const SIZE_35: usize = 174_848;
        const SIZE_35_ERR: usize = SIZE_35 + 683;
        if bytes.len() != SIZE_35 && bytes.len() != SIZE_35_ERR {
            return Err(format!(
                "unsupported D64 size {} bytes (expected {SIZE_35} or {SIZE_35_ERR} for a standard 35-track image)",
                bytes.len()
            ));
        }

        // The disk ID (used in every sector header, and checked by the real
        // DOS against what it reads from the BAM) lives at $A2/$A3 of the
        // BAM sector, track 18 sector 0 -- confirmed directly from
        // Schepers' D64.TXT.
        let bam_offset = Self::d64_offset(18, 0);
        let bam = &bytes[bam_offset..bam_offset + 256];
        let (id1, id2) = (bam[0xA2], bam[0xA3]);

        let mut disk = Disk::blank();
        for track in 1..=35u8 {
            let n_sectors = sectors_per_track(track).unwrap();
            let zone = speed_zone(track).unwrap();
            let mut track_gcr = Vec::new();
            for sector in 0..n_sectors {
                let offset = Self::d64_offset(track, sector);
                let data: [u8; 256] = bytes[offset..offset + 256].try_into().unwrap();
                track_gcr.extend_from_slice(&synth_sector(track, sector, id1, id2, &data));
            }
            pad_to_zone_capacity(&mut track_gcr, zone);
            disk.set_whole_track(track, track_gcr, zone);
        }
        Ok(disk)
    }

    /// Decode this disk's tracks back into a standard 174848-byte D64
    /// image. Any sector that fails to decode (bad sync, bad checksum, or
    /// simply missing because that track was never written) is emitted as
    /// 256 zero bytes -- matching how a real disk editor shows an unreadable
    /// sector, rather than failing the whole export.
    pub fn to_d64(&self) -> Vec<u8> {
        let mut out = vec![0u8; 174_848];
        for track in 1..=35u8 {
            let n_sectors = sectors_per_track(track).unwrap();
            let sectors = self.track_data(track).map(|buf| decode_track_sectors(buf, n_sectors)).unwrap_or_default();
            for sector in 0..n_sectors {
                if let Some(data) = sectors.iter().find_map(|(h, d)| (h.sector == sector).then_some(d)) {
                    let offset = Self::d64_offset(track, sector);
                    out[offset..offset + 256].copy_from_slice(data);
                }
            }
        }
        out
    }
}

/// Build one sector's full on-wire byte sequence: sync, header block (GCR),
/// GAP1, sync, data block (GCR), GAP2.
fn synth_sector(track: u8, sector: u8, id1: u8, id2: u8, data: &[u8; 256]) -> Vec<u8> {
    let mut out = Vec::with_capacity(358);
    out.extend_from_slice(&gcr::SYNC_MARK);
    out.extend_from_slice(&gcr::gcr_encode_blocks(&gcr::header_block_bytes(track, sector, id2, id1)));
    out.extend(std::iter::repeat_n(GAP_FILL, GAP1_LEN));
    out.extend_from_slice(&gcr::SYNC_MARK);
    out.extend_from_slice(&gcr::gcr_encode_blocks(&gcr::data_block_bytes(data)));
    out.extend(std::iter::repeat_n(GAP_FILL, GAP2_LEN));
    out
}

/// Pad a synthesized track out to this zone's total physical byte
/// capacity at 300RPM (see the module doc comment for the arithmetic),
/// with more `GAP_FILL` bytes forming the final "index" gap. A no-op if
/// the sectors already used more than the nominal capacity (they
/// shouldn't, for a standard 35-track disk, but this never truncates real
/// data).
fn pad_to_zone_capacity(track_gcr: &mut Vec<u8>, zone: u8) {
    let bytes_per_track = (zone_bit_rate(zone) as f64 * 0.2 / 8.0) as usize;
    if track_gcr.len() < bytes_per_track {
        track_gcr.resize(bytes_per_track, GAP_FILL);
    }
}

/// Read the bit at `bit_index` (0 = MSB of byte 0) out of a byte buffer.
fn read_bit(bytes: &[u8], bit_index: usize) -> u8 {
    let byte_idx = bit_index / 8;
    let bit_idx = 7 - (bit_index % 8) as u8;
    (bytes[byte_idx] >> bit_idx) & 1
}

/// Pack `num_bytes * 8` consecutive bits starting at `start_bit` (MSB
/// first, matching `mechanics.rs`'s own bit-cell convention) into a fresh,
/// byte-aligned buffer, or `None` if that window runs past the end of
/// `bytes`. A real GCR bitstream has no inherent byte alignment of its
/// own -- a sector written by the drive's write-mode mechanics lands at
/// whatever arbitrary bit offset the head happened to be at when writing
/// began (see `mechanics.rs`'s module doc comment), so a sector's sync
/// mark and data very often do *not* start on a byte boundary of this
/// buffer's own indexing. Re-aligning to a fresh byte buffer here is what
/// lets `gcr::gcr_decode_blocks` (which only operates on already
/// byte-aligned GCR bytes) be used regardless of where in the real
/// bitstream a block actually starts.
fn extract_bits_as_bytes(bytes: &[u8], start_bit: usize, num_bytes: usize) -> Option<Vec<u8>> {
    let total_bits = bytes.len() * 8;
    if start_bit + num_bytes * 8 > total_bits {
        return None;
    }
    let mut out = vec![0u8; num_bytes];
    for i in 0..num_bytes * 8 {
        if read_bit(bytes, start_bit + i) != 0 {
            out[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    Some(out)
}

/// Scan a raw track byte buffer for sync marks and decode every
/// header+data block pair found, returning them keyed by the header's
/// sector number. Tolerant of the track being a *loop* (a sync mark's
/// header/data can start near the end of the buffer and wrap to the
/// beginning) by scanning a doubled copy and de-duplicating by start
/// position modulo the real length.
///
/// Scans at the *bit* level, not the byte level: a sector written by the
/// drive's actual write-mode mechanics can start at any bit offset (see
/// `extract_bits_as_bytes`'s doc comment), so a scan that only ever checks
/// whether a whole byte equals literal `$FF` misses every sync mark that
/// isn't coincidentally byte-aligned -- which, for a freshly-written
/// sector, is the common case, not the exception (Task 9 finding: this is
/// *not* what made the LOAD/read path fail, since `Mechanics`'s own
/// `advance_one_bit` read branch already tracks `sync_run` bit-by-bit and
/// was never affected -- this byte-level scan only lived here, in the
/// offline `to_d64()`/export path). 10 consecutive 1-bits is the real
/// hardware minimum for a sync mark (matching `Mechanics::sync_input_bit`'s
/// own threshold) and is safe against false positives: valid 4-to-5 GCR
/// code is specifically designed so no run of valid codewords ever
/// produces 10 consecutive 1-bits.
fn decode_track_sectors(track: &[u8], _expected_sectors: u8) -> Vec<(gcr::Header, [u8; 256])> {
    if track.is_empty() {
        return Vec::new();
    }
    let len = track.len();
    let mut doubled = Vec::with_capacity(len * 2);
    doubled.extend_from_slice(track);
    doubled.extend_from_slice(track);
    let total_bits = len * 8;
    let doubled_bits = doubled.len() * 8;

    let mut out = Vec::new();
    let mut seen_starts = std::collections::HashSet::new();
    let mut bit = 0usize;
    while bit < total_bits {
        // Find the end of a run of >=10 consecutive 1-bits starting at or
        // after `bit` (the real hardware sync-mark minimum).
        if read_bit(&doubled, bit) == 0 {
            bit += 1;
            continue;
        }
        let run_start = bit;
        while bit < doubled_bits && read_bit(&doubled, bit) == 1 {
            bit += 1;
        }
        if bit - run_start < 10 {
            continue; // too short to be a real sync mark
        }
        let data_start_bit = bit % total_bits;
        if !seen_starts.insert(data_start_bit) {
            continue;
        }
        // Try to decode a header block right after this sync mark.
        if let Some(gcr_bytes) = extract_bits_as_bytes(&doubled, bit, 10) {
            if let Some(raw) = gcr::gcr_decode_blocks(&gcr_bytes) {
                let raw: [u8; 8] = raw.try_into().unwrap();
                if let Some(header) = gcr::parse_header_block(&raw) {
                    // Look for the next sync mark within a small window past
                    // the nominal GAP1 length (real gaps can vary a little),
                    // then decode the data block right after it. The gap
                    // filler ($55 = 0101_0101) has scattered individual
                    // 1-bits of its own, so this has to measure each
                    // candidate run's *full* length (same as the outer
                    // scan) rather than stopping at the first 1-bit seen --
                    // otherwise it mistakes a lone bit inside a gap byte for
                    // the start of the real sync mark.
                    let gap_search_end_bit = (bit + (10 + GAP1_LEN + 8) * 8).min(doubled_bits);
                    let mut j = bit + 10 * 8;
                    let mut dstart = None;
                    while j < gap_search_end_bit {
                        if read_bit(&doubled, j) == 0 {
                            j += 1;
                            continue;
                        }
                        let run_start = j;
                        while j < doubled_bits && read_bit(&doubled, j) == 1 {
                            j += 1;
                        }
                        if j - run_start >= 10 {
                            dstart = Some(j);
                            break;
                        }
                        // too short to be the real sync mark -- keep
                        // scanning from here (`j` is already past this
                        // short run).
                    }
                    if let Some(dstart) = dstart {
                        if let Some(dgcr) = extract_bits_as_bytes(&doubled, dstart, 325) {
                            if let Some(draw) = gcr::gcr_decode_blocks(&dgcr) {
                                let draw: [u8; 260] = draw.try_into().unwrap();
                                if let Some(payload) = gcr::parse_data_block(&draw) {
                                    out.push((header, payload));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_d64() -> Vec<u8> {
        let mut d64 = vec![0u8; 174_848];
        // Put recognizable, distinct content in a handful of sectors across
        // different speed zones -- deliberately *not* including (18, 0),
        // the BAM sector, so the disk-ID bytes set below aren't clobbered.
        for &(track, sector) in &[(1u8, 0u8), (1, 20), (18, 1), (25, 5), (35, 16)] {
            let off = Disk::d64_offset(track, sector);
            for i in 0..256 {
                d64[off + i] = track.wrapping_mul(7).wrapping_add(sector).wrapping_add(i as u8);
            }
        }
        // BAM: track 18 sector 0. Give it a recognizable disk ID, set last
        // so nothing above can overwrite it.
        let bam_off = Disk::d64_offset(18, 0);
        d64[bam_off + 0xA2] = b'A';
        d64[bam_off + 0xA3] = b'B';
        d64
    }

    /// Regression test for the Task 9 half-track translation bug (see
    /// `mechanics.rs`'s module doc comment): confirms a real D64's track 18
    /// decodes correctly and lands at storage half-track 36 (`2 * 18`),
    /// which is the exact half-track `Mechanics`'s physical-to-storage
    /// translation must land on for a real end-to-end directory read to
    /// find it.
    #[test]
    fn from_d64_track18_lands_at_half_track_36() {
        let bytes = std::fs::read("test-disks/hello.d64")
            .or_else(|_| std::fs::read("crates/c64-core/test-disks/hello.d64"))
            .expect("hello.d64 test disk");
        let disk = Disk::from_d64(&bytes).unwrap();
        let t18 = disk.track_data(18).expect("track 18 data present");
        let sectors = decode_track_sectors(t18, sectors_per_track(18).unwrap());
        assert!(!sectors.is_empty(), "track 18 must decode to at least one sector");
        assert!(sectors.iter().all(|(h, _)| h.track == 18), "every decoded header's track field must read 18");

        let via_half_track = disk.half_track_data(36).expect("half_track_data(36) must hold track 18's data");
        assert_eq!(via_half_track, t18, "half_track_data(36) must be exactly track_data(18)");
    }

    #[test]
    fn sectors_per_track_matches_confirmed_zone_table() {
        assert_eq!(sectors_per_track(1), Some(21));
        assert_eq!(sectors_per_track(17), Some(21));
        assert_eq!(sectors_per_track(18), Some(19));
        assert_eq!(sectors_per_track(24), Some(19));
        assert_eq!(sectors_per_track(25), Some(18));
        assert_eq!(sectors_per_track(30), Some(18));
        assert_eq!(sectors_per_track(31), Some(17));
        assert_eq!(sectors_per_track(35), Some(17));
        assert_eq!(sectors_per_track(36), None);
        assert_eq!(sectors_per_track(0), None);
    }

    #[test]
    fn d64_total_sector_count_is_683() {
        let total: u32 = (1..=35u8).map(|t| sectors_per_track(t).unwrap() as u32).sum();
        assert_eq!(total, 683, "35-track D64 must have exactly 683 sectors (174848 / 256)");
    }

    #[test]
    fn from_d64_rejects_wrong_size() {
        assert!(Disk::from_d64(&[0u8; 1000]).is_err());
    }

    #[test]
    fn from_d64_extracts_disk_id_into_every_header() {
        let disk = Disk::from_d64(&synthetic_d64()).unwrap();
        let track1 = disk.track_data(1).unwrap();
        let sectors = decode_track_sectors(track1, 21);
        assert_eq!(sectors.len(), 21, "all 21 sectors of track 1 must decode");
        for (header, _) in &sectors {
            assert_eq!((header.id1, header.id2), (b'A', b'B'));
            assert_eq!(header.track, 1);
        }
    }

    #[test]
    fn d64_round_trips_through_synthesis_and_back() {
        let original = synthetic_d64();
        let disk = Disk::from_d64(&original).unwrap();
        let round_tripped = disk.to_d64();
        assert_eq!(round_tripped.len(), original.len());
        assert_eq!(round_tripped, original, "every sector's 256 bytes must survive synth -> GCR -> decode unchanged");
    }

    #[test]
    fn each_zone_track_is_padded_to_its_own_capacity() {
        let disk = Disk::from_d64(&synthetic_d64()).unwrap();
        // Track 1 (zone 3, fastest) should end up with a larger raw byte
        // buffer than track 35 (zone 0, slowest), reflecting the higher bit
        // rate over the same 200ms rotation -- not just "however many bytes
        // the sectors needed".
        let t1_len = disk.track_data(1).unwrap().len();
        let t35_len = disk.track_data(35).unwrap().len();
        assert!(t1_len > t35_len, "zone 3 track ({t1_len} bytes) should be longer than zone 0 ({t35_len} bytes)");
    }
}
