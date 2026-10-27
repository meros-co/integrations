# JVC (jvc-camera): one vector per command, written by hand from the JVC
# Camcorder Web API Reference 1.18: each command is POSTed to
# /cgi-bin/api.cgi as {"Request":{"Command":...,"SessionID":...,"Params":{...}}}
# with no spaces, the SessionID the login's cookie gave (the vector runner's
# login answers SessionID=vector). Parameters follow each command's request
# table; commands that read back state the change, which goes first.
JV = "jvc-camera"
JV_SET = {"username": "jvc", "password": "pw"}
FORM = "application/x-www-form-urlencoded; charset=UTF-8"


def _jv(command, input, api, params=None, model=None, file=None, **extra):
    body = '{"Request":{"Command":"' + api + '","SessionID":"vector"'
    if params is not None:
        body += ',"Params":{' + params + "}"
    body += "}}"
    v = {"spec": JV, "command": command, "input": input, "settings": dict(JV_SET),
         "expect_request": {"method": "POST", "target": "/cgi-bin/api.cgi", "content_type": FORM, "body": body}}
    if model:
        v["model"] = model
    if file:
        v["file"] = file
    v.update(extra)
    V.append(v)


PZ = "ky-pz400"
OKJ = '{"Response":{"Requested":"SetCamCtrl","Result":"Success"}}'

_jv("get_streaming_settings", {}, "GetStreamingSettings")
_jv("get_streaming_settings", {"stream": 1}, "GetStreamingSettings", '"Stream":1', model=PZ, file="get_streaming_settings-pz400")
_jv("set_streaming_resolution", {"resolution": "1920x1080"}, "SetStreamingResolution", '"Resolution":"1920x1080"')
_jv("set_streaming_resolution", {"stream": 0, "resolution": "1920x1080"}, "SetStreamingResolution",
    '"Stream":0,"Resolution":"1920x1080"', model=PZ, file="set_streaming_resolution-pz400")
_jv("set_streaming_frame_rate", {"frame_rate": "30p"}, "SetStreamingFramerate", '"Framerate":"30p"', model="gy-hm660")
_jv("set_streaming_frame_rate", {"stream": 0, "frame_rate": "30"}, "SetStreamingFramerate", '"Stream":0,"Framerate":"30"',
    model=PZ, file="set_streaming_frame_rate-pz400")
_jv("set_streaming_bitrate", {"bitrate": "3.0M"}, "SetStreamingBitrate", '"Bitrate":"3.0M"')
_jv("set_streaming_bitrate_and_quality", {"stream": 0, "kbps": 4096, "quality": 80, "control": "CBR"}, "SetStreamingBitrate",
    '"Stream":0,"Bitrate":"4096K","Qfactor":"80","BitRateControl":"CBR"', model=PZ)
_jv("get_available_resolutions", {}, "AvailableStreamingResolutionSettings")
_jv("get_available_frame_rates", {"resolution": "1920x1080"}, "AvailableStreamingFramerateSettings",
    '"Resolution":"1920x1080"', model="gy-hm660")
_jv("get_available_bitrates", {"type": "TCP", "resolution": "1920x1080", "frame_rate": "60p"},
    "AvailableStreamingBitrateSettings", '"Type":"TCP","Resolution":"1920x1080","Framerate":"60p"')
_jv("get_available_bitrates", {"stream": 0, "type": "RTSP", "resolution": "1920x1080", "frame_rate": "30"},
    "AvailableStreamingBitrateSettings", '"Stream":0,"Type":"RTSP","Resolution":"1920x1080","Framerate":"30"',
    model=PZ, file="get_available_bitrates-pz400")
_jv("get_streaming_server", {}, "GetCurrentStreamingServerID")
_jv("set_streaming_server", {"server": 4}, "SetCurrentStreamingServerID", '"ID":3')
_jv("get_streaming_server_settings", {"server": 2}, "GetStreamingServerSettings", '"ID":1')
_jv("get_streaming_server_settings_of_type", {"type": "RTSP"}, "GetStreamingServerSettings", '"Type":"RTSP"', model=PZ)
_jv("set_server_udp", {"server": 1, "alias": "Truck", "address": "192.168.0.1", "port": 6504, "pcr_mode": "standard",
                       "pcr_jitter": "low"}, "SetStreamingServerSettingsUDP",
    '"ID":0,"Alias":"Truck","DstAddress":"192.168.0.1","DstPort":6504,"PcrMode":0,"PcrJitter":1')
