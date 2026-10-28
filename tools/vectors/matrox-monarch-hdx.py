MHX = "matrox-monarch-hdx"
# Matrox Monarch HDX Control API (Dev Tools Reference Guide v1.1.7, chapter
# 1): GET /Monarch/syncconnect/sdk.aspx?command=<command>[,<arg>...] with an
# HTTP Basic credential; replies are plain text. Replies follow the guide's
# own examples.

_MHX = "/Monarch/syncconnect/sdk.aspx?command="
_MHX_OK = {"status": 200, "body": "SUCCESS"}
_MHX_ACK = {"ok": {"kind": "ack"}}


def _mhx(command, input, query, **extra):
    http(MHX, command, input, "GET", _MHX + query, **extra)


def _mhx_value(value):
    return {"ok": {"kind": "value", "value": value}}


_mhx("get_status", {}, "GetStatus",
     http_reply={"status": 200, "body": "ENC1:RECORD,ON, ENC2:RTSP,READY, NAME:DEVICE2"},
     expect_result=_mhx_value("ENC1:RECORD,ON, ENC2:RTSP,READY, NAME:DEVICE2"))
_mhx("get_status", {}, "GetStatus", file="get_status.retry",
     http_reply={"status": 200, "body": "RETRY"}, expect_result={"error": {"error": "device_error"}})
_mhx("get_status", {}, "GetStatus", file="get_status.refused",
     http_reply={"status": 401, "body": ""}, expect_result={"error": {"error": "auth"}})
