// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The ROMs this browser has, as the rest of the page sees them: per slot a
// shelf of images the user added, one of which is in use.

import * as Roms from '../roms/index.js';

export class RomSet {
  constructor() {
    this.library = {};
    this.listeners = [];
  }

  async load() {
    this.library = await Roms.loadLibrary();
    this.changed();
  }

  /// The images on a slot's shelf: [{ id, bytes, source, added }].
  entries(slot) { return this.library[slot]?.entries || []; }

  /// The image in use for a slot, or null.
  active(slot) {
    const shelf = this.library[slot];
    return shelf?.entries.find(e => e.id === shelf.active) || null;
  }

  get(slot) { return this.active(slot)?.bytes || null; }

  /// Whether the image in use is a real Commodore chip.
  verified(slot) { return Roms.isKnown(slot, this.get(slot)); }

  /// BASIC, KERNAL and the character ROM: enough to switch on.
  get ready() { return !!(this.get('basic') && this.get('kernal') && this.get('chargen')); }

  /// What the emulator needs: { basic, kernal, chargen, dos }.
  forMachine() {
    return { basic: this.get('basic'), kernal: this.get('kernal'), chargen: this.get('chargen'), dos: this.get('dos') };
  }

  /// Add an image to a slot's shelf and use it.
  async add(slot, bytes, source) {
    this.library[slot] = await Roms.addRom(slot, bytes, source);
    this.changed();
  }

  async use(slot, id) {
    this.library[slot] = await Roms.useRom(slot, id);
    this.changed();
  }

  async remove(slot, id) {
    this.library[slot] = await Roms.removeRom(slot, id);
    this.changed();
  }

  /// `fn()` is called after every change.
  onChange(fn) { this.listeners.push(fn); }
  changed() { for (const fn of this.listeners) fn(); }
}
