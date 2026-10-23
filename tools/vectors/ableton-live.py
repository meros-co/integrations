# Ableton Live through AbletonOSC (ableton-live). Addresses and argument
# order are those of AbletonOSC's README and handler sources (MIT,
# github.com/ideoforms/AbletonOSC): indexes first, from 0 on the wire, then
# the value; booleans written as 1 and 0 as the README does; enumerations as
# the Live Object Model's integer indexes. Commands number tracks, scenes,
# slots, devices, parameters, sends and cue points from 1, so the wire
# carries each minus one. Replies and listener updates arrive on the get
# address with the indexes before the value. OSC bytes come from the encoder
# in make_vectors.py; floats are 32-bit, so the examples are exact in binary.
AL = "ableton-live"
_AU = {"expect_result": {"ok": {"kind": "unverified"}}}


def al(command, input, packets, **extra):
    binary(AL, command, input, packets, **extra)


def alget(command, input, address, args, reply_args, value, **extra):
    """A read: the query, Live's reply on the same address, the value."""
    binary(AL, command, input, osc(address, *args),
           device_reply_hex=hexs(osc(address, *reply_args)),
           expect_result={"ok": {"kind": "value", "value": value}}, **extra)


def i(n):
    return ("i", n)


def f(x):
    return ("f", x)


def s(t):
    return ("s", t)


# ── Application ──
alget("test", {}, "/live/test", [], [s("ok")], "ok")
alget("get_version", {}, "/live/application/get/version", [], [i(12), i(1)], 12)
alget("get_log_level", {}, "/live/api/get/log_level", [], [s("info")], "info")
al("set_log_level", {"level": "warning"}, [osc("/live/api/set/log_level", s("warning")), osc("/live/api/get/log_level")], **_AU)
al("show_message", {"text": "Show starts"}, osc("/live/api/show_message", s("Show starts")))

# ── Song methods: no arguments ──
for m in ["start_playing", "stop_playing", "continue_playing", "stop_all_clips", "tap_tempo", "undo", "redo",
          "capture_midi", "capture_and_insert_scene", "trigger_session_record", "jump_to_next_cue",
          "jump_to_prev_cue", "re_enable_automation", "force_link_beat_time", "create_return_track"]:
    al(m, {}, osc(f"/live/song/{m}"), **(_AU if m == "start_playing" else {}))
al("add_or_delete_cue_point", {}, osc("/live/song/cue_point/add_or_delete"))
al("jump_by", {"beats": -4.0}, osc("/live/song/jump_by", f(-4.0)))
# Position 0 is the end of the list (-1 on the wire); 1 the first.
al("create_audio_track", {"position": 0}, osc("/live/song/create_audio_track", i(-1)))
al("create_midi_track", {"position": 3}, osc("/live/song/create_midi_track", i(2)))
al("create_scene", {"position": 1}, osc("/live/song/create_scene", i(0)))
al("duplicate_track", {"track": 2}, osc("/live/song/duplicate_track", i(1)))
al("duplicate_scene", {"scene": 4}, osc("/live/song/duplicate_scene", i(3)))
al("delete_track", {"track": 5}, osc("/live/song/delete_track", i(4)))
al("delete_scene", {"scene": 1}, osc("/live/song/delete_scene", i(0)))
al("delete_return_track", {"return_track": 2}, osc("/live/song/delete_return_track", i(1)))
al("jump_to_cue_point", {"cue_point": 3}, osc("/live/song/cue_point/jump", i(2)))
al("jump_to_cue_point_named", {"name": "Chorus"}, osc("/live/song/cue_point/jump", s("Chorus")))
al("rename_cue_point", {"cue_point": 1, "name": "Intro"}, osc("/live/song/cue_point/set/name", i(0), s("Intro")))
al("listen_song_time", {}, osc("/live/song/start_listen/current_song_time"))
al("stop_listening_song_time", {}, osc("/live/song/stop_listen/current_song_time"))
alget("reload_tracks", {}, "/live/song/get/num_tracks", [], [i(8)], 8)
alget("reload_scenes", {}, "/live/song/get/num_scenes", [], [i(12)], 12)

# ── Song properties: a read (with a reply) and, where writable, a set
# followed by the read. (name, command suffix, set input, set args, reply, value)
SONG_BOOL = ["arrangement_overdub", "back_to_arranger", "is_ableton_link_enabled", "loop", "metronome",
             "nudge_down", "nudge_up", "punch_in", "punch_out", "record_mode", "session_record"]
for p in SONG_BOOL:
    alget(f"get_{p}", {}, f"/live/song/get/{p}", [], [("T", None)], True)
    al(f"set_{p}", {"value": True}, [osc(f"/live/song/set/{p}", i(1)), osc(f"/live/song/get/{p}")])
al("set_metronome", {"value": False}, [osc("/live/song/set/metronome", i(0)), osc("/live/song/get/metronome")],
   file="set_metronome-off")
for p in ["is_playing", "can_undo", "can_redo"]:
    alget(f"get_{p}", {}, f"/live/song/get/{p}", [], [("F", None)], False)
