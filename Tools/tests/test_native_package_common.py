"""Portable-to-native input validation; no compiler, GUI, Git or network required."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tarfile
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("native_common", Path(__file__).parents[1] / "native_package_common.py")
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
COMMIT = "a" * 40
VERSION = "0.1.0"
WINDOWS = "x86_64-pc-windows-msvc"
LINUX = "x86_64-unknown-linux-gnu"


class NativeInputTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.destination = self.root / "extracted"

    def fixture(self, target=LINUX, update=None, additional=None):
        executable, magic = common.TARGETS[target]
        info = {"version": VERSION, "target": target, "commit": COMMIT, "profile": "release"}
        entries = {name: (f"Document {name}\n".encode(), 0o644) for name in common.ROOT_FILES}
        entries.update({"BUILD-INFO.json": (json.dumps(info).encode(), 0o644), executable: (magic + b" fixture", 0o755), "docs/guide.md": (b"guide", 0o644)})
        entries["SOURCE.txt"] = (f"Source: https://github.com/barak14/fm-editor/commit/{COMMIT}\n".encode(), 0o644)
        if update:
            update(entries)
        rows = [(f"fm-editor-{VERSION}/" + name, value) for name, value in entries.items()]
        rows.extend(additional or [])
        path = self.root / ("portable.zip" if target == WINDOWS else "portable.tar.gz")
        if target == WINDOWS:
            with zipfile.ZipFile(path, "w") as archive:
                for name, (data, mode) in rows:
                    item = zipfile.ZipInfo(name)
                    item.create_system = 3
                    item.external_attr = (stat.S_IFREG | mode) << 16
                    archive.writestr(item, data)
        else:
            with tarfile.open(path, "w:gz") as archive:
                for name, (data, mode) in rows:
                    item = tarfile.TarInfo(name)
                    item.size = len(data)
                    item.mode = mode
                    archive.addfile(item, io.BytesIO(data))
        sums = self.root / "SHA256SUMS"
        sums.write_text(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n")
        return path, sums, entries

    def extract(self, path, target=LINUX, **kwargs):
        return common.extract_portable(path, self.destination, expected_target=target, expected_version=kwargs.get("version", VERSION), expected_commit=kwargs.get("commit", COMMIT))

    def test_both_formats_identity_contents_and_permissions(self):
        for target in (WINDOWS, LINUX):
            with self.subTest(target=target):
                path, sums, entries = self.fixture(target)
                self.assertEqual(common.verify_checksum(path, sums), hashlib.sha256(path.read_bytes()).hexdigest())
                self.destination = self.root / target
                info = self.extract(path, target)
                self.assertEqual(info["commit"], COMMIT)
                for name, (data, mode) in entries.items():
                    output = self.destination / name
                    self.assertEqual(output.read_bytes(), data)
                    if os.name != "nt":
                        self.assertEqual(stat.S_IMODE(output.stat().st_mode), mode)

    def test_checksum_mismatch_missing_duplicate_and_bad_entries(self):
        path, sums, _ = self.fixture()
        original = sums.read_text()
        for content in [original.replace(original[:64], "0" * 64), "", original + original, "../bad\n"]:
            sums.write_text(content)
            with self.assertRaises(ValueError):
                common.verify_checksum(path, sums)

    def test_metadata_mismatches_fail_before_extraction(self):
        for key, value in [("version", "9.0.0"), ("target", WINDOWS), ("commit", "b" * 40), ("profile", "debug")]:
            def change(entries):
                info = json.loads(entries["BUILD-INFO.json"][0])
                info[key] = value
                entries["BUILD-INFO.json"] = (json.dumps(info).encode(), 0o644)
            path, _, _ = self.fixture(update=change)
            with self.assertRaisesRegex(ValueError, "identity/profile"):
                self.extract(path)
            self.assertFalse(self.destination.exists())

    def test_source_provenance_mismatch_is_rejected(self):
        path, _, _ = self.fixture(update=lambda e: e.__setitem__("SOURCE.txt", (b"different source", 0o644)))
        with self.assertRaisesRegex(ValueError, "source provenance"):
            self.extract(path)
        self.assertFalse(self.destination.exists())

    def test_paths_duplicates_private_files_and_case_collisions(self):
        for name in ["fm-editor-0.1.0/../escape", "/absolute/escape", "fm-editor-0.1.0/docs/a:b", "fm-editor-0.1.0/docs\\escape", "fm-editor-0.1.0/.mcp.json", "fm-editor-0.1.0/fm-editor", "fm-editor-0.1.0/docs/GUIDE.md", "other-root/docs/other"]:
            path, _, _ = self.fixture(additional=[(name, (b"bad", 0o644))])
            with self.assertRaises(ValueError, msg=name):
                self.extract(path)
            self.assertFalse(self.destination.exists())

    def test_links_rejected_in_each_format(self):
        path, _, _ = self.fixture()
        with tarfile.open(path, "w:gz") as archive:
            link = tarfile.TarInfo("fm-editor-0.1.0/docs/link")
            link.type = tarfile.SYMTYPE
            link.linkname = "../../outside"
            archive.addfile(link)
        with self.assertRaises(ValueError):
            self.extract(path)
        path, _, _ = self.fixture(WINDOWS)
        with zipfile.ZipFile(path, "a") as archive:
            link = zipfile.ZipInfo("fm-editor-0.1.0/docs/link")
            link.create_system = 3
            link.external_attr = (stat.S_IFLNK | 0o777) << 16
            archive.writestr(link, "../../outside")
        with self.assertRaises(ValueError):
            self.extract(path, WINDOWS)

    def test_bad_executable_mode_magic_and_missing_file(self):
        for alteration in [lambda e: e.__setitem__("fm-editor", (b"\x7fELF", 0o644)), lambda e: e.__setitem__("fm-editor", (b"wrong", 0o755)), lambda e: e.__setitem__("fm-editor", (b"\x7fELF", 0o4755)), lambda e: e.pop("LICENSE")]:
            path, _, _ = self.fixture(update=alteration)
            with self.assertRaises(ValueError):
                self.extract(path)
            self.assertFalse(self.destination.exists())

    def test_existing_destination_is_preserved(self):
        path, _, _ = self.fixture()
        self.destination.mkdir()
        (self.destination / "sentinel").write_text("keep")
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.extract(path)
        self.assertEqual((self.destination / "sentinel").read_text(), "keep")

    def test_non_numeric_version_and_partial_commit_rejected(self):
        path, _, _ = self.fixture()
        for kwargs in [{"version": "0.1.0;bad"}, {"commit": "a" * 7}]:
            with self.assertRaises(ValueError):
                self.extract(path, **kwargs)
        self.assertFalse(self.destination.exists())


if __name__ == "__main__":
    unittest.main()
