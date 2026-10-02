// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Getting a candidate ROM image from a file, a URL or pasted text, and
// finding the ROMs on the web automatically.

import { chargenFromImages, imageDataFrom, parseFontText } from './chargen.js';
import { htmlToText, isMostlyText, parseListing } from './listing.js';
import { buildImage } from './repair.js';
import { SLOTS, identify, isKnown } from './slots.js';

/// Web pages people find that hold a listing, mapped to a plain-text copy
/// of the same listing a browser is allowed to download.
const URL_REWRITES = [
  // pagetable.com's C64 ROM disassembly is generated from mist64/c64ref.
  [/^https?:\/\/(www\.)?pagetable\.com\/c64ref\/c64disasm\/?.*$/i,
    'https://raw.githubusercontent.com/mist64/c64ref/main/src/c64disasm/c64disasm_en.txt',
    'pagetable.com shows this listing as a web page; using its plain-text source on GitHub'],
  // GitHub "blob" pages -> the raw file.
  [/^https:\/\/github\.com\/([^/]+)\/([^/]+)\/blob\/(.*)$/, 'https://raw.githubusercontent.com/$1/$2/$3', null],
];

/// Thrown when the browser may not download from a site (no CORS headers,
/// or a plain-http page on an https site): the user has to copy the page.
export class BlockedByBrowser extends Error {}

const verifiedName = (slot, bytes) => (isKnown(slot, bytes) ? identify(slot, bytes) : null);

/// Turn file, URL or pasted contents into a candidate ROM for `slot`: a
/// binary of the right size as it is, else text read as a listing (merged
/// with the `previous` listings, if any). The result carries `sources` (the
/// parsed listings) so another listing can be merged in later, and
/// `verified` when it is a known Commodore ROM.
export function fromBytesOrText(slot, buf, previous = []) {
  const size = SLOTS[slot].size;
  if (!isMostlyText(buf)) {
    let bytes = null, kind = 'binary';
    if (buf.length === size) bytes = buf;
    else if (buf.length === size + 2) { bytes = buf.slice(2); kind = 'binary (PRG header skipped)'; }
    else if (buf.length === 16384 && (slot === 'basic' || slot === 'kernal')) {
      // 251913-01 style: BASIC and KERNAL in one 16 KB chip (C64C).
      bytes = slot === 'basic' ? buf.slice(0, 8192) : buf.slice(8192);
      kind = 'binary (combined BASIC+KERNAL ROM)';
    }
    if (!bytes) throw new Error(`binary file of ${buf.length} bytes; a ${SLOTS[slot].label} ROM is ${size} bytes`);
    return { bytes, covered: size, gaps: [], conflicts: [], kind, sources: [], verified: verifiedName(slot, bytes), raw: buf };
  }
  const text = new TextDecoder().decode(buf);
  if (slot === 'chargen') {
    const font = parseFontText(htmlToText(text));
    if (font) return { bytes: font, covered: 4096, gaps: [], conflicts: [], kind: 'font drawn as text', sources: [], verified: verifiedName(slot, font) };
  }
  const parsed = parseListing(text);
  if (!parsed.rows.length) throw new Error('no lines with an address and hex bytes found in that text');
  const sources = [...previous, parsed];
  const img = buildImage(slot, sources);
  if (img.error) throw new Error(img.error);
  const lines = sources.reduce((n, s) => n + s.lines, 0);
  const kind = sources.length > 1 ? `${sources.length} listings merged (${lines} lines with bytes)` : `listing (${parsed.lines} lines with bytes)`;
  return { ...img, sources, kind };
}

/// Text pasted by the user (a listing or hex dump, possibly copied from a
/// web page).
export const fromText = (slot, text, previous = []) => fromBytesOrText(slot, new TextEncoder().encode(text), previous);

/// Fetch a URL and turn it into a candidate ROM for `slot`.
export async function fromUrl(slot, url, previous = []) {
  url = url.trim();
  let note = null;
  for (const [re, to, why] of URL_REWRITES) {
    if (re.test(url)) { url = url.replace(re, to); note = why; break; }
  }
  const buf = await download(url);
  if (slot === 'chargen' && buf.type.startsWith('image/')) {
    const r = chargenFromImages([await imageDataFrom(new Blob([buf.bytes], { type: buf.type }))]);
    if (!r.bytes) throw new Error(r.error);
    return { bytes: r.bytes, covered: 4096, gaps: [], conflicts: [], kind: `picture (${r.glyphs} glyphs)`, sources: [], verified: r.verified, note, url };
  }
  const r = fromBytesOrText(slot, buf.bytes, previous);
  if (note) r.note = note;
  r.url = url;
  return r;
}