alget("get_clip_trigger_quantization", {}, "/live/song/get/clip_trigger_quantization", [], [i(4)], 4)
al("set_clip_trigger_quantization", {"value": "1_bar"},
   [osc("/live/song/set/clip_trigger_quantization", i(4)), osc("/live/song/get/clip_trigger_quantization")])
alget("get_midi_recording_quantization", {}, "/live/song/get/midi_recording_quantization", [], [i(5)], 5)
al("set_midi_recording_quantization", {"value": "1/16"},
   [osc("/live/song/set/midi_recording_quantization", i(5)), osc("/live/song/get/midi_recording_quantization")])
alget("get_song_time", {}, "/live/song/get/current_song_time", [], [f(16.5)], 16.5)
al("set_song_time", {"value": 32.0}, [osc("/live/song/set/current_song_time", f(32.0)), osc("/live/song/get/current_song_time")])
for p, v in [("groove_amount", 0.25), ("loop_length", 16.0), ("loop_start", 8.0), ("tempo", 128.0)]:
    alget(f"get_{p}", {}, f"/live/song/get/{p}", [], [f(v)], v)
    al(f"set_{p}", {"value": v}, [osc(f"/live/song/set/{p}", f(v)), osc(f"/live/song/get/{p}")], **(_AU if p == "tempo" else {}))
alget("get_song_length", {}, "/live/song/get/song_length", [], [f(384.0)], 384.0)
for p, v in [("root_note", 9), ("signature_denominator", 8), ("signature_numerator", 6)]:
    alget(f"get_{p}", {}, f"/live/song/get/{p}", [], [i(v)], v)
    al(f"set_{p}", {"value": v}, [osc(f"/live/song/set/{p}", i(v)), osc(f"/live/song/get/{p}")])
alget("get_session_record_status", {}, "/live/song/get/session_record_status", [], [i(1)], 1)
alget("get_scale_name", {}, "/live/song/get/scale_name", [], [s("Minor")], "Minor")
al("set_scale_name", {"value": "Dorian"}, [osc("/live/song/set/scale_name", s("Dorian")), osc("/live/song/get/scale_name")])

# ── View ──
al("select_scene", {"scene": 3}, [osc("/live/view/set/selected_scene", i(2)), osc("/live/view/get/selected_scene")])
al("select_track", {"track": 1}, [osc("/live/view/set/selected_track", i(0)), osc("/live/view/get/selected_track")])
al("select_clip", {"track": 2, "slot": 5}, [osc("/live/view/set/selected_clip", i(1), i(4)), osc("/live/view/get/selected_clip")])
al("select_device", {"track": 4, "device": 2}, [osc("/live/view/set/selected_device", i(3), i(1)), osc("/live/view/get/selected_device")])
alget("get_selected_scene", {}, "/live/view/get/selected_scene", [], [i(2)], 2)
alget("get_selected_track", {}, "/live/view/get/selected_track", [], [i(0)], 0)
alget("get_selected_clip", {}, "/live/view/get/selected_clip", [], [i(1), i(4)], 1)
alget("get_selected_device", {}, "/live/view/get/selected_device", [], [i(3), i(1)], 3)

# ── Tracks (track 3 = index 2) ──
al("stop_track_clips", {"track": 3}, osc("/live/track/stop_all_clips", i(2)))
al("delete_device", {"track": 3, "device": 2}, osc("/live/track/delete_device", i(2), i(1)))
T = {"track": 3}
for p in ["arm", "can_be_armed", "fold_state", "has_audio_input", "has_audio_output", "has_midi_input",
          "has_midi_output", "is_grouped", "is_visible", "mute", "solo"]:
    alget(f"get_track_{p}", T, f"/live/track/get/{p}", [i(2)], [i(2), ("T", None)], True)
for p in ["arm", "fold_state", "mute", "solo"]:
    al(f"set_track_{p}", {"track": 3, "value": True}, [osc(f"/live/track/set/{p}", i(2), i(1)), osc(f"/live/track/get/{p}", i(2))])
al("set_track_mute", {"track": 3, "value": False}, [osc("/live/track/set/mute", i(2), i(0)), osc("/live/track/get/mute", i(2))],
   file="set_track_mute-off")
alget("get_track_color", T, "/live/track/get/color", [i(2)], [i(2), i(16725558)], 16725558)
al("set_track_color", {"track": 3, "value": 16725558}, [osc("/live/track/set/color", i(2), i(16725558)), osc("/live/track/get/color", i(2))])
alget("get_track_color_index", T, "/live/track/get/color_index", [i(2)], [i(2), i(14)], 14)
al("set_track_color_index", {"track": 3, "value": 14}, [osc("/live/track/set/color_index", i(2), i(14)), osc("/live/track/get/color_index", i(2))])
alget("get_track_current_monitoring_state", T, "/live/track/get/current_monitoring_state", [i(2)], [i(2), i(1)], 1)
al("set_track_current_monitoring_state", {"track": 3, "value": "off"},
   [osc("/live/track/set/current_monitoring_state", i(2), i(2)), osc("/live/track/get/current_monitoring_state", i(2))])
