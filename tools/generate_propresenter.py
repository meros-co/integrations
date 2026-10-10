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


FORMS = re.compile(r"either (.+?)(?:,? in order of|\.|$)")
LAYERS = {
    "presentation": "presentation",
    "announcement": "announcement",
    "audio": "audio",
}


def forms(doc: str) -> str:
    """'UUID, name, or index' from the document's description, as 'UUID, name or index'."""
    m = FORMS.search(doc)
    return m.group(1).replace(", or ", " or ") if m else "UUID, name or index"


def noun_from_doc(doc: str, path: str) -> str:
    m = re.match(r"The ID of the (?:required )?(.+?), either", doc)
    noun = m.group(1) if m else "item"
    if noun == "item":
        if "/video_inputs/" in path:
            noun = "video input"
        elif "/audio/" in path:
            noun = "audio item"
    if noun == "screen" and "/stage/" in path:
        noun = "stage screen"
    return noun


def describe(name: str, pname: str, p: dict, path: str, doc: str) -> tuple:
    """A label and a one-sentence description for a parameter, written from
    the document's own description of it and the operation's path."""
    if pname == "id":
        noun = noun_from_doc(doc, path)
        return noun[0].upper() + noun[1:], f"The {noun}, by {forms(doc)}."
    if pname == "playlist_id":
        kind = "audio playlist" if "/audio/" in path else "media playlist" if "/media/" in path else "playlist"
        return (kind[0].upper() + kind[1:],
                f"The {kind}, by UUID, name or index, or a path of them for a playlist within folders.")
    if pname == "library_id":
        return "Library", "The library, by name or index (matching by UUID is deprecated)."
    if pname == "presentation_id":
        return "Presentation", "The presentation in the library, by UUID, name or index."
    if pname == "uuid":
        return "Presentation", "The presentation's UUID."
    if pname == "media_id":
        return "Media item", "The media item in the playlist, by UUID, name or index."
    if pname == "group_id":
        return "Group", "The presentation's group (such as Verse 1), by UUID, name or index."
    if pname == "layout_id":
        return "Stage layout", "The stage layout, by name, index or UUID."
    if pname == "theme_slide":
        return "Theme slide", "The theme slide, by name, index or UUID."
    if pname == "index":
        if "/playlist/" in path:
            return "Item index", "Position of the item in the playlist, counted from 0."
        return "Cue index", "Position of the cue (slide) in the presentation, counted from 0, following its selected arrangement."
    if pname == "operation":
        if "/capture/" in path:
            return "Operation", "Start or stop capturing."
        if "/timers/" in path:
            return "Operation", "Start, stop or reset every configured timer."
        if "/timer/" in path:
            return "Operation", "Start, stop or reset the timer."
        return "Operation", "Play, pause or rewind the timeline."
    if pname == "type":
        return "Capture type", "Capture destination: disk, rtmp (streaming) or resi (Resi streaming)."
    if pname == "layer":
        if "/clear/" in path:
            return "Layer", "The layer to clear: audio, props, messages, announcements, slide, media or video input."
        return "Layer", "The transport layer: presentation, announcement or audio."
    if pname == "time":
        if "/timer/" in path:
            return "Seconds", "Seconds to add to the running timer."
        return "Seconds", "Number of seconds to skip."
    if pname == "body":
        return "Body", "JSON in the shape the API document gives for this operation; ProPresenter validates it."
    if pname == "value":
        if "/stage/message" in path:
            return "Message", "The message to show on the stage screens."
        if p["type"] == "float":
            return "Time", "Time in seconds to move to."
    if pname == "enabled":
        what = "stage" if "stage_screens" in path else "audience"
        return "Enabled", f"True enables the {what} screens, false disables them."
    raise KeyError(f"{name}.{pname}: no label written for this parameter")


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
            doc = (prm.get("description") or "").strip()
            if "enum" in schema:
                params[prm["name"]] = {"type": "enum", "values": schema["enum"], "required": True,
                                       "in": prm["in"], "doc": doc}
            elif kind == "integer":
                params[prm["name"]] = {"type": "int", "min": 0, "required": True, "in": prm["in"],
                                       "doc": doc}
            else:
                params[prm["name"]] = {"type": "string", "max_length": 256, "required": True,
                                       "in": prm["in"], "doc": doc}
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
        optional_body = False
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
                # A structured body: passed as JSON, validated by ProPresenter
                # against the schema the document gives for it.
                params["body"] = {"type": "json", "required": True}
                body = "{body}"
            else:
                optional_body = True
        responses = op.get("responses", {})
        status = next((int(c) for c in ("200", "201", "204") if c in responses), 200)
        content = responses.get(str(status), {}).get("content", {})
        returns_value = status == 200 and "application/json" in content
        query = {k: f"{{{k}}}" for k, p in params.items() if p.get("in") == "query"}
        summary = op.get("summary", "").strip()
        if body == "{body}":
            summary += " The body is JSON in the shape the API document gives for this operation."
        commands[name] = {
            "summary": summary,
            "params": params,
            "method": method.upper(),
            "path": op_path,
            "query": query,
            "body": body,
            "status": status,
            "returns": "value" if returns_value else "ack",
        }
        if optional_body:
            # The body is optional (a message's tokens): a second command
            # sends one.
            with_body = dict(commands[name])
            with_body["params"] = {**params, "body": {"type": "json", "required": True}}
            with_body["body"] = "{body}"
            with_body["summary"] = summary + " With a JSON body in the shape the API document gives."
            commands[name + "_with_body"] = with_body

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
            label, description = describe(name, pname, p, c["path"], p.get("doc", ""))
            pad = " " * (len(f"      {pname}: {{ "))
            lines.append(f"      {pname}: {{ {', '.join(fields)},")
            lines.append(f"{pad}label: {label},")
            lines.append(f"{pad}description: {yq(description)} }}")
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
      success status. Image endpoints and chunked streams are not included.

