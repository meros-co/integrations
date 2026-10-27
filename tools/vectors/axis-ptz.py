# Axis PTZ (axis-ptz): one vector per command, written by hand from the VAPIX
# Pan/tilt/zoom API's argument tables and examples (ptz.cgi, ptzconfig.cgi,
# ptzqueue.cgi, memoryzones/*.cgi) and the Parameter management API's
# action=list and action=update forms. Every request names the video channel
# with camera=, 1 by default. Commands of several requests state the first;
# the read-back that follows is a request the polls also make.
AX = "axis-ptz"
PTZ = "/axis-cgi/com/ptz.cgi?"
CFG = "/axis-cgi/com/ptzconfig.cgi?"
Q = "/axis-cgi/com/ptzqueue.cgi?"
PRM = "/axis-cgi/param.cgi?"
MZ = "/axis-cgi/memoryzones/"
C1 = "&camera=1"
OK204 = {"status": 204}
ACK = {"ok": {"kind": "ack"}}


def _ax(command, input, target, **extra):
    http(AX, command, input, "GET", target, **extra)


_ax("move", {"direction": "upleft"}, PTZ + "move=upleft" + C1, http_reply=OK204, expect_result=ACK)
_ax("home", {}, PTZ + "move=home" + C1)
_ax("stop", {}, PTZ + "move=stop" + C1)
_ax("pan_to", {"pan": -45}, PTZ + "pan=-45.00" + C1)
_ax("tilt_to", {"tilt": 12.5}, PTZ + "tilt=12.50" + C1)
_ax("zoom_to", {"zoom": 9999}, PTZ + "zoom=9999" + C1)
_ax("focus_to", {"focus": 5000}, PTZ + "focus=5000" + C1)
_ax("iris_to", {"iris": 1}, PTZ + "iris=1" + C1)
_ax("brightness_to", {"brightness": 4500}, PTZ + "brightness=4500" + C1)
# The API's own example: rpan=10 pans 10 degrees right.
_ax("pan_by", {"degrees": 10}, PTZ + "rpan=10.00" + C1)
_ax("tilt_by", {"degrees": -2.25}, PTZ + "rtilt=-2.25" + C1)
_ax("zoom_by", {"steps": -500}, PTZ + "rzoom=-500" + C1)
_ax("focus_by", {"steps": 100}, PTZ + "rfocus=100" + C1)
_ax("iris_by", {"steps": -100}, PTZ + "riris=-100" + C1)
_ax("brightness_by", {"steps": 250}, PTZ + "rbrightness=250" + C1)
_ax("set_autofocus", {"enabled": True}, PTZ + "autofocus=on" + C1)
_ax("set_autoiris", {"enabled": False}, PTZ + "autoiris=off" + C1)
_ax("continuous_pan_tilt", {"pan_speed": 50, "tilt_speed": -20}, PTZ + "continuouspantiltmove=50,-20" + C1)
_ax("continuous_pan_tilt", {"pan_speed": 0, "tilt_speed": 0}, PTZ + "continuouspantiltmove=0,0" + C1,
    file="continuous_pan_tilt-stop")
_ax("continuous_pan_tilt_fixed_speed", {"pan_speed": -100, "tilt_speed": 100},
    PTZ + "continuouspantiltmove=-100,100&proportionalspeed=disabled" + C1)
_ax("continuous_zoom", {"speed": 30}, PTZ + "continuouszoommove=30" + C1)
_ax("continuous_focus", {"speed": -10}, PTZ + "continuousfocusmove=-10" + C1)
_ax("continuous_iris", {"speed": 5}, PTZ + "continuousirismove=5" + C1)
_ax("continuous_brightness", {"speed": -5}, PTZ + "continuousbrightnessmove=-5" + C1)
_ax("center", {"x": 320, "y": 240}, PTZ + "center=320,240" + C1)
_ax("center_in_image", {"x": 960, "y": 540, "image_width": 1920, "image_height": 1080},
    PTZ + "center=960,540&imagewidth=1920&imageheight=1080" + C1)
_ax("area_zoom", {"x": 100, "y": 200, "z": 300}, PTZ + "areazoom=100,200,300" + C1)
_ax("area_zoom_in_image", {"x": 100, "y": 200, "z": 50, "image_width": 640, "image_height": 360},
    PTZ + "areazoom=100,200,50&imagewidth=640&imageheight=360" + C1)
