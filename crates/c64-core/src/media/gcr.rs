// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Commodore GCR (Group Coded Recording) encoding: the 4-bit-to-5-bit
//! nibble table used to turn arbitrary 8-bit bytes into a bitstream that's
//! guaranteed never to have more than two consecutive zero-bits (needed for
//! the floppy's self-clocking read circuitry to stay synchronized), plus
//! the sector header/data block layout built on top of it.
//!
//! The nibble table itself is not a 1541-specific secret -- it's the same
//! table used all the way back to the 4040/2040 IEEE-488 drives -- so
//! (unlike the VIA wiring in `via.rs`) it wasn't re-derived from the user's
//! ROM; it's cross-checked between Linus Åkesson's GCR write-up
//! (<https://www.linusakesson.net/programming/gcr-decoding/index.php>) and
//! pagetable.com's "Anatomy of the 4040 Disk Drive"
//! (<https://www.pagetable.com/docs/anatomy-4040.html>), which independently
//! list the same 16 values.
//!
//! Sector header/data block field order and checksums follow the
//! conventional layout documented across the community (also corroborated
//! by the 4040 Anatomy doc's byte-count arithmetic: 8 raw header bytes and
//! 260 raw data-block bytes both divide evenly into groups of 4, matching
//! the confirmed 10-GCR-byte header / 325-GCR-byte data block sizes) --
//! see `/docs/1541-plan.md` for what's independently confirmed vs. accepted
//! as a documented assumption. Because the ultimate test of "did we get the
//! exact byte order right" is whether the *real* 1541 DOS ROM can read a
//! disk this module wrote (see the end-to-end tests planned in
//! `/docs/1541-plan.md`), getting a field order wrong here would be caught
//! empirically, not just left as a silent risk.

/// The 16 valid 4-bit-to-5-bit GCR codes, indexed by the 4-bit nibble.
pub const ENCODE: [u8; 16] = [
    0b01010, 0b01011, 0b10010, 0b10011, 0b01110, 0b01111, 0b10110, 0b10111, 0b01001, 0b11001, 0b11010, 0b11011,
    0b01101, 0b11101, 0b11110, 0b10101,
];

/// Encode one 4-bit nibble (only the low 4 bits of `nibble` are used) into
/// its 5-bit GCR code (returned in the low 5 bits of the result).
pub fn encode_nibble(nibble: u8) -> u8 {
    ENCODE[(nibble & 0x0F) as usize]
}

/// Decode a 5-bit GCR code (low 5 bits of `code`) back to a 4-bit nibble,
/// or `None` if it's one of the 16 codes Commodore's table never produces
/// (a real drive reports these as a GCR/checksum error).
pub fn decode_nibble(code: u8) -> Option<u8> {
    decode_table()[(code & 0x1F) as usize]
}

/// Reverse lookup table, built once at compile time isn't possible without
/// `const fn` gymnastics for a 32-entry search, so it's built lazily via
/// `std::sync::OnceLock` -- cheap, and this crate has no `no_std` constraint.
fn decode_table() -> &'static [Option<u8>; 32] {
    static TABLE: std::sync::OnceLock<[Option<u8>; 32]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [None; 32];
        for (nibble, &code) in ENCODE.iter().enumerate() {
            t[code as usize] = Some(nibble as u8);
        }
        t
    })
}

/// Encode 4 data bytes (8 nibbles, high nibble of each byte first) into 5
/// GCR-coded bytes (40 bits, packed MSB-first), exactly the grouping a real
/// 1541 uses for both header and data blocks.
pub fn encode_4_to_5(src: &[u8; 4]) -> [u8; 5] {
    let mut bits: u64 = 0;
    let mut nbits = 0u32;
    for &byte in src {
        bits = (bits << 5) | encode_nibble(byte >> 4) as u64;
        nbits += 5;
        bits = (bits << 5) | encode_nibble(byte & 0x0F) as u64;
        nbits += 5;
    }
    debug_assert_eq!(nbits, 40);
    bits.to_be_bytes()[3..8].try_into().unwrap()
}

