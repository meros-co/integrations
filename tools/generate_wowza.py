#!/usr/bin/env python3
"""Generate specs/wowza-streaming-engine.yaml from Wowza's published REST API
reference (OpenAPI 3.0), so every command is one of its operations.

    python tools/generate_wowza.py [path/to/openapi.json]

Without a path the document is downloaded from developer.wowza.com. It is not
stored in this repository. The spec's header, quirks, state and telemetry are
written here by hand; the commands are the document's operations, with the
server and virtual host names taken from settings.
"""
import json
import re
import sys
import urllib.request
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
URL = "https://developer.wowza.com/_bundle/docs/wowza-engine/rest-api/@current/openapi.json?download"
OUT = ROOT / "specs" / "wowza-streaming-engine.yaml"


def load() -> dict:
    if len(sys.argv) > 1:
        return json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    request = urllib.request.Request(URL, headers={"User-Agent": "Mozilla/5.0"})
    return json.loads(urllib.request.urlopen(request, timeout=60).read().decode("utf-8"))


doc = load()
SERVER = "/v2/servers/{serverName}"

# ── Names ─────────────────────────────────────────────────────────────────
# A command's name is its operation's summary in snake case. An operation
# offered at several levels (server, virtual host, application) keeps the
# plain name at the application level and is prefixed with its level
# elsewhere; the media cache and publishers' version 3 operations end in _v3.
OVERRIDES = {
    ("POST", SERVER + "/vhosts/{vhostName}/applications/{appName}/instances/{instanceName}/streamrecorders"):
        "create_stream_recorder_from_body",
    ("GET", SERVER + "/vhosts/{vhostName}/applications/{appName}/instances/{instanceName}/streamrecorders/{recorderName}/default"):
        "get_stream_recorder_defaults",
    ("GET", SERVER + "/vhosts/{vhostName}/hostports/{portname}"): "get_host_port",
    ("GET", SERVER + "/vhosts/{vhostName}/tunehostports"): "get_host_ports_tuning",
    ("PUT", SERVER + "/vhosts/{vhostName}/tunehostports"): "update_host_ports_tuning",
    ("GET", SERVER + "/vhosts/{vhostName}/applications/{appName}/transcoder/templates/{templateName}/adv"):
        "get_transcoder_template_adv",
    ("PUT", SERVER + "/vhosts/{vhostName}/applications/{appName}/transcoder/templates/{templateName}/adv"):
        "update_transcoder_template_adv",
    ("GET", SERVER + "/vhosts/{vhostName}/transcoder/templates/{templateName}/adv"):
        "vhost_get_transcoder_template_adv",
    ("PUT", SERVER + "/vhosts/{vhostName}/transcoder/templates/{templateName}/adv"):
        "vhost_update_transcoder_template_adv",
    ("PUT", SERVER + "/vhosts/{vhostName}/transcoder/templates/{templateName}/actions/{action}"):
        "vhost_transcoder_template_actions",
    ("GET", SERVER + "/vhosts/{vhostName}/transcoder/templates/{templateName}/decode/overlays/{overlayName}"):
        "vhost_get_transcoder_overlay_decode",
    ("PUT", SERVER + "/vhosts/{vhostName}/transcoder/templates/{templateName}/decode/overlays/{overlayName}"):
        "vhost_update_transcoder_overlay_decode",
    ("DELETE", SERVER + "/vhosts/{vhostName}/transcoder/templates/{templateName}/decode/overlays/{overlayName}"):
        "vhost_delete_transcoder_overlay_decode",
    ("GET", SERVER + "/vhosts/{vhostName}/applications/{appName}/instances/{instanceName}/incomingstreams/{streamName}/monitoring/current"):
        "get_current_incoming_stream_stats",
}


# Summaries renamed for a clearer command name (the summary still heads the
# command's own summary).
RENAMED = {
    "Get current stats": "get_machine_current_stats",
    "Get historic stats": "get_machine_historic_stats",
    "Get historical stats": "server_get_historic_stats",
    "Reports whether webrtc2 debug logging is enabled": "get_webrtc_debug_log",
    "Enables or disables webrtc2 debug logging": "set_webrtc_debug_log",
    "Get webhook config": "get_webhook_config",
    "Update webhook config": "update_webhook_config",
    "Get filter list": "get_webhook_filter_list",
    "Create filter list": "create_webhook_filters",
    "Get filter": "get_webhook_filter",
    "Update filter": "update_webhook_filter",
    "Delete filter": "delete_webhook_filter",
    "Get source": "get_webhook_source",
    "Update source": "update_webhook_source",
    "Get target list": "get_webhook_target_list",
    "Create target list": "create_webhook_targets",
    "Get target": "get_webhook_target",
    "Update target": "update_webhook_target",
    "Delete target": "delete_webhook_target",
    "Get playback info": "get_test_playback_info",
}


def snake(text: str) -> str:
    if text in RENAMED:
        return RENAMED[text]
    text = text.replace("(adv)", "adv").replace("WEBRTC", "WebRTC")
    text = re.sub(r"[^A-Za-z0-9]+", "_", text).strip("_").lower()
    return text.replace("webrtc", "webrtc")


def level(path: str) -> str:
    if path.startswith("/v3/"):
        return "v3"
    if "/applications/{appName}" in path:
        return "app"
    if "/vhosts/{vhostName}" in path:
        return "vhost"
    if path.startswith(SERVER):
        return "server"
    return "machine"


