# Behringer WING (behringer-wing): one vector per command, plus telemetry.
# Addresses are the "WING Remote Protocols" V3.0.6-27 address tree, written
# out per strip from the document's /ch/1, /aux/1, ... tables (p.47-65, the
# configuration nodes p.34-38, /$syscfg p.39, /$ctl/lib and /$ctl/OSC p.88).
# Wire rules from the same document: a set carries one argument of the
# parameter's own type (,f for dB and other floats, ,i for 0/1 switches and
# integers, ,s for strings and enumerations, p.22); -1 toggles a 0/1 switch
# (p.22); a query is the bare address; a float query answers ,sff (text, raw,
# value), an int ,sfi (text, raw, value) and a string ,s (p.21).
W = "behringer-wing"


def _f32(x):
    """A float as the console sends it: rounded to 32 bits."""
    return struct.unpack(">f", struct.pack(">f", x))[0]


def _q(spec_cmd, inp, address, reply_args, value):
    """A query: bare address out, the documented reply in, the value back."""
    binary(W, spec_cmd, inp, osc(address),
           device_reply_hex=hexs(osc(address, *reply_args)),
           expect_result={"ok": {"kind": "value", "value": value}})


def _fq(spec_cmd, inp, address, text, raw, v):          # ,sff reply
    _q(spec_cmd, inp, address, [("s", text), ("f", raw), ("f", v)], v)


def _iq(spec_cmd, inp, address, v):                     # ,sfi reply
    _q(spec_cmd, inp, address, [("s", str(v)), ("f", float(v)), ("i", v)], v)


def _sq(spec_cmd, inp, address, v):                     # ,s reply
    _q(spec_cmd, inp, address, [("s", v)], v)


# Strip tables: (command stem, OSC node, last index, name length,
# parametric EQ bands or 0, has pan, has dynamics).
_STRIPS = [("channel", "ch", 40, 16, 4, True, True), ("aux", "aux", 8, 16, 4, True, True),
           ("bus", "bus", 16, 16, 6, True, True), ("main", "main", 4, 16, 6, True, True),
           ("matrix", "mtx", 8, 16, 6, True, True), ("dca", "dca", 16, 8, 0, False, False)]

for stem, node, last, nlen, bands, pan, dyn in _STRIPS:
    # Fader in dB as ,f (p.22: "/ch/2/fdr ,f -3.0"); the reply's third value is dB.
    binary(W, f"set_{stem}_fader", {stem: last, "level_db": -3.0}, osc(f"/{node}/{last}/fdr", ("f", -3.0)),
           expect_result={"ok": {"kind": "unverified"}})
    _fq(f"get_{stem}_fader", {stem: 1}, f"/{node}/1/fdr", "-2.0", 0.7, -2.0)
    # Mute ,i 1 / 0, and ,i -1 to toggle.
    binary(W, f"mute_{stem}", {stem: 1, "muted": True}, osc(f"/{node}/1/mute", ("i", 1)))
    _iq(f"get_{stem}_mute", {stem: 1}, f"/{node}/1/mute", 1)
    binary(W, f"toggle_{stem}_mute", {stem: 2}, osc(f"/{node}/2/mute", ("i", -1)))
    binary(W, f"set_{stem}_name", {stem: 3, "name": "Pad"}, osc(f"/{node}/3/name", ("s", "Pad")))
    _sq(f"get_{stem}_name", {stem: 2}, f"/{node}/2/name", "Vocals")
    binary(W, f"set_{stem}_color", {stem: 1, "color": 12}, osc(f"/{node}/1/col", ("i", 12)))
    _iq(f"get_{stem}_color", {stem: 1}, f"/{node}/1/col", 5)
    binary(W, f"solo_{stem}", {stem: 1, "soloed": False}, osc(f"/{node}/1/$solo", ("i", 0)))
    _iq(f"get_{stem}_solo", {stem: 1}, f"/{node}/1/$solo", 0)
    if pan:
        binary(W, f"set_{stem}_pan", {stem: 2, "pan": -50.0}, osc(f"/{node}/2/pan", ("f", -50.0)))
        _fq(f"get_{stem}_pan", {stem: 2}, f"/{node}/2/pan", "-50", 0.25, -50.0)
    if bands:
        binary(W, f"set_{stem}_eq", {stem: 1, "enabled": True}, osc(f"/{node}/1/eq/on", ("i", 1)))
        _iq(f"get_{stem}_eq", {stem: 1}, f"/{node}/1/eq/on", 1)
        binary(W, f"set_{stem}_eq_low_gain", {stem: 1, "gain_db": 3.0}, osc(f"/{node}/1/eq/lg", ("f", 3.0)))
        _fq(f"get_{stem}_eq_low_gain", {stem: 1}, f"/{node}/1/eq/lg", "3.0", 0.6, 3.0)
        binary(W, f"set_{stem}_eq_band_gain", {stem: 1, "band": bands, "gain_db": -4.5},
               osc(f"/{node}/1/eq/{bands}g", ("f", -4.5)))
        _fq(f"get_{stem}_eq_band_gain", {stem: 1, "band": 2}, f"/{node}/1/eq/2g", "-4.5", 0.35, -4.5)
        binary(W, f"set_{stem}_eq_high_gain", {stem: 1, "gain_db": -15.0}, osc(f"/{node}/1/eq/hg", ("f", -15.0)))
        _fq(f"get_{stem}_eq_high_gain", {stem: 1}, f"/{node}/1/eq/hg", "-15.0", 0.0, -15.0)
    if dyn:
        binary(W, f"set_{stem}_dynamics", {stem: 1, "enabled": False}, osc(f"/{node}/1/dyn/on", ("i", 0)))
        _iq(f"get_{stem}_dynamics", {stem: 1}, f"/{node}/1/dyn/on", 0)
    if stem != "dca":
        binary(W, f"set_{stem}_tags", {stem: 1, "tags": "#D1"}, osc(f"/{node}/1/tags", ("s", "#D1")))
        _sq(f"get_{stem}_tags", {stem: 1}, f"/{node}/1/tags", "#D1")

# Channel gate (p.48).
binary(W, "set_channel_gate", {"channel": 4, "enabled": True}, osc("/ch/4/gate/on", ("i", 1)))
_iq("get_channel_gate", {"channel": 4}, "/ch/4/gate/on", 1)

# Channel and aux input: gain and phantom of the connected source, trim,
# invert and the main connection (p.47, p.52).
for stem, node in [("channel", "ch"), ("aux", "aux")]:
    binary(W, f"set_{stem}_gain", {stem: 1, "gain_db": 30.0}, osc(f"/{node}/1/in/set/$g", ("f", 30.0)))
    _fq(f"get_{stem}_gain", {stem: 1}, f"/{node}/1/in/set/$g", "30.0", 0.5, 30.0)
    binary(W, f"set_{stem}_phantom", {stem: 1, "enabled": True}, osc(f"/{node}/1/in/set/$vph", ("i", 1)))
    _iq(f"get_{stem}_phantom", {stem: 1}, f"/{node}/1/in/set/$vph", 1)
    binary(W, f"set_{stem}_trim", {stem: 1, "trim_db": -6.0}, osc(f"/{node}/1/in/set/trim", ("f", -6.0)))
    _fq(f"get_{stem}_trim", {stem: 1}, f"/{node}/1/in/set/trim", "-6.0", 0.3333, -6.0)
    binary(W, f"set_{stem}_invert", {stem: 1, "inverted": True}, osc(f"/{node}/1/in/set/inv", ("i", 1)))
    _iq(f"get_{stem}_invert", {stem: 1}, f"/{node}/1/in/set/inv", 1)
    binary(W, f"set_{stem}_source", {stem: 2, "group": "LCL", "input": 7},
           [osc(f"/{node}/2/in/conn/grp", ("s", "LCL")), osc(f"/{node}/2/in/conn/in", ("i", 7))])
    _sq(f"get_{stem}_source_group", {stem: 2}, f"/{node}/2/in/conn/grp", "A")
    _iq(f"get_{stem}_source_input", {stem: 2}, f"/{node}/2/in/conn/in", 7)

# Sends: /<strip>/N/send/B (bus), /send/MX<x> (matrix), /main/M (main),
# each with on (0/1) and lvl (dB) (p.50-51, p.53-54, p.57, p.61).
for stem, node in [("channel", "ch"), ("aux", "aux"), ("bus", "bus")]:
    dest = "to_bus" if stem == "bus" else "bus"
    binary(W, f"set_{stem}_send_on", {stem: 3, dest: 16, "enabled": True}, osc(f"/{node}/3/send/16/on", ("i", 1)))
    _iq(f"get_{stem}_send_on", {stem: 3, dest: 16}, f"/{node}/3/send/16/on", 1)
    binary(W, f"set_{stem}_send_level", {stem: 3, dest: 2, "level_db": -10.0},
           osc(f"/{node}/3/send/2/lvl", ("f", -10.0)))
    _fq(f"get_{stem}_send_level", {stem: 3, dest: 2}, f"/{node}/3/send/2/lvl", "-oo", 0.0, -144.0)
    binary(W, f"set_{stem}_main_send_on", {stem: 1, "main": 4, "enabled": False}, osc(f"/{node}/1/main/4/on", ("i", 0)))
    _iq(f"get_{stem}_main_send_on", {stem: 1, "main": 1}, f"/{node}/1/main/1/on", 1)
    binary(W, f"set_{stem}_main_send_level", {stem: 1, "main": 1, "level_db": 0.0},
           osc(f"/{node}/1/main/1/lvl", ("f", 0.0)))
    _fq(f"get_{stem}_main_send_level", {stem: 1, "main": 1}, f"/{node}/1/main/1/lvl", "0.0", 0.75, 0.0)
for stem, node in [("channel", "ch"), ("aux", "aux"), ("bus", "bus"), ("main", "main")]:
    binary(W, f"set_{stem}_matrix_send_on", {stem: 1, "matrix": 8, "enabled": True},
           osc(f"/{node}/1/send/MX8/on", ("i", 1)))
    _iq(f"get_{stem}_matrix_send_on", {stem: 1, "matrix": 8}, f"/{node}/1/send/MX8/on", 0)
    binary(W, f"set_{stem}_matrix_send_level", {stem: 1, "matrix": 1, "level_db": 5.0},
           osc(f"/{node}/1/send/MX1/lvl", ("f", 5.0)))
    _fq(f"get_{stem}_matrix_send_level", {stem: 1, "matrix": 1}, f"/{node}/1/send/MX1/lvl", "5.0", 0.875, 5.0)

# Mute groups (p.65).
binary(W, "mute_group", {"group": 8, "muted": False}, osc("/mgrp/8/mute", ("i", 0)))
_iq("get_mute_group", {"group": 8}, "/mgrp/8/mute", 0)
binary(W, "toggle_mute_group", {"group": 1}, osc("/mgrp/1/mute", ("i", -1)))
binary(W, "set_mute_group_name", {"group": 2, "name": "Band"}, osc("/mgrp/2/name", ("s", "Band")))
_sq("get_mute_group_name", {"group": 2}, "/mgrp/2/name", "Band")

# Scenes: /$ctl/lib/$actionidx (int) then /$ctl/lib/$action (string) (p.88).
binary(W, "recall_scene", {"scene": 5},
       [osc("/$ctl/lib/$actionidx", ("i", 5)), osc("/$ctl/lib/$action", ("s", "GO"))])
binary(W, "recall_scene_by_tag", {"tag": 12},
       [osc("/$ctl/lib/$actionidx", ("i", 12)), osc("/$ctl/lib/$action", ("s", "GOTAG"))])
binary(W, "select_next_scene", {},
       [osc("/$ctl/lib/$actionidx", ("i", 0)), osc("/$ctl/lib/$action", ("s", "NEXT"))])
binary(W, "select_previous_scene", {},
       [osc("/$ctl/lib/$actionidx", ("i", 0)), osc("/$ctl/lib/$action", ("s", "PREV"))])
binary(W, "go_scene", {}, osc("/$ctl/lib/$action", ("s", "GO")))
binary(W, "go_next_scene", {}, osc("/$ctl/lib/$action", ("s", "GONEXT")))
binary(W, "go_previous_scene", {}, osc("/$ctl/lib/$action", ("s", "GOPREV")))
_iq("get_active_scene", {}, "/$ctl/lib/$actidx", 3)
_sq("get_active_scene_name", {}, "/$ctl/lib/$active", "I:SHOW2/scene_1.snap")
_sq("get_active_show", {}, "/$ctl/lib/$actshow", "I:SHOW2")
_iq("get_active_scene_tag", {}, "/$ctl/lib/$activeid", 0)

# Talkback: /cfg/talk/A|B/$on, mode, B1..B16, MX1..MX8, M1..M4 (p.36-38).
binary(W, "set_talkback", {"talkback": "A", "enabled": True}, osc("/cfg/talk/A/$on", ("i", 1)))
_iq("get_talkback", {"talkback": "B"}, "/cfg/talk/B/$on", 0)
binary(W, "set_talkback_mode", {"talkback": "B", "mode": "LATCH"}, osc("/cfg/talk/B/mode", ("s", "LATCH")))
_sq("get_talkback_mode", {"talkback": "A"}, "/cfg/talk/A/mode", "PUSH")
binary(W, "set_talkback_bus_assign", {"talkback": "A", "bus": 12, "assigned": True}, osc("/cfg/talk/A/B12", ("i", 1)))
_iq("get_talkback_bus_assign", {"talkback": "A", "bus": 12}, "/cfg/talk/A/B12", 1)
binary(W, "set_talkback_matrix_assign", {"talkback": "B", "matrix": 3, "assigned": False}, osc("/cfg/talk/B/MX3", ("i", 0)))
_iq("get_talkback_matrix_assign", {"talkback": "B", "matrix": 3}, "/cfg/talk/B/MX3", 0)
binary(W, "set_talkback_main_assign", {"talkback": "A", "main": 1, "assigned": True}, osc("/cfg/talk/A/M1", ("i", 1)))
_iq("get_talkback_main_assign", {"talkback": "A", "main": 1}, "/cfg/talk/A/M1", 1)

# Monitoring and solo: /cfg/mon/1|2/$lvl and src, /cfg/solo/$dim and $mono,
# /$stat/solo (p.34-35, p.32).
binary(W, "set_monitor_level", {"monitor": 2, "level_db": -20.0}, osc("/cfg/mon/2/$lvl", ("f", -20.0)))
_fq("get_monitor_level", {"monitor": 1}, "/cfg/mon/1/$lvl", "-20.0", 0.45, -20.0)
binary(W, "set_monitor_source", {"monitor": 1, "source": "MAIN.1"}, osc("/cfg/mon/1/src", ("s", "MAIN.1")))
_sq("get_monitor_source", {"monitor": 1}, "/cfg/mon/1/src", "BUS.3")
binary(W, "set_solo_dim", {"enabled": True}, osc("/cfg/solo/$dim", ("i", 1)))
_iq("get_solo_dim", {}, "/cfg/solo/$dim", 1)
binary(W, "set_solo_mono", {"enabled": False}, osc("/cfg/solo/$mono", ("i", 0)))
_iq("get_solo_mono", {}, "/cfg/solo/$mono", 0)
_iq("get_solo_active", {}, "/$stat/solo", 1)

# Console identity: /? (p.20), /$syscfg (p.39), /$ctl/OSC/ronly (p.88).
_sq("get_console_info", {}, "/?", "WING,192.168.1.71,PGM,ngc-full,NO_SERIAL,1.07.2-40-g1b1b292b:develop")
binary(W, "set_console_name", {"name": "FOH"}, osc("/$syscfg/consolename", ("s", "FOH")))
_sq("get_console_name", {}, "/$syscfg/consolename", "FOH")
_sq("get_firmware", {}, "/$syscfg/$firmware", "3.0.6-27")
_sq("get_serial", {}, "/$syscfg/$serial", "NO_SERIAL")
_sq("get_console_model", {}, "/$syscfg/$cnsmdl", "ngc-full")
_iq("get_osc_read_only", {}, "/$ctl/OSC/ronly", 0)

