#!/usr/bin/env python3
"""Validate every device spec against the JSON Schema, plus the cross-field
rules a schema cannot express.

    python tools/validate.py

Exits non-zero on any failure, so it can gate CI.
"""
from __future__ import annotations

import glob
import json
import re
import sys
from pathlib import Path

try:
    import yaml
    import jsonschema
except ImportError as exc:  # pragma: no cover
    sys.exit(f"missing dependency: {exc}. pip install pyyaml jsonschema")

ROOT = Path(__file__).resolve().parent.parent

# libyaml's parser where PyYAML has it, for the duplicate checks.
FAST_LOADER = getattr(yaml, "CSafeLoader", yaml.SafeLoader)

# {name} or {name:directive:directive...}; SPEC.md §4, "Templates".
PLACEHOLDER = re.compile(r"""(?<!\\)\{(\^*[A-Za-z0-9_][A-Za-z0-9_.]*)((?::[^{}:"\s]+)*)\}""")

# The closed directive set, keyed by the parameter types each one accepts.
DIRECTIVES: list[tuple[re.Pattern[str], set[str]]] = [
    (re.compile(r"^0\d+d$"), {"int"}),                  # zero-padded integer
    (re.compile(r"^[+-]\d+$"), {"int"}),                # integer offset
    (re.compile(r"^signed$"), {"int", "float"}),        # explicit sign
    (re.compile(r"^bit\.([0-9]|[1-5][0-9]|6[0-3])$"), {"int"}),  # one bit, true or false
    (re.compile(r"^\.\d+f$"), {"float"}),               # fixed decimals
    (re.compile(r"^(on_off|bool01|bool10)$"), {"bool"}),
    (re.compile(r"^(upper|lower|json|url)$"), {"string", "enum"}),
    (re.compile(r"^url_decode$"), {"string", "int"}),      # a percent-encoded name a device sends
    (re.compile(r"^(x|0\d+x)$"), {"int"}),              # lowercase hexadecimal digits, zero-padded
    (re.compile(r"^hex$"), {"string", "int"}),            # hexadecimal digits a device sends, as a number
    (re.compile(r"^(md5|sha256)$"), {"string"}),         # a hashed password, lowercase hex
    (re.compile(r"^(to|from)\.[a-z0-9_]+$"), {"int", "float"}),  # a named conversion
    (re.compile(r"^map\.[a-z0-9_]+$"), {"enum", "bool", "int", "string"}),  # a value table
]

CONVERSION = re.compile(r"^(to|from)\.([a-z0-9_]+)$")
MAP = re.compile(r"^map\.([a-z0-9_]+)$")

# Types that cannot carry characters needing escaping, so they are safe in a
# raw_query, which is sent without encoding.
RAW_SAFE_TYPES = {"int", "float", "bool", "enum"}


def template_strings(send) -> list[tuple[str, str]]:
    """Every templated string in a send block, as (context, text).

    Context is one of: text, osc-address, osc-arg-<type>, http-path,
    http-query, http-raw-query, http-body.
    """
    out: list[tuple[str, str]] = []
    items = send if isinstance(send, list) else [send]
    for item in items:
        if isinstance(item, str):
            out.append(("text", item))
        elif isinstance(item, dict) and "message" in item:
            out.append(("text", item["message"]))
        elif isinstance(item, dict) and "address" in item:
            out.append(("osc-address", item["address"]))
            for arg in item.get("args", []):
                out.append((f"osc-arg-{arg['type']}", arg["value"]))
        elif isinstance(item, dict) and "method" in item:
            out.append(("http-path", item["path"]))
            for value in (item.get("query") or {}).values():
                out.append(("http-query", value))
            if "raw_query" in item:
                out.append(("http-raw-query", item["raw_query"]))
            if isinstance(item.get("body"), str):
                out.append(("http-body", item["body"]))
            if "content_type" in item:
                out.append(("http-header", item["content_type"]))
            for value in (item.get("headers") or {}).values():
                out.append(("http-header", value))
            for part in item.get("multipart") or []:
                for key in ("name", "value", "filename", "content_type"):
                    if key in part:
                        out.append(("http-body", part[key]))
    return out


def binary_refs(send) -> list[str]:
    """The parameters a send block sends as bytes: `body: {binary: name}`
    and multipart parts' `binary`."""
    out: list[str] = []
    for item in send if isinstance(send, list) else [send]:
        if not isinstance(item, dict):
            continue
        body = item.get("body")
        if isinstance(body, dict) and "binary" in body:
            out.append(body["binary"])
        out += [p["binary"] for p in item.get("multipart") or [] if "binary" in p]
    return out


def binary_checks(send, params: dict, where: str, transport_type) -> list[str]:
    """SPEC.md §4, "Binary request bodies": a binary parameter is sent only
    as bytes, over HTTP, and is always present; multipart has no body."""
    errors: list[str] = []
    refs = binary_refs(send)
    for item in send if isinstance(send, list) else [send]:
        if isinstance(item, dict) and "multipart" in item and ("body" in item or "content_type" in item):
            errors.append(f"{where}: multipart goes without body and content_type")
        if isinstance(item, dict) and item.get("headers"):
            for name in item["headers"]:
                if name.lower() in ("authorization", "content-type", "host"):
                    errors.append(f"{where}: header {name} is set by the core, not by the spec")
    if refs and transport_type != "http":
        errors.append(f"{where}: binary bodies are for the http transport")
    for name in refs:
        decl = params.get(name)
        if decl is None or decl.get("type") != "binary":
            errors.append(f"{where}: '{name}' sent as bytes is not a binary parameter")
        elif not decl.get("required"):
            errors.append(f"{where}: binary '{name}' must be required")
    for name, decl in params.items():
        if decl.get("type") == "binary" and name not in refs:
            errors.append(f"{where}: binary '{name}' is sent only as a body or a multipart part")
    return errors


def check_template(
    context: str,
    text: str,
    params: dict,
    settings: dict,
    where: str,
    conditional_setting: str | None = None,
    conversions: dict | None = None,
    maps: dict | None = None,
) -> list[str]:
    """Placeholder rules from SPEC.md §4: every reference resolves, is always
    present, and uses directives valid for its type."""
    errors: list[str] = []
    for match in PLACEHOLDER.finditer(text):
        name, raw_directives = match.group(1), match.group(2)
        directives = [d for d in raw_directives.split(":") if d]

        if name.startswith("settings."):
            key = name[len("settings."):]
            decl = settings.get(key)
            if decl is None:
                errors.append(f"{where}: unknown setting '{key}'")
                continue
            always_present = (
                decl.get("required") or "default" in decl or key == conditional_setting
            )
        else:
            decl = params.get(name)
            if decl is None:
                errors.append(f"{where}: '{{{name}}}' is not a declared parameter")
                continue
            always_present = decl.get("required") or "default" in decl

        if not always_present:
            errors.append(
                f"{where}: '{{{name}}}' may be absent; a templated value must be "
                f"required or have a default (split the command instead)"
            )

        ptype = decl["type"]
        if ptype == "binary":
            errors.append(f"{where}: binary '{name}' is sent only as a body or a multipart part, never in a template")
            continue
        # A multiline string may hold line breaks: only `:json` escapes them.
        if decl.get("multiline"):
            if ptype != "string":
                errors.append(f"{where}: multiline applies to a string, not {ptype} '{name}'")
            if directives != ["json"]:
                errors.append(f"{where}: multiline '{name}' is sent only as '{{{name}:json}}'")
        for directive in directives:
            allowed = next((types for pat, types in DIRECTIVES if pat.match(directive)), None)
            if allowed is None:
                errors.append(f"{where}: unknown directive ':{directive}' on '{name}'")
            elif ptype not in allowed:
                errors.append(
                    f"{where}: directive ':{directive}' does not apply to {ptype} '{name}'"
                )

        # A value table names a declared map, is the only directive, and lists
        # every value an enum or a bool can take, so none is refused at run time.
        tables = [MAP.match(d).group(1) for d in directives if MAP.match(d)]
        if tables:
            if len(directives) != 1:
                errors.append(f"{where}: ':map.{tables[0]}' on '{name}' takes no other directive")
            table = (maps or {}).get(tables[0])
            if table is None:
                errors.append(f"{where}: ':map.{tables[0]}' names undeclared map '{tables[0]}'")
            else:
                keys = {str(k) for k in table}
                needed = {"enum": [str(v) for v in decl.get("values", [])],
                          "bool": ["true", "false"]}.get(ptype, [])
                missing = [v for v in needed if v not in keys]
                if missing:
                    errors.append(f"{where}: map '{tables[0]}' does not list {missing} of '{name}'")
            continue

        # Conversions come first, name a declared conversion, and make a
        # float: as text it needs exactly one ':.Nf' after them.
        converted = 0
        while converted < len(directives) and CONVERSION.match(directives[converted]):
            converted += 1
        for d in directives[converted:]:
            if CONVERSION.match(d):
                errors.append(f"{where}: conversion ':{d}' on '{name}' must come before other directives")
        for d in directives[:converted]:
            conv = CONVERSION.match(d).group(2)
            if conv not in (conversions or {}):
                errors.append(f"{where}: ':{d}' names undeclared conversion '{conv}'")

        # A float rendered as text has no language-neutral default form.
        is_numeric_osc_arg = context == "osc-arg-float" and match.group(0) == text
        if converted and not is_numeric_osc_arg:
            rest = directives[converted:]
            if len(rest) != 1 or not re.match(r"^\.\d+f$", rest[0]):
                errors.append(f"{where}: converted '{name}' rendered as text needs one ':.Nf' after the conversion")
        elif ptype == "float" and not is_numeric_osc_arg:
            if not any(re.match(r"^\.\d+f$", d) for d in directives):
                errors.append(f"{where}: float '{name}' rendered as text needs a ':.Nf' directive")

        if context == "http-raw-query" and ptype not in RAW_SAFE_TYPES:
            if not (ptype == "string" and (decl.get("pattern") or "url" in directives)):
                errors.append(
                    f"{where}: raw_query is sent unencoded, so string '{name}' "
                    f"needs a 'pattern' restricting it or the ':url' directive"
                )
    return errors