_jv("set_server_tcp", {"server": 1, "alias": "Server1", "address": "192.168.0.1", "port": 6504},
    "SetStreamingServerSettingsTCP", '"ID":0,"Alias":"Server1","DstAddress":"192.168.0.1","DstPort":6504')
_jv("set_server_rtsp", {"server": 2, "alias": "Web", "username": "user", "password": "abcde", "port": 554, "auth": True},
    "SetStreamingServerSettingsRTSP", '"ID":1,"Alias":"Web","Username":"user","Password":"abcde","DstPort":554,"Auth":1')
_jv("set_server_zixi", {"server": 1, "alias": "Z", "address": "10.0.0.2", "port": 2088, "stream_id": "cam1",
                        "password": "zz", "latency": "medium", "pcr_mode": "fast", "adaptive": False},
    "SetStreamingServerSettingsZIXI",
    '"ID":0,"Alias":"Z","DstAddress":"10.0.0.2","DstPort":2088,"StreamId":"cam1","Password":"zz","Latency":1,"PcrMode":1,'
    '"AdaptiveBitrate":0')
_jv("set_server_rtmp", {"server": 3, "alias": "Live", "stream": 0, "url": "rtmp://a.example/live", "key": "k1",
                        "enabled": True, "video": True, "audio": True}, "SetStreamingServerSettingsRTMP",
    '"ID":2,"Alias":"Live","Stream":0,"DstUrl":"rtmp://a.example/live","StreamKey":"k1","Status":1,"Video":1,"Audio":1')
_jv("set_server_rtmps", {"server": 3, "alias": "Live", "stream": 0, "url": "rtmps://a.example/live", "key": "k1",
                         "enabled": True, "video": True, "audio": False}, "SetStreamingServerSettingsRTMPS",
    '"ID":2,"Alias":"Live","Stream":0,"DstUrl":"rtmps://a.example/live","StreamKey":"k1","Status":1,"Video":1,"Audio":0',
    model="gy-hm25x")
_jv("set_server_rtp", {"server": 1, "alias": "R", "address": "10.0.0.3", "port": 5004, "pcr_mode": "standard", "fec": True,
                       "fec_l": 10, "fec_d": 10}, "SetStreamingServerSettingsRTP",
    '"ID":0,"Alias":"R","DstAddress":"10.0.0.3","DstPort":5004,"PcrMode":0,"Smpte2022Fec":1,"FecMatrixL":10,"FecMatrixD":10',
    model="gy-hm660")
_jv("set_server_srt", {"server": 1, "alias": "S", "address": "10.0.0.4", "port": 9000, "stream_id": "s1", "mode": "caller",
                       "overhead": 25, "latency": 120, "encryption": "aes128", "fec": "off", "adaptive": False,
                       "pcr_mode": "standard", "passphrase": "0123456789", "enabled": True},
    "SetStreamingServerSettingsSRT",
    '"ID":0,"Alias":"S","DstAddress":"10.0.0.4","DstPort":9000,"StreamId":"s1","ConnectionMode":0,"BandwidthOverhead":25,'
    '"Latency":120,"Encryption":1,"FEC":0,"AdaptiveBitrate":0,"PcrMode":0,"Passphrase":"0123456789","Status":1',
    model="gy-hc900")
_jv("set_encode_protocol", {"stream": 0, "protocol": "H265"}, "SetStreamingEncodeProtocol", '"Stream":0,"EncodeProtocol":"H265"', model=PZ)
_jv("set_key_frame_interval", {"stream": 1, "interval": 30}, "SetStreamingIKeyFrameInterval", '"Stream":1,"Interval":30', model=PZ)
_jv("get_video_settings", {}, "GetVideoSettings", model=PZ)
_jv("set_video_output", {"output": "HDMI"}, "SetVideoHdmiSdiOutput", '"Output":"HDMI"', model=PZ)
_jv("set_video_format", {"format": "DialPriority"}, "SetVideoFormat", '"VideoFormat":"DialPriority"', model=PZ)
_jv("set_video_encode_level", {"level": "highprofile"}, "SetVideoEncodeLevel", '"EncodeLevel":"highprofile"', model=PZ)
_jv("set_multicast", {"enabled": True, "address": "239.0.0.1", "port": 5000}, "SetNetworkMulticastSettings",
    '"Status":1,"DstAddress":"239.0.0.1","DstPort":5000', model=PZ)
