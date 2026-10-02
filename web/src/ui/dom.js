// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 R.F. van Ee

// Small DOM helpers shared by the page's modules.

export const $ = (id) => document.getElementById(id);

const messageEl = () => $('message');
let messageTimer = 0;

/// A short message over the picture.
export function flash(text, ms = 3200) {
  const el = messageEl();
  el.textContent = text;
  el.classList.add('show');
  clearTimeout(messageTimer);
  messageTimer = setTimeout(() => el.classList.remove('show'), ms);
}

/// The contents of a picked or dropped file.
export const readFile = (file) => (file ? file.arrayBuffer().then(b => new Uint8Array(b)) : Promise.resolve(null));

/// Offer bytes as a download.
export function download(bytes, name) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([bytes], { type: 'application/octet-stream' }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 5000);
}

/// Wait for the browser to paint (so a "working..." message shows before a
/// long computation).
export const nextPaint = () => new Promise(r => setTimeout(r, 30));

/// Wire a hidden <input type=file> to a button; `onFile(file)` gets the pick.
export function filePicker(button, input, onFile) {
  button.addEventListener('click', () => input.click());
  input.addEventListener('change', async () => {
    const f = input.files[0];
    input.value = '';
    if (f) await onFile(f);
  });
}
