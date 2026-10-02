// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The emulated machine as the page sees it: the WebAssembly C64 (or the
// picture demo when it isn't built), switched on and off, and run in step
// with real time.

import { DemoMachine } from './demo-machine.js';

export class Emulator {
  constructor() {
    this.wasm = null;       // { Machine, memory } once loaded
    this.demo = false;      // no wasm build: still pictures
    this.machine = null;    // the running machine, or null when off
    this.driveAttached = false;
    this.clockHz = 985248;
    this.totalCycles = 0;   // since the page loaded
    this.cycleDebt = 0;
  }

  /// Load web/pkg/c64_wasm.js; without it the page runs the picture demo.
  async load() {
    try {
      const mod = await import('../../pkg/c64_wasm.js');
      const instance = await mod.default();
      this.wasm = { Machine: mod.Machine, memory: instance.memory };
    } catch (e) {
      console.warn('wasm build not found, running the picture demo', e);
      this.demo = true;
    }
  }

  get on() { return !!this.machine; }

  /// Switch on. `roms` = { basic, kernal, chargen, dos }; options: ntsc,
  /// drive (connect a 1541), deviceNumber, sid8580, audioRate, disk (bytes).
  /// Returns a list of problems to tell the user about.
  async powerOn(roms, { ntsc, drive, deviceNumber, sid8580, audioRate, disk }) {
    const problems = [];
    if (this.demo) {
      this.machine = await DemoMachine.create();
      this.driveAttached = true;
    } else {
      this.machine = new this.wasm.Machine(roms.basic, roms.kernal, roms.chargen, ntsc);
      this.driveAttached = false;
      if (drive && !roms.dos) problems.push('No 1541 DOS ROM: the C64 is running without a disk drive (Settings > ROMs)');
      if (drive && roms.dos) {
        try { this.machine.attach_drive(roms.dos, deviceNumber); this.driveAttached = true; }
        catch (e) { problems.push(`1541 ROM rejected: ${e.message}`); }
      }
      if (this.driveAttached && disk) {
        try { this.machine.insert_disk(0, disk); }
        catch (e) { problems.push(`disk image rejected: ${e.message}`); }
      }
      if (sid8580) this.machine.set_sid_8580(true);
      this.machine.set_audio_rate(audioRate);
    }
    this.clockHz = this.machine.clock_hz();
    this.cycleDebt = 0;
    return problems;
  }

  powerOff() {
    if (!this.machine) return;
    this.machine.free?.();
    this.machine = null;
    this.driveAttached = false;
  }

  /// Run for `ms` of real time (or flat out). Returns the cycles run.
  run(ms, fast) {
    const m = this.machine;
    if (!m || m.jammed()) return 0;
    let ran = 0;
    if (fast) ran = m.run_cycles(400000);
    else {
      this.cycleDebt += ms / 1000 * this.clockHz;
      const target = Math.floor(this.cycleDebt);
      if (target > 0) { ran = m.run_cycles(target); this.cycleDebt -= ran; }
    }
    this.totalCycles += ran;
    return ran;
  }

  /// The current picture: { rgba, width, height }.
  frame() {
    const m = this.machine;
    const width = m.framebuffer_width(), height = m.framebuffer_height();
    const rgba = this.demo ? m.frame() : new Uint8Array(this.wasm.memory.buffer, m.framebuffer_ptr(), width * height * 4);
    return { rgba, width, height };
  }

  /// The disk in the drive as a D64 image (with everything written to it).
  extractDisk() {
    if (!this.machine || this.demo || !this.driveAttached) return null;
    const bytes = this.machine.extract_disk(0);
    return bytes.length ? bytes : null;
  }
}
