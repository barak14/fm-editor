#!/usr/bin/env python3
"""Package a clean, already-built fm-editor checkout (Python 3.10+)."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import zipfile


TARGETS = {"x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"}
ROOT_FILES = ("README.md", "LICENSE", "CHANGELOG.md", "AGENTS.md", "ROADMAP.md")
SOURCE_URL = "https://github.com/barak14/fm-editor"


class PackageError(Exception):
    """An unsafe or incomplete release input."""


def command(root: Path, *args: str) -> bytes:
    try:
        return subprocess.run(
            args, cwd=root, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        detail = getattr(exc, "stderr", b"").decode(errors="replace").strip()
        raise PackageError(f"{' '.join(args)} failed: {detail or exc}") from exc


def checkout(root: Path) -> str:
    if command(root, "git", "status", "--porcelain", "--untracked-files=no").strip():
        raise PackageError("Tracked files are dirty; build and package the same clean checkout.")
    commit = command(root, "git", "rev-parse", "--verify", "HEAD").decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit):
        raise PackageError("Git did not return a full HEAD commit.")
    return commit


def package_version(root: Path) -> str:
    raw = command(
        root, "cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"
    )
    try:
        metadata = json.loads(raw)
        packages = [
            package for package in metadata["packages"]
            if package["name"] == "fm-editor"
            and Path(package["manifest_path"]).resolve() == (root / "Cargo.toml").resolve()
        ]
        version = packages[0]["version"] if len(packages) == 1 else None
    except (ValueError, KeyError, TypeError) as exc:
        raise PackageError("Invalid cargo metadata.") from exc
    if not isinstance(version, str) or not re.fullmatch(r"[0-9A-Za-z][0-9A-Za-z.+-]*", version):
        raise PackageError("Cargo metadata must identify the root fm-editor package and version.")
    return version


def release_files(source: Path, source_prefix: str) -> list[tuple[str, bytes, int]]:
    """Read allowlisted files from HEAD, never from local/untracked documentation."""
    files = []
    found = set()
    with tarfile.open(source, "r:gz") as archive:
        for member in archive.getmembers():
            name = member.name.removeprefix(source_prefix + "/")
            if name not in ROOT_FILES and not name.startswith("docs/"):
                continue
            if member.isdir():
                continue
            if not member.isfile() or ".." in Path(name).parts:
                raise PackageError(f"Release documentation must be a regular file: {name}")
            stream = archive.extractfile(member)
            if stream is None:
                raise PackageError(f"Cannot read release documentation: {name}")
            files.append((name, stream.read(), member.mode))
            found.add(name)
    missing = set(ROOT_FILES) - found
    if missing:
        raise PackageError(f"Required tracked release files missing: {', '.join(sorted(missing))}")
    return sorted(files)


def write_binary_archive(path: Path, prefix: str, files: list[tuple[str, bytes, int]], windows: bool) -> None:
    if windows:
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for name, data, mode in files:
                entry = zipfile.ZipInfo(f"{prefix}/{name}")
                entry.create_system = 3
                entry.external_attr = (stat.S_IFREG | mode) << 16
                entry.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(entry, data)
    else:
        with tarfile.open(path, "w:gz") as archive:
            for name, data, mode in files:
                entry = tarfile.TarInfo(f"{prefix}/{name}")
                entry.size = len(data)
                entry.mode = mode
                archive.addfile(entry, io.BytesIO(data))


def package(root: Path, target: str, output_dir: Path) -> list[Path]:
    if target not in TARGETS:
        raise PackageError(f"Unsupported target: {target}")
    root = root.resolve()
    commit = checkout(root)
    version = package_version(root)
    windows = target == "x86_64-pc-windows-msvc"
    executable = "fm-editor.exe" if windows else "fm-editor"
    binary = root / "target" / target / "release" / executable
    if binary.is_symlink() or not binary.is_file():
        raise PackageError(f"Missing regular release binary: {binary}")
    mode = stat.S_IMODE(binary.stat().st_mode)
    if not windows and not mode & 0o111:
        raise PackageError(f"Linux release binary is not executable: {binary}")
    prefix = f"fm-editor-{version}-{target}-{commit}"
    source_prefix = f"fm-editor-{version}-source-{commit}"
    names = [prefix + (".zip" if windows else ".tar.gz"), source_prefix + ".tar.gz", "SHA256SUMS"]
    output_dir = output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    outputs = [output_dir / name for name in names]
    if any(path.exists() or path.is_symlink() for path in outputs):
        raise PackageError("Release output already exists; use a fresh output directory or remove only previous release outputs.")
    with tempfile.TemporaryDirectory(prefix=".package-release-", dir=output_dir) as temporary:
        stage = Path(temporary)
        source = stage / names[1]
        command(root, "git", "-c", "core.autocrlf=false", "-c", "core.eol=lf", "archive", "--format=tar.gz", f"--prefix={source_prefix}/", f"--output={source}", commit)
        files = release_files(source, source_prefix)
        files.append((executable, binary.read_bytes(), mode))
        info = {"version": version, "target": target, "commit": commit, "profile": "release"}
        files.append(("BUILD-INFO.json", (json.dumps(info, indent=2) + "\n").encode(), 0o644))
        source_text = (
            f"Corresponding source for this release: {SOURCE_URL}/commit/{commit}\n"
            f"Source archive distributed alongside this binary: {names[1]}\n"
            f"GitHub source archive: {SOURCE_URL}/archive/{commit}.tar.gz\n"
        )
        files.append(("SOURCE.txt", source_text.encode(), 0o644))
        write_binary_archive(stage / names[0], prefix, files, windows)
        sums = []
        for name in names[:2]:
            digest = hashlib.sha256()
            with (stage / name).open("rb") as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(block)
            sums.append(f"{digest.hexdigest()}  {name}\n")
        (stage / names[2]).write_text("".join(sums), encoding="utf-8")
        if checkout(root) != commit:
            raise PackageError("HEAD changed while packaging; rebuild before packaging.")
        # Exclusive creation protects even against a concurrent packager. On failure,
        # remove only files this invocation created, never pre-existing outputs.
        created = []
        try:
            for name, destination in zip(names, outputs):
                with destination.open("xb") as stream:
                    created.append(destination)
                    with (stage / name).open("rb") as source_stream:
                        shutil.copyfileobj(source_stream, stream)
        except OSError:
            for destination in created:
                destination.unlink()
            raise
    return outputs


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=sorted(TARGETS))
    parser.add_argument("--output-dir", type=Path, default=Path("dist"))
    args = parser.parse_args(argv)
    try:
        outputs = package(Path(__file__).resolve().parent.parent, args.target, args.output_dir)
    except (PackageError, OSError, tarfile.TarError, zipfile.BadZipFile) as exc:
        parser.exit(1, f"error: {exc}\n")
    for path in outputs:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
