#!/usr/bin/env python3
"""Generate specs/propresenter.yaml from Renewed Vision's ProPresenter API
document (OpenAPI 3.0), so every command is one of its operations.

    python tools/generate_propresenter.py [path/to/swagger.json]

Without a path the document is downloaded from openapi.propresenter.com. It is
not stored in this repository.
"""
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
URL = "https://openapi.propresenter.com/swagger.json"


def load() -> dict:
    if len(sys.argv) > 1:
        text = Path(sys.argv[1]).read_text(encoding="utf-8")
    else:
        request = urllib.request.Request(URL, headers={"User-Agent": "Mozilla/5.0"})
        text = urllib.request.urlopen(request, timeout=30).read().decode("utf-8")
    # Published as "var openapi_spec = {...}" for the Swagger UI page.
    return json.loads(text[text.index("{"): text.rindex("}") + 1])


spec = load()


def lookup(ref: str):
    cur = spec
    for part in ref[2:].split("/"):
        part = part.replace("~1", "/").replace("~0", "~").replace("%7B", "{").replace("%7D", "}")
        cur = cur[int(part)] if isinstance(cur, list) else cur[part]
    return cur


def resolve(node, depth: int = 0):
    """Inline $refs, which in this document point into other paths."""
    if depth > 25:
        return node
    if isinstance(node, dict):
        if "$ref" in node:
            return resolve(lookup(node["$ref"]), depth + 1)
        return {k: resolve(v, depth + 1) for k, v in node.items()}
    if isinstance(node, list):
        return [resolve(x, depth + 1) for x in node]
    return node


SKIP_PATH = re.compile(r"/thumbnail|/icon$|/updates$|chord_chart")
SKIP_QUERY = {"chunked", "sse"}


def snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def yq(s: str) -> str:
    """A YAML double-quoted scalar."""
    return json.dumps(s, ensure_ascii=False)


commands = {}
skipped = []
renamed = []
for path, ops in sorted(spec["paths"].items()):
    if SKIP_PATH.search(path) or path == "/v1/status/updates":
        continue
    shared = ops.get("parameters", [])
    for method in ("get", "put", "post", "delete"):
        if method not in ops:
            continue
        op = resolve(ops[method])
        if "operationId" in op:
            name = snake(op["operationId"])
        else:
            parts = [x for x in path.split("/")[2:] if x and not x.startswith("{")]
            name = method + "_" + "_".join(parts)
        if name in commands:
            name = f"{name}_{method}"
        params = {}
        for prm in resolve(shared) + op.get("parameters", []):
            if prm["in"] == "query":
                if prm["name"] in SKIP_QUERY or not prm.get("required"):
                    continue
            if prm["in"] not in ("path", "query"):
                continue
            schema = prm.get("schema", {})
            kind = schema.get("type", "string")
            if "enum" in schema:
                params[prm["name"]] = {"type": "enum", "values": schema["enum"], "required": True,
                                       "in": prm["in"]}
            elif kind == "integer":
                params[prm["name"]] = {"type": "int", "min": 0, "required": True, "in": prm["in"]}
            else:
                params[prm["name"]] = {"type": "string", "max_length": 256, "required": True,
                                       "in": prm["in"]}
        # The document names some path placeholders differently from their
        # parameter (/v1/stage/layout/{id} with layout_id): match by position.
        holders = re.findall(r"\{([^}]+)\}", path)
        path_params = [k for k, p in params.items() if p.get("in") == "path"]
        op_path = path
        if holders != path_params and len(holders) == len(path_params):
            for old_name, new_name in zip(holders, path_params):
                op_path = op_path.replace("{" + old_name + "}", "{" + new_name + "}")
            renamed.append(f"{method.upper()} {path}: {holders} -> {path_params}")
        body = None
        rb = op.get("requestBody")
        if rb:
            schema = rb.get("content", {}).get("application/json", {}).get("schema", {})
            kind = schema.get("type")
            if kind == "string":
                params["value"] = {"type": "string", "max_length": 4096, "required": True}
                body = "{value:json}"
            elif kind == "boolean":
                params["enabled"] = {"type": "bool", "required": True}
                body = "{enabled}"
            elif kind == "number":
                params["value"] = {"type": "float", "min": 0, "required": True}
                body = "{value:.3f}"
            elif kind == "integer":
                params["value"] = {"type": "int", "required": True}
                body = "{value}"
            elif rb.get("required", True):
                skipped.append(f"{method.upper()} {path} ({name}): structured request body")
                continue
        responses = op.get("responses", {})
        status = next((int(c) for c in ("200", "201", "204") if c in responses), 200)
        content = responses.get(str(status), {}).get("content", {})
        returns_value = status == 200 and "application/json" in content
        query = {k: f"{{{k}}}" for k, p in params.items() if p.get("in") == "query"}
        commands[name] = {
            "summary": op.get("summary", "").strip(),
            "params": params,
            "method": method.upper(),
            "path": op_path,
            "query": query,
            "body": body,
            "status": status,
            "returns": "value" if returns_value else "ack",
        }

