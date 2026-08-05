#!/usr/bin/env bash
set -euo pipefail
command -v rustc >/dev/null || { echo 'rustc no está disponible'; exit 2; }
command -v wasmtime >/dev/null || { echo 'wasmtime no está disponible'; exit 2; }
rustc --target wasm32-wasip1 hello.rs -o hello.wasm
wasmtime run hello.wasm
