# Rundown Studio (rundown-studio): one vector per command and telemetry
# vectors for the event stream and the reads, written by hand from the API
# v1 OpenAPI file (https://api-v1.rundownstudio.app/openapi.yml), with its
# example ids and responses. Every vector runs against the engine's loopback
# fake: none reaches rundownstudio.app.
RS = "rundown-studio"
_RS_SET = {"token": "test-token", "rundown_id": "YOmm4UG2SGSomuOci44c"}
_R = "/rundowns/YOmm4UG2SGSomuOci44c"
_J = lambda d: json.dumps(d, separators=(",", ":"))
_STATUS = {"server_time": 1747000050000, "state": "running",
           "active_cue": {"id": "Ec0mmeOaGq68oiASGoSG", "title": "Guest intro", "started_at": 1747000050000,
                          "paused_at": None, "duration_ms": 120000},
           "next_cue": {"id": "ca064OOCWK2wGIQ6KOGq", "title": "Closing"}, "sub_timers": []}
_META = {"request_id": "req_8f4c2b6d9e1a7c3b5d8e2f01", "server_time": 1747000050000, "side_effects": []}
_ACT = {"http_reply": {"status": 200, "body": _J({"ok": True, "message": "Jumped to cue", "data": _STATUS, "meta": _META})},
        "expect_result": {"ok": {"kind": "ack"}}}
_P409 = {"http_reply": {"status": 409, "body": _J({"type": "about:blank", "title": "Conflict", "status": 409,
                                                   "code": "runner.no_next_cue"})},
         "expect_result": {"error": {"error": "device_error", "code": "409"}}}


def _rs(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": RS, "command": command, "input": input, "settings": _RS_SET, "expect_request": request, **extra})


_rs("ping", {}, "GET", "/ping",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "pong", "data": {"version": "1.1.0"}, "meta": _META})},
    expect_result={"ok": {"kind": "value", "value": "1.1.0"}})
_rs("get_token_info", {}, "GET", "/auth")
_rs("check_access", {}, "GET", "/auth/probe?rundown_id=YOmm4UG2SGSomuOci44c")
_rs("get_status", {}, "GET", _R + "/status",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "OK", "data": _STATUS, "meta": _META})},
    expect_result={"ok": {"kind": "value", "value": _STATUS}})
_rs("get_countdown", {}, "GET", _R + "/countdown")
_rs("start", {}, "POST", _R + "/action:start", **_ACT)
_rs("pause", {}, "POST", _R + "/action:pause")
_rs("end", {}, "POST", _R + "/action:end")
_rs("reset_rundown", {}, "POST", _R + "/action:reset-rundown")
_rs("next", {}, "POST", _R + "/action:next", **_P409)
_rs("previous", {}, "POST", _R + "/action:previous")
_rs("jump", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c"}, "POST", _R + "/action:jump", '{"cue_id":"Ii6WeSEQmEqyKY6YAu8c"}', **_ACT)
_rs("adjust_duration", {"delta_ms": -30000}, "POST", _R + "/action:adjust-duration", '{"delta_ms":-30000}')
_rs("get_output_message", {}, "GET", _R + "/output-message")
_rs("set_output_message_text", {"text": "STAND BY"}, "PATCH", _R + "/output-message", '{"text":"STAND BY"}')
_rs("set_output_message_visible", {"visible": True}, "PATCH", _R + "/output-message", '{"visible":true}')
_rs("set_output_message_color", {"color": "#ffffff"}, "PATCH", _R + "/output-message", '{"color":"#ffffff"}')
_rs("set_output_message_bold", {"bold": False}, "PATCH", _R + "/output-message", '{"bold":false}')
_rs("set_output_message_underline", {"underline": True}, "PATCH", _R + "/output-message", '{"underline":true}')
_SUB = '{"cue_id":"Ii6WeSEQmEqyKY6YAu8c","sub_timer_id":"st1"}'
_rs("start_sub_timer", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "sub_timer_id": "st1"}, "POST", _R + "/action:start-sub-timer", _SUB)
_rs("pause_sub_timer", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "sub_timer_id": "st1"}, "POST", _R + "/action:pause-sub-timer", _SUB)
_rs("reset_sub_timer", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "sub_timer_id": "st1"}, "POST", _R + "/action:reset-sub-timer", _SUB)
_rs("list_sub_timers", {}, "GET", _R + "/sub-timers")
_rs("list_rundowns", {}, "GET", "/rundowns?limit=100")
_rs("list_rundowns_page", {"cursor": "eyJ2IjoxfQ", "limit": 50}, "GET", "/rundowns?cursor=eyJ2IjoxfQ&limit=50")
_rs("create_rundown", {"body": {"title": "Evening news", "timezone": "Europe/Oslo"}}, "POST", "/rundowns",
    '{"title":"Evening news","timezone":"Europe/Oslo"}',
    http_reply={"status": 201, "body": _J({"ok": True, "message": "Created", "data": {"id": "NEWrundown1", "title": "Evening news"},
                                           "included": {"cues": [], "columns": [], "cells": []}, "meta": _META})},
    expect_result={"ok": {"kind": "value", "value": "NEWrundown1"}})
