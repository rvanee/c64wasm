// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The read/write electronics between VIA2 and the head: the bit clock,
//! the read shift register with sync detection, the write shift register,
//! and the byte-ready signal.
//!
//! - **Bit clock**: the density bits (VIA2 PB5-6) select one of four bit
//!   rates (`media::zone_bit_rate`); an integer accumulator turns that into
//!   bit cells per 1 MHz drive cycle without drift.
//! - **Read**: bits shift into a byte; ten or more 1-bits in a row are a
//!   sync mark (PB7 reads low while one passes) and restart the byte
//!   framing, so the first byte after a sync is aligned to it.
//! - **Write** (CB2 low): every byte time the value on VIA2 Port A is
//!   written over the 8 cells that byte time spanned (VICE's model). The
//!   CPU answers each byte-ready by putting the next byte on Port A, and a
//!   value left there is written again every byte time, which the
//!   formatter relies on.
//! - **Byte ready** fires for one cycle per completed byte; it drives VIA2
//!   CA1 and the CPU's SO pin.

use crate::drive1541::mechanics::Mechanics;
use crate::media::zone_bit_rate;

#[derive(Debug, Default)]
pub struct ReadWrite {
    density: u8,
    bit_accum: u32,
    write_mode: bool,
    /// Consecutive 1-bits read (sync detection).
    sync_run: u32,
    /// Bits of the current byte so far.
    bit_count: u8,
    shift_reg: u8,
    last_byte: u8,
    /// VIA2 Port A pins, loaded into the write shift register.
    write_data: u8,
    /// Cell where the current written byte began.
    write_start: usize,
    byte_ready: bool,
}

impl ReadWrite {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_density(&mut self, density: u8) {
        self.density = density;
    }

    /// Select write (`true`, CB2 low) or read mode.
    pub fn set_write_mode(&mut self, write: bool) {
        if write != self.write_mode {
            self.write_mode = write;
            self.bit_count = 0;
            self.shift_reg = 0;
            self.sync_run = 0;
            self.byte_ready = false;
        }
    }

    /// VIA2 Port A's pins, every cycle.
    pub fn set_write_data(&mut self, pa: u8) {
        self.write_data = pa;
    }

    /// The head moved to another track: framing starts over.
    pub fn head_moved(&mut self) {
        self.sync_run = 0;
        self.bit_count = 0;
        self.shift_reg = 0;
    }

    /// One drive cycle.
    pub fn clock(&mut self, mech: &mut Mechanics) {
        self.byte_ready = false;
        if !mech.spinning() {
            return;
        }
        self.bit_accum += zone_bit_rate(self.density);
        while self.bit_accum >= 1_000_000 {
            self.bit_accum -= 1_000_000;
            self.bit_cell(mech);
        }
    }

    fn bit_cell(&mut self, mech: &mut Mechanics) {
        if self.write_mode {
            let Some(pos) = mech.advance_write_cell() else { return };
            if self.bit_count == 0 {
                self.write_start = pos;
            }
            self.bit_count += 1;
            if self.bit_count == 8 {
                mech.write_cells(self.write_start, self.write_data);
                self.bit_count = 0;
                self.byte_ready = true;
            } else {
                self.byte_ready = false;
            }
            return;
        }
        // An empty half-track still clocks the data separator: it reads
        // zeros (never a sync), so byte-ready keeps its pace and the DOS's
        // seek-and-look-for-sync loop notices time passing.
        let bit = mech.read_cell().unwrap_or(0);
        self.shift_reg = (self.shift_reg << 1) | bit;
        self.bit_count += 1;
        if bit != 0 {
            self.sync_run += 1;
        } else {
            self.sync_run = 0;
        }
        if self.sync_run >= 10 {
            self.bit_count = 0;
            self.shift_reg = 0;
            self.byte_ready = false;
        } else if self.bit_count == 8 {
            self.last_byte = self.shift_reg;
            self.bit_count = 0;
            self.byte_ready = true;
        } else {
            self.byte_ready = false;
        }
    }

    /// The last byte read (VIA2 Port A input).
    pub fn last_byte(&self) -> u8 {
        self.last_byte
    }

    /// A byte completed in the last cycle.
    pub fn byte_ready(&self) -> bool {
        self.byte_ready
    }

