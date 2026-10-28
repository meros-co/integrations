TDK = "teradek-prism"
# Teradek Prism, REST API v1 (teradekdev.com: Getting Started, Authentication,
# API Reference for firmware 2.26 and 2.25). Every request is
# https://<device>/api/v1/..., JSON bodies wrapped in "settings" as the
# reference's examples are; POST actions go without a body. Example replies
# are the reference's own, shortened where it says "additional fields
# omitted". The token endpoint /oauth/token is the oauth endpoint on port 443.

_J = "application/json"
_FORM = "application/x-www-form-urlencoded"
_OK = '{"status":"ok","message":"Set successfully","timestamp":1792708131,"settings":{}}'


def _get(command, input, target, **extra):
    http(TDK, command, input, "GET", target, **extra)


def _put(command, input, target, body, **extra):
    V.append({"spec": TDK, "command": command, "input": input,
              "expect_request": {"method": "PUT", "target": target, "body": body, "content_type": _J}, **extra})


def _post(command, input, target, body=None, **extra):
    request = {"method": "POST", "target": target}
    if body is not None:
        request["body"] = body
        request["content_type"] = _J
    V.append({"spec": TDK, "command": command, "input": input, "expect_request": request, **extra})


def _delete(command, input, target, **extra):
    http(TDK, command, input, "DELETE", target, **extra)


def _ack(body=_OK):
    return {"http_reply": {"status": 200, "body": body}, "expect_result": {"ok": {"kind": "ack"}}}


# ── Authentication: form-encoded grants to /oauth/token (Authentication) ──
_TOKENS = ('{"expires_in":3600,"token_type":"Bearer","refresh_token":"bNLzvBOEqwDNYApV9jduhjlVXIoOp9kB",'
           '"access_token":"rYDVtxfHdH7Igk5qdMEWMJZAJpg1Gjvy"}')
V.append({"spec": TDK, "command": "get_access_token", "input": {"password": "p@ss word"},
          "expect_request": {"method": "POST", "port": 443, "target": "/oauth/token",
                             "body": "grant_type=password&password=p%40ss%20word", "content_type": _FORM},
          "http_reply": {"status": 200, "body": _TOKENS},
          "expect_result": {"ok": {"kind": "value", "value": {
              "expires_in": 3600, "token_type": "Bearer", "refresh_token": "bNLzvBOEqwDNYApV9jduhjlVXIoOp9kB",
              "access_token": "rYDVtxfHdH7Igk5qdMEWMJZAJpg1Gjvy"}}}})
# A wrong password: 401 fails this command only (refusal_status is empty).
V.append({"spec": TDK, "command": "get_access_token", "input": {"password": "wrong"}, "file": "get_access_token-refused",
          "expect_request": {"method": "POST", "port": 443, "target": "/oauth/token",
                             "body": "grant_type=password&password=wrong", "content_type": _FORM},
          "http_reply": {"status": 401, "body": '{"error":"invalid_grant"}'},
          "expect_result": {"error": {"error": "device_error", "code": "401"}}})
V.append({"spec": TDK, "command": "refresh_access_token", "input": {"refresh_token": "bNLzvBOEqwDNYApV9jduhjlVXIoOp9kB"},
          "expect_request": {"method": "POST", "port": 443, "target": "/oauth/token",
                             "body": "grant_type=refresh_token&refresh_token=bNLzvBOEqwDNYApV9jduhjlVXIoOp9kB",
                             "content_type": _FORM},
          "http_reply": {"status": 200, "body": _TOKENS},
          "expect_result": {"ok": {"kind": "value", "value": {
              "expires_in": 3600, "token_type": "Bearer", "refresh_token": "bNLzvBOEqwDNYApV9jduhjlVXIoOp9kB",
              "access_token": "rYDVtxfHdH7Igk5qdMEWMJZAJpg1Gjvy"}}}})
# With a token set, it goes as Authorization: Bearer (the headers are not
# part of the vector; the request is the same).
_get("get_api_root", {}, "/api/v1", settings={"token": "rYDVtxfHdH7Igk5qdMEWMJZAJpg1Gjvy"},
     http_reply={"status": 200, "body": '{"endpoints":["decoders","encoders","inputs","network","settings","streams","system"]}'},
     expect_result={"ok": {"kind": "value", "value": ["decoders", "encoders", "inputs", "network", "settings",
                                                       "streams", "system"]}})

# ── System ──
_get("get_system_info", {}, "/api/v1/system/info",
     http_reply={"status": 200, "body": '{"settings":{"Device Model":"Prism-855","Serial":"1285500073",'
                                        '"Firmware Version":"2.25.20260217"},"timestamp":1771463349,'
                                        '"message":"Statuses received","status":"ok"}'},
     expect_result={"ok": {"kind": "value", "value": {"Device Model": "Prism-855", "Serial": "1285500073",
                                                      "Firmware Version": "2.25.20260217"}}})
_get("check_firmware", {}, "/api/v1/system/firmware/check")
_get("get_firmware_settings", {}, "/api/v1/system/firmware/settings")
_get("describe_firmware_settings", {}, "/api/v1/system/firmware/settings?command=description")
_put("set_firmware_settings", {"values": {"auto_check": False}}, "/api/v1/system/firmware/settings",
     '{"settings":{"auto_check":false}}', **_ack())
_put("set_firmware_auto_check", {"enabled": True}, "/api/v1/system/firmware/settings",
     '{"settings":{"auto_check":true}}')
_post("upgrade_firmware", {}, "/api/v1/system/firmware/upgrade")

# ── Inputs, encoder pipelines ──
_get("list_inputs", {}, "/api/v1/inputs")
_get("list_encoders", {}, "/api/v1/encoders")
_post("add_encoder", {}, "/api/v1/encoders",
      http_reply={"status": 200, "body": '{"message":"Encoder added","timestamp":1792708131,"id":3,"status":"ok"}'},
      expect_result={"ok": {"kind": "value", "value": 3}})
# Encoder limit reached: status error, HTTP 400 (Response Format).
_post("add_encoder", {}, "/api/v1/encoders", file="add_encoder-refused",
      http_reply={"status": 400, "body": '{"message":"Unable to add a new Track","timestamp":1792708131,"status":"error"}'},
      expect_result={"error": {"error": "device_error", "code": "400"}})
_get("get_capacity", {}, "/api/v1/encoders/capacity")
_get("get_encoder", {"encoder": 2}, "/api/v1/encoders/2")
_put("set_encoder", {"encoder": 0, "values": {"name": "Camera A"}}, "/api/v1/encoders/0",
     '{"settings":{"name":"Camera A"}}')
_put("set_encoder_name", {"encoder": 1, "name": "Camera \"B\""}, "/api/v1/encoders/1",
     '{"settings":{"name":"Camera \\"B\\""}}')
_put("set_encoder_physical_input", {"encoder": 0, "input": "hdmi0"}, "/api/v1/encoders/0",
     '{"settings":{"input_type":"physical","physical_input":"hdmi0"}}')
_put("set_encoder_network_input", {"encoder": 1, "input": "Input/Stream/4"}, "/api/v1/encoders/1",
     '{"settings":{"input_type":"network","network_input":"Input/Stream/4"}}')
_put("set_encoder_audio_source", {"encoder": 0, "source": "analog"}, "/api/v1/encoders/0",
     '{"settings":{"audio_source":"analog"}}')
_delete("delete_encoder", {"encoder": 3}, "/api/v1/encoders/3",
        **_ack('{"message":"Encoder 3 removed","timestamp":1792708131,"status":"ok"}'))
