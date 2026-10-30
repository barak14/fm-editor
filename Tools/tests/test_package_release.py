"""Offline release packaging tests; only Python's standard library and Git are used."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import os
import stat
import sys
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile


SCRIPT = Path(__file__).resolve().parents[1] / "package-release.py"
spec = importlib.util.spec_from_file_location("package_release", SCRIPT)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
WINDOWS = "x86_64-pc-windows-msvc"
LINUX = "x86_64-unknown-linux-gnu"


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        # package() canonicalizes its root. Windows temp paths can use an 8.3
        # alias; normalize before constructing the exact-path stat/symlink mocks.
        self.root = self.root.resolve()
        self.git("init", "--quiet")
        self.git("config", "user.email", "release-test@example.invalid")
        self.git("config", "user.name", "Release Test")
        self.git("config", "core.autocrlf", "false")
        self.tracked = {
            ".gitignore": "target/\ndist/\n.claude/\n.rusty/\n",
            "Cargo.toml": '[package]\nname = "fm-editor"\nversion = "2.3.4"\n',
            "Cargo.lock": "fixture lockfile\n",
            "src/main.rs": "fn main() {}\n",
            "docs/guide.md": "Release guide\n",
            "docs/screenshots/player.png": b"\x89PNG\r\nfixture image",
        }
        self.tracked.update({name: f"Fixture {name}\n" for name in release.ROOT_FILES})
        for name, data in self.tracked.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data.encode() if isinstance(data, str) else data)
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "fixture")
        self.commit = self.git("rev-parse", "HEAD").strip()
        for target, executable in ((WINDOWS, "fm-editor.exe"), (LINUX, "fm-editor")):
            directory = self.root / "target" / target / "release"
            directory.mkdir(parents=True)
            (directory / executable).write_bytes(b"main executable fixture")
            if os.name != "nt":
                (directory / executable).chmod(0o755)
            for diagnostic in ("scan_arrays", "dump_manager", "fm-debug"):
                (directory / diagnostic).write_text("not the main binary")
        for name in (".claude/secret", ".rusty/secret", "config/local", "research/notes", "docs/untracked.md"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("local secret")
        original_command = release.command
        self.cargo_calls = []

        def command(root, *args):
            if args[0] == "cargo":
                self.cargo_calls.append(args)
                return json.dumps({"packages": [{
                    "name": "fm-editor", "version": "2.3.4",
                    "manifest_path": str(self.root / "Cargo.toml"),
                }]}).encode()
            return original_command(root, *args)

        self.addCleanup(mock.patch.stopall)
        mock.patch.object(release, "command", side_effect=command).start()
        self.output = self.root / "dist"
        self.linux_mode = 0o755
        if os.name == "nt":
            # NTFS/Python chmod cannot represent POSIX execute bits. Supply a
            # Linux stat result only for the fake Linux binary; still exercise
            # real tar creation/content/mode checks on Windows, without changing
            # any filesystem checks for the rest of the fixture.
            original_stat = Path.stat
            linux_binary = self.root / "target" / LINUX / "release/fm-editor"

            def linux_stat(path, *args, **kwargs):
                result = original_stat(path, *args, **kwargs)
                if path == linux_binary:
                    fields = list(result)
                    fields[0] = stat.S_IFREG | self.linux_mode
                    return os.stat_result(fields)
                return result

            mock.patch.object(Path, "stat", linux_stat).start()

    def git(self, *args):
        return subprocess.run(
            ["git", *args], cwd=self.root, check=True, capture_output=True, text=True
        ).stdout

    def test_both_formats_contents_metadata_permissions_and_checksums(self):
        for target in (WINDOWS, LINUX):
            with self.subTest(target=target):
                output = self.output / target
                binary, source, sums = release.package(self.root, target, output)
                executable = "fm-editor.exe" if target == WINDOWS else "fm-editor"
                prefix = f"fm-editor-2.3.4-{target}-{self.commit}"
                self.assertEqual(binary.name, prefix + (".zip" if target == WINDOWS else ".tar.gz"))
                source_prefix = f"fm-editor-2.3.4-source-{self.commit}"
                self.assertEqual(source.name, source_prefix + ".tar.gz")
                if target == WINDOWS:
                    with zipfile.ZipFile(binary) as archive:
                        entries = {name.removeprefix(prefix + "/"): archive.read(name) for name in archive.namelist()}
                        self.assertTrue(all(name.startswith(prefix + "/") for name in archive.namelist()))
                else:
                    with tarfile.open(binary) as archive:
                        entries = {member.name.removeprefix(prefix + "/"): archive.extractfile(member).read() for member in archive.getmembers()}
                        self.assertTrue(all(member.name.startswith(prefix + "/") for member in archive.getmembers()))
                        self.assertEqual(archive.getmember(prefix + "/fm-editor").mode, 0o755)
                expected = set(release.ROOT_FILES) | {"docs/guide.md", "docs/screenshots/player.png", executable, "BUILD-INFO.json", "SOURCE.txt"}
                self.assertEqual(set(entries), expected)
                self.assertEqual(entries[executable], b"main executable fixture")
                for name in expected & self.tracked.keys():
                    data = self.tracked[name]
                    self.assertEqual(entries[name], data.encode() if isinstance(data, str) else data)
                self.assertEqual(json.loads(entries["BUILD-INFO.json"]), {
                    "version": "2.3.4", "target": target, "commit": self.commit, "profile": "release",
                })
                self.assertIn(f"https://github.com/barak14/fm-editor/commit/{self.commit}", entries["SOURCE.txt"].decode())
                self.assertIn(source.name, entries["SOURCE.txt"].decode())
                self.assertIn(f"/archive/{self.commit}.tar.gz", entries["SOURCE.txt"].decode())
                with tarfile.open(source) as archive:
                    files = {member.name.removeprefix(source_prefix + "/"): archive.extractfile(member).read() for member in archive.getmembers() if member.isfile()}
                self.assertEqual(set(files), set(self.tracked))
                for name, data in self.tracked.items():
                    self.assertEqual(files[name], data.encode() if isinstance(data, str) else data)
                expected_sums = "".join(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n" for path in (binary, source))
                self.assertEqual(sums.read_text(), expected_sums)
        self.assertTrue(self.cargo_calls)
        self.assertTrue(all(call == ("cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1") for call in self.cargo_calls))

    def test_archive_ignores_user_crlf_config(self):
        self.git("config", "core.autocrlf", "true")
        self.git("config", "core.eol", "crlf")
        self.git("update-index", "--refresh", "-q")
        self.assertEqual(self.git("status", "--porcelain", "--untracked-files=no"), "")
        blobs = {name: subprocess.run(["git", "show", f"{self.commit}:{name}"], cwd=self.root, check=True, capture_output=True).stdout for name in self.tracked}
        self.assertIn(b"\n", blobs["docs/guide.md"])
        self.assertNotIn(b"\r\n", blobs["docs/guide.md"])
        for target in (WINDOWS, LINUX):
            with self.subTest(target=target):
                binary, source, _ = release.package(self.root, target, self.output / target)
                source_prefix = f"fm-editor-2.3.4-source-{self.commit}"
                with tarfile.open(source) as archive:
                    files = {m.name.removeprefix(source_prefix + "/"): archive.extractfile(m).read() for m in archive.getmembers() if m.isfile()}
                self.assertEqual(files, blobs)
                prefix = f"fm-editor-2.3.4-{target}-{self.commit}"
                if target == WINDOWS:
                    with zipfile.ZipFile(binary) as archive:
                        entries = {n.removeprefix(prefix + "/"): archive.read(n) for n in archive.namelist()}
                else:
                    with tarfile.open(binary) as archive:
                        entries = {m.name.removeprefix(prefix + "/"): archive.extractfile(m).read() for m in archive.getmembers()}
                for name in set(entries) & set(blobs):
                    self.assertEqual(entries[name], blobs[name])
                self.assertEqual(entries["docs/guide.md"], blobs["docs/guide.md"])

    def test_unknown_target(self):
        with self.assertRaisesRegex(release.PackageError, "Unsupported target"):
            release.package(self.root, "aarch64-unknown-linux-gnu", self.output)
        self.assertFalse(self.output.exists())

    def test_missing_binary(self):
        (self.root / "target" / LINUX / "release/fm-editor").unlink()
        with self.assertRaisesRegex(release.PackageError, "Missing regular release binary"):
            release.package(self.root, LINUX, self.output)
        self.assertFalse(self.output.exists())

    def test_symlink_binary_is_rejected(self):
        binary = self.root / "target" / LINUX / "release/fm-editor"
        original_is_symlink = Path.is_symlink

        def is_symlink(path):
            return path == binary or original_is_symlink(path)

        # No Windows developer-mode or symlink privilege requirement.
        with mock.patch.object(Path, "is_symlink", is_symlink):
            with self.assertRaisesRegex(release.PackageError, "Missing regular release binary"):
                release.package(self.root, LINUX, self.output)

    def test_unsafe_document_archive_entries_are_rejected(self):
        source = self.root / "unsafe-source.tar.gz"
        for kind, name in (
            (tarfile.SYMTYPE, "docs/guide.md"),
            (tarfile.LNKTYPE, "docs/guide.md"),
            (tarfile.FIFOTYPE, "docs/guide.md"),
            (tarfile.REGTYPE, "docs/../secret"),
        ):
            with self.subTest(kind=kind, name=name):
                with tarfile.open(source, "w:gz") as archive:
                    member = tarfile.TarInfo(f"fixture/{name}")
                    member.type = kind
                    if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                        member.linkname = "../README.md"
                    archive.addfile(member)
                with self.assertRaisesRegex(release.PackageError, "must be a regular file"):
                    release.release_files(source, "fixture")

    def test_nonexecutable_linux_binary(self):
        if os.name == "nt":
            self.linux_mode = 0o644
        else:
            (self.root / "target" / LINUX / "release/fm-editor").chmod(0o644)
        with self.assertRaisesRegex(release.PackageError, "not executable"):
            release.package(self.root, LINUX, self.output)

    def test_dirty_tracked_source_unstaged_staged_and_deleted(self):
        source = self.root / "src/main.rs"
        for change in ("unstaged", "staged", "deleted"):
            with self.subTest(change=change):
                if change == "deleted":
                    source.unlink()
                else:
                    source.write_text("changed source")
                if change == "staged":
                    self.git("add", "src/main.rs")
                with self.assertRaisesRegex(release.PackageError, "Tracked files are dirty"):
                    release.package(self.root, LINUX, self.output)
                self.assertFalse(self.output.exists())
                self.git("reset", "--hard", "HEAD")

    def test_missing_required_tracked_document(self):
        self.git("rm", "LICENSE")
        self.git("commit", "--quiet", "-m", "missing license")
        with self.assertRaisesRegex(release.PackageError, "Required tracked release files missing: LICENSE"):
            release.package(self.root, LINUX, self.output)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_existing_outputs_are_never_clobbered(self):
        self.output.mkdir()
        unrelated = self.output / "unrelated.txt"
        unrelated.write_text("keep me")
        sums = self.output / "SHA256SUMS"
        sums.write_text("previous checksums")
        with self.assertRaisesRegex(release.PackageError, "output already exists"):
            release.package(self.root, LINUX, self.output)
        self.assertEqual(sums.read_text(), "previous checksums")
        self.assertEqual(unrelated.read_text(), "keep me")
        self.assertEqual(len(list(self.output.iterdir())), 2)

    def test_unrelated_outputs_preserved_on_success(self):
        self.output.mkdir()
        unrelated = self.output / "unrelated.txt"
        unrelated.write_text("keep me")
        release.package(self.root, LINUX, self.output)
        self.assertEqual(unrelated.read_text(), "keep me")

    def test_bad_or_wrong_package_metadata(self):
        for raw in (b"not json", b"{}", b'{"packages": []}', json.dumps({"packages": [{"name": "fm-editor", "version": "9.9.9", "manifest_path": str(self.root / "nested/Cargo.toml")}]}).encode()):
            with self.subTest(raw=raw), mock.patch.object(release, "command", return_value=raw):
                with self.assertRaises(release.PackageError):
                    release.package_version(self.root)

    def test_git_or_cargo_failure_is_reported(self):
        with mock.patch.object(subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo", stderr=b"offline metadata unavailable")):
            # Call the unmocked helper through a fresh module.
            fresh = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(fresh)
            with self.assertRaisesRegex(fresh.PackageError, "offline metadata unavailable"):
                fresh.package_version(self.root)

    def test_head_change_during_packaging_is_rejected(self):
        with mock.patch.object(release, "checkout", side_effect=[self.commit, "f" * 40]):
            with self.assertRaisesRegex(release.PackageError, "HEAD changed"):
                release.package(self.root, LINUX, self.output)
        self.assertEqual(list(self.output.iterdir()), [])

    def test_main_uses_script_root_not_working_directory(self):
        fake_script = self.root / "Tools/package-release.py"
        stdout = io.StringIO()
        with mock.patch.object(release, "__file__", str(fake_script)), contextlib.redirect_stdout(stdout):
            self.assertEqual(release.main(["--target", WINDOWS, "--output-dir", str(self.output)]), 0)
        self.assertEqual(len(stdout.getvalue().splitlines()), 3)

    def test_cli_rejects_unknown_target(self):
        result = subprocess.run([sys.executable, str(SCRIPT), "--target", "unknown"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("invalid choice", result.stderr)


if __name__ == "__main__":
    unittest.main()
