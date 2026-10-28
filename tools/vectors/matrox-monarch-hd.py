MHD = "matrox-monarch-hd"
# Matrox Monarch HD Control API (Dev Tools Reference Guide v2.2.0, chapter
# 1): GET /Monarch/syncconnect/sdk.aspx?command=<command>[,<arg>...] with an
# HTTP Basic credential; replies are plain text. Replies follow the guide's
# own examples.

_MHD = "/Monarch/syncconnect/sdk.aspx?command="
_MHD_OK = {"status": 200, "body": "SUCCESS"}
_MHD_ACK = {"ok": {"kind": "ack"}}


def _mhd(command, input, query, **extra):
    http(MHD, command, input, "GET", _MHD + query, **extra)


def _mhd_value(value):
    return {"ok": {"kind": "value", "value": value}}


_mhd("get_status", {}, "GetStatus",
     http_reply={"status": 200, "body": "RECORD: ON, STREAM:RTSP,READY, NAME:DEVICE2"},
     expect_result=_mhd_value("RECORD: ON, STREAM:RTSP,READY, NAME:DEVICE2"))
_mhd("get_status", {}, "GetStatus", file="get_status.retry",
     http_reply={"status": 200, "body": "RETRY"}, expect_result={"error": {"error": "device_error"}})
_mhd("get_status", {}, "GetStatus", file="get_status.refused",
     http_reply={"status": 401, "body": ""}, expect_result={"error": {"error": "auth"}})
_mhd("get_status", {}, "GetStatus", file="get_status.https", settings={"scheme": "https"})
for _c, _q in [("start_streaming", "StartStreaming"), ("start_recording", "StartRecording"),
               ("start_streaming_and_recording", "StartStreamingAndRecording"),
               ("stop_streaming", "StopStreaming"), ("stop_recording", "StopRecording"),
               ("stop_streaming_and_recording", "StopStreamingAndRecording")]:
    _mhd(_c, {}, _q, http_reply=_MHD_OK, expect_result=_MHD_ACK)
_mhd("stop_streaming", {}, "StopStreaming", file="stop_streaming.failed",
     http_reply={"status": 200, "body": "FAILED"}, expect_result={"error": {"error": "device_error"}})
_mhd("get_input_status", {}, "GetInputStatus",
     http_reply={"status": 200, "body": "HDMI: 1920x1080i, 29.97 fps"},
     expect_result=_mhd_value("HDMI: 1920x1080i, 29.97 fps"))
_mhd("set_streaming_video_data_rate", {"kbps": 8500}, "SetStreamingVideoDataRate,8500",
     http_reply=_MHD_OK, expect_result=_MHD_ACK)
_mhd("get_streaming_video_data_rate", {}, "GetStreamingVideoDataRate",
     http_reply={"status": 200, "body": "BITRATE: 8500"}, expect_result=_mhd_value("8500"))
_mhd("set_rtsp", {"name": "Stream1"}, "SetRTSP,Stream1,554", http_reply=_MHD_OK, expect_result=_MHD_ACK)
_mhd("set_rtsp", {"name": "live/cam=1", "port": "8554"}, "SetRTSP,live/cam=1,8554", file="set_rtsp.8554")
_mhd("get_rtsp", {}, "GetRTSP",
     http_reply={"status": 200, "body": "rtsp://192.168.1.20:554/Stream1, Stream1, 554"},
     expect_result=_mhd_value("rtsp://192.168.1.20:554/Stream1, Stream1, 554"))
_mhd("set_rtmp", {"url": "rtmp://192.168.66.200/live", "stream": "livestream"},
     "SetRTMP,rtmp://192.168.66.200/live,livestream", http_reply=_MHD_OK, expect_result=_MHD_ACK)
_mhd("set_rtmp_with_credentials",
     {"url": "rtmp://monarchStream/live", "stream": "show 1", "username": "user", "password": "p@ss+word"},
     "SetRTMP,rtmp://monarchStream/live,show%201,user,p%40ss%2Bword")
_mhd("get_rtmp", {}, "GetRTMP",
     http_reply={"status": 200, "body": "rtmp://192.168.66.200/live, livestream"},
     expect_result=_mhd_value("rtmp://192.168.66.200/live, livestream"))
_mhd("set_record_file_name", {"filename": "File1"}, "SetRecordFileName,File1", http_reply=_MHD_OK,
     expect_result=_MHD_ACK)
_mhd("get_record_file_name", {}, "GetRecordFileName",
     http_reply={"status": 200, "body": "RECORD FILENAME: File1"}, expect_result=_mhd_value("File1"))
