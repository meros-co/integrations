# Cockos REAPER web interface (cockos-reaper-web): GET /_/ followed by
# semicolon-separated commands, as REAPER's main.js reference gives them
# (Plugins/reaper_www_root/main.js). Each command that changes something
# chains the request reading it back, as REAPER's own pages do. Values in the
# path are percent-encoded (unreserved characters kept), which REAPER decodes.
# Replies are \n-separated lines of tab-separated fields.
RW = "cockos-reaper-web"


def rw(command, input, target, **extra):
    http(RW, command, input, "GET", target, **extra)


# ── Transport (action IDs REAPER's own pages send) ───────────────────────
for _c, _t in [("play", "/_/1007;TRANSPORT"), ("pause", "/_/1008;TRANSPORT"),
               ("record", "/_/1013;TRANSPORT"), ("stop", "/_/1016;TRANSPORT"),
               ("stop_save_media", "/_/40667;TRANSPORT"), ("stop_delete_media", "/_/40668;TRANSPORT"),
               ("toggle_metronome", "/_/40364;GET/40364"), ("toggle_snap", "/_/1157;GET/1157"),
               ("next_marker", "/_/40173;TRANSPORT"), ("previous_marker", "/_/40172;TRANSPORT"),
               ("insert_marker", "/_/40157;MARKER"),
               ("undo", "/_/40029;NTRACK;TRACK;MARKER;REGION"),
               ("redo", "/_/40030;NTRACK;TRACK;MARKER;REGION"),
               ("toggle_repeat", "/_/SET/REPEAT/-1;TRANSPORT"),
               ("read_transport", "/_/TRANSPORT;BEATPOS"), ("read_tracks", "/_/NTRACK;TRACK"),
               ("read_markers", "/_/MARKER"), ("read_regions", "/_/REGION"),
               ("begin_undo_block", "/_/SET/UNDO_BEGIN")]:
    rw(_c, {}, _t)
rw("play", {}, "/_/1007;TRANSPORT", file="play.reply",
   http_reply={"status": 200, "body": "TRANSPORT\t1\t3.250000\t0\t3.250\t2.2.25\n"},
   expect_result={"ok": {"kind": "ack"}})
rw("set_repeat", {"enabled": True}, "/_/SET/REPEAT/1;TRANSPORT")
rw("read_repeat", {}, "/_/GET/REPEAT",
   http_reply={"status": 200, "body": "GET/REPEAT\t1\n"},
   expect_result={"ok": {"kind": "value", "value": "1"}})
rw("set_position", {"seconds": 62.5}, "/_/SET/POS/62.500000;TRANSPORT")
# main.js: SET/POS_STR takes any format; the pages send encodeURIComponent(text).
rw("set_position_text", {"position": "5.2.00"}, "/_/SET/POS_STR/5.2.00;TRANSPORT")
rw("go_to_marker", {"id": 1}, "/_/SET/POS_STR/m1;TRANSPORT")
rw("go_to_region", {"id": 1}, "/_/SET/POS_STR/r1;TRANSPORT")
rw("go_to_timeline_marker", {"place": 1}, "/_/SET/POS_STR/M1;TRANSPORT")
rw("go_to_timeline_region", {"place": 2}, "/_/SET/POS_STR/R2;TRANSPORT")

# ── Tracks ───────────────────────────────────────────────────────────────
# Touch released (e) by default; g ignores grouping.
rw("set_track_volume", {"track": 3, "volume": 0.5}, "/_/SET/TRACK/3/VOL/0.500000e;TRACK")
rw("set_track_volume", {"track": 3, "volume": 1.0, "touch": "hold", "ignore_gang": True},
   "/_/SET/TRACK/3/VOL/1.000000g;TRACK", file="set_track_volume.hold-ungrouped")