alget("get_track_fired_slot_index", T, "/live/track/get/fired_slot_index", [i(2)], [i(2), i(-1)], -1)
alget("get_track_playing_slot_index", T, "/live/track/get/playing_slot_index", [i(2)], [i(2), i(0)], 0)
for p, v in [("input_routing_channel", "1/2"), ("input_routing_type", "Ext. In"),
             ("output_routing_channel", "Track In"), ("output_routing_type", "Master"), ("name", "Vocals")]:
    alget(f"get_track_{p}", T, f"/live/track/get/{p}", [i(2)], [i(2), s(v)], v)
    al(f"set_track_{p}", {"track": 3, "value": v}, [osc(f"/live/track/set/{p}", i(2), s(v)), osc(f"/live/track/get/{p}", i(2))])
for p, v in [("output_meter_left", 0.5), ("output_meter_level", 0.5), ("output_meter_right", 0.25)]:
    alget(f"get_track_{p}", T, f"/live/track/get/{p}", [i(2)], [i(2), f(v)], v)
alget("get_track_volume", T, "/live/track/get/volume", [i(2)], [i(2), f(0.75)], 0.75)
al("set_track_volume", {"track": 3, "value": 0.75}, [osc("/live/track/set/volume", i(2), f(0.75)), osc("/live/track/get/volume", i(2))], **_AU)
alget("get_track_panning", T, "/live/track/get/panning", [i(2)], [i(2), f(-0.5)], -0.5)
al("set_track_panning", {"track": 3, "value": -0.5}, [osc("/live/track/set/panning", i(2), f(-0.5)), osc("/live/track/get/panning", i(2))])
alget("get_track_send", {"track": 3, "send": 2}, "/live/track/get/send", [i(2), i(1)], [i(2), i(1), f(0.5)], 0.5)
al("set_track_send", {"track": 3, "send": 2, "level": 0.5},
   [osc("/live/track/set/send", i(2), i(1), f(0.5)), osc("/live/track/get/send", i(2), i(1))])
alget("get_track_num_devices", T, "/live/track/get/num_devices", [i(2)], [i(2), i(3)], 3)
al("listen_track_meters", T, [osc("/live/track/start_listen/output_meter_level", i(2)),
                              osc("/live/track/start_listen/output_meter_left", i(2)),
                              osc("/live/track/start_listen/output_meter_right", i(2))])
al("stop_listening_track_meters", T, [osc("/live/track/stop_listen/output_meter_level", i(2)),
                                      osc("/live/track/stop_listen/output_meter_left", i(2)),
                                      osc("/live/track/stop_listen/output_meter_right", i(2))])

# ── Clip slots (track 2, slot 4 = 1, 3) ──
TS = {"track": 2, "slot": 4}
al("fire_clip_slot", TS, osc("/live/clip_slot/fire", i(1), i(3)), **_AU)
al("stop_clip_slot", TS, osc("/live/clip_slot/stop", i(1), i(3)))
al("create_clip", {"track": 2, "slot": 4, "length": 8.0}, osc("/live/clip_slot/create_clip", i(1), i(3), f(8.0)))
al("delete_clip", TS, osc("/live/clip_slot/delete_clip", i(1), i(3)))
al("duplicate_clip_to", {"track": 2, "slot": 4, "target_track": 3, "target_slot": 1},
   osc("/live/clip_slot/duplicate_clip_to", i(1), i(3), i(2), i(0)))
for p in ["controls_other_clips", "has_clip", "has_stop_button", "is_playing", "is_triggered", "will_record_on_start"]:
    alget(f"get_clip_slot_{p}", TS, f"/live/clip_slot/get/{p}", [i(1), i(3)], [i(1), i(3), ("T", None)], True)
alget("get_clip_slot_playing_status", TS, "/live/clip_slot/get/playing_status", [i(1), i(3)], [i(1), i(3), i(1)], 1)
al("set_clip_slot_has_stop_button", {"track": 2, "slot": 4, "value": False},
   [osc("/live/clip_slot/set/has_stop_button", i(1), i(3), i(0)), osc("/live/clip_slot/get/has_stop_button", i(1), i(3))])

