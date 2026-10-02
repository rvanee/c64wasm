// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Disk images: recognising D64/G64 files, and reading a D64's name and ID.

export const D64_SIZE = 174848;            // 35 tracks
export const D64_SIZE_WITH_ERRORS = 175531; // plus an error byte per sector
const BAM_OFFSET = 357 * 256;              // track 18, sector 0

export const isG64 = (bytes) => bytes.length >= 8 && String.fromCharCode(...bytes.subarray(0, 8)) === 'GCR-1541';
export const isD64 = (bytes) => bytes.length === D64_SIZE || bytes.length === D64_SIZE_WITH_ERRORS;

/// The disk's name and ID from its BAM, as PETSCII bytes, or null.
export function diskLabel(bytes) {
  if (!bytes || !isD64(bytes)) return null;
  const trim = (a) => { let e = a.length; while (e > 0 && (a[e - 1] === 0xA0 || a[e - 1] === 0x20)) e--; return a.subarray(0, e); };
  return {
    name: trim(bytes.subarray(BAM_OFFSET + 0x90, BAM_OFFSET + 0xA0)),
    id: bytes.subarray(BAM_OFFSET + 0xA2, BAM_OFFSET + 0xA4),
  };
}
