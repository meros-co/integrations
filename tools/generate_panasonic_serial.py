"""Generate the Panasonic AW RS-422 spec and its vector module from the
network spec, since the cameras take the same commands on both.

    python tools/generate_panasonic_serial.py

Reads specs/panasonic-ptz.yaml and tools/vectors/panasonic-ptz.py, and writes
specs/panasonic-ptz-serial.yaml and tools/vectors/panasonic-ptz-serial.py.
Edit this file or the network spec, not the output:
tools/test_generate_panasonic_serial.py fails when the committed files differ
from what it writes.

Panasonic's interface specifications for the AW-UE80/UE50/UE40/UE30,
AW-UE150/HE145, AW-UE100, AW-UE160 and AW-UE20/HE20 give one command set for
IP and RS-422 (chapter 3): a pan-tilt head command is '#' + command + CR on
the serial line (aw_ptz over HTTP), a camera command STX + command + ETX
(aw_cam over HTTP); the replies and update notifications are the same text,
framed the same way (chapter 4-1 and 5-2). The camera answers a camera command
with ACK (06H) first, then the reply. The Web settings CGIs, the camera
information batch and the HTTP event registration have no serial form, and
the HE documents (v1.07, v1.12) describe Ethernet control only, so their
models are not in the serial spec.
"""
import json
import re
import sys
import textwrap
from pathlib import Path
from urllib.parse import unquote

import yaml

TOOLS = Path(__file__).resolve().parent
SRC = TOOLS.parent
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else SRC

NETWORK = "panasonic-ptz"
SERIAL = "panasonic-ptz-serial"
PTZ, CAM = "/cgi-bin/aw_ptz", "/cgi-bin/aw_cam"
STX, ETX, CR, ACK = "\u0002", "\u0003", "\r", "\u0006"

# The models whose interface specification describes the RS-422 protocol
# (chapter 4-1). AW-UE155 is the AW-UE150 with one firmware, as in the
# network spec.
MODELS = ["aw-ue80", "aw-ue50", "aw-ue40", "aw-ue30", "aw-ue150", "aw-ue155", "aw-he145", "aw-ue100",
          "aw-ue160", "aw-ue20", "aw-he20"]

# Network-spec quirks that are about HTTP, the Web settings CGIs or the IP
# update notification registration; the serial ones below replace them. Each
# is named by how its text starts, and a missing one stops the generator.
DROPPED_QUIRKS = [
    "Panasonic asks for a gap of 40 ms between pan-tilt head commands",
    "Errors come back as the reply text, with HTTP 200",
    "At most five clients",
    "The Web settings (set_* CGIs",
    "network_ipv4, network_ipv4_interface and http_port",
    "The CGIs answer key=value lines",
]