# ── Clips ──
al("fire_clip", TS, osc("/live/clip/fire", i(1), i(3)))
al("stop_clip", TS, osc("/live/clip/stop", i(1), i(3)))
al("duplicate_clip_loop", TS, osc("/live/clip/duplicate_loop", i(1), i(3)))
CLIP_GETS = [  # (property, reply value tuple, json value)
    ("color", i(3355443), 3355443), ("color_index", i(20), 20), ("end_marker", f(16.0), 16.0),
    ("end_time", f(16.0), 16.0), ("file_path", s("/Samples/Loop.wav"), "/Samples/Loop.wav"),
    ("gain", f(0.5), 0.5), ("gain_display_string", s("0.0 dB"), "0.0 dB"), ("has_groove", ("F", None), False),
    ("is_audio_clip", ("T", None), True), ("is_midi_clip", ("F", None), False),
    ("is_overdubbing", ("F", None), False), ("is_playing", ("T", None), True), ("is_recording", ("F", None), False),
    ("is_triggered", ("F", None), False), ("launch_mode", i(2), 2), ("launch_quantization", i(0), 0),
    ("legato", ("T", None), True), ("length", f(16.0), 16.0), ("loop_end", f(16.0), 16.0),
    ("loop_start", f(4.0), 4.0), ("looping", ("T", None), True), ("muted", ("F", None), False),
    ("name", s("Verse"), "Verse"), ("pitch_coarse", i(-12), -12), ("pitch_fine", f(-25.0), -25.0),
    ("playing_position", f(2.5), 2.5), ("position", f(4.0), 4.0), ("ram_mode", ("F", None), False),
    ("sample_length", i(705600), 705600), ("start_marker", f(0.0), 0.0), ("start_time", f(0.0), 0.0),
    ("velocity_amount", f(0.25), 0.25), ("warp_mode", i(4), 4), ("warping", ("T", None), True),
    ("will_record_on_start", ("F", None), False),
]
for p, reply, value in CLIP_GETS:
    alget(f"get_clip_{p}", TS, f"/live/clip/get/{p}", [i(1), i(3)], [i(1), i(3), reply], value)
CLIP_SETS = [  # (property, input value, wire argument)
    ("color", 3355443, i(3355443)), ("color_index", 20, i(20)), ("end_marker", 16.0, f(16.0)), ("gain", 0.5, f(0.5)),
    ("launch_mode", "toggle", i(2)), ("launch_quantization", "1_bar", i(5)), ("legato", True, i(1)),
    ("loop_end", 16.0, f(16.0)), ("loop_start", 4.0, f(4.0)), ("looping", False, i(0)), ("muted", True, i(1)),
    ("name", "Chorus", s("Chorus")), ("pitch_coarse", -12, i(-12)), ("pitch_fine", 10.5, f(10.5)),
    ("position", 4.0, f(4.0)), ("ram_mode", True, i(1)), ("start_marker", 0.5, f(0.5)),
    ("velocity_amount", 0.25, f(0.25)), ("warp_mode", "complex_pro", i(6)), ("warping", True, i(1)),
]
for p, v, arg in CLIP_SETS:
    al(f"set_clip_{p}", {"track": 2, "slot": 4, "value": v},
       [osc(f"/live/clip/set/{p}", i(1), i(3), arg), osc(f"/live/clip/get/{p}", i(1), i(3))])
al("listen_clip_playing_position", TS, osc("/live/clip/start_listen/playing_position", i(1), i(3)))
al("stop_listening_clip_playing_position", TS, osc("/live/clip/stop_listen/playing_position", i(1), i(3)))
al("add_clip_note", {"track": 2, "slot": 4, "pitch": 60, "start": 1.0, "duration": 0.5, "velocity": 100},
   osc("/live/clip/add/notes", i(1), i(3), i(60), f(1.0), f(0.5), i(100), i(0)))
al("add_clip_note", {"track": 2, "slot": 4, "pitch": 64, "start": 0.0, "duration": 2.0, "velocity": 1, "muted": True},
   osc("/live/clip/add/notes", i(1), i(3), i(64), f(0.0), f(2.0), i(1), i(1)), file="add_clip_note-muted")
al("remove_clip_notes", {"track": 2, "slot": 4, "pitch_start": 36, "pitch_span": 12, "time_start": 0.0, "time_span": 4.0},
   osc("/live/clip/remove/notes", i(1), i(3), i(36), i(12), f(0.0), f(4.0)))
al("remove_all_clip_notes", TS, osc("/live/clip/remove/notes", i(1), i(3)))

# ── Scenes (scene 2 = index 1) ──
al("fire_scene", {"scene": 2}, osc("/live/scene/fire", i(1)), **_AU)
al("fire_scene_as_selected", {"scene": 2}, osc("/live/scene/fire_as_selected", i(1)))
al("fire_selected_scene", {}, osc("/live/scene/fire_selected"))
SC = {"scene": 2}
for p, reply, value in [("color", i(255), 255), ("color_index", i(3), 3), ("is_triggered", ("F", None), False),
                        ("name", s("Bridge"), "Bridge"), ("tempo", f(-1.0), -1.0), ("tempo_enabled", ("F", None), False),
                        ("time_signature_denominator", i(4), 4), ("time_signature_enabled", ("T", None), True),
                        ("time_signature_numerator", i(3), 3)]:
    alget(f"get_scene_{p}", SC, f"/live/scene/get/{p}", [i(1)], [i(1), reply], value)
for p, v, arg in [("color", 255, i(255)), ("color_index", 3, i(3)), ("name", "Bridge", s("Bridge")),
                  ("tempo", 96.0, f(96.0)), ("tempo_enabled", True, i(1)), ("time_signature_denominator", 8, i(8)),
                  ("time_signature_enabled", False, i(0)), ("time_signature_numerator", 7, i(7))]:
    al(f"set_scene_{p}", {"scene": 2, "value": v}, [osc(f"/live/scene/set/{p}", i(1), arg), osc(f"/live/scene/get/{p}", i(1))])

