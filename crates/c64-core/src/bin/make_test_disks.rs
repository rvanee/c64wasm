// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Re-runnable generator for this project's own disk images (there's no
//! copyright-clean real-world 1541 image to grab off the shelf):
//!
//! - `test-disks/hello.d64` (+ `hello.g64`): "1541 TEST DISK",C6 with
//!   `HELLO` (prints "1541 TEST DISK OK"), `SHUTTLE` (the Space Shuttle
//!   bitmap drawing), `SPRITE DEMO` (a loader) and `SPRITES 4001` (the
//!   sprite-collision demo, which must live at $4001 because it puts its
//!   sprite data at $0800).
//! - `test-disks/empty.d64`: a freshly formatted, empty disk
//!   ("EMPTY DISK",01, 664 blocks free).
//!
//! Programs are tokenized from the listings in `test-disks/src/` with
//! `c64_core::tools::basic`, which is checked against the real screen editor by
//! `tests/disk_basic_roundtrip.rs`. Files are laid out like the 1541 DOS
//! lays them out (starting next to the directory track, sector interleave
//! 10), BAM and directory follow Peter Schepers' D64.TXT, and every image
//! is checked to round-trip through the project's own GCR encode/decode
//! before it is written.
//!
//! `SPRITE DEMO` uses the classic "dynamic keyboard" trick: it prints
//! three commands on the screen (move BASIC to $4001 + NEW; LOAD the real
//! program with ,8,1; RUN), then stuffs HOME and three RETURNs into the
//! keyboard buffer, so the screen editor executes them one after another.
//! The rows (0, 3, 8) are where the cursor lands after each command's own
//! output: BASIC prints a blank line before READY., and LOAD prints a
//! blank line, SEARCHING FOR..., LOADING and READY.

use c64_core::media::{sectors_per_track, Disk};
use c64_core::tools::basic;

fn sector_offset(track: u8, sector: u8) -> usize {
    (1..track).map(|t| sectors_per_track(t).unwrap() as usize).sum::<usize>() * 256 + sector as usize * 256
}

/// A D64 under construction: tracks of free/used sectors plus directory.
struct D64 {
    bytes: Vec<u8>,
    used: Vec<Vec<bool>>, // [track-1][sector]
    entries: Vec<[u8; 30]>,
    next_track_order: Vec<u8>,
}

impl D64 {
    fn new() -> Self {
        let used = (1..=35u8).map(|t| vec![false; sectors_per_track(t).unwrap() as usize]).collect();
        // The DOS fills tracks outward from the directory: 17, 19, 16, 20, ...
        let mut order = Vec::new();
        for d in 1..=17u8 {
            order.push(18 - d);
            if 18 + d <= 35 {
                order.push(18 + d);
            }
        }
        D64 { bytes: vec![0u8; 174_848], used, entries: Vec::new(), next_track_order: order }
    }

    fn alloc(&mut self, last: Option<(u8, u8)>) -> (u8, u8) {
        // Same track, 10 sectors on (the DOS's interleave), else next track.
        if let Some((t, s)) = last {
            let n = sectors_per_track(t).unwrap();
            for k in 0..n {
                let cand = (s + 10 + k) % n;
                if !self.used[t as usize - 1][cand as usize] {
                    self.used[t as usize - 1][cand as usize] = true;
                    return (t, cand);
                }
            }
        }
        for &t in &self.next_track_order {
            if let Some(s) = self.used[t as usize - 1].iter().position(|u| !u) {
                self.used[t as usize - 1][s] = true;
                return (t, s as u8);
            }
        }
        panic!("disk full");
    }

    fn add_prg(&mut self, name: &str, prg: &[u8]) {
        let chunks: Vec<&[u8]> = prg.chunks(254).collect();
        let mut place = Vec::new();
        let mut last = None;
        for _ in &chunks {
            let ts = self.alloc(last);
            place.push(ts);
            last = Some(ts);
        }
        for (i, chunk) in chunks.iter().enumerate() {
            let (t, s) = place[i];
            let off = sector_offset(t, s);
            if let Some(&(nt, ns)) = place.get(i + 1) {
                self.bytes[off] = nt;
                self.bytes[off + 1] = ns;
            } else {
                self.bytes[off] = 0;
                self.bytes[off + 1] = (chunk.len() + 1) as u8; // index of the last used byte
            }
            self.bytes[off + 2..off + 2 + chunk.len()].copy_from_slice(chunk);
        }
        // 30-byte directory entry body (entry offsets $02-$1F).
        let mut e = [0u8; 30];
        e[0] = 0x82; // closed PRG
        e[1] = place[0].0;
        e[2] = place[0].1;
        for (i, slot) in e[3..19].iter_mut().enumerate() {
            *slot = *name.as_bytes().get(i).unwrap_or(&0xA0);
        }
        e[28] = chunks.len() as u8; // block count, entry offset $1E
        e[29] = (chunks.len() >> 8) as u8;
        self.entries.push(e);
    }