_jv("set_ndi", {"device_name": "PZ400", "group": "public"}, "SetNDISettings", '"DeviceName":"PZ400","GroupName":"public"', model=PZ)
_jv("get_system_info", {}, "GetSystemInfo")
_jv("get_zoom_presets", {}, "GetPresetZoomPosition")
_jv("set_zoom_preset", {"preset": "A", "position": 300}, "SetPresetZoomPosition", '"ID":"A","Position":300')
_jv("clear_zoom_preset", {"preset": "C"}, "SetPresetZoomPosition", '"ID":"C","Position":-1')
_jv("get_ntp_status", {}, "GetNTPStatus", model="gy-hm660")
_jv("set_ntp_server", {"address": "192.168.0.100"}, "SetNTPServer", '"Address":"192.168.0.100"', model="gy-hm660")
_jv("set_ntp_timecode_sync", {"enabled": True}, "SetNTPSettings", '"TcSync":"On"', model="gy-hm660")
_jv("set_ntp_settings", {"enabled": True, "time_zone": "GMT+0900", "server": "192.168.123.123", "interval": 1440,
                         "main_show": True, "main_x": 0, "main_y": 0, "sub_show": False, "sub_x": 0, "sub_y": 0},
    "SetNTPSettings",
    '"TimeSync":1,"TimeZone":"GMT+0900","ServerAddress":"192.168.123.123","TimeInterval":1440,'
    '"MainTimeShow":{"Status":1,"XPosition":0,"YPosition":0},"SubTimeShow":{"Status":0,"XPosition":0,"YPosition":0}',
    model=PZ)
_jv("get_ntp_settings", {}, "GetNTPSettings", model=PZ)
_jv("reboot", {}, "SystemRequest", '"Request":"Reboot"', model=PZ)
_jv("get_camera_status", {}, "GetCamStatus")
_jv("start_recording", {}, "SetCamCtrl", '"CamCtrl":"Rec"',
    http_reply={"status": 200, "body": OKJ}, expect_result={"ok": {"kind": "ack"}})
_jv("stop_recording", {}, "SetCamCtrl", '"CamCtrl":"Stop"')
_jv("stop_recording", {}, "SetCamCtrl", '"CamCtrl":"Stop"', file="stop_recording-busy",
    http_reply={"status": 200, "body": '{"Response":{"Requested":"SetCamCtrl","Result":"DualExeError"}}'},
    expect_result={"error": {"error": "device_error"}})
_jv("set_zoom_position", {"position": 400}, "SetZoomCtrl", '"Position":400')
_jv("start_streaming", {}, "SetStreamingCtrl", '"Streaming":"On"')
_jv("stop_streaming", {}, "SetStreamingCtrl", '"Streaming":"Off"')
_jv("set_tally_lamp", {"lit": True}, "SetTallyLampCtrl", '"Sw":"On"')
for _cmd, _kind, _button, _model in (
        ("full_auto", "Fullauto", "On", None), ("exposure", "Exposure", "IrisPriority", "ky-pz100"),
        ("iris", "Iris", "Open1", None), ("gain", "Gain", "Lolux", None), ("ae_level", "AeLevel", "AeLevelUp", None),
        ("shutter", "Shutter", "Slower", None), ("white_balance", "Whb", "Faw", None), ("zoom", "Zoom", "Tele2", None),
        ("focus", "Focus", "PushAuto", None), ("user_switch", "User", "Sw1", None), ("display", "Disptv", "On", None),
        ("menu", "Menu", "Menu", None), ("record", "Rec", "Start", None), ("master_black", "MasterBlack", "Up1", "gy-hm660"),
        ("detail", "Detail", "Up", "gy-hm660"), ("speed_with_zoom", "SpeedWithZoom", "On", "ky-pz100"),
        ("bright", "Bright", "Up", PZ), ("luminance", "Luminance", "Down", PZ), ("saturation", "Saturation", "Up", PZ),
        ("contrast", "Contrast", "Up", PZ), ("sharpness", "Sharpness", "Down", PZ), ("hue", "Hue", "Up", PZ),
        ("flip", "Flip", "On", PZ), ("mirror", "Mirror", "Off", PZ)):
    _jv(f"{_cmd}_button", {"button": _button}, "SetWebButtonEvent", f'"Kind":"{_kind}","Button":"{_button}"', model=_model)
