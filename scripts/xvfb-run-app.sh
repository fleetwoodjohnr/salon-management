#!/usr/bin/env bash
# Run the built app on a private virtual X display (for screenshots and E2E on any desktop,
# including Wayland). Usage: scripts/xvfb-run-app.sh <data-dir> [display-number]
# Then: DISPLAY=:<n> xwd -root -silent | magick xwd:- shot.png
set -euo pipefail
DATA="${1:?data dir}"
N="${2:-99}"
BIN="${SRM_BIN:-$(dirname "$0")/../src-tauri/target/debug/salon-resource-manager}"
Xvfb ":$N" -screen 0 1440x900x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
trap 'kill $XPID 2>/dev/null || true' EXIT
sleep 1
DISPLAY=":$N" GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 SRM_DATA_DIR="$DATA" "$BIN"