/// Decode 5 GCR-coded bytes back into the original 4 data bytes. Returns
/// `None` if any of the eight 5-bit groups isn't one of the 16 valid codes
/// (a real drive would report this sector as unreadable).
pub fn decode_5_to_4(src: &[u8; 5]) -> Option<[u8; 4]> {
    let mut bits: u64 = 0;
    for &b in src {
        bits = (bits << 8) | b as u64;
    }
    // 40 bits total, in the low 40 bits of `bits`.
    let mut nibbles = [0u8; 8];
    for (i, nibble) in nibbles.iter_mut().enumerate() {
        let shift = 40 - 5 * (i + 1);
        let code = ((bits >> shift) & 0x1F) as u8;
        *nibble = decode_nibble(code)?;
    }
    Some([
        (nibbles[0] << 4) | nibbles[1],
        (nibbles[2] << 4) | nibbles[3],
        (nibbles[4] << 4) | nibbles[5],
        (nibbles[6] << 4) | nibbles[7],
    ])
}

/// A raw (pre-GCR) sector header block: 8 bytes, encoded as two groups of
/// 4 via `encode_4_to_5`, for 10 GCR bytes total.
pub fn header_block_bytes(track: u8, sector: u8, id2: u8, id1: u8) -> [u8; 8] {
    let checksum = sector ^ track ^ id2 ^ id1;
    [0x08, checksum, sector, track, id2, id1, 0x0F, 0x0F]
}

/// Parsed contents of a decoded header block, or `None` if the block ID
/// byte or checksum didn't match (a real drive would report `21, READ
/// ERROR` for either).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub track: u8,
    pub sector: u8,
    pub id2: u8,
    pub id1: u8,
}

pub fn parse_header_block(bytes: &[u8; 8]) -> Option<Header> {
    let [id, checksum, sector, track, id2, id1, _off1, _off2] = *bytes;
    if id != 0x08 {
        return None;
    }
    if checksum != (sector ^ track ^ id2 ^ id1) {
        return None;
    }
    Some(Header { track, sector, id2, id1 })
}

/// A raw (pre-GCR) sector data block: 1 ID byte + 256 data bytes + 1
/// checksum + 2 off bytes = 260 bytes, encoded as 65 groups of 4 via
/// `encode_4_to_5`, for 325 GCR bytes total.
pub fn data_block_bytes(data: &[u8; 256]) -> [u8; 260] {
    let mut out = [0u8; 260];
    out[0] = 0x07;
    out[1..257].copy_from_slice(data);
    let checksum = data.iter().fold(0u8, |acc, &b| acc ^ b);
    out[257] = checksum;
    out[258] = 0x00;
    out[259] = 0x00;
    out
}

/// Parse a decoded data block back into its 256 payload bytes, or `None` if
/// the block ID or checksum didn't match.
pub fn parse_data_block(bytes: &[u8; 260]) -> Option<[u8; 256]> {
    if bytes[0] != 0x07 {
        return None;
    }
    let data: [u8; 256] = bytes[1..257].try_into().unwrap();
    let checksum = data.iter().fold(0u8, |acc, &b| acc ^ b);
    if bytes[257] != checksum {
        return None;
    }
    Some(data)
}

/// GCR-encode an arbitrary raw byte slice whose length is a multiple of 4,
/// by encoding each 4-byte group independently and concatenating the
/// results (5 GCR bytes per group).
pub fn gcr_encode_blocks(raw: &[u8]) -> Vec<u8> {
    assert_eq!(raw.len() % 4, 0, "GCR block encoding requires a multiple of 4 raw bytes");
    let mut out = Vec::with_capacity(raw.len() / 4 * 5);
    for group in raw.as_chunks::<4>().0 {
        out.extend_from_slice(&encode_4_to_5(group));
    }
    out
}

/// Inverse of `gcr_encode_blocks`. Returns `None` (rather than partial
/// output) if any 5-byte group fails to decode.
pub fn gcr_decode_blocks(gcr: &[u8]) -> Option<Vec<u8>> {
    assert_eq!(gcr.len() % 5, 0, "GCR block decoding requires a multiple of 5 GCR bytes");
    let mut out = Vec::with_capacity(gcr.len() / 5 * 4);
    for group in gcr.as_chunks::<5>().0 {
        out.extend_from_slice(&decode_5_to_4(group)?);
    }
    Some(out)
}

