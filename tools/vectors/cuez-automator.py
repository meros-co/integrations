# Cuez Automator (cuez-automator): one vector per command and telemetry
# vectors for the documented reads. Targets are written by hand from the
# Automator's OpenAPI file (https://download.cuez.app/automator-openapi.yaml),
# using its own example ids; responses are its examples.
CZ = "cuez-automator"
_CZ_OK = {"http_reply": {"status": 200, "body": "OK"}, "expect_result": {"ok": {"kind": "ack"}}}
_BTN = "1839b2bb-aac0-54c3-c844-061220dfa921"
http(CZ, "next", {}, "GET", "/api/trigger/next", **_CZ_OK)
http(CZ, "previous", {}, "GET", "/api/trigger/previous")
http(CZ, "next_trigger", {}, "GET", "/api/trigger/nextTrigger")
http(CZ, "previous_trigger", {}, "GET", "/api/trigger/previousTrigger")
http(CZ, "first_trigger", {}, "GET", "/api/trigger/firstTrigger")
http(CZ, "step", {"step": 2}, "GET", "/api/trigger/step/2", **_CZ_OK)
http(CZ, "trigger_block", {"block": "cd35bd00-b789-4565-98c7-b3e26a5924e9"}, "GET",
     "/api/trigger/block/cd35bd00-b789-4565-98c7-b3e26a5924e9")
http(CZ, "get_current", {}, "GET", "/api/trigger/current",
     http_reply={"status": 200, "body": '{"id":"cd35bd00"}'}, expect_result={"ok": {"kind": "value", "value": {"id": "cd35bd00"}}})
http(CZ, "get_block_content", {}, "GET", "/api/trigger/blockcontent")
_BUTTONS = [{"id": _BTN, "title": "Take", "type": "button", "color": "yellow", "enabled": True},
            {"id": "4c2e91a7-13bd-4f0e-9a55-7e1f0c8b2d34", "title": "Cameras", "type": "buttonpane",
             "color": "blue", "enabled": True},
            {"id": "9f7a2c05-88de-4b31-a1c6-2d5e4b7f9013", "title": "Cam 1", "type": "button", "color": "green",
             "enabled": True, "parentId": "4c2e91a7-13bd-4f0e-9a55-7e1f0c8b2d34", "parentTitle": "Cameras"}]
http(CZ, "get_buttons", {}, "GET", "/api/trigger/button/",
     http_reply={"status": 200, "body": json.dumps(_BUTTONS)}, expect_result={"ok": {"kind": "value", "value": _BUTTONS}})
http(CZ, "trigger_button", {"button": _BTN}, "GET", f"/api/trigger/button/{_BTN}", **_CZ_OK)
http(CZ, "button_on", {"button": _BTN}, "GET", f"/api/trigger/button/{_BTN}/on")
http(CZ, "button_off", {"button": _BTN}, "GET", f"/api/trigger/button/{_BTN}/off")
http(CZ, "button_click", {"button": _BTN}, "GET", f"/api/trigger/button/{_BTN}/click")
http(CZ, "get_button_states", {}, "GET", "/api/trigger/button-states",
     http_reply={"status": 200, "body": json.dumps({_BTN: "on"})},
     expect_result={"ok": {"kind": "value", "value": {_BTN: "on"}}})
http(CZ, "get_shortcuts", {}, "GET", "/api/trigger/shortcut/")
http(CZ, "trigger_shortcut", {"shortcut": "a1cdb62d-b2ab-da7b-712a-3b7a8bde83df"}, "GET",
     "/api/trigger/shortcut/a1cdb62d-b2ab-da7b-712a-3b7a8bde83df")
http(CZ, "prompter_start", {}, "GET", "/api/prompter/start")
http(CZ, "prompter_goto", {"trigger": "234b033a-93ad-4d0e-96fe-ed27561fa4a2"}, "GET",
     "/api/prompter/goto/234b033a-93ad-4d0e-96fe-ed27561fa4a2")
