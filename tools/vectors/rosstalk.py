R = "rosstalk"
# RossTalk (Ross Video online help, RossTalk > Carbonite/Graphite Commands and
# Acuity/Vision Commands, help build 2026-01-30): ASCII command + CR LF on TCP
# 7788, never acknowledged. Expected lines are written from the guide's own
# syntax lines and examples.
U = {"ok": {"kind": "unverified"}}

# ── Shared (both tables) ──
text(R, "custom_control", {"bank": 1, "cc": 5}, "CC 1:05\r\n", expect_result=U)  # guide example
text(R, "fade_to_black", {}, "FTB\r\n", expect_result=U)
text(R, "gpi", {"number": 4}, "GPI 04\r\n")                                  # guide example
text(R, "set_mnemonic", {"source": "IN:6", "name": "CAM 1"}, "MNEM IN:6:CAM 1\r\n")  # guide example
text(R, "select_program", {"source": 6, "me": "2"}, "XPT ME:2:PGM:IN:6\r\n")  # guide example
text(R, "select_preset", {"source": 12}, "XPT ME:1:PST:IN:12\r\n")
text(R, "select_key", {"me": "3", "key": 2, "source": 20}, "XPT ME:3:KEY:2:IN:20\r\n")  # guide example
text(R, "select_key_second", {"me": "1", "key": 3, "source": 4}, "XPT ME:1:KEY:3:KEYBUS:2:IN:4\r\n")
text(R, "crosspoint", {"destination": "AUX:2", "source": "ME:1:CLN"}, "XPT AUX:2:ME:1:CLN\r\n")  # guide example

# ── Carbonite family ──
text(R, "chroma_key_init", {"chroma_key": 2}, "CKINIT 2\r\n")              # guide example
text(R, "chroma_key_init_keyer", {"me": "3", "key": 6}, "CKINIT ME:3:6\r\n", model="carbonite-hypermax")
text(R, "clip_load", {"clip": "trees"}, "CLIPLOAD trees\r\n")
text(R, "clip_load_channel", {"clip": "trees", "channel": 1}, "CLIPLOAD trees:1\r\n")  # guide example
text(R, "clip_eject", {}, "CLIPEJECT\r\n")
text(R, "clip_eject_channel", {"channel": 2}, "CLIPEJECT 2\r\n")
text(R, "clip_play", {}, "CLIPPLAY\r\n")
text(R, "clip_play_channel", {"channel": 1}, "CLIPPLAY 1\r\n")
text(R, "clip_pause", {}, "CLIPPAUSE\r\n")
text(R, "clip_pause_channel", {"channel": 1}, "CLIPPAUSE 1\r\n")
text(R, "clip_loop", {"loop": False}, "CLIPLOOPOFF\r\n")
text(R, "clip_loop_channel", {"channel": 2, "loop": True}, "CLIPLOOPON 2\r\n")
text(R, "create_cc_variable", {"name": "Ad-Counter", "current": 5, "default_value": 0},
     "CREATECCVARIABLE Ad-Counter:5:0\r\n")                                  # guide example
