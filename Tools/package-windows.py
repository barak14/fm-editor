#!/usr/bin/env python3
"""Wrap an existing verified Windows portable release with Inno Setup (stdlib only)."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import zipfile

from native_package_common import extract_portable, verify_checksum


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "packaging/windows/fm-editor.iss"
RELEASES_DIR = ROOT / "packaging/releases"
TARGET = "x86_64-pc-windows-msvc"


def release_target(version: str, commit: str) -> dict:
    """Read pins shipped in the trusted source bundle, not caller metadata."""
    try:
        manifest = json.loads((RELEASES_DIR / f"v{version}.json").read_text(encoding="utf-8"))
        if manifest["version"] != version or manifest["commit"] != commit:
            raise ValueError("Release identity mismatch")
        target = manifest["targets"][TARGET]
        for field in ("canonical_sha256", "executable_sha256"):
            if not isinstance(target[field], str) or not re.fullmatch(r"[0-9a-f]{64}", target[field]):
                raise ValueError("Invalid release digest")
        return target
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ValueError("Release pin mismatch or missing manifest; use the verified source-bundle native input") from error


def package(archive: Path, checksums: Path, commit: str, version: str,
            output_dir: Path, iscc: Path) -> Path:
    """Compile an unsigned per-user installer; never build or modify the app."""
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("A numeric three-part version is required")
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit):
        raise ValueError("A full source commit is required")
    iscc = Path(iscc).resolve()
    if not iscc.is_file():
        raise ValueError(f"ISCC compiler does not exist: {iscc}")
    output_dir = Path(output_dir).resolve()
    output = output_dir / f"fm-editor-{version}-windows-x64-setup.exe"
    if output.exists() or output.is_symlink():
        raise FileExistsError(f"Refusing to overwrite installer: {output}")
    target = release_target(version, commit)
    digest = verify_checksum(Path(archive), Path(checksums))
    with tempfile.TemporaryDirectory(prefix="fm-editor-windows-") as temporary:
        stage = Path(temporary).resolve()
        snapshot = stage / "portable.zip"
        shutil.copyfile(archive, snapshot)
        if hashlib.sha256(snapshot.read_bytes()).hexdigest() != digest:
            raise ValueError("Portable archive changed while copying verified input")
        if digest != target["canonical_sha256"]:
            raise ValueError("Archive is not the pinned release; use the verified source-bundle native input")
        payload = stage / "payload"
        extract_portable(snapshot, payload, expected_target=TARGET,
                         expected_version=version, expected_commit=commit)
        if hashlib.sha256((payload / "fm-editor.exe").read_bytes()).hexdigest() != target["executable_sha256"]:
            raise ValueError("Executable is not the pinned release; use the verified source-bundle native input")
        compiled = stage / "compiled"
        compiled.mkdir()
        # Each /D definition is one argument even when its value contains spaces.
        # The absolute script path keeps assets independent of the caller's cwd.
        subprocess.run([str(iscc), f"/DPayloadDir={payload}",
                        f"/DOutputDir={compiled}", f"/DAppVersion={version}",
                        str(SCRIPT)], check=True)
        installer = compiled / output.name
        if installer.is_symlink() or not installer.is_file() or installer.stat().st_size == 0:
            raise ValueError("ISCC did not produce the expected nonempty installer")
        output_dir.mkdir(parents=True, exist_ok=True)
        # Exclusive creation also protects against an output appearing during compile.
        destination = output.open("xb")
        try:
            with destination, installer.open("rb") as source:
                shutil.copyfileobj(source, destination)
        except BaseException:
            # Close the destination first so cleanup also works on Windows.
            output.unlink()
            raise
    return output


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--checksums", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--iscc", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        output = package(args.archive, args.checksums, args.commit, args.version,
                         args.output_dir, args.iscc)
    except (OSError, ValueError, zipfile.BadZipFile, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Windows packaging failed: {error}\n")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
