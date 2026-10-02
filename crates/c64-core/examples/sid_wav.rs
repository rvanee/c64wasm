// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Boot, type a BASIC program, RUN it and record the SID to a WAV file.
//!
//! Usage: `sid_wav <program.bas> <out.wav> <seconds> [8580]`
//! Lines starting with REM and empty lines are skipped.

use std::io::Write;

use c64_core::sid::SidModel;
use c64_core::tools::RomFiles;
use c64_core::{VideoStandard, C64};

const RATE: u32 = 44_100;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: sid_wav <program.bas> <out.wav> <seconds> [8580]");
        std::process::exit(2);
    }
    let roms = RomFiles::from_repository().expect("ROMs in roms/");
    let mut c64 = C64::new(roms.system(), VideoStandard::Pal).unwrap();
    if args.get(4).is_some_and(|s| s == "8580") {
        c64.set_sid_model(SidModel::Mos8580);
    }
    c64.sid_mut().set_sample_rate(RATE);
    c64.run_cycles(2_600_000);

    let source = std::fs::read_to_string(&args[1])?;
    let mut text = Vec::new();
    for line in source.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("REM")) {
        text.extend(line.to_ascii_uppercase().bytes());
        text.push(b'\r');
    }
    text.extend(b"RUN\r");
    assert!(c64.type_text(&text), "the screen editor isn't taking keys");

    let mut audio = Vec::new();
    c64.take_audio(&mut Vec::new()); // drop the silence so far
    let seconds: f64 = args[3].parse().unwrap();
    let total = (seconds * c64.clock_hz() as f64) as u64;
    let mut done = 0;
    while done < total {
        c64.run_cycles(100_000);
        done += 100_000;
        c64.take_audio(&mut audio);
    }
    write_wav(&args[2], &audio)?;
    println!("{} samples ({:.1} s)", audio.len(), audio.len() as f64 / RATE as f64);
    Ok(())
}

/// 16-bit mono PCM.
fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    let bytes = samples.len() as u32 * 2;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&RATE.to_le_bytes())?;
    f.write_all(&(RATE * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&bytes.to_le_bytes())?;
    for s in samples {
        let v = ((s * 1.6).clamp(-1.0, 1.0) * 32767.0) as i16;
        f.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}