# ── Telemetry ─────────────────────────────────────────────────────────────
# /*s on connecting, then the first of the one-at-a-time queries for current
# values (channel 1's fader). Pushed changes after /*s are the same triplets
# a query returns (p.30): ,sff for floats, ,sfi for ints, ,s for strings.
telemetry(W, "channel-fader", expect_connect_wire_hex=[hexs(osc("/*s")), hexs(osc("/ch/1/fdr"))],
          inbound_hex=hexs(osc("/ch/7/fdr", ("s", "-2.0"), ("f", 0.7), ("f", -2.0))),
          expect_state={"channels": {"7": {"fader": -2.0}},
                        "params": {"ch/7/fdr": {"text": "-2.0", "raw": _f32(0.7), "float": -2.0}}})


# Every reported parameter is also kept under params, keyed by its address
# without the leading / (the generic rules): text always, raw and float or
# int for a number.
def _tf(name, address, text, raw, v, state):
    state = {**state, "params": {address[1:]: {"text": text, "raw": _f32(raw), "float": v}}}
    telemetry(W, name, inbound_hex=hexs(osc(address, ("s", text), ("f", raw), ("f", v))), expect_state=state)


def _ti(name, address, v, state):
    state = {**state, "params": {address[1:]: {"text": str(v), "raw": float(v), "int": v}}}
    telemetry(W, name, inbound_hex=hexs(osc(address, ("s", str(v)), ("f", float(v)), ("i", v))), expect_state=state)


def _ts(name, address, v, state):
    state = {**state, "params": {address[1:]: {"text": v}}}
    telemetry(W, name, inbound_hex=hexs(osc(address, ("s", v))), expect_state=state)


_PLURAL = {"ch": "channels", "aux": "auxes", "bus": "buses", "main": "mains", "mtx": "matrices", "dca": "dcas"}
for stem, node, last, nlen, bands, pan, dyn in _STRIPS:
    P = _PLURAL[node]
    k = str(last)
    if node != "ch":
        _tf(f"{node}-fader", f"/{node}/{last}/fdr", "-oo", 0.0, -144.0, {P: {k: {"fader": -144.0}}})
    _ti(f"{node}-mute", f"/{node}/{last}/mute", 1, {P: {k: {"mute": True}}})
    _ts(f"{node}-name", f"/{node}/{last}/name", "Kick", {P: {k: {"name": "Kick"}}})
    _ti(f"{node}-color", f"/{node}/{last}/col", 7, {P: {k: {"color": 7}}})
    _ti(f"{node}-solo", f"/{node}/{last}/$solo", 0, {P: {k: {"solo": False}}})
    if pan:
        _tf(f"{node}-pan", f"/{node}/{last}/pan", "100", 1.0, 100.0, {P: {k: {"pan": 100.0}}})
    if bands:
        _ti(f"{node}-eq", f"/{node}/{last}/eq/on", 1, {P: {k: {"eq": {"on": True}}}})
        _tf(f"{node}-eq-low-gain", f"/{node}/{last}/eq/lg", "6.0", 0.7, 6.0, {P: {k: {"eq": {"low_gain": 6.0}}}})
        _tf(f"{node}-eq-band-gain", f"/{node}/{last}/eq/{bands}g", "-3.0", 0.4, -3.0,
            {P: {k: {"eq": {"bands": {str(bands): {"gain": -3.0}}}}}})
        _tf(f"{node}-eq-high-gain", f"/{node}/{last}/eq/hg", "1.5", 0.55, 1.5, {P: {k: {"eq": {"high_gain": 1.5}}}})
    if dyn:
        _ti(f"{node}-dynamics", f"/{node}/{last}/dyn/on", 1, {P: {k: {"dynamics": {"on": True}}}})
    if node != "dca":
        _ts(f"{node}-tags", f"/{node}/{last}/tags", "#D2", {P: {k: {"tags": "#D2"}}})

_ti("ch-gate", "/ch/9/gate/on", 1, {"channels": {"9": {"gate": {"on": True}}}})
for node, P in [("ch", "channels"), ("aux", "auxes")]:
    _tf(f"{node}-gain", f"/{node}/3/in/set/$g", "24.5", 0.5, 24.5, {P: {"3": {"input": {"gain": 24.5}}}})
    _ti(f"{node}-phantom", f"/{node}/3/in/set/$vph", 1, {P: {"3": {"input": {"phantom": True}}}})
    _tf(f"{node}-trim", f"/{node}/3/in/set/trim", "-1.5", 0.4583, -1.5, {P: {"3": {"input": {"trim": -1.5}}}})
    _ti(f"{node}-invert", f"/{node}/3/in/set/inv", 0, {P: {"3": {"input": {"invert": False}}}})
    _ts(f"{node}-source-group", f"/{node}/3/in/conn/grp", "USB", {P: {"3": {"input": {"source_group": "USB"}}}})
    _ti(f"{node}-source-input", f"/{node}/3/in/conn/in", 48, {P: {"3": {"input": {"source_input": 48}}}})
for node, P in [("ch", "channels"), ("aux", "auxes"), ("bus", "buses")]:
    _ti(f"{node}-send-on", f"/{node}/2/send/5/on", 1, {P: {"2": {"sends": {"5": {"on": True}}}}})
    _tf(f"{node}-send-level", f"/{node}/2/send/5/lvl", "-10.0", 0.5, -10.0, {P: {"2": {"sends": {"5": {"level": -10.0}}}}})
    _ti(f"{node}-main-send-on", f"/{node}/2/main/3/on", 0, {P: {"2": {"main_sends": {"3": {"on": False}}}}})
    _tf(f"{node}-main-send-level", f"/{node}/2/main/3/lvl", "0.0", 0.75, 0.0,
        {P: {"2": {"main_sends": {"3": {"level": 0.0}}}}})
for node, P in [("ch", "channels"), ("aux", "auxes"), ("bus", "buses"), ("main", "mains")]:
    _ti(f"{node}-matrix-send-on", f"/{node}/1/send/MX6/on", 1, {P: {"1": {"matrix_sends": {"6": {"on": True}}}}})
    _tf(f"{node}-matrix-send-level", f"/{node}/1/send/MX6/lvl", "-oo", 0.0, -144.0,
        {P: {"1": {"matrix_sends": {"6": {"level": -144.0}}}}})

_ti("mgrp-mute", "/mgrp/3/mute", 1, {"mute_groups": {"3": {"mute": True}}})
_ts("mgrp-name", "/mgrp/3/name", "Drums", {"mute_groups": {"3": {"name": "Drums"}}})
_ti("scene-index", "/$ctl/lib/$actidx", 4, {"scene": {"index": 4}})
_ts("scene-name", "/$ctl/lib/$active", "I:SHOW2/scene_1.snap", {"scene": {"name": "I:SHOW2/scene_1.snap"}})
_ts("scene-show", "/$ctl/lib/$actshow", "I:SHOW2", {"scene": {"show": "I:SHOW2"}})
_ti("scene-tag", "/$ctl/lib/$activeid", 12, {"scene": {"tag": 12}})
for t in ("A", "B"):
    lt = t.lower()
    _ti(f"talkback-{lt}-on", f"/cfg/talk/{t}/$on", 1, {"talkback": {lt: {"on": True}}})
    _ts(f"talkback-{lt}-mode", f"/cfg/talk/{t}/mode", "AUTO", {"talkback": {lt: {"mode": "AUTO"}}})
    _ti(f"talkback-{lt}-bus", f"/cfg/talk/{t}/B16", 1, {"talkback": {lt: {"buses": {"16": True}}}})
    _ti(f"talkback-{lt}-matrix", f"/cfg/talk/{t}/MX2", 0, {"talkback": {lt: {"matrices": {"2": False}}}})
    _ti(f"talkback-{lt}-main", f"/cfg/talk/{t}/M4", 1, {"talkback": {lt: {"mains": {"4": True}}}})
_tf("monitor-level", "/cfg/mon/2/$lvl", "-12.0", 0.55, -12.0, {"monitors": {"2": {"level": -12.0}}})
_ts("monitor-source", "/cfg/mon/1/src", "MTX.8", {"monitors": {"1": {"source": "MTX.8"}}})
_ti("solo-dim", "/cfg/solo/$dim", 1, {"solo": {"dim": True}})
_ti("solo-mono", "/cfg/solo/$mono", 0, {"solo": {"mono": False}})
_ti("solo-active", "/$stat/solo", 1, {"solo": {"active": True}})
_ts("console-name", "/$syscfg/consolename", "FOH", {"console": {"name": "FOH"}})
_ts("console-firmware", "/$syscfg/$firmware", "3.0.6-27", {"console": {"firmware": "3.0.6-27"}})
_ts("console-serial", "/$syscfg/$serial", "S123", {"console": {"serial": "S123"}})
_ts("console-model", "/$syscfg/$cnsmdl", "wing-rack", {"console": {"model": "wing-rack"}})
_ti("osc-read-only", "/$ctl/OSC/ronly", 1, {"console": {"osc_read_only": True}})

# Any parameter by its address (p.21-24): a set carries one argument of the
# type chosen, a get is the bare address, a node set is the node's address
# with one string, answered on the address followed by * with OK.
binary(W, "set_parameter_float", {"path": "ch/2/fdr", "value": -3.0}, osc("/ch/2/fdr", ("f", -3.0)),
       expect_result={"ok": {"kind": "unverified"}})
binary(W, "set_parameter_int", {"path": "$ctl/user/1/1/enc/mode", "value": 6},
       osc("/$ctl/user/1/1/enc/mode", ("i", 6)))
binary(W, "set_parameter_string", {"path": "$ctl/user/1/1/enc/mode", "value": "FX"},
       osc("/$ctl/user/1/1/enc/mode", ("s", "FX")))
_q("get_parameter", {"path": "ch/1/mute"}, "/ch/1/mute", [("s", "1"), ("f", 1.0), ("i", 1)], "1")
_q("get_parameter_value", {"path": "ch/2/fdr"}, "/ch/2/fdr", [("s", "-3.0"), ("f", 0.675), ("f", -3.0)], -3.0)
_q("get_parameter_raw", {"path": "ch/2/fdr"}, "/ch/2/fdr", [("s", "-2.0"), ("f", 0.5), ("f", -2.0)], 0.5)
binary(W, "get_node", {"node": "$ctl/user/1/1/enc"}, osc("/$ctl/user/1/1/enc"),
       device_reply_hex=hexs(osc("/$ctl/user/1/1/enc", ("s", "mode"), ("s", "name"), ("s", "$fname"))),
       expect_result={"ok": {"kind": "value", "value": '["mode","name","$fname"]'}})
# Node queries ,s ? / # / * (p.24-25): the reply is one string on the node's
# address; the description here is the document's /fx/1 example, shortened.
_FXDESC = "  mdl            list [NONE, EXT, HALL]\n  fxmix          lin [0 .. 100 %], 101 steps\n"
binary(W, "describe_node", {"node": "fx/1"}, osc("/fx/1", ("s", "?")),
       device_reply_hex=hexs(osc("/fx/1", ("s", _FXDESC))),
       expect_result={"ok": {"kind": "value", "value": _FXDESC}})
binary(W, "describe_node_values", {"node": "fx/1"}, osc("/fx/1", ("s", "#")))
binary(W, "dump_node", {"node": "fx/1"}, osc("/fx/1", ("s", "*")),
       device_reply_hex=hexs(osc("/fx/1", ("s", "mdl=NONE,fxmix=100,"))),
       expect_result={"ok": {"kind": "value", "value": "mdl=NONE,fxmix=100,"}})
binary(W, "get_scene_list", {}, osc("/$ctl/lib", ("s", "?")))
binary(W, "get_usb_playlist_songs", {}, osc("/play", ("s", "?")))
binary(W, "get_wlive_session_list", {"card": 2}, osc("/cards/wlive/2/$stat", ("s", "?")))
binary(W, "set_node", {"node": "ch/1", "assignments": "fdr=4,mute=1"}, osc("/ch/1", ("s", "fdr=4,mute=1")),
       device_reply_hex=hexs(osc("/ch/1*", ("s", "OK"))), expect_result={"ok": {"kind": "ack"}})
binary(W, "set_node", {"assignments": "/ch.1.fdr=-1,mute=0,.2.fdr=0,mute=1"},
       osc("/", ("s", "/ch.1.fdr=-1,mute=0,.2.fdr=0,mute=1")),
       device_reply_hex=hexs(osc("/*", ("s", "OK"))), expect_result={"ok": {"kind": "ack"}}, file="set_node-root")
_ts("param-enum", "/$ctl/user/1/1/enc/mode", "FX", {"custom_controls": {"1": {"1": {"enc": {"function": "FX"}}}}})
_tf("param-float", "/cfg/mon/1/dim", "-20.0", 0.5, -20.0, {"monitors": {"1": {"dim": -20.0}}})
_ti("param-int", "/ch/40/$muteovr", 1, {})

# ── EQ model, mix, frequencies, Qs, types and tilt; pre-send EQ (p.48-63) ─
# Floats in their own unit as ,f; enumerations as ,s (p.22).
for stem, node, n, nb, model, ltype in [("channel", "ch", 40, 4, "MACH4", "SHV"), ("aux", "aux", 8, 4, "PULSAR", "CUT"),
                                        ("bus", "bus", 16, 6, "PIA", "LR48"), ("main", "main", 4, 6, "SOUL", "BW12"),
                                        ("matrix", "mtx", 8, 6, "STD", "PEQ")]:
    P = _PLURAL[node]
    binary(W, f"set_{stem}_eq_model", {stem: n, "model": model}, osc(f"/{node}/{n}/eq/mdl", ("s", model)))
    binary(W, f"set_{stem}_eq_mix", {stem: 1, "mix": 100.0}, osc(f"/{node}/1/eq/mix", ("f", 100.0)))
    binary(W, f"set_{stem}_eq_low_frequency", {stem: 1, "frequency_hz": 80.0}, osc(f"/{node}/1/eq/lf", ("f", 80.0)))
    binary(W, f"set_{stem}_eq_low_q", {stem: 1, "q": 0.44}, osc(f"/{node}/1/eq/lq", ("f", 0.44)))
    binary(W, f"set_{stem}_eq_low_type", {stem: 1, "type": ltype}, osc(f"/{node}/1/eq/leq", ("s", ltype)))
    binary(W, f"set_{stem}_eq_high_frequency", {stem: 2, "frequency_hz": 12000.0}, osc(f"/{node}/2/eq/hf", ("f", 12000.0)))
    binary(W, f"set_{stem}_eq_high_q", {stem: 2, "q": 10.0}, osc(f"/{node}/2/eq/hq", ("f", 10.0)))
    binary(W, f"set_{stem}_eq_high_type", {stem: 2, "type": "PEQ"}, osc(f"/{node}/2/eq/heq", ("s", "PEQ")))
    binary(W, f"set_{stem}_eq_band_frequency", {stem: 3, "band": nb, "frequency_hz": 1000.0},
           osc(f"/{node}/3/eq/{nb}f", ("f", 1000.0)))
    binary(W, f"set_{stem}_eq_band_q", {stem: 3, "band": 1, "q": 2.0}, osc(f"/{node}/3/eq/1q", ("f", 2.0)))
    _ts(f"{stem}-eq-model", f"/{node}/1/eq/mdl", "STD", {P: {"1": {"eq": {"model": "STD"}}}})
    _tf(f"{stem}-eq-mix", f"/{node}/1/eq/mix", "100", 0.8, 100.0, {P: {"1": {"eq": {"mix": 100.0}}}})
    _tf(f"{stem}-eq-low-frequency", f"/{node}/1/eq/lf", "80.2", 0.3, 80.0, {P: {"1": {"eq": {"low_frequency": 80.0}}}})
    _tf(f"{stem}-eq-low-q", f"/{node}/1/eq/lq", "1.00", 0.25, 1.0, {P: {"1": {"eq": {"low_q": 1.0}}}})
    _ts(f"{stem}-eq-low-type", f"/{node}/1/eq/leq", "SHV", {P: {"1": {"eq": {"low_type": "SHV"}}}})
    _tf(f"{stem}-eq-high-frequency", f"/{node}/1/eq/hf", "12k00", 0.75, 12000.0, {P: {"1": {"eq": {"high_frequency": 12000.0}}}})
    _tf(f"{stem}-eq-high-q", f"/{node}/1/eq/hq", "1.00", 0.25, 1.0, {P: {"1": {"eq": {"high_q": 1.0}}}})
    _ts(f"{stem}-eq-high-type", f"/{node}/1/eq/heq", "PEQ", {P: {"1": {"eq": {"high_type": "PEQ"}}}})
    _tf(f"{stem}-eq-band-frequency", f"/{node}/2/eq/3f", "1k50", 0.5, 1500.0, {P: {"2": {"eq": {"bands": {"3": {"frequency": 1500.0}}}}}})
    _tf(f"{stem}-eq-band-q", f"/{node}/2/eq/3q", "2.00", 0.5, 2.0, {P: {"2": {"eq": {"bands": {"3": {"q": 2.0}}}}}})
    if nb == 6:
        binary(W, f"set_{stem}_eq_tilt", {stem: n, "tilt_db": -6.0}, osc(f"/{node}/{n}/eq/tilt", ("f", -6.0)))
        _tf(f"{stem}-eq-tilt", f"/{node}/1/eq/tilt", "1.50", 0.625, 1.5, {P: {"1": {"eq": {"tilt": 1.5}}}})