rw("set_track_pan", {"track": 2, "pan": -0.25}, "/_/SET/TRACK/2/PAN/-0.250000e;TRACK")
rw("set_track_width", {"track": 2, "width": 1.0, "ignore_gang": True}, "/_/SET/TRACK/2/WIDTH/1.000000eg;TRACK")
rw("set_track_mute", {"track": 3, "muted": True}, "/_/SET/TRACK/3/MUTE/1;TRACK")
rw("toggle_track_mute", {"track": 0}, "/_/SET/TRACK/0/MUTE/-1;TRACK")
rw("set_track_solo", {"track": 4, "soloed": False}, "/_/SET/TRACK/4/SOLO/0;TRACK")
rw("toggle_track_solo", {"track": 4}, "/_/SET/TRACK/4/SOLO/-1;TRACK")
rw("set_track_fx_enabled", {"track": 1, "enabled": False}, "/_/SET/TRACK/1/FX/0;TRACK")
rw("toggle_track_fx_enabled", {"track": 1}, "/_/SET/TRACK/1/FX/-1;TRACK")
rw("set_track_arm", {"track": 1, "armed": True}, "/_/SET/TRACK/1/RECARM/1;TRACK")
rw("toggle_track_arm", {"track": 1}, "/_/SET/TRACK/1/RECARM/-1;TRACK")
rw("set_track_monitor", {"track": 5, "mode": 2}, "/_/SET/TRACK/5/RECMON/2;TRACK")
rw("cycle_track_monitor", {"track": 5}, "/_/SET/TRACK/5/RECMON/-1;TRACK")
rw("set_track_selected", {"track": 6, "selected": True}, "/_/SET/TRACK/6/SEL/1;TRACK")
rw("toggle_track_selected", {"track": 6}, "/_/SET/TRACK/6/SEL/-1;TRACK")
# main.js's example: GET/TRACK/1/I_RECINPUT answers GET/TRACK/1/I_RECINPUT\t<index>.
rw("get_track_info", {"track": 1, "attribute": "I_RECINPUT"}, "/_/GET/TRACK/1/I_RECINPUT",
   http_reply={"status": 200, "body": "GET/TRACK/1/I_RECINPUT\t1024\n"},
   expect_result={"ok": {"kind": "value", "value": "1024"}})
rw("set_track_info", {"track": 2, "attribute": "I_AUTOMODE", "value": "1"},
   "/_/SET/TRACK/2/I_AUTOMODE/1;GET/TRACK/2/I_AUTOMODE")
rw("set_track_name", {"track": 2, "name": "Lead Vox"}, "/_/SET/TRACK/2/P_NAME/Lead%20Vox;TRACK")

# ── Sends (0 up: hardware outputs and sends; -1 down: receives) ──────────
rw("get_track_send", {"track": 0, "send": 0}, "/_/GET/TRACK/0/SEND/0")
rw("set_track_send_mute", {"track": 2, "send": 1, "muted": True},
   "/_/SET/TRACK/2/SEND/1/MUTE/1;GET/TRACK/2/SEND/1")
rw("toggle_track_send_mute", {"track": 0, "send": 0}, "/_/SET/TRACK/0/SEND/0/MUTE/-1;GET/TRACK/0/SEND/0")
rw("set_track_send_volume", {"track": 3, "send": -1, "volume": 0.5},
   "/_/SET/TRACK/3/SEND/-1/VOL/0.500000E;GET/TRACK/3/SEND/-1")
rw("set_track_send_pan", {"track": 3, "send": 0, "pan": 0.5, "touch": "release"},
   "/_/SET/TRACK/3/SEND/0/PAN/0.500000e;GET/TRACK/3/SEND/0")

# ── Undo, lyrics, extended state ─────────────────────────────────────────
# index.html: SET/UNDO/Rename%20Track.
rw("add_undo_point", {"message": "Rename Track"}, "/_/SET/UNDO/Rename%20Track")
rw("end_undo_block", {"message": "Clone track"}, "/_/SET/UNDO_END/Clone%20track")
rw("get_lyrics", {"track": 1}, "/_/LYRICS/1")
rw("get_ext_state", {"section": "my script", "key": "mode"}, "/_/GET/EXTSTATE/my%20script/mode",
   http_reply={"status": 200, "body": "EXTSTATE\tmy script\tmode\tfast\n"},
   expect_result={"ok": {"kind": "value", "value": "fast"}})
rw("set_ext_state", {"section": "show", "key": "cue", "value": "A/1;2"},
   "/_/SET/EXTSTATE/show/cue/A%2F1%3B2;GET/EXTSTATE/show/cue")
rw("set_ext_state_persist", {"section": "show", "key": "cue", "value": "7"},
   "/_/SET/EXTSTATEPERSIST/show/cue/7;GET/EXTSTATE/show/cue")
rw("get_project_ext_state", {"section": "show", "key": "act"}, "/_/GET/PROJEXTSTATE/show/act",
   http_reply={"status": 200, "body": "PROJEXTSTATE\tshow\tact\t2\n"},
   expect_result={"ok": {"kind": "value", "value": "2"}})
rw("set_project_ext_state", {"section": "show", "key": "act", "value": "2"},
   "/_/SET/PROJEXTSTATE/show/act/2;GET/PROJEXTSTATE/show/act")

