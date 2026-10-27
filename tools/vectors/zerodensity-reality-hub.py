# Zero Density Reality Hub (zerodensity-reality-hub): one vector per command,
# over the REST API v2.1.0 (Reality 5.8). Paths are the OpenAPI document's,
# under /api/rest/v1, with its example ids (show 4, rundown 2, item 88);
# preview is 0 for program and 1 for preview; bodies are compact JSON from
# the document's schemas and examples. The API key goes in X-API-Key.
ZD = "zerodensity-reality-hub"
_ZK = {"token": "rh-key-1234"}
_Z = "/api/rest/v1"


def _zd(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": _Z + target}
    if body is not None:
        request["body"] = body
    V.append({"spec": ZD, "command": command, "input": input, "settings": _ZK, "expect_request": request, **extra})


_OKTXT = {"http_reply": {"status": 200, "body": "OK"}, "expect_result": {"ok": {"kind": "ack"}}}
_zd("list_engines", {}, "GET", "/engines")
_zd("get_engine", {"engine": 1}, "GET", "/engines/1")
_zd("list_nodes", {"engine": 1}, "GET", "/engines/1/nodes")
_zd("get_node", {"engine": 1, "node": "Mixer Default"}, "GET", "/engines/1/nodes/Mixer%20Default")
_zd("list_properties", {"engine": 1, "node": "Mixer Default"}, "GET", "/engines/1/nodes/Mixer%20Default/properties")
_zd("get_property", {"engine": 1, "node": "Mixer Default", "property": "Transform.Location"}, "GET",
    "/engines/1/nodes/Mixer%20Default/properties/Transform.Location")
_zd("set_property", {"engine": 1, "node": "Mixer Default", "property": "Color", "value": {"x": 0.2, "y": 0.3, "z": 0.4, "w": 0.5}},
    "PATCH", "/engines/1/nodes/Mixer%20Default/properties/Color",
    '{"Interpolation":"Constant","Duration":0.000,"Delay":0.000,"Value":{"x":0.2,"y":0.3,"z":0.4,"w":0.5}}',
    http_reply={"status": 200, "body": "{}"}, expect_result={"ok": {"kind": "ack"}})
_zd("list_functions", {"engine": 1, "node": "Lower Third"}, "GET", "/engines/1/nodes/Lower%20Third/functions")
_zd("get_function", {"engine": 1, "node": "Lower Third", "function": "Default//Play/0"}, "GET",
    "/engines/1/nodes/Lower%20Third/functions/Default%2F%2FPlay%2F0")
_zd("call_function", {"engine": 1, "node": "Lower Third", "function": "Play",
                      "parameters": [{"PropertyPath": "Title", "DisplayName": "Title", "Value": "Breaking News"}]},
    "POST", "/engines/1/nodes/Lower%20Third/functions/Play",
    '{"Parameters":[{"PropertyPath":"Title","DisplayName":"Title","Value":"Breaking News"}]}',
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})
_zd("list_graphs", {}, "GET", "/graphs")
_zd("get_graph", {"graph": 1}, "GET", "/graphs/1")
_zd("list_graph_channels", {"graph": 1}, "GET", "/graphs/1/channels")
_zd("load_graph", {"graph": 1, "host": 1}, "POST", "/graphs/1/load", '{"engineHostId":1}')
_zd("list_projects", {}, "GET", "/projects")
_zd("list_project_channels", {"project": 10}, "GET", "/projects/10/channels")
_zd("list_launcher_shows", {}, "GET", "/launcher")
_zd("get_launcher_show", {"show": 1}, "GET", "/launcher/1")
_zd("list_engine_channels", {"show": 1, "renderer": 2}, "GET", "/launcher/shows/1/engines/2/channels")
_zd("launch_show", {"show": 1}, "PUT", "/launcher/1/launch?mode=live",
    http_reply={"status": 207, "body": '{"success":true,"message":"Show started with warnings","warnings":["x"]}'},
    expect_result={"ok": {"kind": "ack"}})
_zd("stop_show", {"show": 1}, "PUT", "/launcher/1/stop")
_zd("list_lino_shows", {}, "GET", "/lino/shows")
_zd("get_lino_show", {"show": 4}, "GET", "/lino/shows/4")
_zd("list_rundowns", {}, "GET", "/lino/rundowns",
    http_reply={"status": 200, "body": '[{"id":1,"name":"News Rundown"}]'},
    expect_result={"ok": {"kind": "value", "value": [{"id": 1, "name": "News Rundown"}]}})
