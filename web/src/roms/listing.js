// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Reading the bytes out of a disassembly or hex-dump listing.

/// A listing published as a web page: turn the HTML into lines of text
/// (table cells become double-space-separated columns, so a comment column
/// still ends the byte field). Plain text is returned unchanged.
export function htmlToText(text) {
  if (!/<(html|body|pre|table|tr|td|br|p|div)\b/i.test(text.slice(0, 20000))) return text;
  return text
    .replace(/<(script|style)\b[\s\S]*?<\/\1>/gi, '')
    .replace(/<br\s*\/?>|<\/(p|div|tr|li|h\d|pre)>/gi, '\n')
    .replace(/<\/t[dh]>\s*<t[dh][^>]*>/gi, '  ')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"')
    .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(+n))
    .replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCharCode(parseInt(n, 16)))
    .replace(/&amp;/g, '&');
}

/// Data directives: `.BY $AA,$55`, `.BYTE`, `.DB`, `!byte`; `.WD $C8C6`,
/// `.WORD`, `.DW`, `!word` (little-endian). Returns the bytes or null.
function directiveBytes(rest) {
  const m = /^[.!](BY|BYTE|DB|WD|WORD|DW)\b\s*([^;]*)/i.exec(rest);
  if (!m) return null;
  const word = /^(WD|WORD|DW)$/i.test(m[1]);
  const bytes = [];
  for (const item of m[2].split(',')) {
    const v = /^\s*\$?([0-9A-Fa-f]{1,4})\s*$/.exec(item);
    if (!v) break;
    const n = parseInt(v[1], 16);
    if (word) bytes.push(n & 0xFF, n >> 8);
    else if (n <= 0xFF) bytes.push(n);
    else break;
  }
  return bytes;
}

/// The hex bytes at the start of `rest`: pairs separated by single spaces;
/// the first wider gap ends them, so a comment such as "12 return without
/// gosub" after the bytes isn't mistaken for data. (hexdump -C has one
/// double space in the middle of its 16 bytes; xxd groups bytes in pairs.)
/// Returns { bytes, end } with `end` the offset just past the last byte.
function hexBytes(rest, wide) {
  const bytes = [];
  let pos = 0, end = 0;
  while (pos < rest.length && bytes.length < 256) {
    const t = /^([0-9A-Fa-f]{2}|[0-9A-Fa-f]{4})(?=\s|$)/.exec(rest.slice(pos));
    if (!t || (t[1].length === 4 && !wide)) break;
    if (t[1].length === 4) bytes.push(parseInt(t[1].slice(0, 2), 16), parseInt(t[1].slice(2), 16));
    else bytes.push(parseInt(t[1], 16));
    pos += t[1].length;
    end = pos;
    const sp = /^[ \t]*/.exec(rest.slice(pos))[0];
    if (sp.length === 0) break;
    if (sp.length >= 2 || sp.includes('\t')) {
      const midHexdump = wide && sp.length === 2 && bytes.length === 8;
      if (!midHexdump) break;
    }
    pos += sp.length;
  }
  return { bytes, end };
}

/// Parse a disassembly or hex-dump listing into memory bytes.
/// Recognised line shapes (address first, then the bytes, then anything):
///   `E000  85 56     STA $56`        plain disassembly
///   `C100 78 SEI`                    single spaces before the mnemonic
///   `.,E000 85 56    STA $56` / `.:A000 94 E3 ...`   (VICE monitor, c64disasm)
///   `$E000: 85 56` / `E000: 85 56`
///   `0010  E000  85 56   STA $56`    assembler listing with line numbers
///   `SETLDA  C100  78  SEI`          a label column first
///   `0000e000: 8556 2000 ...`        xxd
///   `0000e000  85 56 20 ...  |...|`  hexdump -C
///   `FFE6 .WD $C8C6` / `FFCF .BY $AA,$AA`   data directives
///   `C001  AA ...` then `C0FF  ... AA`      a range of one value, elided
/// Returns { map: Map(addr -> byte), conflicts: [addr...], lines, rows },
/// where `rows` keeps every byte-carrying line in order ({ addr, bytes, text })
/// so that typos in a line's address can be spotted and repaired later.
export function parseListing(text) {
  text = htmlToText(text);
  const map = new Map();
  const conflicts = [];
  const rows = [];
  let lines = 0;
  let elided = null; // { from, value }: "AA ..." waiting for its "... AA"
  const add = (addr, bytes, raw) => {
    lines++;
    rows.push({ addr, bytes, text: raw.trim().slice(0, 48) });
    bytes.forEach((b, i) => {
      const a = addr + i;
      if (map.has(a) && map.get(a) !== b) conflicts.push(a);
      map.set(a, b);
    });
  };
  for (const raw of text.split(/\r?\n/)) {
    let line = raw.replace(/^\s*(?:(?:\.[,:]|>C:|\.C:|[$>*])\s*)?/, '');
    // Assembler listings may start with a decimal line number ("10000 E000
    // 85 56") or a label in its own column ("SETLDA  C100  78  SEI").
    line = line.replace(/^(?:\d{4,6}\s+|[A-Za-z_][\w.]*\s{2,})(?=[0-9A-Fa-f]{4}[:\s]\s*[0-9A-Fa-f]{2}(?:\s|$))/, '');
    const m = line.match(/^([0-9A-Fa-f]{4}|[0-9A-Fa-f]{8})(?::|\s)\s*(.*)$/);
    if (!m) continue;
    let addr = parseInt(m[1], 16);
    let rest = m[2];
    const wide = m[1].length === 8; // hexdump/xxd style
    // Assembler listings put a line number first: "0010 E000 85 56".
    const ln = !wide && /^([0-9A-Fa-f]{4})\s+([0-9A-Fa-f]{2}(?:\s.*)?)$/.exec(rest);
    if (ln) { addr = parseInt(ln[1], 16); rest = ln[2]; }

    const directive = directiveBytes(rest);
    if (directive) {
      if (directive.length) add(addr, directive, raw);
      continue;
    }
    // The end of an elided range: "C0FF  ... AA".
    const tail = /^\.{3}\s+(.*)$/.exec(rest);
    if (tail) {
      const { bytes } = hexBytes(tail[1], wide);
      if (elided && bytes.length && bytes.every(b => b === elided.value) && addr >= elided.from) {
        add(elided.from, new Array(addr - elided.from + bytes.length).fill(elided.value), raw);
      }
      elided = null;
      continue;
    }
    const { bytes, end } = hexBytes(rest, wide);
    if (!bytes.length) continue;
    add(addr, bytes, raw);
    // The start of an elided range: "C001  AA ...".
    elided = /^\s*\.{3}\s*$/.test(rest.slice(end)) ? { from: addr + bytes.length, value: bytes[bytes.length - 1] } : null;
  }
  return { map, conflicts, lines, rows };
}

/// Whether a buffer is text (a listing) rather than a binary image.
export function isMostlyText(bytes) {
  let printable = 0;
  const n = Math.min(bytes.length, 4096);
  for (let i = 0; i < n; i++) {
    const c = bytes[i];
    if (c === 9 || c === 10 || c === 13 || (c >= 32 && c < 127)) printable++;
  }
  return printable / n > 0.9;
}