binary(W, "set_channel_peq", {"channel": 40, "enabled": True}, osc("/ch/40/peq/on", ("i", 1)))
binary(W, "set_channel_peq_band_gain", {"channel": 1, "band": 3, "gain_db": -15.0}, osc("/ch/1/peq/3g", ("f", -15.0)))
binary(W, "set_channel_peq_band_frequency", {"channel": 1, "band": 1, "frequency_hz": 100.0}, osc("/ch/1/peq/1f", ("f", 100.0)))
binary(W, "set_channel_peq_band_q", {"channel": 1, "band": 2, "q": 1.0}, osc("/ch/1/peq/2q", ("f", 1.0)))
_ti("channel-peq-on", "/ch/5/peq/on", 1, {"channels": {"5": {"peq": {"on": True}}}})
_tf("channel-peq-gain", "/ch/5/peq/1g", "3.0", 0.6, 3.0, {"channels": {"5": {"peq": {"bands": {"1": {"gain": 3.0}}}}}})
_tf("channel-peq-frequency", "/ch/5/peq/2f", "999", 0.5, 999.0, {"channels": {"5": {"peq": {"bands": {"2": {"frequency": 999.0}}}}}})
_tf("channel-peq-q", "/ch/5/peq/3q", "1.00", 0.25, 1.0, {"channels": {"5": {"peq": {"bands": {"3": {"q": 1.0}}}}}})

# ── Dynamics and gate (p.48-64) ───────────────────────────────────────────
for stem, node, n in [("channel", "ch", 40), ("bus", "bus", 16), ("main", "main", 4), ("matrix", "mtx", 8)]:
    P = _PLURAL[node]
    ch = node == "ch"
    for cmd, inp, leaf, wire in [
            ("dyn_model", {"model": "CMB24" if ch else "CMB"}, "dyn/mdl", ("s", "CMB24" if ch else "CMB")),
            ("dyn_mix", {"mix": 50.0}, "dyn/mix", ("f", 50.0)),
            ("dyn_gain", {"gain_db": 12.0}, "dyn/gain", ("f", 12.0)),
            ("dyn_threshold", {"threshold_db": -60.0}, "dyn/thr", ("f", -60.0)),
            ("dyn_ratio", {"ratio": "4.0"} if ch else {"ratio": 100.0}, "dyn/ratio", ("s", "4.0") if ch else ("f", 100.0)),
            ("dyn_knee", {"knee": 5}, "dyn/knee", ("i", 5)),
            ("dyn_detector", {"detector": "RMS"}, "dyn/det", ("s", "RMS")),
            ("dyn_attack", {"attack_ms": 120.0}, "dyn/att", ("f", 120.0)),
            ("dyn_hold", {"hold_ms": 1.0}, "dyn/hld", ("f", 1.0)),
            ("dyn_release", {"release_ms": 4000.0}, "dyn/rel", ("f", 4000.0)),
            ("dyn_envelope", {"envelope": "LOG"}, "dyn/env", ("s", "LOG")),
            ("dyn_auto", {"enabled": True}, "dyn/auto", ("i", 1)),
            ("dyn_crossover_depth", {"depth_db": 6.0}, "dynxo/depth", ("f", 6.0)),
            ("dyn_crossover_type", {"type": "HI12"}, "dynxo/type", ("s", "HI12")),
            ("dyn_crossover_frequency", {"frequency_hz": 1000.0}, "dynxo/f", ("f", 1000.0)),
            ("dyn_sidechain_type", {"type": "BP"}, "dynsc/type", ("s", "BP")),
            ("dyn_sidechain_frequency", {"frequency_hz": 20000.0}, "dynsc/f", ("f", 20000.0)),
            ("dyn_sidechain_q", {"q": 2.0}, "dynsc/q", ("f", 2.0)),
            ("dyn_sidechain_source", {"source": "CH.40" if ch else "AUX.8"}, "dynsc/src", ("s", "CH.40" if ch else "AUX.8")),
            ("dyn_sidechain_tap", {"tap": "POST" if ch else "INS2"}, "dynsc/tap", ("s", "POST" if ch else "INS2"))]:
        binary(W, f"set_{stem}_{cmd}", {stem: n, **inp}, osc(f"/{node}/{n}/{leaf}", wire))
    _ts(f"{stem}-dyn-model", f"/{node}/1/dyn/mdl", "COMP", {P: {"1": {"dynamics": {"model": "COMP"}}}})
    for name, leaf, txt, raw, v, key in [("dyn-mix", "dyn/mix", "100", 1.0, 100.0, ["mix"]),
                                          ("dyn-gain", "dyn/gain", "0.0", 0.3333, 0.0, ["gain"]),
                                          ("dyn-threshold", "dyn/thr", "-10.0", 0.8333, -10.0, ["threshold"]),
                                          ("dyn-attack", "dyn/att", "50", 0.4167, 50.0, ["attack"]),
                                          ("dyn-hold", "dyn/hld", "20", 0.1, 20.0, ["hold"]),
                                          ("dyn-release", "dyn/rel", "153", 0.5, 153.0, ["release"]),
                                          ("dyn-crossover-depth", "dynxo/depth", "6.0", 0.3, 6.0, ["crossover", "depth"]),
                                          ("dyn-crossover-frequency", "dynxo/f", "1k0", 0.5, 1000.0, ["crossover", "frequency"]),
                                          ("dyn-sidechain-frequency", "dynsc/f", "1k0", 0.5, 1000.0, ["sidechain", "frequency"]),
                                          ("dyn-sidechain-q", "dynsc/q", "2.00", 0.4, 2.0, ["sidechain", "q"])]:
        st = {key[-1]: v}
        if len(key) == 2:
            st = {key[0]: st}
        _tf(f"{stem}-{name}", f"/{node}/1/{leaf}", txt, raw, v, {P: {"1": {"dynamics": st}}})
    if ch:
        _ts(f"{stem}-dyn-ratio", "/ch/1/dyn/ratio", "3.0", {P: {"1": {"dynamics": {"ratio": "3.0"}}}})
    else:
        _tf(f"{stem}-dyn-ratio", f"/{node}/1/dyn/ratio", "3.0", 0.3, 3.0, {P: {"1": {"dynamics": {"ratio_value": 3.0}}}})
    _ti(f"{stem}-dyn-knee", f"/{node}/1/dyn/knee", 3, {P: {"1": {"dynamics": {"knee": 3}}}})
    _ts(f"{stem}-dyn-detector", f"/{node}/1/dyn/det", "RMS", {P: {"1": {"dynamics": {"detector": "RMS"}}}})
    _ts(f"{stem}-dyn-envelope", f"/{node}/1/dyn/env", "LOG", {P: {"1": {"dynamics": {"envelope": "LOG"}}}})
    _ti(f"{stem}-dyn-auto", f"/{node}/1/dyn/auto", 1, {P: {"1": {"dynamics": {"auto": True}}}})
    _ts(f"{stem}-dyn-crossover-type", f"/{node}/1/dynxo/type", "OFF", {P: {"1": {"dynamics": {"crossover": {"type": "OFF"}}}}})
    _ts(f"{stem}-dyn-sidechain-type", f"/{node}/1/dynsc/type", "LP12", {P: {"1": {"dynamics": {"sidechain": {"type": "LP12"}}}}})
    _ts(f"{stem}-dyn-sidechain-source", f"/{node}/1/dynsc/src", "SELF", {P: {"1": {"dynamics": {"sidechain": {"source": "SELF"}}}}})
    tap = "IN" if ch else "BUS"
    _ts(f"{stem}-dyn-sidechain-tap", f"/{node}/1/dynsc/tap", tap, {P: {"1": {"dynamics": {"sidechain": {"tap": tap}}}}})
for cmd, inp, leaf, wire in [("dyn_threshold", {"threshold_db": 12.0}, "dyn/thr", ("f", 12.0)),
                             ("dyn_depth", {"depth_db": 20.0}, "dyn/depth", ("f", 20.0)),
                             ("dyn_fast", {"enabled": True}, "dyn/fast", ("i", 1)),
                             ("dyn_peak", {"enabled": False}, "dyn/peak", ("i", 0)),
                             ("dyn_input_gain", {"gain": 100.0}, "dyn/ingain", ("f", 100.0)),
                             ("dyn_peak_reduction", {"peak": 0.0}, "dyn/cpeak", ("f", 0.0)),
                             ("dyn_mode", {"mode": "LIM"}, "dyn/cmode", ("s", "LIM"))]:
    binary(W, f"set_aux_{cmd}", {"aux": 8, **inp}, osc(f"/aux/8/{leaf}", wire))
_tf("aux-dyn-threshold", "/aux/2/dyn/thr", "-12.0", 0.5, -12.0, {"auxes": {"2": {"dynamics": {"threshold": -12.0}}}})
_tf("aux-dyn-depth", "/aux/2/dyn/depth", "6.0", 0.3, 6.0, {"auxes": {"2": {"dynamics": {"depth": 6.0}}}})
_ti("aux-dyn-fast", "/aux/2/dyn/fast", 1, {"auxes": {"2": {"dynamics": {"fast": True}}}})
_ti("aux-dyn-peak", "/aux/2/dyn/peak", 0, {"auxes": {"2": {"dynamics": {"peak": False}}}})
_tf("aux-dyn-input-gain", "/aux/2/dyn/ingain", "50", 0.5, 50.0, {"auxes": {"2": {"dynamics": {"input_gain": 50.0}}}})
_tf("aux-dyn-peak-reduction", "/aux/2/dyn/cpeak", "25", 0.25, 25.0, {"auxes": {"2": {"dynamics": {"peak_reduction": 25.0}}}})
_ts("aux-dyn-mode", "/aux/2/dyn/cmode", "COMP", {"auxes": {"2": {"dynamics": {"mode": "COMP"}}}})
for cmd, inp, leaf, wire in [("gate_model", {"model": "DUCK"}, "gate/mdl", ("s", "DUCK")),
                             ("gate_threshold", {"threshold_db": -80.0}, "gate/thr", ("f", -80.0)),
                             ("gate_range", {"range_db": 60.0}, "gate/range", ("f", 60.0)),
                             ("gate_attack", {"attack_ms": 0.0}, "gate/att", ("f", 0.0)),
                             ("gate_hold", {"hold_ms": 200.0}, "gate/hld", ("f", 200.0)),
                             ("gate_release", {"release_ms": 4.0}, "gate/rel", ("f", 4.0)),
                             ("gate_accent", {"accent": 100.0}, "gate/acc", ("f", 100.0)),
                             ("gate_ratio", {"ratio": "gate"}, "gate/ratio", ("s", "gate")),
                             ("gate_sidechain_type", {"type": "HP12"}, "gatesc/type", ("s", "HP12")),
                             ("gate_sidechain_frequency", {"frequency_hz": 20.0}, "gatesc/f", ("f", 20.0)),
                             ("gate_sidechain_q", {"q": 10.0}, "gatesc/q", ("f", 10.0)),
                             ("gate_sidechain_source", {"source": "CH.1"}, "gatesc/src", ("s", "CH.1")),
                             ("gate_sidechain_tap", {"tap": "PFL"}, "gatesc/tap", ("s", "PFL"))]:
    binary(W, f"set_channel_{cmd}", {"channel": 9, **inp}, osc(f"/ch/9/{leaf}", wire))
_ts("channel-gate-model", "/ch/9/gate/mdl", "GATE", {"channels": {"9": {"gate": {"model": "GATE"}}}})
for name, leaf, txt, raw, v, key in [("threshold", "thr", "-40.0", 0.5, -40.0, "threshold"), ("range", "range", "40.0", 0.65, 40.0, "range"),
                                      ("attack", "att", "10", 0.08, 10.0, "attack"), ("hold", "hld", "10", 0.05, 10.0, "hold"),
                                      ("release", "rel", "199", 0.5, 199.0, "release"), ("accent", "acc", "0", 0.0, 0.0, "accent")]:
    _tf(f"channel-gate-{name}", f"/ch/9/gate/{leaf}", txt, raw, v, {"channels": {"9": {"gate": {key: v}}}})
_ts("channel-gate-ratio", "/ch/9/gate/ratio", "1:3", {"channels": {"9": {"gate": {"ratio": "1:3"}}}})
_ts("channel-gate-sidechain-type", "/ch/9/gatesc/type", "OFF", {"channels": {"9": {"gate": {"sidechain": {"type": "OFF"}}}}})
_tf("channel-gate-sidechain-frequency", "/ch/9/gatesc/f", "1k0", 0.5, 1000.0, {"channels": {"9": {"gate": {"sidechain": {"frequency": 1000.0}}}}})
_tf("channel-gate-sidechain-q", "/ch/9/gatesc/q", "2.00", 0.4, 2.0, {"channels": {"9": {"gate": {"sidechain": {"q": 2.0}}}}})
_ts("channel-gate-sidechain-source", "/ch/9/gatesc/src", "SELF", {"channels": {"9": {"gate": {"sidechain": {"source": "SELF"}}}}})
_ts("channel-gate-sidechain-tap", "/ch/9/gatesc/tap", "IN", {"channels": {"9": {"gate": {"sidechain": {"tap": "IN"}}}}})