_delete("delete_encoder", {"encoder": 0}, "/api/v1/encoders/0", file="delete_encoder-last",
        http_reply={"status": 400, "body": '{"message":"Unable to delete the last pipeline","status":"error"}'},
        expect_result={"error": {"error": "device_error", "code": "400"}})
_post("restart_encoder", {"encoder": 0}, "/api/v1/encoders/0/restart",
      **_ack('{"message":"Restarting","timestamp":1792708131,"status":"ok"}'))
_get("list_encoder_streams", {"encoder": 0}, "/api/v1/encoders/0/streams")
_get("get_encoder_info", {"encoder": 0}, "/api/v1/encoders/0/info")

# ── Encoder video ──
_get("get_encoder_video_settings", {"encoder": 0}, "/api/v1/encoders/0/video/settings")
_get("describe_encoder_video_settings", {"encoder": 0}, "/api/v1/encoders/0/video/settings?command=description")
_put("set_encoder_video_settings", {"encoder": 0, "values": {"gop_length": 60}}, "/api/v1/encoders/0/video/settings",
     '{"settings":{"gop_length":60}}')
_put("set_encoder_codec", {"encoder": 0, "codec": "hevc"}, "/api/v1/encoders/0/video/settings",
     '{"settings":{"codec":"hevc"}}')
_put("set_encoder_bitrate", {"encoder": 0, "kbps": 6000}, "/api/v1/encoders/0/video/settings",
     '{"settings":{"bitrate_list":6000}}', **_ack('{"status":"ok","message":"Set successfully","settings":{"bitrate_list":6000}}'))
# The device checks each value (Response Format).
_put("set_encoder_bitrate", {"encoder": 0, "kbps": 6001}, "/api/v1/encoders/0/video/settings",
     '{"settings":{"bitrate_list":6001}}', file="set_encoder_bitrate-invalid",
     http_reply={"status": 400, "body": '{"status":"error","message":"bitrate_list: Invalid choice"}'},
     expect_result={"error": {"error": "device_error", "code": "400"}})
_put("set_encoder_gop_length", {"encoder": 0, "frames": 30}, "/api/v1/encoders/0/video/settings",
     '{"settings":{"gop_length":30}}')

# ── Encoder audio ──
_get("list_encoder_audio_tracks", {"encoder": 0}, "/api/v1/encoders/0/audio")
_get("get_encoder_audio_settings", {"encoder": 0}, "/api/v1/encoders/0/audio/settings")
_get("describe_encoder_audio_settings", {"encoder": 0}, "/api/v1/encoders/0/audio/settings?command=description")
_put("set_encoder_audio_settings", {"encoder": 0, "values": {"bitrate_list": 256000}}, "/api/v1/encoders/0/audio/settings",
     '{"settings":{"bitrate_list":256000}}')
_put("set_encoder_audio_enabled", {"encoder": 0, "enabled": False}, "/api/v1/encoders/0/audio/settings",
     '{"settings":{"enable":false}}')
_get("get_encoder_audio_track", {"encoder": 2, "track": 2}, "/api/v1/encoders/2/audio/2/settings")
# A track not attached to the encoder: 404 (Audio Settings).
_get("get_encoder_audio_track", {"encoder": 2, "track": 0}, "/api/v1/encoders/2/audio/0/settings",
     file="get_encoder_audio_track-not-attached",
     http_reply={"status": 404, "body": '{"status":"error","message":"Audio track 0 is not attached to encoder 2"}'},
     expect_result={"error": {"error": "device_error", "code": "404"}})
_get("describe_encoder_audio_track", {"encoder": 0, "track": 0}, "/api/v1/encoders/0/audio/0/settings?command=description")
_put("set_encoder_audio_track", {"encoder": 0, "track": 1, "values": {"channel_mode": "Stereo"}},
     "/api/v1/encoders/0/audio/1/settings", '{"settings":{"channel_mode":"Stereo"}}')
_put("set_encoder_audio_track_muted", {"encoder": 0, "track": 0, "muted": True}, "/api/v1/encoders/0/audio/0/settings",
     '{"settings":{"mute_track":true}}')

# ── Encoder metadata and SCTE ──
_get("get_encoder_metadata_settings", {"encoder": 0}, "/api/v1/encoders/0/metadata/settings")
_get("describe_encoder_metadata_settings", {"encoder": 0}, "/api/v1/encoders/0/metadata/settings?command=description")
_put("set_encoder_metadata_settings", {"encoder": 0, "values": {"dpi": False}}, "/api/v1/encoders/0/metadata/settings",
     '{"settings":{"dpi":false}}')
_put("set_encoder_closed_captions", {"encoder": 0, "enabled": True}, "/api/v1/encoders/0/metadata/settings",
     '{"settings":{"closed-captions":true}}')
_post("insert_scte_marker", {"encoder": 0}, "/api/v1/encoders/0/metadata/scte",
      **_ack('{"message":"SCTE message sent","status":"ok"}'))
_post("insert_scte_marker", {"encoder": 0}, "/api/v1/encoders/0/metadata/scte", file="insert_scte_marker-unlicensed",
      http_reply={"status": 400, "body": '{"message":"SCTE License Not Found","status":"error"}'},
      expect_result={"error": {"error": "device_error", "code": "400"}})
_post("insert_scte_marker_with", {"encoder": 0, "values": {"break_duration": 60000}}, "/api/v1/encoders/0/metadata/scte",
      '{"settings":{"break_duration":60000}}')

# ── Encoder recording ──
_get("get_encoder_record_settings", {"encoder": 0}, "/api/v1/encoders/0/record/settings")
_get("describe_encoder_record_settings", {"encoder": 0}, "/api/v1/encoders/0/record/settings?command=description")
_put("set_encoder_record_settings", {"encoder": 0, "values": {"max-size-time": 15}}, "/api/v1/encoders/0/record/settings",
     '{"settings":{"max-size-time":15}}')
_get("get_encoder_record_info", {"encoder": 0}, "/api/v1/encoders/0/record/info")
_post("start_encoder_recording", {"encoder": 0}, "/api/v1/encoders/0/record/start",
      **_ack('{"message":"Started","status":"ok"}'))
_post("start_encoder_recording", {"encoder": 0}, "/api/v1/encoders/0/record/start", file="start_encoder_recording-no-signal",
      http_reply={"status": 400, "body": '{"message":"Unable to record","status":"error"}'},
      expect_result={"error": {"error": "device_error", "code": "400"}})
_post("start_encoder_recording_with", {"encoder": 0, "values": {"format": "mp4"}}, "/api/v1/encoders/0/record/start",
      '{"settings":{"format":"mp4"}}')
_post("stop_encoder_recording", {"encoder": 0}, "/api/v1/encoders/0/record/stop",
      **_ack('{"message":"Stopped","status":"ok"}'))
# The reply is the JPEG itself: only its status is checked.
_post("set_encoder_snapshot_settings", {"encoder": 0, "width": 768, "height": 432, "quality": 80},
      "/api/v1/encoders/0/snapshot", '{"settings":{"width":768,"height":432,"quality":80}}',
      http_reply={"status": 200, "body": "JFIF"}, expect_result={"ok": {"kind": "ack"}})

# ── Decoder pipelines ──
_get("list_decoders", {}, "/api/v1/decoders")
_post("add_decoder", {}, "/api/v1/decoders",
      http_reply={"status": 200, "body": '{"message":"Decoder added","timestamp":1792708131,"id":1,"status":"ok"}'},
      expect_result={"ok": {"kind": "value", "value": 1}})
_get("get_decoder", {"decoder": 0}, "/api/v1/decoders/0")
_put("set_decoder", {"decoder": 0, "values": {"failover_timeout_ms": 2000}}, "/api/v1/decoders/0",
     '{"settings":{"failover_timeout_ms":2000}}')