_jv("set_zoom_slider", {"position": 73}, "SetWebSliderEvent", '"Kind":"ZoomBar","Position":73')
_jv("set_iris_slider", {"position": 128}, "SetWebSliderEvent", '"Kind":"IrisBar","Position":128')
_jv("set_white_paint", {"blue": 32, "red": 32}, "SetWebXYFieldEvent", '"Kind":"WhPaintRB","XPosition":32,"YPosition":32')
_jv("get_gps", {}, "GetGPSInfo")
_jv("zoom_seesaw", {"direction": "Wide", "speed": 5}, "SeesawSwitchOperation", '"Kind":"ZoomSeesaw","Direction":"Wide","Speed":5',
    model="gy-hm660")
_jv("iris_seesaw", {"direction": "Open", "speed": 3}, "SeesawSwitchOperation", '"Kind":"IrisSeesaw","Direction":"Open","Speed":3',
    model="gy-hm660")
_jv("master_black_seesaw", {"direction": "Up", "speed": 5}, "SeesawSwitchOperation",
    '"Kind":"MasterBlackSeesaw","Direction":"Up","Speed":5', model="gy-hm660")
_jv("focus_seesaw", {"direction": "Stop", "speed": 0}, "SeesawSwitchOperation", '"Kind":"FocusSeesaw","Direction":"Stop","Speed":0',
    model="gy-hm660")
_jv("pan_tilt", {"pan": "Left", "pan_speed": 10, "tilt": "Up", "tilt_speed": 10}, "SetPTCtrl",
    '"PanDirection":"Left","PanPosition":17540,"PanSpeed":10,"TiltDirection":"Up","TiltPosition":9040,"TiltSpeed":10')
_jv("pan_tilt", {"pan": "Position", "pan_position": 35080, "pan_speed": 30, "tilt": "Position", "tilt_position": 0,
                 "tilt_speed": 30}, "SetPTCtrl",
    '"PanDirection":"Position","PanPosition":35080,"PanSpeed":30,"TiltDirection":"Position","TiltPosition":0,"TiltSpeed":30',
    file="pan_tilt-position")
_jv("joystick", {"pan": "Left", "pan_speed": 10, "tilt": "Up", "tilt_speed": 10}, "JoyStickOperation",
    '"PanDirection":"Left","PanSpeed":10,"TiltDirection":"Up","TiltSpeed":10')
_jv("zoom_switch", {"direction": "Wide", "speed": 5}, "ZoomSwitchOperation", '"Direction":"Wide","Speed":5')
_jv("recall_preset", {"preset": 90}, "SetPTZPreset", '"No":90,"Operation":"Move"')
_jv("store_preset", {"preset": 5}, "SetPTZPreset", '"No":5,"Operation":"Set"')
_jv("delete_preset", {"preset": 5}, "SetPTZPreset", '"No":5,"Operation":"Delete"')
_jv("get_camera_status_minimum", {}, "GetCamStatusMinimum")
_jv("get_pan_tilt_position", {}, "GetPTPosition")
_jv("start_jpeg_encoding", {}, "JpegEncode", '"Operate":"Start"')
_jv("stop_jpeg_encoding", {}, "JpegEncode", '"Operate":"Stop"')
_jv("set_jpeg_size", {"size": "640x360"}, "SetJpegEncodeSize", '"Size":"640x360"')
_jv("get_ifb_settings", {}, "GetIFBSettings", model="gy-hm660")
_jv("set_ifb_settings", {"enabled": True, "address": "192.168.0.10", "port": 8000, "mountpoint": "ifb"}, "SetIFBSettings",
    '"Function":"Enable","Address":"192.168.0.10","Port":8000,"Mountpoint":"ifb"', model="gy-hm660")
_jv("start_ifb", {}, "SetIFBCtrl", '"Streaming":"Start"', model="gy-hm660")
_jv("stop_ifb", {}, "SetIFBCtrl", '"Streaming":"Stop"', model="gy-hm660")
_jv("set_studio_tally", {"indication": "Program"}, "SetStudioTally", '"Indication":"Program"', model="gy-hm660")
_jv("set_tally_priority", {"priority": "Web"}, "SetTallyLampPriority", '"Priority":"Web"')
_jv("get_return_server", {}, "GetCurrentReturnOverIpServerID", model="gy-hc900")
_jv("set_return_server", {"server": 2}, "SetCurrentReturnOverIpServerID", '"ID":1', model="gy-hc900")
_jv("get_return_server_settings", {"server": 1}, "GetReturnOverIpServerSettings", '"ID":0', model="gy-hc900")
_jv("get_return_types", {}, "AvailableTypeOfReturnOverIP", model="gy-hc900")
_jv("get_return_frame_rates", {"recording": "60p"}, "AvailableFrameRateOfReturnOverIP", '"RecordingFrameRate":"60p"',
    model="gy-hc900")
