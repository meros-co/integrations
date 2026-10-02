A = "aja-kipro"
# AJA Ki Pro: GET /config?action=get|set&paramid=...&value=... (AJA REST API,
# gitlab.aja.com/pub/rest_api, chapters 1, 2, 4-6, April 2026; Ki Pro REST
# Automation 4.1.0 api.txt and examples). Query values are percent-encoded,
# keeping only RFC 3986 unreserved characters. A command sending several
# requests sends the next only after the previous reply, so its vector states
# the first request.

_CFG = "/config?action=set&paramid="

# Generic parameter access (chapter 1 p.8-9, chapter 2 p.16, p.22).
http(A, "get_parameter", {"paramid": "eParamID_VideoInSelect"}, "GET",
     "/config?action=get&paramid=eParamID_VideoInSelect",
     http_reply={"status": 200, "body":
                 '{"paramid":"33619978","name":"eParamID_VideoInSelect","value":"1","value_name":"HDMI"}'},
     expect_result={"ok": {"kind": "value", "value": {
         "paramid": "33619978", "name": "eParamID_VideoInSelect", "value": "1", "value_name": "HDMI"}}})
http(A, "set_parameter", {"paramid": "eParamID_VideoInSelect", "value": "0"}, "GET",
     _CFG + "eParamID_VideoInSelect&value=0",
     http_reply={"status": 404, "body": "<html><head><title>404 Not Found</title></head></html>"},
     expect_result={"error": {"error": "device_error", "code": "404"}})
http(A, "get_descriptor", {"paramid": "eParamID_TransportCommand"}, "GET",
     "/descriptors?paramid=eParamID_TransportCommand")
http(A, "get_all_descriptors", {}, "GET", "/desc.json")
http(A, "get_all_descriptors_legacy", {}, "GET", "/descriptors?paramid=*")
# The event connection (api.txt: connect returns {"connectionid":"20"}; the
# wait returns [{param_id, param_type, int_value, str_value,
# last_config_update}], the element values here placeholders).
http(A, "open_event_connection", {}, "GET", "/json?action=connect&configid=0",
     http_reply={"status": 200, "body": '{"connectionid":"20"}'},
     expect_result={"ok": {"kind": "value", "value": "20"}})
http(A, "wait_for_events", {"connection": 20}, "GET",
     "/json?action=wait_for_config_events&configid=0&connectionid=20",
     http_reply={"status": 200, "body":
                 '[{"param_id":"eParamID_DisplayTimecode","param_type":"string","int_value":0,'
                 '"str_value":"00:00:10:05","last_config_update":1}]'},
     expect_result={"ok": {"kind": "value", "value": [
         {"param_id": "eParamID_DisplayTimecode", "param_type": "string", "int_value": 0,
          "str_value": "00:00:10:05", "last_config_update": 1}]}})
http(A, "save_preset", {"preset": 4}, "GET", _CFG + "eParamID_RegisterSave&value=4")
http(A, "recall_preset", {"preset": 2}, "GET", _CFG + "eParamID_RegisterRecall&value=2",
     http_reply={"status": 200, "body":
                 '{"paramid":"1","name":"eParamID_RegisterRecall","value":"2","value_name":"2"}'},
     expect_result={"ok": {"kind": "ack"}})

# Transport: eParamID_TransportCommand enum (chapters 4-6; kipro.py).
http(A, "transport_command", {"command": 12}, "GET", _CFG + "eParamID_TransportCommand&value=12")
http(A, "play", {}, "GET", _CFG + "eParamID_TransportCommand&value=1",
     http_reply={"status": 200, "body":
                 '{"paramid":"1","name":"eParamID_TransportCommand","value":"1","value_name":"Play Command"}'},
     expect_result={"ok": {"kind": "ack"}})
