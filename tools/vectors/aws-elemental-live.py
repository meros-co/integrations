# AWS Elemental Live (aws-elemental-live): one vector per command, and
# telemetry vectors for the polled resources. Targets follow the Elemental
# Live API and User Guide (2.26+): /api/<endpoint>, XML bodies. The small XML
# bodies the commands write are stated here by hand from the guide's
# examples; commands that pass a whole document send it as it is given, and
# their targets are computed from the spec with example values.
EL = "aws-elemental-live"
_ELB = {
    "set_live_event_priority": ({"event": 3, "priority": 50}, "/api/live_events/3/priority", "<priority>50</priority>"),
    "start_live_event": ({"event": 3}, "/api/live_events/3/start", "<start></start>"),
    "stop_live_event": ({"event": 3}, "/api/live_events/3/stop", "<stop></stop>"),
    "cancel_live_event": ({"event": 3}, "/api/live_events/3/cancel", "<cancel></cancel>"),
    "archive_live_event": ({"event": 3}, "/api/live_events/3/archive", "<archive></archive>"),
    "reset_live_event": ({"event": 3}, "/api/live_events/3/reset", "<reset></reset>"),
    "mute_audio": ({"event": 3}, "/api/live_events/3/mute_audio", "<mute_audio></mute_audio>"),
    "unmute_audio": ({"event": 3}, "/api/live_events/3/unmute_audio", "<unmute_audio></unmute_audio>"),
    "reset_video_buffer_stats": ({"event": 3}, "/api/live_events/3/reset_video_buffer_stats",
                                 "<reset_video_buffer_stats></reset_video_buffer_stats>"),
    "start_live_event_at_input": ({"event": 3, "input": 7}, "/api/live_events/3/start?active_input_id=7", "<start></start>"),
    "start_live_event_at_input_label": ({"event": 3, "label": "Backup feed"},
                                        "/api/live_events/3/start?active_input_label=Backup%20feed", "<start></start>"),
    "set_audio_gain": ({"event": 3, "gain": -6}, "/api/live_events/3/adjust_audio_gain", "<gain>-6</gain>"),
    "rollover_output": ({"event": 3, "output": 12}, "/api/live_events/3/rollover_output", "<output_id>12</output_id>"),
    "pause_output": ({"event": 3, "output": 12}, "/api/live_events/3/pause_output", "<output_id>12</output_id>"),
    "unpause_output": ({"event": 3, "output": 12}, "/api/live_events/3/unpause_output", "<output_id>12</output_id>"),
    "stop_output": ({"event": 3, "output": 12}, "/api/live_events/3/stop_output", "<output_id>12</output_id>"),
    "start_output": ({"event": 3, "output": 12}, "/api/live_events/3/start_output", "<output_id>12</output_id>"),
    "pause_output_group": ({"event": 3, "group": 4}, "/api/live_events/3/pause_output_group", "<group_id>4</group_id>"),
    "unpause_output_group": ({"event": 3, "group": 4}, "/api/live_events/3/unpause_output_group", "<group_id>4</group_id>"),
    "stop_output_group": ({"event": 3, "group": 4}, "/api/live_events/3/stop_output_group", "<group_id>4</group_id>"),
    "start_output_group": ({"event": 3, "group": 4}, "/api/live_events/3/start_output_group", "<group_id>4</group_id>"),
    "pause_output_group_by_name": ({"event": 3, "name": "HLS"}, "/api/live_events/3/pause_output_group",
                                   "<group_name>HLS</group_name>"),
    "unpause_output_group_by_name": ({"event": 3, "name": "HLS"}, "/api/live_events/3/unpause_output_group",
                                     "<group_name>HLS</group_name>"),
    "stop_output_group_by_name": ({"event": 3, "name": "HLS"}, "/api/live_events/3/stop_output_group",
                                  "<group_name>HLS</group_name>"),
    "start_output_group_by_name": ({"event": 3, "name": "HLS"}, "/api/live_events/3/start_output_group",
                                   "<group_name>HLS</group_name>"),
    "insert_avail": ({"event": 4, "event_id": 2, "preroll": 8000, "duration": 30}, "/api/live_events/4/cue_point",
                     "<cue_point><event_id>2</event_id><splice_offset>8000</splice_offset><duration>30</duration></cue_point>"),
    "insert_avail_at": ({"event": 4, "event_id": 1, "hours": 0, "minutes": 0, "seconds": 0, "frames": 0, "duration": 30},
                        "/api/live_events/4/cue_point",
                        "<cue_point><event_id>1</event_id><splice_time><hours>0</hours><minutes>0</minutes><seconds>0</seconds>"
                        "<frames>0</frames></splice_time><duration>30</duration></cue_point>"),
    "return_from_avail": ({"event": 4}, "/api/live_events/4/cue_point",
                          "<cue_point><return_offset>0</return_offset></cue_point>"),
    "cancel_avail": ({"event": 4, "event_id": 4}, "/api/live_events/4/cue_point",
                     "<cue_point><cancel_event_id>4</cancel_event_id></cue_point>"),
    "get_stream_time": ({"event": 4}, "/api/live_events/4/cue_point",
                        "<cue_point><get_current_time>1</get_current_time></cue_point>"),
    "insert_time_signal": ({"event": 4, "preroll": 1000, "descriptors": "021B43554549000000027FBF030C54564E413130303030303031300000"},
                           "/api/live_events/4/time_signal",
                           "<time_signal><offset>1000</offset><descriptors>021B43554549000000027FBF030C54564E413130303030303031300000"
                           "</descriptors></time_signal>"),
    "insert_time_signal_at": ({"event": 4, "hours": 14, "minutes": 30, "seconds": 0, "frames": 0},
                              "/api/live_events/4/time_signal",
                              "<time_signal><time><hours>14</hours><minutes>30</minutes><seconds>0</seconds><frames>0</frames>"
                              "</time><descriptors></descriptors></time_signal>"),
    "activate_input": ({"event": 3, "input": 7}, "/api/live_events/3/activate_input", "<input_id>7</input_id>"),
    "activate_input_by_label": ({"event": 3, "label": "Backup feed"}, "/api/live_events/3/activate_input",
                                "<input_label>Backup feed</input_label>"),
    "activate_input_at": ({"event": 3, "input": 7, "utc_time": "20151231T235959.999"}, "/api/live_events/3/activate_input",
                          "<activate_input><input_id>7</input_id><utc_time>20151231T235959.999</utc_time></activate_input>"),
    "prepare_input": ({"event": 3, "input": 8}, "/api/live_events/3/prepare_input", "<input_id>8</input_id>"),
    "set_avail_image": ({"event": 3, "uri": "http://example.com/protected/blanking1.png"}, "/api/live_events/3/avail_image",
                        "<avail_image><uri>http://example.com/protected/blanking1.png</uri></avail_image>"),
    "set_blackout_image": ({"event": 3, "uri": "/opt/elemental_se/web/public/example_files/Elemental_logo.png"},
                           "/api/live_events/3/blackout_image",
                           "<blackout_image><uri>/opt/elemental_se/web/public/example_files/Elemental_logo.png</uri></blackout_image>"),
    "create_live_event_from_profile": ({"name": "Sunday", "profile": "basic_ms_smooth"}, "/api/live_events",
                                       "<live_event><name>Sunday</name><profile>basic_ms_smooth</profile></live_event>"),
    "start_mpts": ({"mpts": 2}, "/api/mpts/2/mux", "<mux></mux>"),
    "stop_service": ({}, "/api/settings/stop", "<stop></stop>"),
    "start_service": ({}, "/api/settings/start", "<start></start>"),
    "rename_device": ({"device": 1, "name": "Camera 1"}, "/api/devices/1", "<device><name>Camera 1</name></device>"),
}