SOURCES = [
    ("AW-UE80/UE50/UE40/UE30 HD/4K Integrated Camera Interface Specifications (2025-03-01)",
     "Panasonic Entertainment & Communication Co., Ltd.",
     "https://eww.pass.panasonic.co.jp/pro-av/support/content/guide/DEF/UE80/AW-UE80UE50UE40_InterfaceSpecification_E.pdf",
     "Manufacturer document. Command types (chapter 3, p.5): pan-tilt head commands start with # and end with CR, "
     "camera commands start with STX (02H) and end with ETX (03H), neither needed over IP. Serial communication "
     "(4-1, p.6): RS-422, half duplex, 9600 bps, 8 data bits, 1 stop bit, no parity, no flow control; a pan-tilt "
     "command is answered with its reply, a camera command with ACK (06H) and then its reply; 40 ms between pan-tilt "
     "commands. Update notifications on the serial line (5-2, p.10): pan-tilt ones end with CR, camera ones are "
     "between STX and ETX. Error returns (chapter 7, p.19-20); commands chapter 9."),
    ("HD/4K Integrated Camera Interface Specifications AW-UE150/AW-HE145 (2022-04-01)",
     "Panasonic Connect Co., Ltd.",
     "https://eww.pass.panasonic.co.jp/pro-av/support/content/guide/DEF/UE150/AW-UE150HE145_InterfaceSpecification_E.pdf",
     "Manufacturer document. Chapter 3, 4-1 (RS-422, half duplex, 9600 bps 8N1, no flow control, the same "
     "sequences) and 5-2 as in the AW-UE80 document; commands chapter 9. AW-UE155 is treated as an AW-UE150."),
    ("HD/4K Integrated Camera Interface Specifications AW-UE100 (2022)",
     "Panasonic Connect Co., Ltd.",
     "https://eww.pass.panasonic.co.jp/pro-av/support/content/guide/DEF/UE100/AW-UE100_InterfaceSpecification_E.pdf",
     "Manufacturer document. Chapter 3, 4-1 (RS-422, half duplex, 9600 bps 8N1, no flow control) and 5-2; "
     "commands chapter 9."),
    ("HD/4K Integrated Camera Interface Specifications AW-UE160 (2025-03-01)",
     "Panasonic Entertainment & Communication Co., Ltd.",
     "https://eww.pass.panasonic.co.jp/pro-av/support/content/guide/DEF/UE160/AW-UE160_InterfaceSpecification_E.pdf",
     "Manufacturer document. Chapter 3, 4-1 (RS-422, half duplex, 9600, 38400 or 115200 bps, 8N1, no flow "
     "control; 16 ms between pan-tilt commands) and 5-2; menu SERIAL CONNECTION, BAUD RATE (OVP:04, not "
     "available while tracking data output to serial is on), chapter 8; commands chapter 9."),
    ("HD/4K Integrated Camera Interface Specifications AW-UE20/AW-HE20 (2022-04-01)",
     "Panasonic Connect Co., Ltd.",
     "https://eww.pass.panasonic.co.jp/pro-av/support/content/guide/DEF/UE20/AW-UE20HE20_InterfaceSpecification_E.pdf",
     "Manufacturer document. Chapter 3, 4-1 (RS-422, half duplex, 9600 bps 8N1, no flow control) and 5-2; menu "
     "Serial Com. Protocol (OVP:02, Panasonic or Standard), with Connector Select (OVP:03) and Baud Rate (OVP:04) "
     "for the Standard protocol only, chapter 8."),
    ("HD/4K Integrated Camera Interface Specifications, version 1.12 (2020-04-27), and version 1.07 (2017-01-10)",
     "Panasonic Corporation",
     "https://eww.pass.panasonic.co.jp/pro-av/support/content/guide/DEF/HE50_120_IP/HDIntegratedCamera_InterfaceSpecifications-E.pdf",
     "Manufacturer documents for the AW-HE40/HE50/HE60/HE65/HE70/HE120/HE130 and AW-UE70, read for a serial "
     "chapter: both describe control over Ethernet only (chapter 1), so those models are not in this spec. The "
     "AW-UE4 document has no serial chapter either."),
]

