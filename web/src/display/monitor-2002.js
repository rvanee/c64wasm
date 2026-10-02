// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The Commodore 2002 colour monitor: its power switch and red power LED,
// and the controls behind the door under the screen. From left to right:
// horizontal position, vertical hold, colour, tint, brightness, contrast,
// volume, and the COMP/SEP switch that selects the front composite input
// or the rear separate luma/chroma (S-Video) inputs.

import { Knob } from '../ui/knob.js';

/// Knob positions (0-1) as they leave the factory: everything centred.
/// Volume is not here: it is the page's main volume.
export const MONITOR_DEFAULTS = {
  hpos: 0.5, vhold: 0.5, color: 0.5, tint: 0.5, brightness: 0.5, contrast: 0.5,
};

const KNOBS = [
  ['hpos', 'Horizontal position'],
  ['vhold', 'Vertical hold'],
  ['color', 'Color'],
  ['tint', 'Tint'],
  ['brightness', 'Brightness'],
  ['contrast', 'Contrast'],
  ['volume', 'Volume'],
];

/// Vertical hold: within this distance of the centre the oscillator locks
/// to the sync pulses; further out the picture rolls, faster the further.
const HOLD_RANGE = 0.18;

export class Monitor2002 {
  /// `panel` is the element the controls are built in; `knobs` holds the
  /// knob positions (saved with the settings); `volume` is { get, set }
  /// for the shared volume; `connection` is { get, set } for the input.
  constructor({ renderer, panel, knobs, volume, connection, onChange }) {
    this.renderer = renderer;
    this.knobs = knobs;
    this.volume = volume;
    this.connection = connection;
    this.onChange = onChange;
    this.roll = 0; // lines
    this.frameLines = 312;
    this.build(panel);
    this.applyPicture();
  }

  build(panel) {
    const row = panel.querySelector('.knobs');
    this.controls = {};
    for (const [key, label] of KNOBS) {
      const isVolume = key === 'volume';
      const knob = new Knob({
        label,
        value: isVolume ? this.volume.get() : this.knobs[key],
        defaultValue: isVolume ? 0.7 : MONITOR_DEFAULTS[key],
        onInput: (v) => {
          if (isVolume) this.volume.set(v);
          else { this.knobs[key] = v; this.applyPicture(); this.onChange(); }
        },
      });
      this.controls[key] = knob;
      row.appendChild(knob.el);
    }
    this.switchEl = panel.querySelector('.comp-sep');
    this.switchEl.addEventListener('click', () => {
      this.connection.set(this.connection.get() === 'svideo' ? 'composite' : 'svideo');
      this.sync();
    });
    this.sync();
  }

  /// Bring the controls in line with settings changed elsewhere.
  sync() {
    this.controls.volume.set(this.volume.get());
    for (const key of Object.keys(MONITOR_DEFAULTS)) this.controls[key].set(this.knobs[key]);
    const sep = this.connection.get() === 'svideo';
    this.switchEl.classList.toggle('sep', sep);
    this.switchEl.setAttribute('aria-pressed', String(sep));
    this.switchEl.title = sep ? 'SEP: luma/chroma inputs (S-Video)' : 'COMP: composite input';
    this.applyPicture();
  }

  setFrameLines(lines) { this.frameLines = lines; }

  applyPicture() {
    const k = this.knobs;
    this.renderer.setPicture({
      hpos: (k.hpos - 0.5) * 28,
      saturation: k.color * 2,
      tint: (k.tint - 0.5) * 1.0,
      black: (k.brightness - 0.5) * 0.3,
      gain: 0.4 + k.contrast * 1.2,
      roll: this.roll,
    });
  }

  /// Advance the vertical roll by `dt` seconds.
  tick(dt) {
    const off = this.knobs.vhold - 0.5;
    const out = Math.abs(off) - HOLD_RANGE;
    if (out <= 0) {
      if (this.roll !== 0) { this.roll = 0; this.applyPicture(); }
      return;
    }
    const k = out / (0.5 - HOLD_RANGE); // 0 at the edge of lock .. 1 at the end stop
    const framesPerSecond = 0.15 + 2.5 * k * k;
    this.roll = (this.roll + Math.sign(off) * framesPerSecond * this.frameLines * dt) % this.frameLines;
    this.renderer.setPicture({ roll: this.roll });
  }
}
