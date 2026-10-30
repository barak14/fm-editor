#!/usr/bin/env bash
# Fast-forward merge a rebased worktree branch into main, then clean up.
# Prerequisite: rebase-worktree.sh must have completed successfully (exit 0).
#
# Usage: bash Tools/ff-merge-worktree.sh <name>
# Example: bash Tools/ff-merge-worktree.sh club-reputation
#
# Exit codes:
#   0 = merged successfully, rebuilt on main, worktree cleaned up. Done.
#   3 = usage / preconditions failed (worktree not found, mid-rebase, etc.)
#   4 = fast-forward failed (main moved since the rebase — re-run
#       rebase-worktree.sh to rebase again, then re-run this script)
#   5 = post-merge build failed (merge landed but binary is broken)
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

# Refuse if worktree is in a mid-rebase state — rebase-worktree.sh must
# have completed cleanly before we can ff-merge.
if [ -d "$WORKTREE_PATH/.git" ]; then
  REBASE_DIR_BASE="$WORKTREE_PATH/.git"
else
  REBASE_DIR_BASE=$(git -C "$WORKTREE_PATH" rev-parse --git-dir)
fi
if [ -d "$REBASE_DIR_BASE/rebase-merge" ] || [ -d "$REBASE_DIR_BASE/rebase-apply" ]; then
  echo "Error: worktree is in the middle of a rebase." >&2
  echo "Run rebase-worktree.sh first to complete the rebase." >&2
  exit 3
fi

# Refuse if worktree has uncommitted changes.
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

cat <<EOF
=== Merging worktree: $NAME ===
  Worktree: $WORKTREE_PATH
  Branch:   $BRANCH
  Target:   main

EOF

# Show what is about to land.
COMMIT_COUNT=$(git -C "$WORKTREE_PATH" rev-list --count main..HEAD 2>/dev/null || echo "?")
echo "Commits about to land in main ($COMMIT_COUNT):"
git -C "$WORKTREE_PATH" log --oneline main..HEAD
echo ""

# Step 1: Switch the main worktree to the main branch if it isn't there.
CURRENT_BRANCH=$(git -C "$PROJECT_PATH" rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
  echo "Main worktree is on '$CURRENT_BRANCH', switching to main..."
  if ! git -C "$PROJECT_PATH" checkout main; then
    echo "" >&2
    echo "=== CHECKOUT FAILED ===" >&2
    echo "Could not switch main worktree to main. Worktree is safe at: $WORKTREE_PATH" >&2
    exit 4
  fi
fi

# Step 2: Fast-forward merge. We just rebased so the branch is a strict
# descendant of main — this MUST succeed unless main moved between
# rebase-worktree.sh and now (race condition).
echo "Fast-forward merging $BRANCH into main..."
if ! git -C "$PROJECT_PATH" merge --ff-only "$BRANCH"; then
  echo "" >&2
  echo "=== FAST-FORWARD FAILED ===" >&2
  echo "" >&2
  echo "Main moved between the rebase and the merge (race condition)." >&2
  echo "Worktree is safe at: $WORKTREE_PATH" >&2
  echo "Re-run rebase-worktree.sh $NAME to rebase again, then re-run this script." >&2
  exit 4
fi
echo "  ok"
echo ""

# Step 3: Rebuild on main so the binary matches the merged code.
echo "Building on main..."
if ! cargo build --manifest-path "$PROJECT_PATH/Cargo.toml" 2>&1; then
  echo "" >&2
  echo "=== POST-MERGE BUILD FAILED ===" >&2
  echo "The merge landed but the build on main is broken." >&2
  echo "Fix it before doing anything else." >&2
  exit 5
fi
echo "  ok"
echo ""

# Step 4: Cleanup — remove worktree, delete branch.
#
# On Windows, the worktree directory often has a transient file lock from
# whatever was holding files open in it (rust-analyzer indexing, the
# previous shell session's cwd, antivirus scanning the new files). The
# first `git worktree remove` can fail with "Permission denied" while the
# second one (a second later) succeeds. Retry once before falling back to
# a direct rm -rf.
echo "Cleaning up..."
remove_ok=false
if git -C "$PROJECT_PATH" worktree remove "$WORKTREE_PATH" 2>/dev/null; then
  remove_ok=true
else
  # Wait a moment for any transient lock to release, then retry.
  sleep 1
  if git -C "$PROJECT_PATH" worktree remove "$WORKTREE_PATH" 2>/dev/null; then
    remove_ok=true
  elif [ -d "$WORKTREE_PATH" ]; then
    # Git's metadata may have been partially cleaned up — its view of the
    # worktree could be gone even though the directory is still there.
    # Try a direct rm -rf as a final fallback.
    if rm -rf "$WORKTREE_PATH" 2>/dev/null; then
      remove_ok=true
    fi
  fi
fi
if ! $remove_ok; then
  echo "  WARN: failed to remove worktree at $WORKTREE_PATH (remove manually with: rm -rf $WORKTREE_PATH)" >&2
fi
if ! git -C "$PROJECT_PATH" branch -d "$BRANCH"; then
  echo "  WARN: failed to delete branch $BRANCH (delete manually with: git branch -D $BRANCH)" >&2
fi
git -C "$PROJECT_PATH" worktree prune 2>/dev/null || true

cat <<EOF

=== $NAME merged ===
Branch '$BRANCH' merged into main and cleaned up.
You're now on main in $PROJECT_PATH.
EOF
