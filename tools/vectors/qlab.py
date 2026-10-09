# QLab: OSC over TCP 53000 with double-END SLIP framing (RFC 1055, as OSC 1.1
# requires; QLab 5 OSC dictionary, Getting Started). Addresses below are copied
# from the QLab 5 and QLab 4 OSC dictionaries. Replies are /reply/<address> with
# one JSON string argument (Replies from QLab); the engine is fed the unframed
# OSC packet, as for every other OSC vector.
Q = "qlab"


def slip(packet: bytes) -> bytes:
    """Double-END SLIP: END, payload with END and ESC escaped, END."""
    body = packet.replace(b"\xdb", b"\xdb\xdd").replace(b"\xc0", b"\xdb\xdc")
    return b"\xc0" + body + b"\xc0"


def q(command, input, address, *args, **extra):
    binary(Q, command, input, slip(osc(address, *args)), **extra)


def q_reply(address, data, status="ok"):
    """A /reply/... message as the dictionary describes it: one JSON string."""
    body = json.dumps({"workspace_id": "1B11984A-3EBC-4A9C-A004-B9E3AA32DA6B",
                       "address": address, "status": status, "data": data})
    # A query returns data (SPEC.md §5, json_path on an OSC argument).
    return osc("/reply" + address, ("s", body)), data


UID = "1B11984A-3EBC-4A9C-A004-B9E3AA32DA6B"

# Application messages.
reply, body = q_reply("/version", "5.5.4")
q("get_version", {}, "/version", device_reply_hex=hexs(reply),
  expect_result={"ok": {"kind": "value", "value": body}})
q("get_workspaces", {}, "/workspaces")
for stem, word, toggle, model in [
        ("midi_input", "midiInputEnabled", "toggleMidiInput", None),
        ("midi_output", "midiOutputEnabled", "toggleMidiOutput", None),
        ("msc_input", "mscInputEnabled", "toggleMscInput", None),
        ("msc_output", "mscOutputEnabled", "toggleMscOutput", None),
        ("sysex_input", "sysexInputEnabled", "toggleSysexInput", None),
        ("sysex_output", "sysexOutputEnabled", "toggleSysexOutput", None),
        ("timecode_input", "timecodeInputEnabled", "toggleTimecodeInput", None),
        ("timecode_output", "timecodeOutputEnabled", "toggleTimecodeOutput", None),
        ("dmx_output", "dmxOutputEnabled", "toggleDmxOutput", None),
        ("network_external_input", "networkExternalInputEnabled", "toggleNetworkExternalInput", None),
        ("network_external_output", "networkExternalOutputEnabled", "toggleNetworkExternalOutput", None),
        ("network_local_input", "networkLocalInputEnabled", "toggleNetworkLocalInput", None),
        ("network_local_output", "networkLocalOutputEnabled", "toggleNetworkLocalOutput", None),
        ("art_net", "artNetEnabled", "toggleArtNet", "qlab-4"),
        ("osc_input", "oscInputEnabled", "toggleOscInput", "qlab-4"),
        ("osc_output", "oscOutputEnabled", "toggleOscOutput", "qlab-4")]:
    m = {"model": model} if model else {}
    q(f"set_override_{stem}", {"enabled": False}, f"/overrides/{word}", ("i", 0), **m)
    q(f"get_override_{stem}", {}, f"/overrides/{word}", **m)
    q(f"toggle_override_{stem}", {}, f"/overrides/{toggle}", **m)

# Workspace messages, rootless.
q("go", {}, "/go", expect_result={"ok": {"kind": "unverified"}})
q("go_to_cue", {"cue": "12.5"}, "/go/12.5")
q("audition_go", {}, "/auditionGo")
q("audition_go_to_cue", {"cue": "A1"}, "/auditionGo/A1")
for command, address in [("stop", "/stop"), ("hard_stop", "/hardStop"), ("pause", "/pause"),
                         ("resume", "/resume"), ("panic", "/panic"), ("reset", "/reset"),
                         ("next_cue", "/playhead/next"), ("previous_cue", "/playhead/previous"),
                         ("next_sequence", "/playhead/nextSequence"),
                         ("previous_sequence", "/playhead/previousSequence"),
                         ("clear_playhead", "/playhead/none"), ("playhead_to_active", "/playhead/active"),
                         ("playhead_to_selected", "/playhead/selected"),
                         ("select_next", "/select/next"), ("select_previous", "/select/previous"),
                         ("toggle_edit_show_mode", "/toggleEditShowMode"),
                         ("toggle_live_fade_preview", "/toggleLiveFadePreview"),
                         ("save", "/save"), ("undo", "/undo"), ("redo", "/redo"),
                         ("delete_selected", "/delete/selected"), ("delete_active", "/delete/active"),
                         ("get_show_mode", "/showMode"), ("get_live_fade_preview", "/liveFadePreview"),
                         ("get_current_cue_list", "/currentCueList"),
                         ("get_current_cue_list_id", "/currentCueListID"),
                         ("get_base_path", "/basePath"),
                         ("get_double_go_window_remaining", "/doubleGoWindowRemaining")]:
    q(command, {}, address)
q("panic_in_time", {"seconds": 2.5}, "/panicInTime", ("f", 2.5))
q("set_playhead", {"cue": "10"}, "/playhead/10")
q("set_playhead_by_id", {"cue_id": UID}, f"/playheadID/{UID}")
q("set_playhead_by_id_qlab4", {"cue_id": "none"}, "/playheadId/none", model="qlab-4")
q("select_cue", {"cue": "7"}, "/select/7")
q("select_cue_by_id", {"cue_id": UID}, f"/select_id/{UID}")
q("set_show_mode", {"enabled": True}, "/showMode", ("i", 1))
q("set_live_fade_preview", {"enabled": True}, "/liveFadePreview", ("i", 1))
q("set_selection_is_playhead", {"enabled": True}, "/selectionIsPlayhead", ("i", 1), model="qlab-4")
q("get_selection_is_playhead", {}, "/selectionIsPlayhead", model="qlab-4")
q("toggle_selection_is_playhead", {}, "/toggleSelectionIsPlayhead", model="qlab-4")
q("set_current_cue_list", {"cue_list": "Main"}, "/currentCueList", ("s", "Main"))
q("set_current_cue_list_by_id", {"cue_list_id": UID}, f"/currentCueListID/{UID}")
# A denied reply fails the query (the dictionary's status values).
reply, _ = q_reply("/workspaces", None, status="denied")
q("get_workspaces", {}, "/workspaces", device_reply_hex=hexs(reply),
  expect_result={"error": {"error": "device_error"}})
reply, body = q_reply("/thump", "thump")
q("thump", {}, "/thump", device_reply_hex=hexs(reply), expect_result={"ok": {"kind": "value", "value": body}})
for stem, scope in [("cue_lists", "cueLists"), ("selected_cues", "selectedCues"),
                    ("running_cues", "runningCues"), ("running_or_paused_cues", "runningOrPausedCues")]:
    q(f"get_{stem}", {}, f"/{scope}")
    q(f"get_{stem}_shallow", {}, f"/{scope}/shallow")
    q(f"get_{stem}_unique_ids", {}, f"/{scope}/uniqueIDs")
    q(f"get_{stem}_unique_ids_shallow", {}, f"/{scope}/uniqueIDs/shallow")
q("delete_cue", {"cue": "3"}, "/delete/3")
q("delete_cue_by_id", {"cue_id": UID}, f"/delete_id/{UID}")
reply, body = q_reply("/new", "5E8F1C2A-0000-4000-8000-000000000001")
q("new_cue", {"cue_type": "midi file"}, "/new", ("s", "midi file"), device_reply_hex=hexs(reply),
  expect_result={"ok": {"kind": "value", "value": body}})
q("new_cue_after", {"cue_type": "audio", "after_cue_id": UID}, "/new", ("s", "audio"), ("s", UID))
q("move_cue", {"cue_id": UID, "new_index": 2}, f"/move/{UID}", ("i", 2))
q("move_cue_to_parent", {"cue_id": UID, "new_index": 0, "parent_cue_id": "ABCD-1234"},
  f"/move/{UID}", ("i", 0), ("s", "ABCD-1234"))
q("renumber", {"start_number": 1.0, "increment": 0.5}, "/renumber", ("f", 1.0), ("f", 0.5))
for stem, word in [("clear", "clear"), ("update_latest_cue", "updateLatestCue"),
                   ("update_originating_cues", "updateOriginatingCues"),
                   ("update_selected_cues", "updateSelectedCues"), ("new_cue_with_all", "newCueWithAll"),
                   ("new_cue_with_changes", "newCueWithChanges"), ("record_all_to_latest", "recordAllToLatest"),
                   ("record_all_to_selected", "recordAllToSelected"), ("revert", "revert"),
                   ("undo", "undo"), ("redo", "redo"), ("next_mode", "nextMode")]:
    q(f"dashboard_{stem}", {}, f"/dashboard/{word}")
