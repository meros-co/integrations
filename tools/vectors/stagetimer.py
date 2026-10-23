# Stagetimer (stagetimer): one vector per command over API v1, and
# telemetry vectors for the Socket.IO events and the HTTP replies. Targets
# are written by hand from the "API Documentation, API Version 1" page
# (https://stagetimer.io/docs/api-v1/): every endpoint a GET under /v1/ with
# room_id first and the other query parameters after it, the key sent as a
# bearer token. Responses are the page's examples; the room ID 6ADP4P5U and
# the IDs are the page's own. Every vector runs against the engine's
# loopback fake: none reaches stagetimer.io.
ST = "stagetimer"
_ST_SET = {"room_id": "6ADP4P5U", "token": "test-key"}
_R = "room_id=6ADP4P5U"
_STATUS = {"_model": "playback_status", "_updated_at": "2023-05-01T08:40:06.463Z",
           "timer_id": "63f59467d68bfdeef6b3bfc3", "running": False, "start": 1677055206453,
           "finish": 1677055206453, "pause": 1677055206453, "server_time": 1677055206453}
_TIMER = {"_id": "63f59467d68bfdeef6b3bfc3", "_model": "timer", "_updated_at": "2023-05-01T08:40:06.463Z",
          "name": "Timer 1", "speaker": "Thomas", "notes": "Very tall, remember to adjust camera angle",
          "labels": [{"name": "VT", "color": "#F44336"}], "appearance": "COUNTDOWN", "type": "DURATION",
          "duration": "0:01:00", "hours": 0, "minutes": 1, "seconds": 30, "wrap_up_yellow": 60,
          "wrap_up_red": 15, "trigger": "MANUAL", "start_time": "2023-05-01T08:40:06.463Z",
          "start_time_uses_date": False, "finish_time": "2023-05-01T08:40:06.463Z",
          "finish_time_uses_date": False}
_MESSAGE = {"_id": "e5f46622be82fdaa44bba54c", "_model": "message", "_updated_at": "2023-05-01T08:40:06.463Z",
            "showing": False, "text": "Question for the speaker: [...]", "color": "white", "bold": False,
            "uppercase": False}
_J = lambda d: json.dumps(d, separators=(",", ":"))
_OK_STATUS = {"http_reply": {"status": 200, "body": _J({"ok": True, "message": "Timer started at 0:01:00",
                                                        "data": _STATUS})},
              "expect_result": {"ok": {"kind": "ack"}}}


def _st(command, input, target, **extra):
    http(ST, command, input, "GET", target, settings=_ST_SET, **extra)


_st("test_auth", {}, "/v1/test_auth?" + _R,
    http_reply={"status": 200, "body": '{"ok":true,"message":"You are authorized to use the Stagetimer API for room: 6ADP4P5U"}'},
    expect_result={"ok": {"kind": "value", "value": "You are authorized to use the Stagetimer API for room: 6ADP4P5U"}})
_st("get_status", {}, "/v1/get_status?" + _R,
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Playback status loaded", "data": _STATUS})},
    expect_result={"ok": {"kind": "value", "value": _STATUS}})
_st("get_room", {}, "/v1/get_room?" + _R)
_st("get_logs", {}, "/v1/get_logs?" + _R + "&limit=20&offset=0",
    http_reply={"status": 200, "body": '{"ok":true,"message":"Logs loaded","data":["2023-05-01T08:40:06.463Z | Device YCB7 | Added `1m` to timer. New value: `0:03:00`"]}'},
    expect_result={"ok": {"kind": "value", "value": ["2023-05-01T08:40:06.463Z | Device YCB7 | Added `1m` to timer. New value: `0:03:00`"]}})

