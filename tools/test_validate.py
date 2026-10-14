"""Unit tests for checks in validate.py: python -m unittest discover -s tools -p "test_*.py"."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import validate  # noqa: E402


class Duplicates(unittest.TestCase):
    def test_a_key_given_twice_is_refused_at_any_depth(self):
        found = validate.duplicates("a: 1\na: 2\nb: {c: 1, c: 2}\nd: [{e: 1, e: 1}]\n")
        self.assertEqual(found, [
            "line 2: key a is already given on line 1",
            "line 3: key b.c is already given on line 3",
            "line 4: key d[0].e is already given on line 4",
        ])

    def test_an_anchor_defined_twice_is_refused(self):
        found = validate.duplicates("a: &x 1\nb: &x 2\nc: *x\n")
        self.assertEqual(found, ["line 2: anchor &x is already defined on line 1"])

    def test_aliases_and_merges_are_not_duplicates(self):
        text = "a: &x {b: 1}\nc: *x\nd: {<<: *x, b: 2}\ne: {f: 1}\ng: {f: 1}\n"
        self.assertEqual(validate.duplicates(text), [])


if __name__ == "__main__":
    unittest.main()
