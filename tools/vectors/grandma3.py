G3 = "grandma3"
# grandMA3 OSC (User Manual v2.5, Remote In and Out > OSC and its Advanced
# Examples page): one OSC 1.0 message per UDP datagram, never acknowledged.
# Executor handles are /[DataPoolZ/][PageY/](Fader|Encoder|Key)X; command-line
# input is /cmd with one string argument, the line exactly as typed on the
# console. Each expected string below is typed from the keyword page or OSC
# example named beside it, not taken from the spec.


def g3(command, input, *messages, **extra):
    """messages: (address, *args) tuples; several become an ordered list."""
    wires = [osc(m[0], *m[1:]) for m in messages]
    binary(G3, command, input, wires if len(wires) > 1 else wires[0], **extra)


def g3cmd(command, input, line, **extra):
    g3(command, input, ("/cmd", ("s", line)), **extra)


# ── Executor handles (Advanced Examples, Executor Control table) ──────────
# /Fader201,i,100 and /DataPool2/Page10/Fader302,f,75.5 are MA's examples; the
# spec sends faders as float.
g3("fader", {"executor": 201, "level": 100.0}, ("/Fader201", ("f", 100.0)),
   expect_result={"ok": {"kind": "unverified"}})
g3("fader_page", {"page": 1, "executor": 201, "level": 50.0}, ("/Page1/Fader201", ("f", 50.0)))
g3("fader_datapool", {"datapool": 2, "executor": 302, "level": 0.0}, ("/DataPool2/Fader302", ("f", 0.0)))
g3("fader_datapool_page", {"datapool": 2, "page": 10, "executor": 302, "level": 75.5},
   ("/DataPool2/Page10/Fader302", ("f", 75.5)))
# SendOSC: "/Page1/Fader201,ii,100,5" is value 100 with a 5 s fade.
g3("fader_fade", {"executor": 201, "level": 100, "fade": 5}, ("/Fader201", ("i", 100), ("i", 5)))
g3("fader_fade_page", {"page": 1, "executor": 201, "level": 100, "fade": 5}, ("/Page1/Fader201", ("i", 100), ("i", 5)))
g3("fader_fade_datapool", {"datapool": 1, "executor": 202, "level": 0, "fade": 3},
   ("/DataPool1/Fader202", ("i", 0), ("i", 3)))
g3("fader_fade_datapool_page", {"datapool": 1, "page": 2, "executor": 202, "level": 40, "fade": 0},
   ("/DataPool1/Page2/Fader202", ("i", 40), ("i", 0)))
# /Encoder201,i,-50 ; /Encoder201,i,-1 (EncoderLeft) ; /Encoder201,i,1 (EncoderRight).
g3("encoder", {"executor": 201, "delta": -50}, ("/Encoder201", ("i", -50)))
g3("encoder_page", {"page": 1, "executor": 301, "delta": -1}, ("/Page1/Encoder301", ("i", -1)))
g3("encoder_datapool", {"datapool": 1, "executor": 401, "delta": 1}, ("/DataPool1/Encoder401", ("i", 1)))
g3("encoder_datapool_page", {"datapool": 3, "page": 4, "executor": 302, "delta": 100},
   ("/DataPool3/Page4/Encoder302", ("i", 100)))
