#!/usr/bin/env python3
"""Fetch pinned successful Actions inputs; never move tags or publish releases."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from native_package_common import canonicalize_portable, verify_checksum

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {
    "x86_64-pc-windows-msvc": ("windows-input.zip", ".zip"),
    "x86_64-unknown-linux-gnu": ("linux-input.tar", ".tar.gz"),
}
OFFLINE_HELP = (
    "Use the bundled verified inputs from packaging-source for an offline rebuild; "
    "do not bypass the release pins."
)


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True)


def load_manifest(tag: str, version: str) -> dict:
    try:
        manifest = json.loads((ROOT / "packaging" / "releases" / f"{tag}.json").read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise ValueError(f"Cannot read trusted release manifest for {tag}: {error}") from error
    if not isinstance(manifest, dict):
        raise ValueError("Invalid release manifest")
    commit = manifest.get("commit")
    run = manifest.get("build_run")
    targets = manifest.get("targets")
    if (manifest.get("tag") != tag or manifest.get("version") != version
            or not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit)
            or type(run) is not int or run <= 0
            or not isinstance(targets, dict) or set(targets) != set(TARGETS)):
        raise ValueError("Invalid release manifest identity, build run or targets")
    for target, (suffix, _) in TARGETS.items():
        pin = targets[target]
        if not isinstance(pin, dict) or pin.get("canonical_name") != f"fm-editor-{version}-{suffix}":
            raise ValueError("Invalid release manifest canonical name")
        for field in ("archive_sha256", "executable_sha256", "canonical_sha256"):
            if not isinstance(pin.get(field), str) or not re.fullmatch(r"[0-9a-f]{64}", pin[field]):
                raise ValueError(f"Invalid release manifest {field}")
    return manifest


def fetch(tag: str, repository: str, output: Path) -> dict:
    match = re.fullmatch(r"v([0-9]+\.[0-9]+\.[0-9]+)", tag)
    if not match or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]*/[A-Za-z0-9][A-Za-z0-9_.-]*", repository):
        raise ValueError("Use a numeric vX.Y.Z tag and an owner/repository name")
    version = match[1]
    manifest = load_manifest(tag, version)
    commit = manifest["commit"]
    run = str(manifest["build_run"])
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise ValueError("Refusing to overwrite native inputs")
    resolved = gh("api", f"repos/{repository}/commits/{tag}", "--jq", ".sha").strip()
    if resolved != commit:
        raise ValueError("Release tag does not match the pinned full commit")
    build = json.loads(gh("run", "view", run, "--repo", repository,
                          "--json", "headSha,status,conclusion,workflowName"))
    if (not isinstance(build, dict) or build.get("headSha") != commit
            or build.get("status") != "completed" or build.get("conclusion") != "success"
            or build.get("workflowName") != "Build and package"):
        raise ValueError("Pinned build must match the full commit and be a completed successful Build and package run")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".native-fetch-", dir=output.parent) as temporary:
        stage = Path(temporary) / "inputs"
        stage.mkdir()
        digests = {}
        for target, (_, extension) in TARGETS.items():
            wrapper = Path(temporary) / target
            wrapper.mkdir()
            try:
                gh("run", "download", run, "--repo", repository,
                   "--name", f"fm-editor-{target}", "--dir", str(wrapper))
            except subprocess.CalledProcessError as error:
                raise ValueError(f"Pinned CI artifact for {target} is unavailable or expired. {OFFLINE_HELP}") from error
            raw_name = f"fm-editor-{version}-{target}-{commit}{extension}"
            source_name = f"fm-editor-{version}-source-{commit}.tar.gz"
            required = {raw_name, source_name, "SHA256SUMS"}
            if {path.name for path in wrapper.iterdir()} != required or any(
                    path.is_symlink() or not path.is_file() for path in wrapper.iterdir()):
                raise ValueError(f"Pinned CI artifact for {target} is missing or has unexpected contents. {OFFLINE_HELP}")
            raw = wrapper / raw_name
            digest = verify_checksum(raw, wrapper / "SHA256SUMS")
            verify_checksum(wrapper / source_name, wrapper / "SHA256SUMS")
            pin = manifest["targets"][target]
            if digest != pin["archive_sha256"]:
                raise ValueError("Portable archive SHA-256 differs from the release manifest pin")
            destination = stage / pin["canonical_name"]
            canonicalize_portable(raw, destination, expected_target=target,
                                  expected_version=version, expected_commit=commit,
                                  expected_executable_sha256=pin["executable_sha256"])
            digest = hashlib.sha256(destination.read_bytes()).hexdigest()
            if digest != pin["canonical_sha256"]:
                raise ValueError("Canonical archive SHA-256 differs from the release manifest pin")
            digests[destination.name] = digest
        (stage / "SHA256SUMS").write_text(
            "".join(f"{digest}  {name}\n" for name, digest in sorted(digests.items())), encoding="utf-8")
        info = {"tag": tag, "version": version, "commit": commit, "repository": repository,
                "build_run": manifest["build_run"], "archives": digests}
        (stage / "INPUTS.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
        # Exclusive creation protects existing output, including a concurrent fetch.
        output.mkdir()
        try:
            for name in [*digests, "SHA256SUMS", "INPUTS.json"]:
                with (output / name).open("xb") as destination, (stage / name).open("rb") as source:
                    shutil.copyfileobj(source, destination)
        except BaseException:
            shutil.rmtree(output)
            raise
    return info


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        info = fetch(args.tag, args.repository, args.output_dir)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Native release input fetch failed: {error}\n")
    print(json.dumps(info))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