# ── Parameters ────────────────────────────────────────────────────────────
NAME = {"type": "string", "pattern": "^[^/\\s][^/]*$", "max_length": 256, "required": True}
PARAMS = {
    "appName": ("app", "Application", "The application's name, such as live."),
    "instanceName": ("instance", "Application instance", "The application instance, _definst_ unless the application runs others."),
    "streamName": ("stream", "Stream", "The stream's name, such as myStream."),
    "entryName": ("entry", "Stream target", "The Stream Targets (push publish) map entry's name."),
    "recorderName": ("recorder", "Recorder", "The stream recorder's name, which is the name of the stream it records."),
    "templateName": ("template", "Transcoder template", "The transcoder template's name, such as transrate."),
    "encodeName": ("encode", "Encode", "The transcoder template encode's name, such as 720p."),
    "overlayName": ("overlay", "Overlay", "The transcoder overlay's name."),
    "groupName": ("group", "Group", "The stream group's or stream name group's name."),
    "publisherName": ("publisher", "Publisher", "The publisher's name (a user allowed to publish streams)."),
    "publisher": ("publisher", "Publisher", "The publisher's name (a user allowed to publish streams)."),
    "smilfileName": ("smil_file", "SMIL file", "The SMIL file's name without its .smil extension."),
    "streamfileName": ("stream_file", "Stream file", "The stream file's name without its .stream extension."),
    "sdpfileName": ("sdp_file", "SDP file", "The SDP file's name without its .sdp extension."),
    "sourceName": ("source", "Media cache source", "The media cache source's name."),
    "storeName": ("store", "Media cache store", "The media cache store's name."),
    "userName": ("user", "User", "The Wowza Streaming Engine user's name."),
    "logName": ("log", "Log file", "The log file's name, as get_log_list gives it."),
    "logType": ("log_type", "Log type", "The type of log, as get_log_types gives it, such as wowzastreamingengine_access."),
    "portname": ("port_name", "Host port", "The host port's name, as get_host_ports_list gives it."),
    "baseClass": ("base_class", "HTTP provider", "The HTTP provider's base class, such as com.wowza.wms.http.HTTPServerVersion."),
    "baseclass": ("base_class", "Listener class", "The virtual host listener's base class."),
    "loggerName": ("logger", "Logger", "The log4j logger's name."),
    "mediacastername": ("media_caster", "Media caster", "The media caster's name, as get_media_caster_list gives it."),
    "dvrStoreName": ("dvr_store", "DVR store", "The nDVR store's name, as get_dvr_store_list gives it."),
    "id": ("id", "Webhook entry", "The webhook filter's or target's id."),
}
QUERY = {
    "dstEntryName": ("destination", "Destination", "The name of the copy to make."),
    "connectAppName": ("connect_app", "Connect to application", "The application the file's stream is connected in."),
    "appInstance": ("connect_instance", "Connect to instance", "The application instance the file's stream is connected in, such as _definst_."),
    "mediaCasterType": ("media_caster_type", "Media caster type", "How the stream is pulled: rtp (RTSP/RTP, SRT, MPEG-TS), rtp-record, shoutcast, shoutcast-record or liverepeater."),
    "filename": ("filename", "File", "The file (heap dump or stack trace location, or the media cache item) the action is for."),
    "direction": ("direction", "Direction", "For zoom and panTilt: ZoomIn, ZoomOut, PanTiltUp, PanTiltRight, PanTiltDown or PanTiltLeft."),
    "publishUser": ("publish_user", "Publish user", "The user name the encoder publishes with."),
    "publishHost": ("publish_host", "Publish host", "The address the encoder publishes to."),
    "publishPort": ("publish_port", "Publish port", "The port the encoder publishes to, such as 1935."),
    "order": ("order", "Order", "The order of the log files: newestFirst or oldestFirst."),
    "tail": ("lines", "Lines", "How many of the last lines to return."),
    "dvrConverterStoreList": ("stores", "Stores", "The nDVR stores to convert, separated by commas."),
}
# Optional query parameters offered by a variant of the command, by
# operation; the others are left to their documented defaults.
VARIANTS = {"dstEntryName": "_copy", "connectAppName": "_connect", "filename": "_with_file",
            "direction": "_direction", "order": "_ordered", "tail": "_tail",
            "dvrConverterStoreList": "_stores"}
# connect's application and instance come together.
TOGETHER = {"connectAppName": ["connectAppName", "appInstance"]}

ACTION_TEXT = {
    "copy": "copy", "restart": "restart", "shutdown": "shut down", "start": "start", "stop": "stop",
    "disableWhipWhep": "turn WHIP/WHEP off", "enableWhipWhep": "turn WHIP/WHEP on",
    "disconnectStream": "disconnect the publisher", "resetStream": "reset the stream",
    "captureImage": "capture an image", "panTilt": "pan or tilt", "zoom": "zoom",
    "disconnectGroup": "disconnect the group", "removeGroup": "remove the group", "resetGroup": "reset the group",
    "splitRecording": "start a new file", "stopRecording": "stop recording", "disable": "disable",
    "enable": "enable", "connect": "connect", "reload": "reload", "debug": "debug", "error": "error",
    "fatal": "fatal", "info": "info", "warn": "warn", "heapDump": "write a heap dump",
    "stackTrace": "write a stack trace",
}


def first_sentence(text: str) -> str:
    text = re.sub(r"\s+", " ", text or "").strip()
    text = re.sub(r" ?This API is available in builds \d+ or later\.?", "", text)
    m = re.match(r"(.+?\.)(\s|$)", text)
    return (m.group(1) if m else text).strip()


def param_for(prm: dict, op_path: str) -> tuple:
    """(our name, declaration, wire name) of a path or query parameter."""
    schema = prm.get("schema", {})
    name = prm["name"]
    if name == "action":
        values = [v.strip() for v in schema.get("enum", [])]
        if values:
            what = "; ".join(f"{v} to {ACTION_TEXT.get(v, v)}" for v in values)
            decl = {"type": "enum", "values": values, "required": True, "label": "Action",
                    "description": f"What to do: {what}."}
        else:
            decl = {"type": "string", "pattern": "^[A-Za-z]+$", "max_length": 64, "required": True,
                    "label": "Action", "description": "The action, by the name the API reference gives it."}
        return "action", decl, name
    table = PARAMS if prm["in"] == "path" else QUERY
    if name not in table:
        raise KeyError(f"{op_path}: no label for parameter {name}")
    ours, label, text = table[name]
    if "enum" in schema:
        values = sorted({v.strip() for v in schema["enum"]}, key=[v.strip() for v in schema["enum"]].index)
        decl = {"type": "enum", "values": values, "required": True, "label": label, "description": text}
        if "default" in schema:
            decl["default"] = schema["default"].strip()
            del decl["required"]
    elif name == "instanceName":
        decl = {**NAME, "label": label, "description": text}
        del decl["required"]
        decl["default"] = "_definst_"
    elif name in ("tail", "publishPort"):
        decl = {"type": "int", "min": 1, "max": 1000000 if name == "tail" else 65535, "required": True,
                "label": label, "description": text}
    elif prm["in"] == "query":
        decl = {"type": "string", "pattern": "\\S", "max_length": 1024, "required": True, "label": label,
                "description": text}
    else:
        decl = {**NAME, "label": label, "description": text}
    return ours, decl, name


