# FreeShow (freeshow): every action is an HTTP POST to / on the REST port with
# the JSON body {"action": ACTION_ID, ...data}, the data keys and value types
# as the FreeShow API page lists them (freeshow.app/api, Examples: REST). Bodies
# are written here with json.dumps (compact separators, keys in the order
# given). Actions answer 204 with no body (issue #3700); get_ queries answer
# 200 with the data, returned as text.
FS = "freeshow"

_FS_ACK = {"ok": {"kind": "ack"}}


def _fs(command, input, action, data=None, **extra):
    body = json.dumps({"action": action, **(data or {})}, separators=(",", ":"))
    V.append({"spec": FS, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": "/", "body": body}, **extra})


def _fs_act(command, input, action, data=None):
    _fs(command, input, action, data, http_reply={"status": 204}, expect_result=_FS_ACK)


SHOW = "e1b2c3d4e5f"
# ── Project ──────────────────────────────────────────────────────────────
_fs_act("select_project", {"id": "proj1"}, "id_select_project", {"id": "proj1"})
_fs_act("select_project_by_index", {"index": 2}, "index_select_project", {"index": 2})
_fs_act("select_project_by_name", {"name": "Sunday"}, "name_select_project", {"value": "Sunday"})
_fs_act("next_project_item", {}, "next_project_item")
_fs_act("previous_project_item", {}, "previous_project_item")
_fs_act("select_project_item", {"index": 3}, "index_select_project_item", {"index": 3})
# ── Shows ────────────────────────────────────────────────────────────────
_fs_act("select_show_by_name", {"name": "Amazing Grace"}, "name_select_show", {"value": "Amazing Grace"})
_fs_act("start_show", {"id": SHOW}, "start_show", {"id": SHOW})
_fs_act("change_layout", {"show": SHOW, "layout": "lay2"}, "change_layout", {"showId": SHOW, "layoutId": "lay2"})
_fs_act("set_plain_text", {"id": SHOW, "text": "Verse 1\nLine \"one\""}, "set_plain_text",
        {"id": SHOW, "value": "Verse 1\nLine \"one\""})
_fs_act("rearrange_groups", {"show": SHOW, "from": 0, "to": 2}, "rearrange_groups",
        {"showId": SHOW, "from": 0, "to": 2})
_fs_act("add_group", {"show": SHOW, "group": "chorus"}, "add_group", {"showId": SHOW, "groupId": "chorus"})
_fs_act("set_template", {"id": "tmpl1"}, "set_template", {"id": "tmpl1"})
_fs_act("transpose_show_up", {"id": SHOW}, "transpose_show_up", {"id": SHOW})
_fs_act("transpose_show_down", {"id": SHOW}, "transpose_show_down", {"id": SHOW})
# ── Presentation ─────────────────────────────────────────────────────────
_fs_act("next_slide", {}, "next_slide")
_fs_act("previous_slide", {}, "previous_slide")
_fs_act("random_slide", {}, "random_slide")
# The example on issue #853: {action: "index_select_slide", index: 2}.
_fs_act("select_slide", {"index": 2}, "index_select_slide", {"index": 2})
_fs_act("select_slide_in_layout", {"show": SHOW, "layout": "lay1", "index": 4}, "index_select_slide",
        {"showId": SHOW, "layoutId": "lay1", "index": 4})
# The npm helper's example: name_select_slide with value "verse".
_fs_act("select_slide_by_name", {"name": "verse"}, "name_select_slide", {"value": "verse"})
_fs_act("select_group", {"id": "grp1"}, "id_select_group", {"id": "grp1"})
_fs_act("start_slide_recording", {}, "start_slide_recording")
# ── Clear ────────────────────────────────────────────────────────────────
for _a in ["restore_output", "clear_all", "clear_background", "clear_slide", "clear_overlays", "clear_audio",
           "clear_next_timer", "clear_drawing"]:
    _fs_act(_a, {}, _a)
_fs_act("clear_overlay", {"id": "ov1"}, "clear_overlay", {"id": "ov1"})
# ── Media ────────────────────────────────────────────────────────────────
_fs_act("start_camera", {"id": "cam-device-1"}, "start_camera", {"id": "cam-device-1"})
_fs_act("start_screen", {"id": "screen:0:0"}, "start_screen", {"id": "screen:0:0"})
_fs_act("play_media", {"path": "C:\\Media\\loop.mp4"}, "play_media", {"path": "C:\\Media\\loop.mp4"})
_fs_act("toggle_playing_media", {}, "toggle_playing_media")
# Seconds are written with three decimals, volume with two.
V.append({"spec": FS, "command": "video_seek", "input": {"seconds": 12.5},
          "expect_request": {"method": "POST", "target": "/", "body": '{"action":"video_seekto","seconds":12.500}'}})
_fs_act("start_effect", {"id": "fx1"}, "id_start_effect", {"id": "fx1"})
# ── Overlays and output ──────────────────────────────────────────────────
_fs_act("select_overlay_by_index", {"index": 1}, "index_select_overlay", {"index": 1})
_fs_act("select_overlay_by_name", {"name": "Logo"}, "name_select_overlay", {"value": "Logo"})
_fs_act("select_overlay", {"id": "ov2"}, "id_select_overlay", {"id": "ov2"})
# The reference from issue #3700.
_fs_act("start_scripture", {"reference": "Titus 2:2-2"}, "start_scripture", {"reference": "Titus 2:2-2"})
_fs_act("scripture_next", {}, "scripture_next")
_fs_act("scripture_previous", {}, "scripture_previous")
_fs_act("toggle_output_lock", {}, "lock_output")
_fs_act("set_output_lock", {"locked": False}, "lock_output", {"value": False})
_fs_act("toggle_output_windows", {}, "toggle_output_windows")
_fs_act("toggle_output", {"id": "out1"}, "toggle_output", {"id": "out1"})
# ── Visual and stage ─────────────────────────────────────────────────────
_fs_act("select_output_style", {"id": "style1"}, "id_select_output_style", {"id": "style1"})
_fs_act("change_output_style", {"output": "out1", "style": "style2"}, "change_output_style",
        {"outputId": "out1", "styleId": "style2"})
_fs_act("change_stage_output_layout", {"output": "out2", "stage_layout": "stage1"}, "change_stage_output_layout",
        {"outputId": "out2", "stageLayoutId": "stage1"})
_fs_act("change_transition", {"target": "text", "type": "fade", "duration": 500}, "change_transition",
        {"id": "text", "type": "fade", "duration": 500, "easing": "sine"})
_fs_act("select_stage_layout", {"id": "stage1"}, "id_select_stage_layout", {"id": "stage1"})
# ── Audio ────────────────────────────────────────────────────────────────
_fs_act("play_audio", {"path": "/music/a.mp3"}, "play_audio", {"path": "/music/a.mp3"})
_fs_act("pause_audio", {"path": "/music/a.mp3"}, "pause_audio", {"path": "/music/a.mp3"})
_fs_act("stop_audio", {"path": "/music/a.mp3"}, "stop_audio", {"path": "/music/a.mp3"})
V.append({"spec": FS, "command": "audio_seek", "input": {"seconds": 90},
          "expect_request": {"method": "POST", "target": "/", "body": '{"action":"audio_seekto","seconds":90.000}'}})
V.append({"spec": FS, "command": "set_volume", "input": {"volume": 0.5},
          "expect_request": {"method": "POST", "target": "/", "body": '{"action":"change_volume","volume":0.50}'},
          "http_reply": {"status": 204}, "expect_result": _FS_ACK})
_fs_act("start_audio_stream", {"id": "radio1"}, "start_audio_stream", {"id": "radio1"})
_fs_act("start_playlist", {"id": "pl1"}, "start_playlist", {"id": "pl1"})
_fs_act("start_playlist_by_name", {"name": "Walk-in"}, "name_start_playlist", {"value": "Walk-in"})
_fs_act("playlist_next", {}, "playlist_next")
_fs_act("start_metronome", {}, "start_metronome")
_fs_act("start_audio_effect", {"path": "/sfx/bell.wav"}, "start_audio_effect", {"path": "/sfx/bell.wav"})
# ── Timers ───────────────────────────────────────────────────────────────
_fs_act("start_timer_by_name", {"name": "Countdown"}, "name_start_timer", {"value": "Countdown"})
_fs_act("start_timer", {"id": "t1"}, "id_start_timer", {"id": "t1"})
_fs_act("start_slide_timers", {}, "start_slide_timers")
_fs_act("pause_timers", {}, "pause_timers")
_fs_act("stop_timers", {}, "stop_timers")
V.append({"spec": FS, "command": "timer_seek", "input": {"seconds": 300},
          "expect_request": {"method": "POST", "target": "/", "body": '{"action":"timer_seekto","seconds":300.000}'}})
_fs_act("pause_timer", {"id": "t1"}, "id_pause_timer", {"id": "t1"})
_fs_act("pause_timer_by_name", {"name": "Countdown"}, "name_pause_timer", {"value": "Countdown"})
_fs_act("stop_timer", {"id": "t1"}, "id_stop_timer", {"id": "t1"})
_fs_act("stop_timer_by_name", {"name": "Countdown"}, "name_stop_timer", {"value": "Countdown"})
# ── Functions and other ──────────────────────────────────────────────────
_fs_act("set_variable", {"name": "Speaker", "value": "Anna"}, "change_variable", {"name": "Speaker", "value": "Anna"})
_fs_act("increment_variable", {"name": "Count"}, "change_variable", {"name": "Count", "variableAction": "increment"})
_fs_act("decrement_variable", {"name": "Count"}, "change_variable", {"name": "Count", "variableAction": "decrement"})
_fs_act("start_trigger", {"id": "trig1"}, "start_trigger", {"id": "trig1"})
_fs_act("sync_drive", {}, "sync_drive")
_fs_act("sync_content_provider", {}, "sync_content_provider")
_fs_act("emit_action", {"emitter": "em1"}, "emit_action", {"emitter": "em1"})
_fs_act("toggle_log_song_usage", {}, "toggle_log_song_usage")
_fs_act("set_log_song_usage", {"enabled": True}, "toggle_log_song_usage", {"value": True})
_fs_act("run_action_by_name", {"name": "Service start"}, "name_run_action", {"value": "Service start"})
_fs_act("run_action", {"id": "act1"}, "run_action", {"id": "act1"})
_fs_act("toggle_action", {"id": "act1"}, "toggle_action", {"id": "act1"})
_fs_act("set_action_enabled", {"id": "act1", "enabled": False}, "toggle_action", {"id": "act1", "value": False})
# ── Edit ─────────────────────────────────────────────────────────────────
_fs_act("add_to_project", {"project": "proj1", "id": SHOW}, "add_to_project", {"projectId": "proj1", "id": SHOW})
_fs_act("create_show", {"text": "Line 1\n\nLine 2"}, "create_show", {"text": "Line 1\n\nLine 2"})
_fs_act("create_show_named", {"text": "Hello", "name": "Welcome", "category": "song"}, "create_show",
        {"text": "Hello", "name": "Welcome", "category": "song"})
_fs_act("create_project", {"name": "Easter"}, "create_project", {"name": "Easter"})
_fs_act("delete_project", {"id": "proj9"}, "delete_project", {"id": "proj9"})
_fs_act("remove_project_item", {"id": "proj1", "index": 0}, "remove_project_item", {"id": "proj1", "index": 0})
_fs_act("rename_project", {"id": "proj1", "name": "Easter Sunday"}, "rename_project",
        {"id": "proj1", "name": "Easter Sunday"})
# ── Queries ──────────────────────────────────────────────────────────────
_SHOWS = '{"e1b2c3d4e5f":{"name":"Amazing Grace","category":"song"}}'
_fs("get_shows", {}, "get_shows", http_reply={"status": 200, "body": _SHOWS},
    expect_result={"ok": {"kind": "value", "value": _SHOWS}})
for _a in ["get_projects", "get_output", "get_output_slide_text", "get_output_group_name",
           "get_playing_video_duration", "get_playing_video_time", "get_playing_video_time_left",
           "get_playing_audio_duration", "get_playing_audio_time", "get_playing_audio_time_left",
           "get_playing_audio_data", "get_variables", "get_timers", "get_playlists", "get_slide"]:
    _fs(_a, {}, _a)
for _a in ["get_show", "get_show_layout", "get_project", "get_plain_text", "get_groups", "get_playlist"]:
    _fs(_a, {"id": SHOW}, _a, {"id": SHOW})
_fs("get_dynamic_value", {"value": "{time_}"}, "get_dynamic_value", {"value": "{time_}"})
# A refused action is a device error, not success.
_fs("get_cleared", {}, "get_cleared", http_reply={"status": 500, "body": ""},
    expect_result={"error": {"error": "device_error", "code": "500"}})
