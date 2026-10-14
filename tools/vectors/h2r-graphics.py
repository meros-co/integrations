# H2R Graphics (h2r-graphics): one vector per command, over the HTTP API.
# Targets are written by hand from H2R's "API > HTTP" page
# (https://h2r.graphics/docs/api/http/): every request is a POST to
# http://<host>:4001/api/<project-id>/..., with ABCD the project id of a new
# installation. The HTTP Listener target is /data/<source-id>
# (https://h2r.graphics/docs/http-listener/). JSON bodies are written out as
# compact JSON. The documents give no success or error status code; 200 is
# taken as success and 404 stands for the v3.4 error response.
H2R = "h2r-graphics"


def _h2r(command, input, target, body=None, **extra):
    request = {"method": "POST", "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": H2R, "command": command, "input": input, "expect_request": request, **extra})


_B = "/api/ABCD"
_OK = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}
_ERR = {"http_reply": {"status": 404, "body": "Graphic not found"},
        "expect_result": {"error": {"error": "device_error", "code": "404"}}}

# Rundown.
_h2r("run", {}, _B + "/run", **_OK)
_h2r("clear", {}, _B + "/clear")

# Graphics. The document's own example graphic id is 1234.
_h2r("show_graphic", {"graphic": "1234"}, _B + "/graphic/1234/show", **_OK)
_h2r("hide_graphic", {"graphic": "1234"}, _B + "/graphic/1234/hide", **_ERR)
_h2r("toggle_graphic", {"graphic": "1234"}, _B + "/graphic/1234/update", '{"status":"toggle"}')
_h2r("update_graphic", {"graphic": "1234", "body": {"line_one": "Jane Doe", "line_two": "Host"}},
     _B + "/graphic/1234/update", '{"line_one":"Jane Doe","line_two":"Host"}')
# "graphic/1234/updateScore/1/1/up/10 ... affect team 1, level 1, and add 10 points".
_h2r("update_score", {"graphic": "1234", "team": 1, "level": 1, "type": "up", "amount": 10},
     _B + "/graphic/1234/updateScore/1/1/up/10", **_OK)

# Variables.
_h2r("set_text_variable", {"variable": 2, "text": 'Live from "Main Stage"'},
     _B + "/updateVariableText/text.2", '{"text":"Live from \\"Main Stage\\""}', **_OK)
# "updateVariableList/1/selectRow/next. Replace next with previous or <number>".
_h2r("select_list_row_next", {"list": 1}, _B + "/updateVariableList/1/selectRow/next")
_h2r("select_list_row_previous", {"list": 1}, _B + "/updateVariableList/1/selectRow/previous")
_h2r("select_list_row", {"list": 2, "row": 5}, _B + "/updateVariableList/2/selectRow/5")

# Outputs, on the Multiple outputs page's example project CCAK.
V.append({"spec": H2R, "command": "open_output", "input": {"output": 2}, "settings": {"project": "CCAK"},
          "expect_request": {"method": "POST", "target": "/api/CCAK/output/2/open"}})

# Speaker Timer: "10 would add 10 seconds. -10 would subtract 10 seconds."
_h2r("timer_run", {"graphic": "1234"}, _B + "/graphic/1234/timer/run", **_OK)
_h2r("timer_pause", {"graphic": "1234"}, _B + "/graphic/1234/timer/pause")
_h2r("timer_reset", {"graphic": "1234"}, _B + "/graphic/1234/timer/reset")
_h2r("timer_jump", {"graphic": "1234", "seconds": -10}, _B + "/graphic/1234/timer/jump/-10")
_h2r("timer_set_duration", {"graphic": "1234", "seconds": 300}, _B + "/graphic/1234/timer/duration/300")

# Lyrics (version 3).
_h2r("lyric_next", {"graphic": "1234"}, _B + "/graphic/1234/lyric/next", **_OK)
_h2r("lyric_previous", {"graphic": "1234"}, _B + "/graphic/1234/lyric/previous")
_h2r("lyric_start", {"graphic": "1234"}, _B + "/graphic/1234/lyric/start")

# HTTP Listener, with the document's example source id and message.
_msg = {"messages": [{
    "id": "ABCD_MUST_BE_UNIQUE",
    "timestamp": "UNIX_TIMESTAMP",
    "snippet": {"displayMessage": "Here is my message to you-ou-ou."},
    "authorDetails": {"displayName": "John Barker", "profileImageUrl": "URL to image"},
    "platform": {"name": "YouTube", "logoUrl": "https://img.url.com"}}]}
_h2r("post_social_messages", {"source": "UXKIOKQJAA", "body": _msg}, "/data/UXKIOKQJAA",
     '{"messages":[{"id":"ABCD_MUST_BE_UNIQUE","timestamp":"UNIX_TIMESTAMP",'
     '"snippet":{"displayMessage":"Here is my message to you-ou-ou."},'
     '"authorDetails":{"displayName":"John Barker","profileImageUrl":"URL to image"},'
     '"platform":{"name":"YouTube","logoUrl":"https://img.url.com"}}]}', **_OK)

# Telemetry: Socket.IO 4 event frames ("42" + JSON array) as H2R's Companion
# module receives them (updateFrontend with data.projects.<id>); the field
# names are those the module reads, the values made up.
_H2R_PROJECT = {
    "cues": [
        {"id": "lt1", "type": "lower_third", "label": "Host", "status": "onair",
         "line_one": "Jane Doe", "line_two": "[text.1]", "line_three": "", "outputTwo": True},
        {"id": "cd1", "type": "time_countdown", "label": "Break", "status": "cued", "state": "running",
         "duration": 300000, "endAt": 1791234567000, "onEnd": "hold"},
        {"id": "vid1", "type": "video", "label": "Intro", "playing": True, "length": "12.5",
         "playedAt": 1791234560000},
    ],
    "dynamicText": {"text.1": "Studio A", "text.2": ""},
    "dynamicLists": [[[{"selected": 3}]], [[{}]]],
}
telemetry("h2r-graphics", "update-frontend",
          inbound_ws="42" + json.dumps(["updateFrontend", {"projects": {"ABCD": _H2R_PROJECT}}],
                                       separators=(",", ":")),
          expect_state={
              "graphics": {
                  "lt1": {"type": "lower_third", "label": "Host", "status": "onair", "on_air": True,
                          "line_one": "Jane Doe", "line_two": "[text.1]", "line_three": "",
                          "outputs": {"2": True}},
                  "cd1": {"type": "time_countdown", "label": "Break", "status": "cued", "on_air": False,
                          "timer_state": "running", "duration_ms": 300000, "end_at": 1791234567000,
                          "on_end": "hold"},
                  "vid1": {"type": "video", "label": "Intro", "playing": True, "length_s": 12.5,
                           "played_at": 1791234560000}},
              "variables": {"text.1": "Studio A", "text.2": ""},
              "lists": {"1": {"selected_row": 3}}})
# A graphic removed in H2R leaves state with the next event.
telemetry("h2r-graphics", "graphic-removed",
          state_before={"graphics": {"old": {"type": "image", "label": "Logo"}}},
          inbound_ws="42" + json.dumps(["updateFrontend", {"projects": {"ABCD": {
              "cues": [{"id": "lt1", "type": "lower_third", "label": "Host"}], "dynamicText": {}}}}],
              separators=(",", ":")),
          expect_state={"graphics": {"lt1": {"type": "lower_third", "label": "Host"}}})
telemetry("h2r-graphics", "other-event", inbound_ws='42["connected",{}]', expect_state={})
