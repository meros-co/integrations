# VDO.Ninja (vdoninja): one vector per command and telemetry vectors for the
# answers and pushes. Messages follow the Companion Ninja README: a JSON
# object {"action", "target", "value"} with "null" for what is unused and an
# optional value2; answers come back as {"callback": {...}} and pushes as
# {"update": {...}}, as the MIT Companion module reads them. The expected
# messages are written here as Python objects and serialized independently.
VN = "vdoninja"
_VNS = {"api_key": "SomeAPIKey"}


def _vn_msg(action, target="null", value="null", value2=None):
    m = {"action": action, "target": target, "value": value}
    if value2 is not None:
        m["value2"] = value2
    return json.dumps(m, separators=(",", ":"))


def _vn(command, input, action, target="null", value="null", value2=None, **extra):
    text(VN, command, input, _vn_msg(action, target, value, value2), settings=_VNS, **extra)


# Local actions.
_vn("set_speaker", {"state": "false"}, "speaker", value="false",
    device_reply=json.dumps({"callback": {"action": "speaker", "target": "null", "value": "false", "result": False}}),
    expect_result={"ok": {"kind": "value", "value": False}})
_vn("set_mic", {"state": "toggle"}, "mic", value="toggle",
    device_reply=json.dumps({"callback": {"action": "mic", "target": "null", "value": "toggle", "result": True}}),
    expect_result={"ok": {"kind": "value", "value": True}})
_vn("set_camera", {"state": "true"}, "camera", value="true")
_vn("set_volume", {"level": 150}, "volume", value=150)
_vn("set_playback_muted", {"muted": "true"}, "volume", value="true")
_vn("send_chat", {"message": "Rolling in 5 \"minutes\""}, "sendChat", value="Rolling in 5 \"minutes\"")
_vn("show_chat_overlay", {"message": "Stand by"}, "showChatOverlay", value="Stand by")
_vn("set_recording", {"recording": "true"}, "record", value="true")
_vn("reload", {}, "reload")
_vn("hangup", {}, "hangup")
_vn("set_incoming_bitrate", {"kbps": 2500}, "bitrate", value=2500)
_vn("reset_incoming_bitrate", {}, "bitrate", value="true")
_vn("pause_incoming_video", {}, "bitrate", value="false")
_vn("set_panning", {"pan": 45}, "panning", value=45)
_vn("center_panning", {}, "panning", value="true")
_vn("toggle_hand", {}, "togglehand")
_vn("toggle_screen_share", {}, "togglescreenshare")
_vn("force_keyframe", {}, "forceKeyframe")
_vn("toggle_group", {"group": 2}, "group", value=2)
_vn("join_group", {"group": 3}, "joinGroup", value=3)
_vn("leave_group", {"group": 3}, "leaveGroup", value=3)
_vn("toggle_view_group", {"group": 1}, "viewGroup", value=1)
_vn("join_view_group", {"group": 8}, "joinViewGroup", value=8)
_vn("leave_view_group", {"group": 8}, "leaveViewGroup", value=8)
_vn("get_details", {}, "getDetails")
_vn("request_stats", {}, "requestStats")
_vn("get_stats", {}, "getStats")
_vn("get_stream_stats", {"target": "abc123"}, "getStats", target="abc123")
_vn("next_slide", {}, "nextSlide")
_vn("prev_slide", {}, "prevSlide")
_vn("set_solo_video", {"state": "toggle"}, "soloVideo", value="toggle")
_vn("mute_all_guests", {"state": "true"}, "muteAllGuests", value="true")
_vn("start_room_timer", {"seconds": 300}, "startRoomTimer", value=300)
_vn("pause_room_timer", {}, "PauseRoomTimer")
_vn("stop_room_timer", {}, "stopRoomTimer")
_vn("get_guest_list", {}, "getGuestList")
_vn("set_buffer_delay", {"ms": 500}, "setBufferDelay", value=500)
_vn("set_stream_buffer_delay", {"target": "abc123", "ms": 1200}, "setBufferDelay", target="abc123", value=1200)
_vn("set_active_speaker", {"mode": "toggle"}, "activeSpeaker", value="toggle")
_vn("set_active_speaker", {"mode": "off"}, "activeSpeaker", value=False, file="set_active_speaker-off")
_vn("set_active_speaker", {"mode": "2"}, "activeSpeaker", value=2, file="set_active_speaker-2")
_vn("set_tally_light", {"state": "onair"}, "tallylight", value="onair")
_vn("set_aspect_ratio", {"ratio": "16:9"}, "aspectRatio", value="16:9")
_vn("set_video_constraint", {"constraint": "frameRate", "value": "30"}, "videoConstraint", target="frameRate",
    value2="30")
