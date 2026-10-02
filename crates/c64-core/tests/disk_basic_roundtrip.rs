// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Task 9 end-to-end regression tests: a real C64 (BASIC + KERNAL) and a
//! real 1541 (DOS ROM), co-simulated through the IEC bus, doing what a
//! user would do at the keyboard:
//!
//! - `LOAD"$",8` + `LIST`, and `LOAD"HELLO",8,1` + `RUN`, from the
//!   project's own fixture disk;
//! - formatting a blank disk with `OPEN15,8,15,"N:..."`, typing in the
//!   Space Shuttle listing, `SAVE`-ing it,
//!   `NEW`, `LOAD`-ing it back and comparing the tokenized program byte
//!   for byte -- then exporting the disk as a D64 and loading it again on
//!   a freshly booted machine (so the bytes really went through the
//!   drive's GCR write path and back);
//! - the sprite-collision listing (`SpriteCollision.bas`), saved from a
//!   relocated BASIC start ($4001) and loaded back both with `,8,1`
//!   (absolute) and, on a fresh machine, with `,8` (relocated to $0801).
//!
//! Text is fed through the KERNAL keyboard buffer ($0277/$C6) rather than
//! the CIA1 matrix, for speed: the matrix is tested in `tests/boot.rs`,
//! and these tests are about the disk path.
//!
//! Skips gracefully if the copyrighted ROM images aren't present, same as
//! the other ROM-backed tests.

mod common;

use c64_core::tools::basic;
use c64_core::tools::RomFiles;
use c64_core::{VideoStandard, C64};
use std::path::PathBuf;

fn load_roms(test: &str) -> Option<RomFiles> {
    common::roms(test, true)
}

fn hello_d64() -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-disks/hello.d64")).unwrap()
}

/// Generous upper bounds, in C64 cycles (~1 s each); a failure means a hang.
const SECOND: u64 = 1_000_000;

struct Machine {
    c64: C64,
    drive: usize,
}

impl Machine {
    fn boot(roms: &RomFiles, disk: &[u8]) -> Machine {
        let mut c64 = C64::new(roms.system(), VideoStandard::Pal).unwrap();
        let drive = c64.attach_drive(roms.dos1541.as_ref().unwrap(), 8).unwrap();
        c64.insert_disk(drive, disk).unwrap();
        let mut m = Machine { c64, drive };
        // C64 reaches READY. after ~2.5M cycles; the drive's own ROM
        // checksum + init takes ~1M drive cycles. 3 s covers both.
        m.run(3 * SECOND);
        assert!(m.screen().contains("READY."), "C64 did not boot:\n{}", m.screen());
        m
    }

    fn total(&self) -> u64 {
        self.c64.cycles()
    }

    fn run(&mut self, cycles: u64) {
        let end = self.total() + cycles;
        while self.total() < end {
            assert!(!self.c64.jammed(), "C64 CPU jammed at ${:04X}", self.c64.cpu().pc);
            assert!(!self.c64.drive(self.drive).unwrap().jammed(), "drive CPU jammed");
            self.c64.step();
        }
    }

    /// True once the screen editor is sitting in its wait-for-key loop
    /// (`$E5CD`-`$E5D4`, the KERNAL's `LDA $C6 / STA $CC / STA $0292 /
    /// BEQ` loop) for most of a 20k-cycle window with nothing buffered --
    /// i.e. the last command has finished and BASIC is back at the prompt.
    fn idle_window(&mut self) -> bool {
        let end = self.total() + 20_000;
        let (mut in_loop, mut n) = (0u32, 0u32);
        while self.total() < end {
            self.c64.step();
            n += 1;
            if (0xE5CD..=0xE5D5).contains(&self.c64.cpu().pc) {
                in_loop += 1;
            }
        }
        self.c64.peek_ram(0xC6) == 0 && in_loop * 2 > n
    }

