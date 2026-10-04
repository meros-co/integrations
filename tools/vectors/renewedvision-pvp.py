# Renewed Vision ProVideoPlayer 3 (renewedvision-pvp): one vector per command.
# Targets are the endpoints of PVP's own API Reference (shipped in the app at
# Contents/Resources/WebContent/help/api/0/index.html, identical in 3.8 and
# 3.9), under its /api/0 prefix. Where the document gives an example request
# (layer 0, playlist "playlistName", opacity 0.5, "Color Burn", "Target Set
# 1", 1.5 seconds, the Edge Outline effect, the Luma Matte blend), it is used.
# Names are percent-encoded as path segments by hand. Replies are the
# document's JSON examples; the success status is assumed to be 200.
PV = "renewedvision-pvp"


def _pv(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": PV, "command": command, "input": input, "expect_request": request, **extra})


_P = "/api/0"
_OK = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}


def _err(code):
    return {"http_reply": {"status": code, "body": ""},
            "expect_result": {"error": {"error": "device_error", "code": str(code)}}}


# Data.
_pv("get_playlists", {}, "GET", _P + "/data/playlists",
    http_reply={"status": 200, "body": json.dumps({"playlist": {
        "items": [], "name": "Root", "children": [
            {"items": [{"name": "002_JB_HD-H264", "uuid": "6CD29387-44B8-4D61-94D7-6BE69359D12E"}],
             "name": "Playlist 1", "children": [], "uuid": "9EE5DFC3-7EE2-47F8-B932-765CEBBA8245"}],
        "uuid": "C0E4E090-F969-4B60-9526-D44EBC5C52ED"}})},
    expect_result={"ok": {"kind": "value", "value": {
        "items": [], "name": "Root", "children": [
            {"items": [{"name": "002_JB_HD-H264", "uuid": "6CD29387-44B8-4D61-94D7-6BE69359D12E"}],
             "name": "Playlist 1", "children": [], "uuid": "9EE5DFC3-7EE2-47F8-B932-765CEBBA8245"}],
        "uuid": "C0E4E090-F969-4B60-9526-D44EBC5C52ED"}}})
_pv("get_playlist", {"playlist": "Playlist 1"}, "GET", _P + "/data/playlist/Playlist%201",
    **_err(404))
_pv("get_cue", {"playlist": "0", "cue": "0"}, "GET", _P + "/data/playlist/0/cue/0",
    http_reply={"status": 200, "body": '{"playlistItem":{"name":"Abstract Cheer Gold Fast-HD 1080",'
                                       '"uuid":"0CC37FC9-3784-4599-9088-B06C55ACB1D0"}}'},
    expect_result={"ok": {"kind": "value", "value": {"name": "Abstract Cheer Gold Fast-HD 1080",
                                                     "uuid": "0CC37FC9-3784-4599-9088-B06C55ACB1D0"}}})
_pv("get_layers", {}, "GET", _P + "/data/layers")
_pv("get_layer", {"layer": "0"}, "GET", _P + "/data/layer/0",
    http_reply={"status": 200, "body": '{"layer":{"isHidden":false,"name":"Layer 1","isMuted":false}}'},
    expect_result={"ok": {"kind": "value", "value": {"isHidden": False, "name": "Layer 1", "isMuted": False}}})

# Clear, mute, hide.
_pv("clear_workspace", {}, "POST", _P + "/clear/workspace", **_OK)
_pv("clear_layer", {"layer": "0"}, "POST", _P + "/clear/layer/0", **_err(400))
_pv("mute_workspace", {}, "POST", _P + "/mute/workspace")
_pv("mute_layer", {"layer": "0"}, "POST", _P + "/mute/layer/0", **_OK)
_pv("unmute_workspace", {}, "POST", _P + "/unmute/workspace")
_pv("unmute_layer", {"layer": "Layer 2"}, "POST", _P + "/unmute/layer/Layer%202")
# A 401 is the token refused: terminal, reported as auth (auth: bearer).
_pv("hide_workspace", {}, "POST", _P + "/hide/workspace",
    http_reply={"status": 401}, expect_result={"error": {"error": "auth"}})