_put("set_decoder_name", {"decoder": 0, "name": "Return Feed"}, "/api/v1/decoders/0", '{"settings":{"name":"Return Feed"}}')
_put("set_decoder_failover_timeout", {"decoder": 0, "ms": 1000}, "/api/v1/decoders/0",
     '{"settings":{"failover_timeout_ms":1000}}')
_delete("delete_decoder", {"decoder": 1}, "/api/v1/decoders/1",
        **_ack('{"message":"Decoder 1 removed","status":"ok"}'))
_post("restart_decoder", {"decoder": 0}, "/api/v1/decoders/0/restart")
_get("list_decoder_streams", {"decoder": 0}, "/api/v1/decoders/0/streams")
_get("get_decoder_info", {"decoder": 0}, "/api/v1/decoders/0/info")
# Encoder-only mode: every decoder path answers 404 (Decoders).
_get("get_decoder_info", {"decoder": 0}, "/api/v1/decoders/0/info", file="get_decoder_info-encoder-only",
     http_reply={"status": 404, "body": '{"status":"error","message":"Decoder not available"}'},
     expect_result={"error": {"error": "device_error", "code": "404"}})
_get("get_decoder_ingest", {"decoder": 0}, "/api/v1/decoders/0/video/mode")
_put("set_decoder_ingest", {"decoder": 0, "values": {"name": "Ingest 2"}}, "/api/v1/decoders/0/video/mode",
     '{"settings":{"name":"Ingest 2"}}')
_put("set_decoder_ingest_mode", {"decoder": 0, "mode": "SRT"}, "/api/v1/decoders/0/video/mode", '{"settings":{"mode":"SRT"}}')
_put("set_decoder_jitter_buffer", {"decoder": 0, "target_ms": 300, "max_ms": 500}, "/api/v1/decoders/0/video/mode",
     '{"settings":{"jitter-buffer-target-delay":300,"jitter-buffer-max-delay":500}}')
_get("get_decoder_video_settings", {"decoder": 0}, "/api/v1/decoders/0/video/settings")
_get("describe_decoder_video_settings", {"decoder": 0}, "/api/v1/decoders/0/video/settings?command=description")
_put("set_decoder_video_settings", {"decoder": 0, "values": {"udp_port": 9720}}, "/api/v1/decoders/0/video/settings",
     '{"settings":{"udp_port":9720}}')

# ── Audio output ──
_get("get_audio_output", {"decoder": 0}, "/api/v1/decoders/0/audio/settings")
_get("describe_audio_output", {"decoder": 0}, "/api/v1/decoders/0/audio/settings?command=description")
_put("set_audio_output", {"decoder": 0, "values": {"delay": 0}}, "/api/v1/decoders/0/audio/settings",
     '{"settings":{"delay":0}}')
_put("set_audio_output_enabled", {"decoder": 0, "enabled": True}, "/api/v1/decoders/0/audio/settings",
     '{"settings":{"enable":true}}')
_get("get_audio_output_channel", {"decoder": 0, "channel": 15}, "/api/v1/decoders/0/audio/15")
_get("describe_audio_output_channel", {"decoder": 0, "channel": 0}, "/api/v1/decoders/0/audio/0?command=description")
_put("set_audio_output_channel", {"decoder": 0, "channel": 3, "values": {"volume": 0}}, "/api/v1/decoders/0/audio/3",
     '{"settings":{"volume":0}}')
_put("set_audio_output_channel_muted", {"decoder": 0, "channel": 1, "muted": True}, "/api/v1/decoders/0/audio/1",
     '{"settings":{"mute":true}}')

# ── Decoder metadata and recording ──
_get("get_decoder_metadata_settings", {"decoder": 0}, "/api/v1/decoders/0/metadata/settings")
_get("describe_decoder_metadata_settings", {"decoder": 0}, "/api/v1/decoders/0/metadata/settings?command=description")
_put("set_decoder_metadata_settings", {"decoder": 0, "values": {"insert_dpi": True}}, "/api/v1/decoders/0/metadata/settings",
     '{"settings":{"insert_dpi":true}}')
_get("get_decoder_record_settings", {"decoder": 0}, "/api/v1/decoders/0/record/settings")
_get("describe_decoder_record_settings", {"decoder": 0}, "/api/v1/decoders/0/record/settings?command=description")
_put("set_decoder_record_settings", {"decoder": 0, "values": {"cyclic": True}}, "/api/v1/decoders/0/record/settings",
     '{"settings":{"cyclic":true}}')
_get("get_decoder_record_info", {"decoder": 0}, "/api/v1/decoders/0/record/info")
_post("start_decoder_recording", {"decoder": 0}, "/api/v1/decoders/0/record/start")
_post("stop_decoder_recording", {"decoder": 0}, "/api/v1/decoders/0/record/stop")
_post("set_decoder_snapshot_settings", {"decoder": 0, "width": 1280, "height": 720, "quality": 90},
      "/api/v1/decoders/0/snapshot", '{"settings":{"width":1280,"height":720,"quality":90}}')

# ── Streams ──
_get("list_streams", {}, "/api/v1/streams")
_post("add_stream", {}, "/api/v1/streams",
      http_reply={"status": 200, "body": '{"message":"Stream added","timestamp":1792708131,"id":10,"status":"ok"}'},
      expect_result={"ok": {"kind": "value", "value": 10}})
_delete("delete_stream", {"stream": 8}, "/api/v1/streams/8", **_ack('{"message":"Stream 8 removed","status":"ok"}'))
_delete("delete_stream", {"stream": 0}, "/api/v1/streams/0", file="delete_stream-protected",
        http_reply={"status": 400, "body": '{"message":"Cannot remove base stream 0!","status":"error"}'},
        expect_result={"error": {"error": "device_error", "code": "400"}})
_get("get_stream", {"stream": 0}, "/api/v1/streams/0")
_put("set_stream", {"stream": 0, "values": {"audio_track_selection": "all"}}, "/api/v1/streams/0",
     '{"settings":{"audio_track_selection":"all"}}')
_put("set_stream_name", {"stream": 0, "name": "Main Output"}, "/api/v1/streams/0", '{"settings":{"name":"Main Output"}}')
_put("set_stream_mode", {"stream": 0, "mode": "SRT"}, "/api/v1/streams/0", '{"settings":{"mode":"SRT"}}')
_put("set_stream_codec_modes", {"stream": 1, "mode_h264": "TS", "mode_hevc": "Disabled"}, "/api/v1/streams/1",
     '{"settings":{"mode-h264":"TS","mode-hevc":"Disabled"}}')
_get("get_stream_settings", {"stream": 0}, "/api/v1/streams/0/settings")
_get("get_stream_settings", {"stream": 5}, "/api/v1/streams/5/settings", file="get_stream_settings-disabled",
     http_reply={"status": 400, "body": '{"status":"error","message":"Stream is Disabled"}'},
     expect_result={"error": {"error": "device_error", "code": "400"}})
_get("describe_stream_settings", {"stream": 0}, "/api/v1/streams/0/settings?command=description")
_put("set_stream_settings", {"stream": 0, "values": {"latency": 500}}, "/api/v1/streams/0/settings",
     '{"settings":{"latency":500}}')
_get("get_stream_info", {"stream": 0}, "/api/v1/streams/0/info")
_post("start_stream", {"stream": 3}, "/api/v1/streams/3/start", **_ack('{"message":"Started","status":"ok"}'))
# TS, SRT and the like run with their pipeline: start is refused (Control).
_post("start_stream", {"stream": 2}, "/api/v1/streams/2/start", file="start_stream-always-on",
      http_reply={"status": 400, "body": '{"status":"error","message":"Invalid: start is not available for a TS stream in its current state"}'},
      expect_result={"error": {"error": "device_error", "code": "400"}})