SERIAL_QUIRKS = [
    ("critical",
     "The RS-422 line is half duplex (chapter 4-1): the core sends one message at a time and the next only after "
     "the reply to the one before. Panasonic asks for a gap of 40 ms between pan-tilt head commands (16 ms on "
     "the AW-UE160), which the core keeps (transport.pacing), counted from each reply; a caller streaming "
     "pan_tilt_speed updates faster than that builds a queue behind them, so it should send only the latest "
     "speed."),
    ("warning",
     "Pan-tilt head commands are sent as # + command + CR, camera commands as STX (02H) + command + ETX (03H), and "
     "the replies come back framed the same way (chapter 3 and 4-1). A camera command is answered with ACK (06H) "
     "and then its reply; the ACK is never taken as a reply (reply_match), so a command succeeds on its reply "
     "text, which it names (reply_contains) so that an update notification arriving first is not taken for it."),
    ("warning",
     "Errors are the reply text: ER1:<command> (camera commands, between STX and ETX) or eR1:<command> (pan-tilt "
     "head commands) for an unsupported command, ER2 when busy or in standby, ER3 when a value is out of range "
     "or not allowed in the current state (chapter 7). error_match ends the command at once with the error "
     "text. An AWB or ABB failure notification (ER3:OWS, ER2:OAS and so on) arriving while another command "
     "waits is taken as that command's error."),
    ("warning",
     "Update notifications come on the serial line as the same framed messages, pan-tilt ones ended by CR and "
     "camera ones between STX and ETX (chapter 5-2); there is no registration, so the network spec's "
     "notification port does not apply. Only the IP sequences are documented for the periodic notifications "
     "(version every 60 seconds, errors every 30 seconds, lens positions every 300 ms, chapter 6); the state "
     "rules read them whenever they arrive."),
    ("info",
     "The line runs at 9600 bps, 8 data bits, no parity, 1 stop bit, no flow control (chapter 4-1). The "
     "AW-UE160 can also be set to 38400 or 115200 bps (menu SERIAL CONNECTION, BAUD RATE; serial_baud_rate), "
     "not while tracking data goes out on the serial port; set baud_rate to match. The AW-UE20/HE20 take these "
     "commands with Serial Com. Protocol set to Panasonic (serial_protocol 0); Connector Select and Baud Rate "
     "apply to the Standard protocol only."),
    ("info",
     "The Web settings CGIs (streaming, RTMP, SRT, NDI, ST 2110, network, date and time, restart), the camera "
     "information batch (camdata.html), still pictures and preset thumbnails are network-only: use the "
     "panasonic-ptz spec for them. The AW-HE40/HE50/HE60/HE65/HE70/HE120/HE130, AW-UE70 and AW-UE4 are not in "
     "this spec: their interface specifications describe network control only."),
]


PLAIN = re.compile(r"[A-Za-z_][A-Za-z0-9_ ./()-]*")
YAML_WORDS = {"true", "false", "null", "yes", "no", "on", "off", "y", "n", "~"}


def q(value):
    """A YAML scalar: plain where that reads back as the same string, single
    quotes for a regex (no escapes in them), JSON otherwise (control
    characters)."""
    if value is True:
        return "true"
    if value is False:
        return "false"
    if value is None:
        return "null"
    if isinstance(value, (int, float)):
        return json.dumps(value)
    if PLAIN.fullmatch(value) and value.lower() not in YAML_WORDS and not value.endswith(" "):
        return value
    if all(" " <= c <= "~" for c in value):
        return "'" + value.replace("'", "''") + "'"
    return json.dumps(value, ensure_ascii=False)


PLAIN_KEY = re.compile(r"^[a-z_][a-z0-9_.*]*$")


def key(k):
    return k if PLAIN_KEY.match(k) and k not in ("true", "false", "null", "yes", "no", "on", "off") else q(k)


def flow(value):
    """A flow-style YAML value."""
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{key(k)}: {flow(v)}" for k, v in value.items()) + " }" if value else "{}"
    if isinstance(value, list):
        return "[" + ", ".join(flow(v) for v in value) + "]"
    return q(value)


def folded(text, indent):
    pad = " " * indent
    lines = textwrap.wrap(" ".join(text.split()), 104 - indent, break_long_words=False, break_on_hyphens=False)
    return ">\n" + "\n".join(pad + line for line in lines)


def kind(path):
    if path == PTZ:
        return "ptz"
    if path == CAM:
        return "cam"
    return None


def frame(kind_, text):
    return text + CR if kind_ == "ptz" else STX + text + ETX


def reply_regex(kind_, regex):
    """The network reply regex for the serial line: a camera reply between
    STX and ETX."""
    assert regex.startswith("^") and regex.endswith("$"), regex
    if kind_ == "ptz":
        return regex
    return "^\\x02" + regex[1:-1] + "\\x03$"


def literal_prefix(regex):
    """The fixed text a reply regex starts with: up to the first special
    character, less a character a quantifier makes optional."""
    out = ""
    for c in regex[1:]:
        if c in "?*+{":
            return out[:-1]
        if c in ".^$|()[]\\":
            return out
        out += c
    return out


def models_of(item):
    """The serial models an item is for: all when it names none, else those
    it names (None when it names only network-only models)."""
    names = item.get("models")
    if names is None or names == ["all"]:
        return "all"
    kept = [m for m in names if m in MODELS]
    if not kept:
        return None
    return "all" if kept == MODELS else kept