# /Key101,i,1 press ; /Key201,f,0 release (the spec releases with i 0).
g3("key_down", {"executor": 101}, ("/Key101", ("i", 1)))
g3("key_up", {"executor": 201}, ("/Key201", ("i", 0)))
g3("press_key", {"executor": 101}, ("/Key101", ("i", 1)), ("/Key101", ("i", 0)))
g3("key_down_page", {"page": 1, "executor": 229}, ("/Page1/Key229", ("i", 1)))
g3("key_up_page", {"page": 1, "executor": 229}, ("/Page1/Key229", ("i", 0)))
g3("press_key_page", {"page": 1, "executor": 230}, ("/Page1/Key230", ("i", 1)), ("/Page1/Key230", ("i", 0)))
g3("key_down_datapool", {"datapool": 2, "executor": 191}, ("/DataPool2/Key191", ("i", 1)))
g3("key_up_datapool", {"datapool": 2, "executor": 191}, ("/DataPool2/Key191", ("i", 0)))
g3("press_key_datapool", {"datapool": 2, "executor": 298}, ("/DataPool2/Key298", ("i", 1)), ("/DataPool2/Key298", ("i", 0)))
g3("key_down_datapool_page", {"datapool": 1, "page": 9999, "executor": 490}, ("/DataPool1/Page9999/Key490", ("i", 1)))
g3("key_up_datapool_page", {"datapool": 1, "page": 9999, "executor": 490}, ("/DataPool1/Page9999/Key490", ("i", 0)))
g3("press_key_datapool_page", {"datapool": 1, "page": 3, "executor": 105},
   ("/DataPool1/Page3/Key105", ("i", 1)), ("/DataPool1/Page3/Key105", ("i", 0)))

# ── Object playback control (OSC page table) ──────────────────────────────
# /13.13.1.6.1,si,Flash,1 press; ...,0 release; sif <Fader function>,3,value;
# sii <Fader function>,handle,delta.
g3("object_key_down", {"object": "13.13.1.6.1", "function": "Flash"}, ("/13.13.1.6.1", ("s", "Flash"), ("i", 1)))
g3("object_key_up", {"object": "13.13.1.6.1", "function": "Flash"}, ("/13.13.1.6.1", ("s", "Flash"), ("i", 0)))
g3("object_press_key", {"object": "13.12.1.1", "function": "Black"},
   ("/13.12.1.1", ("s", "Black"), ("i", 1)), ("/13.12.1.1", ("s", "Black"), ("i", 0)))
g3("object_fader", {"object": "13.12.3.1", "function": "FaderMaster", "level": 63.5},
   ("/13.12.3.1", ("s", "FaderMaster"), ("i", 3), ("f", 63.5)))
g3("object_fader_relative", {"object": "13.13.1.6.1", "function": "FaderMaster", "handle": 0, "delta": -1},
   ("/13.13.1.6.1", ("s", "FaderMaster"), ("i", 0), ("i", -1)))

# ── Command line (/cmd) ───────────────────────────────────────────────────
# OSC page examples: "FaderMaster Page 1.201 At 50 Fade 5", "Fixture 1 At 75",
# "Go+ Exec 402"; SendOSC: "Store Cue 1".
g3cmd("command", {"line": "Go+ Exec 402"}, "Go+ Exec 402")

# Executor (current page), Page X.Y and Sequence N targets.
#   Go+ Executor 101 (Go+), Go- Executor 101 (Go-), Top Executor 105 (Top),
#   On, Off Sequence 1 (Off), Toggle Executor 104 (Toggle), Kill Executor 102
#   (Kill), Pause Executor 201 (Pause), >>> Page 5.211 / <<< Executor 103,
#   Select Executor 105 (Select), Rate1 Executor 105, Speed1 Executor 201,
#   DoubleSpeed Sequence 2, HalfSpeed, LearnSpeed.
for name, kw in [("go", "Go+"), ("go_back", "Go-"), ("top", "Top"), ("on", "On"), ("off", "Off"),
                 ("toggle", "Toggle"), ("kill", "Kill"), ("pause", "Pause"), ("fast_forward", ">>>"),
                 ("fast_back", "<<<"), ("select", "Select"), ("rate1", "Rate1"), ("speed1", "Speed1"),
                 ("double_speed", "DoubleSpeed"), ("half_speed", "HalfSpeed"), ("learn_speed", "LearnSpeed")]:
    g3cmd(f"{name}_executor", {"executor": 101}, f"{kw} Executor 101")
    g3cmd(f"{name}_page_executor", {"page": 5, "executor": 211}, f"{kw} Page 5.211")
    g3cmd(f"{name}_sequence", {"sequence": 3}, f"{kw} Sequence 3")