# ── Channel filters, inserts and effects slots (p.47-66) ──────────────────
for cmd, inp, leaf, wire in [("low_cut", {"enabled": True}, "flt/lc", ("i", 1)),
                             ("low_cut_frequency", {"frequency_hz": 100.0}, "flt/lcf", ("f", 100.0)),
                             ("low_cut_slope", {"slope": "24"}, "flt/lcs", ("s", "24")),
                             ("high_cut", {"enabled": False}, "flt/hc", ("i", 0)),
                             ("high_cut_frequency", {"frequency_hz": 10000.0}, "flt/hcf", ("f", 10000.0)),
                             ("high_cut_slope", {"slope": "6"}, "flt/hcs", ("s", "6")),
                             ("tool_filter", {"enabled": True}, "flt/tf", ("i", 1)),
                             ("filter_model", {"model": "AP2"}, "flt/mdl", ("s", "AP2")),
                             ("filter_tilt", {"tilt_db": 6.0}, "flt/tilt", ("f", 6.0))]:
    binary(W, f"set_channel_{cmd}", {"channel": 11, **inp}, osc(f"/ch/11/{leaf}", wire))
_ti("channel-low-cut", "/ch/11/flt/lc", 0, {"channels": {"11": {"filter": {"low_cut": False}}}})
_tf("channel-low-cut-frequency", "/ch/11/flt/lcf", "100.2", 0.25, 100.0, {"channels": {"11": {"filter": {"low_cut_frequency": 100.0}}}})
_ts("channel-low-cut-slope", "/ch/11/flt/lcs", "24", {"channels": {"11": {"filter": {"low_cut_slope": "24"}}}})
_ti("channel-high-cut", "/ch/11/flt/hc", 1, {"channels": {"11": {"filter": {"high_cut": True}}}})
_tf("channel-high-cut-frequency", "/ch/11/flt/hcf", "10k02", 0.9, 10000.0, {"channels": {"11": {"filter": {"high_cut_frequency": 10000.0}}}})
_ts("channel-high-cut-slope", "/ch/11/flt/hcs", "12", {"channels": {"11": {"filter": {"high_cut_slope": "12"}}}})
_ti("channel-tool-filter", "/ch/11/flt/tf", 0, {"channels": {"11": {"filter": {"tool": False}}}})
_ts("channel-filter-model", "/ch/11/flt/mdl", "TILT", {"channels": {"11": {"filter": {"model": "TILT"}}}})
_tf("channel-filter-tilt", "/ch/11/flt/tilt", "0.00", 0.5, 0.0, {"channels": {"11": {"filter": {"tilt": 0.0}}}})
for stem, node in [("channel", "ch"), ("aux", "aux"), ("bus", "bus"), ("main", "main"), ("matrix", "mtx")]:
    P = _PLURAL[node]
    binary(W, f"set_{stem}_insert1", {stem: 1, "enabled": True}, osc(f"/{node}/1/preins/on", ("i", 1)))
    binary(W, f"set_{stem}_insert1_slot", {stem: 1, "slot": "FX16"}, osc(f"/{node}/1/preins/ins", ("s", "FX16")))
    _ti(f"{stem}-insert1", f"/{node}/2/preins/on", 1, {P: {"2": {"inserts": {"pre": {"on": True}}}}})
    _ts(f"{stem}-insert1-slot", f"/{node}/2/preins/ins", "FX3", {P: {"2": {"inserts": {"pre": {"slot": "FX3"}}}}})
    _ts(f"{stem}-insert1-status", f"/{node}/2/preins/$stat", "OK", {P: {"2": {"inserts": {"pre": {"status": "OK"}}}}})
    if stem != "aux":
        binary(W, f"set_{stem}_insert2", {stem: 1, "enabled": False}, osc(f"/{node}/1/postins/on", ("i", 0)))
        binary(W, f"set_{stem}_insert2_slot", {stem: 1, "slot": "NONE"}, osc(f"/{node}/1/postins/ins", ("s", "NONE")))
        _ti(f"{stem}-insert2", f"/{node}/2/postins/on", 0, {P: {"2": {"inserts": {"post": {"on": False}}}}})
        _ts(f"{stem}-insert2-slot", f"/{node}/2/postins/ins", "FX1", {P: {"2": {"inserts": {"post": {"slot": "FX1"}}}}})
        _ts(f"{stem}-insert2-status", f"/{node}/2/postins/$stat", "N/A", {P: {"2": {"inserts": {"post": {"status": "N/A"}}}}})
binary(W, "set_channel_insert2_mode", {"channel": 1, "mode": "AUTO_X"}, osc("/ch/1/postins/mode", ("s", "AUTO_X")))
binary(W, "set_channel_insert2_weight", {"channel": 1, "weight_db": -12.0}, osc("/ch/1/postins/w", ("f", -12.0)))
_ts("channel-insert2-mode", "/ch/3/postins/mode", "FX", {"channels": {"3": {"inserts": {"post": {"mode": "FX"}}}}})
_tf("channel-insert2-weight", "/ch/3/postins/w", "0.0", 0.5, 0.0, {"channels": {"3": {"inserts": {"post": {"weight": 0.0}}}}})
# Effects: the document's own example, a PIA in slot 1 and its 125 Hz band (p.26).
binary(W, "set_fx_model", {"slot": 1, "model": "PIA"}, osc("/fx/1/mdl", ("s", "PIA")))
binary(W, "set_fx_mix", {"slot": 16, "mix": 100.0}, osc("/fx/16/fxmix", ("f", 100.0)))
binary(W, "set_fx_parameter", {"slot": 1, "parameter": 3, "value": 10.0}, osc("/fx/1/3", ("f", 10.0)))
binary(W, "set_fx_parameter_text", {"slot": 2, "parameter": 40, "value": "ON"}, osc("/fx/2/40", ("s", "ON")))
_ts("fx-model", "/fx/1/mdl", "NONE", {"fx": {"1": {"model": "NONE"}}})
_tf("fx-mix", "/fx/1/fxmix", "100", 1.0, 100.0, {"fx": {"1": {"mix": 100.0}}})
_tf("fx-parameter-float", "/fx/1/3", "10.0", 0.9233, 10.0, {"fx": {"1": {"parameters": {"3": {"text": "10.0", "value": 10.0}}}}})
_ti("fx-parameter-int", "/fx/4/7", 2, {"fx": {"4": {"parameters": {"7": {"text": "2", "value": 2.0}}}}})
_ts("fx-parameter-text", "/fx/4/8", "ON", {"fx": {"4": {"parameters": {"8": {"text": "ON"}}}}})
_ti("fx-source", "/fx/2/$esrc", 12, {"fx": {"2": {"source": 12}}})
_ts("fx-mode", "/fx/2/$emode", "ST", {"fx": {"2": {"mode": "ST"}}})
_ti("fx-assigned-strip", "/fx/2/$a_chn", 5, {"fx": {"2": {"assigned_strip": 5}}})
_ti("fx-assigned-position", "/fx/2/$a_pos", 1, {"fx": {"2": {"assigned_position": 1}}})

# ── Sends, delay and strip settings (p.47-65) ─────────────────────────────
for stem, node in [("channel", "ch"), ("aux", "aux")]:
    P = _PLURAL[node]
    for cs, dkey, x, xs, key in [("send", "bus", 16, "16", "sends"), ("matrix_send", "matrix", 8, "MX8", "matrix_sends")]:
        binary(W, f"set_{stem}_{cs}_pre_always", {stem: 1, dkey: x, "enabled": True}, osc(f"/{node}/1/send/{xs}/pon", ("i", 1)))
        binary(W, f"set_{stem}_{cs}_mode", {stem: 1, dkey: x, "mode": "GRP"}, osc(f"/{node}/1/send/{xs}/mode", ("s", "GRP")))
        binary(W, f"set_{stem}_{cs}_pan_link", {stem: 1, dkey: x, "enabled": False}, osc(f"/{node}/1/send/{xs}/plink", ("i", 0)))
        binary(W, f"set_{stem}_{cs}_pan", {stem: 1, dkey: x, "pan": -100.0}, osc(f"/{node}/1/send/{xs}/pan", ("f", -100.0)))
        y = "9" if dkey == "bus" else "MX1"
        _ti(f"{stem}-{cs.replace('_', '-')}-pre-always", f"/{node}/2/send/{y}/pon", 0, {P: {"2": {key: {y.replace('MX', ''): {"pre_always": False}}}}})
        _ts(f"{stem}-{cs.replace('_', '-')}-mode", f"/{node}/2/send/{y}/mode", "PRE", {P: {"2": {key: {y.replace('MX', ''): {"mode": "PRE"}}}}})
        _ti(f"{stem}-{cs.replace('_', '-')}-pan-link", f"/{node}/2/send/{y}/plink", 1, {P: {"2": {key: {y.replace('MX', ''): {"pan_link": True}}}}})
        _tf(f"{stem}-{cs.replace('_', '-')}-pan", f"/{node}/2/send/{y}/pan", "25", 0.625, 25.0, {P: {"2": {key: {y.replace('MX', ''): {"pan": 25.0}}}}})
for stem, node in [("channel", "ch"), ("aux", "aux"), ("bus", "bus")]:
    P = _PLURAL[node]
    binary(W, f"set_{stem}_main_send_pre", {stem: 1, "main": 4, "enabled": True}, osc(f"/{node}/1/main/4/pre", ("i", 1)))
    _ti(f"{stem}-main-send-pre", f"/{node}/3/main/1/pre", 0, {P: {"3": {"main_sends": {"1": {"pre": False}}}}})
binary(W, "set_bus_send_pre", {"bus": 2, "destination": 3, "enabled": True}, osc("/bus/2/send/3/pre", ("i", 1)))
_ti("bus-send-pre", "/bus/1/send/2/pre", 1, {"buses": {"1": {"sends": {"2": {"pre": True}}}}})
for stem, node in [("bus", "bus"), ("main", "main")]:
    P = _PLURAL[node]
    binary(W, f"set_{stem}_matrix_send_pre", {stem: 1, "matrix": 8, "enabled": False}, osc(f"/{node}/1/send/MX8/pre", ("i", 0)))
    _ti(f"{stem}-matrix-send-pre", f"/{node}/1/send/MX3/pre", 1, {P: {"1": {"matrix_sends": {"3": {"pre": True}}}}})
for stem, node, on, mode, val in [("channel", "ch", "in/set/dlyon", "in/set/dlymode", "in/set/dly"),
                                  ("aux", "aux", "in/set/dlyon", "in/set/dlymode", "in/set/dly"),
                                  ("bus", "bus", "dly/on", "dly/mode", "dly/dly"), ("main", "main", "dly/on", "dly/mode", "dly/dly"),
                                  ("matrix", "mtx", "dly/on", "dly/mode", "dly/dly")]:
    P = _PLURAL[node]
    binary(W, f"set_{stem}_delay", {stem: 1, "enabled": True}, osc(f"/{node}/1/{on}", ("i", 1)))
    binary(W, f"set_{stem}_delay_mode", {stem: 1, "mode": "MS"}, osc(f"/{node}/1/{mode}", ("s", "MS")))
    binary(W, f"set_{stem}_delay_time", {stem: 1, "delay": 500.0}, osc(f"/{node}/1/{val}", ("f", 500.0)))
    _ti(f"{stem}-delay", f"/{node}/1/{on}", 0, {P: {"1": {"delay": {"on": False}}}})
    _ts(f"{stem}-delay-mode", f"/{node}/1/{mode}", "M", {P: {"1": {"delay": {"mode": "M"}}}})
    _tf(f"{stem}-delay-time", f"/{node}/1/{val}", "0.1", 0.0, _f32(0.1), {P: {"1": {"delay": {"time": _f32(0.1)}}}})
for stem, node in [("channel", "ch"), ("aux", "aux"), ("bus", "bus"), ("main", "main"), ("matrix", "mtx"), ("dca", "dca")]:
    P = _PLURAL[node]
    binary(W, f"set_{stem}_icon", {stem: 1, "icon": 999}, osc(f"/{node}/1/icon", ("i", 999)))
    binary(W, f"set_{stem}_scribble_light", {stem: 1, "enabled": False}, osc(f"/{node}/1/led", ("i", 0)))
    binary(W, f"set_{stem}_monitor_bus", {stem: 1, "monitor": "A+B"}, osc(f"/{node}/1/mon", ("s", "A+B")))
    _ti(f"{stem}-icon", f"/{node}/1/icon", 12, {P: {"1": {"icon": 12}}})
    _ti(f"{stem}-scribble-light", f"/{node}/1/led", 1, {P: {"1": {"scribble_light": True}}})
    _ts(f"{stem}-monitor-bus", f"/{node}/1/mon", "A", {P: {"1": {"monitor_bus": "A"}}})
    if stem != "dca":
        binary(W, f"set_{stem}_width", {stem: 1, "width": -150.0}, osc(f"/{node}/1/wid", ("f", -150.0)))
        binary(W, f"set_{stem}_balance", {stem: 1, "balance_db": 9.0}, osc(f"/{node}/1/in/set/bal", ("f", 9.0)))
        _tf(f"{stem}-width", f"/{node}/1/wid", "100", 0.8333, 100.0, {P: {"1": {"width": 100.0}}})
        _tf(f"{stem}-balance", f"/{node}/1/in/set/bal", "0.0", 0.5, 0.0, {P: {"1": {"input": {"balance": 0.0}}}})
    if stem in ("bus", "main", "matrix"):
        binary(W, f"set_{stem}_mono", {stem: 1, "enabled": True}, osc(f"/{node}/1/busmono", ("i", 1)))
        binary(W, f"set_{stem}_trim", {stem: 1, "trim_db": -18.0}, osc(f"/{node}/1/in/set/trim", ("f", -18.0)))
        binary(W, f"set_{stem}_invert", {stem: 1, "enabled": True}, osc(f"/{node}/1/in/set/inv", ("i", 1)))
        _ti(f"{stem}-mono", f"/{node}/1/busmono", 0, {P: {"1": {"mono": False}}})
        _tf(f"{stem}-trim", f"/{node}/1/in/set/trim", "6.0", 0.6667, 6.0, {P: {"1": {"input": {"trim": 6.0}}}})
        _ti(f"{stem}-invert", f"/{node}/1/in/set/inv", 1, {P: {"1": {"input": {"invert": True}}}})
    if stem in ("channel", "aux"):
        binary(W, f"set_{stem}_solo_safe", {stem: 1, "enabled": True}, osc(f"/{node}/1/solosafe", ("i", 1)))
        binary(W, f"set_{stem}_auto_source", {stem: 1, "enabled": False}, osc(f"/{node}/1/in/set/srcauto", ("i", 0)))
        binary(W, f"set_{stem}_alt_input", {stem: 1, "enabled": True}, osc(f"/{node}/1/in/set/altsrc", ("i", 1)))
        binary(W, f"set_{stem}_custom_link", {stem: 1, "enabled": False}, osc(f"/{node}/1/clink", ("i", 0)))
        binary(W, f"set_{stem}_alt_source", {stem: 2, "group": "USB", "input": 47},
               [osc(f"/{node}/2/in/conn/altgrp", ("s", "USB")), osc(f"/{node}/2/in/conn/altin", ("i", 47))])
        _ti(f"{stem}-solo-safe", f"/{node}/1/solosafe", 1, {P: {"1": {"solo_safe": True}}})
        _ti(f"{stem}-auto-source", f"/{node}/1/in/set/srcauto", 0, {P: {"1": {"input": {"auto_source": False}}}})
        _ti(f"{stem}-alt-input", f"/{node}/1/in/set/altsrc", 1, {P: {"1": {"input": {"alt": True}}}})
        _ti(f"{stem}-custom-link", f"/{node}/1/clink", 1, {P: {"1": {"custom_link": True}}})
        _ts(f"{stem}-alt-source-group", f"/{node}/1/in/conn/altgrp", "OFF", {P: {"1": {"input": {"alt_source_group": "OFF"}}}})
        _ti(f"{stem}-alt-source-input", f"/{node}/1/in/conn/altin", 1, {P: {"1": {"input": {"alt_source_input": 1}}}})
