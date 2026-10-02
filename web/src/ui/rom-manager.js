// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The ROMs tab of the settings. Per ROM type: the images added so far (one
// of them in use, switchable), and a way to add another from a file or a
// URL, either a binary image or a disassembly/hex-dump listing. Listings
// are rebuilt, merged and repaired, and every result is checked against the
// real Commodore chips. Above it all: find the missing ones automatically.

import * as Roms from '../roms/index.js';
import { $, nextPaint, readFile } from './dom.js';

const hex = (a) => '$' + a.toString(16).toUpperCase();
const range = ([a, b]) => (a === b ? hex(a) : `${hex(a)}-${hex(b)}`);

/// Create an element: el('div', { class: 'x', text: 'hi' }, child, ...).
function el(tag, props = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === 'text') e.textContent = v;
    else if (k === 'class') e.className = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else if (v !== false && v !== null && v !== undefined) e.setAttribute(k, v === true ? '' : v);
  }
  e.append(...children.filter(Boolean));
  return e;
}

export class RomManager {
  /// `romSet` holds the ROMs; `onChanged()` restarts the machine.
  constructor({ romSet, onChanged }) {
    this.romSet = romSet;
    this.onChanged = onChanged;
    this.partial = {};   // slot -> parsed listings of an incomplete ROM, to merge with the next one
    this.status = {};    // slot -> { text, cls, notes }
    this.pasteFor = null; // slot whose paste box is open
    this.box = $('romManager');
    $('romAutoAll').addEventListener('click', () => this.findMissing());
    romSet.onChange(() => this.render());
  }

  render() {
    this.box.innerHTML = '';
    for (const slot of Object.keys(Roms.SLOTS)) this.box.append(this.card(slot));
  }

