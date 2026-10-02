// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Indicator LEDs drawn over the desk photo: a dark "off" lens, a lit core
// and a glow, each an element styled in style.css.

/// One LED, shown by the element(s) with data-led="<id>".
export class Led {
  constructor(id) {
    this.elements = [...document.querySelectorAll(`[data-led="${id}"]`)];
    this.level = -1;
  }

  /// Brightness 0-1 (the fraction of time a flickering LED was on).
  set(level) {
    level = Math.max(0, Math.min(1, level));
    if (Math.abs(level - this.level) < 0.01) return;
    this.level = level;
    const p = Math.sqrt(level); // perceived brightness: a low duty cycle still glimmers
    for (const el of this.elements) {
      el.querySelector('.core').style.opacity = p.toFixed(3);
      el.querySelector('.halo').style.opacity = (p * 0.95).toFixed(3);
      el.querySelector('.off').style.opacity = (0.85 * (1 - p)).toFixed(3);
    }
  }
}
