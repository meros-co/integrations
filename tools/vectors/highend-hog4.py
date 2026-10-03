HOG = "highend-hog4"
# High End Systems Hog OS (manual 5.2.1, section 22.4; Hog 4 OS 3.15 for the
# MIDI note paths): OSC 1.0 over UDP, never acknowledged. Addresses and
# arguments are typed from the manual's tables and examples ("/hog/playback/go/0
# 15", "/hog/playback/go/0 15.2", "/hog/hardware/choose/9 1",
# "/hog/hardware/fader/3 255", "/hog/hardware/encoderwheel/1 -20",
# "/hog/midi/on/1/10"). Not taken from the spec.


def hog(command, input, *messages, **extra):
    """messages: (address, *args) tuples; one datagram each."""
    wires = [osc(address, *args) for address, *args in messages]
    binary(HOG, command, input, wires if len(wires) > 1 else wires[0], **extra)


# ── Playback: the path number is the object type, the argument its number ─
hog("go_list", {"list": 15}, ("/hog/playback/go/0", ("i", 15)),
    expect_result={"ok": {"kind": "unverified"}})
hog("halt_list", {"list": 15}, ("/hog/playback/halt/0", ("i", 15)))
hog("resume_list", {"list": 15}, ("/hog/playback/resume/0", ("i", 15)))
hog("release_list", {"list": 15}, ("/hog/playback/release/0", ("i", 15)))
hog("go_scene", {"scene": 156}, ("/hog/playback/go/1", ("i", 156)))
hog("halt_scene", {"scene": 156}, ("/hog/playback/halt/1", ("i", 156)))
hog("resume_scene", {"scene": 156}, ("/hog/playback/resume/1", ("i", 156)))
hog("release_scene", {"scene": 156}, ("/hog/playback/release/1", ("i", 156)))
hog("go_macro", {"macro": 18}, ("/hog/playback/go/2", ("i", 18)))
hog("halt_macro", {"macro": 18}, ("/hog/playback/halt/2", ("i", 18)))
hog("release_macro", {"macro": 18}, ("/hog/playback/release/2", ("i", 18)))
# "Goto a cue in a list ... list #.cue # ... /hog/playback/go/0 15.2"
hog("goto_cue", {"list": 15, "cue": "2"}, ("/hog/playback/go/0", ("f", 15.2)))

# ── Masters: 0 = up, 1 = down ─────────────────────────────────────────────
hog("master_key", {"key": "choose", "master": 9, "pressed": True}, ("/hog/hardware/choose/9", ("i", 1)))
hog("master_key_press", {"key": "goback", "master": 9},
    ("/hog/hardware/goback/9", ("i", 1)), ("/hog/hardware/goback/9", ("i", 0)))
hog("set_fader", {"master": 3, "level": 255}, ("/hog/hardware/fader/3", ("i", 255)))
hog("set_grand_master", {"level": 255}, ("/hog/hardware/fader/0", ("i", 255)))

# ── Keys ─────────────────────────────────────────────────────────────────
hog("key", {"key": "maingo", "pressed": True}, ("/hog/hardware/maingo", ("i", 1)))
hog("key_press", {"key": "clear"}, ("/hog/hardware/clear", ("i", 1)), ("/hog/hardware/clear", ("i", 0)))
hog("function_key", {"number": 9, "pressed": True}, ("/hog/hardware/h9", ("i", 1)))
hog("function_key_press", {"number": 1}, ("/hog/hardware/h1", ("i", 1)), ("/hog/hardware/h1", ("i", 0)))
hog("user_key", {"number": 2, "pressed": False}, ("/hog/hardware/u2", ("i", 0)))
hog("user_key_press", {"number": 2}, ("/hog/hardware/u2", ("i", 1)), ("/hog/hardware/u2", ("i", 0)))
hog("encoder_button", {"wheel": 1, "pressed": True}, ("/hog/hardware/ewheelbutton/1", ("i", 1)))

# ── Wheels ───────────────────────────────────────────────────────────────
hog("set_pos_mode", {"enabled": True}, ("/hog/hardware/posmode", ("i", 1)))
hog("move_trackball", {"x": 10, "y": 10}, ("/hog/hardware/trackball", ("i", 10), ("i", 10)))
hog("turn_encoder", {"wheel": 1, "delta": -20}, ("/hog/hardware/encoderwheel/1", ("i", -20)))
hog("turn_rate_wheel", {"delta": -20}, ("/hog/hardware/ratewheel", ("i", -20)))
hog("turn_intensity_wheel", {"delta": 5}, ("/hog/hardware/iwheel", ("i", 5)))

# ── MIDI notes: "/hog/midi/on/1/10 as note on for note 10 on channel 1" ───
hog("midi_note_on", {"channel": 1, "note": 10, "velocity": 100}, ("/hog/midi/on/1/10", ("i", 100)))
hog("midi_note_off", {"channel": 1, "note": 10}, ("/hog/midi/off/1/10", ("i", 0)))

# ── System commands: "/hog/command consoleledrefresh" ────────────────────
hog("refresh_leds", {}, ("/hog/command", ("s", "consoleledrefresh")))
hog("refresh_faders", {}, ("/hog/command", ("s", "consolefaderrefresh")))
hog("refresh_all", {}, ("/hog/command", ("s", "refreshall")))

# ── Telemetry: the status outputs (22.5.2) ───────────────────────────────
telemetry(HOG, "refresh-on-connect", expect_connect_wire_hex=[hexs(osc("/hog/command", ("s", "refreshall")))],
          inbound_hex=hexs(osc("/hog/status/commandline", ("s", "Live List 1"))),
          expect_state={"status": {"command_line": "Live List 1"}})
telemetry(HOG, "led", inbound_hex=hexs(osc("/hog/status/led/clear", ("f", 1.0))),
          expect_state={"leds": {"clear": {"value": 1.0}}})
telemetry(HOG, "led-colour", inbound_hex=hexs(osc("/hog/status/led/clearcolour", ("s", "red"))),
          expect_state={"leds": {"clear": {"colour": "red"}}})
telemetry(HOG, "led-color", inbound_hex=hexs(osc("/hog/status/led/clearcolor", ("s", "red"))),
          expect_state={"leds": {"clear": {"colour": "red"}}})
telemetry(HOG, "encoder-label", inbound_hex=hexs(osc("/hog/status/encoderwheel3/label", ("s", "Pan"))),
          expect_state={"encoder_wheels": {"3": {"label": "Pan"}}})
telemetry(HOG, "encoder-value", inbound_hex=hexs(osc("/hog/status/encoderwheel3/value", ("s", "50%"))),
          expect_state={"encoder_wheels": {"3": {"value": "50%"}}})
telemetry(HOG, "h-key-label", inbound_hex=hexs(osc("/hog/status/h1/line1", ("s", "Spot 1"))),
          expect_state={"function_keys": {"1": {"line1": "Spot 1"}}})
telemetry(HOG, "h-key-label-2", inbound_hex=hexs(osc("/hog/status/h1/line2", ("s", "Intens"))),
          expect_state={"function_keys": {"1": {"line2": "Intens"}}})
telemetry(HOG, "chat", inbound_hex=hexs(osc("/hog/status/chatline1", ("s", "Standby cue 5"))),
          expect_state={"chat": {"line1": "Standby cue 5"}})