  card(slot) {
    const s = Roms.SLOTS[slot];
    const active = this.romSet.active(slot);
    const list = el('div', { class: 'rom-list', role: 'radiogroup', 'aria-label': `${s.label} ROMs` });
    for (const entry of this.romSet.entries(slot)) {
      const id = `rom-${slot}-${entry.id}`;
      const name = Roms.identify(slot, entry.bytes);
      list.append(el('div', { class: 'rom-entry' },
        el('input', { type: 'radio', name: `rom-${slot}`, id, checked: entry.id === active?.id,
          onchange: async () => { await this.romSet.use(slot, entry.id); this.onChanged(); } }),
        el('label', { for: id },
          el('span', { class: Roms.isKnown(slot, entry.bytes) ? 'chip' : 'chip unknown', text: name }),
          el('span', { class: 'src', text: `${entry.source} · ${new Date(entry.added).toLocaleDateString()}` })),
        el('button', { type: 'button', class: 'icon', title: 'Remove', 'aria-label': `Remove ${name}`,
          onclick: async () => { await this.romSet.remove(slot, entry.id); this.onChanged(); } }, '×')));
    }
    if (!list.children.length) {
      list.append(el('div', { class: 'hint', text: slot === 'dos' ? 'None yet: the C64 runs without a disk drive.' : 'None yet.' }));
    }

    const file = el('input', { type: 'file', hidden: true,
      onchange: () => { const f = file.files[0]; file.value = ''; if (f) this.addFile(slot, f); } });
    const url = el('input', { type: 'text', placeholder: 'URL of a ROM file or a disassembly', 'aria-label': `${s.label} URL` });
    const addUrl = () => { if (url.value.trim()) this.addUrl(slot, url.value.trim()); };
    url.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); addUrl(); } });
    const add = el('div', { class: 'row rom-add' },
      el('span', { class: 'hint', text: 'Add:' }),
      el('button', { type: 'button', onclick: () => file.click() }, 'File…'),
      url,
      el('button', { type: 'button', onclick: addUrl }, 'Add URL'),
      file);

    const card = el('section', { class: 'rom-card' }, el('h3', { text: s.label }), list, add);
    const st = this.status[slot];
    if (st) {
      const status = el('div', { class: `hint rom-status ${st.cls || ''}`, text: st.text });
      if (this.partial[slot]) {
        status.append(' ', el('button', { type: 'button', class: 'link',
          onclick: () => { delete this.partial[slot]; delete this.status[slot]; this.render(); } }, 'Start over'));
      }
      card.append(status);
      if (st.notes?.length) card.append(el('details', { class: 'hint' }, el('summary', { text: 'What was repaired' }),
        el('ul', {}, ...st.notes.map(n => el('li', { text: n })))));
    }
    if (this.pasteFor === slot) card.append(this.pasteBox(slot));
    return card;
  }

  /// For sites the page may not read: the user copies the page and pastes it.
  pasteBox(slot) {
    const text = el('textarea', { rows: 5, placeholder: 'Paste the page here (Ctrl+V)' });
    const go = () => { if (text.value.trim()) this.addText(slot, text.value, 'pasted listing'); };
    text.addEventListener('paste', () => setTimeout(go, 0));
    setTimeout(() => text.focus(), 0);
    return el('div', { class: 'rom-paste' },
      el('p', { class: 'hint', text: 'The page may not read that site directly. Open the address in a new tab, select everything (Ctrl+A), copy it (Ctrl+C) and paste it here (Ctrl+V).' }),
      text,
      el('div', { class: 'row' },
        el('button', { type: 'button', onclick: go }, 'Add'),
        el('button', { type: 'button', onclick: () => { this.pasteFor = null; this.render(); } }, 'Cancel')));
  }

  setStatus(slot, text, cls = '', notes = null) {
    this.status[slot] = { text, cls, notes };
    this.render();
  }

  async addFile(slot, f) {
    this.setStatus(slot, `reading ${f.name}...`);
    await nextPaint();
    try {
      const r = slot === 'chargen' && f.type.startsWith('image/')
        ? { ...Roms.chargenFromImages([await Roms.imageDataFrom(f)]), covered: 4096, gaps: [] }
        : Roms.fromBytesOrText(slot, await readFile(f), this.partial[slot] || []);
      await this.accept(slot, r, `file ${f.name}`);
    } catch (e) {
      this.setStatus(slot, `✗ ${f.name}: ${e.message}`, 'bad');
    }
  }

  async addUrl(slot, url) {
    this.pasteFor = null;
    this.setStatus(slot, 'downloading...');
    await nextPaint();
    try {
      const r = await Roms.fromUrl(slot, url, this.partial[slot] || []);
      await this.accept(slot, r, r.url || url);
    } catch (e) {
      if (e instanceof Roms.BlockedByBrowser) {
        this.pasteFor = slot;
        this.setStatus(slot, `✗ ${e.message}.`, 'bad');
      } else this.setStatus(slot, `✗ ${e.message}`, 'bad');
    }
  }

  async addText(slot, text, source) {
    this.setStatus(slot, 'rebuilding...');
    await nextPaint();
    try {
      const r = Roms.fromText(slot, text, this.partial[slot] || []);
      this.pasteFor = null;
      await this.accept(slot, r, source);
    } catch (e) {
      this.setStatus(slot, `✗ ${e.message}`, 'bad');
    }
  }

  /// Put a rebuilt image on the shelf, or say what is missing.
  async accept(slot, r, source) {
    const size = Roms.SLOTS[slot].size;
    if (!r.bytes) throw new Error(r.error || 'nothing usable found');
    if (r.covered !== undefined && r.covered < size) {
      this.partial[slot] = r.sources;
      const gaps = r.gaps.slice(0, 3).map(range).join(', ') + (r.gaps.length > 3 ? ', ...' : '');
      this.setStatus(slot, `Incomplete: ${r.covered} of ${size} bytes (missing ${gaps}). Add another listing to fill the gaps; it is merged with this one.`, 'bad');
      return;
    }
    delete this.partial[slot];
    await this.romSet.add(slot, r.bytes, source);
    const notes = r.repair?.notes || null;
    const parts = [r.verified ? `✓ Added ${r.verified}` : `Added, but it is not a known Commodore ROM (${Roms.identify(slot, r.bytes)}): a modified ROM, or a listing with typos.`];
    if (r.repair) parts.push('Gaps or typos in the listing were fixed, proven by the checksum.');
    // A listing of the whole C64 ROM holds BASIC and KERNAL both.
    const other = slot === 'kernal' ? 'basic' : slot === 'basic' ? 'kernal' : null;
    const also = other && r.verified ? Roms.alsoFor(other, r) : null;
    if (also?.verified && !this.romSet.entries(other).some(e => Roms.identify(other, e.bytes) === also.verified)) {
      await this.romSet.add(other, also.bytes, source);
      parts.push(`It also held ${also.verified}, added under ${Roms.SLOTS[other].label}.`);
    }
    this.setStatus(slot, parts.join(' '), r.verified ? 'ok' : 'bad', notes);
    this.onChanged();
  }

  /// Find every missing (or unrecognised) ROM on the web.
  async findMissing() {
    const button = $('romAutoAll');
    button.disabled = true;
    const cache = new Map();
    for (const slot of Object.keys(Roms.SLOTS)) {
      if (this.romSet.verified(slot)) continue;
      this.setStatus(slot, 'searching...');
      const r = await Roms.autoFind(slot, () => {}, cache);
      if (r) {
        await this.romSet.add(slot, r.bytes, r.source);
        this.setStatus(slot, `✓ Found ${r.verified}`, 'ok');
      } else this.setStatus(slot, '✗ Not found automatically: add a file or a URL.', 'bad');
    }
    button.disabled = false;
    this.onChanged();
  }
}