_pv("hide_layer", {"layer": "0"}, "POST", _P + "/hide/layer/0")
_pv("unhide_workspace", {}, "POST", _P + "/unhide/workspace")
_pv("unhide_layer", {"layer": "394B7D0F-050E-4590-98B9-B30A8823488D"}, "POST",
    _P + "/unhide/layer/394B7D0F-050E-4590-98B9-B30A8823488D")

# Select.
_pv("select_layer", {"layer": "0"}, "POST", _P + "/select/layer/0", **_OK)
_pv("select_layer_target", {"layer": "0", "target": True}, "POST", _P + "/select/layer/0?target=true")
_pv("select_playlist", {"playlist": "-1"}, "POST", _P + "/select/playlist/-1")

# Trigger.
_pv("trigger_cue", {"cue": "0"}, "POST", _P + "/trigger/cue/0", **_OK)
_pv("trigger_playlist", {"playlist": "playlistName"}, "POST", _P + "/trigger/playlist/playlistName")
_pv("trigger_playlist_cue", {"playlist": "playlistName", "cue": "0"}, "POST",
    _P + "/trigger/playlist/playlistName/cue/0", **_err(404))
_pv("trigger_layer_cue", {"layer": "0", "playlist": "0", "cue": "0"}, "POST",
    _P + "/trigger/layer/0/playlist/0/cue/0", **_OK)

# Opacity and blending.
_pv("get_layer_opacity", {"layer": "0"}, "GET", _P + "/opacity/layer/0",
    http_reply={"status": 200, "body": '{"value":0.5}'},
    expect_result={"ok": {"kind": "value", "value": {"value": 0.5}}})
_pv("set_layer_opacity", {"layer": "0", "opacity": 0.5}, "POST", _P + "/opacity/layer/0",
    '{"value":0.500}', **_OK)
_pv("get_blend_modes", {}, "GET", _P + "/blendMode",
    http_reply={"status": 200, "body": '{"data":[{"blendMode":{"name":"Normal","id":0}},'
                                       '{"blendMode":{"name":"Dissolve","id":1}}]}'},
    expect_result={"ok": {"kind": "value", "value": [{"blendMode": {"name": "Normal", "id": 0}},
                                                     {"blendMode": {"name": "Dissolve", "id": 1}}]}})
_pv("get_layer_blend_mode", {"layer": "0"}, "GET", _P + "/blendMode/layer/0",
    http_reply={"status": 200, "body": '{"blendMode":{"name":"Normal","id":0}}'},
    expect_result={"ok": {"kind": "value", "value": {"name": "Normal", "id": 0}}})
_pv("set_layer_blend_mode", {"layer": "0", "mode": "Color Burn"}, "POST", _P + "/blendMode/layer/0",
    '{"value":"Color Burn"}')
_pv("get_layer_blend", {"layer": "0"}, "GET", _P + "/blend/layer/0", **_err(404))
_pv("set_layer_blend", {"layer": "0", "body": {"type": "Luma Matte", "base": {"isInverted": True}}},
    "POST", _P + "/blend/layer/0", '{"type":"Luma Matte","base":{"isInverted":true}}')
_pv("set_layer_blend_standard", {"layer": "1", "mode_index": 3, "opacity": 0.75}, "POST",
    _P + "/blend/layer/1", '{"type":"Standard","base":{"modeIndex":3,"opacity":0.750}}')
_pv("set_layer_blend_alpha_matte", {"layer": "1", "inverted": False}, "POST", _P + "/blend/layer/1",
    '{"type":"Alpha Matte","base":{"isInverted":false}}')
_pv("set_layer_blend_luma_matte", {"layer": "0", "inverted": True}, "POST", _P + "/blend/layer/0",
    '{"type":"Luma Matte","base":{"isInverted":true}}', **_OK)

