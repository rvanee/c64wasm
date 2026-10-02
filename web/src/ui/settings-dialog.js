// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The settings dialog: machine, display, sound, disk drive and ROMs.

import { TUBE_DEFAULTS } from '../display/crt-renderer.js';
import { $ } from './dom.js';

const CONNECTION_HINTS = {
  rf: 'RF: the picture goes through the C64’s RF modulator and a TV tuner: soft, with colour fringes, dot crawl, snow, a faint ghost and a slow hum bar.',
  composite: 'Composite (the 2002’s front input, switch on COMP): luma and chroma share one wire, so fine detail shows dot crawl and rainbow fringes; colour is soft.',
  svideo: 'Separate luma and chroma (the 2002’s rear inputs, switch on SEP), like S-Video: sharp and clean, colour still band-limited.',
};

const TUBE_SLIDERS = [
  ['scanlines', 'Scanlines', 0, 1, 0.01], ['mask', 'Shadow mask', 0, 1, 0.01],
  ['curvature', 'Curvature', 0, 0.15, 0.005], ['glow', 'Glow', 0, 1, 0.01],
  ['persistence', 'Persistence', 0, 0.9, 0.01], ['glass', 'Room light', 0, 3, 0.05],
];

export class SettingsDialog {
  /// `actions` are what changes do: restart(), display(), connection(),
  /// sid(), driveSounds(), driveVolume(), tube(), monitorControls(),
  /// deviceNumber().
  constructor({ settings, save, renderer, romManager, actions }) {
    this.settings = settings;
    this.save = save;
    this.renderer = renderer;
    this.romManager = romManager;
    this.dialog = $('settings');
    this.dialog.querySelectorAll('[role=tab]').forEach(b => b.addEventListener('click', () => this.selectTab(b.dataset.tab)));

    const change = (id, fn) => $(id).addEventListener('change', (e) => { fn(e.target); save(); this.sync(); });
    change('setVideo', (el) => { settings.video = el.value; actions.restart(); });
    change('setSpeed', (el) => { settings.speed = el.value; });
    change('setDisplay', (el) => { settings.display = el.value; actions.display(); });
    change('setConnection', (el) => { settings.connection = el.value; actions.connection(); });
    change('setSid', (el) => { settings.sid = el.value; actions.sid(); });
    change('setDriveSounds', (el) => { settings.driveSounds = el.checked; actions.driveSounds(); });
    change('setDriveLcd', (el) => { settings.driveLcd = el.checked; });
    change('setDriveOn', (el) => { settings.driveOn = el.checked; actions.restart(); });
    change('setDeviceNumber', (el) => { settings.deviceNumber = +el.value; actions.deviceNumber(); });
    $('setDriveVolume').addEventListener('input', (e) => { settings.driveVolume = +e.target.value; save(); actions.driveVolume(); });
    $('monitorControlsBtn').addEventListener('click', () => { this.dialog.close(); actions.monitorControls(); });
    $('resetTube').addEventListener('click', () => { settings.tube = { ...TUBE_DEFAULTS }; save(); this.buildSliders(); });
    this.buildSliders();
  }

  open(tab = 'machine') {
    this.sync();
    this.selectTab(tab);
    this.romManager.render();
    if (!this.dialog.open) this.dialog.showModal();
  }

  selectTab(tab) {
    this.dialog.querySelectorAll('[role=tab]').forEach(b => b.setAttribute('aria-selected', String(b.dataset.tab === tab)));
    this.dialog.querySelectorAll('section[data-pane]').forEach(s => { s.hidden = s.dataset.pane !== tab; });
  }

  sync() {
    const s = this.settings;
    $('setVideo').value = s.video;
    $('setSpeed').value = s.speed;
    $('setDisplay').value = s.display;
    $('setConnection').value = s.connection;
    $('connHint').textContent = CONNECTION_HINTS[s.connection];
    $('tubeBox').hidden = s.display !== 'crt';
    $('setSid').value = s.sid;
    $('setDriveSounds').checked = s.driveSounds;
    $('setDriveVolume').value = s.driveVolume;
    $('setDriveOn').checked = s.driveOn;
    $('setDeviceNumber').value = String(s.deviceNumber);
    $('setDeviceNumber').disabled = !s.driveOn;
    $('setDriveLcd').checked = s.driveLcd;
  }

  buildSliders() {
    const box = $('sliders');
    box.innerHTML = '';
    this.renderer.setParams(this.settings.tube);
    for (const [key, label, min, max, step] of TUBE_SLIDERS) {
      const l = document.createElement('label');
      l.innerHTML = `<span></span><input type="range" min="${min}" max="${max}" step="${step}"><output></output>`;
      l.querySelector('span').textContent = label;
      const input = l.querySelector('input'), out = l.querySelector('output');
      input.value = this.settings.tube[key];
      out.textContent = (+input.value).toFixed(2);
      input.addEventListener('input', () => {
        this.settings.tube[key] = +input.value;
        this.renderer.setParams({ [key]: +input.value });
        out.textContent = (+input.value).toFixed(2);
        this.save();
      });
      box.appendChild(l);
    }
  }
}
