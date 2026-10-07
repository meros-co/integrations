# MediaMTX (mediamtx): one vector per command, a second for the 1.2-1.20
# model where its endpoint name differs, and telemetry vectors for the polled
# lists. Targets are the endpoints of MediaMTX's OpenAPI definition
# (api/openapi.yaml at v1.21.1, apidocs/openapi.yaml at earlier tags);
# answers follow its schemas. Ids and names are made up.
MX = "mediamtx"
_MXNEW = "mediamtx-1-21"
_MXOLD = "mediamtx-1-2"
_MXID = "6f6d1c4e-2b9a-4c1e-9a55-0d3c2a8e1f42"
_MXID2 = "0b7e2d11-5f3c-4f6a-8e21-7c4d9a1b2e30"
_MXJ = lambda o: json.dumps(o, separators=(",", ":"))


def _mx(command, input, method, target, body=None, model=None, file=None, port=None, **extra):
    request = {"method": method, "target": target}
    if port is not None:
        request["port"] = port
    if body is not None:
        request["body"] = body
    v = {"spec": MX, "command": command, "input": input, "expect_request": request, **extra}
    if model:
        v["model"] = model
    if file:
        v["file"] = file
    V.append(v)


def _mx2(command, input, method, new, old, body=None, **extra):
    """A command whose endpoint is named differently before 1.21."""
    _mx(command, input, method, new, body=body, model=_MXNEW, **extra)
    _mx(command, input, method, old, body=body, model=_MXOLD, file=command + "-1-2")


_OK = {"status": 200, "body": _MXJ({"status": "ok"})}
_PG = {"page": 2, "items_per_page": 50}

# General.
_mx("get_info", {}, "GET", "/v3/info",
    http_reply={"status": 200, "body": _MXJ({"version": "v1.21.1", "started": "2026-10-06T08:00:00Z"})},
    expect_result={"ok": {"kind": "value", "value": {"version": "v1.21.1", "started": "2026-10-06T08:00:00Z"}}})
_mx("refresh_jwks", {}, "POST", "/v3/auth/jwks/refresh", http_reply=_OK, expect_result={"ok": {"kind": "ack"}})

# Global configuration.
_mx("get_global_config", {}, "GET", "/v3/config/global/get")
_mx("patch_global_config", {"changes": {"logLevel": "debug", "readTimeout": "15s"}}, "PATCH",
    "/v3/config/global/patch", body='{"logLevel":"debug","readTimeout":"15s"}')
_mx("set_log_level", {"level": "warn"}, "PATCH", "/v3/config/global/patch", body='{"logLevel":"warn"}',
    http_reply=_OK, expect_result={"ok": {"kind": "ack"}})
_mx("set_server_enabled", {"server": "rtmp", "enabled": False}, "PATCH", "/v3/config/global/patch",
    body='{"rtmp":false}')
_USERS = [{"user": "any", "pass": "", "ips": [], "permissions": [{"action": "read", "path": ""}]},
          {"user": "imperio", "pass": "s3cret", "ips": ["10.0.0.0/24"],
           "permissions": [{"action": "api"}, {"action": "publish", "path": ""}]}]
_mx("set_auth_internal_users", {"users": _USERS}, "PATCH", "/v3/config/global/patch",
    body=_MXJ({"authInternalUsers": _USERS}))

# Path defaults.
_mx2("get_path_defaults", {}, "GET", "/v3/config/path-defaults/get", "/v3/config/pathdefaults/get")
_mx2("patch_path_defaults", {"changes": {"recordDeleteAfter": "7d"}}, "PATCH",
     "/v3/config/path-defaults/patch", "/v3/config/pathdefaults/patch", body='{"recordDeleteAfter":"7d"}')
_mx2("set_default_record", {"enabled": True}, "PATCH",
     "/v3/config/path-defaults/patch", "/v3/config/pathdefaults/patch", body='{"record":true}')

