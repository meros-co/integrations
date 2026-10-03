# Green Hippo Hippotizer (greenhippo-hippotizer): one vector per command, over
# the HippoAPI REST component on port 40512. Targets are the paths of Green
# Hippo's Swagger document "Hippotizer API" v1.2, with values in the path,
# percent-encoded as path segments by hand. Response bodies follow the
# document's models (ServerInfo, MixInfo, MediaMapModel, PresetCollectionInfo,
# PinInformation); the level reply is the bare integer the document declares.
# Telemetry: the Web Callbacks events of the Hippotizer V4 manual, topic "Web
# Callbacks", and the subscription array it gives.
GH = "greenhippo-hippotizer"


def _gh(command, input, method, target, **extra):
    V.append({"spec": GH, "command": command, "input": input,
              "expect_request": {"method": method, "target": target}, **extra})


_OK = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}


def _gh_err(code):
    return {"http_reply": {"status": code, "body": ""},
            "expect_result": {"error": {"error": "device_error", "code": str(code)}}}


INFO = {"computerName": "HIPPO-01", "engineStatus": "Running", "hostName": "Hippo 1",
        "iP": "10.0.0.11", "mediaManagerStatus": "Idle",
        "mixes": [{"fXCName": "Standard.fxc", "hasLayers": True, "index": 1, "layerCount": 8,
                   "mixType": "XFade", "name": "Mix 1"}],
        "product": "Boreal+", "productFamily": "V4", "registeredOwner": "",
        "softwareRevision": "18210", "softwareVersion": "4.9.4"}
MEDIA_ID = "3f1c2a9e-1111-4c2b-9d7e-2b8f2d9c1a01_0123456789abcdef0123456789abcdef_5a0e2c6d-2222-4f3a-8a1b-7c9d0e1f2a3b"

# Server and timelines.
_gh("get_info", {}, "GET", "/info", http_reply={"status": 200, "body": json.dumps(INFO)},
    expect_result={"ok": {"kind": "value", "value": INFO}})
_gh("get_timelines", {}, "GET", "/timelines")
_gh("play_timeline", {"timeline": 1}, "GET", "/timelines/1/play", **_OK)
_gh("stop_timeline", {"timeline": 1}, "GET", "/timelines/1/stop", **_gh_err(400))
_gh("reset_timeline", {"timeline": 2}, "GET", "/timelines/2/reset")
_gh("mute_timeline", {"timeline": 1}, "GET", "/timelines/1/mute")
_gh("unmute_timeline", {"timeline": 1}, "GET", "/timelines/1/unmute")
_gh("timeline_next_cue", {"timeline": 1}, "GET", "/timelines/1/gonextcue")
_gh("timeline_previous_cue", {"timeline": 1}, "GET", "/timelines/1/gopreviouscue")
_gh("timeline_go_cue", {"timeline": 1, "cue": "4.5"}, "GET", "/timelines/1/gocue/4.5", **_OK)
_gh("play_all_timelines", {}, "GET", "/timelines/all/play", **_gh_err(500))
_gh("stop_all_timelines", {}, "GET", "/timelines/all/stop")
_gh("reset_all_timelines", {}, "GET", "/timelines/all/reset")
_gh("mute_all_timelines", {}, "GET", "/timelines/all/mute")
_gh("unmute_all_timelines", {}, "GET", "/timelines/all/unmute")
_gh("all_timelines_next_cue", {}, "GET", "/timelines/all/gonextcue")
_gh("all_timelines_previous_cue", {}, "GET", "/timelines/all/gopreviouscue")
_gh("all_timelines_go_cue", {"cue": "12"}, "GET", "/timelines/all/gocue/12")

# Presets.
_gh("get_presets", {"type": "layer"}, "GET", "/presets/layer")
_gh("delete_preset", {"preset_id": "6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a"}, "GET",
    "/presets/delete/6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a", **_OK)

# Mixes.
_gh("get_mix_level", {"mix": 1}, "GET", "/mix/1/level", http_reply={"status": 200, "body": "75"},
    expect_result={"ok": {"kind": "value", "value": 75}})