_ax("goto_preset", {"name": "Stage Left"}, PTZ + "gotoserverpresetname=Stage%20Left" + C1)
_ax("goto_preset_number", {"number": 3}, PTZ + "gotoserverpresetno=3" + C1)
_ax("goto_device_preset", {"number": 95}, PTZ + "gotodevicepreset=95" + C1)
_ax("set_speed", {"speed": 70}, PTZ + "speed=70" + C1)
_ax("set_ir_cut_filter", {"mode": "auto"}, PTZ + "ircutfilter=auto" + C1)
_ax("set_backlight", {"enabled": True}, PTZ + "backlight=on" + C1)
_ax("auxiliary", {"function": "tt:LightIR|On"}, PTZ + "auxiliary=tt%3ALightIR%7COn" + C1)
_ax("speed_dry", {}, PTZ + "auxiliary=speeddry" + C1)
# The API's own example: setserverpresetname=MyPreset&home=yes.
_ax("save_preset", {"name": "Lectern"}, CFG + "setserverpresetname=Lectern" + C1)
_ax("save_preset_as_home", {"name": "MyPreset"}, CFG + "setserverpresetname=MyPreset&home=yes" + C1)
_ax("save_preset_number", {"number": 4}, CFG + "setserverpresetno=4" + C1)
_ax("save_preset_number_as_home", {"number": 1}, CFG + "setserverpresetno=1&home=yes" + C1)
_ax("rename_preset", {"number": 4, "name": "Choir"}, CFG + "renameserverpresetno=4&newname=Choir" + C1)
_ax("remove_preset", {"name": "Choir"}, CFG + "removeserverpresetname=Choir" + C1)
_ax("remove_preset_number", {"number": 4}, CFG + "removeserverpresetno=4" + C1)
_ax("remove_all_presets", {}, CFG + "removeallserverpresets=yes" + C1)
_ax("set_device_preset", {"number": 7}, CFG + "setdevicepreset=7" + C1)
_ax("osd_menu", {"key": "select"}, CFG + "osdmenu=select" + C1)
_ax("get_position", {}, PTZ + "query=position" + C1,
    http_reply={"status": 200, "body": "pan=-12.5\ntilt=-30\nzoom=1\n"},
    expect_result={"ok": {"kind": "value", "value": "pan=-12.5\ntilt=-30\nzoom=1"}})
_ax("get_limits", {}, PTZ + "query=limits" + C1)
_ax("get_presets", {}, PTZ + "query=presetposcam" + C1)
_ax("get_speed", {}, PTZ + "query=speed" + C1)
_ax("get_mode", {}, PTZ + "query=mode" + C1)
_ax("get_driver", {}, PTZ + "whoami=1" + C1)
_ax("get_commands", {}, PTZ + "info=1" + C1)
_ax("get_attributes", {}, PTZ + "query=attributes&format=json" + C1)
_ax("queue_request", {}, Q + "control=request" + C1)
_ax("queue_drop", {}, Q + "control=drop" + C1)
_ax("queue_query", {}, Q + "control=query" + C1)
_ax("get_ptz_parameters", {}, PRM + "action=list&group=PTZ")
_ax("set_ptz_parameter", {"name": "UserAdv.U1.ImageFreeze", "value": "auto"},
    PRM + "action=update&PTZ.UserAdv.U1.ImageFreeze=auto")
# The API's own example: PTZ.ImageSource.I0.PTZEnabled for channel 1.
_ax("set_ptz_enabled", {"enabled": True}, PRM + "action=update&PTZ.ImageSource.I0.PTZEnabled=true")
_ax("set_ptz_enabled", {"enabled": False}, PRM + "action=update&PTZ.ImageSource.I2.PTZEnabled=false",
    settings={"camera": 3}, file="set_ptz_enabled-channel-3")
_ax("set_locked", {"locked": False}, PRM + "action=update&PTZ.Various.V1.Locked=false")
_ax("set_return_to_overview", {"seconds": 120}, PRM + "action=update&PTZ.Various.V1.ReturnToOverview=120")
_ax("add_memory_zone", {"focus": 4200, "zoom_min": 1111, "zoom_max": 3333},
    MZ + "add.cgi?focus=4200&zoomrange=1111:3333" + C1)
