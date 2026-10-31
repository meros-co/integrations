# Ecamm Live (ecamm-live): Ecamm Live Remote API v4.4. Every request is a GET
# of /<command>, parameters URL-encoded in the query (spaces as %20); get
# commands answer JSON, set commands "Done". Scalar answers are one-string
# arrays (the Companion module's fixtures), lists sit under "items".
EC = "ecamm-live"
EC_OK = {"ok": {"kind": "ack"}}
_DONE = {"status": 200, "body": "Done"}


def ec_set(command, input, target, **extra):
    http(EC, command, input, "GET", target, http_reply=_DONE, expect_result=EC_OK, **extra)


def ec_get(command, input, target, body, value, **extra):
    http(EC, command, input, "GET", target, http_reply={"status": 200, "body": body},
         expect_result={"ok": {"kind": "value", "value": value}}, **extra)


SCENE = "8A1DB264-FD68-4B20-973D-62B051A87E0E"

# Reads
ec_get("get_info", {}, "/getInfo", '{"ButtonLabel":"Record","Viewers":"0"}', {"ButtonLabel": "Record", "Viewers": "0"})
ec_get("get_button_label", {}, "/getButtonLabel", '["Go Live"]', "Go Live")
ec_get("get_pause_button_label", {}, "/getPauseButtonLabel", '["None"]', "None")
ec_get("get_scene_list", {}, "/getSceneList", '{"items":[{"title":"Intro","UUID":"A1"}]}', [{"title": "Intro", "UUID": "A1"}])
ec_get("get_scene_dict", {}, "/getSceneDict", '{"A1":{"title":"Intro"}}', {"A1": {"title": "Intro"}})
ec_get("get_scene_image", {"id": SCENE}, f"/getSceneImage?id={SCENE}", '{"icon":"/9j/4AAQ"}', {"icon": "/9j/4AAQ"})
ec_get("get_current_scene", {}, "/getCurrentScene", f'["{SCENE}"]', SCENE)
ec_get("get_mute", {}, "/getMute", '["no"]', "no")
ec_get("get_bus_mute", {"bus": "soundeffects"}, "/getMute?bus=soundeffects", '["yes"]', "yes")
ec_get("get_volume", {"bus": "system audio"}, "/getVolume?bus=system%20audio", '["80"]', "80")
ec_get("get_viewers", {}, "/getViewers", '["12"]', "12")
ec_get("get_cameras", {}, "/getInputs", '{"items":[{"title":"Camera A","UUID":"CAM-A"}]}',
       [{"title": "Camera A", "UUID": "CAM-A"}])
ec_get("get_default_camera", {}, "/getDefaultCamera", '["CAM-A"]', "CAM-A")
ec_get("get_source_mode", {}, "/getCurrentMode", '["cam"]', "cam")
ec_get("get_video_list", {}, "/getVideoList", '{"items":[{"title":"Intro.mp4","UUID":"/Users/me/Intro.mp4"}]}',
       [{"title": "Intro.mp4", "UUID": "/Users/me/Intro.mp4"}])
ec_get("get_video_dict", {}, "/getVideoDict", "{}", {})
ec_get("get_video_image", {"path": "/Users/me/My Clip.mp4"}, "/getVideoImage?id=%2FUsers%2Fme%2FMy%20Clip.mp4",
       '{"icon":"/9j/"}', {"icon": "/9j/"})
ec_get("get_overlay_list", {}, "/getOverlayList", '{"items":[]}', [])
ec_get("get_overlay_dict", {}, "/getOverlayDict", "{}", {})
ec_get("get_overlay_image", {"id": "OV-1"}, "/getOverlayImage?id=OV-1", '{"icon":"/9j/"}', {"icon": "/9j/"})
ec_get("get_sound_list", {}, "/getSoundList", '{"items":[]}', [])
ec_get("get_sound_dict", {}, "/getSoundDict", "{}", {})
ec_get("get_audio_filter_list", {}, "/getAudioFilterList", '{"items":[]}', [])
ec_get("get_audio_filter_dict", {}, "/getAudioFilterDict", "{}", {})
ec_get("get_profile_list", {}, "/getProfileList", '{"items":[{"title":"Default Show","UUID":"P1"}]}',
       [{"title": "Default Show", "UUID": "P1"}])
ec_get("get_channels", {}, "/getChannels", '{"items":[]}', {"items": []})
ec_get("get_live_demo", {}, "/getLiveDemo", '["no"]', "no")
ec_get("get_hide_show_ui", {}, "/getHideShowUI", '["yes"]', "yes")
ec_get("get_connection_status", {}, "/getConnectionStatus", '["Allowed"]', "Allowed")