# "/dashboard/setLight frontlight 50 5" (QLab 5 dictionary example).
q("dashboard_set_light", {"light": "frontlight", "level": 50.0, "time": 5.0},
  "/dashboard/setLight", ("s", "frontlight"), ("f", 50.0), ("f", 5.0))
q("dashboard_set_light_live", {"light": "myMover.cyan", "level": 75.0},
  "/dashboard/setLight/live", ("s", "myMover.cyan"), ("f", 75.0), ("f", 0.0))

# Cue messages: /cue/{cue_number}/... and the equivalent /cue_id/{id}/...
CUE_NUM, CUE_UID = "12.5", "0A1B2C3D-4E5F-6071-8293-A4B5C6D7E8F9"


def cue(command, method, extra=None, args=(), model=None, reply_data=None):
    m = {"model": model} if model else {}
    for name, key, val, root in [(command, "cue", CUE_NUM, "/cue/"),
                                 (command + "_by_id", "cue_id", CUE_UID, "/cue_id/")]:
        address = f"{root}{val}/{method}"
        x = dict(m)
        if reply_data is not None and key == "cue":
            reply, body = q_reply(address, reply_data)
            x.update(device_reply_hex=hexs(reply), expect_result={"ok": {"kind": "value", "value": body}})
        q(name, {key: val, **(extra or {})}, address, *args, **x)


for command, method, model in [
        ("start_cue", "start", None), ("stop_cue", "stop", None), ("hard_stop_cue", "hardStop", None),
        ("pause_cue", "pause", None), ("hard_pause_cue", "hardPause", None),
        ("toggle_pause_cue", "togglePause", None), ("resume_cue", "resume", None),
        ("load_cue", "load", None), ("preview_cue", "preview", None), ("go_cue", "go", None),
        ("panic_cue", "panic", None), ("reset_cue", "reset", None),
        ("load_and_set_playhead_cue", "loadAndSetPlayhead", None),
        ("start_cue_and_autoload_next", "startAndAutoloadNext", None),
        ("audition_go_cue", "auditionGo", None), ("audition_preview_cue", "auditionPreview", None),
        ("capture_timecode_cue", "captureTimecode", None),
        ("playlist_next", "playlist/next", None), ("playlist_previous", "playlist/previous", None)]:
    cue(command, method, model=model)
# "/cue/20/loadAt -30" loads to 30 seconds before the end (QLab 5 example).
cue("load_cue_at", "loadAt", {"seconds": -30.0}, [("f", -30.0)])
cue("load_cue_action_at", "loadActionAt", {"seconds": 15.0}, [("f", 15.0)])
cue("load_cue_file_at", "loadFileAt", {"seconds": 4.0}, [("f", 4.0)])
cue("panic_cue_in_time", "panicInTime", {"seconds": 3.0}, [("f", 3.0)])
cue("solo_cue_in_time", "soloCueInTime", {"seconds": 0.5}, [("f", 0.5)])

# Read/write properties: setter input and argument, then the bare query.
for stem, method, extra, args, model in [
        ("name", "name", {"name": "Overture"}, [("s", "Overture")], None),
        ("notes", "notes", {"notes": "Stand by sound"}, [("s", "Stand by sound")], None),
        ("number", "number", {"new_number": "12.6"}, [("s", "12.6")], None),
        ("armed", "armed", {"armed": False}, [("i", 0)], None),
        ("flagged", "flagged", {"flagged": True}, [("i", 1)], None),
        ("auto_load", "autoLoad", {"auto_load": True}, [("i", 1)], None),
        ("color_name", "colorName", {"color": "sky blue"}, [("s", "sky blue")], None),
        ("second_color_name", "secondColorName", {"color": "red"}, [("s", "red")], None),
        ("use_second_color", "useSecondColor", {"enabled": True}, [("i", 1)], None),
        ("continue_mode", "continueMode", {"mode": 2}, [("i", 2)], None),
        ("pre_wait", "preWait", {"seconds": 5.0}, [("f", 5.0)], None),
        ("post_wait", "postWait", {"seconds": 1.25}, [("f", 1.25)], None),
        ("duration", "duration", {"seconds": 30.0}, [("f", 30.0)], None),
        ("temp_duration", "tempDuration", {"seconds": 12.0}, [("f", 12.0)], None),
        ("duck_others", "duckOthers", {"enabled": True}, [("i", 1)], None),
        ("duck_time", "duckTime", {"seconds": 2.0}, [("f", 2.0)], None),
        ("duck_level", "duckLevel", {"level_db": -12.0}, [("f", -12.0)], None),
        ("fade_and_stop_others", "fadeAndStopOthers", {"mode": 1}, [("i", 1)], None),
        ("fade_and_stop_others_time", "fadeAndStopOthersTime", {"seconds": 3.5}, [("f", 3.5)], None),
        ("second_trigger_action", "secondTriggerAction", {"action": 4}, [("i", 4)], None),
        ("second_trigger_on_release", "secondTriggerOnRelease", {"enabled": False}, [("i", 0)], None),
        ("skip_if_disarmed", "skipIfDisarmed", {"enabled": True}, [("i", 1)], None),
        ("cue_target_number", "cueTargetNumber", {"target": "4"}, [("s", "4")], None),
        ("cue_target_id", "cueTargetID", {"target_id": "none"}, [("s", "none")], None),
        ("cue_target_id_qlab4", "cueTargetId", {"target_id": UID}, [("s", UID)], "qlab-4"),
        ("temp_cue_target_number", "tempCueTargetNumber", {"target": "9"}, [("s", "9")], None),
        ("file_target", "fileTarget", {"path": "~/path/to some/file.mov"}, [("s", "~/path/to some/file.mov")], None),
        ("target_mode", "targetMode", {"mode": 1}, [("i", 1)], None),
        ("patch_target_id", "patchTargetID", {"patch_id": "P1"}, [("s", "P1")], None),
        ("rate", "rate", {"rate": 2.0}, [("f", 2.0)], None),
        ("infinite_loop", "infiniteLoop", {"enabled": True}, [("i", 1)], None)]:
    base, suffix = (stem[:-6], "_qlab4") if stem.endswith("_qlab4") else (stem, "")
    cue(f"set_cue_{base}{suffix}", method, extra, args, model=model)
    cue(f"get_cue_{base}{suffix}", method, model=model,
        reply_data="Overture" if stem == "name" else None)
cue("set_cue_color_name_live", "colorName/live", {"color": "green"}, [("s", "green")])
# "/cue/1/rate/live 2" (QLab 5); QLab 4 used /liveRate.
cue("set_cue_rate_live", "rate/live", {"rate": 2.0}, [("f", 2.0)])
cue("set_cue_rate_live_qlab4", "liveRate", {"rate": 0.5}, [("f", 0.5)], model="qlab-4")
cue("set_cue_level", "level/0/2", {"input": 0, "output": 2, "level_db": -6.0}, [("f", -6.0)])
cue("get_cue_level", "level/1/0", {"input": 1, "output": 0})
cue("set_cue_level_live", "level/0/0/live", {"input": 0, "output": 0, "level_db": -3.0}, [("f", -3.0)])
cue("set_cue_slider_level", "sliderLevel/0", {"output": 0, "level_db": -10.0}, [("f", -10.0)])
cue("get_cue_slider_level", "sliderLevel/3", {"output": 3})
cue("set_cue_slider_level_live", "sliderLevel/1/live", {"output": 1, "level_db": 0.0}, [("f", 0.0)])
cue("set_cue_output_mute", "mute/channel/2", {"output": 2, "muted": True}, [("i", 1)])
cue("get_cue_output_mute", "mute/channel/2", {"output": 2})

# Read-only queries.
for stem, method in [
        ("type", "type"), ("unique_id", "uniqueID"), ("display_name", "displayName"),
        ("list_name", "listName"), ("default_name", "defaultName"), ("is_running", "isRunning"),
        ("is_paused", "isPaused"), ("is_loaded", "isLoaded"), ("is_broken", "isBroken"),
        ("is_action_running", "isActionRunning"), ("is_panicking", "isPanicking"),
        ("is_tailing_out", "isTailingOut"), ("is_overridden", "isOverridden"),
        ("is_auditioning", "isAuditioning"), ("is_warning", "isWarning"),
        ("action_elapsed", "actionElapsed"), ("percent_action_elapsed", "percentActionElapsed"),
        ("pre_wait_elapsed", "preWaitElapsed"), ("percent_pre_wait_elapsed", "percentPreWaitElapsed"),
        ("post_wait_elapsed", "postWaitElapsed"), ("percent_post_wait_elapsed", "percentPostWaitElapsed"),
        ("current_duration", "currentDuration"), ("current_file_time", "currentFileTime"),
        ("parent", "parent"), ("children", "children"), ("children_shallow", "children/shallow"),
        ("children_unique_ids", "children/uniqueIDs"),
        ("children_unique_ids_shallow", "children/uniqueIDs/shallow"),
        ("current_cue_target", "currentCueTarget"), ("current_cue_target_id", "currentCueTargetID"),
        ("current_cue_target_number", "currentCueTargetNumber"),
        ("has_cue_targets", "hasCueTargets"), ("has_file_targets", "hasFileTargets"),
        ("allows_editing_duration", "allowsEditingDuration"),
        ("max_time_in_cue_sequence", "maxTimeInCueSequence"), ("levels", "levels"),
        ("slider_levels", "sliderLevels")]:
    cue(f"get_cue_{stem}", method, reply_data=True if stem == "is_running" else None)