def message(kind_, item):
    return frame(kind_, item["query"]["cmd"])


def convert_send(send):
    """A network command's send for the serial line, or None when it is not
    an aw_ptz or aw_cam request."""
    items = send if isinstance(send, list) else [send]
    out, kinds = [], set()
    for item in items:
        k = kind(item.get("path"))
        if k is None or item.get("method") != "GET" or set(item["query"]) != {"cmd", "res"}:
            return None, None
        kinds.add(k)
        models = models_of(item)
        if models is None:
            continue
        out.append((models, message(k, item)))
    if len(kinds) != 1 or not out:
        return None, None
    if not isinstance(send, list):
        return out[0][1], kinds.pop()
    return [{"models": m, "message": t} if m != "all" else t for m, t in out], kinds.pop()


def main():
    net = yaml.safe_load((SRC / "specs" / f"{NETWORK}.yaml").read_text(encoding="utf-8"))
    order = [m["id"] for m in net["models"]]
    assert [m for m in order if m in MODELS] == MODELS, "a serial model is missing from the network spec"

    # ── Commands ──
    commands, cmd_kind = {}, {}
    for name, c in net["commands"].items():
        send, k = convert_send(c["send"])
        if send is None:
            continue
        expect = c["expect"]
        assert set(expect) == {"status", "matches"}, (name, expect)
        regex = reply_regex(k, expect["matches"])
        prefix = literal_prefix(expect["matches"])
        new_expect = {}
        contains = (STX + prefix) if k == "cam" else prefix
        if contains:
            new_expect["reply_contains"] = contains
        new_expect["matches"] = regex
        commands[name] = {"summary": c["summary"], "params": c.get("params"), "send": send,
                          "expect": new_expect, "returns": c["returns"]}
        cmd_kind[name] = k

    models = []
    for m in net["models"]:
        if m["id"] not in MODELS:
            continue
        nm = {"id": m["id"], "name": m["name"], "supports": [c for c in m["supports"] if c in commands],
              "verification": m["verification"]}
        if "notes" in m:
            nm["notes"] = m["notes"]
        models.append(nm)
    supported = {c for m in models for c in m["supports"]}
    commands = {n: c for n, c in commands.items() if n in supported}
    # A per-model message for a model that does not support the command is not needed.
    for n, c in commands.items():
        if isinstance(c["send"], list):
            users = [m["id"] for m in models if n in m["supports"]]
            c["send"] = [i for i in c["send"]
                         if isinstance(i, str) or any(u in i["models"] for u in users)]

    used_maps = set(re.findall(r"map\.([a-z_0-9]+)", json.dumps([c["send"] for c in commands.values()])))
    maps = {k: v for k, v in (net.get("maps") or {}).items() if k in used_maps}

    # ── Telemetry ──
    poll = []
    for item in net["telemetry"]["poll"]["send"]:
        k = kind(item.get("path"))
        if k is None:
            continue
        models_ = models_of(item)
        if models_ is None:
            continue
        text = message(k, item)
        poll.append(text if models_ == "all" else {"models": models_, "message": text})

    rules, dropped_paths, kept_paths = [], set(), set()

    def written(rule):
        return {re.sub(r"\{[^}]*\}", "*", p) for p in rule.get("state", {})}

    for r in net["telemetry"]["updates"]:
        models_ = models_of(r)
        if "path" in r or models_ is None:
            dropped_paths |= written(r)
            continue
        assert set(r) <= {"models", "match", "state"}, r
        body = r["match"][1:]
        lead = re.sub(r"^\((?:\?:)?", "", body)[:1]
        k = "ptz" if lead.islower() else "cam"
        nr = {}
        if models_ != "all":
            nr["models"] = models_
        nr["match"] = reply_regex(k, r["match"])
        nr["state"] = r["state"]
        nr["_kind"] = k
        nr["_source"] = r["match"]
        rules.append(nr)
        kept_paths |= written(r)

    def keep_state(path):
        def hit(paths):
            return any(re.fullmatch(re.escape(p).replace(r"\*", "[^.]+"), path)
                       or re.fullmatch(re.escape(path).replace(r"\*", "[^.]+"), p) for p in paths)
        return hit(kept_paths) or not hit(dropped_paths)

    state = {p: v for p, v in net["state"].items() if keep_state(p)}

    # ── Quirks ──
    quirks = []
    found = set()
    for qk in net["quirks"]:
        hit = [d for d in DROPPED_QUIRKS if qk["text"].startswith(d)]
        if hit:
            found.add(hit[0])
            continue
        models_ = models_of(qk)
        if models_ is None:
            continue
        quirks.append({"models": ["all"] if models_ == "all" else models_, "severity": qk["severity"],
                       "text": qk["text"]})
    missing = [d for d in DROPPED_QUIRKS if d not in found]
    assert not missing, f"network-spec quirks not found: {missing}"
    quirks = [{"models": ["all"], "severity": s, "text": t} for s, t in SERIAL_QUIRKS] + quirks

    # ── Vectors ──
    vectors = convert_vectors(net, order, commands, cmd_kind, rules, models, state)

    (OUT / "specs" / f"{SERIAL}.yaml").write_bytes(
        emit(models, maps, commands, state, poll, rules, quirks).encode("utf-8"))
    (OUT / "tools" / "vectors" / f"{SERIAL}.py").write_bytes(("\n".join(vectors) + "\n").encode("utf-8"))
    print(f"{SERIAL}: {len(models)} models, {len(commands)} commands, {len(rules)} rules, "
          f"{sum(1 for v in vectors if v.startswith(('text(', 'telemetry(')))} vectors")