_ax("add_memory_zone_current", {}, MZ + "add.cgi?focus=current&zoomrange=1:19999" + C1)
_ax("remove_memory_zone", {"index": 4}, MZ + "remove.cgi?index=4" + C1)
_ax("remove_all_memory_zones", {}, MZ + "remove.cgi?removeall=1" + C1)
_ax("list_memory_zones", {}, MZ + "list.cgi?query=list" + C1)
_ax("goto_memory_zone", {"index": 2}, MZ + "goto.cgi?index=2" + C1)
# A failure: 200 with "Error:" where a success is 204.
_ax("goto_preset", {"name": "Nowhere"}, PTZ + "gotoserverpresetname=Nowhere" + C1,
    http_reply={"status": 200, "body": "Error:\nNo such preset"},
    expect_result={"error": {"error": "device_error", "code": "200"}}, file="goto_preset-error")

# Telemetry: the reply formats of the Responses section.
telemetry(AX, "position", inbound_http={"path": PTZ + "query=position" + C1, "body": (
    "pan=-12.5\ntilt=-30.25\nzoom=4200\nfocus=7000\niris=3000\nbrightness=4999\nautofocus=on\nautoiris=off\n")},
    expect_state={"position": {"pan": -12.5, "tilt": -30.25, "zoom": 4200, "focus": 7000, "iris": 3000,
                               "brightness": 4999},
                  "autofocus": True, "autoiris": False})
telemetry(AX, "speed", inbound_http={"path": PTZ + "query=speed" + C1, "body": "speed=60\n"},
          expect_state={"speed": 60})
telemetry(AX, "limits", inbound_http={"path": PTZ + "query=limits" + C1, "body": (
    "MinPan=-180\nMaxPan=180\nMinTilt=-90\nMaxTilt=20\nMinZoom=1\nMaxZoom=19999\nMinIris=1\nMaxIris=9999\n"
    "MinFocus=1\nMaxFocus=9999\nMinFieldAngle=22\nMaxFieldAngle=623\nMinBrightness=1\nMaxBrightness=9999\n")},
    expect_state={"limits": {"pan": {"min": -180.0, "max": 180.0}, "tilt": {"min": -90.0, "max": 20.0},
                             "zoom": {"min": 1, "max": 19999}, "iris": {"min": 1, "max": 9999},
                             "focus": {"min": 1, "max": 9999}, "field_angle": {"min": 22, "max": 623},
                             "brightness": {"min": 1, "max": 9999}}})
telemetry(AX, "presets", inbound_http={"path": PTZ + "query=presetposcam" + C1, "body": (
    "Preset Positions for camera 1\npresetposno1=Home\npresetposno2=Pulpit\npresetposno5=Wide shot\n")},
    state_before={"presets": {"9": {"name": "Deleted"}}},
    expect_state={"presets": {"1": {"name": "Home"}, "2": {"name": "Pulpit"}, "5": {"name": "Wide shot"}}})
telemetry(AX, "driver", inbound_http={"path": PTZ + "whoami=1" + C1, "body": "Pelco\n"},
          expect_state={"driver": "Pelco"})
telemetry(AX, "mode", inbound_http={"path": PTZ + "query=mode" + C1, "body": "overview\n"},
          expect_state={"panopsis_mode": "overview"})
telemetry(AX, "queue", inbound_http={"path": Q + "control=query" + C1,
                                     "body": '<a name="3"></a><a name="410"></a><a name="5"></a>'},
          expect_state={"queue": {"position": 3, "seconds": 410, "poll_period": 5}})
telemetry(AX, "parameters", inbound_http={"path": PRM + "action=list&group=PTZ", "body": (
    "root.PTZ.BoaProtPTZOperator=password\nroot.PTZ.ImageSource.I0.PTZEnabled=true\n"
    "root.PTZ.Various.V1.Locked=false\nroot.PTZ.Various.V1.ReturnToOverview=0\n")},
    expect_state={"parameters": {"BoaProtPTZOperator": "password", "ImageSource.I0.PTZEnabled": "true",
                                 "Various.V1.Locked": "false", "Various.V1.ReturnToOverview": "0"}})
telemetry(AX, "memory-zones", inbound_http={"path": MZ + "list.cgi?query=list" + C1, "body": json.dumps([
    {"name": "memory_zone_1", "index": 1, "ptpolygon": "0.0,0.0:1.0,0.0:1.0,1.0:0.0,1.0",
     "zoomrange": "1111:3333", "focus": 4200}])},
    expect_state={"memory_zones": {"1": {"name": "memory_zone_1", "focus": 4200, "zoom_range": "1111:3333",
                                         "pan_tilt_polygon": "0.0,0.0:1.0,0.0:1.0,1.0:0.0,1.0"}}})
