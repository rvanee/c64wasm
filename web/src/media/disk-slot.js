// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The disk in the 1541, kept by the page so it survives switching off: the
// image is taken back from the drive (with everything the DOS wrote to it)
// whenever the machine is switched off, the disk is ejected or saved.

import { diskLabel, isD64, isG64 } from './disk.js';

export class DiskSlot {
  /// `emulator` gives access to the running drive; `onChange()` is called
  /// whenever the disk or its label changes.
  constructor(emulator, onChange) {
    this.emulator = emulator;
    this.onChange = onChange;
    this.disk = null;        // { name, bytes }
    this.lastEjected = null;
    this.label = null;       // { name, id } from the BAM, or null
  }

  /// Put an image in. Throws if it isn't a 35-track D64 or a G64.
  insert(name, bytes) {
    if (!isG64(bytes) && !isD64(bytes)) throw new Error(`${name}: not a 35-track D64/G64 (${bytes.length} bytes)`);
    const m = this.emulator.machine;
    if (m && !this.emulator.demo && this.emulator.driveAttached) m.insert_disk(0, bytes);
    this.disk = { name, bytes };
    this.refreshLabel();
  }

  eject() {
    const m = this.emulator.machine;
    if (m && !this.emulator.demo && this.emulator.driveAttached) { this.capture(); m.eject_disk(0); }
    const ejected = this.disk;
    this.lastEjected = ejected;
    this.disk = null;
    this.refreshLabel();
    return ejected;
  }

  /// Take the disk as it is now back from the drive.
  capture() {
    if (!this.disk) return;
    const bytes = this.emulator.extractDisk();
    if (bytes) this.disk = { name: this.disk.name, bytes };
  }

  /// The disk (or the last one ejected) as a D64 file: { name, bytes }.
  forDownload() {
    this.capture();
    const d = this.disk || this.lastEjected;
    if (!d || !d.bytes.length) return null;
    return { name: d.name.replace(/\.(d64|g64)$/i, '') + '.d64', bytes: d.bytes };
  }

  refreshLabel() {
    let bytes = this.disk?.bytes;
    if (bytes && !isD64(bytes)) bytes = this.emulator.extractDisk(); // a G64: decode it in the drive
    this.label = this.disk ? diskLabel(bytes) : null;
    this.onChange();
  }
}
