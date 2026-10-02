// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

//! The keyboard: an 8x8 switch matrix between CIA1's port A (rows) and
//! port B (columns). The KERNAL drives one row low at a time on port A
//! and reads the columns on port B; a pressed key connects its row and
//! column, so the column reads low. It works the other way round too
//! (columns driven, rows read), which some programs use.
//!
//! RESTORE is not in the matrix; it triggers an NMI (see `Board`).
//!
//! Row 0: DEL RETURN CRSR→ F7 F1 F3 F5 CRSR↓;
//! row 1: 3 W A 4 Z S E LSHIFT; row 2: 5 R D 6 C F T X;
//! row 3: 7 Y G 8 B H U V; row 4: 9 I J 0 M K O N;
//! row 5: + P L - . : @ ,; row 6: £ * ; HOME RSHIFT = ↑ /;
//! row 7: 1 ← CTRL 2 SPACE C= Q RUN/STOP (columns 0-7).

#[derive(Debug, Clone, Copy, Default)]
pub struct Keyboard {
    /// Per row, a bit per column: 1 = pressed.
    pressed: [u8; 8],
}

impl Keyboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_key(&mut self, row: usize, col: usize, pressed: bool) {
        if pressed {
            self.pressed[row] |= 1 << col;
        } else {
            self.pressed[row] &= !(1 << col);
        }
    }

    pub fn release_all(&mut self) {
        self.pressed = [0; 8];
    }

    /// Column levels (port B input) with the rows in `rows_low` driven low.
    pub fn columns(&self, rows_low: u8) -> u8 {
        (0..8).filter(|r| rows_low & (1 << r) != 0).fold(0xFF, |cols, r| cols & !self.pressed[r])
    }

    /// Row levels (port A input) with the columns in `columns_low` driven
    /// low.
    pub fn rows(&self, columns_low: u8) -> u8 {
        let mut rows = 0xFF;
        for (r, &keys) in self.pressed.iter().enumerate() {
            if keys & columns_low != 0 {
                rows &= !(1 << r);
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pressed_key_pulls_its_column_when_its_row_is_scanned() {
        let mut k = Keyboard::new();
        k.set_key(1, 2, true); // A
        assert_eq!(k.columns(0b0000_0010), !0b0000_0100);
        assert_eq!(k.columns(0b0000_0001), 0xFF, "another row");
        assert_eq!(k.columns(0xFF), !0b0000_0100, "all rows at once");
        assert_eq!(k.rows(0b0000_0100), !0b0000_0010, "reverse scan");
        k.set_key(1, 2, false);
        assert_eq!(k.columns(0xFF), 0xFF);
    }

    #[test]
    fn release_all() {
        let mut k = Keyboard::new();
        k.set_key(7, 7, true);
        k.set_key(0, 0, true);
        k.release_all();
        assert_eq!(k.columns(0xFF), 0xFF);
    }
}
