# 7thSense Delta (7thsense-delta): external control commands ended by CR on
# TCP 23, as the Delta v2.8 external control reference writes them (PLAY,
# GOTOFRAME n [play], GOTOTIME 0:1:0:15 30.00, LAYERFADE layer %, TL=X...),
# answered OK (recognised) or BAD (not recognised); the STATUS block of the
# return-codes topic, lines ended by CR LF.
DL = "7thsense-delta"
OK = {"ok": {"kind": "ack"}}


def _d(command, input, wire, file=None):
    v = {"spec": DL, "command": command, "input": input, "expect_wire": wire + "\r", "device_reply": "OK\r\n",
         "expect_result": OK}
    if file:
        v["file"] = file
    V.append(v)


for cmd, wire in [("cue", "CUE"), ("play", "PLAY"), ("stop", "STOP"), ("rewind", "REWIND"), ("record", "RECORD"),
                  ("clear_show", "CLEAR"), ("list_markers", "LISTMARKERS"), ("apply_show_mode", "APPLYSHOWMODE"),
                  ("toggle_3d_eyes", "TOGGLE3DEYES"), ("cancel_all_sequences", "CANCELALLSEQUENCES"),
                  ("reload_sequences", "RELOADSEQUENCES"), ("list_sequences", "LISTSEQUENCES"),
                  ("sequence_status", "SEQUENCESTATUS"), ("list_global_variables", "GLOBALVARLIST"),
                  ("get_status", "STATUS"), ("restart_pc", "RESTART_PC"), ("quit_app", "QUIT_APP")]:
    _d(cmd, {}, wire)
_d("cue_timeline", {"timeline": "2"}, "CUE TL=2")
_d("play_timeline", {"timeline": "R"}, "PLAY TL=R")
_d("stop_timeline", {"timeline": "ALL"}, "STOP TL=ALL")
_d("rewind_timeline", {"timeline": "myname"}, "REWIND TL=myname")
_d("advance", {"frames": 5}, "ADVANCE 5")
_d("step_back", {}, "STEPBACK 1")
_d("goto_marker", {"marker": "Act 2", "play": True}, 'GOTOMARKER "Act 2" play')
_d("goto_marker_timeline", {"marker": "Intro", "timeline": "3"}, 'GOTOMARKER "Intro" TL=3')
_d("goto_frame", {"frame": 1500}, "GOTOFRAME 1500")
_d("goto_frame_timeline", {"frame": 0, "play": True, "timeline": "1"}, "GOTOFRAME 0 play TL=1")
_d("goto_time", {"timecode": "0:1:0:15", "framerate": 30.0}, "GOTOTIME 0:1:0:15 30.00")
_d("rate", {"fps": 25.0}, "RATE 25.000")
_d("rate_scale", {"factor": 0.5}, "RATESCALE 0.500")
_d("smpte", {"enabled": True}, "SMPTE on")
_d("clip_search", {"direction": "next", "play": True}, "CLIPSEARCH clip=next play=true")
_d("marker_search", {"direction": "previous"}, "MARKERSEARCH direction=previous play=false")
_d("reverse_playback", {"reverse": True}, "REVERSEPLAYBACK reverse=true")
_d("audio_level", {"percent": 80}, "AUDIOLEVEL 80")
_d("video_level", {"percent": 100}, "VIDEOLEVEL 100")
_d("global_fade", {"direction": "down", "frames": 50}, "GLOBALFADE down 50")
_d("global_audio_fade", {"direction": "up", "frames": 25}, "GLOBALAUDIOFADE up 25")
_d("load_show", {"file": "TILE.xml"}, 'LOAD "TILE.xml"')
_d("merge_show", {"file": "Extra.xml"}, 'MERGE "Extra.xml"')
_d("load_show_all", {"file": "ball bounce.xml"}, 'LOADALL "ball bounce.xml"')
_d("merge_show_all", {"file": "Extra.xml"}, 'MERGEALL "Extra.xml"')
_d("save_show", {"file": "Backup.xml"}, 'SAVE "Backup.xml"')
_d("save_show_all", {"file": "Backup.xml"}, 'SAVEALL "Backup.xml"')
_d("list_shows", {"path": "D:\\Shows"}, 'LISTSHOWS "D:\\Shows"')
_d("display_mode", {"mode": "fullscreen"}, "DISPLAY fullscreen")
_d("display_mode_all", {"mode": "minimized"}, "DISPLAYALL minimized")
for cmd, word in [("geometry", "GEOMETRY"), ("blending", "BLENDING"), ("black_level_uplift", "BLACKLEVELUPLIFT"),
                  ("diagnostic_graph", "GRAPH"), ("diagnostic_stats", "STATS"), ("audio_vu", "AUDIOVU"),
                  ("channel_id", "CHANNELID"), ("reserve_timeline", "RESERVE")]:
    _d(cmd, {"enabled": False}, word + " off")
