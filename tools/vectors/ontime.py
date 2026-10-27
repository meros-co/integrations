# Ontime (ontime): one vector per command over the HTTP API, and telemetry
# vectors for the WebSocket runtime data and the rundown reads. Targets are
# written by hand from Ontime's "HTTP API" page
# (https://docs.getontime.no/api/protocols/http/): every control request is
# a GET answering {"payload":"success"}; the message requests answer the
# message state; the rundown writes are the "Project data" page's POST, PUT
# and DELETE. Runtime messages follow the "Runtime data" and "WebSockets
# API" pages; the values are made up.
ON = "ontime"
_SUCCESS = {"http_reply": {"status": 200, "body": '{"payload":"success"}'},
            "expect_result": {"ok": {"kind": "ack"}}}

http(ON, "get_version", {}, "GET", "/api/version",
     http_reply={"status": 200, "body": '{"payload":"4.0.0"}'},
     expect_result={"ok": {"kind": "value", "value": "4.0.0"}})
http(ON, "poll", {}, "GET", "/api/poll",
     http_reply={"status": 200, "body": '{"payload":{"clock":36000000}}'},
     expect_result={"ok": {"kind": "value", "value": {"clock": 36000000}}})
http(ON, "get_data", {"resource": "custom-fields"}, "GET", "/data/custom-fields",
     http_reply={"status": 200, "body": '{"song":{"label":"Song","type":"text","colour":"#339E4E"}}'},
     expect_result={"ok": {"kind": "value", "value": {"song": {"label": "Song", "type": "text", "colour": "#339E4E"}}}})
http(ON, "get_current_rundown", {}, "GET", "/data/rundowns/current",
     http_reply={"status": 200, "body": '{"id":"default","title":"Show","order":[],"flatOrder":[],"entries":{},"revision":3}'},
     expect_result={"ok": {"kind": "value", "value": {"id": "default", "title": "Show", "order": [],
                                                      "flatOrder": [], "entries": {}, "revision": 3}}})

# Playback.
http(ON, "start", {}, "GET", "/api/start", **_SUCCESS)
http(ON, "start_index", {"index": 2}, "GET", "/api/start/index/2", **_SUCCESS)
http(ON, "start_id", {"id": "32d31"}, "GET", "/api/start/id/32d31", **_SUCCESS)
http(ON, "start_cue", {"cue": "SF1.01"}, "GET", "/api/start/cue/SF1.01",
     http_reply={"status": 400, "body": '{"payload":"Event not found"}'},
     expect_result={"error": {"error": "device_error", "code": "400"}})
http(ON, "start_next", {}, "GET", "/api/start/next", **_SUCCESS)
http(ON, "start_previous", {}, "GET", "/api/start/previous")
http(ON, "pause", {}, "GET", "/api/pause", **_SUCCESS)
http(ON, "load_index", {"index": 1}, "GET", "/api/load/index/1")
http(ON, "load_id", {"id": "32d31"}, "GET", "/api/load/id/32d31")
http(ON, "load_cue", {"cue": "Intro A"}, "GET", "/api/load/cue/Intro%20A")
http(ON, "load_next", {}, "GET", "/api/load/next")
http(ON, "load_previous", {}, "GET", "/api/load/previous")
http(ON, "reload", {}, "GET", "/api/reload")
http(ON, "stop", {}, "GET", "/api/stop", **_SUCCESS)
http(ON, "roll", {}, "GET", "/api/roll")
http(ON, "add_time", {"ms": 60000}, "GET", "/api/addtime/add/60000", **_SUCCESS)
http(ON, "remove_time", {"ms": 30000}, "GET", "/api/addtime/remove/30000")

# Auxiliary timers.
http(ON, "aux_set_duration", {"aux": 1, "ms": 300000}, "GET", "/api/auxtimer/1/duration/300000", **_SUCCESS)
http(ON, "aux_set_direction", {"aux": 2, "direction": "count-up"}, "GET", "/api/auxtimer/2/direction/count-up")
http(ON, "aux_start", {}, "GET", "/api/auxtimer/1/start")
http(ON, "aux_pause", {"aux": 3}, "GET", "/api/auxtimer/3/pause")
http(ON, "aux_stop", {"aux": 1}, "GET", "/api/auxtimer/1/stop")
http(ON, "aux_add_time", {"aux": 1, "ms": -10000}, "GET", "/api/auxtimer/1/addtime/-10000")