def duplicates(text: str) -> list[str]:
    """A mapping key given twice, at any depth, silently keeps only the last
    value, and an anchor defined twice makes each alias name whichever came
    last before it; loaders in other languages refuse both. Reject them."""
    found: list[str] = []
    anchors: dict[str, int] = {}
    for event in yaml.parse(text, Loader=FAST_LOADER):
        anchor = getattr(event, "anchor", None)
        if anchor is None or isinstance(event, yaml.AliasEvent):
            continue
        line = event.start_mark.line + 1
        if anchor in anchors:
            found.append(f"line {line}: anchor &{anchor} is already defined on line {anchors[anchor]}")
        else:
            anchors[anchor] = line
    if found:
        return found
    seen: set[int] = set()

    def walk(node, trail: str) -> None:
        if id(node) in seen:
            return
        seen.add(id(node))
        if isinstance(node, yaml.MappingNode):
            keys: dict[str, int] = {}
            for key, value in node.value:
                if isinstance(key, yaml.ScalarNode) and key.tag != "tag:yaml.org,2002:merge":
                    line = key.start_mark.line + 1
                    here = f"{trail}.{key.value}" if trail else key.value
                    if key.value in keys:
                        found.append(f"line {line}: key {here} is already given on line {keys[key.value]}")
                    else:
                        keys[key.value] = line
                    walk(value, here)
                else:
                    walk(value, trail)
        elif isinstance(node, yaml.SequenceNode):
            for i, item in enumerate(node.value):
                walk(item, f"{trail}[{i}]")

    walk(yaml.compose(text, Loader=FAST_LOADER), "")
    return found


def non_string_keys(node, trail: str = "") -> list[str]:
    """YAML 1.1 reads unquoted on/off/yes/no as booleans and bare digits as
    integers. As a mapping key that silently renames the entry — `on:` becomes
    `True:` — and loaders in other languages disagree on the result, so reject
    any key that did not load as a string."""
    found: list[str] = []
    if isinstance(node, dict):
        for key, value in node.items():
            here = f"{trail}.{key}" if trail else str(key)
            if not isinstance(key, str):
                found.append(
                    f"{here}: key loaded as {type(key).__name__} {key!r}, not a "
                    f"string; quote it (YAML reads on/off/yes/no as booleans and "
                    f"bare digits as integers)"
                )
            found += non_string_keys(value, here)
    elif isinstance(node, list):
        for i, item in enumerate(node):
            found += non_string_keys(item, f"{trail}[{i}]")
    return found


# The commands each native extension answers itself (SPEC.md, "Native
# extensions"), as its module lists them.
EXTENSION_COMMANDS = {
    "hyperdeck-ftp": {
        "list_files", "download_file", "upload_file", "delete_file", "rename_file",
        "create_folder", "cancel_transfer",
    },
    "ultimatte-preset-archive": {
        "export_presets", "import_presets", "export_preset", "import_preset",
        "export_settings", "import_settings",
    },
}


