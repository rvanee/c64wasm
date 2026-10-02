// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The Commodore ROMs: which ones are needed (slots), where they are kept
// (store), and how they are rebuilt from listings and pictures found on the
// web (listing, chargen, repair, sources).

export { SLOTS, identify, isKnown } from './slots.js';
export { loadLibrary, addRom, useRom, removeRom } from './store.js';
export { chargenFromImages, imageDataFrom } from './chargen.js';
export { parseListing } from './listing.js';
export { buildImage } from './repair.js';
export {
  AUTO_SOURCES, BlockedByBrowser,
  alsoFor, autoFind, fromBytesOrText, fromText, fromUrl,
} from './sources.js';
