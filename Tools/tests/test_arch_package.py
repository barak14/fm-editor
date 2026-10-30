"""Stdlib-only, cross-platform tests for Arch staging and the static recipe."""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import shutil
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))
spec = importlib.util.spec_from_file_location("stage_arch_package", TOOLS / "stage-arch-package.py")
arch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(arch)
COMMIT = "a" * 40


class ArchStagingTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.root = self.directory / "repo"
        (self.root / "packaging/arch").mkdir(parents=True)
        (self.root / "packaging/assets").mkdir(parents=True)
        for name in ("PKGBUILD", "fm-editor.desktop"):
            shutil.copyfile(ROOT / "packaging/arch" / name, self.root / "packaging/arch" / name)
        shutil.copyfile(ROOT / "packaging/assets/fm-editor.svg", self.root / "packaging/assets/fm-editor.svg")
        self.recipe = (ROOT / "packaging/arch/PKGBUILD").read_text(encoding="utf-8")
        self.archive = self.directory / "local-release.tar"
        self.checksums = self.directory / "SHA256SUMS"
        self.output = self.directory / "staged"
        self.info = {"version": "0.1.0", "target": "x86_64-unknown-linux-gnu", "commit": COMMIT, "profile": "release"}
        self.files = {name: f"Fixture {name}\n".encode() for name in (
            "README.md", "LICENSE", "CHANGELOG.md", "AGENTS.md", "ROADMAP.md")}
        self.files.update({
            "fm-editor": b"\x7fELFfixture",
            "SOURCE.txt": f"Corresponding source for this release: https://github.com/barak14/fm-editor/commit/{COMMIT}\n".encode(),
            "docs/guide.md": b"guide\n",
            "docs/screenshots/example.png": b"image fixture",
        })
        self.addCleanup(mock.patch.stopall)
        mock.patch.object(arch, "ROOT", self.root).start()
        self.write_archive()

    def write_archive(self, *, root=None, extra=None):
        files = dict(self.files, **{"BUILD-INFO.json": json.dumps(self.info).encode()})
        files.update(extra or {})
        with tarfile.open(self.archive, "w:", format=tarfile.USTAR_FORMAT) as stream:
            for name, data in files.items():
                entry = tarfile.TarInfo(f"{root or arch.ARCHIVE_ROOT}/{name}")
                entry.mode = 0o755 if name == "fm-editor" else 0o644
                entry.size = len(data)
                stream.addfile(entry, io.BytesIO(data))
        self.digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksums.write_text(f"{self.digest}  {self.archive.name}\n", encoding="utf-8")

    def stage_fixture(self, **kwargs):
        # Only fixtures replace the archive pin. Production always checks the
        # published fixed digest; the other pins and all validation stay real.
        recipe = self.recipe.replace(arch.ARCHIVE_SHA256, self.digest)
        (self.root / "packaging/arch/PKGBUILD").write_text(recipe, encoding="utf-8")
        with mock.patch.object(arch, "ARCHIVE_SHA256", self.digest):
            return self.stage(**kwargs)

    def stage(self, **kwargs):
        args = dict(archive=self.archive, checksums=self.checksums, commit=COMMIT,
                    version="0.1.0", output_dir=self.output)
        args.update(kwargs)
        return arch.stage(**args)

    def assert_rejected(self, function, pattern):
        with self.assertRaisesRegex((ValueError, OSError), pattern):
            function()
        self.assertFalse(self.output.exists())

    def test_stages_only_recipe_sources_and_original_archive(self):
        self.assertEqual(self.stage_fixture(), self.output)
        self.assertEqual({p.name for p in self.output.iterdir()}, {
            "PKGBUILD", "fm-editor.desktop", "fm-editor.svg", arch.ARCHIVE_NAME})
        self.assertEqual((self.output / arch.ARCHIVE_NAME).read_bytes(), self.archive.read_bytes())
        self.assertIn(self.digest, (self.output / "PKGBUILD").read_text())

    def test_fixed_release_hash_is_not_updated_from_checksum_file(self):
        self.assert_rejected(self.stage, "fixed Arch recipe SHA-256")

    def test_untrusted_or_missing_checksums(self):
        for contents in ("", "0" * 64 + f"  {self.archive.name}\n", "invalid\n"):
            with self.subTest(contents=contents):
                self.checksums.write_text(contents, encoding="utf-8")
                self.assert_rejected(self.stage_fixture, "checksum|SHA-256")

    def test_version_must_match_static_recipe(self):
        for version in ("0.1.1", "../0.1.0", "0.1.0-rc1"):
            with self.subTest(version=version):
                self.assert_rejected(lambda: self.stage_fixture(version=version), "only supports")

    def test_build_identity_and_release_profile(self):
        for field, value in (("profile", "debug"), ("version", "0.2.0"),
                             ("commit", "b" * 40), ("target", "x86_64-pc-windows-msvc")):
            with self.subTest(field=field):
                original = self.info[field]
                self.info[field] = value
                self.write_archive()
                self.assert_rejected(self.stage_fixture, "identity/profile")
                self.info[field] = original

    def test_commit_must_be_full_sha(self):
        self.assert_rejected(lambda: self.stage_fixture(commit="aaaa"), "full source commit")

    def test_source_identity_must_match(self):
        self.files["SOURCE.txt"] = b"https://github.com/other/project/commit/" + COMMIT.encode()
        self.write_archive()
        self.assert_rejected(self.stage_fixture, "source (identity|provenance)")

    def test_archive_root_must_match_recipe(self):
        self.write_archive(root="fm-editor-wrong-root")
        self.assert_rejected(self.stage_fixture, "Archive root")

    def test_unsafe_paths_and_unexpected_files_are_rejected(self):
        for path in ("../escape", "docs/../../escape", "secret.txt"):
            with self.subTest(path=path):
                self.write_archive(extra={path: b"unsafe"})
                self.assert_rejected(self.stage_fixture, "Unsafe|Unexpected")
        self.assertFalse((self.directory / "escape").exists())

    def test_docs_are_required(self):
        self.files = {name: data for name, data in self.files.items() if not name.startswith("docs/")}
        self.write_archive()
        self.assert_rejected(self.stage_fixture, "documentation tree")

    def test_local_source_and_recipe_pins_are_verified(self):
        desktop = self.root / "packaging/arch/fm-editor.desktop"
        desktop.write_bytes(b"changed")
        self.assert_rejected(self.stage_fixture, "Local source SHA-256")
        shutil.copyfile(ROOT / "packaging/arch/fm-editor.desktop", desktop)
        self.recipe = self.recipe.replace(arch.ICON_SHA256, "0" * 64)
        self.assert_rejected(self.stage_fixture, "checksum pins")

    def test_existing_output_is_never_overwritten(self):
        self.output.mkdir()
        marker = self.output / "keep"
        marker.write_bytes(b"user work")
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.stage_fixture()
        self.assertEqual(marker.read_bytes(), b"user work")
        self.assertEqual(list(self.output.iterdir()), [marker])

    def test_existing_file_output_is_never_overwritten(self):
        self.output.write_bytes(b"user work")
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.stage_fixture()
        self.assertEqual(self.output.read_bytes(), b"user work")

    def test_symlink_output_is_never_followed(self):
        target = self.directory / "absent"
        try:
            self.output.symlink_to(target, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest("symlink creation unavailable")
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.stage_fixture()
        self.assertTrue(self.output.is_symlink())
        self.assertFalse(target.exists())


class ArchRecipeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.recipe = (ROOT / "packaging/arch/PKGBUILD").read_text(encoding="utf-8")

    def array(self, name):
        return re.findall(r"'([^']*)'", re.search(rf"^{name}=\((.*?)\)$", self.recipe, re.M | re.S)[1])

    def test_identity_dependencies_and_prebuilt_options(self):
        for assignment in ("pkgname=fm-editor-bin", "pkgver=0.1.0", "pkgrel=1"):
            self.assertIn(assignment, self.recipe)
        self.assertEqual(self.array("arch"), ["x86_64"])
        self.assertEqual(self.array("depends"), ["glibc", "gcc-libs", "libglvnd", "libx11", "libxcursor",
                         "libxrandr", "libxi", "libxkbcommon", "libxkbcommon-x11", "wayland", "hicolor-icon-theme"])
        self.assertEqual(self.array("license"), ["GPL-3.0-or-later"])
        self.assertEqual(self.array("options"), ["!strip", "!debug"])
        self.assertEqual(self.array("provides"), ["fm-editor"])
        self.assertEqual(self.array("conflicts"), ["fm-editor"])
        self.assertEqual([value.split(":")[0] for value in self.array("optdepends")], ["xdg-desktop-portal", "libcap"])

    def test_fixed_source_pins(self):
        self.assertEqual(self.array("sha256sums"), [arch.ARCHIVE_SHA256, arch.DESKTOP_SHA256, arch.ICON_SHA256])
        for path, digest in (("packaging/arch/fm-editor.desktop", arch.DESKTOP_SHA256),
                             ("packaging/assets/fm-editor.svg", arch.ICON_SHA256)):
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest)
        self.assertIn('source=("fm-editor-${pkgver}-linux-input.tar"', self.recipe)
        self.assertNotIn("https://", self.recipe.split("sha256sums")[0].split("source=")[1])
        self.assertNotIn("SKIP", self.recipe)

    def test_no_hooks_privileged_actions_or_compilation(self):
        self.assertNotRegex(self.recipe, r"(?m)^install=")
        self.assertNotRegex(self.recipe, r"\b(sudo|sysctl|setcap|cargo|make)\b")
        self.assertNotRegex(self.recipe, r"(?m)^(build|prepare)\(\)")
        self.assertFalse(list((ROOT / "packaging/arch").glob("*.install")))

    def test_install_tree_and_desktop(self):
        for text in ("/usr/bin/fm-editor", "/usr/share/applications/fm-editor.desktop",
                     "/usr/share/icons/hicolor/scalable/apps/fm-editor.svg", "/usr/share/doc/${pkgname}",
                     "/usr/share/licenses/${pkgname}/LICENSE", 'cp -a "${payload}/docs" "${docdir}/docs"',
                     "README.md,CHANGELOG.md,AGENTS.md,ROADMAP.md,BUILD-INFO.json,SOURCE.txt,LICENSE"):
            self.assertIn(text, self.recipe)
        desktop = (ROOT / "packaging/arch/fm-editor.desktop").read_text(encoding="utf-8")
        for line in ("Type=Application", "Exec=fm-editor", "Icon=fm-editor", "Terminal=false"):
            self.assertIn(line, desktop.splitlines())


if __name__ == "__main__":
    unittest.main()