/// A raw (post-GCR) sync mark: 5 bytes of all 1-bits (40 consecutive 1s),
/// the length real Commodore firmware writes -- see `/docs/1541-plan.md`.
/// Sync marks are the one part of the bitstream that's deliberately *not*
/// GCR data (10 consecutive 1-bits can never occur inside valid GCR-encoded
/// data, which is exactly what lets the drive's hardware synchronizer find
/// them).
pub const SYNC_MARK: [u8; 5] = [0xFF; 5];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_valid_nibble_round_trips() {
        for n in 0u8..16 {
            let code = encode_nibble(n);
            assert!(code < 32);
            assert_eq!(decode_nibble(code), Some(n), "nibble {n:#x} -> code {code:#07b} should decode back");
        }
    }

    #[test]
    fn encode_table_has_no_duplicate_codes() {
        let mut seen = std::collections::HashSet::new();
        for &code in ENCODE.iter() {
            assert!(seen.insert(code), "GCR code {code:#07b} used for more than one nibble");
        }
    }

    #[test]
    fn invalid_five_bit_codes_fail_to_decode() {
        // Codes with three or more consecutive zero bits, or other patterns
        // Commodore's table never emits, must be rejected, not silently
        // mapped to something.
        let all_codes: std::collections::HashSet<u8> = ENCODE.iter().copied().collect();
        let mut saw_invalid = false;
        for code in 0u8..32 {
            if !all_codes.contains(&code) {
                saw_invalid = true;
                assert_eq!(decode_nibble(code), None);
            }
        }
        assert!(saw_invalid, "sanity: there should be invalid codes to test (32 - 16 = 16 of them)");
    }

    #[test]
    fn four_to_five_byte_round_trip() {
        let cases: [[u8; 4]; 4] =
            [[0x00, 0x00, 0x00, 0x00], [0xFF, 0xFF, 0xFF, 0xFF], [0x12, 0x34, 0x56, 0x78], [0xA5, 0x5A, 0x3C, 0xC3]];
        for src in cases {
            let gcr = encode_4_to_5(&src);
            let back = decode_5_to_4(&gcr).expect("valid GCR must decode");
            assert_eq!(back, src);
        }
    }

    #[test]
    fn header_block_round_trips_through_gcr() {
        let raw = header_block_bytes(18, 5, 0x32, 0x41);
        let gcr = gcr_encode_blocks(&raw);
        assert_eq!(gcr.len(), 10);
        let decoded_raw = gcr_decode_blocks(&gcr).unwrap();
        let decoded_raw: [u8; 8] = decoded_raw.try_into().unwrap();
        assert_eq!(decoded_raw, raw);
        let header = parse_header_block(&decoded_raw).expect("checksum must match");
        assert_eq!(header, Header { track: 18, sector: 5, id2: 0x32, id1: 0x41 });
    }

    #[test]
    fn corrupted_header_checksum_is_rejected() {
        let mut raw = header_block_bytes(1, 0, 0x30, 0x30);
        raw[2] = 5; // corrupt the sector field after the checksum was computed
        assert_eq!(parse_header_block(&raw), None);
    }

    #[test]
    fn data_block_round_trips_through_gcr() {
        let mut data = [0u8; 256];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i * 7 + 3) as u8;
        }
        let raw = data_block_bytes(&data);
        assert_eq!(raw.len(), 260);
        let gcr = gcr_encode_blocks(&raw);
        assert_eq!(gcr.len(), 325);
        let decoded_raw = gcr_decode_blocks(&gcr).unwrap();
        let decoded_raw: [u8; 260] = decoded_raw.try_into().unwrap();
        assert_eq!(decoded_raw, raw);
        let decoded_data = parse_data_block(&decoded_raw).expect("checksum must match");
        assert_eq!(decoded_data, data);
    }

    #[test]
    fn corrupted_data_checksum_is_rejected() {
        let data = [0xAAu8; 256];
        let mut raw = data_block_bytes(&data);
        raw[1] ^= 0xFF; // corrupt one data byte after the checksum was computed
        assert_eq!(parse_data_block(&raw), None);
    }

    #[test]
    fn sync_mark_is_not_a_valid_gcr_encoding_of_anything() {
        // A real sync mark (>= 10 consecutive 1-bits) can never appear
        // inside legitimately GCR-encoded data, because every valid 5-bit
        // code has at most... let's confirm the actual property used: no
        // valid code is all-1s, and no two valid codes concatenated produce
        // 10 consecutive 1-bits either, which is what lets hardware treat
        // "a long run of 1s" as unambiguously "this is a sync mark, not
        // data". Spot-check the concrete byte pattern this module emits.
        assert_eq!(SYNC_MARK, [0xFF; 5]);
        let all_ones_as_data = decode_5_to_4(&SYNC_MARK);
        assert_eq!(all_ones_as_data, None, "0b11111 must not be one of the 16 valid GCR codes");
    }
}