# Path configurations.
_mx("list_path_configs", {}, "GET", "/v3/config/paths/list?page=0&itemsPerPage=100")
_mx("get_path_config", {"name": "live/stage"}, "GET", "/v3/config/paths/get/live%2Fstage")
_mx("add_path", {"name": "cam1", "source": "rtsp://admin:pw@10.0.0.20:554/main", "source_on_demand": True},
    "POST", "/v3/config/paths/add/cam1",
    body='{"source":"rtsp://admin:pw@10.0.0.20:554/main","sourceOnDemand":true,"record":false}',
    http_reply=_OK, expect_result={"ok": {"kind": "ack"}})
_CONF = {"source": "srt://10.0.0.30:9000?streamid=read:feed", "record": True, "recordFormat": "mpegts"}
_mx("add_path_config", {"name": "feed", "config": _CONF}, "POST", "/v3/config/paths/add/feed", body=_MXJ(_CONF))
_mx("patch_path_config", {"name": "cam1", "changes": {"maxReaders": 10}}, "PATCH",
    "/v3/config/paths/patch/cam1", body='{"maxReaders":10}')
_mx("replace_path_config", {"name": "cam1", "config": {"source": "publisher"}}, "POST",
    "/v3/config/paths/replace/cam1", body='{"source":"publisher"}')
# An unknown path: an ordinary failure with MediaMTX's error body.
_mx("delete_path_config", {"name": "cam9"}, "DELETE", "/v3/config/paths/delete/cam9",
    http_reply={"status": 404, "body": _MXJ({"status": "error", "error": "path configuration not found"})},
    expect_result={"error": {"error": "device_error", "code": "404"}})
_mx("set_path_source", {"name": "cam1", "source": "publisher"}, "PATCH", "/v3/config/paths/patch/cam1",
    body='{"source":"publisher"}')
_mx("set_source_on_demand", {"name": "cam1", "enabled": False}, "PATCH", "/v3/config/paths/patch/cam1",
    body='{"sourceOnDemand":false}')
_mx("set_path_record", {"name": "live/stage", "enabled": True}, "PATCH", "/v3/config/paths/patch/live%2Fstage",
    body='{"record":true}')
_mx("set_path_record_format", {"name": "cam1", "format": "fmp4"}, "PATCH", "/v3/config/paths/patch/cam1",
    body='{"recordFormat":"fmp4"}')
_mx("set_path_record_delete_after", {"name": "cam1", "duration": "12h"}, "PATCH", "/v3/config/paths/patch/cam1",
    body='{"recordDeleteAfter":"12h"}')
_mx("set_path_max_readers", {"name": "cam1", "max_readers": 0}, "PATCH", "/v3/config/paths/patch/cam1",
    body='{"maxReaders":0}')
_mx("set_path_override_publisher", {"name": "cam1", "enabled": False}, "PATCH", "/v3/config/paths/patch/cam1",
    body='{"overridePublisher":false}')
_FWD = [{"dest": "srt://10.0.0.40:9000?streamid=publish:stage"}]
_mx("set_path_forward_dests", {"name": "cam1", "dests": _FWD}, "PATCH", "/v3/config/paths/patch/cam1",
    body=_MXJ({"forward": _FWD}))

# Live paths.
_mx("list_paths", _PG, "GET", "/v3/paths/list?page=2&itemsPerPage=50")
_mx("get_path", {"name": "cam1"}, "GET", "/v3/paths/get/cam1")
_mx2("list_forward_dests", {"path": "cam1"}, "GET", "/v3/paths/forward-dests/list?path=cam1&page=0&itemsPerPage=100",
     "/v3/paths/forward/list?path=cam1&page=0&itemsPerPage=100")
_mx2("get_forward_dest", {"path": "cam1", "id": _MXID}, "GET", "/v3/paths/forward-dests/get?id=" + _MXID + "&path=cam1",
     "/v3/paths/forward/get?id=" + _MXID + "&path=cam1")
_mx("get_static_source", {"name": "cam1"}, "GET", "/v3/paths/static-sources/get/cam1")

