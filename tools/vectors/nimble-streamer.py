# Nimble Streamer native API (nimble-streamer): one vector per command, and
# telemetry vectors for the polled resources. Targets and bodies follow the
# native API reference's request examples; reply bodies its response
# examples.
NB = "nimble-streamer"


def _nb(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": NB, "command": command, "input": input, "expect_request": request, **extra})


_NBA = {"app": "live", "stream": "stream"}
_nb("get_server_status", {}, "GET", "/manage/server_status",
    http_reply={"status": 200, "body": json.dumps({"Connections": 10, "OutRate": 5120000})},
    expect_result={"ok": {"kind": "value", "value": {"Connections": 10, "OutRate": 5120000}}})
_nb("sync_panel_settings", {}, "POST", "/manage/sync_panel_settings")
_nb("reload_config", {}, "POST", "/manage/reload_config")
_nb("reload_config_with_drm", {}, "POST", "/manage/reload_config?drm=true")
_nb("reload_ssl_certificates", {}, "POST", "/manage/reload_ssl_certificates")
_nb("get_live_streams_status", {}, "GET", "/manage/live_streams_status")
_nb("list_sessions", {}, "GET", "/manage/sessions")
_nb("delete_sessions", {"ids": [4, 5]}, "POST", "/manage/sessions/delete", body="[4,5]",
    http_reply={"status": 200, "body": '{"status":"Ok"}'}, expect_result={"ok": {"kind": "value", "value": {"status": "Ok"}}})
_nb("get_rtmp_settings", {}, "GET", "/manage/rtmp_settings")
_nb("get_rtmp_republish_stats", {}, "GET", "/manage/rtmp/republish/stats")
_nb("list_rtmp_republish_rules", {}, "GET", "/manage/rtmp/republish")
_nb("get_rtmp_republish_rule", {"rule": "1"}, "GET", "/manage/rtmp/republish/1")
_nb("create_rtmp_republish_rule", {"src_app": "live", "src_stream": "stream", "dest_addr": "127.0.0.1", "dest_port": 1999,
                                   "dest_app": "live-re", "dest_stream": "dest_stream"},
    "POST", "/manage/rtmp/republish",
    body='{"src_app":"live","src_stream":"stream","dest_addr":"127.0.0.1","dest_port":1999,"dest_app":"live-re","dest_stream":"dest_stream"}')
_NBRULE = {"src_app": "live", "src_stream": "stream", "dest_addr": "a.rtmp.youtube.com", "dest_port": 1935, "dest_app": "live2",
           "dest_stream": "abcd-1234", "auth_schema": "NONE", "ssl": False}
_nb("create_rtmp_republish_rule_custom", {"rule": _NBRULE}, "POST", "/manage/rtmp/republish",
    body=json.dumps(_NBRULE, separators=(",", ":")))
_nb("delete_rtmp_republish_rule", {"rule": "1"}, "DELETE", "/manage/rtmp/republish/1",
    http_reply={"status": 200, "body": '{"status": "NotFound"}'},
    expect_result={"ok": {"kind": "value", "value": {"status": "NotFound"}}})
_nb("get_mpegts_status", {}, "GET", "/manage/mpeg2ts_status")
_nb("get_mpegts_settings", {}, "GET", "/manage/mpeg2ts_settings")
_nb("get_srt_sender_stats", {}, "GET", "/manage/srt_sender_stats")
_nb("get_srt_receiver_stats", {}, "GET", "/manage/srt_receiver_stats")
_nb("get_rist_sender_stats", {}, "GET", "/manage/rist_sender_stats")
_nb("get_rist_receiver_stats", {}, "GET", "/manage/rist_receiver_stats")
_nb("list_ndi_streams", {}, "GET", "/manage/ndi/list")
_nb("get_dvr_status", {}, "GET", "/manage/dvr_status")
_nb("get_dvr_timeline", {}, "GET", "/manage/dvr_status?timeline=true")
_nb("get_stream_dvr_status", {"app": "larix", "stream": "rtmp"}, "GET", "/manage/dvr_status/larix/rtmp")
_nb("get_stream_dvr_timeline", {"app": "larix", "stream": "rtmp"}, "GET", "/manage/dvr_status/larix/rtmp?timeline=true")
_nb("export_dvr_subtitles", dict(_NBA, start=1542708934, end=1542712534), "GET",
    "/manage/dvr/export_srt/live/stream?start=1542708934&end=1542712534",
    http_reply={"status": 200, "body": "1\n00:00:01,000 --> 00:00:02,000\nHello\n"},
    expect_result={"ok": {"kind": "value", "value": "1\n00:00:01,000 --> 00:00:02,000\nHello"}})
_nb("export_dvr_subtitles_language", dict(_NBA, start=1542708934, end=1542712534, lang="eng"), "GET",
    "/manage/dvr/export_srt/live/stream?start=1542708934&end=1542712534&lang=eng")
_nb("reload_dvr_archive", _NBA, "POST", "/manage/dvr/reload/live/stream")
_nb("cleanup_dvr_archive", _NBA, "POST", "/manage/dvr/cleanup_archive/live/stream")
_nb("cleanup_dvr_archive_keep", dict(_NBA, minutes=60), "POST", "/manage/dvr/cleanup_archive/live/stream?target_depth=60")
_nb("cleanup_dvr_archive_span", dict(_NBA, **{"from": 1507732548, "to": 1507732600}), "POST",
    "/manage/dvr/cleanup_archive/live/stream?from=1507732548&to=1507732600")
_nb("insert_scte35_out", {"app": "live", "stream": "stream1"}, "POST", "/manage/advertizer/scte35_out/live/stream1")
_nb("insert_scte35_out_for", {"app": "live", "stream": "stream2", "duration": 10}, "POST",
    "/manage/advertizer/scte35_out/live/stream2?duration=10")
_nb("insert_scte35_out_program", {"app": "live", "stream": "stream2", "duration": 10, "program": "prog42"}, "POST",
    "/manage/advertizer/scte35_out/live/stream2?duration=10&unique_program_id=prog42")
_nb("insert_scte35_in", _NBA, "POST", "/manage/advertizer/scte35_in/live/stream")
_nb("insert_scte35_in_program", dict(_NBA, program="prog42"), "POST",
    "/manage/advertizer/scte35_in/live/stream?unique_program_id=prog42")
_nb("insert_scte35_time_signal", dict(_NBA, seg_type="0x34", upid_type="0x8", upid="12345"), "POST",
    "/manage/advertizer/scte35_time_signal/live/stream?seg_type=0x34&upid_type=0x8&upid=12345")
_nb("insert_scte35_time_signal_for", dict(_NBA, seg_type="0x34", upid_type="0x8", upid="12345", duration=10), "POST",
    "/manage/advertizer/scte35_time_signal/live/stream?seg_type=0x34&upid_type=0x8&upid=12345&duration=10")
_nb("get_icecast_stream_info", {"app": "live_radio", "stream": "audio_stream"}, "GET",
    "/manage/icecast_stream_info/live_radio/audio_stream")
_nb("insert_icecast_metadata", {"app": "live", "stream": "radio", "title": "title", "url": "url"}, "POST",
    "/manage/icecast_metadata/live/radio", body='{"streamtitle":"title","streamurl":"url"}')
_nb("get_server_playlist_status", {}, "GET", "/manage/server_playlist_status")
_nb("get_publish_control_status", {}, "GET", "/manage/publish_control/status")
_nb("deny_publishers", {"ids": ["1", "2"]}, "POST", "/manage/publish_control/deny",
    body='{"PublishControlDenyRequest":["1","2"]}')
_nb("get_cache_key", {"url": "http://127.0.0.1:8081/dvr-test-remote-vod/sample.mp4"}, "POST", "/manage/data_cache/get_key",
    body='{"url":"http://127.0.0.1:8081/dvr-test-remote-vod/sample.mp4"}')
_nb("delete_cache_item", {"key": "127.0.0.1:8081/vod/sample.mp4"}, "POST", "/manage/data_cache/delete",
    body='{"key":"127.0.0.1:8081/vod/sample.mp4","dry_run":false}')
_nb("list_sdi_sources", {}, "GET", "/manage/sdi/list")
_nb("list_sdi_sinks", {}, "GET", "/manage/sdi/sink-list")
_nb("list_aja_devices", {}, "GET", "/manage/aja/list")

# Telemetry.
telemetry(NB, "server-status", inbound_http={"path": "/manage/server_status", "body": json.dumps(
    {"Connections": 10, "OutRate": 5120000, "SysInfo": {"ap": 2, "scl": 0, "tpms": 2098434048, "fpms": 775127040,
                                                        "tsss": 2145382400, "fsss": 1707151360}})},
    expect_state={"server": {"connections": 10, "out_rate": 5120000, "processors": 2, "cpu_load": 0.0,
                             "total_memory": 2098434048, "free_memory": 775127040, "total_swap": 2145382400,
                             "free_swap": 1707151360}})
telemetry(NB, "live-streams", inbound_http={"path": "/manage/live_streams_status", "body": json.dumps([
    {"app": "live", "streams": [
        {"acodec": "mp4a.40.2", "vcodec": "avc1.42c01f", "publish_time": "1524060893", "bandwidth": "1697348",
         "protocol": "RTSP", "resolution": "1280x720", "strm": "stream_rtsp", "source_url": "rtmp://127.0.0.1:1935/live/stream",
         "publisher_ip": "192.168.0.95", "publisher_port": 60349},
        {"vcodec": "avc1.64001e", "publish_time": "1524060893", "bandwidth": "286200", "resolution": "640x360",
         "protocol": "ENCODER", "strm": "stream_360p"}]}])},
    state_before={"live": {"live": {"gone": {"protocol": "RTMP"}}}},
    expect_state={"live": {"live": {
        "stream_rtsp": {"acodec": "mp4a.40.2", "vcodec": "avc1.42c01f", "publish_time": 1524060893, "bandwidth": 1697348,
                        "protocol": "RTSP", "resolution": "1280x720", "source_url": "rtmp://127.0.0.1:1935/live/stream",
                        "publisher_ip": "192.168.0.95", "publisher_port": 60349},
        "stream_360p": {"vcodec": "avc1.64001e", "publish_time": 1524060893, "bandwidth": 286200, "resolution": "640x360",
                        "protocol": "ENCODER"}}}})
telemetry(NB, "republish-stats", inbound_http={"path": "/manage/rtmp/republish/stats", "body": json.dumps({
    "stats": [{"bandwidth": 613564, "bytes_recv": 3485, "bytes_sent": 8166049, "dest_addr": "127.0.0.1", "dest_app": "live-re",
               "dest_port": 1999, "dest_stream": "dest_stream", "owner": 1, "retry_count": 1, "rule_id": "1",
               "session_duration": 102, "src_app": "live", "src_stream": "stream", "state": "connected"}],
    "status": "Ok"})},
    expect_state={"republish": {"1": {"state": "connected", "bandwidth": 613564, "bytes_sent": 8166049, "bytes_recv": 3485,
                                      "session_duration": 102, "retry_count": 1, "owner": 1, "source": "live/stream",
                                      "destination": "127.0.0.1:1999/live-re/dest_stream"}}})
telemetry(NB, "playout", inbound_http={"path": "/manage/server_playlist_status", "body": json.dumps([
    {"stream": "live/first_playlist", "block_id": "33", "block_duration": 45, "main_stream": "live/stream1",
     "main_stream_type": "live", "main_stream_duration": 45, "default_stream": "/home/content/mp4/sample.mp4",
     "default_stream_type": "vod", "default_stream_duration": 45},
    {"stream": "live/second_playlist", "block_id": "33", "block_duration": 113, "main_stream": "live/stream2",
     "main_stream_type": "live", "main_stream_duration": 113}])},
    expect_state={"playout": {
        "live/first_playlist": {"block_id": "33", "block_duration": 45, "main_stream": "live/stream1", "main_stream_type": "live",
                                "main_stream_duration": 45, "default_stream": "/home/content/mp4/sample.mp4",
                                "default_stream_type": "vod", "default_stream_duration": 45},
        "live/second_playlist": {"block_id": "33", "block_duration": 113, "main_stream": "live/stream2",
                                 "main_stream_type": "live", "main_stream_duration": 113}}})
telemetry(NB, "publish-control", inbound_http={"path": "/manage/publish_control/status", "body": json.dumps({
    "PublishControlStatus": [{"key": "1", "id": "ID_1", "ip": "192.168.1.1", "stream": "live/stream1"},
                             {"key": "2", "id": "ID_2", "ip": "192.168.1.2", "stream": "live/stream2"}]})},
    expect_state={"publishers": {"1": {"id": "ID_1", "ip": "192.168.1.1", "stream": "live/stream1"},
                                 "2": {"id": "ID_2", "ip": "192.168.1.2", "stream": "live/stream2"}}})
# Downloads saved to the host file named (never written by a vector).
_nb("export_dvr_mp4", dict(_NBA, start=1542708934, end=1542712534, to={"file": "span.mp4"}), "GET",
    "/manage/dvr/export_mp4/live/stream?start=1542708934&end=1542712534")