# Layer presets and target sets.
_pv("get_layer_presets", {}, "GET", _P + "/layerPreset",
    http_reply={"status": 200, "body": '{"data":[{"id":"example","name":"exampleBlendModeName"}]}'},
    expect_result={"ok": {"kind": "value", "value": [{"id": "example", "name": "exampleBlendModeName"}]}})
_pv("get_layer_preset", {"layer": "0"}, "GET", _P + "/layerPreset/layer/0")
_pv("set_layer_preset", {"layer": "0", "preset": "exampleLayerPresetName"}, "POST",
    _P + "/layerPreset/layer/0", '{"value":"exampleLayerPresetName"}')
_pv("get_target_sets", {}, "GET", _P + "/targetSet")
_pv("get_layer_target_set", {"layer": "0"}, "GET", _P + "/targetSet/layer/0",
    http_reply={"status": 200, "body": '{"targetSet":{"name":"Left","uuid":"AC04BC27-F3C2-429D-BC9B-446D04465EE9"}}'},
    expect_result={"ok": {"kind": "value", "value": {"targetSet": {
        "name": "Left", "uuid": "AC04BC27-F3C2-429D-BC9B-446D04465EE9"}}}})
_pv("set_layer_target_set", {"layer": "0", "target_set": "Target Set 1"}, "POST", _P + "/targetSet/layer/0",
    '{"value":"Target Set 1"}', **_OK)
_pv("clear_layer_target_set", {"layer": "0"}, "POST", _P + "/targetSet/layer/0")

# Effects.
_EDGE = {"effect": {"variables": [
    {"type": "Float", "base": {"value": 0.5, "max_value": 1, "min_value": 0, "name": "Outline Amount"}},
    {"type": "Float", "base": {"value": 0.5, "max_value": 1, "min_value": 0, "name": "Color Level"}}],
    "enabled": True, "name": "Edge Outline", "uuid": "86BA6E3E-358F-4DA2-93C4-6A2C0EC831DA"}}
_EDGE_WIRE = ('{"effect":{"variables":['
              '{"type":"Float","base":{"value":0.5,"max_value":1,"min_value":0,"name":"Outline Amount"}},'
              '{"type":"Float","base":{"value":0.5,"max_value":1,"min_value":0,"name":"Color Level"}}],'
              '"enabled":true,"name":"Edge Outline","uuid":"86BA6E3E-358F-4DA2-93C4-6A2C0EC831DA"}}')
_pv("get_effects", {}, "GET", _P + "/effects")
_pv("get_workspace_effects", {}, "GET", _P + "/effects/workspace",
    http_reply={"status": 200, "body": '{"data":[{"effect":{"variables":[],"enabled":true,"name":"Color Invert",'
                                       '"uuid":"EDF55F11-1A1E-4BF3-9718-FA2D3731626D"}}]}'},
    expect_result={"ok": {"kind": "value", "value": [{"effect": {
        "variables": [], "enabled": True, "name": "Color Invert",
        "uuid": "EDF55F11-1A1E-4BF3-9718-FA2D3731626D"}}]}})
_pv("set_workspace_effects", {"body": _EDGE}, "POST", _P + "/effects/workspace", _EDGE_WIRE, **_OK)
_pv("get_layer_effects", {"layer": "0"}, "GET", _P + "/effects/layer/0")
_pv("set_layer_effects", {"layer": "0", "body": _EDGE}, "POST", _P + "/effects/layer/0", _EDGE_WIRE)

# Effect presets.
_pv("get_effects_presets", {}, "GET", _P + "/effectsPreset")
_pv("get_workspace_effects_preset", {}, "GET", _P + "/effectsPreset/workspace")
_pv("set_workspace_effects_preset", {"preset_uuid": "3B076957-6D48-45A7-803F-6D1E06B02F54"}, "POST",
    _P + "/effectsPreset/workspace", '{"value":"3B076957-6D48-45A7-803F-6D1E06B02F54"}')