# Cue lists: /cue/{cue_number}/playhead {string} and friends; here the cue is a list.
LIST_NUM, LIST_UID = "Main", "7F000001-0000-4000-8000-00000000000A"
for name, key, val, root in [("set_cue_list_playhead", "cue_list", LIST_NUM, "/cue/"),
                             ("set_cue_list_playhead_by_id", "cue_list_id", LIST_UID, "/cue_id/")]:
    q(name, {key: val, "target": "5"}, f"{root}{val}/playhead", ("s", "5"))
for stem, method, model in [("get_cue_list_playhead", "playhead", None),
                            ("get_cue_list_playhead_id", "playheadID", None),
                            ("get_cue_list_playhead_id_qlab4", "playheadId", "qlab-4"),
                            ("cue_list_playhead_next", "playhead/next", None),
                            ("cue_list_playhead_previous", "playhead/previous", None)]:
    m = {"model": model} if model else {}
    q(stem, {"cue_list": LIST_NUM}, f"/cue/{LIST_NUM}/{method}", **m)
    q(stem + "_by_id", {"cue_list_id": LIST_UID}, f"/cue_id/{LIST_UID}/{method}", **m)

# Telemetry. On connecting (no passcode configured, so no /connect): /updates 1,
# the show control broadcast subscriptions (QLab 5.3), /workspaces, then the
# first poll of the active cues.
telemetry(Q, "playback-position",
          expect_connect_wire_hex=[hexs(slip(osc("/updates", ("i", 1)))), hexs(slip(osc("/listen/go"))),
                                   hexs(slip(osc("/listen/playhead"))), hexs(slip(osc("/listen/cue/start"))),
                                   hexs(slip(osc("/listen/cue/stop"))), hexs(slip(osc("/workspaces"))),
                                   hexs(slip(osc("/runningOrPausedCues/shallow")))],
          inbound_hex=hexs(osc(f"/update/workspace/{UID}/cueList/{LIST_UID}/playbackPosition", ("s", CUE_UID))),
          expect_state={"workspaces": {UID: {"cue_lists": {LIST_UID: {"playhead_id": CUE_UID}}}}})
# /qlab/event/workspace/playhead "{cue number}" "{cue name}" "{cue uniqueID}" "{cue type}"
telemetry(Q, "playhead",
          inbound_hex=hexs(osc("/qlab/event/workspace/playhead", ("s", "1.10"), ("s", "Preshow"),
                               ("s", CUE_UID), ("s", "Audio"))),
          expect_state={"playhead": {"number": "1.10", "name": "Preshow", "id": CUE_UID, "type": "Audio"}})
telemetry(Q, "go",
          inbound_hex=hexs(osc("/qlab/event/workspace/go", ("s", "53"), ("s", "Blackout"),
                               ("s", CUE_UID), ("s", "Light"))),
          expect_state={"last_go": {"number": "53", "name": "Blackout", "id": CUE_UID, "type": "Light"}})
telemetry(Q, "cue-start",
          inbound_hex=hexs(osc("/qlab/event/workspace/cue/start", ("s", "12.5"), ("s", "Overture"),
                               ("s", CUE_UID), ("s", "Audio"))),
          expect_state={"last_started": {"number": "12.5", "name": "Overture", "id": CUE_UID, "type": "Audio"},
                        "cues": {CUE_UID: {"running": True}}})
telemetry(Q, "cue-stop",
          inbound_hex=hexs(osc("/qlab/event/workspace/cue/stop", ("s", "12.5"), ("s", "Overture"),
                               ("s", CUE_UID), ("s", "Audio"))),
          expect_state={"last_stopped": {"number": "12.5", "name": "Overture", "id": CUE_UID, "type": "Audio"},
                        "cues": {CUE_UID: {"running": False}}})