# Transport.
_st("start", {}, "/v1/start?" + _R, **_OK_STATUS)
_st("stop", {}, "/v1/stop?" + _R)
_st("start_or_stop", {}, "/v1/start_or_stop?" + _R)
_st("reset", {"autostart": True}, "/v1/reset?" + _R + "&autostart=true")
_st("start_fresh", {}, "/v1/start_fresh?" + _R, **_OK_STATUS)
_st("next", {}, "/v1/next?" + _R + "&autostart=false&from_memory=false")
_st("previous", {"autostart": True, "from_memory": True}, "/v1/previous?" + _R + "&autostart=true&from_memory=true")
_st("add_time", {"milliseconds": 30000}, "/v1/add_time?" + _R + "&milliseconds=30000", **_OK_STATUS)
_st("add_time_amount", {"amount": "30s"}, "/v1/add_time?" + _R + "&amount=30s")
_st("subtract_time", {"milliseconds": 30000}, "/v1/subtract_time?" + _R + "&milliseconds=30000")
_st("subtract_time_amount", {"amount": "10m"}, "/v1/subtract_time?" + _R + "&amount=10m")
_st("jump", {"milliseconds": -30000}, "/v1/jump?" + _R + "&milliseconds=-30000")

# Viewer.
_st("start_flashing", {"count": 5}, "/v1/start_flashing?" + _R + "&count=5",
    http_reply={"status": 200, "body": '{"ok":true,"message":"Triggered flashing (\\u00d75)"}'},
    expect_result={"ok": {"kind": "ack"}})
_st("stop_flashing", {}, "/v1/stop_flashing?" + _R)
_st("enable_blackout", {}, "/v1/enable_blackout?" + _R)
_st("disable_blackout", {}, "/v1/disable_blackout?" + _R)
_st("toggle_blackout", {}, "/v1/toggle_blackout?" + _R)
_st("enable_focus", {}, "/v1/enable_focus?" + _R)
_st("disable_focus", {}, "/v1/disable_focus?" + _R)
_st("toggle_focus", {}, "/v1/toggle_focus?" + _R)
_st("enable_on_air", {}, "/v1/enable_on_air?" + _R)
_st("disable_on_air", {}, "/v1/disable_on_air?" + _R)
_st("toggle_on_air", {}, "/v1/toggle_on_air?" + _R,
    http_reply={"status": 401, "body": '{"ok":false,"message":"Access Denied: Valid API key required"}'},
    expect_result={"error": {"error": "auth"}})

# Timers.
_TID = "63f59467d68bfdeef6b3bfc3"
_st("start_timer", {"timer_id": _TID}, f"/v1/start_timer?{_R}&timer_id={_TID}&from_memory=false", **_OK_STATUS)
_st("start_timer_index", {"index": 2, "from_memory": True}, f"/v1/start_timer?{_R}&index=2&from_memory=true")
_st("stop_timer", {"timer_id": _TID}, f"/v1/stop_timer?{_R}&timer_id={_TID}")
_st("stop_timer_index", {"index": 2}, f"/v1/stop_timer?{_R}&index=2")
_st("start_or_stop_timer", {"timer_id": _TID}, f"/v1/start_or_stop_timer?{_R}&timer_id={_TID}&from_memory=false")
_st("start_or_stop_timer_index", {"index": 1}, f"/v1/start_or_stop_timer?{_R}&index=1&from_memory=false")
_st("select_timer", {"timer_id": _TID}, f"/v1/select_timer?{_R}&timer_id={_TID}")
_st("select_timer_index", {"index": 3}, f"/v1/select_timer?{_R}&index=3")
_st("reset_timer", {"timer_id": _TID}, f"/v1/reset_timer?{_R}&timer_id={_TID}")
_st("reset_timer_index", {"index": 3}, f"/v1/reset_timer?{_R}&index=3",
    http_reply={"status": 404, "body": '{"ok":false,"message":"Timer not found"}'},
    expect_result={"error": {"error": "device_error", "code": "404"}})
_st("get_all_timers", {}, "/v1/get_all_timers?" + _R,
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Timers loaded", "data": [_TIMER]})},
    expect_result={"ok": {"kind": "value", "value": [_TIMER]}})
_st("get_timer", {"timer_id": _TID}, f"/v1/get_timer?{_R}&timer_id={_TID}",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Timer loaded", "data": _TIMER})},
    expect_result={"ok": {"kind": "value", "value": _TIMER}})
_st("get_timer_index", {"index": 2}, f"/v1/get_timer?{_R}&index=2")
_st("create_timer", {"name": "Keynote", "speaker": "Thomas", "minutes": 20},
    f"/v1/create_timer?{_R}&name=Keynote&speaker=Thomas&notes=&appearance=COUNTDOWN&type=DURATION"
    "&hours=0&minutes=20&seconds=0&wrap_up_yellow=60&wrap_up_red=15&trigger=MANUAL",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Timer created", "data": _TIMER})},
    expect_result={"ok": {"kind": "value", "value": _TID}})