def cross_field_checks(doc: dict, path: str) -> list[str]:
    """Rules the schema cannot express on its own."""
    errors: list[str] = []
    commands: dict = doc.get("commands") or {}
    settings: dict = doc.get("settings") or {}
    native = doc.get("implementation") == "native"

    if not commands and not doc.get("state") and not doc.get("streams"):
        # A receive-only device (a tally listener) has state and no commands.
        errors.append("a device requires at least one command, declared state, or a stream")

    # Streams are published by a module: a native device, or a spec-driven one
    # through its native extension. The spec engine itself publishes none.
    if doc.get("streams") and not native and not doc.get("extension"):
        errors.append("'streams' needs implementation: native (or a native extension that publishes them)")

    if native and doc.get("extension"):
        errors.append("'extension' adds to a spec-driven device; a native device has its module")
    if native:
        # Wire behaviour lives in the Rust module; the spec is catalogue only.
        if not doc.get("reason"):
            errors.append("implementation: native requires a 'reason'")
        for key in ("transport", "on_connect", "codes", "telemetry"):
            if key in doc:
                errors.append(f"implementation: native must not declare '{key}'")
        for name, command in commands.items():
            for key in ("send", "expect"):
                if key in command:
                    errors.append(f"commands.{name}: a native command must not declare '{key}'")
    else:
        if not doc.get("transport"):
            errors.append("a spec-implemented device requires a 'transport'")
        # Commands a native extension answers itself have no message of the
        # spec's; every other command of a spec-driven device has one.
        own = EXTENSION_COMMANDS.get(doc.get("extension"), set())
        for name, command in commands.items():
            if name in own:
                for key in ("send", "expect"):
                    if key in command:
                        errors.append(f"commands.{name}: the extension '{doc['extension']}' sends it; "
                                      f"it must not declare '{key}'")
            elif "send" not in command:
                errors.append(f"commands.{name}: a spec-driven command requires 'send'")
        for name in sorted(own - set(commands)):
            errors.append(f"the extension '{doc['extension']}' answers '{name}', which the spec must declare")

    model_ids = set()
    for model in doc.get("models", []):
        model_ids.add(model["id"])

        # Every supported command must exist.
        unknown = set(model.get("supports", [])) - set(commands)
        if unknown:
            errors.append(
                f"model '{model['id']}' claims unknown command(s): {sorted(unknown)}"
            )

        # Verification is a promise about evidence — check the vectors exist.
        if model.get("verification") in ("bench", "field"):
            vector_dir = ROOT / "vectors" / doc["id"]
            if not vector_dir.is_dir() or not any(vector_dir.glob("*.yaml")):
                errors.append(
                    f"model '{model['id']}' claims verification "
                    f"'{model['verification']}' but no vectors exist in "
                    f"vectors/{doc['id']}/ — see CONTRIBUTING.md"
                )

    # A stream's models must be real models.
    for name, stream in (doc.get("streams") or {}).items():
        bad = set(stream.get("models") or []) - model_ids
        if bad:
            errors.append(f"streams.{name} references unknown model(s): {sorted(bad)}")

    # Quirks must reference real models (or the literal 'all').
    for i, quirk in enumerate(doc.get("quirks", [])):
        bad = {m for m in quirk["models"] if m != "all" and m not in model_ids}
        if bad:
            errors.append(f"quirk[{i}] references unknown model(s): {sorted(bad)}")

    conversions: dict = doc.get("conversions") or {}
    for cname, conv in conversions.items():
        points = conv.get("points", [])
        wires = [p[0] for p in points]
        values = [p[1] for p in points]
        if any(a >= b for a, b in zip(wires, wires[1:])):
            errors.append(f"conversions.{cname}: wire values must rise strictly")
        rising = all(a < b for a, b in zip(values, values[1:]))
        falling = all(a > b for a, b in zip(values, values[1:]))
        if not (rising or falling):
            errors.append(f"conversions.{cname}: values must rise or fall strictly, so the conversion inverts")

    transport_type = (doc.get("transport") or {}).get("type")

    # Templates: commands, connection setup and the probe.
    for name, command in commands.items():
        params = command.get("params") or {}
        for context, text in template_strings(command.get("send", [])):
            errors += check_template(context, text, params, settings, f"commands.{name}",
                                     conversions=conversions, maps=doc.get("maps") or {})
        if doc.get("implementation") != "native":
            errors += binary_checks(command.get("send", []), params, f"commands.{name}", transport_type)
        expect_ = command.get("expect") or {}
        if "reply_json" in expect_:
            if transport_type != "ws":
                errors.append(f"commands.{name}: expect.reply_json needs a ws transport")
            for path_, template in expect_["reply_json"].items():
                if not path_.startswith("$"):
                    errors.append(f"commands.{name}: reply_json key '{path_}' is not a JSON path")
                errors += check_template("text", template, params, settings,
                                         f"commands.{name}.expect.reply_json")
        if "reply_contains" in expect_:
            if transport_type not in ("line-tcp", "line-serial", "line-udp"):
                errors.append(f"commands.{name}: expect.reply_contains needs a line transport")
            errors += check_template("text", expect_["reply_contains"], params, settings,
                                     f"commands.{name}.expect.reply_contains")
        if expect_.get("replies", 1) != 1 and transport_type not in ("line-tcp", "line-serial", "line-udp"):
            errors.append(f"commands.{name}: expect.replies needs a line transport")
        if "convert" in expect_:
            if expect_["convert"] not in conversions:
                errors.append(f"commands.{name}: expect.convert names undeclared conversion '{expect_['convert']}'")
            if command.get("returns") != "value":
                errors.append(f"commands.{name}: expect.convert needs returns: value")

        expect = command.get("expect") or {}
        returns = command.get("returns", "ack")
        extension_own = name in EXTENSION_COMMANDS.get(doc.get("extension"), set())
        if (not native and not extension_own and returns == "value"
                and not ({"matches", "json_path", "address"} & set(expect))):
            errors.append(
                f"commands.{name}: returns 'value' needs an extractor "
                f"(matches, json_path or address); use 'fields' or 'text' otherwise"
            )
        if "address" in expect:
            errors += check_template("osc-address", expect["address"], params, settings,
                                     f"commands.{name}.expect")

    transport = doc.get("transport") or {}
    websocket = (doc.get("telemetry") or {}).get("websocket")
    schemes = [("transport", transport.get("scheme"),
                {"ws", "wss"} if transport_type == "ws"
                else {"tcp", "tls"} if transport_type == "line-tcp" else {"http", "https"})]
    if websocket:
        schemes.append(("telemetry.websocket", websocket.get("scheme"), {"ws", "wss"}))
    for where_, block in (("telemetry.websocket", websocket or {}),
                          ("telemetry.sse", (doc.get("telemetry") or {}).get("sse") or {})):
        reply = block.get("as_reply")
        if reply is None:
            continue
        if transport_type != "http":
            errors.append(f"{where_}.as_reply offers messages to path rules, which need the http transport")
        if not isinstance(reply, dict) or not {"path", "body"} <= set(reply):
            errors.append(f"{where_}.as_reply needs path and body")
        elif not str(reply.get("prefix", "/")).startswith("/"):
            errors.append(f"{where_}.as_reply.prefix starts with /")
    for where_, scheme, allowed in schemes:
        if not isinstance(scheme, dict):
            continue
        decl = settings.get(scheme.get("setting"))
        if decl is None:
            errors.append(f"{where_}.scheme names unknown setting '{scheme.get('setting')}'")
        elif decl.get("type") != "enum" or (
                set(decl.get("values", [])) - allowed
                # A websocket may follow the http transport's own scheme
                # setting: http opens ws, https wss (SPEC.md §8).
                and not (allowed == {"ws", "wss"} and not set(decl.get("values", [])) - {"http", "https"})):
            errors.append(f"{where_}.scheme's setting must be an enum of {' and '.join(sorted(allowed))}"
                          + (" (or of http and https)" if allowed == {"ws", "wss"} else ""))
    # accept_invalid_certs only means something over TLS.
    for where_, obj, default in [
            ("transport", transport, "ws" if transport_type == "ws"
             else "tcp" if transport_type == "line-tcp" else "http"),
            ("telemetry.websocket", websocket or {}, "ws")]:
        scheme = obj.get("scheme", default)
        if obj.get("accept_invalid_certs") is True and scheme in ("ws", "http", "tcp"):
            errors.append(f"{where_}.accept_invalid_certs needs a TLS scheme (wss, https, tls or a setting)")
    if websocket:
        url = websocket.get("url")
        if url is not None:
            errors += check_template("text", url, {}, settings, "telemetry.websocket.url")
            # The scheme and host are literal; only the path and query may
            # hold placeholders.
            m = re.match(r"^(wss?)://(\[[^\]]*\]|[^/?:{]+)", url)
            host = m.group(2) if m else ""
            if not m or not host:
                errors.append("telemetry.websocket.url needs a literal wss:// or ws:// host")
            elif m.group(1) == "ws" and host not in ("localhost", "127.0.0.1", "[::1]"):
                errors.append("telemetry.websocket.url: ws only to this machine; use wss")
        port = websocket.get("port")
        if isinstance(port, dict):
            decl = settings.get(port.get("setting"))
            if decl is None:
                errors.append(f"telemetry.websocket.port names unknown setting '{port.get('setting')}'")
            elif decl.get("type") != "int":
                errors.append("telemetry.websocket.port's setting must be an int")
        for i, text in enumerate(websocket.get("send", []) if isinstance(websocket.get("send"), list)
                                 else [websocket.get("send")] if websocket.get("send") else []):
            errors += check_template("text", text, {}, settings, f"telemetry.websocket.send[{i}]")
    auth = transport.get("auth")
    methods = set() if isinstance(auth, dict) else {auth}
    if isinstance(auth, dict):
        # The operator's choice: an enum setting whose values are methods.
        if transport_type != "http":
            errors.append("transport.auth: {setting: name} is for the http transport")
        decl = settings.get(auth.get("setting"))
        if decl is None:
            errors.append(f"transport.auth names unknown setting '{auth.get('setting')}'")
            methods = set()
        elif decl.get("type") != "enum" or set(decl.get("values", [])) - {"none", "basic", "digest", "bearer", "header", "oauth2"}:
            errors.append("transport.auth's setting must be an enum of none, basic, digest, bearer, header and oauth2")
            methods = set()
        else:
            methods = set(decl.get("values", []))
    for method in sorted(methods & {"bearer", "header"}):
        if "token" not in settings:
            errors.append(f"auth: {method} needs a 'token' setting")
    for i, rule in enumerate(((doc.get("telemetry") or {}).get("updates")) or []):
        if "match_each_in" in rule:
            if "match_each" not in rule:
                errors.append(f"telemetry.updates[{i}]: match_each_in goes with match_each")
            else:
                # On an address rule, the captures are the address's, then
                # arg_match's in argument order.
                sources = ([rule.get("match", "")] if "address" not in rule else
                           [rule["address"]] + [p for _, p in sorted((rule.get("arg_match") or {}).items(),
                                                                     key=lambda kv: int(kv[0]))])
                try:
                    groups = sum(re.compile(s).groups for s in sources)
                except (re.error, ValueError):
                    groups = 0
                if rule["match_each_in"] > groups:
                    errors.append(f"telemetry.updates[{i}]: match_each_in names group {rule['match_each_in']}, which match does not have")
        if "address" in rule and "match_each" in rule:
            if "match_each_in" not in rule:
                errors.append(f"telemetry.updates[{i}]: match_each on an address rule needs match_each_in")
            if any(k in rule for k in ("json", "json_each", "json_match")):
                errors.append(f"telemetry.updates[{i}]: match_each goes along a capture's text, not JSON")
        if "arg_match" in rule and "address" not in rule:
            errors.append(f"telemetry.updates[{i}]: arg_match goes with address")
        for index, pattern in (rule.get("arg_match") or {}).items():
            try:
                re.compile(pattern)
            except re.error as e:
                errors.append(f"telemetry.updates[{i}].arg_match.{index} does not compile: {e}")
    sse_ = (doc.get("telemetry") or {}).get("sse") or {}
    if "body" in sse_:
        errors += check_template("text", sse_["body"], {}, settings, "telemetry.sse.body")
    if (transport.get("reply_framing") == "fenced") != ("reply_fence" in transport):
        errors.append("transport: reply_framing fenced and reply_fence go together")
    if "reply_fence" in transport:
        try:
            re.compile(transport["reply_fence"])
        except re.error as e:
            errors.append(f"transport.reply_fence does not compile: {e}")
    for key in ("reply_block_header", "reply_line_header"):
        if key not in transport:
            continue
        if transport.get("reply_framing") != "headed-block":
            errors.append(f"transport.{key} goes with reply_framing headed-block")
        try:
            re.compile(transport[key])
        except re.error as e:
            errors.append(f"transport.{key} does not compile: {e}")
    osc_listen = (doc.get("telemetry") or {}).get("osc")
    if osc_listen is not None:
        if transport_type in ("osc-udp", "line-udp"):
            errors.append("telemetry.osc is for a stream or HTTP transport: a UDP transport has listen_port")
        listen = osc_listen.get("listen_port")
        if isinstance(listen, dict) and listen.get("setting") not in settings:
            errors.append(f"telemetry.osc.listen_port names unknown setting '{listen.get('setting')}'")
    for path, pattern in (transport.get("refusal_json") or {}).items():
        try:
            re.compile(pattern)
        except re.error as e:
            errors.append(f"transport.refusal_json.{path} does not compile: {e}")
    rsb = transport.get("refusal_status_body")
    if rsb is not None:
        if transport_type != "http":
            errors.append("transport.refusal_status_body is for the http transport")
        for path, pattern in (rsb.get("json") or {}).items():
            try:
                re.compile(pattern)
            except re.error as e:
                errors.append(f"transport.refusal_status_body.json.{path} does not compile: {e}")
        if rsb.get("match") is not None:
            try:
                re.compile(rsb["match"])
            except re.error as e:
                errors.append(f"transport.refusal_status_body.match does not compile: {e}")
        if transport.get("refusal_status") == []:
            errors.append("transport.refusal_status_body needs a refusal_status")
    for name, text in (transport.get("headers") or {}).items():
        errors += check_template("text", text, {}, settings, f"transport.headers.{name}")
    if ("header" in methods) != bool(transport.get("auth_header")):
        errors.append("auth: header and auth_header go together")
    sio = (websocket or {}).get("socketio")
    if isinstance(sio, dict) and sio.get("auth"):
        errors += check_template("text", sio["auth"], {}, settings, "telemetry.websocket.socketio.auth")
    for method in sorted(methods & {"basic", "digest"}):
        if not {"username", "password"} <= settings.keys():
            errors.append(f"auth: {method} needs 'username' and 'password' settings")
    # OAuth 2: the consumer's tokens as settings, refreshed by the core.
    if "oauth2" in methods:
        if not isinstance(transport.get("oauth"), dict):
            errors.append("auth: oauth2 needs an oauth block with token_url")
        validate = (transport.get("oauth") or {}).get("validate") or {}
        every = validate.get("every_s", 3600)
        if isinstance(every, int) and every < 60:
            errors.append("transport.oauth.validate.every_s is at least 60: a token is not checked more than once a minute")
        for name, kind, secret in [("client_id", "string", False), ("client_secret", "string", True),
                                   ("refresh_token", "string", True), ("access_token", "string", True),
                                   ("expires_at", "int", False), ("token_url", "string", False)]:
            decl = settings.get(name)
            if decl is None:
                errors.append(f"auth: oauth2 needs a '{name}' setting")
            elif decl.get("type") != kind:
                errors.append(f"auth: oauth2's '{name}' setting is of type {kind}")
            elif secret and decl.get("secret") is not True:
                errors.append(f"auth: oauth2's '{name}' setting is secret: true")
            elif decl.get("required"):
                errors.append(f"auth: oauth2's '{name}' setting is not required: a plain or refreshed token may leave it out")
    elif "oauth" in transport:
        errors.append("transport.oauth is for auth: oauth2")

    listen = (doc.get("transport") or {}).get("listen_port")
    if isinstance(listen, dict) and listen.get("setting") not in settings:
        errors.append(f"transport.listen_port names unknown setting '{listen.get('setting')}'")

    for i, step in enumerate(doc.get("on_connect", [])):
        conditional = None
        if isinstance(step, dict) and "send" in step:
            conditional = step.get("when_set")
            if conditional and conditional not in settings:
                errors.append(f"on_connect[{i}]: when_set names unknown setting '{conditional}'")
            transport_type = (doc.get("transport") or {}).get("type")
            if "after_prompt" in step and transport_type not in ("line-tcp", "line-serial"):
                errors.append(f"on_connect[{i}]: after_prompt needs a line-tcp or line-serial transport")
            for key in ("refused", "accepted"):
                if key in step:
                    try:
                        re.compile(step[key])
                    except re.error as e:
                        errors.append(f"on_connect[{i}]: {key} does not compile: {e}")
            if "accepted" in step and "refused" not in step:
                errors.append(f"on_connect[{i}]: accepted ends a refused watch; it needs refused")
            step = step["send"]
        for context, text in template_strings(step):
            errors += check_template(context, text, {}, settings, f"on_connect[{i}]", conditional)

    errors += telemetry_checks(doc, conversions)

    probe = (doc.get("transport") or {}).get("probe")
    if probe is not None:
        for context, text in template_strings(probe):
            errors += check_template(context, text, {}, settings, "transport.probe")

    errors += per_model_checks(doc, model_ids)
    errors += session_checks(doc)
    errors += endpoint_checks(doc)
    return errors