# Changes
ec_set("click_button", {}, "/setClickButton")
ec_set("start_broadcast", {"channel": "CH-1", "title": "Sunday Service", "description": "Live"},
       "/setClickButton?channel=CH-1&title=Sunday%20Service&desc=Live")
ec_set("click_pause_button", {}, "/setClickPauseButton")
ec_set("start_new_recording", {}, "/setStartNewRecording")
ec_set("set_scene", {"id": SCENE}, f"/setScene?id={SCENE}")
ec_set("next_scene", {}, "/setNext")
ec_set("previous_scene", {}, "/setPrev")
ec_set("toggle_mute", {}, "/setMute")
ec_set("toggle_bus_mute", {"bus": "mic"}, "/setMute?bus=mic")
ec_set("set_volume", {"bus": "guest_2", "volume": 75}, "/setVolume?bus=guest_2&volume=75")
ec_set("set_camera", {"id": "CAM-A"}, "/setInput?id=CAM-A")
ec_set("set_source_mode", {"mode": "screen"}, "/setMode?mode=screen")
ec_set("toggle_pip", {}, "/setPIP")
ec_set("play_video", {"path": "/Users/me/Intro.mp4"}, "/setVideo?id=%2FUsers%2Fme%2FIntro.mp4")
ec_set("toggle_overlay", {"id": "OV-1"}, "/setOverlay?id=OV-1")
ec_set("hide_comment", {}, "/setHideComment")
ec_set("show_comment", {}, "/setShowComment")
ec_set("play_sound", {"id": "SND-1", "volume": 50, "action": "loop"}, "/setSound?id=SND-1&volume=50&action=loop")
ec_set("set_sound_volume", {"volume": 20}, "/setSoundVolume?volume=20")
ec_set("stop_sound", {}, "/setSoundStop")
ec_set("pause_sound", {}, "/setSoundPause")
ec_set("toggle_audio_filter", {"id": "AF-1"}, "/setAudioFilter?id=AF-1")
ec_set("toggle_preview_mode", {}, "/setPreviewMode")
ec_set("publish_preview", {}, "/setPublish")
ec_set("set_profile", {"id": "P1"}, "/setProfile?id=P1")
ec_set("zoom_new_meeting", {"personal_meeting_id": False}, "/setZoomNew?pmi=false")
ec_set("zoom_leave_meeting", {}, "/setZoomLeave?end=true")
ec_set("zoom_panel", {"panel": "participants"}, "/setZoomPanel?panel=participants")
ec_set("zoom_video", {}, "/setZoomVideo")
ec_set("zoom_audio", {}, "/setZoomAudio")
ec_set("zoom_mute_all", {}, "/setZoomMuteAll")
ec_set("zoom_raise_hand", {}, "/setZoomRaiseHand")
ec_set("zoom_full_screen", {}, "/setZoomFullScreen")
ec_set("zoom_cloud_record", {}, "/setZoomCloudRecord")
ec_set("zoom_local_record", {}, "/setZoomLocalRecord")
ec_set("zoom_pause_record", {}, "/setZoomPauseRecord")
ec_set("zoom_previous_gallery", {}, "/setZoomPrevGallery")
ec_set("zoom_next_gallery", {}, "/setZoomNextGallery")
ec_set("zoom_gallery_speaker", {}, "/setZoomGallerySpeaker")
ec_set("zoom_spotlight_self", {}, "/setSpotlightSelf")
ec_set("post_comment", {"text": "Welcome all"}, "/setComment?text=Welcome%20all")
ec_set("add_marker", {"text": "Q&A", "show_dialog": True}, "/setMarker?text=Q%26A&dialogbox=true")
ec_set("add_marker_untitled", {}, "/setMarker?dialogbox=false")
ec_set("toggle_live_demo", {}, "/setLiveDemo")
ec_set("toggle_hide_show_ui", {}, "/setHideShowUI")

# Telemetry: getInfo (the Companion module's fixture, Zoom flags added) and
# the lists.
telemetry(EC, "info", inbound_http={"path": "/getInfo", "body": (
    '{"MUTE_ZOOM":"no","ButtonLabel":"Record","Mute":"no","VOLUME_MIC":"100","CurrentScene":"' + SCENE + '",'
    '"PreviewMode":"no","HidingUI":"yes","LiveDemo":"no","Viewers":"3","PauseButtonLabel":"None",'
    '"ZoomInMeeting":"yes","ZoomMuted":"no","ZoomHandRaised":"no","ZoomCamOn":"yes","ZoomCloudRecording":"no",'
    '"ZoomLocalRecording":"no","ZoomRecordingPaused":"no","ZoomHosting":"yes"}')},
    expect_state={
        "button_label": "Record", "pause_button_label": "None", "current_scene": SCENE, "viewers": 3,
        "muted": False, "preview_mode": False, "hiding_ui": True, "live_demo": False,
        "zoom": {"in_meeting": True, "muted": False, "hand_raised": False, "camera_on": True, "cloud_recording": False,
                 "local_recording": False, "recording_paused": False, "hosting": True},
        "info": {"MUTE_ZOOM": "no", "ButtonLabel": "Record", "Mute": "no", "VOLUME_MIC": "100", "CurrentScene": SCENE,
                 "PreviewMode": "no", "HidingUI": "yes", "LiveDemo": "no", "Viewers": "3", "PauseButtonLabel": "None",
                 "ZoomInMeeting": "yes", "ZoomMuted": "no", "ZoomHandRaised": "no", "ZoomCamOn": "yes",
                 "ZoomCloudRecording": "no", "ZoomLocalRecording": "no", "ZoomRecordingPaused": "no",
                 "ZoomHosting": "yes"}})