def _el_vectors() -> None:
    from urllib.parse import quote

    doc = yaml.safe_load((ROOT / "specs" / f"{EL}.yaml").read_text(encoding="utf-8"))
    xml = "<live_event><name>Main</name></live_event>"
    for name, command in doc["commands"].items():
        send = command["send"]
        if name in _ELB:
            values, target, body = _ELB[name]
            V.append({"spec": EL, "command": name, "input": values,
                      "expect_request": {"method": send["method"], "target": target, "body": body}})
            continue
        params = command.get("params") or {}
        values = {}
        for pname, p in params.items():
            if pname == "xml":
                values[pname] = xml
            elif p["type"] == "enum":
                values[pname] = p["values"][-1]
            elif p["type"] == "int":
                values[pname] = 5 if pname not in ("page", "per_page") else (2 if pname == "page" else 50)
            else:
                values[pname] = "sunday_main"
        target = send["path"]
        for pname, v in values.items():
            target = target.replace("{" + pname + "}", quote(str(v), safe=""))
        if send.get("query"):
            parts = []
            for k, v in send["query"].items():
                v = values[v[1:-1]] if v.startswith("{") else v
                parts.append(f"{k}={quote(str(v), safe='')}")
            target += "?" + "&".join(parts)
        request = {"method": send["method"], "target": target}
        if "body" in send:
            request["body"] = values["xml"]
        V.append({"spec": EL, "command": name, "input": values, "expect_request": request})


_el_vectors()
# Replies.
http(EL, "get_live_event_status", {"event": 58}, "GET", "/api/live_events/58/status",
     http_reply={"status": 200, "body": "<live_event><status>running</status></live_event>"},
     expect_result={"ok": {"kind": "value", "value": "<live_event><status>running</status></live_event>"}},
     file="get_live_event_status-reply")
