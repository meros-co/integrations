H = "aja-helo"
# AJA HELO and HELO Plus: GET /config?action=get|set&paramid=...&value=...
# (AJA REST API, gitlab.aja.com/pub/rest_api, chapters 1-3 and 8, April 2026;
# HELO bash and python examples). Query values are percent-encoded, keeping
# only RFC 3986 unreserved characters. Reply bodies follow AJA's documented
# JSON reply {paramid, name, value, value_name}; the paramid numbers are AJA's
# where chapter 3 gives them, placeholders otherwise.

_HSET = "/config?action=set&paramid="
_HGET = "/config?action=get&paramid="


def _hreply(name, value, value_name=None, paramid="1"):
    vn = value if value_name is None else value_name
    return '{"paramid":"%s","name":"%s","value":"%s","value_name":"%s"}' % (paramid, name, value, vn)


# Generic access (chapters 1-2).
http(H, "get_parameter", {"paramid": "eParamID_ReplicatorRecordState"}, "GET",
     _HGET + "eParamID_ReplicatorRecordState",
     http_reply={"status": 200, "body": _hreply("eParamID_ReplicatorRecordState", "1", "eRRSIdle", "2097225226")},
     expect_result={"ok": {"kind": "value", "value": {
         "paramid": "2097225226", "name": "eParamID_ReplicatorRecordState", "value": "1",
         "value_name": "eRRSIdle"}}})
# Chapter 3: with User Authentication on, the request goes after the login
# (POST /authenticator/login password_provided=...), with its session cookie.
http(H, "get_parameter", {"paramid": "eParamID_ReplicatorRecordState"}, "GET",
     _HGET + "eParamID_ReplicatorRecordState",
     file="get_parameter.with-login", settings={"user_authentication": True, "password": "honk"},
     http_reply={"status": 200, "body": _hreply("eParamID_ReplicatorRecordState", "3", "eRRSFailingInIdle",
                                                "2097225226")},
     expect_result={"ok": {"kind": "value", "value": {
         "paramid": "2097225226", "name": "eParamID_ReplicatorRecordState", "value": "3",
         "value_name": "eRRSFailingInIdle"}}})
# Chapter 1: eParamID_VideoInSelect set to SDI; chapter 3: the HTML 404 page
# a unit with User Authentication on gives a request without the session.
http(H, "set_parameter", {"paramid": "eParamID_VideoInSelect", "value": "0"}, "GET",
     _HSET + "eParamID_VideoInSelect&value=0",
     http_reply={"status": 200, "body": _hreply("eParamID_VideoInSelect", "0", "SDI", "33619978")},
     expect_result={"ok": {"kind": "value", "value": {
         "paramid": "33619978", "name": "eParamID_VideoInSelect", "value": "0", "value_name": "SDI"}}})
http(H, "set_parameter", {"paramid": "eParamID_VideoInSelect", "value": "1"}, "GET",
     _HSET + "eParamID_VideoInSelect&value=1", file="set_parameter.not-logged-in",
     http_reply={"status": 404, "body": "<!DOCTYPE HTML PUBLIC \"-//IETF//DTD HTML 2.0//EN\">"
                                        "<html><head><title>404 Not Found</title></head><body>"
                                        "The document requested was not found on this server.</body></html>"},
     expect_result={"error": {"error": "device_error", "code": "404"}})
http(H, "get_descriptor", {"paramid": "eParamID_ReplicatorCommand"}, "GET",
     "/descriptors?paramid=eParamID_ReplicatorCommand")
http(H, "get_all_descriptors", {}, "GET", "/desc.json")
# Chapter 8: the definitions the unit serves.
http(H, "get_proto_definition", {"file": "helo_plus_datafile.proto"}, "GET", "/helo_plus_datafile.proto",
     http_reply={"status": 200, "body": "package aja.config;"},
     expect_result={"ok": {"kind": "value", "value": "package aja.config;"}})
http(H, "get_proto_definition", {"file": "alarm.proto"}, "GET", "/alarm.proto", file="get_proto_definition.alarm")
# helo_recall_preset.py: connect returns {"connectionid"}; the wait returns a
# list of events with param_id (the other fields as AJA documents them for
# Ki Pro, values here placeholders).
http(H, "open_event_connection", {}, "GET", "/config?action=connect",
     http_reply={"status": 200, "body": '{"connectionid":"20"}'},
     expect_result={"ok": {"kind": "value", "value": "20"}})
