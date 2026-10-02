// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The SID sound chip (MOS 6581/8580): three voices, the filter, the
//! mixer and volume, and resampling to the host's audio rate.
//!
//! The digital side (oscillators, noise, envelopes) follows Dag Lem's
//! reSID, which documents it from die analysis. The analog side is
//! simplified: combined waveforms are ANDed, the filter is an ideal
//! state-variable filter with an approximate cutoff curve per model, and
//! the 6581's mixer DC offset is a single constant (enough for volume
//! register "digis"). Paddle inputs read $FF.

mod envelope;
mod filter;
mod model;
mod oscillator;
mod output;
mod voice;

pub use envelope::RATE_PERIODS;
pub use model::SidModel;

use filter::Filter;
use oscillator::CTRL_SYNC;
use output::AudioOutput;
use voice::Voice;

#[derive(Clone, Debug)]
pub struct Sid {
    model: SidModel,
    voices: [Voice; 3],
    /// 11-bit filter cutoff.
    fc: u16,
    /// Resonance (high nibble) and filter routing (low nibble).
    res_filt: u8,
    /// Filter mode (high nibble) and volume (low nibble).
    mode_vol: u8,
    /// Last value written: reading a write-only register returns what's
    /// left on the data bus (reSID's `bus_value`).
    bus_value: u8,
    filter: Filter,
    clock_hz: u32,
    output: AudioOutput,
}

impl Sid {
    pub fn new(model: SidModel, clock_hz: u32) -> Self {
        let mut s = Sid {
            model,
            voices: [Voice::new(), Voice::new(), Voice::new()],
            fc: 0,
            res_filt: 0,
            mode_vol: 0,
            bus_value: 0,
            filter: Filter::new(),
            clock_hz,
            output: AudioOutput::new(clock_hz),
        };
        s.update_filter();
        s
    }

    pub fn model(&self) -> SidModel {
        self.model
    }

    /// Audio output rate for `take_samples` (default 44100 Hz). 0 disables
    /// sample collection (the chip itself keeps running).
    pub fn set_sample_rate(&mut self, hz: u32) {
        self.output.set_sample_rate(hz);
    }

    pub fn sample_rate(&self) -> u32 {
        self.output.sample_rate()
    }

    /// Move the audio produced since the last call into `out`: mono f32
    /// samples in roughly -1..1.
    pub fn take_samples(&mut self, out: &mut Vec<f32>) {
        self.output.take(out);
    }

    /// Envelope level of a voice (0-255).
    pub fn envelope(&self, voice: usize) -> u8 {
        self.voices[voice].env.level()
    }

    /// Phase accumulator of a voice (24 bits).
    pub fn accumulator(&self, voice: usize) -> u32 {
        self.voices[voice].osc.acc
    }