_pv("clear_workspace_effects_preset", {}, "POST", _P + "/effectsPreset/workspace", '{"value":null}', **_OK)
_pv("get_layer_effects_preset", {"layer": "0"}, "GET", _P + "/effectsPreset/layer/0")
_pv("set_layer_effects_preset", {"layer": "0", "preset": "NameOfEffectsPreset"}, "POST",
    _P + "/effectsPreset/layer/0", '{"value":"NameOfEffectsPreset"}')
_pv("clear_layer_effects_preset", {"layer": "0"}, "POST", _P + "/effectsPreset/layer/0", '{"value":null}')

# Transitions.
_BURN = {"transition": {"variables": [{"type": "Color", "base": {"name": "Burn Color", "color": "#6C6C6C"}}],
                        "enabled": False, "name": "Color Burn", "uuid": "2B855B65-6ABC-4F65-8198-A19A5EF97821"}}
_BURN_WIRE = ('{"transition":{"variables":[{"type":"Color","base":{"name":"Burn Color","color":"#6C6C6C"}}],'
              '"enabled":false,"name":"Color Burn","uuid":"2B855B65-6ABC-4F65-8198-A19A5EF97821"}}')
_pv("get_transitions", {}, "GET", _P + "/transition")
_pv("get_workspace_transition", {}, "GET", _P + "/transition/workspace",
    http_reply={"status": 200, "body": '{"transition":{"variables":[],"enabled":false,'
                                       '"name":"Default (Dissolve)","uuid":"EC52A828-AD85-4602-B70C-1DEE7C904DB6"}}'},
    expect_result={"ok": {"kind": "value", "value": {"variables": [], "enabled": False,
                                                     "name": "Default (Dissolve)",
                                                     "uuid": "EC52A828-AD85-4602-B70C-1DEE7C904DB6"}}})
_pv("set_workspace_transition", {"body": _BURN}, "POST", _P + "/transition/workspace", _BURN_WIRE, **_OK)
_pv("set_workspace_transition_uuid", {"transition_uuid": "2B855B65-6ABC-4F65-8198-A19A5EF97821"}, "POST",
    _P + "/transition/workspace", '{"value":"2B855B65-6ABC-4F65-8198-A19A5EF97821"}')
_pv("get_layer_transition", {"layer": "0"}, "GET", _P + "/transition/layer/0")
_pv("set_layer_transition", {"layer": "0", "body": _BURN}, "POST", _P + "/transition/layer/0", _BURN_WIRE)
_pv("set_layer_transition_uuid", {"layer": "0", "transition_uuid": "5584E51F-5C92-47B5-9B65-3C2540C1F20C"},
    "POST", _P + "/transition/layer/0", '{"value":"5584E51F-5C92-47B5-9B65-3C2540C1F20C"}')
_pv("clear_layer_transition", {"layer": "0"}, "POST", _P + "/transition/layer/0", '{"value":null}', **_err(405))

# Transition duration.
_pv("get_workspace_transition_duration", {}, "GET", _P + "/transitionDuration/workspace",
    http_reply={"status": 200, "body": '{"transitionDuration":{"value":0.5}}'},
    expect_result={"ok": {"kind": "value", "value": 0.5}})
_pv("set_workspace_transition_duration", {"seconds": 1.5}, "POST", _P + "/transitionDuration/workspace",
    '{"value":1.50}', **_OK)
_pv("get_layer_transition_duration", {"layer": "0"}, "GET", _P + "/transitionDuration/layer/0",
    http_reply={"status": 200, "body": '{"transitionDuration":{"value":0.5}}'},
    expect_result={"ok": {"kind": "value", "value": 0.5}})
_pv("set_layer_transition_duration", {"layer": "0", "seconds": 1.5}, "POST", _P + "/transitionDuration/layer/0",
    '{"value":1.50}')

