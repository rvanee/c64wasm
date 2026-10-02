# c64wasm documentation

| Document | What it covers |
|---|---|
| [architecture.md](architecture.md) | The three layers (core, bindings, page); one module per chip; flavours; the `Bus` trait; how the C64 and the 1541 are wired; the two clocks; the serial bus; the frame loop |
| [cpu.md](cpu.md) | The 6502 core and the 6510: the cycle model, RDY, interrupts, decimal mode, all illegal opcodes, JAM, the I/O port |
| [vic-ii.md](vic-ii.md) | The VIC-II cycle by cycle: bad lines, the sequencer, memory accesses, BA, sprites and sprite crunch, graphics modes, borders |
| [sid.md](sid.md) | The SID: oscillators, noise, envelopes, filter, mixer, output stage, 6581 vs 8580 |
| [cia-via.md](cia-via.md) | The 6526 CIA and 6522 VIA, their timers and interrupts, and how their pins are wired in the C64 and the 1541 |
| [1541.md](1541.md) | The 1541: logic board, serial interface and the ATN gate, read/write electronics, mechanism, GCR, D64 and G64 |
| [web.md](web.md) | The page: start-up, power, timing, the CRT renderer, the desk, sound, keyboard, disks, the ROM library and rebuilding ROMs from listings |
| [testing.md](testing.md) | Test strategy: references, what is tested where, ROMs in CI, regression checks against real software |
| [sources.md](sources.md) | Where the facts come from and how disagreements were settled |
| [publishing.md](publishing.md) | Putting the project on GitHub with CI and GitHub Pages |

For building and running, see the [README](../README.md) in the
repository root.