# ── Devices (track 1, device 2 = 0, 1) ──
TD = {"track": 1, "device": 2}
alget("get_device_name", TD, "/live/device/get/name", [i(0), i(1)], [i(0), i(1), s("Reverb")], "Reverb")
alget("get_device_type", TD, "/live/device/get/type", [i(0), i(1)], [i(0), i(1), i(1)], 1)
alget("get_device_num_parameters", TD, "/live/device/get/num_parameters", [i(0), i(1)], [i(0), i(1), i(32)], 32)
TDP = {"track": 1, "device": 2, "parameter": 5}
alget("get_device_parameter_value", TDP, "/live/device/get/parameter/value", [i(0), i(1), i(4)],
      [i(0), i(1), i(4), f(0.5)], 0.5)
alget("get_device_parameter_value_string", TDP, "/live/device/get/parameter/value_string", [i(0), i(1), i(4)],
      [i(0), i(1), i(4), s("2500 Hz")], "2500 Hz")
al("set_device_parameter", {"track": 1, "device": 2, "parameter": 5, "value": 0.25},
   [osc("/live/device/set/parameter/value", i(0), i(1), i(4), f(0.25)), osc("/live/device/get/parameter/value", i(0), i(1), i(4))])
al("watch_device_parameters", TD, osc("/live/device/get/parameter/name", i(0), i(1), i(0)))
al("stop_listening_device_parameter", TDP, osc("/live/device/stop_listen/parameter/value", i(0), i(1), i(4)))
al("map_midi_cc", {"track": 1, "device": 2, "parameter": 5, "channel": 1, "cc": 74},
   osc("/live/midimap/map_cc", i(0), i(1), i(4), i(0), i(74)))

# ── resync: the listeners and reads sent on connecting ──
_LISTENED = ["arrangement_overdub", "back_to_arranger", "clip_trigger_quantization", "groove_amount",
             "is_ableton_link_enabled", "is_playing", "loop", "loop_length", "loop_start", "metronome",
             "midi_recording_quantization", "nudge_down", "nudge_up", "punch_in", "punch_out", "record_mode",
             "root_note", "scale_name", "session_record", "session_record_status", "signature_denominator",
             "signature_numerator", "song_length", "tempo"]
al("resync", {}, [osc(f"/live/song/start_listen/{p}") for p in _LISTENED] + [
    osc("/live/song/start_listen/beat"), osc("/live/view/start_listen/selected_scene"),
    osc("/live/view/start_listen/selected_track"), osc("/live/application/get/version"),
    osc("/live/api/get/log_level"), osc("/live/song/get/can_undo"), osc("/live/song/get/can_redo"),
    osc("/live/view/get/selected_clip"), osc("/live/view/get/selected_device"),
    osc("/live/song/get/num_tracks"), osc("/live/song/get/num_scenes")])


# ── Telemetry ──
def alt(name, address, args, state, **extra):
    telemetry(AL, name, inbound_hex=hexs(osc(address, *args)), expect_state=state, **extra)


alt("version", "/live/application/get/version", [i(12), i(1)], {"application": {"version_major": 12, "version_minor": 1}})
alt("log-level", "/live/api/get/log_level", [s("info")], {"application": {"log_level": "info"}})
alt("error", "/live/error", [s("Error handling OSC message: list index out of range")],
    {"application": {"last_error": "Error handling OSC message: list index out of range"}})
alt("beat", "/live/song/get/beat", [i(33)], {"song": {"beat": 33}})
alt("song-tempo", "/live/song/get/tempo", [f(128.0)], {"song": {"tempo": 128.0}})
alt("song-is-playing", "/live/song/get/is_playing", [("T", None)], {"song": {"is_playing": True}})
alt("song-metronome-off", "/live/song/get/metronome", [("F", None)], {"song": {"metronome": False}})
for p in ["arrangement_overdub", "back_to_arranger", "is_ableton_link_enabled", "loop", "nudge_down", "nudge_up",
          "punch_in", "punch_out", "record_mode", "session_record", "can_undo", "can_redo"]:
    alt(f"song-{p}", f"/live/song/get/{p}", [("T", None)], {"song": {p: True}})
alt("song-clip-trigger-quantization", "/live/song/get/clip_trigger_quantization", [i(7)],
    {"song": {"clip_trigger_quantization": "1/4"}})
alt("song-midi-recording-quantization", "/live/song/get/midi_recording_quantization", [i(4)],
    {"song": {"midi_recording_quantization": "1/8+1/8t"}})
for p, v in [("current_song_time", 16.5), ("groove_amount", 0.5), ("loop_length", 16.0), ("loop_start", 8.0),
             ("song_length", 384.0)]:
    alt(f"song-{p}", f"/live/song/get/{p}", [f(v)], {"song": {p: v}})
