// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! Commodore BASIC V2 tokenizer: turns a plain-text listing into the
//! in-memory/PRG form the C64's own screen editor produces when the same
//! lines are typed in (`CRUNCH` at `$A579` in the BASIC ROM), so disk images
//! with ready-to-`LOAD` programs can be built without running the machine.
//!
//! Rules, as the ROM applies them:
//! - Outside quotes, at each position the keyword table is tried in token
//!   order ($80 `END` ... $CB `GO`) and the first keyword whose letters
//!   match exactly is replaced by its token. Spaces are not skipped inside
//!   a keyword, but are kept in the line as typed. `?` is `PRINT`.
//! - Digits, `:` and `;` are never the start of a keyword.
//! - Nothing is tokenized inside quotes, after `REM`, or after `DATA`
//!   until the next `:` outside quotes.
//! - Line numbers are stored as 16-bit binary; every line is preceded by a
//!   2-byte link to the next line and ends in `$00`; the program ends with
//!   a `$0000` link.
//!
//! Input is plain ASCII with upper-case letters (= unshifted PETSCII);
//! lower-case letters are upper-cased, which is how they arrive when typed.

/// The BASIC V2 keyword table, token $80 first.
pub const KEYWORDS: [&str; 76] = [
    "END", "FOR", "NEXT", "DATA", "INPUT#", "INPUT", "DIM", "READ", "LET", "GOTO", "RUN", "IF", "RESTORE", "GOSUB",
    "RETURN", "REM", "STOP", "ON", "WAIT", "LOAD", "SAVE", "VERIFY", "DEF", "POKE", "PRINT#", "PRINT", "CONT", "LIST",
    "CLR", "CMD", "SYS", "OPEN", "CLOSE", "GET", "NEW", "TAB(", "TO", "FN", "SPC(", "THEN", "NOT", "STEP", "+", "-",
    "*", "/", "^", "AND", "OR", ">", "=", "<", "SGN", "INT", "ABS", "USR", "FRE", "POS", "SQR", "RND", "LOG", "EXP",
    "COS", "SIN", "TAN", "ATN", "PEEK", "LEN", "STR$", "VAL", "ASC", "CHR$", "LEFT$", "RIGHT$", "MID$", "GO",
];
const TOKEN_REM: u8 = 0x8F;
const TOKEN_DATA: u8 = 0x83;
const TOKEN_PRINT: u8 = 0x99;

/// Tokenize the text of one line (everything after the line number).
pub fn tokenize_line(text: &str) -> Vec<u8> {
    let src: Vec<u8> = text.bytes().map(|b| b.to_ascii_uppercase()).collect();
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    let mut in_quotes = false;
    let mut in_data = false;
    while i < src.len() {
        let c = src[i];
        if c == b'"' {
            in_quotes = !in_quotes;
            out.push(c);
            i += 1;
            continue;
        }
        if in_quotes {
            out.push(c);
            i += 1;
            continue;
        }
        if c == b':' {
            in_data = false;
        }
        if in_data || c == b' ' || (b'0'..=b';').contains(&c) {
            out.push(c);
            i += 1;
            continue;
        }
        if c == b'?' {
            out.push(TOKEN_PRINT);
            i += 1;
            continue;
        }
        let matched = KEYWORDS.iter().enumerate().find(|(_, kw)| src[i..].starts_with(kw.as_bytes()));
        if let Some((n, kw)) = matched {
            let token = 0x80 + n as u8;
            out.push(token);
            i += kw.len();
            if token == TOKEN_REM {
                out.extend_from_slice(&src[i..]);
                break;
            }
            if token == TOKEN_DATA {
                in_data = true;
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Tokenize a whole listing (lines like `10 PRINT "HI"`; blank lines and
/// lines without a leading line number are ignored) into the program
/// bytes as they sit in memory starting at `load_address`, links included.
/// Lines are sorted by number and later duplicates replace earlier ones,
/// as when typed in.
pub fn tokenize_program(listing: &str, load_address: u16) -> Vec<u8> {
    let mut lines: std::collections::BTreeMap<u16, Vec<u8>> = std::collections::BTreeMap::new();
    for raw in listing.lines() {
        let t = raw.trim_start();
        let digits = t.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 {
            continue;
        }
        let Ok(num) = t[..digits].parse::<u16>() else { continue };
        // The editor skips spaces between the number and the statement.
        let body = t[digits..].trim_start();
        let tokens = tokenize_line(body.trim_end());
        if tokens.is_empty() {
            lines.remove(&num);
        } else {
            lines.insert(num, tokens);
        }
    }
    let mut out = Vec::new();
    let mut addr = load_address;
    for (num, body) in &lines {
        let next = addr + 2 + 2 + body.len() as u16 + 1;
        out.extend_from_slice(&next.to_le_bytes());
        out.extend_from_slice(&num.to_le_bytes());
        out.extend_from_slice(body);
        out.push(0);
        addr = next;
    }
    out.extend_from_slice(&[0, 0]);
    out
}

/// As `tokenize_program`, as a PRG file: the 2-byte load address first.
pub fn prg(listing: &str, load_address: u16) -> Vec<u8> {
    let mut v = load_address.to_le_bytes().to_vec();
    v.extend(tokenize_program(listing, load_address));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_and_literals() {
        assert_eq!(tokenize_line("PRINT \"HELLO\""), [0x99, b' ', b'"', b'H', b'E', b'L', b'L', b'O', b'"']);
        assert_eq!(tokenize_line("?X"), [0x99, b'X']);
        assert_eq!(tokenize_line("FORI=1TO10"), [0x81, b'I', 0xB2, b'1', 0xA4, b'1', b'0']);
        // DATA keeps text literal until ':' ...
        assert_eq!(tokenize_line("DATA TO,AND:END"), [0x83, b' ', b'T', b'O', b',', b'A', b'N', b'D', b':', 0x80]);
        // ... REM to the end of the line.
        assert_eq!(tokenize_line("REM PRINT"), [0x8F, b' ', b'P', b'R', b'I', b'N', b'T']);
        // First match in table order: INPUT# before INPUT, PRINT# before PRINT.
        assert_eq!(tokenize_line("INPUT#1"), [0x84, b'1']);
        // Keywords inside variable names are tokenized too, as on the real machine.
        assert_eq!(tokenize_line("STOP"), [0x90]);
        assert_eq!(tokenize_line("ATN(1)"), [0xC1, b'(', b'1', b')']);
    }

    #[test]
    fn program_links_and_end_marker() {
        let p = tokenize_program("10 PRINT\n20 END\n", 0x0801);
        // 0801: link 0807, line 10, PRINT, 00 ; 0807: link 080D, 20, END, 00 ; 0000
        assert_eq!(p, [0x07, 0x08, 10, 0, 0x99, 0, 0x0D, 0x08, 20, 0, 0x80, 0, 0, 0]);
    }
}
