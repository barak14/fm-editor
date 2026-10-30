#!/usr/bin/env bash
# Validate, build-gate, and rebase a worktree's branch onto main.
# Stops BEFORE the fast-forward merge — run ff-merge-worktree.sh
# separately once you've reviewed the commits that will land.
#
# Usage: bash Tools/rebase-worktree.sh <name>
# Example: bash Tools/rebase-worktree.sh club-reputation
#
# Exit codes:
#   0 = rebased and built successfully — ready for ff-merge-worktree.sh
#   1 = build failed (worktree intact, branch unchanged)
#   2 = rebase conflicts (worktree intact, branch in mid-rebase state) —
#       caller should triage conflicts and re-invoke this script
#   3 = usage / preconditions failed
#   4 = post-rebase build failed (branch in rebased state)
#
# NOTE: `set -e` is deliberately omitted — every error path is handled
# explicitly so the worktree is never silently destroyed on failure.
set -uo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <name>" >&2
  echo "Example: $0 club-reputation" >&2
  exit 3
fi

NAME="$1"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_PATH="$(git -C "$SCRIPT_DIR" worktree list --porcelain 2>/dev/null | awk '/^worktree / {print $2; exit}')"
if [ -z "$PROJECT_PATH" ] || [ ! -d "$PROJECT_PATH" ]; then
  echo "Error: could not determine main worktree path from $SCRIPT_DIR" >&2
  exit 3
fi

WORKTREE_PATH="$PROJECT_PATH/.claude/worktrees/$NAME"

if [ ! -d "$WORKTREE_PATH" ]; then
  echo "Error: worktree not found at $WORKTREE_PATH" >&2
  exit 3
fi

BRANCH=$(git -C "$WORKTREE_PATH" rev-parse --abbrev-ref HEAD)

cat <<EOF
=== Rebasing worktree: $NAME ===
  Worktree: $WORKTREE_PATH
  Branch:   $BRANCH
  Target:   main

EOF

# Step 1: Refuse if worktree is mid-rebase (left over from previous
# attempt that didn't get cleaned up).
if [ -d "$WORKTREE_PATH/.git" ]; then
  REBASE_DIR_BASE="$WORKTREE_PATH/.git"
else
  REBASE_DIR_BASE=$(git -C "$WORKTREE_PATH" rev-parse --git-dir)
fi
if [ -d "$REBASE_DIR_BASE/rebase-merge" ] || [ -d "$REBASE_DIR_BASE/rebase-apply" ]; then
  echo "Error: worktree is in the middle of a rebase." >&2
  echo "Finish or abort it first:" >&2
  echo "  git -C $WORKTREE_PATH status" >&2
  echo "  git -C $WORKTREE_PATH rebase --continue  # if conflicts are resolved" >&2
  echo "  git -C $WORKTREE_PATH rebase --abort      # to bail out entirely" >&2
  exit 3
fi

# Step 2: Refuse if worktree has uncommitted changes.
# Exception: .mcp.json is patched with a per-worktree port by
# create-worktree.sh and is intentionally never committed. Restore it
# silently so it doesn't block.
DIRTY="$(git -C "$WORKTREE_PATH" status --porcelain)"
if [ -n "$DIRTY" ]; then
  DIRTY_OTHER="$(echo "$DIRTY" | grep -v -e '^ M \.mcp\.json$' || true)"
  if [ -n "$DIRTY_OTHER" ]; then
    echo "Error: worktree has uncommitted changes:" >&2
    git -C "$WORKTREE_PATH" status --short >&2
    echo "" >&2
    echo "Commit or stash before merging." >&2
    exit 3
  fi
  git -C "$WORKTREE_PATH" checkout -- .mcp.json
fi

# Step 3: Launch-verification gate — build-and-launch.sh must have been run
# in this worktree and the binary must have survived (sentinel written on exit 0).
if [ ! -f "$WORKTREE_PATH/.launch-verified" ]; then
  echo "Error: worktree has not been launch-tested." >&2
  echo "" >&2
  echo "Run build-and-launch.sh from the worktree before merging:" >&2
  echo "  bash \"$WORKTREE_PATH/Tools/build-and-launch.sh\"" >&2
  echo "" >&2
  echo "This confirms the GUI builds, launches, and survives 1 second." >&2
  exit 3
fi

# Step 4: Build gate — `cargo build` in the worktree.
echo "Running cargo build in worktree..."
if ! cargo build --manifest-path "$WORKTREE_PATH/Cargo.toml" --bin fm-editor 2>&1 | tail -5; then
  echo "" >&2
  echo "=== BUILD FAILED — aborting ===" >&2
  echo "Fix the build in $WORKTREE_PATH and re-run this script." >&2
  exit 1
fi
echo "  ok"
echo ""

# Step 5: Attempt the rebase from inside the worktree.
echo "Rebasing $BRANCH onto main..."
if ! git -C "$WORKTREE_PATH" rebase main; then
  CONFLICTS=$(git -C "$WORKTREE_PATH" diff --name-only --diff-filter=U 2>/dev/null || true)
  echo "" >&2
  echo "=== REBASE CONFLICTS ===" >&2
  echo "" >&2
  echo "Files in conflict:" >&2
  if [ -n "$CONFLICTS" ]; then
    echo "$CONFLICTS" | sed 's/^/  /' >&2
  else
    echo "  (rebase failed but no conflict files reported — check git status)" >&2
  fi
  echo "" >&2
  echo "Worktree is at: $WORKTREE_PATH" >&2
  echo "Branch '$BRANCH' is in mid-rebase state." >&2
  echo "" >&2
  echo "To resolve manually:" >&2
  echo "  git -C $WORKTREE_PATH status" >&2
  echo "  # edit conflicted files, then" >&2
  echo "  git -C $WORKTREE_PATH add <files>" >&2
  echo "  git -C $WORKTREE_PATH rebase --continue" >&2
  echo "  bash Tools/rebase-worktree.sh $NAME" >&2
  echo "" >&2
  echo "To abort and bail out:" >&2
  echo "  git -C $WORKTREE_PATH rebase --abort" >&2
  exit 2
fi
echo "  ok"
echo ""

# Step 6: Re-run build after rebase. The rebase replayed the branch's
# commits on top of the new base — they could compile against themselves
# but break against main if (e.g.) main renamed a function the branch
# was calling.
echo "Re-running cargo build after rebase..."
if ! cargo build --manifest-path "$WORKTREE_PATH/Cargo.toml" --bin fm-editor 2>&1 | tail -5; then
  echo "" >&2
  echo "=== POST-REBASE BUILD FAILED ===" >&2
  echo "" >&2
  echo "The rebased branch '$BRANCH' compiles against itself but breaks against main." >&2
  echo "Worktree is at: $WORKTREE_PATH" >&2
  echo "Branch is on the rebased state — fix the build, commit, and re-run this script." >&2
  exit 4
fi
echo "  ok"
echo ""

# Show what is about to land in main so the caller can review before committing.
COMMIT_COUNT=$(git -C "$WORKTREE_PATH" rev-list --count main..HEAD 2>/dev/null || echo "?")
cat <<EOF
=== Rebase complete: $COMMIT_COUNT commit(s) ready to merge ===

Commits that will land in main:
EOF
git -C "$WORKTREE_PATH" log --oneline main..HEAD
echo ""
echo "Review the commits above, then run:"
echo "  bash Tools/ff-merge-worktree.sh $NAME"
