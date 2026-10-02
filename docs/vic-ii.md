# The VIC-II

Source: `crates/c64-core/src/vic/`.

| File | Contents |
|---|---|
| `mod.rs` | registers, the per-cycle `tick`, bad lines, sequencer, sprite DMA, BA, borders, drawing |
| `graphics.rs` | the graphics data sequencer: the shift register and the eight display modes |
| `sprite.rs` | one sprite unit: DMA state and its pixel shifter |
| `memory.rs` | `VicMem`, what the chip sees of memory in one cycle |
| `model.rs` | `VicModel`: PAL 6569 and NTSC 6567R8 geometry |
| `palette.rs` | the 16 colours (the "Pepto" measurements) |

The model follows Christian Bauer's description of the chip ("The MOS
6567/6569 video controller (VIC-II) and its application in the Commodore
64"), with VICE as the reference where Bauer is silent or where VICE's
model has since proved more exact.

## Geometry

|  | PAL 6569 | NTSC 6567R8 |
|---|---:|---:|
| cycles per line | 63 | 65 |
| lines per frame | 312 | 263 |
| framebuffer | 504 × 312 | 520 × 263 |
| sprite X of the first pixel of cycle 1 | `$194` | `$19C` |
| sprite X wraps at | 504 | 512 |

Cycles are numbered from 1, as in Bauer's tables; `cycle` is the cycle
about to run. The framebuffer holds the whole raster, blanking included,
8 pixels per cycle. `fb_x` rotates it so the display window starts at
column `$18 + 68` (PAL) or `$18 + 76` (NTSC); the page crops it to what a
monitor shows.

## One cycle: `Vic::tick`

`Board` calls `tick(&VicMem)` once per CPU cycle, before the CPU's access.
In order:

1. **Raster interrupt.** In cycle 1 (cycle 2 on line 0) the line is
   compared with the raster compare register. A write to `$D011`/`$D012`
   that sets the compare value to the current line also triggers it.
2. **DEN latch.** If DEN (`$D011` bit 4) is set at any time during line
   `$30`, bad lines are allowed for this frame.
3. **Bad-line condition**, evaluated in every cycle: DEN latch, raster in
   `$30-$F7`, and `raster & 7 == YSCROLL`. A bad line switches to display
   state. Because it is evaluated every cycle, a `$D011` write in the
   middle of a line has the same effect as on the chip: FLD (postponing
   bad lines), FLI (forcing one every line), line crunch, and DMA delay /
   VSP (a bad line starting late in the line).
4. **Sprite DMA** (below).
5. **The sequencer** (below): VC, VCBASE, RC, VMLI, g- and c-accesses.
6. **Sprite fetches**: p- and s-accesses.
7. **Vertical border** checks in the last cycle of the line.
8. **Drawing** the cycle's 8 pixels.
9. **BA** for this cycle. `tick` returns `!BA_low`, the CPU's RDY.
10. The cycle and raster counters advance; at the end of the frame VCBASE
    and the DEN latch are reset.

### Memory accesses

| Access | Cycles | What |
|---|---|---|
| c (video matrix + colour) | 15-54 of a bad line | screen code at VM + VC, colour nibble at colour RAM + VC, into the 40-entry line buffer |
| g (graphics) | 16-55 | character or bitmap byte; in idle state `$3FFF` (`$39FF` with ECM) |
| p (sprite pointer) | sprite n: 58 + 2n, wrapping into the next line | byte at VM + `$3F8` + n |
| s (sprite data) | the p cycle (byte 0) and the next cycle (bytes 1 and 2) | 3 bytes at pointer × 64 + MC |

The g-access address is:

- text modes: `CB13-11 | screen code × 8 | RC`;
- bitmap modes: `CB13 | VC × 8 | RC`;
- with ECM, address bits 9 and 10 are forced low (`& $39FF`).

`VicMem::read` turns a 14-bit address into a byte: the bank comes from
CIA2 PA0-1 (inverted), and in banks 0 and 2 the character ROM is seen at
`$1000-$1FFF` instead of RAM. Colour RAM is a separate 4-bit bus.

### Sequencer (Bauer's VC/RC logic)

- Cycle 14: `VC = VCBASE`, `VMLI = 0`; on a bad line also `RC = 0`.
- Cycles 16-55: in display state each g-access advances VC and VMLI.
- Cycle 58: if `RC == 7`, `VCBASE = VC`, and the chip goes to idle unless
  this is a bad line. In display state RC increments.

### BA and stolen cycles

BA goes low:

- on a bad line, in cycles 12-54 (three cycles of warning before the
  first c-access in cycle 15);
- for each sprite with DMA on, from three cycles before its p-access to
  the end of its s-accesses.

