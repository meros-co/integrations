MLC = "matrox-monarch-lcs"
# Matrox Monarch LCS Control API (Dev Tools Reference Guide v1.1.13, chapter
# 1): GET /Monarch/syncconnect/sdk.aspx?command=<command>[,<arg>...] with an
# HTTP Basic credential; replies are plain text. Requests and replies follow
# the guide's own examples.

_MLC = "/Monarch/syncconnect/sdk.aspx?command="
_MLC_OK = {"status": 200, "body": "SUCCESS"}
_MLC_ACK = {"ok": {"kind": "ack"}}


def _mlc(command, input, query, **extra):
    http(MLC, command, input, "GET", _MLC + query, **extra)


def _mlc_value(value):
    return {"ok": {"kind": "value", "value": value}}


_mlc("get_status", {}, "GetStatus",
     http_reply={"status": 200, "body": "ENC1:RECORD,ON,ENC2:RTSP,READY,FILETRANSFER:OFF,NAME:DEVICE2"},
     expect_result=_mlc_value("ENC1:RECORD,ON,ENC2:RTSP,READY,FILETRANSFER:OFF,NAME:DEVICE2"))
_mlc("get_status", {}, "GetStatus", file="get_status.retry",
     http_reply={"status": 200, "body": "RETRY"}, expect_result={"error": {"error": "device_error"}})
_mlc("get_status", {}, "GetStatus", file="get_status.refused",
     http_reply={"status": 401, "body": ""}, expect_result={"error": {"error": "auth"}})
_mlc("get_status", {}, "GetStatus", file="get_status.https", settings={"scheme": "https"})
_mlc("get_input_status", {}, "GetInputStatus",
     http_reply={"status": 200, "body": "SDI A: 1920x1080i, 29.97 fps;HDMI B: 1920x1080p, 60 fps"},
     expect_result=_mlc_value("SDI A: 1920x1080i, 29.97 fps;HDMI B: 1920x1080p, 60 fps"))
