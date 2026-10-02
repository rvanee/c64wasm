# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 R.F. van Ee
#
# Build the WebAssembly module and its JavaScript glue into web\pkg\ (Windows).
# Needs rustup; installs the wasm32 target and the matching wasm-bindgen-cli.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

function Run($exe, [string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { throw "$exe failed" }
}

Run rustup @('target', 'add', 'wasm32-unknown-unknown')

# wasm-bindgen-cli must be exactly the version of the wasm-bindgen crate.
$lock = Get-Content Cargo.lock
$i = [Array]::IndexOf($lock, 'name = "wasm-bindgen"')
$version = ($lock[$i + 1] -split '"')[1]
$installed = try { (wasm-bindgen --version 2>$null) -join '' } catch { '' }
if ($installed -notlike "*$version*") {
    Run cargo @('install', 'wasm-bindgen-cli', '--version', $version, '--locked')
}

Run cargo @('build', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'c64-wasm')
if (Test-Path web\pkg) { Remove-Item -Recurse -Force web\pkg }
Run wasm-bindgen @('target\wasm32-unknown-unknown\release\c64_wasm.wasm', '--out-dir', 'web\pkg', '--target', 'web', '--no-typescript')

Write-Host 'Done. Start the page with: python serve.py  (then open http://localhost:8000/)'