async function download(url) {
  const onHttps = typeof location !== 'undefined' && location.protocol === 'https:'; // (no location in Node)
  if (onHttps && url.startsWith('http:')) {
    throw new BlockedByBrowser('this page is https and the listing is plain http, which browsers do not mix');
  }
  let resp;
  try {
    resp = await fetch(url);
  } catch {
    throw new BlockedByBrowser('could not download it: the site may not let other web pages download it, or it is unreachable');
  }
  if (!resp.ok) throw new Error(`the server answered HTTP ${resp.status}`);
  return { bytes: new Uint8Array(await resp.arrayBuffer()), type: resp.headers.get('content-type') || '' };
}

/// The same listings (or combined binary) built for another slot: a listing
/// of the whole C64 ROM fills BASIC and KERNAL at once.
export function alsoFor(slot, candidate) {
  if (candidate.raw && candidate.raw.length === 16384 && (slot === 'basic' || slot === 'kernal')) {
    try { return fromBytesOrText(slot, candidate.raw); } catch { return null; }
  }
  if (!candidate.sources || !candidate.sources.length) return null;
  const { base, size } = SLOTS[slot];
  const touches = candidate.sources.some(s => s.rows.some(r => r.addr >= base && r.addr < base + size));
  if (!touches) return null;
  const img = buildImage(slot, candidate.sources);
  return img.verified ? { ...img, sources: candidate.sources, kind: candidate.kind } : null;
}

/// Public material a browser may download (the sites send CORS headers),
/// tried in order by autoFind. Every result is checked against the real
/// chips' CRC32s before it is used.
export const AUTO_SOURCES = {
  basic: [
    { url: 'https://raw.githubusercontent.com/mist64/c64ref/main/src/c64disasm/c64disasm_en.txt', what: 'commented BASIC + KERNAL disassembly (mist64/c64ref, as on pagetable.com)' },
    { url: 'https://raw.githubusercontent.com/lagomorph/c64rom/master/c64rom_en.txt', what: 'the same disassembly, older copy' },
  ],
  kernal: [
    { url: 'https://raw.githubusercontent.com/mist64/c64ref/main/src/c64disasm/c64disasm_en.txt', what: 'commented BASIC + KERNAL disassembly (mist64/c64ref, as on pagetable.com)' },
    { url: 'https://raw.githubusercontent.com/lagomorph/c64rom/master/c64rom_en.txt', what: 'the same disassembly, older copy' },
  ],
  chargen: [
    { url: 'https://raw.githubusercontent.com/mobluse/chargen-maker/master/chargen.txt', what: 'the character set drawn as text (mobluse/chargen-maker)' },
    { url: 'https://commons.wikimedia.org/wiki/Special:FilePath/C64_Petscii_Charts.png', what: 'picture of the character set (Wikimedia Commons)', image: true },
  ],
  dos: [
    { url: 'https://g3sl.github.io/c1541rom.html', what: 'commented 1541 ROM disassembly (g3sl.github.io)' },
  ],
};

// Listings tried and not used automatically: retroisle.com's 1541
// disassembly (Marko Mäkelä) rebuilds 325302-01 + 901229-05 when pasted
// (its unused first page is filled in by KNOWN_FILL), but the site doesn't
// let other pages download it. Frank Kontros' listing on ffd2.com rebuilds
// 325302-01, but its second half matches none of the known 901229 chips.

/// Try the automatic sources for `slot` in turn; `log(text)` reports
/// progress. `cache` shares downloads between slots (one listing holds both
/// BASIC and KERNAL). Resolves to { bytes, verified, source } or null.
export async function autoFind(slot, log = () => {}, cache = new Map()) {
  const label = SLOTS[slot].label;
  for (const src of AUTO_SOURCES[slot] || []) {
    try {
      log(`${label}: trying ${src.what}...`);
      let r;
      if (src.image) {
        r = chargenFromImages([await imageDataFrom(src.url)]);
        if (r.error && !r.verified) throw new Error(r.error);
      } else {
        if (!cache.has(src.url)) cache.set(src.url, (await download(src.url)).bytes);
        r = fromBytesOrText(slot, cache.get(src.url));
      }
      if (r.verified) {
        log(`${label}: ✓ ${r.verified}`);
        return { bytes: r.bytes, verified: r.verified, source: src.url };
      }
      log(`${label}: that source didn't give a verified ROM${r.repairWhy ? ` (${r.repairWhy})` : ''}`);
    } catch (e) {
      log(`${label}: ${src.url} failed (${e.message})`);
    }
  }
  return null;
}
