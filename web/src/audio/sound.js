// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Sound: the SID's samples go to an AudioWorklet, the 1541's mechanics to
// DriveSound. Browsers only start audio after a click or key press, so it
// starts on the first one, unless the volume is all the way down.

import { DriveSound } from './drive-sound.js';

/// Perceived loudness for a 0-1 slider.
export const volumeCurve = (v) => Math.pow(+v, 2);

export class Sound {
  constructor() {
    this.ctx = null;
    this.player = null;   // AudioWorkletNode
    this.drive = null;    // DriveSound
    this.on = false;
  }

  get sampleRate() { return this.on && this.ctx ? this.ctx.sampleRate : 0; }

  /// Start audio (call from a click or key handler). Throws if impossible.
  async start({ volume, driveSounds, driveVolume }) {
    if (!this.ctx) {
      this.ctx = new AudioContext({ latencyHint: 'interactive' });
      await this.ctx.audioWorklet.addModule(new URL('./sid-worklet.js', import.meta.url));
      this.player = new AudioWorkletNode(this.ctx, 'sid-player', { numberOfInputs: 0, outputChannelCount: [2] });
      this.player.connect(this.ctx.destination);
      this.drive = new DriveSound(this.ctx, this.ctx.destination);
    }
    await this.ctx.resume();
    this.on = true;
    this.player.port.postMessage({ volume: volumeCurve(volume), flush: true });
    this.drive.setEnabled(driveSounds);
    this.drive.setVolume(volumeCurve(driveVolume));
  }

  stop() {
    this.on = false;
    this.flush();
    this.drive?.setMotor(false, true);
    this.ctx?.suspend();
  }

  /// Drop buffered samples (the machine was switched off).
  flush() { this.player?.port.postMessage({ flush: true }); }

  setVolume(v) { this.player?.port.postMessage({ volume: volumeCurve(v) }); }
  setDriveSounds(on) { this.drive?.setEnabled(on); }
  setDriveVolume(v) { this.drive?.setVolume(volumeCurve(v)); }

  /// Pass on this frame's output of the machine.
  pump(machine, driveAttached) {
    const samples = machine.take_audio();
    if (this.on && samples.length) this.player.port.postMessage({ samples }, [samples.buffer]);
    if (!driveAttached) return;
    const steps = machine.drive_head_events(0);
    if (this.on) {
      this.drive.setMotor(machine.drive_motor(0));
      this.drive.steps(steps);
    }
  }
}