# ── Vectors ───────────────────────────────────────────────────────────────
def convert_vectors(net, order, commands, cmd_kind, rules, models, state_decl):
    calls = []

    def http(spec, command, input, method, target, **extra):
        calls.append(("http", command, input, method, target, extra))

    def telemetry(spec, name, **fields):
        calls.append(("telemetry", name, fields))

    exec(compile((SRC / "tools" / "vectors" / f"{NETWORK}.py").read_text(encoding="utf-8"), "network vectors",
                 "exec"), {"http": http, "telemetry": telemetry, "json": json})

    supports = {m["id"]: set(m["supports"]) for m in models}
    net_supports = {m["id"]: set(m["supports"]) for m in net["models"]}
    out = [f"PS = {SERIAL!r}",
           "# Generated by tools/generate_panasonic_serial.py from the panasonic-ptz vectors, whose targets are",
           "# the interface specifications' own command text: each command as it goes on the RS-422 line, a",
           "# pan-tilt head command as # + command + CR and a camera command between STX and ETX (chapter 3); a",
           "# camera reply comes after ACK (06H) (chapter 4-1)."]
    covered_cmds, inbound, inbound_states, command_wires = set(), [], [], []
    for call in calls:
        if call[0] == "http":
            _, command, input, method, target, extra = call
            if command not in commands:
                continue
            model = extra.get("model")
            if model is None:
                # The model the network vector ran on must be one the serial spec has.
                model = next(m for m in order if command in net_supports[m])
                if model not in supports:
                    if isinstance(commands[command]["send"], list):
                        raise SystemExit(f"vector for {command} ran on {model}, which has no serial port")
                    model = None
            elif model not in supports:
                continue
            if model is not None and command not in supports[model]:
                continue
            path, _, query = target.partition("?")
            params = dict(p.split("=", 1) for p in query.split("&"))
            k = kind(path)
            assert k == cmd_kind[command], (command, target)
            wire = frame(k, unquote(params["cmd"]))
            kw = []
            reply = extra.get("http_reply")
            if reply is not None:
                body = reply["body"]
                ack = ACK if k == "cam" else ""
                kw.append(f"device_reply={pyrepr(ack + frame(k, body))}")
            for name in ("expect_result", "file"):
                if name in extra:
                    kw.append(f"{name}={pyrepr(extra[name])}")
            if "model" in extra:
                kw.append(f"model={pyrepr(extra['model'])}")
            unknown = set(extra) - {"http_reply", "expect_result", "file", "model"}
            assert not unknown, (command, unknown)
            out.append(f"text(PS, {pyrepr(command)}, {pyrepr(input)}, {pyrepr(wire)}"
                       + "".join(", " + k_ for k_ in kw) + ")")
            covered_cmds.add(command)
            command_wires.append(wire.rstrip(CR))
        else:
            _, name, fields = call
            fields = dict(fields)
            msg = fields.pop("inbound_http")
            if kind(msg["path"].split("?")[0]) is None:
                continue
            # Over HTTP both come back as a body; on the line a pan-tilt head
            # reply (lower-case first letter) ends with CR, a camera reply is
            # between STX and ETX.
            k = "ptz" if msg["body"][:1].islower() else "cam"
            model = fields.get("model")
            if model is not None and model not in supports:
                continue
            text = frame(k, msg["body"])
            kw = [f"inbound={pyrepr(text)}"]
            for f, v in fields.items():
                kw.append(f"{f}={pyrepr(v)}")
            out.append(f"telemetry(PS, {pyrepr(name)}, " + ", ".join(kw) + ")")
            inbound.append((model or models[0]["id"], text.rstrip(CR)))
            inbound_states.append(fields["expect_state"])
    # Every command is played at least once.
    missing = sorted(set(commands) - covered_cmds)
    assert not missing, f"commands without a vector: {missing}"
    # The rules below reproduce the network vectors' states, so they can state
    # the state of a rule those vectors do not play.
    for (model, text), stated in zip(inbound, inbound_states):
        got = apply_rules(rules, state_decl, model, text)
        assert got == stated, f"the rules give {got} for {text!r} on {model}, the vector {stated}"
    # Every rule the network vectors leave out gets a message of its own: a
    # command's own text where one is sent that the rule reads (the camera
    # answers a setting with it), otherwise the first text its pattern takes.
    wires = [w for w in command_wires]
    extra, names = [], set()
    for r in rules:
        models_ = r.get("models")
        pattern = re.compile(r["match"])
        if any(pattern.fullmatch(t) and (models_ is None or m in models_) for m, t in inbound):
            continue
        model = models_[0] if models_ else models[0]["id"]
        sample = next((w for w in wires if pattern.fullmatch(w)), None) or sample_of(r["match"])
        assert pattern.fullmatch(sample), (r["match"], sample)
        state = apply_rules(rules, state_decl, model, sample)
        assert state, f"no state for {sample!r} ({r['_source']})"
        name = "rule-" + re.sub(r"[^a-z0-9]+", "-", sample.strip(STX + ETX).lower()).strip("-")
        if models_:
            name += "-" + model
        while name.lower() in names:
            name += "-2"
        names.add(name.lower())
        extra.append(f"telemetry(PS, {pyrepr(name)}, inbound={pyrepr(frame(r['_kind'], sample.strip(STX + ETX)))}, "
                     f"expect_state={pyrepr(state)}" + (f", model={pyrepr(model)}" if models_ else "") + ")")
        inbound.append((model, sample))
    if extra:
        out.append("# The rules the network vectors leave out: one message each, with the state the rules give it.")
        out.extend(extra)
    return out