_st("set_timer_name", {"timer_id": _TID, "name": "Opening remarks"}, f"/v1/update_timer?{_R}&timer_id={_TID}&name=Opening%20remarks")
_st("set_timer_speaker", {"timer_id": _TID, "speaker": ""}, f"/v1/update_timer?{_R}&timer_id={_TID}&speaker=")
_st("set_timer_notes", {"timer_id": _TID, "notes": "Wireless mic 2"}, f"/v1/update_timer?{_R}&timer_id={_TID}&notes=Wireless%20mic%202")
_st("set_timer_appearance", {"timer_id": _TID, "appearance": "COUNTUP_TOD"}, f"/v1/update_timer?{_R}&timer_id={_TID}&appearance=COUNTUP_TOD")
_st("set_timer_type", {"timer_id": _TID, "type": "FINISH_TIME"}, f"/v1/update_timer?{_R}&timer_id={_TID}&type=FINISH_TIME")
_st("set_timer_duration", {"timer_id": _TID, "hours": 0, "minutes": 1, "seconds": 30},
    f"/v1/update_timer?{_R}&timer_id={_TID}&hours=0&minutes=1&seconds=30",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Timer updated", "data": _TIMER})},
    expect_result={"ok": {"kind": "ack"}})
_st("set_timer_wrap_up", {"timer_id": _TID, "wrap_up_yellow": 30, "wrap_up_red": 10},
    f"/v1/update_timer?{_R}&timer_id={_TID}&wrap_up_yellow=30&wrap_up_red=10")
_st("set_timer_trigger", {"timer_id": _TID, "trigger": "SCHEDULED"}, f"/v1/update_timer?{_R}&timer_id={_TID}&trigger=SCHEDULED")
_st("set_timer_start_time", {"timer_id": _TID, "start_time": "2023-05-01T08:40:06.463Z", "uses_date": True},
    f"/v1/update_timer?{_R}&timer_id={_TID}&start_time=2023-05-01T08%3A40%3A06.463Z&start_time_uses_date=true")
_st("set_timer_finish_time", {"timer_id": _TID, "finish_time": "2023-05-01T09:00:00.000Z"},
    f"/v1/update_timer?{_R}&timer_id={_TID}&finish_time=2023-05-01T09%3A00%3A00.000Z")
# "?labels[0][name]=VT&labels[0][color]=%23F44336"
_st("set_timer_label", {"timer_id": _TID, "name": "VT", "color": "#F44336"},
    f"/v1/update_timer?{_R}&timer_id={_TID}&labels%5B0%5D%5Bname%5D=VT&labels%5B0%5D%5Bcolor%5D=%23F44336")
_st("delete_timer", {"timer_id": _TID}, f"/v1/delete_timer?{_R}&timer_id={_TID}",
    http_reply={"status": 200, "body": '{"ok":true,"message":"Timer deleted"}'},
    expect_result={"ok": {"kind": "ack"}})
_st("delete_timer_index", {"index": 4}, f"/v1/delete_timer?{_R}&index=4")

# Messages.
_MID = "e5f46622be82fdaa44bba54c"
_st("show_message", {"message_id": _MID, "focus": True}, f"/v1/show_message?{_R}&message_id={_MID}&focus=true",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Message showing", "data": _MESSAGE})},
    expect_result={"ok": {"kind": "ack"}})
_st("show_message_index", {"index": 1}, f"/v1/show_message?{_R}&index=1&focus=false")
_st("hide_message", {"message_id": _MID}, f"/v1/hide_message?{_R}&message_id={_MID}")
_st("hide_message_index", {"index": 1}, f"/v1/hide_message?{_R}&index=1")
_st("show_or_hide_message", {"message_id": _MID}, f"/v1/show_or_hide_message?{_R}&message_id={_MID}&focus=false")
_st("show_or_hide_message_index", {"index": 2, "focus": True}, f"/v1/show_or_hide_message?{_R}&index=2&focus=true")
_st("get_all_messages", {}, "/v1/get_all_messages?" + _R,
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Messages loaded", "data": {"messages": [_MESSAGE]}})},
    expect_result={"ok": {"kind": "value", "value": [_MESSAGE]}})