_mhd("get_serial_number", {}, "GetSerialNumber",
     http_reply={"status": 200, "body": "Bx12345"}, expect_result=_mhd_value("Bx12345"))

# Telemetry.
telemetry(MHD, "status", inbound_http={"path": _MHD + "GetStatus",
                                        "body": "RECORD: READY, STREAM:RTMP,READY, NAME:DEVICE1"},
          expect_state={"device": {"name": "DEVICE1"}, "record": {"state": "READY", "active": False},
                        "stream": {"mode": "RTMP", "state": "READY", "active": False}})
telemetry(MHD, "status-streaming", inbound_http={"path": _MHD + "GetStatus",
                                                  "body": "RECORD: DISABLED, STREAM:RTMP,ON, NAME:DEVICE5\r\n"},
          expect_state={"device": {"name": "DEVICE5"}, "record": {"state": "DISABLED", "active": False},
                        "stream": {"mode": "RTMP", "state": "ON", "active": True}})
telemetry(MHD, "input", inbound_http={"path": _MHD + "GetInputStatus", "body": "HDMI: 1920x1080i, 29.97 fps"},
          expect_state={"input": {"status": "HDMI: 1920x1080i, 29.97 fps", "present": True, "connector": "HDMI",
                                  "width": 1920, "height": 1080, "scan": "interlaced", "frame_rate": 29.97}})
telemetry(MHD, "input-none",
          state_before={"input": {"status": "HDMI: 1920x1080i, 29.97 fps", "present": True, "connector": "HDMI",
                                  "width": 1920, "height": 1080, "scan": "interlaced", "frame_rate": 29.97}},
          inbound_http={"path": _MHD + "GetInputStatus", "body": "No video input"},
          expect_state={"input": {"status": "No video input", "present": False}})
telemetry(MHD, "bitrate", inbound_http={"path": _MHD + "GetStreamingVideoDataRate", "body": "BITRATE: 4000"},
          expect_state={"stream": {"bitrate_kbps": 4000}})
telemetry(MHD, "rtsp", inbound_http={"path": _MHD + "GetRTSP",
                                      "body": "rtsp://192.168.1.20:8554/Stream1, Stream1, 8554"},
          expect_state={"stream": {"rtsp": {"url": "rtsp://192.168.1.20:8554/Stream1", "name": "Stream1",
                                            "port": 8554}}})
telemetry(MHD, "rtmp", inbound_http={"path": _MHD + "GetRTMP", "body": "rtmp://192.168.66.200/live, livestream"},
          expect_state={"stream": {"rtmp": {"url": "rtmp://192.168.66.200/live", "stream_name": "livestream"}}})
telemetry(MHD, "record-file-name", inbound_http={"path": _MHD + "GetRecordFileName",
                                                  "body": "RECORD FILENAME: recordings/File1"},
          expect_state={"record": {"file_name": "recordings/File1"}})
telemetry(MHD, "serial-number", inbound_http={"path": _MHD + "GetSerialNumber", "body": "Bx12345\r\n"},
          expect_state={"device": {"serial_number": "Bx12345"}})
telemetry(MHD, "serial-number-retry", inbound_http={"path": _MHD + "GetSerialNumber", "body": "RETRY"},
          expect_state={})

# Read-backs.
_MHD_STATUS = {"method": "GET", "target": _MHD + "GetStatus"}
telemetry(MHD, "readback-start", inbound_http={"path": _MHD + "StartStreamingAndRecording", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[_MHD_STATUS])
telemetry(MHD, "readback-stop", inbound_http={"path": _MHD + "StopRecording", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[_MHD_STATUS])
telemetry(MHD, "readback-failed", inbound_http={"path": _MHD + "StopStreaming", "body": "FAILED"},
          expect_state={}, expect_then_send=[])
telemetry(MHD, "readback-bitrate", inbound_http={"path": _MHD + "SetStreamingVideoDataRate,4000",
                                                  "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHD + "GetStreamingVideoDataRate"}])
telemetry(MHD, "readback-rtsp", inbound_http={"path": _MHD + "SetRTSP,Stream1,554", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHD + "GetRTSP"}, _MHD_STATUS])
telemetry(MHD, "readback-rtmp", inbound_http={"path": _MHD + "SetRTMP,rtmp://a/live,s", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHD + "GetRTMP"}, _MHD_STATUS])
telemetry(MHD, "readback-record-file-name", inbound_http={"path": _MHD + "SetRecordFileName,File1",
                                                           "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHD + "GetRecordFileName"}])