_rs("get_rundown", {}, "GET", _R + "?include=cues%2Ccolumns")
_rs("update_rundown", {"body": {"status": "approved"}}, "PATCH", _R, '{"status":"approved"}')
_rs("delete_rundown", {}, "DELETE", _R)
_rs("duplicate_rundown", {"body": {"title": "Copy"}}, "POST", _R + ":duplicate", '{"title":"Copy"}')
_rs("export_rundown", {"format": "pdf"}, "GET", _R + "/export?format=pdf")
_rs("list_cues", {"limit": 50}, "GET", _R + "/cues?limit=50")
_rs("list_cues_page", {"cursor": "eyJ2IjoxLCJhZnRlcl9pZCI6ImN1ZV94In0"}, "GET",
    _R + "/cues?cursor=eyJ2IjoxLCJhZnRlcl9pZCI6ImN1ZV94In0&limit=100")
_rs("create_cue", {"body": {"type": "cue", "title": "Weather", "duration_ms": 90000, "position": {"after_cue_id": "Ii6WeSEQmEqyKY6YAu8c"}}},
    "POST", _R + "/cues", '{"type":"cue","title":"Weather","duration_ms":90000,"position":{"after_cue_id":"Ii6WeSEQmEqyKY6YAu8c"}}')
_rs("create_cues", {"body": {"cues": [{"type": "heading", "title": "Part 2"}], "position": "end"}}, "POST", _R + "/cues:bulk",
    '{"cues":[{"type":"heading","title":"Part 2"}],"position":"end"}')
_rs("reorder_cues", {"body": {"cue_order": [{"cue_id": "Ii6WeSEQmEqyKY6YAu8c"}]}}, "POST", _R + "/cues:reorder",
    '{"cue_order":[{"cue_id":"Ii6WeSEQmEqyKY6YAu8c"}]}')
_rs("get_cue", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c"}, "GET", _R + "/cues/Ii6WeSEQmEqyKY6YAu8c")
_rs("update_cue", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "body": {"duration_ms": 75000}}, "PATCH", _R + "/cues/Ii6WeSEQmEqyKY6YAu8c",
    '{"duration_ms":75000}')
