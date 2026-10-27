# CharacterWorks (characterworks): one vector per command. Every request is a
# POST of one JSON command object to / on port 5201, written here from
# CharacterWorks' "Remote Control" help page and its examples (compact JSON).
CW = "characterworks"
_CW_OK = {"http_reply": {"status": 200, "body": '{"result":"success"}'}, "expect_result": {"ok": {"kind": "ack"}}}


def _cw(command, input, body, **extra):
    V.append({"spec": CW, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": "/", "body": body}, **extra})


_cw("list_motions", {}, '{"action":"list_motions"}',
    http_reply={"status": 200, "body": '{"motions":["Motion 1","Motion 2"]}'},
    expect_result={"ok": {"kind": "value", "value": ["Motion 1", "Motion 2"]}})
_cw("list_motions_with_ids", {}, '{"action":"list_motions_with_ids"}')
_cw("list_layers", {"channel": "preview"}, '{"action":"list_layers","channel":"preview"}')
_cw("list_layers_under", {"parent": "UnderWater\\sea", "channel": "live1"},
    '{"action":"list_layers","parent":"UnderWater\\\\sea","channel":"live1"}')
for verb in ["play", "stop", "eject", "finish", "pause", "resume", "restart", "finish_and_restart"]:
    extra = _CW_OK if verb == "play" else {}
    _cw(f"{verb}_motions", {"motions": ["Motion 1", "Motion 2"], "channel": "live1"},
        '{"action":"%s_motions","motions":["Motion 1","Motion 2"],"channel":"live1"}' % verb, **extra)
    _cw(f"{verb}_motions_by_id", {"motion_ids": ["18446462600734584000"], "channel": "live2"},
        '{"action":"%s_motions","motion_ids":["18446462600734584000"],"channel":"live2"}' % verb)
# A warning is still success; a failure fails the command.
_cw("play_motions", {"motions": ["Motion 1"], "channel": "live1"},
    '{"action":"play_motions","motions":["Motion 1"],"channel":"live1"}',
    http_reply={"status": 200, "body": '{"result":"success","warnings":[{"code":"already_playing","description":"Motion is already playing"}]}'},
    expect_result={"ok": {"kind": "ack"}}, file="play_motions-warning")
_cw("stop_motions", {"motions": ["Nope"], "channel": "live1"},
    '{"action":"stop_motions","motions":["Nope"],"channel":"live1"}',
    http_reply={"status": 200, "body": '{"result":"failure","errors":[{"code":"motion_not_found","description":"Motion not found"}]}'},
    expect_result={"error": {"error": "device_error"}}, file="stop_motions-failure")
# The page's example: {"action":"set_text","layer":"Motion 1\Text 1","value":"content","channel":"preview"}
_cw("set_text", {"layer": "Motion 1\\Text 1", "value": "content", "channel": "preview"},
    '{"action":"set_text","layer":"Motion 1\\\\Text 1","value":"content","channel":"preview"}', **_CW_OK)
_cw("set_text_by_id", {"layer_id": "18446462600734619424", "value": "Line one\nLine two", "channel": "live1"},
    '{"action":"set_text","layer_id":"18446462600734619424","value":"Line one\\nLine two","channel":"live1"}')
_cw("list_grid_names", {}, '{"action":"list_grid_names"}')
_cw("list_grid_cells", {"grid": "Grid 1"}, '{"action":"list_grid_cells","grid":"Grid 1"}')
_cw("activate_grid_cell", {"grid": "Grid 1", "column": 0, "row": 0, "channel": "live1"},
    '{"action":"activate_grid_cell","grid":"Grid 1","cell":[0,0],"channel":"live1"}')
_cw("run_data_source_query", {"source": "XML/JSON Source 1"}, '{"action":"run_data_source_query","data_source":"XML/JSON Source 1"}')
_cw("set_data_source_query_parameter", {"source": "XML/JSON Source 1", "parameter": "city", "value": "London"},
    '{"action":"set_data_source_query_parameter","data_source":"XML/JSON Source 1","parameter":"city","value":"London"}')
_cw("remove_data_source_query_parameter", {"source": "XML/JSON Source 1", "parameter": "city"},
    '{"action":"set_data_source_query_parameter","data_source":"XML/JSON Source 1","parameter":"city"}')
_cw("select_data_source_rows", {"source": "XML/JSON Source 1", "rows": [0, 1, 5, 6]},
    '{"action":"select_data_source_rows","data_source":"XML/JSON Source 1","index_list":[0,1,5,6]}')
_cw("select_data_source_rows_matching", {"source": "XML/JSON Source 1", "field": "City", "value": "Lon*"},
    '{"action":"select_data_source_rows","data_source":"XML/JSON Source 1","field":"City","value":"Lon*"}')
_cw("send_commands", {"commands": [{"action": "play_motions", "motions": ["Motion 1"]}, {"action": "stop_motions", "motions": ["Motion 2"]}]},
    '[{"action":"play_motions","motions":["Motion 1"]},{"action":"stop_motions","motions":["Motion 2"]}]')

telemetry(CW, "motions", inbound_http={"path": "/", "request": {"action": "list_motions_with_ids"},
                                       "body": '{"motions":[{"name":"Motion 1","id":"18446462600734619424"},{"name":"Motion 2","id":"18446462600734584000"}]}'},
          state_before={"motions": {"Old": {"id": "1"}}},
          expect_state={"motions": {"Motion 1": {"id": "18446462600734619424"}, "Motion 2": {"id": "18446462600734584000"}}})
telemetry(CW, "grids", inbound_http={"path": "/", "request": {"action": "list_grid_names"},
                                     "body": '{"result":"success","grids":["Grid 1","Scores"]}'},
          expect_state={"grids": {"Grid 1": True, "Scores": True}})
