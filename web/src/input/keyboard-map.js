// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// PC keys and characters to the C64's keyboard matrix: [row, column], where
// the row is CIA1 port A and the column port B (see
// crates/c64-core/src/c64/keyboard.rs).

export const LEFT_SHIFT = [1, 7];

const CHARACTERS = {
  0: [4, 3], 1: [7, 0], 2: [7, 3], 3: [1, 0], 4: [1, 3], 5: [2, 0], 6: [2, 3], 7: [3, 0], 8: [3, 3], 9: [4, 0],
  A: [1, 2], B: [3, 4], C: [2, 4], D: [2, 2], E: [1, 6], F: [2, 5], G: [3, 2], H: [3, 5], I: [4, 1], J: [4, 2],
  K: [4, 5], L: [5, 2], M: [4, 4], N: [4, 7], O: [4, 6], P: [5, 1], Q: [7, 6], R: [2, 1], S: [1, 5], T: [2, 6],
  U: [3, 6], V: [3, 7], W: [1, 1], X: [2, 7], Y: [3, 1], Z: [1, 4],
  ' ': [7, 4], ':': [5, 5], ';': [6, 2], ',': [5, 7], '.': [5, 4], '=': [6, 5], '+': [5, 0], '-': [5, 3],
  '*': [6, 1], '/': [6, 7], '@': [5, 6], '^': [6, 6],
};

/// Characters typed with SHIFT on a C64 key.
const SHIFTED = {
  '(': [3, 3], ')': [4, 0], '"': [7, 3], '<': [5, 7], '>': [5, 4], '!': [7, 0], '#': [1, 0],
  '$': [1, 3], '%': [2, 0], '&': [2, 3], "'": [3, 0], '?': [6, 7], '_': [5, 3], '[': [5, 5], ']': [6, 2],
};

/// PC keys (KeyboardEvent.code) with no character of their own.
export const SPECIAL = {
  Enter: [[0, 1]], Backspace: [[0, 0]], Escape: [[7, 7]], Tab: [[7, 2]],
  Home: [[6, 3]], AltLeft: [[7, 5]], ControlLeft: [[7, 2]], ControlRight: [[7, 2]],
  Backquote: [[6, 0]], Insert: [[0, 0], LEFT_SHIFT], Delete: [[0, 0]],
  ArrowRight: [[0, 2]], ArrowLeft: [[0, 2], LEFT_SHIFT], ArrowDown: [[0, 7]], ArrowUp: [[0, 7], LEFT_SHIFT],
  F1: [[0, 4]], F2: [[0, 4], LEFT_SHIFT], F3: [[0, 5]], F4: [[0, 5], LEFT_SHIFT],
  F5: [[0, 6]], F6: [[0, 6], LEFT_SHIFT], F7: [[0, 3]], F8: [[0, 3], LEFT_SHIFT],
};

/// The PC key that works as RESTORE (it isn't in the matrix).
export const RESTORE_KEY = 'PageUp';

/// The matrix cells for one character; `shift` adds SHIFT to a letter.
export function cellsForCharacter(ch, shift = false) {
  if (ch === '\n') return SPECIAL.Enter;
  const upper = ch.toUpperCase();
  if (/^[A-Z]$/.test(upper)) return shift ? [CHARACTERS[upper], LEFT_SHIFT] : [CHARACTERS[upper]];
  if (CHARACTERS[ch]) return [CHARACTERS[ch]];
  if (SHIFTED[ch]) return [SHIFTED[ch], LEFT_SHIFT];
  return null;
}

// Keyboard layouts with dead keys (US-International, many European
// layouts) report the quote key as key "Dead" and only deliver '"' with
// the next key press (space, or a composed letter like "ë"). A C64 has no
// dead keys, so the character is taken from the physical key at once, and
// the composed key press that follows is mapped back to its own key.
const DEAD_KEYS = {
  Quote: ["'", '"'], Backquote: ['`', '~'], Digit6: ['6', '^'], BracketLeft: ['[', '{'], Equal: ['=', '+'], Digit2: ['2', '@'],
};

/// Turns key events into matrix cells, remembering dead keys.
export class KeyMapper {
  constructor() { this.afterDead = null; }

  /// The cells for a keydown, [] to swallow it, or null if it isn't a C64 key.
  cells(e) {
    if (e.key === 'Dead') {
      const pair = DEAD_KEYS[e.code];
      if (!pair) return null;
      this.afterDead = pair[e.shiftKey ? 1 : 0];
      return cellsForCharacter(this.afterDead);
    }
    const dead = this.afterDead;
    this.afterDead = null;
    // "dead key + space" composes the dead character again, already typed.
    if (dead && e.code === 'Space' && e.key === dead) return [];
    if (SPECIAL[e.code]) return SPECIAL[e.code];
    if (e.key.length === 1) {
      const cells = cellsForCharacter(e.key, e.shiftKey);
      if (cells) return cells;
      // A composed/accented character ("ë" after a dead '"'): use the plain key.
      const m = /^Key([A-Z])$/.exec(e.code) || /^Digit([0-9])$/.exec(e.code);
      if (m) return cellsForCharacter(m[1], e.shiftKey && /^Key/.test(e.code));
    }
    return null;
  }
}
