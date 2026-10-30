"""Offline Windows installer packaging checks; no native compiler is run."""
from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import zipfile


TOOLS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("package_windows", TOOLS / "package-windows.py")
windows = importlib.util.module_from_spec(spec)
with mock.patch.object(sys, "path", [str(TOOLS), *sys.path]):
    spec.loader.exec_module(windows)

COMMIT = "3fd" + "a" * 37
VERSION = "0.1.0"


class WindowsInstallerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="installer fixture ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.archive = self.root / "portable.zip"
        self.checksums = self.root / "SHA256SUMS"
        self.iscc = self.root / "compiler space" / "ISCC.exe"
        self.iscc.parent.mkdir()
        self.iscc.write_bytes(b"fake compiler")
        self.output_dir = self.root / "output space"
        self.output = self.output_dir / "fm-editor-0.1.0-windows-x64-setup.exe"
        self.entries = {name: f"fixture {name}\n".encode() for name in (
            "README.md", "LICENSE", "CHANGELOG.md", "AGENTS.md", "ROADMAP.md",
            "docs/ui-guide.md", "docs/nested/image.png")}
        self.entries.update({
            "fm-editor.exe": b"MZ fixture application",
            "BUILD-INFO.json": json.dumps({"version": VERSION, "target": windows.TARGET,
                                            "commit": COMMIT, "profile": "release"}).encode(),
            "SOURCE.txt": f"https://github.com/barak14/fm-editor/commit/{COMMIT}\n".encode(),
        })
        self.write_archive()
        self.compiler = mock.patch.object(windows.subprocess, "run", side_effect=self.compile).start()
        self.addCleanup(mock.patch.stopall)
        self.stages = []

    def write_archive(self):
        with zipfile.ZipFile(self.archive, "w") as archive:
            for name, data in self.entries.items():
                archive.writestr(f"fm-editor-{VERSION}-{windows.TARGET}-{COMMIT}/{name}", data)
        self.checksums.write_text(
            f"{hashlib.sha256(self.archive.read_bytes()).hexdigest()}  {self.archive.name}\n",
            encoding="utf-8")

    def compile(self, args, **kwargs):
        self.assertIsInstance(args, list)
        self.assertEqual(kwargs, {"check": True})
        self.assertEqual(args[0], str(self.iscc))
        self.assertEqual(Path(args[-1]), windows.SCRIPT)
        self.assertTrue(Path(args[-1]).is_absolute())
        self.assertEqual(len(args), 5)
        defines = dict(arg[2:].split("=", 1) for arg in args[1:-1])
        self.assertEqual(defines["AppVersion"], VERSION)
        payload = Path(defines["PayloadDir"])
        self.assertTrue(payload.is_absolute())
        actual = {p.relative_to(payload).as_posix(): p.read_bytes()
                  for p in payload.rglob("*") if p.is_file()}
        self.assertEqual(actual, self.entries)
        self.stages.append(payload.parent)
        (Path(defines["OutputDir"]) / self.output.name).write_bytes(b"MZ fake installer")
        return subprocess.CompletedProcess(args, 0)

    def package(self, **overrides):
        args = dict(archive=self.archive, checksums=self.checksums, commit=COMMIT,
                    version=VERSION, output_dir=self.output_dir, iscc=self.iscc)
        args.update(overrides)
        return windows.package(**args)

    def test_cli_success_intact_payload_and_temporary_staging(self):
        with contextlib.redirect_stdout(io.StringIO()) as printed:
            result = windows.main([
                "--archive", str(self.archive), "--checksums", str(self.checksums),
                "--commit", COMMIT, "--version", VERSION,
                "--output-dir", str(self.output_dir), "--iscc", str(self.iscc)])
        self.assertEqual(result, 0)
        self.assertIn(str(self.output), printed.getvalue())
        self.assertEqual(self.output.read_bytes(), b"MZ fake installer")
        self.compiler.assert_called_once()
        self.assertTrue(all(not path.exists() for path in self.stages))

    def test_bad_checksum_stops_before_compiler(self):
        self.checksums.write_text(f"{'0' * 64}  {self.archive.name}\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.package()
        self.compiler.assert_not_called()
        self.assertFalse(self.output_dir.exists())

    def test_archive_mutated_during_copy_stops_before_compiler(self):
        original_copy = windows.shutil.copyfile
        snapshots = []

        def mutate_then_copy(source, destination):
            self.entries["docs/ui-guide.md"] = b"changed after verification"
            # Change both archive and checksums: the original verified digest
            # must remain authoritative, not a second read of caller checksums.
            self.write_archive()
            snapshots.append(Path(destination))
            return original_copy(source, destination)

        with mock.patch.object(windows.shutil, "copyfile", side_effect=mutate_then_copy):
            with self.assertRaisesRegex(ValueError, "changed while copying"):
                self.package()
        self.compiler.assert_not_called()
        self.assertFalse(self.output_dir.exists())
        self.assertTrue(snapshots)
        self.assertTrue(all(not path.parent.exists() for path in snapshots))

    def test_failed_copy_removes_partial_output(self):
        with mock.patch.object(windows.shutil, "copyfileobj", side_effect=OSError("disk full")):
            with self.assertRaisesRegex(OSError, "disk full"):
                self.package()
        self.assertFalse(self.output.exists())
        self.assertTrue(all(not path.exists() for path in self.stages))

    def test_bad_identity_and_unsafe_payload_stop_before_compiler(self):
        original = self.entries.copy()
        mutations = [
            ("BUILD-INFO.json", b"{}"),
            ("BUILD-INFO.json", b"invalid json"),
            ("fm-editor.exe", b"ELF"),
            ("../outside", b"unsafe"),
            ("opengl32.dll", b"unwanted Mesa"),
        ]
        for name, data in mutations:
            with self.subTest(name=name, data=data):
                self.entries = {**original, name: data}
                self.write_archive()
                with self.assertRaises(ValueError):
                    self.package()
                self.compiler.assert_not_called()
                self.assertFalse(self.output_dir.exists())

    def test_wrong_requested_commit_or_version_stops_before_compiler(self):
        for overrides in ({"commit": "b" * 40}, {"version": "0.1.1"},
                          {"commit": "3fd"}, {"version": "../bad"}):
            with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                self.package(**overrides)
        self.compiler.assert_not_called()

    def test_missing_compiler(self):
        self.iscc.unlink()
        with self.assertRaisesRegex(ValueError, "ISCC"):
            self.package()
        self.compiler.assert_not_called()

    def test_existing_output_is_untouched(self):
        self.output_dir.mkdir()
        self.output.write_bytes(b"keep existing")
        with self.assertRaises(FileExistsError):
            self.package()
        self.assertEqual(self.output.read_bytes(), b"keep existing")
        self.compiler.assert_not_called()

    def test_output_appearing_during_compile_is_untouched(self):
        def compile_with_race(args, **kwargs):
            result = self.compile(args, **kwargs)
            self.output_dir.mkdir()
            self.output.write_bytes(b"concurrent output")
            return result
        self.compiler.side_effect = compile_with_race
        with self.assertRaises(FileExistsError):
            self.package()
        self.assertEqual(self.output.read_bytes(), b"concurrent output")
        self.assertTrue(all(not path.exists() for path in self.stages))

    def test_compile_failure_and_missing_or_empty_output(self):
        for effect in (subprocess.CalledProcessError(1, "ISCC"),
                       lambda *args, **kwargs: subprocess.CompletedProcess(args, 0)):
            with self.subTest(effect=effect):
                self.compiler.side_effect = effect
                with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                    self.package()
                self.assertFalse(self.output.exists())
        def empty_output(args, **kwargs):
            self.compile(args, **kwargs)
            defines = dict(arg[2:].split("=", 1) for arg in args[1:-1])
            (Path(defines["OutputDir"]) / self.output.name).write_bytes(b"")
        self.compiler.side_effect = empty_output
        with self.assertRaisesRegex(ValueError, "nonempty installer"):
            self.package()
        self.assertFalse(self.output.exists())
        self.assertTrue(all(not path.exists() for path in self.stages))

    def test_script_is_per_user_without_install_actions_or_settings_cleanup(self):
        script = windows.SCRIPT.read_text(encoding="utf-8")
        lines = [line.strip() for line in script.splitlines() if not line.startswith(";")]
        for directive in (
            "PrivilegesRequired=lowest", "DefaultDirName={localappdata}\\Programs\\FM Editor",
            "ArchitecturesAllowed=x64compatible", "ArchitecturesInstallIn64BitMode=x64compatible",
            "AppVersion={#AppVersion}", "Uninstallable=yes",
            "UninstallDisplayIcon={app}\\fm-editor.ico",
            "OutputBaseFilename=fm-editor-{#AppVersion}-windows-x64-setup",
        ):
            self.assertIn(directive, lines)
        self.assertRegex(script, r"AppId=\{\{[A-F0-9-]{36}\}")
        self.assertIn('Flags: unchecked', script)
        self.assertIn('Name: "{group}\\FM Editor"', script)
        self.assertIn('Name: "{userdesktop}\\FM Editor"', script)
        self.assertIn('Tasks: desktopicon', script)
        self.assertIn('recursesubdirs', script)
        self.assertIn('fm-editor.ico', script)
        for forbidden in ("PrivilegesRequiredOverridesAllowed", "[Run]", "[UninstallRun]",
                          "[Code]", "[Registry]", "[InstallDelete]", "[UninstallDelete]",
                          "SignedUninstaller", "SignTool", "runas", "setcap", "ptrace"):
            self.assertNotIn(forbidden, "\n".join(lines))
        for macro in ("PayloadDir", "OutputDir", "AppVersion"):
            self.assertIn(f"#ifndef {macro}", lines)


if __name__ == "__main__":
    unittest.main()