http(EL, "create_preset", {"xml": "<preset></preset>"}, "POST", "/api/presets",
     expect_request={"method": "POST", "target": "/api/presets", "body": "<preset></preset>"},
     http_reply={"status": 422, "body": "<errors><error>Name can't be blank</error></errors>"},
     expect_result={"error": {"error": "device_error", "code": "422"}}, file="create_preset-refused")
http(EL, "get_system_status", {}, "GET", "/api/system_status.json",
     http_reply={"status": 200, "body": json.dumps({"status": "green_status"})},
     expect_result={"ok": {"kind": "value", "value": {"status": "green_status"}}}, file="get_system_status-reply")

# Telemetry.
_ELLIST = ('<?xml version="1.0" encoding="UTF-8"?><live_event_list>'
           '<live_event href="/live_events/3" version="2.19.2.0.xxxx" product="Elemental Live"><id>3</id>'
           '<name>Sunday Service</name><input><device_input><channel>1</channel><device_type>AJA</device_type>'
           '<name>SDI 1</name></device_input></input><node_id>2</node_id><priority>50</priority><user_data></user_data>'
           '<submitted>2010-01-26 02:07:08 UTC</submitted><status>running</status></live_event>'
           '<live_event href="/live_events/4"><id>4</id><name>Backup</name><node_id>1</node_id>'
           '<submitted>2010-01-27 10:00:00 UTC</submitted><status>pending</status></live_event></live_event_list>')
telemetry(EL, "live-events", inbound_http={"path": "/api/live_events?page=1&per_page=100", "body": _ELLIST},
          state_before={"live_events": {"9": {"name": "Archived", "status": "complete"}}},
          expect_state={"live_events": {
              "3": {"name": "Sunday Service", "status": "running", "node_id": 2, "priority": 50,
                    "submitted": "2010-01-26 02:07:08 UTC"},
              "4": {"name": "Backup", "status": "pending", "node_id": 1, "submitted": "2010-01-27 10:00:00 UTC"}}})
telemetry(EL, "active-events", inbound_http={"path": "/api/live_events?filter=active&page=1&per_page=100", "body": _ELLIST},
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/api/live_events/3/status"},
                            {"method": "GET", "target": "/api/live_events/4/status"}])
telemetry(EL, "event-status", inbound_http={"path": "/api/live_events/58/status", "body": (
    '<?xml version="1.0" encoding="UTF-8"?><live_event href="https://server_ip:80/live_events/58" '
    'version="2.19.2.0.xxxx" product="Elemental Live"><user_data></user_data>'
    '<submitted>2010-02-22 13:46:34 -0700</submitted><status>error</status><pct_complete>16</pct_complete>'
    '<average_fps>53.4</average_fps><start_time>2010-02-22 13:46:34 -0700</start_time><complete_time></complete_time>'
    '<elapsed>835</elapsed><errors><error><code>1999</code><created_at>2010-02-22 14:00:29 -0700</created_at>'
    '<message>EME timeout detected</message></error></errors></live_event>')},
    expect_state={"event_status": {"58": {"status": "error", "pct_complete": 16, "average_fps": 53.4,
                                          "start_time": "2010-02-22 13:46:34 -0700", "complete_time": "", "elapsed": 835,
                                          "error_code": 1999, "error_message": "EME timeout detected",
                                          "error_time": "2010-02-22 14:00:29 -0700"}}})
telemetry(EL, "event-priority", inbound_http={"path": "/api/live_events/3/priority", "body": "<priority>75</priority>"},
          expect_state={"live_events": {"3": {"priority": 75}}})
telemetry(EL, "system-status", inbound_http={"path": "/api/system_status.json", "body": json.dumps({
    "cpu": {"pct": "1.46", "num": 0}, "mem": {"pct": 48, "num": 5}, "gpu": [{"pct": 0, "num": 0}, {"pct": 37, "num": 1}],
    "status": "orange_status", "date": "Jan 20, 2015 16:01:47", "devices": [], "psu_status": "Status: Present, OK",
    "raid_status": "There was an error while trying to get information about the device."})},
    expect_state={"system": {"status": "orange_status", "cpu_pct": 1.46, "mem_pct": 48.0, "date": "Jan 20, 2015 16:01:47",
                             "psu_status": "Status: Present, OK",
                             "raid_status": "There was an error while trying to get information about the device.",
                             "gpus": {"0": {"pct": 0.0}, "1": {"pct": 37.0}}}})
telemetry(EL, "devices", inbound_http={"path": "/api/devices", "body": (
    '<device_list><device><id>1</id><name>Camera 1</name><device_type>AJA</device_type><device_number>0</device_number>'
    '<quad>false</quad><node_id>1</node_id></device><device><id>2</id><name></name><device_type>AJA</device_type>'
    '<device_number>1</device_number><quad>false</quad><node_id>1</node_id></device></device_list>')},
    expect_state={"devices": {"1": {"name": "Camera 1", "device_type": "AJA", "device_number": 0, "node_id": 1},
                              "2": {"name": "", "device_type": "AJA", "device_number": 1, "node_id": 1}}})
