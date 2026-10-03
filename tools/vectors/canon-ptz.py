# Canon XC protocol (canon-ptz): HTTP GETs under /-wvhttp-01-/. control.cgi
# with pan, tilt and zoom is Canon's public example ("How do you control the
# Pan, Tilt and Zoom?"); every other path and key=value pair is written here
# from the Bitfocus Companion module's requests (src/actions.js, index.js),
# with query values percent-encoded. info.cgi replies are key:=value lines
# (src/polling.js).
CN = "canon-ptz"

_CN_ACK = {"ok": {"kind": "ack"}}


def _cn(command, input, path, query=None, **extra):
    from urllib.parse import quote
    target = "/-wvhttp-01-/" + path
    if query:
        target += "?" + "&".join(f"{k}={quote(str(v), safe='')}" for k, v in query)
    V.append({"spec": CN, "command": command, "input": input,
              "expect_request": {"method": "GET", "target": target}, **extra})


def _ctl(command, input, *pairs, **extra):
    _cn(command, input, "control.cgi", list(pairs), **extra)


_cn("power_on", {}, "standby.cgi", [("cmd", "idle")], http_reply={"status": 200, "body": ""},
    expect_result=_CN_ACK)
_cn("power_standby", {}, "standby.cgi", [("cmd", "standby")])
_ctl("set_camera_name", {"name": "Cam 1"}, ("c.1.name.utf8", "Cam 1"))
_ctl("set_tally", {"mode": "program"}, ("tally", "on"), ("tally.mode", "program"))
_ctl("digital_zoom_off", {}, ("c.1.zoom.mode", "off"))
_ctl("digital_zoom_on", {}, ("c.1.zoom.mode", "dzoom"))
_ctl("advanced_zoom_on", {}, ("c.1.zoom.mode", "advanced"))
_ctl("digital_magnification_on", {}, ("c.1.zoom.mode", "mag"))
_ctl("set_digital_magnification", {"percent": 150}, ("c.1.zoom.mag", 150))
_ctl("set_image_stabilization", {"mode": "on1"}, ("c.1.is", "on1"))
_ctl("set_color_bars", {"state": "off"}, ("c.1.colorbar", "off"))
_ctl("pan_left", {}, ("pan", "left"), ("pan.speed.dir", 625))
_ctl("pan_right", {"speed": 10000}, ("pan", "right"), ("pan.speed.dir", 10000))
_ctl("tilt_up", {"speed": 10}, ("tilt", "up"), ("tilt.speed.dir", 10))
_ctl("tilt_down", {"speed": 300}, ("tilt", "down"), ("tilt.speed.dir", 300))
_ctl("pan_tilt", {"pan": "left", "tilt": "up", "speed": 1250},
     ("pan", "left"), ("pan.speed.dir", 1250), ("tilt", "up"), ("tilt.speed.dir", 1250))
