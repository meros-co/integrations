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
                         ["c: binary bodies are for the http transport or an endpoint"])
        # An HTTP endpoint beside a line transport takes them too.
        self.assertEqual(validate.binary_checks(dict(send, endpoint="files"), self.PARAMS, "c", "line-tcp"), [])

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


class Saves(unittest.TestCase):
    PARAMS = {"to": {"type": "save", "required": True}, "id": {"type": "int", "required": True}}

    def test_a_save_parameter_is_named_by_one_http_request(self):
        good = {"send": {"method": "GET", "path": "/r/{id}", "save": "to"}, "returns": "file"}
        self.assertEqual(validate.save_checks(good, self.PARAMS, "c", "http"), [])
        on_line = {"send": {"method": "GET", "path": "/r", "save": "to"}}
        self.assertEqual(validate.save_checks(on_line, self.PARAMS, "c", "line-tcp"),
                         ["c: save and pages are for HTTP requests"])
        on_endpoint = {"send": {"endpoint": "rest", "method": "GET", "path": "/r", "save": "to"}}
        self.assertEqual(validate.save_checks(on_endpoint, self.PARAMS, "c", "line-tcp"), [])

    def test_what_a_save_may_not_be(self):
        unsaved = {"send": {"method": "GET", "path": "/r"}, "returns": "file"}
        self.assertEqual(validate.save_checks(unsaved, self.PARAMS, "c", "http"), [
            "c: save parameter 'to' is named by no request's save",
            "c: returns 'file' needs a request with save",
        ])
        both = {"send": {"method": "GET", "path": "/r", "save": "to", "pages": {"items": "$", "page": "p"}}}
        self.assertIn("c: a request has pages or save, not both",
                      validate.save_checks(both, self.PARAMS, "c", "http"))
        optional = {"to": {"type": "save"}}
        self.assertEqual(validate.save_checks({"send": {"method": "GET", "path": "/r", "save": "to"}},
                                              optional, "c", "http"),
                         ["c: save 'to' must be required"])
        templated = validate.check_template("http-path", "/r/{to}", self.PARAMS, {}, "c")
        self.assertEqual(templated, ["c: save 'to' names where an answer is saved, never sent in a template"])


class Templates(unittest.TestCase):
    def test_an_escaped_brace_is_no_placeholder(self):
        self.assertEqual(validate.check_template("text", r"\{VC}", {}, {}, "c"), [])
        self.assertTrue(validate.check_template("text", "{VC}", {}, {}, "c"))

    def test_a_doubled_backslash_leaves_the_placeholder(self):
        names = [m.group(1) for m in validate.PLACEHOLDER.finditer(r"P\LOAD\\{ch}\\{scene}\\ \\\{no}")]
        self.assertEqual(names, ["ch", "scene"])

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
                         ["telemetry.updates[0]: quiet_ms needs a line-tcp, osc-tcp or ws transport"])
        self.assertIn("telemetry.updates: at most one quiet_ms rule",
                      validate.telemetry_checks(self.doc("ws", quiet, quiet)))
        mixed = dict(quiet, match="^END$", then_send=["X"])
        self.assertIn("telemetry.updates[0]: a quiet_ms rule has no match, then_send",
                      validate.telemetry_checks(self.doc("line-tcp", mixed)))


class NullStates(unittest.TestCase):
    def doc(self, *rules):
        return {"transport": {"type": "line-tcp"}, "state": {"tx": {"type": "string"}},
                "telemetry": {"updates": list(rules)}}

    def test_a_rule_never_writes_null(self):
        self.assertEqual(validate.telemetry_checks(self.doc({"match": "^X$", "state": {"tx": None}})),
                         ["telemetry.updates[0]: 'tx' is written as null; remove with "
                          "{delete: true}, or write 'none' as an explicit value"])
        mapped = {"match": "^(.)$", "state": {"tx": {"value": "{1}", "map": {"0": None, "1": "a"}}}}
        self.assertEqual(validate.telemetry_checks(self.doc(mapped)),
                         ["telemetry.updates[0]: 'tx' maps a value to null; write 'none' as an explicit value"])
        fields = {"header": "^S:$", "fields": {"t": {"path": "tx", "map": {"-": None}}}}
        self.assertEqual(validate.telemetry_checks(self.doc(fields)),
                         ["telemetry.updates[0]: 'tx' maps a value to null; write 'none' as an explicit value"])

    def test_explicit_values_and_removals_pass(self):
        ok = {"match": "^(.)$", "state": {"tx": {"value": "{1}", "map": {"0": "none", "1": "a"}}}}
        self.assertEqual(validate.telemetry_checks(self.doc(ok)), [])
        gone = {"match": "^X$", "state": {"tx": {"delete": True}}}
        self.assertEqual(validate.telemetry_checks(self.doc(gone)), [])


