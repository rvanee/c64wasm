# The web page

Source: `web/`. Plain ES modules, no framework and no build step; the only
generated files are the WebAssembly module and its JavaScript glue in
`web/pkg/` (`build_wasm.sh` / `build_wasm.cmd`). `serve.py` serves `web/`
locally with caching off and maps `/disks/` to the bundled test disks.

## Start-up

In order:

1. The settings are read from `localStorage` when `settings.js` is
   imported.
2. `main.js` builds the renderer, the scene, the 2002's controls, the
   LEDs, the 1541 display, the ROM manager, the dialogs and the keyboard.
3. The frame loop starts (the desk is shown with the machine off).
4. `pkg/c64_wasm.js` is loaded. If it isn't there (the Rust part hasn't
   been built), `DemoMachine` stands in: two still pictures and a blinking
   drive LED, so the page itself can be worked on without Rust.
5. The ROM library is loaded from IndexedDB.
6. If BASIC, KERNAL and the character ROM are present, the machine
   switches on (the 1541 DOS ROM is optional; without it the C64 runs
   with no drive). Otherwise the first-run dialog opens (find them
   automatically, or specify them), and the machine switches on when the
   settings close with the ROMs complete.

## Power

One power button switches the whole setup. Switching on builds a new
`Machine` with the active ROMs, attaches a 1541 with the chosen device
number (8-11) if the drive is on and a DOS ROM is present, inserts the
disk from the disk slot, and sets the SID model and audio rate. Switching
off first takes the disk back from the drive (with everything the DOS
wrote to it), then frees the machine. Off and on again is the reset.
Changing a ROM, the video standard, the drive on/off or the device number
power-cycles the machine; the SID model changes without a restart.

The part of the raster the monitor shows is set at power-on
(`renderer.setWindow(x0, y0, w, h)` in framebuffer pixels): PAL
`(50, 7, 402, 287)`, NTSC `(58, 6, 402, 240)`.

The monitor has its own power switch and red LED; with the monitor off the
machine keeps running behind a dark screen.

## Timing

`Emulator.run(ms, fast)` is called once per animation frame, with the
elapsed time capped at 250 ms by the frame loop, so a tab coming back from
the background doesn't try to catch up minutes:

- **Real time**: `ms × clock_hz / 1000` cycles are owed; the whole number
  is run (`run_cycles`), the fraction carried to the next frame.
- **As fast as possible**: 400,000 cycles per frame, whatever the time.

The status line shows the effective speed in MHz.

## Picture: `display/crt-renderer.js`

The emulator's RGBA raster (the whole PAL or NTSC raster, borders and
blanking included) is uploaded as a texture each frame. One WebGL1 fragment
shader turns it into what a monitor shows, followed by a glow pass and a
phosphor-persistence pass:

1. **Signal path**, chosen in the settings (RF, composite or S-Video, or
   on the 2002 by its COMP/SEP switch). Each line is filtered horizontally
   in YUV: luma sharpness and chroma bandwidth per connection; on
   composite and RF the chroma is modulated on the colour subcarrier
   (PAL 0.5625, NTSC 0.4375 cycles per C64 pixel) and leaks into luma and
   back (dot crawl, rainbow cross-colour). RF adds noise, ghosting,
   per-line sync jitter and a rolling hum bar.

   | Connection | Luma sharpness | Chroma | Crosstalk | Noise | Ghost | Jitter | Hum |
   |---|---|---|---|---|---|---|---|
   | S-Video | 4.0 | 0.40 | 0 | 0.006 | 0 | 0 | 0 |
   | composite | 2.6 | 0.30 | 0.09 | 0.014 | 0.025 | 0.04 | 0 |
   | RF | 1.5 | 0.18 | 0.16 | 0.05 | 0.09 | 0.25 | 0.025 |

2. **Monitor controls**: horizontal position, vertical roll (vertical
   hold out of lock), colour (saturation), tint (hue rotation), brightness
   (black level), contrast (gain).
3. **Tube**: each raster line is a Gaussian beam spot whose height grows
   with brightness, so bright areas bloom and dark ones show scanline
   gaps; barrel curvature and a little overscan; the glass outline cut
   from the photo; a subtle slot shadow mask; unlit-phosphor grey and a
   reflection.
4. **Power**: warm-up fade at switch-on; collapse to a line and a fading
   dot at switch-off.

The flat panel uses the same shader with the tube effects off (no
curvature, scanlines, mask or glass). Without WebGL the page falls back to
a smoothed 2D blit. The average colour of the picture lights the desk
around the monitor (`scene.updateSpill`).

## The desk: `display/scene.js`, `monitor-2002.js`, `drive-lcd.js`, `led.js`

The page is a photograph of a desk with a C64, a 1541 and one of two
monitors: a Commodore 2002 or a flat panel (`assets/setup.jpg`,
`assets/setup-flat.jpg`). The canvas is placed exactly over the screen in
the photo; `SCREENS` holds the screen's position as fractions of each
photo (the same numbers are in `style.css`). Zoom scales and moves the
whole desk with a CSS transform so the screen fills the window, then
resizes the canvas to its new on-screen size.

