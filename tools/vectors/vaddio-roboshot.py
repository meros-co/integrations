# Vaddio RoboSHOT (vaddio-roboshot): one vector per command, written by hand
# from the Telnet Command Reference of the RoboSHOT manuals (synopses and
# examples). Each command is a line ended by CR LF; the camera answers with
# its data lines, OK and the ">" prompt, which ends the answer.
VD = "vaddio-roboshot"
OKP = "OK\r\n>"


def _vd(command, input, wire, **extra):
    text(VD, command, input, wire if isinstance(wire, list) else wire + "\r\n", **extra)


_vd("home", {}, "camera home", device_reply=OKP, expect_result={"ok": {"kind": "ack"}})
_vd("pan_left", {}, "camera pan left 12")
_vd("pan_right", {"speed": 20}, "camera pan right 20")
_vd("pan_stop", {}, "camera pan stop")
_vd("pan_to", {"degrees": -15}, "camera pan set -15.00")
# The NDI manual's example: camera pan set -15 22.
_vd("pan_to_at_speed", {"degrees": -15, "speed": 22}, "camera pan set -15.00 22")
_vd("get_pan", {}, "camera pan get", device_reply="camera pan get\r\n-15.03\r\nOK\r\n>",
    expect_result={"ok": {"kind": "value", "value": "-15.03"}})
_vd("tilt_up", {}, "camera tilt up 10")
_vd("tilt_down", {"speed": 20}, "camera tilt down 20")
_vd("tilt_stop", {}, "camera tilt stop")
_vd("tilt_to", {"degrees": 5}, "camera tilt set 5.00")
# The NDI manual's example: camera tilt set -5 5.
_vd("tilt_to_at_speed", {"degrees": -5, "speed": 5}, "camera tilt set -5.00 5")
_vd("get_tilt", {}, "camera tilt get")
_vd("zoom_in", {}, "camera zoom in 3")
_vd("zoom_out", {"speed": 7}, "camera zoom out 7")
_vd("zoom_stop", {}, "camera zoom stop")
_vd("zoom_to", {"zoom": 12}, "camera zoom set 12.00")
_vd("zoom_to_at_speed", {"zoom": 14, "speed": 3}, "camera zoom set 14.00 3")
_vd("zoom_to_step", {"zoom": 14}, "camera zoom set 14", model="roboshot-40-uhd")
_vd("get_zoom", {}, "camera zoom get", device_reply="camera zoom get\r\n14\r\nOK\r\n>",
    expect_result={"ok": {"kind": "value", "value": "14"}})
# The NDI manual's example: camera ptz-position pan -15 tilt 5 zoom 1.5 no_wait.
_vd("move_to", {"pan": -15, "tilt": 5, "zoom": 1.5}, "camera ptz-position pan -15.00 tilt 5.00 zoom 1.50")
_vd("move_to_no_wait", {"pan": -15, "tilt": 5, "zoom": 1.5},
    "camera ptz-position pan -15.00 tilt 5.00 zoom 1.50 no_wait")
_vd("focus_near", {}, "camera focus near 4")
_vd("focus_far", {"speed": 7}, "camera focus far 7")
_vd("focus_stop", {}, "camera focus stop")
_vd("focus_auto", {}, ["camera focus mode auto\r\n", "camera focus mode get\r\n"], device_reply=OKP)
_vd("focus_manual", {}, ["camera focus mode manual\r\n", "camera focus mode get\r\n"], device_reply=OKP)
_vd("get_focus_mode", {}, "camera focus mode get", device_reply="camera focus mode get\r\nauto_focus: on\r\nOK\r\n>",
    expect_result={"ok": {"kind": "value", "value": "on"}})
_vd("recall_preset", {"preset": 3}, "camera preset recall 3")
_vd("store_preset", {"preset": 1}, "camera preset store 1")
_vd("store_preset_tri_sync", {"preset": 4, "speed": 15}, "camera preset store 4 tri-sync 15")
_vd("store_preset_with_ccu", {"preset": 2}, "camera preset store 2 save-ccu")
_vd("store_preset_tri_sync_with_ccu", {"preset": 16, "speed": 24}, "camera preset store 16 tri-sync 24 save-ccu")
_vd("recall_factory_scene", {"scene": 2}, ["camera ccu scene recall factory 2\r\n", "camera ccu get all\r\n"],
    device_reply=OKP)