binary(W, "set_channel_process_order", {"channel": 1, "order": "IDGE"}, osc("/ch/1/proc", ("s", "IDGE")))
binary(W, "set_channel_send_tap", {"channel": 1, "tap": "POST"}, osc("/ch/1/ptap", ("s", "POST")))
_ts("channel-process-order", "/ch/1/proc", "GEDI", {"channels": {"1": {"process_order": "GEDI"}}})
_ts("channel-send-tap", "/ch/1/ptap", "5", {"channels": {"1": {"send_tap": "5"}}})
binary(W, "set_matrix_direct_input", {"matrix": 8, "enabled": True}, osc("/mtx/8/dir/on", ("i", 1)))
binary(W, "set_matrix_direct_input_level", {"matrix": 8, "level_db": -144.0}, osc("/mtx/8/dir/lvl", ("f", -144.0)))
binary(W, "set_matrix_direct_input_invert", {"matrix": 8, "enabled": False}, osc("/mtx/8/dir/inv", ("i", 0)))
binary(W, "set_matrix_direct_input_source", {"matrix": 8, "source": "MON.BUS"}, osc("/mtx/8/dir/in", ("s", "MON.BUS")))
_ti("matrix-direct-input", "/mtx/1/dir/on", 0, {"matrices": {"1": {"direct_input": {"on": False}}}})
_tf("matrix-direct-input-level", "/mtx/1/dir/lvl", "0.0", 0.75, 0.0, {"matrices": {"1": {"direct_input": {"level": 0.0}}}})
_ti("matrix-direct-input-invert", "/mtx/1/dir/inv", 1, {"matrices": {"1": {"direct_input": {"invert": True}}}})
_ts("matrix-direct-input-source", "/mtx/1/dir/in", "AES", {"matrices": {"1": {"direct_input": {"source": "AES"}}}})

# ── Input sources, output patching, user signals, oscillators (p.40-46) ──
# /io/in/<group>/<n>/<leaf>; the preamp leaves g, vph, rmt and rcvc on the
# LCL and AES50 (A-C) groups; /io/out/<group>/<n>/grp then in.
binary(W, "set_input_mode", {"group": "LCL", "input": 3, "mode": "ST"}, osc("/io/in/LCL/3/mode", ("s", "ST")))
binary(W, "set_input_mute", {"group": "USB", "input": 48, "muted": True}, osc("/io/in/USB/48/mute", ("i", 1)))
binary(W, "set_input_invert", {"group": "A", "input": 1, "inverted": False}, osc("/io/in/A/1/pol", ("i", 0)))
binary(W, "set_input_color", {"group": "AES", "input": 2, "color": 12}, osc("/io/in/AES/2/col", ("i", 12)))
binary(W, "set_input_name", {"group": "CRD", "input": 64, "name": "Kick In"}, osc("/io/in/CRD/64/name", ("s", "Kick In")))
binary(W, "set_input_icon", {"group": "OSC", "input": 1, "icon": 999}, osc("/io/in/OSC/1/icon", ("i", 999)))
binary(W, "set_input_tags", {"group": "SC", "input": 32, "tags": "#D1"}, osc("/io/in/SC/32/tags", ("s", "#D1")))
binary(W, "set_input_gain", {"group": "B", "input": 48, "gain_db": 45.5}, osc("/io/in/B/48/g", ("f", 45.5)))
binary(W, "set_input_phantom", {"group": "LCL", "input": 1, "enabled": True}, osc("/io/in/LCL/1/vph", ("i", 1)))
binary(W, "set_input_remote", {"group": "C", "input": 2, "remote": "AES A"}, osc("/io/in/C/2/rmt", ("s", "AES A")))
binary(W, "set_input_remote_sync", {"group": "A", "input": 5, "enabled": False}, osc("/io/in/A/5/rcvc", ("i", 0)))
binary(W, "set_output_source", {"group": "AUX", "output": 8, "source_group": "MAIN", "source_input": 1},
       [osc("/io/out/AUX/8/grp", ("s", "MAIN")), osc("/io/out/AUX/8/in", ("i", 1))])
binary(W, "set_user_signal_source", {"signal": 24, "source_group": "CH", "source_input": 40},
       [osc("/io/in/USR/24/user/grp", ("s", "CH")), osc("/io/in/USR/24/user/in", ("i", 40))])
binary(W, "set_user_signal_tap", {"signal": 1, "tap": "PRE"}, osc("/io/in/USR/1/user/tap", ("s", "PRE")))
binary(W, "set_user_signal_side", {"signal": 1, "side": "L+R"}, osc("/io/in/USR/1/user/lr", ("s", "L+R")))
binary(W, "set_user_patch_source", {"patch": 1, "source_group": "A", "source_input": 17},
       [osc("/io/in/USR/25/user/grp", ("s", "A")), osc("/io/in/USR/25/user/in", ("i", 17))])
binary(W, "set_oscillator_level", {"oscillator": 2, "level_db": -40.0}, osc("/io/in/OSC/2/osc/lvl", ("f", -40.0)))
binary(W, "set_oscillator_type", {"oscillator": 1, "type": "PINK"}, osc("/io/in/OSC/1/osc/mode", ("s", "PINK")))
binary(W, "set_oscillator_frequency", {"oscillator": 1, "frequency_hz": 1000.0}, osc("/io/in/OSC/1/osc/f", ("f", 1000.0)))
binary(W, "set_alt_inputs", {"enabled": True}, osc("/io/altsw", ("i", 1)))
binary(W, "set_input_select_override", {"enabled": False}, osc("/io/autoaltovr", ("i", 0)))


def _io(group, n, **kv):
    return {"io": {"inputs": {group: {str(n): kv}}}}


_ts("input-mode", "/io/in/A/7/mode", "M/S", _io("A", 7, mode="M/S"))
_ti("input-mute", "/io/in/PLAY/4/mute", 1, _io("PLAY", 4, mute=True))
_ti("input-invert", "/io/in/USR/56/pol", 0, _io("USR", 56, invert=False))
_ti("input-color", "/io/in/AUX/8/col", 3, _io("AUX", 8, color=3))
_ts("input-name", "/io/in/LCL/24/name", "Talk", _io("LCL", 24, name="Talk"))
_ti("input-icon", "/io/in/MOD/1/icon", 42, _io("MOD", 1, icon=42))
_ts("input-tags", "/io/in/CRD/2/tags", "#D3", _io("CRD", 2, tags="#D3"))
_ti("input-mute-state", "/io/in/SC/1/$mute", 2, _io("SC", 1, mute_state=2))
_tf("input-gain", "/io/in/C/48/g", "30.0", 0.68, 30.0, _io("C", 48, gain=30.0))
_ti("input-phantom", "/io/in/LCL/1/vph", 1, _io("LCL", 1, phantom=True))
_ti("input-preamp-type", "/io/in/B/3/$ha", 4, _io("B", 3, preamp_type=4))
_ts("input-remote", "/io/in/A/1/rmt", "AES B", _io("A", 1, remote="AES B"))
_ti("input-remote-active", "/io/in/A/1/$ract", 1, _io("A", 1, remote_active=True))
_ts("input-remote-destination", "/io/in/A/1/$rdest", "AES A", _io("A", 1, remote_destination="AES A"))
_ti("input-remote-sync", "/io/in/LCL/2/rcvc", 0, _io("LCL", 2, remote_sync=False))
_ts("output-source-group", "/io/out/REC/4/grp", "MAIN", {"io": {"outputs": {"REC": {"4": {"source_group": "MAIN"}}}}})
_ti("output-source-input", "/io/out/CRD/64/in", 2, {"io": {"outputs": {"CRD": {"64": {"source_input": 2}}}}})
_ts("user-signal-group", "/io/in/USR/24/user/grp", "BUS", {"user_signals": {"24": {"source_group": "BUS"}}})
_ti("user-signal-input", "/io/in/USR/1/user/in", 16, {"user_signals": {"1": {"source_input": 16}}})
_ts("user-signal-tap", "/io/in/USR/10/user/tap", "POST", {"user_signals": {"10": {"tap": "POST"}}})
_ts("user-signal-side", "/io/in/USR/19/user/lr", "R", {"user_signals": {"19": {"side": "R"}}})
_ts("user-patch-group", "/io/in/USR/25/user/grp", "LCL", {"user_patches": {"1": {"source_group": "LCL"}}})
_ti("user-patch-input", "/io/in/USR/56/user/in", 64, {"user_patches": {"32": {"source_input": 64}}})
_tf("oscillator-level", "/io/in/OSC/2/osc/lvl", "-18.0", 0.32, -18.0, {"oscillators": {"2": {"level": -18.0}}})
_ts("oscillator-type", "/io/in/OSC/1/osc/mode", "WHITE", {"oscillators": {"1": {"type": "WHITE"}}})
_tf("oscillator-frequency", "/io/in/OSC/1/osc/f", "1k00", 0.5, 1000.0, {"oscillators": {"1": {"frequency": 1000.0}}})
_ti("alt-inputs", "/io/altsw", 1, {"io": {"alt_inputs": True}})
_ti("input-select-override", "/io/autoaltovr", 0, {"io": {"input_select_override": False}})

# ── Monitor buses, solo, talkback, main link, automix, RTA, meters (p.34-38) ──
for cmd_, inp, leaf, wire in [
        ("set_monitor_invert", {"enabled": True}, "inv", ("i", 1)),
        ("set_monitor_pan", {"pan": -100.0}, "pan", ("f", -100.0)),
        ("set_monitor_width", {"width": 150.0}, "wid", ("f", 150.0)),
        ("set_monitor_eq", {"enabled": False}, "eq/on", ("i", 0)),
        ("set_monitor_eq_low_gain", {"gain_db": -15.0}, "eq/lsg", ("f", -15.0)),
        ("set_monitor_eq_low_frequency", {"frequency_hz": 2000.0}, "eq/lsf", ("f", 2000.0)),
        ("set_monitor_eq_band_gain", {"band": 6, "gain_db": 15.0}, "eq/6g", ("f", 15.0)),
        ("set_monitor_eq_band_frequency", {"band": 1, "frequency_hz": 20.0}, "eq/1f", ("f", 20.0)),
        ("set_monitor_eq_band_q", {"band": 3, "q": 0.44}, "eq/3q", ("f", 0.44)),
        ("set_monitor_eq_high_gain", {"gain_db": 2.5}, "eq/hsg", ("f", 2.5)),
        ("set_monitor_eq_high_frequency", {"frequency_hz": 50.0}, "eq/hsf", ("f", 50.0)),
        ("set_monitor_limiter", {"level_db": -40.0}, "lim", ("f", -40.0)),
        ("set_monitor_delay", {"enabled": True}, "dly/on", ("i", 1)),
        ("set_monitor_delay_distance", {"meters": 100.0}, "dly/m", ("f", 100.0)),
        ("set_monitor_dim", {"level_db": -20.0}, "dim", ("f", -20.0)),
        ("set_monitor_pfl_dim", {"level_db": 0.0}, "pfldim", ("f", 0.0)),
        ("set_monitor_band_solo_trim", {"trim_db": 24.0}, "eqbdtrim", ("f", 24.0)),
        ("set_monitor_source_level", {"level_db": -144.0}, "srclvl", ("f", -144.0)),
        ("set_monitor_source_mix", {"level_db": 10.0}, "srcmix", ("f", 10.0)),
        ("set_monitor_tags", {"tags": "#M"}, "tags", ("s", "#M"))]:
    binary(W, cmd_, {"monitor": 2, **inp}, osc(f"/cfg/mon/2/{leaf}", wire))
binary(W, "set_solo_mode", {"mode": "SIP"}, osc("/cfg/solo/mode", ("s", "SIP")))
binary(W, "set_solo_monitor", {"monitor": "PH+SPK"}, osc("/cfg/solo/mon", ("s", "PH+SPK")))
binary(W, "set_solo_mute", {"enabled": True}, osc("/cfg/solo/mute", ("i", 1)))
binary(W, "set_solo_flip", {"enabled": False}, osc("/cfg/solo/$flip", ("i", 0)))
binary(W, "set_solo_tap", {"section": "mtx", "tap": "AFL"}, osc("/cfg/solo/mtxtap", ("s", "AFL")))
binary(W, "set_source_solo_enable", {"value": "CH39"}, osc("/cfg/solo/srcsolo", ("s", "CH39")))
binary(W, "set_source_solo", {"enabled": True}, osc("/cfg/solo/$srcsolo", ("i", 1)))
binary(W, "set_source_solo_input", {"group": 13, "input": 2},
       [osc("/cfg/solo/$srcsgrp", ("i", 13)), osc("/cfg/solo/$srcsin", ("i", 2))])
binary(W, "set_talkback_assign", {"assign": "AUX8"}, osc("/cfg/talk/assign", ("s", "AUX8")))
binary(W, "set_talkback_monitor_dim", {"talkback": "A", "dim_db": -40.0}, osc("/cfg/talk/A/mondim", ("f", -40.0)))
binary(W, "set_talkback_bus_dim", {"talkback": "B", "dim_db": 0.0}, osc("/cfg/talk/B/busdim", ("f", 0.0)))
binary(W, "set_talkback_individual_levels", {"talkback": "B", "enabled": True}, osc("/cfg/talk/B/indiv", ("i", 1)))
binary(W, "set_main_link", {"link": "2-4"}, osc("/cfg/mainlink", ("s", "2-4")))
binary(W, "set_dca_mute_groups", {"enabled": True}, osc("/cfg/dcamgrp", ("i", 1)))
binary(W, "set_automix_group", {"group": "Y", "enabled": False}, osc("/cfg/amix/y", ("i", 0)))
binary(W, "set_rta_source", {"source": 76}, osc("/cfg/rta/rtasrc", ("i", 76)))
binary(W, "set_rta_tap", {"tap": "FXOUT"}, osc("/cfg/rta/rtatap", ("s", "FXOUT")))
for pre, leaf in [("rta", "rta"), ("eq_rta", "eq")]:
    binary(W, f"set_{pre}_decay", {"decay": "FAST"}, osc(f"/cfg/rta/{leaf}decay", ("s", "FAST")))
    binary(W, f"set_{pre}_detector", {"detector": "AVG"}, osc(f"/cfg/rta/{leaf}det", ("s", "AVG")))
    binary(W, f"set_{pre}_range", {"range_db": 60.0}, osc(f"/cfg/rta/{leaf}range", ("f", 60.0)))
    binary(W, f"set_{pre}_gain", {"gain_db": -5.0}, osc(f"/cfg/rta/{leaf}gain", ("f", -5.0)))
    binary(W, f"set_{pre}_autogain", {"enabled": True}, osc(f"/cfg/rta/{leaf}auto", ("i", 1)))
binary(W, "set_meter_scope_source", {"source": 0}, osc("/cfg/mtr/scopesrc", ("i", 0)))
binary(W, "set_meter_scope_tap", {"tap": "MON.SPK"}, osc("/cfg/mtr/scopetap", ("s", "MON.SPK")))
binary(W, "set_fader_meter_tap", {"section": "dca", "tap": "PRE"}, osc("/cfg/mtr/mtrsfc/dca", ("s", "PRE")))
binary(W, "set_meter_page_tap", {"section": "in", "tap": "POST"}, osc("/cfg/mtr/mtrpage/in", ("s", "POST")))
binary(W, "set_main_meter", {"source": "SEL_CH"}, osc("/cfg/mtr/mainmtr", ("s", "SEL_CH")))
binary(W, "set_main_meter_position", {"position": "AUTO"}, osc("/cfg/mtr/mainpos", ("s", "AUTO")))

