// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The PC keyboard on the emulated one, while the picture has focus.

import { KeyMapper, RESTORE_KEY } from './keyboard-map.js';

/// `target` is the focusable element; `machine()` returns the running
/// machine or null.
export function attachKeyboard(target, machine) {
  const mapper = new KeyMapper();
  const held = new Map(); // KeyboardEvent.code -> cells

  window.addEventListener('keydown', (e) => {
    const m = machine();
    if (!m || document.activeElement !== target) return;
    if (e.code === RESTORE_KEY) { e.preventDefault(); if (!e.repeat) m.press_restore(); return; }
    if (held.has(e.code)) { e.preventDefault(); return; }
    const cells = mapper.cells(e);
    if (!cells) return;
    e.preventDefault();
    if (!cells.length) return; // a swallowed dead-key composition
    for (const [r, c] of cells) m.set_key(r, c, true);
    held.set(e.code, cells);
  });

  window.addEventListener('keyup', (e) => {
    const cells = held.get(e.code);
    const m = machine();
    if (!cells || !m) return;
    e.preventDefault();
    for (const [r, c] of cells) m.set_key(r, c, false);
    held.delete(e.code);
  });

  // A key released while the window had no focus never sends keyup.
  window.addEventListener('blur', () => {
    machine()?.release_all_keys();
    held.clear();
  });
}
