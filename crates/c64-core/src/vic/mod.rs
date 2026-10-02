// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The VIC-II video chip (MOS 6569 PAL / 6567R8 NTSC), cycle by cycle,
//! after Christian Bauer's "The MOS 6567/6569 video controller (VIC-II)
//! and its application in the Commodore 64". Every phi2 cycle it:
//!
//! - advances the raster position and raises the raster interrupt at the
//!   start of the compare line (cycle 2 for line 0);
//! - evaluates the bad-line condition in every cycle, so mid-line $D011
//!   writes give FLD, FLI, line crunch and DMA delay as on the real chip;
//! - runs the display/idle sequencer (VC, VCBASE, RC, VMLI) with c-accesses
//!   (video matrix and colour RAM, cycles 15-54 of a bad line) and
//!   g-accesses (cycles 16-55);
//! - runs the sprite units: DMA in cycles 55/56, MC/MCBASE, the
//!   Y-expansion flip-flop, p- and s-accesses, pixel-exact X matching;
//! - holds BA low three cycles before its own bus accesses, so the CPU is
//!   stalled exactly as on the real machine, and reads $FF (with the CPU's
//!   data as colour) in c-accesses whose three-cycle warning hasn't passed;
//! - draws 8 pixels with sprite priority, collisions and the two border
//!   flip-flops, so opened borders work.
//!
//! Simplifications: register changes take effect at the next 8-pixel
//! boundary (the real chip has some pixel-exact delays); the light pen and
//! the 6567R56A are not modelled.

mod graphics;
mod memory;
mod model;
mod palette;
mod sprite;

pub use memory::VicMem;
pub use model::VicModel;
pub use palette::PALETTE;

use graphics::{GFetch, Sequencer};
use sprite::Sprite;

/// Interrupt latch bits ($D019/$D01A).
const IRQ_RASTER: u8 = 0x01;
const IRQ_SPRITE_BACKGROUND: u8 = 0x02;
const IRQ_SPRITE_SPRITE: u8 = 0x04;

pub struct Vic {
    model: VicModel,
    /// The register file; some registers are kept elsewhere (raster
    /// compare, interrupt latch/mask, collision latches).
    pub regs: [u8; 64],
    /// Current cycle within the line, 1-based like Bauer's tables.
    cycle: u16,
    raster: u16,
    raster_compare: u16,
    irr: u8,
    imr: u8,
    /// DEN was set during line $30 of this frame (bad lines allowed).
    den_latch: bool,
    display_state: bool,
    vc: u16,
    vcbase: u16,
    rc: u8,
    vmli: usize,
    /// The 40 video-matrix entries (character, colour) of the current row.
    vmbuf: [(u8, u8); 40],
    /// Consecutive cycles BA has been low (c-accesses need 3).
    ba_low_cycles: u32,
    sprites: [Sprite; 8],
    main_border: bool,
    vert_border: bool,
    gfx: Sequencer,
    collision_ss: u8,
    collision_sb: u8,
    framebuffer: Vec<u8>,
}

impl std::fmt::Debug for Vic {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Vic")
            .field("model", &self.model)
            .field("raster", &self.raster)
            .field("cycle", &self.cycle)
            .finish_non_exhaustive()
    }
}

impl Vic {
    pub fn new(model: VicModel) -> Self {
        let mut sprites = [Sprite::default(); 8];
        for s in sprites.iter_mut() {
            s.exp_ff = true;
        }
        Vic {
            model,
            regs: [0; 64],
            cycle: 1,
            raster: 0,
            raster_compare: 0,
            irr: 0,
            imr: 0,
            den_latch: false,
            display_state: false,
            vc: 0,
            vcbase: 0,
            rc: 0,
            vmli: 0,
            vmbuf: [(0, 0); 40],
            ba_low_cycles: 0,
            sprites,
            main_border: true,
            vert_border: true,
            gfx: Sequencer::default(),
            collision_ss: 0,
            collision_sb: 0,
            framebuffer: vec![0; model.width() as usize * model.height() as usize * 4],
        }
    }

    pub fn model(&self) -> VicModel {
        self.model
    }

    pub fn raster_line(&self) -> u16 {
        self.raster
    }

    /// Current cycle within the line (1-based).
    pub fn cycle(&self) -> u16 {
        self.cycle
    }

    /// The picture as RGBA bytes, the whole raster (see `VicModel::width`
    /// and `height`), row by row.
    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// Sprite units for diagnostics: (dma, display, mc, mcbase, exp_ff).
    pub fn sprite_state(&self) -> [(bool, bool, u8, u8, bool); 8] {
        self.sprites.map(|s| (s.dma, s.display, s.mc, s.mcbase, s.exp_ff))
    }

