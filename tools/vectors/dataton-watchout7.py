# Dataton WATCHOUT 7 HTTP API (dataton-watchout7): one vector per command.
# Paths, query parameters and bodies are those of the WATCHOUT 7.8.2 User
# Guide page "HTTP REST API" (jump-to-time ?time=6000&state=play, inputs
# [{"key","value","duration"}], cue-group-state {"Language": "English"},
# hittest {"cues": ["1/42"], "x", "y"} answered {"hit_cues": ["1/42"]}, msc
# [{"command": {"go": {}}}]). Success is assumed to be HTTP 200. The event
# stream's messages follow the Companion module's reading of /v2/sse.
WO7 = "dataton-watchout7"
_OK7 = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}


def _wo7(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": WO7, "command": command, "input": input, "expect_request": request, **extra})


_wo7("get_info", {}, "GET", "/info")
_wo7("get_state", {}, "GET", "/v0/state")
_wo7("get_show", {}, "GET", "/v0/show")
_wo7("get_timelines", {}, "GET", "/v0/timelines")
_wo7("get_cues", {"timeline": 1}, "GET", "/v0/cues/1")
_wo7("get_inputs", {}, "GET", "/v0/inputs")
_wo7("play_timeline", {"timeline": 1}, "POST", "/v0/play/1", **_OK7)
_wo7("pause_timeline", {"timeline": 1}, "POST", "/v0/pause/1")
_wo7("stop_timeline", {"timeline": 3}, "POST", "/v0/stop/3", **_OK7)
_wo7("set_timeline_state", {"timeline": 1, "state": "run"}, "POST", "/v0/play",
     '{"timelineId":1,"state":"run"}', **_OK7)
_wo7("set_timeline_state_at", {"timeline": 1, "state": "pause", "time_ms": 6000}, "POST", "/v0/play",
     '{"timelineId":1,"state":"pause","timelineTime":6000}')
_wo7("jump_to_time", {"timeline": 1, "time_ms": 6000, "state": "play"}, "POST",
     "/v0/jump-to-time/1?time=6000&state=play", **_OK7)
_wo7("jump_to_cue", {"timeline": 1, "cue": 42}, "POST", "/v0/jump-to-cue/1/42?state=pause")
_wo7("set_input", {"key": "dimmer", "value": 0.5, "duration_ms": 1000}, "POST",
     "/v0/input/dimmer?value=0.5000&duration=1000", **_OK7)
_wo7("set_inputs", {"inputs": [{"key": "dimmer", "value": 0.5}, {"key": "speed", "value": 1, "duration": 500}]},
     "POST", "/v0/inputs", '[{"key":"dimmer","value":0.5},{"key":"speed","value":1,"duration":500}]')
_wo7("get_cue_sets_by_id", {}, "GET", "/v0/cue-group-state/by-id")
_wo7("get_cue_sets_by_name", {}, "GET", "/v0/cue-group-state/by-name",
     http_reply={"status": 200, "body": '{"Language":"English"}'},
     expect_result={"ok": {"kind": "value", "value": {"Language": "English"}}})
_wo7("set_cue_set_by_id", {"group": 4, "variant": 2}, "POST", "/v0/cue-group-state/by-id/4/2")
_wo7("set_cue_set_by_name", {"group": "Language", "variant": "Svenska språk"}, "POST",
     "/v0/cue-group-state/by-name/Language/Svenska%20spr%C3%A5k", **_OK7)
_wo7("set_cue_sets_by_id", {"selection": {"4": "2"}}, "POST", "/v0/cue-group-state/by-id", '{"4":"2"}')
_wo7("set_cue_sets_by_name", {"selection": {"Language": "English"}}, "POST", "/v0/cue-group-state/by-name",
     '{"Language":"English"}')
_wo7("reset_cue_sets", {}, "POST", "/v0/cue-group-state/by-name", "{}", **_OK7)
_wo7("hit_test", {"cues": ["1/42"], "x": 960.0, "y": 540.0}, "POST", "/v0/hittest",
     '{"cues":["1/42"],"x":960.00,"y":540.00}',
     http_reply={"status": 200, "body": '{"hit_cues":["1/42"]}'},
     expect_result={"ok": {"kind": "value", "value": ["1/42"]}})
_wo7("send_msc", {"commands": [{"command": {"go": {}}}]}, "POST", "/v0/msc", '[{"command":{"go":{}}}]',
     http_reply={"status": 400, "body": "MSC Cue not found"},
     expect_result={"error": {"error": "device_error", "code": "400"}})
_wo7("load_show", {"show": {"showName": "Demo"}}, "POST", "/v0/show", '{"showName":"Demo"}')

# ── Telemetry: /v2/sse ──
telemetry(WO7, "playback-state", inbound_sse={"data": json.dumps({"kind": "playbackState", "value": {
    "clockTime": 1759400000000, "timelines": [{"id": 1, "playbackStatus": "run"},
                                              {"id": 3, "playbackStatus": "pause"}]}})},
          expect_state={"clock_time_ms": 1759400000000,
                        "timelines": {"1": {"status": "run"}, "3": {"status": "pause"}}})
telemetry(WO7, "countdowns", inbound_sse={"data": json.dumps({"kind": "timelineCountdowns", "value": [
    {"timelineId": 1, "cueId": 42, "delta": 4200, "status": "Last5"}]})},
          expect_state={"countdowns": {"1": {"cue": 42, "delta_ms": 4200, "status": "Last5"}}})
telemetry(WO7, "show-revision", inbound_sse={"data": json.dumps({"kind": "showRevision", "value": "17"})},
          expect_state={"show": {"revision": "17"}})