_ctl("pan_tilt_stop", {}, ("pan", "stop"), ("tilt", "stop"))
_ctl("pan_stop", {}, ("pan", "stop"))
_ctl("tilt_stop", {}, ("tilt", "stop"))
_ctl("pan_tilt_home", {}, ("pan", 0), ("tilt", 0))
# Canon's public example: control.cgi?pan=XXX&tilt=YYY&zoom=ZZZ.
_ctl("pan_tilt_zoom_to", {"pan": -4500, "tilt": 1200, "zoom": 3000}, ("pan", -4500), ("tilt", 1200), ("zoom", 3000))
_cn("pan_tilt_initialize", {}, "maintain", [("cmd", "platform_reset")])
_ctl("zoom_tele", {}, ("zoom", "tele"), ("zoom.speed.dir", 8))
_ctl("zoom_wide", {"speed": 127}, ("zoom", "wide"), ("zoom.speed.dir", 127))
_ctl("zoom_stop", {}, ("zoom", "stop"))
_ctl("zoom_to", {"position": 5000}, ("zoom", 5000))
_ctl("set_soft_zoom", {"mode": "both"}, ("c.1.zoom.accel", "both"))
_ctl("focus_near", {}, ("focus.action", "near"))
_ctl("focus_far", {}, ("focus.action", "far"))
_ctl("focus_stop", {}, ("focus.action", "stop"))
_ctl("set_focus_speed", {"speed": 2}, ("focus.speed", 2))
_ctl("set_focus_mode", {"mode": "manual"}, ("focus", "manual"))
_ctl("one_shot_af", {}, ("c.1.focus.action", "one_shot"))
_ctl("spot_af", {}, ("c.1.focus.action", "spot"))
_ctl("set_face_detection", {"mode": "facecatch"}, ("c.1.focus.detect", "facecatch"))
_ctl("set_subject_detection", {"mode": "anml_only"}, ("c.1.focus.detect", "anml_only"))
_ctl("set_shooting_mode", {"mode": "manual"}, ("c.1.shooting", "manual"))
_ctl("set_exposure_mode", {"mode": "tv"}, ("c.1.exp", "tv"))
_ctl("set_scene", {"scene": "lowlight"}, ("c.1.scene", "lowlight"))
_ctl("set_ae_gain_limit", {"tenths_db": 330}, ("c.1.ae.gainlimit.max", 330))
_ctl("set_ae_brightness", {"value": -2}, ("c.1.ae.brightness", -2))
_ctl("set_ae_photometry", {"mode": "backlight"}, ("c.1.ae.photometry", "backlight"))
_ctl("set_ae_flicker_reduction", {"mode": "auto"}, ("c.1.ae.flickerreduct", "auto"))
_ctl("set_shutter_mode", {"mode": "speed"}, ("c.1.me.shutter.mode", "speed"))
_ctl("set_shutter", {"denominator": 60}, ("c.1.me.shutter", 60))
_ctl("set_iris_mode", {"mode": "manual"}, ("c.1.me.diaphragm.mode", "manual"))
_ctl("set_iris", {"f_number_x100": 280}, ("c.1.me.diaphragm", 280))
_ctl("set_iris_increment", {"step": "4"}, ("c.1.me.diaphragm.increment", 4))
_ctl("set_iris_fine", {"state": "on"}, ("c.1.me.diaphragm.fine", "on"))
_ctl("set_gain_mode", {"mode": "manual"}, ("c.1.me.gain.mode", "manual"))
_ctl("set_gain", {"tenths_db": -60}, ("c.1.me.gain", -60))
_ctl("set_gain_increment", {"step": "fine"}, ("c.1.me.gain.increment", "fine"))
_ctl("set_nd_filter", {"value": "1600"}, ("c.1.nd.filter", 1600))
_ctl("set_nd_mode", {"mode": "assist"}, ("c.1.nd.mode", "assist"))
_ctl("set_pedestal", {"value": -5}, ("c.1.blacklevel", -5))
_ctl("set_sharpness", {"value": 50}, ("c.1.ac", 50))
_ctl("set_noise_reduction", {"value": 12}, ("c.1.nr", 12))
_ctl("set_white_balance_mode", {"mode": "kelvin"}, ("c.1.wb", "kelvin"))
_ctl("white_balance_one_shot", {"memory": "b"}, ("c.1.wb.action", "one_shot_b"))
_ctl("set_kelvin", {"kelvin": 5600}, ("c.1.wb.kelvin", 5600))
_ctl("set_wb_r_gain", {"value": 10}, ("c.1.wb.shift.rgain", 10))
_ctl("set_wb_b_gain", {"value": -10}, ("c.1.wb.shift.bgain", -10))
_ctl("recall_preset", {"preset": 3}, ("p", 3))
_ctl("recall_preset_time", {"preset": 4, "time_ms": 5000}, ("p", 4), ("p.ptztime", 5000))
_ctl("recall_preset_speed", {"preset": 5, "speed": 100}, ("p", 5), ("p.ptzspeed", 100))
_cn("save_preset", {"preset": 7, "name": "Pulpit wide"}, "preset/set",
    [("p", 7), ("name", "Pulpit wide"), ("all", "enabled")])
_cn("save_preset_selective", {"preset": 8, "name": "Stage", "focus": "disabled", "cp": "disabled"}, "preset/set",
    [("p", 8), ("name", "Stage"), ("ptz", "enabled"), ("focus", "disabled"), ("exp", "enabled"), ("wb", "enabled"),
     ("is", "enabled"), ("cp", "disabled")])
