// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Laying parsed listings out into a ROM image, and repairing the typos
// listings found on the web have, but only when the CRC32 of a real chip
// proves the repair.

import { crc32, crcColumns, crcDelta, gf2Basis, gf2Solve } from './crc32.js';
import { KNOWN, SLOTS, identify, isKnown, layouts } from './slots.js';

// Bytes where the published KERNAL listings (Lee Davison's commented
// disassembly, as used by mist64/c64ref, pagetable.com and their forks)
// differ from the real 901227-03 chip: the ROM checksum byte and two spots
// that carry the older revision's code. Tried as hypotheses, and only kept
// if the CRC then matches a real ROM.
const VARIANT_SITES = {
  kernal: [
    { addr: 0xE4AC, bytes: [0x81], why: 'checksum byte of 901227-03' },
    { addr: 0xE4DB, bytes: [0x86, 0x02], why: '901227-03 colours new characters from $0286' },
    { addr: 0xE622, bytes: [0x91, 0xE5], why: '901227-03 code' },
  ],
};

// Unused ROM space with a known fill, which some listings leave out.
const KNOWN_FILL = {
  dos: [{ from: 0xC001, to: 0xC0FF, value: 0xAA, why: 'unused space, $AA in every 1541 ROM' }],
};

const hex = (n, w = 4) => '$' + n.toString(16).toUpperCase().padStart(w, '0');

