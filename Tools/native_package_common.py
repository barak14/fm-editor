"""Validate portable CI/release inputs before making native packages (stdlib only)."""
from __future__ import annotations

import hashlib
import io
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
        with tarfile.open(archive, "r:*") as source:
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


def canonicalize_portable(archive: Path, destination: Path, *, expected_target: str,
                          expected_version: str, expected_commit: str,
                          expected_executable_sha256: str) -> dict:
    """Normalize package containers, never executable bytes, from a pinned CI input.

    Plain USTAR / stored ZIP avoid platform-specific gzip or deflate output.
    The resulting inputs are bundled with recipes for offline rebuilds.
    """
    destination = Path(destination)
    if destination.exists() or destination.is_symlink():
        raise ValueError("Refusing to overwrite canonical native input")
    if not re.fullmatch(r"[0-9a-f]{64}", expected_executable_sha256):
        raise ValueError("A pinned executable SHA-256 is required")
    with tempfile.TemporaryDirectory(prefix="fm-editor-canonical-") as temporary:
        payload = Path(temporary) / "payload"
        info = extract_portable(archive, payload, expected_target=expected_target,
                                expected_version=expected_version, expected_commit=expected_commit)
        executable = TARGETS[expected_target][0]
        if hashlib.sha256((payload / executable).read_bytes()).hexdigest() != expected_executable_sha256:
            raise ValueError("Application executable differs from the pinned release binary")
        source = payload / "SOURCE.txt"
        old = f"fm-editor-{expected_version}-source-{expected_commit}.tar.gz"
        source.write_bytes(source.read_bytes().replace(old.encode(), f"fm-editor-{expected_version}-source.tar.gz".encode()))
        label = "windows-x64" if executable.endswith(".exe") else "linux-x64"
        prefix = f"fm-editor-{expected_version}-{label}"
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("xb") as stream:
            try:
                if executable.endswith(".exe"):
                    with zipfile.ZipFile(stream, "w", compression=zipfile.ZIP_STORED) as output:
                        for path in sorted((p for p in payload.rglob("*") if p.is_file()), key=lambda p: p.relative_to(payload).as_posix()):
                            name = path.relative_to(payload).as_posix()
                            entry = zipfile.ZipInfo(f"{prefix}/{name}")
                            entry.create_system = 3
                            entry.external_attr = (stat.S_IFREG | (0o755 if name == executable else 0o644)) << 16
                            output.writestr(entry, path.read_bytes())
                else:
                    with tarfile.open(fileobj=stream, mode="w", format=tarfile.USTAR_FORMAT) as output:
                        for path in sorted((p for p in payload.rglob("*") if p.is_file()), key=lambda p: p.relative_to(payload).as_posix()):
                            name = path.relative_to(payload).as_posix()
                            data = path.read_bytes()
                            entry = tarfile.TarInfo(f"{prefix}/{name}")
                            entry.size = len(data)
                            entry.mode = 0o755 if name == executable else 0o644
                            output.addfile(entry, io.BytesIO(data))
            except BaseException:
                stream.close()
                destination.unlink()
                raise
    return info
