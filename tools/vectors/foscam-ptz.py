# Foscam (foscam-ptz): one vector per command, written by hand from the
# Foscam IPCamera CGI User Guide's usage and example lines: GET
# /cgi-bin/CGIProxy.fcgi?cmd=<command>&<arguments>, the account in usr and pwd.
# Commands that read their setting back state the change, which goes first.
FC = "foscam-ptz"
FC_SET = {"username": "admin", "password": "pw"}
FCA = "&usr=admin&pwd=pw"
CGI = "/cgi-bin/CGIProxy.fcgi?cmd="


def _fc(command, input, cgi, **extra):
    extra.setdefault("settings", dict(FC_SET))
    http(FC, command, input, "GET", CGI + cgi + FCA, **extra)


_fc("get_image_settings", {}, "getImageSetting")
_fc("set_brightness", {"level": 50}, "setBrightness&brightness=50")
_fc("set_contrast", {"level": 50}, "setContrast&contrast=50")
_fc("set_hue", {"level": 50}, "setHue&hue=50")
_fc("set_saturation", {"level": 50}, "setSaturation&saturation=50")
_fc("set_sharpness", {"level": 50}, "setSharpness&sharpness=50")
_fc("reset_image_settings", {}, "resetImageSetting")
_fc("get_mirror_flip", {}, "getMirrorAndFlipSetting")
_fc("set_mirror", {"mirrored": True}, "mirrorVideo&isMirror=1")
_fc("set_flip", {"flipped": True}, "flipVideo&isFlip=1")
_fc("set_power_frequency", {"frequency": "60"}, "setPwrFreq&freq=0",
    http_reply={"status": 200, "body": "<CGI_Result><result>0</result></CGI_Result>"},
    expect_result={"ok": {"kind": "ack"}})
_fc("set_power_frequency", {"frequency": "50"}, "setPwrFreq&freq=1",
    http_reply={"status": 200, "body": "<CGI_Result><result>-2</result></CGI_Result>"},
    expect_result={"error": {"error": "device_error"}}, file="set_power_frequency-refused")
_fc("get_video_streams", {}, "getVideoStreamParam")
_fc("set_video_stream", {"stream": 0, "resolution": 0, "bitrate": 2097152, "frame_rate": 30, "gop": 30},
    "setVideoStreamParam&streamType=0&resolution=0&bitRate=2097152&frameRate=30&GOP=30&isVBR=0")
_fc("get_main_stream_type", {}, "getMainVideoStreamType")
_fc("get_sub_stream_type", {}, "getSubVideoStreamType")
_fc("set_main_stream_type", {"stream": 0}, "setMainVideoStreamType&streamType=0")
_fc("set_sub_stream_format", {"format": "h264"}, "setSubStreamFormat&format=0")
_fc("get_osd", {}, "getOSDSetting")
_fc("set_osd", {"time_stamp": True, "name": True, "masks": False},
    "setOSDSetting&isEnableTimeStamp=1&isEnableDevName=1&dispPos=0&isEnableOSDMask=0")
_fc("get_mask_areas", {}, "getOsdMaskArea")
_mask = {f"{c}_{n}": 0 for n in range(4) for c in ("x1", "y1", "x2", "y2")}
_mask.update({"x1_0": 100, "y1_0": 100, "x2_0": 200, "y2_0": 200})
_fc("set_mask_areas", _mask,
    "setOsdMaskArea&x1_0=100&y1_0=100&x2_0=200&y2_0=200&x1_1=0&y1_1=0&x2_1=0&y2_1=0&x1_2=0&y1_2=0&x2_2=0"
    "&y2_2=0&x1_3=0&y1_3=0&x2_3=0&y2_3=0")
_fc("get_mask_enabled", {}, "getOSDMask")
_fc("set_mask_enabled", {"enabled": True}, "setOSDMask&isEnableOSDMask=1")
_fc("move", {"direction": "up"}, "ptzMoveUp")
_fc("move", {"direction": "bottom_right"}, "ptzMoveBottomRight", file="move-bottom-right")
_fc("stop", {}, "ptzStopRun")
_fc("reset_position", {}, "ptzReset")
_fc("get_ptz_speed", {}, "getPTZSpeed")
_fc("set_ptz_speed", {"speed": "normal"}, "setPTZSpeed&speed=2")
_fc("get_presets", {}, "getPTZPresetPointList")
_fc("add_preset", {"name": "test"}, "ptzAddPresetPoint&name=test")
_fc("delete_preset", {"name": "test"}, "ptzDeletePresetPoint&name=test")
_fc("goto_preset", {"name": "test"}, "ptzGotoPresetPoint&name=test")
_fc("get_cruise_maps", {}, "ptzGetCruiseMapList")
_fc("get_cruise_map", {"name": "Horizental"}, "ptzGetCruiseMapInfo&name=Horizental")
_fc("set_cruise_map", {"name": "test", "point0": "test0", "point1": "test1", "point2": "test2", "point3": "test3",
                       "point4": "test4", "point5": "test5", "point6": "", "point7": ""},
    "ptzSetCruiseMap&name=test&point0=test0&point1=test1&point2=test2&point3=test3&point4=test4&point5=test5"
    "&point6=&point7=")