transport:
  type: http
  port: 50001
  scheme: http
  auth: none
  timeout_ms: 4000
  probe: {{ method: GET, path: /version }}

ports:
  - {{ port: 50001, protocol: http, role: control }}

models:
  - id: propresenter-7
    name: ProPresenter 7.9 and later
{chr(10).join(sup_lines)}
    verification: none

commands:
{chr(10).join(out)}

'''

# Telemetry, state and quirks: written by hand, not generated.
TAIL = r'''telemetry:
  # What changes during a service, asked once a second. Their replies, and the
  # replies to the matching get commands, update the state.
  poll:
    every_ms: 1000
    send:
      - { method: GET, path: /v1/status/slide }
      - { method: GET, path: /v1/presentation/slide_index }
      - { method: GET, path: /v1/announcement/slide_index }
      - { method: GET, path: /v1/presentation/focused }
      - { method: GET, path: /v1/status/layers }
      - { method: GET, path: /v1/look/current }
      - { method: GET, path: /v1/playlist/active }
      - { method: GET, path: /v1/playlist/focused }
      - { method: GET, path: /v1/media/playlist/active }
      - { method: GET, path: /v1/media/playlist/focused }
      - { method: GET, path: /v1/audio/playlist/active }
      - { method: GET, path: /v1/audio/playlist/focused }
      - { method: GET, path: /v1/transport/presentation/current }
      - { method: GET, path: /v1/transport/presentation/time }
      - { method: GET, path: /v1/transport/presentation/auto_advance }
      - { method: GET, path: /v1/transport/announcement/current }
      - { method: GET, path: /v1/transport/announcement/time }
      - { method: GET, path: /v1/transport/announcement/auto_advance }
      - { method: GET, path: /v1/transport/audio/current }
      - { method: GET, path: /v1/transport/audio/time }
      - { method: GET, path: /v1/presentation/active/timeline }
      - { method: GET, path: /v1/announcement/active/timeline }
      - { method: GET, path: /v1/timers/current }
      - { method: GET, path: /v1/timer/video_countdown }
      - { method: GET, path: /v1/props }
      - { method: GET, path: /v1/messages }
      - { method: GET, path: /v1/stage/message }
      - { method: GET, path: /v1/stage/layout_map }
      - { method: GET, path: /v1/capture/status }
      - { method: GET, path: /v1/status/audience_screens }
      - { method: GET, path: /v1/status/stage_screens }
  # What is configured, asked on connecting and every 30 s, and again after
  # a command that changes it (the re-read rules below).
  subscribe:
    every_ms: 30000
    send:
      - { method: GET, path: /version }
      - { method: GET, path: /v1/looks }
      - { method: GET, path: /v1/macros }
      - { method: GET, path: /v1/playlists }
      - { method: GET, path: /v1/libraries }
      - { method: GET, path: /v1/media/playlists }
      - { method: GET, path: /v1/audio/playlists }
      - { method: GET, path: /v1/video_inputs }
      - { method: GET, path: /v1/clear/groups }
      - { method: GET, path: /v1/timers }
      - { method: GET, path: /v1/stage/screens }
      - { method: GET, path: /v1/stage/layouts }
      - { method: GET, path: /v1/status/screens }
      - { method: GET, path: /v1/masks }
      - { method: GET, path: /v1/groups }
      - { method: GET, path: /v1/capture/settings }
  updates:
    - path: "^/version$"
      json: { name: "$.name", platform: "$.platform", os: "$.os_version", host: "$.host_description", api: "$.api_version" }
      state:
        device.name: "{name}"
        device.platform: "{platform}"
        device.os_version: "{os}"
        device.host: "{host}"
        device.api_version: "{api}"

    # Slides, presentations and announcements.
    - path: "^/v1/status/slide$"
      json: { current: "$.current.text", current_notes: "$.current.notes", next: "$.next.text", next_notes: "$.next.notes" }
      state:
        slide.current.text: "{current}"
        slide.current.notes: "{current_notes}"
        slide.next.text: "{next}"
        slide.next.notes: "{next_notes}"
    - path: "^/v1/presentation/slide_index$"
      json: { index: "$.presentation_index.index", name: "$.presentation_index.presentation_id.name", uuid: "$.presentation_index.presentation_id.uuid" }
      state:
        presentation.slide_index: "{index}"
        presentation.name: "{name}"
        presentation.uuid: "{uuid}"
    - path: "^/v1/announcement/slide_index$"
      json: { index: "$.announcement_index.index", name: "$.announcement_index.presentation_id.name", uuid: "$.announcement_index.presentation_id.uuid" }
      state:
        announcement.slide_index: "{index}"
        announcement.name: "{name}"
        announcement.uuid: "{uuid}"
    - path: "^/v1/presentation/focused$"
      json: { uuid: "$.uuid", name: "$.name", index: "$.index" }
      state:
        focused_presentation.uuid: "{uuid}"
        focused_presentation.name: "{name}"
        focused_presentation.index: "{index}"
    - path: "^/v1/(presentation|announcement)/active/timeline$"
      json: { running: "$.is_running", time: "$.current_time" }
      state:
        timeline.{1}.running: "{running}"
        timeline.{1}.time: "{time}"

    # Layers and the live look.
    - path: "^/v1/status/layers$"
      json: { video_input: "$.video_input", media: "$.media", slide: "$.slide", announcements: "$.announcements", props: "$.props", messages: "$.messages", audio: "$.audio" }
      state:
        layers.video_input: "{video_input}"
        layers.media: "{media}"
        layers.slide: "{slide}"
        layers.announcements: "{announcements}"
        layers.props: "{props}"
        layers.messages: "{messages}"
        layers.audio: "{audio}"
    - path: "^/v1/look/current$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", screens: "$.screens" }
      state:
        look.uuid: "{uuid}"
        look.name: "{name}"
        look.index: "{index}"
        look.screens: "{screens}"

    # Playlists: the active and focused ones, and the item in them.
    - path: "^/v1/playlist/active$"
      json:
        p_uuid: "$.presentation.playlist.uuid"
        p_name: "$.presentation.playlist.name"
        p_item_uuid: "$.presentation.item.uuid"
        p_item_name: "$.presentation.item.name"
        p_item_index: "$.presentation.item.index"
        a_uuid: "$.announcements.playlist.uuid"
        a_name: "$.announcements.playlist.name"
        a_item_uuid: "$.announcements.item.uuid"
        a_item_name: "$.announcements.item.name"
        a_item_index: "$.announcements.item.index"
      state:
        playlist.active.presentation.uuid: "{p_uuid}"
        playlist.active.presentation.name: "{p_name}"
        playlist.active.presentation.item_uuid: "{p_item_uuid}"
        playlist.active.presentation.item_name: "{p_item_name}"
        playlist.active.presentation.item_index: "{p_item_index}"
        playlist.active.announcements.uuid: "{a_uuid}"
        playlist.active.announcements.name: "{a_name}"
        playlist.active.announcements.item_uuid: "{a_item_uuid}"
        playlist.active.announcements.item_name: "{a_item_name}"
        playlist.active.announcements.item_index: "{a_item_index}"
    - path: "^/v1/playlist/focused$"
      json: { uuid: "$.playlist.uuid", name: "$.playlist.name", item_uuid: "$.item.uuid", item_name: "$.item.name", item_index: "$.item.index" }
      state:
        playlist.focused.uuid: "{uuid}"
        playlist.focused.name: "{name}"
        playlist.focused.item_uuid: "{item_uuid}"
        playlist.focused.item_name: "{item_name}"
        playlist.focused.item_index: "{item_index}"
    - path: "^/v1/(media|audio)/playlist/active$"
      json: { uuid: "$.playlist.uuid", name: "$.playlist.name", item_uuid: "$.item.uuid", item_name: "$.item.name", item_index: "$.item.index" }
      state:
        "{1}_playlist.active.uuid": "{uuid}"
        "{1}_playlist.active.name": "{name}"
        "{1}_playlist.active.item_uuid": "{item_uuid}"
        "{1}_playlist.active.item_name": "{item_name}"
        "{1}_playlist.active.item_index": "{item_index}"
    - path: "^/v1/(media|audio)/playlist/focused$"
      json: { uuid: "$.uuid", name: "$.name", index: "$.index" }
      state:
        "{1}_playlist.focused.uuid": "{uuid}"
        "{1}_playlist.focused.name": "{name}"
        "{1}_playlist.focused.index": "{index}"

    # Transport of the presentation, announcement and audio layers.
    - path: "^/v1/transport/(presentation|announcement|audio)/current$"
      json: { playing: "$.is_playing", uuid: "$.uuid", name: "$.name", artist: "$.artist", audio_only: "$.audio_only", duration: "$.duration" }
      state:
        transport.{1}.playing: "{playing}"
        transport.{1}.uuid: "{uuid}"
        transport.{1}.name: "{name}"
        transport.{1}.artist: "{artist}"
        transport.{1}.audio_only: "{audio_only}"
        transport.{1}.duration: "{duration}"
    - path: "^/v1/transport/(presentation|announcement|audio)/time$"
      json: { time: "$" }
      state: { "transport.{1}.time": "{time}" }
    - path: "^/v1/transport/(presentation|announcement)/auto_advance$"
      json: { advance: "$" }
      state: { "transport.{1}.auto_advance": "{advance}" }

    # Timers: every configured timer. Each read replaces the last, so a
    # deleted timer leaves.
    - path: "^/v1/timers/current$"
      replace: timers
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", time: "$.time", state: "$.state" }
      state:
        timers.{uuid}.name: "{name}"
        timers.{uuid}.time: "{time}"
        timers.{uuid}.state: "{state}"
    - path: "^/v1/timers$"
      replace: timer_settings
      json_each: "$"
      json:
        uuid: "$.id.uuid"
        name: "$.id.name"
        index: "$.id.index"
        overrun: "$.allows_overrun"
        duration: "$.countdown.duration"
        time_of_day: "$.count_down_to_time.time_of_day"
        period: "$.count_down_to_time.period"
        start: "$.elapsed.start_time"
        end: "$.elapsed.end_time"
      state:
        timer_settings.{uuid}.name: "{name}"
        timer_settings.{uuid}.index: "{index}"
        timer_settings.{uuid}.allows_overrun: "{overrun}"
        timer_settings.{uuid}.countdown_duration: "{duration}"
        timer_settings.{uuid}.time_of_day: "{time_of_day}"
        timer_settings.{uuid}.period: "{period}"
        timer_settings.{uuid}.elapsed_start: "{start}"
        timer_settings.{uuid}.elapsed_end: "{end}"
    - path: "^/v1/timer/video_countdown$"
      json: { time: "$" }
      state: { video_countdown: "{time}" }

    # Props and messages, with which are showing.
    - path: "^/v1/props$"
      replace: props
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", active: "$.is_active", auto_clear: "$.auto_clear_enabled", duration: "$.auto_clear_duration" }
      state:
        props.{uuid}.name: "{name}"
        props.{uuid}.index: "{index}"
        props.{uuid}.active: "{active}"
        props.{uuid}.auto_clear_enabled: "{auto_clear}"
        props.{uuid}.auto_clear_duration: "{duration}"
    - path: "^/v1/messages$"
      replace: messages
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", text: "$.message", active: "$.is_active", network: "$.visible_on_network", theme: "$.theme.name" }
      state:
        messages.{uuid}.name: "{name}"
        messages.{uuid}.index: "{index}"
        messages.{uuid}.text: "{text}"
        messages.{uuid}.active: "{active}"
        messages.{uuid}.visible_on_network: "{network}"
        messages.{uuid}.theme: "{theme}"

    # Stage.
    - path: "^/v1/stage/message$"
      json: { message: "$" }
      state: { stage_message: "{message}" }
    - path: "^/v1/stage/layout_map$"
      replace: stage.layout_map
      json_each: "$"
      json: { screen: "$.screen.uuid", screen_name: "$.screen.name", layout: "$.layout.uuid", layout_name: "$.layout.name" }
      state:
        stage.layout_map.{screen}.screen_name: "{screen_name}"
        stage.layout_map.{screen}.layout_uuid: "{layout}"
        stage.layout_map.{screen}.layout_name: "{layout_name}"
    - path: "^/v1/stage/screens$"
      replace: stage.screens
      json_each: "$"
      json: { uuid: "$.uuid", name: "$.name", index: "$.index" }
      state:
        stage.screens.{uuid}.name: "{name}"
        stage.screens.{uuid}.index: "{index}"
    - path: "^/v1/stage/layouts$"
      replace: stage.layouts
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index" }
      state:
        stage.layouts.{uuid}.name: "{name}"
        stage.layouts.{uuid}.index: "{index}"

    # Capture and screens.
    - path: "^/v1/capture/status$"
      json: { status: "$.status", time: "$.capture_time" }
      state:
        capture.status: "{status}"
        capture.time: "{time}"
    # The RTMP stream key is in these settings; it is never kept.
    - path: "^/v1/capture/settings$"
      json:
        source: "$.source"
        disk_location: "$.disk.file_location"
        disk_encoding: "$.disk.encoding"
        disk_width: "$.disk.resolution.width"
        disk_height: "$.disk.resolution.height"
        disk_rate: "$.disk.frame_rate"
        rtmp_url: "$.rtmp.url"
        rtmp_encoding: "$.rtmp.encoding"
        rtmp_save_local: "$.rtmp.save_local"
        rtmp_location: "$.rtmp.file_location"
        resi_event: "$.resi.event_name"
        resi_encoding: "$.resi.encoding"
      state:
        capture.settings.source: "{source}"
        capture.settings.disk.file_location: "{disk_location}"
        capture.settings.disk.encoding: "{disk_encoding}"
        capture.settings.disk.width: "{disk_width}"
        capture.settings.disk.height: "{disk_height}"
        capture.settings.disk.frame_rate: "{disk_rate}"
        capture.settings.rtmp.url: "{rtmp_url}"
        capture.settings.rtmp.encoding: "{rtmp_encoding}"
        capture.settings.rtmp.save_local: "{rtmp_save_local}"
        capture.settings.rtmp.file_location: "{rtmp_location}"
        capture.settings.resi.event_name: "{resi_event}"
        capture.settings.resi.encoding: "{resi_encoding}"
    - path: "^/v1/status/audience_screens$"
      json: { enabled: "$" }
      state: { audience_screens: "{enabled}" }
    - path: "^/v1/status/stage_screens$"
      json: { enabled: "$" }
      state: { stage_screens: "{enabled}" }
    - path: "^/v1/status/screens$"
      replace: screens
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.screen_type", width: "$.size.width", height: "$.size.height" }
      state:
        screens.{uuid}.name: "{name}"
        screens.{uuid}.index: "{index}"
        screens.{uuid}.type: "{type}"
        screens.{uuid}.width: "{width}"
        screens.{uuid}.height: "{height}"

    # What is configured: each list replaces the last.
    - path: "^/v1/(looks|macros|libraries|masks|groups)$"
      replace: "{1}"
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index" }
      state:
        "{1}.{uuid}.name": "{name}"
        "{1}.{uuid}.index": "{index}"
    - path: "^/v1/video_inputs$"
      replace: video_inputs
      json_each: "$"
      json: { uuid: "$.uuid", name: "$.name", index: "$.index" }
      state:
        video_inputs.{uuid}.name: "{name}"
        video_inputs.{uuid}.index: "{index}"
    - path: "^/v1/clear/groups$"
      replace: clear_groups
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", icon: "$.icon", layers: "$.layers" }
      state:
        clear_groups.{uuid}.name: "{name}"
        clear_groups.{uuid}.index: "{index}"
        clear_groups.{uuid}.icon: "{icon}"
        clear_groups.{uuid}.layers: "{layers}"
    # Playlists in folders, three levels deep: each playlist or folder by
    # UUID, with the folder it is in.
    - path: "^/v1/playlists$"
      replace: playlists
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.type" }
      state:
        playlists.{uuid}.name: "{name}"
        playlists.{uuid}.index: "{index}"
        playlists.{uuid}.type: "{type}"
    - path: "^/v1/playlists$"
      json_each: "$[*].playlists"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.type", parent: "$^.id.uuid" }
      state:
        playlists.{uuid}.name: "{name}"
        playlists.{uuid}.index: "{index}"
        playlists.{uuid}.type: "{type}"
        playlists.{uuid}.parent: "{parent}"
    - path: "^/v1/playlists$"
      json_each: "$[*].playlists[*].playlists"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.type", parent: "$^.id.uuid" }
      state:
        playlists.{uuid}.name: "{name}"
        playlists.{uuid}.index: "{index}"
        playlists.{uuid}.type: "{type}"
        playlists.{uuid}.parent: "{parent}"
    - path: "^/v1/(media|audio)/playlists$"
      replace: "{1}_playlists"
      json_each: "$"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.type" }
      state:
        "{1}_playlists.{uuid}.name": "{name}"
        "{1}_playlists.{uuid}.index": "{index}"
        "{1}_playlists.{uuid}.type": "{type}"
    - path: "^/v1/(media|audio)/playlists$"
      json_each: "$[*].children"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.type", parent: "$^.id.uuid" }
      state:
        "{1}_playlists.{uuid}.name": "{name}"
        "{1}_playlists.{uuid}.index": "{index}"
        "{1}_playlists.{uuid}.type": "{type}"
        "{1}_playlists.{uuid}.parent": "{parent}"
    - path: "^/v1/(media|audio)/playlists$"
      json_each: "$[*].children[*].children"
      json: { uuid: "$.id.uuid", name: "$.id.name", index: "$.id.index", type: "$.type", parent: "$^.id.uuid" }
      state:
        "{1}_playlists.{uuid}.name": "{name}"
        "{1}_playlists.{uuid}.index": "{index}"
        "{1}_playlists.{uuid}.type": "{type}"
        "{1}_playlists.{uuid}.parent": "{parent}"

    # Re-reads after a command. A trigger, focus, clear or transport command
    # changes what is live: read it now rather than at the next poll.
    - path: "(/trigger|/focus)$|^/v1/trigger/|^/v1/clear/layer/|^/v1/(message|prop)/[^/]+/(clear|auto_clear/pause|auto_clear/resume)$|/timeline/(play|pause|rewind)$|^/v1/transport/[a-z]+/(play|pause|go_to_end|skip_forward/[^/]+|skip_backward/[^/]+)$"
      json: {}
      then_send:
        - { method: GET, path: /v1/status/slide }
        - { method: GET, path: /v1/presentation/slide_index }
        - { method: GET, path: /v1/status/layers }
        - { method: GET, path: /v1/look/current }
        - { method: GET, path: /v1/playlist/active }
        - { method: GET, path: /v1/presentation/focused }
    - path: "^/v1/(timer/[^/]+/(start|stop|reset|increment/[^/]+)|timers/(start|stop|reset))$"
      json: {}
      then_send: [{ method: GET, path: /v1/timers/current }]
    - path: "^/v1/capture/(start|stop)$"
      json: {}
      then_send: [{ method: GET, path: /v1/capture/status }]
    # Configuration written with a JSON body (PUT and POST) on a path that is
    # also polled has its list read again; a GET carries no body, so the
    # poll's own replies never ask. Paths that are never polled are re-read
    # with or without a body (further down), so each request is queued once.
    - path: "^/v1/look/[^/]+$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/look/current }]
    - path: "^/v1/looks$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/looks }]
    - path: "^/v1/macro_collections$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/macros }]
    - path: "^/v1/prop_collections$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/props }]
    - path: "^/v1/messages$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/messages }]
    - path: "^/v1/clear/groups$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/clear/groups }]
    - path: "^/v1/(playlist/[^/]+|playlists)$"
      request_match: { "$": "^[\\[{]" }
      json: {}
      then_send: [{ method: GET, path: /v1/playlists }]
    - path: "^/v1/timers$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/timers }, { method: GET, path: /v1/timers/current }]
    - path: "^/v1/timer/[^/]+(/(start|stop|reset))?$"
      request_match: { "$": "^\\{" }
      json: {}
      then_send: [{ method: GET, path: /v1/timers }]
    - path: "^/v1/stage/layout_map$"
      request_match: { "$": "^\\[" }
      json: {}
      then_send: [{ method: GET, path: /v1/stage/layout_map }]
    # A DELETE carries no body. These paths are never polled, so a reply on
    # them is a command's: a delete, or a get of one item.
    - path: "^/v1/(macro|macro_collection)/[^/]+$"
      json: {}
      then_send: [{ method: GET, path: /v1/macros }]
    - path: "^/v1/(prop|prop_collection)/[^/]+$"
      json: {}
      then_send: [{ method: GET, path: /v1/props }]
    - path: "^/v1/message/[^/]+$"
      json: {}
      then_send: [{ method: GET, path: /v1/messages }]
    - path: "^/v1/clear/group/[^/]+$"
      json: {}
      then_send: [{ method: GET, path: /v1/clear/groups }]
    - path: "^/v1/stage/layout/[^/]+$"
      json: {}
      then_send: [{ method: GET, path: /v1/stage/layouts }, { method: GET, path: /v1/stage/layout_map }]
    - path: "^/v1/stage/screen/[^/]+/layout/[^/]+$"
      json: {}
      then_send: [{ method: GET, path: /v1/stage/layout_map }]
    # /v1/look/current is polled, so it is left out by name: any other look.
    - path: "^/v1/look/(?:[^c/][^/]*|c(?:[^u/][^/]*)?|cu(?:[^r/][^/]*)?|cur(?:[^r/][^/]*)?|curr(?:[^e/][^/]*)?|curre(?:[^n/][^/]*)?|curren(?:[^t/][^/]*)?|current[^/]+)$"
      json: {}
      then_send: [{ method: GET, path: /v1/looks }]

