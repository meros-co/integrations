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


class Binary(unittest.TestCase):
    PARAMS = {"file": {"type": "binary", "required": True}, "name": {"type": "string", "required": True}}

    def test_a_binary_parameter_is_sent_as_bytes_over_http(self):
        send = {"method": "POST", "path": "/u", "body": {"binary": "file"}}
        self.assertEqual(validate.binary_checks(send, self.PARAMS, "c", "http"), [])
        form = {"method": "POST", "path": "/u",
                "multipart": [{"name": "f", "binary": "file", "filename": "{name}"}]}
        self.assertEqual(validate.binary_checks(form, self.PARAMS, "c", "http"), [])
        self.assertEqual(validate.binary_checks(send, self.PARAMS, "c", "line-tcp"),
                         ["c: binary bodies are for the http transport"])

    def test_a_binary_parameter_is_never_templated_and_always_given(self):
        errors = validate.check_template("http-body", "{file}", self.PARAMS, {}, "c")
        self.assertTrue(any("never in a template" in e for e in errors), errors)
        unsent = validate.binary_checks({"method": "POST", "path": "/u"}, self.PARAMS, "c", "http")
        self.assertEqual(unsent, ["c: binary 'file' is sent only as a body or a multipart part"])
        optional = {"file": {"type": "binary"}}
        send = {"method": "POST", "path": "/u", "body": {"binary": "file"}}
        self.assertEqual(validate.binary_checks(send, optional, "c", "http"), ["c: binary 'file' must be required"])
        self.assertEqual(validate.binary_checks({"method": "POST", "path": "/u", "body": {"binary": "name"}},
                                                self.PARAMS, "c", "http")[0],
                         "c: 'name' sent as bytes is not a binary parameter")

    def test_multipart_has_no_body_and_message_headers_are_the_specs(self):
        send = {"method": "POST", "path": "/u", "body": "x", "headers": {"Content-Type": "a"},
                "multipart": [{"name": "f", "binary": "file"}]}
        errors = validate.binary_checks(send, self.PARAMS, "c", "http")
        self.assertIn("c: multipart goes without body and content_type", errors)
        self.assertIn("c: header Content-Type is set by the core, not by the spec", errors)


class Templates(unittest.TestCase):
    def test_an_escaped_brace_is_no_placeholder(self):
        self.assertEqual(validate.check_template("text", r"\{VC}", {}, {}, "c"), [])
        self.assertTrue(validate.check_template("text", "{VC}", {}, {}, "c"))

    def test_an_outer_elements_place_is_a_placeholder(self):
        names = [m.group(1) for m in validate.PLACEHOLDER.finditer("g.{^index}.s.{index:+1}.{^^key:key}")]
        self.assertEqual(names, ["^index", "index", "^^key"])


if __name__ == "__main__":
    unittest.main()
