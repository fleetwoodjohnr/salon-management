#!/usr/bin/env bash
# Cross-compile the Windows installer (NSIS setup.exe) on Linux, inside the srm-fedora toolbox.
# One-time setup in the toolbox:
#   sudo dnf install mingw32-nsis mingw64-nsis clang lld llvm nasm   (makensis needs both)
#   rustup target add x86_64-pc-windows-msvc && cargo install --locked cargo-xwin
# cargo-xwin downloads Microsoft's CRT and Windows SDK on first use (accepting Microsoft's license).
# Output: src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe
set -euo pipefail
cd "$(dirname "$0")/.."
exec scripts/tb npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis "$@"
