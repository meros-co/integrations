# Softron MovieRecorder REST API and /remote websocket: calls from Softron's release notes and support articles,
# the rest as the Bitfocus Companion module makes them (inferred). The password is sent on every request.
SMR = "softron-movierecorder"
_SMR_OK = {"http_reply": {"status": 200, "body": '{"success":true}'}, "expect_result": {"ok": {"kind": "ack"}}}
V.append({"spec": SMR, "command": "record", "input": {"source": "SRC-1"},
          "expect_request": {"method": "PUT", "target": "/sources/record?password=", "body": '["SRC-1"]'}, **_SMR_OK})
V.append({"spec": SMR, "command": "pause", "input": {"source": "SRC-1"}, "settings": {"password": "1234"},
          "expect_request": {"method": "PUT", "target": "/sources/pause?password=1234", "body": '["SRC-1"]'}})
V.append({"spec": SMR, "command": "resume", "input": {"source": "SRC-1"},
          "expect_request": {"method": "PUT", "target": "/sources/resume?password=", "body": '["SRC-1"]'}})
V.append({"spec": SMR, "command": "stop", "input": {"source": "SRC-2"},
          "expect_request": {"method": "PUT", "target": "/sources/stop?password=", "body": '["SRC-2"]'},
          "http_reply": {"status": 401, "body": ""}, "expect_result": {"error": {"error": "device_error", "code": "401"}}})
http(SMR, "record_selected", {}, "PUT", "/sources/record?selected_only=YES&password=", **_SMR_OK)
http(SMR, "lock", {"source": "SRC-1"}, "GET", "/sources/SRC-1/lock?password=", **_SMR_OK)
http(SMR, "unlock", {"source": "SRC 1"}, "GET", "/sources/SRC%201/unlock?password=")
http(SMR, "split", {"source": "SRC-1"}, "GET", "/sources/SRC-1/manual_split?password=")
V.append({"spec": SMR, "command": "set_recording_name", "input": {"source": "SRC-1", "name": "Keynote"},
          "expect_request": {"method": "PUT", "target": "/sources/SRC-1/recording_name?password=",
                             "body": '{"recording_name":"Keynote"}'}})
V.append({"spec": SMR, "command": "set_destination", "input": {"source": "SRC-1", "destination": "DST-9"},
          "expect_request": {"method": "PUT", "target": "/sources/SRC-1/destinations?password=", "body": '["DST-9"]'}})
V.append({"spec": SMR, "command": "set_scheduled_duration", "input": {"recording": "REC-3", "minutes": 65},
          "expect_request": {"method": "PUT", "target": "/scheduled_recordings/REC-3?password=", "body": '{"duration":65}'}})
V.append({"spec": SMR, "command": "set_scheduled_start", "input": {"recording": "REC-3", "minute_of_day": 0},
          "expect_request": {"method": "PUT", "target": "/scheduled_recordings/REC-3?password=", "body": '{"start_time":0}'}})
V.append({"spec": SMR, "command": "resume_scheduled", "input": {"recording": "REC-3"},
          "expect_request": {"method": "PUT", "target": "/scheduled_recordings/REC-3?password=",
                             "body": '{"stopped_by_user":false}'}})
V.append({"spec": SMR, "command": "restore_configuration", "input": {"name": "Show", "layout": False},
          "expect_request": {"method": "PUT", "target": "/restore_configuration?password=",
                             "body": '{"name":"Show","restore_vtr_bach_list":true,"restore_sources":true,'
                                     '"restore_av_presets":true,"restore_destinations":true,"restore_schedule":true,'
                                     '"restore_layout":false}'}})
http(SMR, "run_applescript", {"script": "My Script"}, "GET", "/applescripts/My%20Script?password=")
http(SMR, "get_info", {}, "GET", "/info?password=")
http(SMR, "get_sources", {}, "GET", "/sources?password=")
http(SMR, "get_all_sources", {}, "GET", "/sources?include_disabled_sources=YES&password=")
http(SMR, "get_destinations", {}, "GET", "/destinations?password=")
http(SMR, "get_scheduled_recordings", {}, "GET", "/scheduled_recordings?password=")
http(SMR, "get_configurations", {}, "GET", "/configurations?password=",
     http_reply={"status": 200, "body": '["Show","Rehearsal"]'},
     expect_result={"ok": {"kind": "value", "value": ["Show", "Rehearsal"]}})

_SRC = {"unique_id": "SRC-1", "display_name": "Camera 1", "is_recording": True, "is_paused": False, "is_locked": False,
        "recording_name": "Keynote", "video_format": "1080p25", "recording_start_date": "2026-10-27T10:00:00Z"}
_SRC_STATE = {"name": "Camera 1", "recording": True, "paused": False, "locked": False, "recording_name": "Keynote",
              "video_format": "1080p25", "recording_start": "2026-10-27T10:00:00Z"}
telemetry(SMR, "ws-source-update", inbound_ws=json.dumps({"dataType": "source", "change": "update", "data": _SRC}),
          expect_state={"sources": {"SRC-1": _SRC_STATE}})
telemetry(SMR, "ws-source-insertion", inbound_ws=json.dumps({"dataType": "source", "change": "insertion",
                                                             "data": dict(_SRC, unique_id="SRC-2")}),
          expect_state={"sources": {"SRC-2": _SRC_STATE}})
telemetry(SMR, "ws-source-removal", state_before={"sources": {"SRC-1": {"name": "Camera 1"}}},
          inbound_ws=json.dumps({"dataType": "source", "change": "removal", "data": {"unique_id": "SRC-1"}}),
          expect_state={"sources": {}})
telemetry(SMR, "sources", state_before={"sources": {"OLD": {"name": "x"}}},
          inbound_http={"path": "/sources?password=", "body": json.dumps([_SRC])},
          expect_state={"sources": {"SRC-1": _SRC_STATE}})
telemetry(SMR, "destinations", inbound_http={"path": "/destinations?password=", "body": json.dumps(
    [{"unique_id": "DST-9", "name": "SSD"}])}, expect_state={"destinations": {"DST-9": {"name": "SSD"}}})
telemetry(SMR, "scheduled", inbound_http={"path": "/scheduled_recordings?password=", "body": json.dumps(
    [{"unique_id": "REC-3", "name": "Morning", "source_unique_id": "SRC-1", "start_time": 540, "duration": 60,
      "is_enabled": True, "weekly_repeated": True, "stopped_by_user": False}])},
    expect_state={"scheduled": {"REC-3": {"name": "Morning", "source": "SRC-1", "start_minute": 540,
                                          "duration_minutes": 60, "enabled": True, "weekly": True,
                                          "stopped_by_user": False}}})
telemetry(SMR, "configurations", inbound_http={"path": "/configurations?password=", "body": '["Show","Rehearsal"]'},
          expect_state={"configurations": {"0": "Show", "1": "Rehearsal"}})
telemetry(SMR, "reread-schedule", inbound_http={"path": "/scheduled_recordings/REC-3?password=", "body": ""},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/scheduled_recordings?password="}])