    fn wait_idle(&mut self, max_cycles: u64, what: &str) {
        let start = self.total();
        let mut consecutive = 0;
        while consecutive < 2 {
            assert!(
                self.total() - start < max_cycles,
                "timed out after {} cycles waiting for {what} to finish; C64 PC ${:04X}, drive PC ${:04X}\n{}",
                self.total() - start,
                self.c64.cpu().pc,
                self.c64.drive(self.drive).unwrap().pc(),
                self.screen()
            );
            if self.idle_window() {
                consecutive += 1;
            } else {
                consecutive = 0;
            }
        }
    }

    /// Type text (uppercase ASCII is unshifted PETSCII for everything
    /// these listings use; newline is RETURN) through the keyboard buffer.
    fn type_text(&mut self, text: &str) {
        let bytes: Vec<u8> = text.bytes().map(|b| if b == b'\n' { 13 } else { b.to_ascii_uppercase() }).collect();
        assert!(
            self.c64.type_text(&bytes),
            "keyboard buffer never drained (PC ${:04X})\n{}",
            self.c64.cpu().pc,
            self.screen()
        );
    }

    /// Type one line + RETURN and wait for BASIC to come back to the prompt.
    fn command(&mut self, line: &str, max_cycles: u64) {
        self.type_text(line);
        self.type_text("\n");
        self.wait_idle(max_cycles, line);
        let s = self.screen();
        for err in ["?SYNTAX", "ERROR", "?FILE NOT FOUND", "?DEVICE NOT PRESENT", "BREAK"] {
            assert!(!s.contains(err), "after {line:?} the screen shows {err}:\n{s}");
        }
    }

    fn type_program(&mut self, lines: &[&str]) {
        for line in lines {
            self.type_text(line);
            self.type_text("\n");
        }
        self.wait_idle(60 * SECOND, "typing the program");
    }

    fn screen(&self) -> String {
        common::screen_text(&self.c64)
    }

    fn peek16(&self, addr: u16) -> u16 {
        self.c64.peek_ram(addr) as u16 | (self.c64.peek_ram(addr + 1) as u16) << 8
    }

    /// The tokenized program in memory: TXTTAB ($2B) up to VARTAB ($2D).
    fn program(&self) -> (u16, Vec<u8>) {
        let (start, end) = (self.peek16(0x2B), self.peek16(0x2D));
        assert!(end > start, "no program in memory (TXTTAB ${start:04X}, VARTAB ${end:04X})");
        (start, (start..end).map(|a| self.c64.peek_ram(a)).collect())
    }

    /// `LOAD"$",8` then `LIST`, returning the screen.
    fn directory(&mut self) -> String {
        self.command("LOAD\"$\",8", 30 * SECOND);
        self.type_text("PRINT CHR$(147)\n");
        self.wait_idle(5 * SECOND, "clear screen");
        self.command("LIST", 10 * SECOND);
        self.screen()
    }
}

/// The program's lines as (line number, token bytes), dropping the 2-byte
/// next-line links -- the only bytes that legitimately differ when the
/// same program is loaded at a different address and relinked by BASIC.
fn lines_without_links(base: u16, bytes: &[u8]) -> Vec<(u16, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let link = bytes[i] as u16 | (bytes[i + 1] as u16) << 8;
        if link == 0 {
            break;
        }
        let num = bytes[i + 2] as u16 | (bytes[i + 3] as u16) << 8;
        let body_end = bytes[i + 4..].iter().position(|&b| b == 0).unwrap() + i + 4;
        out.push((num, bytes[i + 4..body_end].to_vec()));
        i = (link - base) as usize;
    }
    out
}