http(A, "record", {}, "GET", _CFG + "eParamID_TransportCommand&value=3")
http(A, "stop", {}, "GET", _CFG + "eParamID_TransportCommand&value=4")
http(A, "fast_forward", {}, "GET", _CFG + "eParamID_TransportCommand&value=5")
http(A, "fast_reverse", {}, "GET", _CFG + "eParamID_TransportCommand&value=6")
http(A, "step_forward", {}, "GET", _CFG + "eParamID_TransportCommand&value=7")
http(A, "step_reverse", {}, "GET", _CFG + "eParamID_TransportCommand&value=8")
http(A, "next_clip", {}, "GET", _CFG + "eParamID_TransportCommand&value=9")
http(A, "previous_clip", {}, "GET", _CFG + "eParamID_TransportCommand&value=10")
http(A, "variable_speed_play", {}, "GET", _CFG + "eParamID_TransportCommand&value=11")
http(A, "preroll", {}, "GET", _CFG + "eParamID_TransportCommand&value=12")
http(A, "assemble_edit", {}, "GET", _CFG + "eParamID_TransportCommand&value=13")
http(A, "cue", {}, "GET", _CFG + "eParamID_TransportCommand&value=14")
http(A, "play_at_system_time", {}, "GET", _CFG + "eParamID_TransportCommand&value=16")
http(A, "record_at_system_time", {}, "GET", _CFG + "eParamID_TransportCommand&value=17")
http(A, "go_to_idle", {}, "GET", _CFG + "eParamID_TransportCommand&value=18")
# cue.py: eParamID_CueToTimecode = "00:00:10:05", then command 14.
http(A, "cue_to_timecode", {"timecode": "00:00:10:05"}, "GET",
     _CFG + "eParamID_CueToTimecode&value=00%3A00%3A10%3A05")
# Chapter 5-6: 16, -1.5, 0.5; reverse.py: command 11, then -1.0.
http(A, "set_playback_speed", {"speed": -1.5}, "GET",
     _CFG + "eParamID_TransportRequestedSpeed&value=-1.500")
http(A, "play_at_speed", {"speed": -1.0}, "GET", _CFG + "eParamID_TransportCommand&value=11")
http(A, "get_transport_state", {}, "GET", "/config?action=get&paramid=eParamID_TransportState",
     http_reply={"status": 200, "body":
                 '{"paramid":"1","name":"eParamID_TransportState","value":"3","value_name":"Playing Forward"}'},
     expect_result={"ok": {"kind": "value", "value": "Playing Forward"}})
http(A, "get_display_timecode", {}, "GET", "/config?action=get&paramid=eParamID_DisplayTimecode",
     http_reply={"status": 200, "body":
                 '{"paramid":"1","name":"eParamID_DisplayTimecode","value":"01:00:05:12","value_name":""}'},
     expect_result={"ok": {"kind": "value", "value": "01:00:05:12"}})

# Clips (api.txt; go_to_clipname.py; record_with_clipname.py; playlist.py).
http(A, "get_current_clip", {}, "GET", "/config?action=get&paramid=eParamID_CurrentClip")
http(A, "go_to_clip", {"clip": "Clip1ATK2.mov"}, "GET", _CFG + "eParamID_GoToClip&value=Clip1ATK2.mov")
http(A, "set_clip_name", {"name": "Show 1"}, "GET", _CFG + "eParamID_CustomClipName&value=Show%201")
http(A, "set_use_custom_clip_name", {"enabled": True}, "GET", _CFG + "eParamID_UseCustomClipName&value=1")
http(A, "set_use_custom_clip_take", {"enabled": True}, "GET", _CFG + "eParamID_UseCustomClipTake&value=1")
http(A, "record_named", {"name": "Take 4"}, "GET", _CFG + "eParamID_UseCustomClipTake&value=0")
http(A, "get_clips", {}, "GET", "/clips",
     http_reply={"status": 200, "body":
                 '[ { clipname: "Clip1ATK2.MOV", framecount: "474", framerate: "29.97" } ];\n'},
     expect_result={"ok": {"kind": "value", "value":
                           '[ { clipname: "Clip1ATK2.MOV", framecount: "474", framerate: "29.97" } ];'}})
http(A, "delete_clip", {"clip": "Clip1ATK43.mov"}, "POST", "/clips?action=delete&clipname=Clip1ATK43.mov")
http(A, "select_next_slot", {}, "POST", "/options?transport_command=next_slot")
http(A, "select_playlist", {"playlist": "Show Reel", "updated": 1472148607}, "GET",
     _CFG + "eParamID_CurrentPlaylist&value=Show%20Reel")