class MaMscEthernet(unittest.TestCase):
    DOC = {"id": "x", "transport": {"type": "line-tcp"}, "extension": "ma-msc-ethernet",
           "models": [{"id": "m", "supports": []}]}

    def errors(self, settings, ports):
        doc = dict(self.DOC, settings=settings, ports=ports)
        return [e for e in validate.cross_field_checks(doc, "x.yaml") if "ma-msc-ethernet" in e]

    def test_the_port_setting_and_its_listener_are_required(self):
        port = {"port": 6004, "protocol": "udp", "role": "feedback", "listener": "core", "setting": "msc_port"}
        self.assertEqual(self.errors({"msc_port": {"type": "int"}}, [port]), [])
        self.assertEqual(len(self.errors({}, [port])), 1)
        self.assertEqual(len(self.errors({"msc_port": {"type": "int"}}, [])), 1)


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


class Framing(unittest.TestCase):
    def test_a_delimited_framing_needs_a_close(self):
        for close in (None, "", "  ", "\r\n"):
            t = {"type": "line-tcp", "port": 1, "framing": "delimited", "open": "<"}
            if close is not None:
                t["close"] = close
            doc = {"transport": t, "ports": [{"port": 1, "protocol": "tcp", "role": "control"}]}
            self.assertIn("transport.close: framing delimited needs a close that is not only whitespace",
                          validate.port_checks(doc))


class Pacing(unittest.TestCase):
    def doc(self, pacing):
        t = {"type": "line-tcp", "port": 23, "pacing": pacing}
        return {"transport": t, "models": [{"id": "a"}, {"id": "b"}]}

    def test_a_pacing_block_names_known_models_and_regexes(self):
        good = {"min_interval_ms": 40, "by_model": [{"models": ["b"], "min_interval_ms": 16}],
                "groups": {"pan_tilt": {"match": "^#P", "min_interval_ms": 40,
                                        "by_model": [{"models": ["a"], "min_interval_ms": 16}]}}}
        self.assertEqual(validate.pacing_checks(self.doc(good)), [])
        bad = {"by_model": [{"models": ["c"], "min_interval_ms": 1}, {"models": ["a", "a"], "min_interval_ms": 2}],
               "groups": {"g": {"match": "(", "min_interval_ms": 1}, "h": {"match": "(?=x)", "after_ms": 1}}}
        errors = validate.pacing_checks(self.doc(bad))
        at = "transport.pacing"
        self.assertIn(f"{at}.by_model: no model 'c'", errors)
        self.assertIn(f"{at}.by_model: model 'a' is named twice", errors)
        self.assertTrue(any(e.startswith(f"{at}.groups.g.match") for e in errors))
        self.assertIn(f"{at}.groups.h.match: lookaround and backreferences are not RE2", errors)


class LineTls(unittest.TestCase):
    def doc(self):
        return {
            "transport": {"type": "line-tcp", "port": 9090, "scheme": {"setting": "scheme"},
                          "tls_port": 9091, "accept_invalid_certs": True},
            "settings": {"scheme": {"type": "enum", "values": ["tcp", "tls"], "default": "tls"}},
            "ports": [{"port": 9091, "protocol": "tls", "role": "control", "when": "scheme tls"},
                      {"port": 9090, "protocol": "tcp", "role": "control", "when": "scheme tcp"}],
        }

    def errors(self, doc):
        return validate.session_checks(doc) + [
            e for e in validate.cross_field_checks(doc, "x.yaml") if "scheme" in e or "accept" in e]

    def test_a_line_transport_may_choose_tls(self):
        self.assertEqual(self.errors(self.doc()), [])

    def test_what_tls_on_a_line_transport_needs(self):
        doc = self.doc()
        doc["ports"] = doc["ports"][1:]
        self.assertIn("ports: no tls control entry for tls_port 9091", self.errors(doc))
        doc = self.doc()
        doc["settings"]["scheme"]["values"] = ["http", "https"]
        self.assertTrue(any("enum of tcp and tls" in e for e in self.errors(doc)))
        doc = self.doc()
        doc["transport"] = {"type": "line-tcp", "port": 9090, "accept_invalid_certs": True}
        self.assertTrue(any("accept_invalid_certs needs a TLS scheme" in e for e in self.errors(doc)))