_jv("set_return_rtsp", {"server": 1, "alias": "Ret", "protocol": "UDP", "address": "10.0.0.5", "port": 554,
                        "stream_id": "ret", "username": "u", "password": "p", "frame_rate": "30p"},
    "SetReturnOverIpServerSettingsRTSP",
    '"ID":0,"Alias":"Ret","Protocol":"UDP","SrcAddress":"10.0.0.5","SrcPort":554,"StreamId":"ret","Username":"u",'
    '"Password":"p","FrameRate":2', model="gy-hc900")
_jv("set_return_zixi", {"server": 1, "alias": "Ret", "address": "10.0.0.5", "port": 2077, "stream_id": "ret",
                        "password": "p", "name": "n", "latency": "low", "frame_rate": "60p"},
    "SetReturnOverIpServerSettingsZIXI",
    '"ID":0,"Alias":"Ret","SrcAddress":"10.0.0.5","SrcPort":2077,"StreamId":"ret","Password":"p","Name":"n","Latency":0,'
    '"FrameRate":1', model="gy-hc900")
_jv("set_return_icecast", {"server": 1, "alias": "Ret", "address": "10.0.0.5", "port": 8000, "mountpoint": "ret"},
    "SetReturnOverIpServerSettingsIcecast", '"ID":0,"Alias":"Ret","SrcAddress":"10.0.0.5","SrcPort":8000,"Mountpoint":"ret"',
    model="gy-hc900")
_jv("set_return_srt", {"server": 1, "alias": "Ret", "address": "10.0.0.5", "port": 9000, "mode": "listener", "latency": 200,
                       "encryption": False, "passphrase": "", "frame_rate": "25p"},
    "SetReturnOverIpServerSettingsSRT",
    '"ID":0,"Alias":"Ret","SrcAddress":"10.0.0.5","Port":9000,"ConnectionMode":1,"Latency":200,"Encryption":0,'
    '"Passphrase":"","FrameRate":5', model="gy-hc900")
_jv("start_return", {}, "SetReturnOverIpCtrl", '"Operate":"Start"', model="gy-hc900")
_jv("stop_return", {}, "SetReturnOverIpCtrl", '"Operate":"Stop"', model="gy-hc900")


# Telemetry: the document's example answers (compacted, as the camera sends them).
def _jvt(name, response, state, model=None, request=None, **extra):
    fields = {"settings": dict(JV_SET), "inbound_http": {"path": "/cgi-bin/api.cgi", "body": json.dumps(response, separators=(",", ":"))},
              "expect_state": state}
    if model:
        fields["model"] = model
    if request:
        fields["inbound_http"]["request"] = request
    fields.update(extra)
    telemetry(JV, name, **fields)


_jvt("cam-status", {"Response": {"Requested": "GetCamStatus", "Result": "Success", "Data": {
    "Camera": {"Status": "Standby", "Mode": "Camera", "RecMode": "Normal", "TC": "12", "AspectRatio": "16:9", "WebAccess": "On"},
    "Iris": {"Status": "Auto", "Value": "F2.8", "Position": 120},
    "Gain": {"Status": "Alc", "Value": "6dB"},
    "Whb": {"Status": "Faw", "Value": "5600K", "WhPRScale": 64, "WhPBScale": 64, "WhPRPosition": 32, "WhPBPosition": 30,
            "WhPRValue": "0", "WhPBValue": "-2"},
    "Zoom": {"Dynamic": "Off", "DynamicPos": 0, "Position": 100, "DisplayValue": "Z100"},
    "Focus": {"Status": "AF", "Value": "3.5m"},
    "Streaming": {"Status": "Stop"},
    "TallyLamp": {"Priority": "Camera", "Lighting": "Off", "StudioTally": "Off"},
    "SlotA": {"Status": "Select", "Protect": "Unlock", "Remain": "120", "ClipNum": 50, "RemainWarning": 0},
    "Battery": {"Info": "Time", "Level": "8", "Value": "80"},
    "Enable": {"Iris": {"Open1": 1, "Close1": 0}},
    "ButtonString": {"User": {"Sw1": "Focus Assist"}}}}},
    {"camera": {"status": "Standby", "mode": "Camera", "rec_mode": "Normal", "timecode": "12", "aspect_ratio": "16:9",
                "web_access": "On"},
     "iris": {"mode": "Auto", "value": "F2.8", "position": 120}, "gain": {"mode": "Alc", "value": "6dB"},
     "white_balance": {"mode": "Faw", "value": "5600K", "paint_red": {"scale": 64, "position": 32, "value": "0"},
                       "paint_blue": {"scale": 64, "position": 30, "value": "-2"}},
     "zoom": {"dynamic": "Off", "dynamic_position": 0, "position": 100, "display": "Z100"},
     "focus": {"mode": "AF", "value": "3.5m"}, "streaming": {"status": "Stop"},
     "tally": {"priority": "Camera", "lighting": "Off", "studio": "Off"},
     "slots": {"a": {"status": "Select", "protect": "Unlock", "remaining_minutes": 120, "clips": 50, "low": False}},
     "battery": {"info": "Time", "level": "8", "value": "80"},
     "buttons": {"Iris": {"Open1": True, "Close1": False}}, "button_names": {"User": {"Sw1": "Focus Assist"}}})