_st("get_message", {"message_id": _MID}, f"/v1/get_message?{_R}&message_id={_MID}")
_st("get_message_index", {"index": 1}, f"/v1/get_message?{_R}&index=1")
_st("create_message", {"text": "Wrap up!", "color": "red", "bold": True},
    f"/v1/create_message?{_R}&text=Wrap%20up%21&color=red&bold=true&uppercase=false",
    http_reply={"status": 200, "body": _J({"ok": True, "message": "Message created", "data": _MESSAGE})},
    expect_result={"ok": {"kind": "value", "value": _MID}})
_st("set_message_text", {"message_id": _MID, "text": "5 minutes"}, f"/v1/update_message?{_R}&message_id={_MID}&text=5%20minutes")
_st("set_message_color", {"message_id": _MID, "color": "green"}, f"/v1/update_message?{_R}&message_id={_MID}&color=green")
_st("set_message_bold", {"message_id": _MID, "bold": False}, f"/v1/update_message?{_R}&message_id={_MID}&bold=false")
_st("set_message_uppercase", {"message_id": _MID, "uppercase": True}, f"/v1/update_message?{_R}&message_id={_MID}&uppercase=true")
_st("delete_message", {"message_id": _MID}, f"/v1/delete_message?{_R}&message_id={_MID}")
_st("delete_message_index", {"index": 1}, f"/v1/delete_message?{_R}&index=1")
_st("get_output_links", {"short_url": True}, f"/v1/get_output_links?{_R}&short_url=true&hide_controls=false&delay=0")

# Telemetry: Socket.IO events ("42" + JSON array), the page's examples.
_E = lambda name, data: "42" + _J([name, data])
telemetry(ST, "playback-status", settings=_ST_SET,
          inbound_ws=_E("playback_status", {"_model": "playback_status", "_updated_at": "2026-02-19T09:11:30.845Z",
                                            "timer_id": "63a422789477ef3e82c3597b", "running": False,
                                            "start": 1771165298937, "finish": 1771165658937,
                                            "pause": 1771165298937, "server_time": 1771492290845}),
          expect_state={"playback": {"timer_id": "63a422789477ef3e82c3597b", "running": False,
                                     "start": 1771165298937, "finish": 1771165658937, "pause": 1771165298937,
                                     "server_time": 1771492290845, "updated_at": "2026-02-19T09:11:30.845Z"}})
telemetry(ST, "room", settings=_ST_SET,
          inbound_ws=_E("room", {"_id": "4PS785DA", "_model": "room", "_updated_at": "2026-02-15T14:21:38.938Z",
                                 "name": "Demo: Pawnee Townhall", "date": "2026-02-15", "blackout": False,
                                 "focus_message": False, "on_air": False,
                                 "logo": "https://stagetimer.io/spa-assets/logo-dark.png",
                                 "timezone": "America/Los_Angeles"}),
          expect_state={"room": {"name": "Demo: Pawnee Townhall", "date": "2026-02-15", "blackout": False,
                                 "focus_message": False, "on_air": False,
                                 "logo": "https://stagetimer.io/spa-assets/logo-dark.png",
                                 "timezone": "America/Los_Angeles"}})
telemetry(ST, "room-without-date", settings=_ST_SET, state_before={"room": {"date": "2026-02-15"}},
          inbound_ws=_E("room", {"_id": "4PS785DA", "_model": "room", "name": "Main", "date": None,
                                 "blackout": True, "on_air": True}),
          expect_state={"room": {"name": "Main", "blackout": True, "on_air": True}})
telemetry(ST, "message", settings=_ST_SET,
          inbound_ws=_E("message", {"_id": "638368e354cc641dcd3f1fe1", "_model": "message",
                                    "_updated_at": "2023-06-15T20:25:34.539Z", "showing": True, "text": "Wrap up!",
                                    "color": "red", "bold": False, "uppercase": False}),
          expect_state={"messages": {"638368e354cc641dcd3f1fe1": {"showing": True, "text": "Wrap up!", "color": "red",
                                                                  "bold": False, "uppercase": False}},
                        "active_message": {"id": "638368e354cc641dcd3f1fe1", "showing": True}})