http(CZ, "prompter_black_on", {}, "GET", "/api/prompter/black/on")
http(CZ, "prompter_black_off", {}, "GET", "/api/prompter/black/off")
http(CZ, "get_macros", {}, "GET", "/api/macro/")
http(CZ, "fire_macro", {"macro": "c66690ca-5879-a303-fc1a-dd65b4996db8"}, "GET",
     "/api/macro/c66690ca-5879-a303-fc1a-dd65b4996db8",
     http_reply={"status": 400, "body": '{"response":"unknown macro"}'},
     expect_result={"error": {"error": "device_error", "code": "400"}})
V.append({"spec": CZ, "command": "fire_macro_with_values",
          "input": {"macro": "c66690ca-5879-a303-fc1a-dd65b4996db8",
                    "values": {"a729fc67-c701-438e-8812-5f139ea9a27b": "hello world"}},
          "expect_request": {"method": "POST", "target": "/api/macro/c66690ca-5879-a303-fc1a-dd65b4996db8",
                             "body": '{"a729fc67-c701-438e-8812-5f139ea9a27b":"hello world"}'}, **_CZ_OK})
V.append({"spec": CZ, "command": "fire_macro_by_name",
          "input": {"name": "my-macro-name", "values": {"my-variable-name": "hello world"}},
          "expect_request": {"method": "POST", "target": "/api/macro/my-macro-name",
                             "body": '{"my-variable-name":"hello world"}'}})
http(CZ, "get_timers", {}, "GET", "/api/timer")
http(CZ, "start_timer", {"timer": "9c245298-806f-2c5c-8f5a-4a6a96909877"}, "GET",
     "/api/timer/start/9c245298-806f-2c5c-8f5a-4a6a96909877")
http(CZ, "stop_timer", {"timer": "112af65b-60a1-fb19-fa17-926e2f75986b"}, "GET",
     "/api/timer/stop/112af65b-60a1-fb19-fa17-926e2f75986b")
http(CZ, "stop_all_timers", {}, "GET", "/api/timer/stop")
http(CZ, "get_episodes", {}, "GET", "/api/project/episodes")
http(CZ, "select_episode", {"episode": "daeb8e4a-413e-972e-d393-d94f75674356"}, "PATCH",
     "/api/episode/daeb8e4a-413e-972e-d393-d94f75674356/select", **_CZ_OK)
http(CZ, "unload_episode", {}, "DELETE", "/api/episode")
http(CZ, "get_episode", {}, "GET", "/api/episode")
http(CZ, "get_episode_items", {}, "GET", "/api/episode/items")
http(CZ, "get_episode_item", {"item": "e0db32fe-1cd3-a2ca-27e6-5aa98a42a62f"}, "GET",
     "/api/episode/items/e0db32fe-1cd3-a2ca-27e6-5aa98a42a62f")
http(CZ, "get_script", {}, "GET", "/api/episode/script")
V.append({"spec": CZ, "command": "set_block_column",
          "input": {"instance": "4f6f5b9e-9c1e-4b2a-8e3d-2a6f7c9d1b0a", "column": "7c1e2f3a-4b5c-6d7e-8f9a-0b1c2d3e4f5a",
                    "value": {"html": "Hello world"}},
          "expect_request": {"method": "POST", "target": "/api/episode/block/4f6f5b9e-9c1e-4b2a-8e3d-2a6f7c9d1b0a/column",
                             "body": '{"columnId":"7c1e2f3a-4b5c-6d7e-8f9a-0b1c2d3e4f5a","value":{"html":"Hello world"}}'}})
_VARS = {"variables": [
    {"id": "3f1c2a", "name": "clip-link", "description": "the link to your clip", "kind": "variable",
     "dataType": "string", "default": "", "valueB": None, "options": None, "listId": None,
     "value": "https://example.com/clip.mp4", "rawValue": None, "isSet": True,
     "updatedAt": "2026-10-09T10:00:00.000Z"},
    {"id": "8b7d01", "name": "my AB alternator", "description": "", "kind": "alternator", "dataType": "string",
     "default": "A", "valueB": "B", "options": None, "listId": None, "value": "A", "rawValue": None,
     "isSet": False, "updatedAt": None}]}
