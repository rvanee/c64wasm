// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The drive mechanism: spindle motor, stepper motor and head carriage,
//! the write-protect sensor and the disk itself. Everything electronic
//! (the bit clock, shift registers, sync detection) is in
//! `electronics::read_write`.
//!
//! ## Stepper
//! The stepper has four phases driven by VIA2 PB0-1. Stepping the phase
//! up (mod 4) moves the head one half-track inward (towards higher track
//! numbers), down moves it outward (DOS seek routine at $FA2E-$FA78). At
//! the outer stop the head can't move further; the DOS's famous "head
//! bang" when it recalibrates is stepping against that stop.
//!
//! ## Two half-track numbering spaces
//! The head position counts physical steps from the outer stop, which is
//! half-track 1 (track 1); track `t` is at physical half-track `2t - 1`.
//! `media::Disk` stores track `t` at half-track `2t`, so the disk is
//! addressed with `physical + 1`.
//!
//! ## Rotation
//! The head reads or writes the cell at `bit_pos` of the current track's
//! bit stream; the read/write electronics advance it one cell per bit
//! clock. After a seek the position starts at 0 (the real landing point is
//! arbitrary, and the DOS always resynchronises on a sync mark).

use crate::media::{Disk, HALF_TRACK_SLOTS};

#[derive(Debug, Default)]
pub struct Mechanics {
    disk: Option<Disk>,
    /// Head position, physical half-tracks (see the module doc).
    half_track: u8,
    /// Last stepper phase seen, to tell which way it moved. `None` until
    /// the first one: establishing the phase isn't a step.
    last_phase: Option<u8>,
    motor_on: bool,
    /// Rotational position: bit index into the current track.
    bit_pos: usize,
    /// Drive cycles since power-on (the time base of `head_events`).
    cycles: u64,
    /// Steps since the last `take_head_events`: (cycle, bumped against the
    /// stop). For the stepper sounds of a front end.
    head_events: Vec<(u64, bool)>,
}

impl Mechanics {
    pub fn new() -> Self {
        // Parked at the outer stop (track 1), where the DOS's power-on
        // recalibration leaves it.
        Mechanics { half_track: 1, ..Default::default() }
    }

    pub fn insert_disk(&mut self, disk: Disk) {
        self.disk = Some(disk);
    }

    pub fn eject_disk(&mut self) -> Option<Disk> {
        self.disk.take()
    }

    pub fn disk(&self) -> Option<&Disk> {
        self.disk.as_ref()
    }

    pub fn set_motor(&mut self, on: bool) {
        self.motor_on = on;
    }

    pub fn motor_on(&self) -> bool {
        self.motor_on
    }

    /// The disk is turning under the head.
    pub fn spinning(&self) -> bool {
        self.motor_on && self.disk.is_some()
    }

    /// Drive the stepper phase (0-3). Returns true if the head moved.
    pub fn set_stepper_phase(&mut self, phase: u8) -> bool {
        let mut moved = false;
        if let Some(last) = self.last_phase {
            if phase != last {
                // A jump of two isn't a single step and is ignored.
                match (phase as i8 - last as i8).rem_euclid(4) {
                    1 => moved = self.move_head(1),
                    3 => moved = self.move_head(-1),
                    _ => {}
                }
            }
        }
        self.last_phase = Some(phase);
        moved
    }

    fn move_head(&mut self, delta: i8) -> bool {
        let max = HALF_TRACK_SLOTS as i16 - 1;
        let new = (self.half_track as i16 + delta as i16).clamp(1, max) as u8;
        if self.head_events.len() < 4096 {
            self.head_events.push((self.cycles, new == self.half_track));
        }
        if new == self.half_track {
            return false;
        }
        self.half_track = new;
        self.bit_pos = 0;
        true
    }

    /// One drive cycle passed.
    pub fn clock(&mut self) {
        self.cycles += 1;
    }

    /// The bit under the head, advancing one cell; `None` on a half-track
    /// without data (the disk turns, but there's nothing to read).
    pub fn read_cell(&mut self) -> Option<u8> {
        let track = self.disk.as_ref()?.half_track_data(self.half_track + 1)?;
        if track.is_empty() {
            return None;
        }
        let bit_len = track.len() * 8;
        let pos = self.bit_pos;
        let bit = (track[pos / 8] >> (7 - (pos % 8) as u8)) & 1;
        self.bit_pos = (pos + 1) % bit_len;
        Some(bit)
    }

    /// The cell position under the head before advancing one cell for
    /// writing; `None` on a half-track without data.
    pub fn advance_write_cell(&mut self) -> Option<usize> {
        let track = self.disk.as_ref()?.half_track_data(self.half_track + 1)?;
        if track.is_empty() {
            return None;
        }
        let pos = self.bit_pos;
        self.bit_pos = (pos + 1) % (track.len() * 8);
        Some(pos)
    }

    /// Write `value` MSB first into the 8 cells starting at `start`
    /// (wrapping around the track).
    pub fn write_cells(&mut self, start: usize, value: u8) {
        let half_track = self.half_track + 1;
        let Some(track) = self.disk.as_mut().and_then(|d| d.half_track_data_mut(half_track)) else { return };
        let bit_len = track.len() * 8;
        for k in 0..8 {
            let p = (start + k) % bit_len;
            let mask = 1u8 << (7 - (p % 8));
            if (value >> (7 - k)) & 1 != 0 {
                track[p / 8] |= mask;
            } else {
                track[p / 8] &= !mask;
            }
        }
    }

    /// Write-protect sensor (VIA2 PB4): true = writable. The notch is
    /// open on a writable disk; with no disk the sensor reads protected.
    pub fn writable(&self) -> bool {
        self.disk.as_ref().is_some_and(|d| !d.write_protected)
    }

    /// Head position in physical half-tracks (track `t` = `2t - 1`).
    pub fn current_half_track(&self) -> u8 {
        self.half_track
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn take_head_events(&mut self, out: &mut Vec<(u64, bool)>) {
        out.append(&mut self.head_events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepper_moves_the_head_and_stops_at_both_ends() {
        let mut m = Mechanics::new();
        let mut phase = 0u8;
        m.set_stepper_phase(phase);
        assert_eq!(m.current_half_track(), 1);
        for _ in 0..200 {
            phase = (phase + 1) % 4;
            m.set_stepper_phase(phase);
        }
        assert_eq!(m.current_half_track(), HALF_TRACK_SLOTS as u8 - 1);
        for _ in 0..300 {
            phase = (phase + 3) % 4;
            m.set_stepper_phase(phase);
        }
        assert_eq!(m.current_half_track(), 1);
        for _ in 0..5 {
            phase = (phase + 1) % 4;
            m.set_stepper_phase(phase);
        }
        assert_eq!(m.current_half_track(), 6);
        // holding a phase doesn't move the head
        assert!(!m.set_stepper_phase(phase));
        assert_eq!(m.current_half_track(), 6);
    }

    #[test]
    fn stepping_against_the_stop_is_recorded_as_a_bump() {
        let mut m = Mechanics::new();
        m.set_stepper_phase(0);
        m.set_stepper_phase(3); // outward, already at the stop
        let mut ev = Vec::new();
        m.take_head_events(&mut ev);
        assert_eq!(ev, vec![(0, true)]);
    }
}