def build_command(method: str, path: str, op: dict, extra_query: list) -> dict:
    params, query = {}, {}
    wire_path = path.replace("{serverName}", "{settings.server}").replace("{vhostName}", "{settings.vhost}")
    declared = {p["name"] for p in op.get("parameters", []) if p["in"] == "path"}
    # Some operations leave a path placeholder undeclared: it is one all the same.
    implied = [{"in": "path", "name": h, "required": True, "schema": {"type": "string"}}
               for h in re.findall(r"\{([^}]+)\}", path) if h not in declared]
    for prm in implied + op.get("parameters", []):
        if prm["name"] in ("serverName", "vhostName"):
            continue
        if prm["in"] == "query" and not prm.get("required") and prm["name"] not in extra_query:
            continue
        ours, decl, wire = param_for(prm, path)
        params[ours] = decl
        if prm["in"] == "path":
            wire_path = wire_path.replace("{" + wire + "}", "{" + ours + "}")
        else:
            query[wire] = "{" + ours + "}"
    send = {"method": method, "path": wire_path}
    if query:
        send["query"] = query
    body = op.get("requestBody", {}).get("content", {})
    if body:
        schema = next(iter(body.values())).get("schema", {}).get("$ref", "").split("/")[-1]
        params["config"] = {"type": "json", "required": True, "label": "Configuration",
                            "description": f"A JSON object in the API reference's {schema} shape; fields left out keep their values where the reference says so."}
        send["body"] = "{config}"
        send["content_type"] = "application/json"
    summary = (op.get("summary") or "").strip()
    sentence = first_sentence(op.get("description", ""))
    text = sentence if sentence else summary
    if extra_query:
        text = text.rstrip(".") + ", with " + " and ".join(QUERY[q][1].lower() for q in extra_query)
    text = text.rstrip(".")
    cmd = {"summary": f"{text} ({summary})"}
    if params:
        cmd["params"] = params
    cmd["send"] = send
    if method == "GET":
        cmd["expect"] = {"status": 200, "json_path": "$"}
        cmd["returns"] = "value"
    else:
        cmd["expect"] = {"status": [200, 201, 204]}
        cmd["returns"] = "ack"
    return cmd


ops = []
for path, item in sorted(doc["paths"].items()):
    for method in ("get", "put", "post", "delete"):
        if method in item:
            ops.append((method.upper(), path, item[method]))

base = {}
for method, path, op in ops:
    base[(method, path)] = OVERRIDES.get((method, path)) or snake(op.get("summary") or method + path)
counts = {}
for name in base.values():
    counts[name] = counts.get(name, 0) + 1
names = {}
for key, name in base.items():
    lvl = level(key[1])
    if key in OVERRIDES:
        names[key] = name
    elif lvl == "v3":
        names[key] = re.sub(r"_v3$", "", name) + "_v3"
    elif counts[name] > 1 and lvl in ("server", "vhost"):
        names[key] = f"{lvl}_{name}"
    else:
        names[key] = name
seen = {}
for key, name in names.items():
    if name in seen:
        raise SystemExit(f"two operations named {name}: {seen[name]} and {key}")
    seen[name] = key

commands = {}
for method, path, op in ops:
    name = names[(method, path)]
    commands[name] = build_command(method, path, op, [])
    optional = [p["name"] for p in op.get("parameters", []) if p["in"] == "query" and not p.get("required")]
    for wire, suffix in VARIANTS.items():
        if wire in optional:
            group = TOGETHER.get(wire, [wire])
            variant = name + suffix
            if variant in commands or variant in seen:
                raise SystemExit(f"variant {variant} clashes")
            commands[variant] = build_command(method, path, op, group)

# Hand-written commands for the everyday operations, beside the generated
# ones: the request is the same operation with its body written out.
APP = "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{app}"
HAND = {
    "start_recording": {
        "summary": "Start recording a live stream with the application's default recorder settings (Create stream recorder, recorderName the stream)",
        "params": {
            "app": {**NAME, "label": "Application", "description": "The application's name, such as live."},
            "instance": {**{k: v for k, v in NAME.items() if k != "required"}, "default": "_definst_",
                         "label": "Application instance", "description": "The application instance, _definst_ unless the application runs others."},
            "stream": {**NAME, "label": "Stream", "description": "The name of the live stream to record."},
        },
        "send": {"method": "POST", "path": APP + "/instances/{instance}/streamrecorders/{stream}",
                 "body": '{"recorderName":{stream:json},"instanceName":{instance:json},"defaultRecorder":true}',
                 "content_type": "application/json"},
        "expect": {"status": [200, 201, 204]},
        "returns": "ack",
    },
    "stop_recording": {
        "summary": "Stop recording a live stream (Stream recorder actions, stopRecording)",
        "params": {
            "app": {**NAME, "label": "Application", "description": "The application's name, such as live."},
            "instance": {**{k: v for k, v in NAME.items() if k != "required"}, "default": "_definst_",
                         "label": "Application instance", "description": "The application instance, _definst_ unless the application runs others."},
            "stream": {**NAME, "label": "Stream", "description": "The name of the stream whose recording stops."},
        },
        "send": {"method": "PUT", "path": APP + "/instances/{instance}/streamrecorders/{stream}/actions/stopRecording"},
        "expect": {"status": [200, 201, 204]},
        "returns": "ack",
    },
    "set_stream_target_enabled": {
        "summary": "Turn a stream target (push publish entry) on or off (Push publish actions, enable or disable)",
        "params": {
            "app": {**NAME, "label": "Application", "description": "The application's name, such as live."},
            "entry": {**NAME, "label": "Stream target", "description": "The Stream Targets (push publish) map entry's name."},
            "enabled": {"type": "bool", "required": True, "label": "Enabled",
                        "description": "On to send the stream to the target, off to stop sending it."},
        },
        "send": {"method": "PUT", "path": APP + "/pushpublish/mapentries/{entry}/actions/{enabled:map.enable_action}"},
        "expect": {"status": [200, 201, 204]},
        "returns": "ack",
    },
}
for name in HAND:
    if name in commands:
        raise SystemExit(f"hand-written {name} clashes")