http(H, "wait_for_events", {"connection": 20}, "GET",
     "/config?action=wait_for_config_events&connectionid=20",
     http_reply={"status": 200, "body":
                 '[{"param_id":"eParamID_RegisterRecallResult","param_type":"enum","int_value":2,'
                 '"str_value":"Complete"}]'},
     expect_result={"ok": {"kind": "value", "value": [
         {"param_id": "eParamID_RegisterRecallResult", "param_type": "enum", "int_value": 2,
          "str_value": "Complete"}]}})

# Recording and streaming: eParamID_ReplicatorCommand (chapter 3).
http(H, "start_recording", {}, "GET", _HSET + "eParamID_ReplicatorCommand&value=1",
     http_reply={"status": 200, "body": _hreply("eParamID_ReplicatorCommand", "1", "Record Command")},
     expect_result={"ok": {"kind": "ack"}})
http(H, "start_recording", {}, "GET", _HSET + "eParamID_ReplicatorCommand&value=1", model="helo-plus",
     file="start_recording.helo-plus")
http(H, "stop_recording", {}, "GET", _HSET + "eParamID_ReplicatorCommand&value=2")
http(H, "start_streaming", {}, "GET", _HSET + "eParamID_ReplicatorCommand&value=3")
http(H, "stop_streaming", {}, "GET", _HSET + "eParamID_ReplicatorCommand&value=4")
http(H, "shutdown", {}, "GET", _HSET + "eParamID_ReplicatorCommand&value=5")
http(H, "get_record_state", {}, "GET", _HGET + "eParamID_ReplicatorRecordState",
     http_reply={"status": 200, "body": _hreply("eParamID_ReplicatorRecordState", "2", "eRRSRecording",
                                                "2097225226")},
     expect_result={"ok": {"kind": "value", "value": "eRRSRecording"}})
http(H, "get_stream_state", {}, "GET", _HGET + "eParamID_ReplicatorStreamState")

# Profiles (chapter 3, 0 to 9).
http(H, "select_recording_profile", {"profile": 9}, "GET", _HSET + "eParamID_RecordingProfileSel&value=9")
http(H, "get_recording_profile", {}, "GET", _HGET + "eParamID_RecordingProfileSel",
     http_reply={"status": 200, "body": _hreply("eParamID_RecordingProfileSel", "4")},
     expect_result={"ok": {"kind": "value", "value": "4"}})
http(H, "select_streaming_profile", {"profile": 0}, "GET", _HSET + "eParamID_StreamingProfileSel&value=0")
http(H, "get_streaming_profile", {}, "GET", _HGET + "eParamID_StreamingProfileSel")

# Recording name: chapter 3's own example.
http(H, "set_filename_prefix", {"prefix": "Biology Lecture"}, "GET",
     _HSET + "eParamID_FilenamePrefix&value=Biology%20Lecture")
http(H, "set_filename_prefix", {"prefix": "Física/1"}, "GET",
     _HSET + "eParamID_FilenamePrefix&value=F%C3%ADsica%2F1", file="set_filename_prefix.encoded")
http(H, "get_filename_prefix", {}, "GET", _HGET + "eParamID_FilenamePrefix",
     http_reply={"status": 200, "body": _hreply("eParamID_FilenamePrefix", "clip", "")},
     expect_result={"ok": {"kind": "value", "value": "clip"}})

# Media state (chapter 3).
http(H, "enter_data_lan_mode", {}, "GET", _HSET + "eParamID_MediaState&value=1")
# A clip saved to the host file named (never written by a vector).
http(H, "download_clip", {"clip": "clip_0001.mov", "to": {"file": "clip_0001.mov"}}, "GET", "/media0/clip_0001.mov",
     http_reply={"status": 200, "body": ""},
     expect_result={"ok": {"kind": "value", "value": {"file": "clip_0001.mov", "size": None, "content_type": None}}})
http(H, "enter_record_stream_mode", {}, "GET", _HSET + "eParamID_MediaState&value=0")
http(H, "get_media_state", {}, "GET", _HGET + "eParamID_MediaState",
     http_reply={"status": 200, "body": _hreply("eParamID_MediaState", "1", "Data-LAN")},
     expect_result={"ok": {"kind": "value", "value": "Data-LAN"}})

