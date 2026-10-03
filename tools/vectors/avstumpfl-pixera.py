# AV Stumpfl PIXERA Native API over JSON/TCP (dl) (avstumpfl-pixera): one
# vector per command. Each request is the JSON-RPC 2.0 message of the PIXERA
# 26.1 R 1 "API Commands" reference for that method (rev204 examples for the
# _legacy commands, keyed timelineName), with id 1, followed by the
# delimiter 0xPX (Native API Introduction, Quick Start). Replies are JSON-RPC
# responses: {"jsonrpc":"2.0","id":1} for a void method and with "result"
# otherwise, and the allowlist refusal of the Allowlist article. Monitoring
# pushes follow the 2021 API documentation, sections 6 and 7.
PX = "avstumpfl-pixera"
_DL = "0xPX"
_VOID = {"device_reply": '{"jsonrpc":"2.0","id":1}' + _DL, "expect_result": {"ok": {"kind": "ack"}}}
_DENIED = {"device_reply": '{"error":{"code":-32602,"message":"Access is not authorized."},"id":1,'
                           '"jsonrpc":"2.0","result":null}' + _DL,
           "expect_result": {"error": {"error": "device_error"}}}


def _px(command, input, method, params=None, model=None, **extra):
    msg = '{"jsonrpc":"2.0","id":1,"method":"%s"' % method
    if params is not None:
        msg += ',"params":{%s}' % params
    msg += "}"
    if model:
        extra["model"] = model
    text(PX, command, input, msg + _DL, **extra)


def _result(value, raw):
    return {"device_reply": '{"jsonrpc":"2.0","id":1,"result":%s}' % raw + _DL,
            "expect_result": {"ok": {"kind": "value", "value": value}}}


C = "Pixera.Compound."
_px("play_timeline", {"timeline": "Timeline 1"}, C + "setTransportModeOnTimeline",
    '"timelineName":"Timeline 1","mode":1', **_VOID)
_px("pause_timeline", {"timeline": "Timeline 1"}, C + "setTransportModeOnTimeline", '"timelineName":"Timeline 1","mode":2')
_px("stop_timeline", {"timeline": "Timeline 1"}, C + "setTransportModeOnTimeline", '"timelineName":"Timeline 1","mode":3',
    **_DENIED)
_px("toggle_timeline", {"timeline": "Show"}, C + "toggleTransport", '"timelineName":"Show"')
_px("get_timeline_transport", {"timeline": "Timeline 1"}, C + "getTransportModeOnTimeline", '"timelineName":"Timeline 1"',
    **_result(2, "2"))
_px("play_timeline_index", {"index": 0}, C + "setTransportModeOnTimelineAtIndex", '"index":0,"mode":1',
    **_result(True, "true"))
_px("pause_timeline_index", {"index": 1}, C + "setTransportModeOnTimelineAtIndex", '"index":1,"mode":2')
_px("stop_timeline_index", {"index": 1}, C + "setTransportModeOnTimelineAtIndex", '"index":1,"mode":3')
_px("start_first_timeline", {}, C + "startFirstTimeline", **_VOID)
_px("pause_first_timeline", {}, C + "pauseFirstTimeline")
_px("stop_first_timeline", {}, C + "stopFirstTimeline")
_px("set_timeline_opacity", {"timeline": "Timeline 1", "opacity": 0.5}, C + "setOpacityOnTimeline",
    '"timelineName":"Timeline 1","opacity":0.500')
_px("get_timeline_opacity", {"timeline": "Timeline 1"}, C + "getOpacityOnTimeline", '"timelineName":"Timeline 1"',
    **_result(1.0, "1.0"))
_px("apply_cue", {"timeline": "Timeline 1", "cue": "Intro"}, C + "applyCueOnTimeline",
    '"timelineName":"Timeline 1","cueName":"Intro"', **_VOID)
_px("apply_cue_blend", {"timeline": "Timeline 1", "cue": "Intro", "blend_s": 2.5}, C + "applyCueOnTimeline",
    '"timelineName":"Timeline 1","cueName":"Intro","blendDuration":2.500')
_px("apply_cue_index", {"timeline_index": 0, "cue_index": 3}, C + "applyCueAtIndexOnTimelineAtIndex",
    '"cueIndex":3,"timelineIndex":0')
