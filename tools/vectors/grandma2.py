G = "grandma2"
# grandMA2 telnet (User Manual v3.9, Telnet Remote): one command-line entry per
# line, CR LF terminated, on TCP 30000; never acknowledged. Each expected line
# is the syntax of the keyword page named beside it (key_keyword_<name>.html).
# "Executor [ID]" is on the current page, "Executor [Page].[ID]" names the page
# (Executor keyword). Keywords are case-insensitive (General syntax rules), so
# On/Off render as ON/OFF.

# Selected executor (Go, GoBack, Goto, DefGoPause keywords).
text(G, "go", {}, "Go\r\n", expect_result={"ok": {"kind": "unverified"}})
text(G, "go_back", {}, "GoBack\r\n")
text(G, "goto_cue", {"cue": "3"}, "Goto Cue 3\r\n")
text(G, "pause", {}, "DefGoPause\r\n")

# Go / GoBack / Goto: "Go Executor 3", "GoBack Executor 3", "Goto Cue 5 Executor 4".
text(G, "go_executor", {"executor": 3}, "Go Executor 3\r\n")
text(G, "go_page_executor", {"page": 2, "executor": 3}, "Go Executor 2.3\r\n")
text(G, "go_back_executor", {"executor": 3}, "GoBack Executor 3\r\n")
text(G, "go_back_page_executor", {"page": 1, "executor": 101}, "GoBack Executor 1.101\r\n")
text(G, "goto_cue_executor", {"cue": "5", "executor": 4}, "Goto Cue 5 Executor 4\r\n")
text(G, "goto_cue_page_executor", {"cue": "4.5", "page": 3, "executor": 4}, "Goto Cue 4.5 Executor 3.4\r\n")

# Load, LoadNext, LoadPrev: "Load Cue 5 Executor 4", "LoadNext Executor 2".
text(G, "load_cue_executor", {"cue": "5", "executor": 4}, "Load Cue 5 Executor 4\r\n")
text(G, "load_cue_page_executor", {"cue": "9999.999", "page": 1, "executor": 4}, "Load Cue 9999.999 Executor 1.4\r\n")
text(G, "load_next_executor", {"executor": 2}, "LoadNext Executor 2\r\n")
text(G, "load_next_page_executor", {"page": 5, "executor": 2}, "LoadNext Executor 5.2\r\n")
text(G, "load_prev_executor", {"executor": 2}, "LoadPrev Executor 2\r\n")
text(G, "load_prev_page_executor", {"page": 5, "executor": 2}, "LoadPrev Executor 5.2\r\n")

# >>> GoFastForward and <<< GoFastBack: ">>> Executor 3", "<<< Executor 3".
text(G, "fast_forward_executor", {"executor": 3}, ">>> Executor 3\r\n")
text(G, "fast_forward_page_executor", {"page": 1, "executor": 3}, ">>> Executor 1.3\r\n")
text(G, "fast_back_executor", {"executor": 3}, "<<< Executor 3\r\n")
text(G, "fast_back_page_executor", {"page": 1, "executor": 3}, "<<< Executor 1.3\r\n")

# Pause: "Pause Executor 3" toggles; "Pause On|Off [Object-list]".
text(G, "pause_executor", {"executor": 3}, "Pause Executor 3\r\n")
text(G, "pause_page_executor", {"page": 2, "executor": 3}, "Pause Executor 2.3\r\n")
text(G, "set_pause_executor", {"active": True, "executor": 3}, "Pause ON Executor 3\r\n")
text(G, "set_pause_page_executor", {"active": False, "page": 2, "executor": 3}, "Pause OFF Executor 2.3\r\n")

# On, Off, Toggle, Top, Kill: "Toggle Executor 4", "Top Executor 5", "Kill Executor 1".
text(G, "on_executor", {"executor": 1}, "On Executor 1\r\n")
text(G, "on_page_executor", {"page": 1, "executor": 1}, "On Executor 1.1\r\n")
text(G, "off_executor", {"executor": 9}, "Off Executor 9\r\n")
text(G, "off_page_executor", {"page": 5, "executor": 9}, "Off Executor 5.9\r\n")
text(G, "toggle_executor", {"executor": 4}, "Toggle Executor 4\r\n")
text(G, "toggle_page_executor", {"page": 1, "executor": 4}, "Toggle Executor 1.4\r\n")
text(G, "top_executor", {"executor": 5}, "Top Executor 5\r\n")
text(G, "top_page_executor", {"page": 1, "executor": 5}, "Top Executor 1.5\r\n")
text(G, "kill_executor", {"executor": 1}, "Kill Executor 1\r\n")
text(G, "kill_page_executor", {"page": 3, "executor": 1}, "Kill Executor 3.1\r\n")