def endpoint_checks(doc: dict) -> list[str]:
    """`transport.endpoints` and the request items naming one (SPEC.md §2,
    Endpoints): settings exist and fit, names resolve, and liveness and the
    session stay on the transport's own port."""
    errors: list[str] = []
    transport = doc.get("transport") or {}
    settings = doc.get("settings") or {}
    endpoints = transport.get("endpoints") or {}
    for name, e in endpoints.items():
        where = f"transport.endpoints.{name}"
        for key, kind, values in (("port", "int", None), ("scheme", "enum", {"http", "https"}),
                                  ("auth", "enum", {"inherit", "none"})):
            ref = e.get(key)
            if not isinstance(ref, dict):
                continue
            decl = settings.get(ref.get("setting"))
            if decl is None:
                errors.append(f"{where}.{key} names unknown setting '{ref.get('setting')}'")
            elif decl.get("type") != kind or (values and set(decl.get("values", [])) - values):
                errors.append(f"{where}.{key}'s setting must be "
                              + ("an int" if kind == "int" else f"an enum of {' and '.join(sorted(values))}"))
        # Beside a transport that is not HTTP there is no credential to
        # inherit: the endpoint's requests go without one.
        if transport.get("type") != "http" and ("auth" in e and e.get("auth") != "none"):
            errors.append(f"{where}.auth is none beside a {transport.get('type')} transport")
        if "accept_invalid_certs" in e and not isinstance(e["accept_invalid_certs"], bool):
            errors.append(f"{where}.accept_invalid_certs is true or false")
    telemetry = doc.get("telemetry") or {}
    places = [(f"commands.{n}.send", c.get("send")) for n, c in (doc.get("commands") or {}).items()]
    places += [(where, group.get("send")) for where, group in poll_groups(telemetry)]
    places += [("telemetry.subscribe.send", (telemetry.get("subscribe") or {}).get("send"))]
    places += [(f"on_connect[{i}]", step.get("send") if isinstance(step, dict) and "send" in step else step)
               for i, step in enumerate(doc.get("on_connect") or [])]
    places += [(f"telemetry.updates[{i}].then_send", rule.get("then_send"))
               for i, rule in enumerate(telemetry.get("updates") or [])]
    for where, send in places:
        for i, item in enumerate(items_of(send)):
            if isinstance(item, dict) and "endpoint" in item and item["endpoint"] not in endpoints:
                errors.append(f"{where}[{i}]: endpoint '{item['endpoint']}' is not in transport.endpoints")
    for where, send in (("transport.probe", transport.get("probe")),
                        ("transport.session.login", (transport.get("session") or {}).get("login"))):
        if any(isinstance(i, dict) and "endpoint" in i for i in items_of(send)):
            errors.append(f"{where}: goes to the transport's own port; it cannot name an endpoint")
    return errors


