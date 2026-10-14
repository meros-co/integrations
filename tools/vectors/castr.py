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

# Stream settings (Update a live stream).
_cs("set_varying_rewind", {"stream_id": _CSI, "seconds": "7200"}, "PATCH", _CSL,
    body='{"settings":{"varying_rewind":7200}}')
_cs("set_embed_password", {"stream_id": _CSI, "password": "pass1234"}, "PATCH", _CSL,
    body='{"settings":{"embed_password":"pass1234"}}')
_cs("set_country_block", {"stream_id": _CSI, "countries": ["US", "CA"]}, "PATCH", _CSL,
    body='{"settings":{"country_block":["US","CA"]}}')
_cs("set_country_whitelist", {"stream_id": _CSI, "countries": []}, "PATCH", _CSL,
    body='{"settings":{"country_whitelist":[]}}')
_cs("set_domain_whitelist", {"stream_id": _CSI, "domains": ["example.org"]}, "PATCH", _CSL,
    body='{"settings":{"domain_whitelist":["example.org"]}}')
telemetry(CS, "stream-settings", settings=_CSS, inbound_http={"path": _CSL, "body": json.dumps({
    **_CSSTREAM, "platforms": [],
    "playback": {**_CSSTREAM["playback"], "ll_embed_url": "https://player.castr.com/ll/lv_6ccbd6609e9011ecad519393af92c1a1"},
    "settings": {**_CSSTREAM["settings"], "varying_rewind": 7200, "country_block": ["US"], "country_whitelist": [],
                 "domain_whitelist": ["example.org"], "embed_password": "pass1234"}})},
    expect_state={"streams": {_CSI: {
        "name": "Sunday Service", "enabled": True, "broadcasting_status": "online", "abr": False,
        "cloud_recording": True, "low_latency_playback": False, "chat_enabled": True,
        "embed_url": _CSSTREAM["playback"]["embed_url"], "hls_url": _CSSTREAM["playback"]["hls_url"],
        "ll_embed_url": "https://player.castr.com/ll/lv_6ccbd6609e9011ecad519393af92c1a1",
        "varying_rewind": 7200, "country_block": '["US"]', "country_whitelist": "[]",
        "domain_whitelist": '["example.org"]'}}})

# Sub-second streams (reference, and the Create Sub Second Streams guide).
_SS = "69d60e39487fd33f934ce568"
_SSL = "/v2/sub_second_streams/" + _SS
_SSOBJ = {"_id": _SS, "name": "NewSubSecondStream", "enabled": True,
          "ingest": {"server": "rtmp://live.castr.io/static", "key": "ss_key_secret"},
          "playback": {"embed_url": "https://player.castr.com/ss/69d60e39"}, "platforms": [], "settings": {},
          "broadcasting_status": "offline", "user": "63be6c169f8c1d3cdbb4491d",
          "creation_time": "2026-04-08T08:13:45.749Z"}
_SSSTATE = {"name": "NewSubSecondStream", "enabled": True, "broadcasting_status": "offline",
            "embed_url": "https://player.castr.com/ss/69d60e39", "creation_time": "2026-04-08T08:13:45.749Z"}
_cs("list_sub_second_streams", {}, "GET", "/v2/sub_second_streams?page=1&limit=20")
_cs("get_sub_second_stream", {"stream_id": _SS}, "GET", _SSL)
_cs("create_sub_second_stream", {"name": "NewSubSecondStream"}, "POST", "/v2/sub_second_streams",
    body='{"name":"NewSubSecondStream","enabled":true}',
    http_reply={"status": 200, "body": json.dumps(_SSOBJ)}, expect_result={"ok": {"kind": "value", "value": _SSOBJ}})
_SSNEW = {"name": "Low latency", "enabled": True, "settings": {"country_block": ["US"]}}
_cs("create_sub_second_stream_custom", {"stream": _SSNEW}, "POST", "/v2/sub_second_streams",
    body=json.dumps(_SSNEW, separators=(",", ":")))
_cs("update_sub_second_stream", {"stream_id": _SS, "changes": {"enabled": False}}, "PATCH", _SSL,
    body='{"enabled":false}')
_cs("rename_sub_second_stream", {"stream_id": _SS, "name": "Stage cam"}, "PATCH", _SSL, body='{"name":"Stage cam"}')
_cs("set_sub_second_stream_enabled", {"stream_id": _SS, "enabled": False}, "PATCH", _SSL, body='{"enabled":false}')
_cs("delete_sub_second_stream", {"stream_id": _SS}, "DELETE", _SSL,
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})
telemetry(CS, "sub-second-streams", settings=_CSS, inbound_http={
    "path": "/v2/sub_second_streams?page=1&limit=100",
    "body": json.dumps([{**_SSOBJ, "settings": {"country_block": [], "country_whitelist": ["GB"], "domain_whitelist": []},
                         "broadcasting_status": "online"}])},
    expect_state={"sub_second_streams": {_SS: {**_SSSTATE, "broadcasting_status": "online", "country_block": "[]",
                                               "country_whitelist": '["GB"]', "domain_whitelist": "[]"}}})