_gh("set_mix_level", {"mix": 1, "level": 100}, "GET", "/mix/1/level/100", **_OK)
_gh("load_mix_preset", {"mix": 1, "preset": 1283}, "GET", "/mix/1/preset/1283")
_gh("load_mix_preset_bank_slot", {"mix": 2, "bank": 5, "slot": 3}, "GET", "/mix/2/preset/5/3",
    **_gh_err(404))
_gh("load_mix_preset_id", {"mix": 1, "preset_id": "6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a"}, "GET",
    "/mix/1/preset/6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a")

# Layers.
_gh("get_layer_level", {"mix": 1, "layer": 2}, "GET", "/mix/1/layer/2/level",
    http_reply={"status": 200, "body": "40"}, expect_result={"ok": {"kind": "value", "value": 40}})
_gh("set_layer_level", {"mix": 1, "layer": 2, "level": 0}, "GET", "/mix/1/layer/2/level/0")
_gh("load_layer_media", {"mix": 1, "layer": 1, "media_id": MEDIA_ID}, "GET",
    "/mix/1/layer/1/media/" + MEDIA_ID)
_gh("load_layer_media_index", {"mix": 1, "layer": 1, "index": 17}, "GET", "/mix/1/layer/1/media/17", **_OK)
_gh("play_layer_media_oneshot", {"mix": 1, "layer": 3, "media_id": MEDIA_ID}, "GET",
    "/mix/1/layer/3/mediaoneshot/" + MEDIA_ID)
_gh("play_layer_media_oneshot_index", {"mix": 1, "layer": 3, "index": 2}, "GET",
    "/mix/1/layer/3/mediaoneshot/2")
_gh("load_layer_preset", {"mix": 1, "layer": 1, "preset": 0}, "GET", "/mix/1/layer/1/preset/0")
_gh("load_layer_preset_bank_slot", {"mix": 1, "layer": 1, "bank": 0, "slot": 12}, "GET",
    "/mix/1/layer/1/preset/0/12", **_OK)
_gh("load_layer_preset_id", {"mix": 1, "layer": 1, "preset_id": "Preset A"}, "GET",
    "/mix/1/layer/1/preset/Preset%20A")

# Media.
_gh("get_media", {}, "GET", "/media")
_gh("get_media_info", {"media_id": MEDIA_ID}, "GET", "/media/" + MEDIA_ID, **_gh_err(404))
_gh("get_media_info_index", {"index": 1}, "GET", "/media/1")
_gh("get_media_map", {}, "GET", "/media/map")
_gh("delete_media", {"media_id": MEDIA_ID}, "DELETE", "/media/delete/" + MEDIA_ID, **_OK)
_gh("delete_media_map_entry", {"index": 65535}, "DELETE", "/media/deletemapentry/65535")
_gh("add_media_map_entry", {"index": 4, "media_id": MEDIA_ID}, "PUT", "/media/addmapentry/4/" + MEDIA_ID)
_gh("sync_media", {}, "GET", "/media/sync")

# Pins.
_gh("get_pin_value", {"pin": "Engine_Mix1_Layer1_Mixer_Level"}, "GET",
    "/pin/getvalue/Engine_Mix1_Layer1_Mixer_Level",
    http_reply={"status": 200, "body": "0.5"}, expect_result={"ok": {"kind": "value", "value": "0.5"}})
_gh("set_pin_value", {"pin": "Engine_Mix1_Layer1_Source_MediaPlayer_PlayMode", "value": "1"}, "GET",
    "/pin/setvalue/Engine_Mix1_Layer1_Source_MediaPlayer_PlayMode/1", **_OK)
_gh("reset_pin", {"pin": "Engine_Mix1_Layer1_Mixer_Level"}, "GET", "/pin/reset/Engine_Mix1_Layer1_Mixer_Level",
    **_gh_err(404))
_gh("get_pin_info", {"pin": "LEDComponent_SystemStatus"}, "GET", "/pin/getinfo/LEDComponent_SystemStatus")
_gh("fade_pin_value", {"pin": "Engine_Mix1_Layer1_Mixer_Level", "value": 0.25, "time_ms": 2000}, "GET",
    "/pin/fadevalue/Engine_Mix1_Layer1_Mixer_Level/0.2500/2000")