def items_of(send) -> list:
    return send if isinstance(send, list) else [send] if send is not None else []


def poll_groups(telemetry: dict) -> list[tuple[str, dict]]:
    """`telemetry.poll`: one group (an object), or a list of groups each at
    its own interval (SPEC.md §8, Telemetry)."""
    poll = telemetry.get("poll")
    if isinstance(poll, list):
        return [(f"telemetry.poll[{i}].send", g or {}) for i, g in enumerate(poll)]
    return [("telemetry.poll.send", poll)] if poll else []


def per_model_checks(doc: dict, model_ids: set) -> list[str]:
    """Messages naming `models` (SPEC.md §4, Messages per model): the models
    exist, and every model a command supports has a message."""
    errors: list[str] = []
    transport = doc.get("transport") or {}
    telemetry = doc.get("telemetry") or {}
    places = [(f"commands.{n}.send", c.get("send")) for n, c in (doc.get("commands") or {}).items()]
    places += [("transport.probe", transport.get("probe")),
               ("transport.session.login", (transport.get("session") or {}).get("login")),
               *[(where, group.get("send")) for where, group in poll_groups(telemetry)],
               ("telemetry.subscribe.send", (telemetry.get("subscribe") or {}).get("send")),
               ("on_connect", doc.get("on_connect")),
               # Update rules naming `models` (SPEC.md §8, Rules per model).
               ("telemetry.updates", telemetry.get("updates"))]
    for where, send in places:
        for i, item in enumerate(items_of(send)):
            if isinstance(item, dict) and "models" in item:
                bad = set(item["models"]) - model_ids
                if bad:
                    errors.append(f"{where}[{i}]: models names unknown model(s) {sorted(bad)}")
            if (isinstance(item, dict) and "message" in item and where != "telemetry.updates"
                    and transport.get("type") not in ("line-tcp", "line-serial", "line-udp", "ws")):
                errors.append(f"{where}[{i}]: {{models, message}} is for line and websocket transports")
            if isinstance(item, dict) and "session" in item and transport.get("type") != "http":
                errors.append(f"{where}[{i}]: session applies to http requests")
    for model in doc.get("models", []):
        for name in model.get("supports", []):
            send = ((doc.get("commands") or {}).get(name) or {}).get("send")
            if send is None:
                continue
            if not any(not isinstance(i, dict) or model["id"] in i.get("models", [model["id"]])
                       for i in items_of(send)):
                errors.append(f"commands.{name}: no message for model '{model['id']}', which supports it")
        login = (transport.get("session") or {}).get("login")
        if login is not None and not any(model["id"] in i.get("models", [model["id"]]) for i in items_of(login)):
            errors.append(f"transport.session.login: no login for model '{model['id']}'")
    return errors


def session_checks(doc: dict) -> list[str]:
    """`transport.session`, `https_port`, `expect.header` and `code_path`."""
    errors: list[str] = []
    transport = doc.get("transport") or {}
    settings = doc.get("settings") or {}
    is_http = transport.get("type") == "http"
    session = transport.get("session")
    if session is not None:
        if not is_http:
            errors.append("transport.session is for the http transport")
        if "when_set" in session and session["when_set"] not in settings:
            errors.append(f"transport.session.when_set names unknown setting '{session['when_set']}'")
        for i, item in enumerate(items_of(session.get("login"))):
            for context, text in template_strings(item):
                errors += check_template(context, text, {}, settings, f"transport.session.login[{i}]")
        for key in ("relogin", "refused"):
            for jpath, pattern in ((session.get(key) or {}).get("json") or {}).items():
                if not jpath.startswith("$"):
                    errors.append(f"transport.session.{key}.json: '{jpath}' is not a JSON path")
                try:
                    re.compile(pattern)
                except re.error as e:
                    errors.append(f"transport.session.{key}.json: {e}")
    # `scheme: tls` on line-tcp, and `tls_port` (SPEC.md §2, TLS).
    if "scheme" in transport and transport.get("type") not in ("http", "ws", "line-tcp"):
        errors.append(f"transport.scheme is not for {transport.get('type')}")
    if "tls_port" in transport:
        if not isinstance(transport.get("scheme"), dict):
            errors.append("transport.tls_port needs scheme: {setting: name}")
        if not any(e["port"] == transport["tls_port"] and e["role"] == "control" and e["protocol"] == "tls"
                   for e in doc.get("ports") or []):
            errors.append(f"ports: no tls control entry for tls_port {transport['tls_port']}")
    if transport.get("type") == "line-tcp" and transport.get("scheme") not in (None, "tcp") and "tls_port" not in transport:
        if not any(e["role"] == "control" and e["protocol"] == "tls" for e in doc.get("ports") or []):
            errors.append("ports: a line-tcp transport with a tls scheme needs a tls control entry")
    if "https_port" in transport:
        if not isinstance(transport.get("scheme"), dict):
            errors.append("transport.https_port needs scheme: {setting: name}")
        if not any(e["port"] == transport["https_port"] and e["role"] == "control"
                   for e in doc.get("ports") or []):
            errors.append(f"ports: no control entry for https_port {transport['https_port']}")
    for name, command in (doc.get("commands") or {}).items():
        expect = command.get("expect") or {}
        if "header" in expect and not is_http:
            errors.append(f"commands.{name}: expect.header needs an http transport")
        if "code_path" in expect:
            if not str(expect["code_path"]).startswith("$"):
                errors.append(f"commands.{name}: expect.code_path is not a JSON path")
            if "code_range" not in expect:
                errors.append(f"commands.{name}: expect.code_path is read by code_range")
    return errors