# Messages: each answers the message state.
_MSG = '{"payload":{"secondary":"new text","timer":{"text":"","visible":true,"blink":false,"blackout":false,"secondarySource":null}}}'
http(ON, "set_secondary_message", {"text": "new text"}, "GET", "/api/message/secondary/new%20text",
     http_reply={"status": 200, "body": _MSG}, expect_result={"ok": {"kind": "ack"}})
http(ON, "set_secondary_source", {"source": "aux"}, "GET", "/api/message/timer?secondarySource=aux")
http(ON, "set_blackout", {"blackout": True}, "GET", "/api/message/timer?blackout=true")
http(ON, "set_timer_message_text", {"text": "Wrap up"}, "GET", "/api/message/timer?text=Wrap%20up")
http(ON, "set_timer_message_visible", {"visible": False}, "GET", "/api/message/timer?visible=false")
http(ON, "set_timer_message_blink", {"blink": True}, "GET", "/api/message/timer?blink=true")

# Change an event: "GET <localhost:4001>/api/change/<my-event-id>?title=new-title".
http(ON, "set_event_title", {"id": "32d31", "title": "new-title"}, "GET", "/api/change/32d31?title=new-title", **_SUCCESS)
http(ON, "set_event_note", {"id": "32d31", "note": "Camera 2"}, "GET", "/api/change/32d31?note=Camera%202")
http(ON, "set_event_cue", {"id": "32d31", "cue": "SF1.02"}, "GET", "/api/change/32d31?cue=SF1.02")
http(ON, "set_event_colour", {"id": "32d31", "colour": "#FF0000"}, "GET", "/api/change/32d31?colour=%23FF0000")
http(ON, "set_event_skip", {"id": "32d31", "skip": True}, "GET", "/api/change/32d31?skip=true")
http(ON, "set_event_end_action", {"id": "32d31", "end_action": "play-next"}, "GET", "/api/change/32d31?endAction=play-next")
http(ON, "set_event_timer_type", {"id": "32d31", "timer_type": "count-up"}, "GET", "/api/change/32d31?timerType=count-up")
http(ON, "set_event_time_warning", {"id": "32d31", "ms": 600000}, "GET", "/api/change/32d31?timeWarning=600000")
http(ON, "set_event_time_danger", {"id": "32d31", "ms": 300000}, "GET", "/api/change/32d31?timeDanger=300000")
http(ON, "set_event_duration", {"id": "32d31", "ms": 1200000}, "GET", "/api/change/32d31?duration=1200000")
http(ON, "set_event_time_start", {"id": "32d31", "ms": 36000000}, "GET", "/api/change/32d31?timeStart=36000000")
http(ON, "set_event_time_end", {"id": "32d31", "ms": 37200000}, "GET", "/api/change/32d31?timeEnd=37200000")
# "GET <localhost:4001>/api/change/<my-event-id>?custom:<field-name>=new-value"
http(ON, "set_event_custom_field", {"id": "32d31", "field": "song", "value": "Sekret"}, "GET",
     "/api/change/32d31?custom:song=Sekret", **_SUCCESS)

# Rundown entries.
V.append({"spec": ON, "command": "create_entry",
          "input": {"rundown": "default", "body": {"type": "event", "title": "Albania", "after": "32d31"}},
          "expect_request": {"method": "POST", "target": "/data/rundowns/default/entry",
                             "body": '{"type":"event","title":"Albania","after":"32d31"}'},
          "http_reply": {"status": 201, "body": '{"id":"a1b2c","type":"event","title":"Albania"}'},
          "expect_result": {"ok": {"kind": "ack"}}})
