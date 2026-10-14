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

# Full control: effect chains, structure, by-id forms, transport and settings. Effect
# URLs are the document's effect:///<chain>/<name>[/<preset>] form; a moved effect
# is named as in the document's effect-by-id example, effects/by-id/<id>.
_rs('add_effect_to_composition', {'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_composition_at', {'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_composition', {'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_composition_at', {'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_composition_effect', {'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/effects/video/2')
_rs('rename_composition_effect', {'kind': 'video', 'offset': 2, 'display_name': 'Big Blow'}, 'POST', '/api/v1/composition/effects/video/2/set-display-name', 'Big Blow')
_rs('copy_effects_to_composition', {'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_layer', {'layer': 2, 'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/2/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_layer_at', {'layer': 2, 'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/2/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_layer', {'layer': 2, 'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layers/2/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_layer_at', {'layer': 2, 'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layers/2/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_layer_effect', {'layer': 2, 'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layers/2/effects/video/2')
_rs('rename_layer_effect', {'layer': 2, 'kind': 'video', 'offset': 2, 'display_name': 'Big Blow'}, 'POST', '/api/v1/composition/layers/2/effects/video/2/set-display-name', 'Big Blow')
_rs('copy_effects_to_layer', {'layer': 2, 'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layers/2/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_layer_by_id', {'layer_id': 1658311521181, 'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_layer_at_by_id', {'layer_id': 1658311521181, 'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_layer_by_id', {'layer_id': 1658311521181, 'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_layer_at_by_id', {'layer_id': 1658311521181, 'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_layer_effect_by_id', {'layer_id': 1658311521181, 'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layers/by-id/1658311521181/effects/video/2')
_rs('copy_effects_to_layer_by_id', {'layer_id': 1658311521181, 'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_selected_layer', {'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/selected/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_selected_layer_at', {'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/selected/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('delete_selected_layer_effect', {'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layers/selected/effects/video/2')
_rs('copy_effects_to_selected_layer', {'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layers/selected/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_group', {'group': 1, 'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layergroups/1/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_group_at', {'group': 1, 'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layergroups/1/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_group', {'group': 1, 'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layergroups/1/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_group_at', {'group': 1, 'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layergroups/1/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_group_effect', {'group': 1, 'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layergroups/1/effects/video/2')
_rs('rename_group_effect', {'group': 1, 'kind': 'video', 'offset': 2, 'display_name': 'Big Blow'}, 'POST', '/api/v1/composition/layergroups/1/effects/video/2/set-display-name', 'Big Blow')
_rs('copy_effects_to_group', {'group': 1, 'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layergroups/1/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_group_by_id', {'group_id': 1658311522986, 'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_group_at_by_id', {'group_id': 1658311522986, 'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_group_by_id', {'group_id': 1658311522986, 'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_group_at_by_id', {'group_id': 1658311522986, 'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_group_effect_by_id', {'group_id': 1658311522986, 'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layergroups/by-id/1658311522986/effects/video/2')
_rs('copy_effects_to_group_by_id', {'group_id': 1658311522986, 'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_selected_group', {'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layergroups/selected/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_selected_group_at', {'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layergroups/selected/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('delete_selected_group_effect', {'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layergroups/selected/effects/video/2')
_rs('copy_effects_to_selected_group', {'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layergroups/selected/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_clip', {'layer': 2, 'clip': 3, 'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/2/clips/3/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_clip_at', {'layer': 2, 'clip': 3, 'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/layers/2/clips/3/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_clip', {'layer': 2, 'clip': 3, 'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layers/2/clips/3/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_clip_at', {'layer': 2, 'clip': 3, 'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/layers/2/clips/3/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_clip_effect', {'layer': 2, 'clip': 3, 'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/layers/2/clips/3/effects/video/2')
_rs('rename_clip_effect', {'layer': 2, 'clip': 3, 'kind': 'video', 'offset': 2, 'display_name': 'Big Blow'}, 'POST', '/api/v1/composition/layers/2/clips/3/effects/video/2/set-display-name', 'Big Blow')
_rs('copy_effects_to_clip', {'layer': 2, 'clip': 3, 'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/layers/2/clips/3/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_clip_by_id', {'clip_id': 1658311744128, 'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_clip_at_by_id', {'clip_id': 1658311744128, 'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('move_effect_to_clip_by_id', {'clip_id': 1658311744128, 'kind': 'video', 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/effects/video/move', 'effects/by-id/1658311809911')
_rs('move_effect_to_clip_at_by_id', {'clip_id': 1658311744128, 'kind': 'video', 'offset': 2, 'effect_id': 1658311809911}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/effects/video/move/2', 'effects/by-id/1658311809911')
_rs('delete_clip_effect_by_id', {'clip_id': 1658311744128, 'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/clips/by-id/1658311744128/effects/video/2')
_rs('copy_effects_to_clip_by_id', {'clip_id': 1658311744128, 'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/copy-effects', '/composition/layers/1/clips/1')
_rs('add_effect_to_selected_clip', {'kind': 'video', 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/clips/selected/effects/video/add', 'effect:///video/Blow/Solid')
_rs('add_effect_to_selected_clip_at', {'kind': 'video', 'offset': 2, 'effect': 'Blow/Solid'}, 'POST', '/api/v1/composition/clips/selected/effects/video/add/2', 'effect:///video/Blow/Solid')
_rs('delete_selected_clip_effect', {'kind': 'video', 'offset': 2}, 'DELETE', '/api/v1/composition/clips/selected/effects/video/2')
_rs('copy_effects_to_selected_clip', {'source': '/composition/layers/1/clips/1'}, 'POST', '/api/v1/composition/clips/selected/copy-effects', '/composition/layers/1/clips/1')
_rs('rename_effect', {'effect_id': 1658311809911, 'display_name': 'Big Blow'}, 'POST', '/api/v1/composition/effects/by-id/1658311809911/set-display-name', 'Big Blow')
_rs('get_effect', {'effect_id': 1658311809911}, 'GET', '/api/v1/composition/effects/by-id/1658311809911')
_rs('set_effect_bypassed', {'effect_id': 1658311809911, 'bypassed': True}, 'PUT', '/api/v1/composition/effects/by-id/1658311809911', '{"bypassed":{"value":true}}')
_rs('set_effect', {'effect_id': 1658311809911, 'body': {'bypassed': {'value': True}}}, 'PUT', '/api/v1/composition/effects/by-id/1658311809911', '{"bypassed":{"value":true}}')
_rs('get_sources', {}, 'GET', '/api/v1/sources')
_rs('get_effects', {}, 'GET', '/api/v1/effects')
_rs('get_audio_effects', {}, 'GET', '/api/v1/effects/audio')
_rs('get_file_info', {'file': 'file:///C:/Media/Clip%201.mov'}, 'POST', '/api/v1/file-info', '["file:///C:/Media/Clip%201.mov"]')
_rs('get_monitors', {}, 'GET', '/api/v1/composition/monitors')
_rs('reset_parameter_animation', {'parameter_id': 1824357891293}, 'POST', '/api/v1/parameter/by-id/1824357891293/reset', '{"resetanimation":true}')
_rs('get_phase_source', {'parameter_id': 1824357891293}, 'GET', '/api/v1/parameter/by-id/1824357891293/phase-source')
_rs('set_phase_source', {'parameter_id': 1824357891293, 'phase_source': 'bpm_sync'}, 'PUT', '/api/v1/parameter/by-id/1824357891293/phase-source', '{"phase_source":"bpm_sync"}')
_rs('set_phase_source_dashboard', {'parameter_id': 1824357891293, 'dial': 3}, 'PUT', '/api/v1/parameter/by-id/1824357891293/phase-source', '{"phase_source":"dashboard","dial":3}')
_rs('clear_phase_source', {'parameter_id': 1824357891293}, 'DELETE', '/api/v1/parameter/by-id/1824357891293/phase-source')
_rs('save_composition_as', {'file_url': 'file:///C:/Shows/Main%20Show.avc'}, 'POST', '/api/v1/composition/save', 'file:///C:/Shows/Main%20Show.avc')
_rs('grow_composition', {'column_count': 15, 'layer_count': 8}, 'POST', '/api/v1/composition/grow-to', '{"column_count":15,"layer_count":8}')
_rs('set_composition_name', {'name': 'Main'}, 'PUT', '/api/v1/composition', '{"name":{"value":"Main"}}')
_rs('set_composition_description', {'description': 'Opening night'}, 'PUT', '/api/v1/composition', '{"description":{"value":"Opening night"}}')
_rs('set_composition_volume', {'volume': -6.0}, 'PUT', '/api/v1/composition', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_composition_pan', {'pan': 0.25}, 'PUT', '/api/v1/composition', '{"audio":{"pan":{"value":0.250}}}')
_rs('set_composition_high_detail', {'flag': True}, 'PUT', '/api/v1/composition', '{"highdetail":{"value":true}}')
_rs('set_composition_frame_rate', {'rate': 50.0}, 'PUT', '/api/v1/composition', '{"framerate":{"value":50.000}}')
_rs('set_composition_clip_target', {'option': 'Forward'}, 'PUT', '/api/v1/composition', '{"cliptarget":{"value":"Forward"}}')
_rs('set_composition_clip_trigger_style', {'option': 'Forward'}, 'PUT', '/api/v1/composition', '{"cliptriggerstyle":{"value":"Forward"}}')
_rs('set_composition_clip_beat_snap', {'option': 'Forward'}, 'PUT', '/api/v1/composition', '{"clipbeatsnap":{"value":"Forward"}}')
_rs('set_crossfader_behaviour', {'option': 'Forward'}, 'PUT', '/api/v1/composition', '{"crossfader":{"behaviour":{"value":"Forward"}}}')
_rs('set_crossfader_curve', {'option': 'Forward'}, 'PUT', '/api/v1/composition', '{"crossfader":{"curve":{"value":"Forward"}}}')
_rs('set_tempo_play_state', {'option': 'Forward'}, 'PUT', '/api/v1/composition', '{"tempocontroller":{"play_state":{"value":"Forward"}}}')
_rs('delete_column', {'column': 4}, 'DELETE', '/api/v1/composition/columns/4')
_rs('duplicate_column', {'column': 4}, 'POST', '/api/v1/composition/columns/4/duplicate')
_rs('insert_column_copy', {'column': 4, 'source_column': 1}, 'POST', '/api/v1/composition/columns/4/insert-copy', '/composition/columns/1')
_rs('insert_column_move', {'column': 4, 'source_column': 1}, 'POST', '/api/v1/composition/columns/4/insert-move', '/composition/columns/1')
_rs('add_column_before', {'column': 4}, 'POST', '/api/v1/composition/columns/add', '/composition/columns/4')
_rs('swap_columns', {'column': 4, 'other_column': 1}, 'POST', '/api/v1/composition/columns/swap', '["/composition/columns/4","/composition/columns/1"]')
_rs('delete_column_by_id', {'column_id': 1658311520090}, 'DELETE', '/api/v1/composition/columns/by-id/1658311520090')
_rs('duplicate_column_by_id', {'column_id': 1658311520090}, 'POST', '/api/v1/composition/columns/by-id/1658311520090/duplicate')
_rs('insert_column_copy_by_id', {'column_id': 1658311520090, 'source_column_id': 1658311520091}, 'POST', '/api/v1/composition/columns/by-id/1658311520090/insert-copy', '/composition/columns/by-id/1658311520091')
_rs('insert_column_move_by_id', {'column_id': 1658311520090, 'source_column_id': 1658311520091}, 'POST', '/api/v1/composition/columns/by-id/1658311520090/insert-move', '/composition/columns/by-id/1658311520091')
_rs('add_column_before_by_id', {'column_id': 1658311520090}, 'POST', '/api/v1/composition/columns/add', '/composition/columns/by-id/1658311520090')
_rs('swap_columns_by_id', {'column_id': 1658311520090, 'other_column_id': 1658311520091}, 'POST', '/api/v1/composition/columns/swap', '["/composition/columns/by-id/1658311520090","/composition/columns/by-id/1658311520091"]')
_rs('insert_column_copy_at_selected', {'source_column': 1}, 'POST', '/api/v1/composition/columns/selected/insert-copy', '/composition/columns/1')
_rs('insert_column_move_at_selected', {'source_column': 1}, 'POST', '/api/v1/composition/columns/selected/insert-move', '/composition/columns/1')
_rs('trigger_column_by_id', {'column_id': 1658311520090}, 'POST', '/api/v1/composition/columns/by-id/1658311520090/connect')
_rs('press_column_by_id', {'column_id': 1658311520090, 'pressed': False}, 'POST', '/api/v1/composition/columns/by-id/1658311520090/connect', 'false')
_rs('select_column_by_id', {'column_id': 1658311520090}, 'POST', '/api/v1/composition/columns/by-id/1658311520090/select')
_rs('get_column_by_id', {'column_id': 1658311520090}, 'GET', '/api/v1/composition/columns/by-id/1658311520090')
_rs('set_column_name_by_id', {'column_id': 1658311520090, 'name': 'Main'}, 'PUT', '/api/v1/composition/columns/by-id/1658311520090', '{"name":{"value":"Main"}}')
_rs('set_column_color_by_id', {'column_id': 1658311520090, 'color': 'Red'}, 'PUT', '/api/v1/composition/columns/by-id/1658311520090', '{"colorid":{"value":"Red"}}')
_rs('set_column_beat_snap_by_id', {'column_id': 1658311520090, 'option': 'Forward'}, 'PUT', '/api/v1/composition/columns/by-id/1658311520090', '{"beatsnap":{"value":"Forward"}}')
_rs('set_layer_opacity_by_id', {'layer_id': 1658311521181, 'opacity': 0.5}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"video":{"opacity":{"value":0.500}}}')
_rs('set_layer_master_by_id', {'layer_id': 1658311521181, 'level': 0.75}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"master":{"value":0.750}}')
_rs('set_layer_bypassed_by_id', {'layer_id': 1658311521181, 'bypassed': True}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"bypassed":{"value":true}}')
_rs('set_layer_solo_by_id', {'layer_id': 1658311521181, 'solo': True}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"solo":{"value":true}}')
_rs('set_layer_name_by_id', {'layer_id': 1658311521181, 'name': 'Main'}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"name":{"value":"Main"}}')
_rs('set_layer_crossfader_group', {'layer': 2, 'crossfader_group': 'B'}, 'PUT', '/api/v1/composition/layers/2', '{"crossfadergroup":{"value":"B"}}')
_rs('set_layer_crossfader_group_by_id', {'layer_id': 1658311521181, 'crossfader_group': 'B'}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"crossfadergroup":{"value":"B"}}')
_rs('set_layer_volume', {'layer': 2, 'volume': -6.0}, 'PUT', '/api/v1/composition/layers/2', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_layer_volume_by_id', {'layer_id': 1658311521181, 'volume': -6.0}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_layer_pan', {'layer': 2, 'pan': 0.25}, 'PUT', '/api/v1/composition/layers/2', '{"audio":{"pan":{"value":0.250}}}')
_rs('set_layer_pan_by_id', {'layer_id': 1658311521181, 'pan': 0.25}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"audio":{"pan":{"value":0.250}}}')
_rs('set_layer_transition_duration', {'layer': 2, 'duration': 1.5}, 'PUT', '/api/v1/composition/layers/2', '{"transition":{"duration":{"value":1.500}}}')
_rs('set_layer_transition_duration_by_id', {'layer_id': 1658311521181, 'duration': 1.5}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"transition":{"duration":{"value":1.500}}}')
_rs('set_layer_transition_blend_mode', {'layer': 2, 'blend_mode': 'Alpha'}, 'PUT', '/api/v1/composition/layers/2', '{"transition":{"blend_mode":{"value":"Alpha"}}}')
_rs('set_layer_transition_blend_mode_by_id', {'layer_id': 1658311521181, 'blend_mode': 'Alpha'}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"transition":{"blend_mode":{"value":"Alpha"}}}')
_rs('set_layer_mask_mode', {'layer': 2, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2', '{"maskmode":{"value":"Forward"}}')
_rs('set_layer_mask_mode_by_id', {'layer_id': 1658311521181, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"maskmode":{"value":"Forward"}}')
_rs('set_layer_ignore_column_trigger', {'layer': 2, 'flag': True}, 'PUT', '/api/v1/composition/layers/2', '{"ignorecolumntrigger":{"value":true}}')
_rs('set_layer_ignore_column_trigger_by_id', {'layer_id': 1658311521181, 'flag': True}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"ignorecolumntrigger":{"value":true}}')
_rs('set_layer_fader_start', {'layer': 2, 'flag': True}, 'PUT', '/api/v1/composition/layers/2', '{"faderstart":{"value":true}}')
_rs('set_layer_fader_start_by_id', {'layer_id': 1658311521181, 'flag': True}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"faderstart":{"value":true}}')
_rs('set_layer_color', {'layer': 2, 'color': 'Red'}, 'PUT', '/api/v1/composition/layers/2', '{"colorid":{"value":"Red"}}')
_rs('set_layer_color_by_id', {'layer_id': 1658311521181, 'color': 'Red'}, 'PUT', '/api/v1/composition/layers/by-id/1658311521181', '{"colorid":{"value":"Red"}}')
_rs('delete_layer', {'layer': 2}, 'DELETE', '/api/v1/composition/layers/2')
_rs('delete_layer_by_id', {'layer_id': 1658311521181}, 'DELETE', '/api/v1/composition/layers/by-id/1658311521181')
_rs('duplicate_layer', {'layer': 2}, 'POST', '/api/v1/composition/layers/2/duplicate')
_rs('duplicate_layer_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/duplicate')
_rs('duplicate_selected_layer', {}, 'POST', '/api/v1/composition/layers/selected/duplicate')
_rs('move_layer', {'layer': 2, 'before_layer': 5}, 'POST', '/api/v1/composition/layers/2/move', '/composition/layers/5')
_rs('move_layer_by_id', {'layer_id': 1658311521181, 'before_layer_id': 1658311520090}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/move', '/composition/layers/by-id/1658311520090')
_rs('move_layer_to_end', {'layer': 2}, 'POST', '/api/v1/composition/layers/2/move')
_rs('move_layer_to_end_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/move')
_rs('move_layer_into_group', {'group': 1, 'layer': 2}, 'POST', '/api/v1/composition/layergroups/1/move-layer', '/composition/layers/2')
_rs('move_layer_into_group_by_id', {'group_id': 1658311522986, 'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/move-layer', '/composition/layers/by-id/1658311521181')
_rs('remove_layer_from_group', {'layer': 2}, 'POST', '/api/v1/composition/layers/2/remove-from-group')
_rs('remove_layer_from_group_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/remove-from-group')
_rs('add_layer_before', {'layer': 2}, 'POST', '/api/v1/composition/layers/add', '/composition/layers/2')
_rs('add_layer_before_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/add', '/composition/layers/by-id/1658311521181')
_rs('select_layer_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/select')
_rs('clear_layer_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/clear')
_rs('clear_layer_clips_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/clearclips')
_rs('clear_selected_layer_clips', {}, 'POST', '/api/v1/composition/layers/selected/clearclips')
_rs('get_layer_by_id', {'layer_id': 1658311521181}, 'GET', '/api/v1/composition/layers/by-id/1658311521181')
_rs('get_active_clip_by_id', {'layer_id': 1658311521181}, 'GET', '/api/v1/composition/layers/by-id/1658311521181/clips/active')
_rs('add_group', {}, 'POST', '/api/v1/composition/layergroups/add')
_rs('add_group_with_layer', {'layer': 2}, 'POST', '/api/v1/composition/layergroups/add', '/composition/layers/2')
_rs('add_group_with_layer_by_id', {'layer_id': 1658311521181}, 'POST', '/api/v1/composition/layergroups/add', '/composition/layers/by-id/1658311521181')
_rs('add_group_before', {'group': 1}, 'POST', '/api/v1/composition/layergroups/add', '/composition/layergroups/1')
_rs('delete_group', {'group': 1}, 'DELETE', '/api/v1/composition/layergroups/1')
_rs('delete_group_by_id', {'group_id': 1658311522986}, 'DELETE', '/api/v1/composition/layergroups/by-id/1658311522986')
_rs('delete_selected_group', {}, 'DELETE', '/api/v1/composition/layergroups/selected')
_rs('duplicate_group', {'group': 1}, 'POST', '/api/v1/composition/layergroups/1/duplicate')
_rs('duplicate_group_by_id', {'group_id': 1658311522986}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/duplicate')
_rs('duplicate_selected_group', {}, 'POST', '/api/v1/composition/layergroups/selected/duplicate')
_rs('move_group', {'group': 1, 'before_group': 2}, 'POST', '/api/v1/composition/layergroups/1/move', '/composition/layergroups/2')
_rs('move_group_before_layer', {'group': 1, 'layer': 2}, 'POST', '/api/v1/composition/layergroups/1/move', '/composition/layers/2')
_rs('move_group_to_end', {'group': 1}, 'POST', '/api/v1/composition/layergroups/1/move')
_rs('move_group_by_id', {'group_id': 1658311522986, 'before_group_id': 1658387238991}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/move', '/composition/layergroups/by-id/1658387238991')
_rs('move_group_to_end_by_id', {'group_id': 1658311522986}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/move')
_rs('add_layer_to_group', {'group': 1}, 'POST', '/api/v1/composition/layergroups/1/add-layer')
_rs('add_layer_to_group_before', {'group': 1, 'layer': 2}, 'POST', '/api/v1/composition/layergroups/1/add-layer', '/composition/layers/2')
_rs('add_layer_to_group_by_id', {'group_id': 1658311522986}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/add-layer')
_rs('press_group_clear', {'group': 1, 'pressed': False}, 'POST', '/api/v1/composition/layergroups/1/clear', 'false')
_rs('clear_group_by_id', {'group_id': 1658311522986}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/clear')
_rs('clear_selected_group', {}, 'POST', '/api/v1/composition/layergroups/selected/clear')
_rs('select_group_by_id', {'group_id': 1658311522986}, 'POST', '/api/v1/composition/layergroups/by-id/1658311522986/select')
_rs('get_group_by_id', {'group_id': 1658311522986}, 'GET', '/api/v1/composition/layergroups/by-id/1658311522986')
_rs('press_group_column', {'group': 1, 'column': 4, 'pressed': False}, 'POST', '/api/v1/composition/layergroups/1/columns/4/connect', 'false')
_rs('select_group_column', {'group': 1, 'column': 4}, 'POST', '/api/v1/composition/layergroups/1/columns/4/select')
_rs('get_group_column', {'group': 1, 'column': 4}, 'GET', '/api/v1/composition/layergroups/1/columns/4')
_rs('set_group_column_name', {'group': 1, 'column': 4, 'name': 'Main'}, 'PUT', '/api/v1/composition/layergroups/1/columns/4', '{"name":{"value":"Main"}}')
_rs('set_group_master_by_id', {'group_id': 1658311522986, 'level': 0.75}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"master":{"value":0.750}}')
_rs('set_group_opacity_by_id', {'group_id': 1658311522986, 'opacity': 0.5}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"video":{"opacity":{"value":0.500}}}')
_rs('set_group_speed_by_id', {'group_id': 1658311522986, 'speed': 1.5}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"speed":{"value":1.500}}')
_rs('set_group_bypassed_by_id', {'group_id': 1658311522986, 'bypassed': True}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"bypassed":{"value":true}}')
_rs('set_group_solo_by_id', {'group_id': 1658311522986, 'solo': True}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"solo":{"value":true}}')
_rs('set_group_name_by_id', {'group_id': 1658311522986, 'name': 'Main'}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"name":{"value":"Main"}}')
_rs('set_group_crossfader_group', {'group': 1, 'crossfader_group': 'B'}, 'PUT', '/api/v1/composition/layergroups/1', '{"crossfadergroup":{"value":"B"}}')
_rs('set_group_crossfader_group_by_id', {'group_id': 1658311522986, 'crossfader_group': 'B'}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"crossfadergroup":{"value":"B"}}')
_rs('set_group_volume', {'group': 1, 'volume': -6.0}, 'PUT', '/api/v1/composition/layergroups/1', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_group_volume_by_id', {'group_id': 1658311522986, 'volume': -6.0}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_group_pan', {'group': 1, 'pan': 0.25}, 'PUT', '/api/v1/composition/layergroups/1', '{"audio":{"pan":{"value":0.250}}}')
_rs('set_group_pan_by_id', {'group_id': 1658311522986, 'pan': 0.25}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"audio":{"pan":{"value":0.250}}}')
_rs('set_group_ignore_column_trigger', {'group': 1, 'flag': True}, 'PUT', '/api/v1/composition/layergroups/1', '{"ignorecolumntrigger":{"value":true}}')
_rs('set_group_ignore_column_trigger_by_id', {'group_id': 1658311522986, 'flag': True}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"ignorecolumntrigger":{"value":true}}')
_rs('set_group_color', {'group': 1, 'color': 'Red'}, 'PUT', '/api/v1/composition/layergroups/1', '{"colorid":{"value":"Red"}}')
_rs('set_group_color_by_id', {'group_id': 1658311522986, 'color': 'Red'}, 'PUT', '/api/v1/composition/layergroups/by-id/1658311522986', '{"colorid":{"value":"Red"}}')
_rs('delete_deck', {'deck': 2}, 'DELETE', '/api/v1/composition/decks/2')
_rs('delete_deck_by_id', {'deck_id': 1681349832470}, 'DELETE', '/api/v1/composition/decks/by-id/1681349832470')
_rs('duplicate_deck', {'deck': 2}, 'POST', '/api/v1/composition/decks/2/duplicate')
_rs('duplicate_deck_by_id', {'deck_id': 1681349832470}, 'POST', '/api/v1/composition/decks/by-id/1681349832470/duplicate')
_rs('add_deck_before', {'deck': 2}, 'POST', '/api/v1/composition/decks/add', '/composition/decks/2')
_rs('add_deck_before_by_id', {'deck_id': 1681349832470}, 'POST', '/api/v1/composition/decks/add', '/composition/decks/by-id/1681349832470')
_rs('select_deck_by_id', {'deck_id': 1681349832470}, 'POST', '/api/v1/composition/decks/by-id/1681349832470/select')
_rs('open_deck_by_id', {'deck_id': 1681349832470}, 'POST', '/api/v1/composition/decks/by-id/1681349832470/open')
_rs('close_deck_by_id', {'deck_id': 1681349832470}, 'POST', '/api/v1/composition/decks/by-id/1681349832470/close')
_rs('get_deck_by_id', {'deck_id': 1681349832470}, 'GET', '/api/v1/composition/decks/by-id/1681349832470')
_rs('set_deck_name_by_id', {'deck_id': 1681349832470, 'name': 'Main'}, 'PUT', '/api/v1/composition/decks/by-id/1681349832470', '{"name":{"value":"Main"}}')
_rs('set_deck_color_by_id', {'deck_id': 1681349832470, 'color': 'Red'}, 'PUT', '/api/v1/composition/decks/by-id/1681349832470', '{"colorid":{"value":"Red"}}')
_rs('trigger_clip_by_id', {'clip_id': 1658311744128}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/connect')
_rs('press_clip_by_id', {'clip_id': 1658311744128, 'pressed': False}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/connect', 'false')
_rs('press_selected_clip', {'pressed': False}, 'POST', '/api/v1/composition/clips/selected/connect', 'false')
_rs('select_clip_by_id', {'clip_id': 1658311744128}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/select')
_rs('get_clip_by_id', {'clip_id': 1658311744128}, 'GET', '/api/v1/composition/clips/by-id/1658311744128')
_rs('get_selected_clip', {}, 'GET', '/api/v1/composition/clips/selected')
_rs('open_clip_by_id', {'clip_id': 1658311744128, 'url': 'source:///video/Checkered'}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/open', 'source:///video/Checkered')
_rs('open_selected_clip', {'url': 'source:///video/Checkered'}, 'POST', '/api/v1/composition/clips/selected/open', 'source:///video/Checkered')
_rs('clear_clip_by_id', {'clip_id': 1658311744128}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/clear')
_rs('clear_selected_clip', {}, 'POST', '/api/v1/composition/clips/selected/clear')
_rs('remove_clip_audio_track', {'layer': 2, 'clip': 3}, 'POST', '/api/v1/composition/layers/2/clips/3/remove-audio-track')
_rs('remove_clip_video_track', {'layer': 2, 'clip': 3}, 'POST', '/api/v1/composition/layers/2/clips/3/remove-video-track')
_rs('insert_into_clip', {'layer': 2, 'clip': 3, 'url': 'source:///video/Checkered'}, 'POST', '/api/v1/composition/layers/2/clips/3/insert', '["source:///video/Checkered"]')
_rs('remove_clip_audio_track_by_id', {'clip_id': 1658311744128}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/remove-audio-track')
_rs('remove_clip_video_track_by_id', {'clip_id': 1658311744128}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/remove-video-track')
_rs('insert_into_clip_by_id', {'clip_id': 1658311744128, 'url': 'source:///video/Checkered'}, 'POST', '/api/v1/composition/clips/by-id/1658311744128/insert', '["source:///video/Checkered"]')
_rs('merge_clip', {'layer': 2, 'clip': 3, 'source_layer': 5, 'source_clip': 1, 'clear_source': True}, 'POST', '/api/v1/composition/layers/2/clips/3/merge', '{"source":"/composition/layers/5/clips/1","clear_source":true}')
_rs('merge_clip_by_id', {'layer_id': 1658311521181, 'clip_id': 1658311744128, 'source_clip_id': 1658311744129, 'clear_source': True}, 'POST', '/api/v1/composition/layers/by-id/1658311521181/clips/by-id/1658311744128/merge', '{"source":"/composition/clips/by-id/1658311744129","clear_source":true}')
_rs('swap_clips', {'layer': 2, 'clip': 3, 'other_layer': 5, 'other_clip': 1}, 'POST', '/api/v1/composition/clips/swap', '["/composition/layers/2/clips/3","/composition/layers/5/clips/1"]')
_rs('swap_clips_by_id', {'clip_id': 1658311744128, 'other_clip_id': 1658311744129}, 'POST', '/api/v1/composition/clips/swap', '["/composition/clips/by-id/1658311744128","/composition/clips/by-id/1658311744129"]')
_rs('set_clip_name_by_id', {'clip_id': 1658311744128, 'name': 'Main'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"name":{"value":"Main"}}')
_rs('set_clip_speed', {'layer': 2, 'clip': 3, 'speed': 1.5}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"transport":{"controls":{"speed":{"value":1.500}}}}')
_rs('set_clip_speed_by_id', {'clip_id': 1658311744128, 'speed': 1.5}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"transport":{"controls":{"speed":{"value":1.500}}}}')
_rs('set_clip_position', {'layer': 2, 'clip': 3, 'position': 1250.0}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"transport":{"position":{"value":1250.000}}}')
_rs('set_clip_position_by_id', {'clip_id': 1658311744128, 'position': 1250.0}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"transport":{"position":{"value":1250.000}}}')
_rs('set_clip_play_direction', {'layer': 2, 'clip': 3, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"transport":{"controls":{"playdirection":{"value":"Forward"}}}}')
_rs('set_clip_play_direction_by_id', {'clip_id': 1658311744128, 'option': 'Forward'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"transport":{"controls":{"playdirection":{"value":"Forward"}}}}')
_rs('set_clip_play_mode', {'layer': 2, 'clip': 3, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"transport":{"controls":{"playmode":{"value":"Forward"}}}}')
_rs('set_clip_play_mode_by_id', {'clip_id': 1658311744128, 'option': 'Forward'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"transport":{"controls":{"playmode":{"value":"Forward"}}}}')
_rs('set_clip_transport_type', {'layer': 2, 'clip': 3, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"transporttype":{"value":"Forward"}}')
_rs('set_clip_transport_type_by_id', {'clip_id': 1658311744128, 'option': 'Forward'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"transporttype":{"value":"Forward"}}')
_rs('set_clip_volume', {'layer': 2, 'clip': 3, 'volume': -6.0}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_clip_volume_by_id', {'clip_id': 1658311744128, 'volume': -6.0}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"audio":{"volume":{"value":-6.000}}}')
_rs('set_clip_opacity', {'layer': 2, 'clip': 3, 'opacity': 0.5}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"video":{"opacity":{"value":0.500}}}')
_rs('set_clip_opacity_by_id', {'clip_id': 1658311744128, 'opacity': 0.5}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"video":{"opacity":{"value":0.500}}}')
_rs('set_clip_trigger_style', {'layer': 2, 'clip': 3, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"triggerstyle":{"value":"Forward"}}')
_rs('set_clip_trigger_style_by_id', {'clip_id': 1658311744128, 'option': 'Forward'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"triggerstyle":{"value":"Forward"}}')
_rs('set_clip_target', {'layer': 2, 'clip': 3, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"target":{"value":"Forward"}}')
_rs('set_clip_target_by_id', {'clip_id': 1658311744128, 'option': 'Forward'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"target":{"value":"Forward"}}')
_rs('set_clip_beat_snap', {'layer': 2, 'clip': 3, 'option': 'Forward'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"beatsnap":{"value":"Forward"}}')
_rs('set_clip_beat_snap_by_id', {'clip_id': 1658311744128, 'option': 'Forward'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"beatsnap":{"value":"Forward"}}')
_rs('set_clip_color', {'layer': 2, 'clip': 3, 'color': 'Red'}, 'PUT', '/api/v1/composition/layers/2/clips/3', '{"colorid":{"value":"Red"}}')
_rs('set_clip_color_by_id', {'clip_id': 1658311744128, 'color': 'Red'}, 'PUT', '/api/v1/composition/clips/by-id/1658311744128', '{"colorid":{"value":"Red"}}')

# Telemetry: replies to get_product (and the probe) and get_composition; the
# ProductInfo and Composition schemas, cut to the properties the rules read. The document lists no option names for ChoiceParameters, so
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

# A get_composition reply with a layer with nothing playing: active_clip is null
# and is not assigned by the rules (the extension removes it by push).
telemetry(RS, "layer-idle", inbound_http={"path": "/api/v1/composition", "body": json.dumps({
    "layers": [{"id": 1641549604810, "name": {"value": "Overlay"}, "selected": {"value": False},
                "bypassed": {"value": False}, "solo": {"value": True}, "master": {"value": 0.5},
                "video": {"opacity": {"value": 1.0}}, "active_clip": None}]})},
    expect_state={"layers": {"1641549604810": {"name": "Overlay", "opacity": 1.0, "master": 0.5,
                                               "bypassed": False, "solo": True, "selected": False}}})

# The websocket API (support article v7.8) is read by the resolume-push
# extension, not by these rules: its composition reading, subscriptions by
# parameter id and updates are tested in crates/core/src/modules/resolume_push.rs
# and crates/core/tests/resolume_session.rs.
