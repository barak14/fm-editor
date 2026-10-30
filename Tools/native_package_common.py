"""Validate portable CI/release inputs before making native packages (stdlib only)."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tarfile
import tempfile
import zipfile


TARGETS = {
    "x86_64-pc-windows-msvc": ("fm-editor.exe", b"MZ"),
    "x86_64-unknown-linux-gnu": ("fm-editor", b"\x7fELF"),
}
ROOT_FILES = {"README.md", "LICENSE", "CHANGELOG.md", "AGENTS.md", "ROADMAP.md", "BUILD-INFO.json", "SOURCE.txt"}


def verify_checksum(archive: Path, checksums: Path) -> str:
    """Check one archive against a trusted SHA256SUMS; checksums are not signatures."""
    found = {}
    for line in Path(checksums).read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-fA-F]{64})  ([^/\\]+)", line)
        if not match or match[2] in found:
            raise ValueError("Invalid or duplicate checksum entry")
        found[match[2]] = match[1].lower()
    archive = Path(archive)
    if archive.is_symlink() or not archive.is_file():
        raise ValueError("Portable input must be a regular archive")
    expected = found.get(archive.name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if expected is None or digest != expected:
        raise ValueError("Portable archive SHA-256 mismatch or missing entry")
    return digest


def _relative(name: str) -> tuple[str, str]:
    parts = name.rstrip("/").split("/")
    if len(parts) < 2 or any(p in ("", ".", "..") for p in parts) or "\\" in name or ":" in name or "\x00" in name:
        raise ValueError(f"Unsafe portable archive path: {name!r}")
    if not parts[0].startswith("fm-editor-"):
        raise ValueError("Unexpected portable archive root")
    return parts[0], "/".join(parts[1:])


def _entries(archive: Path, target: str) -> dict[str, tuple[bytes, int]]:
    entries = {}
    roots = set()
    folded = set()

    def add(name, data, mode):
        root, relative = _relative(name)
        roots.add(root)
        if relative.casefold() in folded:
            raise ValueError("Duplicate or case-colliding portable file")
        if mode & 0o7000:
            raise ValueError("Special permissions are not allowed in portable inputs")
        executable = TARGETS[target][0]
        if relative not in ROOT_FILES | {executable} and not relative.startswith("docs/"):
            raise ValueError(f"Unexpected portable file: {relative}")
        folded.add(relative.casefold())
        entries[relative] = (data, mode & 0o777 or 0o644)

    if target == "x86_64-pc-windows-msvc":
        with zipfile.ZipFile(archive) as source:
            for entry in source.infolist():
                mode = entry.external_attr >> 16
                if entry.is_dir():
                    continue
                if entry.flag_bits & 1 or stat.S_ISLNK(mode):
                    raise ValueError("Encrypted files and symlinks are not allowed")
                if stat.S_IFMT(mode) not in (0, stat.S_IFREG):
                    raise ValueError("Nonregular ZIP file")
                add(entry.filename, source.read(entry), mode)
    else:
        with tarfile.open(archive, "r:gz") as source:
            for entry in source.getmembers():
                if entry.isdir():
                    continue
                if not entry.isfile():
                    raise ValueError("Portable tar entries must be regular files")
                add(entry.name, source.extractfile(entry).read(), entry.mode)
    if len(roots) != 1:
        raise ValueError("Portable archive must have one root")
    for name in entries:
        parts = name.split("/")
        if any("/".join(parts[:n]).casefold() in folded for n in range(1, len(parts))):
            raise ValueError("Portable file/directory collision")
    return entries


def extract_portable(
    archive: Path,
    destination: Path,
    *,
    expected_target: str,
    expected_version: str,
    expected_commit: str,
) -> dict:
    """Strip the one top folder, validate identity, and extract into a fresh directory.

    Call verify_checksum separately with trusted release checksums before this.
    Returns the unchanged BUILD-INFO.json object. No installation or privilege changes.
    """
    if expected_target not in TARGETS:
        raise ValueError("Unsupported portable target")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", expected_version):
        raise ValueError("Native packages require a numeric three-part version")
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", expected_commit):
        raise ValueError("A full source commit is required")
    archive, destination = Path(archive), Path(destination)
    if destination.exists() or destination.is_symlink():
        raise ValueError("Refusing to overwrite the extraction directory")
    entries = _entries(archive, expected_target)
    executable, magic = TARGETS[expected_target]
    missing = (ROOT_FILES | {executable}) - entries.keys()
    if missing:
        raise ValueError(f"Portable input missing files: {sorted(missing)}")
    try:
        info = json.loads(entries["BUILD-INFO.json"][0])
    except (ValueError, UnicodeError) as error:
        raise ValueError("Invalid portable build metadata") from error
    if not isinstance(info, dict) or any(info.get(key) != value for key, value in {
        "version": expected_version, "target": expected_target,
        "commit": expected_commit, "profile": "release",
    }.items()):
        raise ValueError("Portable build identity/profile does not match the requested release")
    source_url = f"https://github.com/barak14/fm-editor/commit/{expected_commit}".encode()
    if source_url not in entries["SOURCE.txt"][0]:
        raise ValueError("Portable source provenance does not match the requested release")
    if not entries[executable][0].startswith(magic):
        raise ValueError("Wrong executable format")
    if expected_target.endswith("linux-gnu") and not entries[executable][1] & 0o111:
        raise ValueError("Linux executable permissions are missing")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".native-input-", dir=destination.parent) as temporary:
        stage = Path(temporary) / "payload"
        stage.mkdir()
        for name, (data, mode) in entries.items():
            path = stage / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            path.chmod(mode)
        if destination.exists():
            raise ValueError("Extraction destination appeared while staging")
        os.rename(stage, destination)
    return info
