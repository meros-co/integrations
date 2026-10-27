# Softron Multicam Logger REST API (Softron support article 360021661233): targets written by hand from the guide.
SML = "softron-multicam-logger"
_SML_OK = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}
http(SML, "start_logging", {}, "GET", "/start", **_SML_OK)
http(SML, "stop_logging", {}, "GET", "/stop", **_SML_OK)
http(SML, "get_inputs", {}, "GET", "/inputs", http_reply={"status": 200, "body": '["INPUT 1","INPUT 2"]'},
     expect_result={"ok": {"kind": "value", "value": ["INPUT 1", "INPUT 2"]}})
http(SML, "get_status", {}, "GET", "/status")
http(SML, "add_marker", {"name": "This is my Marker"}, "GET", "/add_marker?name=This%20is%20my%20Marker", **_SML_OK)
V.append({"spec": SML, "command": "add_marker_offset", "input": {"name": "Goal", "offset": "-00:00:05:00"},
          "expect_request": {"method": "POST", "target": "/add_marker",
                             "body": '{"name":"Goal","reference":"live","offset":"-00:00:05:00"}'}, **_SML_OK})
V.append({"spec": SML, "command": "add_marker_at", "input": {"name": "Start", "time": "01:00:05:00"},
          "expect_request": {"method": "POST", "target": "/add_marker",
                             "body": '{"name":"Start","reference":"absolute","time":"01:00:05:00"}'}})
http(SML, "get_markers", {}, "GET", "/markers")
http(SML, "get_marker", {"index": 1}, "GET", "/markers/1",
     http_reply={"status": 200, "body": '{"name":"Marker 2","time":"01:00:15;00"}'},
     expect_result={"ok": {"kind": "value", "value": {"name": "Marker 2", "time": "01:00:15;00"}}})
http(SML, "change_angle", {"program": 0, "preview": 1}, "GET", "/change_angle?program=0&preview=1", **_SML_OK)
http(SML, "set_program", {"program": 2}, "GET", "/change_angle?program=2")
http(SML, "set_preview", {"preview": 3}, "GET", "/change_angle?preview=3")
http(SML, "take", {}, "GET", "/change_angle", **_SML_OK)
http(SML, "get_angle_changes", {}, "GET", "/angle_changes")

telemetry(SML, "status", inbound_http={"path": "/status", "body": json.dumps(
    {"preview": "7", "document_name": "Untitled", "logging_state": False, "program": "8", "timecode": "14:25:33;01"})},
    expect_state={"program": 8, "preview": 7, "timecode": "14:25:33;01", "document_name": "Untitled", "logging": False})
telemetry(SML, "inputs", state_before={"inputs": {"5": "OLD"}},
          inbound_http={"path": "/inputs", "body": '["INPUT 1","INPUT 2","INPUT 3"]'},
          expect_state={"inputs": {"0": "INPUT 1", "1": "INPUT 2", "2": "INPUT 3"}})
telemetry(SML, "markers", inbound_http={"path": "/markers", "body": json.dumps(
    [{"name": "Marker 1", "time": "01:00:05;00"}, {"name": "Marker 2", "time": "01:00:15;00"}])},
    expect_state={"markers": {"0": {"name": "Marker 1", "time": "01:00:05;00"}, "1": {"name": "Marker 2", "time": "01:00:15;00"}}})
telemetry(SML, "angle-changes", inbound_http={"path": "/angle_changes", "body": json.dumps(
    [{"out_point": "01:00:11;00", "in_point": "01:00:01;00", "program": 0, "duration": "00:00:10;00"}])},
    expect_state={"angle_changes": {"0": {"in_point": "01:00:01;00", "out_point": "01:00:11;00", "program": 0,
                                          "duration": "00:00:10;00"}}})
telemetry(SML, "reread-after-angle", inbound_http={"path": "/change_angle?program=0", "body": ""}, expect_state={},
          expect_then_send=[{"method": "GET", "target": "/status"}])
telemetry(SML, "reread-after-marker", inbound_http={"path": "/add_marker?name=x", "body": ""}, expect_state={},
          expect_then_send=[{"method": "GET", "target": "/markers"}])
