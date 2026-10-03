EP = "etc-paradigm"
# ETC Paradigm Serial Access Protocol (configuration guide 7180M1250-4.0.0 Rev
# B): text over UDP, each command ended by CR. Strings are typed from the
# guide's command tables and its examples ("chan int:128 Zone 1", "chan
# int:75% Dimmer 2, Primary Space 1", "pst act Preset 1, Primary Space 1, 5",
# "seq start Sequence 1", "spc off Primary Space 1", "wall open Wall 1",
# "macro on Macro 1", "ovr enab Override 1"). Not taken from the spec.


def ep(command, input, wire, **extra):
    text(EP, command, input, wire + "\r", **extra)


SP = "Primary Space 1"
# ── Channels ─────────────────────────────────────────────────────────────
ep("channel_set", {"channel": "Zone 1", "level": 128}, "chan int:128 Zone 1",
   expect_result={"ok": {"kind": "unverified"}})
ep("channel_set_in_space", {"channel": "Dimmer 2", "space": SP, "level": 255}, "chan int:255 Dimmer 2, Primary Space 1")
ep("channel_set_in_space_fade", {"channel": "Dimmer 2", "space": SP, "level": 0, "fade": 2.5},
   "chan int:0 Dimmer 2, Primary Space 1, 2.5")
ep("channel_set_percent", {"channel": "Zone 1", "percent": 50}, "chan int:50% Zone 1")
ep("channel_set_percent_in_space", {"channel": "Dimmer 2", "space": SP, "percent": 75}, "chan int:75% Dimmer 2, Primary Space 1")
ep("channel_set_percent_in_space_fade", {"channel": "Dimmer 2", "space": SP, "percent": 75, "fade": 3.0},
   "chan int:75% Dimmer 2, Primary Space 1, 3.0")
for verb, word in [("raise", "ras"), ("lower", "low")]:
    ep(f"channel_{verb}", {"channel": "Zone 1", "amount": 25}, f"chan {word}:25 Zone 1")
    ep(f"channel_{verb}_in_space", {"channel": "Zone 1", "space": SP, "amount": 25}, f"chan {word}:25 Zone 1, Primary Space 1")
    ep(f"channel_{verb}_in_space_fade", {"channel": "Zone 1", "space": SP, "amount": 25, "fade": 1.0},
       f"chan {word}:25 Zone 1, Primary Space 1, 1.0")
ep("channel_toggle", {"channel": "Zone 1"}, "chan tog Zone 1")
ep("channel_toggle_in_space", {"channel": "Zone 1", "space": SP}, "chan tog Zone 1, Primary Space 1")
ep("channel_toggle_in_space_fade", {"channel": "Zone 1", "space": SP, "fade": 0.5}, "chan tog Zone 1, Primary Space 1, 0.5")
ep("channel_set_minimum", {"channel": "Zone 1", "level": 10}, "chan min:10 Zone 1")
ep("channel_set_minimum_in_space", {"channel": "Zone 1", "space": SP, "level": 10}, "chan min:10 Zone 1, Primary Space 1")
ep("channel_set_maximum", {"channel": "Zone 1", "level": 240}, "chan max:240 Zone 1")
ep("channel_set_maximum_in_space", {"channel": "Zone 1", "space": SP, "level": 240}, "chan max:240 Zone 1, Primary Space 1")

# ── Groups ───────────────────────────────────────────────────────────────
ep("group_set", {"group": "Group 1", "level": 128}, "grp int:128 Group 1")
ep("group_set_in_space", {"group": "Group 1", "space": SP, "level": 128}, "grp int:128 Group 1, Primary Space 1")
ep("group_set_in_space_fade", {"group": "Group 1", "space": SP, "level": 128, "fade": 3.0},
   "grp int:128 Group 1, Primary Space 1, 3.0")
ep("group_set_percent", {"group": "Group 1", "percent": 100}, "grp int:100% Group 1")
ep("group_set_percent_in_space", {"group": "Group 1", "space": SP, "percent": 100}, "grp int:100% Group 1, Primary Space 1")
ep("group_set_percent_in_space_fade", {"group": "Group 1", "space": SP, "percent": 100, "fade": 3.0},
   "grp int:100% Group 1, Primary Space 1, 3.0")
for verb, word in [("raise", "ras"), ("lower", "low")]:
    ep(f"group_{verb}", {"group": "Group 1", "amount": 10}, f"grp {word}:10 Group 1")
    ep(f"group_{verb}_in_space", {"group": "Group 1", "space": SP, "amount": 10}, f"grp {word}:10 Group 1, Primary Space 1")
    ep(f"group_{verb}_in_space_fade", {"group": "Group 1", "space": SP, "amount": 10, "fade": 1.5},
       f"grp {word}:10 Group 1, Primary Space 1, 1.5")
