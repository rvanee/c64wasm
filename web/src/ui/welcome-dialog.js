// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The first-run dialog: the emulator needs Commodore's ROMs. Either the
// page finds them automatically, or the user specifies them in the ROM
// settings.

import * as Roms from '../roms/index.js';
import { $ } from './dom.js';

export class WelcomeDialog {
  /// `romSet` holds the ROMs; `onStart()` switches the machine on;
  /// `onManual()` opens the ROM settings.
  constructor({ romSet, onStart, onManual }) {
    this.romSet = romSet;
    this.dialog = $('welcome');
    $('wzAuto').addEventListener('click', () => this.findAll());
    $('wzManual').addEventListener('click', () => { this.dialog.close(); onManual(); });
    $('wzStart').addEventListener('click', () => { this.dialog.close(); onStart(); });
    romSet.onChange(() => { if (this.dialog.open) this.render(); });
  }

  open() {
    this.render();
    if (!this.dialog.open) this.dialog.showModal();
  }

  /// One line per ROM: what is there, or what is happening.
  render(progress = {}) {
    const box = $('wzRows');
    box.innerHTML = '';
    for (const [slot, s] of Object.entries(Roms.SLOTS)) {
      const have = this.romSet.get(slot);
      const row = document.createElement('div');
      row.className = 'wz-row';
      const label = document.createElement('strong');
      label.textContent = s.label;
      const state = document.createElement('span');
      state.className = 'id';
      if (progress[slot]) state.textContent = progress[slot];
      else if (have) state.textContent = `✓ ${Roms.identify(slot, have)}`;
      else state.textContent = slot === 'dos' ? 'missing (optional: for the disk drive)' : 'missing';
      state.classList.toggle('bad', !have && !progress[slot]);
      row.append(label, state);
      box.append(row);
    }
    $('wzStart').disabled = !this.romSet.ready;
  }

  async findAll() {
    $('wzAuto').disabled = true;
    const progress = {};
    const cache = new Map();
    for (const slot of Object.keys(Roms.SLOTS)) {
      if (this.romSet.verified(slot)) continue;
      progress[slot] = 'searching...';
      this.render(progress);
      const r = await Roms.autoFind(slot, () => {}, cache);
      if (r) { delete progress[slot]; await this.romSet.add(slot, r.bytes, r.source); }
      else progress[slot] = '✗ not found: add it with "Specify them myself"';
      this.render(progress);
    }
    $('wzAuto').disabled = false;
    const missing = Object.keys(Roms.SLOTS).filter(s => !this.romSet.get(s));
    $('wzResult').textContent = !missing.length
      ? 'All found. Click Switch on.'
      : this.romSet.ready
        ? 'The 1541 DOS ROM is still missing: the C64 works, but without a disk drive. You can add it later in the settings.'
        : 'Some ROMs were not found. Click "Specify them myself" to give a file or a URL for them.';
  }
}
