# PHABRIX Qx REST API on port 8080 (PHABRIX Qx User Manual, software release
# 4.8, p.16-2 to 16-5). The requests and payloads are the manual's own examples:
# PUT /api/v1/generator/standards/1920x1080p60/YCbCr%3A422%3A10/3G_A_Rec.709/
# 100%25%20Bars with {"action":"start","pathological":{"type":"CheckField",
# "pairs":100}}, the audio QuickConfig, {"action":"enable"} on
# /eventlog/config/restApi, the multicast request and stream selection bodies,
# and {"action":"clearList"}. The manual shows no reply bodies; the replies here
# are placeholders, kept whole as JSON text.
QX = "phabrix-qx"
_OK = {"ok": {"kind": "ack"}}


def _qx(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": QX, "command": command, "input": input, "expect_request": req, **extra})


_qx("browse", {"path": "v1"}, "GET", "/api/v1",
    http_reply={"status": 200, "body": '{"analyser":{},"generator":{}}'},
    expect_result={"ok": {"kind": "value", "value": {"analyser": {}, "generator": {}}}})
_qx("get_analyzer_status", {}, "GET", "/api/v1/analyser/status")
_qx("get_crc_summary", {}, "GET", "/api/v1/analyser/crcSummary")
_qx("get_generator_status", {}, "GET", "/api/v1/generator/status")
_qx("get_generator_standards", {}, "GET", "/api/v1/generator/standards")
_qx("get_generator_standards_under", {"path": "1920x1080p60/YCbCr:422:10"}, "GET",
    "/api/v1/generator/standards/1920x1080p60/YCbCr%3A422%3A10")
_STD = {"resolution": "1920x1080p60", "color_format": "YCbCr:422:10", "level": "3G_A_Rec.709",
        "pattern": "100% Bars"}
_STD_PATH = "/api/v1/generator/standards/1920x1080p60/YCbCr%3A422%3A10/3G_A_Rec.709/100%25%20Bars"
_qx("generate_standard", _STD, "PUT", _STD_PATH, '{"action":"start"}',
    http_reply={"status": 200, "body": ""}, expect_result=_OK)
_qx("generate_standard_pathological", {**_STD, "type": "CheckField", "pairs": 100}, "PUT", _STD_PATH,
    '{"action":"start","pathological":{"type":"CheckField","pairs":100}}')
_qx("get_generator_audio", {}, "GET", "/api/v1/generator/audio")
_qx("set_generator_audio_quick", {"mode": "Fixed Tone", "gain_type": "Fixed Levels", "gain": -10, "pitch": "E 3"},
    "PUT", "/api/v1/generator/audio",
    '{"mode":"Fixed Tone","quickConfig":{"gainType":"Fixed Levels","gain":-10,"pitch":"E 3"}}')
_qx("set_generator_audio", {"config": {"mode": "Custom", "groups": []}}, "PUT", "/api/v1/generator/audio",
    '{"mode":"Custom","groups":[]}')
_qx("get_eye_status", {}, "GET", "/api/v1/eye/status")
_qx("get_jitter_status", {}, "GET", "/api/v1/jitter/status")
_qx("get_event_logs", {}, "GET", "/api/v1/eventlog/logs")
_qx("set_event_log_config", {"item": "restApi", "enabled": True}, "PUT", "/api/v1/eventlog/config/restApi",
    '{"action":"enable"}', http_reply={"status": 200, "body": ""}, expect_result=_OK)
_qx("set_event_log_config", {"item": "restApi", "enabled": False}, "PUT", "/api/v1/eventlog/config/restApi",
    '{"action":"disable"}', file="set_event_log_config.disable",
    http_reply={"status": 400, "body": ""}, expect_result={"error": {"error": "device_error", "code": "400"}})

_STREAM = {"destination_address": "230.0.0.15", "destination_port": 16, "match_ssrc": True,
           "source_address": "192.168.10.30", "source_port": 16, "ssrc": 123456}
_STREAM_BODY = ('{"action":"analyseStream","destinationIpAddress":"230.0.0.15","destinationUdpPort":16,'
                '"matchSsrc":true,"sourceIpAddress":"192.168.10.30","sourceUdpPort":16,"ssrc":123456}')
for suffix, base in (("", "/api/v1/ipReceive"), ("_2110", "/api/v1/2110/receive")):
    _qx("get_multicast_requests" + suffix, {}, "GET", base + "/multicastRequests")
    _qx("request_multicast" + suffix, {"address": "230.0.0.15"}, "PUT", base + "/multicastRequests",
        '{"action":"request","destinationIpAddress":"230.0.0.15"}')
    _qx("get_media_streams" + suffix, {}, "GET", base + "/mediaStreams")
    _qx("analyze_media_stream" + suffix, _STREAM, "PUT", base + "/mediaStreams", _STREAM_BODY)
    _qx("get_requested_media_streams" + suffix, {}, "GET", base + "/requestedMediaStreams")
    _qx("clear_media_streams" + suffix, {}, "PUT", base + "/mediaStreams", '{"action":"clearList"}')


# ── Telemetry: each reading kept whole as JSON text ──
def _tel(name, path, body, expect_state):
    telemetry(QX, name, inbound_http={"path": path, "body": json.dumps(body, separators=(",", ":"))},
              expect_state=expect_state)


_A = {"standard": "1920x1080p60"}
_tel("analyser-status", "/api/v1/analyser/status", _A, {"analyzer": {"status": '{"standard":"1920x1080p60"}'}})
_tel("crc-summary", "/api/v1/analyser/crcSummary", {"errors": 0}, {"analyzer": {"crc_summary": '{"errors":0}'}})
_tel("generator-status", "/api/v1/generator/status", _A, {"generator": {"status": '{"standard":"1920x1080p60"}'}})
_tel("generator-audio", "/api/v1/generator/audio", {"mode": "Fixed Tone"},
     {"generator": {"audio": '{"mode":"Fixed Tone"}'}})
_tel("eye", "/api/v1/eye/status", {"amplitude": 800}, {"eye": {"status": '{"amplitude":800}'}})
_tel("jitter", "/api/v1/jitter/status", {"alignment": 0.1}, {"jitter": {"status": '{"alignment":0.1}'}})
_tel("eventlog", "/api/v1/eventlog/logs", [], {"eventlog": {"logs": "[]"}})
_tel("multicast-2022-6", "/api/v1/ipReceive/multicastRequests", {"destinationIpAddress": "230.0.0.15"},
     {"ip_2022_6": {"multicast_requests": '{"destinationIpAddress":"230.0.0.15"}'}})
_tel("streams-2022-6", "/api/v1/ipReceive/mediaStreams", [], {"ip_2022_6": {"media_streams": "[]"}})
_tel("requested-2022-6", "/api/v1/ipReceive/requestedMediaStreams", [],
     {"ip_2022_6": {"requested_media_streams": "[]"}})
_tel("multicast-2110", "/api/v1/2110/receive/multicastRequests", [], {"ip_2110": {"multicast_requests": "[]"}})
_tel("streams-2110", "/api/v1/2110/receive/mediaStreams", [{"ssrc": 1}],
     {"ip_2110": {"media_streams": '[{"ssrc":1}]'}})
_tel("requested-2110", "/api/v1/2110/receive/requestedMediaStreams", [],
     {"ip_2110": {"requested_media_streams": "[]"}})
