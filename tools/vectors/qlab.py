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

# State. Workspace and cue IDs as QLab reports them (uniqueID, UUID strings).
WS = UID
L1, C1, G1, C2 = LIST_UID, CUE_UID, "6A000001-0000-4000-8000-0000000000C3", "6A000001-0000-4000-8000-0000000000C4"
VFK = '["uniqueID","number","name","listName","displayName","type","colorName","colorName/live","secondColorName","useSecondColor","flagged","armed","notes","autoLoad","continueMode","preWait","postWait","duration","currentDuration","parent","cueTargetID","cueTargetNumber","fileTarget","targetMode","patchTargetID","duckOthers","duckLevel","duckTime","fadeAndStopOthers","fadeAndStopOthersTime","secondTriggerAction","secondTriggerOnRelease","skipIfDisarmed","rate","infiniteLoop","hasFileTargets","hasCueTargets","levels","sliderLevels","isRunning","isPaused","isLoaded","isBroken","isAuditioning","isPanicking","isTailingOut","isActionRunning","isOverridden","isWarning","actionElapsed","percentActionElapsed","preWaitElapsed","percentPreWaitElapsed","postWaitElapsed","percentPostWaitElapsed","currentFileTime","cueTargetId"]'
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

