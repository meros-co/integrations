# Resolume Arena / Avenue (resolume): one vector per command, over the REST API.
# Targets are the paths of Resolume's Arena & Avenue REST API OpenAPI document
# (https://resolume.com/docs/restapi/swagger.yaml) under its server URL
# /api/v1. JSON bodies are the document's parameter schemas with only the
# changed property: {"value": ...} inside the property's path in the Layer,
# Composition, Column, Deck or LayerGroup schema. Text bodies are the
# operation's text/plain requestBody.
RS = "resolume"


def _rs(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": RS, "command": command, "input": input, "expect_request": request, **extra})


_A = "/api/v1"
_C = _A + "/composition"

# Product and composition.
_rs("get_product", {}, "GET", _A + "/product",
    http_reply={"status": 200, "body": '{"name":"Arena","major":7,"minor":23,"micro":0,"revision":1}'},
    expect_result={"ok": {"kind": "value", "value": {"name": "Arena", "major": 7, "minor": 23,
                                                     "micro": 0, "revision": 1}}})
_rs("get_composition", {}, "GET", _C)
_rs("disconnect_all", {}, "POST", _C + "/disconnect-all",
    http_reply={"status": 204}, expect_result={"ok": {"kind": "ack"}})
_rs("set_composition_master", {"level": 0.8}, "PUT", _C, '{"master":{"value":0.800}}')
_rs("set_composition_opacity", {"opacity": 0.5}, "PUT", _C, '{"video":{"opacity":{"value":0.500}}}')
_rs("set_composition_speed", {"speed": 0.25}, "PUT", _C, '{"speed":{"value":0.250}}')
_rs("set_composition_bypassed", {"bypassed": True}, "PUT", _C, '{"bypassed":{"value":true}}')
_rs("set_crossfader", {"phase": -0.5}, "PUT", _C, '{"crossfader":{"phase":{"value":-0.500}}}')
_rs("set_tempo", {"bpm": 128.0}, "PUT", _C, '{"tempocontroller":{"tempo":{"value":128.00}}}')

# Parameters by unique id.
_rs("trigger_parameter", {"parameter_id": 1865335878211}, "POST", _A + "/parameter/by-id/1865335878211/trigger",
    http_reply={"status": 404}, expect_result={"error": {"error": "device_error", "code": "404"}})
_rs("get_parameter", {"parameter_id": 1824357891293}, "GET", _A + "/parameter/by-id/1824357891293",
    http_reply={"status": 200, "body": '{"id":1824357891293,"valuetype":"ParamRange","min":0.0,"max":1.0,"value":0.5}'},
    expect_result={"ok": {"kind": "value", "value": 0.5}})
_rs("set_parameter", {"parameter_id": 1824357891293, "body": {"value": 0.5}}, "PUT",
    _A + "/parameter/by-id/1824357891293", '{"value":0.5}')
_rs("reset_parameter", {"parameter_id": 1824357891293}, "POST", _A + "/parameter/by-id/1824357891293/reset")

# Composition file and structure.
_rs("undo", {}, "POST", _C + "/action", "undo",
    http_reply={"status": 412, "body": "Nothing to undo"}, expect_result={"error": {"error": "device_error", "code": "412"}})
_rs("redo", {}, "POST", _C + "/action", "redo",
    http_reply={"status": 200, "body": "{ Insert Layer }"}, expect_result={"ok": {"kind": "ack"}})
_rs("save_composition", {}, "POST", _C + "/save")
_rs("open_composition", {"file_url": "file:///C:/Users/Resolume/Documents/file%201.avc"}, "POST", _C + "/open",
    "file:///C:/Users/Resolume/Documents/file%201.avc")
_rs("new_composition", {}, "POST", _C + "/new")
_rs("add_layer", {}, "POST", _C + "/layers/add")
_rs("add_column", {}, "POST", _C + "/columns/add")
_rs("add_deck", {}, "POST", _C + "/decks/add")

# Columns.
_rs("trigger_column", {"column": 4}, "POST", _C + "/columns/4/connect",
    http_reply={"status": 204}, expect_result={"ok": {"kind": "ack"}})
_rs("press_column", {"column": 4, "pressed": False}, "POST", _C + "/columns/4/connect", "false")
_rs("select_column", {"column": 2}, "POST", _C + "/columns/2/select")
_rs("get_column", {"column": 2}, "GET", _C + "/columns/2")
_rs("get_column_name", {"column": 2}, "GET", _C + "/columns/2",
    http_reply={"status": 200, "body": '{"id":1641549605447,"name":{"valuetype":"ParamString","value":"Intro"}}'},
    expect_result={"ok": {"kind": "value", "value": "Intro"}})
_rs("set_column_name", {"column": 2, "name": 'Drop "B"'}, "PUT", _C + "/columns/2",
    '{"name":{"value":"Drop \\"B\\""}}')

# Layers.
_rs("clear_layer", {"layer": 1}, "POST", _C + "/layers/1/clear",
    http_reply={"status": 404}, expect_result={"error": {"error": "device_error", "code": "404"}})
_rs("clear_selected_layer", {}, "POST", _C + "/layers/selected/clear")
_rs("clear_layer_clips", {"layer": 3}, "POST", _C + "/layers/3/clearclips")
_rs("select_layer", {"layer": 2}, "POST", _C + "/layers/2/select")
_rs("set_layer_opacity", {"layer": 1, "opacity": 0.25}, "PUT", _C + "/layers/1",
    '{"video":{"opacity":{"value":0.250}}}', http_reply={"status": 204}, expect_result={"ok": {"kind": "ack"}})
_rs("set_layer_master", {"layer": 2, "level": 1.0}, "PUT", _C + "/layers/2", '{"master":{"value":1.000}}')
_rs("set_layer_bypassed", {"layer": 2, "bypassed": False}, "PUT", _C + "/layers/2", '{"bypassed":{"value":false}}')
_rs("set_layer_solo", {"layer": 3}, "PUT", _C + "/layers/3", '{"solo":{"value":true}}')
_rs("set_layer_name", {"layer": 1, "name": "Background"}, "PUT", _C + "/layers/1", '{"name":{"value":"Background"}}')
_rs("get_layer", {"layer": 1}, "GET", _C + "/layers/1")
_rs("get_layer_name", {"layer": 1}, "GET", _C + "/layers/1",
    http_reply={"status": 200, "body": '{"id":1641549604807,"name":{"value":"Background"}}'},
    expect_result={"ok": {"kind": "value", "value": "Background"}})
_rs("get_active_clip", {"layer": 1}, "GET", _C + "/layers/1/clips/active")
_rs("get_active_clip_name", {"layer": 1}, "GET", _C + "/layers/1/clips/active",
    http_reply={"status": 200, "body": '{"id":1641549604745,"name":{"value":"Clouds"}}'},
    expect_result={"ok": {"kind": "value", "value": "Clouds"}})

# Clips.
_rs("trigger_clip", {"layer": 2, "clip": 3}, "POST", _C + "/layers/2/clips/3/connect",
    http_reply={"status": 204}, expect_result={"ok": {"kind": "ack"}})
_rs("press_clip", {"layer": 2, "clip": 3}, "POST", _C + "/layers/2/clips/3/connect", "true")
_rs("trigger_selected_clip", {}, "POST", _C + "/clips/selected/connect")
_rs("select_clip", {"layer": 2, "clip": 3}, "POST", _C + "/layers/2/clips/3/select")
_rs("get_clip", {"layer": 2, "clip": 3}, "GET", _C + "/layers/2/clips/3")
_rs("get_clip_name", {"layer": 2, "clip": 3}, "GET", _C + "/layers/2/clips/3",
    http_reply={"status": 200, "body": '{"id":1641549604745,"name":{"value":"Clouds"}}'},
    expect_result={"ok": {"kind": "value", "value": "Clouds"}})
_rs("get_clip_connected", {"layer": 2, "clip": 3}, "GET", _C + "/layers/2/clips/3",
    http_reply={"status": 200, "body": '{"id":1641549604745,"connected":{"valuetype":"ParamState","value":"Connected","index":3}}'},
    expect_result={"ok": {"kind": "value", "value": "Connected"}})
_rs("set_clip_name", {"layer": 2, "clip": 3, "name": "Clouds"}, "PUT", _C + "/layers/2/clips/3",
    '{"name":{"value":"Clouds"}}')
_rs("open_clip", {"layer": 1, "clip": 1, "url": "source:///video/Checkered"}, "POST", _C + "/layers/1/clips/1/open",
    "source:///video/Checkered")
_rs("clear_clip", {"layer": 1, "clip": 1}, "POST", _C + "/layers/1/clips/1/clear")

# Decks.
_rs("select_deck", {"deck": 2}, "POST", _C + "/decks/2/select")
_rs("get_deck", {"deck": 2}, "GET", _C + "/decks/2")
_rs("get_deck_name", {"deck": 2}, "GET", _C + "/decks/2",
    http_reply={"status": 200, "body": '{"id":1641549604727,"closed":false,"name":{"value":"Set 2"}}'},
    expect_result={"ok": {"kind": "value", "value": "Set 2"}})
_rs("set_deck_name", {"deck": 2, "name": "Set 2"}, "PUT", _C + "/decks/2", '{"name":{"value":"Set 2"}}')
_rs("open_deck", {"deck": 3}, "POST", _C + "/decks/3/open")
_rs("close_deck", {"deck": 3}, "POST", _C + "/decks/3/close")

# Layer groups (Arena).
_rs("clear_group", {"group": 1}, "POST", _C + "/layergroups/1/clear")
_rs("select_group", {"group": 1}, "POST", _C + "/layergroups/1/select")
_rs("trigger_group_column", {"group": 1, "column": 2}, "POST", _C + "/layergroups/1/columns/2/connect")
_rs("set_group_master", {"group": 1, "level": 0.75}, "PUT", _C + "/layergroups/1", '{"master":{"value":0.750}}')
_rs("set_group_opacity", {"group": 1, "opacity": 0.1}, "PUT", _C + "/layergroups/1",
    '{"video":{"opacity":{"value":0.100}}}')
_rs("set_group_speed", {"group": 1, "speed": 0.5}, "PUT", _C + "/layergroups/1", '{"speed":{"value":0.500}}')
_rs("set_group_bypassed", {"group": 1, "bypassed": True}, "PUT", _C + "/layergroups/1", '{"bypassed":{"value":true}}')
_rs("set_group_solo", {"group": 1, "solo": False}, "PUT", _C + "/layergroups/1", '{"solo":{"value":false}}')
_rs("set_group_name", {"group": 1, "name": "Front"}, "PUT", _C + "/layergroups/1", '{"name":{"value":"Front"}}')
_rs("get_group", {"group": 1}, "GET", _C + "/layergroups/1")

# Telemetry: the ProductInfo and Composition schemas, cut to the properties
# the rules read. The document lists no option names for ChoiceParameters, so
# "Fade", "Linear", "Playing" and "A" are placeholders carried through as text.
telemetry(RS, "product", inbound_http={
    "path": "/api/v1/product",
    "body": '{"name":"Avenue","major":7,"minor":8,"micro":0,"revision":12345}'},
    expect_state={"product": {"name": "Avenue", "major": 7, "minor": 8, "micro": 0, "revision": 12345}})

telemetry(RS, "composition", inbound_http={"path": "/api/v1/composition", "body": json.dumps({
    "name": {"value": "Main Show"},
    "bypassed": {"valuetype": "ParamBoolean", "value": False},
    "master": {"valuetype": "ParamRange", "min": 0.0, "max": 1.0, "value": 1.0},
    "speed": {"valuetype": "ParamRange", "value": 0.5},
    "video": {"opacity": {"valuetype": "ParamRange", "value": 0.75}},
    "crossfader": {"id": 1, "phase": {"value": 0.0}, "behaviour": {"value": "Fade"},
                   "curve": {"value": "Linear"}, "sidea": {"id": 2001, "valuetype": "ParamEvent"},
                   "sideb": {"id": 2002, "valuetype": "ParamEvent"}},
    "tempocontroller": {"tempo": {"value": 128.0}, "play_state": {"value": "Playing"},
                        "tempo_pull": {"id": 3001}, "tempo_push": {"id": 3002},
                        "tempo_tap": {"id": 3003}, "resync": {"id": 3004}},
    "decks": [{"id": 1641549604727, "closed": False, "name": {"value": "Set 1"}, "selected": {"value": True}},
              {"id": 1641549604728, "closed": True, "name": {"value": "Set 2"}, "selected": {"value": False}}],
    "layers": [{"id": 1641549604807, "name": {"value": "Background"}, "selected": {"value": True},
                "bypassed": {"value": False}, "solo": {"value": False}, "master": {"value": 1.0},
                "crossfadergroup": {"value": "A", "index": 1},
                "video": {"opacity": {"value": 0.25}},
                "active_clip": {"id": 1641549604745, "name": {"value": "Clouds"}}}],
    "columns": [{"id": 1641549605447, "name": {"value": "Intro"}, "connected": {"value": "Connected"},
                 "selected": {"value": False}}],
    "layergroups": [{"id": 1641549604808, "name": {"value": "Front"}, "selected": {"value": False},
                     "bypassed": {"value": True}, "solo": {"value": False}, "master": {"value": 0.5},
                     "video": {"opacity": {"value": 1.0}}}]})},
    expect_state={
        "composition": {"name": "Main Show", "master": 1.0, "opacity": 0.75, "speed": 0.5, "bypassed": False},
        "crossfader": {"phase": 0.0, "behaviour": "Fade", "curve": "Linear",
                       "side_a_parameter": 2001, "side_b_parameter": 2002},
        "tempo": {"bpm": 128.0, "play_state": "Playing", "tap_parameter": 3003, "resync_parameter": 3004,
                  "push_parameter": 3002, "pull_parameter": 3001},
        "layers": {"1641549604807": {"name": "Background", "opacity": 0.25, "master": 1.0, "bypassed": False,
                                     "solo": False, "selected": True, "crossfader_group": "A",
                                     "active_clip": 1641549604745, "active_clip_name": "Clouds"}},
        "columns": {"1641549605447": {"name": "Intro", "connected": "Connected", "selected": False}},
        "decks": {"1641549604727": {"name": "Set 1", "selected": True, "closed": False},
                  "1641549604728": {"name": "Set 2", "selected": False, "closed": True}},
        "groups": {"1641549604808": {"name": "Front", "opacity": 1.0, "master": 0.5, "bypassed": True,
                                     "solo": False, "selected": False}}})

# A layer with nothing playing: active_clip is null and is not assigned.
telemetry(RS, "layer-idle", inbound_http={"path": "/api/v1/composition", "body": json.dumps({
    "layers": [{"id": 1641549604810, "name": {"value": "Overlay"}, "selected": {"value": False},
                "bypassed": {"value": False}, "solo": {"value": True}, "master": {"value": 0.5},
                "video": {"opacity": {"value": 1.0}}, "active_clip": None}]})},
    expect_state={"layers": {"1641549604810": {"name": "Overlay", "opacity": 1.0, "master": 0.5,
                                               "bypassed": False, "solo": True, "selected": False}}})

# Websocket API (support article v7.8): subscriptions sent when the websocket
# opens, as {"action": "subscribe", "parameter": <logical path>}, and the
# parameter Resolume sends back with 'type' and 'path' added.
_RS_SUBSCRIBED = ["/composition/master", "/composition/video/opacity", "/composition/speed",
                  "/composition/bypassed", "/composition/crossfader/phase",
                  "/composition/tempocontroller/tempo"]
telemetry(RS, "ws-master", expect_connect_ws=[
    json.dumps({"action": "subscribe", "parameter": p}, separators=(",", ":")) for p in _RS_SUBSCRIBED],
    inbound_ws=json.dumps({"type": "parameter_update", "path": "/composition/master", "id": 1650000000001,
                           "valuetype": "ParamRange", "min": 0.0, "max": 1.0, "value": 0.5}),
    expect_state={"composition": {"master": 0.5}})
telemetry(RS, "ws-tempo-subscribed", inbound_ws=json.dumps({
    "type": "parameter_subscribed", "path": "/composition/tempocontroller/tempo", "id": 1650000000002,
    "valuetype": "ParamRange", "min": 20.0, "max": 500.0, "value": 128.0}),
    expect_state={"tempo": {"bpm": 128.0}})
telemetry(RS, "ws-bypassed", inbound_ws=json.dumps({
    "type": "parameter_update", "path": "/composition/bypassed", "id": 1650000000003,
    "valuetype": "ParamBoolean", "value": True}),
    expect_state={"composition": {"bypassed": True}})
telemetry(RS, "ws-sources-ignored", inbound_ws=json.dumps({"type": "sources_update", "value": {}}),
          expect_state={})
