// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The character ROM rebuilt from a drawing of the character set: either a
// text file with one row of 8 symbols per glyph line, or a picture of the
// glyphs in code order.

import { crc32, solveBytes } from './crc32.js';
import { KNOWN } from './slots.js';

/// A font drawn as text, 8 characters per glyph row, two symbols for
/// off/on (e.g. mobluse/chargen-maker's chargen.txt, with '\\' and '@', or
/// '.' and '#'). Lines starting with '# ' are comments. Returns the
/// 4096-byte character ROM, or null if the text isn't one.
export function parseFontText(text) {
  const rows = [];
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.replace(/\s+$/, '');
    if (!line || /^# /.test(line) || line === '#') continue;
    if (line.length !== 8) { if (rows.length) return null; continue; }
    rows.push(line);
  }
  if (rows.length !== 4096) return null;
  const counts = new Map();
  for (const r of rows) for (const ch of r) counts.set(ch, (counts.get(ch) || 0) + 1);
  if (counts.size !== 2) return null;
  const ink = [...counts.entries()].sort((x, y) => y[1] - x[1])[1][0]; // the rarer symbol
  const out = new Uint8Array(4096);
  rows.forEach((r, i) => { let v = 0; for (const ch of r) v = (v << 1) | (ch === ink ? 1 : 0); out[i] = v; });
  return out;
}

// A few glyphs of the C64 font, used only to find where the glyph grid of a
// picture starts and how far apart the glyphs are.
const GLYPH_AT = [0x3C, 0x66, 0x6E, 0x6E, 0x60, 0x62, 0x3C, 0x00];
const GLYPH_A = [0x18, 0x3C, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00];
const GLYPH_a = [0x00, 0x00, 0x3C, 0x06, 0x3E, 0x66, 0x3E, 0x00];

/// Rebuild the character ROM from pictures of the character set as a grid
/// of glyphs in code order (any integer zoom, with or without gaps between
/// glyphs, 8-64 glyphs per row): either all 512 glyphs, or the 128
/// non-reversed glyphs of each of the two sets (the reversed halves are
/// then generated). Several pictures are combined (e.g. one per set).
/// `images` are { data: RGBA bytes, width, height }. Returns
/// { bytes, verified, glyphs } or { error }; only a result matching the
/// real ROM's CRC32 is "verified".
export function chargenFromImages(images) {
  const glyphLists = [];
  for (const img of images) {
    const g = glyphsFromImage(img);
    if (g.error) return g;
    glyphLists.push(g.glyphs);
  }
  const all = glyphLists.flat();
  const tries = [];
  const pack = (glyphs) => {
    const out = new Uint8Array(4096);
    glyphs.slice(0, 512).forEach((gl, i) => out.set(gl, i * 8));
    return out;
  };
  const withReverse = (set1, set2) => {
    const g = [];
    for (const set of [set1, set2]) {
      for (let i = 0; i < 128; i++) g.push(set[i]);
      for (let i = 0; i < 128; i++) g.push(set[i].map(b => b ^ 0xFF));
    }
    return pack(g);
  };
  if (all.length >= 512) tries.push(pack(all));
  if (all.length >= 256) {
    tries.push(withReverse(all.slice(0, 128), all.slice(128, 256)));
    // one picture per set, each with its reversed half drawn too
    if (glyphLists.length === 2 && glyphLists[0].length >= 128 && glyphLists[1].length >= 128) {
      tries.push(withReverse(glyphLists[0], glyphLists[1]));
    }
  }
  for (const t of tries) {
    if (KNOWN.chargen[crc32(t)]) return { bytes: t, verified: KNOWN.chargen[crc32(t)], glyphs: all.length };
    // The real ROM's reversed "@" isn't quite the inverse of "@" (one row
    // differs, in both sets): solve those two bytes from the checksum.
    const fixed = solveBytes(t, [128 * 8 + 5, 384 * 8 + 5], Object.keys(KNOWN.chargen));
    if (fixed) return { bytes: fixed, verified: KNOWN.chargen[crc32(fixed)], glyphs: all.length };
  }
  if (!tries.length) return { error: `found only ${all.length} glyphs; need the 128 normal glyphs of both sets (or all 512)` };
  return { bytes: tries[0], verified: null, glyphs: all.length, error: `read ${all.length} glyphs, but they don't add up to the real character ROM` };
}