def telemetry_checks(doc: dict, conversions: dict | None = None) -> list[str]:
    """Every path a telemetry rule writes must be declared in 'state', so its
    type is known, every regex must compile, and every conversion a value
    names must be declared."""
    errors: list[str] = []
    telemetry = doc.get("telemetry") or {}
    transport_type = (doc.get("transport") or {}).get("type")

    groups = poll_groups(telemetry)
    if len(groups) > 4:
        errors.append("telemetry.poll: at most 4 poll groups")
    for where, group in groups:
        for i, item in enumerate(items_of(group.get("send"))):
            if isinstance(item, dict) and "reply_address" in item:
                if transport_type not in ("osc-udp", "osc-tcp"):
                    errors.append(f"{where}[{i}]: reply_address needs an OSC transport")
                try:
                    re.compile(item["reply_address"])
                except re.error as e:
                    errors.append(f"{where}[{i}].reply_address: {e}")

    # JSON messages arrive on a websocket, an event stream, or as the lines of
    # a line transport; HTTP replies are matched with `path`.
    has_json = (transport_type in ("ws", "line-tcp", "line-serial", "line-udp")
                or "websocket" in telemetry or "sse" in telemetry)
    if "sse" in telemetry and transport_type != "http":
        errors.append("telemetry.sse needs an http transport")
    declared = [key.split(".") for key in (doc.get("state") or {})]

    def is_declared(path: str, prefix: bool = False) -> bool:
        # As the engine reads it: a templated segment may stand for any
        # declared segment (mimoLive's '{type}.{id}.name' writes layers.<id>.name
        # and sources.<id>.name), and a concrete path no declaration matches is
        # not assigned at run time. A deletion may remove a whole subtree, so
        # for it a prefix of declared paths will do.
        parts = ["*" if "{" in seg else seg for seg in path.split(".")]
        return any(
            (len(d) >= len(parts) if prefix else len(d) == len(parts))
            and all(a == "*" or b == "*" or a == b for a, b in zip(d, parts))
            for d in declared
        )

    # A quiet rule matches no message: it times the stream's silence
    # (SPEC.md §8, A status dump that ends in silence).
    quiet = [i for i, r in enumerate(telemetry.get("updates", [])) if "quiet_ms" in r]
    if len(quiet) > 1:
        errors.append("telemetry.updates: at most one quiet_ms rule")
    for i in quiet:
        rule = telemetry["updates"][i]
        if transport_type not in ("line-tcp", "line-serial", "osc-tcp", "ws"):
            errors.append(f"telemetry.updates[{i}]: quiet_ms needs a line-tcp, line-serial, osc-tcp or ws transport")
        others = sorted(k for k in ("match", "address", "header", "path", "json_match",
                                    "then_send", "replace") if k in rule)
        if others:
            errors.append(f"telemetry.updates[{i}]: a quiet_ms rule has no {', '.join(others)}")

    for i, rule in enumerate(telemetry.get("updates", [])):
        where = f"telemetry.updates[{i}]"
        for key in ("match", "match_each", "header", "each_line", "address", "path"):
            if key in rule:
                try:
                    re.compile(rule[key])
                except re.error as e:
                    errors.append(f"{where}.{key}: {e}")
        asked = rule.get("request_match")
        if isinstance(asked, str):
            # The text of the line message a reply answers (SPEC.md §8).
            try:
                re.compile(asked)
            except re.error as e:
                errors.append(f"{where}.request_match: {e}")
            if "match" not in rule or transport_type not in ("line-tcp", "line-serial", "line-udp"):
                errors.append(f"{where}: a request_match regex goes with match, on a line transport")
        if "match_each" in rule and ("match" not in rule or "path" in rule) and "address" not in rule:
            errors.append(f"{where}: match_each reads the rest of a message a match rule matched, "
                          "or a capture of an address rule")
        if "each_match" in rule and "json_each" not in rule:
            errors.append(f"{where}: each_match selects elements of json_each")
        # A json_each path may name settings only (SPEC.md §8, a member
        # chosen by a setting), with no directives.
        for m in PLACEHOLDER.finditer(rule.get("json_each") or ""):
            name = m.group(1)
            if not name.startswith("settings.") or m.group(2):
                errors.append(f"{where}.json_each: '{m.group(0)}' is not a plain {{settings.name}}")
            elif name[len("settings."):] not in (doc.get("settings") or {}):
                errors.append(f"{where}.json_each: unknown setting '{name[len('settings.'):]}'")
        for key in ("json_match", "request_match", "each_match"):
            if not isinstance(rule.get(key) or {}, dict):
                continue
            for jpath, pattern in (rule.get(key) or {}).items():
                if not jpath.startswith("$"):
                    errors.append(f"{where}.{key}: '{jpath}' is not a JSON path")
                try:
                    re.compile(pattern)
                except re.error as e:
                    errors.append(f"{where}.{key}: {e}")
        if "json_match" in rule and "path" not in rule and "address" not in rule and not has_json:
            errors.append(f"{where}: json_match reads JSON messages; the spec has no websocket, "
                          "event stream or line transport")
        if isinstance(asked, dict) and "path" not in rule:
            errors.append(f"{where}: request_match applies to a path rule")
        if "headers" in rule and "path" not in rule:
            errors.append(f"{where}: headers applies to a path rule (an HTTP reply)")
        # then_send: templates over the rule's captures and the settings.
        captures = set(rule.get("json") or {}) | set(rule.get("headers") or {})
        settings = doc.get("settings") or {}
        for context, text in template_strings(rule.get("then_send") or []):
            for m in PLACEHOLDER.finditer(text):
                name = m.group(1)
                if name.startswith("settings."):
                    errors += check_template(context, m.group(0), {}, settings, f"{where}.then_send")
                elif not (name.isdigit() or name in captures or name.startswith("arg")
                          or re.fullmatch(r"\^*(index|key)", name)
                          # An XML element's captures are named by the reply.
                          or "xml_each" in rule):
                    errors.append(f"{where}.then_send: '{{{name}}}' is not one of the rule's captures")
        if "arg_types" in rule and "address" not in rule:
            errors.append(f"{where}: arg_types applies to an address rule")
        for value in (rule.get("state") or {}).values():
            template = value if isinstance(value, str) else value.get("value", "")
            for m in PLACEHOLDER.finditer(template):
                for d in [d for d in m.group(2).split(":") if d]:
                    c = CONVERSION.match(d)
                    if c and c.group(2) not in (conversions or {}):
                        errors.append(f"{where}: ':{d}' names undeclared conversion '{c.group(2)}'")
                    if d == "key":
                        errors.append(f"{where}: ':key' marks a state path segment, not a value")
                    elif not any(pat.match(d) for pat, _ in DIRECTIVES):
                        errors.append(f"{where}: unknown directive ':{d}' in '{m.group(0)}'")
        # `:key` keeps a path segment whole, dots included (SPEC.md §8, Keys
        # holding dots): only in a state or replace path, and last.
        replace_ = rule.get("replace") or []
        for path in list(rule.get("state") or {}) + ([replace_] if isinstance(replace_, str) else replace_):
            for m in PLACEHOLDER.finditer(path):
                directives = [d for d in m.group(2).split(":") if d]
                if "key" in directives[:-1]:
                    errors.append(f"{where}: ':key' goes last in '{m.group(0)}'")
        for _, text in template_strings(rule.get("then_send") or []):
            if any("key" in m.group(2).split(":") for m in PLACEHOLDER.finditer(text)):
                errors.append(f"{where}.then_send: ':key' marks a state path segment, not a message")
        deletions = {p for p, v in (rule.get("state") or {}).items()
                     if isinstance(v, dict) and v.get("delete") is True}
        paths = list((rule.get("state") or {}).keys())
        for field in (rule.get("fields") or {}).values():
            paths.append(field if isinstance(field, str) else field.get("path", ""))
        for path in paths:
            if not is_declared(path, prefix=path in deletions):
                errors.append(f"{where}: '{path}' is not declared in 'state'")
        replace = rule.get("replace") or []
        for path in [replace] if isinstance(replace, str) else replace:
            if not is_declared(path, prefix=True):
                errors.append(f"{where}: replace '{path}' is not declared in 'state'")
        # A rule that only replaces sits beside the plain rule writing the
        # values for the same messages (SPEC.md §8, Lists).
        if replace and not any(k in rule for k in ("state", "then_send", "fields")):
            rules = telemetry.get("updates", [])
            key = next((k for k in ("path", "header", "address", "match") if k in rule), "json_match")
            targets = [replace] if isinstance(replace, str) else replace
            beside = [rules[j] for j in (i - 1, i + 1) if 0 <= j < len(rules)]
            if not any(
                key in r
                and any(p.startswith(t + ".") for p in (r.get("state") or {}) for t in targets)
                for r in beside
            ):
                errors.append(f"{where}: a rule with only replace must sit beside a {key} rule "
                              "that writes under what it replaces")
    return errors