_rs("delete_cue", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c"}, "DELETE", _R + "/cues/Ii6WeSEQmEqyKY6YAu8c")
_rs("list_columns", {}, "GET", _R + "/columns")
_rs("create_column", {"body": {"name": "Camera", "width": 200}}, "POST", _R + "/columns", '{"name":"Camera","width":200}')
_rs("reorder_columns", {"body": {"column_order": [{"column_id": "EoeiemGEmeMOY8GwUK6s"}]}}, "POST", _R + "/columns:reorder",
    '{"column_order":[{"column_id":"EoeiemGEmeMOY8GwUK6s"}]}')
_rs("get_column", {"column_id": "EoeiemGEmeMOY8GwUK6s"}, "GET", _R + "/columns/EoeiemGEmeMOY8GwUK6s")
_rs("update_column", {"column_id": "EoeiemGEmeMOY8GwUK6s", "name": "Audio", "width": 300}, "PATCH",
    _R + "/columns/EoeiemGEmeMOY8GwUK6s", '{"name":"Audio","width":300}')
_rs("delete_column", {"column_id": "EoeiemGEmeMOY8GwUK6s"}, "DELETE", _R + "/columns/EoeiemGEmeMOY8GwUK6s")
_rs("list_cells", {}, "GET", _R + "/cells?limit=1000")
_rs("list_cue_cells", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c"}, "GET", _R + "/cells?cue_id=Ii6WeSEQmEqyKY6YAu8c&limit=1000")
_rs("list_column_cells", {"column_id": "EoeiemGEmeMOY8GwUK6s"}, "GET", _R + "/cells?column_id=EoeiemGEmeMOY8GwUK6s&limit=1000")
_rs("set_cell", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "column_id": "EoeiemGEmeMOY8GwUK6s", "content": "Mic check"}, "PATCH",
    _R + "/cells", '{"cue_id":"Ii6WeSEQmEqyKY6YAu8c","column_id":"EoeiemGEmeMOY8GwUK6s","content":"Mic check"}')
_rs("set_cell_html", {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "column_id": "EoeiemGEmeMOY8GwUK6s", "content_html": "<p>Mic check</p>"},
    "PATCH", _R + "/cells", '{"cue_id":"Ii6WeSEQmEqyKY6YAu8c","column_id":"EoeiemGEmeMOY8GwUK6s","content_html":"<p>Mic check</p>"}')
_rs("set_cells", {"body": {"cells": [{"cue_id": "a", "column_id": "b", "content": "x"}]}}, "PATCH", _R + "/cells:bulk",
    '{"cells":[{"cue_id":"a","column_id":"b","content":"x"}]}')
_rs("list_mentions", {}, "GET", _R + "/mentions")
_rs("create_mention", {"name": "Anna", "color": "#2196F3"}, "POST", _R + "/mentions",
    '{"name":"Anna","color":"#2196F3","description":""}')
_rs("update_mentions", {"body": {"mentions": [{"id": "m1", "color": "#F44336"}]}}, "POST", _R + "/mentions:bulk",
    '{"mentions":[{"id":"m1","color":"#F44336"}]}')
_rs("get_mention", {"mention_id": "m1"}, "GET", _R + "/mentions/m1")
_rs("update_mention", {"mention_id": "m1", "name": "Anna", "color": "#F44336", "description": "Host"}, "PATCH",
    _R + "/mentions/m1", '{"name":"Anna","color":"#F44336","description":"Host"}')
_rs("delete_mention", {"mention_id": "m1"}, "DELETE", _R + "/mentions/m1")
_rs("list_text_variables", {}, "GET", _R + "/text-variables")
_rs("create_text_variable", {"key": "guest", "value": "Jane Doe"}, "POST", _R + "/text-variables",
    '{"key":"guest","value":"Jane Doe"}')
_rs("set_text_variables", {"variables": [{"key": "guest", "value": "John"}], "mode": "merge"}, "POST",
    _R + "/text-variables:bulk", '{"text_variables":[{"key":"guest","value":"John"}],"mode":"merge"}')
_rs("get_text_variable", {"key": "guest"}, "GET", _R + "/text-variables/guest",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "OK", "data": {"key": "guest", "value": "Jane Doe"}, "meta": _META})},
    expect_result={"ok": {"kind": "value", "value": "Jane Doe"}})
_rs("set_text_variable", {"key": "guest", "value": "John"}, "PATCH", _R + "/text-variables/guest", '{"value":"John"}')
_rs("delete_text_variable", {"key": "guest"}, "DELETE", _R + "/text-variables/guest",
    http_reply={"status": 401, "body": _J({"status": 401, "code": "auth.invalid_token"})},
    expect_result={"error": {"error": "auth"}})

# Telemetry: the event stream (the API file's example frames).
_SSE_STATUS = {"server_time": 1747000000000, "state": "running",
               "active_cue": {"id": "Ii6WeSEQmEqyKY6YAu8c", "title": "Opening monologue", "started_at": 1747000000000,
                              "paused_at": None, "duration_ms": 60000},
               "next_cue": {"id": "Ec0mmeOaGq68oiASGoSG", "title": "Guest intro"},
               "sub_timers": [{"id": "st1", "name": "Q&A", "cue_id": "Ii6WeSEQmEqyKY6YAu8c", "cue_title": "Opening monologue",
                               "started_at": 1747000001000, "paused_at": 1747000002000, "duration_ms": 30000}]}