_post("stop_stream", {"stream": 3}, "/api/v1/streams/3/stop")
_post("start_all_streams", {}, "/api/v1/streams/0/start_all")
_post("stop_all_streams", {}, "/api/v1/streams/0/stop_all")
for _cmd, _action in [("start_preview", "preview"), ("end_preview", "endpreview"), ("complete_stream", "complete"),
                      ("publish_stream", "publish"), ("unpublish_stream", "unpublish"),
                      ("broadcast_stream", "broadcast"), ("halt_stream", "halt"), ("cancel_stream", "cancel")]:
    _post(_cmd, {"stream": 4}, f"/api/v1/streams/4/{_action}")

# ── Network ──
_get("get_wired", {"port": 1}, "/api/v1/network/wired/1")
_get("get_wifi", {}, "/api/v1/network/wifi")
_get("describe_wifi", {}, "/api/v1/network/wifi?command=description")
_put("set_wifi", {"values": {"Network": "MyNetwork"}}, "/api/v1/network/wifi", '{"settings":{"Network":"MyNetwork"}}')
_get("get_modem", {"modem": 7}, "/api/v1/network/modem/7")

# ── Settings sync ──
_post("import_settings", {"host": "192.168.1.50", "password": "admin"}, "/api/v1/settings/sync/import",
      '{"type":"encoder","settings":{"host":"192.168.1.50","password":"admin"}}')
_post("push_settings", {"targets": [{"host": "192.168.1.51"}]}, "/api/v1/settings/sync/push",
      '{"targets":[{"host":"192.168.1.51"}]}')
_post("push_settings_to", {"host": "192.168.1.52", "password": "admin"}, "/api/v1/settings/sync/push",
      '{"targets":[{"host":"192.168.1.52","password":"admin"}]}')


# ── Telemetry ─────────────────────────────────────────────────────────────
def _t(name, path, body, state, **extra):
    telemetry(TDK, name, inbound_http={"path": path, "body": body, **extra.pop("http", {})},
              expect_state=state, **extra)


def _g(target):
    return {"method": "GET", "target": target}


_t("rate-limit", "/api/v1/system/firmware/upgrade", '{"status":"ok"}',
   {"rate_limit": {"limit": 60, "remaining": 57, "reset": 1792708191}},
   http={"headers": {"X-RateLimit-Limit": "60", "X-RateLimit-Remaining": "57", "X-RateLimit-Reset": "1792708191"}})
_t("system-info", "/api/v1/system/info",
   '{"settings":{"Device Model":"Prism-855","Serial":"1285500073","Firmware Version":"2.25.20260217"},'
   '"timestamp":1771463349,"message":"Statuses received","status":"ok"}',
   {"system": {"model": "Prism-855", "serial": "1285500073", "firmware_version": "2.25.20260217"}})
_t("firmware-check", "/api/v1/system/firmware/check",
   '{"settings":{"Latest Firmware":"2.24.20260106"},"timestamp":1771463367,"message":"Device is up to date","status":"ok"}',
   {"system": {"latest_firmware": "2.24.20260106", "firmware_check": "Device is up to date"}})
_t("firmware-settings", "/api/v1/system/firmware/settings",
   '{"timestamp":1771463350,"settings":{"auto_check":true},"status":"ok"}',
   {"system": {"firmware_auto_check": True}})
# A write's echo, then the setting read back.
_t("firmware-settings-write", "/api/v1/system/firmware/settings",
   '{"status":"ok","message":"Set successfully","settings":{"auto_check":false}}',
   {"system": {"firmware_auto_check": False}},
   http={"request": {"settings": {"auto_check": False}}},
   expect_then_send=[_g("/api/v1/system/firmware/settings")])
# A description is not state.
_t("firmware-settings-description", "/api/v1/system/firmware/settings?command=description",
   '{"settings":{"auto_check":{"type":"boolean","description":"Automatically check for firmware updates","default":true}},"status":"ok"}',
   {})

_INPUTS = json.dumps({"message": "Inputs retrieved", "timestamp": 1792708131, "video": [
    {"id": "sdi0", "label": "SDI 1", "port": "SDI", "index": 0, "info": {
        "Input Format": "1920x1080p60", "Input Scan": "Progressive", "Framerate": "60", "Color Depth": 10,
        "SubSampling": 422, "Colorimetry": "BT.709", "Range": "Legal", "EOTF": "SDR",
        "enc-input-resolution": "1920x1080p60"}},
    {"id": "hdmi0", "label": "HDMI", "port": "HDMI", "index": 0, "info": None}],
    "audio": [{"id": "embedded", "label": "Embedded"}, {"id": "usb", "label": "USB"}], "status": "ok"})
_t("inputs", "/api/v1/inputs", _INPUTS,
   {"inputs": {
       "video": {"sdi0": {"label": "SDI 1", "port": "SDI", "index": 0, "format": "1920x1080p60", "scan": "Progressive",
                          "framerate": "60", "color_depth": 10, "subsampling": 422, "colorimetry": "BT.709",
                          "range": "Legal", "eotf": "SDR", "resolution": "1920x1080p60"},
                 "hdmi0": {"label": "HDMI", "port": "HDMI", "index": 0}},
       "audio": {"embedded": {"label": "Embedded"}, "usb": {"label": "USB"}}}},
   # The analog input is no longer listed: the lists are replaced.
   state_before={"inputs": {"video": {"sdi1": {"label": "SDI 2"}}, "audio": {"analog": {"label": "Analog"}}}})
_t("capacity", "/api/v1/encoders/capacity",
   '{"message":"Statuses retrieved","settings":{"encode_budget_mpixps":497.7,"encode_used_mpixps":497.7,'
   '"encode_percent":100,"decode_budget_mpixps":497.7,"decode_used_mpixps":0,"decode_percent":0,'
   '"limiting":"Encode capacity","last_refusal":null,"physical_inputs_used":1,"physical_inputs_total":2},"status":"ok"}',
   {"capacity": {"encode_budget_mpixps": 497.7, "encode_used_mpixps": 497.7, "encode_percent": 100.0,
                 "decode_budget_mpixps": 497.7, "decode_used_mpixps": 0.0, "decode_percent": 0.0,
                 "limiting": "Encode capacity", "physical_inputs_used": 1, "physical_inputs_total": 2}})

# The encoder list: each encoder's status and configuration are read, then
# its streams'.
_ENCODERS = json.dumps({"message": "Pipelines retrieved", "timestamp": 1792708131, "encoders": [
    {"id": 0, "name": "Encoder 1", "state": "Encoding", "uptime": "01:16:43", "input_type": "physical",
     "physical_input": "sdi0", "network_input": "", "audio_source": "embedded", "streams": [6, 0], "audio_tracks": [0]}],
    "limit": 4, "minimum": 1, "status": "ok"})