# Connections and sessions of each protocol.
for proto in ("rtsp", "rtsps"):
    _mx2(f"list_{proto}_conns", {}, "GET", f"/v3/{proto}/conns/list?page=0&itemsPerPage=100",
         f"/v3/{proto}conns/list?page=0&itemsPerPage=100")
    _mx2(f"get_{proto}_conn", {"id": _MXID}, "GET", f"/v3/{proto}/conns/get/{_MXID}", f"/v3/{proto}conns/get/{_MXID}")
for proto in ("rtsp", "rtsps", "webrtc", "hls", "moq"):
    _mx2(f"list_{proto}_sessions", _PG, "GET", f"/v3/{proto}/sessions/list?page=2&itemsPerPage=50",
         f"/v3/{proto}sessions/list?page=2&itemsPerPage=50")
    _mx2(f"get_{proto}_session", {"id": _MXID}, "GET", f"/v3/{proto}/sessions/get/{_MXID}",
         f"/v3/{proto}sessions/get/{_MXID}")
    _mx2(f"kick_{proto}_session", {"id": _MXID}, "POST", f"/v3/{proto}/sessions/kick/{_MXID}",
         f"/v3/{proto}sessions/kick/{_MXID}", http_reply=_OK, expect_result={"ok": {"kind": "ack"}})
for proto in ("rtmp", "rtmps", "srt"):
    _mx2(f"list_{proto}_conns", {}, "GET", f"/v3/{proto}/conns/list?page=0&itemsPerPage=100",
         f"/v3/{proto}conns/list?page=0&itemsPerPage=100")
    _mx2(f"get_{proto}_conn", {"id": _MXID}, "GET", f"/v3/{proto}/conns/get/{_MXID}", f"/v3/{proto}conns/get/{_MXID}")
    _mx2(f"kick_{proto}_conn", {"id": _MXID}, "POST", f"/v3/{proto}/conns/kick/{_MXID}",
         f"/v3/{proto}conns/kick/{_MXID}")
_mx2("list_hls_muxers", {}, "GET", "/v3/hls/muxers/list?page=0&itemsPerPage=100",
     "/v3/hlsmuxers/list?page=0&itemsPerPage=100")
_mx2("get_hls_muxer", {"name": "live/stage"}, "GET", "/v3/hls/muxers/get/live%2Fstage",
     "/v3/hlsmuxers/get/live%2Fstage")

# Recordings.
_mx("list_recordings", {}, "GET", "/v3/recordings/list?page=0&itemsPerPage=100")
_mx("get_recordings", {"name": "cam1"}, "GET", "/v3/recordings/get/cam1",
    http_reply={"status": 200, "body": _MXJ({"name": "cam1", "segments": [{"start": "2026-10-06T08:00:00.000123+01:00"}]})},
    expect_result={"ok": {"kind": "value", "value": {"name": "cam1", "segments": [
        {"start": "2026-10-06T08:00:00.000123+01:00"}]}}})
_mx2("delete_recording_segment", {"path": "cam1", "start": "2026-10-06T08:00:00.000123+01:00"}, "DELETE",
     "/v3/recordings/segments/delete?path=cam1&start=2026-10-06T08%3A00%3A00.000123%2B01%3A00",
     "/v3/recordings/deletesegment?path=cam1&start=2026-10-06T08%3A00%3A00.000123%2B01%3A00")
# The playback server: another port of the same host (api/playback.openapi.yaml).
_SPANS = [{"start": "2026-10-06T08:00:00.000123+01:00", "duration": 60.0,
           "url": "http://192.0.2.10:9996/get?path=cam1&start=2026-10-06T08%3A00%3A00.000123%2B01%3A00&duration=60"}]
_mx("list_playback_spans", {"path": "live/stage"}, "GET", "/list?path=live%2Fstage", port=9996,
    http_reply={"status": 200, "body": _MXJ(_SPANS)},
    expect_result={"ok": {"kind": "value", "value": _SPANS}})