/// The Space Shuttle drawing (also `test-disks/src/shuttle.bas`).
const SHUTTLE: &[&str] = &[
    "10 GOTO 1000",
    "100 POKE 53272,PEEK(53272) OR 8",
    "110 POKE 53265,PEEK(53265) OR 32",
    "120 RETURN",
    "200 DATA 0,165,252,197,254,208,7,165",
    "210 DATA 251,197,253,208,1,96,160,0",
    "220 DATA 173,80,195,145,251,230,251",
    "230 DATA 208,232,230,252,76,81,195",
    "240 RESTORE : FOR C=50000 TO 50029",
    "250 READ BYTE : POKE C,BYTE : NEXT C",
    "260 POKE 251,0 : POKE 252,4 : POKE 253,232",
    "270 POKE 254,7 : POKE 50000,COL : SYS 50001",
    "280 POKE 251,0 : POKE 252,32 : POKE 253,64",
    "290 POKE 254,63 : POKE 50000,0 : SYS 50001 : RETURN",
    "300 BYTE=8192+INT(LY/8)*320+INT(LX/8)*8+(LY AND 7)",
    "310 MASK=2^(7-(LX AND 7))",
    "320 RETURN",
    "400 GOSUB 300",
    "410 POKE BYTE,PEEK(BYTE) OR MASK",
    "420 CMEM=1024+INT(LY/8)*40+INT(LX/8)",
    "430 POKE CMEM,COL",
    "440 RETURN",
    "500 GOSUB 300",
    "520 POKE BYTE,PEEK(BYTE) AND (255-MASK)",
    "530 RETURN",
    "600 GT=ABS(NX-LX)",
    "610 IF ABS(NY-LY)>GT THEN GT=ABS(NY-LY)",
    "620 XINC=(NX-LX)/GT : YINC=(NY-LY)/GT",
    "630 XX=LX+0.5 : YY=LY+0.5",
    "640 FOR CC=1 TO GT",
    "650 LX=INT(XX) : LY=INT(YY) : GOSUB 400",
    "660 XX=XX+XINC : YY=YY+YINC",
    "670 NEXT CC : LX=NX : LY=NY : RETURN",
    "1000 POKE 53280,0 : GOSUB 100",
    "1010 COL=208 : GOSUB 200",
    "1020 PEN=0 : LX=0 : LY=0",
    "1030 READ NX,NY",
    "1040 IF NX>=0 THEN 1090",
    "1050 IF NY=1 THEN PEN=1 : GOTO 1030",
    "1060 IF NY=0 THEN PEN=0 : GOTO 1030",
    "1070 IF NY=2 THEN 1070",
    "1080 COL=-NY : GOTO 1030",
    "1090 IF PEN=0 THEN LX=NX : LY=NY : GOTO 1030",
    "1100 GOSUB 600 : LX=NX : LY=NY : GOTO 1030",
    "1200 DATA -1,0,49,123,-1,1,74,102,73,112,78,112,81,98,74,102,88,94,212,78",
    "1210 DATA 207,85,254,4,266,2,256,82,207,85,-1,0,258,72,-1,1",
    "1220 DATA 272,70,286,98,250,137,236,108,272,70",
    "1230 DATA 258,72,272,62,304,60,280,86,-1,0,245,125,-1,1",
    "1240 DATA 90,130,136,156,145,192,173,195,245,125",
    "1250 DATA -1,0,236,108,-1,1,158,114,-1,0,106,140,-1,1,78,140,49,134,49,123",
    "1260 DATA -1,0,250,137,-1,1,233,137",
    "1360 DATA -1,2",
];

/// `SpriteCollision.bas`, minus its REM header (read from the file so the
/// test and the listing can't drift apart).
fn sprite_program() -> Vec<String> {
    let src = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("SpriteCollision.bas")).unwrap();
    src.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("REM")).map(|l| l.to_string()).collect()
}

/// The offline tokenizer (`c64_core::tools::basic`, used to build the demo disk)
/// must produce exactly what the real screen editor produces when the same
/// listings are typed in.
#[test]
fn offline_tokenizer_matches_the_real_screen_editor() {
    let Some(roms) = load_roms("offline_tokenizer_matches_the_real_screen_editor") else { return };
    let mut m = Machine::boot(&roms, &hello_d64());
    m.type_program(SHUTTLE);
    let (start, typed) = m.program();
    assert_eq!(basic::tokenize_program(&SHUTTLE.join("\n"), start), typed, "shuttle");

    m.command("POKE 642,64:POKE 44,64:POKE 16384,0:NEW", 2 * SECOND);
    let lines = sprite_program();
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    m.type_program(&refs);
    let (start, typed) = m.program();
    assert_eq!(start, 0x4001);
    assert_eq!(basic::tokenize_program(&lines.join("\n"), start), typed, "sprites");
}