_vd("recall_custom_scene", {"scene": 3}, ["camera ccu scene recall custom 3\r\n", "camera ccu get all\r\n"],
    device_reply=OKP)
_vd("store_custom_scene", {"scene": 1}, "camera ccu scene store custom 1")
_vd("get_ccu", {"setting": "red_gain"}, "camera ccu get red_gain")
_vd("get_ccu_all", {}, "camera ccu get all")
_vd("set_auto_white_balance", {"enabled": False},
    ["camera ccu set auto_white_balance off\r\n", "camera ccu get auto_white_balance\r\n"], device_reply=OKP)
# The manual's example: camera ccu set red_gain 10.
_vd("set_red_gain", {"gain": 10}, ["camera ccu set red_gain 10\r\n", "camera ccu get red_gain\r\n"], device_reply=OKP)
_vd("set_blue_gain", {"gain": 193}, ["camera ccu set blue_gain 193\r\n", "camera ccu get blue_gain\r\n"],
    device_reply=OKP)
_vd("set_backlight_compensation", {"enabled": True},
    ["camera ccu set backlight_compensation on\r\n", "camera ccu get backlight_compensation\r\n"], device_reply=OKP)
# The manual's example: camera ccu set auto_iris off.
_vd("set_auto_iris", {"enabled": False}, ["camera ccu set auto_iris off\r\n", "camera ccu get auto_iris\r\n"],
    device_reply=OKP)
_vd("set_iris", {"iris": 11}, ["camera ccu set iris 11\r\n", "camera ccu get iris\r\n"], device_reply=OKP,
    model="roboshot-40-uhd")
_vd("set_iris_ndi", {"iris": 13}, ["camera ccu set iris 13\r\n", "camera ccu get iris\r\n"], device_reply=OKP)
_vd("set_gain", {"gain": 0}, ["camera ccu set gain 0\r\n", "camera ccu get gain\r\n"], device_reply=OKP,
    model="roboshot-40-uhd")
_vd("set_gain_ndi", {"gain": 1}, ["camera ccu set gain 1\r\n", "camera ccu get gain\r\n"], device_reply=OKP)
_vd("set_detail", {"detail": 8}, ["camera ccu set detail 8\r\n", "camera ccu get detail\r\n"], device_reply=OKP)
_vd("set_chroma", {"chroma": 2}, ["camera ccu set chroma 2\r\n", "camera ccu get chroma\r\n"], device_reply=OKP)
_vd("set_gamma", {"gamma": -64}, ["camera ccu set gamma -64\r\n", "camera ccu get gamma\r\n"], device_reply=OKP)
_vd("set_wide_dynamic_range", {"enabled": True},
    ["camera ccu set wide_dynamic_range on\r\n", "camera ccu get wide_dynamic_range\r\n"], device_reply=OKP)
_vd("set_led", {"enabled": False}, ["camera led off\r\n", "camera led get\r\n"], device_reply=OKP)
_vd("get_led", {}, "camera led get", device_reply="camera led get\r\nled: on\r\nOK\r\n>",
    expect_result={"ok": {"kind": "value", "value": "on"}})
_vd("standby_on", {}, ["camera standby on\r\n", "camera standby get\r\n"], device_reply=OKP)
_vd("standby_off", {}, ["camera standby off\r\n", "camera standby get\r\n"], device_reply=OKP)
_vd("standby_toggle", {}, ["camera standby toggle\r\n", "camera standby get\r\n"], device_reply=OKP)
_vd("get_standby", {}, "camera standby get")
_vd("video_mute_on", {}, ["video mute on\r\n", "video mute get\r\n"], device_reply=OKP, model="roboshot-40-uhd")
_vd("video_mute_off", {}, ["video mute off\r\n", "video mute get\r\n"], device_reply=OKP, model="roboshot-40-uhd")
_vd("video_mute_toggle", {}, ["video mute toggle\r\n", "video mute get\r\n"], device_reply=OKP,
    model="roboshot-40-uhd")
