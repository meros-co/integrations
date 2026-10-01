#!/usr/bin/env python3
"""Generate the commands of specs/obs-studio.yaml from obs-websocket's own
machine-readable protocol description (docs/generated/protocol.json), so every
command is one of its requests.

    python tools/generate_obs.py [path/to/protocol.json]

Without a path the description is downloaded from the obs-websocket
repository. It is read as documentation; no code is used, and it is not stored
here. Everything outside `commands:` and the model's `supports` is kept as
written.

Naming is mechanical and reversible, and the native module relies on it: a
command is the request type in snake_case (GetSceneList -> get_scene_list),
and a parameter is the request field in snake_case (sceneName -> scene_name).
"""
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "specs" / "obs-studio.yaml"
URL = "https://raw.githubusercontent.com/obsproject/obs-websocket/master/docs/generated/protocol.json"


def load() -> dict:
    if len(sys.argv) > 1:
        return json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    return json.loads(urllib.request.urlopen(URL, timeout=30).read().decode("utf-8"))


def snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def camel(name: str, upper: bool) -> str:
    s = "".join(p[:1].upper() + p[1:] for p in name.split("_"))
    return s if upper else s[:1].lower() + s[1:]


def yq(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def bounds(restriction: str) -> dict:
    out = {}
    for part in (restriction or "").split(","):
        m = re.match(r"\s*(>=|<=)\s*(-?[0-9.]+)\s*$", part)
        if m:
            out["min" if m.group(1) == ">=" else "max"] = m.group(2)
    return out


protocol = load()
commands = []
names = []
for r in protocol["requests"]:
    if r.get("deprecated"):
        continue
    request_type = r["requestType"]
    name = snake(request_type)
    assert camel(name, True) == request_type, request_type
    names.append(name)
    summary = " ".join(r["description"].split())
    if r.get("initialVersion") and r["initialVersion"] != "5.0.0":
        summary += f" (obs-websocket {r['initialVersion']} and later.)"
    lines = [f"  {name}:", f"    summary: {yq(summary)}"]
    fields = r["requestFields"]
    # Nested fields (keyModifiers.shift) describe an object parameter: it is
    # one json parameter, and its fields are named in the summary.
    nested = {}
    for f in fields:
        if "." in f["valueName"]:
            parent, child = f["valueName"].split(".", 1)
            nested.setdefault(parent, []).append(child)
    if nested:
        described = "; ".join(
            f"{snake(p)} is an object with {', '.join(c)}" for p, c in nested.items()
        )
        lines[1] = f"    summary: {yq(summary + ' ' + described + '.')}"
    if fields:
        lines.append("    params:")
        for f in fields:
            field = f["valueName"]
            if "." in field:
                continue
            pname = snake(field)
            assert camel(pname, False) == field, field
            kind = {"String": "string", "Boolean": "bool", "Number": "float"}.get(
                f["valueType"], "json")
            spec = [f"type: {kind}"]
            for k, v in bounds(f.get("valueRestrictions")).items():
                spec.append(f"{k}: {v}")
            if not f.get("valueOptional"):
                spec.append("required: true")
            lines.append(f"      {pname}: {{ {', '.join(spec)} }}")
    lines.append(f"    returns: {'value' if r['responseFields'] else 'ack'}")
    commands.append("\n".join(lines))

text = SPEC.read_text(encoding="utf-8")
head, rest = text.split("\ncommands:\n", 1)
tail = rest[rest.index("\nstate:\n"):]

supports, line = [], "    supports: ["
for i, n in enumerate(names):
    piece = n + (", " if i < len(names) - 1 else "]")
    if len(line) + len(piece) > 88:
        supports.append(line.rstrip())
        line = "               "
    line += piece
supports.append(line)
head = re.sub(r"    supports: \[.*?\]\n", "\n".join(supports) + "\n", head, count=1, flags=re.S)

SPEC.write_text(head + "\ncommands:\n" + "\n\n".join(commands) + "\n" + tail,
                encoding="utf-8", newline="\n")
print(f"{len(commands)} commands")
