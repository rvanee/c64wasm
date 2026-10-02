// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! How CIA2's port A is wired to the serial bus.
//!
//! Outputs go through 7406 inverters: setting PA3 (ATN OUT), PA4 (CLOCK
//! OUT) or PA5 (DATA OUT) pulls that line low. Inputs come straight from
//! the lines: PA6 (CLOCK IN) and PA7 (DATA IN) read 0 while the line is
//! asserted. PA0-1 select the VIC-II's bank, PA2 is the user port.

use crate::cbm_iec::{IecLines, IecOutput};

const PA_ATN_OUT: u8 = 0b0000_1000;
const PA_CLOCK_OUT: u8 = 0b0001_0000;
const PA_DATA_OUT: u8 = 0b0010_0000;
const PA_CLOCK_IN: u8 = 0b0100_0000;
const PA_DATA_IN: u8 = 0b1000_0000;

/// What the C64 drives onto the bus, from CIA2's port A pins.
pub fn bus_output(cia2_pa: u8) -> IecOutput {
    IecOutput { atn: cia2_pa & PA_ATN_OUT != 0, clock: cia2_pa & PA_CLOCK_OUT != 0, data: cia2_pa & PA_DATA_OUT != 0 }
}

/// CIA2 port A input levels for the bus lines.
pub fn port_input(lines: IecLines) -> u8 {
    let mut bits = 0xFF;
    if lines.clock {
        bits &= !PA_CLOCK_IN;
    }
    if lines.data {
        bits &= !PA_DATA_IN;
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_assert_when_set() {
        assert_eq!(bus_output(0x00), IecOutput::default());
        assert_eq!(bus_output(PA_ATN_OUT), IecOutput { atn: true, ..Default::default() });
        assert_eq!(bus_output(PA_CLOCK_OUT | PA_DATA_OUT), IecOutput { atn: false, clock: true, data: true });
    }

    #[test]
    fn inputs_read_low_when_asserted() {
        assert_eq!(port_input(IecLines::default()), 0xFF);
        assert_eq!(port_input(IecLines { clock: true, ..Default::default() }), !PA_CLOCK_IN);
        assert_eq!(port_input(IecLines { data: true, atn: true, clock: false }), !PA_DATA_IN);
    }
}