_zd("get_rundown", {"rundown": 2}, "GET", "/lino/rundown/2")
_zd("list_show_rundowns", {"show": 4}, "GET", "/lino/rundowns/4")
_zd("list_templates", {}, "GET", "/lino/templates")
_zd("list_items", {"show": 4, "rundown": 2}, "GET", "/lino/rundown/4/2/items/")
_zd("get_item", {"show": 4, "rundown": 2, "item": 88}, "GET", "/lino/rundown/4/2/items/88")
_zd("get_item_by_number", {"show": 4, "rundown": 2, "number": 3}, "GET", "/lino/rundown/4/2/items/by-item-no/3")
_zd("create_item", {"show": 4, "rundown": 2, "name": "Headline", "template": 2200, "after": 88,
                    "data": {"Title": "Breaking News"}}, "POST", "/lino/rundown/4/2/items/",
    '{"name":"Headline","templateId":2200,"insertAfter":88,"data":{"Title":"Breaking News"}}')
_zd("create_item_at_end", {"show": 4, "rundown": 2, "name": "Headline", "template": 2200, "data": {"Title": "Breaking News"}},
    "POST", "/lino/rundown/4/2/items/", '{"name":"Headline","templateId":2200,"data":{"Title":"Breaking News"}}')
_zd("update_item", {"show": 4, "rundown": 2, "item": 88, "name": "Headline", "template": 2200, "data": {"Title": "Update"}},
    "PATCH", "/lino/rundown/4/2/items/88", '{"name":"Headline","templateId":2200,"data":{"Title":"Update"}}')
_zd("update_item_by_number", {"show": 4, "rundown": 2, "number": 3, "name": "Headline", "template": 2200, "data": {}},
    "PATCH", "/lino/rundown/4/2/items/by-item-no/3", '{"name":"Headline","templateId":2200,"data":{}}')
_zd("delete_item", {"show": 4, "rundown": 2, "item": 88}, "DELETE", "/lino/rundown/4/2/items/88", **_OKTXT)
_zd("delete_item_by_number", {"show": 4, "rundown": 2, "number": 3}, "DELETE", "/lino/rundown/4/2/items/by-item-no/3")
_zd("get_item_buttons", {"show": 4, "rundown": 2, "item": 88}, "GET", "/lino/rundown/4/2/items/88/buttons",
    http_reply={"status": 200, "body": '{"PLAY":"PLAY","REVERSE":"REVERSE"}'},
    expect_result={"ok": {"kind": "value", "value": {"PLAY": "PLAY", "REVERSE": "REVERSE"}}})
_zd("get_item_buttons_by_number", {"show": 4, "rundown": 2, "number": 3}, "GET", "/lino/rundown/4/2/items/by-item-no/3/buttons")
_zd("press_item_button", {"show": 4, "rundown": 2, "item": 88, "button": "PLAY"}, "POST",
    "/lino/rundown/4/2/items/88/buttons/PLAY", **_OKTXT)
_zd("press_item_button_by_number", {"show": 4, "rundown": 2, "number": 3, "button": "REVERSE"}, "POST",
    "/lino/rundown/4/2/items/by-item-no/3/buttons/REVERSE")
_zd("press_combo_item_button", {"show": 4, "rundown": 2, "item": 88, "button": "PLAY", "subtemplates": [2200]}, "POST",
    "/lino/rundown/4/2/items/88/buttons/PLAY", '{"subtemplatesIds":[2200]}',
    http_reply={"status": 400, "body": '{"error":"Button key is ambiguous for combo template. Provide subtemplatesIds."}'},
    expect_result={"error": {"error": "device_error", "code": "400"}})
_zd("delete_rundown", {"show": 4, "rundown": 2}, "DELETE", "/lino/rundown/4/2")
_zd("load_rundown", {"show": 4, "rundown": 2}, "PUT", "/lino/rundown/4/2/load",
    http_reply={"status": 200, "body": '{"id":2,"name":"Sports Rundown"}'}, expect_result={"ok": {"kind": "ack"}})
_zd("append_rundown", {"show": 4, "rundown": 2, "new_rundown": 5}, "PUT", "/lino/rundown/4/2/append/5")
_zd("unload_rundown", {"show": 4, "rundown": 2}, "PUT", "/lino/rundown/4/2/unload")
_zd("play_item", {"show": 4, "item": 88, "output": "program"}, "PUT", "/lino/rundown/4/play/88/0", **_OKTXT)
_zd("play_item_by_number", {"show": 4, "rundown": 2, "number": 3, "output": "preview"}, "PUT",
    "/lino/rundown/4/play/by-item-no/2/3/1")