_vd("get_video_mute", {}, "video mute get", model="roboshot-40-uhd")
_vd("set_glass_mode", {"mode": "manual"}, ["camera glass mode set manual\r\n", "camera glass mode get\r\n"],
    device_reply=OKP, model="roboshot-iw")
_vd("toggle_glass_mode", {}, ["camera glass mode toggle\r\n", "camera glass mode get\r\n"], device_reply=OKP,
    model="roboshot-iw")
_vd("get_glass_mode", {}, "camera glass mode get", model="roboshot-iw")
# The IW manual's example: camera glass state set frosted.
_vd("set_glass_state", {"state": "frosted"}, ["camera glass state set frosted\r\n", "camera glass state get\r\n"],
    device_reply=OKP, model="roboshot-iw")
_vd("toggle_glass_state", {}, ["camera glass state toggle\r\n", "camera glass state get\r\n"], device_reply=OKP,
    model="roboshot-iw")
_vd("get_glass_state", {}, "camera glass state get", model="roboshot-iw",
    device_reply="camera glass state get\r\nfrosted\r\nOK\r\n>", expect_result={"ok": {"kind": "value", "value": "frosted"}})
# The IW manual's example: camera resolution set 1080p/30.
_vd("set_resolution", {"resolution": "1080p/30"}, ["camera resolution set 1080p/30\r\n", "camera resolution get\r\n"],
    device_reply=OKP, model="roboshot-iw")
_vd("get_resolution", {}, "camera resolution get", model="roboshot-iw",
    device_reply='camera resolution get\r\n"720p/59.94"\r\n>', expect_result={"ok": {"kind": "value", "value": "720p/59.94"}})
_vd("recalibrate", {}, "camera recalibrate", model="roboshot-iw")
_vd("get_streaming_settings", {}, "streaming settings get", model="roboshot-40-uhd")
_vd("streaming_on", {}, "streaming ip enable on", model="roboshot-40-uhd")
_vd("streaming_off", {}, "streaming ip enable off", model="roboshot-40-uhd")
_vd("streaming_toggle", {}, "streaming ip enable toggle", model="roboshot-40-uhd")
_vd("get_streaming", {}, "streaming ip enable get", model="roboshot-40-uhd")
_vd("get_network_settings", {}, "network settings get")
_vd("ping", {"address": "192.168.1.66"}, "network ping 192.168.1.66")
_vd("reboot", {}, "system reboot")
_vd("reboot_in", {"seconds": 30}, "system reboot 30")
_vd("get_factory_reset", {}, "system factory-reset get")
_vd("set_factory_reset", {"enabled": True}, "system factory-reset on")
_vd("get_version", {}, "version")

# Telemetry: the manuals' example answers, as the camera sends them up to its
# prompt.
telemetry(VD, "pan", request="camera pan get", inbound="camera pan get\r\n-15.03\r\nOK\r\n>",
          expect_state={"position": {"pan": -15.03}})
telemetry(VD, "tilt", request="camera tilt get", inbound="camera tilt get\r\n5.5\r\nOK\r\n>",
          expect_state={"position": {"tilt": 5.5}})
telemetry(VD, "zoom", request="camera zoom get", inbound="camera zoom get\r\n14\r\nOK\r\n>",
          expect_state={"position": {"zoom": 14.0}})
telemetry(VD, "ccu-all", inbound=(
    "camera ccu get all\r\nauto_iris                      on\r\nauto_white_balance             on\r\n"
    "backlight_compensation off\r\nblue_gain                      193\r\nchroma                         2\r\n"
    "detail                         8\r\ngain                           3\r\niris                           11\r\n"
    "red_gain                       201\r\nwide_dynamic_range             off\r\ngamma                          -4\r\nOK\r\n>"),
    expect_state={"ccu": {"auto_iris": True, "auto_white_balance": True, "backlight_compensation": False,
                          "blue_gain": 193, "chroma": 2, "detail": 8, "gain": 3, "iris": 11, "red_gain": 201,
                          "wide_dynamic_range": False, "gamma": -4}})
