// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Rebuild the Commodore ROMs from public listings on the web, exactly as the
// page does for a visitor, and write them to roms/ so the tests that need
// them can run (CI does this; the ROMs themselves are never committed).
//
// Usage: node web/tools/rebuild-roms.mjs [output-dir]   (default: roms/)
//
// Every ROM is checked against the real chip's CRC32 before it is written.
// A ROM that can't be rebuilt (a site is down, say) is reported and left
// out; the tests that need it then skip themselves. In GitHub Actions the
// outcome also goes to the run's summary, and a missing ROM is a warning.

import { existsSync, mkdirSync, writeFileSync, appendFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SLOTS, autoFind } from '../src/roms/index.js';

const FILE_NAMES = { basic: 'basic.rom', kernal: 'kernal.rom', chargen: 'chargen.rom', dos: '1541.rom' };

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const out = resolve(process.argv[2] || join(repo, 'roms'));
if (!existsSync(out)) mkdirSync(out, { recursive: true });

const results = [];
const cache = new Map(); // one listing holds both BASIC and KERNAL
for (const slot of Object.keys(SLOTS)) {
  const r = await autoFind(slot, (line) => console.log(line), cache);
  if (r) {
    writeFileSync(join(out, FILE_NAMES[slot]), r.bytes);
    results.push({ slot, ok: true, text: `${r.verified}, from ${r.source}` });
  } else {
    results.push({ slot, ok: false, text: 'could not be rebuilt; the tests that need it will skip' });
    if (process.env.GITHUB_ACTIONS) console.log(`::warning title=ROM not rebuilt::${SLOTS[slot].label} ROM could not be rebuilt from the public listings`);
  }
}

console.log('');
for (const r of results) console.log(`${r.ok ? '✓' : '✗'} ${SLOTS[r.slot].label}: ${r.text}`);

if (process.env.GITHUB_STEP_SUMMARY) {
  const lines = ['### ROMs rebuilt from public listings', '', '| ROM | Result |', '|---|---|'];
  for (const r of results) lines.push(`| ${SLOTS[r.slot].label} | ${r.ok ? '✓' : '✗'} ${r.text} |`);
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, lines.join('\n') + '\n');
}
