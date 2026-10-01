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
http(T, "shortcut", {"name": "main_take"}, "GET", "/v1/shortcut?name=main_take")
http(T, "shortcut_with_value", {"name": "main_a_row", "value": "3"}, "GET", "/v1/shortcut?name=main_a_row&value=3")
http(T, "trigger", {"name": "Lower Third"}, "GET", "/v1/trigger?name=Lower%20Third")
http(T, "set_datalink", {"key": "%Score%", "value": "2-1"}, "GET", "/v1/datalink?key=%25Score%25&value=2-1")
telemetry(T, "switcher", inbound_http={
    "path": "/v1/dictionary?key=switcher",
    "body": '<switcher_update main_source="BFR4" preview_source="INPUT1"/>'},
    expect_state={"switcher": {"program": "BFR4", "preview": "INPUT1"}})
telemetry(T, "shortcut-states", inbound_http={
    "path": "/v1/dictionary?key=shortcut_states",
    "body": '<shortcut_states><shortcut_state name="record_toggle" value="1" type="bool" sender="unknown"/>'
            '<shortcut_state name="input1_long_name" value="Camera 1" type="" sender="unknown"/></shortcut_states>'},
    expect_state={"shortcuts": {"record_toggle": "1", "input1_long_name": "Camera 1"}})

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
