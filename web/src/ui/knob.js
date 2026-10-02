// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// A rotary knob: grab it and turn it (drag around its centre), turn the
// mouse wheel, or use the arrow keys; double-click returns it to its
// default. Values are 0-1.

const SWEEP = 270; // degrees from minimum to maximum

export class Knob {
  /// `label` is shown under the knob; `onInput(value)` is called while it
  /// turns.
  constructor({ label, value = 0.5, defaultValue = 0.5, onInput = () => {} }) {
    this.value = value;
    this.defaultValue = defaultValue;
    this.onInput = onInput;

    this.el = document.createElement('div');
    this.el.className = 'knob';
    this.dial = document.createElement('div');
    this.dial.className = 'dial';
    this.dial.tabIndex = 0;
    this.dial.setAttribute('role', 'slider');
    this.dial.setAttribute('aria-label', label);
    this.dial.title = `${label}: grab and turn (double-click: centre)`;
    this.dial.setAttribute('aria-valuemin', '0');
    this.dial.setAttribute('aria-valuemax', '100');
    this.dial.innerHTML = '<div class="cap"><div class="mark"></div></div>';
    const caption = document.createElement('div');
    caption.className = 'caption';
    caption.textContent = label;
    this.el.append(this.dial, caption);

    this.dial.addEventListener('pointerdown', (e) => this.startDrag(e));
    this.dial.addEventListener('wheel', (e) => { e.preventDefault(); this.turn(-Math.sign(e.deltaY) * 0.03); }, { passive: false });
    this.dial.addEventListener('keydown', (e) => this.key(e));
    this.dial.addEventListener('dblclick', () => this.set(this.defaultValue, true));
    this.render();
  }

  set(value, notify = false) {
    this.value = Math.min(1, Math.max(0, value));
    this.render();
    if (notify) this.onInput(this.value);
  }

  turn(delta) { this.set(this.value + delta, true); }

  render() {
    this.dial.querySelector('.cap').style.transform = `rotate(${(this.value - 0.5) * SWEEP}deg)`;
    this.dial.setAttribute('aria-valuenow', String(Math.round(this.value * 100)));
  }

  /// Grab the knob and turn it: the value follows the pointer's angle
  /// around the knob's centre (clockwise turns it up), as with a real knob.
  startDrag(e) {
    e.preventDefault();
    this.dial.focus();
    this.dial.setPointerCapture(e.pointerId);
    const r = this.dial.getBoundingClientRect();
    const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
    const angleOf = (ev) => Math.atan2(ev.clientY - cy, ev.clientX - cx);
    const nearCentre = (ev) => Math.hypot(ev.clientX - cx, ev.clientY - cy) < r.width * 0.15;
    let last = angleOf(e);
    const move = (ev) => {
      if (nearCentre(ev)) return; // the angle is meaningless there
      const a = angleOf(ev);
      let d = a - last;
      if (d > Math.PI) d -= 2 * Math.PI;
      if (d < -Math.PI) d += 2 * Math.PI;
      last = a;
      this.turn((d * 180 / Math.PI) / SWEEP);
    };
    const up = () => {
      this.dial.removeEventListener('pointermove', move);
      this.dial.removeEventListener('pointerup', up);
      this.dial.removeEventListener('pointercancel', up);
    };
    this.dial.addEventListener('pointermove', move);
    this.dial.addEventListener('pointerup', up);
    this.dial.addEventListener('pointercancel', up);
  }

  key(e) {
    const steps = { ArrowUp: 0.02, ArrowRight: 0.02, ArrowDown: -0.02, ArrowLeft: -0.02, PageUp: 0.1, PageDown: -0.1 };
    if (e.key in steps) this.turn(steps[e.key]);
    else if (e.key === 'Home') this.set(0, true);
    else if (e.key === 'End') this.set(1, true);
    else return;
    e.preventDefault();
    e.stopPropagation(); // not for the emulated keyboard
  }
}
