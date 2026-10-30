#!/usr/bin/env bash
# Build the dump_manager diagnostic binary if needed, run it, and print
# the output. Use this when reverse-engineering staff or human manager
# fields — it dumps the raw bytes of the human manager struct and runs
# search heuristics for personality / name pointer / role sub-record.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$(uname -s)" = "Linux" ]; then EXT=""; else EXT=".exe"; fi
BIN="$ROOT_DIR/target/debug/dump_manager$EXT"

if [ ! -x "$BIN" ] || [ "$ROOT_DIR/src/bin/dump_manager.rs" -nt "$BIN" ]; then
  echo "building dump_manager..."
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --bin dump_manager 2>&1 | tail -3
fi

"$BIN"