# Authentication (chapter 3).
http(H, "enable_authentication", {}, "GET", _HSET + "eParamID_Authentication&value=1")
http(H, "disable_authentication", {}, "GET", _HSET + "eParamID_Authentication&value=0")
http(H, "get_authentication", {}, "GET", _HGET + "eParamID_Authentication",
     http_reply={"status": 200, "body": _hreply("eParamID_Authentication", "0", "Disabled")},
     expect_result={"ok": {"kind": "value", "value": "Disabled"}})

# Presets (chapter 2; chapter 3's recall of preset 2 and its result).
http(H, "save_preset", {"preset": 4}, "GET", _HSET + "eParamID_RegisterSave&value=4")
http(H, "recall_preset", {"preset": 2}, "GET", _HSET + "eParamID_RegisterRecall&value=2",
     http_reply={"status": 200, "body": _hreply("eParamID_RegisterRecall", "2")},
     expect_result={"ok": {"kind": "ack"}})
http(H, "get_recall_result", {}, "GET", _HGET + "eParamID_RegisterRecallResult",
     http_reply={"status": 200, "body": _hreply("eParamID_RegisterRecallResult", "2", "Complete", "1560412160")},
     expect_result={"ok": {"kind": "value", "value": "Complete"}})

# Video source (chapters 1-2: 0 SDI, 1 HDMI, 2 Test Pattern).
http(H, "set_video_input", {"input": "hdmi"}, "GET", _HSET + "eParamID_VideoInSelect&value=1")
http(H, "set_video_input", {"input": "test_pattern"}, "GET", _HSET + "eParamID_VideoInSelect&value=2",
     file="set_video_input.test-pattern")
http(H, "get_video_input", {}, "GET", _HGET + "eParamID_VideoInSelect",
     http_reply={"status": 200, "body": _hreply("eParamID_VideoInSelect", "1", "HDMI", "33619978")},
     expect_result={"ok": {"kind": "value", "value": "HDMI"}})

# Streaming bit rate instance AJA names (chapter 2: 100 to 20000).
http(H, "set_streaming_video_bitrate_1", {"bitrate": 20000}, "GET",
     _HSET + "eParamID_StreamingVideoBitrate_1&value=20000")
http(H, "get_streaming_video_bitrate_1", {}, "GET", _HGET + "eParamID_StreamingVideoBitrate_1")

# Telemetry: replies to gets and sets (the event forms go through the
# aja-config-events extension, tested in its own tests).
_RS = _HGET + "eParamID_ReplicatorRecordState"
telemetry(H, "record-state", inbound_http={
    "path": _RS, "body": _hreply("eParamID_ReplicatorRecordState", "1", "eRRSIdle", "2097225226")},
    expect_state={"params": {"eParamID_ReplicatorRecordState": {"value": "1", "value_name": "eRRSIdle"}},
                  "record": {"state": "eRRSIdle", "state_code": 1}})
telemetry(H, "stream-state", inbound_http={
    "path": _HGET + "eParamID_ReplicatorStreamState",
    "body": _hreply("eParamID_ReplicatorStreamState", "2", "Streaming")},
    expect_state={"params": {"eParamID_ReplicatorStreamState": {"value": "2", "value_name": "Streaming"}},
                  "stream": {"state": "Streaming", "state_code": 2}})
telemetry(H, "replicator-command-reply", inbound_http={
    "path": _HSET + "eParamID_ReplicatorCommand&value=3",
    "body": _hreply("eParamID_ReplicatorCommand", "3", "Stream Command")},
    expect_state={"params": {"eParamID_ReplicatorCommand": {"value": "3", "value_name": "Stream Command"}}},
    expect_then_send=[{"method": "GET", "target": _HGET + "eParamID_ReplicatorRecordState"},
                      {"method": "GET", "target": _HGET + "eParamID_ReplicatorStreamState"}])
telemetry(H, "recording-profile", inbound_http={
    "path": _HSET + "eParamID_RecordingProfileSel&value=7",
    "body": _hreply("eParamID_RecordingProfileSel", "7")},
    expect_state={"params": {"eParamID_RecordingProfileSel": {"value": "7", "value_name": "7"}},
                  "profiles": {"recording": 7}})
