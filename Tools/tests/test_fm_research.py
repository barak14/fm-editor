"""Host-independent tests for the field-research helper; no game or MCP server required."""

import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "fm_research", Path(__file__).parents[1] / "fm-research.py"
)
fm_research = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fm_research)


class ParsingTests(unittest.TestCase):
    def test_parse_dump_reads_both_hex_groups_and_ignores_ascii(self):
        text = (
            "0000  48 88 A7 45 01 00 00 00  EC 02 00 00 93 03 00 00  H..E............\n"
            "0010  E0 25 12 56 00 00 00 00                           .%.V....\n"
        )
        data = fm_research.parse_dump(text)
        self.assertEqual(len(data), 24)
        self.assertEqual(data[:2], b"\x48\x88")
        self.assertEqual(data[16:20], b"\xE0\x25\x12\x56")

    def test_date_decoding_strips_flags_and_rejects_garbage(self):
        # Day 184 of 2023 with FM's flag bits above the low nine bits.
        self.assertEqual(fm_research.date_text(184 | (0xD << 9), 2023), "2023-07-03 flags=0xd")
        self.assertEqual(fm_research.date_text(1, 1900), "1900-01-01")
        self.assertTrue(fm_research.date_text(0, 0).startswith("raw("))

    def test_signed_decoding(self):
        self.assertEqual(fm_research.decode(None, "i32", b"\xff\xff\xff\xff"), -1)
        self.assertEqual(fm_research.decode(None, "u8", b"\x05"), 5)

    def test_matches_handles_numbers_dates_and_missing(self):
        self.assertTrue(fm_research.matches(7500000, "7500000"))
        self.assertTrue(fm_research.matches(-1, "-1"))
        self.assertTrue(fm_research.matches("2023-07-03 flags=0xd", "2023-07-03"))
        self.assertTrue(fm_research.matches(None, "-"))
        self.assertFalse(fm_research.matches(197000, "195000"))
        self.assertIsNone(fm_research.matches(3, None))


class LeadTests(unittest.TestCase):
    def test_translate_uses_nearest_anchor_shifts(self):
        anchors = [(0x200, 0x100), (0x300, 0x320)]
        self.assertEqual(fm_research.translate(0x250, anchors), [0x150, 0x270])
        self.assertEqual(fm_research.translate(0x350, anchors), [0x370])
        self.assertEqual(fm_research.translate(0x10, []), [])

    def test_schema_parser_reads_repository_fields(self):
        fields, subs = fm_research.schema("fm24")
        self.assertEqual(fields["p_weight"]["offset"], 0x14C)
        self.assertEqual(fields["p_contract_expiry"]["sub"], "contract")
        self.assertEqual(subs[("player", "contract")], (None, 0x340))
        self.assertEqual(subs[("player", "club")], ("team", 0x30))


if __name__ == "__main__":
    unittest.main()