_ti("monitor-invert", "/cfg/mon/1/inv", 1, {"monitors": {"1": {"invert": True}}})
_tf("monitor-pan", "/cfg/mon/1/pan", "-20", 0.4, -20.0, {"monitors": {"1": {"pan": -20.0}}})
_tf("monitor-width", "/cfg/mon/2/wid", "100", 0.8333, 100.0, {"monitors": {"2": {"width": 100.0}}})
_ti("monitor-eq", "/cfg/mon/2/eq/on", 0, {"monitors": {"2": {"eq": {"on": False}}}})
_tf("monitor-eq-low-gain", "/cfg/mon/1/eq/lsg", "3.0", 0.6, 3.0, {"monitors": {"1": {"eq": {"low_gain": 3.0}}}})
_tf("monitor-eq-low-frequency", "/cfg/mon/1/eq/lsf", "100", 0.3, 100.0, {"monitors": {"1": {"eq": {"low_frequency": 100.0}}}})
_tf("monitor-eq-band-gain", "/cfg/mon/1/eq/5g", "-6.0", 0.3, -6.0, {"monitors": {"1": {"eq": {"bands": {"5": {"gain": -6.0}}}}}})
_tf("monitor-eq-band-frequency", "/cfg/mon/2/eq/2f", "2k00", 0.55, 2000.0, {"monitors": {"2": {"eq": {"bands": {"2": {"frequency": 2000.0}}}}}})
_tf("monitor-eq-band-q", "/cfg/mon/2/eq/6q", "0.75", 0.1, 0.75, {"monitors": {"2": {"eq": {"bands": {"6": {"q": 0.75}}}}}})
_tf("monitor-eq-high-gain", "/cfg/mon/1/eq/hsg", "-1.5", 0.45, -1.5, {"monitors": {"1": {"eq": {"high_gain": -1.5}}}})
_tf("monitor-eq-high-frequency", "/cfg/mon/1/eq/hsf", "10k0", 0.8, 10000.0, {"monitors": {"1": {"eq": {"high_frequency": 10000.0}}}})
_tf("monitor-limiter", "/cfg/mon/1/lim", "-6", 0.85, -6.0, {"monitors": {"1": {"limiter": -6.0}}})
_ti("monitor-delay", "/cfg/mon/1/dly/on", 1, {"monitors": {"1": {"delay": {"on": True}}}})
_tf("monitor-delay-distance", "/cfg/mon/1/dly/m", "3.5", 0.03, 3.5, {"monitors": {"1": {"delay": {"distance": 3.5}}}})
_tf("monitor-dim", "/cfg/mon/2/dim", "-20", 0.5, -20.0, {"monitors": {"2": {"dim": -20.0}}})
_tf("monitor-pfl-dim", "/cfg/mon/2/pfldim", "-10", 0.75, -10.0, {"monitors": {"2": {"pfl_dim": -10.0}}})
_tf("monitor-band-solo-trim", "/cfg/mon/2/eqbdtrim", "12", 0.5, 12.0, {"monitors": {"2": {"band_solo_trim": 12.0}}})
_tf("monitor-source-level", "/cfg/mon/1/srclvl", "0.0", 0.75, 0.0, {"monitors": {"1": {"source_level": 0.0}}})
_tf("monitor-source-mix", "/cfg/mon/1/srcmix", "-oo", 0.0, -144.0, {"monitors": {"1": {"source_mix": -144.0}}})
_tf("monitor-fader-level", "/cfg/mon/1/$lvlact", "-10.0", 0.5, -10.0, {"monitors": {"1": {"fader_level": -10.0}}})
_ts("monitor-tags", "/cfg/mon/2/tags", "#PH", {"monitors": {"2": {"tags": "#PH"}}})
_ts("solo-mode", "/cfg/solo/mode", "LIVE", {"solo": {"mode": "LIVE"}})
_ts("solo-monitor", "/cfg/solo/mon", "SPK", {"solo": {"monitor": "SPK"}})
_ti("solo-mute", "/cfg/solo/mute", 1, {"solo": {"mute": True}})
_ti("solo-flip", "/cfg/solo/$flip", 0, {"solo": {"flip": False}})
_ts("solo-tap", "/cfg/solo/chtap", "PFL", {"solo": {"taps": {"ch": "PFL"}}})
_ts("source-solo-enable", "/cfg/solo/srcsolo", "OFF", {"solo": {"source_solo": {"enable": "OFF"}}})
_ti("source-solo", "/cfg/solo/$srcsolo", 1, {"solo": {"source_solo": {"on": True}}})
_ti("source-solo-group", "/cfg/solo/$srcsgrp", 3, {"solo": {"source_solo": {"group": 3}}})
_ti("source-solo-input", "/cfg/solo/$srcsin", 48, {"solo": {"source_solo": {"input": 48}}})
_ti("solo-in-place", "/$stat/sip", 1, {"solo": {"in_place": True}})
for node, P in _PLURAL.items():
    _ti(f"{node}-solo-led", f"/{node}/2/$sololed", 1, {P: {"2": {"solo_led": True}}})
_ts("talkback-assign", "/cfg/talk/assign", "CH40", {"talkback": {"assign": "CH40"}})
_tf("talkback-level", "/cfg/talk/$lvl", "-10.0", 0.5, -10.0, {"talkback": {"level": -10.0}})
for t in ("A", "B"):
    lt = t.lower()
    _tf(f"talkback-{lt}-monitor-dim", f"/cfg/talk/{t}/mondim", "-20", 0.5, -20.0, {"talkback": {lt: {"monitor_dim": -20.0}}})
    _tf(f"talkback-{lt}-bus-dim", f"/cfg/talk/{t}/busdim", "-6", 0.85, -6.0, {"talkback": {lt: {"bus_dim": -6.0}}})
    _ti(f"talkback-{lt}-individual", f"/cfg/talk/{t}/indiv", 1, {"talkback": {lt: {"individual_levels": True}}})
_ts("main-link", "/cfg/mainlink", "2-3", {"config": {"main_link": "2-3"}})
_ti("dca-mute-groups", "/cfg/dcamgrp", 0, {"config": {"dca_mute_groups": False}})
_ti("automix-x", "/cfg/amix/x", 1, {"automix": {"x": True}})
_ti("rta-source", "/cfg/rta/rtasrc", 12, {"rta": {"source": 12}})
_ts("rta-tap", "/cfg/rta/rtatap", "PREEQ", {"rta": {"tap": "PREEQ"}})
_ts("rta-decay", "/cfg/rta/rtadecay", "MED", {"rta": {"decay": "MED"}})
_ts("rta-detector", "/cfg/rta/rtadet", "RMS", {"rta": {"detector": "RMS"}})
_tf("rta-range", "/cfg/rta/rtarange", "60", 1.0, 60.0, {"rta": {"range": 60.0}})
_tf("rta-gain", "/cfg/rta/rtagain", "10", 0.27, 10.0, {"rta": {"gain": 10.0}})
_ti("rta-autogain", "/cfg/rta/rtaauto", 1, {"rta": {"autogain": True}})
_ts("eq-rta-decay", "/cfg/rta/eqdecay", "SLOW", {"rta": {"eq": {"decay": "SLOW"}}})
_ts("eq-rta-detector", "/cfg/rta/eqdet", "PEAK", {"rta": {"eq": {"detector": "PEAK"}}})
_tf("eq-rta-range", "/cfg/rta/eqrange", "30", 0.0, 30.0, {"rta": {"eq": {"range": 30.0}}})
_tf("eq-rta-gain", "/cfg/rta/eqgain", "0", 0.09, 0.0, {"rta": {"eq": {"gain": 0.0}}})
_ti("eq-rta-autogain", "/cfg/rta/eqauto", 0, {"rta": {"eq": {"autogain": False}}})
_ti("rta-active-source", "/cfg/rta/$src", 5, {"rta": {"active": {"source": 5}}})
_ts("rta-active-tap", "/cfg/rta/$tap", "POST", {"rta": {"active": {"tap": "POST"}}})
_ts("rta-active-decay", "/cfg/rta/$dec", "FAST", {"rta": {"active": {"decay": "FAST"}}})
_ts("rta-active-detector", "/cfg/rta/$det", "PEAK", {"rta": {"active": {"detector": "PEAK"}}})
_ti("meter-scope-source", "/cfg/mtr/scopesrc", 40, {"meters": {"scope_source": 40}})
_ts("meter-scope-tap", "/cfg/mtr/scopetap", "SOLO", {"meters": {"scope_tap": "SOLO"}})
_ti("meter-active-scope-source", "/cfg/mtr/$scopesrc", 41, {"meters": {"active_scope_source": 41}})
_ts("meter-active-scope-tap", "/cfg/mtr/$scopetap", "IN", {"meters": {"active_scope_tap": "IN"}})
_ts("main-meter", "/cfg/mtr/mainmtr", "MTX.8", {"meters": {"main_meter": "MTX.8"}})
_ts("main-meter-position", "/cfg/mtr/mainpos", "PRE", {"meters": {"main_position": "PRE"}})
_ts("fader-meter-tap", "/cfg/mtr/mtrsfc/bus", "POST", {"meters": {"fader_taps": {"bus": "POST"}}})
_ts("meter-page-tap", "/cfg/mtr/mtrpage/mtx", "PRE", {"meters": {"page_taps": {"mtx": "PRE"}}})

# ── Surface preferences and status, console preferences, globals, system,
# status (p.32-33, p.39, p.71-73, p.88-89; names as in ce_data p.284-288) ──
binary(W, "set_surface_switch", {"switch": "muteovr", "enabled": True}, osc("/$ctl/cfg/muteovr", ("i", 1)))
binary(W, "set_surface_switch", {"switch": "autosel_CMPCT", "enabled": False}, osc("/$ctl/cfg/autosel_CMPCT", ("i", 0)),
       file="set_surface_switch-autosel")
binary(W, "set_sof_faders", {"mode": "L/C"}, osc("/$ctl/cfg/soffdr", ("s", "L/C")))
binary(W, "set_sof_button_mode", {"mode": "FLASH"}, osc("/$ctl/cfg/sofbutton", ("s", "FLASH")))
binary(W, "set_select_double_click", {"mode": "BUSFX"}, osc("/$ctl/cfg/seldblclick", ("s", "BUSFX")))
binary(W, "set_function_keys_mode", {"mode": "CC"}, osc("/$ctl/cfg/usrmode", ("s", "CC")))
binary(W, "set_compact_main_fader", {"mode": "DCA.16"}, osc("/$ctl/cfg/mfdr", ("s", "DCA.16")))
binary(W, "set_compact_button_mode", {"mode": "USER"}, osc("/$ctl/cfg/cscmode", ("s", "USER")))
binary(W, "set_rack_control_mode", {"mode": "SD-B"}, osc("/$ctl/cfg/rackmode", ("s", "SD-B")))
binary(W, "set_surface_light", {"light": "lamp", "level": 100}, osc("/$ctl/cfg/lights/lamp", ("i", 100)))
binary(W, "set_rta_view_home_size", {"value": "1/3"}, osc("/$ctl/cfg/rta/homedisp", ("s", "1/3")))
binary(W, "set_rta_view_home_color", {"value": "BL50"}, osc("/$ctl/cfg/rta/homecol", ("s", "BL50")))
binary(W, "set_rta_view_home_tap", {"value": "EQ"}, osc("/$ctl/cfg/rta/hometap", ("s", "EQ")))
binary(W, "set_rta_view_eq_size", {"value": "OVL/3"}, osc("/$ctl/cfg/rta/eqdisp", ("s", "OVL/3")))
binary(W, "set_rta_view_eq_color", {"value": "RD25"}, osc("/$ctl/cfg/rta/eqcol", ("s", "RD25")))
binary(W, "set_rta_view_eq_tap", {"value": "POST"}, osc("/$ctl/cfg/rta/cheqtap", ("s", "POST")))
binary(W, "set_rta_view_filter_tap", {"value": "PRE"}, osc("/$ctl/cfg/rta/chflttap", ("s", "PRE")))
binary(W, "select_strip", {"strip": 76}, osc("/$ctl/$stat/selidx", ("i", 76)))
binary(W, "set_channel_page", {"page": 30}, osc("/$ctl/$stat/pageidx", ("i", 30)))
binary(W, "select_eq_band", {"band": 8}, osc("/$ctl/$stat/bandidx", ("i", 8)))
binary(W, "set_sends_on_fader", {"strip": -1}, osc("/$ctl/$stat/sof", ("i", -1)))
binary(W, "set_send_page", {"page": "MATRIX"}, osc("/$ctl/$stat/sendpage", ("s", "MATRIX")))
binary(W, "set_home_tab", {"tab": 4}, osc("/$ctl/$stat/chsetuptab", ("i", 4)))
binary(W, "set_preference_switch", {"switch": "cf_load", "enabled": False}, osc("/$ctl/$globals/cf_load", ("i", 0)))
binary(W, "set_fader_resolution", {"value": "FINE"}, osc("/$ctl/$globals/fdrres", ("s", "FINE")))
binary(W, "set_fader_speed", {"value": "SLOW"}, osc("/$ctl/$globals/fdrspd", ("s", "SLOW")))
binary(W, "set_tap_tempo_flash", {"value": "8X"}, osc("/$ctl/$globals/tapflash", ("s", "8X")))
binary(W, "set_time_format", {"value": "12H"}, osc("/$ctl/$globals/timefmt", ("s", "12H")))
binary(W, "set_date_format", {"value": "DMY"}, osc("/$ctl/$globals/datefmt", ("s", "DMY")))
binary(W, "set_file_sort", {"value": "9->0"}, osc("/$ctl/$globals/$filesort", ("s", "9->0")))
binary(W, "set_mouse_speed", {"speed": 2.0}, osc("/$ctl/$globals/mousespd", ("f", 2.0)))
binary(W, "set_autosave", {"enabled": True}, osc("/$ctl/$globals/$noautosave", ("i", 0)))
binary(W, "save_console_data", {}, osc("/$ctl/$globals/$savenow", ("i", 1)))
binary(W, "set_clock_rate", {"rate": "44100.0"}, osc("/$globals/clkrate", ("s", "44100.0")))
binary(W, "set_clock_source", {"source": "CARD"}, osc("/$globals/clksrc", ("s", "CARD")))
binary(W, "set_startup_mute", {"enabled": True}, osc("/$globals/startmute", ("i", 1)))
binary(W, "set_usb_audio_config", {"config": "48/48"}, osc("/$globals/usbacfg", ("s", "48/48")))
binary(W, "set_stageconnect_config", {"config": "16/16"}, osc("/$globals/sccfg", ("s", "16/16")))
binary(W, "set_ha_remote", {"port": "B", "enabled": True}, osc("/$globals/harmt/b", ("i", 1)))
binary(W, "set_custom_sync", {"port": "C", "enabled": False}, osc("/$globals/custsync/c", ("i", 0)))
binary(W, "set_log_flags", {"flags": "0"}, osc("/$syscfg/logflags", ("s", "0")))
binary(W, "set_network_mode", {"mode": "STATIC"}, osc("/$syscfg/ipmode", ("s", "STATIC")))
binary(W, "set_network_address", {"octet1": 192, "octet2": 168, "octet3": 1, "octet4": 71},
       [osc("/$syscfg/ip0", ("i", 192)), osc("/$syscfg/ip1", ("i", 168)), osc("/$syscfg/ip2", ("i", 1)), osc("/$syscfg/ip3", ("i", 71))])
binary(W, "set_network_mask", {"octet1": 255, "octet2": 255, "octet3": 255, "octet4": 0},
       [osc("/$syscfg/msk0", ("i", 255)), osc("/$syscfg/msk1", ("i", 255)), osc("/$syscfg/msk2", ("i", 255)), osc("/$syscfg/msk3", ("i", 0))])
