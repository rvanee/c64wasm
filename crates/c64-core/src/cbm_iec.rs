// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The Commodore serial bus ("CBM IEC"): three open-collector lines, ATN,
//! CLOCK and DATA, shared by the computer and its drives. Any device can
//! pull a line low (assert it); a line is high only when nobody does. So
//! the level of each line is just "is anyone asserting it", recomputed
//! from every device's output each cycle.
//!
//! How each machine wires the lines to its ports is in
//! `c64::serial_port` (CIA2) and `drive1541::electronics::serial_interface`
//! (VIA1).

/// What one device drives onto the bus: `true` = pulling the line low.
/// Only the computer drives ATN.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IecOutput {
    pub atn: bool,
    pub clock: bool,
    pub data: bool,
}

/// The bus lines: `true` = asserted (low).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IecLines {
    pub atn: bool,
    pub clock: bool,
    pub data: bool,
}

/// The bus itself: the wired-AND of everyone's output.
#[derive(Debug, Clone, Copy, Default)]
pub struct IecBus {
    lines: IecLines,
}

impl IecBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recompute the lines from the computer's output and the devices'.
    pub fn update(&mut self, computer: IecOutput, devices: &[IecOutput]) {
        let clock = computer.clock || devices.iter().any(|d| d.clock);
        let data = computer.data || devices.iter().any(|d| d.data);
        self.lines = IecLines { atn: computer.atn, clock, data };
    }

    pub fn lines(&self) -> IecLines {
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_bus_has_all_lines_released() {
        let mut bus = IecBus::new();
        bus.update(IecOutput::default(), &[IecOutput::default()]);
        assert_eq!(bus.lines(), IecLines::default());
    }

    #[test]
    fn only_the_computer_drives_atn() {
        let mut bus = IecBus::new();
        let device = IecOutput { atn: true, clock: true, data: true };
        bus.update(IecOutput::default(), &[device, device]);
        assert!(!bus.lines().atn);
        bus.update(IecOutput { atn: true, ..Default::default() }, &[]);
        assert!(bus.lines().atn);
    }

    #[test]
    fn any_device_asserting_a_line_pulls_it_low_for_everyone() {
        let mut bus = IecBus::new();
        let device = IecOutput { data: true, ..Default::default() };
        bus.update(IecOutput { clock: true, ..Default::default() }, &[IecOutput::default(), device]);
        assert!(bus.lines().clock);
        assert!(bus.lines().data);
        bus.update(IecOutput::default(), &[IecOutput::default(), IecOutput::default()]);
        assert!(!bus.lines().clock && !bus.lines().data);
    }
}