ep("group_toggle", {"group": "Group 1"}, "grp tog Group 1")
ep("group_toggle_in_space", {"group": "Group 1", "space": SP}, "grp tog Group 1, Primary Space 1")
ep("group_toggle_in_space_fade", {"group": "Group 1", "space": SP, "fade": 2.0}, "grp tog Group 1, Primary Space 1, 2.0")

# ── Presets ──────────────────────────────────────────────────────────────
for verb, word in [("activate", "act"), ("toggle", "tog"), ("activate_htp", "acth"), ("toggle_htp", "togh")]:
    ep(f"preset_{verb}", {"preset": "Preset 1"}, f"pst {word} Preset 1")
    ep(f"preset_{verb}_in_space", {"preset": "Preset 1", "space": SP}, f"pst {word} Preset 1, Primary Space 1")
    ep(f"preset_{verb}_in_space_fade", {"preset": "Preset 1", "space": SP, "fade": 5.0},
       f"pst {word} Preset 1, Primary Space 1, 5.0")
    ep(f"preset_{verb}_priority", {"preset": "Preset 1", "priority": 100}, f"pst {word}:100 Preset 1")
    ep(f"preset_{verb}_priority_in_space", {"preset": "Preset 1", "space": SP, "priority": 1},
       f"pst {word}:1 Preset 1, Primary Space 1")
    ep(f"preset_{verb}_priority_in_space_fade", {"preset": "Preset 1", "space": SP, "priority": 200, "fade": 5.0},
       f"pst {word}:200 Preset 1, Primary Space 1, 5.0")
for verb, word in [("deactivate", "dact"), ("deactivate_htp", "dacth")]:
    ep(f"preset_{verb}", {"preset": "Preset 1"}, f"pst {word} Preset 1")
    ep(f"preset_{verb}_in_space", {"preset": "Preset 1", "space": SP}, f"pst {word} Preset 1, Primary Space 1")
    ep(f"preset_{verb}_in_space_fade", {"preset": "Preset 1", "space": SP, "fade": 5.0},
       f"pst {word} Preset 1, Primary Space 1, 5.0")
ep("preset_record", {"preset": "Preset 1"}, "pst rec Preset 1")
ep("preset_record_in_space", {"preset": "Preset 1", "space": SP}, "pst rec Preset 1, Primary Space 1")

# ── Sequences ────────────────────────────────────────────────────────────
ep("sequence_start", {"sequence": "Sequence 1"}, "seq start Sequence 1")
ep("sequence_start_in_space", {"sequence": "Sequence 1", "space": SP}, "seq start Sequence 1, Primary Space 1")
ep("sequence_start_priority", {"sequence": "Sequence 1", "priority": 50}, "seq start:50 Sequence 1")
ep("sequence_start_priority_in_space", {"sequence": "Sequence 1", "space": SP, "priority": 50},
   "seq start:50 Sequence 1, Primary Space 1")
ep("sequence_stop", {"sequence": "Sequence 1"}, "seq stop Sequence 1")
ep("sequence_stop_in_space", {"sequence": "Sequence 1", "space": SP}, "seq stop Sequence 1, Primary Space 1")
ep("sequence_pause", {"sequence": "Sequence 1"}, "seq pause Sequence 1")
ep("sequence_resume", {"sequence": "Sequence 1"}, "seq resume Sequence 1")
ep("sequence_raise", {"sequence": "Sequence 1", "amount": 20}, "seq ras:20 Sequence 1")
ep("sequence_lower", {"sequence": "Sequence 1", "amount": 20}, "seq low:20 Sequence 1")
ep("sequence_rate", {"sequence": "Sequence 1", "rate": 1.5}, "seq rate:1.50 Sequence 1")

# ── Spaces ───────────────────────────────────────────────────────────────
ep("space_off", {"space": SP}, "spc off Primary Space 1")
ep("space_off_fade", {"space": SP, "fade": 10.0}, "spc off Primary Space 1, 10.0")
ep("space_raise", {"space": SP, "amount": 25}, "spc ras:25 Primary Space 1")
ep("space_raise_fade", {"space": SP, "amount": 25, "fade": 1.0}, "spc ras:25 Primary Space 1, 1.0")
ep("space_lower", {"space": SP, "amount": 25}, "spc low:25 Primary Space 1")
ep("space_lower_fade", {"space": SP, "amount": 25, "fade": 1.0}, "spc low:25 Primary Space 1, 1.0")
ep("space_master", {"space": SP, "level": 255}, "spc master:255 Primary Space 1")
ep("space_master_fade", {"space": SP, "level": 128, "fade": 2.0}, "spc master:128 Primary Space 1, 2.0")