#[test]
fn directory_listing_and_load_run_hello() {
    let Some(roms) = load_roms("directory_listing_and_load_run_hello") else { return };
    let mut m = Machine::boot(&roms, &hello_d64());

    let dir = m.directory();
    assert!(dir.contains("0 \"1541 TEST DISK  \" C6 2A"), "directory header missing:\n{dir}");
    for entry in [
        "1    \"HELLO\"            PRG",
        "6    \"SHUTTLE\"          PRG",
        "1    \"SPRITE DEMO\"      PRG",
        "4    \"SPRITES 4001\"     PRG",
    ] {
        assert!(dir.contains(entry), "{entry:?} missing:\n{dir}");
    }
    assert!(dir.contains("652 BLOCKS FREE."), "free count missing:\n{dir}");

    m.command("LOAD\"HELLO\",8,1", 30 * SECOND);
    m.command("RUN", 5 * SECOND);
    assert!(m.screen().contains("1541 TEST DISK OK"), "HELLO did not run:\n{}", m.screen());
}

#[test]
fn demo_programs_on_the_test_disk_run() {
    let Some(roms) = load_roms("demo_programs_on_the_test_disk_run") else { return };
    // SHUTTLE switches to bitmap mode and starts drawing.
    let mut m = Machine::boot(&roms, &hello_d64());
    m.command("LOAD\"SHUTTLE\",8", 30 * SECOND);
    m.type_text("RUN\n");
    m.run(8 * SECOND);
    assert!(m.c64.read(0xD011) & 0x20 != 0, "SHUTTLE must have switched the VIC-II to bitmap mode");
    assert!(
        m.c64.peek_ram(0xC350) == 0x00 && m.c64.peek_ram(0xC351) == 165,
        "machine-code fill routine POKEd to 50000"
    );

    // SPRITE DEMO moves BASIC to $4001, loads SPRITES 4001 with ,8,1 and
    // RUNs it, all through the keyboard buffer.
    let mut m = Machine::boot(&roms, &hello_d64());
    m.command("LOAD\"SPRITE DEMO\",8", 30 * SECOND);
    m.type_text("RUN\n");
    m.run(25 * SECOND);
    let s = m.screen();
    assert!(!s.contains("ERROR"), "{s}");
    assert_eq!(m.peek16(0x2B), 0x4001, "BASIC must now start at $4001:\n{s}");
    assert_eq!(m.c64.read(0xD015), 3, "the sprite program must have enabled sprites 0 and 1:\n{s}");
}

#[test]
fn empty_disk_is_formatted_and_empty() {
    let Some(roms) = load_roms("empty_disk_is_formatted_and_empty") else { return };
    let empty = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-disks/empty.d64")).unwrap();
    let mut m = Machine::boot(&roms, &empty);
    let dir = m.directory();
    assert!(dir.contains("0 \"EMPTY DISK      \" 01 2A"), "{dir}");
    assert!(dir.contains("664 BLOCKS FREE."), "{dir}");
}