/// Try to turn an imperfect listing into a verified ROM region. Listings
/// found on the web have typos: a line with a mistyped address both
/// overwrites a neighbour and leaves a hole where it belonged, a line has
/// one byte too many, a few bytes are simply missing. Hypotheses tried:
///  - every "suspicious" line (one that overlaps another line of the same
///    listing, or that could fill a hole) is either where it says it is,
///    right after the previous line, or right before the next line;
///  - where lines disagree on a byte, either value;
///  - known listing-vs-chip differences (VARIANT_SITES);
///  - the remaining missing bytes are solved for from the CRC.
/// A result is only accepted if it is the single image that reproduces a
/// known ROM's CRC32, and the chance that a wrong image did so by luck,
/// summed over all hypotheses tried, stays below 1 in 4096.
function repairRegion(ctx, region) {
  const { rows, base, img: full, have: fullHave, srcClash, sites } = ctx;
  const { off, len, crcs } = region;
  if (!crcs.length) return { why: 'no known ROM to check against' };
  const lo = base + off;
  const img = full.slice(off, off + len);
  const have = fullHave.subarray(off, off + len);
  const cols = crcColumns(len);
  const inRegion = (a) => a >= lo && a < lo + len;

  const altsOf = (r) => {
    const s = new Set([r.addr]);
    if (r.prev) s.add(r.prev.addr + r.prev.bytes.length);
    if (r.next) s.add(r.next.addr - r.bytes.length);
    return [...s].filter(a => a + r.bytes.length > lo && a < lo + len);
  };
  const suspects = [];
  for (const r of rows) {
    const alts = altsOf(r);
    if (alts.length < 2 || !alts.includes(r.addr)) continue;
    let suspicious = false;
    for (let i = 0; i < r.bytes.length && !suspicious; i++) {
      const o = r.addr + i - base;
      if (o >= 0 && o < srcClash[r.src].length && srcClash[r.src][o]) suspicious = true;
    }
    for (const a of alts) {
      for (let i = 0; i < r.bytes.length && !suspicious; i++) if (inRegion(a + i) && !have[a + i - lo]) suspicious = true;
    }
    if (suspicious) suspects.push({ r, alts: [r.addr, ...alts.filter(a => a !== r.addr)] });
  }
  // A line with a mistyped address often has a neighbour with one too:
  // also doubt lines that could move into a doubtful line's place.
  const doubtful = new Set();
  for (const { r } of suspects) for (let i = 0; i < r.bytes.length; i++) doubtful.add(r.addr + i);
  const already = new Set(suspects.map(s => s.r));
  for (const r of rows) {
    if (already.has(r)) continue;
    const alts = altsOf(r);
    if (alts.length < 2 || !alts.includes(r.addr)) continue;
    if (alts.some(a => a !== r.addr && r.bytes.some((_, i) => doubtful.has(a + i)))) {
      suspects.push({ r, alts: [r.addr, ...alts.filter(a => a !== r.addr)] });
    }
  }
  const siteList = (sites || []).filter(st => inRegion(st.addr) && st.bytes.some((b, i) => !have[st.addr + i - lo] || img[st.addr + i - lo] !== b));

  // Offsets whose value depends on the hypotheses.
  const affected = new Set();
  for (const { r, alts } of suspects) for (const a of alts) for (let i = 0; i < r.bytes.length; i++) if (inRegion(a + i)) affected.add(a + i - lo);
  for (const st of siteList) st.bytes.forEach((_, i) => affected.add(st.addr + i - lo));
  for (const a of ctx.conflicts) if (inRegion(a)) affected.add(a - lo);
  const originalConflicts = new Set(ctx.conflicts.filter(inRegion).map(a => a - lo));
  const affectedList = [...affected].sort((x, y) => x - y);
  const fixedGaps = [];
  for (let o = 0; o < len; o++) if (!have[o] && !affected.has(o)) fixedGaps.push(o);
  if (fixedGaps.length > 3) return { why: `${fixedGaps.length} bytes are missing; that's more than a checksum can fill in` };

  // What the lines that aren't in doubt say about the affected offsets.
  const suspectRows = new Set(suspects.map(s => s.r));
  const baseVals = new Map(affectedList.map(o => [o, []]));
  for (const r of rows) {
    if (suspectRows.has(r)) continue;
    r.bytes.forEach((b, i) => { const o = r.addr + i - lo; if (baseVals.has(o)) baseVals.get(o).push([r.order, b, r]); });
  }

  const radix = [...suspects.map(s => s.alts.length), ...siteList.map(() => 2)];
  const combos = radix.reduce((p, n) => p * n, 1);
  if (combos > 1 << 18) return { why: `too many doubtful lines (${suspects.length}) to try every reading` };

  const imgCrc = parseInt(crc32(img), 16);
  const riskByUnknowns = new Float64Array(33);
  const seenSystems = new Set();
  const found = new Map(); // key -> { bytes, crc, notes, u }
  const digits = new Array(radix.length).fill(0);
  for (let c = 0; c < combos; c++) {
    for (let k = 0, v = c; k < radix.length; k++) { digits[k] = v % radix[k]; v = Math.floor(v / radix[k]); }
    const vals = new Map();
    for (const o of affectedList) vals.set(o, baseVals.get(o).slice());
    suspects.forEach((s, k) => {
      const a = s.alts[digits[k]];
      s.r.bytes.forEach((b, i) => { const o = a + i - lo; if (vals.has(o)) vals.get(o).push([s.r.order, b, s.r]); });
    });
    const forced = new Map();
    siteList.forEach((st, k) => { if (digits[suspects.length + k]) st.bytes.forEach((b, i) => forced.set(st.addr + i - lo, b)); });
    // Per affected offset: unknown, settled, or a choice between values.
    const unknown = [...fixedGaps], settled = new Map(), choices = [];
    for (const o of affectedList) {
      if (forced.has(o)) { settled.set(o, forced.get(o)); continue; }
      const v = vals.get(o);
      if (!v.length) { unknown.push(o); continue; }
      v.sort((x, y) => x[0] - y[0]);
      const opts = [...new Set(v.map(e => e[1]).reverse())];
      if (opts.length === 1) settled.set(o, opts[0]);
      else choices.push({ o, opts, from: v });
    }
    // In a right reading lines don't contradict each other: a move that
    // creates a disagreement that wasn't there as written is wrong.
    if (choices.some(ch => !originalConflicts.has(ch.o))) continue;
    if (unknown.length > 3) continue;
    let nChoice = choices.reduce((p, ch) => p * ch.opts.length, 1);
    if (nChoice > 64) { choices.forEach(ch => settled.set(ch.o, ch.opts[0])); choices.length = 0; nChoice = 1; }
    let s0 = 0;
    for (const [o, b] of settled) if (b !== img[o]) s0 ^= crcDelta(cols, o, b ^ img[o]);
    const columns = [];
    for (const o of unknown) for (let j = 0; j < 8; j++) columns.push(cols[o * 8 + j]);
    const basis = gf2Basis(columns);
    if (!basis) continue;
    const u = columns.length;
    for (let q = 0; q < nChoice; q++) {
      let s = s0;
      const picked = [];
      for (let k = 0, v = q; k < choices.length; k++) {
        const ch = choices[k], b = ch.opts[v % ch.opts.length];
        v = Math.floor(v / ch.opts.length);
        picked.push([ch, b]);
        if (b !== img[ch.o]) s ^= crcDelta(cols, ch.o, b ^ img[ch.o]);
      }
      // Different readings often lead to the same equations; count each
      // distinct system once.
      const systemKey = unknown.join(',') + ':' + s;
      if (seenSystems.has(systemKey)) continue;
      seenSystems.add(systemKey);
      for (const t of crcs) {
        riskByUnknowns[u] += 2 ** (u - 32);
        const x = gf2Solve(basis, (imgCrc ^ t ^ s) >>> 0);
        if (x < 0) continue;
        const out = img.slice();
        for (const [o, b] of settled) out[o] = b;
        for (const [ch, b] of picked) out[ch.o] = b;
        unknown.forEach((o, k) => { out[o] = img[o] ^ ((x >>> (k * 8)) & 0xFF); });
        const key = t + ':' + affectedList.concat(fixedGaps).map(o => out[o]).join(',');
        const previous = found.get(key);
        if (previous && previous.u <= u) continue;
        const notes = [];
        suspects.forEach((sp, k) => {
          const a = sp.alts[digits[k]];
          if (a !== sp.r.addr) notes.push(`line “${sp.r.text}” belongs at ${hex(a)}`);
        });
        siteList.forEach((st, k) => {
          if (digits[suspects.length + k]) notes.push(`${hex(st.addr)}: ${st.bytes.map(b => hex(b, 2)).join(' ')} (${st.why})`);
        });
        for (const [ch, b] of picked) if (b !== ch.opts[0]) notes.push(`${hex(lo + ch.o)}: lines disagree, ${hex(b, 2)} is right`);
        const filled = unknown.map(o => `${hex(lo + o)}=${hex(out[o], 2)}`);
        if (filled.length) notes.push(`missing byte${filled.length > 1 ? 's' : ''} worked out from the checksum: ${filled.join(', ')}`);
        found.set(key, { bytes: out, crc: t, notes, u });
      }
    }
  }
  // The fewer bytes had to be solved for, the more of the CRC is left as a
  // check. Take the solution(s) needing the fewest unknowns; readings with
  // more unknowns produce chance matches and are ignored -- what counts is
  // the chance of a fluke among hypotheses at least as well checked.
  const all = [...found.values()];
  const minUnknowns = all.reduce((m, x) => Math.min(m, x.u), 99);
  const solutions = all.filter(x => x.u === minUnknowns);
  let risk = 0;
  for (let k = 0; k <= Math.min(minUnknowns, 32); k++) risk += riskByUnknowns[k];
  if (!solutions.length) return { why: 'no reading of the listing matches a known ROM' };
  if (solutions.length > 1) return { why: `${solutions.length} different readings match a known ROM, so none can be trusted` };
  if (risk > 1 / 4096) return { why: 'too much is uncertain to be sure of a repair' };
  return solutions[0];
}

