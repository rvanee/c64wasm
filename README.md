# c64wasm

A cycle-exact Commodore 64 with a 1541 disk drive, written in Rust and
running in the browser through WebAssembly, shown on a Commodore 2002
monitor on a desk.

By R.F. van Ee. Licensed under MIT or Apache-2.0, at your option.

**Live demo: <https://rvanee.github.io/c64wasm/>.** GitHub Actions builds
it from the sources in this repository and publishes it on every push to
`main`. The first time you open it, it asks for the Commodore ROMs; click
**Find them automatically** (see [ROMs](#roms)).

- **6510 CPU**: validated against Tom Harte's single-step tests, with
  interrupt timing checked against VICE. The same 6502 core runs the 1541.
- **VIC-II**: modelled cycle by cycle after Christian Bauer's description.
  It handles bad lines, sprites, opened borders and mid-line register tricks
  (FLD, FLI, line crunch, DMA delay).
- **CIAs**: the 6526's timer pipeline delays, measured against VICE.
- **SID**: 6581 or 8580, with filters and digi playback.
- **A real 1541**: its own 6502, two VIAs, the GCR disk surface and the
  original DOS ROM. Fast loaders and demos run, and disks can be written and
  saved.
- **The desk**: photos of a C64, a 1541 and a monitor, with the live
  picture on the monitor's screen and a zoom onto it. Choose between two
  monitors:
  - a Commodore 2002, with a CRT simulation. Its power switch and red LED
    work, and so do the controls behind its door: position, vertical hold,
    colour, tint, brightness, contrast, volume, and the COMP/SEP input
    switch;
  - a modern flat panel.

  The video can come over RF, composite or separate luma/chroma, with
  realistic artefacts. The C64 and 1541 LEDs glow. The 1541 has an add-on
  display showing the disk's name, and you hear the drive. One power button
  switches everything; off and on again is the reset.

## ROMs

The emulator runs Commodore's original firmware: BASIC, the KERNAL, the
character ROM and the 1541 DOS ROM. These are copyrighted, so they are not
in this repository or on the web page. The first time the page opens, it
asks for them and offers two choices:

- **Find them automatically.** The page downloads public listings and
  rebuilds each ROM:
  - [mist64/c64ref](https://github.com/mist64/c64ref)'s commented BASIC and
    KERNAL disassembly (the source of pagetable.com's);
  - [mobluse/chargen-maker](https://github.com/mobluse/chargen-maker)'s
    character set drawn as text;
  - g3sl.github.io's commented 1541 disassembly.
- **Specify them myself.** For each ROM type, give a file or a URL. That can
  be a ROM image, a disassembly or hex dump in any common format (web pages
  too), or, for the character ROM, a picture of the character set. Some
  sites don't let other pages download them; the page then asks you to copy
  the site's page and paste it.

Each type keeps a list of every ROM added, so you can switch between, say,
two KERNAL versions in the settings.

Every rebuilt ROM is checked against the CRC32 of the real chip. Listings on
the web have typos: mistyped addresses, missing bytes, overlapping lines,
and unused space left out. The page tries the plausible corrections and
solves a few missing bytes from the checksum. It accepts a repair only when
exactly one reading matches a real ROM. An incomplete listing can be
completed with a second one; the two are merged. ROMs are stored only in
your browser (IndexedDB).

## Building

You need Rust (via [rustup](https://rustup.rs)) and Python 3 for the local
web server.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
cargo build --release --target wasm32-unknown-unknown -p c64-wasm
wasm-bindgen target/wasm32-unknown-unknown/release/c64_wasm.wasm --out-dir web/pkg --target web --no-typescript
python serve.py          # then open http://localhost:8000/
```

`build_wasm.sh` (Linux/macOS) and `build_wasm.cmd` (Windows) do the steps
before `serve.py`. `serve.py` tells the browser not to cache, so a rebuild
shows up with a normal reload, and it serves the bundled test disks.

The page itself has no build step: it is plain ES modules in `web/src/`.

## Tests

```sh
node web/tools/rebuild-roms.mjs       # put the ROMs in roms/ (Node 20+), once
cargo test --release --workspace      # the emulator
cd web && node --test                 # ROM rebuilding
```

Tests that need Commodore ROMs look for them in `roms/` (see
`roms/README.md`) and skip themselves when they aren't there.
`web/tools/rebuild-roms.mjs` rebuilds them from the same public listings
the page uses and checks each against the real chip. CI does the same
for every run. With ROMs present, the suite also covers:

- booting to READY;
- typing through the keyboard matrix;
- formatting a disk, SAVEing and LOADing through the real 1541 DOS;
- disk round-trips;
- rebuilding ROMs from damaged listings.

The CIA and interrupt timing tests (`crates/c64-core/tests/cia_timing.rs`)
run small machine-code programs and compare the results with VICE.

## Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request. Each run:

1. checks formatting (`cargo fmt`) and lints (`cargo clippy`);
2. rebuilds the Commodore ROMs from public listings, for this run only;
   they are never committed, cached or uploaded;
3. runs the Rust and JavaScript tests, including those that need the ROMs;
4. builds the WebAssembly page;
5. uploads it as a downloadable artifact (`c64wasm-web.zip`).

If a listing site is down, that ROM is missing for the run. The run shows a
warning, and the tests that need the ROM skip themselves.

Pushes to `main` also publish the page to GitHub Pages. Tags like `v1.0`
also attach the zip to a GitHub release. See `docs/publishing.md`.

## Layout

### `crates/c64-core`

The emulator, in plain Rust with no dependencies. Each chip or unit is its
own module.

| Module      | Contents                                                         |
|-------------|------------------------------------------------------------------|
| `cpu`       | 6502 core; `Mos6510` adds the I/O port                           |
| `vic`       | VIC-II: model, sequencer, sprites, palette                       |
| `sid`       | SID: oscillator, envelope, voice, filter, output, model          |
| `cia`       | 6526 CIA                                                         |
| `via`       | 6522 VIA                                                         |
| `port`      | parallel port shared by the CIA and VIA                          |
| `memory`    | `Ram<N>`, `Rom<N>`                                               |
| `clock`     | `VideoStandard`, plus `ClockBridge` between two crystals         |
| `led`       | indicator LEDs                                                   |
| `cbm_iec`   | the serial bus                                                   |
| `media`     | disks, GCR, D64/G64                                              |
| `c64`       | the C64: board, PLA, keyboard matrix, serial port                |
| `drive1541` | the 1541: `mechanics` (motor, stepper, head, disk) and `electronics` (logic board, read/write channel, serial interface) |
| `tools`     | BASIC tokenizer and ROM file loading, for tests and examples     |

Chips that came in several versions take a model enum: `VicModel`,
`SidModel`, `VideoStandard`, `LedColor`.

The crate also contains:

- `examples/`: `demo_probe`, `vic_trace`, `run_prg`, `sid_wav`;
- `src/bin/`: `make_test_disks`, and `tomharte` for the CPU test vectors;
- `test-disks/`: the bundled disks.

### `crates/c64-wasm`

The JavaScript bindings.

### `web/`

The page:

- `src/machine/`: the emulator and the ROM set;
- `src/display/`: CRT renderer, desk scene (both photos), Commodore 2002,
  LEDs, 1541 display;
- `src/input/`: keyboard map, keyboard, BASIC typer;
- `src/audio/`: SID player and drive sounds;
- `src/media/`: disks;
- `src/roms/`: ROM library storage, identification, listing parser,
  repair, character ROM, sources;
- `src/ui/`: settings, ROM manager, first-run dialog, knobs;
- `test/`: Node tests.

### `docs/`

Design notes and publishing steps.

## Credits

- Christian Bauer's VIC-II article.
- The VICE team, used as the timing reference.
- MAME's ROM CRC tables.
- Michael Steil's c64ref disassemblies.
- Marko Mäkelä's and g3sl's 1541 disassemblies.
- mobluse's chargen-maker.
- Tom Harte's 6502 tests.

## How this was made

Written by R.F. van Ee with the help of Claude (Anthropic), an AI
assistant. Claude was used for:

- code generation and refactoring;
- writing tests;
- checking emulation against VICE.

Design decisions, reviews and testing on real software are the author's.