    /// 12-bit waveform output of a voice right now.
    pub fn waveform_output(&self, voice: usize) -> u16 {
        self.voices[voice].waveform(self.voices[(voice + 2) % 3].osc.acc & 0x80_0000 != 0)
    }

    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x1F {
            0x19 | 0x1A => 0xFF,
            0x1B => (self.waveform_output(2) >> 4) as u8,
            0x1C => self.voices[2].env.level(),
            _ => self.bus_value,
        }
    }

    pub fn write(&mut self, reg: u8, val: u8) {
        self.bus_value = val;
        let reg = reg & 0x1F;
        if reg < 0x15 {
            self.voices[(reg / 7) as usize].write(reg % 7, val);
            return;
        }
        match reg {
            0x15 => {
                self.fc = (self.fc & 0x7F8) | (val & 0x07) as u16;
                self.update_filter();
            }
            0x16 => {
                self.fc = (self.fc & 0x007) | ((val as u16) << 3);
                self.update_filter();
            }
            0x17 => {
                self.res_filt = val;
                self.update_filter();
            }
            0x18 => self.mode_vol = val,
            _ => {}
        }
    }

    fn update_filter(&mut self) {
        self.filter.configure(self.model.cutoff_hz(self.fc), self.res_filt >> 4, self.clock_hz);
    }

    /// Advance the chip by one phi2 cycle.
    #[inline]
    pub fn clock(&mut self) {
        for v in self.voices.iter_mut() {
            v.osc.clock(v.control);
        }
        // Hard sync: voice n is reset by voice n-1's MSB rising edge.
        let rising = [self.voices[0].osc.msb_rising, self.voices[1].osc.msb_rising, self.voices[2].osc.msb_rising];
        for i in 0..3 {
            let src = (i + 2) % 3;
            if rising[src] && self.voices[i].control & CTRL_SYNC != 0 {
                self.voices[i].osc.acc = 0;
            }
        }
        for v in self.voices.iter_mut() {
            v.env.clock();
        }
        let out = self.mix();
        self.output.push(out);
    }

    /// Instantaneous analog output (before AC coupling), scaled so one
    /// voice at full envelope and full volume is about +/-0.33.
    #[inline]
    fn mix(&mut self) -> f32 {
        let mut filt_in = 0.0f32;
        let mut direct = 0.0f32;
        for i in 0..3 {
            let v = &self.voices[i];
            let ring_msb = self.voices[(i + 2) % 3].osc.acc & 0x80_0000 != 0;
            let wave = v.waveform(ring_msb) as i32 - 0x800;
            let s = (wave * v.env.level() as i32) as f32 / (2048.0 * 255.0);
            if self.res_filt & (1 << i) != 0 {
                filt_in += s;
            } else if !(i == 2 && self.mode_vol & 0x80 != 0) {
                // bit 7 of $D418 disconnects voice 3
                direct += s;
            }
        }

        let f = self.filter.step(filt_in);
        let mut filtered = 0.0;
        if self.mode_vol & 0x10 != 0 {
            filtered += f.lp;
        }
        if self.mode_vol & 0x20 != 0 {
            filtered += f.bp;
        }
        if self.mode_vol & 0x40 != 0 {
            filtered += f.hp;
        }

        let vol = (self.mode_vol & 0x0F) as f32 / 15.0;
        (direct + filtered + self.model.mixer_dc()) * vol / 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLK: u32 = 985_248;

    fn run(s: &mut Sid, cycles: u32) {
        for _ in 0..cycles {
            s.clock();
        }
    }

    #[test]
    fn sawtooth_period_matches_frequency_register() {
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.set_sample_rate(0);
        s.write(0x00, 0x00);
        s.write(0x01, 0x10); // freq = $1000 -> 2^24 / 4096 = 4096 cycles per period
        s.write(0x04, 0x20); // sawtooth, gate off
        let mut wraps = 0;
        let mut prev = s.accumulator(0);
        for _ in 0..4096 * 10 {
            s.clock();
            let a = s.accumulator(0);
            if a < prev {
                wraps += 1;
            }
            prev = a;
        }
        assert_eq!(wraps, 10);
        assert_eq!(s.waveform_output(0), (s.accumulator(0) >> 12) as u16);
    }

    #[test]
    fn test_bit_holds_oscillator_at_zero() {
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x01, 0x20);
        s.write(0x04, 0x28); // sawtooth + TEST
        run(&mut s, 1000);
        assert_eq!(s.accumulator(0), 0);
        s.write(0x04, 0x20);
        run(&mut s, 10);
        assert_eq!(s.accumulator(0), 0x2000 * 10);
    }

    #[test]
    fn attack_takes_255_rate_periods_then_decays_to_sustain() {
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x05, 0x00); // attack 0 (9 cycles/step), decay 0
        s.write(0x06, 0xA0); // sustain $A -> level $AA, release 0
        s.write(0x04, 0x01); // gate on
        run(&mut s, 9 * 255 - 1);
        assert!(s.envelope(0) >= 0xFE, "env {} after ~255 attack steps", s.envelope(0));
        run(&mut s, 20);
        assert!(s.envelope(0) >= 0xFC, "at the peak, decay just starting");
        run(&mut s, 50_000);
        assert_eq!(s.envelope(0), 0xAA, "decays to sustain level and holds there");
        s.write(0x04, 0x00); // gate off -> release
        run(&mut s, 200_000);
        assert_eq!(s.envelope(0), 0, "released to zero and frozen there");
        run(&mut s, 50_000);
        assert_eq!(s.envelope(0), 0);
    }

    #[test]
    fn slow_attack_rate_is_respected() {
        // Attack $8 = 392 cycles per step: after 100 steps the envelope is
        // at 100 (allowing for the counter phase at gate-on).
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x05, 0x80);
        s.write(0x04, 0x01);
        run(&mut s, 392 * 100 + 10);
        let e = s.envelope(0);
        assert!((99..=101).contains(&e), "env {e}");
    }

    #[test]
    fn adsr_delay_bug_when_rate_lowered_below_counter() {
        // Start a slow attack, let the 15-bit rate counter run up, then
        // switch to the fastest rate: the counter has already passed 9, so
        // the next step only comes after it wraps at 32768.
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x05, 0xF0); // attack 15: 31251 cycles/step
        s.write(0x04, 0x01);
        run(&mut s, 1000);
        let before = s.envelope(0);
        s.write(0x05, 0x00); // attack 0
        run(&mut s, 30_000);
        assert_eq!(s.envelope(0), before, "no step until the counter wraps");
        run(&mut s, 3_000);
        assert!(s.envelope(0) > before);
    }

    #[test]
    fn noise_is_not_silent_and_changes() {
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x01, 0x40);
        s.write(0x04, 0x80); // noise
        let mut seen = std::collections::HashSet::new();
        for _ in 0..20_000 {
            s.clock();
            seen.insert(s.waveform_output(0));
        }
        assert!(seen.len() > 50, "noise produced only {} distinct values", seen.len());
    }

    #[test]
    fn osc3_and_env3_are_readable() {
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x0E + 1, 0x10); // voice 3 freq hi (reg $0F)
        s.write(0x12, 0x21); // voice 3 sawtooth + gate
        run(&mut s, 5000);
        assert_eq!(s.read(0x1B), (s.accumulator(2) >> 16) as u8);
        assert_eq!(s.read(0x1C), s.envelope(2));
        assert_eq!(s.read(0x19), 0xFF);
    }

    #[test]
    fn hard_sync_resets_the_synced_voice() {
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.write(0x01, 0x80); // voice 1: fast, wraps every 256 cycles
        s.write(0x08, 0x01); // voice 2: very slow
        s.write(0x0B, 0x22); // voice 2: sawtooth + SYNC
        run(&mut s, 2000);
        assert!(s.accumulator(1) < 0x100 * 256, "voice 2 keeps getting reset by voice 1's MSB");
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn tone(model: SidModel, freq_reg: u16, lowpass_fc: Option<u16>) -> Vec<f32> {
        let mut s = Sid::new(model, CLK);
        s.set_sample_rate(44_100);
        s.write(0x00, freq_reg as u8);
        s.write(0x01, (freq_reg >> 8) as u8);
        s.write(0x05, 0x00);
        s.write(0x06, 0xF0);
        if let Some(fc) = lowpass_fc {
            s.write(0x15, (fc & 7) as u8);
            s.write(0x16, (fc >> 3) as u8);
            s.write(0x17, 0x01); // voice 1 through the filter
            s.write(0x18, 0x1F); // LP, volume 15
        } else {
            s.write(0x18, 0x0F);
        }
        s.write(0x04, 0x21); // sawtooth, gate
        run(&mut s, CLK / 2);
        let mut out = Vec::new();
        s.take_samples(&mut out);
        out.split_off(out.len() / 2) // steady-state half
    }

    #[test]
    fn produces_audio_at_the_expected_pitch() {
        // 440 Hz: freq = 440 * 2^24 / 985248 = 7493.
        let samples = tone(SidModel::Mos6581, 7493, None);
        assert!(rms(&samples) > 0.05, "rms {}", rms(&samples));
        // A sawtooth ramps up through zero exactly once per period.
        let ups = samples.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let secs = samples.len() as f32 / 44_100.0;
        let hz = ups as f32 / secs;
        assert!((hz - 440.0).abs() < 10.0, "measured {hz} Hz");
    }

    #[test]
    fn lowpass_filter_attenuates_a_high_tone() {
        // 4 kHz sawtooth through a low cutoff vs unfiltered.
        let freq = (4000.0 * 16_777_216.0 / CLK as f32) as u16;
        let open = rms(&tone(SidModel::Mos8580, freq, None));
        let closed = rms(&tone(SidModel::Mos8580, freq, Some(40))); // ~260 Hz
        assert!(closed < open * 0.2, "filtered rms {closed} vs unfiltered {open}");
    }

    #[test]
    fn volume_register_writes_are_audible_on_a_6581() {
        // $D418 digi: toggling the volume with no voices playing makes a
        // square wave out of the mixer DC offset.
        let mut s = Sid::new(SidModel::Mos6581, CLK);
        s.set_sample_rate(44_100);
        for i in 0..200 {
            s.write(0x18, if i % 2 == 0 { 0x0F } else { 0x00 });
            run(&mut s, 1000);
        }
        let mut out = Vec::new();
        s.take_samples(&mut out);
        assert!(rms(&out[out.len() / 2..]) > 0.02);
    }
}
