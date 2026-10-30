"""Release-input transport tests; archive validation is covered separately."""
import hashlib
import importlib.util
import json
from pathlib import Path
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


class FetchNativeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / "inputs"
        self.bad_digest = False
        self.calls = []

    def gh(self, *args):
        self.calls.append(args)
        if args[0] == "api":
            return COMMIT
        if args[:2] == ("release", "view"):
            return json.dumps({"tagName": "v0.1.0"})
        if args[:2] == ("release", "download"):
            out = Path(args[args.index("--dir") + 1])
            out.mkdir()
            sums = []
            for index, value in enumerate(args):
                if value != "--pattern":
                    continue
                name = args[index + 1]
                if name == "SHA256SUMS":
                    continue
                content = name.encode()
                (out / name).write_bytes(content)
                digest = "0" * 64 if self.bad_digest else hashlib.sha256(content).hexdigest()
                sums.append(f"{digest}  {name}\n")
            (out / "SHA256SUMS").write_text("".join(sums))
            return ""
        raise AssertionError(args)

    def test_fetch_uses_exact_tag_and_read_only_operations(self):
        with patch.object(fetcher, "gh", side_effect=self.gh), patch.object(fetcher, "extract_portable", return_value={}) as extract:
            info = fetcher.fetch("v0.1.0", "barak14/fm-editor", self.output)
        self.assertEqual(info["commit"], COMMIT)
        self.assertEqual(len(info["archives"]), 3)
        self.assertEqual(extract.call_count, 2)
        for call in extract.call_args_list:
            self.assertEqual(call.kwargs["expected_commit"], COMMIT)
            self.assertEqual(call.kwargs["expected_version"], "0.1.0")
        self.assertEqual(json.loads((self.output / "INPUTS.json").read_text()), info)
        self.assertEqual(len(list(self.output.iterdir())), 5)
        self.assertTrue(all(call[0] == "api" or call[:2] in [("release", "view"), ("release", "download")] for call in self.calls))

    def test_bad_input_never_uses_network(self):
        for tag, repository in [("v0.1.0;bad", "barak14/fm-editor"), ("v0.1.0", "../bad"), ("0.1.0", "barak14/fm-editor")]:
            with patch.object(fetcher, "gh") as gh:
                with self.assertRaises(ValueError):
                    fetcher.fetch(tag, repository, self.output)
                gh.assert_not_called()
        self.assertFalse(self.output.exists())

    def test_bad_checksum_or_portable_identity_leaves_no_output(self):
        self.bad_digest = True
        with patch.object(fetcher, "gh", side_effect=self.gh), patch.object(fetcher, "extract_portable") as extract:
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                fetcher.fetch("v0.1.0", "barak14/fm-editor", self.output)
            extract.assert_not_called()
        self.assertFalse(self.output.exists())
        self.bad_digest = False
        with patch.object(fetcher, "gh", side_effect=self.gh), patch.object(fetcher, "extract_portable", side_effect=ValueError("wrong identity")):
            with self.assertRaisesRegex(ValueError, "wrong identity"):
                fetcher.fetch("v0.1.0", "barak14/fm-editor", self.output)
        self.assertFalse(self.output.exists())

    def test_existing_output_is_not_overwritten(self):
        self.output.mkdir()
        (self.output / "sentinel").write_text("keep")
        with patch.object(fetcher, "gh", side_effect=self.gh):
            with self.assertRaisesRegex(ValueError, "overwrite"):
                fetcher.fetch("v0.1.0", "barak14/fm-editor", self.output)
        self.assertEqual((self.output / "sentinel").read_text(), "keep")
        self.assertFalse(any(call[:2] == ("release", "download") for call in self.calls))


if __name__ == "__main__":
    unittest.main()
