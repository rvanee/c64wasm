// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// How the machine is shown: a photo of the desk with the live picture on
// the monitor's screen, either the Commodore 2002 (a picture tube) or a
// modern flat panel; and the zoom onto the screen.

import { $ } from '../ui/dom.js';

const ZOOM_IN = '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="2" d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/></svg>';
const ZOOM_OUT = '<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="2" d="M9 4v5H4M20 9h-5V4M15 20v-5h5M4 15h5v5"/></svg>';

// The screen in each photo, as fractions of the photo (the same numbers
// place #crt in style.css), and how much room to leave around it when
// zoomed in.
const SCREENS = {
  crt: { x: 0.0909, y: 0.1264, w: 0.3784, h: 0.4674, pad: 1.08 },
  flat: { x: 0.0817, y: 0.1800, w: 0.4171, h: 0.4235, pad: 1.04 },
};

export class Scene {
  constructor({ renderer, settings, save }) {
    this.renderer = renderer;
    this.settings = settings;
    this.save = save;
    this.stage = $('stage');
    this.desk = $('scene');
    this.canvas = $('crt');
    this.spill = $('spill');
    $('zoomBtn').addEventListener('click', () => this.toggleZoom());
    this.canvas.addEventListener('dblclick', () => this.toggleZoom());
    new ResizeObserver(() => this.applyZoom()).observe(this.stage);
  }

  get flatPanel() { return this.settings.display === 'lcd'; }

  /// Show the 2002 or the flat panel, as the settings say.
  applyDisplay() {
    const flat = this.flatPanel;
    this.stage.classList.toggle('flat', flat);
    this.stage.classList.toggle('crt', !flat);
    this.renderer.setMode(flat ? 'lcd' : 'crt');
    this.renderer.setConnection(this.settings.connection);
    this.applyZoom();
  }

  toggleZoom() {
    this.settings.zoomed = !this.settings.zoomed;
    this.save();
    this.applyZoom();
    this.stage.focus();
  }

  applyZoom() {
    const W = this.stage.clientWidth, H = this.stage.clientHeight;
    if (this.settings.zoomed) {
      const s = SCREENS[this.flatPanel ? 'flat' : 'crt'];
      const box = { x: s.x * W, y: s.y * H, w: s.w * W, h: s.h * H };
      const scale = Math.min(W / (box.w * s.pad), H / (box.h * s.pad));
      const tx = W / 2 - (box.x + box.w / 2) * scale, ty = H / 2 - (box.y + box.h / 2) * scale;
      this.desk.style.transform = `translate(${tx}px, ${ty}px) scale(${scale})`;
      this.desk.dataset.scale = scale;
    } else {
      this.desk.style.transform = '';
      this.desk.dataset.scale = 1;
    }
    $('zoomBtn').innerHTML = this.settings.zoomed ? ZOOM_OUT : ZOOM_IN;
    $('zoomBtn').title = this.settings.zoomed ? 'Show the whole setup' : 'Zoom in on the screen';
    this.resize();
    setTimeout(() => this.resize(), 750); // after the transition
  }

  /// Size the renderer to the picture's on-screen size.
  resize() {
    const scale = parseFloat(this.desk.dataset.scale || 1);
    this.renderer.resize(this.canvas.clientWidth * scale, this.canvas.clientHeight * scale);
  }

  /// The screen's light falling on the desk.
  updateSpill(now) {
    const [r, g, b] = this.renderer.glowColor(now);
    const lum = 0.3 * r + 0.59 * g + 0.11 * b;
    this.spill.style.setProperty('--spill', `${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)}`);
    this.spill.style.opacity = (Math.min(1, lum * 1.6) * 0.32).toFixed(3);
  }
}