# Media (chapters 4-6).
http(A, "enter_data_lan_mode", {}, "GET", _CFG + "eParamID_MediaState&value=1")
http(A, "enter_record_play_mode", {}, "GET", _CFG + "eParamID_MediaState&value=0")
http(A, "get_media_state", {}, "GET", "/config?action=get&paramid=eParamID_MediaState",
     http_reply={"status": 200, "body":
                 '{"paramid":"1","name":"eParamID_MediaState","value":"1","value_name":"Data - LAN"}'},
     expect_result={"ok": {"kind": "value", "value": "Data - LAN"}})
http(A, "get_storage_path", {}, "GET", "/config?action=get&paramid=eParamID_StoragePath",
     http_reply={"status": 200, "body":
                 '{"paramid":"2063663370","name":"eParamID_StoragePath","value":"/mnt/S1/AJA","value_name":""}'},
     expect_result={"ok": {"kind": "value", "value": "/mnt/S1/AJA"}})
http(A, "set_file_system_format_hfs", {}, "GET", _CFG + "eParamID_FileSystemFormat&value=0")
http(A, "set_file_system_format_exfat", {}, "GET", _CFG + "eParamID_FileSystemFormat&value=1")
http(A, "format_media", {}, "GET", _CFG + "eParamID_StorageCommand&value=4")
# /mediaedit, sent exactly as AJA's example URL.
http(A, "subclip_start", {"source": "/mnt/S1/AJA/clip04.mov", "destination": "/mnt/S1/AJA/subclip04.mov",
                          "start": "10:00:20:00", "duration": "00:01:00:00"}, "GET",
     "/mediaedit?action=subclip&arg1=/mnt/S1/AJA/clip04.mov&arg2=/mnt/S1/AJA/subclip04.mov"
     "&arg3=10:00:20:00&arg4=00:01:00:00",
     http_reply={"status": 200, "body": "error: none\nid: 3146\n"},
     expect_result={"ok": {"kind": "value", "value": {"error": "none", "id": "3146"}}})
http(A, "subclip_status", {"id": 3146}, "GET", "/mediaedit?action=status&id=3146",
     http_reply={"status": 200, "body": "error: none\nid: 3146\nstatus: 44.75% complete\nrunning: yes\n"
                                        "time_now: 1472148646\ntime_start: 1472148607\ntime_stop: 0\n"},
     expect_result={"ok": {"kind": "value", "value": {
         "error": "none", "id": "3146", "status": "44.75% complete", "running": "yes",
         "time_now": "1472148646", "time_start": "1472148607", "time_stop": "0"}}})
http(A, "subclip_kill", {"id": 3146}, "GET", "/mediaedit?action=kill&id=3146",
     http_reply={"status": 200, "body": "error: none\nid: 3146\n"},
     expect_result={"ok": {"kind": "value", "value": {"error": "none", "id": "3146"}}})

# Inputs (api.txt descriptor examples; chapter 4).
http(A, "set_video_input_sdi", {}, "GET", _CFG + "eParamID_VideoInSelect&value=0")
http(A, "set_video_input_hdmi", {}, "GET", _CFG + "eParamID_VideoInSelect&value=1")
http(A, "set_video_input_component", {}, "GET", _CFG + "eParamID_VideoInSelect&value=2")
http(A, "set_audio_input_sdi", {}, "GET", _CFG + "eParamID_AudioInSelect&value=0")
http(A, "set_audio_input_rca", {}, "GET", _CFG + "eParamID_AudioInSelect&value=1")
http(A, "set_audio_input_xlr", {}, "GET", _CFG + "eParamID_AudioInSelect&value=2")
http(A, "set_audio_input_hdmi", {}, "GET", _CFG + "eParamID_AudioInSelect&value=3")
http(A, "set_video_input_1_sdi", {"sdi": 3}, "GET", _CFG + "eParamID_VideoInput_1&value=2")
http(A, "set_video_input_1_hdmi", {"hdmi": 1}, "GET", _CFG + "eParamID_VideoInput_1&value=4")
http(A, "set_audio_input_1_follow_video", {}, "GET", _CFG + "eParamID_AudioInput_1&value=0")
http(A, "set_audio_input_1_analog", {}, "GET", _CFG + "eParamID_AudioInput_1&value=1")