The CPU sees BA as RDY: its reads wait while BA is low, its writes do not
(see [cpu.md](cpu.md#the-cycle-model)). A c-access in one of the first
three cycles of BA low happens while the CPU still drives the bus, so the
chip reads `$FF` as the screen code and, as the colour, the low nibble of
the RAM byte at the CPU's program counter (its current opcode, unless the
code runs from ROM). This is how a DMA-delayed line gets its
"garbage" first characters. (The board provides that nibble from the
CPU's program counter, `Bus::note_pc`, as VICE does.)

## Sprites

Each of the eight units (`Sprite`) has DMA and display flags, the
Y-expansion flip-flop, MC, MCBASE, the pointer and 24 bits of data.

| Cycle | What happens |
|---|---|
| 55 | for each Y-expanded sprite, the expansion flip-flop toggles |
| 55, 56 | a sprite whose enable bit is set and whose Y matches the line's low 8 bits turns DMA on: `MCBASE = 0`, and if it is Y-expanded the flip-flop is cleared |
| 58 | `MC = MCBASE`; display turns off if DMA is off, and on if the Y coordinate matches |
| p, p+1 | pointer and three data bytes are fetched (MC advances by 3) |
| 16 | if the flip-flop is set, `MCBASE = MC`; DMA stops when MCBASE reaches 63 |

Bauer describes MCBASE as "+2 in cycle 15, +1 in cycle 16". With the three
s-accesses of the line before, that is `MCBASE = MC` in cycle 16, which is
how VICE does it and what makes the sprite crunch come out right.

**Sprite crunch.** Clearing a sprite's Y-expansion bit (`$D017`) in cycle
15 while its flip-flop is clear mixes MC and MCBASE:

```
MC = (0x2A & (MCBASE & MC)) | (0x15 & (MCBASE | MC))
```

then MCBASE takes that value in cycle 16. The sprite's rows then run out
of order and it shows for more than 21 lines (Performers' "face" scene
depends on it). Any `$D017` write that clears a bit while the flip-flop
is clear sets the flip-flop.

**Pixels.** The shifter starts when the beam's X coordinate equals the
sprite's X (9 bits, `$D000+2n` and `$D010`), at single-pixel resolution.
X expansion doubles each pixel; multicolour takes two bits per pixel
(`01` = `$D025`, `10` = the sprite's colour, `11` = `$D026`). The shifter
stops after 24 bits or at the end of the line.

**Priority and collisions.** The lowest-numbered opaque sprite wins.
If its priority bit (`$D01B`) is set, it is drawn behind foreground
graphics. Sprite-sprite collision: two or more sprites are opaque on the
same pixel (also when it is hidden by the border). Sprite-background
collision: a sprite is opaque on a foreground pixel outside the border.
Each sets its bits in `$D01E`/`$D01F` and raises its interrupt flag only
when the register was zero before. Reading the register clears it.

## Graphics modes

The sequencer (`graphics.rs`) is loaded with each g-access byte when the
beam reaches its X position, `$18 + 8 × (cycle − 16) + XSCROLL`, so
XSCROLL shifts the picture by delaying the load. The mode bits (ECM, BMM,
MCM) are taken at load time, so a mode change applies from the next
8-pixel boundary.

| ECM BMM MCM | Mode | Pixel colours |
|---|---|---|
| 0 0 0 | standard text | 1 = colour nibble, 0 = `$D021` |
| 0 0 1 | multicolour text | if colour bit 3 is set: 00 `$D021`, 01 `$D022`, 10 `$D023`, 11 colour & 7; otherwise as standard text with colour & 7 |
| 0 1 0 | standard bitmap | 1 = screen code high nibble, 0 = low nibble |
| 0 1 1 | multicolour bitmap | 00 `$D021`, 01 high nibble, 10 low nibble, 11 colour RAM |
| 1 0 0 | extended background | 1 = colour nibble, 0 = `$D021-$D024` by screen code bits 6-7 |
| 1 0 1, 1 1 0, 1 1 1 | invalid | black, but foreground/background is still decoded for collisions |

"Foreground" for collisions and sprite priority is: set bits in hi-res
modes, and `10`/`11` in the multicolour modes (`01` is background).

## Borders

Two flip-flops, as in Bauer's description:

- **Vertical**: set at the bottom line and cleared at the top line (only if
  DEN is set), checked in the last cycle of the line and at the left
  border column. RSEL: 25 rows = lines 51-250, 24 rows = 55-246.
- **Main**: set at the right column, cleared at the left column if the
  vertical flip-flop is clear. CSEL: 40 columns = X `$18`-`$157`, 38 columns
  = `$1F`-`$14E`.

Because both are checked at exact positions, opening the top/bottom border
(switching RSEL at the right line) and the side borders (switching CSEL
in the right cycle) works. The border colour covers sprites too.

## Registers

`regs[64]` stores what is written, except the raster compare, the
interrupt latch and mask, and the collision registers, which are kept
separately. Reads add the chip's unused bits:
`$D016` reads `| $C0`, `$D018` `| $01`, `$D019` `| $70` (bit 7 = an enabled
interrupt is pending), `$D01A` `| $F0`, colours `| $F0`, and `$D02F-$D03F`
read `$FF`. `$D011` bit 7 and `$D012` read the current raster line.
Writing `$D019` acknowledges the bits written as 1.

The interrupt output (raster, sprite-background, sprite-sprite; light pen
is never raised) is ORed onto the CPU's IRQ line with CIA1's.

## Simplifications

- Register changes that the real chip applies at a specific pixel within
  the cycle are applied at the next 8-pixel boundary (graphics) or at the
  pixel the beam reaches (sprites, borders).
- The light pen and the 64-cycle NTSC 6567R56A are not modelled.
- The palette is fixed RGB. Colour, tint, brightness and contrast are
  applied by the page's monitor simulation (see [web.md](web.md)).
