#!/usr/bin/env bash
# Build the scan_arrays diagnostic binary if needed, run it, and filter
# the output to the meaningful matches (count > 1).
#
# Use this when an array RVA needs verifying or you suspect the count is
# off compared to a reference tool (FMLE/FMRTE). The binary scans the
# .data region of game_plugin.dll for (data, end) std::vector pointer
# pairs whose count matches a target.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$(uname -s)" = "Linux" ]; then EXT=""; else EXT=".exe"; fi
BIN="$ROOT_DIR/target/debug/scan_arrays$EXT"

if [ ! -x "$BIN" ] || [ "$ROOT_DIR/src/bin/scan_arrays.rs" -nt "$BIN" ]; then
  echo "building scan_arrays..."
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --bin scan_arrays 2>&1 | tail -3
fi

# Drop the count=1 noise (vector slots holding a single element). Anything
# in [10, 1_000_000] is potentially interesting.
"$BIN" 2>&1 | grep -vE 'count=[[:space:]]*1[[:space:]]'