    /// The interrupt output, ORed onto the CPU's IRQ line.
    pub fn irq_line(&self) -> bool {
        self.irr & self.imr & 0x0F != 0
    }

    fn bad_line(&self) -> bool {
        self.den_latch && (0x30..=0xF7).contains(&self.raster) && (self.raster & 7) as u8 == self.regs[0x11] & 7
    }

    fn check_raster_irq_on_write(&mut self, old_compare: u16) {
        if self.raster_compare != old_compare && self.raster == self.raster_compare {
            self.irr |= IRQ_RASTER;
        }
    }

    /// Register read (`reg` 0-63). Reading the collision registers clears
    /// them.
    pub fn read(&mut self, reg: usize) -> u8 {
        match reg {
            0x11 => (self.regs[0x11] & 0x7F) | (((self.raster >> 8) as u8 & 1) << 7),
            0x12 => self.raster as u8,
            0x16 => self.regs[0x16] | 0xC0,
            0x18 => self.regs[0x18] | 0x01,
            0x19 => self.irr | 0x70 | if self.irq_line() { 0x80 } else { 0 },
            0x1A => self.imr | 0xF0,
            0x1E => std::mem::take(&mut self.collision_ss),
            0x1F => std::mem::take(&mut self.collision_sb),
            0x20..=0x2E => self.regs[reg] | 0xF0,
            0x2F..=0x3F => 0xFF,
            _ => self.regs[reg],
        }
    }

    pub fn write(&mut self, reg: usize, val: u8) {
        match reg {
            0x11 => {
                self.regs[0x11] = val;
                let old = self.raster_compare;
                self.raster_compare = (self.raster_compare & 0xFF) | (((val >> 7) as u16) << 8);
                self.check_raster_irq_on_write(old);
                if self.raster == 0x30 && val & 0x10 != 0 {
                    self.den_latch = true;
                }
            }
            0x12 => {
                let old = self.raster_compare;
                self.raster_compare = (self.raster_compare & 0x100) | val as u16;
                self.check_raster_irq_on_write(old);
            }
            0x17 => {
                self.regs[0x17] = val;
                // A write lands after this cycle's VIC work, so `self.cycle`
                // is already the next one: 16 means the write is in cycle 15.
                let in_cycle_15 = self.cycle == 16;
                for (i, s) in self.sprites.iter_mut().enumerate() {
                    if val & (1 << i) == 0 && !s.exp_ff {
                        if in_cycle_15 {
                            // Sprite crunch: clearing Y expansion in cycle 15,
                            // while the flip-flop is reset, mixes MC and MCBASE
                            // (as VICE models it); MCBASE takes the result in
                            // cycle 16. The sprite's rows then run out of
                            // order and it lasts longer than 21 lines.
                            s.mc = (0x2A & (s.mcbase & s.mc)) | (0x15 & (s.mcbase | s.mc));
                        }
                        s.exp_ff = true;
                    }
                }
            }
            0x19 => self.irr &= !(val & 0x0F),
            0x1A => self.imr = val & 0x0F,
            0x1E | 0x1F => {}
            0x2F..=0x3F => {}
            _ => self.regs[reg] = val,
        }
    }

    /// One phi2 cycle. Returns the BA level the CPU sees on RDY (false =
    /// the CPU may not read in this cycle).
    pub fn tick(&mut self, mem: &VicMem) -> bool {
        let c = self.cycle;
        let line = self.raster;
        let cpl = self.model.cycles_per_line();

        // The raster interrupt fires in cycle 1, or cycle 2 for line 0.
        let compare_cycle = if line == 0 { 2 } else { 1 };
        if c == compare_cycle && line == self.raster_compare {
            self.irr |= IRQ_RASTER;
        }
        if line == 0x30 && self.regs[0x11] & 0x10 != 0 {
            self.den_latch = true;
        }
        let bad = self.bad_line();
        if bad {
            self.display_state = true;
        }

        self.sprite_dma(c, line);
        self.sequencer(c, bad, mem);
        self.sprite_fetches(c, mem);

        // vertical border, cycle-63 rules
        let rsel = self.regs[0x11] & 0x08 != 0;
        let (top, bottom) = if rsel { (51u16, 251u16) } else { (55, 247) };
        let den = self.regs[0x11] & 0x10 != 0;
        if c == cpl {
            if line == bottom {
                self.vert_border = true;
            }
            if line == top && den {
                self.vert_border = false;
            }
        }

        self.draw_cycle(top, bottom, den);

        let ba_low = self.ba_low(c, bad);
        if ba_low {
            self.ba_low_cycles += 1;
        } else {
            self.ba_low_cycles = 0;
        }

        self.cycle += 1;
        if self.cycle > cpl {
            self.cycle = 1;
            self.raster += 1;
            if self.raster >= self.model.lines_per_frame() {
                self.raster = 0;
                self.vcbase = 0;
                self.den_latch = false;
            }
        }
        !ba_low
    }