commands.update(HAND)


HEADER = r'''# Generated by tools/generate_wowza.py from Wowza's REST API reference; edit
# the generator, not this file.
spec: 1
id: wowza-streaming-engine
name: Wowza Streaming Engine (REST API)
vendor: Wowza Media Systems
category: other

# A media server, on the venue network or in a data center: its REST API
# (the Streaming Engine Manager's own back end) on TCP 8087. Every operation
# of the published reference is a command; the server and virtual host are
# settings, as nearly every installation has one of each (_defaultServer_
# and _defaultVHost_). State follows the server, each application's live
# streams, recorders and stream targets.

source:
  - title: Wowza Streaming Engine REST API reference (OpenAPI 3.0.3, version 2.0.0)
    author: Wowza Media Systems
    url: https://developer.wowza.com/docs/wowza-engine/rest-api/
    note: >
      Manufacturer document, retrieved 2026-10-28 as the reference's
      openapi.json (converted by Wowza from the server's own Swagger 1.2
      document served at /swagger.json on port 8089). 164 paths: /restinfo,
      /v2/machine/monitoring, and under /v2/servers/{serverName} the server
      (actions heapDump, restart, stackTrace, start, stop; adv, licenses,
      listeners, log4j, logs and log files, media cache, media casters,
      monitoring, publishers, status, transcoder, tune, users, virtual host
      listeners, webhooks) and /vhosts/{vhostName} (actions restart, start,
      stop; applications with actions copy, restart, shutdown, start,
      enableWhipWhep, disableWhipWhep; their adv, DRM, DVR, instances with
      DVR stores, incoming streams (actions disconnectStream, resetStream;
      live encoder settings, monitoring, source control actions captureImage,
      panTilt, zoom), stream groups (disconnectGroup, removeGroup,
      resetGroup) and stream recorders (splitRecording, stopRecording);
      modules, monitoring, publishers, push publish map entries (copy,
      disable, enable, restart), SDP, SMIL and stream files (connect, copy),
      security, stream configuration, transcoder and its templates, encodes,
      overlays and stream name groups, WebRTC), host ports, SSL, startup
      streams, testPlaybackInfo and tuning), and version 3 of the media cache
      and publishers. Security schemes: HTTP Basic and Digest with the
      Streaming Engine administrator's credentials. Bodies and answers are
      JSON with Accept and Content-Type application/json.
  - title: Wowza Streaming Engine documentation, "Access the Wowza Streaming Engine REST API" and "Record live streams with the Wowza Streaming Engine REST API"
    author: Wowza Media Systems
    url: https://www.wowza.com/docs/wowza-streaming-engine-rest-api
    note: >
      Manufacturer documents, consulted 2026-10-28 for the access defaults:
      the REST API listens on port 8087 (Server.xml RESTInterface), with
      digest authentication by default and basic or none configurable, and
      a stream recorder is created with POST .../streamrecorders/{stream}
      naming recorderName, instanceName and defaultRecorder true.

transport:
  type: http
  port: 8087
  scheme: { setting: scheme }
  # Digest by default; Server.xml's RESTInterface can be set to basic or
  # none.
  auth: { setting: auth }
  accept_invalid_certs: true
  refusal_status: [401]
  headers:
    Accept: application/json
  timeout_ms: 10000
  # The server's status: one cheap request that also checks the credential.
  probe: { method: GET, path: "/v2/servers/{settings.server}/status" }

settings:
  auth: { type: enum, values: [digest, basic, none], default: digest,
          label: Authentication,
          description: "The REST API's authentication as Server.xml sets it: digest (the default), basic or none." }
  username: { type: string, max_length: 256, default: "",
              label: Username,
              description: "A Streaming Engine administrator's user name." }
  password: { type: string, secret: true, max_length: 256, default: "",
              label: Password,
              description: "That administrator's password." }
  scheme: { type: enum, values: [http, https], default: http,
            label: Connection,
            description: "http, or https where the REST API's host port has SSL turned on." }
  server: { type: string, pattern: "^[^/\\s]+$", max_length: 128, default: _defaultServer_,
            label: Server name,
            description: "The server's name in the API's paths, _defaultServer_ unless renamed." }
  vhost: { type: string, pattern: "^[^/\\s]+$", max_length: 128, default: _defaultVHost_,
           label: Virtual host,
           description: "The virtual host the commands and state follow, _defaultVHost_ unless another is configured." }

ports:
  - { port: 8087, protocol: http, role: control, when: "scheme http", note: "the REST API (Server.xml RESTInterface), which also serves the Streaming Engine Manager" }
  - { port: 8087, protocol: https, role: control, when: "scheme https", note: "the REST API with SSL turned on in its host port" }

maps:
  enable_action: { "true": enable, "false": disable }
'''