V.append({"spec": ON, "command": "update_entry",
          "input": {"rundown": "default", "entry": "32d31", "body": {"id": "32d31", "title": "Albania"}},
          "expect_request": {"method": "PUT", "target": "/data/rundowns/default/32d31",
                             "body": '{"id":"32d31","title":"Albania"}'}})
V.append({"spec": ON, "command": "delete_entry", "input": {"rundown": "default", "entry": "32d31"},
          "expect_request": {"method": "DELETE", "target": "/data/rundowns/default/32d31",
                             "body": '{"ids":["32d31"]}'},
          "http_reply": {"status": 204, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}})

# Telemetry: the WebSocket.
_J = lambda d: json.dumps(d, separators=(",", ":"))
telemetry(ON, "connect", inbound_ws='{"tag":"version","payload":"4.0.0"}',
          expect_connect_ws=['{"tag":"version"}', '{"tag":"poll"}'],
          expect_state={"app": {"version": "4.0.0"}})
_RUNTIME = {
    "clock": 36061000, "onAir": True,
    "timer": {"addedTime": 0, "current": 1140000, "duration": 1200000, "elapsed": 60000, "playback": "play",
              "phase": "default", "secondaryTimer": None, "startedAt": 36001000, "expectedFinish": 37201000,
              "finishedAt": None},
    "runtime": {"numEvents": 12, "selectedEventIndex": 0, "plannedStart": 36000000, "plannedEnd": 43200000,
                "actualStart": 36001000, "currentDay": 0, "actualGroupStart": None, "offset": -1000,
                "expectedEnd": 43201000},
    "offset": {"absolute": -1000, "relative": 0, "mode": "absolute", "expectedGroupEnd": None,
               "expectedRundownEnd": 43201000, "expectedFlagStart": None},
    "message": {"secondary": "Next: Q&A", "timer": {"text": "Wrap up", "visible": False, "blink": False,
                                                     "blackout": False, "secondarySource": "aux1"}},
    "auxtimer1": {"duration": 300000, "current": 240000, "playback": "play", "direction": "count-down"},
    "auxtimer2": {"duration": 0, "current": 0, "playback": "stop", "direction": "count-up"},
    "auxtimer3": {"duration": 60000, "current": 60000, "playback": "pause", "direction": "count-down"},
    "eventNow": {"id": "32d31", "type": "event", "title": "Albania", "cue": "SF1.01", "note": "SF1.01",
                 "colour": "", "timeStart": 36000000, "timeEnd": 37200000, "duration": 1200000, "delay": 0,
                 "skip": False, "endAction": "none", "timerType": "count-down", "timeWarning": 600000,
                 "timeDanger": 300000, "custom": {"song": "Sekret", "artist": "Ronela Hajati"}},
    "eventNext": None,
}
_RUNTIME_STATE = {
    "clock": 36061000, "on_air": True,
    "timer": {"added_time": 0, "current": 1140000, "duration": 1200000, "elapsed": 60000, "playback": "play",
              "phase": "default", "started_at": 36001000, "expected_finish": 37201000},
    "runtime": {"num_events": 12, "selected_event_index": 0, "planned_start": 36000000, "planned_end": 43200000,
                "actual_start": 36001000, "current_day": 0, "offset": -1000, "expected_end": 43201000},
    "offset": {"absolute": -1000, "relative": 0, "mode": "absolute", "expected_rundown_end": 43201000},
    "message": {"secondary": "Next: Q&A", "timer": {"text": "Wrap up", "visible": False, "blink": False,
                                                     "blackout": False, "secondary_source": "aux1"}},
    "aux_timers": {"1": {"duration": 300000, "current": 240000, "playback": "play", "direction": "count-down"},
                   "2": {"duration": 0, "current": 0, "playback": "stop", "direction": "count-up"},
                   "3": {"duration": 60000, "current": 60000, "playback": "pause", "direction": "count-down"}},
    "event_now": {"id": "32d31", "type": "event", "title": "Albania", "cue": "SF1.01", "note": "SF1.01",
                  "colour": "", "time_start": 36000000, "time_end": 37200000, "duration": 1200000, "delay": 0,
                  "skip": False, "end_action": "none", "timer_type": "count-down", "time_warning": 600000,
                  "time_danger": 300000, "custom": {"song": "Sekret", "artist": "Ronela Hajati"}},
}
telemetry(ON, "poll-reply", inbound_ws=_J({"tag": "poll", "payload": _RUNTIME}), expect_state=_RUNTIME_STATE)
telemetry(ON, "ontime-broadcast", inbound_ws=_J({"type": "ontime", "payload": _RUNTIME}), expect_state=_RUNTIME_STATE)
# No event loaded: eventNow null empties the section, the timer's nulls leave state.
telemetry(ON, "event-now-cleared",
          state_before={"event_now": {"id": "32d31", "title": "Albania", "custom": {"song": "Sekret"}},
                        "timer": {"current": 1140000, "playback": "play"}},
          inbound_ws=_J({"type": "ontime", "payload": {
              "eventNow": None,
              "timer": {"addedTime": 0, "current": None, "duration": None, "elapsed": None,
                        "playback": "stop", "startedAt": None, "expectedFinish": None, "finishedAt": None}}}),
          expect_state={"timer": {"added_time": 0, "playback": "stop"}})