V.append({"spec": MX, "command": "list_playback_spans", "file": "list_playback_spans-other-port",
          "input": {"path": "cam1"}, "settings": {"playback_port": 8996, "playback_scheme": "https"},
          "expect_request": {"method": "GET", "port": 8996, "target": "/list?path=cam1"}})
# No recording in the range: 404 with a JSON error (1.11.1 and later).
V.append({"spec": MX, "command": "list_playback_spans", "file": "list_playback_spans-none",
          "input": {"path": "cam1"},
          "expect_request": {"method": "GET", "port": 9996, "target": "/list?path=cam1"},
          "http_reply": {"status": 404, "body": _MXJ({"status": "error", "error": "no recordings found"})},
          "expect_result": {"error": {"error": "device_error", "code": "404"}}})
# Without the credential (playback_auth none) a 401 is that command's answer;
# with it (inherit) it is the API credential refused, terminal.
V.append({"spec": MX, "command": "list_playback_spans", "file": "list_playback_spans-401-uncredentialed",
          "input": {"path": "cam1"}, "settings": {"auth": "basic", "username": "operator", "password": "pw"},
          "expect_request": {"method": "GET", "port": 9996, "target": "/list?path=cam1"},
          "http_reply": {"status": 401, "body": _MXJ({"status": "error", "error": "authentication error"})},
          "expect_result": {"error": {"error": "device_error", "code": "401"}}})
V.append({"spec": MX, "command": "list_playback_spans", "file": "list_playback_spans-refused",
          "input": {"path": "cam1"},
          "settings": {"auth": "basic", "username": "operator", "password": "wrong", "playback_auth": "inherit"},
          "expect_request": {"method": "GET", "port": 9996, "target": "/list?path=cam1"},
          "http_reply": {"status": 401, "body": _MXJ({"status": "error", "error": "authentication error"})},
          "expect_result": {"error": {"error": "auth"}}})
_mx("list_playback_spans_between",
    {"path": "cam1", "start": "2026-10-06T08:00:00Z", "end": "2026-10-06T18:00:00+01:00"}, "GET",
    "/list?path=cam1&start=2026-10-06T08%3A00%3A00Z&end=2026-10-06T18%3A00%3A00%2B01%3A00", port=9996)
# A refused credential: terminal, reported as auth.
V.append({"spec": MX, "command": "get_path", "file": "get_path-refused", "input": {"name": "cam1"},
          "settings": {"auth": "basic", "username": "imperio", "password": "wrong"},
          "expect_request": {"method": "GET", "target": "/v3/paths/get/cam1"},
          "http_reply": {"status": 401, "body": _MXJ({"status": "error", "error": "authentication error"})},
          "expect_result": {"error": {"error": "auth"}}})

# ── Telemetry ─────────────────────────────────────────────────────────────
_PATH1 = {
    "name": "cam1", "confName": "cam1", "source": {"type": "rtspSource", "id": ""},
    "ready": True, "readyTime": "2026-10-06T08:00:01Z", "available": True, "availableTime": "2026-10-06T08:00:01Z",
    "online": True, "onlineTime": "2026-10-06T08:00:01Z",
    "tracks": ["H264", "MPEG-4 Audio"],
    "tracks2": [{"codec": "H264", "codecProps": {"width": 1920, "height": 1080, "profile": "High", "level": "4.1"}},
                {"codec": "MPEG-4 Audio", "codecProps": {"sampleRate": 48000, "channelCount": 2}}],
    "bytesReceived": 52428800, "bytesSent": 104857600, "inboundBytes": 52428800, "outboundBytes": 104857600,
    "inboundFramesInError": 0,
    "readers": [{"type": "webRTCSession", "id": _MXID}, {"type": "hlsMuxer", "id": ""}]}
_PATH2 = {
    "name": "live/stage", "confName": "all_others", "source": {"type": "srtConn", "id": _MXID2},
    "ready": False, "readyTime": None, "available": False, "availableTime": None, "online": False, "onlineTime": None,
    "tracks": [], "tracks2": [], "bytesReceived": 0, "bytesSent": 0, "inboundBytes": 0, "outboundBytes": 0,
    "inboundFramesInError": 0, "readers": []}
