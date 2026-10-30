"""Offline Bash/Git worktree cleanup integration tests (Linux only)."""

import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest


TOOLS = Path(__file__).resolve().parents[1]
BASH = shutil.which("bash")
GIT = shutil.which("git")


@unittest.skipUnless(
    sys.platform.startswith("linux") and BASH and GIT,
    "Bash worktree integration fixtures require Linux, Bash and Git; "
    "native Windows release packaging/smoke tests run separately",
)
class WorktreeCleanupTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.root = self.base / "repo"
        self.root.mkdir()
        self.bin = self.base / "bin"
        self.bin.mkdir()
        self.log = self.base / "commands.log"
        self.env = {
            key: value for key, value in os.environ.items()
            if not key.startswith("GIT_")
        }
        self.env.update({
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_TERMINAL_PROMPT": "0",
            "PATH": str(self.bin) + os.pathsep + os.environ.get("PATH", ""),
            "COMMAND_LOG": str(self.log),
        })
        self.git("init", "--quiet", "-b", "main")
        self.git("config", "user.name", "Worktree Test")
        self.git("config", "user.email", "worktree-test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.autocrlf", "false")
        (self.root / "Tools").mkdir()
        for name in ("create-worktree.sh", "ff-merge-worktree.sh"):
            shutil.copyfile(TOOLS / name, self.root / "Tools" / name)
        (self.root / ".gitignore").write_text(".claude/\n.mcp-port\nignored-marker\n")
        (self.root / "Cargo.toml").write_text("# stub cargo fixture\n")
        (self.root / "tracked").write_text("base\n")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "fixture")
        self.initial = self.git("rev-parse", "HEAD").stdout.strip()
        self.stub("cargo", 'echo "cargo $*" >> "$COMMAND_LOG"\nexit "${CARGO_EXIT:-0}"\n')
        # Delegate every operation to real Git except explicitly injected failures.
        self.stub("git", '''if [ "${3:-}" = worktree ] && [ "${4:-}" = remove ]; then
  echo "remove $*" >> "$COMMAND_LOG"
  case "${REMOVE_MODE:-}" in
    fail) exit 1 ;;
    retry) if [ ! -f "$COMMAND_LOG.retry" ]; then
      touch "$COMMAND_LOG.retry"
      exit 1
    fi ;;
  esac
fi
if [ "${3:-}" = branch ] && [ "${4:-}" = -d ] && [ "${FAIL_BRANCH_DELETE:-}" = 1 ]; then
  exit 1
fi
exec ''' + shlex.quote(GIT) + ' "$@"\n')

    def stub(self, name, body):
        path = self.bin / name
        path.write_text("#!/usr/bin/env bash\nset -eu\n" + body)
        path.chmod(0o755)

    def git(self, *args, cwd=None, check=True):
        return subprocess.run(
            [GIT, "-C", str(cwd or self.root), *args], env=self.env,
            text=True, capture_output=True, check=check,
        )

    def run_script(self, name, expected):
        result = subprocess.run(
            [BASH, str(self.root / "Tools" / name), "feature"],
            cwd=self.root, env=self.env, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result

    def feature(self):
        worktree = self.root / ".claude/worktrees/feature"
        self.git("worktree", "add", "-b", "feature", str(worktree))
        (worktree / "tracked").write_text("feature\n")
        self.git("commit", "--quiet", "-am", "feature", cwd=worktree)
        self.tip = self.git("rev-parse", "HEAD", cwd=worktree).stdout.strip()
        return worktree

    def assert_branch_preserved(self):
        self.assertEqual(self.git("rev-parse", "feature").stdout.strip(), self.tip)

    def assert_merge_and_build_attempted(self):
        self.assertEqual(self.git("rev-parse", "main").stdout.strip(), self.tip)
        self.assertIn("cargo build --manifest-path " + str(self.root / "Cargo.toml"),
                      self.log.read_text())

    def assert_no_build_or_cleanup(self):
        self.assertFalse(self.log.exists())
        self.assert_branch_preserved()

    def test_create_preserves_unregistered_directory(self):
        orphan = self.root / ".claude/worktrees/unregistered"
        orphan.mkdir(parents=True)
        sentinel = orphan / "sentinel"
        sentinel.write_bytes(b"valuable local work\x00\n")
        result = self.run_script("create-worktree.sh", 0)
        self.assertEqual(sentinel.read_bytes(), b"valuable local work\x00\n")
        self.assertIn("WARN", result.stderr)
        self.assertIn("unregistered", result.stderr)
        self.assertIn("skipping", result.stderr)
        self.assertTrue((self.root / ".claude/worktrees/feature/.git").is_file())

    def test_failed_safe_removal_preserves_ignored_file_and_branch(self):
        worktree = self.feature()
        marker = worktree / "ignored-marker"
        marker.write_text("do not delete\n")
        self.env["REMOVE_MODE"] = "fail"
        result = self.run_script("ff-merge-worktree.sh", 6)
        self.assertEqual(marker.read_text(), "do not delete\n")
        self.assert_branch_preserved()
        self.assert_merge_and_build_attempted()
        self.assertEqual(self.log.read_text().count("remove -C"), 2)
        self.assertIn("main merge and build already succeeded", result.stderr)
        self.assertIn("preserved", result.stderr)
        self.assertNotIn("rm -rf", result.stderr)
        self.assertNotIn("cleaned up", result.stdout)
        self.assertIn(str(worktree), self.git("worktree", "list", "--porcelain").stdout)

    def test_success_safely_removes_clean_worktree_and_branch(self):
        worktree = self.feature()
        self.run_script("ff-merge-worktree.sh", 0)
        self.assert_merge_and_build_attempted()
        self.assertFalse(worktree.exists())
        self.assertNotEqual(self.git("rev-parse", "--verify", "feature", check=False).returncode, 0)
        self.assertEqual(self.log.read_text().count("remove -C"), 1)

    def test_safe_removal_is_retried(self):
        worktree = self.feature()
        self.env["REMOVE_MODE"] = "retry"
        self.run_script("ff-merge-worktree.sh", 0)
        self.assertFalse(worktree.exists())
        self.assertEqual(self.log.read_text().count("remove -C"), 2)
        self.assert_merge_and_build_attempted()

    def test_branch_deletion_failure_reports_incomplete_cleanup(self):
        worktree = self.feature()
        self.env["FAIL_BRANCH_DELETE"] = "1"
        result = self.run_script("ff-merge-worktree.sh", 6)
        self.assertFalse(worktree.exists())
        self.assert_branch_preserved()
        self.assert_merge_and_build_attempted()
        self.assertIn("branch 'feature' could not be deleted", result.stderr)
        self.assertNotIn("cleaned up", result.stdout)

    def test_untracked_file_blocks_merge_before_build_or_cleanup(self):
        worktree = self.feature()
        marker = worktree / "untracked-marker"
        marker.write_text("do not delete\n")
        self.run_script("ff-merge-worktree.sh", 3)
        self.assertEqual(marker.read_text(), "do not delete\n")
        self.assert_no_build_or_cleanup()
        self.assertEqual(self.git("rev-parse", "main").stdout.strip(), self.initial)

    def test_mid_rebase_blocks_merge_before_build_or_cleanup(self):
        worktree = self.feature()
        git_dir = Path(self.git("rev-parse", "--git-dir", cwd=worktree).stdout.strip())
        (git_dir / "rebase-merge").mkdir()
        self.run_script("ff-merge-worktree.sh", 3)
        self.assertTrue(worktree.exists())
        self.assert_no_build_or_cleanup()
        self.assertEqual(self.git("rev-parse", "main").stdout.strip(), self.initial)

    def test_diverged_main_blocks_build_and_cleanup(self):
        worktree = self.feature()
        (self.root / "main-only").write_text("main advanced\n")
        self.git("add", "main-only")
        self.git("commit", "--quiet", "-m", "main advanced")
        main_tip = self.git("rev-parse", "HEAD").stdout.strip()
        self.run_script("ff-merge-worktree.sh", 4)
        self.assertTrue(worktree.exists())
        self.assert_no_build_or_cleanup()
        self.assertEqual(self.git("rev-parse", "main").stdout.strip(), main_tip)

    def test_build_failure_preserves_worktree_and_branch(self):
        worktree = self.feature()
        self.env["CARGO_EXIT"] = "1"
        self.run_script("ff-merge-worktree.sh", 5)
        self.assertTrue(worktree.exists())
        self.assert_branch_preserved()
        self.assert_merge_and_build_attempted()
        self.assertNotIn("remove -C", self.log.read_text())


if __name__ == "__main__":
    unittest.main()