_t("encoders", "/api/v1/encoders", _ENCODERS,
   {"pools": {"encoders": {"0": {"name": "Encoder 1"}}},
    "encoders": {"0": {"name": "Encoder 1", "state": "Encoding", "uptime": "01:16:43", "input_type": "physical",
                       "physical_input": "sdi0", "network_input": "", "audio_source": "embedded",
                       "streams": "[6,0]", "audio_tracks": "[0]"}},
    "limits": {"encoders": 4, "encoders_minimum": 1}},
   # Encoder 3 was deleted: the pool is replaced.
   state_before={"pools": {"encoders": {"3": {"name": "Encoder 4"}}}},
   expect_then_send=[_g("/api/v1/encoders/0/info"), _g("/api/v1/encoders/0/record/info"),
                     _g("/api/v1/encoders/0/video/settings"), _g("/api/v1/encoders/0/audio"),
                     _g("/api/v1/encoders/0/metadata/settings"), _g("/api/v1/encoders/0/record/settings"),
                     _g("/api/v1/streams/6"), _g("/api/v1/streams/6/settings"), _g("/api/v1/streams/6/info"),
                     _g("/api/v1/streams/0"), _g("/api/v1/streams/0/settings"), _g("/api/v1/streams/0/info")])
# add_encoder's answer on the same path: the list read again, the pool kept.
_t("encoder-added", "/api/v1/encoders", '{"message":"Encoder added","timestamp":1792708131,"id":3,"status":"ok"}',
   {"pools": {"encoders": {"0": {"name": "Encoder 1"}}}},
   state_before={"pools": {"encoders": {"0": {"name": "Encoder 1"}}}},
   expect_then_send=[_g("/api/v1/encoders")])
_t("encoder-removed", "/api/v1/encoders/3", '{"message":"Encoder 3 removed","timestamp":1792708131,"status":"ok"}',
   {"encoders": {"0": {"name": "Encoder 1"}}},
   state_before={"encoders": {"0": {"name": "Encoder 1"}, "3": {"name": "Encoder 4", "state": "Encoding"}}},
   expect_then_send=[_g("/api/v1/encoders")])
_t("encoder", "/api/v1/encoders/0", json.dumps({
    "message": "Settings retrieved successfully", "timestamp": 1792708131,
    "settings": {"id": 0, "name": "Encoder 1", "input_type": "physical", "physical_input": "sdi0", "network_input": "",
                 "audio_source": "embedded", "streams": [6, 3, 0], "audio_tracks": [0]},
    "info": {"State": "Encoding", "Uptime": "01:16:43", "StartedAt": 1792703528, "AudioSource": "embedded"},
    "endpoints": ["audio", "info"], "status": "ok"}),
   {"encoders": {"0": {"name": "Encoder 1", "input_type": "physical", "physical_input": "sdi0", "network_input": "",
                       "audio_source": "embedded", "streams": "[6,3,0]", "audio_tracks": "[0]", "state": "Encoding",
                       "uptime": "01:16:43", "started_at": 1792703528}}})
_t("encoder-write", "/api/v1/encoders/0",
   '{"status":"ok","message":"Set successfully","settings":{"physical_input":"hdmi0"}}',
   {"encoders": {"0": {"physical_input": "hdmi0"}}},
   http={"request": {"settings": {"physical_input": "hdmi0"}}},
   expect_then_send=[_g("/api/v1/encoders/0")])
_t("encoder-restart", "/api/v1/encoders/0/restart", '{"message":"Restarting","status":"ok"}', {},
   expect_then_send=[_g("/api/v1/encoders/0/info")])
_t("encoder-streams", "/api/v1/encoders/0/streams", json.dumps({
    "message": "Streams retrieved", "streams": [{"id": 6, "name": "Stream 7", "mode": "TS", "state": "Running"},
                                                {"id": 0, "name": "Stream 1", "mode": "SRT", "state": "Running"}],
    "status": "ok"}),
   {"streams": {"6": {"name": "Stream 7", "mode": "TS", "state": "Running"},
                "0": {"name": "Stream 1", "mode": "SRT", "state": "Running"}}})
_t("encoder-info", "/api/v1/encoders/0/info", json.dumps({
    "message": "Statuses retrieved", "settings": {
        "Format": "1920x1080p60", "Status": "OK", "Generator": "Off", "enc-output-resolution": "1920x1080p60",
        "Bitrate": 5710862, "Target Bitrate": 5700000, "State": "Encoding", "Uptime": "01:16:43",
        "StartedAt": 1792703528, "AudioSource": "embedded"}, "status": "ok"}),
   {"encoders": {"0": {"info": {"format": "1920x1080p60", "status": "OK", "generator": "Off",
                                "output_resolution": "1920x1080p60", "bitrate": 5710862, "target_bitrate": 5700000},
                       "state": "Encoding", "uptime": "01:16:43", "started_at": 1792703528,
                       "audio_source": "embedded"}}})
_VIDEO = {"codec": "h264", "profile_h264": "AVC_HIGH", "profile_hevc": "HEVC_MAIN", "bitrate_list": 25000,
          "control-rate": "CBR", "gop_length": 30, "kfi_seconds": 2, "num_slices": 8, "level": 5.1,
          "tier": "MAIN_TIER", "entropy-mode": "MODE_CABAC", "interlacer": "progressive", "resolution": "auto",
          "resolution_common": 1080, "width": 1920, "height": 1080, "framerate_setting": "auto",
          "framerate_common": 30, "framerate_exact": 30, "bit_depth": 8, "chroma_subsampling": 420,
          "color_override": False, "colorimetry": "default", "eotf": "Default", "range": "Default"}
_t("encoder-video", "/api/v1/encoders/0/video/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _VIDEO, "status": "ok"}),
   {"encoders": {"0": {"video": {
       "codec": "h264", "profile_h264": "AVC_HIGH", "profile_hevc": "HEVC_MAIN", "bitrate_kbps": 25000,
       "rate_control": "CBR", "gop_length": 30, "keyframe_interval_s": 2.0, "slices": 8, "level": 5.1,
       "tier": "MAIN_TIER", "entropy_mode": "MODE_CABAC", "interlacer": "progressive", "resolution": "auto",
       "resolution_common": 1080, "width": 1920, "height": 1080, "framerate_setting": "auto",
       "framerate_common": 30.0, "framerate_exact": 30.0, "bit_depth": 8, "chroma_subsampling": 420,
       "color_override": False, "colorimetry": "default", "eotf": "Default", "range": "Default",
       "settings_json": json.dumps(_VIDEO, separators=(",", ":"))}}}})
# The write's echo holds the keys sent only: those are kept, the whole
# object is not replaced, and the settings are read back.
_t("encoder-video-write", "/api/v1/encoders/0/video/settings",
   '{"status":"ok","message":"Set successfully","settings":{"bitrate_list":6000}}',
   {"encoders": {"0": {"video": {"bitrate_kbps": 6000}}}},
   http={"request": {"settings": {"bitrate_list": 6000}}},
   expect_then_send=[_g("/api/v1/encoders/0/video/settings")])
_t("encoder-audio-tracks", "/api/v1/encoders/2/audio", json.dumps({
    "message": "Audio tracks retrieved", "tracks": [{"id": 2, "codec": "aac", "channel_mode": "5.1",
                                                     "bitrate_list": 128000, "samplerate_list": 48000,
                                                     "mute_track": False, "enable": True}], "status": "ok"}),
   {"encoders": {"2": {"tracks": {"2": {"codec": "aac", "channel_mode": "5.1", "bitrate_bps": 128000,
                                        "samplerate_hz": 48000, "mute_track": False, "enable": True}}}}},
   state_before={"encoders": {"2": {"tracks": {"5": {"codec": "aac"}}}}},
   expect_then_send=[_g("/api/v1/encoders/2/audio/2/settings")])
_AUDIO = {"codec": "aac", "bitrate_list": 128000, "channel_mode": "5.1", "channel_mode2": "Stereo", "samplerate": "auto",
          "samplerate_list": 48000, "default-channel-map": "remap", "delay": 0, "afterburner": True,
          "mute_track": False, "enable": True}