binary(W, "set_network_gateway", {"octet1": 192, "octet2": 168, "octet3": 1, "octet4": 1},
       [osc("/$syscfg/gw0", ("i", 192)), osc("/$syscfg/gw1", ("i", 168)), osc("/$syscfg/gw2", ("i", 1)), osc("/$syscfg/gw3", ("i", 1))])
binary(W, "apply_network_settings", {}, osc("/$syscfg/$ipapply", ("i", 1)))
binary(W, "set_tcp_lock", {"enabled": True}, osc("/$syscfg/tcplock", ("i", 1)))
binary(W, "set_usb_host_speed", {"speed": "HS"}, osc("/$syscfg/usbh_spd", ("s", "HS")))
binary(W, "set_ethernet_mode", {"mode": "SWITCHED"}, osc("/$syscfg/eth/cfg", ("s", "SWITCHED")))
binary(W, "reset_aes50_errors", {"port": "A"}, osc("/$stat/A/clrerr", ("i", 1)))

_ti("surface-switch", "/$ctl/cfg/soloexcl", 1, {"surface": {"switches": {"soloexcl": True}}})
_ti("surface-switch-autosel", "/$ctl/cfg/autosel_R", 0, {"surface": {"switches": {"autosel_R": False}}})
_ts("surface-sof-faders", "/$ctl/cfg/soffdr", "ALL", {"surface": {"sof_faders": "ALL"}})
_ts("surface-sof-button", "/$ctl/cfg/sofbutton", "AUTO", {"surface": {"sof_button": "AUTO"}})
_ts("surface-select-double-click", "/$ctl/cfg/seldblclick", "HOME", {"surface": {"select_double_click": "HOME"}})
_ts("surface-function-keys", "/$ctl/cfg/usrmode", "BUS", {"surface": {"function_keys": "BUS"}})
_ts("surface-compact-main-fader", "/$ctl/cfg/mfdr", "MAIN.1", {"surface": {"compact_main_fader": "MAIN.1"}})
_ts("surface-compact-buttons", "/$ctl/cfg/cscmode", "DCA", {"surface": {"compact_buttons": "DCA"}})
_ts("surface-rack-controls", "/$ctl/cfg/rackmode", "MGRP", {"surface": {"rack_controls": "MGRP"}})
_ti("surface-light", "/$ctl/cfg/lights/chlcdctr", 50, {"surface": {"lights": {"chlcdctr": 50}}})
_ts("rta-view-home-size", "/$ctl/cfg/rta/homedisp", "FULL", {"surface": {"rta": {"home_size": "FULL"}}})
_ts("rta-view-home-color", "/$ctl/cfg/rta/homecol", "AM75", {"surface": {"rta": {"home_color": "AM75"}}})
_ts("rta-view-home-tap", "/$ctl/cfg/rta/hometap", "IN", {"surface": {"rta": {"home_tap": "IN"}}})
_ts("rta-view-eq-size", "/$ctl/cfg/rta/eqdisp", "1/4", {"surface": {"rta": {"eq_size": "1/4"}}})
_ts("rta-view-eq-color", "/$ctl/cfg/rta/eqcol", "BL75", {"surface": {"rta": {"eq_color": "BL75"}}})
_ts("rta-view-eq-tap", "/$ctl/cfg/rta/cheqtap", "PRE", {"surface": {"rta": {"eq_tap": "PRE"}}})
_ts("rta-view-filter-tap", "/$ctl/cfg/rta/chflttap", "POST", {"surface": {"rta": {"filter_tap": "POST"}}})
_ti("surface-selected-strip", "/$ctl/$stat/selidx", 0, {"surface": {"selected_strip": 1}})
_ti("surface-page", "/$ctl/$stat/pageidx", 2, {"surface": {"page": 2}})
_ti("surface-eq-band", "/$ctl/$stat/bandidx", 3, {"surface": {"eq_band": 3}})
_ti("surface-sends-on-fader", "/$ctl/$stat/sof", -1, {"surface": {"sends_on_fader": -1}})
_ts("surface-lock", "/$ctl/$stat/cnslock", "1001001000000000000", {"surface": {"lock": "1001001000000000000"}})
_ts("surface-send-page", "/$ctl/$stat/sendpage", "BUS", {"surface": {"send_page": "BUS"}})
_ti("surface-home-tab", "/$ctl/$stat/chsetuptab", 2, {"surface": {"home_tab": 2}})
_ti("preference-switch", "/$ctl/$globals/usewheel", 1, {"preferences": {"switches": {"usewheel": True}}})
_ts("preference-fader-resolution", "/$ctl/$globals/fdrres", "AUTO", {"preferences": {"fader_resolution": "AUTO"}})
_ts("preference-fader-speed", "/$ctl/$globals/fdrspd", "MED", {"preferences": {"fader_speed": "MED"}})
_ts("preference-tap-flash", "/$ctl/$globals/tapflash", "ON", {"preferences": {"tap_flash": "ON"}})
_ts("preference-time-format", "/$ctl/$globals/timefmt", "24H", {"preferences": {"time_format": "24H"}})
_ts("preference-date-format", "/$ctl/$globals/datefmt", "YMD", {"preferences": {"date_format": "YMD"}})
_ts("preference-file-sort", "/$ctl/$globals/$filesort", "NAME", {"preferences": {"file_sort": "NAME"}})
_tf("preference-mouse-speed", "/$ctl/$globals/mousespd", "1.75", 0.88, 1.75, {"preferences": {"mouse_speed": 1.75}})
_ti("preference-autosave", "/$ctl/$globals/$noautosave", 0, {"preferences": {"autosave": True}})
_ts("globals-clock-rate", "/$globals/clkrate", "48000.0", {"globals": {"clock_rate": "48000.0"}})
_ts("globals-clock-source", "/$globals/clksrc", "INT", {"globals": {"clock_source": "INT"}})
_ti("globals-startup-mute", "/$globals/startmute", 0, {"globals": {"startup_mute": False}})
_ts("globals-usb-audio", "/$globals/usbacfg", "32/32", {"globals": {"usb_audio": "32/32"}})
_ts("globals-stageconnect", "/$globals/sccfg", "AUTO", {"globals": {"stageconnect": "AUTO"}})
_ti("globals-ha-remote", "/$globals/harmt/a", 1, {"globals": {"ha_remote": {"a": True}}})
_ti("globals-custom-sync", "/$globals/custsync/b", 0, {"globals": {"custom_sync": {"b": False}}})
_ts("network-mode", "/$syscfg/ipmode", "DHCP", {"network": {"mode": "DHCP"}})
_ti("network-address", "/$syscfg/ip0", 192, {"network": {"address": {"1": 192}}})
_ti("network-mask", "/$syscfg/msk3", 0, {"network": {"mask": {"4": 0}}})
_ti("network-gateway", "/$syscfg/gw2", 1, {"network": {"gateway": {"3": 1}}})
_ts("console-log-flags", "/$syscfg/logflags", "0", {"console": {"log_flags": "0"}})
_ts("console-build", "/$syscfg/$cnscfg", "wing-rack", {"console": {"build": "wing-rack"}})
_ts("console-hardware-version", "/$syscfg/$chwversion", "2", {"console": {"hardware_version": "2"}})
_ti("console-tcp-lock", "/$syscfg/tcplock", 0, {"console": {"tcp_lock": False}})
_ts("console-usb-host-speed", "/$syscfg/usbh_spd", "FS", {"console": {"usb_host_speed": "FS"}})
_ts("console-usb-host-speed-active", "/$syscfg/$usbspd_act", "HS", {"console": {"usb_host_speed_active": "HS"}})
_ts("console-ethernet-mode", "/$syscfg/eth/cfg", "SEPARATE", {"console": {"ethernet_mode": "SEPARATE"}})
_ts("console-option-module", "/$syscfg/opt_mod", "DANTE", {"console": {"option_module": "DANTE"}})
_ts("status-aes50-state", "/$stat/B/stat", "OK", {"status": {"aes50": {"b": {"state": "OK"}}}})
_ts("status-aes50-device", "/$stat/C/dev", "SD16", {"status": {"aes50": {"c": {"device": "SD16"}}}})
_ti("status-aes50-corrected", "/$stat/A/errorsc", 3, {"status": {"aes50": {"a": {"corrected_errors": 3}}}})
_ti("status-aes50-uncorrected", "/$stat/A/errorsu", 0, {"status": {"aes50": {"a": {"uncorrected_errors": 0}}}})
_ts("status-aes50-remote", "/$stat/rmt_b", "FOH", {"status": {"aes50": {"b": {"remote_console": "FOH"}}}})
_ti("status-clock-lock", "/$stat/lock", 1, {"status": {"clock_lock": True}})
_ti("status-clock-ppm", "/$stat/ppm", -3, {"status": {"clock_ppm": -3}})
_ti("status-rtc-error", "/$stat/rtcerr", 0, {"status": {"rtc_error": False}})
_ts("status-time", "/$stat/time", "21:04:33", {"status": {"time": "21:04:33"}})
_ts("status-date", "/$stat/date", "2026-10-13", {"status": {"date": "2026-10-13"}})
_ts("status-usb-state", "/$stat/usbstate", "IDLE", {"status": {"usb": {"state": "IDLE"}}})
_ts("status-usb-volume", "/$stat/usbvolname", "STICK", {"status": {"usb": {"volume": "STICK"}}})
_ts("status-stageconnect-state", "/$stat/sc_stat", "OK", {"status": {"stageconnect": {"state": "OK"}}})
_ts("status-stageconnect-devices", "/$stat/sc_devices", "DL32", {"status": {"stageconnect": {"devices": "DL32"}}})
_ti("status-stageconnect-upstreams", "/$stat/sc_upcnt", 16, {"status": {"stageconnect": {"upstreams": 16}}})
_ti("status-stageconnect-downstreams", "/$stat/sc_dncnt", 16, {"status": {"stageconnect": {"downstreams": 16}}})
_ts("status-stageconnect-routing", "/$stat/sc_uprout", "1-16", {"status": {"stageconnect": {"upstream_routing": "1-16"}}})

# ── USB player and recorder, W-Live cards (p.68-70) ──────────────────────
for c, a in [("usb_player_play", "PLAY"), ("usb_player_stop", "STOP"), ("usb_player_pause", "PAUSE"),
             ("usb_player_next", "NEXT"), ("usb_player_previous", "PREV")]:
    binary(W, c, {}, osc("/play/$action", ("s", a)))
binary(W, "usb_player_play_file", {"path": "U:/music/intro.wav"},
       [osc("/play/$playfile", ("s", "U:/music/intro.wav")), osc("/play/$action", ("s", "PLAYFILE"))])
binary(W, "usb_player_select", {"entry": 3}, osc("/play/$actionidx", ("i", 3)))
binary(W, "usb_player_seek", {"position_s": 90.0}, osc("/play/$pos", ("f", 90.0)))
binary(W, "set_usb_player_repeat", {"enabled": True}, osc("/play/repeat", ("i", 1)))
for c, a in [("usb_recorder_record", "REC"), ("usb_recorder_stop", "STOP"), ("usb_recorder_pause", "PAUSE"),
             ("usb_recorder_new_file", "NEWFILE")]:
    binary(W, c, {}, osc("/rec/$action", ("s", a)))
binary(W, "set_usb_recorder_path", {"path": "U:/rec"}, osc("/rec/path", ("s", "U:/rec")))
binary(W, "set_usb_recorder_resolution", {"bits": "24"}, osc("/rec/resolution", ("s", "24")))
binary(W, "set_usb_recorder_channels", {"channels": "4"}, osc("/rec/channels", ("s", "4")))
binary(W, "set_wlive_sd_link", {"mode": "PAR"}, osc("/cards/wlive/sdlink", ("s", "PAR")))
binary(W, "set_wlive_auto_input", {"input": "2"}, osc("/cards/wlive/autoin", ("s", "2")))
binary(W, "set_wlive_meters", {"enabled": True}, osc("/cards/wlive/meters", ("i", 1)))
binary(W, "set_wlive_auto_routing", {"when": "rec", "setting": "ALT"}, osc("/cards/wlive/auto_rec", ("s", "ALT")))
binary(W, "wlive_transport", {"card": 2, "action": "REC"}, osc("/cards/wlive/2/$ctl/control", ("s", "REC")))
binary(W, "wlive_open_session", {"card": 1, "session": 100}, osc("/cards/wlive/1/$ctl/opensession", ("i", 100)))
binary(W, "wlive_edit_marker", {"card": 1, "marker": 4}, osc("/cards/wlive/1/$ctl/editmarker", ("i", 4)))
binary(W, "wlive_goto_marker", {"card": 1, "marker": 0}, osc("/cards/wlive/1/$ctl/gotomarker", ("i", 0)))
binary(W, "wlive_delete_marker", {"card": 2, "marker": 7}, osc("/cards/wlive/2/$ctl/deletemarker", ("i", 7)))
binary(W, "wlive_delete_session", {"card": 2, "session": 1}, osc("/cards/wlive/2/$ctl/deletesession", ("i", 1)))
binary(W, "wlive_goto_time", {"card": 1, "time_ms": 60000.0},
       [osc("/cards/wlive/1/$ctl/stime", ("f", 60000.0)), osc("/cards/wlive/1/$ctl/gotomarker", ("i", 101))])
binary(W, "wlive_name_session", {"card": 1, "name": "Sunday AM"}, osc("/cards/wlive/1/$ctl/namesession", ("s", "Sunday AM")))
binary(W, "wlive_add_marker", {"card": 2}, osc("/cards/wlive/2/$ctl/setmarker", ("i", 1)))
binary(W, "wlive_format_sd", {"card": 1}, osc("/cards/wlive/1/$ctl/formatsdcard", ("i", 1)))
binary(W, "set_wlive_record_tracks", {"card": 1, "tracks": "16"}, osc("/cards/wlive/1/cfg/rectracks", ("s", "16")))
binary(W, "set_wlive_play_mode", {"card": 2, "mode": "A->B"}, osc("/cards/wlive/2/cfg/playmode", ("s", "A->B")))
binary(W, "wlive_select_session", {"card": 1, "session": 2}, osc("/cards/wlive/1/$stat/sessionpos", ("i", 2)))
binary(W, "wlive_select_marker", {"card": 1, "marker": 3}, osc("/cards/wlive/1/$stat/markerpos", ("i", 3)))
binary(W, "set_wlive_play_range", {"card": 2, "start_ms": 1000.0, "stop_ms": 5000.0},
       [osc("/cards/wlive/2/$stat/start", ("f", 1000.0)), osc("/cards/wlive/2/$stat/stop", ("f", 5000.0))])