state:
  device.name:              { type: string, description: "Application name" }
  device.platform:          { type: string, description: "win, mac or unknown" }
  device.os_version:        { type: string, description: "Operating system version" }
  device.host:              { type: string, description: "Host description" }
  device.api_version:       { type: string, description: "API version" }
  slide.current.text:       { type: string, description: "Text of the live slide" }
  slide.current.notes:      { type: string, description: "Notes of the live slide" }
  slide.next.text:          { type: string, description: "Text of the next slide" }
  slide.next.notes:         { type: string, description: "Notes of the next slide" }
  presentation.name:        { type: string, description: "Active presentation" }
  presentation.uuid:        { type: string, description: "Active presentation's UUID" }
  presentation.slide_index: { type: int, description: "Index of the live slide in the active presentation" }
  announcement.name:        { type: string, description: "Active announcement presentation" }
  announcement.uuid:        { type: string, description: "Active announcement presentation's UUID" }
  announcement.slide_index: { type: int, description: "Index of the live cue in the active announcement" }
  focused_presentation.uuid:  { type: string, description: "Presentation focused in the main window" }
  focused_presentation.name:  { type: string, description: "Name of the focused presentation" }
  focused_presentation.index: { type: int, description: "Index of the focused presentation" }
  timeline.*.running:       { type: bool, description: "Timeline of the active presentation or announcement running, keyed presentation or announcement" }
  timeline.*.time:          { type: float, unit: s, description: "Timeline position" }
  layers.video_input:       { type: bool, description: "Video input layer active" }
  layers.media:             { type: bool, description: "Media layer active" }
  layers.slide:             { type: bool, description: "Slide layer active" }
  layers.announcements:     { type: bool, description: "Announcements layer active" }
  layers.props:             { type: bool, description: "Props layer active" }
  layers.messages:          { type: bool, description: "Messages layer active" }
  layers.audio:             { type: bool, description: "Audio layer active" }
  look.uuid:                { type: string, description: "Live audience look" }
  look.name:                { type: string, description: "Name of the live look" }
  look.index:               { type: int, description: "Index of the live look" }
  look.screens:             { type: string, description: "The live look's screens as JSON text, as look_current_put takes them" }
  playlist.active.*.uuid:        { type: string, description: "Active playlist for the destination, keyed presentation or announcements" }
  playlist.active.*.name:        { type: string, description: "Name of the active playlist" }
  playlist.active.*.item_uuid:   { type: string, description: "Active item in it" }
  playlist.active.*.item_name:   { type: string, description: "Name of the active item" }
  playlist.active.*.item_index:  { type: int, description: "Index of the active item" }
  playlist.focused.uuid:         { type: string, description: "Focused playlist" }
  playlist.focused.name:         { type: string, description: "Name of the focused playlist" }
  playlist.focused.item_uuid:    { type: string, description: "Focused item in it" }
  playlist.focused.item_name:    { type: string, description: "Name of the focused item" }
  playlist.focused.item_index:   { type: int, description: "Index of the focused item" }
  media_playlist.active.uuid:       { type: string, description: "Active media playlist" }
  media_playlist.active.name:       { type: string, description: "Name of the active media playlist" }
  media_playlist.active.item_uuid:  { type: string, description: "Active media item" }
  media_playlist.active.item_name:  { type: string, description: "Name of the active media item" }
  media_playlist.active.item_index: { type: int, description: "Index of the active media item" }
  media_playlist.focused.uuid:      { type: string, description: "Focused media playlist" }
  media_playlist.focused.name:      { type: string, description: "Name of the focused media playlist" }
  media_playlist.focused.index:     { type: int, description: "Index of the focused media playlist" }
  audio_playlist.active.uuid:       { type: string, description: "Active audio playlist" }
  audio_playlist.active.name:       { type: string, description: "Name of the active audio playlist" }
  audio_playlist.active.item_uuid:  { type: string, description: "Active audio item" }
  audio_playlist.active.item_name:  { type: string, description: "Name of the active audio item" }
  audio_playlist.active.item_index: { type: int, description: "Index of the active audio item" }
  audio_playlist.focused.uuid:      { type: string, description: "Focused audio playlist" }
  audio_playlist.focused.name:      { type: string, description: "Name of the focused audio playlist" }
  audio_playlist.focused.index:     { type: int, description: "Index of the focused audio playlist" }
  transport.*.playing:      { type: bool, description: "Content playing on the layer, keyed presentation, announcement or audio" }
  transport.*.uuid:         { type: string, description: "UUID of the content on the layer" }
  transport.*.name:         { type: string, description: "Name of the content on the layer" }
  transport.*.artist:       { type: string, description: "Artist of the content, for audio" }
  transport.*.audio_only:   { type: bool, description: "The content is audio only" }
  transport.*.duration:     { type: float, unit: s, description: "Length of the content" }
  transport.*.time:         { type: float, unit: s, description: "Transport position" }
  transport.*.auto_advance: { type: bool, description: "Auto-advance active (presentation and announcement layers)" }
  timers.*.name:            { type: string, description: "Timer name, keyed by the timer's UUID" }
  timers.*.time:            { type: string, description: "Time shown, such as 00:21:43 or -00:00:02" }
  timers.*.state:           { type: string, description: "stopped, running, complete, overrunning or overran" }
  timer_settings.*.name:    { type: string, description: "Timer name, keyed by the timer's UUID" }
  timer_settings.*.index:   { type: int, description: "Timer index" }
  timer_settings.*.allows_overrun: { type: bool, description: "The timer runs on past zero" }
  timer_settings.*.countdown_duration: { type: int, unit: s, description: "Duration of a countdown timer" }
  timer_settings.*.time_of_day: { type: int, unit: s, description: "Time of day a count-down-to-time timer counts to, in seconds" }
  timer_settings.*.period:  { type: string, description: "am, pm or 24_hour, for a count-down-to-time timer" }
  timer_settings.*.elapsed_start: { type: int, unit: s, description: "Start time of an elapsed timer" }
  timer_settings.*.elapsed_end: { type: int, unit: s, description: "End time of an elapsed timer" }
  video_countdown:          { type: string, description: "Video countdown timer" }
  props.*.name:             { type: string, description: "Prop name, keyed by UUID" }
  props.*.index:            { type: int, description: "Prop index" }
  props.*.active:           { type: bool, description: "Prop showing" }
  props.*.auto_clear_enabled: { type: bool, description: "Prop clears itself after a time" }
  props.*.auto_clear_duration: { type: float, unit: s, description: "Time after which the prop clears itself" }
  messages.*.name:          { type: string, description: "Message name, keyed by UUID" }
  messages.*.index:         { type: int, description: "Message index" }
  messages.*.text:          { type: string, description: "Message text with its token placeholders" }
  messages.*.active:        { type: bool, description: "Message showing" }
  messages.*.visible_on_network: { type: bool, description: "Message shown on the network" }
  messages.*.theme:         { type: string, description: "Name of the message's theme" }
  stage_message:            { type: string, description: "Stage message shown; empty when none" }
  stage.layout_map.*.screen_name: { type: string, description: "Stage screen name, keyed by the screen's UUID" }
  stage.layout_map.*.layout_uuid: { type: string, description: "Stage layout on the screen" }
  stage.layout_map.*.layout_name: { type: string, description: "Name of the stage layout on the screen" }
  stage.screens.*.name:     { type: string, description: "Stage screen, keyed by UUID" }
  stage.screens.*.index:    { type: int, description: "Stage screen index" }
  stage.layouts.*.name:     { type: string, description: "Stage layout, keyed by UUID" }
  stage.layouts.*.index:    { type: int, description: "Stage layout index" }
  capture.status:           { type: string, description: "active, inactive, caution or error" }
  capture.time:             { type: string, description: "Capture running time, hh:mm:ss" }
  capture.settings.source:  { type: string, description: "Capture source" }
  capture.settings.disk.file_location: { type: string, description: "Folder disk captures are written to" }
  capture.settings.disk.encoding: { type: string, description: "Disk capture encoding" }
  capture.settings.disk.width: { type: int, unit: px, description: "Disk capture width" }
  capture.settings.disk.height: { type: int, unit: px, description: "Disk capture height" }
  capture.settings.disk.frame_rate: { type: float, description: "Disk capture frame rate" }
  capture.settings.rtmp.url: { type: string, description: "RTMP server URL; the stream key is never kept" }
  capture.settings.rtmp.encoding: { type: string, description: "RTMP encoding" }
  capture.settings.rtmp.save_local: { type: bool, description: "RTMP capture also saved locally" }
  capture.settings.rtmp.file_location: { type: string, description: "Folder the local RTMP copy is written to" }
  capture.settings.resi.event_name: { type: string, description: "Resi event name" }
  capture.settings.resi.encoding: { type: string, description: "Resi encoding" }
  audience_screens:         { type: bool, description: "Audience screens enabled" }
  stage_screens:            { type: bool, description: "Stage screens enabled" }
  screens.*.name:           { type: string, description: "Screen, keyed by UUID" }
  screens.*.index:          { type: int, description: "Screen index" }
  screens.*.type:           { type: string, description: "audience or stage" }
  screens.*.width:          { type: int, unit: px, description: "Screen width" }
  screens.*.height:         { type: int, unit: px, description: "Screen height" }
  looks.*.name:             { type: string, description: "Saved audience look, keyed by UUID; the live look is look" }
  looks.*.index:            { type: int, description: "Look index" }
  macros.*.name:            { type: string, description: "Macro, keyed by UUID" }
  macros.*.index:           { type: int, description: "Macro index" }
  libraries.*.name:         { type: string, description: "Library, keyed by UUID" }
  libraries.*.index:        { type: int, description: "Library index" }
  masks.*.name:             { type: string, description: "Mask, keyed by UUID" }
  masks.*.index:            { type: int, description: "Mask index" }
  groups.*.name:            { type: string, description: "Global group (such as Verse or Chorus), keyed by UUID" }
  groups.*.index:           { type: int, description: "Group index" }
  video_inputs.*.name:      { type: string, description: "Video input in the video inputs playlist, keyed by UUID" }
  video_inputs.*.index:     { type: int, description: "Video input index" }
  clear_groups.*.name:      { type: string, description: "Clear group, keyed by UUID" }
  clear_groups.*.index:     { type: int, description: "Clear group index" }
  clear_groups.*.icon:      { type: string, description: "Clear group icon" }
  clear_groups.*.layers:    { type: string, description: "Layers the group clears, as JSON text" }
  playlists.*.name:         { type: string, description: "Playlist or folder, keyed by UUID" }
  playlists.*.index:        { type: int, description: "Index within its folder" }
  playlists.*.type:         { type: string, description: "playlist or group (a folder)" }
  playlists.*.parent:       { type: string, description: "UUID of the folder it is in; absent at the top" }
  media_playlists.*.name:   { type: string, description: "Media playlist or folder, keyed by UUID" }
  media_playlists.*.index:  { type: int, description: "Index within its folder" }
  media_playlists.*.type:   { type: string, description: "playlist or group (a folder)" }
  media_playlists.*.parent: { type: string, description: "UUID of the folder it is in; absent at the top" }
  audio_playlists.*.name:   { type: string, description: "Audio playlist or folder, keyed by UUID" }
  audio_playlists.*.index:  { type: int, description: "Index within its folder" }
  audio_playlists.*.type:   { type: string, description: "playlist or group (a folder)" }
  audio_playlists.*.parent: { type: string, description: "UUID of the folder it is in; absent at the top" }

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
      Names can change and indexes move; UUIDs are stable, and state keys
      lists by UUID.

  - models: [all]
    severity: info
    text: >
      Operations that take a structured body (looks, messages with tokens,
      timers, playlists, props, macros, clear groups, stage layout maps) take
      it as a JSON parameter, which ProPresenter validates against the shape
      its API document gives. Image and thumbnail endpoints are not included.

  - models: [all]
    severity: info
    text: >
      State is polled: what changes during a service (slides, the live look,
      active and focused presentations and playlists, transport, timelines,
      timers, props, messages, stage, capture, screens) once a second, and
      what is configured (looks, macros, playlists, libraries, media and audio
      playlists, video inputs, clear groups, timer settings, stage screens and
      layouts, screens, masks, groups, capture settings) on connecting and
      every 30 s. A command that changes either has it read again at once.
      Lists are replaced on every read, so a deleted item leaves state; for
      the lists read once a second (timers, props, messages, the stage layout
      map), every read is a removal and the list again, two state events in a
      row.

  - models: [all]
    severity: info
    text: >
      ProPresenter also streams changes over HTTP (/v1/status/updates, and
      the updates endpoints of playlists and the chord chart): one request
      whose chunked reply carries a JSON object per change. Reading it would
      need the HTTP transport to deliver a reply in parts, which the core does
      not yet do, so state is polled instead.

  - models: [all]
    severity: info
    text: >
      Not kept in state, as the API gives them only on request (the get
      commands): the contents of each library and playlist, the slides of
      the active presentation and announcement, themes, macro and prop
      collections, a macro's actions, and the system time. Auto-advance can
      only be cancelled (transport_layer_auto_advance_delete); the API has no
      way to turn it on. The RTMP stream key in the capture settings is never
      kept.
'''
(ROOT / "specs" / "propresenter.yaml").write_text(doc + TAIL, encoding="utf-8", newline="\n")
print(len(commands), "commands;", len(skipped), "skipped:")
for s in skipped:
    print("  ", s)
print("renamed placeholders:", renamed)