# ── Walls, macros, overrides ─────────────────────────────────────────────
for verb, word in [("open", "open"), ("close", "close"), ("toggle", "tog")]:
    ep(f"wall_{verb}", {"wall": "Wall 1"}, f"wall {word} Wall 1")
    ep(f"wall_{verb}_in_space", {"wall": "Wall 1", "space": SP}, f"wall {word} Wall 1, Primary Space 1")
for verb, word in [("on", "on"), ("off", "off"), ("toggle", "tog"), ("cancel", "cancel")]:
    ep(f"macro_{verb}", {"macro": "Macro 1"}, f"macro {word} Macro 1")
for verb, word in [("enable", "enab"), ("disable", "disab"), ("toggle", "tog")]:
    ep(f"override_{verb}", {"override": "Override 1"}, f"ovr {word} Override 1")

# ── Status queries and their returned strings ────────────────────────────
ep("channel_get", {"channel": "Zone 1"}, "chan get Zone 1",
   device_reply_hex=hexs(b"chan int:128 Zone 1, Primary Space 1\r"),
   expect_result={"ok": {"kind": "value", "value": "128"}})
ep("channel_get_in_space", {"channel": "Zone 1", "space": SP}, "chan get Zone 1, Primary Space 1")
ep("group_get", {"group": "Group 1"}, "grp get Group 1")
ep("group_get_in_space", {"group": "Group 1", "space": SP}, "grp get Group 1, Primary Space 1")
ep("preset_get", {"preset": "Preset 1"}, "pst get Preset 1",
   device_reply_hex=hexs(b"pst act Preset 1, Primary Space 1\r"),
   expect_result={"ok": {"kind": "value", "value": "act"}})
ep("preset_get_in_space", {"preset": "Preset 1", "space": SP}, "pst get Preset 1, Primary Space 1")
ep("wall_get", {"wall": "Wall 1"}, "wall get Wall 1")
ep("wall_get_in_space", {"wall": "Wall 1", "space": SP}, "wall get Wall 1, Primary Space 1")
ep("sequence_get", {"sequence": "Sequence 1"}, "seq get Sequence 1")
ep("sequence_get_in_space", {"sequence": "Sequence 1", "space": SP}, "seq get Sequence 1, Primary Space 1")
ep("macro_get", {"macro": "Macro 1"}, "macro get Macro 1",
   device_reply_hex=hexs(b"macro running Macro 1\r"),
   expect_result={"ok": {"kind": "value", "value": "running"}})
ep("override_get", {"override": "Override 1"}, "ovr get Override 1")
ep("help", {}, "help")

# ── Telemetry: returned strings and PSAP triggers have the same form, one
# datagram each ─────────────────────────────────────────────────────────
telemetry(EP, "channel", inbound_hex=hexs(b"chan int:128 Zone 1, Primary Space 1\r"),
          expect_state={"spaces": {SP: {"channels": {"Zone 1": {"level": 128}}}}})
telemetry(EP, "group", inbound_hex=hexs(b"grp int:255 Group 1, Primary Space 1\r"),
          expect_state={"spaces": {SP: {"groups": {"Group 1": {"level": 255}}}}})
telemetry(EP, "preset", inbound_hex=hexs(b"pst alt Preset 1, Primary Space 1\r"),
          expect_state={"spaces": {SP: {"presets": {"Preset 1": {"state": "altered"}}}}})
telemetry(EP, "sequence", inbound_hex=hexs(b"seq pause Sequence 1, Primary Space 1\r"),
          expect_state={"spaces": {SP: {"sequences": {"Sequence 1": {"state": "paused"}}}}})
telemetry(EP, "wall", inbound_hex=hexs(b"wall close Wall 1, Primary Space 1\r"),
          expect_state={"spaces": {SP: {"walls": {"Wall 1": {"state": "closed"}}}}})
telemetry(EP, "macro", inbound_hex=hexs(b"macro running Macro 1\r"), expect_state={"macros": {"Macro 1": {"state": "running"}}})
telemetry(EP, "override", inbound_hex=hexs(b"ovr enab Override 1\r"), expect_state={"overrides": {"Override 1": {"enabled": True}}})
