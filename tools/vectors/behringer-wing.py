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
binary(W, "set_node", {"node": "ch/1", "assignments": "fdr=4,mute=1"}, osc("/ch/1", ("s", "fdr=4,mute=1")),
       device_reply_hex=hexs(osc("/ch/1*", ("s", "OK"))), expect_result={"ok": {"kind": "ack"}})
binary(W, "set_node", {"assignments": "/ch.1.fdr=-1,mute=0,.2.fdr=0,mute=1"},
       osc("/", ("s", "/ch.1.fdr=-1,mute=0,.2.fdr=0,mute=1")),
       device_reply_hex=hexs(osc("/*", ("s", "OK"))), expect_result={"ok": {"kind": "ack"}}, file="set_node-root")
_ts("param-enum", "/$ctl/user/1/1/enc/mode", "FX", {})
_tf("param-float", "/cfg/mon/1/dim", "-20.0", 0.5, -20.0, {})
_ti("param-int", "/ch/40/in/set/srcauto", 1, {})

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