telemetry(H, "streaming-profile", inbound_http={
    "path": _HGET + "eParamID_StreamingProfileSel",
    "body": _hreply("eParamID_StreamingProfileSel", "0")},
    expect_state={"params": {"eParamID_StreamingProfileSel": {"value": "0", "value_name": "0"}},
                  "profiles": {"streaming": 0}})
telemetry(H, "filename-prefix", inbound_http={
    "path": _HSET + "eParamID_FilenamePrefix&value=Biology%20Lecture",
    "body": _hreply("eParamID_FilenamePrefix", "Biology Lecture", "")},
    expect_state={"params": {"eParamID_FilenamePrefix": {"value": "Biology Lecture", "value_name": ""}},
                  "record": {"filename_prefix": "Biology Lecture"}})
telemetry(H, "media-state", inbound_http={
    "path": _HGET + "eParamID_MediaState", "body": _hreply("eParamID_MediaState", "0", "Record-Stream")},
    expect_state={"params": {"eParamID_MediaState": {"value": "0", "value_name": "Record-Stream"}},
                  "media": {"state": "Record-Stream", "data_lan": False}})
telemetry(H, "authentication", inbound_http={
    "path": _HGET + "eParamID_Authentication", "body": _hreply("eParamID_Authentication", "1", "Login")},
    expect_state={"params": {"eParamID_Authentication": {"value": "1", "value_name": "Login"}},
                  "authentication": {"mode": "Login", "enabled": True}})
telemetry(H, "video-input", inbound_http={
    "path": _HGET + "eParamID_VideoInSelect", "body": _hreply("eParamID_VideoInSelect", "2", "Test Pattern")},
    expect_state={"params": {"eParamID_VideoInSelect": {"value": "2", "value_name": "Test Pattern"}},
                  "inputs": {"video": "test_pattern"}})
telemetry(H, "streaming-bitrate", inbound_http={
    "path": _HGET + "eParamID_StreamingVideoBitrate_1",
    "body": _hreply("eParamID_StreamingVideoBitrate_1", "5000")},
    expect_state={"params": {"eParamID_StreamingVideoBitrate_1": {"value": "5000", "value_name": "5000"}},
                  "stream": {"profile_1_video_bitrate": 5000}})
telemetry(H, "preset-saved", inbound_http={
    "path": _HSET + "eParamID_RegisterSave&value=4", "body": _hreply("eParamID_RegisterSave", "4")},
    expect_state={"params": {"eParamID_RegisterSave": {"value": "4", "value_name": "4"}},
                  "presets": {"last_saved": 4}})
telemetry(H, "preset-recalled", inbound_http={
    "path": _HSET + "eParamID_RegisterRecall&value=2", "body": _hreply("eParamID_RegisterRecall", "2")},
    expect_state={"params": {"eParamID_RegisterRecall": {"value": "2", "value_name": "2"}},
                  "presets": {"last_recalled": 2}},
    expect_then_send=[{"method": "GET", "target": _HGET + "eParamID_RegisterRecallResult"}])
telemetry(H, "recall-result", inbound_http={
    "path": _HGET + "eParamID_RegisterRecallResult",
    "body": _hreply("eParamID_RegisterRecallResult", "2", "Complete", "1560412160")},
    expect_state={"params": {"eParamID_RegisterRecallResult": {"value": "2", "value_name": "Complete"}},
                  "presets": {"recall_result": "Complete", "recall_result_code": 2}})
# Chapter 8: a protobuf alarm, kept as its base64 text.
telemetry(H, "alarm", inbound_http={
    "path": _HGET + "eParamID_Dropped_Frames_Alarm",
    "body": _hreply("eParamID_Dropped_Frames_Alarm", "CAAQABogCg5Ecm9wcGVkIEZyYW1lcxIORHJvcHBlZCBGcmFtZXM=",
                    "", "2114391040")},
    expect_state={"params": {"eParamID_Dropped_Frames_Alarm": {
        "value": "CAAQABogCg5Ecm9wcGVkIEZyYW1lcxIORHJvcHBlZCBGcmFtZXM=", "value_name": ""}}})
telemetry(H, "not-json", inbound_http={
    "path": _RS, "body": "<html><head><title>404 Not Found</title></head><body></body></html>"},
    expect_state={})