# ── Actions and OSC ──────────────────────────────────────────────────────
rw("run_action", {"command": 40364}, "/_/40364;GET/40364")
rw("run_named_action", {"command": "_SWS_ABOUT"}, "/_/_SWS_ABOUT;GET/_SWS_ABOUT")
rw("get_action_state", {"command": 40364}, "/_/GET/40364",
   http_reply={"status": 200, "body": "CMDSTATE\t40364\t1\n"},
   expect_result={"ok": {"kind": "value", "value": "1"}})
rw("get_named_action_state", {"command": "_SWS_ABOUT"}, "/_/GET/_SWS_ABOUT",
   http_reply={"status": 200, "body": "CMDSTATE\t_SWS_ABOUT\t-1\n"},
   expect_result={"ok": {"kind": "value", "value": "-1"}})
rw("send_osc", {"address": "/track/3/mute/toggle"}, "/_/OSC/%2Ftrack%2F3%2Fmute%2Ftoggle")
rw("send_osc_float", {"address": "/track/3/volume", "value": 0.5}, "/_/OSC/%2Ftrack%2F3%2Fvolume:0.500000")
rw("send_osc_string", {"address": "/track/3/name", "text": "Vox"}, "/_/OSC/%2Ftrack%2F3%2Fname:sVox")

# ── Telemetry: the polls' replies and the read-backs ─────────────────────
telemetry(RW, "transport", inbound_http={
    "path": "/_/TRANSPORT;BEATPOS",
    "body": "TRANSPORT\t5\t12.500000\t1\t0:12.500\t7.1.00\n"
            "BEATPOS\t5\t12.500000\t24.000000\t6\t0.000000\t4\t4\n"},
    expect_state={"transport": {"play_state": "recording", "playing": True, "paused": False,
                                "recording": True, "position": 12.5, "repeat": True,
                                "position_text": "0:12.500", "position_beats": "7.1.00",
                                "beat_position": 24.0, "measure": 6, "beat_in_measure": 0.0,
                                "time_signature_numerator": 4, "time_signature_denominator": 4}})
telemetry(RW, "transport-paused", inbound_http={
    "path": "/_/1008;TRANSPORT", "body": "TRANSPORT\t2\t3.000000\t0\t0:03.000\t2.3.00\n"},
    expect_state={"transport": {"play_state": "paused", "playing": False, "paused": True,
                                "recording": False, "position": 3.0, "repeat": False,
                                "position_text": "0:03.000", "position_beats": "2.3.00"}})
telemetry(RW, "repeat", inbound_http={"path": "/_/GET/REPEAT", "body": "GET/REPEAT\t0\n"},
          expect_state={"transport": {"repeat": False}})
# The track poll: count, every track (0 the master), metronome and snap.
telemetry(RW, "tracks", inbound_http={
    "path": "/_/NTRACK;TRACK;GET/40364;GET/1157",
    "body": "NTRACK\t2\n"
            "TRACK\t0\tMASTER\t4\t1.000000\t0.000000\t-60\t-120\t1.000000\t3\t0\t0\t1\t0\n"
            "TRACK\t1\tKick\t72\t0.500000\t-0.250000\t-100\t-150\t1.000000\t3\t1\t0\t0\t0x01FF0000\n"
            "TRACK\t2\tVox\t26\t1.000000\t0.000000\t-1500\t-1500\t0.500000\t5\t0\t1\t0\t0\n"
            "CMDSTATE\t40364\t1\n"
            "CMDSTATE\t1157\t0\n"},
    state_before={"tracks": {"3": {"name": "Gone"}}},
    expect_state={"track_count": 2,
                  "tracks": {
                      "0": {"name": "MASTER", "flags": 4, "volume": 1.0, "pan": 0.0, "peak_db10": -60,
                            "meter_db10": -120, "width": 1.0, "pan_mode": 3, "send_count": 0,
                            "receive_count": 0, "hw_out_count": 1, "color": "0"},
                      "1": {"name": "Kick", "flags": 72, "volume": 0.5, "pan": -0.25, "peak_db10": -100,
                            "meter_db10": -150, "width": 1.0, "pan_mode": 3, "send_count": 1,
                            "receive_count": 0, "hw_out_count": 0, "color": "0x01FF0000"},
                      "2": {"name": "Vox", "flags": 26, "volume": 1.0, "pan": 0.0, "peak_db10": -1500,
                            "meter_db10": -1500, "width": 0.5, "pan_mode": 5, "send_count": 0,
                            "receive_count": 1, "hw_out_count": 0, "color": "0"}},
                  "actions": {"40364": {"state": 1}, "1157": {"state": 0}},
                  "transport": {"metronome": True},
                  "options": {"snap": False}})