telemetry(RS, "sse-status", settings=_RS_SET, inbound_sse={"event": "status", "data": _J(_SSE_STATUS)},
          state_before={"status": {"next_cue": {"id": "old", "title": "Old"}}},
          expect_state={"status": {"state": "running", "server_time": 1747000000000,
                                   "active_cue": {"id": "Ii6WeSEQmEqyKY6YAu8c", "title": "Opening monologue",
                                                  "started_at": 1747000000000, "duration_ms": 60000},
                                   "next_cue": {"id": "Ec0mmeOaGq68oiASGoSG", "title": "Guest intro"},
                                   "sub_timers": {"st1": {"name": "Q&A", "cue_id": "Ii6WeSEQmEqyKY6YAu8c",
                                                          "cue_title": "Opening monologue", "started_at": 1747000001000,
                                                          "paused_at": 1747000002000, "duration_ms": 30000}}}})
telemetry(RS, "sse-stopped", settings=_RS_SET,
          state_before={"status": {"active_cue": {"id": "x"}, "next_cue": {"id": "y"}, "sub_timers": {"st1": {"name": "Q&A"}}}},
          inbound_sse={"event": "status", "data": '{"server_time":1747000099000,"state":"stopped","active_cue":null,"next_cue":null,"sub_timers":[]}'},
          expect_state={"status": {"state": "stopped", "server_time": 1747000099000}})
telemetry(RS, "sse-cue", settings=_RS_SET, inbound_sse={"event": "cue", "data": _J({
              "change": "modified", "id": "Ii6WeSEQmEqyKY6YAu8c",
              "cue": {"id": "Ii6WeSEQmEqyKY6YAu8c", "type": "cue", "title": "Opening monologue", "subtitle": "",
                      "duration_ms": 60000, "background_color": None, "prevent_edits": False, "skipped_by": None,
                      "start_time": None, "created_at": None, "updated_at": None}})},
          expect_state={"cues": {"Ii6WeSEQmEqyKY6YAu8c": {"type": "cue", "title": "Opening monologue", "subtitle": "",
                                                          "duration_ms": 60000, "prevent_edits": False}}})
telemetry(RS, "sse-cue-removed", settings=_RS_SET, state_before={"cues": {"c1": {"title": "Gone"}, "c2": {"title": "Kept"}}},
          inbound_sse={"event": "cue", "data": '{"change":"removed","id":"c1"}'},
          expect_state={"cues": {"c2": {"title": "Kept"}}})
telemetry(RS, "sse-column", settings=_RS_SET, inbound_sse={"event": "column", "data": _J({
              "change": "added", "id": "EoeiemGEmeMOY8GwUK6s",
              "column": {"id": "EoeiemGEmeMOY8GwUK6s", "name": "Camera", "width": 200}})},
          expect_state={"columns": {"EoeiemGEmeMOY8GwUK6s": {"name": "Camera", "width": 200}}})
telemetry(RS, "sse-column-removed", settings=_RS_SET, state_before={"columns": {"k1": {"name": "Gone"}}},
          inbound_sse={"event": "column", "data": '{"change":"removed","id":"k1"}'}, expect_state={"columns": {}})
telemetry(RS, "sse-cell", settings=_RS_SET, inbound_sse={"event": "cell", "data": _J({
              "change": "modified", "cue_id": "Ii6WeSEQmEqyKY6YAu8c", "column_id": "EoeiemGEmeMOY8GwUK6s",
              "cell": {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "column_id": "EoeiemGEmeMOY8GwUK6s", "content": "Mic check",
                       "content_html": "<p>Mic check</p>", "created_at": "2024-05-11T18:00:00.000Z",
                       "updated_at": "2024-05-12T01:46:40.000Z"}})},
          expect_state={"cells": {"Ii6WeSEQmEqyKY6YAu8c": {"EoeiemGEmeMOY8GwUK6s": {"content": "Mic check",
                                                                                   "html": "<p>Mic check</p>"}}}})
telemetry(RS, "sse-cell-removed", settings=_RS_SET, state_before={"cells": {"c1": {"k1": {"content": "x"}}}},
          inbound_sse={"event": "cell", "data": '{"change":"removed","cue_id":"c1","column_id":"k1"}'},
          expect_state={"cells": {"c1": {}}})