def sample_of(regex):
    """The first text a regex takes: the first of each alternative and class,
    one of each repetition."""
    import re._parser as sre  # noqa: the standard library's regex parser

    def walk(items):
        s = ""
        for op, av in items:
            name = str(op)
            if name == "LITERAL":
                s += chr(av)
            elif name == "ANY":
                s += "A"
            elif name == "IN":
                s += first_in(av)
            elif name in ("MAX_REPEAT", "MIN_REPEAT"):
                low, _high, item = av
                s += walk(item) * max(low, 1)
            elif name == "SUBPATTERN":
                s += walk(av[-1])
            elif name == "BRANCH":
                s += walk(av[1][0])
            elif name == "AT":
                pass
            else:
                raise SystemExit(f"sample_of: {name} in {regex}")
        return s

    def first_in(av):
        op, val = av[0]
        name = str(op)
        if name == "LITERAL":
            return chr(val)
        if name == "RANGE":
            return chr(val[0])
        if name == "CATEGORY" and "DIGIT" in str(val):
            return "1"
        raise SystemExit(f"sample_of: class {name} in {regex}")

    return walk(sre.parse(regex))


def declared(state_decl, path):
    for p, spec in state_decl.items():
        if re.fullmatch(re.escape(p).replace(r"\*", "[^.]+"), path):
            return spec
    raise SystemExit(f"state path {path} is not declared")