#[test]
fn format_save_load_shuttle_and_reload_from_exported_d64() {
    let Some(roms) = load_roms("format_save_load_shuttle_and_reload_from_exported_d64") else { return };
    // A never-formatted platter: all-zero D64 (every sector readable but
    // garbage BAM). `N:` with an ID does a full low-level format anyway.
    let mut m = Machine::boot(&roms, &vec![0u8; 174_848]);

    let t0 = m.total();
    m.command("OPEN15,8,15,\"N:SAVE TEST,T1\":CLOSE15", 200 * SECOND);
    let format_cycles = m.total() - t0;
    eprintln!("format took {format_cycles} C64 cycles (~{:.1} s emulated)", format_cycles as f64 / 985_248.0);

    let dir = m.directory();
    assert!(dir.contains("0 \"SAVE TEST       \" T1 2A"), "formatted disk header wrong:\n{dir}");
    assert!(dir.contains("664 BLOCKS FREE."), "freshly formatted disk must have 664 blocks free:\n{dir}");

    m.command("NEW", 2 * SECOND);
    m.type_program(SHUTTLE);
    let (start, typed) = m.program();
    assert_eq!(start, 0x0801);

    m.command("SAVE\"SHUTTLE\",8", 60 * SECOND);
    m.command("NEW", 2 * SECOND);
    m.command("LOAD\"SHUTTLE\",8", 60 * SECOND);
    let (start2, loaded) = m.program();
    assert_eq!(start2, 0x0801);
    assert_eq!(loaded, typed, "program loaded back from disk differs from the one saved");

    let blocks = (typed.len() + 2).div_ceil(254);
    let dir = m.directory();
    let entry = format!("{blocks:<4} \"SHUTTLE\"          PRG");
    assert!(dir.contains(&entry), "expected {entry:?} in directory:\n{dir}");
    assert!(dir.contains(&format!("{} BLOCKS FREE.", 664 - blocks)), "free count wrong:\n{dir}");

    // The disk image as it now exists in the drive, through to_d64 (GCR
    // decode of what the DOS actually wrote), into a brand-new machine.
    let d64 = m.c64.extract_disk(m.drive).unwrap();
    let mut fresh = Machine::boot(&roms, &d64);
    fresh.command("LOAD\"SHUTTLE\",8", 60 * SECOND);
    assert_eq!(fresh.program().1, typed, "program loaded from the exported D64 differs");
}

#[test]
fn save_sprite_program_from_relocated_basic_and_load_absolute_and_relocated() {
    let Some(roms) = load_roms("save_sprite_program_from_relocated_basic_and_load_absolute_and_relocated") else {
        return;
    };
    let mut m = Machine::boot(&roms, &hello_d64());

    // SpriteCollision.bas's own prerequisite: move BASIC to $4001.
    m.command("POKE 642,64:POKE 44,64:POKE 16384,0:NEW", 2 * SECOND);
    let lines = sprite_program();
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    m.type_program(&refs);
    let (start, typed) = m.program();
    assert_eq!(start, 0x4001, "BASIC must start at $4001 after the relocation POKEs");

    m.command("SAVE\"SPRITES\",8", 60 * SECOND);
    m.command("NEW", 2 * SECOND);
    m.command("LOAD\"SPRITES\",8,1", 60 * SECOND);
    let (start2, loaded) = m.program();
    assert_eq!(start2, 0x4001);
    assert_eq!(loaded, typed, "absolute (,8,1) load must restore the exact bytes at $4001");

    // Both files must now be on the disk alongside the fixture's HELLO.
    let dir = m.directory();
    assert!(dir.contains("\"HELLO\""), "{dir}");
    assert!(dir.contains("\"SPRITES\""), "{dir}");

    // A stock machine loads it relocated to $0801: same lines, relinked.
    let d64 = m.c64.extract_disk(m.drive).unwrap();
    if let Ok(path) = std::env::var("DUMP_D64") {
        std::fs::write(&path, &d64).unwrap();
        std::fs::write(format!("{path}.g64"), m.c64.drive(m.drive).unwrap().disk().unwrap().to_g64()).unwrap();
    }
    let mut fresh = Machine::boot(&roms, &d64);
    fresh.command("LOAD\"SPRITES\",8", 60 * SECOND);
    let (start3, relocated) = fresh.program();
    assert_eq!(start3, 0x0801);
    assert_eq!(
        lines_without_links(0x0801, &relocated),
        lines_without_links(0x4001, &typed),
        "relocated load must carry the same tokenized lines"
    );
    // And HELLO still loads and runs from the same disk.
    fresh.command("LOAD\"HELLO\",8", 30 * SECOND);
    fresh.command("RUN", 5 * SECOND);
    assert!(fresh.screen().contains("1541 TEST DISK OK"), "{}", fresh.screen());
}
