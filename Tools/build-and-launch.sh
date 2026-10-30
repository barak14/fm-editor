#!/usr/bin/env bash
# Kill any running fm-editor instance for this branch, rebuild from source,
# launch the new binary in the background, and report PID. The standard
# "I changed something, show me the result" inner loop.
#
# In a feature worktree the binary is copied to fme-<branch>(.exe) so
# that multiple worktrees can run simultaneously without killing each other.
#
# Usage: bash Tools/build-and-launch.sh
#
# Exit codes:
#   0 = built and launched successfully
#   1 = build failed (no launch attempted)
#   2 = launched but didn't survive 1 second (likely panic on startup)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$(uname -s)" = "Linux" ]; then
  EXT=""
else
  EXT=".exe"
fi

# Derive per-branch exe name. On main we keep the canonical fm-editor;
# on any feature branch we use fme-<branch> (short prefix avoids the
# Windows tasklist 25-char / Linux comm 15-char name truncation that
# breaks process detection).
BRANCH=$(git -C "$ROOT_DIR" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
if [ "$BRANCH" = "main" ] || [ -z "$BRANCH" ]; then
  EXE_STEM="fm-editor"
else
  # Replace any slashes or underscores with hyphens for a clean filename.
  SAFE=$(echo "$BRANCH" | tr '/_' '-')
  EXE_STEM="fme-$SAFE"
fi
EXE_FILE="${EXE_STEM}${EXT}"

bash "$ROOT_DIR/Tools/kill-fm-editor.sh" "$EXE_FILE"

echo "building fm-editor..."
if ! cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --bin fm-editor 2>&1 | tail -5; then
  echo "BUILD FAILED" >&2
  exit 1
fi

# Copy canonical binary to the branch-specific name so kill/detect
# targets only this worktree's process.
if [ "$EXE_STEM" != "fm-editor" ]; then
  cp "$ROOT_DIR/target/debug/fm-editor$EXT" "$ROOT_DIR/target/debug/$EXE_FILE"
fi

echo "launching $EXE_FILE..."
# Pass --mcp-port if this worktree has a port assigned (written by create-worktree.sh).
MCP_PORT_ARG=""
if [ -f "$ROOT_DIR/.mcp-port" ]; then
  MCP_PORT=$(cat "$ROOT_DIR/.mcp-port")
  MCP_PORT_ARG="--mcp-port $MCP_PORT"
  echo "  MCP debug port: $MCP_PORT"
fi
# Pass --branch for non-main so the window title and status bar show which
# worktree is running.
BRANCH_ARG=""
if [ "$BRANCH" != "main" ] && [ -n "$BRANCH" ]; then
  BRANCH_ARG="--branch $BRANCH"
fi
"$ROOT_DIR/target/debug/$EXE_FILE" $MCP_PORT_ARG $BRANCH_ARG >"/tmp/${EXE_STEM}.log" 2>&1 &
LAUNCH_PID=$!
disown 2>/dev/null || true

# Give the GUI a moment to either start successfully or panic on first
# frame. We can't use sleep > 1s in Claude harness, so two short waits.
sleep 0.5
sleep 0.5

if [ "$(uname -s)" = "Linux" ]; then
  ALIVE=$(kill -0 "$LAUNCH_PID" 2>/dev/null && echo yes || echo no)
  PID="$LAUNCH_PID"
else
  # On Windows (git-bash) the shell's $! doesn't map to the Win32 PID, so
  # detect and report via tasklist instead.
  if tasklist //FI "IMAGENAME eq $EXE_FILE" 2>/dev/null | grep -qi "${EXE_STEM}"; then
    ALIVE=yes
    PID=$(tasklist //FI "IMAGENAME eq $EXE_FILE" 2>/dev/null | grep -i "${EXE_STEM}" | head -1 | awk '{print $2}')
  else
    ALIVE=no
  fi
fi

if [ "$ALIVE" != "yes" ]; then
  echo "LAUNCH FAILED — $EXE_FILE exited within 1 second" >&2
  echo "--- /tmp/${EXE_STEM}.log tail ---" >&2
  tail -20 "/tmp/${EXE_STEM}.log" >&2 || true
  exit 2
fi

echo "$EXE_FILE running as PID $PID"

# Write sentinel so rebase-worktree.sh can verify this worktree was actually
# launched and survived before it's allowed to merge into main.
touch "$ROOT_DIR/.launch-verified"