_zd("play_next", {"show": 4, "output": "program"}, "PUT", "/lino/rundown/4/playnext/0")
_zd("continue_item", {"show": 4, "item": 88, "output": "program"}, "PUT", "/lino/rundown/4/continue/88/0",
    http_reply={"status": 404, "body": "Item not found"}, expect_result={"error": {"error": "device_error", "code": "404"}})
_zd("continue_item_by_number", {"show": 4, "rundown": 2, "number": 3, "output": "program"}, "PUT",
    "/lino/rundown/4/continue/by-item-no/2/3/0")
_zd("take_out_item", {"show": 4, "item": 88, "output": "preview"}, "PUT", "/lino/rundown/4/out/88/1")
_zd("take_out_item_by_number", {"show": 4, "rundown": 2, "number": 3, "output": "program"}, "PUT",
    "/lino/rundown/4/out/by-item-no/2/3/0")
_zd("clear_output", {"show": 4, "output": "preview"}, "PUT", "/lino/rundown/4/clear/1")
_zd("batch", {"requests": [{"id": 1, "url": "/lino/rundowns", "verb": "GET"}]}, "PUT", "/batch",
    '[{"id":1,"url":"/lino/rundowns","verb":"GET"}]')
# A refused key is the terminal refusal.
_zd("list_engines", {}, "GET", "/engines", file="list_engines-refused",
    http_reply={"status": 401, "body": '{"error":"Invalid API key","message":"The provided API key is invalid.","code":"INVALID_API_KEY"}'},
    expect_result={"error": {"error": "auth"}})

# Telemetry: the document's schemas filled with its example values.
telemetry(ZD, "lino-shows", settings=_ZK, inbound_http={"path": _Z + "/lino/shows", "body": json.dumps([
    {"id": 4, "name": "Engine 1", "started": True, "loadedRundownInfo": [{"id": 2}],
     "engines": [{"id": 1, "pollLevelsEnabled": True, "rundownServerPort": 30030, "remoteControlPort": 30020, "devices": []}]},
    {"id": 5, "name": "Engine 2", "started": False, "loadedRundownInfo": []}])},
    expect_state={"lino": {"shows": {"4": {"name": "Engine 1", "started": True, "loaded_rundowns": {"2": True}},
                                     "5": {"name": "Engine 2", "started": False}}}},
    expect_then_send=[{"method": "GET", "target": _Z + "/lino/rundown/4/2/items/"}])
telemetry(ZD, "rundown-items", settings=_ZK, inbound_http={"path": _Z + "/lino/rundown/4/2/items/", "body": json.dumps([
    {"id": 88, "itemNo": 1, "name": "Headline", "template": "News_IT_LowerText", "templateId": 2200, "itemType": "vs",
     "outputChannel": "Main", "data": {"Title": "Breaking"}, "buttons": {"PLAY": "PLAY"}, "isComboTemplate": False,
     "status": {"preview": "Available", "program": "Playing", "online": True, "activeIn": ["program"]}},
    {"id": 89, "itemNo": 2, "name": "Map", "template": "Map", "templateId": 2300, "itemType": "md", "outputChannel": None,
     "status": {"preview": "Unavailable", "program": "Available", "online": False}}])},
    state_before={"lino": {"rundowns": {"2": {"items": {"77": {"name": "Deleted"}}}}}},
    expect_state={"lino": {"rundowns": {"2": {"show": 4, "items": {
        "88": {"number": 1, "name": "Headline", "template": "News_IT_LowerText", "type": "vs", "channel": "Main",
               "preview": "Available", "program": "Playing", "online": True},
        "89": {"number": 2, "name": "Map", "template": "Map", "type": "md", "preview": "Unavailable",
               "program": "Available", "online": False}}}}}})
telemetry(ZD, "engines", settings=_ZK, inbound_http={"path": _Z + "/engines", "body": json.dumps([
    {"id": 1, "name": "RE1", "ip": "10.0.0.11", "role": "Broadcast", "status": "connected", "cacheDir": "D:/cache",
     "minimumFreeSpace": 10, "usedSpace": 120, "diskSpace": 1000, "displayName": "Studio A", "re_enabled": True,
     "port": 6060, "ownerStudioId": None, "instances": []}])},
    expect_state={"engines": {"1": {"name": "Studio A", "ip": "10.0.0.11", "role": "Broadcast", "status": "connected",
                                    "connected": True, "used_space": 120, "disk_space": 1000}}})
telemetry(ZD, "launcher", settings=_ZK, inbound_http={"path": _Z + "/launcher", "body": json.dumps([
    {"id": 1, "name": "My Show", "channels": [], "running": False, "linoeditingmode": False}])},
    expect_state={"launcher": {"shows": {"1": {"name": "My Show", "running": False}}}})
