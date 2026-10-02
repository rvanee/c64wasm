// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! How VIA1 is wired to the serial bus (confirmed from the DOS ROM, see
//! `docs/1541-plan.md`).
//!
//! VIA1 Port B: bit 0 DATA IN, 1 DATA OUT, 2 CLOCK IN, 3 CLOCK OUT, 4 ATNA
//! (ATN acknowledge), 5-6 device number, 7 ATN IN. CA1 is ATN, active on
//! the rising edge (the DOS sets PCR = $01 at $EB2F). All inputs read 1
//! while the line is asserted, and outputs assert a line when set (the
//! board has inverting drivers and receivers).
//!
//! **ATN acknowledge.** An XOR gate (UD3) pulls DATA low whenever ATN and
//! ATNA differ, as in MAME's `c64h156` model: the moment the computer
//! asserts ATN, DATA is pulled low by hardware -- even while the drive's
//! CPU is busy -- and released once the DOS sets ATNA. This is what the
//! KERNAL's "device present?" check relies on.
//!
//! **Device number.** Two solder pads pull PB5/PB6: both closed reads 0 and
//! selects device 8; cutting them selects 9, 10 or 11. The DOS reads them
//! at $EB3A and computes its talk address `$48 + number`.

use crate::cbm_iec::{IecLines, IecOutput};

const PB_DATA_IN: u8 = 0b0000_0001;
const PB_DATA_OUT: u8 = 0b0000_0010;
const PB_CLOCK_IN: u8 = 0b0000_0100;
const PB_CLOCK_OUT: u8 = 0b0000_1000;
const PB_ATNA: u8 = 0b0001_0000;
const PB_DEVICE_LOW: u8 = 0b0010_0000;
const PB_DEVICE_HIGH: u8 = 0b0100_0000;
const PB_ATN_IN: u8 = 0b1000_0000;

pub const MIN_DEVICE: u8 = 8;
pub const MAX_DEVICE: u8 = 11;

/// The device-number bits (PB5-6) for device `number` (8-11).
pub fn device_number_bits(number: u8) -> u8 {
    assert!((MIN_DEVICE..=MAX_DEVICE).contains(&number), "device number must be 8-11, got {number}");
    let offset = number - MIN_DEVICE;
    let mut bits = 0u8;
    if offset & 0b01 != 0 {
        bits |= PB_DEVICE_LOW;
    }
    if offset & 0b10 != 0 {
        bits |= PB_DEVICE_HIGH;
    }
    bits
}

/// What the drive drives onto the bus, given VIA1's Port B pins and the
/// bus's ATN line (for the acknowledge gate).
pub fn bus_output(via1_pb: u8, atn: bool) -> IecOutput {
    let clock = via1_pb & PB_CLOCK_OUT != 0;
    let atna = via1_pb & PB_ATNA != 0;
    let data = via1_pb & PB_DATA_OUT != 0 || (atn != atna);
    IecOutput { atn: false, clock, data }
}

/// VIA1 Port B input bits for the bus lines.
pub fn port_input(lines: IecLines) -> u8 {
    let mut bits = 0u8;
    if lines.data {
        bits |= PB_DATA_IN;
    }
    if lines.clock {
        bits |= PB_CLOCK_IN;
    }
    if lines.atn {
        bits |= PB_ATN_IN;
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_assert_the_lines_their_bits_name() {
        assert_eq!(bus_output(0, false), IecOutput::default());
        assert_eq!(bus_output(PB_CLOCK_OUT | PB_DATA_OUT, false), IecOutput { atn: false, clock: true, data: true });
    }

    #[test]
    fn atn_without_acknowledge_pulls_data_until_the_dos_sets_atna() {
        assert!(bus_output(0, true).data, "hardware answers ATN at once");
        assert!(!bus_output(PB_ATNA, true).data, "acknowledged: DATA OUT alone decides");
        assert!(bus_output(PB_ATNA, false).data, "held until the DOS clears ATNA");
    }

    #[test]
    fn inputs_read_one_while_a_line_is_asserted() {
        assert_eq!(port_input(IecLines::default()), 0);
        assert_eq!(port_input(IecLines { atn: true, clock: true, data: true }), PB_DATA_IN | PB_CLOCK_IN | PB_ATN_IN);
    }

    #[test]
    fn device_number_pads_match_the_dos_talk_address_computation() {
        // $EB3A: talk address = $48 | (bit5 + 2 * bit6)
        for number in 8..=11u8 {
            let bits = device_number_bits(number);
            let computed = 8 + ((bits >> 5) & 1) + 2 * ((bits >> 6) & 1);
            assert_eq!(computed, number);
        }
    }

    #[test]
    #[should_panic]
    fn device_numbers_beyond_eleven_are_rejected() {
        device_number_bits(12);
    }
}
