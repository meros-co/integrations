#!/usr/bin/env python3
"""Write the expectation vectors in vectors/.

Expected wire formats are stated here by hand from each protocol's documented
syntax, and OSC bytes come from the small encoder below. Neither shares code
with the Rust core, so the core's test suite compares its output against an
independent statement of what the wire must carry.

    python tools/make_vectors.py

These are expectations, not recordings: none carries `recorded`, so none
promotes a model's verification status.
"""
from __future__ import annotations

import json
import struct
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent


# ── An independent OSC 1.0 encoder ────────────────────────────────────────
def _pad(b: bytes) -> bytes:
    return b + b"\0" * (4 - len(b) % 4)


def osc(address: str, *args) -> bytes:
    """args: ("i", int) | ("f", float) | ("s", str)."""
    out = _pad(address.encode())
    out += _pad(("," + "".join(t for t, _ in args)).encode())
    for t, v in args:
        if t == "i":
            out += struct.pack(">i", v)
        elif t == "f":
            out += struct.pack(">f", v)
        elif t == "s":
            out += _pad(v.encode())
    return out


def length_prefixed(packet: bytes) -> bytes:
    return struct.pack(">I", len(packet)) + packet


def hexs(b: bytes) -> str:
    return b.hex()


# ── The vectors ───────────────────────────────────────────────────────────
V: list[dict] = []


def text(spec, command, input, wire, **extra):
    V.append({"spec": spec, "command": command, "input": input, "expect_wire": wire, **extra})


def binary(spec, command, input, wire, **extra):
    wires = [hexs(w) for w in wire] if isinstance(wire, list) else hexs(wire)
    V.append({"spec": spec, "command": command, "input": input, "expect_wire_hex": wires, **extra})


def telemetry(spec, name, **fields):
    """An inbound message and the state it must produce; optionally what the
    engine sends on connecting."""
    V.append({"spec": spec, "telemetry": name, **fields})


def http(spec, command, input, method, target, **extra):
    V.append({
        "spec": spec, "command": command, "input": input,
        "expect_request": {"method": method, "target": target}, **extra,
    })


# HyperDeck: payload + CRLF (Blackmagic HyperDeck Ethernet Protocol).
H = "blackmagic-hyperdeck"
text(H, "record", {}, "record\r\n", device_reply="200 ok\r\n", expect_result={"ok": {"kind": "ack"}})
text(H, "record_named", {"name": "Take 4"}, "record: name: Take 4\r\n")
text(H, "record_spill", {}, "record spill\r\n")
text(H, "record_spill_to_slot", {"slot_id": 2}, "record: spill: slot id: 2\r\n")
text(H, "stop", {}, "stop\r\n", device_reply="111 remote control disabled\r\n",
     expect_result={"error": {"error": "device_error", "code": "111"}})
text(H, "play", {"speed": -50}, "play: single clip: true loop: false speed: -50\r\n")
text(H, "goto_clip", {"clip_id": 3}, "goto: clip id: 3\r\n")
text(H, "goto_clip_offset", {"offset": 2}, "goto: clip id: +2\r\n")
text(H, "get_transport_info", {}, "transport info\r\n",
     device_reply="208 transport info:\r\nstatus: play\r\nspeed: 100\r\n\r\n",
     expect_result={"ok": {"kind": "value", "value": {"status": "play", "speed": "100"}}})
text(H, "get_device_info", {}, "device info\r\n")
text(H, "get_clip_count", {}, "clips count\r\n",
     device_reply="214 clips count:\r\nclip count: 7\r\n\r\n", expect_result={"ok": {"kind": "value", "value": "7"}})
text(H, "get_clips", {}, "clips get\r\n")
text(H, "get_configuration", {}, "configuration\r\n")
text(H, "disk_list", {}, "disk list\r\n")
text(H, "disk_list_slot", {"slot_id": 1}, "disk list: slot id: 1\r\n")
text(H, "preview_enable", {}, "preview: enable: true\r\n")
text(H, "preview_disable", {}, "preview: enable: false\r\n")
text(H, "get_playrange", {}, "playrange\r\n")
text(H, "set_playrange_clip", {"clip_id": 5}, "playrange set: clip id: 5\r\n")
text(H, "set_playrange_clips", {"clip_id": 5, "count": 7}, "playrange set: clip id: 5 count: 7\r\n")
text(H, "set_playrange_timecode", {"in": "00:01:00:00", "out": "00:02:00:00"},
     "playrange set: in: 00:01:00:00 out: 00:02:00:00\r\n")
