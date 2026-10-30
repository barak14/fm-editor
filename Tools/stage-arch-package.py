#!/usr/bin/env python3
"""Stage a pinned Arch prebuilt package from a verified local input tar (stdlib only).

The first source is local (no network): makepkg reuses the staged archive and
checks the recipe's fixed SHA-256. This tool never installs or changes privileges.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import shutil
import tarfile
import tempfile

from native_package_common import extract_portable, verify_checksum

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.1.0"
ARCHIVE_NAME = "fm-editor-0.1.0-linux-input.tar"
ARCHIVE_ROOT = "fm-editor-0.1.0-linux-x64"
ARCHIVE_SHA256 = "186a0ab076084c7baaf70e0a5e17080f5e314b9a8900008540f1f2621446d594"
DESKTOP_SHA256 = "d54a0fd346af7814c55176bcb0754d38ac1fbf957ac7243ab211a9f5cf5864a5"
ICON_SHA256 = "205ab5d2560312caeb1cda6b33b2e92baa4b4ab54e00a79a54b8b538cae348a8"


def stage(archive: Path, checksums: Path, commit: str, version: str, output_dir: Path) -> Path:
    """Validate in a temporary payload, then create a fresh makepkg input folder."""
    if version != VERSION:
        raise ValueError(f"The static Arch recipe only supports version {VERSION}")
    archive, checksums, output_dir = Path(archive), Path(checksums), Path(output_dir)
    if output_dir.exists() or output_dir.is_symlink():
        raise ValueError("Refusing to overwrite the Arch output directory")
    # Validate the supplied path before copying; a second check binds all further
    # validation and the output to the same private snapshot of the archive.
    verify_checksum(archive, checksums)
    with tempfile.TemporaryDirectory(prefix="fm-editor-arch-") as temporary:
        temporary = Path(temporary)
        snapshot = temporary / archive.name
        shutil.copyfile(archive, snapshot)
        digest = verify_checksum(snapshot, checksums)
        if digest != ARCHIVE_SHA256:
            raise ValueError("Archive does not match the fixed Arch recipe SHA-256")
        with tarfile.open(snapshot, "r:*") as source:
            if any(entry.name.rstrip("/").split("/")[0] != ARCHIVE_ROOT for entry in source.getmembers()):
                raise ValueError("Archive root does not match the fixed Arch recipe")
        payload = temporary / "payload"
        extract_portable(snapshot, payload, expected_target="x86_64-unknown-linux-gnu",
                         expected_version=version, expected_commit=commit)
        source_url = f"https://github.com/barak14/fm-editor/commit/{commit}"
        if source_url not in (payload / "SOURCE.txt").read_text(encoding="utf-8").split():
            raise ValueError("Portable source identity does not match the requested commit")
        if not (payload / "docs").is_dir() or not any((payload / "docs").rglob("*")):
            raise ValueError("Portable documentation tree is missing")
        files = {
            "PKGBUILD": ROOT / "packaging/arch/PKGBUILD",
            "fm-editor.desktop": ROOT / "packaging/arch/fm-editor.desktop",
            "fm-editor.svg": ROOT / "packaging/assets/fm-editor.svg",
        }
        # Bind local sources to the recipe as well; never silently refresh pins.
        contents = {name: path.read_bytes() for name, path in files.items()}
        for name, expected in (("fm-editor.desktop", DESKTOP_SHA256), ("fm-editor.svg", ICON_SHA256)):
            if hashlib.sha256(contents[name]).hexdigest() != expected:
                raise ValueError(f"Local source SHA-256 mismatch: {name}")
        recipe = contents["PKGBUILD"].decode("utf-8")
        if any(pin not in recipe for pin in (ARCHIVE_SHA256, DESKTOP_SHA256, ICON_SHA256)):
            raise ValueError("Arch recipe checksum pins do not match the stager")
        output_dir.parent.mkdir(parents=True, exist_ok=True)
        output_dir.mkdir()  # Atomic reservation; existing directories/symlinks fail.
        try:
            for name, data in contents.items():
                with (output_dir / name).open("xb") as destination:
                    destination.write(data)
            with (output_dir / ARCHIVE_NAME).open("xb") as destination, snapshot.open("rb") as source:
                shutil.copyfileobj(source, destination)
        except BaseException:
            shutil.rmtree(output_dir)
            raise
    return output_dir


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--checksums", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        stage(args.archive, args.checksums, args.commit, args.version, args.output_dir)
    except (OSError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f"Arch staging failed: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