http(CZ, "get_variables", {}, "GET", "/api/system/variables",
     http_reply={"status": 200, "body": json.dumps(_VARS)},
     expect_result={"ok": {"kind": "value", "value": _VARS["variables"]}})
http(CZ, "get_configurations", {}, "GET", "/api/configuration")
http(CZ, "select_configuration", {"id": "default"}, "PATCH", "/api/configuration?id=default")
http(CZ, "get_project_config", {"project": "64c1a7de-11c5-4e2e-8f3a-9d6b2c7a1e40"}, "GET",
     "/api/projects/64c1a7de-11c5-4e2e-8f3a-9d6b2c7a1e40/config",
     http_reply={"status": 404, "body": '{"error":"Unknown project id"}'},
     expect_result={"error": {"error": "device_error", "code": "404"}})
V.append({"spec": CZ, "command": "set_project_config",
          "input": {"project": "64c1a7de-11c5-4e2e-8f3a-9d6b2c7a1e40",
                    "config": {"nrcs": "mos", "mos_id": "CUEZAUTOMATOR.MOS", "mos_ncs_id": "NEWSROOM",
                               "mos_newsroom_host": "192.168.1.20", "mos_lower_port": "10540",
                               "mos_upper_port": "10541", "mos_dual_peer_enabled": False}},
          "expect_request": {"method": "PUT", "target": "/api/projects/64c1a7de-11c5-4e2e-8f3a-9d6b2c7a1e40",
                             "body": '{"nrcs":"mos","mos_id":"CUEZAUTOMATOR.MOS","mos_ncs_id":"NEWSROOM",'
                                     '"mos_newsroom_host":"192.168.1.20","mos_lower_port":"10540",'
                                     '"mos_upper_port":"10541","mos_dual_peer_enabled":false}'},
          "http_reply": {"status": 200, "body": '{"message":"UNPAIRED"}'},
          "expect_result": {"ok": {"kind": "value", "value": "UNPAIRED"}}})
http(CZ, "get_web_connection", {}, "GET", "/api/app/webconnection")

telemetry(CZ, "buttons", state_before={"buttons": {"gone": {"title": "Old"}}},
          inbound_http={"path": "/api/trigger/button/", "body": json.dumps(_BUTTONS)},
          expect_state={"buttons": {
              _BTN: {"index": 0, "title": "Take", "type": "button", "color": "yellow", "enabled": True},
              "4c2e91a7-13bd-4f0e-9a55-7e1f0c8b2d34": {"index": 1, "title": "Cameras", "type": "buttonpane",
                                                       "color": "blue", "enabled": True},
              "9f7a2c05-88de-4b31-a1c6-2d5e4b7f9013": {"index": 2, "title": "Cam 1", "type": "button",
                                                       "color": "green", "enabled": True,
                                                       "parent": "4c2e91a7-13bd-4f0e-9a55-7e1f0c8b2d34"}}})
telemetry(CZ, "button-states",
          inbound_http={"path": "/api/trigger/button-states",
                        "body": json.dumps({_BTN: "on", "9f7a2c05-88de-4b31-a1c6-2d5e4b7f9013": "off"})},
          expect_state={"switches": {_BTN: True, "9f7a2c05-88de-4b31-a1c6-2d5e4b7f9013": False}})
telemetry(CZ, "variables", inbound_http={"path": "/api/system/variables", "body": json.dumps(_VARS)},
          expect_state={"variables": {
              "3f1c2a": {"name": "clip-link", "kind": "variable", "data_type": "string",
                         "value": "https://example.com/clip.mp4", "is_set": True,
                         "updated_at": "2026-10-09T10:00:00.000Z"},
              "8b7d01": {"name": "my AB alternator", "kind": "alternator", "data_type": "string", "value": "A",
                         "is_set": False}}})
telemetry(CZ, "switch-reread", inbound_http={"path": f"/api/trigger/button/{_BTN}/on", "body": "OK"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/api/trigger/button-states"}])
