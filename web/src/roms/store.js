// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// The ROM library, kept in the browser's IndexedDB and never sent anywhere:
// per slot, every ROM image the user has added, and which one is in use.

import { crc32 } from './crc32.js';
import { SLOTS } from './slots.js';

const DB_NAME = 'c64wasm-roms', STORE = 'roms';

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function transaction(mode, fn) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const t = db.transaction(STORE, mode);
    const r = fn(t.objectStore(STORE));
    t.oncomplete = () => resolve(r && 'result' in r ? r.result : undefined);
    t.onerror = () => reject(t.error);
  });
}

const get = (key) => transaction('readonly', s => s.get(key)).then(v => v ?? null);
const put = (key, v) => transaction('readwrite', s => s.put(v, key));
const remove = (key) => transaction('readwrite', s => s.delete(key));

/// One slot's shelf: { entries: [{ id, bytes, source, added }], active: id }.
const emptyShelf = () => ({ entries: [], active: null });
const shelfKey = (slot) => `library:${slot}`;

/// Every slot's shelf. Single ROMs stored by earlier versions of the page
/// are moved onto the shelves.
export async function loadLibrary() {
  const library = {};
  for (const [slot, s] of Object.entries(SLOTS)) {
    let shelf = await get(shelfKey(slot)) || emptyShelf();
    const old = await get(s.db);
    if (old) {
      shelf = addTo(shelf, old, (await get(s.db + ':source')) || 'stored earlier');
      await put(shelfKey(slot), shelf);
      await remove(s.db);
      await remove(s.db + ':source');
    }
    library[slot] = shelf;
  }
  return library;
}

/// The shelf with `bytes` added (or found, if the same image is already
/// there) and made the one in use.
function addTo(shelf, bytes, source) {
  const id = crc32(bytes);
  const entries = shelf.entries.some(e => e.id === id)
    ? shelf.entries
    : [...shelf.entries, { id, bytes, source, added: new Date().toISOString() }];
  return { entries, active: id };
}

/// Add a ROM to a slot's shelf and use it. Returns the new shelf.
export async function addRom(slot, bytes, source) {
  const shelf = addTo(await get(shelfKey(slot)) || emptyShelf(), bytes, source);
  await put(shelfKey(slot), shelf);
  return shelf;
}

/// Use another ROM from the shelf. Returns the new shelf.
export async function useRom(slot, id) {
  const shelf = await get(shelfKey(slot)) || emptyShelf();
  if (shelf.entries.some(e => e.id === id)) shelf.active = id;
  await put(shelfKey(slot), shelf);
  return shelf;
}

/// Take a ROM off the shelf; if it was in use, the most recent other one
/// takes its place. Returns the new shelf.
export async function removeRom(slot, id) {
  const shelf = await get(shelfKey(slot)) || emptyShelf();
  shelf.entries = shelf.entries.filter(e => e.id !== id);
  if (shelf.active === id) shelf.active = shelf.entries.length ? shelf.entries[shelf.entries.length - 1].id : null;
  await put(shelfKey(slot), shelf);
  return shelf;
}