telemetry(RS, "sse-rundown", settings=_RS_SET, inbound_sse={"event": "rundown", "data": _J({
              "change": "modified", "id": "YOmm4UG2SGSomuOci44c",
              "rundown": {"id": "YOmm4UG2SGSomuOci44c", "title": "Evening news", "timezone": "Europe/Oslo",
                          "start_time": 1747000000000, "end_time": None, "status": "approved",
                          "updated_at": "2024-05-12T01:46:40.000Z"}})},
          expect_state={"rundown": {"title": "Evening news", "timezone": "Europe/Oslo", "start_time": 1747000000000,
                                    "status": "approved", "updated_at": "2024-05-12T01:46:40.000Z"}},
          expect_then_send=[{"method": "GET", "target": _R + "?include=cues%2Ccolumns"}])
telemetry(RS, "sse-disconnect", settings=_RS_SET, inbound_sse={"event": "disconnect", "data": '{"reason":"lifetime"}'},
          expect_state={"stream": {"last_disconnect": "lifetime"}})
telemetry(RS, "sse-heartbeat", settings=_RS_SET, inbound_sse={"event": "heartbeat", "data": '{"at":1747000005000}'},
          expect_state={})

# Telemetry: the reads.
telemetry(RS, "action-status", settings=_RS_SET,
          inbound_http={"path": _R + "/action:jump", "body": _J({"ok": True, "message": "Jumped to cue", "data": _STATUS, "meta": _META})},
          expect_state={"status": {"state": "running", "server_time": 1747000050000,
                                   "active_cue": {"id": "Ec0mmeOaGq68oiASGoSG", "title": "Guest intro",
                                                  "started_at": 1747000050000, "duration_ms": 120000},
                                   "next_cue": {"id": "ca064OOCWK2wGIQ6KOGq", "title": "Closing"}}})
telemetry(RS, "countdown", settings=_RS_SET,
          inbound_http={"path": _R + "/countdown", "body": _J({"ok": True, "message": "OK", "data": {
              "state": "running", "server_time": 1747000000000,
              "active_cue": {"cue_id": "Ii6WeSEQmEqyKY6YAu8c", "title": "Opening monologue", "duration_ms": 60000,
                             "remaining_ms": 42000, "remaining": {"is_overtime": False, "prefix": "", "days": 0,
                                                                  "hours": 0, "minutes": 0, "seconds": 42,
                                                                  "formatted": "00:42"}},
              "sub_timers": []}, "meta": _META})},
          expect_state={"countdown": {"remaining_ms": 42000, "formatted": "00:42", "overtime": False}})
telemetry(RS, "output-message", settings=_RS_SET,
          inbound_http={"path": _R + "/output-message", "body": _J({"ok": True, "message": "Output message updated", "data": {
              "text": "STAND BY", "visible": True, "color": "#ffffff", "bold": False, "underline": False}, "meta": _META})},
          expect_state={"output_message": {"text": "STAND BY", "visible": True, "color": "#ffffff", "bold": False,
                                            "underline": False}})
