# The 6502 and the 6510

Source: `crates/c64-core/src/cpu/`.

| File | Contents |
|---|---|
| `mos6502.rs` | registers, `rd`/`wr`, interrupt polling, `step`, interrupt entry |
| `execute.rs` | the 256-entry opcode dispatch, RMW, branches, stack, BRK, JAM |
| `addressing.rs` | the bus-access sequence of every addressing mode |
| `ops.rs` | what operations compute: ALU, decimal mode, illegal opcodes |
| `mos6510.rs` | the 6510: the core plus the I/O port at $00/$01 |
| `bus.rs` | the `Bus` trait (see [architecture.md](architecture.md#the-bus-trait-how-a-cpu-is-wired)) |
| `tests.rs` | unit tests on a 64 KB RAM bus |

One core, `Mos6502`, serves both machines: the 1541 uses it directly, the
C64 wraps it in `Mos6510`.

## The cycle model

An NMOS 6502 accesses the bus in every cycle, also in cycles where it has
nothing to read or write: implied instructions re-read the next byte,
indexed modes read the "wrong" address before fixing the high byte,
read-modify-write instructions write the old value back before the new
one. The core reproduces every one of these accesses, in order, through
two functions:

- `rd(bus, addr)`: calls `bus.tick()`, checks the SO pin, counts the
  cycle, polls the interrupt inputs, and reads only if RDY was high;
  otherwise the cycle repeats.
- `wr(bus, addr, val)`: the same, but a write never waits for RDY.

So the number of cycles an instruction takes is the number of accesses it
makes, and `step()` returns it. There is no table of cycle counts. Tom
Harte's single-step tests check the exact sequence of addresses, values
and directions for every opcode (see [testing.md](testing.md#cpu)).

`step()` runs one whole instruction. That is enough even with RDY: RDY
only holds reads, and a held read just repeats, which `rd` does by
looping. The other chips are clocked inside `tick()`, so they are always
exactly as far as the CPU.

### Addressing modes

Each mode in `addressing.rs` performs exactly the real chip's accesses:

| Mode | Extra accesses |
|---|---|
| `zp,X` / `zp,Y` | dummy read of the unindexed zero-page address |
| `abs,X` / `abs,Y`, read | dummy read at the un-carried address, only when the page is crossed |
| `abs,X` / `abs,Y`, write or RMW | that dummy read always |
| `(zp,X)` | dummy read of the pointer before adding X; the pointer wraps in zero page |
| `(zp),Y`, read | dummy read only when crossing a page |
| `(zp),Y`, write or RMW | dummy read always |
| `JMP ($xxFF)` | the high byte comes from `$xx00` (the page-wrap bug) |
| read-modify-write | read, write the old value back, write the new value |
| branches | +1 cycle (dummy opcode read) when taken, +1 (read at the un-carried address) when crossing a page |

`_c` variants of the indexed modes also report whether the page was
crossed; the unstable stores (below) need it.

## Interrupts

**Polling.** Every cycle `poll()` records what the CPU sees on its inputs:
whether an NMI edge is latched, and whether IRQ is asserted while the I
flag is clear. `step()` acts on what was recorded in the last cycle of the
previous instruction. This matches the real chip, which decides in its
second-to-last cycle: the chips' outputs change on the opposite clock
phase, so state "after the last cycle's clock" is what the CPU saw. It was
checked against VICE with a CIA timer program
(`tests/cia_timing.rs`). Because the I flag is sampled during that cycle:

- after `CLI`, an IRQ is taken only after the next instruction;
- after `SEI`, an IRQ that was already pending is still taken once.

**Order.** NMI wins over IRQ and ignores the I flag.

**NMI is an edge.** The board detects the edge (CIA2's output rising, or
RESTORE) and latches it; the CPU consumes it with `take_nmi_edge()`. A
CIA2 interrupt acknowledged within one instruction is still serviced, as
on the real machine.

**IRQ is a level.** It stays until the source is acknowledged (reading
CIA1's ICR, writing `$D019`).

**Entry sequence** (7 cycles, `service_interrupt`): two dummy reads at PC
(where the next opcode fetch would have been), push PCH, PCL and P with
B = 0 and bit 5 = 1, set I, read the vector (`$FFFA` NMI, `$FFFE` IRQ).
`BRK` uses the same sequence, with its opcode fetch and padding byte in
place of the two dummy reads and B = 1 in the pushed status. `PLP` and `RTI` always leave B
= 0 and bit 5 = 1 in the register.

**SO pin.** The 1541 wires its byte-ready signal to the 6502's SO input,
which sets V asynchronously. `rd`/`wr` call `take_so_edge()` every cycle,
so V is set in the cycle the edge arrives, even in the middle of an
instruction. A `BVC` tests V right after its opcode fetch: it sees an edge
that arrived by then, otherwise the next `BVC` does.
The C64's board never reports an SO edge.

## Decimal mode

The NMOS behaviour, after Bruce Clark's "Decimal Mode in NMOS 6500 series"
(`ops.rs`):

- **ADC** with D = 1: Z comes from the plain binary sum; N and V from the
  intermediate result after the low-nibble correction but before the
  high-nibble one; C and A from the fully corrected result.
- **SBC** with D = 1: all flags are those of a binary subtraction; only
  A is decimal-corrected.

`RRA` and `ISC` go through the same paths; `ARR` has its own decimal
correction (see below).

## Illegal opcodes

All 256 opcodes are implemented. The 105 that are not in the official
instruction set behave as on the NMOS 6510, including the unstable ones.
They pass the same single-step tests as the official opcodes: all
2,560,000 cases (10,000 per opcode) of SingleStepTests/65x02 `6502/v1`,
registers, memory and every bus cycle, as of October 2026.
Names follow the common naming (with aliases). Flags: `C` is set from the
shift or rotate; N and Z come from the final result unless noted.

### Combined read-modify-write instructions

Each does a normal RMW on memory (read, write back, write new) and then
combines the new value with A. Addressing modes: `zp`, `zp,X`, `abs`,
`abs,X`, `abs,Y`, `(zp,X)`, `(zp),Y`, with the RMW access patterns (the
indexed ones always take the extra cycle).

| Name | Opcodes | Memory becomes | Then |
|---|---|---|---|
| SLO (ASO) | 07 17 0F 1F 1B 03 13 | M << 1 | A \|= M |
| RLA | 27 37 2F 3F 3B 23 33 | M rotated left through C | A &= M |
| SRE (LSE) | 47 57 4F 5F 5B 43 53 | M >> 1 | A ^= M |
| RRA | 67 77 6F 7F 7B 63 73 | M rotated right through C | A = A + M + C (ADC, decimal-aware) |
| DCP (DCM) | C7 D7 CF DF DB C3 D3 | M − 1 | CMP A with M |
| ISC (ISB, INS) | E7 F7 EF FF FB E3 F3 | M + 1 | A = A − M − !C (SBC, decimal-aware) |

### Loads and stores

| Name | Opcodes (modes) | Effect |
|---|---|---|
| LAX | A7 zp, B7 zp,Y, AF abs, BF abs,Y, A3 (zp,X), B3 (zp),Y | A = X = M |
| SAX (AXS) | 87 zp, 97 zp,Y, 8F abs, 83 (zp,X) | M = A & X, flags unchanged |
| LAS (LAR) | BB abs,Y | A = X = S = M & S |

### Immediate operations

| Name | Opcode | Effect |
|---|---|---|
| ANC | 0B, 2B | A &= imm; C = bit 7 of A |
| ALR (ASR) | 4B | A &= imm, then LSR A |
| ARR | 6B | A &= imm, then ROR A; C = bit 6, V = bit 6 XOR bit 5 of the result. In decimal mode the flags come from that binary result and A gets a per-nibble BCD correction (which can also set C) |
| SBX (AXS) | CB | X = (A & X) − imm, without borrow; C = no borrow; flags like CMP |
| USBC | EB | the same as SBC #imm (E9) |
| ANE (XAA) | 8B | A = (A \| magic) & X & imm |
| LXA (LAX #) | AB | A = X = (A \| magic) & imm |

ANE and LXA are unstable on real chips: the result depends on the chip and
even its temperature. `magic` is a constant, `UNSTABLE_MAGIC = $EE` in
`ops.rs`, the value most often cited; it is the one thing to change if a
reference needs another.

### The unstable stores (high-byte AND)

| Name | Opcodes (modes) | Stores |
|---|---|---|
| SHA (AHX, AXA) | 9F abs,Y, 93 (zp),Y | A & X & H |
| SHX (SXA, XAS) | 9E abs,Y | X & H |
| SHY (SYA, SAY) | 9C abs,X | Y & H |
| TAS (XAS, SHS) | 9B abs,Y | S = A & X, then stores S & H |

H is the high byte of the base address (before indexing) plus one. When
adding the index crosses a page, the real chip also corrupts the address:
the high byte driven during the write is the stored value itself, not
the correct high byte. Both effects are modelled and verified against the
single-step tests (`93.json`, `9b.json`, `9c.json`, `9e.json`).

### NOPs

| Opcodes | Mode | Cycles |
|---|---|---|
| 1A 3A 5A 7A DA FA | implied | 2 |
| 80 82 89 C2 E2 | immediate | 2 |
| 04 44 64 | zp (reads it) | 3 |
| 14 34 54 74 D4 F4 | zp,X | 4 |
| 0C | abs | 4 |
| 1C 3C 5C 7C DC FC | abs,X (+1 when crossing a page) | 4-5 |

They do the reads of their addressing mode, which matters when the
address is an I/O register with read side effects (for example, a CIA's
ICR).

### JAM (KIL, HLT)

Opcodes 02 12 22 32 42 52 62 72 92 B2 D2 F2 stop the CPU. `jam()` sets
`jammed`, reproduces the access pattern the single-step tests record (a
read at PC, then `$FFFF`, `$FFFE`, `$FFFE` and six reads of `$FFFF`, 11
cycles in all). The core then takes no more interrupts, and its callers
stop calling `step()`: `C64::run_until` returns (the page shows a message
and needs a power cycle), and `Drive1541::run_from_host` stops running the
drive until it is switched off, as on the real hardware.

## The 6510

`Mos6510` is a `Mos6502` plus a `ProcessorPort`: data direction at `$00`,
data at `$01`. The port sits in front of the bus (an internal `PortBus`
adapter): accesses to `$00`/`$01` are answered by the port, and every
write to them reports the new pin levels through
`Bus::processor_port_changed`. Pin levels are `(data & ddr) | !ddr`:
inputs read 1.

On the C64, bits 0-2 (LORAM, HIRAM, CHAREN) select the memory
configuration; bits 3-5 (datasette) are not connected to anything here.
Registers and flags are reached through `Deref<Target = Mos6502>`, so
`c64.cpu().pc` works for both CPUs.

Not modelled: the port's input-bit "fade" (bits that were outputs keep
their value for a while after becoming inputs), and the CPU's behaviour
during the reset sequence (the machines start at the reset vector with
S = `$FD` and I set).

## Using the core

```rust
use c64_core::cpu::{Bus, Mos6502};

struct Ram([u8; 65536]);
impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 { self.0[a as usize] }
    fn write(&mut self, a: u16, v: u8) { self.0[a as usize] = v }
}

let mut ram = Ram([0; 65536]);
let mut cpu = Mos6502::new();
cpu.pc = 0x0400;
let cycles = cpu.step(&mut ram);
```

A bus that needs nothing else uses the trait's defaults: `tick()` never
stalls, no interrupts, no SO.