for p, v in [("root_note", 9), ("session_record_status", 1), ("signature_denominator", 8), ("signature_numerator", 6)]:
    alt(f"song-{p}", f"/live/song/get/{p}", [i(v)], {"song": {p: v}})
alt("song-scale-name", "/live/song/get/scale_name", [s("Minor")], {"song": {"scale_name": "Minor"}})
alt("selected-scene", "/live/view/get/selected_scene", [i(2)], {"view": {"selected_scene": 3}})
alt("selected-track", "/live/view/get/selected_track", [i(0)], {"view": {"selected_track": 1}})
alt("selected-clip", "/live/view/get/selected_clip", [i(1), i(4)], {"view": {"selected_clip_track": 2, "selected_clip_slot": 5}})
alt("selected-device", "/live/view/get/selected_device", [i(3), i(1)], {"view": {"selected_device_track": 4, "selected_device": 2}})


# AbletonOSC started again: every listener and the walks, queued.
_STARTUP = [osc(f"/live/song/start_listen/{p}") for p in _LISTENED] + [
    osc("/live/song/start_listen/beat"), osc("/live/view/start_listen/selected_scene"),
    osc("/live/view/start_listen/selected_track"), osc("/live/application/get/version"),
    osc("/live/api/get/log_level"), osc("/live/view/get/selected_clip"), osc("/live/view/get/selected_device"),
    osc("/live/song/get/num_tracks"), osc("/live/song/get/num_scenes")]
alt("startup", "/live/startup", [], {}, expect_then_send_hex=[hexs(p) for p in _STARTUP])

# The track count: tracks read from scratch, listeners for every track, the walk from track 1.
_STAR = ["color", "color_index", "fired_slot_index", "mute", "name", "panning", "playing_slot_index", "solo", "volume"]
alt("num-tracks", "/live/song/get/num_tracks", [i(2)], {"song": {"num_tracks": 2}},
    state_before={"tracks": {"7": {"name": "Gone"}}},
    expect_then_send_hex=[hexs(osc(f"/live/track/start_listen/{p}", s("*"))) for p in _STAR]
    + [hexs(osc("/live/track/get/is_foldable", i(0)))])
# A track of the walk (track 2, a plain track): the next track, its reads, and its sub-walks.
_READS = ["can_be_armed", "has_audio_input", "has_audio_output", "has_midi_input", "has_midi_output", "is_grouped",
          "is_visible", "input_routing_type", "input_routing_channel", "output_routing_type", "output_routing_channel",
          "num_devices"]
alt("track-walk", "/live/track/get/is_foldable", [i(1), ("F", None)], {"tracks": {"2": {"is_foldable": False}}},
    expect_then_send_hex=[hexs(osc("/live/track/get/is_foldable", i(2)))]
    + [hexs(osc(f"/live/track/get/{p}", i(1))) for p in _READS]
    + [hexs(osc("/live/track/get/send", i(1), i(0))), hexs(osc("/live/clip_slot/get/is_group_slot", i(1), i(0))),
       hexs(osc("/live/device/get/class_name", i(1), i(0)))])
# A group track also has its fold state read.
alt("track-walk-group", "/live/track/get/is_foldable", [i(0), ("T", None)], {"tracks": {"1": {"is_foldable": True}}},
    expect_then_send_hex=[hexs(osc("/live/track/get/is_foldable", i(1)))]
    + [hexs(osc(f"/live/track/get/{p}", i(0))) for p in _READS]
    + [hexs(osc("/live/track/get/send", i(0), i(0))), hexs(osc("/live/clip_slot/get/is_group_slot", i(0), i(0))),
       hexs(osc("/live/device/get/class_name", i(0), i(0))), hexs(osc("/live/track/get/fold_state", i(0)))])
# A track that can be armed: arm and monitoring listened to.
alt("track-can-be-armed", "/live/track/get/can_be_armed", [i(1), ("T", None)], {"tracks": {"2": {"can_be_armed": True}}},
    expect_then_send_hex=[hexs(osc("/live/track/start_listen/arm", i(1))),
                          hexs(osc("/live/track/start_listen/current_monitoring_state", i(1)))])
alt("track-cannot-be-armed", "/live/track/get/can_be_armed", [i(1), ("F", None)], {"tracks": {"2": {"can_be_armed": False}}},
    expect_then_send_hex=[])
for p in ["arm", "fold_state", "has_audio_input", "has_audio_output", "has_midi_input", "has_midi_output", "is_grouped",
          "is_visible", "mute", "solo"]:
    alt(f"track-{p}", f"/live/track/get/{p}", [i(2), ("T", None)], {"tracks": {"3": {p: True}}})
alt("track-mute-off", "/live/track/get/mute", [i(2), ("F", None)], {"tracks": {"3": {"mute": False}}})
alt("track-color", "/live/track/get/color", [i(2), i(16725558)], {"tracks": {"3": {"color": 16725558}}})
alt("track-color-index", "/live/track/get/color_index", [i(2), i(14)], {"tracks": {"3": {"color_index": 14}}})
alt("track-monitoring", "/live/track/get/current_monitoring_state", [i(2), i(1)],
    {"tracks": {"3": {"current_monitoring_state": "auto"}}})