# Granular section messages.
telemetry(ON, "section-clock", inbound_ws='{"type":"ontime-clock","payload":36062000}', expect_state={"clock": 36062000})
telemetry(ON, "section-timer", inbound_ws=_J({"type": "ontime-timer", "payload": {
              "addedTime": 60000, "current": -5000, "duration": 1200000, "elapsed": 1265000, "playback": "play",
              "startedAt": 36001000, "expectedFinish": 37261000, "finishedAt": 37261000}}),
          expect_state={"timer": {"added_time": 60000, "current": -5000, "duration": 1200000, "elapsed": 1265000,
                                  "playback": "play", "started_at": 36001000, "expected_finish": 37261000,
                                  "finished_at": 37261000}})
telemetry(ON, "section-rundown", inbound_ws=_J({"type": "ontime-rundown", "payload": {
              "numEvents": 3, "selectedEventIndex": 1, "plannedStart": 0, "plannedEnd": 3600000,
              "actualStart": None, "currentDay": None, "actualGroupStart": None, "offset": None,
              "expectedEnd": None}}),
          expect_state={"runtime": {"num_events": 3, "selected_event_index": 1, "planned_start": 0,
                                    "planned_end": 3600000}})
telemetry(ON, "section-offset", inbound_ws=_J({"type": "ontime-offset", "payload": {
              "absolute": 5000, "relative": 2000, "mode": "relative", "expectedGroupEnd": 40000000,
              "expectedRundownEnd": 43205000, "expectedFlagStart": 39000000}}),
          expect_state={"offset": {"absolute": 5000, "relative": 2000, "mode": "relative",
                                   "expected_group_end": 40000000, "expected_rundown_end": 43205000,
                                   "expected_flag_start": 39000000}})
telemetry(ON, "section-message",
          state_before={"message": {"timer": {"secondary_source": "aux1"}}},
          inbound_ws=_J({"type": "ontime-message", "payload": {
              "secondary": "", "timer": {"text": "Lunch", "visible": True, "blink": True, "blackout": False,
                                         "secondarySource": None}}}),
          expect_state={"message": {"secondary": "", "timer": {"text": "Lunch", "visible": True, "blink": True,
                                                               "blackout": False}}})
telemetry(ON, "section-auxtimer", inbound_ws=_J({"type": "ontime-auxtimer2", "payload": {
              "duration": 90000, "current": 45000, "playback": "pause", "direction": "count-down"}}),
          expect_state={"aux_timers": {"2": {"duration": 90000, "current": 45000, "playback": "pause",
                                             "direction": "count-down"}}})
telemetry(ON, "section-event-now", inbound_ws=_J({"type": "ontime-eventNow", "payload": {
              "id": "a1", "type": "event", "title": "Keynote", "cue": "1", "skip": False,
              "custom": {"mic": "HH1"}}}),
          expect_state={"event_now": {"id": "a1", "type": "event", "title": "Keynote", "cue": "1", "skip": False,
                                      "custom": {"mic": "HH1"}}})
