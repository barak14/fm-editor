#!/usr/bin/env bash
# Kill any running fm-editor instance so the next `cargo build` can
# overwrite the binary. Safe to run when nothing is running — exits 0 in
# both cases.
#
# Usage: bash Tools/kill-fm-editor.sh [exe-name]
#   exe-name  — image name to kill (default: fm-editor.exe on Windows,
#               fm-editor on Linux; a .exe suffix is ignored on Linux)
#
# This script exists because every "rebuild and relaunch" cycle hits the
# Windows file lock on target/debug/fm-editor.exe if a previous instance
# is still running. Kill first, build second. (Linux has no such lock but
# killing the stale instance keeps the single-running-editor invariant.)
set -euo pipefail

if [ "$(uname -s)" = "Linux" ]; then
  STEM="${1:-fm-editor}"
  STEM="${STEM%.exe}"
  # comm is kernel-truncated to 15 chars; pkill -x matches comm exactly.
  COMM=$(printf '%.15s' "$STEM")
  if pkill -x "$COMM" 2>/dev/null; then
    echo "killed $STEM"
  else
    echo "no $STEM running"
  fi
  exit 0
fi

EXE_NAME="${1:-fm-editor.exe}"

# Tasklist quirk: it returns exit 0 even when nothing matches, but writes
# "INFO: No tasks are running..." to stdout. Filter it out.
RUNNING=$(tasklist //FI "IMAGENAME eq $EXE_NAME" 2>/dev/null | grep -i "${EXE_NAME%.exe}" || true)

if [ -z "$RUNNING" ]; then
  echo "no $EXE_NAME running"
  exit 0
fi

# Pull every PID and kill them all. There may be more than one if a
# previous launch left an orphan.
echo "$RUNNING" | awk '{print $2}' | while read -r pid; do
  if [ -n "$pid" ]; then
    echo "killing $EXE_NAME pid $pid"
    taskkill //PID "$pid" //F 2>&1 || true
  fi
done