_mlc("start_both_encoders", {}, "StartBothEncoders", http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("stop_both_encoders", {}, "StopBothEncoders", http_reply={"status": 200, "body": "FAILED"},
     expect_result={"error": {"error": "device_error"}})
_mlc("set_streaming_video_data_rate", {"encoder": 1, "kbps": 3000}, "SetStreamingVideoDataRate,1,3000",
     http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("get_streaming_video_data_rate", {"encoder": 2}, "GetStreamingVideoDataRate,2",
     http_reply={"status": 200, "body": "BITRATE: 6000"}, expect_result=_mlc_value("6000"))
_mlc("set_rtsp", {"encoder": 1, "name": "Stream1", "port": "8554"}, "SetRTSP,1,Stream1,8554",
     http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("get_rtsp", {"encoder": 1}, "GetRTSP,1",
     http_reply={"status": 200, "body": "rtsp://192.168.1.30:8554/Stream1, Stream1, 8554"},
     expect_result=_mlc_value("rtsp://192.168.1.30:8554/Stream1, Stream1, 8554"))
_mlc("set_rtmp", {"encoder": 2, "url": "rtmp://192.168.66.200/live", "stream": "lecture"},
     "SetRTMP,2,rtmp://192.168.66.200/live,lecture", http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("set_rtmp_with_credentials",
     {"encoder": 2, "url": "rtmp://192.168.66.200/live", "stream": "lecture", "username": "u", "password": "p w"},
     "SetRTMP,2,rtmp://192.168.66.200/live,lecture,u,p%20w")
_mlc("get_rtmp", {"encoder": 1}, "GetRTMP,1",
     http_reply={"status": 200, "body": "rtmp://192.168.66.200/live, lecture"},
     expect_result=_mlc_value("rtmp://192.168.66.200/live, lecture"))
_mlc("set_both_rtmp_paths",
     {"url1": "rtmp://a.example/live", "stream1": "cam", "url2": "rtmps://b.example/app", "stream2": "slides"},
     "SetBothRtmpPaths,rtmp://a.example/live,cam,rtmps://b.example/app,slides", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
_mlc("set_both_rtmp_paths_with_credentials",
     {"url1": "rtmp://url1", "stream1": "streamname1", "username1": "username1", "password1": "password1",
      "url2": "rtmp://url2", "stream2": "streamname2", "username2": "username2", "password2": "password2"},
     "SetBothRtmpPaths,rtmp://url1,streamname1,username1,password1,rtmp://url2,streamname2,username2,password2")
_mlc("set_record_file_name", {"encoder": 2, "filename": "Lecture-1"}, "SetRecordFileName,2,Lecture-1",
     http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("get_record_file_name", {"encoder": 2}, "GetRecordFileName,2",
     http_reply={"status": 200, "body": "RECORD FILENAME: Lecture-1"}, expect_result=_mlc_value("Lecture-1"))
_mlc("set_video_input", {"input": "INPUT_B_HDMI"}, "SetVideoInput,INPUT_B_HDMI", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
_mlc("get_video_input", {}, "GetVideoInput",
     http_reply={"status": 200, "body": "VIDEO INPUT: INPUT_A_SDI"}, expect_result=_mlc_value("INPUT_A_SDI"))
_mlc("set_audio_input", {"input": "INPUT_ANALOG"}, "SetAudioInput,INPUT_ANALOG", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
_mlc("get_audio_input", {}, "GetAudioInput",
     http_reply={"status": 200, "body": "AUDIO INPUT: INPUT_A_HDMI"}, expect_result=_mlc_value("INPUT_A_HDMI"))
_mlc("set_transfer_location", {"source": "USB1", "destination": "123.456.78.90"},
     "SetTransferLocation,USB1,123.456.78.90", http_reply=_MLC_OK, expect_result=_MLC_ACK)
# The guide's example.
_mlc("set_transfer_location_with_credentials",
     {"source": "USB1", "destination": "123.456.78.90", "username": "MYUSERNAME", "password": "MYPASSWORD"},
     "SetTransferLocation,USB1,123.456.78.90,WORKGROUP,MYUSERNAME,MYPASSWORD", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
_mlc("set_transfer_location_with_credentials",
     {"source": "SDCARD", "destination": "\\\\server\\share", "domain": "OFFICE", "username": "rec",
      "password": "s3cret!"},
     "SetTransferLocation,SDCARD,%5C%5Cserver%5Cshare,OFFICE,rec,s3cret%21", file="set_transfer_location_with_credentials.unc")
_mlc("start_transfer", {}, "StartTransfer", http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("start_transfer_all", {}, "StartTransferAll", http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("stop_transfer", {}, "StopTransfer", http_reply=_MLC_OK, expect_result=_MLC_ACK)
_mlc("set_operating_mode",
     {"mode": "DUALISOLATED", "video_input": "INPUT_A_HDMI", "audio_input": "INPUT_A", "sdi_output": "INPUT_A",
      "hdmi_output": "COMPOSITION"},
     "SetOperatingMode,DUALISOLATED,INPUT_A_HDMI,INPUT_A,INPUT_A,COMPOSITION", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
# The guide's second example.
_mlc("set_operating_mode_with_composition",
     {"mode": "SIDEBYSIDE", "video_input": "INPUT_A_SDI", "audio_input": "INPUT_A", "sdi_output": "INPUT_A",
      "hdmi_output": "COMPOSITION", "composition": "SBS_BA"},
     "SetOperatingMode,SIDEBYSIDE,INPUT_A_SDI,INPUT_A,INPUT_A,COMPOSITION,SBS_BA,FALSE", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
_mlc("set_operating_mode_with_composition",
     {"mode": "PICTUREINPICTURE", "video_input": "INPUT_A_HDMI", "audio_input": "INPUT_ANALOG",
      "sdi_output": "INPUT_B", "hdmi_output": "COMPOSITION", "composition": "PIP_BR", "crop": True},
     "SetOperatingMode,PICTUREINPICTURE,INPUT_A_HDMI,INPUT_ANALOG,INPUT_B,COMPOSITION,PIP_BR,TRUE",
     file="set_operating_mode_with_composition.pip")
_mlc("set_dynamic_output", {"output": "SHOW_B"}, "SetDynamicOutput,SHOW_B", http_reply=_MLC_OK,
     expect_result=_MLC_ACK)
_mlc("reboot", {}, "AskForReboot", http_reply=_MLC_OK, expect_result=_MLC_ACK)

# Telemetry.
telemetry(MLC, "status", inbound_http={"path": _MLC + "GetStatus",
                                        "body": "ENC1:RECORD,READY,ENC2:RTMP,READY,FILETRANSFER:ON,NAME:DEVICE1"},
          expect_state={"device": {"name": "DEVICE1"}, "file_transfer": {"state": "ON", "on": True},
                        "encoders": {"1": {"mode": "RECORD", "state": "READY", "active": False},
                                     "2": {"mode": "RTMP", "state": "READY", "active": False}}})
telemetry(MLC, "status-spaced", inbound_http={"path": _MLC + "GetStatus",
                                               "body": "ENC1:RECORD,ON, ENC2:NONE,DISABLED,FILETRANSFER:OFF,NAME:DEVICE3\n"},
          expect_state={"device": {"name": "DEVICE3"}, "file_transfer": {"state": "OFF", "on": False},
                        "encoders": {"1": {"mode": "RECORD", "state": "ON", "active": True},
                                     "2": {"mode": "NONE", "state": "DISABLED", "active": False}}})
telemetry(MLC, "inputs", inbound_http={"path": _MLC + "GetInputStatus",
                                        "body": "SDI A: 1920x1080i, 29.97 fps;HDMI B: 1920x1080p, 60 fps"},
          expect_state={"input": {"status": "SDI A: 1920x1080i, 29.97 fps;HDMI B: 1920x1080p, 60 fps"},
                        "inputs": {"a": {"connector": "SDI", "width": 1920, "height": 1080, "scan": "interlaced",
                                         "frame_rate": 29.97},
                                   "b": {"connector": "HDMI", "width": 1920, "height": 1080, "scan": "progressive",
                                         "frame_rate": 60.0}}})
telemetry(MLC, "inputs-none",
          state_before={"input": {"status": "HDMI A: 1280x720p, 50 fps"},
                        "inputs": {"a": {"connector": "HDMI", "width": 1280, "height": 720, "scan": "progressive",
                                         "frame_rate": 50.0}}},
          inbound_http={"path": _MLC + "GetInputStatus", "body": "No video input"},
          expect_state={"input": {"status": "No video input"}})
telemetry(MLC, "bitrate", inbound_http={"path": _MLC + "GetStreamingVideoDataRate,1", "body": "BITRATE: 3000"},
          expect_state={"encoders": {"1": {"bitrate_kbps": 3000}}})
telemetry(MLC, "rtsp", inbound_http={"path": _MLC + "GetRTSP,2",
                                      "body": "rtsp://192.168.1.30:554/Stream2, Stream2, 554"},
          expect_state={"encoders": {"2": {"rtsp": {"url": "rtsp://192.168.1.30:554/Stream2", "name": "Stream2",
                                                    "port": 554}}}})
telemetry(MLC, "rtmp", inbound_http={"path": _MLC + "GetRTMP,1", "body": "rtmp://192.168.66.200/live, lecture"},
          expect_state={"encoders": {"1": {"rtmp": {"url": "rtmp://192.168.66.200/live", "stream_name": "lecture"}}}})
telemetry(MLC, "record-file-name", inbound_http={"path": _MLC + "GetRecordFileName,1",
                                                  "body": "RECORD FILENAME: Lecture-1"},
          expect_state={"encoders": {"1": {"record_file_name": "Lecture-1"}}})
telemetry(MLC, "video-input", inbound_http={"path": _MLC + "GetVideoInput",
                                             "body": "VIDEO INPUT: INPUT_A_HDMI and INPUT_B_HDMI"},
          expect_state={"video_input": "INPUT_A_HDMI and INPUT_B_HDMI"})
telemetry(MLC, "audio-input", inbound_http={"path": _MLC + "GetAudioInput", "body": "AUDIO INPUT: INPUT_ANALOG"},
          expect_state={"audio_input": "INPUT_ANALOG"})

# Read-backs.
_MLC_STATUS = {"method": "GET", "target": _MLC + "GetStatus"}
telemetry(MLC, "readback-start", inbound_http={"path": _MLC + "StartBothEncoders", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[_MLC_STATUS])
telemetry(MLC, "readback-transfer", inbound_http={"path": _MLC + "StartTransferAll", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[_MLC_STATUS])
telemetry(MLC, "readback-retry", inbound_http={"path": _MLC + "StopTransfer", "body": "RETRY"},
          expect_state={}, expect_then_send=[])
telemetry(MLC, "readback-bitrate", inbound_http={"path": _MLC + "SetStreamingVideoDataRate,2,6000",
                                                  "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetStreamingVideoDataRate,2"}])
telemetry(MLC, "readback-rtsp", inbound_http={"path": _MLC + "SetRTSP,1,Stream1,554", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetRTSP,1"}, _MLC_STATUS])
telemetry(MLC, "readback-rtmp", inbound_http={"path": _MLC + "SetRTMP,2,rtmp://a/live,s", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetRTMP,2"}, _MLC_STATUS])
telemetry(MLC, "readback-both-rtmp", inbound_http={"path": _MLC + "SetBothRtmpPaths,rtmp://a,s,rtmp://b,t",
                                                    "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetRTMP,1"},
                                             {"method": "GET", "target": _MLC + "GetRTMP,2"}])
telemetry(MLC, "readback-record-file-name",
          inbound_http={"path": _MLC + "SetRecordFileName,1,Lecture-2", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetRecordFileName,1"}])
telemetry(MLC, "readback-operating-mode",
          inbound_http={"path": _MLC + "SetOperatingMode,SINGLEISOLATED,INPUT_B_HDMI,INPUT_B,INPUT_B,INPUT_B",
                        "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetVideoInput"},
                                             {"method": "GET", "target": _MLC + "GetAudioInput"},
                                             {"method": "GET", "target": _MLC + "GetInputStatus"}, _MLC_STATUS])
telemetry(MLC, "readback-audio-input", inbound_http={"path": _MLC + "SetAudioInput,INPUT_B", "body": "SUCCESS"},
          expect_state={}, expect_then_send=[{"method": "GET", "target": _MLC + "GetAudioInput"}])
