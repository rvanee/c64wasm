// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! JavaScript bindings for `c64-core`: one [`Machine`] per emulator session.
//! Timing (real time or as fast as possible), the PC-keyboard mapping and
//! everything on screen are the page's business; this crate only passes
//! calls through.

use c64_core::sid::SidModel;
use c64_core::{SystemRoms, VideoStandard, C64};
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen(start)]
pub fn init() {
    // Rust panics show up in the browser console with their message.
    console_error_panic_hook::set_once();
}

/// A C64 with its disk drives.
#[wasm_bindgen]
pub struct Machine {
    c64: C64,
}

#[wasm_bindgen]
impl Machine {
    /// Power on with the BASIC (8 KB), KERNAL (8 KB) and character (4 KB)
    /// ROMs. `ntsc` picks the NTSC machine (65 cycles per line, 263 lines,
    /// 1022727 Hz) instead of PAL (63, 312, 985248 Hz).
    #[wasm_bindgen(constructor)]
    pub fn new(basic: &[u8], kernal: &[u8], chargen: &[u8], ntsc: bool) -> Result<Machine, JsError> {
        let standard = if ntsc { VideoStandard::Ntsc } else { VideoStandard::Pal };
        let c64 = C64::new(SystemRoms { basic, kernal, chargen }, standard).map_err(js_err)?;
        Ok(Machine { c64 })
    }

    /// The system clock in Hz, for pacing against real time.
    pub fn clock_hz(&self) -> u32 {
        self.c64.clock_hz()
    }

    /// Run at least `cycles` cycles (whole instructions); returns the cycles
    /// actually run. Stops early if the CPU jams.
    pub fn run_cycles(&mut self, cycles: u32) -> u32 {
        self.c64.run_cycles(cycles as u64) as u32
    }

    /// The CPU hit a JAM opcode and hangs until reset.
    pub fn jammed(&self) -> bool {
        self.c64.jammed()
    }

    /// Where the RGBA picture is in wasm memory. Read it again every
    /// frame: memory can move when it grows.
    pub fn framebuffer_ptr(&self) -> *const u8 {
        self.c64.framebuffer().as_ptr()
    }

    pub fn framebuffer_width(&self) -> u32 {
        self.c64.framebuffer_size().0
    }

    pub fn framebuffer_height(&self) -> u32 {
        self.c64.framebuffer_size().1
    }

    // ---- keyboard ----

    /// Press or release a key of the matrix: `row` is the CIA1 port A line,
    /// `col` the port B line (see `c64_core::c64::keyboard`).
    pub fn set_key(&mut self, row: u8, col: u8, pressed: bool) {
        self.c64.set_key(row as usize & 7, col as usize & 7, pressed);
    }

    /// Release everything (the page lost focus mid-press).
    pub fn release_all_keys(&mut self) {
        self.c64.release_all_keys();
    }

    pub fn press_restore(&mut self) {
        self.c64.press_restore();
    }

    // ---- sound ----

    /// The audio output rate (the AudioContext's); 0 stops collecting.
    pub fn set_audio_rate(&mut self, hz: u32) {
        self.c64.sid_mut().set_sample_rate(hz);
    }

    /// The audio since the last call: mono samples, about -1..1.
    pub fn take_audio(&mut self) -> Vec<f32> {
        let mut samples = Vec::new();
        self.c64.take_audio(&mut samples);
        samples
    }

    /// `true` = 8580 (C64C), `false` = 6581 (the original). Resets the SID.
    pub fn set_sid_8580(&mut self, yes: bool) {
        self.c64.set_sid_model(if yes { SidModel::Mos8580 } else { SidModel::Mos6581 });
    }

    // ---- disk drives ----

    /// Connect a 1541 with its 16 KB DOS ROM as device 8-11; returns its
    /// index.
    pub fn attach_drive(&mut self, dos_rom: &[u8], device_number: u8) -> Result<usize, JsError> {
        self.c64.attach_drive(dos_rom, device_number).map_err(js_err)
    }

    pub fn drive_count(&self) -> usize {
        self.c64.drive_count()
    }

    /// Insert a D64 or G64 image.
    pub fn insert_disk(&mut self, drive: usize, image: &[u8]) -> Result<(), JsError> {
        self.c64.insert_disk(drive, image).map_err(js_err)
    }

    /// Returns whether there was a disk.
    pub fn eject_disk(&mut self, drive: usize) -> bool {
        self.c64.eject_disk(drive).is_some()
    }

    /// The disk in the drive as a D64 image (empty if there is none).
    pub fn extract_disk(&self, drive: usize) -> Vec<u8> {
        self.c64.extract_disk(drive).unwrap_or_default()
    }

    pub fn drive_has_disk(&self, drive: usize) -> bool {
        self.c64.drive(drive).is_some_and(|d| d.disk().is_some())
    }

    /// Head steps since the last call, as `[seconds ago, bumped, ...]`;
    /// `bumped` is 1 when the head hit the end stop. For drive sounds.
    pub fn drive_head_events(&mut self, drive: usize) -> Vec<f64> {
        let Some(d) = self.c64.drive_mut(drive) else { return Vec::new() };
        let mut events = Vec::new();
        d.take_head_events(&mut events);
        let now = d.cycles();
        events
            .into_iter()
            .flat_map(|(cycle, bumped)| {
                [now.saturating_sub(cycle) as f64 / 1_000_000.0, if bumped { 1.0 } else { 0.0 }]
            })
            .collect()
    }

    /// How much of the time (0-1) the red activity LED was lit since the
    /// last call; call once per frame.
    pub fn drive_led(&mut self, drive: usize) -> f32 {
        self.c64.drive_mut(drive).map_or(0.0, |d| d.take_led_duty())
    }

    pub fn drive_motor(&self, drive: usize) -> bool {
        self.c64.drive(drive).is_some_and(|d| d.motor_on())
    }

    /// The head's track (1-based; 18.5 is a half track).
    pub fn drive_track(&self, drive: usize) -> f32 {
        self.c64.drive(drive).map_or(0.0, |d| (d.current_half_track() as f32 + 1.0) / 2.0)
    }
}
