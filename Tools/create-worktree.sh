#!/usr/bin/env bash
# Create a feature worktree at .claude/worktrees/<name> on a new branch.
#
# Usage: bash Tools/create-worktree.sh <name>
# Example: bash Tools/create-worktree.sh club-reputation
#
# Exit codes:
#   0 = worktree created
#   1 = usage error / preconditions failed
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <name>" >&2
  echo "Example: $0 club-reputation" >&2
  exit 1
fi

NAME="$1"

# Validate name — git branches can't start with a dash, contain spaces,
# or have a slash (we'd want a subdirectory layout, which complicates
# the orphan-cleanup logic). Keep it simple.
if [[ "$NAME" =~ [[:space:]] ]] || [[ "$NAME" == -* ]] || [[ "$NAME" == */* ]]; then
  echo "Error: name '$NAME' is invalid. Use simple alphanumeric + dash only (no spaces, no slashes, no leading dash)." >&2
  exit 1
fi

# Find the MAIN worktree, regardless of where this script is invoked
# from. `git worktree list --porcelain` always lists the main worktree
# first; we take its path so this works whether the user runs the
# script from main or from inside another worktree.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_PATH="$(git -C "$SCRIPT_DIR" worktree list --porcelain 2>/dev/null | awk '/^worktree / {print $2; exit}')"
if [ -z "$PROJECT_PATH" ] || [ ! -d "$PROJECT_PATH" ]; then
  echo "Error: could not determine main worktree path from $SCRIPT_DIR" >&2
  exit 1
fi

WORKTREE_PATH="$PROJECT_PATH/.claude/worktrees/$NAME"

if [ -d "$WORKTREE_PATH" ]; then
  echo "Error: worktree already exists at $WORKTREE_PATH" >&2
  echo "  cd $WORKTREE_PATH                          # to use it" >&2
  echo "  bash Tools/rebase-worktree.sh $NAME        # to validate and rebase it" >&2
  echo "  bash Tools/ff-merge-worktree.sh $NAME      # then to merge it back" >&2
  exit 1
fi

if git -C "$PROJECT_PATH" rev-parse --verify "$NAME" >/dev/null 2>&1; then
  echo "Error: branch '$NAME' already exists." >&2
  echo "Pick a different name, or delete the branch first: git branch -d $NAME" >&2
  exit 1
fi

echo "=== Creating worktree: $NAME ==="

# Prune stale Git metadata only. Unregistered directories may contain valuable
# local work, so report them without changing their contents. Compare exact
# porcelain paths (with the glob's trailing slash removed).
git -C "$PROJECT_PATH" worktree prune 2>/dev/null || true
if [ -d "$PROJECT_PATH/.claude/worktrees" ]; then
  registered="$(git -C "$PROJECT_PATH" worktree list --porcelain 2>/dev/null || true)"
  for dir in "$PROJECT_PATH/.claude/worktrees"/*/; do
    [ -d "$dir" ] || continue
    if ! printf '%s\n' "$registered" | grep -qxF "worktree ${dir%/}"; then
      orphan=$(basename "$dir")
      echo "WARN: skipping unregistered worktree directory: $orphan ($dir)" >&2
    fi
  done
fi

# Create the worktree + branch
git -C "$PROJECT_PATH" worktree add "$WORKTREE_PATH" -b "$NAME"

# Assign a random MCP debug-server port for this worktree (range 7800-7899).
# Main always uses the default 7799, so worktrees never collide with it.
# The port is written to .mcp-port (gitignored) and patched into .mcp.json
# (working-tree only — never committed) and, when present, a local
# .rusty/mcp.toml, so an agent's fm-debug MCP client connects to the right
# port from the first session.
MCP_PORT=$((7800 + ($RANDOM % 100)))
echo "$MCP_PORT" > "$WORKTREE_PATH/.mcp-port"
for cfg in "$WORKTREE_PATH/.mcp.json" "$WORKTREE_PATH/.rusty/mcp.toml"; do
  if [ -f "$cfg" ]; then
    sed -i "s|http://localhost:[0-9][0-9]*/mcp|http://localhost:${MCP_PORT}/mcp|g" "$cfg"
  fi
done

cat <<EOF

=== Worktree ready ===
  Path:     $WORKTREE_PATH
  Branch:   $NAME
  MCP port: $MCP_PORT  (fm-debug MCP server; main uses 7799)

Next steps:
  cd $WORKTREE_PATH
  bash Tools/build-and-launch.sh         # rebuild + launch from this worktree
  bash Tools/rebase-worktree.sh $NAME    # when done: validate, build, rebase onto main
  bash Tools/ff-merge-worktree.sh $NAME  # then fast-forward main and clean up

Note: .mcp.json has been patched with port $MCP_PORT (local working-tree
change, not committed). A new agent session in this worktree connects to
its fm-debug server automatically. Do not commit it.
EOF