    fn finish(mut self, disk_name: &str, id: &str) -> Vec<u8> {
        assert!(self.entries.len() <= 8, "this builder only writes one directory sector");
        self.used[17][0] = true;
        self.used[17][1] = true;
        let dir = sector_offset(18, 1);
        self.bytes[dir] = 0x00;
        self.bytes[dir + 1] = 0xFF;
        for (i, e) in self.entries.iter().enumerate() {
            let off = dir + i * 32 + 2;
            self.bytes[off..off + 30].copy_from_slice(e);
        }
        let bam = sector_offset(18, 0);
        let b = &mut self.bytes;
        b[bam] = 18;
        b[bam + 1] = 1;
        b[bam + 2] = b'A';
        for (i, slot) in b[bam + 0x90..bam + 0xA0].iter_mut().enumerate() {
            *slot = *disk_name.as_bytes().get(i).unwrap_or(&0xA0);
        }
        b[bam + 0xA0] = 0xA0;
        b[bam + 0xA1] = 0xA0;
        b[bam + 0xA2] = id.as_bytes()[0];
        b[bam + 0xA3] = id.as_bytes()[1];
        b[bam + 0xA4] = 0xA0;
        b[bam + 0xA5] = b'2';
        b[bam + 0xA6] = b'A';
        for k in 0xA7..=0xAA {
            b[bam + k] = 0xA0;
        }
        for t in 1..=35usize {
            let used = &self.used[t - 1];
            let mut bits: u32 = 0;
            for (s, &u) in used.iter().enumerate() {
                if !u {
                    bits |= 1 << s;
                }
            }
            let off = bam + 4 + (t - 1) * 4;
            b[off] = used.iter().filter(|u| !**u).count() as u8;
            b[off + 1] = bits as u8;
            b[off + 2] = (bits >> 8) as u8;
            b[off + 3] = (bits >> 16) as u8;
        }
        self.bytes
    }
}

/// Walk a file's sector chain back out of a finished image.
fn read_file(d64: &[u8], name: &str) -> Vec<u8> {
    let dir = sector_offset(18, 1);
    for i in 0..8 {
        let e = dir + i * 32 + 2;
        let n: Vec<u8> = d64[e + 3..e + 19].iter().copied().take_while(|&c| c != 0xA0).collect();
        if n == name.as_bytes() {
            let (mut t, mut s) = (d64[e + 1], d64[e + 2]);
            let mut out = Vec::new();
            loop {
                let off = sector_offset(t, s);
                if d64[off] == 0 {
                    out.extend_from_slice(&d64[off + 2..off + d64[off + 1] as usize + 1]);
                    return out;
                }
                out.extend_from_slice(&d64[off + 2..off + 256]);
                (t, s) = (d64[off], d64[off + 1]);
            }
        }
    }
    panic!("{name} not in directory");
}

fn check_and_write(dir: &std::path::Path, stem: &str, d64: &[u8], with_g64: bool) {
    let disk = Disk::from_d64(d64).expect("built D64 must parse");
    assert_eq!(disk.to_d64(), d64, "{stem}: must round-trip exactly through GCR synthesis + decode");
    std::fs::write(dir.join(format!("{stem}.d64")), d64).unwrap();
    println!("wrote {stem}.d64");
    if with_g64 {
        let g64 = disk.to_g64();
        let reloaded = Disk::from_g64(&g64).expect("G64 must parse");
        for t in 1..=35u8 {
            assert_eq!(reloaded.track_data(t), disk.track_data(t), "track {t} must survive D64 -> G64 -> parse");
        }
        std::fs::write(dir.join(format!("{stem}.g64")), g64).unwrap();
        println!("wrote {stem}.g64");
    }
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test-disks");
    let src = |f: &str| std::fs::read_to_string(dir.join("src").join(f)).unwrap();

    let files: Vec<(&str, Vec<u8>)> = vec![
        ("HELLO", basic::prg(&src("hello.bas"), 0x0801)),
        ("SHUTTLE", basic::prg(&src("shuttle.bas"), 0x0801)),
        ("SPRITE DEMO", basic::prg(&src("sprite-demo.bas"), 0x0801)),
        ("SPRITES 4001", basic::prg(&src("sprites.bas"), 0x4001)),
    ];
    let mut d = D64::new();
    for (name, prg) in &files {
        d.add_prg(name, prg);
    }
    let hello = d.finish("1541 TEST DISK", "C6");
    for (name, prg) in &files {
        assert_eq!(&read_file(&hello, name), prg, "{name}: sector chain must read back exactly");
        println!("  {name:<14} {:>5} bytes, {} blocks", prg.len(), prg.len().div_ceil(254));
    }
    check_and_write(&dir, "hello", &hello, true);

    let empty = D64::new().finish("EMPTY DISK", "01");
    check_and_write(&dir, "empty", &empty, false);
    println!("all self-checks passed.");
}