alt("track-playing-slot", "/live/track/get/playing_slot_index", [i(2), i(3)], {"tracks": {"3": {"playing_slot_index": 4}}})
alt("track-playing-none", "/live/track/get/playing_slot_index", [i(2), i(-1)], {"tracks": {"3": {"playing_slot_index": 0}}})
alt("track-fired-stop", "/live/track/get/fired_slot_index", [i(2), i(-2)], {"tracks": {"3": {"fired_slot_index": -1}}})
for p, v in [("name", "Vocals"), ("input_routing_type", "Ext. In"), ("input_routing_channel", "1/2"),
             ("output_routing_type", "Master"), ("output_routing_channel", "Track In")]:
    alt(f"track-{p}", f"/live/track/get/{p}", [i(2), s(v)], {"tracks": {"3": {p: v}}})
for p, v in [("volume", 0.75), ("panning", -0.5), ("output_meter_level", 0.5), ("output_meter_left", 0.5),
             ("output_meter_right", 0.25)]:
    alt(f"track-{p}", f"/live/track/get/{p}", [i(2), f(v)], {"tracks": {"3": {p: v}}})
alt("track-num-devices", "/live/track/get/num_devices", [i(2), i(3)], {"tracks": {"3": {"num_devices": 3}}})
alt("track-send", "/live/track/get/send", [i(2), i(1), f(0.5)], {"tracks": {"3": {"sends": {"2": {"level": 0.5}}}}},
    expect_then_send_hex=[hexs(osc("/live/track/get/send", i(2), i(2)))])

# Clip slots: the slot walk (track 2, slot 4).
alt("slot-walk", "/live/clip_slot/get/is_group_slot", [i(1), i(3), ("F", None)],
    {"tracks": {"2": {"slots": {"4": {"is_group_slot": False}}}}},
    expect_then_send_hex=[hexs(osc("/live/clip_slot/get/is_group_slot", i(1), i(4))),
                          hexs(osc("/live/clip_slot/get/is_playing", i(1), i(3))),
                          hexs(osc("/live/clip_slot/get/will_record_on_start", i(1), i(3)))]
    + [hexs(osc(f"/live/clip_slot/start_listen/{p}", i(1), i(3)))
       for p in ["controls_other_clips", "has_clip", "has_stop_button", "is_triggered", "playing_status"]])
for p in ["controls_other_clips", "has_stop_button", "is_playing", "is_triggered", "will_record_on_start"]:
    alt(f"slot-{p}", f"/live/clip_slot/get/{p}", [i(1), i(3), ("T", None)], {"tracks": {"2": {"slots": {"4": {p: True}}}}})
alt("slot-playing-status", "/live/clip_slot/get/playing_status", [i(1), i(3), i(2)],
    {"tracks": {"2": {"slots": {"4": {"playing_status": 2}}}}})
_CLIP_LISTEN = ["color", "color_index", "end_marker", "end_time", "gain", "is_overdubbing", "is_recording",
                "launch_mode", "launch_quantization", "legato", "loop_end", "loop_start", "looping", "muted", "name",
                "pitch_coarse", "pitch_fine", "position", "ram_mode", "start_marker", "velocity_amount", "warp_mode",
                "warping"]
_CLIP_READ = ["file_path", "gain_display_string", "has_groove", "is_audio_clip", "is_midi_clip", "is_playing",
              "is_triggered", "length", "sample_length", "start_time", "will_record_on_start"]
alt("slot-has-clip", "/live/clip_slot/get/has_clip", [i(1), i(3), ("T", None)],
    {"tracks": {"2": {"slots": {"4": {"has_clip": True}}}}},
    expect_then_send_hex=[hexs(osc(f"/live/clip/start_listen/{p}", i(1), i(3))) for p in _CLIP_LISTEN]
    + [hexs(osc(f"/live/clip/get/{p}", i(1), i(3))) for p in _CLIP_READ])
alt("slot-emptied", "/live/clip_slot/get/has_clip", [i(1), i(3), ("F", None)],
    {"tracks": {"2": {"slots": {"4": {"has_clip": False}}}}},
    state_before={"tracks": {"2": {"slots": {"4": {"has_clip": True, "clip": {"name": "Verse", "looping": True}}}}}})

