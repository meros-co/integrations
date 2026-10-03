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

# {name} or {name:directive:directive...}; SPEC.md §4, "Templates".
PLACEHOLDER = re.compile(r"""\{([A-Za-z0-9_][A-Za-z0-9_.]*)((?::[^{}:"\s]+)*)\}""")

# The closed directive set, keyed by the parameter types each one accepts.
DIRECTIVES: list[tuple[re.Pattern[str], set[str]]] = [
    (re.compile(r"^0\d+d$"), {"int"}),                  # zero-padded integer
    (re.compile(r"^[+-]\d+$"), {"int"}),                # integer offset
    (re.compile(r"^signed$"), {"int"}),                 # explicit sign
    (re.compile(r"^\.\d+f$"), {"float"}),               # fixed decimals
    (re.compile(r"^(on_off|bool01|bool10)$"), {"bool"}),
    (re.compile(r"^(upper|lower|json|url)$"), {"string", "enum"}),
    (re.compile(r"^(to|from)\.[a-z0-9_]+$"), {"int", "float"}),  # a named conversion
]

CONVERSION = re.compile(r"^(to|from)\.([a-z0-9_]+)$")

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
            if "body" in item:
                out.append(("http-body", item["body"]))
    return out


def check_template(
    context: str,
    text: str,
    params: dict,
    settings: dict,
    where: str,
    conditional_setting: str | None = None,
    conversions: dict | None = None,
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
        for directive in directives:
            allowed = next((types for pat, types in DIRECTIVES if pat.match(directive)), None)
            if allowed is None:
                errors.append(f"{where}: unknown directive ':{directive}' on '{name}'")
            elif ptype not in allowed:
                errors.append(
                    f"{where}: directive ':{directive}' does not apply to {ptype} '{name}'"
                )

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
        for name, command in commands.items():
            if "send" not in command:
                errors.append(f"commands.{name}: a spec-driven command requires 'send'")

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
                                     conversions=conversions)
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
            if transport_type not in ("line-tcp", "line-udp"):
                errors.append(f"commands.{name}: expect.reply_contains needs a line transport")
            errors += check_template("text", expect_["reply_contains"], params, settings,
                                     f"commands.{name}.expect.reply_contains")
        if "convert" in expect_:
            if expect_["convert"] not in conversions:
                errors.append(f"commands.{name}: expect.convert names undeclared conversion '{expect_['convert']}'")
            if command.get("returns") != "value":
                errors.append(f"commands.{name}: expect.convert needs returns: value")

        expect = command.get("expect") or {}
        returns = command.get("returns", "ack")
        if not native and returns == "value" and not ({"matches", "json_path", "address"} & set(expect)):
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
                {"ws", "wss"} if transport_type == "ws" else {"http", "https"})]
    if websocket:
        schemes.append(("telemetry.websocket", websocket.get("scheme"), {"ws", "wss"}))
    for where_, scheme, allowed in schemes:
        if not isinstance(scheme, dict):
            continue
        decl = settings.get(scheme.get("setting"))
        if decl is None:
            errors.append(f"{where_}.scheme names unknown setting '{scheme.get('setting')}'")
        elif decl.get("type") != "enum" or set(decl.get("values", [])) - allowed:
            errors.append(f"{where_}.scheme's setting must be an enum of {' and '.join(sorted(allowed))}")
    # accept_invalid_certs only means something over TLS.
    for where_, obj, default in [
            ("transport", transport, "ws" if transport_type == "ws" else "http"),
            ("telemetry.websocket", websocket or {}, "ws")]:
        scheme = obj.get("scheme", default)
        if obj.get("accept_invalid_certs") is True and scheme in ("ws", "http"):
            errors.append(f"{where_}.accept_invalid_certs needs a TLS scheme (wss, https or a setting)")
    if websocket:
        for i, text in enumerate(websocket.get("send", []) if isinstance(websocket.get("send"), list)
                                 else [websocket.get("send")] if websocket.get("send") else []):
            errors += check_template("text", text, {}, settings, f"telemetry.websocket.send[{i}]")
    if transport.get("auth") == "bearer" and "token" not in settings:
        errors.append("auth: bearer needs a 'token' setting")
    if transport.get("auth") in ("basic", "digest") and not {"username", "password"} <= settings.keys():
        errors.append(f"auth: {transport.get('auth')} needs 'username' and 'password' settings")

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
            if "after_prompt" in step and transport_type != "line-tcp":
                errors.append(f"on_connect[{i}]: after_prompt needs a line-tcp transport")
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

    return errors