_CT = {"_id": "63a422789477ef3e82c3597b", "_model": "timer", "_updated_at": "2026-02-03T09:12:50.631Z",
       "name": "Introduction", "speaker": "Tom", "notes": "Don't forget to mention the mayor",
       "labels": [{"name": "VT", "color": "#F44336"}, {"name": "MUSIC", "color": "#2196F3"}],
       "appearance": "COUNTDOWN", "type": "DURATION", "duration": "0:06:00", "hours": 0, "minutes": 6,
       "seconds": 0, "wrap_up_yellow": 30, "wrap_up_red": 7.5, "trigger": "MANUAL",
       "start_time": "2026-02-15T10:30:00.000Z", "start_time_uses_date": True, "finish_time": None,
       "finish_time_uses_date": False}
_CT_STATE = {"name": "Introduction", "speaker": "Tom", "notes": "Don't forget to mention the mayor",
             "labels": {"1": {"name": "VT", "color": "#F44336"}, "2": {"name": "MUSIC", "color": "#2196F3"}},
             "appearance": "COUNTDOWN", "type": "DURATION", "duration": "0:06:00", "hours": 0, "minutes": 6,
             "seconds": 0, "wrap_up_yellow": 30.0, "wrap_up_red": 7.5, "trigger": "MANUAL",
             "start_time": "2026-02-15T10:30:00.000Z"}
telemetry(ST, "current-timer", settings=_ST_SET, inbound_ws=_E("current_timer", _CT),
          expect_state={"current_timer": {"id": "63a422789477ef3e82c3597b"},
                        "timers": {"63a422789477ef3e82c3597b": _CT_STATE}})
telemetry(ST, "next-timer-none", settings=_ST_SET, state_before={"next_timer": {"id": "x"}},
          inbound_ws=_E("next_timer", None), expect_state={})
