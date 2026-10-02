// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The MOS 6522 VIA (Versatile Interface Adapter), independent of what its
//! pins are wired to. The 1541 has two: VIA1 for the serial bus, VIA2 for
//! the disk mechanics (see `drive1541::electronics`).
//!
//! Register behaviour follows the W65C22S datasheet, which documents the
//! same register-level behaviour as the NMOS 6522 for everything here.
//!
//! Simplifications:
//! - Reading an output bit returns the output register on both ports (the
//!   real Port A returns the pin level). The 1541 has separate input and
//!   output bits for every serial line, so it never depends on this.
//! - Timer 1 free-runs with the real N+2 period, but writes take effect
//!   from the next cycle without the chip's internal load latency.
//! - The shift register only stores what is written.

use crate::port::Port;

/// Interrupt flag / enable register bit positions (identical layout for
/// IFR and IER), per the datasheet's Table 2-11.
pub const IFR_CA2: u8 = 0x01;
pub const IFR_CA1: u8 = 0x02;
pub const IFR_SR: u8 = 0x04;
pub const IFR_CB2: u8 = 0x08;
pub const IFR_CB1: u8 = 0x10;
pub const IFR_T2: u8 = 0x20;
pub const IFR_T1: u8 = 0x40;
pub const IFR_IRQ: u8 = 0x80;

/// The 3-bit CA2/CB2 control field values from PCR Table 2-5. Named so
/// callers (and this module's own PCR decode) don't have to remember the
/// bit patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum C2Mode {
    InputNegEdge,
    IndependentNegEdge,
    InputPosEdge,
    IndependentPosEdge,
    HandshakeOutput,
    PulseOutput,
    ManualLow,
    ManualHigh,
}

impl C2Mode {
    fn decode(bits: u8) -> C2Mode {
        match bits & 0b111 {
            0b000 => C2Mode::InputNegEdge,
            0b001 => C2Mode::IndependentNegEdge,
            0b010 => C2Mode::InputPosEdge,
            0b011 => C2Mode::IndependentPosEdge,
            0b100 => C2Mode::HandshakeOutput,
            0b101 => C2Mode::PulseOutput,
            0b110 => C2Mode::ManualLow,
            0b111 => C2Mode::ManualHigh,
            _ => unreachable!(),
        }
    }
    fn is_output(self) -> bool {
        matches!(self, C2Mode::HandshakeOutput | C2Mode::PulseOutput | C2Mode::ManualLow | C2Mode::ManualHigh)
    }
    fn is_independent(self) -> bool {
        matches!(self, C2Mode::IndependentNegEdge | C2Mode::IndependentPosEdge)
    }
    /// Whether a *rising* edge (vs falling) is the active one, for the two
    /// input modes.
    fn active_edge_is_rising(self) -> bool {
        matches!(self, C2Mode::InputPosEdge | C2Mode::IndependentPosEdge)
    }
}

#[derive(Debug, Clone)]
pub struct Via6522 {
    port_a: Port,
    port_b: Port,
    /// Input latches for Port A / Port B, captured on an active CA1/CB1
    /// edge when the corresponding ACR latch-enable bit is set. Read
    /// instead of the live external input bits when latching is enabled.
    ila: u8,
    ilb: u8,

    t1c: u16,
    t1l: u16,
    /// One-shot mode (ACR bit6=0) only fires its single interrupt/PB7
    /// pulse once per explicit reload; this tracks whether that's still
    /// armed.
    t1_one_shot_armed: bool,
    /// Free-run mode: the counter shows `$FFFF` for one cycle after an
    /// underflow before the latch is reloaded, giving the real 6522's
    /// N+2-cycle free-run period (not N+1).
    t1_reload_pending: bool,
    pb7: bool,

    t2c: u16,
    t2l_lo: u8,
    t2_one_shot_armed: bool,

    pub sr: u8,