# Monitoring (chapters 4 and 6).
http(A, "set_headphone_monitor_channel", {"channel": 4}, "GET",
     _CFG + "eParamID_HeadphoneMonitorChannel&value=3")
http(A, "set_hdmi_monitor_channel", {"channel": 1}, "GET", _CFG + "eParamID_HDMIMonitorChannel&value=0")
http(A, "set_hdmi_monitor_all", {}, "GET", _CFG + "eParamID_HDMIMonitorChannel&value=4")
http(A, "set_sdi_monitor_channel", {"channel": 2}, "GET", _CFG + "eParamID_SDIMonitorChannel&value=1")
http(A, "set_sdi_monitor_all", {}, "GET", _CFG + "eParamID_SDIMonitorChannel&value=4")
http(A, "set_hdmi_out_channel", {"channel": 3}, "GET", _CFG + "eParamID_HDMIOutChannel&value=2")
http(A, "set_hdmi_out_all", {}, "GET", _CFG + "eParamID_HDMIOutChannel&value=4")
http(A, "set_headphone_channels", {"pair": 8}, "GET", _CFG + "eParamID_AudioChannelsFocus&value=7")
http(A, "set_headphone_encode_channel", {"channel": 1}, "GET",
     _CFG + "eParamID_AudioEncodeChannelFocus&value=0")

# Telemetry: replies to the poll and to commands (chapter 1 JSON reply shape).
telemetry(A, "transport", inbound_http={
    "path": "/config?action=get&paramid=eParamID_TransportState",
    "body": '{"paramid":"1","name":"eParamID_TransportState","value":"2","value_name":"Recording"}'},
    expect_state={"params": {"eParamID_TransportState": {"value": "2", "value_name": "Recording"}},
                  "transport": {"state": "Recording", "state_code": 2}})
telemetry(A, "timecode", inbound_http={
    "path": "/config?action=get&paramid=eParamID_DisplayTimecode",
    "body": '{"paramid":"1","name":"eParamID_DisplayTimecode","value":"00:00:10:05","value_name":""}'},
    expect_state={"params": {"eParamID_DisplayTimecode": {"value": "00:00:10:05", "value_name": ""}},
                  "timecode": {"display": "00:00:10:05"}})
telemetry(A, "current-clip", inbound_http={
    "path": "/config?action=get&paramid=eParamID_CurrentClip",
    "body": '{"paramid":"1","name":"eParamID_CurrentClip","value":"Clip1ATK6.MOV","value_name":""}'},
    expect_state={"params": {"eParamID_CurrentClip": {"value": "Clip1ATK6.MOV", "value_name": ""}},
                  "clip": {"current": "Clip1ATK6.MOV"}})
telemetry(A, "media-state", inbound_http={
    "path": "/config?action=get&paramid=eParamID_MediaState",
    "body": '{"paramid":"1","name":"eParamID_MediaState","value":"0","value_name":"Record-Play"}'},
    expect_state={"params": {"eParamID_MediaState": {"value": "0", "value_name": "Record-Play"}},
                  "media": {"state": "Record-Play", "data_lan": False}})
telemetry(A, "storage-path", inbound_http={
    "path": "/config?action=get&paramid=eParamID_StoragePath",
    "body": '{"paramid":"2063663370","name":"eParamID_StoragePath","value":"/mnt/S1/AJA","value_name":""}'},
    expect_state={"params": {"eParamID_StoragePath": {"value": "/mnt/S1/AJA", "value_name": ""}},
                  "media": {"storage_path": "/mnt/S1/AJA"}})
telemetry(A, "set-reply", inbound_http={
    "path": "/config?action=set&paramid=eParamID_VideoInSelect&value=1",
    "body": '{"paramid":"33619978","name":"eParamID_VideoInSelect","value":"1","value_name":"HDMI"}'},
    expect_state={"params": {"eParamID_VideoInSelect": {"value": "1", "value_name": "HDMI"}}})
telemetry(A, "not-json", inbound_http={
    "path": "/config?action=get&paramid=eParamID_TransportState",
    "body": "<html><head><title>404 Not Found</title></head></html>"},
    expect_state={})