    /// Sprite DMA switching, the expansion flip-flop and MC/MCBASE.
    fn sprite_dma(&mut self, c: u16, line: u16) {
        let ymatch = |regs: &[u8; 64], i: usize| regs[0x01 + 2 * i] == (line & 0xFF) as u8;
        let regs = self.regs;
        if c == 55 {
            for (i, s) in self.sprites.iter_mut().enumerate() {
                if regs[0x17] & (1 << i) != 0 {
                    s.exp_ff = !s.exp_ff;
                }
            }
        }
        if c == 55 || c == 56 {
            for (i, s) in self.sprites.iter_mut().enumerate() {
                if regs[0x15] & (1 << i) != 0 && ymatch(&regs, i) && !s.dma {
                    s.dma = true;
                    s.mcbase = 0;
                    if regs[0x17] & (1 << i) != 0 {
                        s.exp_ff = false;
                    }
                }
            }
        }
        if c == 58 {
            for (i, s) in self.sprites.iter_mut().enumerate() {
                s.mc = s.mcbase;
                if !s.dma {
                    s.display = false;
                } else if ymatch(&regs, i) {
                    s.display = true;
                }
            }
        }
        if c == 16 {
            for s in self.sprites.iter_mut() {
                // Bauer describes MCBASE += 2 in cycle 15 and += 1 in cycle
                // 16; with the three s-accesses of the line before, that is
                // MCBASE = MC, which is how VICE does it (and what makes the
                // sprite crunch above come out right).
                if s.exp_ff {
                    s.mcbase = s.mc;
                }
                // DMA stops here; display is only re-evaluated in cycle 58,
                // so the row fetched in the previous line still shows on
                // this one -- which makes a sprite 21 lines tall.
                if s.mcbase == 63 {
                    s.dma = false;
                }
            }
        }
    }

    /// VC/RC/VMLI, g-accesses and c-accesses.
    fn sequencer(&mut self, c: u16, bad: bool, mem: &VicMem) {
        if c == 14 {
            self.vc = self.vcbase;
            self.vmli = 0;
            if bad {
                self.rc = 0;
            }
        }
        // g-access (phi1, cycles 16-55)
        if (16..=55).contains(&c) {
            let xscroll = (self.regs[0x16] & 7) as u16;
            let load_x = 0x18 + 8 * (c - 16) + xscroll;
            let ecm = self.regs[0x11] & 0x40 != 0;
            let bmm = self.regs[0x11] & 0x20 != 0;
            let f = if self.display_state {
                let (ch, col) = self.vmbuf[self.vmli];
                let mut addr = if bmm {
                    (((self.regs[0x18] & 0x08) as u16) << 10) | (self.vc << 3) | self.rc as u16
                } else {
                    (((self.regs[0x18] & 0x0E) as u16) << 10) | ((ch as u16) << 3) | self.rc as u16
                };
                if ecm {
                    addr &= 0x39FF;
                }
                let g = mem.read(addr);
                self.vc = (self.vc + 1) & 0x3FF;
                self.vmli = (self.vmli + 1) & 63;
                GFetch { load_x, gdata: g, char_code: ch, color: col, valid: true }
            } else {
                let g = mem.read(if ecm { 0x39FF } else { 0x3FFF });
                GFetch { load_x, gdata: g, char_code: 0, color: 0, valid: true }
            };
            self.gfx.queue(f);
        }
        // c-access (phi2, cycles 15-54 of a bad line)
        if bad && (15..=54).contains(&c) && self.vmli < 40 {
            let vm = ((self.regs[0x18] >> 4) as u16) << 10;
            let entry = if self.ba_low_cycles >= 3 {
                (mem.read(vm | self.vc), mem.color(self.vc))
            } else {
                (0xFF, mem.cpu_nibble)
            };
            self.vmbuf[self.vmli] = entry;
        }
        if c == 58 {
            if self.rc == 7 {
                self.vcbase = self.vc;
                if !bad {
                    self.display_state = false;
                }
            }
            if self.display_state {
                self.rc = (self.rc + 1) & 7;
            }
        }
    }

    /// Sprite pointer (p) and data (s) accesses in each sprite's cycles.
    fn sprite_fetches(&mut self, c: u16, mem: &VicMem) {
        let vm = ((self.regs[0x18] >> 4) as u16) << 10;
        let cpl = self.model.cycles_per_line();
        for i in 0..8 {
            let p = self.model.sprite_p_cycle(i);
            let s = &mut self.sprites[i];
            if c == p {
                s.pointer = mem.read(vm | 0x3F8 | i as u16);
                if s.dma {
                    let base = (s.pointer as u16) << 6;
                    let b0 = mem.read(base | s.mc as u16);
                    s.mc = (s.mc + 1) & 63;
                    s.data = (s.data & 0x00FFFF) | ((b0 as u32) << 16);
                }
            } else if c == p % cpl + 1 && s.dma {
                let base = (s.pointer as u16) << 6;
                let b1 = mem.read(base | s.mc as u16);
                s.mc = (s.mc + 1) & 63;
                let b2 = mem.read(base | s.mc as u16);
                s.mc = (s.mc + 1) & 63;
                s.data = (s.data & 0xFF0000) | ((b1 as u32) << 8) | b2 as u32;
            }
        }
    }