/// Lay listing rows out into a ROM image for `slot`. `sources` is one parsed
/// listing or several (later ones win where they overlap, earlier ones fill
/// what later ones lack). If the image doesn't match a known ROM, a repair
/// is attempted (see repairRegion).
/// Returns { bytes, base, covered, gaps: [[from,to]...], conflicts, verified, repair, repairWhy }.
export function buildImage(slot, sources) {
  if (!Array.isArray(sources)) sources = [sources];
  const { size, base: usualBase } = SLOTS[slot];
  const rows = [];
  sources.forEach((src, si) => src.rows.forEach((r, i) => rows.push({
    ...r, src: si, order: rows.length, prev: src.rows[i - 1] ?? null, next: src.rows[i + 1] ?? null,
  })));
  if (!rows.length) return { error: 'no address/byte lines found in that text' };
  // Where does the ROM start? The slot's usual address if the listing uses
  // it, else the listing's lowest address rounded down to the ROM size
  // (hex dumps of a file start at 0).
  const lowest = rows.reduce((m, r) => Math.min(m, r.addr), Infinity);
  const usesUsual = usualBase !== null && rows.some(r => r.addr + r.bytes.length > usualBase && r.addr < usualBase + size);
  const base = usesUsual ? usualBase : Math.floor(lowest / size) * size;
  const bytes = new Uint8Array(size).fill(0xFF);
  const have = new Uint8Array(size);
  // Per listing: the value each line put at each offset, and whether two
  // lines of the same listing disagreed there (overlapping lines with the
  // same bytes are normal, e.g. message tables listed message by message).
  const srcVal = sources.map(() => new Int16Array(size).fill(-1));
  const srcClash = sources.map(() => new Uint8Array(size));
  const conflictSet = new Set();
  for (const r of rows) r.bytes.forEach((b, i) => {
    const o = r.addr - base + i;
    if (o < 0 || o >= size) return;
    if (have[o] && bytes[o] !== b) conflictSet.add(base + o);
    bytes[o] = b; have[o] = 1;
    const sv = srcVal[r.src];
    if (sv[o] >= 0 && sv[o] !== b) srcClash[r.src][o] = 1;
    sv[o] = b;
  });
  const conflicts = [...conflictSet].sort((a, b) => a - b);
  const fillNotes = [];
  for (const f of KNOWN_FILL[slot] || []) {
    let n = 0;
    for (let a = f.from; a <= f.to; a++) {
      const o = a - base;
      if (o >= 0 && o < size && !have[o]) { bytes[o] = f.value; have[o] = 1; n++; }
    }
    if (n) fillNotes.push(`${hex(f.from)}-${hex(f.to)} not in the listing: ${f.why}`);
  }
  const gaps = [];
  for (let o = 0; o < size; o++) {
    if (have[o]) continue;
    let e = o;
    while (e + 1 < size && !have[e + 1]) e++;
    gaps.push([base + o, base + e]);
    o = e;
  }
  const covered = have.reduce((n, h) => n + h, 0);
  const result = { bytes, base, covered, gaps, conflicts, verified: null, repair: null, fillNotes };
  if (KNOWN[slot] && isKnown(slot, bytes) && !conflicts.length && covered === size) {
    result.verified = identify(slot, bytes);
    return result;
  }
  const ctx = { rows, base, img: bytes, have, srcClash, sites: VARIANT_SITES[slot], conflicts };
  let why = '';
  for (const layout of layouts(slot)) {
    const out = bytes.slice(), notes = [];
    let ok = true;
    for (const region of layout) {
      const part = bytes.subarray(region.off, region.off + region.len);
      const partHave = have.subarray(region.off, region.off + region.len);
      const clean = partHave.every(h => h) && !conflicts.some(a => a >= base + region.off && a < base + region.off + region.len);
      if (clean && region.crcs.includes(parseInt(crc32(part), 16))) continue;
      const fix = repairRegion(ctx, region);
      if (!fix.bytes) { ok = false; why = why || fix.why; break; }
      out.set(fix.bytes, region.off);
      notes.push(...fix.notes);
    }
    if (ok && isKnown(slot, out)) {
      return {
        ...result, bytes: out, covered: size, gaps: [], verified: identify(slot, out),
        repair: { notes: [...fillNotes, ...notes], original: { covered, gaps, conflicts } },
      };
    }
  }
  result.repairWhy = why;
  return result;
}