text(R, "default_cc_variable", {"name": "Home-Score"}, "DEFAULTCCVARIABLE Home-Score\r\n")
text(R, "delete_cc_variable", {"name": "Low-Score"}, "DELETECCVARIABLE Low-Score\r\n")
text(R, "set_cc_variable", {"name": "Ad-Counter", "value": 10}, "SETCCVARIABLE Ad-Counter:10\r\n")
text(R, "sequencer_up", {"sequencer": 1}, "UP 1\r\n")
text(R, "sequencer_down", {"sequencer": 4}, "DOWN 4\r\n")
text(R, "sequencer_focus", {"sequencer": 3, "event": 2}, "FOCUS 3:2\r\n")
text(R, "sequencer_next", {"sequencer": 2}, "NEXT 2\r\n")
text(R, "sequence_load", {"sequencer": 3, "sequence": 15}, "SEQI 3:15\r\n")
text(R, "sequence_unload", {"sequencer": 1}, "SEQO 1\r\n")
text(R, "gpi_level", {"number": 8, "level": "HIGH"}, "GPI 08:HIGH\r\n", model="graphite")
text(R, "key_auto", {"me": "1", "key": 4}, "KEYAUTO ME:1:4\r\n")
text(R, "key_auto_on", {"me": "1", "key": 4}, "KEYAUTOON ME:1:4\r\n")
text(R, "key_auto_off", {"me": "P/P", "key": 4}, "KEYAUTOOFF ME:P/P:4\r\n", model="carbonite-ultra")
text(R, "key_cut", {"me_type": "MME", "me": "2", "key": 1}, "KEYCUT MME:2:1\r\n")
text(R, "key_cut_on", {"me_type": "MME", "me": "2", "key": 1}, "KEYCUTON MME:2:1\r\n")
text(R, "key_cut_off", {"me_type": "MME", "me": "2", "key": 1}, "KEYCUTOFF MME:2:1\r\n")
text(R, "key_mode", {"me": "2", "key": 1, "mode": "NORMAL"}, "KEYMODE ME:2:1:NORMAL\r\n")
text(R, "load_set", {"name": "set1"}, "LOADSET set1\r\n")
text(R, "load_set_from", {"location": 1, "name": "set1"}, "LOADSET 1:set1\r\n", model="carbonite-ultra-60")
text(R, "save_set", {"name": "set1"}, "SAVESET set1\r\n")
text(R, "save_set_to", {"location": 1, "name": "news6"}, "SAVESET 1:news6\r\n", model="carbonite-hypermax")
text(R, "auto_transition", {"me_type": "MSC", "me": "2"}, "MEAUTO MSC:2\r\n")
text(R, "cut", {"me": "1B"}, "MECUT ME:1B\r\n", model="carbonite-hypermax")
text(R, "recall_memory", {"bank": 1, "memory": 9, "me": "2"}, "MEM 19:ME:2\r\n")
text(R, "recall_memory_multi", {"bank": 1, "memory": 9, "targets": "ME:2:MME:1:CK:1"},
     "MEM 19:ME:2:MME:1:CK:1\r\n")                                           # guide example
text(R, "save_memory", {"bank": 2, "memory": 5, "me_type": "MME", "me": "3"}, "MEMSAVE 25:MME:3\r\n")
text(R, "save_memory_multi", {"bank": 2, "memory": 5, "targets": "ME:1:MME:3"}, "MEMSAVE 25:ME:1:MME:3\r\n")
text(R, "media_store_load", {"channel": 1, "location": 0, "media": 2}, "MS 1:0:002\r\n")  # guide example
text(R, "media_store_load_id", {"channel": 3, "media": 35}, "MS 3:035\r\n", model="carbonite-hypermax")
text(R, "motiontext_load", {"bank": 3, "preset": 22}, "MTLoad 3:22\r\n", model="carbonite-code")
text(R, "motiontext_take_in", {}, "MTTakeIn\r\n", model="carbonite-code")
text(R, "motiontext_take_out", {}, "MTTakeOut\r\n", model="carbonite-code")
text(R, "motiontext_set_text", {"text_box": 4, "text": "The quick brown fox jumps over the lazy dog."},
     "MTText 4:The quick brown fox jumps over the lazy dog.\r\n", model="carbonite-code")
text(R, "multiviewer_box", {"processor": "VP", "multiviewer": 1, "box": 5, "source": "IN:6"},
     "MVBOX VP:1:5:IN:6\r\n")
text(R, "multiviewer_box_shift", {"processor": "VP", "multiviewer": 1, "box": 5, "source": "IN:6"},
     "MVBOXSHIFT VP:1:5:IN:6\r\n")
text(R, "noop", {}, "NOOP\r\n")
text(R, "ntp_off", {}, "NTP OFF\r\n")
text(R, "ntp_set", {"address": "10.10.10.10"}, "NTP SET:10.10.10.10\r\n")
text(R, "reboot_all", {}, "REBOOTALL\r\n", model="carbonite-hypermax")
text(R, "transition_include", {"me": "2", "include": "B:2:3"}, "TRANSINCL ME:2:B:2:3\r\n")
text(R, "transition_rate", {"me": "1", "rate": 15}, "TRANSRATE ME:1:15\r\n")
text(R, "transition_type", {"me_type": "MSC", "me": "2", "type": "DISS"}, "TRANSTYPE MSC:2:DISS\r\n")
text(R, "select_aux", {"aux": 2, "source": 6}, "XPT AUX:2:IN:6\r\n")
text(R, "select_minime", {"minime": 1, "source": 3}, "XPT MME:1:IN:3\r\n")

# ── Acuity family ──
A = {"model": "acuity"}
text(R, "acuity_capture", {"target": "DISK", "channel": 1, "source": "AUX:8:3", "frames": 6},
     "CAPTURE DISK:1:AUX:8:3:6\r\n", **A)                                   # guide example
