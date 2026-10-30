#!/usr/bin/env python3
"""Download and verify existing portable release inputs; never move tags or publish."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from native_package_common import extract_portable, verify_checksum


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True)


def fetch(tag: str, repository: str, output: Path) -> dict:
    match = re.fullmatch(r"v([0-9]+\.[0-9]+\.[0-9]+)", tag)
    if not match or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]*/[A-Za-z0-9][A-Za-z0-9_.-]*", repository):
        raise ValueError("Use a numeric vX.Y.Z tag and an owner/repository name")
    version = match[1]
    commit = gh("api", f"repos/{repository}/commits/{tag}", "--jq", ".sha").strip()
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit):
        raise ValueError("Release tag did not resolve to a full commit")
    release = json.loads(gh("release", "view", tag, "--repo", repository, "--json", "tagName"))
    if release.get("tagName") != tag:
        raise ValueError("Wrong release tag")
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise ValueError("Refusing to overwrite native inputs")
    output.parent.mkdir(parents=True, exist_ok=True)
    files = [f"fm-editor-{version}-windows-x64.zip", f"fm-editor-{version}-linux-x64.tar.gz", f"fm-editor-{version}-source.tar.gz", "SHA256SUMS"]
    with tempfile.TemporaryDirectory(prefix=".native-fetch-", dir=output.parent) as temporary:
        stage = Path(temporary) / "inputs"
        args = ["release", "download", tag, "--repo", repository, "--dir", str(stage)]
        for name in files:
            args += ["--pattern", name]
        gh(*args)
        digests = {}
        for name in files[:-1]:
            digests[name] = verify_checksum(stage / name, stage / "SHA256SUMS")
        for target, name in [("x86_64-pc-windows-msvc", files[0]), ("x86_64-unknown-linux-gnu", files[1])]:
            extract_portable(stage / name, Path(temporary) / target, expected_target=target, expected_version=version, expected_commit=commit)
        info = {"tag": tag, "version": version, "commit": commit, "repository": repository, "archives": digests}
        (stage / "INPUTS.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
        # Copy only the known release inputs/manifest, not any extraction scratch.
        output.mkdir()
        try:
            for name in files + ["INPUTS.json"]:
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