# ── Telemetry ──
SUBSCRIBE = ['[{"subscribe":{"category":"MEDIA"}},{"subscribe":{"category":"PRESETS"}},'
             '{"subscribe":{"category":"SYSTEM"}}]']
telemetry(GH, "info", inbound_http={"path": "/info", "body": json.dumps(INFO)},
          expect_state={"server": {"computer_name": "HIPPO-01", "host_name": "Hippo 1", "ip": "10.0.0.11",
                                   "product": "Boreal+", "product_family": "V4", "software_version": "4.9.4",
                                   "software_revision": "18210", "engine_status": "Running",
                                   "media_manager_status": "Idle", "registered_owner": ""},
                        "mixes": {"1": {"name": "Mix 1", "type": "XFade", "layer_count": 8,
                                        "has_layers": True, "fxc_name": "Standard.fxc"}}})
telemetry(GH, "media-map", inbound_http={"path": "/media/map", "body": json.dumps(
    {"entries": [{"index": 1, "mediaID": MEDIA_ID, "name": "Intro.mov"}]})},
          expect_state={"media_map": {"1": {"media_id": MEDIA_ID, "name": "Intro.mov"}}})
telemetry(GH, "layer-level", inbound_http={"path": "/mix/1/layer/2/level", "body": "40"},
          expect_state={"mixes": {"1": {"layers": {"2": {"level": 40}}}}})
telemetry(GH, "mix-level", inbound_http={"path": "/mix/2/level", "body": "100"},
          expect_state={"mixes": {"2": {"level": 100}}})
telemetry(GH, "preset-banks", inbound_http={"path": "/presets/mix", "body": json.dumps(
    {"bankCount": 256, "presetType": "mix",
     "banks": [{"hasPresets": True, "index": 0, "name": "Looks", "presets": [], "thumbPreset": ""}]})},
          expect_state={"preset_banks": {"mix": {"0": {"name": "Looks", "has_presets": True}}}})
telemetry(GH, "system-status", expect_connect_ws=SUBSCRIBE,
          inbound_ws='{"category":"SYSTEM","event":"SYSTEM_STATUS_CHANGED","data":"Configuring"}',
          expect_state={"server": {"output_status": "Configuring"},
                        "last_event": {"category": "SYSTEM", "event": "SYSTEM_STATUS_CHANGED"}})
telemetry(GH, "preset-added",
          inbound_ws=json.dumps({"category": "PRESETS", "event": "PRESET_ADDED",
                                 "data": {"presetid": "6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a",
                                          "bank": "5", "slot": "16", "type": "layer"}}),
          expect_state={"presets": {"6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a": {
              "bank": 5, "slot": 16, "type": "layer", "deleted": False}},
              "last_event": {"category": "PRESETS", "event": "PRESET_ADDED"}})
telemetry(GH, "preset-deleted",
          inbound_ws='{"category":"PRESETS","event":"PRESET_DELETED","data":"6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a"}',
          expect_state={"presets": {"6f1b7c1e-8a5d-4f0e-9c1a-0e5b2d3c4f5a": {"deleted": True}},
                        "last_event": {"category": "PRESETS", "event": "PRESET_DELETED"}})
telemetry(GH, "media-added",
          inbound_ws=json.dumps({"category": "MEDIA", "event": "MEDIAFILES_ADDED", "data": [{"mediaID": MEDIA_ID}]}),
          expect_state={"media": {MEDIA_ID: {"deleted": False}},
                        "last_event": {"category": "MEDIA", "event": "MEDIAFILES_ADDED"}})
telemetry(GH, "media-deleted",
          inbound_ws=json.dumps({"category": "MEDIA", "event": "MEDIAFILE_DELETED", "data": MEDIA_ID}),
          expect_state={"media": {MEDIA_ID: {"deleted": True}},
                        "last_event": {"category": "MEDIA", "event": "MEDIAFILE_DELETED"}})
telemetry(GH, "map-changed", inbound_ws='{"category":"MEDIA","event":"MEDIAMAP_CHANGED"}',
          expect_state={"last_event": {"category": "MEDIA", "event": "MEDIAMAP_CHANGED"}})
