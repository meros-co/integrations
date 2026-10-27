# OBSBOT Tail 2 and Tail Air (obsbot-tail): one vector per command, written
# by hand from OBSBOT's OSC definition spreadsheets for the Tail 2 and the
# Tail Air (address, type tags and value column of each row) and encoded with
# this script's own OSC encoder. Every command carries int arguments.
OB = "obsbot-tail"
G = "/OBSBOT/WebCam/General/"
T2 = "/OBSBOT/Camera/Tail2/"
TA = "/OBSBOT/Camera/TailAir/"


def _ob(command, input, address, *ints, model=None, file=None):
    extra = {}
    if model:
        extra["model"] = model
    if file:
        extra["file"] = file
    binary(OB, command, input, osc(address, *[("i", v) for v in ints]), **extra)


_ob("connect", {}, G + "Connected", 0)
_ob("disconnect", {}, G + "Disconnected", 0)
_ob("reset_gimbal", {}, G + "ResetGimbal", 0)
_ob("set_zoom", {"zoom": 40}, G + "SetZoom", 40)
_ob("set_zoom_with_speed", {"zoom": 100, "speed": 10}, G + "SetZoomSpeed", 100, 10)
_ob("set_zoom_with_speed", {"zoom": 0}, G + "SetZoomSpeed", 0, 0, file="set_zoom_with_speed-default-speed")
_ob("set_zoom_with_speed_tail_air", {"zoom": 55, "speed": 11}, G + "SetZoomSpeed", 55, 11, model="tail-air")
_ob("zoom_max", {}, G + "SetZoomMax", 0)
_ob("zoom_min", {}, G + "SetZoomMin", 0)
_ob("gimbal_up", {"speed": 30}, G + "SetGimbalUp", 30)
_ob("gimbal_down", {}, G + "SetGimbalDown", 50)
_ob("gimbal_left", {"speed": 100}, G + "SetGimbalLeft", 100)
_ob("gimbal_right", {"speed": 0}, G + "SetGimbalRight", 0)
binary(OB, "gimbal_stop", {}, [osc(G + "SetGimbalUp", ("i", 0)), osc(G + "SetGimbalLeft", ("i", 0))])
_ob("set_gimbal_angles", {"speed": 90, "pan": -159, "pitch": 62}, G + "SetGimMotorDegree", 90, -159, 62)
_ob("set_gimbal_angles_tail_air", {"pan": 129, "pitch": -59}, G + "SetGimMotorDegree", 45, 129, -59,
    model="tail-air")
_ob("set_mirror", {"mirrored": True}, G + "SetMirror", 1)
_ob("set_auto_focus", {"enabled": False}, G + "SetAutoFocus", 0)
_ob("set_manual_focus", {"focus": 73}, G + "SetManualFocus", 73)
_ob("set_auto_exposure", {"enabled": True}, G + "SetAutoExposure", 1)
_ob("set_exposure_compensation", {"value": "-17"}, G + "SetExposureCompensate", -17)
_ob("set_shutter_speed", {"speed": "6400"}, G + "SetShutterSpeed", 6400)
_ob("set_iso", {"iso": 800}, G + "SetISO", 800)
_ob("set_auto_white_balance", {"enabled": False}, G + "SetAutoWhiteBalance", 0)
_ob("set_color_temperature", {"kelvin": 5600}, G + "SetColorTemperature", 5600)
_ob("set_ai_enabled", {"enabled": True}, T2 + "SetAiEnable", 1)
_ob("set_ai_mode_tail_2", {"mode": "multi"}, T2 + "SetAiMode", 1)
_ob("set_tracking_speed_tail_2", {"speed": "custom", "pan_auto": False, "pan_speed": 7, "pitch_auto": True,
                                  "pitch_speed": 3}, T2 + "SetTrackingSpeed", 5, 0, 7, 1, 3)
_ob("set_tracking_speed_tail_2", {"speed": "crazy"}, T2 + "SetTrackingSpeed", 4, 1, 5, 1, 5,
    file="set_tracking_speed_tail_2-preset")
_ob("set_auto_zoom", {"framing": "long_shot_2"}, T2 + "SetAutoZoom", 7)
_ob("set_only_me", {"enabled": False}, T2 + "SetOnlyMe", 0)
_ob("set_ai_mode_tail_air", {"mode": "animal"}, TA + "SetAiMode", 6, model="tail-air")
_ob("set_tracking_speed_tail_air", {"speed": "fast"}, TA + "SetTrackingSpeed", 2, model="tail-air")
_ob("set_recording", {"recording": True}, T2 + "SetRecording", 1)
_ob("set_recording", {"recording": False}, TA + "SetRecording", 0, model="tail-air", file="set_recording-tail-air")
_ob("snapshot", {}, T2 + "Snapshot", 1)
_ob("snapshot", {}, TA + "Snapshot", 1, model="tail-air", file="snapshot-tail-air")
_ob("trigger_preset", {"preset": 1}, T2 + "TriggerPreset", 0)
_ob("trigger_preset", {"preset": 3}, TA + "TriggerPreset", 2, model="tail-air", file="trigger_preset-tail-air")
_ob("get_device_info", {}, G + "GetDeviceInfo", 0)
_ob("get_zoom_info", {}, G + "GetZoomInfo", 0)
_ob("get_gimbal_position", {}, G + "GetGimbalPosInfo", 0)

# Telemetry: the replies' type tags and meanings from the spreadsheets'
# "Get Device Information" rows.
telemetry(OB, "connected", inbound_hex=hexs(osc(G + "ConnectedResp", ("i", 1))),
          expect_state={"connected": True},
          expect_connect_wire_hex=[hexs(osc(G + "Connected", ("i", 0))), hexs(osc(G + "GetDeviceInfo", ("i", 0))),
                              hexs(osc(G + "GetZoomInfo", ("i", 0))), hexs(osc(G + "GetGimbalPosInfo", ("i", 0)))])
telemetry(OB, "device-info", inbound_hex=hexs(osc(
    G + "DeviceInfo", ("i", 1), ("s", "OBSBOT Tail 2"), ("i", 0), ("s", ""), ("i", 0), ("s", ""), ("i", 0), ("s", ""),
    ("i", 0), ("i", 1), ("i", 3))),
    expect_state={"devices": {"1": {"connected": True, "name": "OBSBOT Tail 2"},
                              "2": {"connected": False, "name": ""}, "3": {"connected": False, "name": ""},
                              "4": {"connected": False, "name": ""}},
                  "selected_device": 1, "running": True, "device_type": 3})
telemetry(OB, "zoom-info", inbound_hex=hexs(osc(G + "ZoomInfo", ("i", 62), ("i", 1))),
          expect_state={"zoom": {"position": 62, "field_of_view": 78}})
telemetry(OB, "gimbal-position", inbound_hex=hexs(osc(G + "GetGimbalPosInfoResp", ("i", 0), ("i", -12), ("i", 95))),
          expect_state={"gimbal": {"roll": 0, "pitch": -12, "yaw": 95}})