    pub acr: u8,
    pub pcr: u8,
    ifr: u8,
    ier: u8,

    // Edge-detect state for the four control lines.
    ca1_in: bool,
    ca2_in: bool,
    cb1_in: bool,
    cb2_in: bool,
}

impl Default for Via6522 {
    fn default() -> Self {
        Self::new()
    }
}

impl Via6522 {
    pub fn new() -> Self {
        let mut port_a = Port::default();
        let mut port_b = Port::default();
        // Unconnected inputs read 0 until the board drives them.
        port_a.set_input(0);
        port_b.set_input(0);
        Via6522 {
            port_a,
            port_b,
            ila: 0,
            ilb: 0,
            t1c: 0,
            t1l: 0,
            t1_one_shot_armed: false,
            t1_reload_pending: false,
            pb7: false,
            t2c: 0,
            t2l_lo: 0,
            t2_one_shot_armed: false,
            sr: 0,
            acr: 0,
            pcr: 0,
            ifr: 0,
            ier: 0,
            ca1_in: false,
            ca2_in: false,
            cb1_in: false,
            cb2_in: false,
        }
    }

    // ---- external wiring: called by whatever owns this VIA -------------

    /// Set the live external Port A input levels (all 8 bits; only the
    /// ones configured as inputs in DDRA are actually used).
    pub fn set_pa_input(&mut self, bits: u8) {
        self.port_a.set_input(bits);
    }

    /// Set the live external Port B input levels (all 8 bits; only the
    /// ones configured as inputs in DDRB are actually used).
    pub fn set_pb_input(&mut self, bits: u8) {
        self.port_b.set_input(bits);
    }

    /// Drive CA1 to a new external level; an active-edge transition (per
    /// PCR bit 0) sets the CA1 interrupt flag and, if Port A latching is
    /// enabled (ACR bit 0), latches the current Port A input.
    pub fn set_ca1(&mut self, level: bool) {
        let rising = level && !self.ca1_in;
        let falling = !level && self.ca1_in;
        self.ca1_in = level;
        let active = if self.pcr & 0x01 != 0 { rising } else { falling };
        if active {
            self.ifr |= IFR_CA1;
            self.update_irq_summary();
            if self.acr & 0x01 != 0 {
                self.ila = self.port_a_input_bits();
            }
        }
    }

    /// Drive CA2 to a new external level. Only meaningful while CA2 is
    /// configured as an input (one of the four `0xx` PCR modes) -- calling
    /// this while CA2 is in an output mode has no effect, matching real
    /// hardware (the pin is being driven the other way).
    pub fn set_ca2(&mut self, level: bool) {
        let mode = C2Mode::decode(self.pcr >> 1);
        let rising = level && !self.ca2_in;
        let falling = !level && self.ca2_in;
        self.ca2_in = level;
        if mode.is_output() {
            return;
        }
        let active = if mode.active_edge_is_rising() { rising } else { falling };
        if active {
            self.ifr |= IFR_CA2;
            self.update_irq_summary();
        }
    }

    /// As `set_ca1`, but for CB1 (Port B's edge input, also the shift
    /// register's external clock in the `ext` SR modes).
    pub fn set_cb1(&mut self, level: bool) {
        let rising = level && !self.cb1_in;
        let falling = !level && self.cb1_in;
        self.cb1_in = level;
        let active = if self.pcr & 0x10 != 0 { rising } else { falling };
        if active {
            self.ifr |= IFR_CB1;
            self.update_irq_summary();
            if self.acr & 0x02 != 0 {
                self.ilb = self.port_b_input_bits();
            }
        }
    }

    /// As `set_ca2`, but for CB2.
    pub fn set_cb2(&mut self, level: bool) {
        let mode = C2Mode::decode(self.pcr >> 5);
        let rising = level && !self.cb2_in;
        let falling = !level && self.cb2_in;
        self.cb2_in = level;
        if mode.is_output() {
            return;
        }
        let active = if mode.active_edge_is_rising() { rising } else { falling };
        if active {
            self.ifr |= IFR_CB2;
            self.update_irq_summary();
        }
    }