    /// VIA2 PB7: low while a sync mark passes the head.
    pub fn no_sync(&self) -> bool {
        self.sync_run < 10
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::Disk;

    /// A disk whose half-track 2 (track 1) holds `track_bytes` at `zone`.
    fn disk_with_track(track_bytes: &[u8], zone: u8) -> Disk {
        let mut g64 = Vec::new();
        g64.extend_from_slice(b"GCR-1541");
        g64.push(0);
        g64.push(2);
        g64.extend_from_slice(&7928u16.to_le_bytes());
        let header_len = 12 + 2 * 4 * 2;
        for off in [0u32, header_len as u32] {
            g64.extend_from_slice(&off.to_le_bytes());
        }
        for z in [0u32, zone as u32] {
            g64.extend_from_slice(&z.to_le_bytes());
        }
        g64.extend_from_slice(&(track_bytes.len() as u16).to_le_bytes());
        g64.extend_from_slice(track_bytes);
        Disk::from_g64(&g64).unwrap()
    }

    fn spinning(disk: Disk) -> (ReadWrite, Mechanics) {
        let mut m = Mechanics::new();
        m.insert_disk(disk);
        m.set_motor(true);
        (ReadWrite::new(), m)
    }

    const CYCLES_PER_BIT: usize = 4; // zone 0: 250 kbit/s

    #[test]
    fn nothing_happens_while_the_motor_is_off() {
        let (mut rw, mut m) = spinning(disk_with_track(&[0xFF; 20], 3));
        m.set_motor(false);
        rw.set_density(3);
        for _ in 0..10_000 {
            rw.clock(&mut m);
            assert!(!rw.byte_ready());
        }
    }

    #[test]
    fn bytes_after_a_sync_mark_read_back_in_order() {
        let track = [vec![0xFFu8; 5], vec![0x08, 0xA5]].concat();
        let (mut rw, mut m) = spinning(disk_with_track(&track, 0));
        let mut bytes = Vec::new();
        for _ in 0..track.len() * 8 * CYCLES_PER_BIT {
            rw.clock(&mut m);
            if rw.byte_ready() {
                bytes.push(rw.last_byte());
            }
        }
        assert!(bytes.ends_with(&[0x08, 0xA5]), "{bytes:?}");
    }

    #[test]
    fn sync_reads_low_only_while_ten_or_more_ones_pass() {
        let track = [vec![0xFFu8; 5], vec![0x00]].concat();
        let (mut rw, mut m) = spinning(disk_with_track(&track, 0));
        assert!(rw.no_sync());
        for bit in 0..41 {
            for _ in 0..CYCLES_PER_BIT {
                rw.clock(&mut m);
            }
            match bit {
                0..=8 => assert!(rw.no_sync(), "bit {bit}"),
                9..=39 => assert!(!rw.no_sync(), "bit {bit}"),
                _ => assert!(rw.no_sync(), "the first 0 ends the sync"),
            }
        }
    }

    #[test]
    fn written_bytes_read_back() {
        let (mut rw, mut m) = spinning(disk_with_track(&[0u8; 4], 0));
        let data = [0x12u8, 0x34, 0xAB, 0xCD];
        let mut next = data.iter().copied();
        rw.set_write_data(next.next().unwrap());
        rw.set_write_mode(true);
        let mut pulses = 0;
        for _ in 0..data.len() * 8 * CYCLES_PER_BIT {
            rw.clock(&mut m);
            if rw.byte_ready() {
                pulses += 1;
                if let Some(b) = next.next() {
                    rw.set_write_data(b);
                }
            }
        }
        assert_eq!(pulses, data.len());
        rw.set_write_mode(false);
        let mut read = Vec::new();
        for _ in 0..data.len() * 8 * CYCLES_PER_BIT {
            rw.clock(&mut m);
            if rw.byte_ready() {
                read.push(rw.last_byte());
            }
        }
        assert_eq!(read, data);
    }

    #[test]
    fn a_byte_left_on_port_a_is_written_every_byte_time() {
        let (mut rw, mut m) = spinning(disk_with_track(&[0u8; 16], 0));
        rw.set_write_data(0x55);
        rw.set_write_mode(true);
        for _ in 0..16 * 8 * CYCLES_PER_BIT {
            rw.clock(&mut m);
        }
        assert_eq!(m.disk().unwrap().half_track_data(2).unwrap(), &[0x55u8; 16][..]);
    }
}