_mhx("get_status", {}, "GetStatus", file="get_status.https", settings={"scheme": "https"})
_mhx("start_encoder", {"encoder": 1}, "StartEncoder1", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("start_encoder", {"encoder": 2}, "StartEncoder2", file="start_encoder.failed",
     http_reply={"status": 200, "body": "FAILED"}, expect_result={"error": {"error": "device_error"}})
_mhx("stop_encoder", {"encoder": 2}, "StopEncoder2", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("start_both_encoders", {}, "StartBothEncoders", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("stop_both_encoders", {}, "StopBothEncoders", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("get_input_status", {}, "GetInputStatus",
     http_reply={"status": 200, "body": "SDI A: 1280 x 720p, 59.94 fps (SDI-CEA-608)"},
     expect_result=_mhx_value("SDI A: 1280 x 720p, 59.94 fps (SDI-CEA-608)"))
_mhx("set_streaming_video_data_rate", {"encoder": 2, "kbps": 8500}, "SetStreamingVideoDataRate,2,8500",
     http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("get_streaming_video_data_rate", {"encoder": 1}, "GetStreamingVideoDataRate,1",
     http_reply={"status": 200, "body": "BITRATE: 4000"}, expect_result=_mhx_value("4000"))
_mhx("set_rtsp", {"encoder": 2, "name": "Stream1"}, "SetRTSP,2,Stream1,554", http_reply=_MHX_OK,
     expect_result=_MHX_ACK)
_mhx("set_rtsp", {"encoder": 1, "name": "live/cam=1", "port": "8554"}, "SetRTSP,1,live/cam=1,8554",
     file="set_rtsp.8554")
_mhx("get_rtsp", {"encoder": 2}, "GetRTSP,2",
     http_reply={"status": 200, "body": "rtsp://192.168.1.20:554/Stream1, Stream1, 554"},
     expect_result=_mhx_value("rtsp://192.168.1.20:554/Stream1, Stream1, 554"))
_mhx("set_rtmp", {"encoder": 2, "url": "rtmp://192.168.66.200/live", "stream": "livestream"},
     "SetRTMP,2,rtmp://192.168.66.200/live,livestream", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("set_rtmp_with_credentials",
     {"encoder": 1, "url": "rtmp://monarchStream/live", "stream": "show 1", "username": "user",
      "password": "p@ss+word"},
     "SetRTMP,1,rtmp://monarchStream/live,show%201,user,p%40ss%2Bword")
_mhx("get_rtmp", {"encoder": 2}, "GetRTMP,2",
     http_reply={"status": 200, "body": "rtmp://192.168.66.200/live, livestream"},
     expect_result=_mhx_value("rtmp://192.168.66.200/live, livestream"))
_mhx("set_record_file_name", {"encoder": 1, "filename": "shows/File1"}, "SetRecordFileName,1,shows/File1",
     http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("get_record_file_name", {"encoder": 1}, "GetRecordFileName,1",
     http_reply={"status": 200, "body": "RECORD FILENAME: shows/File1"}, expect_result=_mhx_value("shows/File1"))
_mhx("set_video_input", {"input": "HDMI"}, "SetVideoInput,HDMI", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("get_video_input", {}, "GetVideoInput",
     http_reply={"status": 200, "body": "VIDEO INPUT: SDI"}, expect_result=_mhx_value("SDI"))
_mhx("set_audio_input", {"input": "ANALOG"}, "SetAudioInput,ANALOG", http_reply=_MHX_OK, expect_result=_MHX_ACK)
_mhx("get_audio_input", {}, "GetAudioInput",
     http_reply={"status": 200, "body": "AUDIO INPUT: DIGITAL."}, expect_result=_mhx_value("DIGITAL"))

# Telemetry.
telemetry(MHX, "status", inbound_http={"path": _MHX + "GetStatus",
                                        "body": "ENC1:RECORD,READY, ENC2:RTMP,ON, NAME:DEVICE1"},
          expect_state={"device": {"name": "DEVICE1"}, "encoders": {
              "1": {"mode": "RECORD", "state": "READY", "active": False},
              "2": {"mode": "RTMP", "state": "ON", "active": True}}})
telemetry(MHX, "status-none", inbound_http={"path": _MHX + "GetStatus",
                                             "body": "ENC1:RECORD,ON, ENC2:NONE,DISABLED, NAME:DEVICE3\r\n"},
          expect_state={"device": {"name": "DEVICE3"}, "encoders": {
              "1": {"mode": "RECORD", "state": "ON", "active": True},
              "2": {"mode": "NONE", "state": "DISABLED", "active": False}}})
telemetry(MHX, "input", state_before={"input": {"present": False, "status": "No video input"}},
          inbound_http={"path": _MHX + "GetInputStatus", "body": "SDI A: 1280 x 720p, 59.94 fps (SDI-CEA-608)"},
          expect_state={"input": {"status": "SDI A: 1280 x 720p, 59.94 fps (SDI-CEA-608)", "present": True,
                                  "connector": "SDI A", "width": 1280, "height": 720, "scan": "progressive",
                                  "frame_rate": 59.94, "captions": "SDI-CEA-608"}})
telemetry(MHX, "input-no-captions",
          inbound_http={"path": _MHX + "GetInputStatus", "body": "HDMI: 1920x1080i, 29.97 fps"},
          expect_state={"input": {"status": "HDMI: 1920x1080i, 29.97 fps", "present": True, "connector": "HDMI",
                                  "width": 1920, "height": 1080, "scan": "interlaced", "frame_rate": 29.97,
                                  "captions": ""}})
telemetry(MHX, "input-none",
          state_before={"input": {"status": "HDMI: 1920x1080i, 29.97 fps", "present": True, "connector": "HDMI",
                                  "width": 1920, "height": 1080, "scan": "interlaced", "frame_rate": 29.97,
                                  "captions": ""}},
          inbound_http={"path": _MHX + "GetInputStatus", "body": "No video input"},
          expect_state={"input": {"status": "No video input", "present": False}})
telemetry(MHX, "bitrate", inbound_http={"path": _MHX + "GetStreamingVideoDataRate,2", "body": "BITRATE: 8500"},
          expect_state={"encoders": {"2": {"bitrate_kbps": 8500}}})
telemetry(MHX, "rtsp", inbound_http={"path": _MHX + "GetRTSP,1",
                                      "body": "rtsp://192.168.1.20:8554/Stream1, Stream1, 8554"},
          expect_state={"encoders": {"1": {"rtsp": {"url": "rtsp://192.168.1.20:8554/Stream1", "name": "Stream1",
                                                    "port": 8554}}}})
telemetry(MHX, "rtmp", inbound_http={"path": _MHX + "GetRTMP,2", "body": "rtmp://192.168.66.200/live, livestream"},
          expect_state={"encoders": {"2": {"rtmp": {"url": "rtmp://192.168.66.200/live",
                                                    "stream_name": "livestream"}}}})
telemetry(MHX, "record-file-name", inbound_http={"path": _MHX + "GetRecordFileName,2",
                                                  "body": "RECORD FILENAME: //server/share/File2"},
          expect_state={"encoders": {"2": {"record_file_name": "//server/share/File2"}}})
telemetry(MHX, "video-input", inbound_http={"path": _MHX + "GetVideoInput", "body": "VIDEO INPUT: HDMI"},
          expect_state={"video_input": "HDMI"})
telemetry(MHX, "audio-input", inbound_http={"path": _MHX + "GetAudioInput", "body": "AUDIO INPUT: ANALOG"},
          expect_state={"audio_input": "ANALOG"})
telemetry(MHX, "retry-ignored", state_before={"video_input": "SDI"},
          inbound_http={"path": _MHX + "GetVideoInput", "body": "RETRY"}, expect_state={"video_input": "SDI"})

# Read-backs.
_MHX_STATUS = {"method": "GET", "target": _MHX + "GetStatus"}
telemetry(MHX, "readback-start", inbound_http={"path": _MHX + "StartEncoder2", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[_MHX_STATUS])
telemetry(MHX, "readback-stop-both", inbound_http={"path": _MHX + "StopBothEncoders", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[_MHX_STATUS])
telemetry(MHX, "readback-failed", inbound_http={"path": _MHX + "StartEncoder1", "body": "FAILED"},
          expect_state={}, expect_then_send=[])
telemetry(MHX, "readback-bitrate", inbound_http={"path": _MHX + "SetStreamingVideoDataRate,1,4000",
                                                  "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHX + "GetStreamingVideoDataRate,1"}])
telemetry(MHX, "readback-rtsp", inbound_http={"path": _MHX + "SetRTSP,2,Stream1,554", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHX + "GetRTSP,2"}, _MHX_STATUS])
telemetry(MHX, "readback-rtmp", inbound_http={"path": _MHX + "SetRTMP,1,rtmp://a/live,s", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHX + "GetRTMP,1"}, _MHX_STATUS])
telemetry(MHX, "readback-record-file-name",
          inbound_http={"path": _MHX + "SetRecordFileName,2,File2", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHX + "GetRecordFileName,2"}])
telemetry(MHX, "readback-video-input", inbound_http={"path": _MHX + "SetVideoInput,SDI", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHX + "GetVideoInput"},
                                             {"method": "GET", "target": _MHX + "GetInputStatus"}])
telemetry(MHX, "readback-audio-input", inbound_http={"path": _MHX + "SetAudioInput,DIGITAL", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MHX + "GetAudioInput"}])