    // ---- reading the actual pin state (for whatever owns this VIA to
    // wire up to its bus / IEC lines / disk head) ------------------------

    /// The Port A pin levels this VIA is currently driving/reading: for
    /// each bit, the OR register value if DDRA says output, else the live
    /// external input.
    pub fn port_a_pins(&self) -> u8 {
        self.port_a.pins()
    }

    /// As `port_a_pins`, for Port B.
    pub fn port_b_pins(&self) -> u8 {
        self.port_b.pins()
    }

    /// Port B's data direction register, for diagnostics.
    pub fn ddrb(&self) -> u8 {
        self.port_b.ddr
    }

    /// Current CA2 output level (only meaningful in an output PCR mode).
    pub fn ca2_output(&self) -> bool {
        match C2Mode::decode(self.pcr >> 1) {
            C2Mode::ManualLow => false,
            C2Mode::ManualHigh => true,
            C2Mode::HandshakeOutput | C2Mode::PulseOutput => self.ca2_in,
            _ => true,
        }
    }

    /// Current CB2 output level (only meaningful in an output PCR mode).
    pub fn cb2_output(&self) -> bool {
        match C2Mode::decode(self.pcr >> 5) {
            C2Mode::ManualLow => false,
            C2Mode::ManualHigh => true,
            C2Mode::HandshakeOutput | C2Mode::PulseOutput => self.cb2_in,
            _ => true,
        }
    }

    fn port_a_input_bits(&self) -> u8 {
        self.port_a.pins()
    }
    fn port_b_input_bits(&self) -> u8 {
        self.port_b.pins()
    }

    // ---- CPU-facing register access -------------------------------------

    /// Recompute IFR bit 7 (the "any enabled interrupt pending" summary
    /// bit) from the other bits and IER. Called after any change to IFR or
    /// IER.
    fn update_irq_summary(&mut self) {
        if self.ifr & self.ier & 0x7F != 0 {
            self.ifr |= IFR_IRQ;
        } else {
            self.ifr &= !IFR_IRQ;
        }
    }

    /// Whether this VIA is currently asserting the shared IRQ line.
    pub fn irq_pending(&self) -> bool {
        self.ifr & IFR_IRQ != 0
    }