telemetry(ST, "flash", settings=_ST_SET, inbound_ws=_E("flash", {"count": 3}), expect_state={"flash": {"count": 3}})
telemetry(ST, "timers-changed", settings=_ST_SET,
          inbound_ws=_E("timers_changed", [{"timer_id": "63a422789477ef3e82c3597b", "change": "updated"}]),
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/v1/get_all_timers?" + _R}])
telemetry(ST, "messages-changed", settings=_ST_SET,
          inbound_ws=_E("messages_changed", [{"message_id": "638368e354cc641dcd3f1fe1", "change": "deleted"}]),
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/v1/get_all_messages?" + _R}])

# Telemetry: HTTP replies.
telemetry(ST, "http-status", settings=_ST_SET,
          inbound_http={"path": "/v1/start?" + _R, "body": _J({"ok": True, "message": "Timer started", "data": _STATUS})},
          expect_state={"playback": {"timer_id": _TID, "running": False, "start": 1677055206453,
                                     "finish": 1677055206453, "pause": 1677055206453,
                                     "server_time": 1677055206453, "updated_at": "2023-05-01T08:40:06.463Z"}})
telemetry(ST, "http-room", settings=_ST_SET,
          inbound_http={"path": "/v1/get_room?" + _R, "body": _J({"ok": True, "message": "Room loaded", "data": {
              "_id": "6ADP4P5U", "_model": "room", "_updated_at": "2023-05-01T08:40:06.463Z", "name": "Main stage",
              "blackout": False, "focus_message": False, "on_air": False,
              "logo": "https://stagetimer.io/spa-assets/logo-dark.png", "timezone": "Europe/Berlin",
              "date": "2023-05-01"}})},
          expect_state={"room": {"name": "Main stage", "blackout": False, "focus_message": False, "on_air": False,
                                 "logo": "https://stagetimer.io/spa-assets/logo-dark.png",
                                 "timezone": "Europe/Berlin", "date": "2023-05-01"}})
_TIMER_STATE = {"name": "Timer 1", "speaker": "Thomas", "notes": "Very tall, remember to adjust camera angle",
                "labels": {"1": {"name": "VT", "color": "#F44336"}}, "appearance": "COUNTDOWN",
                "type": "DURATION", "duration": "0:01:00", "hours": 0, "minutes": 1, "seconds": 30,
                "wrap_up_yellow": 60.0, "wrap_up_red": 15.0, "trigger": "MANUAL",
                "start_time": "2023-05-01T08:40:06.463Z", "finish_time": "2023-05-01T08:40:06.463Z"}
telemetry(ST, "http-timer", settings=_ST_SET,
          state_before={"timers": {_TID: {"labels": {"1": {"name": "Old", "color": "#000000"},
                                                     "2": {"name": "Gone", "color": "#FFFFFF"}}}}},
          inbound_http={"path": f"/v1/update_timer?{_R}&timer_id={_TID}&name=Timer%201",
                        "body": _J({"ok": True, "message": "Timer updated", "data": _TIMER})},
          expect_state={"timers": {_TID: _TIMER_STATE}})
telemetry(ST, "http-message", settings=_ST_SET,
          inbound_http={"path": f"/v1/show_message?{_R}&index=1&focus=false",
                        "body": _J({"ok": True, "message": "Message showing", "data": _MESSAGE})},
          expect_state={"messages": {_MID: {"showing": False, "text": "Question for the speaker: [...]",
                                            "color": "white", "bold": False, "uppercase": False}}})
telemetry(ST, "all-timers", settings=_ST_SET,
          state_before={"timers": {"gone": {"name": "Removed"}}},
          inbound_http={"path": "/v1/get_all_timers?" + _R,
                        "body": _J({"ok": True, "message": "Timers loaded", "data": [
                            _TIMER, {"_id": "t2", "_model": "timer", "name": "Break", "labels": [],
                                     "appearance": "TOD", "trigger": "LINKED", "start_time": None,
                                     "finish_time": None}]})},
          expect_state={"timers": {_TID: {"index": 1, **_TIMER_STATE},
                                   "t2": {"index": 2, "name": "Break", "appearance": "TOD", "trigger": "LINKED"}}})
telemetry(ST, "all-messages", settings=_ST_SET,
          state_before={"messages": {"gone": {"text": "Removed"}}},
          inbound_http={"path": "/v1/get_all_messages?" + _R,
                        "body": _J({"ok": True, "message": "Messages loaded", "data": {"messages": [_MESSAGE]}})},
          expect_state={"messages": {_MID: {"index": 1, "showing": False, "text": "Question for the speaker: [...]",
                                            "color": "white", "bold": False, "uppercase": False}}})
telemetry(ST, "output-links", settings=_ST_SET,
          inbound_http={"path": f"/v1/get_output_links?{_R}&short_url=false&hide_controls=false&delay=0",
                        "body": _J({"ok": True, "message": "Output links generated", "data": [
                            {"id": _TID, "name": "Main Display", "index": 1,
                             "url": "https://stagetimer.io/output/63f59467d68bfdeef6b3bfc3/?v=2&signature=abc123"}]})},
          expect_state={"outputs": {_TID: {"name": "Main Display", "index": 1,
                                           "url": "https://stagetimer.io/output/63f59467d68bfdeef6b3bfc3/?v=2&signature=abc123"}}})
telemetry(ST, "logs", settings=_ST_SET,
          inbound_http={"path": f"/v1/get_logs?{_R}&limit=20&offset=0",
                        "body": '{"ok":true,"message":"Logs loaded","data":["2023-05-01T08:40:06.463Z | Device YCB7 | Added `1m` to timer. New value: `0:03:00`"]}'},
          expect_state={"logs": {"1": "2023-05-01T08:40:06.463Z | Device YCB7 | Added `1m` to timer. New value: `0:03:00`"}})
telemetry(ST, "timer-deleted", settings=_ST_SET,
          inbound_http={"path": f"/v1/delete_timer?{_R}&timer_id={_TID}", "body": '{"ok":true,"message":"Timer deleted"}'},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/v1/get_all_timers?" + _R}])
telemetry(ST, "message-deleted", settings=_ST_SET,
          inbound_http={"path": f"/v1/delete_message?{_R}&index=1", "body": '{"ok":true,"message":"Message deleted"}'},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/v1/get_all_messages?" + _R}])