# Top Executor 105 Fade 3 (Top).
g3cmd("top_fade_executor", {"executor": 105, "fade": 3}, "Top Executor 105 Fade 3")
g3cmd("top_fade_page_executor", {"page": 1, "executor": 105, "fade": 3}, "Top Page 1.105 Fade 3")
g3cmd("top_fade_sequence", {"sequence": 1, "fade": 0}, "Top Sequence 1 Fade 0")

# Flash On Executor 201, Temp Off Executor 104, Black On Executor 201,
# Swap (On/Off), Pause (On/Off).
for name, kw in [("flash", "Flash"), ("temp", "Temp"), ("swap", "Swap"), ("black", "Black"),
                 ("set_pause", "Pause")]:
    g3cmd(f"{name}_executor", {"state": "On", "executor": 201}, f"{kw} On Executor 201")
    g3cmd(f"{name}_page_executor", {"state": "Off", "page": 2, "executor": 104}, f"{kw} Off Page 2.104")
    g3cmd(f"{name}_sequence", {"state": "On", "sequence": 5}, f"{kw} On Sequence 5")
# Fix On Executor 101 (Fix).
g3cmd("fix_executor", {"state": "On", "executor": 101}, "Fix On Executor 101")
g3cmd("fix_page_executor", {"state": "Off", "page": 1, "executor": 103}, "Fix Off Page 1.103")

# Goto Cue 105 Executor 104 ; Goto Cue 10 Executor 201 Fade 2 ; Goto Sequence 42 Cue ...
g3cmd("goto_cue_executor", {"cue": "105", "executor": 104}, "Goto Cue 105 Executor 104")
g3cmd("goto_cue_page_executor", {"cue": "2.5", "page": 3, "executor": 104}, "Goto Cue 2.5 Page 3.104")
g3cmd("goto_cue_sequence", {"sequence": 42, "cue": "7"}, "Goto Sequence 42 Cue 7")
g3cmd("goto_cue_fade_executor", {"cue": "10", "executor": 201, "fade": 2}, "Goto Cue 10 Executor 201 Fade 2")
g3cmd("goto_cue_fade_page_executor", {"cue": "10", "page": 1, "executor": 201, "fade": 2}, "Goto Cue 10 Page 1.201 Fade 2")
g3cmd("goto_cue_fade_sequence", {"sequence": 42, "cue": "9999.999", "fade": 2}, "Goto Sequence 42 Cue 9999.999 Fade 2")
# Load Executor 114 Cue 5 (Load).
g3cmd("load_cue_executor", {"executor": 114, "cue": "5"}, "Load Executor 114 Cue 5")
g3cmd("load_cue_page_executor", {"page": 2, "executor": 114, "cue": "5"}, "Load Page 2.114 Cue 5")
g3cmd("load_cue_sequence", {"sequence": 3, "cue": "0.5"}, "Load Sequence 3 Cue 0.5")

# FaderMaster Page 1.201 At 50 ; FaderMaster Sequence 1 At 30 ; FaderTemp
# Sequence 2 At 42 ; FaderCrossFade(A/B) Sequence 5 At 10 ; FaderRate Sequence 1
# At 100 ; FaderTime Sequence 5 At 10.
for name, kw in [("set_master", "FaderMaster"), ("set_fader_temp", "FaderTemp"),
                 ("set_crossfade", "FaderCrossFade"), ("set_crossfade_a", "FaderCrossFadeA"),
                 ("set_crossfade_b", "FaderCrossFadeB"), ("set_rate", "FaderRate"), ("set_time", "FaderTime")]:
    g3cmd(f"{name}_executor", {"executor": 205, "level": 50}, f"{kw} Executor 205 At 50")
    g3cmd(f"{name}_page_executor", {"page": 1, "executor": 201, "level": 50}, f"{kw} Page 1.201 At 50")
    g3cmd(f"{name}_sequence", {"sequence": 1, "level": 30}, f"{kw} Sequence 1 At 30")
g3cmd("set_master_fade_executor", {"executor": 205, "level": 50, "fade": 5}, "FaderMaster Executor 205 At 50 Fade 5")
g3cmd("set_master_fade_page_executor", {"page": 1, "executor": 201, "level": 50, "fade": 5},
      "FaderMaster Page 1.201 At 50 Fade 5")
