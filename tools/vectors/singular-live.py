# Singular.live (singular-live, singular-datastream): one vector per command,
# over the REST API v2 and the Data Stream API. Targets and bodies are written
# by hand from Singular's API reference (developer.singular.live): the app
# token in the path, the PATCH .../control body as an array of
# sub-composition updates, POST .../command {"action":"TakeOutAllOutput"}, and
# PUT /datastreams/<private token>. The token is the portal's example token.
SL = "singular-live"
_T = {"app_token": "1H7sRGX6UhgQHlAgtAXTB6"}
_B = "/apiv2/controlapps/1H7sRGX6UhgQHlAgtAXTB6"
_OK = {"http_reply": {"status": 200, "body": '{"success": true}'}, "expect_result": {"ok": {"kind": "ack"}}}


def _sl(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": SL, "command": command, "input": input, "settings": _T,
              "expect_request": request, **extra})


_sl("get_metadata", {}, "GET", _B,
    http_reply={"status": 200, "body": '{"id":1630476,"type":"controlapp"}'},
    expect_result={"ok": {"kind": "value", "value": {"id": 1630476, "type": "controlapp"}}})
_sl("get_model", {}, "GET", _B + "/model")
_sl("get_control", {}, "GET", _B + "/control")
_sl("update_control", {"body": [{"subCompositionName": "Lower Third", "payload": {"Name": "John Doe"}, "state": "In"}]},
    "PATCH", _B + "/control",
    '[{"subCompositionName":"Lower Third","payload":{"Name":"John Doe"},"state":"In"}]', **_OK)
_sl("set_content", {"subcomposition": "816df4ac-3231-49a2-b744-ccd48aa33a40",
                    "payload": {"text-control-node-id": "Text from Google Sheet"}},
    "PATCH", _B + "/control",
    '[{"subCompositionId":"816df4ac-3231-49a2-b744-ccd48aa33a40","payload":{"text-control-node-id":"Text from Google Sheet"}}]',
    **_OK)
_sl("set_content_by_name", {"subcomposition": "compTextArea", "payload": {"text-area-control-node-id": "Title\nSubtitle"}},
    "PATCH", _B + "/control",
    '[{"subCompositionName":"compTextArea","payload":{"text-area-control-node-id":"Title\\nSubtitle"}}]')
_sl("set_state", {"subcomposition": "816df4ac-3231-49a2-b744-ccd48aa33a40", "state": "In"}, "PATCH", _B + "/control",
    '[{"subCompositionId":"816df4ac-3231-49a2-b744-ccd48aa33a40","state":"In"}]', **_OK)
_sl("set_state_by_name", {"subcomposition": "compTextArea", "state": "Out2"}, "PATCH", _B + "/control",
    '[{"subCompositionName":"compTextArea","state":"Out2"}]',
    http_reply={"status": 429, "body": "Too Many Requests"},
    expect_result={"error": {"error": "device_error", "code": "429"}})
_sl("set_content_and_state", {"subcomposition": "7b7c8944-ba1f-4eec-a361-2c9ce308d462",
                              "payload": {"name": "NYG", "pos": "88"}, "state": "In"},
    "PATCH", _B + "/control",
    '[{"subCompositionId":"7b7c8944-ba1f-4eec-a361-2c9ce308d462","payload":{"name":"NYG","pos":"88"},"state":"In"}]')
_sl("set_content_and_state_by_name", {"subcomposition": "Score", "payload": {"Home": 3}, "state": "Out1"},
    "PATCH", _B + "/control", '[{"subCompositionName":"Score","payload":{"Home":3},"state":"Out1"}]')
_sl("set_control_node", {"subcomposition": "ab6e39f9-5550-47c2-81be-fa07611a6666",
                         "node": "checkbox-control-node-id", "value": True},
    "PATCH", _B + "/control",
    '[{"subCompositionId":"ab6e39f9-5550-47c2-81be-fa07611a6666","payload":{"checkbox-control-node-id":true}}]', **_OK)
_sl("set_control_node_by_name", {"subcomposition": "Colors", "node": "color-control-node-id", "value": "#33AAFF"},
    "PATCH", _B + "/control",
    '[{"subCompositionName":"Colors","payload":{"color-control-node-id":"#33AAFF"}}]')
_sl("timer_control", {"subcomposition": "1e602003-5bfa-45f4-8049-8892220fdff2", "node": "time-control-node-id",
                      "command": "play"},
    "PATCH", _B + "/control",
    '[{"subCompositionId":"1e602003-5bfa-45f4-8049-8892220fdff2","payload":{"time-control-node-id":{"command":"play"}}}]')
_sl("timer_control_by_name", {"subcomposition": "Clock", "node": "time-control-node-id", "command": "reset"},
    "PATCH", _B + "/control",
    '[{"subCompositionName":"Clock","payload":{"time-control-node-id":{"command":"reset"}}}]')
_sl("press_button", {"subcomposition": "755bd36f-fff0-4250-b703-4cbd310f07fb", "node": "cnVideoClipPlay"},
    "PATCH", _B + "/control",
    '[{"subCompositionId":"755bd36f-fff0-4250-b703-4cbd310f07fb","payload":{"cnVideoClipPlay":"execute"}}]')