    /// BA is low during a bad line's c-access window, and from three
    /// cycles before to the end of each active sprite's accesses.
    fn ba_low(&self, c: u16, bad: bool) -> bool {
        if bad && (12..=54).contains(&c) {
            return true;
        }
        let cpl = self.model.cycles_per_line() as i32;
        (0..8).any(|i| {
            if !self.sprites[i].dma {
                return false;
            }
            let p = self.model.sprite_p_cycle(i) as i32;
            let d = (c as i32 - p + cpl) % cpl; // cycles after p (mod line)
            d <= 1 || d >= cpl - 3
        })
    }

    /// Draw the 8 pixels of this cycle.
    fn draw_cycle(&mut self, top: u16, bottom: u16, den: bool) {
        let c = self.cycle;
        let line = self.raster;
        let regs = self.regs;
        let csel = regs[0x16] & 0x08 != 0;
        let (left, right) = if csel { (0x18u16, 0x158u16) } else { (0x1F, 0x14F) };
        let ecm = regs[0x11] & 0x40 != 0;
        let bmm = regs[0x11] & 0x20 != 0;
        let mcm = regs[0x16] & 0x10 != 0;
        let mode_now = ((ecm as u8) << 2) | ((bmm as u8) << 1) | mcm as u8;
        let fb_row = line as usize * self.model.width() as usize;
        let mut new_ss = 0u8;
        let mut new_sb = 0u8;

        for p in 0..8u16 {
            let raw = (c - 1) * 8 + p;
            let x = self.model.xpos(raw);

            self.gfx.load_if_due(x, mode_now);
            let (gcol, fg) = self.gfx.pixel(&regs);

            // border flip-flops (Bauer's rules 1, 4-6)
            if x == right {
                self.main_border = true;
            }
            if x == left {
                if line == bottom {
                    self.vert_border = true;
                }
                if line == top && den {
                    self.vert_border = false;
                }
                if !self.vert_border {
                    self.main_border = false;
                }
            }

            // sprites: the lowest-numbered opaque sprite wins
            let mut spr_col: Option<u8> = None;
            let mut spr_behind = false;
            let mut opaque = 0u8;
            for (i, s) in self.sprites.iter_mut().enumerate() {
                if !s.display {
                    continue;
                }
                let sx = regs[2 * i] as u16 | (((regs[0x10] >> i) & 1) as u16) << 8;
                let cur = s.pixel(x, sx, regs[0x1D] & (1 << i) != 0, regs[0x1C] & (1 << i) != 0);
                if cur != 0 {
                    opaque |= 1 << i;
                    if spr_col.is_none() {
                        spr_col = Some(match cur {
                            1 => regs[0x25] & 15,
                            3 => regs[0x26] & 15,
                            _ => regs[0x27 + i] & 15,
                        });
                        spr_behind = regs[0x1B] & (1 << i) != 0;
                    }
                }
            }
            if opaque & (opaque.wrapping_sub(1)) != 0 {
                new_ss |= opaque;
            }
            if fg && opaque != 0 && !self.main_border {
                new_sb |= opaque;
            }

            let mut color = gcol;
            if let Some(sc) = spr_col {
                if !spr_behind || !fg {
                    color = sc;
                }
            }
            // the border covers sprites too
            if self.main_border {
                color = regs[0x20] & 15;
            }
            let (r, g, b) = PALETTE[color as usize];
            let idx = (fb_row + self.model.fb_x(raw) as usize) * 4;
            if idx + 3 < self.framebuffer.len() {
                self.framebuffer[idx..idx + 4].copy_from_slice(&[r, g, b, 0xFF]);
            }
        }
        if new_ss != 0 {
            if self.collision_ss == 0 {
                self.irr |= IRQ_SPRITE_SPRITE;
            }
            self.collision_ss |= new_ss;
        }
        if new_sb != 0 {
            if self.collision_sb == 0 {
                self.irr |= IRQ_SPRITE_BACKGROUND;
            }
            self.collision_sb |= new_sb;
        }
        if c == self.model.cycles_per_line() {
            self.gfx.end_of_line();
            for s in self.sprites.iter_mut() {
                s.end_of_line();
            }
        }
    }
}