_PATHSTATE = {
    "path_count": 2,
    "paths": {
        "cam1": {"conf_name": "cam1", "ready": True, "online": True, "available": True,
                 "ready_time": "2026-10-06T08:00:01Z", "online_time": "2026-10-06T08:00:01Z",
                 "source_type": "rtspSource", "source_id": "",
                 "readers": _MXJ([{"type": "webRTCSession", "id": _MXID}, {"type": "hlsMuxer", "id": ""}]),
                 "codecs": '["H264","MPEG-4 Audio"]',
                 "inbound_bytes": 52428800, "outbound_bytes": 104857600, "inbound_frames_in_error": 0,
                 "tracks": {"0": {"codec": "H264", "width": 1920, "height": 1080, "profile": "High", "level": "4.1"},
                            "1": {"codec": "MPEG-4 Audio", "sample_rate": 48000, "channel_count": 2}}},
        "live/stage": {"conf_name": "all_others", "ready": False, "online": False, "available": False,
                       "source_type": "srtConn", "source_id": _MXID2, "readers": "[]", "codecs": "[]",
                       "inbound_bytes": 0, "outbound_bytes": 0, "inbound_frames_in_error": 0}}}
telemetry(MX, "paths", inbound_http={
    "path": "/v3/paths/list?itemsPerPage=1000",
    "body": _MXJ({"itemCount": 2, "pageCount": 1, "items": [_PATH1, _PATH2]})},
    # A path that has gone leaves: the whole list replaces it.
    state_before={"paths": {"old": {"ready": True}, "cam1": {"tracks": {"2": {"codec": "Opus"}}}}},
    expect_state=_PATHSTATE)
# Before 1.16: ready only, counters as bytesReceived and bytesSent, codec names only.
telemetry(MX, "paths-1-5", inbound_http={
    "path": "/v3/paths/list?itemsPerPage=1000",
    "body": _MXJ({"itemCount": 1, "pageCount": 1, "items": [{
        "name": "cam1", "confName": "cam1", "source": {"type": "rtmpConn", "id": _MXID},
        "ready": True, "readyTime": "2026-10-06T08:00:01Z", "tracks": ["H264"],
        "bytesReceived": 1000, "bytesSent": 2000, "readers": []}]})},
    expect_state={"path_count": 1, "paths": {"cam1": {
        "conf_name": "cam1", "ready": True, "ready_time": "2026-10-06T08:00:01Z", "source_type": "rtmpConn",
        "source_id": _MXID, "readers": "[]", "codecs": '["H264"]', "inbound_bytes": 1000, "outbound_bytes": 2000}}})
# A list longer than a page: the entries it holds are updated, nothing is removed.
telemetry(MX, "paths-paged", inbound_http={
    "path": "/v3/paths/list?itemsPerPage=1000",
    "body": _MXJ({"itemCount": 1001, "pageCount": 2, "items": [_PATH2]})},
    state_before={"paths": {"old": {"ready": True}}},
    expect_state={"path_count": 1001, "paths": {"old": {"ready": True},
                                                "live/stage": _PATHSTATE["paths"]["live/stage"]}})
telemetry(MX, "info", inbound_http={"path": "/v3/info",
                                    "body": _MXJ({"version": "v1.21.1", "started": "2026-10-06T08:00:00Z"})},
          expect_state={"server": {"version": "v1.21.1", "started": "2026-10-06T08:00:00Z"}})
