#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 R.F. van Ee
#
# Build the WebAssembly module and its JavaScript glue into web/pkg/.
# Needs rustup; installs the wasm32 target and the matching wasm-bindgen-cli.
set -euo pipefail
cd "$(dirname "$0")"

rustup target add wasm32-unknown-unknown

# wasm-bindgen-cli must be exactly the version of the wasm-bindgen crate.
version=$(grep -A1 'name = "wasm-bindgen"' Cargo.lock | grep version | head -1 | cut -d'"' -f2)
if ! command -v wasm-bindgen >/dev/null || [ "$(wasm-bindgen --version | awk '{print $2}')" != "$version" ]; then
  cargo install wasm-bindgen-cli --version "$version" --locked
fi

cargo build --release --target wasm32-unknown-unknown -p c64-wasm
rm -rf web/pkg
wasm-bindgen target/wasm32-unknown-unknown/release/c64_wasm.wasm --out-dir web/pkg --target web --no-typescript

echo "Done. Start the page with: python3 serve.py  (then open http://localhost:8000/)"
