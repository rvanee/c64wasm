# CIAs, VIAs and their ports

| Chip | Source | Where |
|---|---|---|
| 6526 CIA | `crates/c64-core/src/cia/` | C64: CIA1 at `$DC00`, CIA2 at `$DD00` |
| 6522 VIA | `crates/c64-core/src/via/mod.rs` | 1541: VIA1 at `$1800`, VIA2 at `$1C00` |
| port | `crates/c64-core/src/port.rs` | the 8-bit port inside both |

Each chip type knows only its own registers and pins. What the pins are
connected to is set by the board that owns it, through `set_input`,
`set_pa_input`, `set_ca1` and so on, and by reading `pins()` (see
[architecture.md](architecture.md)).

## The port

`Port` is the circuit the CIA and the VIA share: an output latch (`data`),
a data-direction register (`ddr`, 1 = output) and the levels the outside
world puts on the pins (`input`).

- `pins()` = `(data & ddr) | (input & !ddr)`: output bits show the latch,
  input bits the outside level.
- `set_input(levels)`: the board sets all eight outside levels.
- `driven_low()`: output bits currently at 0, which is what a keyboard
  matrix scans with.

An unconnected CIA input floats high (`input = $FF`). The VIA starts with
inputs at 0 until the board drives them.

## 6526 CIA

### Registers

| Reg | Read | Write |
|---|---|---|
| 0, 1 | port A / B pins | port A / B latch |
| 2, 3 | DDR A / B | DDR A / B |
| 4-7 | timer A / B counter, low and high | timer A / B latch, low and high |
| 8-C | time of day, serial data (stored only) | stored |
| D | ICR: flags, bit 7 = an enabled source fired; **reading clears all flags** | mask: bit 7 = 1 sets the named bits, 0 clears them |
| E, F | CRA, CRB | CRA, CRB (bit 4 = force load, a strobe) |

The 16 registers repeat through the chip's 256-byte window.

### Timers

`cia/timer.rs` models the 6526's pipeline delays, measured against VICE
x64sc 3.7.1 with a test program (`tests/cia_timing.rs`):

- **Start**: after the write that starts a stopped timer, the first
  decrement happens three cycles later.
- **Stop**: after the write that stops it, the timer still counts for
  three cycles.
- **Force load**: the latch reaches the counter two cycles after the
  write, and the cycle after the load doesn't count.
- **Underflow**: the counter goes from 0 to the latch value (so the period
  is latch + 1) and the interrupt flag is set. In one-shot mode (CR bit 3)
  the timer stops.
- Writing the high latch byte of a **stopped** timer also loads the
  counter.
- Timer B with CRB bits 5-6 = `10` counts timer A underflows. Modes `01`
  (CNT pulses) and `11` (timer A underflows while CNT is high) count
  cycles instead.

Getting the start delay wrong by two cycles made the "Next Level" demo's
NMI-driven raster effects land early; the VICE-measured values fixed it.

### Interrupts