telemetry(CS, "sub-second-stream-created", settings=_CSS, inbound_http={
    "path": "/v2/sub_second_streams", "body": json.dumps(_SSOBJ)},
    expect_state={"sub_second_streams": {_SS: _SSSTATE}})
telemetry(CS, "sub-second-stream-deleted", settings=_CSS, inbound_http={"path": _SSL, "body": '{"success":true}'},
          state_before={"sub_second_streams": {_SS: _SSSTATE, "a" * 24: {"name": "kept"}}},
          expect_state={"sub_second_streams": {"a" * 24: {"name": "kept"}}})

# Video hosting (reference, and the Uploading Video guide).
_FO = "6517f0a4b5ffafac50584572"
_FOL = "/v2/videos/" + _FO
_SF = "6517f0a4b5ffafac50584599"
_CT = "LfC3rMO4J1RYOiPe.mp4"
_FOLDER = {"_id": _FO, "name": "myfolder", "enabled": True, "type": "vod", "data": [],
           "user": "6517f056305a32ac3a0a8c9c", "creation_time": "2023-09-30T09:55:48.648Z"}
_UPLOAD = {"url": "https://castr-us-east-1.s3.us-east-1.amazonaws.com/",
           "fields": {"bucket": "castr-us-east-1", "key": "vod_uploads/vd883b52005f7711ee9be6/my-awesome-video.mp4"}}
_cs("list_folders", {}, "GET", "/v2/videos?page=1&limit=20")
_cs("create_folder", {"name": "myfolder"}, "POST", "/v2/videos", body='{"name":"myfolder"}',
    http_reply={"status": 200, "body": json.dumps(_FOLDER)}, expect_result={"ok": {"kind": "value", "value": _FOLDER}})
_FNEW = {"name": "Sermons", "settings": {"embed_password": "pw"}}
_cs("create_folder_custom", {"folder": _FNEW}, "POST", "/v2/videos", body=json.dumps(_FNEW, separators=(",", ":")))
_cs("update_folder", {"folder_id": _FO, "changes": {"settings": {"domain_whitelist": ["example.org"]}}}, "PATCH", _FOL,
    body='{"settings":{"domain_whitelist":["example.org"]}}')
_cs("rename_folder", {"folder_id": _FO, "name": "Sermons 2026"}, "PATCH", _FOL, body='{"name":"Sermons 2026"}')
_cs("delete_folder", {"folder_id": _FO}, "DELETE", _FOL)
_cs("list_folder_contents", {"folder_id": _FO, "include_subfolders": True}, "GET",
    _FOL + "/contents?page=1&limit=20&include_subfolders=true")
_cs("delete_folder_content", {"folder_id": _FO, "content_id": _CT}, "DELETE", _FOL + "/contents/" + _CT)
_cs("move_content_to_subfolder", {"folder_id": _FO, "content_id": _CT, "subfolder_id": _SF}, "PATCH",
    _FOL + "/contents/" + _CT, body='{"subfolder_id":"' + _SF + '"}')
_cs("move_content_to_root", {"folder_id": _FO, "content_id": _CT}, "PATCH", _FOL + "/contents/" + _CT,
    body='{"subfolder_id":null}')
_cs("create_upload_url", {"folder_id": _FO}, "POST", _FOL + "/uploads",
    http_reply={"status": 200, "body": json.dumps(_UPLOAD)}, expect_result={"ok": {"kind": "value", "value": _UPLOAD}})
_cs("create_upload_url_named", {"folder_id": _FO, "file_name": "my awesome video.mp4"}, "POST",
    _FOL + "/uploads?customFileName=my%20awesome%20video.mp4")
_cs("list_subfolders", {"folder_id": _FO}, "GET", _FOL + "/subfolders")
_cs("get_subfolder", {"folder_id": _FO, "subfolder_id": _SF}, "GET", _FOL + "/subfolders/" + _SF)
_cs("create_subfolder", {"folder_id": _FO, "name": "Easter"}, "POST", _FOL + "/subfolders", body='{"name":"Easter"}')
_cs("rename_subfolder", {"folder_id": _FO, "subfolder_id": _SF, "name": "Easter 2026"}, "PATCH",
    _FOL + "/subfolders/" + _SF, body='{"name":"Easter 2026"}')
