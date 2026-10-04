# Castr (castr): one vector per command, and telemetry vectors for the polled
# resources. Targets are the endpoints as Castr's API reference gives them
# (https://api.castr.com/v2/...). Reply bodies follow the guides' examples;
# ids are made up.
CS = "castr"
_CSS = {"username": "tok_5b2e1c65", "password": "s3cr3t", "stream_id": "6226ce5f31ddbeb3754aa243"}
_CSI = "6226ce5f31ddbeb3754aa243"
_CSP = "6226ce8431ddbeb3754aa249"
_CSR = "65354c2524f23b7c0f855627"
_CSL = "/v2/live_streams/" + _CSI


def _cs(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": CS, "command": command, "input": input, "settings": _CSS,
              "expect_request": request, **extra})


_CSINGEST = {"server": "rtmp://live.castr.com/static", "key": "lv_6ccbd6609e9011ecad519393af92c1a1?password=b47020b2"}
_CSSTREAM = {
    "_id": _CSI, "name": "Sunday Service", "enabled": True, "ingest": _CSINGEST,
    "playback": {"embed_url": "https://player.castr.com/lv_6ccbd6609e9011ecad519393af92c1a1",
                 "hls_url": "https://stream.castr.com/620e0ba1ce8f11ddde571d1c/lv_6ccbd6609e9011ecad519393af92c1a1/index.m3u8"},
    "platforms": [{"_id": _CSP, "name": "YouTube", "template": "custom", "enabled": True,
                   "server": "rtmp://a.rtmp.youtube.com/live2", "key": "debs-4ux2-r565-1sv4-68zh",
                   "broadcasting_status": "online"}],
    "settings": {"abr": False, "cloud_recording": True, "low_latency_playback": False, "chat_enabled": True},
    "broadcasting_status": "online", "user": "620e0ba1ce8f11ddde571d1c",
    "creation_time": "2022-03-08T03:32:47.990Z"}
_S = {"stream_id": _CSI}

# Live streams.
_cs("list_streams", {}, "GET", "/v2/live_streams?page=1&limit=20")
_cs("get_stream", _S, "GET", _CSL)
_cs("get_ingest", _S, "GET", _CSL,
    http_reply={"status": 200, "body": json.dumps(_CSSTREAM)},
    expect_result={"ok": {"kind": "value", "value": _CSINGEST}})
_cs("get_playback", _S, "GET", _CSL)
_cs("get_pull_urls", _S, "GET", _CSL)
_cs("create_stream", {"name": "Sunday Service", "cloud_recording": True}, "POST", "/v2/live_streams",
    body='{"name":"Sunday Service","enabled":true,"settings":{"abr":false,"cloud_recording":true}}',
    http_reply={"status": 200, "body": json.dumps(_CSSTREAM)},
    expect_result={"ok": {"kind": "value", "value": _CSI}})
_NEW = {"name": "July event", "enabled": False, "settings": {"abr": False, "embed_password": "abcd1234"}}
_cs("create_stream_custom", {"stream": _NEW}, "POST", "/v2/live_streams", body=json.dumps(_NEW, separators=(",", ":")))
_CHG = {"name": "Stream updated name", "settings": {"abr": False, "cloud_recording": True}}
_cs("update_stream", {"stream_id": _CSI, "changes": _CHG}, "PATCH", _CSL, body=json.dumps(_CHG, separators=(",", ":")))
_cs("rename_stream", {"stream_id": _CSI, "name": "Sunday \"Service\""}, "PATCH", _CSL,
    body='{"name":"Sunday \\"Service\\""}')
_cs("set_stream_enabled", {"stream_id": _CSI, "enabled": False}, "PATCH", _CSL, body='{"enabled":false}',
    http_reply={"status": 200, "body": json.dumps(dict(_CSSTREAM, enabled=False))},
    expect_result={"ok": {"kind": "ack"}})
_cs("set_cloud_recording", {"stream_id": _CSI, "enabled": True}, "PATCH", _CSL,
    body='{"settings":{"cloud_recording":true}}')
_cs("set_abr", {"stream_id": _CSI, "enabled": False}, "PATCH", _CSL, body='{"settings":{"abr":false}}')
_cs("set_low_latency_playback", {"stream_id": _CSI, "enabled": True}, "PATCH", _CSL,
    body='{"settings":{"low_latency_playback":true}}')
_cs("set_chat_enabled", {"stream_id": _CSI, "enabled": False}, "PATCH", _CSL, body='{"settings":{"chat_enabled":false}}')
_cs("delete_stream", _S, "DELETE", _CSL,
    http_reply={"status": 200, "body": json.dumps({"success": True})}, expect_result={"ok": {"kind": "ack"}})

# Multistream platforms.
_cs("add_platform", {"stream_id": _CSI, "name": "YouTube", "server": "rtmp://a.rtmp.youtube.com/live2",
                     "key": "debs-4ux2-r565-1sv4-68zh"},
    "POST", _CSL + "/platforms",
    body='{"template":"custom","name":"YouTube","server":"rtmp://a.rtmp.youtube.com/live2","key":"debs-4ux2-r565-1sv4-68zh","enabled":true}')
