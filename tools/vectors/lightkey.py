LK = "lightkey"
# Lightkey OSC (Lightkey User Guide, "OSC addresses", pp. 302-312), UDP to
# port 21600. Addresses and arguments are the guide's own examples where it
# gives one.


def lk(command, input, *messages, **extra):
    wires = [osc(*m) for m in messages]
    binary(LK, command, input, wires if len(wires) > 1 else wires[0], **extra)


lk("select_page", {"page": "Main_Page"}, ("/live/Main_Page/select",), expect_result={"ok": {"kind": "unverified"}})
lk("select_page_fade", {"page": "Main_Page", "fade": 2.0}, ("/live/Main_Page/select", ("f", 2.0)))
# "/live/Main_Page/cue/Red_Backlight/activate"
lk("cue", {"page": "Main_Page", "cue": "Red_Backlight", "action": "activate"},
   ("/live/Main_Page/cue/Red_Backlight/activate",))
# "/live/*/cue/Red_Backlight/activate 0.5"
lk("cue_fade", {"page": "*", "cue": "Red_Backlight", "action": "activate", "fade": 0.5},
   ("/live/*/cue/Red_Backlight/activate", ("f", 0.5)))
lk("cue_intensity", {"page": "selected", "cue": "Fader_1", "value": 0.75},
   ("/live/selected/cue/Fader_1/intensity", ("f", 0.75)))
lk("frame_step", {"page": "selected", "frame": "Backlight", "direction": "nextCue"},
   ("/live/selected/frame/Backlight/nextCue",))
lk("frame_step_fade", {"page": "selected", "frame": "Backlight", "direction": "previousCue", "fade": 1.0},
   ("/live/selected/frame/Backlight/previousCue", ("f", 1.0)))
# "/live/selected/frame/Backlight/cue/*/deactivate"
lk("live_path", {"path": "selected/frame/Backlight/cue/*", "action": "deactivate"},
   ("/live/selected/frame/Backlight/cue/*/deactivate",))
# "/live/*/cue/*/deactivate 0"
lk("live_path_fade", {"path": "*/cue/*", "action": "deactivate", "fade": 0.0},
   ("/live/*/cue/*/deactivate", ("f", 0.0)))
# "/live/selected/cue/Waterfall/speed 1.2"
lk("modifier", {"target": "selected/cue/Waterfall", "modifier": "speed", "value": 1.2},
   ("/live/selected/cue/Waterfall/speed", ("f", 1.2)))
# "/live/selected/cue/Waterfall/beatMultiplier /6"
lk("modifier_text", {"target": "selected/cue/Waterfall", "modifier": "beatMultiplier", "value": "/6"},
   ("/live/selected/cue/Waterfall/beatMultiplier", ("s", "/6")))
lk("cuelist", {"action": "togglePaused"}, ("/live/togglePaused",))
lk("cuelist_fade", {"direction": "nextCue", "fade": 3.0}, ("/live/nextCue", ("f", 3.0)))
lk("xfade", {"level": 0.5}, ("/live/xfade", ("f", 0.5)))
lk("cue_skip", {"cuelist": "Song_1", "cue": "001_Intro", "skip": "skip"}, ("/live/Song_1/cue/001_Intro/skip",))
# "/palette/Movers/Colors/Indigo"
lk("palette", {"path": "Movers/Colors/Indigo", "action": "activate"}, ("/palette/Movers/Colors/Indigo/activate",))
lk("output", {"action": "toggleBlind"}, ("/output/toggleBlind",))
lk("set_master", {"level": 1.0}, ("/output/master", ("f", 1.0)))
lk("beat", {"action": "syncDownbeat"}, ("/beat/syncDownbeat",))
lk("set_tempo", {"bpm": 128.0}, ("/beat/tempo", ("f", 128.0)))
# "/fixture/F4/overrides/dimmer 1.0"
lk("fixture_override", {"fixture": "F4", "property": "dimmer", "value": 1.0},
   ("/fixture/F4/overrides/dimmer", ("f", 1.0)))
# "/fixture/F4/overrides/color \"4b0082\""
lk("fixture_override_text", {"fixture": "F4", "property": "color", "value": "4b0082"},
   ("/fixture/F4/overrides/color", ("s", "4b0082")))
# "/fixture/F4/overrides/color 0.29 0.0 0.51"
lk("fixture_override_rgb", {"fixture": "F4", "property": "color", "red": 0.29, "green": 0.0, "blue": 0.51},
   ("/fixture/F4/overrides/color", ("f", 0.29), ("f", 0.0), ("f", 0.51)))
# "/fixture/F6/overrides/Effect Effect_7 0.15"
lk("fixture_override_option", {"fixture": "F6", "property": "Effect", "option": "Effect_7", "value": 0.15},
   ("/fixture/F6/overrides/Effect", ("s", "Effect_7"), ("f", 0.15)))
lk("fixture_clear_overrides", {"fixture": "F4"}, ("/fixture/F4/overrides/clear",))
# "/fixture/*/overrides/shutter/clear"
lk("fixture_clear_override", {"fixture": "*", "property": "shutter"}, ("/fixture/*/overrides/shutter/clear",))
lk("fixture_lamp", {"fixture": "selected", "lamp": "on"}, ("/fixture/selected/lamp/on",))
lk("fixture_command", {"fixture": "F4", "command": "Reset"}, ("/fixture/F4/command/Reset",))
lk("fixture_select", {"fixture": "F*", "action": "deselect"}, ("/fixture/F*/deselect",))
