// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// 1541 mechanical sounds, synthesized with WebAudio (no sample files):
//  - spindle motor: a belt-driven DC motor whir with the 300 rpm (5 Hz)
//    rotation of the disk in its jacket audible as a slow swish,
//    spinning up and down rather than switching instantly;
//  - head steps: a short tick per half-track step of the stepper motor;
//  - the head bang: when the DOS drives the stepper against the track-1
//    end stop (it does ~45 half-track steps blindly on a read error or at
//    `N:`/`I:` initialisation), each attempt is a loud knock -- the famous
//    1541 rattle.
// Step events come from the emulator with their emulated time, so the
// rhythm (e.g. ~6 ms per step during a bump) is reproduced, not just the
// count.

export class DriveSound {
  constructor(ctx, destination) {
    this.ctx = ctx;
    this.out = ctx.createGain();
    this.out.gain.value = 0.6;
    this.out.connect(destination);
    this.enabled = true;
    this.motorOn = false;
    this.latency = 0.07; // matches the SID player's jitter buffer, roughly
    this.noise = this.makeNoise(2);
    this.buildMotor();
  }

  makeNoise(seconds) {
    const b = this.ctx.createBuffer(1, Math.round(this.ctx.sampleRate * seconds), this.ctx.sampleRate);
    const d = b.getChannelData(0);
    let brown = 0;
    for (let i = 0; i < d.length; i++) {
      const w = Math.random() * 2 - 1;
      brown = (brown + 0.02 * w) / 1.02;
      d[i] = w * 0.6 + brown * 3.0;
    }
    return b;
  }

  buildMotor() {
    const ctx = this.ctx;
    this.motorGain = ctx.createGain();
    this.motorGain.gain.value = 0;
    this.motorGain.connect(this.out);

    // Motor whir: a couple of harmonics of the motor speed, low-passed.
    this.motorOsc = ctx.createOscillator();
    this.motorOsc.type = 'sawtooth';
    this.motorOsc.frequency.value = 18;
    const whirLp = ctx.createBiquadFilter();
    whirLp.type = 'lowpass'; whirLp.frequency.value = 420; whirLp.Q.value = 0.8;
    const whirGain = ctx.createGain(); whirGain.gain.value = 0.10;
    this.motorOsc.connect(whirLp).connect(whirGain).connect(this.motorGain);

    // Belt/bearing rumble and the disk rubbing in its jacket: band-limited
    // noise, amplitude-modulated at the 5 Hz rotation rate.
    const src = ctx.createBufferSource();
    src.buffer = this.noise; src.loop = true;
    const bp = ctx.createBiquadFilter();
    bp.type = 'bandpass'; bp.frequency.value = 900; bp.Q.value = 0.6;
    const swish = ctx.createGain(); swish.gain.value = 0.05;
    const lfo = ctx.createOscillator(); lfo.frequency.value = 5;
    const lfoDepth = ctx.createGain(); lfoDepth.gain.value = 0.02;
    lfo.connect(lfoDepth).connect(swish.gain);
    src.connect(bp).connect(swish).connect(this.motorGain);

    this.motorOsc.start(); src.start(); lfo.start();
  }

  setEnabled(on) {
    this.enabled = on;
    if (!on) this.setMotor(false, true);
  }

  setVolume(v) { this.out.gain.setTargetAtTime(v, this.ctx.currentTime, 0.02); }

  setMotor(on, immediate = false) {
    on = on && this.enabled;
    if (on === this.motorOn) return;
    this.motorOn = on;
    const t = this.ctx.currentTime + (immediate ? 0 : this.latency);
    const g = this.motorGain.gain, f = this.motorOsc.frequency;
    g.cancelScheduledValues(t); f.cancelScheduledValues(t);
    if (on) {
      // spin-up: ~0.4 s to speed
      g.setTargetAtTime(1, t, 0.12);
      f.setValueAtTime(6, t);
      f.setTargetAtTime(18, t, 0.15);
    } else {
      g.setTargetAtTime(0, t, 0.25);
      f.setTargetAtTime(4, t, 0.3);
    }
  }

  /// One stepper event. `when` is an AudioContext time.
  click(when, bumped) {
    const ctx = this.ctx;
    const n = ctx.createBufferSource();
    n.buffer = this.noise;
    const offs = Math.random() * 1.5;
    const bp = ctx.createBiquadFilter();
    bp.type = 'bandpass';
    bp.frequency.value = bumped ? 1300 + Math.random() * 300 : 2600 + Math.random() * 400;
    bp.Q.value = bumped ? 3 : 1.6;
    const g = ctx.createGain();
    const peak = bumped ? 0.9 : 0.28;
    g.gain.setValueAtTime(0, when);
    g.gain.linearRampToValueAtTime(peak, when + 0.0008);
    g.gain.exponentialRampToValueAtTime(0.001, when + (bumped ? 0.045 : 0.018));
    n.connect(bp).connect(g).connect(this.out);
    n.start(when, offs, 0.06);

    // body thump: the head carriage hitting the stop resonates the chassis
    const o = ctx.createOscillator();
    o.type = 'sine';
    o.frequency.setValueAtTime(bumped ? 140 : 220, when);
    o.frequency.exponentialRampToValueAtTime(bumped ? 70 : 150, when + 0.04);
    const og = ctx.createGain();
    og.gain.setValueAtTime(0, when);
    og.gain.linearRampToValueAtTime(bumped ? 0.7 : 0.12, when + 0.001);
    og.gain.exponentialRampToValueAtTime(0.001, when + (bumped ? 0.06 : 0.025));
    o.connect(og).connect(this.out);
    o.start(when); o.stop(when + 0.08);
  }

  /// Feed this frame's head events: flattened [age_seconds, bumped, ...]
  /// from `Machine.drive_head_events`.
  steps(events) {
    if (!this.enabled || !events || !events.length) return;
    const now = this.ctx.currentTime;
    // Too many in one frame (fast mode): thin them out, keep the rhythm.
    const stride = events.length > 160 ? Math.ceil(events.length / 160) * 2 : 2;
    for (let i = 0; i < events.length; i += stride) {
      const when = Math.max(now, now + this.latency - events[i]);
      this.click(when, events[i + 1] > 0);
    }
  }
}