text(H, "set_playrange_frames", {"timeline_in": 0, "timeline_out": 250},
     "playrange set: timeline in: 0 timeline out: 250\r\n")
text(H, "clear_playrange", {}, "playrange clear\r\n")
text(H, "set_playback_stop_mode", {"mode": "black"}, "play option: stop mode: black\r\n")

# Videohub: a block ended by a blank line; inputs and outputs 0-based on the wire.
VH = "blackmagic-videohub"
text(VH, "set_route", {"input": 1, "output": 5}, "VIDEO OUTPUT ROUTING:\n4 0\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(VH, "set_input_label", {"input": 2, "label": "Cam 2"}, "INPUT LABELS:\n1 Cam 2\n\n")
text(VH, "set_output_label", {"output": 1, "label": "PGM"}, "OUTPUT LABELS:\n0 PGM\n\n")
text(VH, "lock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 O\n\n")
text(VH, "unlock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 U\n\n")
text(VH, "force_unlock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 F\n\n")
text(VH, "set_take_mode", {"enabled": True}, "CONFIGURATION:\nTake Mode: true\n\n")

# Kramer Protocol 3000: payload + CR; destination before source.
K = "kramer-p3000"
text(K, "route_video", {"input": 3, "output": 2}, "#ROUTE 1,2,3\r",
     device_reply="~01@ROUTE 1,2,3 OK\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "route_audio", {"input": 0, "output": 1}, "#ROUTE 2,1,0\r",
     device_reply="~01@ROUTE 2,1,0 ERR 003\r\n", expect_result={"error": {"error": "device_error"}})
text(K, "get_model", {}, "#MODEL?\r", device_reply="~01@MODEL VS-88UT\r\n",
     expect_result={"ok": {"kind": "value", "value": "VS-88UT"}})

# RossTalk: payload + CRLF, never acknowledged.
R = "rosstalk"
text(R, "select_program", {"source": 4}, "XPT ME:1:PGM:IN:4\r\n", expect_result={"ok": {"kind": "unverified"}})
text(R, "select_preset", {"source": 12, "me": 2}, "XPT ME:2:PST:IN:12\r\n")
text(R, "cut", {}, "MECUT ME:1\r\n")
text(R, "auto_transition", {"me": 3}, "MEAUTO ME:3\r\n")
text(R, "fade_to_black", {}, "FTB\r\n")
text(R, "custom_control", {"bank": 1, "cc": 12}, "CC 1:12\r\n")
text(R, "gpi", {"number": 5}, "GPI 5\r\n")

# grandMA2 telnet: payload + CRLF, never acknowledged.
G = "grandma2"
text(G, "go", {}, "Go\r\n", expect_result={"ok": {"kind": "unverified"}})
text(G, "go_executor", {"executor": 3}, "Go Executor 3\r\n")
text(G, "go_back", {}, "GoBack\r\n")
text(G, "go_back_executor", {"executor": 3}, "GoBack Executor 3\r\n")
text(G, "goto_cue", {"cue": "4.5"}, "Goto Cue 4.5\r\n")
text(G, "goto_cue_executor", {"cue": "4.5", "executor": 2}, "Goto Cue 4.5 Executor 2\r\n")
text(G, "fire_macro", {"macro": 10}, "Go Macro 10\r\n")
text(G, "command", {"line": "Blackout"}, "Blackout\r\n")

# Behringer X32: OSC over UDP.
X = "behringer-x32"
binary(X, "set_channel_fader", {"channel": 7, "level": 0.75}, osc("/ch/07/mix/fader", ("f", 0.75)))
binary(X, "mute_channel", {"channel": 7, "muted": True}, osc("/ch/07/mix/on", ("i", 0)),
       expect_result={"ok": {"kind": "unverified"}})
binary(X, "set_dca_fader", {"dca": 2, "level": 0.5}, osc("/dca/2/fader", ("f", 0.5)))
binary(X, "mute_dca", {"dca": 2, "muted": False}, osc("/dca/2/on", ("i", 1)))
binary(X, "set_main_fader", {"level": 1.0}, osc("/main/st/mix/fader", ("f", 1.0)))
binary(X, "mute_main", {"muted": True}, osc("/main/st/mix/on", ("i", 0)))
binary(X, "get_channel_name", {"channel": 1}, osc("/ch/01/config/name"),
       device_reply_hex=hexs(osc("/ch/01/config/name", ("s", "Kick"))),
       expect_result={"ok": {"kind": "value", "value": "Kick"}})
binary(X, "set_channel_name", {"channel": 12, "name": "Vox"}, osc("/ch/12/config/name", ("s", "Vox")))

# Behringer WING: OSC over UDP 2223, 1-based strip numbers, faders in dB,
# mute as int 1/0 (WING Remote Protocols p.22, p.47-66).
W = "behringer-wing"
for stem, node, n in [("channel", "ch", 40), ("aux", "aux", 8), ("bus", "bus", 16),
                      ("main", "main", 4), ("matrix", "mtx", 8), ("dca", "dca", 16)]:
    binary(W, f"set_{stem}_fader", {stem: n, "level_db": -3.0}, osc(f"/{node}/{n}/fdr", ("f", -3.0)),
           expect_result={"ok": {"kind": "unverified"}})
    binary(W, f"mute_{stem}", {stem: 1, "muted": True}, osc(f"/{node}/1/mute", ("i", 1)))
    binary(W, f"get_{stem}_name", {stem: 2}, osc(f"/{node}/2/name"),
           device_reply_hex=hexs(osc(f"/{node}/2/name", ("s", "Vocals"))),
           expect_result={"ok": {"kind": "value", "value": "Vocals"}})
    binary(W, f"set_{stem}_name", {stem: 3, "name": "Pad"}, osc(f"/{node}/3/name", ("s", "Pad")))
# A float query answers ,sff: display text, raw 0-1 position, dB (p.21).
binary(W, "get_channel_fader", {"channel": 1}, osc("/ch/1/fdr"),
       device_reply_hex=hexs(osc("/ch/1/fdr", ("s", "-2.0"), ("f", 0.7), ("f", -2.0))),
       expect_result={"ok": {"kind": "value", "value": -2.0}})
# An int query answers ,sfi: display text, raw, int (p.21).
binary(W, "get_channel_mute", {"channel": 1}, osc("/ch/1/mute"),
       device_reply_hex=hexs(osc("/ch/1/mute", ("s", "1"), ("f", 1.0), ("i", 1))),
       expect_result={"ok": {"kind": "value", "value": 1}})
binary(W, "mute_group", {"group": 8, "muted": False}, osc("/mgrp/8/mute", ("i", 0)))
binary(W, "recall_scene", {"scene": 5},
       [osc("/$ctl/lib/$actionidx", ("i", 5)), osc("/$ctl/lib/$action", ("s", "GO"))])
binary(W, "get_console_info", {}, osc("/?"),
       device_reply_hex=hexs(osc("/?", ("s", "WING,192.168.1.71,PGM,ngc-full,NO_SERIAL,1.07.2"))),
       expect_result={"ok": {"kind": "value", "value": "WING,192.168.1.71,PGM,ngc-full,NO_SERIAL,1.07.2"}})

# QLab: OSC over UDP, no replies to the sender.
Q = "qlab"
for command, address in [("go", "/go"), ("stop", "/stop"), ("pause", "/pause"), ("resume", "/resume"),
                         ("panic", "/panic"), ("next_cue", "/playhead/next"),
                         ("previous_cue", "/playhead/previous")]:
    binary(Q, command, {}, osc(address))
binary(Q, "start_cue", {"cue": "12.5"}, osc("/cue/12.5/start"), expect_result={"ok": {"kind": "unverified"}})
binary(Q, "stop_cue", {"cue": "A1"}, osc("/cue/A1/stop"))

# Resolume: OSC over UDP.
RS = "resolume"
binary(RS, "trigger_clip", {"layer": 2, "clip": 3}, osc("/composition/layers/2/clips/3/connect", ("i", 1)))
binary(RS, "trigger_column", {"column": 4}, osc("/composition/columns/4/connect", ("i", 1)))
binary(RS, "clear_layer", {"layer": 1}, osc("/composition/layers/1/clear", ("i", 1)))
binary(RS, "set_layer_opacity", {"layer": 1, "opacity": 0.25},
       osc("/composition/layers/1/video/opacity/values", ("f", 0.25)))

# ETC Eos: OSC 1.0 over TCP with a 4-byte length prefix.
E = "etc-eos"
binary(E, "go", {}, [length_prefixed(osc("/eos/key/go_0", ("f", 1.0))),
                     length_prefixed(osc("/eos/key/go_0", ("f", 0.0)))],
       expect_result={"ok": {"kind": "unverified"}})
binary(E, "fire_cue", {"cue": "1.5"}, length_prefixed(osc("/eos/cue/1/1.5/fire")))
binary(E, "set_submaster", {"submaster": 3, "level": 0.5}, length_prefixed(osc("/eos/sub/3", ("f", 0.5))))
binary(E, "fire_macro", {"macro": 101}, length_prefixed(osc("/eos/macro/fire", ("i", 101))))

# PTZOptics: positional CGI arguments, sent verbatim.
P = "ptzoptics"
http(P, "move", {"direction": "left", "pan_speed": 12}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&left&12&10",
     http_reply={"status": 200}, expect_result={"ok": {"kind": "ack"}})
http(P, "stop_move", {}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&ptzstop&1&1")
http(P, "zoom_in", {}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&zoomin&5")
http(P, "zoom_out", {"speed": 7}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&zoomout&7")
http(P, "stop_zoom", {}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&zoomstop&0")
http(P, "recall_preset", {"preset": 3}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&poscall&3",
     http_reply={"status": 404}, expect_result={"error": {"error": "device_error", "code": "404"}})
http(P, "save_preset", {"preset": 89}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&posset&89")
http(P, "home", {}, "GET", "/cgi-bin/ptzctrl.cgi?ptzcmd&home")

# TriCaster: GET /v1/shortcut?name=NAME&value=VALUE (Automation and
# Integration Guide p.63).
T = "newtek-tricaster"
for command, query in [("take", "name=main_take"), ("auto", "name=main_auto"),
                       ("background_take", "name=main_background_take"),
                       ("background_auto", "name=main_background_auto"),
                       ("start_recording", "name=record_start"), ("stop_recording", "name=record_stop"),
                       ("start_streaming", "name=streaming_toggle&value=1"),
                       ("stop_streaming", "name=streaming_toggle&value=0"),
                       ("stop_all_macros", "name=stop_all_macros")]:
    http(T, command, {}, "GET", f"/v1/shortcut?{query}")
http(T, "set_program", {"source": "input3"}, "GET", "/v1/shortcut?name=main_a_row_named_input&value=input3",
     http_reply={"status": 200, "body": ""}, expect_result={"ok": {"kind": "ack"}})
http(T, "set_preview", {"source": "ddr1"}, "GET", "/v1/shortcut?name=main_b_row_named_input&value=ddr1")
http(T, "dsk_take", {"dsk": 2}, "GET", "/v1/shortcut?name=main_dsk2_take")
http(T, "dsk_auto", {"dsk": 1}, "GET", "/v1/shortcut?name=main_dsk1_auto")
http(T, "ddr_play", {"ddr": 2}, "GET", "/v1/shortcut?name=ddr2_play")
http(T, "ddr_stop", {"ddr": 2}, "GET", "/v1/shortcut?name=ddr2_stop")
http(T, "play_macro", {"name": "Open Show"}, "GET", "/v1/shortcut?name=play_macro_byname&value=Open%20Show")

# AJA Ki Pro: key/value query.
A = "aja-kipro"
http(A, "record", {}, "GET", "/config?action=set&paramid=eParamID_TransportCommand&value=3")
http(A, "play", {}, "GET", "/config?action=set&paramid=eParamID_TransportCommand&value=1")
http(A, "stop", {}, "GET", "/config?action=set&paramid=eParamID_TransportCommand&value=4")
http(A, "set_clip_name", {"name": "Show 1"}, "GET",
     "/config?action=set&paramid=eParamID_CustomClipName&value=Show%201")
http(A, "get_transport_state", {}, "GET", "/config?action=get&paramid=eParamID_TransportState",
     http_reply={"status": 200, "body": '{"value":"1","value_name":"Playing"}'},
     expect_result={"ok": {"kind": "value", "value": "Playing"}})

# ProPresenter 7 HTTP API.
# ProPresenter: HTTP API.
PP = "propresenter"


def _pp_vectors() -> None:
    """One vector per ProPresenter command. The command list comes from the
    spec (itself generated from Renewed Vision's OpenAPI document); the
    expected request is computed here independently: example values put into
    the path with RFC 3986 encoding, and the JSON body written by json.dumps."""
    from urllib.parse import quote

    doc = yaml.safe_load((ROOT / "specs" / "propresenter.yaml").read_text(encoding="utf-8"))
    examples = {"int": 2, "float": 15.5, "bool": True, "string": "Song 1",
                "json": {"name": "Song 1", "enabled": True}}
    for name, command in doc["commands"].items():
        params = command.get("params") or {}
        values = {}
        for pname, p in params.items():
            values[pname] = p["values"][0] if p["type"] == "enum" else examples[p["type"]]
        send = command["send"]
        target = send["path"]
        for pname, v in values.items():
            target = target.replace("{" + pname + "}", quote(str(v), safe=""))
        if send.get("query"):
            target += "?" + "&".join(f"{k}={quote(str(values[k]), safe='')}" for k in send["query"])
        request = {"method": send["method"], "target": target}
        if "body" in send:
            v = values[next(iter(k for k in params if k in ("value", "enabled", "body")))]
            if isinstance(v, dict):
                request["body"] = json.dumps(v, separators=(",", ":"))
            elif isinstance(v, bool):
                request["body"] = "true" if v else "false"
            elif isinstance(v, float):
                request["body"] = f"{v:.3f}"
            else:
                request["body"] = json.dumps(v)
        V.append({"spec": PP, "command": name, "input": values, "expect_request": request})


_pp_vectors()
# Replies, from the document's own examples.
http(PP, "version_get", {}, "GET", "/version",
     http_reply={"status": 200, "body": '{"name":"ProPresenter","major_version":7}'},
     expect_result={"ok": {"kind": "value", "value": {"name": "ProPresenter", "major_version": 7}}})
telemetry(PP, "slide", inbound_http={"path": "/v1/status/slide", "body": json.dumps({
    "current": {"text": "Amazing Grace, how sweet the sound", "notes": "", "uuid": "1"},
    "next": {"text": "That saved a wretch like me", "notes": "Start quiet", "uuid": "2"}})},
    expect_state={"slide": {"current": {"text": "Amazing Grace, how sweet the sound", "notes": ""},
                            "next": {"text": "That saved a wretch like me", "notes": "Start quiet"}}})
telemetry(PP, "timers", inbound_http={"path": "/v1/timers/current", "body": json.dumps([
    {"id": {"uuid": "a1", "name": "Countdown", "index": 0}, "time": "00:00:01", "state": "stopped"},
    {"id": {"uuid": "b2", "name": "Elapsed", "index": 1}, "time": "00:21:43", "state": "running"}])},
    expect_state={"timers": {"a1": {"name": "Countdown", "time": "00:00:01", "state": "stopped"},
                             "b2": {"name": "Elapsed", "time": "00:21:43", "state": "running"}}})


# ── Telemetry ─────────────────────────────────────────────────────────────
# Videohub: status blocks on connect and after every change; 0-based on the
# wire, 1-based in the state (Videohub Ethernet Protocol).
telemetry(VH, "routing", inbound="VIDEO OUTPUT ROUTING:\n0 5\n1 0\n\n",
          expect_state={"outputs": {"1": {"input": 6}, "2": {"input": 1}}})
telemetry(VH, "labels", inbound="INPUT LABELS:\n0 Camera 1\n1 Camera 2\n\n",
          expect_state={"inputs": {"1": {"label": "Camera 1"}, "2": {"label": "Camera 2"}}})
telemetry(VH, "locks", inbound="VIDEO OUTPUT LOCKS:\n0 O\n1 L\n2 U\n\n",
          expect_state={"outputs": {"1": {"lock": "ours"}, "2": {"lock": "other"}, "3": {"lock": "unlocked"}}})
telemetry(VH, "device", inbound="VIDEOHUB DEVICE:\nDevice present: true\nModel name: Smart Videohub 12G 40x40\n"
          "Video inputs: 40\nVideo outputs: 40\n\n",
          expect_state={"device": {"model": "Smart Videohub 12G 40x40", "inputs": 40, "outputs": 40}})
telemetry(VH, "take-mode", inbound="CONFIGURATION:\nTake Mode: true\n\n", expect_state={"take_mode": True})

# HyperDeck: notify on connect, then transport info; the asynchronous 508 and
# the 208 reply carry the same fields (HyperDeck Ethernet Protocol).
telemetry(H, "transport", expect_connect_wire=["notify: transport: true\r\n"],
          inbound="508 transport info:\r\nstatus: play\r\nspeed: 100\r\nslot id: 1\r\nclip id: 3\r\n"
          "single clip: false\r\nloop: true\r\ntimecode: 00:00:10:00\r\n\r\n",
          expect_state={"transport": {"status": "play", "speed": 100, "slot": 1, "clip": 3,
                                      "single_clip": False, "loop": True, "timecode": "00:00:10:00"}})

# TriCaster: the tally dictionary (Automation and Integration Guide p.67-68).
telemetry("newtek-tricaster", "tally", inbound_http={
    "path": "/v1/dictionary?key=tally",
    "body": '<tally><column name="input1" index="0" on_pgm="true" on_prev="false" ndi_id="0"/>'
            '<column name="ddr1" index="16" on_pgm="false" on_prev="true"/></tally>'},
    expect_state={"tally": {"input1": {"program": True, "preview": False},
                            "ddr1": {"program": False, "preview": True}}})

# AJA Ki Pro: the transport state parameter's JSON (AJA REST automation guide).
telemetry("aja-kipro", "transport", inbound_http={
    "path": "/config?action=get&paramid=eParamID_TransportState",
    "body": '{"paramid":"eParamID_TransportState","value":"2","value_name":"Recording"}'},
    expect_state={"transport": {"state": "Recording"}})

# X32: /xremote on connect; pushed changes arrive on the same addresses as sets.
# The subscription, then the first of the one-at-a-time queries for current values.
telemetry(X, "channel-mute", expect_connect_wire_hex=[hexs(osc("/xremote")), hexs(osc("/ch/01/mix/on"))],
          inbound_hex=hexs(osc("/ch/07/mix/on", ("i", 0))),
          expect_state={"channels": {"7": {"mute": True}}})
telemetry(X, "channel-name", inbound_hex=hexs(osc("/ch/12/config/name", ("s", "Vox"))),
          expect_state={"channels": {"12": {"name": "Vox"}}})
telemetry(X, "main-fader", inbound_hex=hexs(osc("/main/st/mix/fader", ("f", 0.5))),
          expect_state={"main": {"fader": 0.5}})


# Large per-device sets, one file each, written in this file's helpers.
for extra in sorted((ROOT / "tools" / "vectors").glob("*.py")):
    exec(compile(extra.read_text(encoding="utf-8"), str(extra), "exec"), globals())


def main() -> None:
    written = set()
    for v in V:
        name = v["command"] if "command" in v else f"telemetry-{v['telemetry']}"
        path = ROOT / "vectors" / v["spec"] / f"{name}.yaml"
        path.parent.mkdir(parents=True, exist_ok=True)
        body = yaml.safe_dump(v, sort_keys=False, allow_unicode=True, width=100)
        path.write_text(body, encoding="utf-8", newline="\n")
        written.add(path)
    print(f"wrote {len(written)} vectors")


if __name__ == "__main__":
    main()