telemetry(EC, "scenes", inbound_http={"path": "/getSceneList", "body": (
    '{"items":[{"title":"Me (full)","UUID":"9FEA0460","Locked":false},'
    '{"title":"Speaker Stage","UUID":"6D68813C","CURRENT":true,"Locked":true}]}')},
    expect_state={"scenes": {"9FEA0460": {"title": "Me (full)", "index": 0, "locked": False},
                             "6D68813C": {"title": "Speaker Stage", "index": 1, "current": True, "locked": True}}})
telemetry(EC, "current-scene", inbound_http={"path": "/getCurrentScene", "body": f'["{SCENE}"]'},
          expect_state={"current_scene": SCENE})
telemetry(EC, "overlays", inbound_http={"path": "/getOverlayList", "body": (
    '{"items":[{"title":"Lower third","UUID":"C9264107","Visible":true,"IsComment":false}]}')},
    expect_state={"overlays": {"C9264107": {"title": "Lower third", "visible": True, "comment": False}}})
telemetry(EC, "cameras", inbound_http={"path": "/getInputs", "body": (
    '{"items":[{"title":"Camera A","UUID":"CAM-A"},'
    '{"title":"EXAMPLE-MAC.LOCAL (macOS AV Output)","UUID":"EXAMPLE-MAC.LOCAL (macOS AV Output)"}]}')},
    expect_state={"cameras": {"CAM-A": {"title": "Camera A"},
                              "EXAMPLE-MAC.LOCAL (macOS AV Output)": {"title": "EXAMPLE-MAC.LOCAL (macOS AV Output)"}}})
telemetry(EC, "default-camera", inbound_http={"path": "/getDefaultCamera", "body": '["ZOOM_ACTIVE_SPEAKER"]'},
          expect_state={"default_camera": "ZOOM_ACTIVE_SPEAKER"})
telemetry(EC, "source-mode", inbound_http={"path": "/getCurrentMode", "body": '["video"]'},
          expect_state={"source_mode": "video"})
telemetry(EC, "videos", inbound_http={"path": "/getVideoList", "body": '{"items":[{"title":"-No recent items-","UUID":""}]}'},
          expect_state={"videos": {"0": {"title": "-No recent items-", "path": ""}}})
telemetry(EC, "sounds", inbound_http={"path": "/getSoundList", "body": (
    '{"items":[{"Volume":1,"Duration":5.5,"Action":"stop","title":"Applause","UUID":"2D574CA3"}]}')},
    expect_state={"sounds": {"2D574CA3": {"title": "Applause", "duration": 5.5}}})
telemetry(EC, "audio-filters", inbound_http={"path": "/getAudioFilterList", "body": '{"items":[{"title":"Noise Gate","UUID":"AF-1"}]}'},
          expect_state={"audio_filters": {"AF-1": {"title": "Noise Gate"}}})
telemetry(EC, "profiles", inbound_http={"path": "/getProfileList", "body": '{"items":[{"title":"Default Show","UUID":"B98B50E6"}]}'},
          expect_state={"profiles": {"B98B50E6": {"title": "Default Show"}}})
telemetry(EC, "channels", inbound_http={"path": "/getChannels", "body": '{"items":[]}'},
          expect_state={"channels": '{"items":[]}'})
telemetry(EC, "connection-status", inbound_http={"path": "/getConnectionStatus", "body": '["Pending"]'},
          expect_state={"connection_status": "Pending"})
telemetry(EC, "mute", inbound_http={"path": "/getMute", "body": '["yes"]'}, expect_state={"muted": True})
telemetry(EC, "live-demo", inbound_http={"path": "/getLiveDemo", "body": '["yes"]'}, expect_state={"live_demo": True})
telemetry(EC, "hide-show-ui", inbound_http={"path": "/getHideShowUI", "body": '["no"]'}, expect_state={"hiding_ui": False})
telemetry(EC, "viewers", inbound_http={"path": "/getViewers", "body": '["42"]'}, expect_state={"viewers": 42})