_cn("trace_prepare", {"trace": 1}, "trace/control", [("t", 1), ("cmd", "prepare")])
_cn("trace_start", {"trace": 10}, "trace/control", [("t", 10), ("cmd", "start")])
_cn("trace_stop", {"trace": 2}, "trace/control", [("t", 2), ("cmd", "stop")])
_INFO = "s.firmware:=Ver.1.2.0\r\nc.1.zoom:=3000\r\nf.standby:=idle\r\n"
_cn("get_info", {}, "info.cgi", None, http_reply={"status": 200, "body": _INFO},
    expect_result={"ok": {"kind": "value", "value": _INFO.rstrip("\r\n")}})

# info.cgi replies, as key:=value lines.
_INFO_PATH = "/-wvhttp-01-/info.cgi"
telemetry(CN, "system", inbound_http={"path": _INFO_PATH, "body":
    "c.1.type:=CR-N500\r\nc.1.name.utf8:=Cam 1\r\ns.firmware:=Ver.1.2.0\r\ns.protocol:=1.4\r\n"
    "s.hardware.address:=00:1e:8f:11:22:33\r\nf.standby:=idle\r\nf.tally:=on\r\nf.tally.mode:=program\r\n"},
    expect_state={"model": "CR-N500", "name": "Cam 1", "firmware": "Ver.1.2.0", "protocol_version": "1.4",
                  "mac_address": "00:1e:8f:11:22:33", "power": "idle",
                  "tally": {"state": "on", "mode": "program"}})
telemetry(CN, "lens", inbound_http={"path": _INFO_PATH, "body":
    "c.1.zoom.mode:=dzoom\nc.1.zoom.mag:=150\nc.1.zoom:=3000\nc.1.zoom.accel:=both\nc.1.is:=on1\n"
    "c.1.focus:=manual\nc.1.focus.speed:=1\nc.1.focus.value:=-120\nc.1.focus.detect:=faceonly\n"},
    expect_state={"zoom": {"mode": "dzoom", "magnification": 150, "position": 3000, "soft": "both"},
                  "image_stabilization": "on1",
                  "focus": {"mode": "manual", "speed": 1, "position": -120, "detection": "faceonly"}})
telemetry(CN, "exposure", inbound_http={"path": _INFO_PATH, "body":
    "c.1.shooting:=manual\nc.1.exp:=tv\nc.1.scene:=sports\nc.1.ae.gainlimit.max:=330\nc.1.ae.brightness:=-2\n"
    "c.1.ae.photometry:=center\nc.1.ae.flickerreduct:=auto\nc.1.me.shutter.mode:=speed\nc.1.me.shutter:=60\n"
    "c.1.me.diaphragm.mode:=manual\nc.1.me.diaphragm:=280\nc.1.me.gain.mode:=manual\nc.1.me.gain:=120\n"
    "c.1.nd.filter:=400\nc.1.nd.mode:=assist\n"},
    expect_state={"exposure": {"shooting": "manual", "mode": "tv", "scene": "sports", "gain_limit": 330,
                               "brightness": -2, "photometry": "center", "flicker_reduction": "auto"},
                  "shutter": {"mode": "speed", "value": "60"}, "iris": {"mode": "manual", "value": "280"},
                  "gain": {"mode": "manual", "value": "120"}, "nd": {"filter": "400", "mode": "assist"}})
telemetry(CN, "picture", inbound_http={"path": _INFO_PATH, "body":
    "c.1.blacklevel:=-3\nc.1.ac:=10\nc.1.nr:=4\nc.1.wb:=kelvin\nc.1.wb.kelvin:=5600\nc.1.wb.shift.rgain:=5\n"
    "c.1.wb.shift.bgain:=-5\nc.1.colorbar:=off\nc.1.platform.status:=normal\np:=3\np.count:=100\n"},
    expect_state={"pedestal": -3, "sharpness": 10, "noise_reduction": 4,
                  "white_balance": {"mode": "kelvin", "kelvin": 5600, "r_gain": 5, "b_gain": -5},
                  "color_bars": False, "pan_tilt": {"status": "normal"}, "preset": {"last": 3, "count": 100}})