telemetry(MX, "global-config", inbound_http={"path": "/v3/config/global/get", "body": _MXJ({
    "logLevel": "info", "authMethod": "internal", "api": True, "rtsp": True, "rtmp": True, "hls": True,
    "webrtc": True, "srt": True, "moq": False, "playback": False, "metrics": True,
    "rtspAddress": ":8554", "rtspsAddress": ":8322", "rtmpAddress": ":1935", "rtmpsAddress": ":1936",
    "hlsAddress": ":8888", "webrtcAddress": ":8889", "srtAddress": ":8890", "playbackAddress": ":9996",
    "metricsAddress": ":9998", "authInternalUsers": []})},
    expect_state={"config": {
        "log_level": "info", "auth_method": "internal", "rtsp": True, "rtmp": True, "hls": True, "webrtc": True,
        "srt": True, "moq": False, "playback": False, "metrics": True, "rtsp_address": ":8554",
        "rtsps_address": ":8322", "rtmp_address": ":1935", "rtmps_address": ":1936", "hls_address": ":8888",
        "webrtc_address": ":8889", "srt_address": ":8890", "playback_address": ":9996",
        "metrics_address": ":9998"}})
_DEF = {"source": "publisher", "record": False, "recordFormat": "fmp4", "recordDeleteAfter": "1d",
        "sourceOnDemand": False, "maxReaders": 0}
_DEFSTATE = {"path_defaults": {"record": False, "record_format": "fmp4", "record_delete_after": "1d",
                               "source_on_demand": False, "max_readers": 0}}
telemetry(MX, "path-defaults", inbound_http={"path": "/v3/config/path-defaults/get", "body": _MXJ(_DEF)},
          expect_state=_DEFSTATE)
telemetry(MX, "path-defaults-1-2", inbound_http={"path": "/v3/config/pathdefaults/get", "body": _MXJ(_DEF)},
          expect_state=_DEFSTATE)
telemetry(MX, "path-configs", inbound_http={"path": "/v3/config/paths/list?itemsPerPage=1000", "body": _MXJ({
    "itemCount": 2, "pageCount": 1, "items": [
        dict(_DEF, name="cam1", source="rtsp://10.0.0.20/main", sourceOnDemand=True, record=True,
             overridePublisher=True, alwaysAvailable=False),
        dict(_DEF, name="all_others", overridePublisher=True, alwaysAvailable=False)]})},
    state_before={"path_configs": {"removed": {"record": True}}},
    expect_state={"path_config_count": 2, "path_configs": {
        "cam1": {"record": True, "record_format": "fmp4", "record_delete_after": "1d", "source_on_demand": True,
                 "max_readers": 0, "override_publisher": True, "always_available": False},
        "all_others": {"record": False, "record_format": "fmp4", "record_delete_after": "1d",
                       "source_on_demand": False, "max_readers": 0, "override_publisher": True,
                       "always_available": False}}})

_RTSP = {"id": _MXID, "created": "2026-10-06T08:01:00Z", "remoteAddr": "10.0.0.50:51234", "state": "publish",
         "path": "cam1", "query": "", "transport": "TCP", "profile": "AVP", "user": "imperio",
         "bytesReceived": 1000, "bytesSent": 20, "inboundBytes": 1000, "outboundBytes": 20, "conns": []}
_RTSPSTATE = {"path": "cam1", "state": "publish", "remote_addr": "10.0.0.50:51234", "transport": "TCP",
              "user": "imperio", "created": "2026-10-06T08:01:00Z", "inbound_bytes": 1000, "outbound_bytes": 20}
telemetry(MX, "rtsp-sessions", inbound_http={"path": "/v3/rtsp/sessions/list?itemsPerPage=1000",
                                             "body": _MXJ({"itemCount": 1, "pageCount": 1, "items": [_RTSP]})},
          # A closed session leaves.
          state_before={"rtsp": {"sessions": {_MXID2: {"path": "cam2"}}}},
          expect_state={"rtsp": {"session_count": 1, "sessions": {_MXID: _RTSPSTATE}}})
# 1.2-1.16: the earlier name and counters only as bytesReceived and bytesSent.
_RTSPOLD = {k: v for k, v in _RTSP.items() if k not in ("inboundBytes", "outboundBytes", "user", "conns")}
telemetry(MX, "rtsps-sessions-1-2", inbound_http={"path": "/v3/rtspssessions/list?itemsPerPage=1000",
                                                  "body": _MXJ({"itemCount": 1, "pageCount": 1, "items": [_RTSPOLD]})},
          state_before={"rtsps": {"sessions": {_MXID2: {"path": "cam2"}}}},
          expect_state={"rtsps": {"session_count": 1, "sessions": {
              _MXID: {k: v for k, v in _RTSPSTATE.items() if k != "user"}}}})