_AUDIO_STATE = {"codec": "aac", "bitrate_bps": 128000, "channel_mode": "5.1", "channel_mode2": "Stereo",
                "samplerate": "auto", "samplerate_hz": 48000, "channel_map": "remap", "delay": 0, "afterburner": True,
                "mute_track": False, "enable": True, "settings_json": json.dumps(_AUDIO, separators=(",", ":"))}
_t("encoder-audio", "/api/v1/encoders/0/audio/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _AUDIO, "status": "ok"}),
   {"encoders": {"0": {"audio": _AUDIO_STATE}}})
_t("encoder-audio-write", "/api/v1/encoders/0/audio/settings",
   '{"status":"ok","message":"Set successfully","settings":{"enable":false}}',
   {"encoders": {"0": {"audio": {"enable": False}}}},
   http={"request": {"settings": {"enable": False}}},
   expect_then_send=[_g("/api/v1/encoders/0/audio/settings"), _g("/api/v1/encoders/0/audio")])
_t("encoder-audio-track", "/api/v1/encoders/2/audio/2/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _AUDIO, "status": "ok"}),
   {"encoders": {"2": {"tracks": {"2": _AUDIO_STATE}}}})
_t("encoder-audio-track-write", "/api/v1/encoders/2/audio/2/settings",
   '{"status":"ok","message":"Set successfully","settings":{"mute_track":true}}',
   {"encoders": {"2": {"tracks": {"2": {"mute_track": True}}}}},
   http={"request": {"settings": {"mute_track": True}}},
   expect_then_send=[_g("/api/v1/encoders/2/audio/2/settings")])
_t("encoder-metadata", "/api/v1/encoders/0/metadata/settings", json.dumps({
    "message": "Settings retrieved successfully", "settings": {
        "break_duration": 30000, "program_id": 0, "avail_expect": 0, "timecode": "sdi-hdmi-timecode", "pre_roll": 8000,
        "network_event": "out-of-network", "avail_num": 0, "dpi": True, "closed-captions": False, "event_id": "0",
        "splice_type": "normal"}, "status": "ok"}),
   {"encoders": {"0": {"metadata": {
       "break_duration_ms": 30000, "program_id": 0, "avail_expect": 0, "timecode": "sdi-hdmi-timecode",
       "pre_roll_ms": 8000, "network_event": "out-of-network", "avail_num": 0, "dpi": True,
       "closed_captions": False, "event_id": "0", "splice_type": "normal"}}}})
_t("encoder-metadata-write", "/api/v1/encoders/0/metadata/settings",
   '{"status":"ok","message":"Set successfully","settings":{"closed-captions":true}}',
   {"encoders": {"0": {"metadata": {"closed_captions": True}}}},
   http={"request": {"settings": {"closed-captions": True}}},
   expect_then_send=[_g("/api/v1/encoders/0/metadata/settings")])
_t("scte-with-fields", "/api/v1/encoders/0/metadata/scte", '{"message":"SCTE message sent","status":"ok"}', {},
   http={"request": {"settings": {"break_duration": 60000}}},
   expect_then_send=[_g("/api/v1/encoders/0/metadata/settings")])
_RECORD = {"mode": "RECUpload", "audio_track_selection": "all", "audio_tracks": [], "name": "$PN_$DATE_$TIME_$CAM",
           "format": "mp4", "max-files": 144, "max-size-time": 10, "bonded_uploads": True, "cyclic": False,
           "file-loop-index": 3, "camera_type": "canon", "auto_record": "off", "drive_id": "nfs2",
           "fragmented": False, "timecode": True, "sidecar": False}
_RECORD_STATE = {"mode": "RECUpload", "audio_track_selection": "all", "audio_tracks": "[]",
                 "name": "$PN_$DATE_$TIME_$CAM", "format": "mp4", "max_files": 144, "max_size_time": 10,
                 "bonded_uploads": True, "cyclic": False, "file_loop_index": 3, "camera_type": "canon",
                 "auto_record": "off", "drive_id": "nfs2", "fragmented": False, "timecode": True, "sidecar": False,
                 "settings_json": json.dumps(_RECORD, separators=(",", ":"))}
_t("encoder-record", "/api/v1/encoders/0/record/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _RECORD, "status": "ok"}),
   {"encoders": {"0": {"record": _RECORD_STATE}}})
_t("encoder-record-write", "/api/v1/encoders/0/record/settings",
   '{"status":"ok","message":"Set successfully","settings":{"cyclic":true}}',
   {"encoders": {"0": {"record": {"cyclic": True}}}},
   http={"request": {"settings": {"cyclic": True}}},
   expect_then_send=[_g("/api/v1/encoders/0/record/settings")])
_RINFO = {"State": "Ready", "Uptime": "00:00:00", "Format": "NFS4", "Size": "1.8T", "Used": "1.1T",
          "Available": "615.5G", "Percent Used": "65%"}
_t("encoder-record-info", "/api/v1/encoders/0/record/info",
   json.dumps({"message": "Statuses retrieved", "settings": _RINFO, "status": "ok"}),
   {"encoders": {"0": {"record_info": {"state": "Ready", "uptime": "00:00:00", "format": "NFS4", "size": "1.8T",
                                       "used": "1.1T", "available": "615.5G", "percent_used": "65%",
                                       "info_json": json.dumps(_RINFO, separators=(",", ":"))}}}})
_t("record-start", "/api/v1/encoders/0/record/start", '{"message":"Started","status":"ok"}', {},
   expect_then_send=[_g("/api/v1/encoders/0/record/info")])
_t("record-start-with-settings", "/api/v1/encoders/0/record/start", '{"message":"Started","status":"ok"}', {},
   http={"request": {"settings": {"format": "mp4"}}},
   expect_then_send=[_g("/api/v1/encoders/0/record/info"), _g("/api/v1/encoders/0/record/settings")])
_t("record-stop-decoder", "/api/v1/decoders/0/record/stop", '{"message":"Stopped","status":"ok"}', {},
   expect_then_send=[_g("/api/v1/decoders/0/record/info")])

# ── Decoders ──
_DECODERS = json.dumps({"message": "Pipelines retrieved", "decoders": [
    {"id": 0, "name": "Decoder 1", "state": "Inactive", "uptime": "00:00:00", "ingests": [1], "restreams": [5],
     "failover_timeout_ms": 1000}], "limit": 4, "minimum": 1, "status": "ok"})
_t("decoders", "/api/v1/decoders", _DECODERS,
   {"pools": {"decoders": {"0": {"name": "Decoder 1"}}},
    "decoders": {"0": {"name": "Decoder 1", "state": "Inactive", "uptime": "00:00:00", "ingests": "[1]",
                       "restreams": "[5]", "failover_timeout_ms": 1000}},
    "limits": {"decoders": 4, "decoders_minimum": 1}},
   expect_then_send=[_g("/api/v1/decoders/0/info"), _g("/api/v1/decoders/0/record/info"),
                     _g("/api/v1/decoders/0/video/mode"), _g("/api/v1/decoders/0/video/settings"),
                     _g("/api/v1/decoders/0/metadata/settings"), _g("/api/v1/decoders/0/record/settings"),
                     _g("/api/v1/streams/5"), _g("/api/v1/streams/5/settings"), _g("/api/v1/streams/5/info"),
                     _g("/api/v1/decoders/0/audio/settings")]
                    + [_g(f"/api/v1/decoders/0/audio/{n}") for n in range(16)])
_t("decoder-removed", "/api/v1/decoders/1", '{"message":"Decoder 1 removed","status":"ok"}', {"decoders": {}},
   state_before={"decoders": {"1": {"name": "Decoder 2"}}},
   expect_then_send=[_g("/api/v1/decoders")])