    /// Read register `reg` (0-15, as addressed by the 4 low address lines
    /// wired to a real 6522 -- the board is responsible for
    /// mirroring this across whatever wider address range the chip is
    /// mapped into).
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg & 0x0F {
            0x0 => {
                // ORB/IRB: reading clears CB1's flag (and CB2's, if CB2 is
                // in a non-independent input mode).
                let v = if self.acr & 0x02 != 0 { self.ilb } else { self.port_b_input_bits() };
                self.clear_c1_c2_on_port_access(false);
                v
            }
            0x1 => {
                let v = if self.acr & 0x01 != 0 { self.ila } else { self.port_a_input_bits() };
                self.clear_c1_c2_on_port_access(true);
                v
            }
            0x2 => self.port_b.ddr,
            0x3 => self.port_a.ddr,
            0x4 => {
                // T1C-L: read low-order counter, clear T1 IFR.
                let v = (self.t1c & 0xFF) as u8;
                self.ifr &= !IFR_T1;
                self.update_irq_summary();
                v
            }
            0x5 => (self.t1c >> 8) as u8,
            0x6 => (self.t1l & 0xFF) as u8,
            0x7 => (self.t1l >> 8) as u8,
            0x8 => {
                let v = (self.t2c & 0xFF) as u8;
                self.ifr &= !IFR_T2;
                self.update_irq_summary();
                v
            }
            0x9 => (self.t2c >> 8) as u8,
            0xA => self.sr,
            0xB => self.acr,
            0xC => self.pcr,
            0xD => self.ifr,
            0xE => self.ier | 0x80, // bit 7 always reads as 1
            0xF => {
                // ORA without handshake: same data as $1, no CA1/CA2 side
                // effects.
                if self.acr & 0x01 != 0 {
                    self.ila
                } else {
                    self.port_a_input_bits()
                }
            }
            _ => unreachable!(),
        }
    }

    /// Write register `reg`.
    pub fn write(&mut self, reg: u8, val: u8) {
        match reg & 0x0F {
            0x0 => {
                self.port_b.data = val;
                self.clear_c1_c2_on_port_access(false);
            }
            0x1 => {
                self.port_a.data = val;
                self.clear_c1_c2_on_port_access(true);
            }
            0x2 => self.port_b.ddr = val,
            0x3 => self.port_a.ddr = val,
            0x4 => {
                // T1L-L (not T1C-L!) -- a write to the "counter" address
                // for the low byte only ever touches the latch.
                self.t1l = (self.t1l & 0xFF00) | val as u16;
            }
            0x5 => {
                self.t1l = (self.t1l & 0x00FF) | ((val as u16) << 8);
                self.t1c = self.t1l;
                self.t1_reload_pending = false;
                self.ifr &= !IFR_T1;
                self.update_irq_summary();
                self.t1_one_shot_armed = true;
                if self.acr & 0x80 != 0 {
                    self.pb7 = false; // one-shot/free-run pulse starts low
                }
            }
            0x6 => self.t1l = (self.t1l & 0xFF00) | val as u16,
            0x7 => {
                self.t1l = (self.t1l & 0x00FF) | ((val as u16) << 8);
                self.ifr &= !IFR_T1;
                self.update_irq_summary();
            }
            0x8 => self.t2l_lo = val,
            0x9 => {
                self.t2c = ((val as u16) << 8) | self.t2l_lo as u16;
                self.ifr &= !IFR_T2;
                self.update_irq_summary();
                self.t2_one_shot_armed = true;
            }
            0xA => self.sr = val,
            0xB => self.acr = val,
            0xC => self.pcr = val,
            0xD => {
                // IFR: writing a 1 to a bit clears that flag (bit 7 can't
                // be written directly -- it's a pure summary bit).
                self.ifr &= !(val & 0x7F);
                self.update_irq_summary();
            }
            0xE => {
                // IER: bit 7 selects set (1) or clear (0) for the bits
                // named in the rest of the byte.
                if val & 0x80 != 0 {
                    self.ier |= val & 0x7F;
                } else {
                    self.ier &= !(val & 0x7F);
                }
                self.update_irq_summary();
            }
            0xF => self.port_a.data = val, // ORA without handshake: no side effects
            _ => unreachable!(),
        }
    }

    fn clear_c1_c2_on_port_access(&mut self, port_a: bool) {
        if port_a {
            self.ifr &= !IFR_CA1;
            let mode = C2Mode::decode(self.pcr >> 1);
            if !mode.is_output() && !mode.is_independent() {
                self.ifr &= !IFR_CA2;
            }
        } else {
            self.ifr &= !IFR_CB1;
            let mode = C2Mode::decode(self.pcr >> 5);
            if !mode.is_output() && !mode.is_independent() {
                self.ifr &= !IFR_CB2;
            }
        }
        self.update_irq_summary();
    }

    // ---- timers: advance by exactly one PHI2 cycle ----------------------

    /// Advance both timers by one clock cycle. Call exactly once per PHI2
    /// cycle this VIA is powered.
    pub fn tick(&mut self) {
        if self.t1_reload_pending {
            self.t1_reload_pending = false;
            self.t1c = self.t1l;
        } else {
            self.t1c = self.t1c.wrapping_sub(1);
        }
        if self.t1c == 0xFFFF {
            let free_run = self.acr & 0x40 != 0;
            let pb7_enabled = self.acr & 0x80 != 0;
            if free_run || self.t1_one_shot_armed {
                self.ifr |= IFR_T1;
                self.update_irq_summary();
                if pb7_enabled {
                    if free_run {
                        self.pb7 = !self.pb7;
                    } else {
                        self.pb7 = true;
                    }
                }
            }
            if free_run {
                self.t1_reload_pending = true;
            } else {
                self.t1_one_shot_armed = false;
            }
        }

        // T2 pulse-counting mode (ACR bit5=1) counts PB6 edges instead of
        // PHI2 cycles -- advanced via `pulse_t2` below, not here.
        if self.acr & 0x20 == 0 {
            self.t2c = self.t2c.wrapping_sub(1);
            if self.t2c == 0xFFFF && self.t2_one_shot_armed {
                self.ifr |= IFR_T2;
                self.update_irq_summary();
                self.t2_one_shot_armed = false;
            }
        }
    }

    /// Register one negative-edge pulse on PB6, for T2's pulse-counting
    /// mode (ACR bit 5 = 1). No-op in timed-interrupt mode.
    pub fn pulse_t2(&mut self) {
        if self.acr & 0x20 != 0 {
            self.t2c = self.t2c.wrapping_sub(1);
            if self.t2c == 0xFFFF && self.t2_one_shot_armed {
                self.ifr |= IFR_T2;
                self.update_irq_summary();
                self.t2_one_shot_armed = false;
            }
        }
    }

    /// Current PB7 output level (only meaningful when ACR bit 7 enables
    /// it, and only actually visible on the real pin if DDRB bit 7 is also
    /// set output, which real firmware always does before using this
    /// feature).
    pub fn pb7(&self) -> bool {
        self.pb7
    }

    /// Timer 1's counter, for diagnostics.
    pub fn t1_counter(&self) -> u16 {
        self.t1c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddr_selects_between_driven_and_external_bits() {
        let mut via = Via6522::new();
        via.write(0x2, 0b0000_1111); // DDRB: low nibble output, high nibble input
        via.write(0x0, 0b0000_1010); // ORB
        via.set_pb_input(0b1010_0000);
        // low nibble comes from ORB, high nibble from the external input
        assert_eq!(via.port_b_pins(), 0b1010_1010);
    }

    #[test]
    fn t1_one_shot_fires_once_per_load() {
        let mut via = Via6522::new();
        via.write(0xB, 0x00); // ACR: one-shot, PB7 disabled, T2 timed
        via.write(0x6, 3); // T1L-L = 3
        via.write(0x5, 0); // T1L-H = 0, loads counter=3, arms one-shot
        assert!(!via.irq_pending());
        for _ in 0..3 {
            via.tick();
        }
        // Counter wraps 3,2,1,0,then underflow to 0xFFFF fires on the 4th tick
        assert!(!via.irq_pending());
        via.tick();
        assert!(via.ier == 0); // IER wasn't enabled, so IFR bit 6 sets but summary doesn't
        assert_eq!(via.read(0xD) & IFR_T1, IFR_T1, "IFR T1 bit should be set on underflow");
        // Further underflows do nothing more until reloaded (one-shot).
        via.write(0xD, IFR_T1); // clear it
        for _ in 0..70000 {
            via.tick();
        }
        assert_eq!(via.read(0xD) & IFR_T1, 0, "one-shot must not refire without a reload");
    }

    #[test]
    fn t1_free_run_refires_and_toggles_pb7() {
        let mut via = Via6522::new();
        via.write(0xB, 0xC0); // ACR: free-run (bit6=1), PB7 enabled (bit7=1)
        via.write(0x6, 1); // T1L-L = 1
        via.write(0x5, 0); // load: counter=1
        assert!(!via.pb7()); // starts low
        via.tick(); // 1 -> 0
        via.tick(); // 0 -> underflow ($FFFF): fires, toggles pb7
        assert!(via.pb7());
        assert_eq!(via.t1_counter(), 0xFFFF);
        via.tick(); // $FFFF -> reload to latch (1): the extra cycle of N+2
        assert_eq!(via.t1_counter(), 1);
        via.tick(); // 1 -> 0
        assert!(via.pb7());
        via.tick(); // 0 -> underflow again
        assert!(!via.pb7(), "second underflow, N+2 = 3 cycles later, should toggle pb7 back low");
    }

    #[test]
    fn ier_enables_the_shared_irq_summary_bit() {
        let mut via = Via6522::new();
        via.write(0xB, 0x00);
        via.write(0x6, 0);
        via.write(0x5, 0); // T1 loaded with 0 -> underflows next tick
        via.tick();
        assert_eq!(via.read(0xD) & IFR_T1, IFR_T1);
        assert!(!via.irq_pending(), "flag set but not enabled: no IRQ yet");
        via.write(0xE, 0x80 | IFR_T1); // enable T1 interrupt
        assert!(via.irq_pending(), "enabling an already-set flag must assert IRQ immediately");
    }

    #[test]
    fn ca1_edge_sets_flag_and_reading_ora_clears_it() {
        let mut via = Via6522::new();
        via.write(0xC, 0x00); // PCR: CA1 negative edge (bit0=0), CA2 input neg edge
        via.set_ca1(true);
        via.set_ca1(false); // negative edge
        assert_eq!(via.read(0xD) & IFR_CA1, IFR_CA1);
        // Reading ORA (reg 1) clears CA1 per the datasheet.
        let _ = via.read(0x1);
        assert_eq!(via.read(0xD) & IFR_CA1, 0);
    }

    #[test]
    fn ca2_independent_mode_does_not_autoclear_on_ora_access() {
        let mut via = Via6522::new();
        // CA2 control = 001 (independent interrupt, negative edge) in bits 1-3.
        via.write(0xC, 0b0000_0010);
        via.set_ca2(true);
        via.set_ca2(false);
        assert_eq!(via.read(0xD) & IFR_CA2, IFR_CA2);
        let _ = via.read(0x1);
        assert_eq!(via.read(0xD) & IFR_CA2, IFR_CA2, "independent mode must require an explicit IFR write to clear");
        via.write(0xD, IFR_CA2);
        assert_eq!(via.read(0xD) & IFR_CA2, 0);
    }

    #[test]
    fn ora_no_handshake_register_has_no_side_effects() {
        let mut via = Via6522::new();
        via.write(0xC, 0x00);
        via.set_ca1(true);
        via.set_ca1(false);
        assert_eq!(via.read(0xD) & IFR_CA1, IFR_CA1);
        let _ = via.read(0xF); // ORA, no handshake
        assert_eq!(via.read(0xD) & IFR_CA1, IFR_CA1, "reg $F must not clear CA1");
    }

    #[test]
    fn pcr_manual_output_modes_drive_ca2_cb2_directly() {
        let mut via = Via6522::new();
        // CA2 = manual high (111 in bits 1-3), CB2 = manual low (110 in bits 5-7)
        via.write(0xC, (0b111 << 1) | (0b110 << 5));
        assert!(via.ca2_output());
        assert!(!via.cb2_output());
    }

    #[test]
    fn port_a_latching_captures_value_at_ca1_edge() {
        let mut via = Via6522::new();
        via.write(0xB, 0x01); // ACR bit0: PA latching enabled
        via.write(0x3, 0x00); // DDRA all input
        via.set_pa_input(0x42);
        via.set_ca1(true);
        via.set_ca1(false); // active (negative) edge: latch 0x42
        via.set_pa_input(0x99); // the "byte" changes on the wire afterward
        assert_eq!(via.read(0x1), 0x42, "latched value must survive the input changing before the CPU reads it");
    }
}