text(R, "ultrix_acuity_capture", {"me": 3, "source": "PGMC", "channel": 2}, "CAPTURE 3:PGMC:2\r\n",
     model="ultrix-acuity")
text(R, "vtr_cue_clip", {"source": "IN:7", "clip": "Intro_6"}, "CUECLIP IN:7:Intro_6\r\n", **A)
text(R, "vtr_eject", {"source": "IN:10"}, "EJECT IN:10\r\n", **A)
text(R, "vtr_play", {"source": "IN:20"}, "PLAY IN:20\r\n", **A)
text(R, "vtr_pause", {"source": "IN:20"}, "PAUSE IN:20\r\n", **A)
text(R, "gpi_set", {"number": 4, "state": False}, "GPI 04:0\r\n", **A)
text(R, "acuity_key_auto", {"me": 1, "key": 4}, "KEYAUTO 1:4\r\n", **A)
text(R, "acuity_key_auto_on", {"me": 3, "key": 2}, "KEYAUTOON 3:2\r\n", **A)
text(R, "acuity_key_auto_off", {"me": 1, "key": 4}, "KEYAUTOOFF 1:4\r\n", **A)
text(R, "acuity_key_cut", {"me": 2, "key": 1}, "KEYCUT 2:1\r\n", **A)
text(R, "acuity_key_cut_on", {"me": 3, "key": 2}, "KEYCUTON 3:2\r\n", **A)
text(R, "acuity_key_cut_off", {"me": 1, "key": 4}, "KEYCUTOFF 1:4\r\n", **A)
text(R, "acuity_key_shaped", {"me": 2, "key": 1, "shaped": True}, "KEYSHAPED 2:1:ON\r\n", **A)
text(R, "acuity_load_set", {"drive": "HD", "name": "SETUP01"}, "LOADSET HD:SETUP01\r\n", **A)
text(R, "acuity_save_set", {"drive": "USB", "setup": 5, "name": "MORNING"}, "SAVESET USB:5:MORNING\r\n", **A)
text(R, "acuity_save_set_by_name", {"drive": "USB", "name": "MORNING"}, "SAVESET USB:MORNING\r\n", **A)
text(R, "acuity_auto_transition", {"me": 2}, "MEAUTO 2\r\n", **A)
text(R, "acuity_cut", {}, "MECUT 1\r\n", **A)
text(R, "acuity_recall_memory", {"bank": 1, "memory": 9, "me": 2}, "MEM 19:2\r\n", **A)
text(R, "acuity_recall_memory_multi", {"bank": 1, "memory": 9, "mes": "2:1"}, "MEM 19:2:1\r\n", **A)
text(R, "acuity_save_memory", {"bank": 2, "memory": 3, "me": 1}, "MEMSAVE 23:1\r\n", **A)
text(R, "acuity_save_memory_multi", {"bank": 2, "memory": 3, "mes": "1:2:4"}, "MEMSAVE 23:1:2:4\r\n", **A)
text(R, "acuity_media_store_load", {"store": "4", "channel": 2, "media": 52}, "MS 4:2:52\r\n", **A)
text(R, "acuity_media_store_play", {"store": "GSA", "channel": 2}, "MSPLAY GSA:2\r\n", **A)
text(R, "acuity_multiviewer_box", {"multiviewer": 1, "box": 5, "source": "IN:6"}, "MVBOX 1:5:IN:6\r\n", **A)
text(R, "reset_all", {}, "RESETALL\r\n", **A)
text(R, "set_video_mode", {"format": "1080i 59.94"}, "SETVIDMODE VID:1080i 59.94\r\n", **A)
text(R, "set_reference_mode", {"format": "1080i 50"}, "SETVIDMODE REF:1080i 50\r\n", **A)
text(R, "acuity_transition_include", {"me": 2, "include": "B:2:3"}, "TRANSINCL 2:B:2:3\r\n", **A)
text(R, "acuity_transition_rate", {"me": 2, "rate": 15}, "TRANSRATE 2:15\r\n", **A)
text(R, "acuity_transition_type", {"me": 3, "type": "DISS"}, "TRANSTYPE 3:DISS\r\n", **A)
text(R, "user_variable_set", {"name": "A", "value": 10}, "USERVAR A:10\r\n", **A)
text(R, "user_variable_operate", {"name": "A", "operation": "+", "value": 5}, "USERVAR A:+:5\r\n", **A)
text(R, "acuity_select_aux", {"bank": 8, "aux": 3, "source": 6}, "XPT AUX:8:3:IN:6\r\n", **A)
