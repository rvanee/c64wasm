# The SID

Source: `crates/c64-core/src/sid/`.

| File | Contents |
|---|---|
| `mod.rs` | `Sid`: registers, hard sync, the mixer, `clock` |
| `voice.rs` | one voice: oscillator + envelope + control register |
| `oscillator.rs` | phase accumulator, noise LFSR, the four waveforms |
| `envelope.rs` | ADSR envelope generator |
| `filter.rs` | the filter (state-variable) |
| `output.rs` | down-sampling to the host rate and the output capacitor |
| `model.rs` | `SidModel`: 6581 and 8580 |

The digital part (oscillators, noise, envelopes) follows Dag Lem's reSID,
which was derived from die analysis. The analog part is deliberately
simple: an ideal filter with a per-model cutoff curve, and one DC offset
for the mixer. That is enough for music, sound effects and volume-register
"digis"; it is not a model of the 6581's filter distortion.

## Clocking

`Board::tick` calls `Sid::clock()` once per CPU cycle (985,248 Hz PAL,
1,022,727 Hz NTSC). Each call:

1. clocks the three oscillators;
2. applies hard sync: voice n's accumulator is reset when voice n−1's
   accumulator MSB rose in this cycle (voice 1 is synced by voice 3) and
   n's SYNC bit is set;
3. clocks the three envelopes;
4. mixes one output value and hands it to the output stage.

## Oscillator

A 24-bit phase accumulator adds the 16-bit frequency register every
cycle, so the output frequency is `freq × clock / 2^24`.

| Waveform | 12-bit output |
|---|---|
| triangle | accumulator bits 22-11, inverted when the MSB is set (the MSB XOR the previous voice's MSB with RING) |
| sawtooth | accumulator bits 23-12 |
| pulse | `$FFF` while bits 23-12 ≥ the 12-bit pulse width (or TEST is set), else 0 |
| noise | 8 bits of a 23-bit LFSR, placed at output bits 11-4 |

The LFSR (taps 22 and 17, reset to `$7FFFF8`) shifts when accumulator bit
19 rises, so the noise pitch follows the frequency register. TEST holds
the accumulator at 0 and resets the LFSR. Several waveform bits at once
are ANDed together; the real chip's combined waveforms are more complex
(a known simplification).

Registers `$D41B` (OSC3: the upper 8 bits of voice 3's waveform) and
`$D41C` (ENV3: voice 3's envelope) are readable, which programs use for
random numbers and modulation.

## Envelope

An 8-bit level counter, stepped by a 15-bit rate counter:

- The rate periods for the 16 A/D/R values are reSID's:
  9, 32, 63, 95, 149, 220, 267, 313, 392, 977, 1954, 3126, 3907, 11720,
  19532, 31251 cycles (`RATE_PERIODS`).
- The rate counter is compared for equality and wraps at 2^15. Lowering
  the rate while the counter is already past the new period makes it run
  all the way round first: the real "ADSR delay bug" (tested in
  `adsr_delay_bug_when_rate_lowered_below_counter`).
- Attack counts up by one per period to `$FF`, then decay starts.
- Decay and release count down with an extra divider that grows as the
  level falls (1, 2, 4, 8, 16, 30 at levels `$FF`, `$5D`, `$36`, `$1A`,
  `$0E`, `$06`), giving the piecewise-exponential curve. Decay stops at
  sustain × `$11`.
- At level 0 the counter is frozen until the next attack.

A gate bit going from 0 to 1 starts the attack; 1 to 0 starts the
release.

## Filter and mixer

Each voice's output is `(waveform − $800) × envelope`, scaled to about
±1/3 at full level. Voices whose bit is set in `$D417` go through the
filter; the others go straight to the mixer. Bit 7 of `$D418` disconnects
voice 3 when it is not filtered.

The filter is a Chamberlin state-variable filter, run every cycle:

```
hp = in − lp − bp / Q
bp += f × hp
lp += f × bp
f = 2 sin(π × cutoff / clock),   Q = 0.707 + resonance / 8
```

`$D418` bits 4-6 select which of LP, BP and HP are summed. The cutoff in
Hz comes from the 11-bit FC register (`$D415` bits 0-2, `$D416`):

- **8580**: linear, 30 Hz to 12 kHz;
- **6581**: a piecewise-linear table from 220 Hz at FC = 0 to 12.3 kHz at
  `$7FF`, with the steep rise around FC = `$300` that 6581 tunes rely on.
  Real 6581s differ a lot from chip to chip; the table is a typical one.

The mixer adds a DC offset (0.35 for the 6581, 0.02 for the 8580) and
multiplies everything by the volume (`$D418` bits 0-3). Because the offset
goes through the volume too, writing the volume register produces a
step in the output: that is how 4-bit "digis" play on a 6581, and why they
are almost silent on an 8580, as on the real machines.

## Output stage

`AudioOutput` turns one value per cycle into samples at the host's rate
(set with `set_sample_rate`, by default 44,100 Hz; the page uses the
AudioContext's rate):

1. **Down-sampling**: the values of all cycles that fall into one output
   sample are averaged (a box filter), with an exact integer phase
   accumulator, so there is no drift between the C64's clock and the
   audio clock.
2. **Output capacitor**: a first-order high-pass at 16 Hz removes the
   mixer's DC offset, like the coupling capacitor on the C64's audio
   output.

Samples collect in a buffer that `take_samples` empties. If nobody takes
them, the oldest are dropped beyond about two seconds. A rate of 0 stops
collecting (the chip keeps running).

## Registers

Writes to `$D400-$D414` go to the three voices (seven registers each:
frequency lo/hi, pulse width lo/hi (12 bits), control, attack/decay,
sustain/release). `$D415-$D418` are the filter and volume. Reading a
write-only register returns the last value written to any SID register
(what is left on the data bus). The paddle registers `$D419`/`$D41A` read
`$FF`.

## Models

`SidModel::Mos6581` (default, as in the original C64) or `Mos8580` (the
C64C). The page's setting switches between them; switching replaces the
chip, so the registers start from zero again.

## Not modelled

- The analog behaviour of the 6581's filter (distortion, its dependence on
  the input level) and chip-to-chip spread.
- Combined-waveform tables (combined waveforms are a plain AND).
- The paddle (POT) inputs.
