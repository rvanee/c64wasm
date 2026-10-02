// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Tests of ROM rebuilding: `node --test web/test`. The tests that need real
// Commodore ROMs (in roms/, never committed) skip themselves without them.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { crc32, solveBytes } from '../src/roms/crc32.js';
import { parseListing } from '../src/roms/listing.js';
import { buildImage } from '../src/roms/repair.js';
import { parseFontText } from '../src/roms/chargen.js';
import { fromText } from '../src/roms/sources.js';
import { isKnown } from '../src/roms/slots.js';

const bytesAt = (parsed, from, n) => Array.from({ length: n }, (_, i) => parsed.map.get(from + i));

test('crc32 of the standard check string', () => {
  assert.equal(crc32(new TextEncoder().encode('123456789')), 'cbf43926');
});

test('missing bytes are solved from the checksum', () => {
  const data = Uint8Array.from({ length: 1000 }, (_, i) => (i * 37 + 11) & 0xFF);
  const crc = crc32(data);
  const damaged = data.slice();
  damaged[5] = 0; damaged[500] = 0; damaged[999] = 0;
  assert.deepEqual(solveBytes(damaged, [5, 500, 999], [crc]), data);
});

test('listing shapes', () => {
  const p = parseListing([
    'E000  85 56     STA $56',
    '.,E002 20 0F BC JSR $BC0F',
    '$E005: A5 61',
    '0010  E007  C9 88   CMP #$88',
    'C100 78 SEI',
    '    C101  A9 F7  LDA #$F7',
    'SETLDA  C103  2D 00 1C  AND $1C00',
    'FFE6 .WD $C8C6 ; format',
    'FFCF .BY $AA,$55,$01',
    '0000e010: 0102 0304  ....',
    '0000e020  10 11 12 13 14 15 16 17  18 19 1a 1b 1c 1d 1e 1f  |................|',
    'E100  12 return without gosub', // a comment after the bytes
  ].join('\n'));
  assert.deepEqual(bytesAt(p, 0xE000, 2), [0x85, 0x56]);
  assert.deepEqual(bytesAt(p, 0xE002, 3), [0x20, 0x0F, 0xBC]);
  assert.deepEqual(bytesAt(p, 0xE005, 2), [0xA5, 0x61]);
  assert.deepEqual(bytesAt(p, 0xE007, 2), [0xC9, 0x88]);
  assert.deepEqual(bytesAt(p, 0xC100, 6), [0x78, 0xA9, 0xF7, 0x2D, 0x00, 0x1C]);
  assert.deepEqual(bytesAt(p, 0xFFE6, 2), [0xC6, 0xC8]);
  assert.deepEqual(bytesAt(p, 0xFFCF, 3), [0xAA, 0x55, 0x01]);
  assert.deepEqual(bytesAt(p, 0xE010, 4), [1, 2, 3, 4]);
  assert.deepEqual(bytesAt(p, 0xE020, 16), Array.from({ length: 16 }, (_, i) => 0x10 + i));
  assert.deepEqual(bytesAt(p, 0xE100, 2), [0x12, undefined]);
});

test('an elided range of one value', () => {
  const p = parseListing('C000   97\nC001   AA ...\nC0FF   ... AA\nC100   78  SEI');
  assert.equal(p.map.get(0xC000), 0x97);
  for (let a = 0xC001; a <= 0xC0FF; a++) assert.equal(p.map.get(a), 0xAA);
  assert.equal(p.map.get(0xC100), 0x78);
});

test('HTML tables become columns', () => {
  const html = '<html><body><table><tr><td>START</td><td>C100</td>\n<td>78</td><td>SEI</td></tr>'
    + '<tr><td></td><td>C101</td><td>A9 F7</td><td>LDA #$F7</td></tr></table></body></html>';
  const p = parseListing(html);
  assert.deepEqual(bytesAt(p, 0xC100, 3), [0x78, 0xA9, 0xF7]);
});

test('a font drawn as text', () => {
  const rows = [];
  for (let i = 0; i < 4096; i++) rows.push((i % 8 === 0 ? '#' : '.').repeat(8));
  const font = parseFontText(rows.join('\n'));
  assert.equal(font.length, 4096);
  assert.equal(font[0], 0xFF);
  assert.equal(font[1], 0x00);
});

test('gaps are reported', () => {
  const r = buildImage('kernal', parseListing('E000 01 02 03\nE010 04'));
  assert.equal(r.covered, 4);
  assert.equal(r.verified, null);
  assert.deepEqual(r.gaps[0], [0xE003, 0xE00F]);
});

// ---- with the real ROMs ----

const ROM_DIR = new URL('../../roms/', import.meta.url);

/// The ROM file if it is there and is a real Commodore chip.
function realRom(name, slot) {
  const url = new URL(name, ROM_DIR);
  if (!existsSync(url)) return null;
  const rom = new Uint8Array(readFileSync(url));
  return isKnown(slot, rom) ? rom : null;
}
const dos = realRom('1541.rom', 'dos');
const kernal = realRom('kernal.rom', 'kernal');

/// A listing of a ROM image in the plain "ADDR  BB BB BB  XXX" form.
function listing(rom, base, from = base) {
  const lines = [];
  for (let a = from; a < base + rom.length; a += 3) {
    const n = Math.min(3, base + rom.length - a);
    lines.push(`${a.toString(16).toUpperCase()}  ${Array.from(rom.subarray(a - base, a - base + n), b => b.toString(16).toUpperCase().padStart(2, '0')).join(' ')}  XXX`);
  }
  return lines;
}

test('1541 listing without its unused first page, and with a mistyped line', { skip: !dos && 'no 1541 ROM' }, () => {
  const rom = dos;
  const lines = listing(rom, 0xC000, 0xC100);
  // A typo: line $D003 says $D004 (overlapping its neighbour, leaving a hole).
  const i = lines.findIndex(l => l.startsWith('D003'));
  lines[i] = lines[i].replace('D003', 'D004');
  const r = fromText('dos', lines.join('\n'));
  assert.ok(r.verified, r.repairWhy);
  assert.deepEqual(r.bytes, rom);
});

test('KERNAL listing with a byte left out', { skip: !kernal && 'no known KERNAL ROM' }, () => {
  const rom = kernal;
  const lines = listing(rom, 0xE000);
  lines[100] = lines[100].replace(/ [0-9A-F]{2}  XXX$/, '  XXX'); // one byte gone: solved from the CRC
  const r = fromText('kernal', lines.join('\n'));
  assert.ok(r.verified, r.repairWhy);
  assert.deepEqual(r.bytes, rom);
});