def render(template, groups, path=False):
    """A template with the captures in it; in a state path a numeric capture
    is a number (00 is key 0), as SPEC.md §8 renders it."""
    def one(m):
        n, _, offset = m.group(1).partition(":")
        value = groups[int(n) - 1]
        if offset:
            assert re.fullmatch(r"[+-]\d+", offset), template
            return str(int(value) + int(offset))
        if path and re.fullmatch(r"\d+", value):
            return str(int(value))
        return value
    return re.sub(r"\{([^}]*)\}", one, template)


def apply_rules(rules, state_decl, model, text):
    """The state the rules give a message, as SPEC.md §8 assigns it: every
    matching rule for the model, each value converted to its path's type,
    one that does not convert not assigned."""
    out = {}
    for r in rules:
        if r.get("models") and model not in r["models"]:
            continue
        m = re.fullmatch(r["match"], text)
        if not m:
            continue
        groups = [g or "" for g in m.groups()]
        for path_t, value_t in r["state"].items():
            path = render(path_t, groups, path=True)
            spec = declared(state_decl, path)
            if isinstance(value_t, dict):
                raw = render(value_t["value"], groups)
                if raw not in value_t["map"]:
                    continue
                value = value_t["map"][raw]
            else:
                value = render(value_t, groups)
            kind_ = spec["type"]
            if kind_ == "int":
                if isinstance(value, bool) or not (isinstance(value, int) or re.fullmatch(r"-?\d+", str(value))):
                    continue
                value = int(value)
            elif kind_ == "bool":
                if not isinstance(value, bool):
                    if value not in ("true", "false"):
                        continue
                    value = value == "true"
            elif kind_ == "string":
                if not isinstance(value, str):
                    raise SystemExit(f"{path}: a {type(value).__name__} for a string")
            else:
                raise SystemExit(f"{path}: type {kind_}")
            node = out
            parts = path.split(".")
            for part in parts[:-1]:
                node = node.setdefault(part, {})
            node[parts[-1]] = value
    return out


def pyrepr(value):
    return repr(value)