_fc("delete_cruise_map", {"name": "test"}, "ptzDelCruiseMap&name=test")
_fc("start_cruise", {"name": "test"}, "ptzStartCruise&mapName=test")
_fc("stop_cruise", {}, "ptzStopCruise")
_fc("zoom_in", {}, "zoomIn")
_fc("zoom_out", {}, "zoomOut")
_fc("zoom_stop", {}, "zoomStop")
_fc("get_zoom_speed", {}, "getZoomSpeed")
_fc("set_zoom_speed", {"speed": "normal"}, "setZoomSpeed&speed=1")
_fc("get_self_test_mode", {}, "getPTZSelfTestMode")
_fc("set_self_test_mode", {"mode": "normal_then_preset"}, "setPTZSelfTestMode&mode=2")
_fc("get_self_test_preset", {}, "getPTZPrePointForSelfTest")
_fc("set_self_test_preset", {"name": "TopMost"}, "setPTZPrePointForSelfTest&name=TopMost")
_fc("get_rs485", {}, "get485Info")
_fc("set_rs485", {"protocol": 0, "address": 1, "baud": "1200", "data_bits": 7, "stop_bits": 1, "parity": 0},
    "set485Info&rs485Protocol=0&rs485Addr=1&rs485Baud=1200&rs485DataBit=7&rs485StopBit=1&rs485Check=0")
_fc("get_device_state", {}, "getDevState")
_fc("get_device_info", {}, "getDevInfo")
_fc("get_device_name", {}, "getDevName")
_fc("set_device_name", {"name": "Stage cam"}, "setDevName&devName=Stage%20cam")
_fc("ir_led_on", {}, "openInfraLed")
_fc("ir_led_off", {}, "closeInfraLed")
_fc("get_ir_led_mode", {}, "getInfraLedConfig")
_fc("set_ir_led_mode", {"mode": "auto"}, "setInfraLedConfig&mode=0")
_fc("get_system_time", {}, "getSystemTime")
_fc("set_time_from_ntp", {"server": "time.nist.gov", "date_format": "YYYY-MM-DD", "time_format": "24", "time_zone": 0,
                          "dst": False},
    "setSystemTime&timeSource=0&ntpServer=time.nist.gov&dateFormat=0&timeFormat=1&timeZone=0&isDst=0&dst=0")
# The guide's example: timeSource=1, 2012-01-06 09:35:00.
_fc("set_time_manually", {"date_format": "YYYY-MM-DD", "time_format": "12", "time_zone": 0, "dst": False,
                          "year": 2012, "month": 1, "day": 6, "hour": 9, "minute": 35, "second": 0},
    "setSystemTime&timeSource=1&ntpServer=&dateFormat=0&timeFormat=0&timeZone=0&isDst=0&dst=0&year=2012&mon=1"
    "&day=6&hour=9&minute=35&sec=0")
_fc("reboot", {}, "rebootSystem")
_fc("factory_reset", {}, "restoreToFactorySetting")


def _fct(name, cgi, body, state, **extra):
    telemetry(FC, name, settings=dict(FC_SET), inbound_http={"path": CGI + cgi + FCA, "body": body},
              expect_state=state, **extra)


_fct("device-state", "getDevState",
     "<CGI_Result><result>0</result><IOAlarm>0</IOAlarm><motionDetectAlarm>1</motionDetectAlarm><soundAlarm>2</soundAlarm>"
     "<record>1</record><sdState>1</sdState><sdFreeSpace>1024</sdFreeSpace><sdTotalSpace>8192</sdTotalSpace>"
     "<ntpState>1</ntpState><ddnspState>0</ddnspState><url></url><upnpState>2</upnpState><isWifiConnected>0</isWifiConnected>"
     "<wifiConnectedAP></wifiConnectedAP><infraLedState>1</infraLedState></CGI_Result>",
     {"alarms": {"io": "disabled", "motion": "none", "sound": "alarm"}, "recording": True,
      "sd": {"state": "ok", "free_kb": 1024, "total_kb": 8192}, "ntp_state": "updated",
      "ddns": {"state": "disabled", "url": ""}, "upnp_state": "failed", "wifi": {"connected": False, "ssid": ""},
      "ir_led": True})