_t("decoder-added", "/api/v1/decoders", '{"message":"Decoder added","id":1,"status":"ok"}', {},
   expect_then_send=[_g("/api/v1/decoders")])
_t("decoder", "/api/v1/decoders/0", json.dumps({
    "message": "Settings retrieved successfully",
    "settings": {"id": 0, "name": "Decoder 1", "failover_timeout_ms": 1000, "ingests": [1], "restreams": [5]},
    "info": {"State": "Inactive", "Uptime": "00:00:00", "StartedAt": 0}, "status": "ok"}),
   {"decoders": {"0": {"name": "Decoder 1", "failover_timeout_ms": 1000, "ingests": "[1]", "restreams": "[5]",
                       "state": "Inactive", "uptime": "00:00:00", "started_at": 0}}})
_t("decoder-write", "/api/v1/decoders/0", '{"status":"ok","message":"Set successfully","settings":{"name":"Return Feed"}}',
   {"decoders": {"0": {"name": "Return Feed"}}},
   http={"request": {"settings": {"name": "Return Feed"}}},
   expect_then_send=[_g("/api/v1/decoders/0")])
_t("decoder-info", "/api/v1/decoders/0/info",
   '{"message":"Statuses retrieved","settings":{"Format":"1920x1080p59","Status":"OK"},"status":"ok"}',
   {"decoders": {"0": {"info": {"format": "1920x1080p59", "status": "OK"}}}})
_t("decoder-ingest", "/api/v1/decoders/0/video/mode", json.dumps({
    "message": "Settings retrieved successfully", "settings": {
        "mode": "TS", "name": "Ingest 2", "jitter-buffer-target-delay": 300, "jitter-buffer-max-delay": 500,
        "restreamed": False}, "status": "ok"}),
   {"decoders": {"0": {"ingest": {"mode": "TS", "name": "Ingest 2", "jitter_target_ms": 300, "jitter_max_ms": 500,
                                  "restreamed": False}}}})
# A new ingest mode: the mode and that mode's settings read back.
_t("decoder-ingest-write", "/api/v1/decoders/0/video/mode",
   '{"status":"ok","message":"Set successfully","settings":{"mode":"SRT"}}',
   {"decoders": {"0": {"ingest": {"mode": "SRT"}}}},
   http={"request": {"settings": {"mode": "SRT"}}},
   expect_then_send=[_g("/api/v1/decoders/0/video/mode"), _g("/api/v1/decoders/0/video/settings")])
_DVIDEO = {"protocol": "udp", "udp_port": 9720, "tcp_port": 9710, "server_host": "172.24.103.144",
           "server_port": 5000, "multicast_host": "", "multicast_port": 9730, "interface_name": "any"}
_t("decoder-video", "/api/v1/decoders/0/video/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _DVIDEO, "status": "ok"}),
   {"decoders": {"0": {"video": {"settings_json": json.dumps(_DVIDEO, separators=(",", ":"))}}}})
_t("audio-output", "/api/v1/decoders/0/audio/settings", json.dumps({
    "message": "Settings retrieved successfully", "settings": {
        "enable": True, "channel_mode": "8", "default-channel-map": "smpte", "delay": 0,
        "embedded-output-mute": False, "embedded-output-volume": 0, "remapping": False, "selected": 0},
    "status": "ok"}),
   {"audio_output": {"enable": True, "channel_mode": "8", "channel_map": "smpte", "delay": 0, "embedded_mute": False,
                     "embedded_volume": 0, "remapping": False, "selected": 0,
                     "settings_json": '{"enable":true,"channel_mode":"8","default-channel-map":"smpte","delay":0,'
                                      '"embedded-output-mute":false,"embedded-output-volume":0,"remapping":false,'
                                      '"selected":0}'}})
_t("audio-output-channel", "/api/v1/decoders/0/audio/4",
   '{"message":"Settings retrieved successfully","settings":{"channel":4,"mute":false,"track":0,"volume":0},"status":"ok"}',
   {"audio_output": {"channels": {"4": {"mute": False, "track": 0, "volume": 0}}}})
_t("audio-output-channel-write", "/api/v1/decoders/0/audio/4",
   '{"status":"ok","message":"Set successfully","settings":{"mute":true}}',
   {"audio_output": {"channels": {"4": {"mute": True}}}},
   http={"request": {"settings": {"mute": True}}},
   expect_then_send=[_g("/api/v1/decoders/0/audio/4")])
_t("decoder-metadata", "/api/v1/decoders/0/metadata/settings", json.dumps({
    "message": "Settings retrieved successfully", "settings": {
        "insert_cc": "off", "cc_insertion_line": 9, "insert_dpi": False, "dpi_insertion_line": 10,
        "insert_timecode": "0"}, "status": "ok"}),
   {"decoders": {"0": {"metadata": {"insert_cc": "off", "cc_insertion_line": 9, "insert_dpi": False,
                                    "dpi_insertion_line": 10, "insert_timecode": "0"}}}})
_DRECORD = {"mode": "RECUpload", "name": "$PN_$DATE_$TIME", "format": "mp4", "max-files": 144, "max-size-time": 10,
            "drive_id": "sd1", "auto_record": "off", "cyclic": False, "timecode": False}
_t("decoder-record", "/api/v1/decoders/0/record/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _DRECORD, "status": "ok"}),
   {"decoders": {"0": {"record": {"mode": "RECUpload", "name": "$PN_$DATE_$TIME", "format": "mp4", "max_files": 144,
                                  "max_size_time": 10, "drive_id": "sd1", "auto_record": "off", "cyclic": False,
                                  "timecode": False,
                                  "settings_json": json.dumps(_DRECORD, separators=(",", ":"))}}}})
_t("decoder-record-info", "/api/v1/decoders/0/record/info",
   '{"message":"Statuses retrieved","settings":{"Size":"13.8G","State":"Disabled"},"status":"ok"}',
   {"decoders": {"0": {"record_info": {"size": "13.8G", "state": "Disabled",
                                       "info_json": '{"Size":"13.8G","State":"Disabled"}'}}}})

# ── Streams ──
_STREAMS = json.dumps({"message": "Streams retrieved", "streams": [
    {"id": 0, "name": "Stream 1", "mode": "SRT", "state": "Running", "owner": "encoders/0"},
    {"id": 5, "name": "Stream 6", "mode": "Disabled", "state": None, "owner": "decoders/0"},
    {"id": 9, "name": "Stream 10", "mode": "RIST", "state": "Error", "owner": None}],
    "limit": 10, "minimum": 1, "protected": [0], "status": "ok"})
_t("streams", "/api/v1/streams", _STREAMS,
   {"pools": {"streams": {"0": {"name": "Stream 1"}, "5": {"name": "Stream 6"}, "9": {"name": "Stream 10"}}},
    "streams": {"0": {"name": "Stream 1", "mode": "SRT", "state": "Running", "owner": "encoders/0"},
                "5": {"name": "Stream 6", "mode": "Disabled", "owner": "decoders/0"},
                "9": {"name": "Stream 10", "mode": "RIST", "state": "Error"}},
    "limits": {"streams": 10, "streams_minimum": 1, "streams_protected": "[0]"}},
   state_before={"pools": {"streams": {"8": {"name": "Stream 9"}}}},
   expect_then_send=[_g("/api/v1/encoders/0/info"), _g("/api/v1/encoders/0/record/info")])
_t("stream-removed", "/api/v1/streams/8", '{"message":"Stream 8 removed","status":"ok"}', {"streams": {}},
   state_before={"streams": {"8": {"name": "Stream 9", "mode": "TS"}}},
   expect_then_send=[_g("/api/v1/streams")])