# Clip properties.
for p, reply, value in [("name", s("Verse"), "Verse"), ("color", i(3355443), 3355443), ("color_index", i(20), 20),
                        ("end_marker", f(16.0), 16.0), ("end_time", f(16.0), 16.0), ("file_path", s("/Samples/Loop.wav"), "/Samples/Loop.wav"),
                        ("gain", f(0.5), 0.5), ("gain_display_string", s("0.0 dB"), "0.0 dB"),
                        ("has_groove", ("T", None), True), ("is_audio_clip", ("T", None), True),
                        ("is_midi_clip", ("F", None), False), ("is_overdubbing", ("T", None), True),
                        ("is_playing", ("T", None), True), ("is_recording", ("F", None), False),
                        ("is_triggered", ("F", None), False), ("legato", ("T", None), True), ("length", f(16.0), 16.0),
                        ("loop_end", f(16.0), 16.0), ("loop_start", f(4.0), 4.0), ("looping", ("T", None), True),
                        ("muted", ("F", None), False), ("pitch_coarse", i(-12), -12), ("pitch_fine", f(-25.0), -25.0),
                        ("playing_position", f(2.5), 2.5), ("position", f(4.0), 4.0), ("ram_mode", ("F", None), False),
                        ("sample_length", i(705600), 705600), ("start_marker", f(0.0), 0.0), ("start_time", f(0.0), 0.0),
                        ("velocity_amount", f(0.25), 0.25), ("warping", ("T", None), True),
                        ("will_record_on_start", ("F", None), False)]:
    alt(f"clip-{p}", f"/live/clip/get/{p}", [i(1), i(3), reply], {"tracks": {"2": {"slots": {"4": {"clip": {p: value}}}}}})
alt("clip-launch-mode", "/live/clip/get/launch_mode", [i(1), i(3), i(3)],
    {"tracks": {"2": {"slots": {"4": {"clip": {"launch_mode": "repeat"}}}}}})
alt("clip-launch-quantization", "/live/clip/get/launch_quantization", [i(1), i(3), i(0)],
    {"tracks": {"2": {"slots": {"4": {"clip": {"launch_quantization": "global"}}}}}})
alt("clip-warp-mode", "/live/clip/get/warp_mode", [i(1), i(3), i(3)],
    {"tracks": {"2": {"slots": {"4": {"clip": {"warp_mode": "re_pitch"}}}}}})

# Devices: the device walk (track 1, device 2).
alt("device-walk", "/live/device/get/class_name", [i(0), i(1), s("Reverb")],
    {"tracks": {"1": {"devices": {"2": {"class_name": "Reverb"}}}}},
    expect_then_send_hex=[hexs(osc("/live/device/get/class_name", i(0), i(2))),
                          hexs(osc("/live/device/start_listen/name", i(0), i(1))),
                          hexs(osc("/live/device/get/type", i(0), i(1))),
                          hexs(osc("/live/device/get/num_parameters", i(0), i(1)))])
alt("device-name", "/live/device/get/name", [i(0), i(1), s("Hall Reverb")],
    {"tracks": {"1": {"devices": {"2": {"name": "Hall Reverb"}}}}})
alt("device-type", "/live/device/get/type", [i(0), i(1), i(1)], {"tracks": {"1": {"devices": {"2": {"type": 1}}}}})
alt("device-num-parameters", "/live/device/get/num_parameters", [i(0), i(1), i(32)],
    {"tracks": {"1": {"devices": {"2": {"num_parameters": 32}}}}})
alt("parameter-walk", "/live/device/get/parameter/name", [i(0), i(1), i(4), s("Decay Time")],
    {"tracks": {"1": {"devices": {"2": {"parameters": {"5": {"name": "Decay Time"}}}}}}},
    expect_then_send_hex=[hexs(osc("/live/device/get/parameter/name", i(0), i(1), i(5))),
                          hexs(osc("/live/device/start_listen/parameter/value", i(0), i(1), i(4)))])
alt("parameter-value", "/live/device/get/parameter/value", [i(0), i(1), i(4), f(0.5)],
    {"tracks": {"1": {"devices": {"2": {"parameters": {"5": {"value": 0.5}}}}}}})
alt("parameter-value-string", "/live/device/get/parameter/value_string", [i(0), i(1), i(4), s("2.50 s")],
    {"tracks": {"1": {"devices": {"2": {"parameters": {"5": {"value_string": "2.50 s"}}}}}}})

# Scenes: count, walk, properties.
alt("num-scenes", "/live/song/get/num_scenes", [i(8)], {"song": {"num_scenes": 8}},
    state_before={"scenes": {"9": {"name": "Gone"}}},
    expect_then_send_hex=[hexs(osc("/live/scene/get/is_empty", i(0)))])
alt("scene-walk", "/live/scene/get/is_empty", [i(1), ("F", None)], {"scenes": {"2": {"is_empty": False}}},
    expect_then_send_hex=[hexs(osc("/live/scene/get/is_empty", i(2)))]
    + [hexs(osc(f"/live/scene/start_listen/{p}", i(1)))
       for p in ["color", "color_index", "is_triggered", "name", "tempo", "tempo_enabled", "time_signature_denominator",
                 "time_signature_enabled", "time_signature_numerator"]])
for p, reply, value in [("name", s("Bridge"), "Bridge"), ("color", i(255), 255), ("color_index", i(3), 3),
                        ("is_triggered", ("T", None), True), ("tempo", f(-1.0), -1.0), ("tempo_enabled", ("F", None), False),
                        ("time_signature_denominator", i(8), 8), ("time_signature_enabled", ("T", None), True),
                        ("time_signature_numerator", i(7), 7)]:
    alt(f"scene-{p}", f"/live/scene/get/{p}", [i(1), reply], {"scenes": {"2": {p: value}}})
