// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Clocks: the C64's system clock (set by its TV system) and the bridge
//! that runs a device with its own crystal, like the 1541, in step with it.

/// The C64's TV system, which fixes the VIC-II variant and the system
/// clock (derived from the colour subcarrier crystal: PAL 17.734472 MHz
/// / 18, NTSC 14.31818 MHz / 14).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VideoStandard {
    #[default]
    Pal,
    Ntsc,
}

impl VideoStandard {
    pub fn cycles_per_line(self) -> u16 {
        match self {
            VideoStandard::Pal => 63,
            VideoStandard::Ntsc => 65,
        }
    }

    pub fn lines_per_frame(self) -> u16 {
        match self {
            VideoStandard::Pal => 312,
            VideoStandard::Ntsc => 263,
        }
    }

    /// The system (phi2) clock in Hz.
    pub fn clock_hz(self) -> u32 {
        match self {
            VideoStandard::Pal => 985_248,
            VideoStandard::Ntsc => 1_022_727,
        }
    }
}

/// Runs a device clocked at `device_hz` from the ticks of a host clock at
/// `host_hz`, with exact integer arithmetic (no drift). The device runs
/// whole instructions, so it can be a few cycles ahead; that is carried
/// over as negative credit.
#[derive(Debug, Clone)]
pub struct ClockBridge {
    device_hz: i64,
    host_hz: i64,
    /// Device cycles owed, in units of 1/host_hz device cycles.
    credit: i64,
}

impl ClockBridge {
    pub fn new(device_hz: u32, host_hz: u32) -> Self {
        ClockBridge { device_hz: device_hz as i64, host_hz: host_hz as i64, credit: 0 }
    }

    /// One host cycle passed.
    #[inline]
    pub fn host_tick(&mut self) {
        self.credit += self.device_hz;
    }

    /// Whether the device is owed at least one cycle.
    #[inline]
    pub fn device_due(&self) -> bool {
        self.credit >= self.host_hz
    }

    /// The device ran `cycles` cycles.
    #[inline]
    pub fn device_ran(&mut self, cycles: u32) {
        self.credit -= cycles as i64 * self.host_hz;
    }

    /// Forget what is owed (a halted device).
    pub fn clear(&mut self) {
        self.credit = 0;
    }

    /// What is owed, in units of 1/host_hz device cycles (negative when
    /// the device ran ahead).
    pub fn owed(&self) -> i64 {
        self.credit
    }
}