The interrupt output is `pending & mask != 0`. CIA1's goes to the CPU's
IRQ line (ORed with the VIC-II's), CIA2's to NMI, where the board turns
its rising edge into a latched NMI (see [cpu.md](cpu.md#interrupts)).

### Not modelled

- The time-of-day clock and the serial shift register: their registers
  store what is written, but don't run and don't interrupt.
- The CNT input (timer modes that use it count cycles instead).
- Timer outputs on PB6/PB7.
- Reading port B returns the latch on output bits; on a real CIA a key
  connecting two output lines can pull an output bit low.

### C64 wiring

| CIA1 | |
|---|---|
| Port A | keyboard rows (output in the KERNAL's scan) |
| Port B | keyboard columns (input) |
| Interrupt | IRQ |

| CIA2 | |
|---|---|
| PA0-1 | VIC-II bank, inverted: bank = 3 − (PA & 3) |
| PA2 | user port (not connected) |
| PA3, 4, 5 | ATN, CLOCK, DATA out, through 7406 inverters (1 = pull the line low) |
| PA6, 7 | CLOCK, DATA in (0 = line low) |
| Port B | user port (not connected) |
| Interrupt | NMI |

The keyboard is applied each time the CPU reads CIA1: the board computes
the inputs of both ports from the matrix and what each port drives low,
so scanning works in both directions. Joysticks are not connected.

## 6522 VIA

Register behaviour follows the WDC W65C22S datasheet, which documents the
NMOS 6522's register-level behaviour for everything used here.

### Registers

| Reg | Name | Notes |
|---|---|---|
| 0 | ORB/IRB | read: port B (or the latched value if ACR bit 1); clears CB1, and CB2 in a non-independent input mode |
| 1 | ORA/IRA | the same for port A, CA1 and CA2 (ACR bit 0 latches) |
| 2, 3 | DDRB, DDRA | |
| 4 | T1C-L | read: counter low, clears the T1 flag. Write: latch low |
| 5 | T1C-H | write: latch high, then counter = latch, clear the T1 flag, arm one-shot, PB7 low (if ACR bit 7) |
| 6, 7 | T1L-L, T1L-H | latch only; a T1L-H write clears the T1 flag |
| 8 | T2C-L | read: counter low, clears the T2 flag. Write: latch low |
| 9 | T2C-H | write: counter = latch, clear the flag, arm one-shot |
| A | SR | stored only |
| B | ACR | T1 mode (bits 6-7), T2 mode (bit 5), shift register (bits 2-4), latching (bits 0-1) |
| C | PCR | CA1/CB1 edge (bits 0, 4), CA2/CB2 modes (bits 1-3, 5-7) |
| D | IFR | bit 7 = any enabled flag; writing 1s clears flags |
| E | IER | bit 7 of a write selects set or clear; reads with bit 7 = 1 |
| F | ORA/IRA | without handshake: no side effects |

The board mirrors the 16 registers (the 1541 repeats them every 16 bytes
through `$1BFF` and `$1FFF`).

### Timers

- **T1** counts down every cycle. When it passes 0 to `$FFFF` it sets its
  flag, once per write of T1C-H in one-shot mode, every time in free-run
  mode (ACR bit 6). In free-run mode the counter shows `$FFFF` for one
  cycle and then reloads, so the period is latch + 2, as the datasheet
  says. With ACR bit 7, PB7 toggles (free-run) or goes high (one-shot).
- **T2** counts cycles, or PB6 pulses with ACR bit 5 (`pulse_t2`, not
  wired on the 1541). One-shot only.

The 1541's DOS runs its job loop from a VIA2 T1 free-run interrupt with
latch `$3A00`, re-arming it from the latch in the handler, which gives
one tick every 14,992 drive cycles, about 15 ms. Its motor spin-up wait is
60 such ticks.

### Control lines

`set_ca1`/`set_cb1` take the line's level; the active edge (PCR bit 0/4)
sets the flag and, with latching enabled, latches the port. `set_ca2`/
`set_cb2` do the same in the four input modes and are ignored in the
output modes. The output modes (handshake, pulse, manual low, manual high)
are decoded by PCR, and `ca2_output()`/`cb2_output()` give the level.

### Simplifications

- Reading an output bit returns the output register on both ports (on
  the real port A, the pin level). The 1541 has separate input and output
  bits for every serial line, so it never depends on this.
- Writes to T1 take effect from the next cycle, without the real chip's
  extra load cycle in one-shot mode.
- The shift register only stores what is written.
- The PB7 timer output is computed (`pb7()`) but not put on the port
  pins (the 1541 doesn't use it).

### 1541 wiring

| VIA1 (`$1800`) | |
|---|---|
| PB0 | DATA in (1 = line low) |
| PB1 | DATA out (1 = pull low) |
| PB2 | CLOCK in |
| PB3 | CLOCK out |
| PB4 | ATNA, ATN acknowledge (output) |
| PB5-6 | device number pads (input) |
| PB7 | ATN in |
| CA1 | ATN, interrupt on the rising edge (the DOS sets PCR = `$01`) |
| Port A | not used by the 1541 |

| VIA2 (`$1C00`) | |
|---|---|
| PB0-1 | stepper motor phase |
| PB2 | spindle motor |
| PB3 | activity LED |
| PB4 | write protect sensor (input; 1 = writable) |
| PB5-6 | density: bit rate of the read/write clock |
| PB7 | SYNC (input; 0 = a sync mark is under the head) |
| Port A | data to and from the read/write head |
| CA1 | byte ready (falling edge) |
| CA2 | output, unused (the DOS sets it high) |
| CB2 | read (high) / write (low) |

How these signals are produced is in [1541.md](1541.md).
