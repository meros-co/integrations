#!/usr/bin/env python3
"""Generate the commands of specs/vmix.yaml, and the command-to-function table
crates/core/src/modules/vmix_functions.rs, from vMix's Shortcut Function
Reference, so every command is one of its functions.

    python tools/generate_vmix.py [path/to/ShortcutFunctionReference.html]

Without a path the page is downloaded from vmix.com. It is not stored here.
Everything in the spec outside `commands:` and the model's `supports` is kept.

A command is the function in snake_case; its parameters are the function's
(Input, Value, Duration, SelectedName, SelectedIndex, Mix, Channel) in
snake_case, all optional, since vMix applies its own defaults and refuses what
it cannot use (FUNCTION ER). Function names do not convert back from
snake_case reliably (PTZMoveUp), so the module looks them up in the table.
"""
import html
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "specs" / "vmix.yaml"
TABLE = ROOT / "crates" / "core" / "src" / "modules" / "vmix_functions.rs"
URL = "https://www.vmix.com/help29/ShortcutFunctionReference.html"

PARAMS = {
    "Input": ('input', "{ type: string, max_length: 256 }"),
    "Value": ('value', "{ type: string, max_length: 4096 }"),
    "Duration": ('duration', "{ type: int, min: 0, max: 600000 }"),
    "SelectedName": ('selected_name', "{ type: string, max_length: 256 }"),
    "SelectedIndex": ('selected_index', "{ type: int, min: 0 }"),
    "Mix": ('mix', "{ type: int, min: 0, max: 16 }"),
    "Channel": ('channel', "{ type: string, max_length: 16 }"),
}


def load() -> str:
    if len(sys.argv) > 1:
        return Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
    request = urllib.request.Request(URL, headers={"User-Agent": "Mozilla/5.0"})
    return urllib.request.urlopen(request, timeout=30).read().decode("utf-8", "replace")


def snake(name: str) -> str:
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name)
    return s.lower()


def yq(s: str) -> str:
    return json.dumps(s, ensure_ascii=False)


def cells(row: str) -> list[str]:
    return [html.unescape(re.sub(r"<[^>]+>", "", c)).strip()
            for c in re.findall(r"<t[dh][^>]*>(.*?)</t[dh]>", row, re.S)]


page = load()
functions = []  # (command, function, section, description, params)
section = ""
for row in re.findall(r"<tr[^>]*>(.*?)</tr>", page, re.S)[1:]:
    c = cells(row)
    if len(c) != 3:
        continue
    name, description, params = c
    if not description and not params:
        section = name
        continue
    params = [p.strip() for p in params.split(",") if p.strip() and p.strip() != "None"]
    unknown = [p for p in params if p not in PARAMS]
    if unknown:
        raise SystemExit(f"{name}: unknown parameters {unknown}")
    functions.append((snake(name), name, section, " ".join(description.split()), params))

# Transitions are functions named as in vMix's interface; the page names Cut
# and Fade, and says the rest follow the interface.
extra = [
    ("cut", "Cut", "Transition", "Cut to the input, or from preview to program", ["Input", "Mix"]),
    ("fade", "Fade", "Transition", "Fade to the input, or from preview to program", ["Duration", "Input", "Mix"]),
]
seen = {f[0] for f in functions}
for e in extra:
    if e[0] not in seen:
        functions.append(e)
seen = {}
for cmd, fn, *_ in functions:
    if cmd in seen and seen[cmd] != fn:
        raise SystemExit(f"{fn} and {seen[cmd]} both become {cmd}")
    seen[cmd] = fn

commands = []
for cmd, fn, sec, desc, params in functions:
    summary = f"{fn} ({sec})" + (f": {desc}" if desc else "")
    lines = [f"  {cmd}:", f"    summary: {yq(summary)}"]
    if params:
        lines.append("    params:")
        for p in params:
            pname, ptype = PARAMS[p]
            lines.append(f"      {pname}: {ptype}")
    lines.append("    returns: ack")
    commands.append("\n".join(lines))
commands.append("""  run_transition:
    summary: "Run a transition by its name in vMix's interface (Fade, Zoom, Wipe, Merge, Stinger1 and so on)"
    params:
      name:     { type: string, pattern: "^[A-Za-z0-9]+$", max_length: 64, required: true }
      duration: { type: int, min: 0, max: 600000 }
      input:    { type: string, max_length: 256 }
      mix:      { type: int, min: 0, max: 16 }
    returns: ack""")

names = [f[0] for f in functions] + ["run_transition"]
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

rows = "\n".join(f'    ("{cmd}", "{fn}"),' for cmd, fn, *_ in sorted(functions))
TABLE.write_text(f'''//! Generated by tools/generate_vmix.py from vMix's Shortcut Function
//! Reference. Do not edit by hand.

/// Command name to vMix function name, sorted by command for binary search.
pub(crate) const FUNCTIONS: &[(&str, &str)] = &[
{rows}
];
''', encoding="utf-8", newline="\n")
print(f"{len(names)} commands")
