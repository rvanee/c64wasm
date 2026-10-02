// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The page's settings, kept in localStorage per browser.

import { TUBE_DEFAULTS } from './display/crt-renderer.js';
import { MONITOR_DEFAULTS } from './display/monitor-2002.js';

const KEY = 'c64wasm-settings';

export const DEFAULT_SETTINGS = {
  video: 'pal',          // 'pal' | 'ntsc'
  speed: 'realtime',     // 'realtime' | 'fast'
  display: 'crt',        // 'crt' (the 2002 on the desk) | 'lcd' (a flat panel)
  connection: 'composite', // 'rf' | 'composite' | 'svideo'
  sid: '6581',           // '6581' | '8580'
  volume: 0.7,           // 0 = muted
  lastVolume: 0.7,       // restored when unmuting
  driveSounds: true,
  driveVolume: 0.6,
  driveOn: true,
  deviceNumber: 8,
  driveLcd: true,        // the add-on display on the 1541's front
  zoomed: false,
  tube: { ...TUBE_DEFAULTS },
  monitor: { ...MONITOR_DEFAULTS },
};

function load() {
  const settings = structuredClone(DEFAULT_SETTINGS);
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) || 'null');
    if (saved && typeof saved === 'object') {
      for (const key of Object.keys(DEFAULT_SETTINGS)) {
        const def = DEFAULT_SETTINGS[key], value = saved[key];
        if (typeof value !== typeof def || value === null) continue; // missing, or from an older version
        settings[key] = typeof def === 'object' ? { ...def, ...value } : value;
      }
    }
  } catch { /* private window or bad data: defaults */ }
  return settings;
}

/// The live settings object; change fields, then call save().
export const settings = load();

export function save() {
  try { localStorage.setItem(KEY, JSON.stringify(settings)); } catch { /* not persisted */ }
}
