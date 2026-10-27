# PLAYDECK REST API v1 (joy-playdeck): one vector per command, from the
# OpenAPI document's paths and request bodies (openapi-v1.yaml 1.0.0), and
# telemetry vectors for the status and project replies (StatusRoot,
# ProjectRoot schemas).
PD = "joy-playdeck"
C = "/api/v1/channels/2"
OK = {"status": 200, "body": '{"ok":true,"command":"<play|2>"}'}


def pd(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
    V.append({"spec": PD, "command": command, "input": input, "expect_request": req, **extra})


pd("get_health", {}, "GET", "/api/v1/health",
   http_reply={"status": 200, "body": '{"ok":true,"apiVersion":"1.0.0","appVersion":"4.1"}'},
   expect_result={"ok": {"kind": "value", "value": {"ok": True, "apiVersion": "1.0.0", "appVersion": "4.1"}}})
pd("get_status", {}, "GET", "/api/v1/status")
pd("get_project", {}, "GET", "/api/v1/project")
pd("get_channels", {}, "GET", "/api/v1/channels")
pd("get_channel", {"channel": 2}, "GET", C)
for v, route in [("cue", "cue"), ("play", "play"), ("fade_in", "fadein")]:
    p = f"{C}/{route}"
    pd(v, {"channel": 2}, "POST", p, "{}", **({"http_reply": OK, "expect_result": {"ok": {"kind": "ack"}}} if v == "play" else {}))
    pd(f"{v}_block", {"channel": 2, "block": 3}, "POST", p, '{"block":3}')
    pd(f"{v}_clip", {"channel": 2, "block": 3, "clip": 4}, "POST", p, '{"block":3,"clip":4}')
    pd(f"{v}_uid", {"channel": 2, "uid": 1207}, "POST", p, '{"uid":1207}')
    pd(f"{v}_flex", {"channel": 2, "pattern": 'b:main "x" c#1'}, "POST", p, '{"pattern":"b:main \\"x\\" c#1"}')
    pd(f"{v}_list", {"channel": 2, "list": "2", "block": 1, "clip": 5}, "POST", p, '{"list":2,"block":1,"clip":5}')
for v, route in [("toggle_pause", "pause"), ("hold", "hold"), ("resume", "resume"), ("stop", "stop"),
                 ("fade_out", "fadeout"), ("play_next", "playnext"), ("cue_next", "cuenext"),
                 ("play_next_block", "playnextblock"), ("cue_next_block", "cuenextblock"),
                 ("mute_channel", "mute"), ("unmute_channel", "unmute"), ("start_desktop", "desktop/start"),
                 ("stop_desktop", "desktop/stop"), ("stop_all_overlays", "overlays/stopall"),
                 ("stop_all_actions", "actions/stopall")]:
    pd(v, {"channel": 2}, "POST", f"{C}/{route}")
pd("seek", {"channel": 2, "timestamp": "-0:10:00"}, "POST", f"{C}/position", '{"timestamp":"-0:10:00"}')
pd("seek_marker", {"channel": 2, "marker": "chorus"}, "POST", f"{C}/position", '{"timestamp":"","marker":"chorus"}')
pd("select_block", {"channel": 2, "block": 3}, "POST", f"{C}/select", '{"block":3}')
pd("select_clip", {"channel": 2, "block": 3, "clip": 1}, "POST", f"{C}/select", '{"block":3,"clip":1}')
pd("select_uid", {"channel": 2, "uid": 88}, "POST", f"{C}/select", '{"uid":88}')
for verb in ["start", "stop", "toggle"]:
    pd(f"{verb}_overlay", {"channel": 2, "overlay": 12}, "POST", f"{C}/overlays/12/{verb}")
for verb in ["start", "stop"]:
    pd(f"{verb}_action", {"channel": 2, "action": 7}, "POST", f"{C}/actions/7/{verb}")
    pd(f"{verb}_stream", {"stream": 15}, "POST", f"/api/v1/streams/15/{verb}")
    pd(f"{verb}_recording", {"recording": 4}, "POST", f"/api/v1/recordings/4/{verb}")
for verb in ["mute", "unmute"]:
    pd(f"{verb}_input", {"input": 12}, "POST", f"/api/v1/inputs/12/{verb}")
pd("run_commands", {"commands": "<cue|1|1><wait|1000><play|1>"}, "POST", "/api/v1/commands",
   '{"raw":"<cue|1|1><wait|1000><play|1>"}',
   http_reply={"status": 200, "body": '{"ok":true,"executed":["<cue|1|1>","<wait|1000>","<play|1>"]}'},
   expect_result={"ok": {"kind": "ack"}})
pd("run_named_command", {"name": "startstream", "args": ["2"]}, "POST", "/api/v1/commands",
   '{"name":"startstream","args":["2"]}')

telemetry(PD, "status", inbound_http={"path": "/api/v1/status", "body": json.dumps({
    "Project": {"ProjectName": "Gala", "ProjectFilename": "gala.pdproj", "ClockTime": "19:30:00",
                "Timestamp": "2026-08-01 19:30:00", "TimestampUnix": 1785612600,
                "ProjectHash": "a3f5c8e1b2d94f0a8c7e6d5b4a392817", "LicenseEdition": "Studio",
                "LicenseCustomer": "Venue", "LicenseLease": ""},
    "Channel": [
        {"ChannelState": 2, "ChannelName": "Main", "BlockCount": 4, "TallyStatus": 1, "PreviewNote": "",
         "StageWidth": 1920, "StageHeight": 1080, "SelectedID": 31, "PlayState": 2, "ProgressVisible": True,
         "ScheduleVisible": False, "AutoplayVisible": False, "BlockName": "Opener", "BlockNumber": 1,
         "BlockID": 30, "BlockPosition": 12.5, "BlockPositionString": "00:00:12", "BlockDuration": 300,
         "BlockDurationString": "00:05:00", "BlockProgress": 0.04, "BlockRemainCaption": "Remain",
         "BlockRemain": 287.5, "BlockRemainString": "00:04:47", "BlockEndCaption": "End",
         "BlockEnd": 1785612887, "BlockEndString": "19:34:47", "BlockProgressAlert": False,
         "ClipName": "Intro", "ClipNumber": 1, "ClipID": 31, "ClipPosition": 12.5,
         "ClipPositionString": "00:00:12", "ClipDuration": 60, "ClipDurationString": "00:01:00",
         "ClipProgress": 0.2, "ClipProgressAlert": False, "ClipRemainCaption": "Remain", "ClipRemain": 47.5,
         "ClipRemainString": "00:00:47", "ClipEnd": 1785612647, "ClipEndString": "19:30:47",
         "ScheduleBlockName": "", "ScheduleTime": 0, "ScheduleTimeString": "", "ScheduleRemain": 0,
         "ScheduleRemainString": "", "ScheduleIcons": "", "ScheduleAlert": False, "AutoplayBlockName": "",
         "AutoplayTime": 0, "AutoplayTimeString": "", "AutoplayRemain": 0, "AutoplayRemainString": "",
         "AutoplayAlert": False, "OverlayActive3": True, "ActionActive12": True}],
    "DirectorView": [{"DirectorType": 1, "ChannelIDs": "1,2", "ShowTimer": True, "ShowName": True,
                      "ShowTally": False, "ShowNotes": False}],
    "ObjectState": {"Channel1": 2, "Stream1": 0}})},
    state_before={"channels": {"1": {"active_overlays": {"5": "true"}}}},
    expect_state={
        "project": {"name": "Gala", "filename": "gala.pdproj", "clock_time": "19:30:00",
                    "timestamp": "2026-08-01 19:30:00", "timestamp_unix": 1785612600,
                    "hash": "a3f5c8e1b2d94f0a8c7e6d5b4a392817", "license_edition": "Studio",
                    "license_customer": "Venue", "license_lease": ""},
        "channels": {"1": {
            "state": "ready", "name": "Main", "block_count": 4, "tally": 1, "preview_note": "",
            "stage_width": 1920, "stage_height": 1080, "selected_id": 31, "play_state": "playing",
            "progress_visible": True, "schedule_visible": False, "autoplay_visible": False,
            "block": {"name": "Opener", "number": 1, "id": 30, "position": 12.5, "position_text": "00:00:12",
                      "duration": 300.0, "duration_text": "00:05:00", "progress": 0.04, "remain_caption": "Remain",
                      "remain": 287.5, "remain_text": "00:04:47", "end_caption": "End", "end": 1785612887,
                      "end_text": "19:34:47", "progress_alert": False},
            "clip": {"name": "Intro", "number": 1, "id": 31, "position": 12.5, "position_text": "00:00:12",
                     "duration": 60.0, "duration_text": "00:01:00", "progress": 0.2, "progress_alert": False,
                     "remain_caption": "Remain", "remain": 47.5, "remain_text": "00:00:47", "end": 1785612647,
                     "end_text": "19:30:47"},
            "schedule": {"block_name": "", "time": 0, "time_text": "", "remain": 0.0, "remain_text": "",
                         "icons": "", "alert": False},
            "autoplay": {"block_name": "", "time": 0, "time_text": "", "remain": 0.0, "remain_text": "",
                         "alert": False},
            "active_overlays": {"3": "true"}, "active_actions": {"12": "true"}}},
        "director_views": {"1": {"type": 1, "channel_ids": "1,2", "show_timer": True, "show_name": True,
                                 "show_tally": False, "show_notes": False}},
        "objects": {"Channel1": 2, "Stream1": 0}})

_clip = {"CanPlay": True, "Name": "Intro", "Number": 1, "ID": 31, "ItemType": 1, "Active": True,
         "FileName": "C:\\media\\intro.mp4", "ItemInput": 0, "DurationString": "00:01:00", "FileSize": 1048576,
         "FileSizeString": "1 MB", "FileDateUnix": 1785000000, "FileDateString": "2026-07-25", "FileType": 1,
         "FileTypeString": "Video", "TransitionType": "Cut", "TransitionTime": 0.5, "DurationGfx": 0,
         "DurationGfxType": 0, "DurationGfxClockUnix": 0, "PauseMode": 0, "PauseFollow": 0, "PauseFollowTime": 0,
         "MuteAudio": False, "Gain": 0, "GainCustom": False, "EBUGain": -2.5, "EBUScanned": True,
         "ScaleType": "Fit", "ScaleAspect": "16:9", "CutIn": 0, "CutOut": 60, "CutOutRelative": False,
         "CropLeft": 0, "CropTop": 0, "CropRight": 0, "CropBottom": 0, "LoopType": 0, "LoopSeconds": 0,
         "LoopCount": 0, "LoopClock": 0, "LoopPartial": False, "VideoTrack": 1, "AudioTrack": 1,
         "SubtitleTrack": 0, "SubtitleOffset": 0}
telemetry(PD, "project", inbound_http={"path": "/api/v1/project", "body": json.dumps({
    "Project": {"ProjectName": "Gala"},
    "Channel": [{"ChannelState": 2, "ChannelName": "Main", "BlockCount": 1, "StageWidth": 1920,
                 "StageHeight": 1080,
                 "Block": [{"CanPlay": True, "Name": "Opener", "Number": 1, "ID": 30, "ClipCount": 1,
                            "Active": True, "StopType": "Stop", "DurationString": "00:01:00",
                            "StopFollowTime": 0, "LoopClips": 0, "BreakOption": 0, "JumpUniqueID": 0,
                            "ShuffleType": "None", "ShuffleClips": 0, "ScheduledUnix": 0,
                            "ScheduledWithDay": False, "ScheduledWithRepeat": False,
                            "ScheduledRepeatDuration": 0, "ScheduledMethod": 0, "ScheduledWeekdays": "",
                            "Clip": [_clip]}],
                 "Overlays": [{"ButtonName": "Lower third", "HasContent": True, "IsGroup": False,
                               "OverlayType": 1}],
                 "Actions": [{"ButtonName": "Start all", "HasContent": True}]}]})},
    state_before={"channels": {"1": {"blocks": {"2": {"name": "Removed"}}}}},
    expect_state={"channels": {"1": {
        "name": "Main", "block_count": 1, "stage_width": 1920, "stage_height": 1080,
        "blocks": {"1": {
            "can_play": True, "name": "Opener", "number": 1, "id": 30, "clip_count": 1, "active": True,
            "stop_type": "Stop", "duration_text": "00:01:00", "stop_follow_time": 0.0, "loop_clips": 0,
            "break_option": 0, "jump_uid": 0, "shuffle_type": "None", "shuffle_clips": 0, "scheduled_unix": 0,
            "scheduled_with_day": False, "scheduled_with_repeat": False, "scheduled_repeat_duration": 0.0,
            "scheduled_method": 0, "scheduled_weekdays": "",
            "clips": {"1": {
                "can_play": True, "name": "Intro", "number": 1, "id": 31, "item_type": 1, "active": True,
                "file_name": "C:\\media\\intro.mp4", "item_input": 0, "duration_text": "00:01:00",
                "file_size": 1048576, "file_size_text": "1 MB", "file_date_unix": 1785000000,
                "file_date_text": "2026-07-25", "file_type": 1, "file_type_text": "Video",
                "transition_type": "Cut", "transition_time": 0.5, "duration_gfx": 0.0, "duration_gfx_type": 0,
                "duration_gfx_clock_unix": 0, "pause_mode": 0, "pause_follow": 0, "pause_follow_time": 0.0,
                "mute_audio": False, "gain": 0.0, "gain_custom": False, "ebu_gain": -2.5, "ebu_scanned": True,
                "scale_type": "Fit", "scale_aspect": "16:9", "cut_in": 0.0, "cut_out": 60.0,
                "cut_out_relative": False, "crop_left": 0.0, "crop_top": 0.0, "crop_right": 0.0,
                "crop_bottom": 0.0, "loop_type": 0, "loop_seconds": 0.0, "loop_count": 0, "loop_clock": 0,
                "loop_partial": False, "video_track": 1, "audio_track": 1, "subtitle_track": 0,
                "subtitle_offset": 0.0}}}},
        "overlays": {"1": {"name": "Lower third", "has_content": True, "is_group": False, "type": 1}},
        "actions": {"1": {"name": "Start all", "has_content": True}}}}})