telemetry(MX, "webrtc-sessions", inbound_http={"path": "/v3/webrtc/sessions/list?itemsPerPage=1000", "body": _MXJ({
    "itemCount": 1, "pageCount": 1, "items": [{
        "id": _MXID2, "created": "2026-10-06T08:02:00Z", "remoteAddr": "10.0.0.60:50000",
        "peerConnectionEstablished": True, "localCandidate": "host/udp/10.0.0.2/8189",
        "remoteCandidate": "prflx/udp/10.0.0.60/50000", "state": "read", "path": "cam1", "query": "",
        "user": "", "userAgent": "Mozilla/5.0", "inboundBytes": 3000, "outboundBytes": 9000000}]})},
    expect_state={"webrtc": {"session_count": 1, "sessions": {_MXID2: {
        "path": "cam1", "state": "read", "remote_addr": "10.0.0.60:50000", "user": "",
        "created": "2026-10-06T08:02:00Z", "peer_connection_established": True, "inbound_bytes": 3000,
        "outbound_bytes": 9000000}}}})
# HLS sessions have no state or inbound counter; MoQ sessions no user.
telemetry(MX, "hls-sessions", inbound_http={"path": "/v3/hls/sessions/list?itemsPerPage=1000", "body": _MXJ({
    "itemCount": 1, "pageCount": 1, "items": [{
        "id": _MXID, "created": "2026-10-06T08:03:00Z", "isCDN": False, "remoteAddr": "10.0.0.70:40000",
        "path": "live/stage", "query": "", "user": "viewer", "userAgent": "AppleCoreMedia", "outboundBytes": 4096}]})},
    expect_state={"hls": {"session_count": 1, "sessions": {_MXID: {
        "path": "live/stage", "remote_addr": "10.0.0.70:40000", "user": "viewer", "created": "2026-10-06T08:03:00Z",
        "outbound_bytes": 4096}}}})
# No MoQ session: the list empties.
telemetry(MX, "moq-sessions-empty", inbound_http={"path": "/v3/moq/sessions/list?itemsPerPage=1000",
                                                  "body": _MXJ({"itemCount": 0, "pageCount": 0, "items": []})},
          state_before={"moq": {"session_count": 1, "sessions": {_MXID: {"path": "cam1"}}}},
          expect_state={"moq": {"session_count": 0}})
_RTMP = {"id": _MXID, "created": "2026-10-06T08:04:00Z", "remoteAddr": "10.0.0.80:60000", "state": "publish",
         "path": "obs", "query": "", "user": "", "userAgent": "", "bytesReceived": 77, "bytesSent": 3,
         "inboundBytes": 77, "outboundBytes": 3, "outboundFramesDiscarded": 0}
_RTMPSTATE = {"path": "obs", "state": "publish", "remote_addr": "10.0.0.80:60000", "user": "",
              "created": "2026-10-06T08:04:00Z", "inbound_bytes": 77, "outbound_bytes": 3}
telemetry(MX, "rtmp-conns", inbound_http={"path": "/v3/rtmp/conns/list?itemsPerPage=1000",
                                          "body": _MXJ({"itemCount": 1, "pageCount": 1, "items": [_RTMP]})},
          state_before={"rtmp": {"conns": {_MXID2: {"path": "gone"}}}},
          expect_state={"rtmp": {"conn_count": 1, "conns": {_MXID: _RTMPSTATE}}})
telemetry(MX, "rtmps-conns-1-2", inbound_http={"path": "/v3/rtmpsconns/list?itemsPerPage=1000",
                                               "body": _MXJ({"itemCount": 1, "pageCount": 1, "items": [_RTMP]})},
          expect_state={"rtmps": {"conn_count": 1, "conns": {_MXID: _RTMPSTATE}}})