for suffix, key, model in [("", "name", None), ("_legacy", "timelineName", "pixera-legacy")]:
    _px("set_timeline_time_s" + suffix, {"timeline": "Timeline 1", "time_s": 12.5},
        C + "setCurrentTimeOfTimelineInSeconds", '"%s":"Timeline 1","time":12.500' % key, model, **_VOID)
    _px("set_timeline_time_frames" + suffix, {"timeline": "Timeline 1", "frame": 750},
        C + "setCurrentTimeOfTimeline", '"%s":"Timeline 1","time":750' % key, model)
    _px("set_timeline_time_and_mode" + suffix, {"timeline": "Timeline 1", "time_s": 0.0, "mode": 1},
        C + "setCurrentTimeAndTransportModeOfTimelineInSeconds", '"%s":"Timeline 1","time":0.000,"mode":1' % key, model)
    _px("get_timeline_time_s" + suffix, {"timeline": "Timeline 1"}, C + "getCurrentTimeOfTimelineInSeconds",
        '"%s":"Timeline 1"' % key, model, **_result(12.5, "12.5"))
    _px("get_timeline_time_frames" + suffix, {"timeline": "Timeline 1"}, C + "getCurrentTimeOfTimeline",
        '"%s":"Timeline 1"' % key, model, **_result(750, "750"))
    _px("get_timeline_timecode" + suffix, {"timeline": "Timeline 1"}, C + "getCurrentHMSFOfTimeline",
        '"%s":"Timeline 1"' % key, model, **_result("00:00:12:12", '"00:00:12:12"'))
    _px("get_timeline_countdown" + suffix, {"timeline": "Timeline 1"}, C + "getCurrentCountdownOfTimeline",
        '"%s":"Timeline 1"' % key, model)
    _px("get_timeline_countdown_timecode" + suffix, {"timeline": "Timeline 1"}, C + "getCurrentCountdownHMSFOfTimeline",
        '"%s":"Timeline 1"' % key, model)
    _px("get_timeline_fps" + suffix, {"timeline": "Timeline 1"}, C + "getFpsOfTimeline",
        '"%s":"Timeline 1"' % key, model, **_result(60.0, "60.0"))
_px("set_param_value", {"path": "Timeline 1.Layer 1.Opacity", "value": 0.25}, C + "setParamValue",
    '"path":"Timeline 1.Layer 1.Opacity","value":0.2500', **_VOID)
_px("get_param_value", {"path": "Timeline 1.Layer 1.Opacity"}, C + "getParamValue", '"path":"Timeline 1.Layer 1.Opacity"')
_px("set_layer_transport", {"layer": "Timeline 1.Layer 1", "mode": 1, "loop": True}, C + "setTransportModeOnLayer",
    '"layerPath":"Timeline 1.Layer 1","mode":1,"loop":true')
_px("get_layer_transport", {"layer": "Timeline 1.Layer 1"}, C + "getTransportModeOnLayer", '"layerPath":"Timeline 1.Layer 1"')
_px("assign_resource_to_layer", {"resource": "Media/Folder/video.mov", "layer": "Timeline 1.Layer 1"},
    C + "assignResourceToLayer", '"resourcePath":"Media/Folder/video.mov","layerPath":"Timeline 1.Layer 1"', **_VOID)
_px("get_layer_resource", {"layer": "Timeline 1.Layer 1"}, C + "getResourceAssignedToLayer",
    '"layerPath":"Timeline 1.Layer 1"', **_result("Media/Folder/video.mov", '"Media/Folder/video.mov"'))
_px("pause_smpte_input", {}, C + "setPauseSmpteInput", '"doPause":true')
_px("get_timeline_names", {}, "Pixera.Timelines.getTimelineNames",
    **_result(["Timeline 1", "Show"], '["Timeline 1","Show"]'))
_px("get_timeline_handle", {"name": "Timeline 1"}, "Pixera.Timelines.getTimelineFromName", '"name":"Timeline 1"',
    **_result(6511533668781846, "6511533668781846"))
_px("get_timeline_attributes", {"handle": 6511533668781846}, "Pixera.Timelines.Timeline.getAttributes",
    '"handle":6511533668781846',
    **_result({"index": 0, "name": "Timeline 1", "fps": 60.0, "mode": 1},
              '{"index":0,"name":"Timeline 1","fps":60.0,"mode":1}'))