_cs("delete_subfolder", {"folder_id": _FO, "subfolder_id": _SF}, "DELETE", _FOL + "/subfolders/" + _SF,
    http_reply={"status": 200, "body": '{"removed":true,"removedVideos":3}'},
    expect_result={"ok": {"kind": "value", "value": 3}})
_cs("list_subfolder_contents", {"folder_id": _FO, "subfolder_id": _SF}, "GET",
    _FOL + "/subfolders/" + _SF + "/contents?page=1&limit=20")
_cs("delete_subfolder_content", {"folder_id": _FO, "subfolder_id": _SF, "content_id": _CT}, "DELETE",
    _FOL + "/subfolders/" + _SF + "/contents/" + _CT)
_cs("create_subfolder_upload_url", {"folder_id": _FO, "subfolder_id": _SF}, "POST", _FOL + "/subfolders/" + _SF + "/uploads")
_cs("create_subfolder_upload_url_named", {"folder_id": _FO, "subfolder_id": _SF, "file_name": "easter.mp4"}, "POST",
    _FOL + "/subfolders/" + _SF + "/uploads?customFileName=easter.mp4")
_FOLDER_STATE = {"name": "myfolder", "enabled": True}
telemetry(CS, "folders", settings=_CSS, inbound_http={
    "path": "/v2/videos?page=1&limit=20",
    "body": json.dumps({"total": 1, "page": 1, "pages": 1, "docs": [{**_FOLDER, "settings": {"country_block": []}}]})},
    expect_state={"folders": {_FO: {**_FOLDER_STATE, "country_block": "[]"}}})
telemetry(CS, "folders-in-array", settings=_CSS, inbound_http={
    "path": "/v2/videos?page=1&limit=20", "body": json.dumps([{"total": 1, "page": 1, "pages": 1, "docs": [_FOLDER]}])},
    expect_state={"folders": {_FO: _FOLDER_STATE}})
telemetry(CS, "folder-created", settings=_CSS, inbound_http={"path": "/v2/videos", "body": json.dumps(_FOLDER)},
          expect_state={"folders": {_FO: _FOLDER_STATE}})
telemetry(CS, "folder-deleted", settings=_CSS, inbound_http={"path": _FOL, "body": '{"success":true}'},
          state_before={"folders": {_FO: _FOLDER_STATE, "b" * 24: {"name": "kept"}}},
          expect_state={"folders": {"b" * 24: {"name": "kept"}}})
_SUB = {"_id": _SF, "name": "Easter", "folder_id": _FO, "creation_time": "2026-04-01T10:00:00.000Z", "content_count": 3}
_SUB_STATE = {"name": "Easter", "content_count": 3, "creation_time": "2026-04-01T10:00:00.000Z"}
telemetry(CS, "subfolders", settings=_CSS, inbound_http={"path": _FOL + "/subfolders", "body": json.dumps([_SUB])},
          state_before={"folders": {_FO: {"name": "myfolder", "subfolders": {"c" * 24: {"name": "gone"}}}}},
          expect_state={"folders": {_FO: {"name": "myfolder", "subfolders": {_SF: _SUB_STATE}}}})
telemetry(CS, "subfolder-created", settings=_CSS, inbound_http={"path": _FOL + "/subfolders", "body": json.dumps(_SUB)},
          state_before={"folders": {_FO: {"subfolders": {"c" * 24: {"name": "kept"}}}}},
          expect_state={"folders": {_FO: {"subfolders": {"c" * 24: {"name": "kept"}, _SF: _SUB_STATE}}}})
telemetry(CS, "subfolder-renamed", settings=_CSS, inbound_http={"path": _FOL + "/subfolders/" + _SF,
                                                                  "body": '{"success":true}'},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _FOL + "/subfolders/" + _SF}])
telemetry(CS, "subfolder-deleted", settings=_CSS, inbound_http={"path": _FOL + "/subfolders/" + _SF,
                                                                  "body": '{"removed":true,"removedVideos":3}'},
          state_before={"folders": {_FO: {"name": "myfolder", "subfolders": {_SF: _SUB_STATE}}}},
          expect_state={"folders": {_FO: {"name": "myfolder", "subfolders": {}}}})
telemetry(CS, "content-moved", settings=_CSS, inbound_http={"path": _FOL + "/contents/" + _CT,
                                                              "body": '{"success":true,"subfolder_id":"' + _SF + '"}'},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _FOL + "/subfolders"}])

# Activity.
_cs("get_stream_logs", _S, "GET", "/v2/activity/logs/stream/" + _CSI)
_cs("get_stream_events", _S, "GET", "/v2/activity/events/stream/" + _CSI)
_cs("get_video_logs", {"video_id": _CT}, "GET", "/v2/activity/logs/stream/" + _CT)
_cs("get_video_events", {"video_id": _CT}, "GET", "/v2/activity/events/stream/" + _CT)
