# Sofie Chef (sofie-chef): one vector per command and telemetry vectors for
# the status push and the window list. Targets are written by hand from the
# README's Rest API table; message shapes from src/lib/api.ts. Every request
# carries apiKey, empty when none is set.
SC = "sofie-chef"
_SC_OK = {"http_reply": {"status": 200, "body": '""'}, "expect_result": {"ok": {"kind": "ack"}}}
http(SC, "get_status", {}, "GET", "/api/status?apiKey=",
     http_reply={"status": 200, "body": '{"version":"0.7.0"}'},
     expect_result={"ok": {"kind": "value", "value": {"version": "0.7.0"}}})
http(SC, "list_windows", {}, "GET", "/api/list?apiKey=")
http(SC, "list_api", {}, "GET", "/api?apiKey=")
V.append({"spec": SC, "command": "play_url", "input": {"window": "main", "url": "http://10.0.0.5:8080/lower-third"},
          "settings": {"api_key": "k3y"},
          "expect_request": {"method": "PUT", "target": "/api/playURL/main?apiKey=k3y",
                             "body": '{"url":"http://10.0.0.5:8080/lower-third"}'}, **_SC_OK})
V.append({"spec": SC, "command": "play_url_with_script",
          "input": {"window": "main", "url": "http://10.0.0.5:8080/clock", "js_code": "start();\nshow()"},
          "expect_request": {"method": "PUT", "target": "/api/playURL/main?apiKey=",
                             "body": '{"url":"http://10.0.0.5:8080/clock","jsCode":"start();\\nshow()"}'}})
http(SC, "restart", {"window": "main"}, "PUT", "/api/restart/main?apiKey=", **_SC_OK)
http(SC, "stop", {"window": "side 2"}, "PUT", "/api/stop/side%202?apiKey=",
     http_reply={"status": 404, "body": '"Window not found"'},
     expect_result={"error": {"error": "device_error", "code": "404"}})
V.append({"spec": SC, "command": "execute", "input": {"window": "main", "js_code": "next()"},
          "expect_request": {"method": "PUT", "target": "/api/execute/main?apiKey=", "body": '{"jsCode":"next()"}'}})

telemetry(SC, "status", state_before={"window_status": {"gone": {"status": "good"}}},
          inbound_ws=json.dumps({"type": "status", "status": {
              "version": "0.7.0", "app": {"statusCode": "good", "message": ""},
              "windows": {"main": {"statusCode": "warning", "message": "Unable to load the correct font"},
                          "side": {"statusCode": "good", "message": "Nothing to see here"}}}}),
          expect_state={"app": {"version": "0.7.0", "status": "good", "message": ""},
                        "window_status": {"main": {"status": "warning", "message": "Unable to load the correct font"},
                                          "side": {"status": "good", "message": "Nothing to see here"}}})
telemetry(SC, "reply", inbound_ws='{"type":"reply","replyTo":3,"result":{"code":200,"body":""}}', expect_state={})
telemetry(SC, "list", state_before={"windows": {"old": {"url": "x"}}},
          inbound_http={"path": "/api/list?apiKey=", "body": json.dumps([
              {"id": "main", "url": "http://10.0.0.5:8080/lower-third", "actualUrl": "http://10.0.0.5:8080/lower-third",
               "statusCode": "good", "statusMessage": ""},
              {"id": "side", "url": None, "actualUrl": "about:blank", "statusCode": "good", "statusMessage": ""}])},
          expect_state={"windows": {"main": {"url": "http://10.0.0.5:8080/lower-third",
                                             "actual_url": "http://10.0.0.5:8080/lower-third", "status": "good",
                                             "message": ""},
                                    "side": {"actual_url": "about:blank", "status": "good", "message": ""}}})
telemetry(SC, "reread-after-command", inbound_http={"path": "/api/stop/main?apiKey=", "body": '""'},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/api/list?apiKey="}])