**The 2002**: its power switch and red LED (in `main.js`), and, in
`monitor-2002.js`, the controls behind the door under the screen: horizontal position, vertical
hold, colour, tint, brightness, contrast, volume, and the COMP/SEP switch
(front composite input or rear separate luma/chroma). Vertical hold locks
within ±0.18 of the knob's centre; further out the picture rolls,
faster the further, with the blanking bar showing. Knobs (`ui/knob.js`)
turn by grabbing and dragging around their centre, by the wheel, or with
the arrow keys; a double-click resets them.

**LEDs** (`led.js`) are drawn over the photo: the C64's and the 1541's
power LEDs, the 2002's power LED and the 1541's activity LED, whose
brightness is the duty cycle the emulator reports (smoothed over frames).

**The 1541 display** (`drive-lcd.js`) is a modern add-on, not original
hardware: a 16×2 character LCD on the drive's front, drawn with the C64's
own character ROM. It shows the disk name, ID and head track (read from
the disk's BAM, refreshed when the motor stops after a write). Hovering
enlarges it four times.

## Sound: `audio/`

- **SID** (`sid-worklet.js`): each frame the main thread posts the SID's
  samples (at the AudioContext's rate) to an AudioWorklet. It keeps a
  jitter buffer of about 60 ms; when the emulator runs ahead (fast mode,
  catch-up) the oldest samples are dropped so latency stays bounded; on an
  underrun it fades to silence instead of clicking.
- **Drive** (`drive-sound.js`): synthesized, no samples. The spindle motor
  is a whir with the disk's 5 Hz (300 rpm) swish, spinning up and down;
  each head step is a tick, and a step against the end stop is a knock.
  Steps carry their emulated time, so the rhythm of a head bang is
  reproduced.
- `sound.js` starts and stops the audio graph and sets the volumes;
  `main.js` starts it on the first click or key press (browsers require a
  gesture) unless the volume is all the way down, and handles mute.

## Keyboard: `input/`

The C64 keyboard is a matrix (see [architecture.md](architecture.md#the-c64)).
`keyboard-map.js` maps by **character**: the key a PC user presses to get
`"` is mapped to the C64's SHIFT+2, so typing works on any layout. Keys
without a character (Enter, Backspace, cursor keys, F-keys) map by
position. Dead keys (US-International and many European layouts) are
resolved at once: the character comes from the physical key, and the
composed key press that follows is mapped back. PageUp is RESTORE. Keys
are released when the window loses focus.

`typer.js` types a BASIC listing (from a file) on the matrix, holding each
key for a fixed number of emulated cycles with a pause between keys, so it
works at any speed setting.

## Disks: `media/`

`DiskSlot` holds the disk image, so it survives switching off. Disks come
from a file, a drop on the desk, or the bundled `hello.d64` and
`empty.d64`. A G64 is decoded by the drive when it is taken back. The disk
is taken back from the drive as a D64 at power-off, eject and download, so
everything the DOS wrote is kept.

## ROMs: `roms/`, `machine/rom-set.js`, `ui/rom-manager.js`

The emulator needs Commodore's BASIC, KERNAL, character and 1541 DOS ROMs.
They are copyrighted, so the page never contains or uploads them. Each
visitor's browser gets them once and keeps them in IndexedDB.

### Slots and identification (`slots.js`)

| Slot | Size | Base | Known chips (by CRC32) |
|---|---|---|---|
| BASIC | 8 KB | `$A000` | 901226-01 |
| KERNAL | 8 KB | `$E000` | 901227-01, -02, -03 |
| character | 4 KB | – (a listing's lowest address) | 901225-01 |
| 1541 DOS | 16 KB | `$C000` | 325302-01 + 901229-01, -02, -03, -05, -06 (two halves, identified separately); 1541-II 251968-01, -02, -03, 355640-01 |

A ROM is "verified" only if its CRC32 is a known chip's.

### The library (`store.js`, `rom-set.js`)

Per slot, IndexedDB keeps a shelf: `library:<slot>` →
`{ entries: [{ id, bytes, source, added }], active }`, with the image's
CRC32 as its id. Adding the same image twice finds the existing entry.
The ROM manager lists each shelf and lets the user switch the active
image (for example between two KERNALs); the machine power-cycles with
the new one. ROMs stored by older versions of the page are moved onto the
shelves when the library loads.

### Getting a ROM (`sources.js`)

A ROM can come from:

- a **binary file or URL**: an image of the right size, the same with a
  2-byte PRG header, or a combined 16 KB BASIC+KERNAL ROM (split);
- a **listing**, as a file, URL or pasted text: a disassembly or hex dump
  in any common format, including web pages;
- for the character ROM, a **font drawn as text** or a **picture** of the
  character set.

Some sites don't allow other pages to download them (no CORS headers, or
plain http from an https page). The page then says so and asks the user to
open the page, copy it and paste it. Known pages are rewritten to a
downloadable copy (pagetable.com's disassembly to its plain-text source on
GitHub, GitHub "blob" pages to raw files).

**Find automatically** tries public sources per slot, in order, and keeps
the first verified result:

| Slot | Sources |
|---|---|
| BASIC, KERNAL | mist64/c64ref's commented disassembly (as on pagetable.com); an older copy (lagomorph/c64rom). One download fills both. |
| character | mobluse/chargen-maker's font as text; a picture on Wikimedia Commons |
| 1541 DOS | g3sl.github.io's commented disassembly; its GitHub repository as a fallback |

### Reading listings (`listing.js`)

`parseListing` reads the address and bytes of every line it recognises
and ignores the rest:

| Shape | Example |
|---|---|
| disassembly | `E000  85 56     STA $56` |
| VICE monitor, c64disasm | `.,E000 85 56  STA $56`, `.:A000 94 E3` |
| with colon | `$E000: 85 56` |
| line number first | `0010  E000  85 56   STA $56` |
| label column first | `SETLDA  C100  78  SEI` |
| xxd | `0000e000: 8556 2000 ...` |
| hexdump -C | `0000e000  85 56 20 ...  \|...\|` |
| data directives | `FFE6 .WD $C8C6`, `FFCF .BY $AA,$AA` (also `.BYTE`, `.DB`, `!byte`, `.WORD`, `.DW`, `!word`) |
| elided range | `C001  AA ...` then `C0FF  ... AA` |

Bytes are hex pairs separated by single spaces; the first wider gap ends
them, so a comment that starts with hex-looking text ("12 return without
gosub") isn't read as data. HTML is turned into text first (table cells
become columns). Every line is kept with its address, so wrong addresses
can be repaired later.

### Building and repairing (`repair.js`, `crc32.js`)

`buildImage` lays the lines of one or more listings out in a ROM image.
Later listings win where they overlap; earlier ones fill what later ones
lack, so an incomplete listing can be completed with a second one. The
base is the slot's usual address if the listing uses it, otherwise its
lowest address rounded down to the ROM size.

If the result is not a known chip, `repairRegion` tries to repair it.
Listings on the web have typos: a mistyped address overwrites a neighbour
and leaves a hole, a line has one byte too many, a few bytes are missing.
The hypotheses:

- every suspicious line (overlapping another line of the same listing, or
  able to fill a hole) is where it says, right after the previous line, or
  right before the next;
- where lines disagree on a byte, either value;
- known differences between published listings and the real chip
  (`VARIANT_SITES`: the 901227-03 KERNAL's checksum byte and two code
  changes that Lee Davison's commented disassembly lacks);
- unused space with a known fill (`KNOWN_FILL`: `$C001-$C0FF` is `$AA` in
  every 1541 ROM; some listings leave it out);
- up to three remaining missing bytes are solved from the CRC.

Solving uses the fact that, for a fixed length, CRC32 is affine over
GF(2): `crc(a) XOR crc(b)` depends only on `a XOR b`. `crcColumns` gives
the CRC change for each bit position; unknown bytes become up to 24 unknown
bits in a linear system, solved by Gaussian elimination (`gf2Basis`,
`gf2Solve`). This also lets thousands of hypotheses be checked without
recomputing the CRC of 8 KB each time.

A repair is accepted only if exactly one image matches a known chip's
CRC32, and the chance that a wrong image matched by luck, summed over all
hypotheses tried, stays below 1 in 4,096. The fewer bytes had to be
solved, the more of the CRC remains as a check, so the solutions with the
fewest unknowns are the ones counted.

### The character ROM from a drawing (`chargen.js`)

- **Text**: 8 symbols per glyph row, two symbols for off and on (e.g. `.`
  and `#`), 4,096 rows.
- **Picture**: a grid of glyphs in code order, at any integer zoom, with or
  without gaps, 8 to 64 glyphs per row; all 512 glyphs, or the 128
  non-reversed glyphs of each set (the reversed halves are generated).
  Several pictures can be combined.

Either result is verified by CRC like any other ROM.

### The same code in Node

`web/tools/rebuild-roms.mjs` runs `autoFind` in Node and writes
`roms/basic.rom`, `kernal.rom`, `chargen.rom` and `1541.rom` for the
native tests (see [testing.md](testing.md)). The modules avoid
browser-only globals where they don't need them (`download` checks for
`location` before looking at the page's protocol).

## Settings: `settings.js`

Kept in `localStorage` under `c64wasm-settings`: video standard, speed,
display (2002 or flat panel), connection, SID model, volumes, drive on/off
and device number, drive display, zoom, tube parameters and the 2002's
knob positions. Values saved by an older version that no longer fit are
ignored.

## Publishing

`.github/workflows/ci.yml` builds the wasm module, copies the test disks
into `web/disks/`, checks every JavaScript file with `node --check`, and
publishes `web/` to GitHub Pages. See [publishing.md](publishing.md).
