#!/usr/bin/env bash
# Run the real-app end-to-end chain against the debug build on a fresh data folder.
# Needs Xvfb, tauri-driver (cargo install tauri-driver) and WebKitWebDriver on PATH.
# Usage: scripts/e2e-all.sh [data-dir]   (screenshots go to e2e/screenshots/)
set -euo pipefail
DIR="${1:-$(mktemp -d)/srm-e2e}"
rm -rf "$DIR" e2e/screenshots
for s in smoke slice2 slice3 slice4 slice5 slice6; do
  echo "== $s"
  node "e2e/$s.mjs" "$DIR"
done
echo "All E2E scripts passed (data in $DIR)"