telemetry(VD, "ccu-iris", inbound="camera ccu get iris\r\niris 6\r\nOK\r\n>", expect_state={"ccu": {"iris": 6}})
telemetry(VD, "focus-mode", inbound="camera focus mode get\r\nauto_focus: on\r\nOK\r\n>",
          expect_state={"auto_focus": True})
telemetry(VD, "led", inbound="camera led get\r\nled: off\r\nOK\r\n>", expect_state={"led": False})
telemetry(VD, "standby", inbound="camera standby get\r\nstandby:         on\r\nOK\r\n>", expect_state={"standby": True})
telemetry(VD, "video-mute", inbound="video mute get\r\nmute: off\r\n>", expect_state={"video_mute": False})
telemetry(VD, "streaming-enabled", inbound="streaming ip enable get\r\nenabled: true\r\n>",
          expect_state={"streaming": {"enabled": True}})
telemetry(VD, "streaming-settings", inbound=(
    "streaming settings get\r\nIP Custom_Frame_Rate 30\r\nIP Custom_Resolution 1080p\r\nIP Enabled              true\r\n"
    "IP MTU                  1400\r\nIP Port                 554\r\nIP Preset_Quality       Standard (Better)\r\n"
    "IP Preset_Resolution 720p\r\nIP Protocol             RTSP\r\nIP URL                  vaddio-roboshot-uhd-stream\r\n"
    "IP Video_Mode           preset\r\nOK\r\n>"),
    expect_state={"streaming": {"custom_frame_rate": "30", "custom_resolution": "1080p", "enabled": True, "mtu": 1400,
                                "port": 554, "preset_quality": "Standard (Better)", "preset_resolution": "720p",
                                "protocol": "RTSP", "url": "vaddio-roboshot-uhd-stream", "video_mode": "preset"}})
telemetry(VD, "network", inbound=(
    "network settings get\r\nName        eth0:WAN\r\nMAC Address 48:6F:77:64:79:21\r\nIP Address  192.168.1.67\r\n"
    "Netmask     255.255.255.0\r\nVLAN        Disabled\r\nGateway     192.168.1.254\r\nOK\r\n>"),
    expect_state={"network": {"mac": "48:6F:77:64:79:21", "ip": "192.168.1.67", "netmask": "255.255.255.0",
                              "vlan": "Disabled", "gateway": "192.168.1.254"}})
telemetry(VD, "factory-reset", inbound=(
    "system factory-reset get\r\nfactory-reset (software):  off\r\nfactory-reset (hardware):  off\r\nOK\r\n>"),
    expect_state={"factory_reset": {"software": False, "hardware": False}})
telemetry(VD, "version", inbound=(
    "version\r\nCommit                        d033ddb2378357a871011eb820706dcaa64ec0e2\r\nHDLink                       TX4.6.1*0.03\r\n"
    "Pan Motor Version            0.32.6214\r\nSensor Version               01.20\r\n"
    "System Version               RoboSHOT 20 UHD 1.3.0\r\nTilt Motor Version           0.3.6214\r\nOK\r\n>"),
    expect_state={"version": {"hdlink": "TX4.6.1*0.03", "pan_motor": "0.32.6214", "sensor": "01.20",
                              "system": "RoboSHOT 20 UHD 1.3.0", "tilt_motor": "0.3.6214"}})
telemetry(VD, "glass-state", model="roboshot-iw", request="camera glass state get",
          inbound="camera glass state get\r\nfrosted\r\nOK\r\n>", expect_state={"glass": {"state": "frosted"}})
telemetry(VD, "glass-mode", model="roboshot-iw", request="camera glass mode get",
          inbound="camera glass mode get\r\nmanual\r\nOK\r\n>", expect_state={"glass": {"mode": "manual"}})
telemetry(VD, "resolution", model="roboshot-iw", request="camera resolution get",
          inbound='camera resolution get\r\n"720p/59.94"\r\n>', expect_state={"resolution": "720p/59.94"})
