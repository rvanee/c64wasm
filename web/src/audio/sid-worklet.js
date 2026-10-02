// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// AudioWorklet that plays the SID samples the main thread posts each
// animation frame. A small jitter buffer (~60 ms) absorbs frame-timing
// noise; if the emulator runs ahead (e.g. "as fast as possible" mode, or a
// catch-up burst after a stalled tab), the oldest samples are dropped so
// latency stays bounded. On underrun the output eases to silence instead
// of clicking.
class SidPlayer extends AudioWorkletProcessor {
  constructor() {
    super();
    this.size = sampleRate * 2;
    this.buf = new Float32Array(this.size);
    this.r = 0; this.w = 0; this.n = 0;
    this.target = Math.round(sampleRate * 0.06);
    this.max = Math.round(sampleRate * 0.25);
    this.primed = false;
    this.last = 0;
    this.volume = 1;
    this.port.onmessage = (e) => {
      const d = e.data;
      if (d.samples) this.push(d.samples);
      if (typeof d.volume === 'number') this.volume = d.volume;
      if (d.flush) { this.r = this.w = this.n = 0; this.primed = false; }
    };
  }
  push(s) {
    for (let i = 0; i < s.length; i++) {
      this.buf[this.w] = s[i];
      this.w = (this.w + 1) % this.size;
      if (this.n < this.size) this.n++; else this.r = (this.r + 1) % this.size;
    }
    if (this.n > this.max) {
      const drop = this.n - this.target;
      this.r = (this.r + drop) % this.size;
      this.n -= drop;
    }
  }
  process(_inputs, outputs) {
    const out = outputs[0];
    const ch0 = out[0];
    if (!this.primed && this.n >= this.target) this.primed = true;
    for (let i = 0; i < ch0.length; i++) {
      if (this.primed && this.n > 0) {
        this.last = this.buf[this.r];
        this.r = (this.r + 1) % this.size;
        this.n--;
      } else {
        this.primed = false;
        this.last *= 0.995;
      }
      ch0[i] = this.last * this.volume;
    }
    for (let c = 1; c < out.length; c++) out[c].set(ch0);
    return true;
  }
}
registerProcessor('sid-player', SidPlayer);