_d("layer_fade", {"layer": "3", "percent": 50}, "LAYERFADE 3 50")
_d("layer_fade_all", {"layer": "3", "percent": 0}, "LAYERFADEALL 3 0")
_d("layer_enable", {"layer": "2", "enabled": True}, "LAYERENABLE 2 on")
_d("layer_enable_all", {"layer": "2", "enabled": False}, "LAYERENABLEALL 2 off")
_d("layer_fade_timed", {"layer": "1", "direction": "up", "frames": 30}, "LAYERFADETIMED 1 up 30")
_d("layer_fade_timed_all", {"layer": "1", "direction": "down", "frames": 30}, "LAYERFADETIMEDALL 1 down 30")
_d("run_sequence", {"sequence": "Opening"}, 'SEQUENCE "Opening"')
_d("run_sequence_all", {"sequence": "Opening"}, 'SEQUENCEALL "Opening"')
_d("cancel_sequence", {"sequence": "Opening"}, 'CANCELSEQUENCE "Opening"')
_d("pause_sequence", {"sequence": "Opening", "paused": True}, 'PAUSESEQUENCE "Opening" y')
_d("load_sequences", {"file": "sequences.xml"}, 'LOADSEQUENCES "sequences.xml"')
_d("set_global_variable", {"variable": "count", "value": "5"}, "GLOBALVARSET $count=5")
_d("get_global_variable", {"variable": "count"}, "GLOBALVARGET $count")
_d("delete_global_variable", {"variable": "count"}, "GLOBALVARDELETE $count")
_d("timeline_opacity", {"timeline": "2", "percent": 50}, "TIMELINEOPACITY value=50 tl=2")
_d("timeline_opacity_fade", {"timeline": "2", "percent": 0, "time": 2.5}, "TIMELINEOPACITY value=0 Time=2.50 tl=2")
_d("timeline_visibility", {"timeline": "R", "visible": False}, "SETTIMELINEVISIBILITY visible=n tl=R")
_d("playlist_next", {"playlist": "PL1", "index": 3}, "PLAYLISTNEXT playlistextctrlname=PL1 index=3")
_d("media_enable", {"control": "logo", "enabled": True}, "MEDIAENABLE logo y")
_d("artnet", {"input": True, "output": False}, "ARTNET inputenable=y outputenable=n")
_d("external_control_comms", {"enabled": True}, "EXTCTRLCOMMS enable=true")
_d("shutdown_pc", {"immediate": True}, "SHUTDOWN_PC immediate")
_d("send_command", {"command": "TESTPATTERN pattern=grid enable=true"}, "TESTPATTERN pattern=grid enable=true")
# An unrecognised command is answered BAD.
V.append({"spec": DL, "command": "send_command", "file": "send_command.bad", "input": {"command": "PLAYY"},
          "expect_wire": "PLAYY\r", "device_reply": "BAD\r\n", "expect_result": {"error": {"error": "device_error"}}})

telemetry(DL, "status-leader", inbound="SERVER  : Server1 (Leader)\r\n",
          expect_state={"server": {"name": "Server1", "role": "Leader"}})
telemetry(DL, "banner-server", inbound="SERVER: DELTA-PC\r\n", expect_state={"server": {"name": "DELTA-PC"}})
telemetry(DL, "status-file", inbound="FILE   : ball bounce.xml\r\n", expect_state={"show": {"file": "ball bounce.xml"}})
telemetry(DL, "status-mode", inbound="MODE   : Preparing to Play\r\n", expect_state={"playback": {"mode": "Preparing to Play"}})
telemetry(DL, "status-frame", inbound="CURRENTFRAME: 311\r\n", expect_state={"playback": {"frame": 311}})
telemetry(DL, "status-framerate", inbound="FRAMERATE: 25.0\r\n", expect_state={"playback": {"framerate": 25.0}})
telemetry(DL, "status-smpte", inbound="SMPTE : OFF\r\n", expect_state={"playback": {"smpte": False}})
telemetry(DL, "status-audio", inbound="AUDIOLEVEL: 100\r\n", expect_state={"levels": {"audio": 100}})
telemetry(DL, "status-video", inbound="VIDEOLEVEL: 75\r\n", expect_state={"levels": {"video": 75}})
telemetry(DL, "status-licence", inbound="LICENCE : Permanent\r\n", expect_state={"system": {"licence": "Permanent"}})
telemetry(DL, "status-graphics", inbound="GRAPHICS: 1920 X 1080 60 HZ\r\n", expect_state={"system": {"graphics": "1920 X 1080 60 HZ"}})
telemetry(DL, "status-drives", inbound="MOVIEDRIVES: C C C C\r\n", expect_state={"system": {"movie_drives": "C C C C"}})
telemetry(DL, "status-other-drive", inbound="OTHERDRIVE: C\r\n", expect_state={"system": {"other_drive": "C"}})
telemetry(DL, "banner-ready", inbound="READY\r\n", expect_state={"device": {"ready": True}})
