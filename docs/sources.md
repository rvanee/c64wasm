# Information sources

What the emulator is based on, and how each source was used. Where sources
disagreed, the tie-breaker was always the real thing: the real ROMs
running on the emulated hardware, measurements of real chips, or VICE
running the same test program.

## How facts were established

| Method | Used for |
|---|---|
| **Measurements of real chips** | Every 6502 opcode, legal and illegal, cycle by cycle (SingleStepTests) |
| **VICE as a reference** | CIA timer delays, interrupt latency, raster interrupt timing, sprite MC/MCBASE and sprite crunch, the colour of c-accesses during the first BA cycles, the 1541 write-path timing. Test programs ran in VICE x64sc 3.7.1 and the values were read from its monitor |
| **The real ROMs** | The 1541's VIA wiring, edge polarities, stepper direction, write-protect polarity, motor spin-up, formatter behaviour: read from what the DOS code does (`$EB2F`, `$E85B`, `$F259`, `$FA2E`, `$FB00`, `$FDA3`, `$F5CA`, ...) and confirmed by running it. The KERNAL's boot, keyboard scan and serial routines |
| **Datasheets** | 6522 register behaviour (WDC W65C22S) |
| **Schematic-level models** | The 1541's ATN-acknowledge gate (MAME) |
| **Screenshots** | Demo scenes compared with VICE (e.g. the Performers face) |

## Processors

- **SingleStepTests/65x02** (Tom Harte et al.),
  <https://github.com/SingleStepTests/65x02>: JSON test vectors for every
  NMOS 6502 opcode, with the full bus trace of each case. The reference for
  the CPU core, including JAM, ANE/LXA, ARR in decimal mode and the
  SHA/SHX/SHY/TAS address corruption. Run with `src/bin/tomharte.rs`.
- **Bruce Clark, "Decimal Mode in NMOS 6500 series"**,
  <http://www.6502.org/tutorials/decimal_mode.html>: which flags ADC and SBC
  set in decimal mode.
- The ANE/LXA constant `$EE` is the value most often given for real
  6510s in the literature on undocumented opcodes; real chips vary.

## VIC-II

- **Christian Bauer, "The MOS 6567/6569 video controller (VIC-II) and its
  application in the Commodore 64"** (1996),
  <http://www.zimmers.net/cbmpics/cbm/c64/vic-ii.txt>: the cycle tables,
  bad lines, VC/RC/VMLI, sprite DMA, the border flip-flops. The structure
  of `vic/mod.rs` follows it.
- **VICE** (x64sc), <https://vice-emu.sourceforge.io/>: where Bauer's
  description is incomplete: MCBASE = MC in cycle 16, the sprite-crunch
  formula, the CPU's data during the first three BA cycles.
- **Pepto's palette** (Philip Timmermann's measurements of a PAL C64),
  <https://www.pepto.de/projects/colorvic/>.

## SID

- **reSID** by Dag Lem (part of VICE and libsidplayfp,
  <https://github.com/libsidplayfp/resid>): the oscillator, the noise LFSR
  taps and output bits, the envelope rate periods, the exponential decay
  steps and the ADSR delay bug. The analog part (filter curve, DC offset)
  is a simplification, not reSID's model.

## CIA and VIA

- **VICE** with `tests/data/cia_timing.prg` and `raster_irq.prg`: the
  6526 timer start, stop and force-load delays, and interrupt latency.
- **WDC W65C22S datasheet**, <https://eater.net/datasheets/w65c22.pdf>:
  6522 registers, IFR/IER, PCR and ACR modes, the T1 free-run period
  (N + 2).

## 1541 and disks

- **The DOS ROM itself** (325302-01 + 901229-01/-05), disassembled and run:
  see the table above and [1541.md](1541.md).
- **MAME**, `src/devices/bus/cbmiec/c1541.cpp` and its `c64h156` gate array
  (<https://github.com/mamedev/mame>): DATA is pulled low while ATN and
  ATNA differ (`m_atni ^ m_atna`); the motor has no spin-up ramp.
- **VICE**'s 1541 write path: the value on VIA2 port A is committed over
  the byte time that just ended.
- **Peter Schepers, D64.TXT and G64.TXT**,
  <http://unusedino.de/ec64/technical/formats/d64.html> and
  <http://unusedino.de/ec64/technical/formats/g64.html>: the image formats,
  BAM layout, disk ID location.
- **Linus Åkesson, "GCR decoding on the fly"**,
  <https://www.linusakesson.net/programming/gcr-decoding/index.php>, and
  **pagetable.com, "Anatomy of the 4040 Disk Drive"**,
  <https://www.pagetable.com/docs/anatomy-4040.html>: the GCR table and
  the header and data block layouts, which agree with each other.
- Commented 1541 ROM disassemblies, to read the DOS:
  g3sl's (<https://g3sl.github.io/c1541rom.html>), Marko Mäkelä's on
  retroisle.com
  (<https://retroisle.com/commodore/c64128/Technical/Firmware/1541romdisassembly.php>)
  and Frank Kontros' on ffd2.com
  (<http://www.ffd2.com/fridge/docs/1541dis.html>).
- Speed zones and sectors per track: Schepers, the 1541 service manual and
  several community descriptions, which agree.

## C64

- The KERNAL and BASIC ROMs: boot, keyboard scan, the serial protocol and
  the BASIC tokenizer (`CRUNCH` at `$A579`), whose rules `tools::basic`
  copies and `tests/disk_basic_roundtrip.rs` checks against the running
  ROM.
- **Michael Steil's c64ref** (<https://github.com/mist64/c64ref>, as on
  pagetable.com): the commented BASIC and KERNAL disassembly (Lee
  Davison's), for reading the KERNAL and as a source for rebuilding the
  ROMs.
- The PLA's memory configurations, the keyboard matrix and the CIA2 serial
  and bank bits are standard C64 facts, cross-checked by booting the real
  ROMs and typing through the matrix.

## ROM identification and rebuilding

- **MAME's and VICE's ROM sets**: the CRC32s and chip numbers of the real
  ROMs (`web/src/roms/slots.js`).
- Public listings used to rebuild the ROMs: mist64/c64ref,
  lagomorph/c64rom, mobluse/chargen-maker
  (<https://github.com/mobluse/chargen-maker>), a PETSCII chart on
  Wikimedia Commons, and g3sl's 1541 disassembly. See
  [web.md](web.md#roms-roms-machinerom-setjs-uirom-managerjs).

## Hardware for the desk

- Photos of a desk with a C64, a 1541 and a Commodore 2002 monitor, and
  of the same desk with a flat panel (`web/assets/`).
- The 2002's controls behind its door: H-position, V-hold, colour, tint,
  brightness, contrast, volume and the COMP/SEP input switch.
- The 1541's 16×2 display is a modern add-on, not original hardware.
