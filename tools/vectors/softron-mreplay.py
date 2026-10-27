# Softron M|Replay REST API: calls from Softron's support articles and release notes, the rest as the Bitfocus
# Companion module makes them (inferred). The password is sent on every request.
SMP = "softron-mreplay"
_SMP_OK = {"http_reply": {"status": 200, "body": '{"success":true}'}, "expect_result": {"ok": {"kind": "ack"}}}
V.append({"spec": SMP, "command": "add_marker", "input": {"name": "This is my Marker"}, "settings": {"password": "1234"},
          "expect_request": {"method": "POST", "target": "/markers?password=1234", "body": '{"name":"This is my Marker"}'},
          **_SMP_OK})
V.append({"spec": SMP, "command": "add_marker_at", "input": {"name": "Goal", "reference": "live", "offset": "-10"},
          "expect_request": {"method": "POST", "target": "/markers?password=",
                             "body": '{"name":"Goal","reference":"live","offset":"-10"}'}})
V.append({"spec": SMP, "command": "create_clip_in_duration",
          "input": {"name": "Goal", "in_reference": "live", "in_point": "-10", "duration": "10"},
          "expect_request": {"method": "POST", "target": "/clips?password=",
                             "body": '{"name":"Goal","unique_name":false,"reference_in_point":"live","in_point":"-10","duration":"10"}'}})
V.append({"spec": SMP, "command": "create_clip_in_out",
          "input": {"name": "Save", "in_reference": "absolute", "in_point": "01:00:00:00", "out_reference": "absolute",
                    "out_point": "01:00:10:00", "unique_name": True},
          "expect_request": {"method": "POST", "target": "/clips?password=",
                             "body": '{"name":"Save","unique_name":true,"reference_in_point":"absolute","in_point":"01:00:00:00",'
                                     '"reference_out_point":"absolute","out_point":"01:00:10:00"}'}})
V.append({"spec": SMP, "command": "create_clip_out_duration",
          "input": {"name": "Crowd", "out_reference": "playhead", "out_point": "0", "duration": "5"},
          "expect_request": {"method": "POST", "target": "/clips?password=",
                             "body": '{"name":"Crowd","unique_name":false,"reference_out_point":"playhead","out_point":"0","duration":"5"}'}})
http(SMP, "set_playback_rate", {"rate": -0.5}, "GET", "/playbackrate?rate=-0.50&password=", **_SMP_OK)
http(SMP, "set_position", {"reference": "playhead", "position": "-00:00:10:00"}, "GET",
     "/set_position?position=-00%3A00%3A10%3A00&reference=playhead&password=")
V.append({"spec": SMP, "command": "select_output", "input": {"output": "OUT-1"},
          "expect_request": {"method": "PUT", "target": "/outputs/OUT-1?password=", "body": '{"selected":true}'}, **_SMP_OK})
V.append({"spec": SMP, "command": "assign_input", "input": {"output": "OUT-1", "input": "IN-2"},
          "expect_request": {"method": "PUT", "target": "/outputs/OUT-1?password=", "body": '{"input_unique_id":"IN-2"}'}})
http(SMP, "pause_inputs", {}, "GET", "/inputs/pause?password=")
http(SMP, "resume_inputs", {}, "GET", "/inputs/resume?password=")
http(SMP, "play_playlist", {"playlist": "Highlights"}, "GET", "/playlists/Highlights/play?password=")
http(SMP, "pause_playlist", {"playlist": "PL-1"}, "GET", "/playlists/PL-1/pause?password=")
http(SMP, "stop_playlist", {"playlist": "PL-1"}, "GET", "/playlists/PL-1/stop?password=")
http(SMP, "playlist_previous", {"playlist": "PL-1"}, "GET", "/playlists/PL-1/skip_previous?password=")
http(SMP, "playlist_next", {"playlist": "PL-1"}, "GET", "/playlists/PL-1/skip_next?password=")
http(SMP, "play_playlist_item", {"playlist": "PL-1", "item": "2"}, "GET", "/playlists/PL-1/items/2/play?password=")
http(SMP, "pause_playlist_item", {"playlist": "PL-1", "item": "2"}, "GET", "/playlists/PL-1/items/2/pause?password=")
http(SMP, "get_info", {}, "GET", "/info?password=")
http(SMP, "get_status", {}, "GET", "/status?password=")
http(SMP, "get_inputs", {}, "GET", "/inputs?password=")
http(SMP, "get_outputs", {}, "GET", "/outputs?password=")
http(SMP, "get_playlists", {}, "GET", "/playlists?password=")
http(SMP, "get_markers", {}, "GET", "/markers?password=",
     http_reply={"status": 200, "body": '[{"name":"My First Marker","start":511090399.1733954}]'},
     expect_result={"ok": {"kind": "value", "value": [{"name": "My First Marker", "start": 511090399.1733954}]}})

telemetry(SMP, "status", inbound_http={"path": "/status?password=", "body": json.dumps({
    "playback_rate": 1, "current_time": "3600", "current_timecode": "01:00:00:00", "playhead_time": "3590",
    "playhead_timecode": "00:59:50:00", "delay_time": "10", "delay_timecode": "00:00:10:00", "remaining_time": "7200",
    "remaining_timecode": "02:00:00:00", "remaining_space": "512 GB", "framerate": "25", "drop_frame": False,
    "loop_recording": True})},
    expect_state={"status": {"playback_rate": 1.0, "current_time": "3600", "current_timecode": "01:00:00:00",
                             "playhead_time": "3590", "playhead_timecode": "00:59:50:00", "delay_time": "10",
                             "delay_timecode": "00:00:10:00", "remaining_time": "7200",
                             "remaining_timecode": "02:00:00:00", "remaining_space": "512 GB", "framerate": "25",
                             "drop_frame": False, "loop_recording": True}})
telemetry(SMP, "info", inbound_http={"path": "/info?password=", "body": json.dumps({
    "application_version": "2.3.8", "application_build_number": "1234", "computer_name": "Replay-1",
    "application_in_demo": False})},
    expect_state={"info": {"version": "2.3.8", "build": "1234", "computer_name": "Replay-1", "demo": False}})
telemetry(SMP, "inputs", inbound_http={"path": "/inputs?password=", "body": json.dumps(
    [{"unique_id": "IN-1", "name": "CAM 1", "enabled": True}])},
    expect_state={"inputs": {"IN-1": {"name": "CAM 1", "enabled": True}}})
telemetry(SMP, "outputs", inbound_http={"path": "/outputs?password=", "body": json.dumps(
    [{"unique_id": "OUT-1", "name": "PGM", "enabled": True, "selected": True, "input_unique_id": "IN-1"}])},
    expect_state={"outputs": {"OUT-1": {"name": "PGM", "enabled": True, "selected": True, "input": "IN-1"}}})
telemetry(SMP, "playlists", inbound_http={"path": "/playlists?password=", "body": json.dumps(
    [{"unique_id": "PL-1", "name": "Highlights"}])}, expect_state={"playlists": {"PL-1": {"name": "Highlights"}}})
telemetry(SMP, "markers", inbound_http={"path": "/markers?password=", "body": json.dumps(
    [{"name": "My First Marker", "start": 511090399.5}])},
    expect_state={"markers": {"0": {"name": "My First Marker", "start": 511090399.5}}})
telemetry(SMP, "reread-outputs", inbound_http={"path": "/outputs/OUT-1?password=", "body": ""}, expect_state={},
          expect_then_send=[{"method": "GET", "target": "/outputs?password="}])