# Transport state.
_pv("get_workspace_transport_state", {}, "GET", _P + "/transportState/workspace", **_err(500))
_pv("get_layer_transport_state", {"layer": "0"}, "GET", _P + "/transportState/layer/0",
    http_reply={"status": 200, "body": '{"transportState":{"timeElapsed":0,"playbackRate":0,'
                                       '"isScrubbing":false,"timeRemaining":0,"isPlaying":false}}'},
    expect_result={"ok": {"kind": "value", "value": {"timeElapsed": 0, "playbackRate": 0, "isScrubbing": False,
                                                     "timeRemaining": 0, "isPlaying": False}}})

# Telemetry: the document's JSON examples, cut to the properties the rules read.
_L1 = "394B7D0F-050E-4590-98B9-B30A8823488D"
_L2 = "75969169-B4B3-45A1-8B4A-2A037CABDB97"
telemetry(PV, "layers", inbound_http={"path": "/api/0/data/layers", "body": json.dumps({"data": [
    {"layer": {"isHidden": False, "name": "Layer 1", "isMuted": False, "effectPresetUUID": "null",
               "effects": [], "uuid": _L1, "opacity": 1,
               "targetSetUUID": "AC04BC27-F3C2-429D-BC9B-446D04465EE9", "transitionDuration": 0.5}},
    {"layer": {"isHidden": True, "name": "Layer 2", "isMuted": True, "effectPresetUUID": "null",
               "effects": [], "uuid": _L2, "opacity": 0.74831989247311825,
               "targetSetUUID": "95DE9073-DF30-427A-A824-45120205EB29", "transitionDuration": 0.5}}]})},
    expect_state={"layers": {
        _L1: {"name": "Layer 1", "muted": False, "hidden": False, "opacity": 1.0, "transition_duration": 0.5,
              "target_set_uuid": "AC04BC27-F3C2-429D-BC9B-446D04465EE9", "effects_preset_uuid": "null"},
        _L2: {"name": "Layer 2", "muted": True, "hidden": True, "opacity": 0.74831989247311825,
              "transition_duration": 0.5, "target_set_uuid": "95DE9073-DF30-427A-A824-45120205EB29",
              "effects_preset_uuid": "null"}}})

telemetry(PV, "layer", inbound_http={"path": "/api/0/data/layer/0", "body": json.dumps({"layer": {
    "isHidden": False, "name": "Layer 1", "isMuted": False, "effectPresetUUID": "null", "effects": [],
    "uuid": _L1, "opacity": 1, "targetSetUUID": "AC04BC27-F3C2-429D-BC9B-446D04465EE9",
    "transitionDuration": 0.5}})},
    expect_state={"layers": {_L1: {
        "name": "Layer 1", "muted": False, "hidden": False, "opacity": 1.0, "transition_duration": 0.5,
        "target_set_uuid": "AC04BC27-F3C2-429D-BC9B-446D04465EE9", "effects_preset_uuid": "null"}}})

_TS = {"timeElapsed": 12.5, "playbackRate": 1, "isScrubbing": False, "timeRemaining": 47.5, "isPlaying": True,
       "playingItem": {"name": "Cue One", "uuid": "CFCAC8BA-98AB-4DE9-9D95-FAB3D0B9FC84"},
       "layer": {"isHidden": False, "name": "Layer 1", "isMuted": False,
                 "effectPresetUUID": "3B076957-6D48-45A7-803F-6D1E06B02F54", "effects": [],
                 "transition": {"variables": [], "enabled": False, "name": "Color Burn",
                                "uuid": "2B855B65-6ABC-4F65-8198-A19A5EF97821"},
                 "uuid": _L1, "opacity": 0.5, "targetSetUUID": "95DE9073-DF30-427A-A824-45120205EB29",
                 "transitionDuration": 1.5}}