# ── Full control ─────────────────────────────────────────────────────────
# Cue-type properties and workspace settings, from the QLab 5 dictionary
# (QLab 4 where marked): each setter with its documented argument types, each
# getter as the bare address, live forms with /live appended.
cue('set_cue_start_time', 'startTime', {'seconds': 1.5}, [('f', 1.5)])
cue('get_cue_start_time', 'startTime', {}, [])
cue('set_cue_end_time', 'endTime', {'seconds': 42.0}, [('f', 42.0)])
cue('get_cue_end_time', 'endTime', {}, [])
cue('set_cue_play_count', 'playCount', {'count': 3}, [('i', 3)])
cue('get_cue_play_count', 'playCount', {}, [])
cue('set_cue_last_slice_play_count', 'lastSlicePlayCount', {'count': -1}, [('i', -1)])
cue('get_cue_last_slice_play_count', 'lastSlicePlayCount', {}, [])
cue('set_cue_last_slice_infinite_loop', 'lastSliceInfiniteLoop', {'enabled': True}, [('i', 1)])
cue('get_cue_last_slice_infinite_loop', 'lastSliceInfiniteLoop', {}, [])
cue('set_cue_preserve_pitch', 'preservePitch', {'enabled': True}, [('i', 1)])
cue('get_cue_preserve_pitch', 'preservePitch', {}, [])
cue('set_cue_do_pitch_shift', 'doPitchShift', {'enabled': True}, [('i', 1)], model="qlab-4")
cue('get_cue_do_pitch_shift', 'doPitchShift', {}, [], model="qlab-4")
cue('set_cue_do_fade', 'doFade', {'enabled': True}, [('i', 1)])
cue('get_cue_do_fade', 'doFade', {}, [])
cue('set_cue_lock_fade_to_cue', 'lockFadeToCue', {'enabled': True}, [('i', 1)])
cue('get_cue_lock_fade_to_cue', 'lockFadeToCue', {}, [])
cue('set_cue_gang', 'gang/1/2', {'input': 1, 'output': 2, 'gang': 'A'}, [('s', 'A')])
cue('get_cue_gang', 'gang/1/2', {'input': 1, 'output': 2}, [])
cue('set_cue_slice_marker', 'sliceMarker/1', {'index': 1, 'time': 12.5, 'play_count': 2}, [('f', 12.5), ('i', 2)])
cue('get_cue_slice_marker', 'sliceMarker/1', {'index': 1}, [])
cue('set_cue_slice_marker_time', 'sliceMarker/1/time', {'index': 1, 'time': 12.5}, [('f', 12.5)])
cue('get_cue_slice_marker_time', 'sliceMarker/1/time', {'index': 1}, [])
cue('set_cue_slice_marker_play_count', 'sliceMarker/1/playCount', {'index': 1, 'play_count': 4}, [('i', 4)])
cue('get_cue_slice_marker_play_count', 'sliceMarker/1/playCount', {'index': 1}, [])
cue('get_cue_slice_markers', 'sliceMarkers', {}, [])
cue('add_cue_slice_marker', 'addSliceMarker', {'time': 30.0, 'play_count': 1}, [('f', 30.0), ('i', 1)])
cue('delete_cue_slice_marker', 'deleteSliceMarker/1', {'index': 1}, [])
cue('delete_cue_slice_markers', 'deleteSliceMarkers', {}, [])
cue('set_cue_default_levels', 'setDefaultLevels', {}, [])
cue('set_cue_silent_levels', 'setSilentLevels', {}, [])
cue('set_cue_output_solo', 'solo/2', {'output': 2, 'soloed': True}, [('i', 1)])
cue('get_cue_output_solo', 'solo/2', {'output': 2}, [])
cue('clear_cue_mutes', 'mute/clear', {}, [])
cue('clear_cue_solos', 'solo/clear', {}, [])
cue('get_cue_mute_channels', 'muteChannels', {}, [])
cue('get_cue_solo_channels', 'soloChannels', {}, [])
cue('get_cue_num_channels_in', 'numChannelsIn', {}, [])
cue('set_cue_input_channel_name', 'inputChannelName/1', {'input': 1, 'name': 'Vox'}, [('s', 'Vox')])
cue('get_cue_input_channel_name', 'inputChannelName/1', {'input': 1}, [])
cue('get_cue_levels_live', 'levels/live', {}, [])
cue('get_cue_slider_levels_live', 'sliderLevels/live', {}, [])
cue('get_cue_live_average_level', 'liveAverageLevel/1', {'output': 1}, [])
cue('set_cue_audio_output_patch_name', 'audioOutputPatchName', {'patch': 'Main PA'}, [('s', 'Main PA')])
cue('get_cue_audio_output_patch_name', 'audioOutputPatchName', {}, [])
cue('set_cue_audio_output_patch_number', 'audioOutputPatchNumber', {'patch': 2}, [('i', 2)])
cue('get_cue_audio_output_patch_number', 'audioOutputPatchNumber', {}, [])
cue('set_cue_audio_output_patch_id', 'audioOutputPatchID', {'patch_id': 'none'}, [('s', 'none')])
cue('get_cue_audio_output_patch_id', 'audioOutputPatchID', {}, [])
cue('set_cue_patch_qlab4', 'patch', {'patch': 2}, [('i', 2)], model="qlab-4")
cue('get_cue_patch_qlab4', 'patch', {}, [], model="qlab-4")
cue('set_cue_audio_map_id', 'audioMapID', {'map_id': 'none'}, [('s', 'none')])
cue('get_cue_audio_map_id', 'audioMapID', {}, [])
cue('set_cue_audio_map_name', 'audioMapName', {'map': 'Stage'}, [('s', 'Stage')])
cue('get_cue_audio_map_name', 'audioMapName', {}, [])
cue('set_cue_audio_map_number', 'audioMapNumber', {'map': 1}, [('i', 1)])
cue('get_cue_audio_map_number', 'audioMapNumber', {}, [])
cue('set_cue_object_level', 'objectLevel/0/A', {'row': 0, 'object': 'A', 'level_db': -6.0}, [('f', -6.0)])
cue('set_cue_object_level_live', 'objectLevel/0/A/live', {'row': 0, 'object': 'A', 'level_db': -6.0}, [('f', -6.0)])
cue('get_cue_object_level', 'objectLevel/0/A', {'row': 0, 'object': 'A'}, [])
cue('get_cue_object_levels', 'objectLevels', {}, [])
cue('get_cue_objects', 'objects', {}, [])
cue('set_cue_object_mute', 'mute/object/A', {'object': 'A', 'muted': True}, [('i', 1)])
cue('get_cue_object_mute', 'mute/object/A', {'object': 'A'}, [])
cue('set_cue_object_solo', 'solo/object/A', {'object': 'A', 'soloed': True}, [('i', 1)])
cue('get_cue_object_solo', 'solo/object/A', {'object': 'A'}, [])
cue('set_cue_object_position', 'object/A/position', {'object': 'A', 'x': 10.0, 'y': -20.0}, [('f', 10.0), ('f', -20.0)])
cue('set_cue_object_position_live', 'object/A/position/live', {'object': 'A', 'x': 10.0, 'y': -20.0}, [('f', 10.0), ('f', -20.0)])
cue('get_cue_object_position', 'object/A/position', {'object': 'A'}, [])
cue('set_cue_object_spread', 'object/A/spread', {'object': 'A', 'spread': 25.0}, [('f', 25.0)])
cue('set_cue_object_spread_live', 'object/A/spread/live', {'object': 'A', 'spread': 25.0}, [('f', 25.0)])
cue('get_cue_object_spread', 'object/A/spread', {'object': 'A'}, [])
cue('set_cue_audio_input_patch_name', 'audioInputPatchName', {'patch': 'Mics'}, [('s', 'Mics')])
cue('get_cue_audio_input_patch_name', 'audioInputPatchName', {}, [])
cue('set_cue_audio_input_patch_number', 'audioInputPatchNumber', {'patch': 1}, [('i', 1)])
cue('get_cue_audio_input_patch_number', 'audioInputPatchNumber', {}, [])
cue('set_cue_audio_input_patch_id', 'audioInputPatchID', {'patch_id': 'none'}, [('s', 'none')])
cue('get_cue_audio_input_patch_id', 'audioInputPatchID', {}, [])
cue('set_cue_channel_offset', 'channelOffset', {'offset': 2}, [('i', 2)])
cue('get_cue_channel_offset', 'channelOffset', {}, [])
cue('set_cue_channels', 'channels', {'channels': 2}, [('i', 2)])
cue('get_cue_channels', 'channels', {}, [])
cue('set_cue_anchor', 'anchor', {'x': 10.0, 'y': -20.0}, [('f', 10.0), ('f', -20.0)])
cue('set_cue_anchor_live', 'anchor/live', {'x': 10.0, 'y': -20.0}, [('f', 10.0), ('f', -20.0)])
cue('get_cue_anchor', 'anchor', {}, [])
cue('set_cue_anchor_x', 'anchor/x', {'x': 10.0}, [('f', 10.0)])
cue('set_cue_anchor_x_live', 'anchor/x/live', {'x': 10.0}, [('f', 10.0)])
cue('get_cue_anchor_x', 'anchor/x', {}, [])
cue('set_cue_anchor_y', 'anchor/y', {'y': -20.0}, [('f', -20.0)])
cue('set_cue_anchor_y_live', 'anchor/y/live', {'y': -20.0}, [('f', -20.0)])
cue('get_cue_anchor_y', 'anchor/y', {}, [])
cue('set_cue_origin', 'origin', {'x': 10.0, 'y': -20.0}, [('f', 10.0), ('f', -20.0)])
cue('get_cue_origin', 'origin', {}, [])
cue('set_cue_origin_x_qlab4', 'originX', {'x': 10.0}, [('f', 10.0)], model="qlab-4")
cue('get_cue_origin_x_qlab4', 'originX', {}, [], model="qlab-4")
cue('set_cue_origin_y_qlab4', 'originY', {'y': -20.0}, [('f', -20.0)], model="qlab-4")
cue('get_cue_origin_y_qlab4', 'originY', {}, [], model="qlab-4")
cue('set_cue_blend_mode', 'blendMode', {'mode': 'Multiply'}, [('s', 'Multiply')])
cue('get_cue_blend_mode', 'blendMode', {}, [])
cue('set_cue_clock_type', 'clockType', {'clock': 'video'}, [('s', 'video')])
cue('get_cue_clock_type', 'clockType', {}, [])
cue('set_cue_crop', 'crop', {'top': 10.0, 'bottom': 20.0, 'left': 30.0, 'right': 40.0}, [('f', 10.0), ('f', 20.0), ('f', 30.0), ('f', 40.0)])
cue('set_cue_crop_live', 'crop/live', {'top': 10.0, 'bottom': 20.0, 'left': 30.0, 'right': 40.0}, [('f', 10.0), ('f', 20.0), ('f', 30.0), ('f', 40.0)])
cue('get_cue_crop', 'crop', {}, [])
cue('set_cue_crop_top', 'cropTop', {'pixels': 12.0}, [('f', 12.0)])
cue('set_cue_crop_top_live', 'cropTop/live', {'pixels': 12.0}, [('f', 12.0)])
cue('get_cue_crop_top', 'cropTop', {}, [])
cue('set_cue_crop_bottom', 'cropBottom', {'pixels': 12.0}, [('f', 12.0)])
cue('set_cue_crop_bottom_live', 'cropBottom/live', {'pixels': 12.0}, [('f', 12.0)])
cue('get_cue_crop_bottom', 'cropBottom', {}, [])
cue('set_cue_crop_left', 'cropLeft', {'pixels': 12.0}, [('f', 12.0)])
cue('set_cue_crop_left_live', 'cropLeft/live', {'pixels': 12.0}, [('f', 12.0)])
cue('get_cue_crop_left', 'cropLeft', {}, [])
cue('set_cue_crop_right', 'cropRight', {'pixels': 12.0}, [('f', 12.0)])
cue('set_cue_crop_right_live', 'cropRight/live', {'pixels': 12.0}, [('f', 12.0)])
cue('get_cue_crop_right', 'cropRight', {}, [])
cue('get_cue_cue_size', 'cueSize', {}, [])
cue('set_cue_fill_stage', 'fillStage', {'enabled': True}, [('i', 1)])
cue('get_cue_fill_stage', 'fillStage', {}, [])
cue('set_cue_full_surface_qlab4', 'fullSurface', {'enabled': True}, [('i', 1)], model="qlab-4")
cue('get_cue_full_surface_qlab4', 'fullSurface', {}, [], model="qlab-4")
cue('set_cue_fill_style', 'fillStyle', {'style': 1}, [('i', 1)])
cue('get_cue_fill_style', 'fillStyle', {}, [])
cue('set_cue_hold_last_frame', 'holdLastFrame', {'enabled': True}, [('i', 1)])
cue('get_cue_hold_last_frame', 'holdLastFrame', {}, [])
cue('set_cue_layer', 'layer', {'layer': 10}, [('i', 10)])
cue('get_cue_layer', 'layer', {}, [])
cue('set_cue_opacity', 'opacity', {'opacity': 0.5}, [('f', 0.5)])
cue('set_cue_opacity_live', 'opacity/live', {'opacity': 0.5}, [('f', 0.5)])
cue('get_cue_opacity', 'opacity', {}, [])
cue('set_cue_preserve_aspect_ratio', 'preserveAspectRatio', {'enabled': True}, [('i', 1)])
cue('get_cue_preserve_aspect_ratio', 'preserveAspectRatio', {}, [])
cue('set_cue_quaternion', 'quaternion', {'a': 1.0, 'b': 0.0, 'c': 0.0, 'd': 0.0}, [('f', 1.0), ('f', 0.0), ('f', 0.0), ('f', 0.0)])
cue('get_cue_quaternion', 'quaternion', {}, [])
cue('reset_cue_rotation', 'resetRotation', {}, [])
cue('set_cue_rotate_x', 'rotate/x', {'degrees': 45.0}, [('f', 45.0)])
cue('set_cue_rotate_x_live', 'rotate/x/live', {'degrees': 45.0}, [('f', 45.0)])
cue('set_cue_rotate_y', 'rotate/y', {'degrees': 45.0}, [('f', 45.0)])
cue('set_cue_rotate_y_live', 'rotate/y/live', {'degrees': 45.0}, [('f', 45.0)])
cue('set_cue_rotate_z', 'rotate/z', {'degrees': 45.0}, [('f', 45.0)])
cue('set_cue_rotate_z_live', 'rotate/z/live', {'degrees': 45.0}, [('f', 45.0)])
cue('set_cue_rotate_x_qlab4', 'rotateX', {'degrees': 45.0}, [('f', 45.0)], model="qlab-4")
cue('set_cue_rotate_y_qlab4', 'rotateY', {'degrees': 45.0}, [('f', 45.0)], model="qlab-4")
cue('set_cue_rotate_z_qlab4', 'rotateZ', {'degrees': 45.0}, [('f', 45.0)], model="qlab-4")
cue('set_cue_scale', 'scale', {'x': 1.5, 'y': 0.75}, [('f', 1.5), ('f', 0.75)])
cue('set_cue_scale_live', 'scale/live', {'x': 1.5, 'y': 0.75}, [('f', 1.5), ('f', 0.75)])
cue('get_cue_scale', 'scale', {}, [])
cue('set_cue_scale_x', 'scale/x', {'x': 1.5}, [('f', 1.5)])
cue('set_cue_scale_x_live', 'scale/x/live', {'x': 1.5}, [('f', 1.5)])
cue('get_cue_scale_x', 'scale/x', {}, [])
cue('set_cue_scale_y', 'scale/y', {'y': 0.75}, [('f', 0.75)])
cue('set_cue_scale_y_live', 'scale/y/live', {'y': 0.75}, [('f', 0.75)])
cue('get_cue_scale_y', 'scale/y', {}, [])
cue('set_cue_scale_x_qlab4', 'scaleX', {'x': 1.5}, [('f', 1.5)], model="qlab-4")
cue('get_cue_scale_x_qlab4', 'scaleX', {}, [], model="qlab-4")
cue('set_cue_scale_y_qlab4', 'scaleY', {'y': 0.75}, [('f', 0.75)], model="qlab-4")
cue('get_cue_scale_y_qlab4', 'scaleY', {}, [], model="qlab-4")
cue('set_cue_smooth', 'smooth', {'enabled': True}, [('i', 1)])
cue('get_cue_smooth', 'smooth', {}, [])
cue('set_cue_stage_id', 'stageID', {'stage_id': 'none'}, [('s', 'none')])
cue('get_cue_stage_id', 'stageID', {}, [])
cue('set_cue_stage_name', 'stageName', {'stage': 'Main'}, [('s', 'Main')])
cue('get_cue_stage_name', 'stageName', {}, [])
cue('set_cue_stage_number', 'stageNumber', {'stage': 1}, [('i', 1)])
cue('get_cue_stage_number', 'stageNumber', {}, [])
cue('set_cue_surface_id_qlab4', 'surfaceID', {'surface': 2}, [('i', 2)], model="qlab-4")
cue('get_cue_surface_id_qlab4', 'surfaceID', {}, [], model="qlab-4")
cue('set_cue_surface_name_qlab4', 'surfaceName', {'surface': 'Main'}, [('s', 'Main')], model="qlab-4")
cue('get_cue_surface_name_qlab4', 'surfaceName', {}, [], model="qlab-4")
cue('set_cue_translation', 'translation', {'x': 100.0, 'y': -50.0}, [('f', 100.0), ('f', -50.0)])
cue('set_cue_translation_live', 'translation/live', {'x': 100.0, 'y': -50.0}, [('f', 100.0), ('f', -50.0)])
cue('get_cue_translation', 'translation', {}, [])
cue('set_cue_translation_x', 'translation/x', {'x': 100.0}, [('f', 100.0)])
cue('set_cue_translation_x_live', 'translation/x/live', {'x': 100.0}, [('f', 100.0)])
cue('get_cue_translation_x', 'translation/x', {}, [])
cue('set_cue_translation_y', 'translation/y', {'y': -50.0}, [('f', -50.0)])
cue('set_cue_translation_y_live', 'translation/y/live', {'y': -50.0}, [('f', -50.0)])
cue('get_cue_translation_y', 'translation/y', {}, [])
cue('set_cue_translation_x_qlab4', 'translationX', {'x': 100.0}, [('f', 100.0)], model="qlab-4")
cue('get_cue_translation_x_qlab4', 'translationX', {}, [], model="qlab-4")
cue('set_cue_translation_y_qlab4', 'translationY', {'y': -50.0}, [('f', -50.0)], model="qlab-4")
cue('get_cue_translation_y_qlab4', 'translationY', {}, [], model="qlab-4")
cue('get_cue_video_effects', 'videoEffects', {}, [])
cue('add_cue_video_effect', 'videoEffects/add', {'effect': 'ColorControls'}, [('s', 'ColorControls')])
cue('insert_cue_video_effect', 'videoEffects/insert', {'effect': 'GaussianBlur', 'index': 0}, [('s', 'GaussianBlur'), ('i', 0)])
cue('delete_cue_video_effect', 'videoEffectIndex/0/delete', {'effect': 0}, [])
cue('move_cue_video_effect', 'videoEffectIndex/0/move', {'effect': 0, 'new_index': 2}, [('i', 2)])
cue('set_cue_video_effect_enabled', 'videoEffectIndex/0/enabled', {'effect': 0, 'enabled': True}, [('i', 1)])
cue('get_cue_video_effect_enabled', 'videoEffectIndex/0/enabled', {'effect': 0}, [])
cue('set_cue_video_effect_parameter', 'videoEffectIndex/0/parameter/inputRadius', {'effect': 0, 'key': 'inputRadius', 'value': 4.0}, [('f', 4.0)])
cue('set_cue_video_effect_parameter_live', 'videoEffectIndex/0/parameter/inputRadius/live', {'effect': 0, 'key': 'inputRadius', 'value': 4.0}, [('f', 4.0)])
cue('get_cue_video_effect_parameter', 'videoEffectIndex/0/parameter/inputRadius', {'effect': 0, 'key': 'inputRadius'}, [])
cue('set_cue_video_effect_parameters', 'videoEffectIndex/0/parameters', {'effect': 0, 'parameters': '{"inputBrightness":0.25}'}, [('s', '{"inputBrightness":0.25}')])
cue('set_cue_video_effect_parameters_live', 'videoEffectIndex/0/parameters/live', {'effect': 0, 'parameters': '{"inputBrightness":0.25}'}, [('s', '{"inputBrightness":0.25}')])
cue('get_cue_video_effect_parameters', 'videoEffectIndex/0/parameters', {'effect': 0}, [])
cue('get_cue_audio_track_id', 'audioTrackID', {}, [])
cue('get_cue_audio_track_formats', 'audioTrackFormats', {}, [])
cue('set_cue_video_input_patch_name', 'videoInputPatchName', {'patch': 'Cam 1'}, [('s', 'Cam 1')])
cue('get_cue_video_input_patch_name', 'videoInputPatchName', {}, [])
cue('set_cue_video_input_patch_number', 'videoInputPatchNumber', {'patch': 1}, [('i', 1)])
cue('get_cue_video_input_patch_number', 'videoInputPatchNumber', {}, [])
cue('set_cue_video_input_patch_id', 'videoInputPatchID', {'patch_id': 'none'}, [('s', 'none')])
cue('get_cue_video_input_patch_id', 'videoInputPatchID', {}, [])
cue('set_cue_camera_patch_qlab4', 'cameraPatch', {'patch': 2}, [('i', 2)], model="qlab-4")
cue('get_cue_camera_patch_qlab4', 'cameraPatch', {}, [], model="qlab-4")
cue('set_cue_text', 'text', {'text': 'Act One'}, [('s', 'Act One')])
cue('set_cue_text_live', 'text/live', {'text': 'Act One'}, [('s', 'Act One')])
cue('get_cue_text', 'text', {}, [])
cue('set_cue_text_live_qlab4', 'liveText', {'text': 'Act One'}, [('s', 'Act One')], model="qlab-4")
cue('set_cue_fixed_width', 'fixedWidth', {'width': 800.0}, [('f', 800.0)])
cue('get_cue_fixed_width', 'fixedWidth', {}, [])
cue('set_cue_text_format', 'text/format', {'format': '[{"fontSize":72}]'}, [('s', '[{"fontSize":72}]')])
cue('set_cue_text_format_live', 'text/format/live', {'format': '[{"fontSize":72}]'}, [('s', '[{"fontSize":72}]')])
cue('get_cue_text_format', 'text/format', {}, [])
cue('set_cue_text_alignment', 'text/format/alignment', {'alignment': 'center'}, [('s', 'center')])
cue('set_cue_text_alignment_live', 'text/format/alignment/live', {'alignment': 'center'}, [('s', 'center')])
cue('get_cue_text_alignment', 'text/format/alignment', {}, [])
cue('set_cue_text_color', 'text/format/color', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('set_cue_text_color_live', 'text/format/color/live', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('get_cue_text_color', 'text/format/color', {}, [])
cue('set_cue_text_background_color', 'text/format/backgroundColor', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('set_cue_text_background_color_live', 'text/format/backgroundColor/live', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('get_cue_text_background_color', 'text/format/backgroundColor', {}, [])
cue('set_cue_text_shadow_color', 'text/format/shadowColor', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('set_cue_text_shadow_color_live', 'text/format/shadowColor/live', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('get_cue_text_shadow_color', 'text/format/shadowColor', {}, [])
cue('set_cue_text_strikethrough_color', 'text/format/strikethroughColor', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('set_cue_text_strikethrough_color_live', 'text/format/strikethroughColor/live', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('get_cue_text_strikethrough_color', 'text/format/strikethroughColor', {}, [])
cue('set_cue_text_underline_color', 'text/format/underlineColor', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('set_cue_text_underline_color_live', 'text/format/underlineColor/live', {'red': 1.0, 'green': 0.5, 'blue': 0.0, 'alpha': 1.0}, [('f', 1.0), ('f', 0.5), ('f', 0.0), ('f', 1.0)])
cue('get_cue_text_underline_color', 'text/format/underlineColor', {}, [])
cue('get_cue_text_font_family', 'text/format/fontFamily', {}, [])
cue('get_cue_text_font_style', 'text/format/fontStyle', {}, [])
cue('set_cue_text_font_family_and_style', 'text/format/fontFamilyAndStyle', {'family': 'Helvetica', 'style': 'Bold'}, [('s', 'Helvetica'), ('s', 'Bold')])
cue('set_cue_text_font_family_and_style_live', 'text/format/fontFamilyAndStyle/live', {'family': 'Helvetica', 'style': 'Bold'}, [('s', 'Helvetica'), ('s', 'Bold')])
cue('get_cue_text_font_family_and_style', 'text/format/fontFamilyAndStyle', {}, [])
cue('set_cue_text_font_name', 'text/format/fontName', {'font': 'Helvetica-Bold'}, [('s', 'Helvetica-Bold')])
cue('set_cue_text_font_name_live', 'text/format/fontName/live', {'font': 'Helvetica-Bold'}, [('s', 'Helvetica-Bold')])
cue('get_cue_text_font_name', 'text/format/fontName', {}, [])
cue('set_cue_text_font_size', 'text/format/fontSize', {'size': 72.0}, [('f', 72.0)])
cue('set_cue_text_font_size_live', 'text/format/fontSize/live', {'size': 72.0}, [('f', 72.0)])
cue('get_cue_text_font_size', 'text/format/fontSize', {}, [])
cue('set_cue_text_line_spacing', 'text/format/lineSpacing', {'spacing': 1.2}, [('f', 1.2)])
cue('set_cue_text_line_spacing_live', 'text/format/lineSpacing/live', {'spacing': 1.2}, [('f', 1.2)])
cue('get_cue_text_line_spacing', 'text/format/lineSpacing', {}, [])
cue('set_cue_text_shadow_blur_radius', 'text/format/shadowBlurRadius', {'radius': 4.0}, [('f', 4.0)])
cue('set_cue_text_shadow_blur_radius_live', 'text/format/shadowBlurRadius/live', {'radius': 4.0}, [('f', 4.0)])
cue('get_cue_text_shadow_blur_radius', 'text/format/shadowBlurRadius', {}, [])
cue('set_cue_text_shadow_offset', 'text/format/shadowOffset', {'width': 3.0, 'height': -3.0}, [('f', 3.0), ('f', -3.0)])
cue('set_cue_text_shadow_offset_live', 'text/format/shadowOffset/live', {'width': 3.0, 'height': -3.0}, [('f', 3.0), ('f', -3.0)])
cue('get_cue_text_shadow_offset', 'text/format/shadowOffset', {}, [])
cue('set_cue_text_strikethrough_style', 'text/format/strikethroughStyle', {'style': 'single'}, [('s', 'single')])
cue('set_cue_text_strikethrough_style_live', 'text/format/strikethroughStyle/live', {'style': 'single'}, [('s', 'single')])
cue('get_cue_text_strikethrough_style', 'text/format/strikethroughStyle', {}, [])
cue('set_cue_text_underline_style', 'text/format/underlineStyle', {'style': 'double'}, [('s', 'double')])
cue('set_cue_text_underline_style_live', 'text/format/underlineStyle/live', {'style': 'double'}, [('s', 'double')])
cue('get_cue_text_underline_style', 'text/format/underlineStyle', {}, [])
cue('get_cue_text_output_size', 'text/outputSize', {}, [])
cue('set_cue_light_command_text', 'lightCommandText', {'text': 'front = 50'}, [('s', 'front = 50')])
cue('get_cue_light_command_text', 'lightCommandText', {}, [])
cue('set_cue_always_collate', 'alwaysCollate', {'enabled': True}, [('i', 1)])
cue('get_cue_always_collate', 'alwaysCollate', {}, [])
cue('set_cue_subcontroller', 'subcontroller', {'enabled': True}, [('i', 1)])
cue('get_cue_subcontroller', 'subcontroller', {}, [])
cue('collate_and_start_cue', 'collateAndStart', {}, [])
cue('prune_cue_light_commands', 'pruneCommands', {}, [])
cue('sort_cue_light_commands', 'safeSortCommands', {}, [])
cue('set_cue_light', 'setLight', {'light': 'front', 'level': 75.0}, [('s', 'front'), ('f', 75.0)])
cue('remove_cue_light_commands', 'removeLightCommandsMatching', {'command': 'front = 50'}, [('s', 'front = 50')])
cue('replace_cue_light_command', 'replaceLightCommand', {'old_command': 'front = 50', 'new_command': 'front = 80'}, [('s', 'front = 50'), ('s', 'front = 80')])
cue('update_cue_light_command_qlab4', 'updateLightCommand', {'light': 'front', 'level': 60.0}, [('s', 'front'), ('f', 60.0)], model="qlab-4")
cue('set_cue_levels_mode', 'levelsMode', {'mode': 1}, [('i', 1)])
cue('get_cue_levels_mode', 'levelsMode', {}, [])
cue('set_cue_mode', 'mode', {'mode': 3}, [('i', 3)])
cue('get_cue_mode', 'mode', {}, [])
cue('set_cue_geo_mode', 'geoMode', {'mode': 0}, [('i', 0)])
cue('get_cue_geo_mode', 'geoMode', {}, [])
cue('set_cue_fade_type', 'fadeType', {'type': 1}, [('i', 1)])
cue('get_cue_fade_type', 'fadeType', {}, [])
cue('set_cue_stop_target_when_done', 'stopTargetWhenDone', {'enabled': True}, [('i', 1)])
cue('get_cue_stop_target_when_done', 'stopTargetWhenDone', {}, [])
cue('set_cue_do_opacity', 'doOpacity', {'enabled': True}, [('i', 1)])
cue('get_cue_do_opacity', 'doOpacity', {}, [])
cue('set_cue_do_rate', 'doRate', {'enabled': True}, [('i', 1)])
cue('get_cue_do_rate', 'doRate', {}, [])
cue('set_cue_do_rotation', 'doRotation', {'enabled': True}, [('i', 1)])
cue('get_cue_do_rotation', 'doRotation', {}, [])
cue('set_cue_do_scale', 'doScale', {'enabled': True}, [('i', 1)])
cue('get_cue_do_scale', 'doScale', {}, [])
cue('set_cue_do_translation', 'doTranslation', {'enabled': True}, [('i', 1)])
cue('get_cue_do_translation', 'doTranslation', {}, [])
cue('set_cue_do_level', 'doLevel/0/2', {'row': 0, 'column': 2, 'enabled': True}, [('i', 1)])
cue('get_cue_do_level', 'doLevel/0/2', {'row': 0, 'column': 2}, [])
cue('set_cue_will_fade', 'willFade/0/2', {'row': 0, 'column': 2, 'enabled': True}, [('i', 1)])
cue('get_cue_will_fade', 'willFade/0/2', {'row': 0, 'column': 2}, [])
cue('get_cue_do_levels', 'doLevel', {}, [])
cue('set_cue_rotation', 'rotation', {'degrees': 90.0}, [('f', 90.0)])
cue('get_cue_rotation', 'rotation', {}, [])
cue('set_cue_rotation_type', 'rotationType', {'type': 3}, [('i', 3)])
cue('get_cue_rotation_type', 'rotationType', {}, [])
cue('set_cue_path_width', 'pathWidth', {'width': 100.0}, [('f', 100.0)])
cue('get_cue_path_width', 'pathWidth', {}, [])
cue('set_cue_path_height', 'pathHeight', {'height': 100.0}, [('f', 100.0)])
cue('get_cue_path_height', 'pathHeight', {}, [])
cue('set_cue_path_smooth', 'pathSmooth', {'enabled': True}, [('i', 1)])
cue('get_cue_path_smooth', 'pathSmooth', {}, [])
cue('set_cue_geometry_from_target', 'setGeometryFromTarget', {}, [])
cue('set_cue_levels_from_target', 'setLevelsFromTarget', {}, [])
cue('set_cue_audio_map_target_id', 'audioMapTargetID', {'map_id': '5E8F1C2A-0000-4000-8000-0000000000C1'}, [('s', '5E8F1C2A-0000-4000-8000-0000000000C1')])
cue('get_cue_audio_map_target_id', 'audioMapTargetID', {}, [])

# State. Workspace and cue IDs as QLab reports them (uniqueID, UUID strings).
WS = UID
L1, C1, G1, C2 = LIST_UID, CUE_UID, "6A000001-0000-4000-8000-0000000000C3", "6A000001-0000-4000-8000-0000000000C4"
VFK = '["uniqueID","number","name","listName","displayName","type","colorName","colorName/live","secondColorName","useSecondColor","flagged","armed","notes","autoLoad","continueMode","preWait","postWait","duration","currentDuration","parent","cueTargetID","cueTargetNumber","fileTarget","targetMode","patchTargetID","duckOthers","duckLevel","duckTime","fadeAndStopOthers","fadeAndStopOthersTime","secondTriggerAction","secondTriggerOnRelease","skipIfDisarmed","rate","infiniteLoop","hasFileTargets","hasCueTargets","levels","sliderLevels","isRunning","isPaused","isLoaded","isBroken","isAuditioning","isPanicking","isTailingOut","isActionRunning","isOverridden","isWarning","actionElapsed","percentActionElapsed","preWaitElapsed","percentPreWaitElapsed","postWaitElapsed","percentPostWaitElapsed","currentFileTime","startTime","endTime","playCount","lastSlicePlayCount","lastSliceInfiniteLoop","preservePitch","doPitchShift","doFade","lockFadeToCue","sliceMarkers","muteChannels","soloChannels","numChannelsIn","audioOutputPatchName","audioOutputPatchNumber","audioOutputPatchID","patch","audioMapID","objects","audioInputPatchName","audioInputPatchID","channelOffset","channels","anchor/x","anchor/y","blendMode","clockType","cropTop","cropBottom","cropLeft","cropRight","cueSize","fillStage","fullSurface","fillStyle","holdLastFrame","layer","opacity","preserveAspectRatio","quaternion","scale/x","scale/y","smooth","stageID","stageName","surfaceID","translation/x","translation/y","videoEffects","videoInputPatchName","videoInputPatchID","cameraPatch","text","fixedWidth","text/format","text/format/alignment","lightCommandText","alwaysCollate","subcontroller","levelsMode","mode","geoMode","fadeType","stopTargetWhenDone","doOpacity","doRate","doRotation","doScale","doTranslation","doLevel","rotation","rotationType","pathWidth","pathHeight","pathSmooth","audioMapTargetID","cueTargetId"]'
TIMING_KEYS = '["isRunning","isPaused","isLoaded","isBroken","isAuditioning","isPanicking","isTailingOut","isActionRunning","isOverridden","isWarning","actionElapsed","percentActionElapsed","preWaitElapsed","percentPreWaitElapsed","postWaitElapsed","percentPostWaitElapsed","currentFileTime","currentDuration"]'


def framed(address, *args):
    return hexs(slip(osc(address, *args)))


def reply(address, data, status="ok", workspace=True):
    body = {"address": address, "status": status, "data": data}
    if workspace:
        body = {"workspace_id": WS, **body}
    return hexs(osc("/reply" + address, ("s", json.dumps(body))))


def workspace_reads(ws):
    """What a workspace's discovery or update reads, in the spec's order (QLab 5)."""
    reads = [framed(f"/workspace/{ws}/{m}") for m in ['cueLists', 'showMode']]
    reads += [framed(f"/{m}") for m in []]
    if False:
        reads.append(framed(f"/workspace/{ws}/settings/audio/patchList"))
    return reads


# /workspaces: each open workspace is kept and read.
telemetry(Q, "workspaces",
          inbound_hex=reply("/workspaces", [{"uniqueID": WS, "displayName": "show", "port": 53000,
                                              "udpReplyPort": 53001, "version": "5.5.4"}], workspace=False),
          expect_state={"workspaces": {WS: {"name": "show", "version": "5.5.4", "port": 53000}}},
          expect_then_send_hex=workspace_reads(WS))
telemetry(Q, "workspace-update", inbound_hex=hexs(osc(f"/update/workspace/{WS}")),
          expect_state={}, expect_then_send_hex=workspace_reads(WS))
telemetry(Q, "root-group-update", inbound_hex=hexs(osc(f"/update/workspace/{WS}/cue_id/__root__")),
          expect_state={}, expect_then_send_hex=[framed(f"/workspace/{WS}/cueLists")])
telemetry(Q, "workspace-disconnect", inbound_hex=hexs(osc(f"/update/workspace/{WS}/disconnect")),
          state_before={"workspaces": {WS: {"name": "show"}, "OTHER-1": {"name": "other"}}},
          expect_state={"workspaces": {"OTHER-1": {"name": "other"}}})
# A cue changed: its values and its children are read.
telemetry(Q, "cue-update", inbound_hex=hexs(osc(f"/update/workspace/{WS}/cue_id/{C1}")),
          expect_state={},
          expect_then_send_hex=[framed(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", ("s", VFK)),
                                framed(f"/workspace/{WS}/cue_id/{C1}/children/shallow")])
# No argument: the playhead is unset.
telemetry(Q, "playback-position-none",
          inbound_hex=hexs(osc(f"/update/workspace/{WS}/cueList/{L1}/playbackPosition")),
          state_before={"workspaces": {WS: {"cue_lists": {L1: {"playhead_id": C1, "index": 0}}}}},
          expect_state={"workspaces": {WS: {"cue_lists": {L1: {"index": 0}}}}})
# The cue lists: the tree replaces what was known; every cue's values are read.
telemetry(Q, "cue-lists",
          inbound_hex=reply(f"/workspace/{WS}/cueLists", [
              {"uniqueID": L1, "number": "", "name": "Main Cue List", "listName": "Main Cue List",
               "type": "Cue List", "colorName": "none", "colorName/live": "none", "flagged": False, "armed": True,
               "cues": [
                   {"uniqueID": C1, "number": "1", "name": "Intro", "listName": "Intro", "type": "Audio",
                    "colorName": "red", "colorName/live": "red", "flagged": 1, "armed": 1},
                   {"uniqueID": G1, "number": "2", "name": "", "listName": "Group", "type": "Group",
                    "colorName": "none", "colorName/live": "none", "flagged": 0, "armed": 1,
                    "cues": [{"uniqueID": C2, "number": "2.1", "type": "Video"}]}]}]),
          state_before={"workspaces": {WS: {"name": "show", "cues": {"OLD-1": {"name": "Deleted"}},
                                             "cue_lists": {"OLD-2": {"index": 1, "playhead_id": "OLD-1"}}}}},
          expect_state={"workspaces": {WS: {
              "name": "show",
              "cues": {
                  L1: {"number": "", "name": "Main Cue List", "list_name": "Main Cue List", "type": "Cue List",
                       "color": "none", "color_live": "none", "flagged": False, "armed": True, "index": 0,
                       "children": {"0": C1, "1": G1}},
                  C1: {"number": "1", "name": "Intro", "list_name": "Intro", "type": "Audio", "color": "red",
                       "color_live": "red", "flagged": True, "armed": True, "index": 0, "parent": L1},
                  G1: {"number": "2", "name": "", "list_name": "Group", "type": "Group", "color": "none",
                       "color_live": "none", "flagged": False, "armed": True, "index": 1, "parent": L1,
                       "children": {"0": C2}},
                  C2: {"number": "2.1", "type": "Video", "index": 0, "parent": G1}},
              "cue_lists": {L1: {"index": 0}}}}},
          expect_then_send_hex=[framed(f"/workspace/{WS}/cue_id/{L1}/valuesForKeys", ("s", VFK)),
                                framed(f"/workspace/{WS}/cue_id/{L1}/playheadID"),
                                framed(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", ("s", VFK)),
                                framed(f"/workspace/{WS}/cue_id/{G1}/valuesForKeys", ("s", VFK)),
                                framed(f"/workspace/{WS}/cue_id/{C2}/valuesForKeys", ("s", VFK))])
# A Group's children after it changed: its children list is replaced.
telemetry(Q, "children",
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{G1}/children/shallow", [
              {"uniqueID": C2, "number": "2.1", "name": "Logo", "listName": "Logo", "type": "Video",
               "colorName": "blue", "colorName/live": "blue", "flagged": False, "armed": False}]),
          state_before={"workspaces": {WS: {"cues": {G1: {"children": {"0": "OLD-1", "1": C2}}}}}},
          expect_state={"workspaces": {WS: {"cues": {
              G1: {"children": {"0": C2}},
              C2: {"number": "2.1", "name": "Logo", "list_name": "Logo", "type": "Video", "color": "blue",
                   "color_live": "blue", "flagged": False, "armed": False, "index": 0, "parent": G1}}}}})
telemetry(Q, "list-playhead",
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{L1}/playheadID", C1),
          expect_state={"workspaces": {WS: {"cue_lists": {L1: {"playhead_id": C1}}}}})
telemetry(Q, "list-playhead-none",
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{L1}/playheadID", "none"),
          state_before={"workspaces": {WS: {"cue_lists": {L1: {"playhead_id": C1}}}}},
          expect_state={"workspaces": {WS: {"cue_lists": {L1: {}}}}})
# A cue's values: the keys that apply to it.
telemetry(Q, "cue-values",
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", {
              "uniqueID": C1, "number": "1", "name": "Intro", "type": "Audio", "flagged": 0, "armed": True,
              "continueMode": 2, "preWait": 1.5, "postWait": 0, "duration": 182.25, "parent": L1,
              "colorName/live": "green", "isRunning": True, "isPaused": False, "actionElapsed": 12.5,
              "percentActionElapsed": 0.0686, "cueTargetID": "", "levels": [[0, -6], [-3, 0]], "notes": None}),
          expect_state={"workspaces": {WS: {"cues": {C1: {
              "number": "1", "name": "Intro", "type": "Audio", "flagged": False, "armed": True,
              "continue_mode": 2, "pre_wait": 1.5, "post_wait": 0.0, "duration": 182.25, "parent": L1,
              "color_live": "green", "running": True, "paused": False, "action_elapsed": 12.5,
              "percent_action_elapsed": 0.0686, "cue_target_id": "", "levels": "[[0,-6],[-3,0]]"}}}}})
# The active cues, polled: the list is replaced and each one's times read.
telemetry(Q, "active-cues",
          inbound_hex=reply("/runningOrPausedCues/shallow", [{"uniqueID": C1, "number": "1"}, {"uniqueID": C2}]),
          state_before={"workspaces": {WS: {"active": {"0": "OLD-1", "1": "OLD-2", "2": "OLD-3"}}}},
          expect_state={"workspaces": {WS: {"active": {"0": C1, "1": C2}}}},
          expect_then_send_hex=[framed(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", ("s", TIMING_KEYS)),
                                framed(f"/workspace/{WS}/cue_id/{C2}/valuesForKeys", ("s", TIMING_KEYS))])
telemetry(Q, "show-mode", inbound_hex=reply(f"/workspace/{WS}/showMode", True),
          expect_state={"workspaces": {WS: {"show_mode": True}}})

# The audio keys of a cue's values (sample values by type).
telemetry(Q, 'cue-values-audio',
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", {'startTime': 'x', 'endTime': 'x', 'playCount': 2, 'lastSlicePlayCount': 2, 'lastSliceInfiniteLoop': True, 'preservePitch': True, 'doPitchShift': True, 'doFade': True, 'lockFadeToCue': True, 'sliceMarkers': 'x', 'muteChannels': 'x', 'soloChannels': 'x', 'numChannelsIn': 2, 'audioOutputPatchName': 'x', 'audioOutputPatchNumber': 2, 'audioOutputPatchID': 'x', 'patch': 2, 'audioMapID': 'x', 'objects': 'x', 'audioInputPatchName': 'x', 'audioInputPatchID': 'x', 'channelOffset': 2, 'channels': 2}),
          expect_state={"workspaces": {WS: {"cues": {C1: {'start_time': 'x', 'end_time': 'x', 'play_count': 2, 'last_slice_play_count': 2, 'last_slice_infinite_loop': True, 'preserve_pitch': True, 'do_pitch_shift': True, 'do_fade': True, 'lock_fade_to_cue': True, 'slice_markers': 'x', 'mute_channels': 'x', 'solo_channels': 'x', 'num_channels_in': 2, 'audio_output_patch_name': 'x', 'audio_output_patch_number': 2, 'audio_output_patch_id': 'x', 'patch': 2, 'audio_map_id': 'x', 'objects': 'x', 'audio_input_patch_name': 'x', 'audio_input_patch_id': 'x', 'channel_offset': 2, 'channels': 2}}}}})

# The video keys of a cue's values (sample values by type).
telemetry(Q, 'cue-values-video',
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", {'anchor/x': 1.5, 'anchor/y': 1.5, 'blendMode': 'x', 'clockType': 'x', 'cropTop': 1.5, 'cropBottom': 1.5, 'cropLeft': 1.5, 'cropRight': 1.5, 'cueSize': 'x', 'fillStage': True, 'fullSurface': True, 'fillStyle': 2, 'holdLastFrame': True, 'layer': 2, 'opacity': 1.5, 'preserveAspectRatio': True, 'quaternion': 'x', 'scale/x': 1.5, 'scale/y': 1.5, 'smooth': True, 'stageID': 'x', 'stageName': 'x', 'surfaceID': 2, 'translation/x': 1.5, 'translation/y': 1.5, 'videoEffects': 'x', 'videoInputPatchName': 'x', 'videoInputPatchID': 'x', 'cameraPatch': 2}),
          expect_state={"workspaces": {WS: {"cues": {C1: {'anchor_x': 1.5, 'anchor_y': 1.5, 'blend_mode': 'x', 'clock_type': 'x', 'crop_top': 1.5, 'crop_bottom': 1.5, 'crop_left': 1.5, 'crop_right': 1.5, 'cue_size': 'x', 'fill_stage': True, 'full_surface': True, 'fill_style': 2, 'hold_last_frame': True, 'layer': 2, 'opacity': 1.5, 'preserve_aspect_ratio': True, 'quaternion': 'x', 'scale_x': 1.5, 'scale_y': 1.5, 'smooth': True, 'stage_id': 'x', 'stage_name': 'x', 'surface_id': 2, 'translation_x': 1.5, 'translation_y': 1.5, 'video_effects': 'x', 'video_input_patch_name': 'x', 'video_input_patch_id': 'x', 'camera_patch': 2}}}}})

# The text keys of a cue's values (sample values by type).
telemetry(Q, 'cue-values-text',
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", {'text': 'x', 'fixedWidth': 1.5, 'text/format': 'x', 'text/format/alignment': 'x'}),
          expect_state={"workspaces": {WS: {"cues": {C1: {'text': 'x', 'fixed_width': 1.5, 'text_format': 'x', 'text_alignment': 'x'}}}}})

# The light keys of a cue's values (sample values by type).
telemetry(Q, 'cue-values-light',
          inbound_hex=reply(f"/workspace/{WS}/cue_id/{C1}/valuesForKeys", {'lightCommandText': 'x', 'alwaysCollate': True, 'subcontroller': True, 'levelsMode': 2, 'mode': 2, 'geoMode': 2, 'fadeType': 2, 'stopTargetWhenDone': True, 'doOpacity': True, 'doRate': True, 'doRotation': True, 'doScale': True, 'doTranslation': True, 'doLevel': 'x', 'rotation': 1.5, 'rotationType': 2, 'pathWidth': 1.5, 'pathHeight': 1.5, 'pathSmooth': True, 'audioMapTargetID': 'x'}),
          expect_state={"workspaces": {WS: {"cues": {C1: {'light_command_text': 'x', 'always_collate': True, 'subcontroller': True, 'levels_mode': 2, 'mode': 2, 'geo_mode': 2, 'fade_type': 2, 'stop_target_when_done': True, 'do_opacity': True, 'do_rate': True, 'do_rotation': True, 'do_scale': True, 'do_translation': True, 'do_levels': 'x', 'rotation': 1.5, 'rotation_type': 2, 'path_width': 1.5, 'path_height': 1.5, 'path_smooth': True, 'audio_map_target_id': 'x'}}}}})