function glyphsFromImage({ data, width, height }) {
  // 1. Ink or paper by luminance (Otsu threshold); ink is the rarer class.
  const lum = new Uint8Array(width * height);
  const hist = new Array(256).fill(0);
  for (let i = 0; i < lum.length; i++) {
    const v = (data[i * 4] * 299 + data[i * 4 + 1] * 587 + data[i * 4 + 2] * 114) / 1000 | 0;
    lum[i] = v; hist[v]++;
  }
  let threshold = 128, bestVar = -1, sum = 0, sumB = 0, wB = 0;
  for (let t = 0; t < 256; t++) sum += t * hist[t];
  for (let t = 0; t < 256; t++) {
    wB += hist[t]; if (!wB) continue;
    const wF = lum.length - wB; if (!wF) break;
    sumB += t * hist[t];
    const mB = sumB / wB, mF = (sum - sumB) / wF, v = wB * wF * (mB - mF) ** 2;
    if (v > bestVar) { bestVar = v; threshold = t; }
  }
  const dark = lum.reduce((n, v) => n + (v <= threshold ? 1 : 0), 0);
  const inkDark = dark < lum.length / 2;
  const bit = (x, y) => { const v = lum[y * width + x] <= threshold; return inkDark ? v : !v; };
  // 2. Zoom factor: the GCD of horizontal run lengths.
  const gcd = (a, b) => (b ? gcd(b, a % b) : a);
  let zoom = 0;
  for (let y = 0; y < height; y += Math.max(1, height >> 6)) {
    let run = 1;
    for (let x = 1; x <= width; x++) {
      if (x < width && bit(x, y) === bit(x - 1, y)) { run++; continue; }
      if (x < width) zoom = gcd(zoom, run);
      run = 1;
    }
  }
  zoom = Math.max(1, zoom || 1);
  const w = Math.floor(width / zoom), h = Math.floor(height / zoom);
  const px = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) px[y * w + x] = bit(x * zoom + (zoom >> 1), y * zoom + (zoom >> 1)) ? 1 : 0;
  const glyphAt = (x0, y0) => {
    if (x0 < 0 || y0 < 0 || x0 + 8 > w || y0 + 8 > h) return null;
    const g = [];
    for (let r = 0; r < 8; r++) { let v = 0; for (let c = 0; c < 8; c++) v = (v << 1) | px[(y0 + r) * w + x0 + c]; g.push(v); }
    return g;
  };
  const same = (g, t) => g && g.every((v, i) => v === t[i]);
  // 3. Find '@' (code 0) and the 'A' after it: origin and horizontal pitch.
  for (let y0 = 0; y0 + 8 <= h; y0++) {
    for (let x0 = 0; x0 + 8 <= w; x0++) {
      if (!same(glyphAt(x0, y0), GLYPH_AT)) continue;
      for (let pitchX = 8; pitchX <= 24; pitchX++) {
        const next = glyphAt(x0 + pitchX, y0);
        if (!same(next, GLYPH_A) && !same(next, GLYPH_a)) continue;
        // 4. The vertical pitch is usually the horizontal one; otherwise
        // take the one whose cells look most like glyphs (most C64 glyphs
        // have a blank bottom row).
        const cols = Math.floor((w - x0 - 8) / pitchX) + 1;
        let pick = null;
        const pitches = [pitchX, ...Array.from({ length: 17 }, (_, i) => i + 8).filter(v => v !== pitchX)];
        for (const pitchY of pitches) {
          const rows = Math.floor((h - y0 - 8) / pitchY) + 1;
          const glyphs = [];
          for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) glyphs.push(glyphAt(x0 + c * pitchX, y0 + r * pitchY));
          const score = glyphs.reduce((n, g) => n + (g && g[7] === 0 ? 1 : 0), 0) / glyphs.length;
          if (pitchY === pitchX && glyphs.length >= 128 && score > 0.3) { pick = { score, glyphs }; break; }
          if (!pick || score > pick.score + 0.05) pick = { score, glyphs };
        }
        const glyphs = pick.glyphs;
        while (glyphs.length && glyphs[glyphs.length - 1].every(v => v === 0)) glyphs.pop(); // a partly filled last row
        return { glyphs, zoom };
      }
    }
  }
  return { error: 'could not find the character grid (looked for "@" followed by "A") in that picture' };
}

/// Decode a picture (a Blob or a URL) into what chargenFromImages takes.
export async function imageDataFrom(source) {
  const blob = source instanceof Blob ? source : await (await fetch(source)).blob();
  const bmp = await createImageBitmap(blob);
  const canvas = typeof OffscreenCanvas !== 'undefined'
    ? new OffscreenCanvas(bmp.width, bmp.height)
    : Object.assign(document.createElement('canvas'), { width: bmp.width, height: bmp.height });
  const ctx = canvas.getContext('2d');
  ctx.drawImage(bmp, 0, 0);
  return ctx.getImageData(0, 0, bmp.width, bmp.height);
}