_ts("player-playlist", "/play/$actlist", "U:/music", {"usb_player": {"playlist": "U:/music"}})
_ti("player-entry", "/play/$actidx", 2, {"usb_player": {"entry": 2}})
_ts("player-state", "/play/$actstate", "PLAY", {"usb_player": {"state": "PLAY"}})
_ts("player-file", "/play/$actfile", "U:/music/a.wav", {"usb_player": {"file": "U:/music/a.wav"}})
_ts("player-song", "/play/$song", "Intro", {"usb_player": {"song": "Intro"}})
_ts("player-album", "/play/$album", "Live", {"usb_player": {"album": "Live"}})
_ts("player-artist", "/play/$artist", "Band", {"usb_player": {"artist": "Band"}})
_tf("player-position", "/play/$pos", "0:12", 0.0003, 12.0, {"usb_player": {"position": 12.0}})
_tf("player-total", "/play/$total", "3:30", 0.0058, 210.0, {"usb_player": {"total": 210.0}})
_ts("player-resolution", "/play/$resolution", "24", {"usb_player": {"resolution": "24"}})
_ts("player-channels", "/play/$channels", "2", {"usb_player": {"channels": "2"}})
_ts("player-rate", "/play/$rate", "48", {"usb_player": {"rate": "48"}})
_ts("player-format", "/play/$format", "FLAC", {"usb_player": {"format": "FLAC"}})
_ti("player-repeat", "/play/repeat", 0, {"usb_player": {"repeat": False}})
_ts("recorder-state", "/rec/$actstate", "REC", {"usb_recorder": {"state": "REC"}})
_ts("recorder-file", "/rec/$actfile", "U:/rec/r1.wav", {"usb_recorder": {"file": "U:/rec/r1.wav"}})
_ts("recorder-path", "/rec/path", "U:/rec", {"usb_recorder": {"path": "U:/rec"}})
_ts("recorder-resolution", "/rec/resolution", "16", {"usb_recorder": {"resolution": "16"}})
_ts("recorder-channels", "/rec/channels", "2", {"usb_recorder": {"channels": "2"}})
_tf("recorder-time", "/rec/$time", "0:05", 0.0001, 5.0, {"usb_recorder": {"time": 5.0}})
_ts("cards-type", "/cards/$type", "WLIVE", {"cards": {"type": "WLIVE"}})
_ts("cards-version", "/cards/$ver", "1.0", {"cards": {"version": "1.0"}})
_ts("wlive-sd-link", "/cards/wlive/sdlink", "IND", {"cards": {"wlive": {"sd_link": "IND"}}})
_ts("wlive-active-link", "/cards/wlive/$actlink", "PAR", {"cards": {"wlive": {"active_link": "PAR"}}})
_ts("wlive-battery", "/cards/wlive/$battstate", "GOOD", {"cards": {"wlive": {"battery": "GOOD"}}})
_ts("wlive-auto-input", "/cards/wlive/autoin", "OFF", {"cards": {"wlive": {"auto_input": "OFF"}}})
_ti("wlive-meters", "/cards/wlive/meters", 1, {"cards": {"wlive": {"meters": True}}})
_ts("wlive-when-stopped", "/cards/wlive/auto_stop", "KEEP", {"cards": {"wlive": {"when_stopped": "KEEP"}}})
_ts("wlive-when-playing", "/cards/wlive/auto_play", "ALT", {"cards": {"wlive": {"when_playing": "ALT"}}})
_ts("wlive-when-recording", "/cards/wlive/auto_rec", "MAIN", {"cards": {"wlive": {"when_recording": "MAIN"}}})


def _sd(n, **kv):
    return {"cards": {"wlive": {"sd": {str(n): kv}}}}


_ts("wlive-record-tracks", "/cards/wlive/1/cfg/rectracks", "32", _sd(1, record_tracks="32"))
_ts("wlive-play-mode", "/cards/wlive/2/cfg/playmode", "LOOP", _sd(2, play_mode="LOOP"))
_ts("wlive-state", "/cards/wlive/1/$stat/state", "PLAY", _sd(1, state="PLAY"))
_tf("wlive-time", "/cards/wlive/1/$stat/etime", "00:01:00", 0.0017, 60000.0, _sd(1, time=60000.0))
_tf("wlive-sd-free", "/cards/wlive/2/$stat/sdfree", "10:00:00", 1.0, 36000000.0, _sd(2, sd_free=36000000.0))
_ti("wlive-sd-size", "/cards/wlive/2/$stat/sdsize", 64, _sd(2, sd_size=64))
_ts("wlive-sd-state", "/cards/wlive/2/$stat/sdstate", "READY", _sd(2, sd_state="READY"))
_ts("wlive-session-list", "/cards/wlive/1/$stat/sessionlist", "2020-04-04 10:16:36", _sd(1, first_session_date="2020-04-04 10:16:36"))
_ts("wlive-marker-list", "/cards/wlive/1/$stat/markerlist", "00:00:10", _sd(1, marker_time="00:00:10"))
_ts("wlive-session-names", "/cards/wlive/1/$stat/snamelist", "CC Mr Jones", _sd(1, first_session_name="CC Mr Jones"))
_ti("wlive-sessions", "/cards/wlive/1/$stat/sessions", 12, _sd(1, sessions=12))
_ti("wlive-markers", "/cards/wlive/1/$stat/markers", 3, _sd(1, markers=3))
_tf("wlive-session-length", "/cards/wlive/1/$stat/sessionlen", "01:00:00", 0.1, 3600000.0, _sd(1, session_length=3600000.0))
_ti("wlive-session", "/cards/wlive/1/$stat/sessionpos", 5, _sd(1, session=5))
_ti("wlive-marker", "/cards/wlive/1/$stat/markerpos", 1, _sd(1, marker=1))
_ts("wlive-tracks", "/cards/wlive/1/$stat/tracks", "32", _sd(1, tracks="32"))
_ts("wlive-rate", "/cards/wlive/1/$stat/rate", "48", _sd(1, rate="48"))
_ti("wlive-linked-session", "/cards/wlive/2/$stat/linkedpos", 5, _sd(2, linked_session=5))
_tf("wlive-start", "/cards/wlive/2/$stat/start", "00:00:01", 0.00003, 1000.0, _sd(2, start=1000.0))
_tf("wlive-stop", "/cards/wlive/2/$stat/stop", "00:00:05", 0.00014, 5000.0, _sd(2, stop=5000.0))
_ts("wlive-error", "/cards/wlive/1/$stat/errormessage", "NO CARD", _sd(1, error="NO CARD"))
_ti("wlive-error-code", "/cards/wlive/1/$stat/errorcode", 2, _sd(1, error_code=2))

# ── Custom controls, layers, GPIO, DAW, MIDI, OSC lock, safes (p.73-88,
# button appendix p.145-150) ────────────────────────────────────────────
binary(W, "select_custom_control_page", {"page": 16}, osc("/$ctl/user/sel", ("i", 16)))
binary(W, "set_custom_button_mode", {"mode": "2TRK"}, osc("/$ctl/user/mode", ("s", "2TRK")))
binary(W, "set_custom_channel_mode", {"mode": "PAN"}, osc("/$ctl/user/cmode", ("s", "PAN")))
binary(W, "set_custom_control_function", {"bank": "3", "position": 2, "control": "enc", "function": "SD A"},
       osc("/$ctl/user/3/2/enc/mode", ("s", "SD A")))
binary(W, "set_custom_control_name", {"bank": "daw1", "position": 1, "control": "bu", "name": "STOP"},
       osc("/$ctl/user/daw1/1/bu/name", ("s", "STOP")))
binary(W, "set_custom_control_parameter", {"bank": "gpio", "position": 4, "control": "bu", "parameter": "dca", "value": "DCA.3"},
       osc("/$ctl/user/gpio/4/bu/dca", ("s", "DCA.3")))
binary(W, "set_custom_control_parameter_number", {"bank": "MM", "position": 1, "control": "bu", "parameter": "ch", "value": 76},
       osc("/$ctl/user/MM/1/bu/ch", ("i", 76)))
binary(W, "set_custom_led", {"bank": "16", "position": 4, "enabled": True}, osc("/$ctl/user/16/4/led", ("i", 1)))
binary(W, "set_custom_led_color", {"bank": "D4", "position": 1, "color": 9}, osc("/$ctl/user/D4/1/col", ("i", 9)))
binary(W, "set_custom_knob_send", {"knob": 3, "send": 24}, osc("/$ctl/user/cuser/3", ("i", 24)))
binary(W, "select_layer", {"section": "C", "layer": 22}, osc("/$ctl/layer/C/sel", ("i", 22)))
binary(W, "set_layer_spill", {"section": "R", "group": 63}, osc("/$ctl/layer/R/spidx", ("i", 63)))
binary(W, "set_layer_offset", {"section": "L", "layer": 1, "offset": 4}, osc("/$ctl/layer/L/1/ofs", ("i", 4)))
binary(W, "set_layer_name", {"section": "CMPCT", "layer": 6, "name": "VOX"}, osc("/$ctl/layer/CMPCT/6/name", ("s", "VOX")))
binary(W, "set_layer_strip", {"section": "L", "layer": 6, "position": 24, "type": "SEND", "index": 3, "destination": 28},
       [osc("/$ctl/layer/L/6/24/type", ("s", "SEND")), osc("/$ctl/layer/L/6/24/i", ("i", 3)), osc("/$ctl/layer/L/6/24/dst", ("i", 28))])
binary(W, "set_layer_strip_midi_value", {"section": "R", "layer": 7, "position": 1, "value": 127},
       osc("/$ctl/layer/R/7/1/val", ("i", 127)))
binary(W, "set_gpio_mode", {"gpio": 4, "mode": "OUTNC"}, osc("/$ctl/gpio/4/mode", ("s", "OUTNC")))
binary(W, "set_gpio_state", {"gpio": 1, "closed": True}, osc("/$ctl/gpio/1/gpstate", ("i", 1)))
binary(W, "set_daw", {"enabled": True}, osc("/$ctl/daw/on", ("i", 1)))
binary(W, "set_daw_connection", {"connection": "DIN"}, osc("/$ctl/daw/conn", ("s", "DIN")))
binary(W, "set_daw_emulation", {"emulation": "HUI"}, osc("/$ctl/daw/emul", ("s", "HUI")))
binary(W, "set_daw_configuration", {"configuration": "MSTR2EXT"}, osc("/$ctl/daw/config", ("s", "MSTR2EXT")))
binary(W, "set_daw_upper_cc", {"enabled": False}, osc("/$ctl/daw/ccup", ("i", 0)))
binary(W, "set_daw_disable_wheel", {"enabled": True}, osc("/$ctl/daw/disjog", ("i", 1)))
binary(W, "set_daw_preset", {"preset": "reaper"}, osc("/$ctl/daw/preset", ("s", "reaper")))
binary(W, "set_daw_switch", {"enabled": False}, osc("/$ctl/daw/$on", ("i", 0)))
binary(W, "set_daw_button_page", {"page": 4}, osc("/$ctl/daw/$bpage", ("i", 4)))
binary(W, "set_daw_button_option", {"option": "$btnrecrdy", "enabled": True}, osc("/$ctl/daw/$btnrecrdy", ("i", 1)))
binary(W, "set_midi_port", {"function": "enscenes", "port": "USB"}, osc("/$ctl/midi/enscenes", ("s", "USB")))
binary(W, "set_osc_read_only", {"enabled": False}, osc("/$ctl/OSC/ronly", ("i", 0)))
binary(W, "set_safes", {"section": "source/LCL", "safes": "++  "}, osc("/$ctl/safes/source/LCL", ("s", "++  ")))
binary(W, "set_safes", {"section": "ch", "safes": "+"}, osc("/$ctl/safes/ch", ("s", "+")), file="set_safes-ch")

_ti("custom-page", "/$ctl/user/sel", 0, {"custom_controls": {"page": 1}})
_ts("custom-button-mode", "/$ctl/user/mode", "MGRP", {"custom_controls": {"button_mode": "MGRP"}})
_ts("custom-channel-mode", "/$ctl/user/cmode", "HA", {"custom_controls": {"channel_mode": "HA"}})
_ts("custom-function", "/$ctl/user/daw2/3/bd/mode", "DAWBTN", {"custom_controls": {"daw2": {"3": {"bd": {"function": "DAWBTN"}}}}})
_ts("custom-name", "/$ctl/user/12/1/enc/name", "|*Vox", {"custom_controls": {"12": {"1": {"enc": {"name": "|*Vox"}}}}})
_ts("custom-label", "/$ctl/user/U2/4/bu/$fname", "MUTE CH1", {"custom_controls": {"U2": {"4": {"bu": {"label": "MUTE CH1"}}}}})
_ts("custom-parameter", "/$ctl/user/user/2/bu/btn", "T1", {"custom_controls": {"user": {"2": {"bu": {"params": {"btn": "T1"}}}}}})
_ti("custom-parameter-number", "/$ctl/user/1/1/bu/ch", 5, {"custom_controls": {"1": {"1": {"bu": {"params": {"ch": "5"}}}}}})
_ti("custom-led", "/$ctl/user/1/2/led", 1, {"custom_controls": {"1": {"2": {"led": True}}}})
_ti("custom-led-color", "/$ctl/user/D1/3/col", 4, {"custom_controls": {"D1": {"3": {"color": 4}}}})
_ti("custom-knob", "/$ctl/user/cuser/2", 17, {"custom_controls": {"knobs": {"2": 17}}})
_ti("layer-selected", "/$ctl/layer/L/sel", 6, {"layers": {"L": {"selected": 6}}})
_ti("layer-spill", "/$ctl/layer/RCK/spidx", 61, {"layers": {"RCK": {"spill": 61}}})
_ti("layer-offset", "/$ctl/layer/C/9/ofs", 8, {"layers": {"C": {"9": {"offset": 8}}}})
_ts("layer-name", "/$ctl/layer/R/1/name", "MAIN", {"layers": {"R": {"1": {"name": "MAIN"}}}})
_ts("layer-strip-type", "/$ctl/layer/L/6/12/type", "DCA", {"layers": {"L": {"6": {"strips": {"12": {"type": "DCA"}}}}}})
_ti("layer-strip-index", "/$ctl/layer/CMPCT/7/1/i", 17, {"layers": {"CMPCT": {"7": {"strips": {"1": {"index": 17}}}}}})
_ti("layer-strip-destination", "/$ctl/layer/C/5/16/dst", 2, {"layers": {"C": {"5": {"strips": {"16": {"destination": 2}}}}}})
_ti("layer-strip-value", "/$ctl/layer/R/6/3/val", 64, {"layers": {"R": {"6": {"strips": {"3": {"value": 64}}}}}})
_ts("gpio-mode", "/$ctl/gpio/2/mode", "INNO", {"gpio": {"2": {"mode": "INNO"}}})
_ti("gpio-closed", "/$ctl/gpio/3/gpstate", 1, {"gpio": {"3": {"closed": True}}})
_ti("gpio-input-state", "/$ctl/gpio/1/$state", 0, {"gpio": {"1": {"input_state": False}}})
_ti("daw-on", "/$ctl/daw/on", 1, {"daw": {"on": True}})
_ts("daw-connection", "/$ctl/daw/conn", "USB", {"daw": {"connection": "USB"}})
_ts("daw-emulation", "/$ctl/daw/emul", "MCU", {"daw": {"emulation": "MCU"}})
_ts("daw-configuration", "/$ctl/daw/config", "MSTR", {"daw": {"configuration": "MSTR"}})
_ti("daw-upper-cc", "/$ctl/daw/ccup", 0, {"daw": {"upper_cc": False}})
_ti("daw-disable-wheel", "/$ctl/daw/disjog", 1, {"daw": {"disable_wheel": True}})
_ts("daw-preset", "/$ctl/daw/preset", "-", {"daw": {"preset": "-"}})
_ti("daw-switch", "/$ctl/daw/$on", 1, {"daw": {"switch": True}})
_ti("daw-button-page", "/$ctl/daw/$bpage", 2, {"daw": {"button_page": 2}})
_ti("daw-button-option", "/$ctl/daw/$btnvpot", 1, {"daw": {"buttons": {"btnvpot": True}}})
_ts("midi-port", "/$ctl/midi/enchctl", "DIN", {"midi": {"enchctl": "DIN"}})
_ts("safes-section", "/$ctl/safes/dca", "+ + ", {"safes": {"dca": "+ + "}})
_ts("safes-source", "/$ctl/safes/source/USR", "+", {"safes": {"sources": {"USR": "+"}}})
_ts("safes-output", "/$ctl/safes/output/REC", " +", {"safes": {"outputs": {"REC": " +"}}})
_ts("safes-area", "/$ctl/safes/area/COMPACT", "+", {"safes": {"areas": {"COMPACT": "+"}}})