class EndpointAuth(unittest.TestCase):
    def doc(self, auth, settings=None):
        return {"transport": {"type": "line-tcp", "port": 3005, "endpoints": {"rest": {"port": 1234, "auth": auth}}},
                "settings": settings if settings is not None else {
                    "username": {"type": "string"}, "password": {"type": "string", "secret": True},
                    "rest_auth": {"type": "enum", "values": ["basic", "none"]}}}

    def test_beside_a_line_transport_an_endpoint_takes_none_or_its_own_basic(self):
        self.assertEqual(validate.endpoint_checks(self.doc("basic")), [])
        self.assertEqual(validate.endpoint_checks(self.doc("none")), [])
        self.assertEqual(validate.endpoint_checks(self.doc({"setting": "rest_auth"})), [])
        self.assertEqual(validate.endpoint_checks(self.doc("inherit")),
                         ["transport.endpoints.rest.auth is none or basic beside a line-tcp transport"])

    def test_basic_needs_the_username_and_password_settings(self):
        self.assertEqual(validate.endpoint_checks(self.doc("basic", {})),
                         ["transport.endpoints.rest.auth basic needs a 'username' setting",
                          "transport.endpoints.rest.auth basic needs a 'password' setting"])



class Selections(unittest.TestCase):
    def doc(self, select, setting_extra=None):
        setting = {"type": "string", "select": select, **(setting_extra or {})}
        return {
            "settings": {"thing": setting, "other": {"type": "string", "default": "x"}},
            "models": [{"id": "m", "supports": ["list_things", "start"]}],
            "commands": {
                "list_things": {"params": {"group": {"type": "string", "required": True}},
                                "send": {"method": "GET", "path": "/things"}, "returns": "value"},
                "start": {"send": {"method": "POST", "path": "/things/{settings.thing}/start"}, "returns": "ack"},
            },
            "state": {"thing.status": {"type": "string"}},
        }

    def test_a_selection_names_a_lister_that_works_before_it(self):
        good = {"command": "list_things", "id": "$.id", "params": {"group": "other"}, "clears": ["thing"]}
        self.assertEqual(validate.select_checks(self.doc(good)), [])
        errors = validate.select_checks(self.doc(good, {"required": True}))
        self.assertTrue(any("neither required" in e for e in errors))
        errors = validate.select_checks(self.doc({"command": "start", "id": "$.id"}))
        self.assertIn("settings.thing.select: 'start' names the setting it lists the choices for", errors)
        self.assertIn("settings.thing.select: 'start' must return the list (returns: value)", errors)
        bad = {"command": "list_things", "id": "$.id", "id_match": "^x$", "params": {"nope": "thing"},
               "clears": ["nothing"]}
        errors = validate.select_checks(self.doc(bad))
        self.assertIn("settings.thing.select.id_match: the id is its first group", errors)
        self.assertIn("settings.thing.select.params: 'list_things' has no parameter 'nope'", errors)
        self.assertIn("settings.thing.select.params: 'thing' is not another setting", errors)
        self.assertIn("settings.thing.select.clears: 'nothing' is no declared state", errors)

    def test_polled_is_the_cores_own_state(self):
        doc = {"state": {"polled.1": {"type": "int"}},
               "telemetry": {"poll": [{"name": "a", "send": []}, {"name": "a", "send": []}]}}
        errors = validate.polled_checks(doc)
        self.assertIn("state: polled.* is the core's own; leave it out", errors)
        self.assertIn("telemetry.poll: two groups named 'a'", errors)


if __name__ == "__main__":
    unittest.main()