TAIL = r'''quirks:
  - models: [all]
    severity: info
    text: >
      Every operation of the published reference is a command, named after
      its summary; one offered at several levels keeps the plain name for
      the application and is prefixed server_ or vhost_ elsewhere, and the
      version 3 media cache and publisher operations end in _v3. The server
      and virtual host in every path are the server and vhost settings. A
      command taking a configuration takes it as JSON in the shape the
      reference's schema gives (its name is in the parameter's
      description); fields left out keep their values where the reference
      says so, and reading the resource first gives the whole shape.
      start_recording, stop_recording and set_stream_target_enabled are the
      same operations with their bodies or actions written out.

  - models: [all]
    severity: info
    text: >
      Optional query parameters are offered by a variant of the command:
      _copy (the copy's name), _connect (the application and instance a
      file's stream is connected in), _with_file (a heap dump's or media
      cache item's file), _direction (pan, tilt and zoom), _ordered (the
      log file list's order), _tail (the last lines of a log) and _stores
      (the nDVR stores a conversion takes). The other optional parameters
      (log offsets, filters and searches; the nDVR converter's options) keep
      the server's defaults. The reference lists the media caster type
      rtp-record with a leading space; it is sent without it.

  - models: [all]
    severity: info
    text: >
      Wowza pushes nothing to the REST API (its webhooks call a URL the
      consumer runs). Every 10 seconds the core reads the machine's and the
      virtual host's current statistics and the application list, and for
      each application its instances (incoming streams, recorders, stream
      groups), its stream targets with their session status, and its
      current statistics, then each incoming stream's statistics: 3 + 3 per
      application + 1 per live stream requests. Every 60 seconds it reads
      the server's status and the startup streams. Each instance's incoming
      streams, recorders and stream groups, each application's stream
      targets, and the startup streams are replaced whole by each read, so
      a stream that stops publishing leaves incoming_streams; its last
      statistics stay in stream_stats until it publishes again. An
      application deleted elsewhere keeps its last values until the
      integration is opened again.

  - models: [all]
    severity: info
    text: >
      Stream target passwords, access tokens and keys (password,
      facebook.accessToken, srtPassPhrase, secureTokenSharedSecret and the
      like) are returned by the get commands but never kept in state or
      logged. A REST user's or publisher's password is likewise only sent.

  - models: [all]
    severity: critical
    text: >
      server_actions stop and restart stop every stream the server carries;
      vhost_actions and application_actions shutdown or restart drop every
      connection of that virtual host or application; delete_app,
      delete_stream_file, delete_smil_file, delete_transcoder_template and
      the other delete_ commands remove configuration that cannot be
      restored except by recreating it; incoming_stream_actions
      disconnectStream drops the publisher. None of these can be undone
      live.

  - models: [all]
    severity: info
    text: >
      A change to an application's configuration (update_app,
      update_transcoder, update_dvr_config and the like) takes effect when
      the application restarts (application_actions restart), as in the
      Manager. The Manager's own pages use the same API, so anything set
      there shows in the next read.

state:
  server.name:                  { type: string, description: "The Streaming Engine's name (wseName)" }
  server.version:               { type: string, description: "Its version, such as 4.9.0" }
  server.build:                 { type: string, description: "Its build number" }
  server.mode:                  { type: string, description: "The server mode" }
  server.license_type:          { type: string, description: "The license type, such as Subscription or Developer" }
  server.license_expire_date:   { type: string, description: "When the license expires, as the server writes it" }
  server.current_connections:   { type: int, description: "Connections now" }
  server.max_connections:       { type: int, description: "Connections the license allows" }
  server.max_incoming_streams:  { type: int, description: "Incoming streams the license allows" }
  server.time_running:          { type: float, unit: s, description: "Time since the server started" }
  server.processor_cores:       { type: int, description: "Processor cores" }
  server.total_memory:          { type: int, unit: B, description: "Memory installed" }
  server.available_memory:      { type: int, unit: B, description: "Memory free" }
  server.transcoder_licensed:   { type: bool, description: "The Transcoder is licensed" }
  server.transcoder_available:  { type: bool, description: "The Transcoder can run (licensed and supported here)" }
  server.transcoder_licenses:   { type: int, description: "Transcoder licenses" }
  server.transcoder_licenses_in_use: { type: int, description: "Transcoder licenses in use" }
  server.transcoder_watermark:  { type: bool, description: "Transcoded output is watermarked (an unlicensed trial)" }
  server.gpu_acceleration:      { type: string, description: "The GPU acceleration the Transcoder uses" }
  server.dvr_licensed:          { type: bool, description: "nDVR is licensed" }
  server.dvr_in_use:            { type: bool, description: "nDVR is in use" }
  server.drm_licensed:          { type: bool, description: "DRM is licensed" }
  server.drm_in_use:            { type: bool, description: "DRM is in use" }
  server.transcoder_applications: { type: string, description: "Applications with the Transcoder on, as a JSON array" }
  server.dvr_applications:      { type: string, description: "Applications with nDVR on, as a JSON array" }
  server.drm_applications:      { type: string, description: "Applications with DRM on, as a JSON array" }
  server.java_version:          { type: string, description: "The Java version it runs on" }
  server.os_name:               { type: string, description: "The operating system" }
  machine.cpu_user:             { type: int, unit: "%", description: "Processor time in user space" }
  machine.cpu_system:           { type: int, unit: "%", description: "Processor time in the system" }
  machine.cpu_idle:             { type: int, unit: "%", description: "Processor time idle" }
  machine.memory_used:          { type: int, unit: B, description: "Memory used" }
  machine.memory_free:          { type: int, unit: B, description: "Memory free" }
  machine.heap_used:            { type: int, unit: B, description: "Java heap used" }
  machine.heap_free:            { type: int, unit: B, description: "Java heap free" }
  machine.disk_used:            { type: int, unit: B, description: "Disk used" }
  machine.disk_free:            { type: int, unit: B, description: "Disk free" }
  machine.connections:          { type: int, description: "Connections to the server" }
  machine.uptime:               { type: int, unit: s, description: "Time the server has been up" }
  vhost.connections:            { type: int, description: "Connections to the virtual host" }
  vhost.bytes_in:               { type: int, unit: B, description: "Bytes received by the virtual host" }
  vhost.bytes_out:              { type: int, unit: B, description: "Bytes sent by the virtual host" }
  vhost.uptime:                 { type: int, unit: s, description: "Time the virtual host has been up" }
  applications.*.app_type:      { type: string, description: "An application, keyed by name: its type (Live, VOD, LiveEdge, ...)" }
  applications.*.transcoder_enabled: { type: bool, description: "The Transcoder is on for it" }
  applications.*.dvr_enabled:   { type: bool, description: "nDVR is on for it" }
  applications.*.drm_enabled:   { type: bool, description: "DRM is on for it" }
  applications.*.stream_targets_enabled: { type: bool, description: "Stream Targets (push publish) is on for it" }
  applications.*.connections:   { type: int, description: "Connections to it now" }
  applications.*.bytes_in:      { type: int, unit: B, description: "Bytes it received" }
  applications.*.bytes_out:     { type: int, unit: B, description: "Bytes it sent" }
  applications.*.bytes_in_rate: { type: int, unit: B/s, description: "Bytes received per second now" }
  applications.*.bytes_out_rate: { type: int, unit: B/s, description: "Bytes sent per second now" }
  applications.*.uptime:        { type: int, unit: s, description: "Time it has been running" }
  applications.*.instances.*.incoming_streams.*.connected: { type: bool, description: "A live stream published to an application instance, keyed by instance and stream name: it is connected" }
  applications.*.instances.*.incoming_streams.*.source_ip: { type: string, description: "Where the stream comes from" }
  applications.*.instances.*.incoming_streams.*.recording: { type: bool, description: "A recorder is set for it" }
  applications.*.instances.*.incoming_streams.*.published_to_vod: { type: bool, description: "It is published to VOD" }
  applications.*.instances.*.incoming_streams.*.ptz_enabled: { type: bool, description: "Its camera's pan, tilt and zoom can be controlled" }
  applications.*.instances.*.incoming_streams.*.stream_manager: { type: bool, description: "It was started by the Stream Manager" }
  applications.*.instances.*.stream_stats.*.connections: { type: int, description: "A stream's viewers and connections, keyed by stream name, as last read" }
  applications.*.instances.*.stream_stats.*.bytes_in: { type: int, unit: B, description: "Bytes the stream received" }
  applications.*.instances.*.stream_stats.*.bytes_out: { type: int, unit: B, description: "Bytes sent of it" }
  applications.*.instances.*.stream_stats.*.bytes_in_rate: { type: int, unit: B/s, description: "Its incoming rate" }
  applications.*.instances.*.stream_stats.*.bytes_out_rate: { type: int, unit: B/s, description: "Its outgoing rate" }
  applications.*.instances.*.stream_stats.*.uptime: { type: int, unit: s, description: "Time it has been live" }
  applications.*.instances.*.recorders.*.state: { type: string, description: "A stream recorder, keyed by name: its state, such as Recording in progress or Waiting for stream" }
  applications.*.instances.*.recorders.*.current_file: { type: string, description: "The file it writes now" }
  applications.*.instances.*.recorders.*.current_size: { type: int, unit: B, description: "That file's size" }
  applications.*.instances.*.recorders.*.current_duration: { type: int, unit: ms, description: "That file's duration" }
  applications.*.instances.*.recorders.*.start_time: { type: string, description: "When recording started, as the server writes it" }
  applications.*.instances.*.recorders.*.output_path: { type: string, description: "Where it writes" }
  applications.*.instances.*.recorders.*.file_format: { type: string, description: "MP4 or FLV" }
  applications.*.instances.*.recorders.*.segmentation_type: { type: string, description: "How files are split: None, SegmentByDuration, SegmentBySize or SegmentBySchedule" }
  applications.*.instances.*.recorders.*.error: { type: string, description: "Its error, empty when none" }
  applications.*.instances.*.stream_groups.*.members: { type: string, description: "A stream group, keyed by name: its member streams as a JSON array" }
  applications.*.instances.*.stream_groups.*.transcode_result: { type: bool, description: "It is the Transcoder's output group" }
  applications.*.stream_targets.*.enabled: { type: bool, description: "A stream target (push publish map entry), keyed by entry name: it is on" }
  applications.*.stream_targets.*.profile: { type: string, description: "Its destination type, such as rtmp, srt or cupertino-cdn" }
  applications.*.stream_targets.*.source_stream: { type: string, description: "The stream it sends" }
  applications.*.stream_targets.*.host: { type: string, description: "The destination host" }
  applications.*.stream_targets.*.port: { type: int, description: "The destination port" }
  applications.*.stream_targets.*.application: { type: string, description: "The destination application" }
  applications.*.stream_targets.*.stream_name: { type: string, description: "The stream name at the destination" }
  applications.*.stream_targets.*.session_status: { type: string, description: "The session's status, such as Active, Waiting or Error" }
  applications.*.stream_targets.*.failure_reason: { type: string, description: "Why the session failed, empty when it did not" }
  startup_streams.*.*.*.media_caster_type: { type: string, description: "A stream started with the server, keyed by application, instance and stream name: how it is pulled" }

telemetry:
  # Wowza pushes nothing to the API.
  poll:
    - every_ms: 10000
      send:
        - { method: GET, path: /v2/machine/monitoring/current }
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/monitoring/current" }
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications" }
    - every_ms: 60000
      send:
        - { method: GET, path: "/v2/servers/{settings.server}/status" }
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/startupstreams" }
  updates:
    - path: "^/v2/servers/[^/]+/status$"
      json:
        name: "$.wseName"
        version: "$.serverVersion"
        build: "$.serverBuild"
        mode: "$.serverMode"
        license: "$.licenseType"
        expires: "$.licenseExpireDate"
        connections: "$.currentConnections"
        max_connections: "$.maxConnections"
        max_streams: "$.maxIncommingStreams"
        running: "$.timeRunning"
        cores: "$.processorCores"
        total: "$.totalMemory"
        available: "$.availableMemory"
        transcoder: "$.transcoderLicensed"
        transcoder_available: "$.transcoderAvailable"
        licenses: "$.transcoderLicenses"
        licenses_used: "$.transcoderLicensesInUse"
        watermark: "$.transcoderWatermark"
        gpu: "$.gpuAcceleration"
        dvr: "$.dvrLicensed"
        dvr_used: "$.dvrInUse"
        drm: "$.drmLicensed"
        drm_used: "$.drmInUse"
        transcoder_apps: "$.transcoderApplications"
        dvr_apps: "$.dvrApplications"
        drm_apps: "$.drmApplications"
        java: "$.javaVersion"
        os: "$.osName"
      state:
        server.name: "{name}"
        server.version: "{version}"
        server.build: "{build}"
        server.mode: "{mode}"
        server.license_type: "{license}"
        server.license_expire_date: "{expires}"
        server.current_connections: "{connections}"
        server.max_connections: "{max_connections}"
        server.max_incoming_streams: "{max_streams}"
        server.time_running: "{running}"
        server.processor_cores: "{cores}"
        server.total_memory: "{total}"
        server.available_memory: "{available}"
        server.transcoder_licensed: "{transcoder}"
        server.transcoder_available: "{transcoder_available}"
        server.transcoder_licenses: "{licenses}"
        server.transcoder_licenses_in_use: "{licenses_used}"
        server.transcoder_watermark: "{watermark}"
        server.gpu_acceleration: "{gpu}"
        server.dvr_licensed: "{dvr}"
        server.dvr_in_use: "{dvr_used}"
        server.drm_licensed: "{drm}"
        server.drm_in_use: "{drm_used}"
        server.transcoder_applications: "{transcoder_apps}"
        server.dvr_applications: "{dvr_apps}"
        server.drm_applications: "{drm_apps}"
        server.java_version: "{java}"
        server.os_name: "{os}"

    - path: "^/v2/machine/monitoring/current$"
      json: { user: "$.cpuUser", system: "$.cpuSystem", idle: "$.cpuIdle", used: "$.memoryUsed", free: "$.memoryFree",
              heap_used: "$.heapUsed", heap_free: "$.heapFree", disk_used: "$.diskUsed", disk_free: "$.diskFree",
              connections: "$.connectionCount", uptime: "$.serverUptime" }
      state:
        machine.cpu_user: "{user}"
        machine.cpu_system: "{system}"
        machine.cpu_idle: "{idle}"
        machine.memory_used: "{used}"
        machine.memory_free: "{free}"
        machine.heap_used: "{heap_used}"
        machine.heap_free: "{heap_free}"
        machine.disk_used: "{disk_used}"
        machine.disk_free: "{disk_free}"
        machine.connections: "{connections}"
        machine.uptime: "{uptime}"

    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/monitoring/current$"
      json: { connections: "$.totalConnections", bytes_in: "$.bytesIn", bytes_out: "$.bytesOut", uptime: "$.uptime" }
      state:
        vhost.connections: "{connections}"
        vhost.bytes_in: "{bytes_in}"
        vhost.bytes_out: "{bytes_out}"
        vhost.uptime: "{uptime}"

    # The application list; each application's live state is read in turn.
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications$"
      json_each: "$.applications"
      json: { id: "$.id", type: "$.appType", transcoder: "$.transcoderEnabled", dvr: "$.dvrEnabled",
              drm: "$.drmEnabled", targets: "$.streamTargetsEnabled" }
      state:
        "applications.{id:key}.app_type": "{type}"
        "applications.{id:key}.transcoder_enabled": "{transcoder}"
        "applications.{id:key}.dvr_enabled": "{dvr}"
        "applications.{id:key}.drm_enabled": "{drm}"
        "applications.{id:key}.stream_targets_enabled": "{targets}"
      then_send:
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{id}/instances" }
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{id}/pushpublish/mapentries" }
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{id}/monitoring/current" }

    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/monitoring/current$"
      json: { connections: "$.totalConnections", bytes_in: "$.bytesIn", bytes_out: "$.bytesOut",
              in_rate: "$.bytesInRate", out_rate: "$.bytesOutRate", uptime: "$.uptime" }
      state:
        "applications.{1:url_decode:key}.connections": "{connections}"
        "applications.{1:url_decode:key}.bytes_in": "{bytes_in}"
        "applications.{1:url_decode:key}.bytes_out": "{bytes_out}"
        "applications.{1:url_decode:key}.bytes_in_rate": "{in_rate}"
        "applications.{1:url_decode:key}.bytes_out_rate": "{out_rate}"
        "applications.{1:url_decode:key}.uptime": "{uptime}"

    # Each instance's streams, recorders and groups are whole in each read.
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances$"
      json_each: "$.instanceList"
      json: { instance: "$.name" }
      replace:
        - "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams"
        - "applications.{1:url_decode:key}.instances.{instance:key}.recorders"
        - "applications.{1:url_decode:key}.instances.{instance:key}.stream_groups"
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances$"
      json_each: "$.instanceList[*].incomingStreams"
      json: { instance: "$^.name", stream: "$.name", connected: "$.isConnected", ip: "$.sourceIp",
              recording: "$.isRecordingSet", vod: "$.isPublishedToVOD", ptz: "$.isPTZEnabled",
              manager: "$.isStreamManagerStream" }
      state:
        "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams.{stream:key}.connected": "{connected}"
        "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams.{stream:key}.source_ip": "{ip}"
        "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams.{stream:key}.recording": "{recording}"
        "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams.{stream:key}.published_to_vod": "{vod}"
        "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams.{stream:key}.ptz_enabled": "{ptz}"
        "applications.{1:url_decode:key}.instances.{instance:key}.incoming_streams.{stream:key}.stream_manager": "{manager}"
      then_send:
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{1:url_decode}/instances/{instance}/incomingstreams/{stream}/monitoring/current" }
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances$"
      json_each: "$.instanceList[*].recorders"
      json:
        instance: "$^.name"
        recorder: "$.recorderName"
        state: "$.recorderState"
        file: "$.currentFile"
        size: "$.currentSize"
        duration: "$.currentDuration"
        started: "$.recordingStartTime"
        path: "$.outputPath"
        format: "$.fileFormat"
        segmentation: "$.segmentationType"
        error: "$.recorderErrorString"
      state:
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.state": "{state}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.current_file": "{file}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.current_size": "{size}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.current_duration": "{duration}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.start_time": "{started}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.output_path": "{path}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.file_format": "{format}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.segmentation_type": "{segmentation}"
        "applications.{1:url_decode:key}.instances.{instance:key}.recorders.{recorder:key}.error": "{error}"
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances$"
      json_each: "$.instanceList[*].streamGroups"
      json: { instance: "$^.name", group: "$.groupName", members: "$.members", result: "$.isTranscodeResult" }
      state:
        "applications.{1:url_decode:key}.instances.{instance:key}.stream_groups.{group:key}.members": "{members}"
        "applications.{1:url_decode:key}.instances.{instance:key}.stream_groups.{group:key}.transcode_result": "{result}"
    # One instance's recorders, read on their own.
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances/([^/]+)/streamrecorders$"
      json_each: "$.streamrecorder"
      json:
        recorder: "$.recorderName"
        state: "$.recorderState"
        file: "$.currentFile"
        size: "$.currentSize"
        duration: "$.currentDuration"
        started: "$.recordingStartTime"
        path: "$.outputPath"
        format: "$.fileFormat"
        segmentation: "$.segmentationType"
        error: "$.recorderErrorString"
      replace: "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders"
      state:
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.state": "{state}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.current_file": "{file}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.current_size": "{size}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.current_duration": "{duration}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.start_time": "{started}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.output_path": "{path}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.file_format": "{format}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.segmentation_type": "{segmentation}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.recorders.{recorder:key}.error": "{error}"

    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances/([^/]+)/incomingstreams/([^/]+)/monitoring/current$"
      json: { connections: "$.totalConnections", bytes_in: "$.bytesIn", bytes_out: "$.bytesOut",
              in_rate: "$.bytesInRate", out_rate: "$.bytesOutRate", uptime: "$.uptime" }
      state:
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.stream_stats.{3:url_decode:key}.connections": "{connections}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.stream_stats.{3:url_decode:key}.bytes_in": "{bytes_in}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.stream_stats.{3:url_decode:key}.bytes_out": "{bytes_out}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.stream_stats.{3:url_decode:key}.bytes_in_rate": "{in_rate}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.stream_stats.{3:url_decode:key}.bytes_out_rate": "{out_rate}"
        "applications.{1:url_decode:key}.instances.{2:url_decode:key}.stream_stats.{3:url_decode:key}.uptime": "{uptime}"

    # Stream targets: secrets in the entries are never kept.
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/pushpublish/mapentries$"
      json_each: "$.mapEntries"
      replace: "applications.{1:url_decode:key}.stream_targets"
      json: { entry: "$.entryName", enabled: "$.enabled", profile: "$.profile", source: "$.sourceStreamName",
              host: "$.host", port: "$.port", application: "$.application", stream: "$.streamName",
              status: "$.sessionStatus", reason: "$.sessionFailureReason" }
      state:
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.enabled": "{enabled}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.profile": "{profile}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.source_stream": "{source}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.host": "{host}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.port": "{port}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.application": "{application}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.stream_name": "{stream}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.session_status": "{status}"
        "applications.{1:url_decode:key}.stream_targets.{entry:key}.failure_reason": "{reason}"
    # A change to a target or a recorder reads the list back.
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/pushpublish/mapentries/[^/]+(/actions/[A-Za-z]+)?$"
      json_match: { "$.success": "^true$" }
      json: {}
      then_send:
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{1:url_decode}/pushpublish/mapentries" }
    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/applications/([^/]+)/instances/([^/]+)/streamrecorders/[^/]+(/actions/[A-Za-z]+)?$"
      json_match: { "$.success": "^true$" }
      json: {}
      then_send:
        - { method: GET, path: "/v2/servers/{settings.server}/vhosts/{settings.vhost}/applications/{1:url_decode}/instances/{2:url_decode}/streamrecorders" }

    - path: "^/v2/servers/[^/]+/vhosts/[^/]+/startupstreams$"
      json_each: "$.Streams"
      replace: startup_streams
      json: { app: "$.appName", instance: "$.instance", stream: "$.streamName", type: "$.mediaCasterType" }
      state:
        "startup_streams.{app:key}.{instance:key}.{stream:key}.media_caster_type": "{type}"
'''

