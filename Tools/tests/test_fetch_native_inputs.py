"""Offline Actions transport tests; real canonicalization is tested separately."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).parents[1]
sys.path.insert(0, str(TOOLS))
spec = importlib.util.spec_from_file_location("fetch_native", TOOLS / "fetch-native-inputs.py")
fetcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetcher)
COMMIT = "a" * 40
RUN = 41024678363


def sha(data):
    return hashlib.sha256(data).hexdigest()


class FetchNativeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / "inputs"
        self.manifest_path = self.root / "packaging/releases/v0.1.0.json"
        self.manifest_path.parent.mkdir(parents=True)
        self.manifest = {"tag": "v0.1.0", "version": "0.1.0", "commit": COMMIT,
                         "build_run": RUN, "targets": {}}
        for target, (suffix, _) in fetcher.TARGETS.items():
            self.manifest["targets"][target] = {
                "archive_sha256": sha(target.encode()), "executable_sha256": "b" * 64,
                "canonical_name": f"fm-editor-0.1.0-{suffix}",
                "canonical_sha256": sha(("canonical:" + target).encode()),
            }
        self.save_manifest()
        self.calls = []
        self.commit = COMMIT
        self.build = {"headSha": COMMIT, "status": "completed", "conclusion": "success",
                      "workflowName": "Build and package"}
        self.bad_checksum = None
        self.bad_raw_pin = False
        self.bad_canonical = False
        self.missing = False
        self.expired = False
        self.extra = False
        self.start_patch("ROOT", self.root)
        self.start_patch("gh", side_effect=self.gh)
        self.canonicalizer = self.start_patch("canonicalize_portable", side_effect=self.canonicalize)

    def start_patch(self, name, *args, **kwargs):
        patcher = patch.object(fetcher, name, *args, **kwargs)
        result = patcher.start()
        self.addCleanup(patcher.stop)
        return result

    def save_manifest(self):
        self.manifest_path.write_text(json.dumps(self.manifest))

    def gh(self, *args):
        self.calls.append(args)
        if args[0] == "api":
            self.assertEqual(args, ("api", "repos/barak14/fm-editor/commits/v0.1.0", "--jq", ".sha"))
            return self.commit
        if args[:2] == ("run", "view"):
            self.assertEqual(args, ("run", "view", str(RUN), "--repo", "barak14/fm-editor",
                                    "--json", "headSha,status,conclusion,workflowName"))
            return json.dumps(self.build)
        if args[:2] == ("run", "download"):
            self.assertEqual(args[2:5], (str(RUN), "--repo", "barak14/fm-editor"))
            target = args[args.index("--name") + 1].removeprefix("fm-editor-")
            self.assertIn(target, fetcher.TARGETS)
            if self.expired:
                raise subprocess.CalledProcessError(1, ["gh", *args])
            out = Path(args[args.index("--dir") + 1])
            if self.missing:
                return ""
            names = {"raw": f"fm-editor-0.1.0-{target}-{COMMIT}{fetcher.TARGETS[target][1]}",
                     "source": f"fm-editor-0.1.0-source-{COMMIT}.tar.gz"}
            sums = []
            for kind, name in names.items():
                content = target.encode() if kind == "raw" else b"source"
                if kind == "raw" and self.bad_raw_pin:
                    content += b"changed"
                (out / name).write_bytes(content)
                digest = "0" * 64 if self.bad_checksum == kind else sha(content)
                sums.append(f"{digest}  {name}\n")
            (out / "SHA256SUMS").write_text("".join(sums))
            if self.extra:
                (out / "unexpected").write_text("not allowed")
            return ""
        raise AssertionError(f"Unexpected GitHub operation: {args}")

    def canonicalize(self, raw, destination, **kwargs):
        target = kwargs["expected_target"]
        self.assertEqual(kwargs, {"expected_target": target, "expected_version": "0.1.0",
                                  "expected_commit": COMMIT, "expected_executable_sha256": "b" * 64})
        self.assertFalse(destination.exists())
        destination.write_bytes(("changed" if self.bad_canonical else "canonical:" + target).encode())
        return {}

    def fetch(self):
        return fetcher.fetch("v0.1.0", "barak14/fm-editor", self.output)

    def assert_clean_failure(self, message):
        with self.assertRaisesRegex(ValueError, message):
            self.fetch()
        self.assertFalse(self.output.exists())
        self.assertEqual(list(self.root.glob(".native-fetch-*")), [])

    def test_fetch_uses_pinned_actions_and_only_canonical_outputs(self):
        info = self.fetch()
        self.assertEqual(info["commit"], COMMIT)
        self.assertEqual(info["build_run"], RUN)
        self.assertEqual(len(info["archives"]), 2)
        self.assertEqual(self.canonicalizer.call_count, 2)
        self.assertEqual(json.loads((self.output / "INPUTS.json").read_text()), info)
        self.assertEqual({p.name for p in self.output.iterdir()}, set(info["archives"]) | {"SHA256SUMS", "INPUTS.json"})
        for name, digest in info["archives"].items():
            self.assertEqual(fetcher.verify_checksum(self.output / name, self.output / "SHA256SUMS"), digest)
        self.assertEqual([call[:2] for call in self.calls],
                         [("api", "repos/barak14/fm-editor/commits/v0.1.0"),
                          ("run", "view"), ("run", "download"), ("run", "download")])
        self.assertEqual(list(self.root.glob(".native-fetch-*")), [])

    def test_bad_input_never_uses_network(self):
        for tag, repository in [("v0.1.0;bad", "barak14/fm-editor"), ("v0.1.0", "../bad"),
                                ("0.1.0", "barak14/fm-editor"), ("v0.1.0", "-bad/repo")]:
            with self.subTest(tag=tag, repository=repository), self.assertRaises(ValueError):
                fetcher.fetch(tag, repository, self.output)
        self.assertEqual(self.calls, [])

    def test_invalid_manifest_never_uses_network(self):
        original = json.dumps(self.manifest)
        mutations = [lambda m: m.update(tag="v9.9.9"), lambda m: m.update(version="9.9.9"),
                     lambda m: m.update(commit="a" * 39), lambda m: m.update(build_run="123"),
                     lambda m: m.update(build_run=True), lambda m: m.update(build_run=0),
                     lambda m: m["targets"].pop(next(iter(fetcher.TARGETS))),
                     lambda m: m["targets"].update(unknown={})]
        for field, value in [("archive_sha256", "f" * 63), ("executable_sha256", None),
                             ("canonical_sha256", "z" * 64), ("canonical_name", "../escape.zip")]:
            mutations.append(lambda m, field=field, value=value: m["targets"][next(iter(fetcher.TARGETS))].update({field: value}))
        for mutate in mutations:
            self.manifest = json.loads(original)
            mutate(self.manifest)
            self.save_manifest()
            self.assert_clean_failure("manifest")
        self.assertEqual(self.calls, [])

    def test_wrong_tag_commit_and_run_identity_are_rejected(self):
        self.commit = "c" * 40
        self.assert_clean_failure("pinned full commit")
        self.commit = COMMIT
        for field, value in [("headSha", "c" * 40), ("headSha", COMMIT[:12]),
                             ("status", "in_progress"), ("conclusion", "failure"),
                             ("workflowName", "Other workflow")]:
            original = self.build[field]
            self.build[field] = value
            self.assert_clean_failure("completed successful")
            self.build[field] = original
        self.canonicalizer.assert_not_called()
        self.assertFalse(any(call[:2] == ("run", "download") for call in self.calls))

    def test_both_checksums_and_raw_manifest_pin_are_required(self):
        for kind in ("raw", "source"):
            self.bad_checksum = kind
            self.assert_clean_failure("SHA-256")
        self.bad_checksum = None
        self.bad_raw_pin = True
        self.assert_clean_failure("release manifest pin")
        self.canonicalizer.assert_not_called()

    def test_canonical_pin_and_identity_failures_are_rejected(self):
        self.bad_canonical = True
        self.assert_clean_failure("Canonical archive SHA-256")
        self.bad_canonical = False
        self.canonicalizer.side_effect = ValueError("wrong executable or identity")
        self.assert_clean_failure("wrong executable or identity")

    def test_missing_expired_or_unexpected_artifacts_have_offline_guidance(self):
        for mode in ("missing", "expired", "extra"):
            setattr(self, mode, True)
            self.assert_clean_failure("bundled verified inputs from packaging-source.*do not bypass")
            setattr(self, mode, False)
        self.canonicalizer.assert_not_called()

    def test_existing_output_is_not_overwritten(self):
        self.output.mkdir()
        (self.output / "sentinel").write_text("keep")
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.fetch()
        self.assertEqual((self.output / "sentinel").read_text(), "keep")
        self.assertEqual(self.calls, [])

    def test_existing_dangling_symlink_is_preserved(self):
        self.output.symlink_to(self.root / "absent", target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.fetch()
        self.assertTrue(self.output.is_symlink())
        self.assertEqual(self.calls, [])

    def test_failed_output_copy_cleans_only_owned_output(self):
        sentinel = self.root / "sentinel"
        sentinel.write_text("keep")
        with patch.object(fetcher.shutil, "copyfileobj", side_effect=OSError("disk full")):
            with self.assertRaisesRegex(OSError, "disk full"):
                self.fetch()
        self.assertFalse(self.output.exists())
        self.assertEqual(sentinel.read_text(), "keep")
        self.assertEqual(list(self.root.glob(".native-fetch-*")), [])


if __name__ == "__main__":
    unittest.main()
