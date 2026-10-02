// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// CRC32 (as used for ROM dumps by MAME and VICE), and the algebra that lets
// missing bytes be solved for from a known CRC.
//
// For messages of a fixed length CRC32 is affine over GF(2): crc(a) ^ crc(b)
// depends only on a ^ b, and is the XOR of one fixed 32-bit vector per
// flipped bit. So a few unknown bytes can be solved for from a known ROM's
// CRC, and thousands of "which reading of this listing is right" hypotheses
// can be checked without re-running the CRC over 8 KB each time.

export const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

/// CRC32 as an 8-digit lower-case hex string.
export function crc32(bytes) {
  let c = 0xFFFFFFFF;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xFF] ^ (c >>> 8);
  return ((c ^ 0xFFFFFFFF) >>> 0).toString(16).padStart(8, '0');
}

const columnCache = new Map();

/// For a message of `len` bytes: cols[p * 8 + j] is the change in the CRC
/// when bit j of byte p flips.
export function crcColumns(len) {
  if (columnCache.has(len)) return columnCache.get(len);
  const cols = new Uint32Array(len * 8);
  for (let j = 0; j < 8; j++) {
    let s = CRC_TABLE[1 << j];
    for (let p = len - 1; p >= 0; p--) {
      cols[p * 8 + j] = s;
      s = (CRC_TABLE[s & 0xFF] ^ (s >>> 8)) >>> 0;
    }
  }
  columnCache.set(len, cols);
  return cols;
}

/// The change in the CRC when byte `p` is XORed with `diff`.
export function crcDelta(cols, p, diff) {
  let s = 0;
  for (let j = 0; j < 8; j++) if ((diff >> j) & 1) s ^= cols[p * 8 + j];
  return s >>> 0;
}

/// Row-reduce up to 24 columns. Returns null if they're linearly dependent
/// (a solution then wouldn't be unique).
export function gf2Basis(columns) {
  const basis = new Array(32).fill(null);
  for (let k = 0; k < columns.length; k++) {
    let v = columns[k] >>> 0, m = 1 << k, placed = false;
    for (let b = 31; b >= 0 && v; b--) {
      if (!((v >>> b) & 1)) continue;
      if (basis[b]) { v = (v ^ basis[b][0]) >>> 0; m ^= basis[b][1]; }
      else { basis[b] = [v, m]; placed = true; break; }
    }
    if (!placed) return null;
  }
  return basis;
}

/// The x for which the XOR of columns[k] over the set bits k of x equals
/// `rhs`, or -1.
export function gf2Solve(basis, rhs) {
  let r = rhs >>> 0, x = 0;
  for (let b = 31; b >= 0; b--) {
    if (!((r >>> b) & 1)) continue;
    if (!basis[b]) return -1;
    r = (r ^ basis[b][0]) >>> 0; x ^= basis[b][1];
  }
  return x;
}

/// Fill the bytes at `positions` (at most 3) of `bytes` so that its CRC32
/// becomes one of `crcs` (hex strings); null if impossible or ambiguous.
export function solveBytes(bytes, positions, crcs) {
  const cols = crcColumns(bytes.length);
  const columns = [];
  for (const p of positions) for (let j = 0; j < 8; j++) columns.push(cols[p * 8 + j]);
  const basis = gf2Basis(columns);
  if (!basis) return null;
  const current = parseInt(crc32(bytes), 16);
  const hits = [];
  for (const c of crcs) {
    const x = gf2Solve(basis, (current ^ parseInt(c, 16)) >>> 0);
    if (x < 0) continue;
    const out = bytes.slice();
    positions.forEach((p, k) => { out[p] ^= (x >>> (k * 8)) & 0xFF; });
    hits.push(out);
  }
  return hits.length === 1 ? hits[0] : null;
}