def port_checks(doc: dict) -> list[str]:
    """`ports` must list the ports the core really uses (SPEC.md §2, Ports)."""
    errors = []
    ports = doc.get("ports") or []
    settings = doc.get("settings") or {}
    control = [e for e in ports if e["role"] == "control"]
    if not control:
        errors.append("ports: no control port")
    for i, e in enumerate(ports):
        name = e.get("setting")
        if name and name not in settings:
            errors.append(f"ports[{i}].setting: '{name}' is not a declared setting")
        if e["port"] is None and e["role"] == "control" and "note" not in e:
            errors.append(f"ports[{i}]: a control port with no default needs a note saying so")
    t = doc.get("transport")
    if t is not None:
        # A delimited reply ends at `close`, compared without surrounding
        # whitespace: an empty one would never end a message.
        if (t.get("type") in ("line-tcp", "line-serial") and t.get("framing") == "delimited"
                and not str(t.get("close", "")).strip() and "reply_framing" not in t):
            errors.append("transport.close: framing delimited needs a close that is not only whitespace")
        if t.get("type") == "line-serial":
            if not any(e["protocol"] == "serial" for e in control):
                errors.append("ports: a line-serial transport needs a serial control entry")
        elif t.get("port") not in [e["port"] for e in control]:
            errors.append(f"ports: the transport's port {t.get('port')} is not a control port")
        listen = t.get("listen_port")
        if isinstance(listen, dict) and not any(e.get("setting") == listen["setting"] for e in ports):
            errors.append(f"ports: no entry for the listen port setting '{listen['setting']}'")
        if isinstance(listen, int) and not any(e["port"] == listen and e.get("listener") == "core" for e in ports):
            errors.append(f"ports: no core-listener entry for listen_port {listen}")
        # Each endpoint is a control port: its default, or its setting.
        for name, ep in (t.get("endpoints") or {}).items():
            port = ep.get("port")
            if isinstance(port, dict):
                setting = port.get("setting")
                entries = [e for e in control if e.get("setting") == setting]
                default = (settings.get(setting) or {}).get("default")
                if not entries:
                    errors.append(f"ports: no control entry with setting '{setting}' for endpoint '{name}'")
                elif default is not None and not any(e["port"] == default for e in entries):
                    errors.append(f"ports: endpoint '{name}''s control entry is not at its setting's default {default}")
            elif not any(e["port"] == port for e in control):
                errors.append(f"ports: endpoint '{name}''s port {port} is not a control port")
        tel = doc.get("telemetry") or {}
        osc_listen = (tel.get("osc") or {}).get("listen_port")
        if isinstance(osc_listen, dict) and not any(
                e.get("setting") == osc_listen["setting"] and e["role"] == "feedback"
                and e.get("listener") == "core" and e["protocol"] == "udp" for e in ports):
            errors.append(f"ports: no core-listener udp feedback entry for the OSC port setting "
                          f"'{osc_listen['setting']}'")
        if isinstance(osc_listen, int) and not any(
                e["port"] == osc_listen and e.get("listener") == "core" and e["protocol"] == "udp"
                for e in ports):
            errors.append(f"ports: no core-listener udp entry for telemetry.osc.listen_port {osc_listen}")
        if ("websocket" in tel or "sse" in tel) and not any(e["role"] == "push" for e in ports):
            errors.append("ports: the push channel has no push entry")
    return errors


# The settings a device on a serial port declares (SPEC.md §2, line-serial),
# each a type and the values the core can open a port with.
SERIAL_SETTINGS = ("serial_port", "baud_rate", "data_bits", "parity", "stop_bits", "flow_control")
SERIAL_CHOICES = {"parity": {"none", "odd", "even"}, "flow_control": {"none", "xon-xoff", "rts-cts"}}
SERIAL_RANGES = {"data_bits": (5, 8), "stop_bits": (1, 2)}


def serial_checks(doc: dict) -> list[str]:
    """A serial port is named by the serial_port setting, and its line
    settings are declared with the device's defaults (SPEC.md §2)."""
    errors = []
    ports = doc.get("ports") or []
    entries = [e for e in ports if e["protocol"] == "serial"]
    line_serial = (doc.get("transport") or {}).get("type") == "line-serial"
    for i, e in enumerate(ports):
        if e["protocol"] != "serial":
            continue
        if e["port"] is not None:
            errors.append(f"ports[{i}]: a serial port has no number (port: null)")
        if e.get("setting") != "serial_port":
            errors.append(f"ports[{i}]: a serial port's setting is serial_port")
        if e["role"] != "control" or e.get("listener", "device") != "device":
            errors.append(f"ports[{i}]: a serial port is a control entry the device listens on")
    if not entries:
        return errors
    settings = doc.get("settings") or {}
    for name in SERIAL_SETTINGS:
        s = settings.get(name)
        if s is None:
            errors.append(f"settings.{name}: a device on a serial port declares it")
            continue
        kind = s.get("type")
        if name != "serial_port" and "default" not in s:
            errors.append(f"settings.{name}: needs the device's default")
        if name == "serial_port":
            if kind != "string":
                errors.append("settings.serial_port: must be a string")
            if line_serial and not s.get("required"):
                errors.append("settings.serial_port: must be required on a line-serial transport")
        elif name == "baud_rate":
            ok = (kind == "int" and s.get("min", 1) >= 1) or (
                kind == "enum" and all(str(v).isdigit() and int(v) > 0 for v in s.get("values", [])))
            if not ok:
                errors.append("settings.baud_rate: an int of 1 or more, or an enum of digits")
        elif name in SERIAL_RANGES:
            low, high = SERIAL_RANGES[name]
            if kind != "int" or s.get("min", 0) < low or s.get("max", 99) > high:
                errors.append(f"settings.{name}: an int within {low} to {high} (give min and max)")
        else:
            bad = set(s.get("values") or []) - SERIAL_CHOICES[name]
            if kind != "enum" or bad:
                errors.append(f"settings.{name}: an enum of {sorted(SERIAL_CHOICES[name])}")
    return errors


LINK_SETTINGS = ("link",) + SERIAL_SETTINGS


# The settings that name a MIDI port of the host (SPEC.md §2, Ports).
MIDI_SETTINGS = ("midi_input", "midi_output")


def midi_checks(doc: dict) -> list[str]:
    """A MIDI port is named by the midi_input or midi_output setting, a
    string, and is only a native module's (no transport opens one)."""
    errors = []
    ports = doc.get("ports") or []
    settings = doc.get("settings") or {}
    for i, e in enumerate(ports):
        if e["protocol"] != "midi":
            continue
        if e["port"] is not None:
            errors.append(f"ports[{i}]: a MIDI port has no number (port: null)")
        if e.get("setting") not in MIDI_SETTINGS:
            errors.append(f"ports[{i}]: a MIDI port's setting is midi_input or midi_output")
        elif (settings.get(e["setting"]) or {}).get("type") != "string":
            errors.append(f"settings.{e['setting']}: must be a string")
        if e["role"] != "control" or e.get("listener", "device") != "device":
            errors.append(f"ports[{i}]: a MIDI port is a control entry the device listens on")
        if doc.get("implementation") != "native":
            errors.append(f"ports[{i}]: only a native module opens a MIDI port")
    return errors


# The transport keys a `serial` block's `overrides` may change (SPEC.md §2,
# "The same protocol on a serial port"), as in the core's SERIAL_OVERRIDES.
SERIAL_OVERRIDES = ("framing", "open", "close", "terminator", "reply_framing",
                    "reply_terminators", "reply_fence", "reply_match", "error_match", "pacing")


def pacing_checks(doc: dict) -> list[str]:
    """`transport.pacing` and a serial block's `pacing` overrides (SPEC.md §2,
    Pacing): the models they name exist, each model is named once in a
    `by_model` list, and every group's `match` is a regex."""
    t = doc.get("transport") or {}
    model_ids = {m["id"] for m in doc.get("models") or []}
    blocks = []
    if isinstance(t.get("pacing"), dict):
        blocks.append(("transport.pacing", t["pacing"]))
    overrides = (t.get("serial") or {}).get("overrides")
    entries = overrides if isinstance(overrides, list) else [overrides] if isinstance(overrides, dict) else []
    for i, entry in enumerate(entries):
        if isinstance(entry, dict) and isinstance(entry.get("pacing"), dict):
            blocks.append((f"transport.serial.overrides[{i}].pacing", entry["pacing"]))
    errors: list[str] = []

    def by_model(at: str, owner: dict) -> None:
        named: set[str] = set()
        for entry in owner.get("by_model") or []:
            for m in entry.get("models") or []:
                if m not in model_ids:
                    errors.append(f"{at}.by_model: no model '{m}'")
                elif m in named:
                    errors.append(f"{at}.by_model: model '{m}' is named twice")
                named.add(m)

    for at, block in blocks:
        by_model(at, block)
        for name, group in (block.get("groups") or {}).items():
            by_model(f"{at}.groups.{name}", group)
            pattern = group.get("match")
            try:
                re.compile(pattern)
            except (re.error, TypeError) as e:
                errors.append(f"{at}.groups.{name}.match: {e}")
            else:
                if re.search(r"\(\?<?[=!]|\[1-9]", pattern):
                    errors.append(f"{at}.groups.{name}.match: lookaround and backreferences are not RE2")
    return errors