_t("stream-added", "/api/v1/streams", '{"message":"Stream added","id":10,"status":"ok"}', {},
   expect_then_send=[_g("/api/v1/streams")])
_t("stream", "/api/v1/streams/0", json.dumps({
    "message": "Settings retrieved successfully", "settings": {
        "name": "Stream 1", "mode": "SRT", "audio_track_selection": "all", "audio_tracks": [], "owner": "encoders/0",
        "mode-h264": "SRT", "mode-hevc": "SRT"}, "status": "ok"}),
   {"streams": {"0": {"name": "Stream 1", "mode": "SRT", "audio_track_selection": "all", "audio_tracks": "[]",
                      "owner": "encoders/0", "mode_h264": "SRT", "mode_hevc": "SRT"}}})
# Firmware 2.25: per-codec modes and no mode (API Reference, Firmware 2.25).
_t("stream-2-25", "/api/v1/streams/1", json.dumps({
    "status": "ok", "endpoints": ["complete", "info"], "timestamp": 1771463360,
    "settings": {"mode-h264": "TS", "name": "", "mode-hevc": "Disabled"}, "message": "Settings retrieved successfully"}),
   {"streams": {"1": {"name": "", "mode_h264": "TS", "mode_hevc": "Disabled"}}})
_t("stream-write", "/api/v1/streams/0", '{"status":"ok","message":"Set successfully","settings":{"mode":"RTMP"}}',
   {"streams": {"0": {"mode": "RTMP"}}},
   http={"request": {"settings": {"mode": "RTMP"}}},
   expect_then_send=[_g("/api/v1/streams/0"), _g("/api/v1/streams/0/settings"), _g("/api/v1/streams")])
_SSRT = {"connection_type": "listener", "listener_port": 9710, "caller_host": "", "caller_port": 9710, "latency": 500,
         "encryption": False, "passphrase": "********", "key_length": 16, "stream_id": "stream", "interface_name": "any"}
_t("stream-settings", "/api/v1/streams/0/settings",
   json.dumps({"message": "Settings retrieved successfully", "settings": _SSRT, "status": "ok"}),
   {"streams": {"0": {"settings_json": json.dumps(_SSRT, separators=(",", ":"))}}})
_t("stream-settings-write", "/api/v1/streams/0/settings",
   '{"status":"ok","message":"Set successfully","settings":{"latency":800}}', {},
   http={"request": {"settings": {"latency": 800}}},
   expect_then_send=[_g("/api/v1/streams/0/settings")])
_SINFO = {"State": "Running", "State Details": "", "Uptime": "01:16:41", "Stream": "srt://@:9710?mode=listener",
          "Port": 9710, "Latency": "500 ms", "Encryption": "Disabled",
          "thisSideStats": {"Run-time": "0 01:16:41", "Total Encoder Bytes": 3360892272, "Total Network Bytes": 0,
                            "Peers": []}}
_t("stream-info", "/api/v1/streams/0/info",
   json.dumps({"message": "Statuses retrieved", "settings": _SINFO, "status": "ok"}),
   {"streams": {"0": {"state": "Running", "info": {
       "state_details": "", "uptime": "01:16:41", "url": "srt://@:9710?mode=listener", "port": 9710,
       "latency": "500 ms", "encryption": "Disabled", "info_json": json.dumps(_SINFO, separators=(",", ":"))}}}})
_t("stream-action", "/api/v1/streams/3/publish", '{"message":"Published","status":"ok"}', {},
   expect_then_send=[_g("/api/v1/streams/3/info"), _g("/api/v1/streams")])
_t("stream-start-all", "/api/v1/streams/0/start_all", '{"status":"ok"}', {},
   expect_then_send=[_g("/api/v1/streams")])

# ── Network ──
_WIRED = {"Custom Labels": {"intf_name": "Wired 1"}, "Detail": "Unable to detect ethernet cable",
          "MAC Address": "FC:C2:3D:2F:D1:47", "Status": "No Cable"}
_t("wired", "/api/v1/network/wired/0",
   json.dumps({"settings": _WIRED, "timestamp": 1771463363, "message": "Statuses received", "status": "ok"}),
   {"network": {"wired": {"0": {"label": "Wired 1", "status": "No Cable", "detail": "Unable to detect ethernet cable",
                                "mac_address": "FC:C2:3D:2F:D1:47",
                                "status_json": json.dumps(_WIRED, separators=(",", ":"))}}}})
_WIFI = {"Custom Labels": {"intf_name": "Wi-Fi"}, "Status": "Established Access Point", "Network": "Prism-855-00073",
         "Subnet Mask": "255.255.255.0", "MAC Address": "64:BC:58:9E:E8:BE", "IP Address": "172.16.1.1"}
_t("wifi", "/api/v1/network/wifi",
   json.dumps({"settings": _WIFI, "timestamp": 1771463364, "message": "Statuses received", "status": "ok"}),
   {"network": {"wifi": {"label": "Wi-Fi", "status": "Established Access Point", "network": "Prism-855-00073",
                         "subnet_mask": "255.255.255.0", "mac_address": "64:BC:58:9E:E8:BE", "ip_address": "172.16.1.1",
                         "settings_json": json.dumps(_WIFI, separators=(",", ":"))}}})
_t("wifi-write", "/api/v1/network/wifi", '{"status":"ok","message":"Set successfully","settings":{"Network":"MyNetwork"}}',
   {"network": {"wifi": {"network": "MyNetwork"}}},
   http={"request": {"settings": {"Network": "MyNetwork"}}},
   expect_then_send=[_g("/api/v1/network/wifi")])
_MODEM = {"Modem Type": "multislot", "IMSI": "31188204002110", "RSSI": -200, "Modem Report": "EMM attach failed",
          "IMEI": "863547050070800", "Version": "EM120RGLAUR02A07M4G_01.202.01.202",
          "Custom Labels": {"slot": {"2": "Slot 2", "1": "Slot 1"}, "intf_name": "Modem 1"}, "Status": "No Carrier",
          "Model": "Node II-CBRS", "ICCID": "8901882047100211064F", "Detail": "Unable to detect carrier"}
_t("modem", "/api/v1/network/modem/0",
   json.dumps({"settings": _MODEM, "timestamp": 1771463365, "message": "Statuses received", "status": "ok"}),
   {"network": {"modems": {"0": {"type": "multislot", "model": "Node II-CBRS",
                                 "version": "EM120RGLAUR02A07M4G_01.202.01.202", "status": "No Carrier",
                                 "detail": "Unable to detect carrier", "report": "EMM attach failed", "rssi": -200,
                                 "imei": "863547050070800", "imsi": "31188204002110", "iccid": "8901882047100211064F",
                                 "label": "Modem 1", "status_json": json.dumps(_MODEM, separators=(",", ":"))}}}})
_t("settings-import", "/api/v1/settings/sync/import", '{"status":"ok","message":"Imported"}', {},
   expect_then_send=[_g("/api/v1/encoders"), _g("/api/v1/decoders"), _g("/api/v1/streams")])
# Downloads saved to the host file named (never written by a vector).
V.append({"spec": "teradek-prism", "command": "save_encoder_snapshot", "input": {"encoder": 0, "to": {"file": "enc.jpg"}}, "expect_request": {"method": "GET", "target": "/api/v1/encoders/0/snapshot"}})
_post("save_decoder_snapshot", {"decoder": 0, "width": 1280, "height": 720, "quality": 90, "to": {"file": "dec.jpg"}},
      "/api/v1/decoders/0/snapshot", '{"settings":{"width":1280,"height":720,"quality":90}}')