telemetry(RS, "rundown-with-cues", settings=_RS_SET,
          state_before={"cues": {"old": {"title": "Old"}}, "columns": {"oldc": {"name": "Old"}}},
          inbound_http={"path": _R + "?include=cues%2Ccolumns", "body": _J({
              "ok": True, "message": "OK",
              "data": {"id": "YOmm4UG2SGSomuOci44c", "title": "Evening news", "timezone": "Europe/Oslo",
                       "start_time": 1747000000000, "end_time": 1747003600000, "status": "draft",
                       "settings": {"cue_numbering": {"start_from": 1, "prefix": "", "padding": 1}, "tod_display_format": None},
                       "cue_order": [{"id": "h1", "children": [{"id": "c1"}, {"id": "c2"}]}, {"id": "c3"}],
                       "column_order": ["k1", "k2"], "created_at": None, "updated_at": "2024-05-12T01:46:40.000Z"},
              "included": {"cues": [
                  {"id": "h1", "type": "group", "title": "Opening", "subtitle": "", "duration_ms": 0, "background_color": "#333333",
                   "prevent_edits": False, "skipped_by": None, "start_time": None},
                  {"id": "c1", "type": "cue", "title": "Monologue", "subtitle": "Host", "duration_ms": 60000, "background_color": None,
                   "prevent_edits": True, "skipped_by": "group", "start_time": 1747000000000},
                  {"id": "c2", "type": "cue", "title": "Guest", "subtitle": "", "duration_ms": 120000, "background_color": None,
                   "prevent_edits": False, "skipped_by": None, "start_time": None},
                  {"id": "c3", "type": "cue", "title": "Closing", "subtitle": "", "duration_ms": 30000, "background_color": None,
                   "prevent_edits": False, "skipped_by": "self", "start_time": None}],
                  "columns": [{"id": "k1", "name": "Camera", "width": 200}, {"id": "k2", "name": "Audio", "width": 150}]},
              "meta": {**_META, "applied_includes": ["cues", "columns"]}})},
          expect_state={
              "rundown": {"title": "Evening news", "timezone": "Europe/Oslo", "start_time": 1747000000000,
                          "end_time": 1747003600000, "status": "draft", "updated_at": "2024-05-12T01:46:40.000Z"},
              "cues": {"h1": {"type": "group", "title": "Opening", "subtitle": "", "duration_ms": 0,
                              "background_color": "#333333", "prevent_edits": False, "position": 0},
                       "c1": {"type": "cue", "title": "Monologue", "subtitle": "Host", "duration_ms": 60000,
                              "prevent_edits": True, "skipped_by": "group", "start_time": 1747000000000,
                              "position": 0, "parent": "h1"},
                       "c2": {"type": "cue", "title": "Guest", "subtitle": "", "duration_ms": 120000,
                              "prevent_edits": False, "position": 1, "parent": "h1"},
                       "c3": {"type": "cue", "title": "Closing", "subtitle": "", "duration_ms": 30000,
                              "prevent_edits": False, "skipped_by": "self", "position": 1}},
              "columns": {"k1": {"name": "Camera", "width": 200, "position": 0},
                          "k2": {"name": "Audio", "width": 150, "position": 1}}})
telemetry(RS, "cells", settings=_RS_SET, state_before={"cells": {"old": {"k": {"content": "x"}}}},
          inbound_http={"path": _R + "/cells?limit=1000", "body": _J({"ok": True, "message": "OK", "data": {
              "items": [{"cue_id": "c1", "column_id": "k1", "content": "Cam 1", "content_html": "<p>Cam 1</p>",
                         "created_at": None, "updated_at": None}], "count": 1, "total": 1}, "meta": _META})},
          expect_state={"cells": {"c1": {"k1": {"content": "Cam 1", "html": "<p>Cam 1</p>"}}}})
telemetry(RS, "columns", settings=_RS_SET,
          inbound_http={"path": _R + "/columns", "body": _J({"ok": True, "message": "OK", "data": {
              "items": [{"id": "k1", "name": "Camera", "width": 220}], "count": 1}, "meta": _META})},
          expect_state={"columns": {"k1": {"name": "Camera", "width": 220}}})
telemetry(RS, "sub-timers", settings=_RS_SET,
          inbound_http={"path": _R + "/sub-timers", "body": _J({"ok": True, "message": "OK", "data": {"items": [
              {"id": "st1", "name": "Q&A", "cue_id": "c1", "cue_title": "Monologue", "direction": "down",
               "duration_ms": 30000, "end_action": "stop", "auto_start": False, "show_on_output": True}], "count": 1},
              "meta": _META})},
          expect_state={"sub_timers": {"st1": {"name": "Q&A", "cue_id": "c1", "direction": "down", "duration_ms": 30000,
                                               "end_action": "stop", "auto_start": False, "show_on_output": True}}})
telemetry(RS, "mentions", settings=_RS_SET,
          inbound_http={"path": _R + "/mentions", "body": _J({"ok": True, "message": "OK", "data": {"items": [
              {"id": "m1", "name": "Anna", "color": "#2196F3", "description": "Host", "created_at": None,
               "updated_at": None}], "count": 1}, "meta": _META})},
          expect_state={"mentions": {"m1": {"name": "Anna", "color": "#2196F3", "description": "Host"}}})
telemetry(RS, "text-variables", settings=_RS_SET,
          inbound_http={"path": _R + "/text-variables", "body": _J({"ok": True, "message": "OK", "data": {"items": [
              {"key": "guest", "value": "Jane Doe"}, {"key": "show.name", "value": "News"}], "count": 2}, "meta": _META})},
          expect_state={"text_variables": {"guest": "Jane Doe", "show.name": "News"}})
