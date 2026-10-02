// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The four ROM sockets the emulator needs, and the real Commodore chips
// that fit them, identified by CRC32.

import { crc32 } from './crc32.js';

export const SLOTS = {
  kernal:  { label: 'KERNAL',        size: 8192,  base: 0xE000, db: 'kernal' },
  basic:   { label: 'BASIC',         size: 8192,  base: 0xA000, db: 'basic' },
  chargen: { label: 'Character ROM', size: 4096,  base: null,   db: 'chargen' },
  dos:     { label: '1541 DOS',      size: 16384, base: 0xC000, db: 'dos1541' },
};

// CRC32s of known dumps (MAME/VICE naming). The 1541's two 8 KB halves were
// separate chips on the original drive and are identified separately.
export const KNOWN = {
  kernal: { dce782fa: '901227-01 (KERNAL rev 1)', a5c687b3: '901227-02 (KERNAL rev 2)', dbe3e7c7: '901227-03 (KERNAL rev 3)' },
  basic: { f833d117: '901226-01 (BASIC V2)' },
  chargen: { ec4272ee: '901225-01 (character ROM)' },
  dosLow: { '29ae9752': '325302-01' },
  dosHigh: { '9a48d3f0': '901229-01', b29bab75: '901229-02', '9126e74a': '901229-03', '361c9f37': '901229-05', '3a235039': '901229-06' },
  dos: { '1b3ca08d': '251968-01 (1541-II)', '2d862d20': '251968-02 (1541-II)', '899fa3c5': '251968-03 (1541-II)', '57224cde': '355640-01 (1541-II)' },
};

/// A readable identification of an image for a slot: the chip number, or
/// why it isn't one.
export function identify(slot, bytes) {
  if (!bytes) return 'not loaded';
  if (bytes.length !== SLOTS[slot].size) return `wrong size (${bytes.length} bytes)`;
  const crc = crc32(bytes);
  if (slot === 'dos') {
    if (KNOWN.dos[crc]) return KNOWN.dos[crc];
    const lo = KNOWN.dosLow[crc32(bytes.subarray(0, 8192))];
    const hi = KNOWN.dosHigh[crc32(bytes.subarray(8192))];
    if (lo && hi) return `${lo} + ${hi} (1541)`;
    return `unknown version (CRC32 ${crc})`;
  }
  return KNOWN[slot][crc] ?? `unknown version (CRC32 ${crc})`;
}

/// Whether `identify` named a real chip.
export const isKnown = (slot, bytes) => !/unknown|wrong|not loaded/.test(identify(slot, bytes));

/// The checks an image for `slot` can pass: one or more layouts, each a list
/// of regions with the CRCs allowed there.
export function layouts(slot) {
  const nums = (o) => Object.keys(o).map(h => parseInt(h, 16));
  if (slot === 'dos') return [
    [{ off: 0, len: 8192, crcs: nums(KNOWN.dosLow) }, { off: 8192, len: 8192, crcs: nums(KNOWN.dosHigh) }],
    [{ off: 0, len: 16384, crcs: nums(KNOWN.dos) }],
  ];
  return [[{ off: 0, len: SLOTS[slot].size, crcs: nums(KNOWN[slot]) }]];
}
