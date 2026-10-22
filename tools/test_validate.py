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


class QuietRules(unittest.TestCase):
    def doc(self, transport, *rules):
        return {"transport": {"type": transport}, "state": {"done": {"type": "bool"}},
                "telemetry": {"updates": list(rules)}}

    def test_a_quiet_rule_stands_alone_on_a_stream(self):
        quiet = {"quiet_ms": 1000, "state": {"done": "true"}}
        self.assertEqual(validate.telemetry_checks(self.doc("line-tcp", quiet)), [])
        self.assertEqual(validate.telemetry_checks(self.doc("http", quiet)),
                         ["telemetry.updates[0]: quiet_ms needs a line-tcp, line-serial, osc-tcp or ws transport"])
        self.assertIn("telemetry.updates: at most one quiet_ms rule",
                      validate.telemetry_checks(self.doc("ws", quiet, quiet)))
        mixed = dict(quiet, match="^END$", then_send=["X"])
        self.assertIn("telemetry.updates[0]: a quiet_ms rule has no match, then_send",
                      validate.telemetry_checks(self.doc("line-tcp", mixed)))


class ExtensionCommands(unittest.TestCase):
    DOC = {"id": "x", "transport": {"type": "line-tcp"}, "extension": "ultimatte-preset-archive",
           "models": [{"id": "m", "supports": []}]}

    def commands(self, **extra):
        own = validate.EXTENSION_COMMANDS["ultimatte-preset-archive"]
        commands = {name: {"returns": "value"} for name in own}
        commands.update(extra)
        return dict(self.DOC, commands=commands)

    def test_the_extensions_commands_need_no_message(self):
        errors = validate.cross_field_checks(self.commands(ping={"send": "PING:"}), "x")
        self.assertFalse([e for e in errors if "send" in e or "extension" in e or "extractor" in e], errors)

    def test_other_commands_still_need_one_and_the_extensions_may_not_have_one(self):
        doc = self.commands(ping={"returns": "ack"}, import_preset={"send": "X", "returns": "ack"})
        errors = validate.cross_field_checks(doc, "x")
        self.assertIn("commands.ping: a spec-driven command requires 'send'", errors)
        self.assertTrue(any("import_preset" in e and "must not declare 'send'" in e for e in errors), errors)

    def test_every_command_the_extension_answers_is_declared(self):
        doc = dict(self.DOC, commands={"ping": {"send": "PING:"}})
        errors = validate.cross_field_checks(doc, "x")
        self.assertTrue(any("answers 'export_presets'" in e for e in errors), errors)


class Serial(unittest.TestCase):
    SETTINGS = {
        "serial_port": {"type": "string", "required": True},
        "baud_rate": {"type": "enum", "values": ["9600", "38400"], "default": "9600"},
        "data_bits": {"type": "int", "min": 8, "max": 8, "default": 8},
        "parity": {"type": "enum", "values": ["none"], "default": "none"},
        "stop_bits": {"type": "int", "min": 1, "max": 1, "default": 1},
        "flow_control": {"type": "enum", "values": ["none", "xon-xoff"], "default": "none"},
    }
    PORT = {"port": None, "protocol": "serial", "role": "control", "setting": "serial_port", "note": "RS-232"}

    def doc(self, settings=None, ports=None):
        return {"transport": {"type": "line-serial", "framing": "terminated"},
                "settings": self.SETTINGS if settings is None else settings,
                "ports": [self.PORT] if ports is None else ports}

    def test_a_serial_device_with_its_settings_is_valid(self):
        self.assertEqual(validate.serial_checks(self.doc()) + validate.port_checks(self.doc()), [])

    def test_each_serial_setting_is_declared_with_a_default_the_core_can_use(self):
        settings = dict(self.SETTINGS, parity={"type": "enum", "values": ["mark"], "default": "mark"},
                        stop_bits={"type": "int", "min": 1, "max": 3})
        del settings["flow_control"]
        errors = validate.serial_checks(self.doc(settings))
        self.assertIn("settings.flow_control: a device on a serial port declares it", errors)
        self.assertIn("settings.parity: an enum of ['even', 'none', 'odd']", errors)
        self.assertIn("settings.stop_bits: needs the device's default", errors)
        self.assertIn("settings.stop_bits: an int within 1 to 2 (give min and max)", errors)

    def test_a_delimited_framing_needs_a_close(self):
        for close in (None, "", "  ", "\r\n"):
            t = {"type": "line-tcp", "port": 1, "framing": "delimited", "open": "<"}
            if close is not None:
                t["close"] = close
            doc = {"transport": t, "ports": [{"port": 1, "protocol": "tcp", "role": "control"}]}
            self.assertIn("transport.close: framing delimited needs a close that is not only whitespace",
                          validate.port_checks(doc))

    def test_a_line_serial_transport_needs_a_serial_control_entry(self):
        doc = self.doc(ports=[{"port": 23, "protocol": "tcp", "role": "control"}])
        self.assertIn("ports: a line-serial transport needs a serial control entry", validate.port_checks(doc))
        bad = dict(self.PORT, port=1, setting="port")
        errors = validate.serial_checks(self.doc(ports=[bad]))
        self.assertIn("ports[0]: a serial port has no number (port: null)", errors)
        self.assertIn("ports[0]: a serial port's setting is serial_port", errors)


if __name__ == "__main__":
    unittest.main()
