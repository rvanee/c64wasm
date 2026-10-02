// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Stands in for the emulator when the WebAssembly module hasn't been built
// (web/pkg missing): it shows two still pictures and blinks the drive, so
// the page itself can be worked on and tested without Rust.

const CLOCK_HZ = 985248;

export class DemoMachine {
  static async create() {
    const frames = [];
    for (const name of ['ready', 'dir']) {
      const img = new Image();
      img.src = `assets/demo-${name}.png`;
      await img.decode();
      const c = document.createElement('canvas');
      c.width = 504; c.height = 312;
      const g = c.getContext('2d');
      g.drawImage(img, 0, 0);
      frames.push(new Uint8Array(g.getImageData(0, 0, 504, 312).data.buffer));
    }
    return new DemoMachine(frames);
  }

  constructor(frames) { this.frames = frames; this.t = 0; }
  get seconds() { return this.t / CLOCK_HZ; }
  frame() { return this.frames[Math.floor(this.seconds / 4) % this.frames.length]; }
  clock_hz() { return CLOCK_HZ; }
  run_cycles(n) { this.t += n; return n; }
  jammed() { return false; }
  framebuffer_width() { return 504; }
  framebuffer_height() { return 312; }
  set_key(row, col, pressed) { (window.__demoKeys ??= []).push([row, col, pressed]); } // seen by the page tests
  release_all_keys() {}
  press_restore() {}
  set_audio_rate() {}
  set_sid_8580() {}
  take_audio() { return new Float32Array(0); }
  drive_count() { return 1; }
  drive_led() { const s = this.seconds % 8; return s > 3.2 && s < 4.6 ? 1 : 0; }
  drive_motor() { const s = this.seconds % 8; return s > 2.5 && s < 6.2; }
  drive_track() { return 18; }
  drive_has_disk() { return true; }
  drive_head_events() { return []; }
  insert_disk() {}
  eject_disk() { return true; }
  extract_disk() { return new Uint8Array(0); }
  free() {}
}