_fct("device-info", "getDevInfo",
     "<CGI_Result><result>0</result><productName>FI9821P</productName><serialNo>0000000000000001</serialNo>"
     "<devName>Lobby</devName><mac>C4D6553A1234</mac><firmwareVer>1.11.1.10</firmwareVer>"
     "<hardwareVer>1.4.1.8</hardwareVer></CGI_Result>",
     {"device": {"model": "FI9821P", "serial": "0000000000000001", "name": "Lobby", "mac": "C4D6553A1234",
                 "firmware": "1.11.1.10", "hardware": "1.4.1.8"}})
_fct("image", "getImageSetting",
     "<CGI_Result><result>0</result><brightness>50</brightness><contrast>48</contrast><hue>50</hue>"
     "<saturation>52</saturation><sharpness>40</sharpness><denoiseLevel>0</denoiseLevel></CGI_Result>",
     {"image": {"brightness": 50, "contrast": 48, "hue": 50, "saturation": 52, "sharpness": 40}})
_fct("mirror-flip", "getMirrorAndFlipSetting",
     "<CGI_Result><result>0</result><isMirror>1</isMirror><isFlip>0</isFlip></CGI_Result>",
     {"image": {"mirrored": True, "flipped": False}})
_fct("video-streams", "getVideoStreamParam",
     "<CGI_Result><result>0</result><resolution0>0</resolution0><bitRate0>2097152</bitRate0><frameRate0>30</frameRate0>"
     "<GOP0>30</GOP0><isVBR0>0</isVBR0><resolution1>2</resolution1><bitRate1>524288</bitRate1><frameRate1>15</frameRate1>"
     "<GOP1>15</GOP1><isVBR1>0</isVBR1></CGI_Result>",
     {"streams": {"0": {"resolution": 0, "bitrate": 2097152, "frame_rate": 30, "gop": 30},
                  "1": {"resolution": 2, "bitrate": 524288, "frame_rate": 15, "gop": 15}}})
_fct("osd", "getOSDSetting",
     "<CGI_Result><result>0</result><isEnableTimeStamp>1</isEnableTimeStamp><isEnableDevName>0</isEnableDevName>"
     "<dispPos>0</dispPos><isEnableOSDMask>1</isEnableOSDMask></CGI_Result>",
     {"osd": {"time_stamp": True, "name": False, "masks": True}})
_fct("ptz-speed", "getPTZSpeed", "<CGI_Result><result>0</result><speed>3</speed></CGI_Result>", {"ptz_speed": "fast"})
_fct("zoom-speed", "getZoomSpeed", "<CGI_Result><result>0</result><speed>0</speed></CGI_Result>", {"zoom_speed": "slow"})
_fct("ir-mode", "getInfraLedConfig", "<CGI_Result><result>0</result><mode>1</mode></CGI_Result>", {"ir_led_mode": "manual"})
_fct("presets", "getPTZPresetPointList",
     "<CGI_Result><result>0</result><cnt>5</cnt><point0>LeftMost</point0><point1>RightMost</point1><point2>TopMost</point2>"
     "<point3>BottomMost</point3><point4>Stage</point4></CGI_Result>",
     {"presets": {"1": {"name": "LeftMost"}, "2": {"name": "RightMost"}, "3": {"name": "TopMost"},
                  "4": {"name": "BottomMost"}, "5": {"name": "Stage"}}},
     state_before={"presets": {"6": {"name": "Deleted"}}})
_fct("cruise-maps", "ptzGetCruiseMapList",
     "<CGI_Result><result>0</result><cnt>2</cnt><map0>Horizental</map0><map1>Vertical</map1></CGI_Result>",
     {"cruise_maps": {"1": {"name": "Horizental"}, "2": {"name": "Vertical"}}})
_fct("system-time", "getSystemTime",
     "<CGI_Result><result>0</result><timeSource>1</timeSource><ntpServer>time.nist.gov</ntpServer><dateFormat>0</dateFormat>"
     "<timeFormat>1</timeFormat><timeZone>-3600</timeZone><isDst>0</isDst><dst>0</dst><year>2026</year><mon>10</mon>"
     "<day>27</day><hour>14</hour><minute>5</minute><sec>9</sec></CGI_Result>",
     {"time": {"source": "manual", "ntp_server": "time.nist.gov", "zone": -3600, "dst": False, "year": 2026, "month": 10,
               "day": 27, "hour": 14, "minute": 5, "second": 9}})