def expand_serial(doc: dict) -> list[str]:
    """A line-tcp transport's `serial` block stands for the `link` setting and
    the six serial settings (SPEC.md §2, "The same protocol on a serial
    port"); add them, as the core does, so the other checks see them. Returns
    what is wrong with the block."""
    t = doc.get("transport") or {}
    block = t.get("serial")
    errors: list[str] = []
    steps_with_link = [i for i, s in enumerate(doc.get("on_connect") or [])
                       if isinstance(s, dict) and "link" in s]
    if block is None:
        for i in steps_with_link:
            errors.append(f"on_connect[{i}]: link needs transport.serial")
        return errors
    if t.get("type") != "line-tcp":
        return ["transport.serial is for a line-tcp transport"]
    model_ids = {m["id"] for m in doc.get("models") or []}
    for key in ("serial_only", "models"):
        for m in block.get(key) or []:
            if m not in model_ids:
                errors.append(f"transport.serial.{key}: no model '{m}'")
    # `overrides`: the framing and reply keys that differ on the serial
    # port, for every model or the models an entry names; with a delimited
    # framing and an empty close, replies need a framing of their own.
    overrides = block.get("overrides")
    entries = overrides if isinstance(overrides, list) else [overrides] if overrides is not None else []
    if overrides is not None and (not entries or not all(isinstance(e, dict) for e in entries)):
        errors.append("transport.serial.overrides: a mapping or a list of them")
        entries = []
    for i, entry in enumerate(entries):
        for key in entry:
            if key != "models" and key not in SERIAL_OVERRIDES:
                errors.append(f"transport.serial.overrides[{i}].{key}: not a framing, reply or pacing key a serial port can change")
        for m in entry.get("models") or []:
            if m not in model_ids:
                errors.append(f"transport.serial.overrides[{i}].models: no model '{m}'")
    for m in sorted(model_ids):
        merged = dict(t)
        for entry in entries:
            if "models" not in entry or m in entry["models"]:
                merged.update({k: v for k, v in entry.items() if k != "models"})
        if (merged.get("framing") == "delimited" and not str(merged.get("close", "")).strip()
                and "reply_framing" not in merged):
            errors.append(f"transport.serial.overrides: on the serial port of {m}, framing delimited "
                          "with an empty close needs reply_framing")
        for key in ("reply_match", "error_match", "reply_fence"):
            if isinstance(merged.get(key), str):
                try:
                    re.compile(merged[key])
                except re.error as e:
                    errors.append(f"transport.serial.overrides: {key} for {m}: {e}")
    only = "serial_only" in block
    link = block.get("link", "tcp")
    if only and link == "serial":
        errors.append("transport.serial: serial_only names the models that default to serial, so link stays tcp")
    settings = doc.setdefault("settings", {}) or {}
    doc["settings"] = settings
    for name in LINK_SETTINGS:
        if name in settings:
            errors.append(f"settings.{name}: transport.serial declares it; leave it out")

    model_ids = {m["id"] for m in doc.get("models") or []}
    serial_models = block.get("models")

    def line(name):
        v = block[name]
        if isinstance(v, dict):
            if v["default"] not in v["values"]:
                errors.append(f"transport.serial.{name}: the default is not one of the values")
            # A default that differs by model: each model named once, with
            # the serial port, and a value the port allows.
            seen = set()
            for i, entry in enumerate(v.get("by_model") or []):
                if entry["default"] not in v["values"]:
                    errors.append(f"transport.serial.{name}.by_model[{i}]: the default is not one of the values")
                for m in entry["models"]:
                    if m not in model_ids:
                        errors.append(f"transport.serial.{name}.by_model[{i}]: no model '{m}'")
                    elif serial_models is not None and m not in serial_models:
                        errors.append(f"transport.serial.{name}.by_model[{i}]: model '{m}' has no serial port")
                    if m in seen:
                        errors.append(f"transport.serial.{name}.by_model[{i}]: model '{m}' is named twice")
                    seen.add(m)
            return v["default"], v["values"]
        return v, [v]

    settings["link"] = {"type": "enum", "values": ["tcp", "serial"], "label": "Link", "description": "generated"}
    if not only:
        settings["link"]["default"] = link
    settings["serial_port"] = {"type": "string", "max_length": 256, "label": "Serial port", "description": "generated"}
    default, values = line("baud_rate")
    if any(not isinstance(v, int) or v <= 0 for v in values):
        errors.append("transport.serial.baud_rate: bits per second, above 0")
    if len(values) == 1:
        settings["baud_rate"] = {"type": "int", "min": default, "max": default, "default": default,
                                 "label": "Baud rate", "description": "generated"}
    else:
        settings["baud_rate"] = {"type": "enum", "values": [str(v) for v in values], "default": str(default),
                                 "label": "Baud rate", "description": "generated"}
    for name in ("data_bits", "stop_bits"):
        default, values = line(name)
        n = sorted(set(values))
        if len(n) != len(values) or n[-1] - n[0] + 1 != len(n):
            errors.append(f"transport.serial.{name}: whole numbers with no gaps")
        settings[name] = {"type": "int", "min": n[0], "max": n[-1], "default": default,
                          "label": name, "description": "generated"}
    for name in ("parity", "flow_control"):
        default, values = line(name)
        settings[name] = {"type": "enum", "values": list(values), "default": default,
                          "label": name, "description": "generated"}
    # The ports say which link each entry is for.
    ports = doc.get("ports") or []
    if not any(e["protocol"] == "serial" and e.get("when") == "link serial" for e in ports):
        errors.append('ports: transport.serial needs a serial control entry with when: "link serial"')
    # "link tcp", or "link tcp, scheme tcp" beside a TLS scheme setting.
    if not any(e["protocol"] == "tcp" and e["port"] == t.get("port")
               and str(e.get("when", "")).split(",")[0].strip() == "link tcp"
               for e in ports):
        errors.append(f'ports: the transport port {t.get("port")} needs when: "link tcp" beside transport.serial')
    return errors


def label_checks(doc: dict) -> list[str]:
    """Every parameter and setting has a label and a description (SPEC.md §4)."""
    errors = []
    for name, spec in (doc.get("settings") or {}).items():
        for field in ("label", "description"):
            if not spec.get(field):
                errors.append(f"settings.{name}: no {field}")
    for command, cs in (doc.get("commands") or {}).items():
        for name, spec in ((cs or {}).get("params") or {}).items():
            for field in ("label", "description"):
                if not spec.get(field):
                    errors.append(f"commands.{command}.params.{name}: no {field}")
    return errors


def main() -> int:
    schema = json.loads((ROOT / "schema" / "device-spec-1.json").read_text("utf-8"))
    files = sorted(glob.glob(str(ROOT / "specs" / "*.yaml")))
    if not files:
        print("no specs found in specs/")
        return 1

    failed = False
    seen_ids: dict[str, str] = {}

    for path in files:
        rel = Path(path).relative_to(ROOT).as_posix()
        try:
            doc = yaml.safe_load(Path(path).read_text("utf-8"))
        except yaml.YAMLError as exc:
            failed = True
            print(f"INVALID  {rel}")
            print(f"    YAML parse error (unquoted braces? see CONTRIBUTING.md): {exc}")
            continue

        bad_keys = duplicates(Path(path).read_text("utf-8")) + non_string_keys(doc)
        if bad_keys:
            failed = True
            print(f"INVALID  {rel}")
            for err in bad_keys:
                print(f"    {err}")
            continue

        try:
            jsonschema.validate(doc, schema)
        except jsonschema.ValidationError as exc:
            failed = True
            print(f"INVALID  {rel}")
            print(f"    path: {list(exc.absolute_path)}")
            print(f"    {exc.message}")
            continue

        errors = expand_serial(doc)
        errors += cross_field_checks(doc, rel) + port_checks(doc) + serial_checks(doc) + midi_checks(doc) + label_checks(doc)
        errors += pacing_checks(doc)

        # Spec ids are the consumer-facing contract; they must be unique.
        spec_id = doc["id"]
        if spec_id in seen_ids:
            errors.append(f"duplicate spec id '{spec_id}' (also in {seen_ids[spec_id]})")
        seen_ids[spec_id] = rel

        if errors:
            failed = True
            print(f"INVALID  {rel}")
            for err in errors:
                print(f"    {err}")
        else:
            models = len(doc.get("models", []))
            cmds = len(doc.get("commands") or {})
            print(f"VALID    {rel}  ({models} models, {cmds} commands)")

    print("---")
    print("FAILURES PRESENT" if failed else f"all {len(files)} specs valid")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