# A command's read-back: the set, then the whole track list after it.
telemetry(RW, "track-readback", inbound_http={
    "path": "/_/SET/TRACK/1/MUTE/1;TRACK",
    "body": "TRACK\t0\tMASTER\t0\t1.000000\t0.000000\t-1500\t-1500\t1.000000\t3\t0\t0\t1\t0\n"
            "TRACK\t1\tKick\t8\t1.000000\t0.000000\t-1500\t-1500\t1.000000\t3\t0\t0\t0\t0\n"},
    expect_state={"tracks": {
        "0": {"name": "MASTER", "flags": 0, "volume": 1.0, "pan": 0.0, "peak_db10": -1500,
              "meter_db10": -1500, "width": 1.0, "pan_mode": 3, "send_count": 0,
              "receive_count": 0, "hw_out_count": 1, "color": "0"},
        "1": {"name": "Kick", "flags": 8, "volume": 1.0, "pan": 0.0, "peak_db10": -1500,
              "meter_db10": -1500, "width": 1.0, "pan_mode": 3, "send_count": 0,
              "receive_count": 0, "hw_out_count": 0, "color": "0"}}})
# The marker and region poll; the color field is optional.
telemetry(RW, "markers-regions", inbound_http={
    "path": "/_/MARKER;REGION",
    "body": "MARKER_LIST\n"
            "MARKER\tIntro\t1\t0.000000\n"
            "MARKER\tChorus\t5\t32.500000\t0x01FF8000\n"
            "MARKER_LIST_END\n"
            "REGION_LIST\n"
            "REGION\tVerse 1\t2\t8.000000\t24.000000\t0\n"
            "REGION_LIST_END\n"},
    state_before={"markers": {"3": {"name": "Old"}}, "regions": {"2": {"name": "Old"}}},
    expect_state={"markers": {"1": {"name": "Intro", "id": 1, "position": 0.0},
                              "2": {"name": "Chorus", "id": 5, "position": 32.5, "color": "0x01FF8000"}},
                  "regions": {"1": {"name": "Verse 1", "id": 2, "start": 8.0, "end": 24.0, "color": "0"}}})
telemetry(RW, "markers-empty", inbound_http={"path": "/_/MARKER", "body": "MARKER_LIST\nMARKER_LIST_END\n"},
          state_before={"markers": {"1": {"name": "Intro", "id": 1}}},
          expect_state={})
telemetry(RW, "track-info", inbound_http={
    "path": "/_/SET/TRACK/2/I_AUTOMODE/1;GET/TRACK/2/I_AUTOMODE", "body": "GET/TRACK/2/I_AUTOMODE\t1\n"},
    expect_state={"track_info": {"2": {"I_AUTOMODE": "1"}}})
telemetry(RW, "send", inbound_http={
    "path": "/_/SET/TRACK/0/SEND/0/MUTE/-1;GET/TRACK/0/SEND/0", "body": "SEND\t0\t0\t8\t1.000000\t0.000000\t-1\n"},
    expect_state={"track_sends": {"0": {"0": {"flags": 8, "volume": 1.0, "pan": 0.0, "other_track": -1}}}})
telemetry(RW, "receive", inbound_http={
    "path": "/_/GET/TRACK/3/SEND/-1", "body": "SEND\t3\t-1\t0\t0.500000\t-1.000000\t1\n"},
    expect_state={"track_sends": {"3": {"-1": {"flags": 0, "volume": 0.5, "pan": -1.0, "other_track": 1}}}})
telemetry(RW, "lyrics", inbound_http={
    "path": "/_/LYRICS/1", "body": "LYRICS\t1\t0.000000\tHel-\t0.500000\tlo\n"},
    state_before={"track_lyrics": {"1": {"3": {"text": "old"}}}},
    expect_state={"track_lyrics": {"1": {"1": {"beat": 0.0, "text": "Hel-"}, "2": {"beat": 0.5, "text": "lo"}}}})
telemetry(RW, "ext-state", inbound_http={
    "path": "/_/GET/EXTSTATE/my%20script/mode", "body": "EXTSTATE\tmy script\tui.mode\tfast\\nlane\n"},
    expect_state={"ext_state": {"my script": {"ui.mode": "fast\\nlane"}}})
telemetry(RW, "project-ext-state", inbound_http={
    "path": "/_/GET/PROJEXTSTATE/show/act", "body": "PROJEXTSTATE\tshow\tact\t2\n"},
    expect_state={"project_ext_state": {"show": {"act": "2"}}})
telemetry(RW, "named-action-state", inbound_http={
    "path": "/_/_SWS_ABOUT;GET/_SWS_ABOUT", "body": "CMDSTATE\t_SWS_ABOUT\t-1\n"},
    expect_state={"actions": {"_SWS_ABOUT": {"state": -1}}})
