// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Types a BASIC listing on the emulated keyboard, key by key, in emulated
// time (so it works at any speed setting).

import { cellsForCharacter } from './keyboard-map.js';

const HOLD_CYCLES = 50_000;
const GAP_CYCLES = 20_000;

export class Typer {
  /// `onStatus(text)` reports progress; `onDone()` is called when typing
  /// ends either way.
  constructor({ onStatus, onDone }) {
    this.onStatus = onStatus;
    this.onDone = onDone;
    this.job = null;
  }

  get busy() { return !!this.job; }

  start(text, now) {
    this.cancel();
    const chars = [];
    for (const line of text.replace(/\r\n?/g, '\n').replace(/\n+$/, '').split('\n')) chars.push(...line, '\n');
    this.job = { chars, index: 0, holding: null, until: now };
  }

  cancel(machine, message) {
    if (!this.job) return;
    if (this.job.holding && machine) for (const [r, c] of this.job.holding) machine.set_key(r, c, false);
    this.job = null;
    if (message) this.onStatus(message);
    this.onDone();
  }

  /// Press and release keys up to emulated cycle `now`.
  advance(machine, now) {
    const job = this.job;
    while (this.job && now >= job.until) {
      if (job.holding) {
        for (const [r, c] of job.holding) machine.set_key(r, c, false);
        job.holding = null;
        job.until = now + GAP_CYCLES;
        continue;
      }
      if (job.index >= job.chars.length) { this.cancel(machine, `done: typed ${job.chars.length} characters.`); break; }
      const cells = cellsForCharacter(job.chars[job.index++]);
      this.onStatus(`typing... ${job.index}/${job.chars.length}`);
      if (!cells) continue; // no such key on a C64
      for (const [r, c] of cells) machine.set_key(r, c, true);
      job.holding = cells;
      job.until = now + HOLD_CYCLES;
    }
  }
}