out = []
for name, c in commands.items():
    lines = [f"  {name}:", f"    summary: {yq(c['summary'])}"]
    if c["params"]:
        lines.append("    params:")
        for pname, p in c["params"].items():
            fields = [f"type: {p['type']}"]
            if p["type"] == "enum":
                fields.append("values: [" + ", ".join(yq(v) for v in p["values"]) + "]")
            if "min" in p:
                fields.append(f"min: {p['min']}")
            if "max_length" in p:
                fields.append(f"max_length: {p['max_length']}")
            fields.append("required: true")
            lines.append(f"      {pname}: {{ {', '.join(fields)} }}")
    send = [f"method: {c['method']}", f"path: {yq(c['path'])}"]
    if c["query"]:
        send.append("query: { " + ", ".join(f"{k}: {yq(v)}" for k, v in c["query"].items()) + " }")
    if c["body"]:
        send.append(f"body: {yq(c['body'])}")
        send.append("content_type: application/json")
    lines.append(f"    send: {{ {', '.join(send)} }}")
    if c["returns"] == "value":
        lines.append(f"    expect: {{ status: {c['status']}, json_path: \"$\" }}")
    else:
        lines.append(f"    expect: {{ status: {c['status']} }}")
    lines.append(f"    returns: {c['returns']}")
    out.append("\n".join(lines))

supports = list(commands)
sup_lines, line = [], "    supports: ["
for i, s in enumerate(supports):
    piece = s + (", " if i < len(supports) - 1 else "]")
    if len(line) + len(piece) > 88:
        sup_lines.append(line.rstrip())
        line = "               "
    line += piece
sup_lines.append(line)

