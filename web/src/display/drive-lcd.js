// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// A 16x2 character LCD on the 1541's front (a modern add-on, not original
// hardware), drawn with the C64's own character ROM as its dot-matrix
// font. Hovering over it shows it four times larger.

export const LCD_COLUMNS = 16;
const ROWS = 2;
const DOT = 4; // off-screen pixels per dot

/// PETSCII to the character ROM's screen code.
export function petsciiToScreen(c) {
  if (c >= 0x40 && c <= 0x5F) return c - 0x40;
  if (c >= 0x60 && c <= 0x7F) return c - 0x20;
  if (c >= 0xA0 && c <= 0xBF) return c - 0x40;
  if (c >= 0xC0 && c <= 0xDF) return c - 0x80;
  if (c >= 0x20 && c <= 0x3F) return c;
  return 0x20;
}

/// ASCII text as PETSCII bytes (upper case).
export const petscii = (text) => Uint8Array.from(text.toUpperCase(), (ch) => ch.charCodeAt(0));

export class DriveLcd {
  /// `canvases` are drawn with the same content (the display and its
  /// enlarged copy); `font()` returns the character ROM or null.
  constructor(canvases, font) {
    this.canvases = canvases;
    this.font = font;
    this.drawn = new Map();
    this.panel = document.createElement('canvas');
  }

  /// Show two lines of PETSCII bytes; `lit` is the backlight.
  show(lines, lit) {
    const content = lit + '|' + lines.map(l => Array.from(l).join(',')).join('|');
    if (content !== this.content) {
      this.content = content;
      this.renderPanel(lines, lit);
      this.drawn.clear();
    }
    for (const canvas of this.canvases) this.present(canvas);
  }

  /// Dots are drawn large off screen, then filtered down to the display's
  /// real pixel size (a plain browser downscale skips dots).
  renderPanel(lines, lit) {
    const W = (LCD_COLUMNS * 9 + 5) * DOT, H = (ROWS * 9 + 5) * DOT;
    this.panel.width = W; this.panel.height = H;
    const g = this.panel.getContext('2d');
    g.fillStyle = lit ? '#9fc23a' : '#2b3122'; // STN yellow-green, backlit or dark
    g.fillRect(0, 0, W, H);
    const font = this.font();
    for (let row = 0; row < ROWS; row++) {
      const text = lines[row] || new Uint8Array(0);
      for (let col = 0; col < LCD_COLUMNS; col++) {
        const code = col < text.length ? petsciiToScreen(text[col]) : 0x20;
        for (let y = 0; y < 8; y++) {
          const bits = font ? font[code * 8 + y] : 0;
          for (let x = 0; x < 8; x++) {
            const on = (bits >> (7 - x)) & 1;
            g.fillStyle = !lit ? 'rgba(0,0,0,0.12)' : on ? '#17260b' : 'rgba(40,70,0,0.13)';
            g.fillRect((3 + col * 9 + x) * DOT, (3 + row * 9 + y) * DOT, DOT - 1, DOT - 1);
          }
        }
      }
    }
    if (!font && lit) { // no character ROM yet: plain text
      g.fillStyle = '#17260b';
      g.font = `${8 * DOT}px monospace`;
      g.textBaseline = 'top';
      lines.forEach((l, i) => g.fillText(String.fromCharCode(...l), 3 * DOT, (3 + i * 9) * DOT));
    }
  }

  present(canvas) {
    if (canvas.hidden) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(16, Math.round(canvas.clientWidth * dpr));
    const h = Math.max(4, Math.round(canvas.clientHeight * dpr));
    if (this.drawn.get(canvas) === w + 'x' + h) return;
    this.drawn.set(canvas, w + 'x' + h);
    if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
    // Two-step downscale keeps the dot pattern's average instead of aliasing.
    const mid = document.createElement('canvas');
    mid.width = Math.min(this.panel.width, w * 2); mid.height = Math.min(this.panel.height, h * 2);
    const mg = mid.getContext('2d');
    mg.imageSmoothingEnabled = true; mg.imageSmoothingQuality = 'high';
    mg.drawImage(this.panel, 0, 0, mid.width, mid.height);
    const g = canvas.getContext('2d');
    g.imageSmoothingEnabled = true; g.imageSmoothingQuality = 'high';
    g.drawImage(mid, 0, 0, w, h);
  }
}