_vn("zoom", {"step": 0.125}, "zoom", value=0.125)
_vn("set_zoom", {"position": 0.625}, "zoom", value=0.625, value2="abs")
_vn("focus", {"step": -0.075}, "focus", value=-0.075)
_vn("pan", {"step": 0.375}, "pan", value=0.375)
_vn("tilt", {"step": -0.375}, "tilt", value=-0.375)
_vn("set_exposure", {"level": 0.875}, "exposure", value=0.875)
_vn("set_layout", {"layout": 3}, "layout", value=3)
_LAYOUT = [{"x": 0, "y": 0, "w": 50, "h": 100, "c": False, "slot": 0}, {"x": 50, "y": 0, "w": 50, "h": 100, "c": False, "slot": 1}]
_vn("set_layout_custom", {"layout": _LAYOUT}, "layout", value=_LAYOUT)

# Director actions on a guest.
_vn("forward_guest", {"target": "2", "room": "greenroom"}, "forward", target="2", value="greenroom")
_vn("toggle_guest_scene", {"target": "1", "scene": "1"}, "addScene", target="1", value="1")
_vn("set_guest_scene", {"target": "abc123", "scene": "interview", "in_scene": "false"}, "setScene", target="abc123",
    value="interview", value2=False)
_vn("activate_queued_guest", {"target": "abc123"}, "activateQueuedGuest", target="abc123")
_vn("set_guest_slot", {"target": "abc123", "slot": 0}, "setslot", target="abc123", value=0)
_vn("toggle_guest_scene_audio", {"target": "3", "scene": "0"}, "muteScene", target="3", value="0")
_vn("toggle_guest_group", {"target": "3", "group": 4}, "group", target="3", value=4)
_vn("toggle_guest_mic", {"target": "1"}, "mic", target="1",
    device_reply=json.dumps({"callback": {"action": "mic", "target": "1", "value": "null", "result": True}}),
    expect_result={"ok": {"kind": "value", "value": True}})
_vn("hangup_guest", {"target": "1"}, "hangup", target="1")
_vn("toggle_guest_solo_chat", {"target": "1"}, "soloChat", target="1")
_vn("toggle_guest_solo_chat_bidirectional", {"target": "1"}, "soloChatBidirectional", target="1")
_vn("toggle_guest_speaker", {"target": "1"}, "speaker", target="1")
_vn("toggle_guest_display", {"target": "1"}, "display", target="1")
_vn("set_guest_mirror", {"target": "1", "state": "true"}, "mirror", target="1", value="true")
_vn("rotate_guest", {"target": "1", "rotation": "next"}, "rotate", target="1", value=True)
_vn("rotate_guest", {"target": "1", "rotation": "270"}, "rotate", target="1", value=270, file="rotate_guest-270")
_vn("set_guest_channel", {"target": "1", "channel": 2}, "channel", target="1", value=2)
_vn("send_director_chat", {"target": "2", "message": "You're live"}, "sendDirectorChat", target="2", value="You're live")
_vn("send_pinned_director_chat", {"target": "2", "message": "Mute please"}, "sendPinnedDirectorChat", target="2",
    value="Mute please")
_vn("force_guest_keyframe", {"target": "2"}, "forceKeyframe", target="2")
_vn("toggle_guest_solo_video", {"target": "2"}, "soloVideo", target="2")
_vn("set_guest_volume", {"target": "2", "level": 80}, "volume", target="2", value=80)
_vn("start_guest_timer", {"target": "2", "seconds": 60}, "startRoomTimer", target="2", value=60)
_vn("pause_guest_timer", {"target": "2"}, "PauseRoomTimer", target="2")
_vn("stop_guest_timer", {"target": "2"}, "stopRoomTimer", target="2")
for _axis in ("Zoom", "Focus", "Pan", "Tilt"):
    _vn(f"guest_{_axis.lower()}", {"target": "2", "step": 0.125}, f"ptz{_axis}", target="2", value=0.125)
    _vn(f"set_guest_{_axis.lower()}", {"target": "2", "position": 0.625}, f"ptz{_axis}", target="2", value=0.625, value2="abs")
_vn("set_guest_autofocus", {"target": "2", "state": "manual"}, "ptzAutofocus", target="2", value="manual")
_vn("request_guest_resolution", {"target": "2", "resolution": "1280x720"}, "requestResolution", target="2",
    value="1280x720")