telemetry(MX, "srt-conns", inbound_http={"path": "/v3/srt/conns/list?itemsPerPage=1000", "body": _MXJ({
    "itemCount": 1, "pageCount": 1, "items": [{
        "id": _MXID2, "created": "2026-10-06T08:05:00Z", "remoteAddr": "10.0.0.90:9000", "state": "publish",
        "path": "live/stage", "query": "", "user": "", "bytesReceived": 123456, "bytesSent": 789,
        "msRTT": 12.5, "mbpsReceiveRate": 6.25, "mbpsSendRate": 0.01, "mbpsLinkCapacity": 940.0,
        "packetsReceivedLoss": 3, "packetsReceivedDrop": 1, "packetsRetrans": 0, "packetsSendDrop": 0,
        "msReceiveTsbPdDelay": 120}]})},
    expect_state={"srt": {"conn_count": 1, "conns": {_MXID2: {
        "path": "live/stage", "state": "publish", "remote_addr": "10.0.0.90:9000", "user": "",
        "created": "2026-10-06T08:05:00Z", "inbound_bytes": 123456, "outbound_bytes": 789, "rtt": 12.5,
        "receive_rate": 6.25, "send_rate": 0.01, "link_capacity": 940.0, "packets_received_loss": 3,
        "packets_received_drop": 1, "packets_retrans": 0, "packets_send_drop": 0, "receive_latency": 120}}}})
telemetry(MX, "hls-muxers", inbound_http={"path": "/v3/hls/muxers/list?itemsPerPage=1000", "body": _MXJ({
    "itemCount": 1, "pageCount": 1, "items": [{
        "path": "live/stage", "created": "2026-10-06T08:06:00Z", "lastRequest": "2026-10-06T08:07:00Z",
        "bytesSent": 5000, "outboundBytes": 5000, "outboundFramesDiscarded": 0}]})},
    state_before={"hls": {"muxers": {"cam1": {"outbound_bytes": 1}}}},
    expect_state={"hls": {"muxer_count": 1, "muxers": {"live/stage": {
        "created": "2026-10-06T08:06:00Z", "last_request": "2026-10-06T08:07:00Z", "outbound_bytes": 5000}}}})
telemetry(MX, "hls-muxers-1-2", inbound_http={"path": "/v3/hlsmuxers/list?itemsPerPage=1000", "body": _MXJ({
    "itemCount": 1, "pageCount": 1, "items": [{
        "path": "cam1", "created": "2026-10-06T08:06:00Z", "lastRequest": "2026-10-06T08:07:00Z", "bytesSent": 42}]})},
    expect_state={"hls": {"muxer_count": 1, "muxers": {"cam1": {
        "created": "2026-10-06T08:06:00Z", "last_request": "2026-10-06T08:07:00Z", "outbound_bytes": 42}}}})
# This integration's own changes are read back at once.
telemetry(MX, "config-changed", inbound_http={"path": "/v3/config/paths/patch/cam1", "body": _MXJ({"status": "ok"})},
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/v3/config/paths/list?itemsPerPage=1000"},
                            {"method": "GET", "target": "/v3/config/global/get"},
                            {"method": "GET", "target": "/v3/paths/list?itemsPerPage=1000"}])
telemetry(MX, "kicked", inbound_http={"path": "/v3/srt/conns/kick/" + _MXID2, "body": _MXJ({"status": "ok"})},
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/v3/srt/conns/list?itemsPerPage=1000"},
                            {"method": "GET", "target": "/v3/paths/list?itemsPerPage=1000"}])
telemetry(MX, "kicked-1-2", inbound_http={"path": "/v3/rtspsessions/kick/" + _MXID, "body": _MXJ({"status": "ok"})},
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/v3/rtspsessions/list?itemsPerPage=1000"},
                            {"method": "GET", "target": "/v3/paths/list?itemsPerPage=1000"}])
