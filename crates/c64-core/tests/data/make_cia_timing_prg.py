# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 R.F. van Ee
# Generates cia_timing.prg (used by tests/cia_timing.rs). Run: python3 make_cia_timing_prg.py
# Build a CIA timing test PRG. Results at $0400+.
code = bytearray(); labels = {}; fix = []
ORG = 0x0810
def here(): return ORG + len(code)
def b(*xs): code.extend(xs)
def lda_i(v): b(0xA9, v)
def sta(a): b(0x8D, a & 0xFF, a >> 8)
def lda(a): b(0xAD, a & 0xFF, a >> 8)
def ldx_i(v): b(0xA2, v)
def ldy_i(v): b(0xA0, v)
def jmp(lbl): b(0x4C, 0, 0); fix.append((len(code) - 2, lbl))
def label(n): labels[n] = here()
def lda_lbl_lo(lbl): b(0xA9, 0); fix.append((len(code)-1, lbl, 'lo'))
def lda_lbl_hi(lbl): b(0xA9, 0); fix.append((len(code)-1, lbl, 'hi'))
RES = 0x0400
b(0x78)                 # SEI
lda_i(0x7F); sta(0xDC0D); sta(0xDD0D); lda(0xDC0D); lda(0xDD0D)
lda_i(0x35); sta(0x0001)
lda_lbl_lo('irq'); sta(0xFFFE); lda_lbl_hi('irq'); sta(0xFFFF)
lda_lbl_lo('nmi'); sta(0xFFFA); lda_lbl_hi('nmi'); sta(0xFFFB)
# --- read tests: start timer then read after d cycles
slot = 0
for pad in range(4):
    lda_i(0); sta(0xDC0E)
    lda_i(0x80); sta(0xDC04); lda_i(0); sta(0xDC05)
    lda_i(0x01); sta(0xDC0E)
    for _ in range(pad): b(0xEA)    # NOP (2 cycles)
    lda(0xDC04); sta(RES + slot); slot += 1
# force load while running: write latch, then STA $DC0E #$11, read
lda_i(0x40); sta(0xDC04); lda_i(0x11); sta(0xDC0E); lda(0xDC04); sta(RES + slot); slot += 1
# --- interrupt latency tests (IRQ via CIA1, NMI via CIA2)
for chip, en in ((0xDC00, 'irq'), (0xDD00, 'nmi')):
    for n in range(8, 16):
        label(f'cont_{chip:x}_{n}')
        lda_i(0); sta(chip + 0x0E)
        lda_i(n); sta(chip + 4); lda_i(0); sta(chip + 5)
        lda(chip + 0x0D)
        lda_i(0x81); sta(chip + 0x0D)
        ldy_i(slot); slot += 1
        ldx_i(0)
        b(0x58)                      # CLI
        lda_i(0x09); sta(chip + 0x0E)   # one-shot, start
        for _ in range(60): b(0xE8)  # INX
        b(0x02)                      # JAM if no interrupt
        label(f'after_{chip:x}_{n}')
        b(0x78)                      # SEI
        lda_i(0x7F); sta(chip + 0x0D); lda(chip + 0x0D)
    # next chip
label('done'); b(0x4C, here() & 0xFF, here() >> 8)   # loop forever (here() before emit)
# handler: store X at $0400,Y; ack; drop frame; jump to continuation in table
label('irq'); label('nmi')
b(0x8A); b(0x99, RES & 0xFF, RES >> 8)  # TXA; STA $0400,Y
lda(0xDC0D); lda(0xDD0D)
b(0x68, 0x68, 0x68)                     # PLA x3
# continuation: return to the instruction after the JAM byte: pushed PC points into INX chain;
# simply jump via table indexed by Y
b(0xB9); fix.append((len(code), 'tlo')); b(0, 0)   # LDA tlo,Y
b(0x8D); fix.append((len(code), 'jv')); b(0, 0)    # STA jv
b(0xB9); fix.append((len(code), 'thi')); b(0, 0)
b(0x8D); fix.append((len(code), 'jv1')); b(0, 0)
label('jmpi'); b(0x6C); fix.append((len(code), 'jv')); b(0, 0)  # JMP (jv)
label('jv'); b(0, 0); labels['jv1'] = labels['jv'] + 1
# continuation table, indexed by result slot
conts = {}
s = 5
for chip in (0xDC00, 0xDD00):
    for n in range(8, 16):
        conts[s] = labels[f'after_{chip:x}_{n}']; s += 1
label('tlo'); b(*[conts.get(i, labels['done']) & 0xFF for i in range(32)])
label('thi'); b(*[conts.get(i, labels['done']) >> 8 for i in range(32)])
for f in fix:
    if len(f) == 3:
        off, lbl, part = f; v = labels[lbl]; code[off] = v & 0xFF if part == 'lo' else v >> 8
    else:
        off, lbl = f; v = labels[lbl]; code[off] = v & 0xFF; code[off+1] = v >> 8
stub = bytes([0x0B, 0x08, 0x0A, 0x00, 0x9E]) + b'2064' + bytes([0, 0, 0])
prg = bytes([0x01, 0x08]) + stub + bytes(ORG - 0x0801 - len(stub)) + bytes(code)
open('cia_timing.prg', 'wb').write(prg)
print('len', len(prg), 'done at %04x' % labels['done'])