_vn("request_guest_aspect_ratio", {"target": "2", "ratio": "16:9", "max": 1920}, "requestAspectRatio", target="2",
    value="16:9", value2=1920)
_vn("set_guest_width", {"target": "2", "pixels": 1920}, "setWidth", target="2", value=1920)
_vn("set_guest_height", {"target": "2", "pixels": 1080}, "setHeight", target="2", value=1080)
_vn("set_guest_aspect_ratio", {"target": "2", "ratio": 1.7778}, "setAspectRatio", target="2", value=1.7778)
_vn("refresh_guest_video", {"target": "2"}, "refreshVideo", target="2")
_vn("restart_guest_connection", {"target": "2"}, "refreshConnection", target="2")
_vn("recover_guest_stream", {"target": "2"}, "recoverStream", target="2")
_vn("move_guest_mix_order", {"target": "2", "direction": "earlier"}, "mixorder", target="2", value=-1)

# Telemetry.
_VNDETAILS = [
    {"streamID": "dir1", "label": "", "director": True, "localStream": True, "muted": False, "videoMuted": True,
     "speakerMuted": False, "scenes": {}, "others": {}},
    {"streamID": "abc123", "label": "Host", "position": 1, "muted": True, "videoMuted": False, "speakerMuted": False,
     "scenes": {"1": True, "2": False}, "others": {"mute-guest": "0", "hide-guest": "1"}}]
_VNSTATE = {"streams": {
    "dir1": {"label": "", "director": True, "local": True, "muted": False, "video_muted": True,
             "speaker_muted": False, "scenes": "{}"},
    "abc123": {"label": "Host", "position": 1, "muted": True, "video_muted": False, "speaker_muted": False,
               "scenes": '{"1":true,"2":false}', "director_muted": False, "director_hidden": True}}}
telemetry(VN, "details", settings=_VNS,
          inbound_ws=json.dumps({"callback": {"action": "getDetails", "target": "null", "value": "null",
                                              "result": _VNDETAILS}}),
          state_before={"streams": {"gone": {"label": "Left"}}},
          expect_state=_VNSTATE,
          expect_connect_ws=['{"join":"SomeAPIKey"}', '{"action":"getDetails","target":"null","value":"null"}'])
telemetry(VN, "details-object", settings=_VNS,
          inbound_ws=json.dumps({"callback": {"action": "getDetails", "result": {"0": _VNDETAILS[0], "1": _VNDETAILS[1]}}}),
          expect_state=_VNSTATE)
telemetry(VN, "guest-list", settings=_VNS,
          inbound_ws=json.dumps({"callback": {"action": "getGuestList", "result": {
              "1": {"streamID": "abc123", "label": "Host"}, "2": {"streamID": "def456", "label": "Guest"}}}}),
          expect_state={"guests": {"1": {"stream_id": "abc123", "label": "Host"},
                                   "2": {"stream_id": "def456", "label": "Guest"}}})
telemetry(VN, "command-answer", settings=_VNS,
          inbound_ws=json.dumps({"callback": {"action": "addScene", "target": "1", "value": "1", "result": True}}),
          expect_state={},
          expect_then_send=['{"action":"getDetails","target":"null","value":"null"}'])
telemetry(VN, "update-mic", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "abc123", "action": "remoteMuted", "value": False}}),
          expect_state={"streams": {"abc123": {"muted": False}}})
telemetry(VN, "update-camera", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "abc123", "action": "camera", "value": True}}),
          expect_state={"streams": {"abc123": {"video_muted": True}}})
telemetry(VN, "update-director-muted", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "abc123", "action": "directorMuted", "value": True}}),
          expect_state={"streams": {"abc123": {"director_muted": True}}})
telemetry(VN, "update-director-hidden", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "abc123", "action": "directorVideoHide", "value": False}}),
          expect_state={"streams": {"abc123": {"director_hidden": False}}})
telemetry(VN, "update-hangup", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "abc123", "action": "hangup", "value": True}}),
          state_before={"streams": {"abc123": {"label": "Host"}}},
          expect_state={"streams": {}})
telemetry(VN, "update-end-view", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "dir1", "action": "endViewConnection", "value": "abc123"}}),
          state_before={"streams": {"abc123": {"label": "Host"}}},
          expect_state={"streams": {}})
telemetry(VN, "update-new-connection", settings=_VNS,
          inbound_ws=json.dumps({"update": {"streamID": "def456", "action": "newViewConnection", "value": True}}),
          expect_state={},
          expect_then_send=['{"action":"getDetails","target":"null","value":"null"}'])