_sl("press_button_by_name", {"subcomposition": "Video", "node": "cnVideoClipPause"}, "PATCH", _B + "/control",
    '[{"subCompositionName":"Video","payload":{"cnVideoClipPause":"execute"}}]')
_sl("take_out_all", {}, "POST", _B + "/command", '{"action":"TakeOutAllOutput"}', **_OK)

# Telemetry, from the reference's control-data and metadata examples.
telemetry(SL, "control", settings=_T, inbound_http={"path": _B + "/control", "body": json.dumps([
    {"subCompositionId": "-NC_s8grzroliz2xbx7e", "mainComposition": True, "state": "Out1", "payload": {}},
    {"subCompositionId": "1e602003-5bfa-45f4-8049-8892220fdff2", "subCompositionName": "Clock",
     "mainComposition": False, "state": "In",
     "payload": {"time-control-node-id": {"UTC": 1664044077270, "isRunning": True, "value": 9917}}},
    {"subCompositionId": "816df4ac-3231-49a2-b744-ccd48aa33a40", "mainComposition": False, "state": "Out2",
     "payload": {"text-control-node-id": "Text from Google Sheet", "checkbox": True}}])},
    state_before={"subcompositions": {"gone": {"state": "In"}}},
    expect_state={"subcompositions": {
        "-NC_s8grzroliz2xbx7e": {"main": True, "state": "Out1", "on_air": False},
        "1e602003-5bfa-45f4-8049-8892220fdff2": {
            "name": "Clock", "main": False, "state": "In", "on_air": True,
            "payload": {"time-control-node-id": '{"UTC":1664044077270,"isRunning":true,"value":9917}'}},
        "816df4ac-3231-49a2-b744-ccd48aa33a40": {
            "main": False, "state": "Out2", "on_air": False,
            "payload": {"text-control-node-id": "Text from Google Sheet", "checkbox": "true"}}}})
telemetry(SL, "metadata", settings=_T, inbound_http={"path": _B, "body": json.dumps({
    "id": 1630476, "userId": 25290, "accountId": 22123, "updatedAt": "2022-09-26T10:41:36.000Z",
    "type": "controlapp", "outputUrl": "https://app.singular.live/output/3AVwVLk51gGVCFWckgBcti/Output?aspect=16:9",
    "broadcastOutputUrl": "https://app.singular.live/output/3AVwVLk51gGVCFWckgBcti/Broadcast?aspect=16:9",
    "publicControlUrl": "https://app.singular.live/control/1H7sRGX6UhgQHlAgtAXTB6", "appTemplateId": 518,
    "appTemplateVersion": 10, "name": "REST-Control Node Field Types app", "compositionId": 454606})},
    expect_state={"app": {
        "name": "REST-Control Node Field Types app", "type": "controlapp", "id": 1630476, "composition_id": 454606,
        "output_url": "https://app.singular.live/output/3AVwVLk51gGVCFWckgBcti/Output?aspect=16:9",
        "broadcast_output_url": "https://app.singular.live/output/3AVwVLk51gGVCFWckgBcti/Broadcast?aspect=16:9",
        "control_url": "https://app.singular.live/control/1H7sRGX6UhgQHlAgtAXTB6", "template_id": 518,
        "template_version": 10, "updated_at": "2022-09-26T10:41:36.000Z"}})
telemetry(SL, "rate-limits", settings=_T, inbound_http={
    "path": _B + "/control", "body": '{"success": true}',
    "headers": {"X-Singular-Ratelimit-Burst-Calls": '{"limit": 500, "remaining": 499, "reset": 1614430792}',
                "X-Singular-Ratelimit-Daily-Calls": '{"limit": 100000, "remaining": 99854, "reset": 1614470401}',
                "X-Singular-Ratelimit-Burst-Data": '{"limit": 1048576, "remaining": 1048452, "reset": 1614430792}',
                "X-Singular-Ratelimit-Daily-Data": '{"limit": 524288000, "remaining": 522565096, "reset": 1614470401}'}},
    expect_state={"rate_limit": {
        "burst_calls": '{"limit": 500, "remaining": 499, "reset": 1614430792}',
        "daily_calls": '{"limit": 100000, "remaining": 99854, "reset": 1614470401}',
        "burst_data": '{"limit": 1048576, "remaining": 1048452, "reset": 1614430792}',
        "daily_data": '{"limit": 524288000, "remaining": 522565096, "reset": 1614470401}'}})

# Data stream: the reference's example body.
V.append({"spec": "singular-datastream", "command": "send_data", "settings": {"private_token": "a1B2c3D4e5"},
          "input": {"data": {"number": 123, "text": "this is text", "isActive": True, "color": "#3333ff"}},
          "expect_request": {"method": "PUT", "target": "/datastreams/a1B2c3D4e5",
                             "body": '{"number":123,"text":"this is text","isActive":true,"color":"#3333ff"}'},
          "http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}})