_TS_STATE = {"playing": True, "scrubbing": False, "time_elapsed": 12.5, "time_remaining": 47.5,
             "playback_rate": 1.0, "cue": "Cue One", "cue_uuid": "CFCAC8BA-98AB-4DE9-9D95-FAB3D0B9FC84",
             "transition": "Color Burn", "transition_uuid": "2B855B65-6ABC-4F65-8198-A19A5EF97821"}
telemetry(PV, "transport-workspace", inbound_http={
    "path": "/api/0/transportState/workspace",
    "body": json.dumps({"data": [{"transportState": {
        **_TS, "playingMedia": {"name": "MediaName.jpg", "uuid": "7358B6A7-85A4-4038-8ED3-88A72AC106E0"}}}]})},
    expect_state={"layers": {_L1: {**_TS_STATE, "media": "MediaName.jpg",
                                   "media_uuid": "7358B6A7-85A4-4038-8ED3-88A72AC106E0"}}})
telemetry(PV, "transport-layer", inbound_http={
    "path": "/api/0/transportState/layer/0", "body": json.dumps({"transportState": _TS})},
    expect_state={"layers": {_L1: _TS_STATE}})

telemetry(PV, "workspace-transition", inbound_http={
    "path": "/api/0/transition/workspace",
    "body": '{"transition":{"variables":[],"enabled":false,"name":"Default (Dissolve)",'
            '"uuid":"EC52A828-AD85-4602-B70C-1DEE7C904DB6"}}'},
    expect_state={"workspace": {"transition": "Default (Dissolve)",
                                "transition_uuid": "EC52A828-AD85-4602-B70C-1DEE7C904DB6"}})
telemetry(PV, "workspace-transition-duration", inbound_http={
    "path": "/api/0/transitionDuration/workspace", "body": '{"transitionDuration":{"value":1.5}}'},
    expect_state={"workspace": {"transition_duration": 1.5}})
telemetry(PV, "workspace-effects-preset", inbound_http={
    "path": "/api/0/effectsPreset/workspace",
    "body": '{"effectPreset":{"name":"ColorInvert","effects":[{"variables":[],"enabled":true,'
            '"name":"Color Invert","uuid":"EDF55F11-1A1E-4BF3-9718-FA2D3731626D"}],'
            '"uuid":"3B076957-6D48-45A7-803F-6D1E06B02F54"}}'},
    expect_state={"workspace": {"effects_preset": "ColorInvert",
                                "effects_preset_uuid": "3B076957-6D48-45A7-803F-6D1E06B02F54"}})
telemetry(PV, "playlists", inbound_http={"path": "/api/0/data/playlists", "body": json.dumps({"playlist": {
    "items": [], "name": "Root", "children": [
        {"items": [{"name": "002_JB_HD-H264", "uuid": "6CD29387-44B8-4D61-94D7-6BE69359D12E"}],
         "name": "Playlist 1", "children": [], "uuid": "9EE5DFC3-7EE2-47F8-B932-765CEBBA8245"},
        {"items": [], "name": "Group A", "children": [
            {"items": [], "name": "Playlist 2", "children": [], "uuid": "2F9A1691-B659-4199-8655-AA46729D0C21"}],
         "uuid": "D62AFD1D-59B5-46FC-A33B-6F7E1AC7C5A0"},
        {"items": [], "name": "Video Input", "children": [], "uuid": "5F34B21B-76F7-4837-A337-92F13CF6F856"}],
    "uuid": "C0E4E090-F969-4B60-9526-D44EBC5C52ED"}})},
    # A deleted playlist leaves: the read replaces the playlists.
    state_before={"playlists": {"11111111-2222-3333-4444-555555555555": {"name": "Deleted"}}},
    expect_state={"playlists": {
        "9EE5DFC3-7EE2-47F8-B932-765CEBBA8245": {"name": "Playlist 1"},
        "D62AFD1D-59B5-46FC-A33B-6F7E1AC7C5A0": {"name": "Group A"},
        "5F34B21B-76F7-4837-A337-92F13CF6F856": {"name": "Video Input"}}})