_PLAT = {"template": "custom", "name": "Facebook Event", "server": "live.fb.com", "key": "UUID_KEY", "enabled": False}
_cs("add_platform_custom", {"stream_id": _CSI, "platform": _PLAT}, "POST", _CSL + "/platforms",
    body=json.dumps(_PLAT, separators=(",", ":")))
_cs("set_platform_enabled", {"stream_id": _CSI, "platform_id": _CSP, "enabled": False}, "PATCH",
    _CSL + "/platforms/" + _CSP, body='{"enabled":false}')
_cs("set_platform_meta", {"stream_id": _CSI, "platform_id": _CSP, "title": "Sunday Service",
                          "description": "Live from the main hall"},
    "PATCH", _CSL + "/platforms/" + _CSP,
    body='{"metadata":{"title":"Sunday Service","description":"Live from the main hall"}}')
_cs("update_platform", {"stream_id": _CSI, "platform_id": _CSP, "changes": {"name": "YouTube main"}}, "PATCH",
    _CSL + "/platforms/" + _CSP, body='{"name":"YouTube main"}')
_cs("delete_platform", {"stream_id": _CSI, "platform_id": _CSP}, "DELETE", _CSL + "/platforms/" + _CSP)

# Stats and recordings.
# Offline: Castr answers 404, an ordinary failure.
_cs("get_stream_stats", _S, "GET", _CSL + "/stats",
    http_reply={"status": 404, "body": json.dumps({"statusCode": 404, "error": "Not Found",
                                                   "message": "Stream not found in upstreams."})},
    expect_result={"error": {"error": "device_error", "code": "404"}})
_cs("list_temporary_recordings", _S, "GET", _CSL + "/temporary_recordings")
_cs("convert_recording_to_vod", {"stream_id": _CSI, "recording_id": _CSR, "from": "2023-10-20T20:13:52.000Z",
                                 "duration": 3600},
    "POST", _CSL + "/temporary_recordings/" + _CSR, body='{"from":"2023-10-20T20:13:52.000Z","duration":3600}')
_cs("list_vod_recordings", {"stream_id": _CSI, "page": 2}, "GET", _CSL + "/recordings?page=2&limit=20")
# A refused key: terminal, reported as auth.
_cs("get_epg", _S, "GET", _CSL + "/epg",
    http_reply={"status": 401, "body": json.dumps({"statusCode": 401, "error": "Unauthorized"})},
    expect_result={"error": {"error": "auth"}})

# Telemetry.
_CSSTATE = {"streams": {_CSI: {
    "name": "Sunday Service", "enabled": True, "broadcasting_status": "online", "abr": False,
    "cloud_recording": True, "low_latency_playback": False, "chat_enabled": True,
    "embed_url": "https://player.castr.com/lv_6ccbd6609e9011ecad519393af92c1a1",
    "hls_url": "https://stream.castr.com/620e0ba1ce8f11ddde571d1c/lv_6ccbd6609e9011ecad519393af92c1a1/index.m3u8",
    "platforms": {_CSP: {"name": "YouTube", "template": "custom", "enabled": True,
                         "broadcasting_status": "online"}}}}}
telemetry(CS, "stream", settings=_CSS, inbound_http={"path": _CSL, "body": json.dumps(_CSSTREAM)},
          # A target deleted elsewhere leaves: the stream lists every platform.
          state_before={"streams": {_CSI: {"platforms": {"a" * 24: {"name": "Deleted", "enabled": False}}}}},
          expect_state=_CSSTATE)
# Add Platform answers the whole stream.
telemetry(CS, "platform-added", settings=_CSS, inbound_http={"path": _CSL + "/platforms", "body": json.dumps(_CSSTREAM)},
          expect_state=_CSSTATE)
telemetry(CS, "stats", settings=_CSS, inbound_http={
    "path": _CSL + "/stats",
    "body": json.dumps({
        "status": "running", "bitrate": 1333, "last_dts": 1749711852486, "push_stats": {}, "alive": True,
        "srt_port_resolve": False, "retry_count": 0, "opened_at": 1749711734724,
        "media_info": {"flow_type": "stream", "stream_id": 0, "tracks": [
            {"profile": "High", "level": "4", "width": 1920, "codec": "h264", "track_id": "v1", "content": "video",
             "bitrate": 1202, "height": 1080, "fps": 30, "avg_fps": 30},
            {"codec": "aac", "track_id": "a1", "content": "audio", "bitrate": 131, "channels": 2,
             "sample_rate": 44100}]},
        "pull_urls": {"rtmp": "rtmp://sg-1.castr.com/static/live_xx",
                      "srt": "srt://sg-1.castr.com:9998/?streamid=#!::r=xx,m=request"}})},
    # The track a2 is gone from the ingest: each answer replaces the tracks.
    state_before={"streams": {_CSI: {"tracks": {"a2": {"content": "audio", "codec": "opus"}}}}},
    expect_state={"streams": {_CSI: {
        "stats_status": "running", "stats_alive": True, "stats_bitrate": 1333,
        "stats_opened_at": 1749711734724, "stats_retry_count": 0,
        "tracks": {"v1": {"content": "video", "codec": "h264", "bitrate": 1202, "width": 1920, "height": 1080,
                          "fps": 30.0},
                   "a1": {"content": "audio", "codec": "aac", "bitrate": 131, "sample_rate": 44100,
                          "channels": 2}}}}})