# Flash On/Off, FlashGo, FlashOn: "Flash On Executor 1", "FlashGo Executor 1".
text(G, "flash_executor", {"active": True, "executor": 1}, "Flash ON Executor 1\r\n")
text(G, "flash_page_executor", {"active": False, "page": 2, "executor": 4}, "Flash OFF Executor 2.4\r\n")
text(G, "flash_go_executor", {"executor": 1}, "FlashGo Executor 1\r\n")
text(G, "flash_go_page_executor", {"page": 1, "executor": 1}, "FlashGo Executor 1.1\r\n")
text(G, "flash_on_executor", {"executor": 1}, "FlashOn Executor 1\r\n")
text(G, "flash_on_page_executor", {"page": 1, "executor": 1}, "FlashOn Executor 1.1\r\n")

# Temp, Swop, SwopOn, SwopGo, Black with On/Off: "Temp On Executor 1",
# "Swop Off Executor 1", "SwopOn Off Executor 1", "SwopGo Off Executor 1",
# "Black On Executor 1".
text(G, "temp_executor", {"active": True, "executor": 1}, "Temp ON Executor 1\r\n")
text(G, "temp_page_executor", {"active": False, "page": 1, "executor": 1}, "Temp OFF Executor 1.1\r\n")
text(G, "swop_executor", {"active": True, "executor": 1}, "Swop ON Executor 1\r\n")
text(G, "swop_page_executor", {"active": False, "page": 1, "executor": 1}, "Swop OFF Executor 1.1\r\n")
text(G, "swop_on_executor", {"active": True, "executor": 1}, "SwopOn ON Executor 1\r\n")
text(G, "swop_on_page_executor", {"active": False, "page": 1, "executor": 1}, "SwopOn OFF Executor 1.1\r\n")
text(G, "swop_go_executor", {"active": True, "executor": 1}, "SwopGo ON Executor 1\r\n")
text(G, "swop_go_page_executor", {"active": False, "page": 1, "executor": 1}, "SwopGo OFF Executor 1.1\r\n")
text(G, "black_executor", {"active": True, "executor": 1}, "Black ON Executor 1\r\n")
text(G, "black_page_executor", {"active": False, "page": 1, "executor": 1}, "Black OFF Executor 1.1\r\n")

# ToFull, ToZero: "ToFull Executor 1", "ToZero Executor 1".
text(G, "to_full_executor", {"executor": 1}, "ToFull Executor 1\r\n")
text(G, "to_full_page_executor", {"page": 2, "executor": 1}, "ToFull Executor 2.1\r\n")
text(G, "to_zero_executor", {"executor": 1}, "ToZero Executor 1\r\n")
text(G, "to_zero_page_executor", {"page": 2, "executor": 1}, "ToZero Executor 2.1\r\n")

# Crossfade [value] [Executor-list]: "Crossfade 70 Executor 1".
text(G, "set_crossfade_executor", {"level": 70, "executor": 1}, "Crossfade 70 Executor 1\r\n")
text(G, "set_crossfade_page_executor", {"level": 0, "page": 1, "executor": 1}, "Crossfade 0 Executor 1.1\r\n")

# Learn, Rate1, DoubleRate, HalfRate, DoubleSpeed, HalfSpeed: "Rate1 Executor 5",
# "DoubleRate Executor 5", "HalfSpeed Executor 5".
text(G, "learn_executor", {"executor": 5}, "Learn Executor 5\r\n")
text(G, "learn_page_executor", {"page": 1, "executor": 5}, "Learn Executor 1.5\r\n")
text(G, "reset_rate_executor", {"executor": 5}, "Rate1 Executor 5\r\n")
text(G, "reset_rate_page_executor", {"page": 1, "executor": 5}, "Rate1 Executor 1.5\r\n")
text(G, "double_rate_executor", {"executor": 5}, "DoubleRate Executor 5\r\n")
text(G, "double_rate_page_executor", {"page": 1, "executor": 5}, "DoubleRate Executor 1.5\r\n")
text(G, "half_rate_executor", {"executor": 5}, "HalfRate Executor 5\r\n")
text(G, "half_rate_page_executor", {"page": 1, "executor": 5}, "HalfRate Executor 1.5\r\n")
text(G, "double_speed_executor", {"executor": 5}, "DoubleSpeed Executor 5\r\n")
text(G, "double_speed_page_executor", {"page": 1, "executor": 5}, "DoubleSpeed Executor 1.5\r\n")
text(G, "half_speed_executor", {"executor": 5}, "HalfSpeed Executor 5\r\n")
text(G, "half_speed_page_executor", {"page": 1, "executor": 5}, "HalfSpeed Executor 1.5\r\n")