# ── Emitting the spec ─────────────────────────────────────────────────────
def emit(models, maps, commands, state, poll, rules, quirks):
    o = []
    w = o.append
    w("spec: 1")
    w(f"id: {SERIAL}")
    w("name: Panasonic AW-series PTZ cameras on RS-422")
    w("vendor: Panasonic")
    w("category: camera")
    w("")
    w("# Generated by tools/generate_panasonic_serial.py from specs/panasonic-ptz.yaml: edit those, not this")
    w("# file. The same commands as over IP, on the camera's RS-422 port: a pan-tilt head command is")
    w("# '#' + command + CR, a camera command STX + command + ETX, and the replies and update notifications")
    w("# are framed the same way (chapter 3, 4-1 and 5-2 of each interface specification).")
    w("source:")
    for title, author, url, note in SOURCES:
        w(f"  - title: {q(title)}")
        w(f"    author: {author}")
        w(f"    url: {url}")
        w(f"    note: {folded(note, 6)}")
    w("")
    w("transport:")
    w("  type: line-serial")
    w("  # Each template carries its own markers: CR after a pan-tilt head command, STX and ETX around a camera")
    w("  # command. A reply ends at CR, ETX or the ACK byte; the ACK a camera command gets first is not a reply.")
    w("  framing: delimited")
    w('  open: ""')
    w('  close: ""')
    w("  reply_framing: terminated")
    w(f"  reply_terminators: {flow([CR, ETX, ACK])}")
    w("  reply_match: '^[^\\x06]'")
    w("  error_match: '^\\x02?[Ee]R[1-3]:'")
    w("  timeout_ms: 4000")
    w("  # A gap of 40 ms between pan-tilt head commands (chapter 4, restriction 1; 16 ms on the AW-UE160),")
    w("  # counted from each reply.")
    w("  pacing:")
    w("    from: reply")
    w("    groups:")
    w("      pan_tilt:")
    w('        match: "^#"')
    w("        min_interval_ms: 40")
    w("        by_model: [{ models: [aw-ue160], min_interval_ms: 16 }]")
    w(f"  probe: {q(frame('ptz', '#O'))}")
    w("")
    if maps:
        w("# The AW-UE160 writes format codes as two hex digits (OSA:87:01).")
        w("maps:")
        for k, v in maps.items():
            w(f"  {k}: {flow(v)}")
        w("")
    w("settings:")
    w('  serial_port: { type: string, required: true, max_length: 256, label: Serial port, description: "The host\'s '
      'serial port wired to the camera\'s RS-422 port (through an RS-422 adapter): COM3 on Windows, /dev/ttyUSB0 on '
      'Linux, /dev/cu.usbserial-1420 on macOS." }')
    w('  baud_rate: { type: enum, values: ["9600", "38400", "115200"], default: "9600", label: Baud rate, '
      'description: "Bits per second: 9600 on every model; the AW-UE160 can be set to 38400 or 115200 in its menu '
      '(SERIAL CONNECTION, BAUD RATE)." }')
    w('  data_bits: { type: int, min: 8, max: 8, default: 8, label: Data bits, description: "Bits per character: '
      '8 (chapter 4-1)." }')
    w('  parity: { type: enum, values: [none], default: none, label: Parity, description: "No parity '
      '(chapter 4-1)." }')
    w('  stop_bits: { type: int, min: 1, max: 1, default: 1, label: Stop bits, description: "One stop bit '
      '(chapter 4-1)." }')
    w('  flow_control: { type: enum, values: [none], default: none, label: Flow control, description: "No flow '
      'control (chapter 4-1)." }')
    w("")
    w("ports:")
    w('  - { port: null, protocol: serial, role: control, setting: serial_port, note: "The camera\'s RS-422 port, '
      'half duplex" }')
    w("")
    w("models:")
    for m in models:
        w(f"  - id: {m['id']}")
        w(f"    name: {m['name']}")
        w("    supports:")
        o.extend(textwrap.wrap("[" + ", ".join(m["supports"]) + "]", 106, initial_indent="      ",
                               subsequent_indent="      ", break_long_words=False, break_on_hyphens=False))
        w(f"    verification: {m['verification']}")
        if "notes" in m:
            w(f"    notes: {folded(m['notes'], 6)}")
    w("")
    w("commands:")
    for name, c in commands.items():
        w(f"  {name}:")
        w(f"    summary: {q(c['summary'])}")
        if c["params"]:
            w("    params:")
            for p, spec in c["params"].items():
                w(f"      {key(p)}: {flow(spec)}")
        if isinstance(c["send"], list):
            w("    send:")
            for item in c["send"]:
                w(f"      - {flow(item)}")
        else:
            w(f"    send: {q(c['send'])}")
        w(f"    expect: {flow(c['expect'])}")
        w(f"    returns: {c['returns']}")
    w("")
    w("state:")
    for p, spec in state.items():
        w(f"  {key(p)}: {flow(spec)}")
    w("")
    w("telemetry:")
    w("  # Asked once on connecting; the camera sends later changes as update notifications on the serial")
    w("  # line (chapter 5-2). Do not poll settings at regular intervals (chapter 4, restriction 3).")
    w("  poll:")
    w("    send:")
    for item in poll:
        w(f"      - {flow(item)}")
    w("  updates:")
    for r in rules:
        first = True
        for k in ("models", "match"):
            if k in r:
                w(("    - " if first else "      ") + f"{k}: {flow(r[k])}")
                first = False
        w("      state:")
        for p, v in r["state"].items():
            w(f"        {q(p)}: {flow(v)}")
    w("")
    w("quirks:")
    for qk in quirks:
        w(f"  - models: {flow(qk['models'])}")
        w(f"    severity: {qk['severity']}")
        w(f"    text: {folded(qk['text'], 6)}")
        w("")
    while o[-1] == "":
        o.pop()
    return "\n".join(o) + "\n"


if __name__ == "__main__":
    main()
