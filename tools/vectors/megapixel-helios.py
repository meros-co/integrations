# Megapixel HELIOS (megapixel-helios): one vector per command over the Public
# API (v24.07.0.22791). Property writes are PATCH /api/v1/public with the part
# of the tree as the JSON body (3.1.2-3.1.3, 8.2-8.6); reads are GET
# /api/v1/public with dotted property paths in the query (3.1.1, 7.1); saved
# configurations, stills and previews have their own endpoints (3.2-3.4).
# Bodies are written here with json.dumps from the document's examples, not
# from the spec's templates; floats sent with fixed decimals are written by
# hand.
MH = "megapixel-helios"


def _mh(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body if isinstance(body, str) else json.dumps(body, separators=(",", ":"))
    V.append({"spec": MH, "command": command, "input": input, "expect_request": request, **extra})


def _mh_patch(command, input, tree, **extra):
    _mh(command, input, "PATCH", "/api/v1/public", tree, **extra)


_MH_ACK = {"ok": {"kind": "ack"}}

# ── Reading ──────────────────────────────────────────────────────────────
_mh("get_state", {}, "GET", "/api/v1/public",
    http_reply={"status": 200, "body": '{"dev":{"display":{"brightness":50,"gamma":1.5}}}'},
    expect_result={"ok": {"kind": "value", "value": {"dev": {"display": {"brightness": 50, "gamma": 1.5}}}}})
# 3.1.1.2: brightness and gamma.
_mh("get_properties", {"paths": "dev.display.brightness&dev.display.gamma"}, "GET",
    "/api/v1/public?dev.display.brightness&dev.display.gamma",
    http_reply={"status": 200, "body": '{"dev":{"display":{"brightness":50,"gamma":2.4}}}'},
    expect_result={"ok": {"kind": "value", "value": {"dev": {"display": {"brightness": 50, "gamma": 2.4}}}}})
# 3.1.3: brightness and gamma in one PATCH; the reply holds what was set.
_mh_patch("set_properties", {"tree": {"dev": {"display": {"brightness": 42, "gamma": 2.2}}}},
          {"dev": {"display": {"brightness": 42, "gamma": 2.2}}},
          http_reply={"status": 200, "body": '{"dev":{"display":{"brightness":42,"gamma":2.200000047683716}}}'},
          expect_result={"ok": {"kind": "value", "value": {"dev": {"display": {"brightness": 42,
                                                                              "gamma": 2.200000047683716}}}}})
# 7.1 and 7.1.1.
_mh("get_alerts", {}, "GET", "/api/v1/public?sys.alerts&sys.alertsCount&sys.alertsSeverity",
    http_reply={"status": 200, "body": '{"sys":{"alertsCount":2,"alertsSeverity":3}}'},
    expect_result={"ok": {"kind": "value", "value": {"alertsCount": 2, "alertsSeverity": 3}}})

# ── Display ──────────────────────────────────────────────────────────────
# 8.2.1: blackout, answered with the new value.
_mh_patch("set_blackout", {"enabled": True}, {"dev": {"display": {"blackout": True}}},
          http_reply={"status": 200, "body": '{"dev": {"display": {"blackout": true}}}'}, expect_result=_MH_ACK)
_mh_patch("set_freeze", {"enabled": True}, {"dev": {"display": {"freeze": True}}})
# 8.3.2: brightness 50 (sent with two decimals).
_mh_patch("set_brightness", {"brightness": 50}, '{"dev":{"display":{"brightness":50.00}}}',
          http_reply={"status": 200, "body": '{"dev": {"display": {"brightness": 50}}}'}, expect_result=_MH_ACK)
_mh_patch("set_cct", {"kelvin": 6504}, {"dev": {"display": {"cct": 6504}}})
_mh_patch("set_cct_duv", {"duv": 0.0032}, '{"dev":{"display":{"cctDuv":0.0032}}}')
# 8.4.2: gamma 2.1.
_mh_patch("set_gamma", {"gamma": 2.1}, '{"dev":{"display":{"gamma":2.10}}}')
_mh_patch("set_black_clipping", {"level": 0.25}, '{"dev":{"display":{"blackClipping":0.250}}}')
_mh_patch("set_display_gains", {"r": 1, "g": 0.95, "b": 1},
          '{"dev":{"display":{"gains":{"r":1.000,"g":0.950,"b":1.000}}}}')
# The Postman collection's examples: x 500, y 600; width 1840, height 1160.
_mh_patch("set_display_window", {"x": 500, "y": 600, "width": 1840, "height": 1160},
          {"dev": {"display": {"x": 500, "y": 600, "width": 1840, "height": 1160}}})
_mh_patch("set_display_size", {"width": 1840, "height": 1160}, {"dev": {"display": {"width": 1840, "height": 1160}}})
_mh_patch("set_output_adjust_enabled", {"enabled": True}, {"dev": {"display": {"out": {"adjust": {"enabled": True}}}}})
for _k in ("gain", "gamma"):
    _mh_patch(f"set_output_adjust_{_k}", {"r": 1, "g": 1.05, "b": 0.9},
              '{"dev":{"display":{"out":{"adjust":{"' + _k + '":{"r":1.000,"g":1.050,"b":0.900}}}}}}')
for _k in ("lift", "offset"):
    _mh_patch(f"set_output_adjust_{_k}", {"r": 0, "g": -0.01, "b": 0.02},
              '{"dev":{"display":{"out":{"adjust":{"' + _k + '":{"r":0.000,"g":-0.010,"b":0.020}}}}}}')
_mh_patch("set_output_adjust_saturation", {"saturation": 1.2},
          '{"dev":{"display":{"out":{"adjust":{"saturation":1.200}}}}}')

# ── Redundancy ───────────────────────────────────────────────────────────
_mh_patch("set_redundancy_mode", {"mode": "failover"}, {"dev": {"display": {"redundancy": {"mode": "failover"}}}})
_mh_patch("set_redundancy_role", {"role": "backup"}, {"dev": {"display": {"redundancy": {"role": "backup"}}}})
# 8.6.1: flip to main, answered with state active.
_mh_patch("redundancy_flip", {"to": "main"}, {"dev": {"display": {"redundancy": {"state": "main"}}}},
          http_reply={"status": 200, "body": '{"dev":{"display":{"redundancy":{"state":"active"}}}}'},
          expect_result=_MH_ACK)

# ── Processor ────────────────────────────────────────────────────────────
_mh_patch("select_input", {"input": "sdi1"}, {"dev": {"ingest": {"input": "sdi1"}}})
# 8.5.1 and 8.5.2.
_mh_patch("set_test_pattern", {"type": "colorBars"},
          {"dev": {"ingest": {"testPattern": {"enabled": True, "type": "colorBars"}}}})
_mh_patch("set_test_pattern_enabled", {"enabled": False}, {"dev": {"ingest": {"testPattern": {"enabled": False}}}})
_mh_patch("set_test_pattern_motion", {"enabled": True}, {"dev": {"ingest": {"testPattern": {"motion": True}}}})

# ── Groups (dev.groups.0) ────────────────────────────────────────────────
_mh_patch("set_group_blackout", {"group": 0, "enabled": True}, {"dev": {"groups": {"0": {"blackout": True}}}})
_mh_patch("set_group_name", {"group": 2, "name": "Ceiling"}, {"dev": {"groups": {"2": {"name": "Ceiling"}}}})
_mh_patch("set_group_gains", {"group": 1, "r": 1, "g": 1, "b": 0.9, "i": 0.5},
          '{"dev":{"groups":{"1":{"gains":{"r":1.000,"g":1.000,"b":0.900,"i":0.500}}}}}')
_mh_patch("set_group_mask_enabled", {"group": 0, "enabled": False}, {"dev": {"groups": {"0": {"mask": {"enabled": False}}}}})
_mh_patch("set_group_mask", {"group": 0, "l": 10, "t": 0, "b": 0, "r": 10},
          '{"dev":{"groups":{"0":{"mask":{"l":10.000,"t":0.000,"b":0.000,"r":10.000}}}}}')
_mh_patch("set_group_test_pattern_enabled", {"group": 3}, {"dev": {"groups": {"3": {"testPattern": {"enabled": True}}}}})
_mh_patch("set_group_test_pattern_color", {"group": 3, "r": 1, "g": 0, "b": 0, "a": 1},
          '{"dev":{"groups":{"3":{"testPattern":{"r":1.000,"g":0.000,"b":0.000,"a":1.000}}}}}')

# ── Receivers (6.1.4: keyed by MAC address) ──────────────────────────────
_mh_patch("set_receiver_group", {"mac": "58:98:6f:00:01:7d", "group": 2},
          {"dev": {"receivers": {"58:98:6f:00:01:7d": {"groupId": 2}}}})
_mh_patch("set_receiver_position", {"mac": "58:98:6f:00:01:7d", "x": 360, "y": 0},
          {"dev": {"receivers": {"58:98:6f:00:01:7d": {"x": 360, "y": 0}}}})
_mh_patch("set_processor_name", {"name": "Stage Left"}, {"sys": {"description": "Stage Left"}})

# ── Saved configurations (3.3) ───────────────────────────────────────────
_MH_PRESETS = [{"id": 3, "presetName": "Global Settings", "createdAt": "2021-01-05T20:36:42.227Z",
                "updatedAt": "2021-01-05T20:36:42.227Z"}]
_mh("list_presets", {}, "GET", "/api/v1/presets/list",
    http_reply={"status": 200, "body": json.dumps({"presets": _MH_PRESETS})},
    expect_result={"ok": {"kind": "value", "value": _MH_PRESETS}})
_mh("list_presets_full", {}, "GET", "/api/v1/presets")
# 3.3.5: apply id 3; 3.3.3: an invalid one answers 404.
_mh("recall_preset", {"id": 4}, "POST", "/api/v1/presets/4/apply",
    http_reply={"status": 404, "body": "Not Found"}, expect_result={"error": {"error": "device_error", "code": "404"}})
_mh("recall_preset_by_name", {"name": "Global Settings"}, "POST", "/api/v1/presets/apply",
    {"presetName": "Global Settings"},
    http_reply={"status": 200, "body": '{"dev":{"display":{"brightness":5.7,"cct":6504,"gamma":2.4,"blackout":false}}}'},
    expect_result=_MH_ACK)

# ── Stills (3.4) ─────────────────────────────────────────────────────────
_mh("list_stills", {}, "GET", "/api/v1/media")
# 3.4.6: show id 26, answered 204.
_mh("show_still", {"id": 26}, "POST", "/api/v1/media/26/show", http_reply={"status": 204, "body": ""},
    expect_result=_MH_ACK)
_mh("show_still_by_name", {"name": "VGA-no-signal-image.jpeg"}, "POST", "/api/v1/media/show",
    {"name": "VGA-no-signal-image.jpeg"})
_mh("hide_still", {}, "POST", "/api/v1/media/hide")
# 3.2.1.
_mh("list_preview_sources", {}, "GET", "/api/v1/preview",
    http_reply={"status": 200, "body": '["preview","hdmi","sdi1"]'},
    expect_result={"ok": {"kind": "value", "value": ["preview", "hdmi", "sdi1"]}})

# ── Telemetry ────────────────────────────────────────────────────────────
# 4.1: the state method is sent when the websocket opens; its result is the tree.
telemetry(MH, "ws-state", expect_connect_ws=['{"jsonrpc":"2.0","id":1,"method":"state"}'],
          inbound_ws=json.dumps({"jsonrpc": "2.0", "id": 1, "result": {
              "dev": {"display": {"brightness": 28.2, "gamma": 1.5, "blackout": False,
                                  "redundancy": {"mode": "failover", "role": "main", "state": "active"}},
                      "ingest": {"input": "hdmi1", "testPattern": {"enabled": False},
                                 "inputs": {"hdmi1": {"valid": True, "resolution": "3840x2160p60.00", "freq": 60}}},
                      "groups": {"0": {"name": "Floor", "blackout": True}}},
              "sys": {"alertsCount": 2, "alertsSeverity": 3}}}),
          expect_state={"display": {"brightness": 28.2, "gamma": 1.5, "blackout": False,
                                    "redundancy": {"mode": "failover", "role": "main", "state": "active"}},
                        "ingest": {"input": "hdmi1", "test_pattern": {"enabled": False},
                                   "inputs": {"hdmi1": {"valid": True, "resolution": "3840x2160p60.00", "freq": 60.0}}},
                        "groups": {"0": {"name": "Floor", "blackout": True}},
                        "sys": {"alerts_count": 2, "alerts_severity": 3}})
# 8.3.1: an update notification.
telemetry(MH, "ws-update", inbound_ws=json.dumps({"jsonrpc": "2.0", "method": "update",
                                                  "params": {"dev": {"display": {"brightness": 33.29999923706055}}}}),
          expect_state={"display": {"brightness": 33.29999923706055}})
# 8.6.1: the reply to a redundancy flip.
telemetry(MH, "http-redundancy", inbound_http={"path": "/api/v1/public",
                                               "body": '{"dev":{"display":{"redundancy":{"state":"active"}}}}'},
          expect_state={"display": {"redundancy": {"state": "active"}}})
telemetry(MH, "http-query", inbound_http={"path": "/api/v1/public?sys.alertsCount&sys.alertsSeverity",
                                          "body": '{"sys":{"alertsCount":0,"alertsSeverity":-1}}'},
          expect_state={"sys": {"alerts_count": 0, "alerts_severity": -1}})
# 3.3.5: applying a saved configuration answers with the applied state.
telemetry(MH, "preset-applied", inbound_http={"path": "/api/v1/presets/3/apply", "body": json.dumps({
    "dev": {"display": {"brightness": 5.7, "cct": 6504, "gamma": 2.4, "blackout": False},
            "ingest": {"testPattern": {"enabled": False}}}})},
    expect_state={"display": {"brightness": 5.7, "cct": 6504.0, "gamma": 2.4, "blackout": False},
                  "ingest": {"test_pattern": {"enabled": False}}})
