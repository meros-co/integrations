# Blackmagic cameras (blackmagic-camera): one vector per command, over the
# Camera Control REST API. Targets are the endpoints of Blackmagic's "REST API
# for Blackmagic Cameras" (Developer Information, August 2025) under the base
# path /control/api/v1 (p.2), written here from the document's endpoint
# headings; bodies are the operation's documented JSON parameters, compact,
# with only the fields the command sets. Path parameters are percent-encoded
# as one segment; audio channels are 1-based in the input and 0-based on the
# wire ("Channels index from 0", p.45). Reply bodies are shaped from each
# operation's documented response table; the document prints no examples.
BC = "blackmagic-camera"
_BA = "/control/api/v1"
_ACK = {"ok": {"kind": "ack"}}


def _bc(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": _BA + target}
    if body is not None:
        request["body"] = body
    V.append({"spec": BC, "command": command, "input": input, "expect_request": request, **extra})


def _err(code):
    return {"error": {"error": "device_error", "code": str(code)}}


def _val(v):
    return {"ok": {"kind": "value", "value": v}}


# Transport (p.27-30)
_bc("get_transport_mode", {}, "GET", "/transports/0",
    http_reply={"status": 200, "body": '{"mode":"InputRecord"}'}, expect_result=_val("InputRecord"))
_bc("set_transport_mode", {"mode": "Output"}, "PUT", "/transports/0", '{"mode":"Output"}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_transport_stopped", {}, "GET", "/transports/0/stop",
    http_reply={"status": 200, "body": "true"}, expect_result=_val(True))
_bc("stop", {}, "POST", "/transports/0/stop", http_reply={"status": 204}, expect_result=_ACK)
_bc("get_transport_playing", {}, "GET", "/transports/0/play",
    http_reply={"status": 200, "body": "false"}, expect_result=_val(False))
_bc("play", {}, "POST", "/transports/0/play", http_reply={"status": 400}, expect_result=_err(400))
_bc("get_playback", {}, "GET", "/transports/0/playback",
    http_reply={"status": 200, "body": '{"type":"Play","loop":false,"singleClip":false,"speed":1.0,"position":120}'},
    expect_result=_val({"type": "Play", "loop": False, "singleClip": False, "speed": 1.0, "position": 120}))
_bc("set_playback", {"body": {"type": "Shuttle", "speed": -2.0}}, "PUT", "/transports/0/playback",
    '{"type":"Shuttle","speed":-2.0}')
_bc("set_playback_type", {"type": "Jog"}, "PUT", "/transports/0/playback", '{"type":"Jog"}')
_bc("set_playback_loop", {"loop": True}, "PUT", "/transports/0/playback", '{"loop":true}')
_bc("set_playback_single_clip", {"single_clip": False}, "PUT", "/transports/0/playback", '{"singleClip":false}')
_bc("set_playback_speed", {"speed": -0.5}, "PUT", "/transports/0/playback", '{"speed":-0.500}',
    http_reply={"status": 204}, expect_result=_ACK)
_bc("set_playback_position", {"position": 1500}, "PUT", "/transports/0/playback", '{"position":1500}')
_bc("get_recording", {}, "GET", "/transports/0/record",
    http_reply={"status": 200, "body": '{"recording":true}'}, expect_result=_val(True))
_bc("record", {}, "POST", "/transports/0/record", http_reply={"status": 204}, expect_result=_ACK)
_bc("record_named", {"clip_name": 'Scene "4"'}, "POST", "/transports/0/record", '{"clipName":"Scene \\"4\\""}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("set_recording", {"recording": False}, "PUT", "/transports/0/record", '{"recording":false}')
_bc("get_clip_index", {}, "GET", "/transports/0/clipIndex",
    http_reply={"status": 200, "body": '{"clipIndex":2}'}, expect_result=_val(2))
_bc("get_timecode", {}, "GET", "/transports/0/timecode",
    http_reply={"status": 200, "body": '{"display":"10:00:01:12","timeline":"00:00:01:12"}'},
    expect_result=_val({"display": "10:00:01:12", "timeline": "00:00:01:12"}))
_bc("get_timecode_source", {}, "GET", "/transports/0/timecode/source",
    http_reply={"status": 200, "body": '{"timecode":"Clip"}'}, expect_result=_val("Clip"))

# Timeline (p.31-34)
_bc("get_timeline", {}, "GET", "/timelines/0", http_reply={"status": 404}, expect_result=_err(404))
_bc("add_timeline_clips", {"body": {"insertBefore": 0, "clips": [{"clipUniqueId": 7, "clipIn": 25, "frameCount": 100}]}},
    "POST", "/timelines/0", '{"insertBefore":0,"clips":[{"clipUniqueId":7,"clipIn":25,"frameCount":100}]}')
_bc("append_timeline_clip", {"clip_id": 7}, "POST", "/timelines/0", '{"clips":7}',
    http_reply={"status": 501}, expect_result=_err(501))
_bc("insert_timeline_clip", {"insert_before": 2, "clip_id": 9}, "POST", "/timelines/0", '{"insertBefore":2,"clips":9}')
_bc("clear_timeline", {}, "POST", "/timelines/0/clear", http_reply={"status": 204}, expect_result=_ACK)
_bc("remove_timeline_clip", {"index": 3}, "DELETE", "/timelines/0/clips/3")

# Clips and media (p.8-9, p.34-37)
_bc("get_clips", {}, "GET", "/clips", http_reply={"status": 404}, expect_result=_err(404))
_bc("get_working_set", {}, "GET", "/media/workingset")
_bc("get_active_media", {}, "GET", "/media/active",
    http_reply={"status": 204}, expect_result=_err(204))
_bc("set_active_media", {"index": 1}, "PUT", "/media/active", '{"workingsetIndex":1}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_format_filesystems", {}, "GET", "/media/devices/doformatSupportedFilesystems",
    http_reply={"status": 200, "body": '["exFAT","HFS+"]'}, expect_result=_val(["exFAT", "HFS+"]))
_bc("get_media_device_state", {"device": "cfast1"}, "GET", "/media/devices/cfast1",
    http_reply={"status": 200, "body": '{"state":"Mounted"}'}, expect_result=_val("Mounted"))
_bc("prepare_format", {"device": "cfast1"}, "GET", "/media/devices/cfast1/doformat",
    http_reply={"status": 200, "body": '{"deviceName":"cfast1","key":"a1b2c3"}'}, expect_result=_val("a1b2c3"))
_bc("format_media", {"device": "cfast1", "key": "a1b2c3", "filesystem": "exFAT", "volume": "A001"},
    "PUT", "/media/devices/cfast1/doformat", '{"key":"a1b2c3","filesystem":"exFAT","volume":"A001"}',
    http_reply={"status": 400}, expect_result=_err(400))

# Lens (p.50-54)
_bc("get_iris", {}, "GET", "/lens/iris",
    http_reply={"status": 200, "body": '{"continuousApertureAutoExposure":false,"apertureStop":2.8,"normalised":0.25,"apertureNumber":3}'},
    expect_result=_val({"continuousApertureAutoExposure": False, "apertureStop": 2.8, "normalised": 0.25, "apertureNumber": 3}))
_bc("set_iris_stop", {"stop": 5.6}, "PUT", "/lens/iris", '{"apertureStop":5.6}',
    http_reply={"status": 403}, expect_result=_err(403))
_bc("set_iris_normalised", {"value": 0.5}, "PUT", "/lens/iris", '{"normalised":0.500}',
    http_reply={"status": 204}, expect_result=_ACK)
_bc("set_iris_number", {"number": 4}, "PUT", "/lens/iris", '{"apertureNumber":4}')
_bc("adjust_iris", {"steps": -1}, "PUT", "/lens/iris", '{"adjustmentStep":-1}')
_bc("get_iris_description", {}, "GET", "/lens/iris/description")
_bc("get_zoom", {}, "GET", "/lens/zoom")
_bc("set_zoom_focal_length", {"mm": 35}, "PUT", "/lens/zoom", '{"focalLength":35}',
    http_reply={"status": 403}, expect_result=_err(403))
_bc("set_zoom_normalised", {"value": 0.0}, "PUT", "/lens/zoom", '{"normalised":0.000}')
_bc("adjust_zoom_focal_length", {"mm": -5}, "PUT", "/lens/zoom", '{"adjustmentFocalLength":-5}')
_bc("adjust_zoom_normalised", {"amount": 0.05}, "PUT", "/lens/zoom", '{"adjustmentNormalised":0.050}')
_bc("get_zoom_description", {}, "GET", "/lens/zoom/description")
_bc("get_focus", {}, "GET", "/lens/focus",
    http_reply={"status": 200, "body": '{"normalised":0.75}'}, expect_result=_val(0.75))
_bc("set_focus_normalised", {"value": 0.125}, "PUT", "/lens/focus", '{"normalised":0.125}')
_bc("set_focus_distance", {"distance": 3000}, "PUT", "/lens/focus", '{"focusDistance":3000}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("auto_focus", {}, "PUT", "/lens/focus/doAutoFocus", http_reply={"status": 204}, expect_result=_ACK)
_bc("auto_focus_at", {"x": 0.5, "y": 0.25}, "PUT", "/lens/focus/doAutoFocus", '{"position":{"x":0.500,"y":0.250}}')
_bc("get_focus_description", {}, "GET", "/lens/focus/description")
_bc("get_optical_image_stabilization", {}, "GET", "/lens/opticalImageStabilization",
    http_reply={"status": 501}, expect_result=_err(501))
_bc("set_optical_image_stabilization", {"enabled": True}, "PUT", "/lens/opticalImageStabilization", '{"enabled":true}')

# Video (p.54-62)
_bc("get_iso", {}, "GET", "/video/iso", http_reply={"status": 200, "body": '{"iso":800}'}, expect_result=_val(800))
_bc("set_iso", {"iso": 3200}, "PUT", "/video/iso", '{"iso":3200}', http_reply={"status": 403}, expect_result=_err(403))
_bc("get_supported_isos", {}, "GET", "/video/supportedISOs",
    http_reply={"status": 200, "body": '{"supportedISOs":[400,800,3200]}'}, expect_result=_val([400, 800, 3200]))
_bc("get_gain", {}, "GET", "/video/gain", http_reply={"status": 200, "body": '{"gain":-6}'}, expect_result=_val(-6))
_bc("set_gain", {"db": 12}, "PUT", "/video/gain", '{"gain":12}', http_reply={"status": 204}, expect_result=_ACK)
_bc("get_supported_gains", {}, "GET", "/video/supportedGains")
_bc("get_white_balance", {}, "GET", "/video/whiteBalance",
    http_reply={"status": 200, "body": '{"whiteBalance":5600}'}, expect_result=_val(5600))
_bc("set_white_balance", {"kelvin": 3200}, "PUT", "/video/whiteBalance", '{"whiteBalance":3200}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_white_balance_range", {}, "GET", "/video/whiteBalance/description",
    http_reply={"status": 200, "body": '{"whiteBalance":{"min":2500,"max":10000}}'},
    expect_result=_val({"min": 2500, "max": 10000}))
_bc("auto_white_balance", {}, "PUT", "/video/whiteBalance/doAuto", http_reply={"status": 204}, expect_result=_ACK)
_bc("get_white_balance_tint", {}, "GET", "/video/whiteBalanceTint")
_bc("set_white_balance_tint", {"tint": -10}, "PUT", "/video/whiteBalanceTint", '{"whiteBalanceTint":-10}')
_bc("get_white_balance_tint_range", {}, "GET", "/video/whiteBalanceTint/description")
_bc("get_nd_filter", {}, "GET", "/video/ndFilter", http_reply={"status": 200, "body": '{"stop":2.0}'}, expect_result=_val(2.0))
_bc("set_nd_filter", {"stop": 4.0}, "PUT", "/video/ndFilter", '{"stop":4.0}', http_reply={"status": 501}, expect_result=_err(501))
_bc("get_supported_nd_filters", {}, "GET", "/video/supportedNDFilters")
_bc("get_supported_nd_display_modes", {}, "GET", "/video/supportedNDFilterDisplayModes")
_bc("get_nd_display_mode", {}, "GET", "/video/ndFilter/displayMode")
_bc("set_nd_display_mode", {"mode": "Fraction"}, "PUT", "/video/ndFilter/displayMode", '{"displayMode":"Fraction"}')
_bc("get_nd_selectable", {}, "GET", "/video/ndFilterSelectable")
_bc("get_shutter", {}, "GET", "/video/shutter")
_bc("set_shutter_speed", {"speed": 50}, "PUT", "/video/shutter", '{"shutterSpeed":50}',
    http_reply={"status": 204}, expect_result=_ACK)
_bc("set_shutter_angle", {"angle": 172.8}, "PUT", "/video/shutter", '{"shutterAngle":172.8}')
_bc("get_shutter_measurement", {}, "GET", "/video/shutter/measurement")
_bc("set_shutter_measurement", {"measurement": "ShutterAngle"}, "PUT", "/video/shutter/measurement",
    '{"measurement":"ShutterAngle"}', http_reply={"status": 400}, expect_result=_err(400))
_bc("get_supported_shutters", {}, "GET", "/video/supportedShutters")
_bc("get_flicker_free_shutters", {}, "GET", "/video/flickerFreeShutters")
_bc("get_auto_exposure", {}, "GET", "/video/autoExposure")
_bc("set_auto_exposure_mode", {"mode": "Continuous"}, "PUT", "/video/autoExposure", '{"mode":"Continuous"}')
_bc("set_auto_exposure", {"mode": "OneShot", "type": "Iris,Shutter"}, "PUT", "/video/autoExposure",
    '{"mode":"OneShot","type":"Iris,Shutter"}', http_reply={"status": 400}, expect_result=_err(400))
_bc("get_detail_sharpening", {}, "GET", "/video/detailSharpening")
_bc("set_detail_sharpening", {"enabled": False}, "PUT", "/video/detailSharpening", '{"enabled":false}')
_bc("get_detail_sharpening_level", {}, "GET", "/video/detailSharpeningLevel")
_bc("set_detail_sharpening_level", {"level": "High"}, "PUT", "/video/detailSharpeningLevel", '{"level":"High"}')

# Colour correction (p.66-70)
_bc("get_color_lift", {}, "GET", "/colorCorrection/lift")
_bc("set_color_lift", {"red": 0.1, "green": 0.0, "blue": -0.1, "luma": 0.0}, "PUT", "/colorCorrection/lift",
    '{"red":0.100,"green":0.000,"blue":-0.100,"luma":0.000}', http_reply={"status": 204}, expect_result=_ACK)
_bc("set_color_lift_component", {"component": "blue", "value": -0.05}, "PUT", "/colorCorrection/lift", '{"blue":-0.050}')
_bc("get_color_gamma", {}, "GET", "/colorCorrection/gamma")
_bc("set_color_gamma", {"red": 0.0, "green": 0.0, "blue": 0.0, "luma": 0.25}, "PUT", "/colorCorrection/gamma",
    '{"red":0.000,"green":0.000,"blue":0.000,"luma":0.250}')
_bc("set_color_gamma_component", {"component": "luma", "value": 0.2}, "PUT", "/colorCorrection/gamma", '{"luma":0.200}')
_bc("get_color_gain", {}, "GET", "/colorCorrection/gain")
_bc("set_color_gain", {"red": 1.0, "green": 1.0, "blue": 1.25, "luma": 1.0}, "PUT", "/colorCorrection/gain",
    '{"red":1.000,"green":1.000,"blue":1.250,"luma":1.000}')
_bc("set_color_gain_component", {"component": "red", "value": 1.5}, "PUT", "/colorCorrection/gain", '{"red":1.500}')
_bc("get_color_offset", {}, "GET", "/colorCorrection/offset")
_bc("set_color_offset", {"red": 0.0, "green": 0.0, "blue": 0.0, "luma": -0.5}, "PUT", "/colorCorrection/offset",
    '{"red":0.000,"green":0.000,"blue":0.000,"luma":-0.500}')
_bc("set_color_offset_component", {"component": "green", "value": 0.01}, "PUT", "/colorCorrection/offset", '{"green":0.010}')
_bc("get_color_contrast", {}, "GET", "/colorCorrection/contrast")
_bc("set_color_contrast", {"pivot": 0.5, "adjust": 1.1}, "PUT", "/colorCorrection/contrast", '{"pivot":0.500,"adjust":1.100}')
_bc("set_color_contrast_pivot", {"pivot": 0.45}, "PUT", "/colorCorrection/contrast", '{"pivot":0.450}')
_bc("set_color_contrast_adjust", {"adjust": 1.0}, "PUT", "/colorCorrection/contrast", '{"adjust":1.000}')
_bc("get_color_hue_saturation", {}, "GET", "/colorCorrection/color")
_bc("set_color_hue_saturation", {"hue": 0.0, "saturation": 1.2}, "PUT", "/colorCorrection/color", '{"hue":0.000,"saturation":1.200}')
_bc("set_color_hue", {"hue": -0.25}, "PUT", "/colorCorrection/color", '{"hue":-0.250}')
_bc("set_color_saturation", {"saturation": 0.0}, "PUT", "/colorCorrection/color", '{"saturation":0.000}')
_bc("get_color_luma_contribution", {}, "GET", "/colorCorrection/lumaContribution",
    http_reply={"status": 200, "body": '{"lumaContribution":1.0}'}, expect_result=_val(1.0))
_bc("set_color_luma_contribution", {"value": 0.5}, "PUT", "/colorCorrection/lumaContribution", '{"lumaContribution":0.500}')

# Camera (p.62-65)
_bc("get_color_bars", {}, "GET", "/camera/colorBars", http_reply={"status": 200, "body": '{"enabled":false}'},
    expect_result=_val(False))
_bc("set_color_bars", {"enabled": True}, "PUT", "/camera/colorBars", '{"enabled":true}',
    http_reply={"status": 204}, expect_result=_ACK)
_bc("get_program_feed_display", {}, "GET", "/camera/programFeedDisplay")
_bc("set_program_feed_display", {"enabled": False}, "PUT", "/camera/programFeedDisplay", '{"enabled":false}')
_bc("get_tally", {}, "GET", "/camera/tallyStatus", http_reply={"status": 200, "body": '{"status":"Program"}'},
    expect_result=_val("Program"))
_bc("get_power", {}, "GET", "/camera/power")
_bc("get_power_display_mode", {}, "GET", "/camera/power/displayMode")
_bc("set_power_display_mode", {"mode": "Voltage"}, "PUT", "/camera/power/displayMode", '{"mode":"Voltage"}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_timing_reference_lock", {}, "GET", "/camera/timingReferenceLock",
    http_reply={"status": 200, "body": '{"locked":true}'}, expect_result=_val(True))

# Presets (p.43-45)
_bc("get_presets", {}, "GET", "/presets", http_reply={"status": 200, "body": '{"presets":["Studio A.cset"]}'},
    expect_result=_val(["Studio A.cset"]))
_bc("get_active_preset", {}, "GET", "/presets/active", http_reply={"status": 200, "body": '{"preset":"default"}'},
    expect_result=_val("default"))
_bc("set_active_preset", {"preset": "Studio A.cset"}, "PUT", "/presets/active", '{"preset":"Studio A.cset"}',
    http_reply={"status": 404}, expect_result=_err(404))
_bc("save_preset", {"preset": "Studio A"}, "PUT", "/presets/Studio%20A", http_reply={"status": 204}, expect_result=_ACK)
_bc("delete_preset", {"preset": "Studio A.cset"}, "DELETE", "/presets/Studio%20A.cset",
    http_reply={"status": 404}, expect_result=_err(404))

# Audio (p.45-50)
_bc("get_audio_channel_count", {}, "GET", "/audio/channels", http_reply={"status": 200, "body": '{"channels":4}'},
    expect_result=_val(4))
_bc("get_supported_audio_inputs", {}, "GET", "/audio/supportedInputs")
_bc("get_audio_input", {"channel": 1}, "GET", "/audio/channel/0/input",
    http_reply={"status": 200, "body": '{"input":"XLR Mic"}'}, expect_result=_val("XLR Mic"))
_bc("set_audio_input", {"channel": 2, "input": "XLR Line"}, "PUT", "/audio/channel/1/input", '{"input":"XLR Line"}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_audio_input_description", {"channel": 1}, "GET", "/audio/channel/0/input/description")
_bc("get_audio_channel_inputs", {"channel": 3}, "GET", "/audio/channel/2/supportedInputs",
    http_reply={"status": 404}, expect_result=_err(404))
_bc("get_audio_level", {"channel": 1}, "GET", "/audio/channel/0/level")
_bc("set_audio_gain", {"channel": 1, "db": -12.5}, "PUT", "/audio/channel/0/level", '{"gain":-12.50}',
    http_reply={"status": 204}, expect_result=_ACK)
_bc("set_audio_level", {"channel": 2, "level": 0.8}, "PUT", "/audio/channel/1/level", '{"normalised":0.800}')
_bc("get_audio_phantom_power", {"channel": 1}, "GET", "/audio/channel/0/phantomPower")
_bc("set_audio_phantom_power", {"channel": 1, "enabled": True}, "PUT", "/audio/channel/0/phantomPower", '{"enabled":true}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_audio_padding", {"channel": 2}, "GET", "/audio/channel/1/padding")
_bc("set_audio_padding", {"channel": 2, "enabled": False}, "PUT", "/audio/channel/1/padding", '{"enabled":false}')
_bc("get_audio_low_cut", {"channel": 1}, "GET", "/audio/channel/0/lowCutFilter")
_bc("set_audio_low_cut", {"channel": 4, "enabled": True}, "PUT", "/audio/channel/3/lowCutFilter", '{"enabled":true}')
_bc("get_audio_available", {"channel": 1}, "GET", "/audio/channel/0/available",
    http_reply={"status": 200, "body": '{"available":false}'}, expect_result=_val(False))

# Monitoring (p.12-20)
_bc("get_displays", {}, "GET", "/monitoring/display",
    http_reply={"status": 200, "body": '{"displays":["LCD","HDMI","SDI"]}'}, expect_result=_val(["LCD", "HDMI", "SDI"]))
_bc("get_display_clean_feed", {"display": "HDMI"}, "GET", "/monitoring/HDMI/cleanFeed",
    http_reply={"status": 404}, expect_result=_err(404))
_bc("set_display_clean_feed", {"display": "HDMI", "enabled": True}, "PUT", "/monitoring/HDMI/cleanFeed", '{"enabled":true}',
    http_reply={"status": 422}, expect_result=_err(422))
_bc("get_display_display_lut", {"display": "LCD"}, "GET", "/monitoring/LCD/displayLUT")
_bc("set_display_display_lut", {"display": "LCD", "enabled": False}, "PUT", "/monitoring/LCD/displayLUT", '{"enabled":false}')
_bc("get_display_zebra", {"display": "LCD"}, "GET", "/monitoring/LCD/zebra",
    http_reply={"status": 200, "body": '{"enabled":true}'}, expect_result=_val(True))
_bc("set_display_zebra", {"display": "LCD", "enabled": True}, "PUT", "/monitoring/LCD/zebra", '{"enabled":true}',
    http_reply={"status": 204}, expect_result=_ACK)
_bc("get_display_focus_assist", {"display": "LCD"}, "GET", "/monitoring/LCD/focusAssist")
_bc("set_display_focus_assist", {"display": "LCD", "body": {"enabled": True}}, "PUT", "/monitoring/LCD/focusAssist",
    '{"enabled":true}')
_bc("get_display_frame_guide", {"display": "SDI"}, "GET", "/monitoring/SDI/frameGuide")
_bc("set_display_frame_guide", {"display": "SDI", "enabled": True}, "PUT", "/monitoring/SDI/frameGuide", '{"enabled":true}')
_bc("get_display_frame_grids", {"display": "SDI"}, "GET", "/monitoring/SDI/frameGrids")
_bc("set_display_frame_grids", {"display": "SDI", "enabled": False}, "PUT", "/monitoring/SDI/frameGrids", '{"enabled":false}')
_bc("get_display_safe_area", {"display": "Main LCD"}, "GET", "/monitoring/Main%20LCD/safeArea")
_bc("set_display_safe_area", {"display": "LCD", "enabled": True}, "PUT", "/monitoring/LCD/safeArea", '{"enabled":true}')
_bc("get_display_false_color", {"display": "LCD"}, "GET", "/monitoring/LCD/falseColor")
_bc("set_display_false_color", {"display": "LCD", "enabled": True}, "PUT", "/monitoring/LCD/falseColor", '{"enabled":true}')
_bc("get_focus_assist", {}, "GET", "/monitoring/focusAssist")
_bc("set_focus_assist", {"mode": "ColoredLines", "color": "Red", "intensity": 75}, "PUT", "/monitoring/focusAssist",
    '{"mode":"ColoredLines","color":"Red","intensity":75}', http_reply={"status": 400}, expect_result=_err(400))
_bc("get_frame_guide_ratio", {}, "GET", "/monitoring/frameGuideRatio",
    http_reply={"status": 200, "body": '{"ratio":"2.39:1"}'}, expect_result=_val("2.39:1"))
_bc("set_frame_guide_ratio", {"ratio": "1.85:1"}, "PUT", "/monitoring/frameGuideRatio", '{"ratio":"1.85:1"}')
_bc("get_frame_guide_ratio_presets", {}, "GET", "/monitoring/frameGuideRatio/presets")
_bc("get_frame_grids", {}, "GET", "/monitoring/frameGrids",
    http_reply={"status": 200, "body": '{"frameGrids":["Thirds","Horizon"]}'}, expect_result=_val(["Thirds", "Horizon"]))
_bc("set_frame_grids", {"grids": ["Thirds", "Crosshair"]}, "PUT", "/monitoring/frameGrids",
    '{"frameGrids":["Thirds","Crosshair"]}')
_bc("get_safe_area_percent", {}, "GET", "/monitoring/safeAreaPercent")
_bc("set_safe_area_percent", {"percent": 90}, "PUT", "/monitoring/safeAreaPercent", '{"percent":90}',
    http_reply={"status": 204}, expect_result=_ACK)

# Immersive (p.65)
_bc("get_display_eye", {"display": "LCD"}, "GET", "/immersive/display/LCD/eye", model="ursa-cine-immersive",
    http_reply={"status": 200, "body": '{"eye":"Left"}'}, expect_result=_val("Left"))
_bc("set_display_eye", {"display": "LCD", "eye": "Right"}, "PUT", "/immersive/display/LCD/eye", '{"eye":"Right"}',
    model="ursa-cine-immersive", http_reply={"status": 422}, expect_result=_err(422))

# Slate (p.37-43)
_bc("get_next_clip_slate", {}, "GET", "/slates/nextClip", http_reply={"status": 409}, expect_result=_err(409))
_bc("set_next_clip_slate", {"body": {"clip": {"scene": "12A", "take": 3}, "lens": {"filter": "ND 0.6"}}},
    "PUT", "/slates/nextClip", '{"clip":{"scene":"12A","take":3},"lens":{"filter":"ND 0.6"}}',
    http_reply={"status": 200, "body": '{"clip":{"scene":"12A","take":3}}'}, expect_result=_ACK)
_bc("set_slate_reel", {"reel": 2}, "PUT", "/slates/nextClip", '{"clip":{"reel":2}}')
_bc("set_slate_scene", {"scene": "12A"}, "PUT", "/slates/nextClip", '{"clip":{"scene":"12A"}}',
    http_reply={"status": 409, "body": '{"error":"Partial update","details":[{"field":"scene","message":"invalid"}]}'},
    expect_result=_err(409))
_bc("set_slate_take", {"take": 4}, "PUT", "/slates/nextClip", '{"clip":{"take":4}}')
_bc("set_slate_good_take", {"good": True}, "PUT", "/slates/nextClip", '{"clip":{"goodTake":true}}')
_bc("set_slate_shot_type", {"shot_type": "MCU"}, "PUT", "/slates/nextClip", '{"clip":{"shotType":"MCU"}}')
_bc("set_slate_take_type", {"take_type": "VFX"}, "PUT", "/slates/nextClip", '{"clip":{"takeType":"VFX"}}')
_bc("set_slate_scene_location", {"location": "Exterior"}, "PUT", "/slates/nextClip", '{"clip":{"sceneLocation":"Exterior"}}')
_bc("set_slate_scene_time", {"time": "Night"}, "PUT", "/slates/nextClip", '{"clip":{"sceneTime":"Night"}}')
_bc("set_slate_project_name", {"name": "Evening News"}, "PUT", "/slates/nextClip", '{"project":{"projectName":"Evening News"}}')
_bc("set_slate_director", {"name": "A. Smith"}, "PUT", "/slates/nextClip", '{"project":{"director":"A. Smith"}}')
_bc("set_slate_camera", {"camera": "A"}, "PUT", "/slates/nextClip", '{"project":{"camera":"A"}}')
_bc("set_slate_camera_operator", {"name": "J. Doe"}, "PUT", "/slates/nextClip", '{"project":{"cameraOperator":"J. Doe"}}')
_bc("reset_slate_project_data", {}, "POST", "/slates/nextClip/resetProjectData", http_reply={"status": 200}, expect_result=_ACK)
_bc("reset_slate_lens_data", {}, "POST", "/slates/nextClip/resetLensData")
_bc("get_clip_slate", {"device": "cfast1", "path": "A001_08151230_C001.braw"}, "GET",
    "/slates/clips/cfast1/A001_08151230_C001.braw", http_reply={"status": 404}, expect_result=_err(404))
_bc("set_clip_slate", {"device": "cfast1", "path": "A001_C001.braw", "body": {"clip": {"goodTake": True}}}, "PUT",
    "/slates/clips/cfast1/A001_C001.braw", '{"clip":{"goodTake":true}}')
_bc("reset_clip_project_data", {"device": "cfast1", "path": "A001_C001.braw"}, "POST",
    "/slates/clips/cfast1/A001_C001.braw/resetProjectData")
_bc("reset_clip_lens_data", {"device": "cfast1", "path": "Day 1/A001_C001.braw"}, "POST",
    "/slates/clips/cfast1/Day%201%2FA001_C001.braw/resetLensData")

# System (p.21-26)
_bc("get_system", {}, "GET", "/system", http_reply={"status": 501}, expect_result=_err(501))
_bc("get_product", {}, "GET", "/system/product",
    http_reply={"status": 200, "body": '{"deviceName":"Camera 1","productName":"Blackmagic URSA Broadcast G2","softwareVersion":"9.1"}'},
    expect_result=_val({"deviceName": "Camera 1", "productName": "Blackmagic URSA Broadcast G2", "softwareVersion": "9.1"}))
_bc("get_supported_codec_formats", {}, "GET", "/system/supportedCodecFormats")
_bc("get_codec_format", {}, "GET", "/system/codecFormat")
_bc("set_codec_format", {"codec": "ProRes:HQ", "container": "QuickTime"}, "PUT", "/system/codecFormat",
    '{"codec":"ProRes:HQ","container":"QuickTime"}', http_reply={"status": 400}, expect_result=_err(400))
_bc("get_video_format", {}, "GET", "/system/videoFormat")
_bc("set_video_format_by_name", {"name": "1080p50"}, "PUT", "/system/videoFormat", '{"name":"1080p50"}',
    http_reply={"status": 409}, expect_result=_err(409))
_bc("set_video_format", {"frame_rate": "59.94", "height": 2160, "width": 3840}, "PUT", "/system/videoFormat",
    '{"frameRate":"59.94","height":2160,"width":3840,"interlaced":false}', http_reply={"status": 204}, expect_result=_ACK)
_bc("get_supported_video_formats", {}, "GET", "/system/supportedVideoFormats")
_bc("get_supported_formats", {}, "GET", "/system/supportedFormats")
_bc("get_format", {}, "GET", "/system/format")
_bc("set_format", {"body": {"codec": "BRaw:Q0", "frameRate": "24"}}, "PUT", "/system/format",
    '{"codec":"BRaw:Q0","frameRate":"24"}')
_bc("set_off_speed", {"enabled": True}, "PUT", "/system/format", '{"offSpeedEnabled":true}')
_bc("set_off_speed_frame_rate", {"fps": 60}, "PUT", "/system/format", '{"offSpeedFrameRate":60}',
    http_reply={"status": 501}, expect_result=_err(501))
_bc("get_event_list", {}, "GET", "/event/list",
    http_reply={"status": 200, "body": '{"events":["/lens/iris","/video/iso"]}'}, expect_result=_val(["/lens/iris", "/video/iso"]))

# Livestream (p.4-8)
_bc("get_livestream", {}, "GET", "/livestreams/0")
_bc("get_livestream_active", {}, "GET", "/livestreams/0/start", http_reply={"status": 200, "body": "true"},
    expect_result=_val(True))
_bc("start_livestream", {}, "PUT", "/livestreams/0/start", http_reply={"status": 204}, expect_result=_ACK)
_bc("get_livestream_inactive", {}, "GET", "/livestreams/0/stop")
_bc("stop_livestream", {}, "PUT", "/livestreams/0/stop")
_bc("get_livestream_platform", {}, "GET", "/livestreams/0/activePlatform")
_bc("set_livestream_platform", {"body": {"platform": "YouTube", "server": "Primary", "key": "abcd-1234", "quality": "Streaming High"}},
    "PUT", "/livestreams/0/activePlatform",
    '{"platform":"YouTube","server":"Primary","key":"abcd-1234","quality":"Streaming High"}',
    http_reply={"status": 400}, expect_result=_err(400))
_bc("get_livestream_platforms", {}, "GET", "/livestreams/platforms")
_bc("get_livestream_platform_config", {"platform": "YouTube"}, "GET", "/livestreams/platforms/YouTube")
_bc("get_custom_platforms", {}, "GET", "/livestreams/customPlatforms",
    http_reply={"status": 200, "body": '["Custom.xml"]'}, expect_result=_val(["Custom.xml"]))
_bc("delete_custom_platforms", {}, "DELETE", "/livestreams/customPlatforms")
_bc("get_custom_platform", {"filename": "Custom.xml"}, "GET", "/livestreams/customPlatforms/Custom.xml",
    http_reply={"status": 200, "body": "<streaming><service><name>My Service</name></service></streaming>"},
    expect_result=_val("<streaming><service><name>My Service</name></service></streaming>"))
_bc("set_custom_platform", {"filename": "Custom.xml", "xml": "<streaming><service><name>My Service</name></service></streaming>"},
    "PUT", "/livestreams/customPlatforms/Custom.xml",
    "<streaming><service><name>My Service</name></service></streaming>", http_reply={"status": 204}, expect_result=_ACK)
_bc("delete_custom_platform", {"filename": "Custom.xml"}, "DELETE", "/livestreams/customPlatforms/Custom.xml",
    http_reply={"status": 404}, expect_result=_err(404))

# Blackmagic Cloud media pool (p.9-12)
_bc("get_cloud_projects", {}, "GET", "/cloud/projects")
_bc("get_cloud_active_project", {}, "GET", "/cloud/projects/active")
_bc("get_cloud_project", {"project": 42}, "GET", "/cloud/projects/42", http_reply={"status": 404}, expect_result=_err(404))
_bc("get_cloud_clips", {}, "GET", "/cloud/clips")
_bc("get_cloud_uploading_clips", {}, "GET", "/cloud/clips/activeUploading")
_bc("get_cloud_clip", {"device": "cfast1", "path": "A001_C001.braw"}, "GET", "/cloud/clips/cfast1/A001_C001.braw")

# ── Telemetry: one vector per rule, replies shaped from each property's
# documented value (p.76-94) ──
_P = _BA


def _bt(name, path, body, state):
    telemetry(BC, name, inbound_http={"path": _P + path, "body": body}, expect_state=state)


_bt("transport-mode", "/transports/0", '{"mode":"InputRecord"}', {"transport": {"mode": "InputRecord"}})
_bt("transport-record", "/transports/0/record", '{"recording":true}', {"transport": {"recording": True}})
_bt("transport-play", "/transports/0/play", "true", {"transport": {"playing": True}})
_bt("transport-stop", "/transports/0/stop", "false", {"transport": {"stopped": False}})
_bt("playback", "/transports/0/playback", '{"type":"Shuttle","loop":true,"singleClip":false,"speed":2.5,"position":48}',
    {"playback": {"type": "Shuttle", "loop": True, "single_clip": False, "speed": 2.5, "position": 48}})
_bt("timecode", "/transports/0/timecode", '{"display":"01:02:03:04","timeline":"00:00:10:00"}',
    {"timecode": {"display": "01:02:03:04", "timeline": "00:00:10:00"}})
_bt("timecode-source", "/transports/0/timecode/source", '{"timecode":"Timeline"}', {"timecode": {"source": "Timeline"}})
_bt("clip-index", "/transports/0/clipIndex", '{"clipIndex":1}', {"transport": {"clip_index": 1}})
_bt("tally", "/camera/tallyStatus", '{"status":"Preview"}', {"camera": {"tally": "Preview"}})
_bt("power", "/camera/power",
    '{"source":"Battery","milliVolt":14800,"batteries":[{"milliVolt":14800,"chargeRemainingPercent":80,"statusFlags":["Battery Is Present"]}]}',
    {"camera": {"power": {"source": "Battery", "millivolt": 14800}}})
_bt("power-display-mode", "/camera/power/displayMode", '{"mode":"Percentage"}', {"camera": {"power": {"display_mode": "Percentage"}}})
_bt("color-bars", "/camera/colorBars", '{"enabled":true}', {"camera": {"color_bars": True}})
_bt("program-feed-display", "/camera/programFeedDisplay", '{"enabled":false}', {"camera": {"program_feed_display": False}})
_bt("timing-reference-lock", "/camera/timingReferenceLock", '{"locked":false}', {"camera": {"timing_reference_locked": False}})
_bt("iris", "/lens/iris", '{"continuousApertureAutoExposure":true,"apertureStop":4.0,"normalised":0.4,"apertureNumber":5}',
    {"lens": {"iris": {"stop": 4.0, "normalised": 0.4, "number": 5, "auto_exposure": True}}})
_bt("zoom", "/lens/zoom", '{"focalLength":50,"normalised":0.3}', {"lens": {"zoom": {"focal_length": 50, "normalised": 0.3}}})
_bt("focus", "/lens/focus", '{"normalised":0.6}', {"lens": {"focus": {"normalised": 0.6}}})
_bt("optical-image-stabilization", "/lens/opticalImageStabilization", '{"enabled":true}',
    {"lens": {"optical_image_stabilization": True}})
_bt("iso", "/video/iso", '{"iso":1250}', {"video": {"iso": 1250}})
_bt("gain", "/video/gain", '{"gain":6}', {"video": {"gain": 6}})
_bt("white-balance", "/video/whiteBalance", '{"whiteBalance":4300}', {"video": {"white_balance": 4300}})
_bt("white-balance-tint", "/video/whiteBalanceTint", '{"whiteBalanceTint":-5}', {"video": {"white_balance_tint": -5}})
_bt("nd-filter", "/video/ndFilter", '{"stop":4.0}', {"video": {"nd_filter": 4.0}})
_bt("nd-display-mode", "/video/ndFilter/displayMode", '{"displayMode":"Number"}', {"video": {"nd_display_mode": "Number"}})
_bt("shutter", "/video/shutter", '{"continuousShutterAutoExposure":false,"shutterSpeed":60}',
    {"video": {"shutter": {"speed": 60, "auto_exposure": False}}})
_bt("shutter-measurement", "/video/shutter/measurement", '{"measurement":"ShutterSpeed"}',
    {"video": {"shutter": {"measurement": "ShutterSpeed"}}})
_bt("auto-exposure", "/video/autoExposure", '{"mode":"Continuous","type":"Iris"}',
    {"video": {"auto_exposure": {"mode": "Continuous", "type": "Iris"}}})
_bt("detail-sharpening", "/video/detailSharpening", '{"enabled":true}', {"video": {"detail_sharpening": True}})
_bt("detail-sharpening-level", "/video/detailSharpeningLevel", '{"level":"Medium"}', {"video": {"detail_sharpening_level": "Medium"}})
_bt("color-lift", "/colorCorrection/lift", '{"red":0.1,"green":0.0,"blue":-0.1,"luma":0.05}',
    {"color": {"lift": {"red": 0.1, "green": 0.0, "blue": -0.1, "luma": 0.05}}})
_bt("color-gain", "/colorCorrection/gain", '{"red":1.2,"green":1.0,"blue":0.9,"luma":1.0}',
    {"color": {"gain": {"red": 1.2, "green": 1.0, "blue": 0.9, "luma": 1.0}}})
_bt("color-contrast", "/colorCorrection/contrast", '{"pivot":0.5,"adjust":1.2}',
    {"color": {"contrast": {"pivot": 0.5, "adjust": 1.2}}})
_bt("color-hue-saturation", "/colorCorrection/color", '{"hue":0.1,"saturation":1.3}',
    {"color": {"hue": 0.1, "saturation": 1.3}})
_bt("color-luma-contribution", "/colorCorrection/lumaContribution", '{"lumaContribution":0.75}',
    {"color": {"luma_contribution": 0.75}})
_bt("active-preset", "/presets/active", '{"preset":"Studio A.cset"}', {"presets": {"active": "Studio A.cset"}})
_bt("active-media", "/media/active", '{"workingsetIndex":0,"deviceName":"cfast1"}',
    {"media": {"active": {"index": 0, "device": "cfast1"}}})
_bt("working-set", "/media/workingset",
    '{"size":2,"workingset":[{"volume":"A001","deviceName":"cfast1","remainingRecordTime":3600,"totalSpace":256000000000,'
    '"remainingSpace":128000000000,"clipCount":12},null]}',
    {"media": {"devices": {"cfast1": {"volume": "A001", "remaining_record_time": 3600, "total_space": 256000000000,
                                      "remaining_space": 128000000000, "clip_count": 12}}}})
# The drive usb1 was taken out: its slot is null now, and it leaves the state.
telemetry(BC, "working-set-removed", inbound_http={"path": _P + "/media/workingset", "body":
          '{"size":2,"workingset":[{"volume":"A001","deviceName":"cfast1","remainingRecordTime":3600,'
          '"totalSpace":256000000000,"remainingSpace":128000000000,"clipCount":12},null]}'},
          state_before={"media": {"active": {"index": 0, "device": "cfast1"},
                                  "devices": {"usb1": {"volume": "T7", "clip_count": 3}}}},
          expect_state={"media": {"active": {"index": 0, "device": "cfast1"},
                                  "devices": {"cfast1": {"volume": "A001", "remaining_record_time": 3600,
                                                         "total_space": 256000000000,
                                                         "remaining_space": 128000000000, "clip_count": 12}}}})
_bt("product", "/system/product", '{"deviceName":"Camera 1","productName":"Blackmagic PYXIS 6K","softwareVersion":"9.2"}',
    {"system": {"device_name": "Camera 1", "product_name": "Blackmagic PYXIS 6K", "software_version": "9.2"}})
_bt("codec-format", "/system/codecFormat", '{"codec":"BRaw:Q0","container":"Braw"}',
    {"system": {"codec": "BRaw:Q0", "container": "Braw"}})
_bt("video-format", "/system/videoFormat", '{"name":"2160p25","frameRate":"25","height":2160,"width":3840,"interlaced":false}',
    {"system": {"video_format": {"name": "2160p25", "frame_rate": "25", "width": 3840, "height": 2160, "interlaced": False}}})
_bt("format", "/system/format",
    '{"codec":"BRaw:Q0","frameRate":"24","offSpeedEnabled":true,"offSpeedFrameRate":48,'
    '"recordResolution":{"width":6048,"height":4032},"sensorResolution":{"width":6048,"height":4032}}',
    {"system": {"format": {"codec": "BRaw:Q0", "frame_rate": "24", "off_speed_enabled": True, "off_speed_frame_rate": 48.0,
                           "record_width": 6048, "record_height": 4032}}})
_bt("livestream", "/livestreams/0",
    '{"status":"Streaming","bitrate":6000000,"effectiveVideoFormat":"1080p30","duration":95,"cache":3}',
    {"livestream": {"status": "Streaming", "bitrate": 6000000, "video_format": "1080p30", "duration": 95, "cache": 3}})
_bt("next-clip-slate", "/slates/nextClip",
    '{"clip":{"clipName":"A001_C014","reel":1,"scene":"12A","sceneLocation":"Interior","sceneTime":"Day","shotType":"CU",'
    '"slateFor":"Next Clip","take":3,"takeType":"None","goodTake":false},'
    '"lens":{"lensType":"","iris":"f4","focalLength":"50mm","distance":"","filter":""},'
    '"project":{"projectName":"Pilot","director":"A. Smith","camera":"A","cameraOperator":"J. Doe"}}',
    {"slate": {"clip_name": "A001_C014", "reel": 1, "scene": "12A", "take": 3, "good_take": False, "shot_type": "CU",
               "take_type": "None", "scene_location": "Interior", "scene_time": "Day", "project_name": "Pilot",
               "director": "A. Smith", "camera": "A", "camera_operator": "J. Doe"}})
_bt("audio-channel-count", "/audio/channels", '{"channels":2}', {"audio": {"channel_count": 2}})
_bt("audio-level", "/audio/channel/1/level", '{"gain":-6.0,"normalised":0.5}',
    {"audio": {"channels": {"2": {"gain": -6.0, "level": 0.5}}}})
_bt("audio-input", "/audio/channel/0/input", '{"input":"XLR Mic"}', {"audio": {"channels": {"1": {"input": "XLR Mic"}}}})
_bt("audio-phantom-power", "/audio/channel/0/phantomPower", '{"enabled":true}',
    {"audio": {"channels": {"1": {"phantom_power": True}}}})
_bt("audio-padding", "/audio/channel/1/padding", '{"enabled":false}', {"audio": {"channels": {"2": {"padding": False}}}})
_bt("audio-low-cut", "/audio/channel/0/lowCutFilter", '{"enabled":true}', {"audio": {"channels": {"1": {"low_cut": True}}}})
_bt("audio-available", "/audio/channel/3/available", '{"available":false}', {"audio": {"channels": {"4": {"available": False}}}})
_bt("display-flags", "/monitoring/LCD/zebra", '{"enabled":true}', {"monitoring": {"displays": {"LCD": {"zebra": True}}}})
_bt("focus-assist", "/monitoring/focusAssist", '{"mode":"Peak","color":"Green","intensity":60}',
    {"monitoring": {"focus_assist": {"mode": "Peak", "color": "Green", "intensity": 60}}})
_bt("frame-guide-ratio", "/monitoring/frameGuideRatio", '{"ratio":"2.39:1"}', {"monitoring": {"frame_guide_ratio": "2.39:1"}})
_bt("safe-area-percent", "/monitoring/safeAreaPercent", '{"percent":90}', {"monitoring": {"safe_area_percent": 90}})
_bt("display-eye", "/immersive/display/LCD/eye", '{"eye":"Right"}', {"immersive": {"displays": {"LCD": {"eye": "Right"}}}})
