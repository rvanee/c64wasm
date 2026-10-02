@echo off
rem SPDX-License-Identifier: MIT OR Apache-2.0
rem Copyright (c) 2026 R.F. van Ee
rem Build the WebAssembly module into web\pkg\ (Windows): runs build_wasm.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_wasm.ps1"