# Fix On|Off, Select: "Fix On Executor 1 Thru 5", "Select Executor 4.2".
text(G, "fix_executor", {"active": True, "executor": 3}, "Fix ON Executor 3\r\n")
text(G, "fix_page_executor", {"active": False, "page": 1, "executor": 3}, "Fix OFF Executor 1.3\r\n")
text(G, "select_executor", {"executor": 5}, "Select Executor 5\r\n")
text(G, "select_page_executor", {"page": 4, "executor": 2}, "Select Executor 4.2\r\n")

# At after an object list: "Executor 3 At 50".
text(G, "set_fader_executor", {"level": 50, "executor": 3}, "Executor 3 At 50\r\n")
text(G, "set_fader_page_executor", {"level": 100, "page": 1, "executor": 15}, "Executor 1.15 At 100\r\n")

# Macro, Sequence/Cue, Timecode, Timer, Pause: "Go Macro 2", "Toggle Sequence 1",
# "Go Timecode 2", "Top Timecode 2".
text(G, "fire_macro", {"macro": 2}, "Go Macro 2\r\n")
text(G, "pause_macro", {"macro": 2}, "Pause Macro 2\r\n")
text(G, "goto_cue_sequence", {"cue": "3.999", "sequence": 5}, "Goto Cue 3.999 Sequence 5\r\n")
text(G, "toggle_sequence", {"sequence": 1}, "Toggle Sequence 1\r\n")
text(G, "go_timecode", {"timecode": 2}, "Go Timecode 2\r\n")
text(G, "top_timecode", {"timecode": 2}, "Top Timecode 2\r\n")
text(G, "pause_timecode", {"timecode": 2}, "Pause Timecode 2\r\n")
text(G, "pause_timer", {"timer": 4}, "Pause Timer 4\r\n")

# Page, FaderPage, ButtonPage, ChannelPage: "Page 5", "FaderPage 5",
# "ButtonPage 20", "ChannelPage 5", "Pause Page 3".
text(G, "call_page", {"page": 5}, "Page 5\r\n")
text(G, "fader_page", {"page": 5}, "FaderPage 5\r\n")
text(G, "button_page", {"page": 20}, "ButtonPage 20\r\n")
text(G, "channel_page", {"page": 5}, "ChannelPage 5\r\n")
text(G, "pause_page", {"page": 3}, "Pause Page 3\r\n")

# SpecialMaster 2.1 (Grand): "SpecialMaster ... At 50", "ToFull SpecialMaster 2.1",
# "ToZero SpecialMaster 2.1".
text(G, "set_grand_master", {"level": 50}, "SpecialMaster 2.1 At 50\r\n")
text(G, "grand_master_full", {}, "ToFull SpecialMaster 2.1\r\n")
text(G, "grand_master_zero", {}, "ToZero SpecialMaster 2.1\r\n")

# Blackout, Blind, Freeze, Solo, Highlight: "Blackout On", or alone to toggle.
text(G, "set_blackout", {"active": True}, "Blackout ON\r\n")
text(G, "toggle_blackout", {}, "Blackout\r\n")
text(G, "set_blind", {"active": False}, "Blind OFF\r\n")
text(G, "toggle_blind", {}, "Blind\r\n")
text(G, "set_freeze", {"active": True}, "Freeze ON\r\n")
text(G, "toggle_freeze", {}, "Freeze\r\n")
text(G, "set_solo", {"active": True}, "Solo ON\r\n")
text(G, "toggle_solo", {}, "Solo\r\n")
text(G, "set_highlight", {"active": False}, "Highlight OFF\r\n")
text(G, "toggle_highlight", {}, "Highlight\r\n")

# View, ViewPage, World: "View 2", "ViewPage 2", "World 4".
text(G, "call_view", {"view": 2}, "View 2\r\n")
text(G, "view_page", {"page": 11}, "ViewPage 11\r\n")
text(G, "call_world", {"world": 4}, "World 4\r\n")

# Programmer: "Group 3", "Fixture 1 At 75" (At), "At Preset 3.2", Clear family.
text(G, "select_group", {"group": 3}, "Group 3\r\n")
text(G, "group_at", {"group": 3, "level": 75}, "Group 3 At 75\r\n")
text(G, "fixture_at", {"fixture": 53, "level": 100}, "Fixture 53 At 100\r\n")
text(G, "channel_at", {"channel": 34, "level": 0}, "Channel 34 At 0\r\n")
text(G, "at_preset", {"preset_type": 3, "preset": 2}, "At Preset 3.2\r\n")
text(G, "clear", {}, "Clear\r\n")
text(G, "clear_selection", {}, "ClearSelection\r\n")
text(G, "clear_active", {}, "ClearActive\r\n")
text(G, "clear_all", {}, "ClearAll\r\n")

# SaveShow; and the raw entry point.
text(G, "save_show", {}, "SaveShow\r\n")
text(G, "command", {"line": "Blackout"}, "Blackout\r\n")