def telemetry_checks(doc: dict, conversions: dict | None = None) -> list[str]:
    """Every path a telemetry rule writes must be declared in 'state', so its
    type is known, every regex must compile, and every conversion a value
    names must be declared."""
    errors: list[str] = []
    telemetry = doc.get("telemetry") or {}
    transport_type = (doc.get("transport") or {}).get("type")

    poll = (telemetry.get("poll") or {}).get("send", [])
    for i, item in enumerate(poll if isinstance(poll, list) else [poll]):
        if isinstance(item, dict) and "reply_address" in item:
            if transport_type not in ("osc-udp", "osc-tcp"):
                errors.append(f"telemetry.poll.send[{i}]: reply_address needs an OSC transport")
            try:
                re.compile(item["reply_address"])
            except re.error as e:
                errors.append(f"telemetry.poll.send[{i}].reply_address: {e}")

    # JSON messages arrive on a websocket, an event stream, or as the lines of
    # a line transport; HTTP replies are matched with `path`.
    has_json = (transport_type in ("ws", "line-tcp", "line-udp")
                or "websocket" in telemetry or "sse" in telemetry)
    if "sse" in telemetry and transport_type != "http":
        errors.append("telemetry.sse needs an http transport")
    declared = [key.split(".") for key in (doc.get("state") or {})]

    def is_declared(path: str) -> bool:
        parts = ["*" if "{" in seg else seg for seg in path.split(".")]
        return any(
            len(d) == len(parts) and all(a == "*" or a == b for a, b in zip(d, parts))
            for d in declared
        )

    for i, rule in enumerate(telemetry.get("updates", [])):
        where = f"telemetry.updates[{i}]"
        for key in ("match", "header", "each_line", "address", "path"):
            if key in rule:
                try:
                    re.compile(rule[key])
                except re.error as e:
                    errors.append(f"{where}.{key}: {e}")
        for key in ("json_match", "request_match"):
            for jpath, pattern in (rule.get(key) or {}).items():
                if not jpath.startswith("$"):
                    errors.append(f"{where}.{key}: '{jpath}' is not a JSON path")
                try:
                    re.compile(pattern)
                except re.error as e:
                    errors.append(f"{where}.{key}: {e}")
        if "json_match" in rule and "path" not in rule and not has_json:
            errors.append(f"{where}: json_match reads JSON messages; the spec has no websocket, "
                          "event stream or line transport")
        if "request_match" in rule and "path" not in rule:
            errors.append(f"{where}: request_match applies to a path rule")
        if "arg_types" in rule and "address" not in rule:
            errors.append(f"{where}: arg_types applies to an address rule")
        for value in (rule.get("state") or {}).values():
            template = value if isinstance(value, str) else value.get("value", "")
            for m in PLACEHOLDER.finditer(template):
                for d in [d for d in m.group(2).split(":") if d]:
                    c = CONVERSION.match(d)
                    if c and c.group(2) not in (conversions or {}):
                        errors.append(f"{where}: ':{d}' names undeclared conversion '{c.group(2)}'")
        paths = list((rule.get("state") or {}).keys())
        for field in (rule.get("fields") or {}).values():
            paths.append(field if isinstance(field, str) else field.get("path", ""))
        for path in paths:
            if not is_declared(path):
                errors.append(f"{where}: '{path}' is not declared in 'state'")
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
        if t.get("port") not in [e["port"] for e in control]:
            errors.append(f"ports: the transport's port {t.get('port')} is not a control port")
        listen = t.get("listen_port")
        if isinstance(listen, dict) and not any(e.get("setting") == listen["setting"] for e in ports):
            errors.append(f"ports: no entry for the listen port setting '{listen['setting']}'")
        if isinstance(listen, int) and not any(e["port"] == listen and e.get("listener") == "core" for e in ports):
            errors.append(f"ports: no core-listener entry for listen_port {listen}")
        tel = doc.get("telemetry") or {}
        if ("websocket" in tel or "sse" in tel) and not any(e["role"] == "push" for e in ports):
            errors.append("ports: the push channel has no push entry")
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

        bad_keys = non_string_keys(doc)
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

        errors = cross_field_checks(doc, rel) + port_checks(doc)

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