g3cmd("set_master_fade_sequence", {"sequence": 1, "level": 0, "fade": 10}, "FaderMaster Sequence 1 At 0 Fade 10")

# Globals.
g3cmd("call_page", {"page": 2}, "Page 2")
g3cmd("next_page", {}, "Next Page")
g3cmd("select_datapool", {"datapool": 3}, "Select DataPool 3")
g3cmd("go_macro", {"macro": 2}, "Go+ Macro 2")
g3cmd("set_grand_master", {"level": 80}, "FaderMaster Master 2.1 At 80")
g3cmd("set_blind", {"state": "On"}, "Blind On")
g3cmd("toggle_blind", {}, "Blind")
g3cmd("set_freeze", {"state": "On"}, "Freeze On")
g3cmd("toggle_freeze", {}, "Freeze")
g3cmd("set_highlight", {"state": "Off"}, "Highlight Off")
g3cmd("toggle_highlight", {}, "Highlight")
g3cmd("set_solo", {"state": "On"}, "Solo On")
g3cmd("clear", {}, "Clear")
g3cmd("clear_all", {}, "ClearAll")
g3cmd("clear_selection", {}, "ClearSelection")
g3cmd("clear_active", {}, "ClearActive")
g3cmd("select_fixture", {"fixture": 2}, "Fixture 2")
g3cmd("fixture_at", {"fixture": 1, "level": 75}, "Fixture 1 At 75")
g3cmd("select_group", {"group": 3}, "Group 3")
g3cmd("group_at", {"group": 3, "level": 100}, "Group 3 At 100")
g3cmd("at_level", {"level": 75}, "At 75")
g3cmd("at_preset", {"feature_group": 21, "preset": 45}, "At Preset 21.45")
g3cmd("zero", {}, "Zero")
g3cmd("release", {}, "Release")
g3cmd("call_view", {"view": 2}, "View 2")
g3cmd("call_world", {"world": 3}, "World 3")
g3cmd("go_timecode", {"timecode": 1}, "Go+ Timecode 1")
g3cmd("off_timecode", {"timecode": 1}, "Off Timecode 1")
g3cmd("pause_timecode", {"timecode": 1}, "Pause Timecode 1")
g3cmd("top_timecode", {"timecode": 1}, "Top Timecode 1")
g3cmd("go_timer", {"timer": 4}, "Go+ Timer 4")
g3cmd("off_timer", {"timer": 4}, "Off Timer 4")
g3cmd("pause_timer", {"timer": 4}, "Pause Timer 4")
g3cmd("save_show", {}, "SaveShow")


# ── Object Playback Feedback (OSC page; Advanced Examples feedback table) ──
# Received on the feedback_port when the line sends to this host. The sis
# example is MA's own: /13.13.1.6.1,sis,Flash,1,Strobe 1 Cue 1.
telemetry(G3, "sequence-key", inbound_hex=hexs(osc("/13.13.1.6.1", ("s", "Flash"), ("i", 1), ("s", "Strobe 1 Cue 1"))),
          expect_state={"sequences": {"1": {"keys": {"Flash": True}, "text": "Strobe 1 Cue 1"}}})
telemetry(G3, "sequence-fader", inbound_hex=hexs(osc("/13.13.1.6.4", ("s", "Master"), ("i", 3), ("f", 75.5))),
          expect_state={"sequences": {"4": {"faders": {"Master": 75.5}}}})
telemetry(G3, "sequence-relative-ignored", inbound_hex=hexs(osc("/13.13.1.6.4", ("s", "Master"), ("i", 0), ("i", 10))),
          expect_state={})
telemetry(G3, "grand-master-fader", inbound_hex=hexs(osc("/13.12.2.1", ("s", "Master"), ("i", 1), ("f", 50.0))),
          expect_state={"masters": {"2": {"1": {"faders": {"Master": 50.0}}}}})
telemetry(G3, "master-key", inbound_hex=hexs(osc("/13.12.2.1", ("s", "Flash"), ("i", 0))),
          expect_state={"masters": {"2": {"1": {"keys": {"Flash": False}}}}})