doc = f'''spec: 1
id: propresenter
name: ProPresenter 7 (ProPresenter API)
vendor: Renewed Vision
category: playback

# Generated from Renewed Vision's OpenAPI document; regenerate rather than
# editing commands by hand.

source:
  - title: ProPresenter API (OpenAPI 3.0.2, version 1.0)
    author: Renewed Vision
    url: https://openapi.propresenter.com/
    note: >
      Manufacturer document. Every command here is an operation in it, named by
      its operationId, with its path and query parameters, request body and
      success status. Operations whose body is a structured object, image
      endpoints and chunked streams are not included yet.

transport:
  type: http
  port: 50001
  scheme: http
  auth: none
  timeout_ms: 4000
  probe: {{ method: GET, path: /version }}

models:
  - id: propresenter-7
    name: ProPresenter 7.9 and later
{chr(10).join(sup_lines)}
    verification: none

commands:
{chr(10).join(out)}

telemetry:
  # Status endpoints, asked once a second. Their replies, and the replies to
  # the matching get commands, update the state.
  poll:
    every_ms: 1000
    send:
      - {{ method: GET, path: /v1/status/slide }}
      - {{ method: GET, path: /v1/presentation/slide_index }}
      - {{ method: GET, path: /v1/status/layers }}
      - {{ method: GET, path: /v1/timers/current }}
      - {{ method: GET, path: /v1/stage/message }}
      - {{ method: GET, path: /v1/capture/status }}
      - {{ method: GET, path: /v1/status/audience_screens }}
  updates:
    - path: "^/v1/status/slide$"
      json: {{ current: "$.current.text", current_notes: "$.current.notes", next: "$.next.text", next_notes: "$.next.notes" }}
      state:
        slide.current.text: "{{current}}"
        slide.current.notes: "{{current_notes}}"
        slide.next.text: "{{next}}"
        slide.next.notes: "{{next_notes}}"
    - path: "^/v1/presentation/slide_index$"
      json: {{ index: "$.presentation_index.index", name: "$.presentation_index.presentation_id.name", uuid: "$.presentation_index.presentation_id.uuid" }}
      state:
        presentation.slide_index: "{{index}}"
        presentation.name: "{{name}}"
        presentation.uuid: "{{uuid}}"
    - path: "^/v1/status/layers$"
      json: {{ video_input: "$.video_input", media: "$.media", slide: "$.slide", announcements: "$.announcements", props: "$.props", messages: "$.messages", audio: "$.audio" }}
      state:
        layers.video_input: "{{video_input}}"
        layers.media: "{{media}}"
        layers.slide: "{{slide}}"
        layers.announcements: "{{announcements}}"
        layers.props: "{{props}}"
        layers.messages: "{{messages}}"
        layers.audio: "{{audio}}"
    - path: "^/v1/timers/current$"
      json_each: "$"
      json: {{ uuid: "$.id.uuid", name: "$.id.name", time: "$.time", state: "$.state" }}
      state:
        timers.{{uuid}}.name: "{{name}}"
        timers.{{uuid}}.time: "{{time}}"
        timers.{{uuid}}.state: "{{state}}"
    - path: "^/v1/stage/message$"
      json: {{ message: "$" }}
      state: {{ stage_message: "{{message}}" }}
    - path: "^/v1/capture/status$"
      json: {{ status: "$.status", time: "$.capture_time" }}
      state:
        capture.status: "{{status}}"
        capture.time: "{{time}}"
    - path: "^/v1/status/audience_screens$"
      json: {{ enabled: "$" }}
      state: {{ audience_screens: "{{enabled}}" }}

state:
  slide.current.text:       {{ type: string, description: "Text of the live slide" }}
  slide.current.notes:      {{ type: string, description: "Notes of the live slide" }}
  slide.next.text:          {{ type: string, description: "Text of the next slide" }}
  slide.next.notes:         {{ type: string, description: "Notes of the next slide" }}
  presentation.name:        {{ type: string, description: "Active presentation" }}
  presentation.uuid:        {{ type: string, description: "Active presentation's UUID" }}
  presentation.slide_index: {{ type: int, description: "Index of the live slide in the active presentation" }}
  layers.video_input:       {{ type: bool, description: "Video input layer active" }}
  layers.media:             {{ type: bool, description: "Media layer active" }}
  layers.slide:             {{ type: bool, description: "Slide layer active" }}
  layers.announcements:     {{ type: bool, description: "Announcements layer active" }}
  layers.props:             {{ type: bool, description: "Props layer active" }}
  layers.messages:          {{ type: bool, description: "Messages layer active" }}
  layers.audio:             {{ type: bool, description: "Audio layer active" }}
  timers.*.name:            {{ type: string, description: "Timer name, keyed by the timer's UUID" }}
  timers.*.time:            {{ type: string, description: "Time shown, such as 00:21:43 or -00:00:02" }}
  timers.*.state:           {{ type: string, description: "stopped, running, complete or overrun" }}
  stage_message:            {{ type: string, description: "Stage message shown; empty when none" }}
  capture.status:           {{ type: string, description: "active, inactive, caution or error" }}
  capture.time:             {{ type: string, description: "Capture running time, hh:mm:ss" }}
  audience_screens:         {{ type: bool, description: "Audience screens enabled" }}

quirks:
  - models: [all]
    severity: info
    text: >
      The API needs ProPresenter 7.9 or later with the network API enabled
      (Settings, Network). The port is set there; 50001 is assumed here, so
      give the configured port when opening the device.

  - models: [all]
    severity: info
    text: >
      Items are addressed by UUID, name or index wherever the API takes an id.
      Names can change and indexes move; UUIDs are stable.

  - models: [all]
    severity: info
    text: >
      Not included yet: operations that take a structured body (creating or
      updating looks, messages with tokens, timers, playlists, props, macros,
      clear groups, stage layout maps), image and thumbnail endpoints, and the
      chunked or server-sent status streams. State is polled once a second.
'''
(ROOT / "specs" / "propresenter.yaml").write_text(doc, encoding="utf-8", newline="\n")
print(len(commands), "commands;", len(skipped), "skipped:")
for s in skipped:
    print("  ", s)
print("renamed placeholders:", renamed)