telemetry(ON, "section-event-next", inbound_ws=_J({"type": "ontime-eventNext", "payload": {
              "id": "a2", "type": "event", "title": "Panel", "duration": 1800000, "custom": {"mic": "HH2"}}}),
          expect_state={"event_next": {"id": "a2", "type": "event", "title": "Panel", "duration": 1800000,
                                       "custom": {"mic": "HH2"}}})
telemetry(ON, "section-event-next-none",
          state_before={"event_next": {"id": "a2", "title": "Panel"}},
          inbound_ws='{"type":"ontime-eventNext","payload":null}', expect_state={})

# Telemetry: HTTP replies.
telemetry(ON, "http-version", inbound_http={"path": "/api/version", "body": '{"payload":"4.0.0"}'},
          expect_state={"app": {"version": "4.0.0"}})
telemetry(ON, "http-message", inbound_http={"path": "/api/message/timer?blackout=true", "body": _J({"payload": {
              "secondary": "", "timer": {"text": "", "visible": True, "blink": False, "blackout": True,
                                         "secondarySource": "secondary"}}})},
          expect_state={"message": {"secondary": "", "timer": {"text": "", "visible": True, "blink": False,
                                                               "blackout": True, "secondary_source": "secondary"}}})
telemetry(ON, "rundown",
          state_before={"rundown": {"entries": {"gone": {"title": "Removed"}}}},
          inbound_http={"path": "/data/rundowns/current", "body": _J({
              "id": "default", "title": "Eurovision", "order": ["32d31", "g1"], "flatOrder": ["32d31", "g1", "b2"],
              "revision": 7,
              "entries": {
                  "32d31": {"id": "32d31", "type": "event", "title": "Albania", "cue": "SF1.01", "note": "SF1.01",
                            "colour": "", "timeStart": 36000000, "timeEnd": 37200000, "duration": 1200000,
                            "delay": 0, "skip": False, "linkStart": False, "timeDanger": 300000,
                            "timeWarning": 600000, "endAction": "none", "timerType": "count-down",
                            "countToEnd": False, "timeStrategy": "lock-duration", "revision": 0,
                            "custom": {"song": "Sekret", "artist": "Ronela Hajati"}},
                  "g1": {"id": "g1", "type": "group", "title": "Semi final", "colour": "#779BE7"},
                  "b2": {"id": "b2", "type": "delay", "duration": 60000}}})},
          expect_state={"rundown": {"id": "default", "title": "Eurovision", "revision": 7, "entries": {
              "32d31": {"index": 0, "type": "event", "title": "Albania", "cue": "SF1.01", "note": "SF1.01",
                        "colour": "", "time_start": 36000000, "time_end": 37200000, "duration": 1200000,
                        "delay": 0, "skip": False, "link_start": False, "time_danger": 300000,
                        "time_warning": 600000, "end_action": "none", "timer_type": "count-down",
                        "count_to_end": False, "time_strategy": "lock-duration", "revision": 0,
                        "custom": {"song": "Sekret", "artist": "Ronela Hajati"}},
              "g1": {"index": 1, "type": "group", "title": "Semi final", "colour": "#779BE7"},
              "b2": {"index": 2, "type": "delay", "duration": 60000}}}})
telemetry(ON, "custom-fields",
          state_before={"custom_fields": {"old": {"label": "Old"}}},
          inbound_http={"path": "/data/custom-fields", "body": _J({
              "song": {"label": "Song", "type": "text", "colour": "#339E4E"},
              "artist": {"label": "Artist", "type": "text", "colour": ""}})},
          expect_state={"custom_fields": {"song": {"label": "Song", "type": "text", "colour": "#339E4E"},
                                          "artist": {"label": "Artist", "type": "text", "colour": ""}}})
telemetry(ON, "change-rereads-rundown",
          inbound_http={"path": "/api/change/32d31?title=new-title", "body": '{"payload":"success"}'},
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/data/rundowns/current"}])