class Flow(dict):
    """A mapping written on one line."""


yaml.SafeDumper.add_representer(
    Flow, lambda dumper, data: dumper.represent_mapping("tag:yaml.org,2002:map", data, flow_style=True))
for command in commands.values():
    for pname, decl in command.get("params", {}).items():
        command["params"][pname] = Flow(decl)
    command["send"] = Flow(command["send"])
    command["expect"] = Flow(command["expect"])

supports = sorted(commands)
models = yaml.safe_dump([{"id": "wse-4", "name": "Wowza Streaming Engine 4 (REST API v2)",
                          "supports": supports, "verification": "none",
                          "notes": "Builds 15089 and later serve the REST API; some operations name a later build."}],
                        sort_keys=False, width=110, allow_unicode=True)
body = yaml.safe_dump({"commands": commands}, sort_keys=False, width=1000, allow_unicode=True)
body = re.sub(r"\n(  [a-z0-9_]+:\n    summary:)", r"\n\n\1", body)
text = HEADER + "\nmodels:\n" + "\n".join("  " + line if line else line for line in models.splitlines()) + "\n\n" + body + "\n" + TAIL
OUT.write_text(text, encoding="utf-8", newline="\n")
print(f"wrote {OUT.relative_to(ROOT)}: {len(commands)} commands")