_jvt("pt-position", {"Response": {"Requested": "GetPTPosition", "Result": "Success", "Data": {"Pan": 17540, "Tilt": 9040}}},
     {"position": {"pan": 17540, "tilt": 9040}}, model="ky-pz100")
_jvt("system-info", {"Response": {"Requested": "GetSystemInfo ", "Result": "Success", "Data": {
    "Model": "PZ400", "Destination": "JP", "ApiVersion": "1.14.001", "Serial": "123A1234", "Srt": "On"}}},
    {"system": {"model": "PZ400", "destination": "JP", "api_version": "1.14.001", "serial": "123A1234", "srt": "On"}})
_jvt("streaming-settings", {"Response": {"Requested": "GetStreamingSettings", "Result": "Success", "Data": {
    "Resolution": "1920x1080", "Framerate": "60i", "Bitrate": "3.0M"}}},
    {"streams": {"1": {"resolution": "1920x1080", "frame_rate": "60i", "bitrate": "3.0M"}}})
_jvt("streaming-settings-pz", {"Response": {"Requested": "GetStreamingSettings", "Result": "Success", "Data": {
    "Stream": 0, "Resolution": "1920x1080", "Framerate": "60i", "Bitrate": "4096K", "EncodeProtocol": "H264",
    "IKeyFrameInterval": 30, "BitRateControl": "CBR"}}},
    {"streams": {"1": {"resolution": "1920x1080", "frame_rate": "60i", "bitrate": "4096K", "encoding": "H264",
                       "key_frame_interval": 30, "rate_control": "CBR"}}}, model=PZ)
_jvt("server-id", {"Response": {"Requested": "GetCurrentStreamingServerID", "Result": "Success", "Data": {"ID": 3}}},
     {"streaming": {"server": 4}})
_jvt("server-settings", {"Response": {"Requested": "GetStreamingServerSettings", "Result": "Success", "Data": {
    "Alias": "Server1", "Type": "TCP", "Tcp": {"DstAddress": "192.168.0.1", "DstPort": 6504}}}},
    {"servers": {"2": {"alias": "Server1", "type": "TCP",
                       "settings": '{"Alias":"Server1","Type":"TCP","Tcp":{"DstAddress":"192.168.0.1","DstPort":6504}}'}}},
    request={"Request": {"Command": "GetStreamingServerSettings", "SessionID": "vector", "Params": {"ID": 1}}})
_jvt("zoom-presets", {"Response": {"Requested": "GetPresetZoomPosition", "Result": "Success", "Data": {"A": 10, "B": 20, "C": -1}}},
     {"zoom_presets": {"a": 10, "b": 20, "c": -1}})
_jvt("ntp-status", {"Response": {"Requested": "GetNTPStatus", "Result": "Success", "Data": {
    "Address": "192.168.0.100", "TcSync": "On", "Status": "Syncronized"}}},
    {"ntp": {"server": "192.168.0.100", "timecode_sync": "On", "status": "Syncronized"}})
_jvt("gps", {"Response": {"Requested": "GetGPSInfo", "Result": "Success", "Data": {
    "AntennaLevel": "Level3", "Location": "+35.360452+138.727820+3775.630/"}}},
    {"gps": {"antenna": "Level3", "location": "+35.360452+138.727820+3775.630/"}})
_jvt("video-settings", {"Response": {"Requested": "GetVideoSettings", "Result": "Success", "Data": {
    "Output": "SDI", "VideoFormat": "60Hz", "EncodeLevel": "highprofile"}}},
    {"video": {"output": "SDI", "format": "60Hz", "encode_level": "highprofile"}}, model=PZ)