_px("next_cue", {"handle": 6511533668781846}, "Pixera.Timelines.Timeline.moveToNextCue", '"handle":6511533668781846',
    **_VOID)
_px("previous_cue", {"handle": 6511533668781846}, "Pixera.Timelines.Timeline.moveToPreviousCue",
    '"handle":6511533668781846')
_px("apply_cue_number", {"handle": 6511533668781846, "number": "1.2.3"},
    "Pixera.Timelines.Timeline.applyCueWithNumberString", '"handle":6511533668781846,"numberStr":"1.2.3"')
_px("blend_to_time", {"handle": 6511533668781846, "goal_frame": 600.0, "blend_frames": 30.0},
    "Pixera.Timelines.Timeline.blendToTime", '"handle":6511533668781846,"goalTime":600.000,"blendDuration":30.000')
_px("get_cue_names", {"handle": 6511533668781846}, "Pixera.Timelines.Timeline.getCueNames", '"handle":6511533668781846')
_px("get_screen_names", {}, "Pixera.Screens.getScreenNames")
_px("set_screen_blackout", {"handle": 123456789}, "Pixera.Screens.Screen.setBlackout",
    '"handle":123456789,"isActive":true', **_VOID)
_px("set_projector_blackout", {"handle": 123456789, "blackout": False}, "Pixera.Projectors.Projector.setBlackout",
    '"handle":123456789,"isActive":false')
_px("get_api_revision", {}, "Pixera.Utility.getApiRevision", **_result(452, "452"))
_px("has_function", {"function": "Pixera.Compound.toggleTransport"}, "Pixera.Utility.getHasFunction",
    '"functionName":"Pixera.Compound.toggleTransport"', **_result(False, "false"))
_px("noop", {}, "Pixera.Utility.noop", **_VOID)
_px("subscribe_monitoring", {"subject": "timelinePositions"}, "Pixera.Utility.subscribeMonitoringSubject",
    '"subject":"timelinePositions"')
_px("unsubscribe_monitoring", {"subject": "timelineCountdowns"}, "Pixera.Utility.unsubscribeMonitoringSubject",
    '"subject":"timelineCountdowns"', **_result(True, "true"))
_px("save_project", {}, "Pixera.Session.saveProject", **_VOID)
_px("load_project", {"path": "D:/Shows/Main.pxp"}, "Pixera.Session.loadProject", '"path":"D:/Shows/Main.pxp"')
_px("close_app", {"save": False}, "Pixera.Session.closeApp", '"saveProject":false', **_DENIED)

# ── Telemetry: monitoring pushes (id -1) ──
PX_CONNECT = ['{"jsonrpc":"2.0","id":1,"method":"Pixera.Utility.setMonitoringHasDelimiter","params":'
              '{"hasDelimiter":true}}' + _DL]


def _mon(name, entries):
    return json.dumps({"jsonrpc": "2.0", "id": -1, "type": "monEvent", "name": name, "entries": entries},
                      separators=(",", ":")) + _DL


telemetry(PX, "transport", expect_connect_wire=PX_CONNECT,
          inbound=_mon("timelineTransport", [{"handle": 6511533668781846, "value": 1}]),
          expect_state={"timelines": {"6511533668781846": {"transport": "play"}},
                        "monitoring": {"last_event": "timelineTransport"}})
telemetry(PX, "positions", inbound=_mon("timelinePositions", [{"handle": 6511533668781846, "value": 750},
                                                             {"handle": 42, "value": 0}]),
          expect_state={"timelines": {"6511533668781846": {"position_frames": 750}, "42": {"position_frames": 0}},
                        "monitoring": {"last_event": "timelinePositions"}})
telemetry(PX, "countdowns", inbound=_mon("timelineCountdowns", [{"handle": 42, "value": 120, "flag": 2}]),
          expect_state={"timelines": {"42": {"countdown_frames": 120, "countdown": "waiting_at_cue"}},
                        "monitoring": {"last_event": "timelineCountdowns"}})
telemetry(PX, "cue-applied", inbound=_mon("cueApplied", [{"handles": [6511533668781846]}]),
          expect_state={"monitoring": {"last_event": "cueApplied"}})
