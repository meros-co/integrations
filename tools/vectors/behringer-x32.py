# Behringer X32 / Midas M32: OSC over UDP 10023. Addresses from Maillot,
# "Unofficial X32/M32 OSC Remote Protocol" 4.02-01: strips p.25-38, /headamp
# p.42, /load p.50-51, /-stat p.60-66, /-action p.67-68, /fx p.39, user
# controls p.23-24 and p.116-119. Floats are 0-1; mix/on is 1 = passing audio.
X = "behringer-x32"

# Strips with a number: (command stem, parameter, wire prefix for number n).
_X32_NUMBERED = [
    ("channel", "channel", lambda n: f"/ch/{n:02d}"),
    ("aux", "aux", lambda n: f"/auxin/{n:02d}"),
    ("fx_return", "fx_return", lambda n: f"/fxrtn/{n:02d}"),
    ("bus", "bus", lambda n: f"/bus/{n:02d}"),
    ("matrix", "matrix", lambda n: f"/mtx/{n:02d}"),
]
# Every numbered strip and both mains: fader, mute, name, colour, icon.
for stem, p, pre in _X32_NUMBERED:
    binary(X, f"set_{stem}_fader", {p: 6, "level": 0.75}, osc(f"{pre(6)}/mix/fader", ("f", 0.75)))
    binary(X, f"mute_{stem}", {p: 6, "muted": True}, osc(f"{pre(6)}/mix/on", ("i", 0)),
           expect_result={"ok": {"kind": "unverified"}})
    binary(X, f"get_{stem}_fader", {p: 2}, osc(f"{pre(2)}/mix/fader"),
           device_reply_hex=hexs(osc(f"{pre(2)}/mix/fader", ("f", 0.5))),
           expect_result={"ok": {"kind": "value", "value": 0.5}})
    binary(X, f"get_{stem}_on", {p: 2}, osc(f"{pre(2)}/mix/on"),
           device_reply_hex=hexs(osc(f"{pre(2)}/mix/on", ("i", 1))),
           expect_result={"ok": {"kind": "value", "value": 1}})
    binary(X, f"get_{stem}_name", {p: 1}, osc(f"{pre(1)}/config/name"),
           device_reply_hex=hexs(osc(f"{pre(1)}/config/name", ("s", "Kick"))),
           expect_result={"ok": {"kind": "value", "value": "Kick"}})
    binary(X, f"set_{stem}_name", {p: 3, "name": "Vox"}, osc(f"{pre(3)}/config/name", ("s", "Vox")))
    binary(X, f"set_{stem}_color", {p: 4, "color": 11}, osc(f"{pre(4)}/config/color", ("i", 11)))
    binary(X, f"set_{stem}_icon", {p: 5, "icon": 74}, osc(f"{pre(5)}/config/icon", ("i", 74)))
for stem, pre in [("main", "/main/st"), ("mono", "/main/m")]:
    binary(X, f"set_{stem}_fader", {"level": 1.0}, osc(f"{pre}/mix/fader", ("f", 1.0)))
    binary(X, f"mute_{stem}", {"muted": False}, osc(f"{pre}/mix/on", ("i", 1)))
    binary(X, f"get_{stem}_fader", {}, osc(f"{pre}/mix/fader"),
           device_reply_hex=hexs(osc(f"{pre}/mix/fader", ("f", 0.75))),
           expect_result={"ok": {"kind": "value", "value": 0.75}})
    binary(X, f"get_{stem}_on", {}, osc(f"{pre}/mix/on"))
    binary(X, f"get_{stem}_name", {}, osc(f"{pre}/config/name"),
           device_reply_hex=hexs(osc(f"{pre}/config/name", ("s", "PA"))),
           expect_result={"ok": {"kind": "value", "value": "PA"}})
    binary(X, f"set_{stem}_name", {"name": "PA"}, osc(f"{pre}/config/name", ("s", "PA")))
    binary(X, f"set_{stem}_color", {"color": 7}, osc(f"{pre}/config/color", ("i", 7)))
    binary(X, f"set_{stem}_icon", {"icon": 1}, osc(f"{pre}/config/icon", ("i", 1)))
    binary(X, f"set_{stem}_matrix_send_level", {"matrix": 6, "level": 0.25}, osc(f"{pre}/mix/06/level", ("f", 0.25)))
    binary(X, f"set_{stem}_matrix_send_on", {"matrix": 1, "enabled": True}, osc(f"{pre}/mix/01/on", ("i", 1)))
    binary(X, f"set_{stem}_eq_on", {"enabled": False}, osc(f"{pre}/eq/on", ("i", 0)))
    binary(X, f"set_{stem}_dyn_on", {"enabled": True}, osc(f"{pre}/dyn/on", ("i", 1)))
    binary(X, f"set_{stem}_insert_on", {"enabled": True}, osc(f"{pre}/insert/on", ("i", 1)))
    binary(X, f"set_{stem}_insert_select", {"insert": 22}, osc(f"{pre}/insert/sel", ("i", 22)))
binary(X, "set_main_pan", {"pan": 0.5}, osc("/main/st/mix/pan", ("f", 0.5)))

# Pan: channels, aux inputs, FX returns, buses (matrices have none, p.33).
binary(X, "set_channel_pan", {"channel": 2, "pan": 0.75}, osc("/ch/02/mix/pan", ("f", 0.75)))
binary(X, "set_aux_pan", {"aux": 8, "pan": 0.0}, osc("/auxin/08/mix/pan", ("f", 0.0)))
binary(X, "set_fx_return_pan", {"fx_return": 1, "pan": 1.0}, osc("/fxrtn/01/mix/pan", ("f", 1.0)))
binary(X, "set_bus_pan", {"bus": 16, "pan": 0.25}, osc("/bus/16/mix/pan", ("f", 0.25)))

# Sends to mix buses 01-16 from inputs (p.27, p.28, p.30).
for stem, p, pre in _X32_NUMBERED[:3]:
    binary(X, f"set_{stem}_send_level", {p: 1, "bus": 16, "level": 0.75}, osc(f"{pre(1)}/mix/16/level", ("f", 0.75)))
    binary(X, f"set_{stem}_send_on", {p: 8, "bus": 3, "enabled": False}, osc(f"{pre(8)}/mix/03/on", ("i", 0)))
    binary(X, f"get_{stem}_send_level", {p: 1, "bus": 9}, osc(f"{pre(1)}/mix/09/level"),
           device_reply_hex=hexs(osc(f"{pre(1)}/mix/09/level", ("f", 0.5))),
           expect_result={"ok": {"kind": "value", "value": 0.5}})
# Buses send to matrices 01-06 (p.32).
binary(X, "set_bus_matrix_send_level", {"bus": 12, "matrix": 2, "level": 0.5}, osc("/bus/12/mix/02/level", ("f", 0.5)))
binary(X, "set_bus_matrix_send_on", {"bus": 1, "matrix": 6, "enabled": True}, osc("/bus/01/mix/06/on", ("i", 1)))

# Main / mono assignment, mono level, DCA and mute-group bitmaps.
for stem, p, pre in _X32_NUMBERED[:4]:
    binary(X, f"assign_{stem}_to_main", {p: 7, "assigned": False}, osc(f"{pre(7)}/mix/st", ("i", 0)))
    binary(X, f"assign_{stem}_to_mono", {p: 7, "assigned": True}, osc(f"{pre(7)}/mix/mono", ("i", 1)))
    binary(X, f"set_{stem}_mono_level", {p: 7, "level": 0.0}, osc(f"{pre(7)}/mix/mlevel", ("f", 0.0)))
    binary(X, f"set_{stem}_dca_groups", {p: 2, "groups": 129}, osc(f"{pre(2)}/grp/dca", ("i", 129)))
    binary(X, f"set_{stem}_mute_groups", {p: 2, "groups": 63}, osc(f"{pre(2)}/grp/mute", ("i", 63)))

# Processing switches.
binary(X, "set_channel_eq_on", {"channel": 32, "enabled": True}, osc("/ch/32/eq/on", ("i", 1)))
binary(X, "set_aux_eq_on", {"aux": 1, "enabled": False}, osc("/auxin/01/eq/on", ("i", 0)))
binary(X, "set_fx_return_eq_on", {"fx_return": 2, "enabled": True}, osc("/fxrtn/02/eq/on", ("i", 1)))
binary(X, "set_bus_eq_on", {"bus": 10, "enabled": True}, osc("/bus/10/eq/on", ("i", 1)))
binary(X, "set_matrix_eq_on", {"matrix": 3, "enabled": True}, osc("/mtx/03/eq/on", ("i", 1)))
binary(X, "set_channel_dyn_on", {"channel": 1, "enabled": True}, osc("/ch/01/dyn/on", ("i", 1)))
binary(X, "set_bus_dyn_on", {"bus": 4, "enabled": False}, osc("/bus/04/dyn/on", ("i", 0)))
binary(X, "set_matrix_dyn_on", {"matrix": 6, "enabled": True}, osc("/mtx/06/dyn/on", ("i", 1)))
binary(X, "set_channel_gate_on", {"channel": 9, "enabled": True}, osc("/ch/09/gate/on", ("i", 1)))
binary(X, "set_channel_insert_on", {"channel": 3, "enabled": True}, osc("/ch/03/insert/on", ("i", 1)))
binary(X, "set_bus_insert_on", {"bus": 3, "enabled": False}, osc("/bus/03/insert/on", ("i", 0)))
binary(X, "set_matrix_insert_on", {"matrix": 1, "enabled": True}, osc("/mtx/01/insert/on", ("i", 1)))
binary(X, "set_channel_insert_select", {"channel": 3, "insert": 1}, osc("/ch/03/insert/sel", ("i", 1)))
binary(X, "set_bus_insert_select", {"bus": 3, "insert": 0}, osc("/bus/03/insert/sel", ("i", 0)))
binary(X, "set_matrix_insert_select", {"matrix": 1, "insert": 17}, osc("/mtx/01/insert/sel", ("i", 17)))
binary(X, "set_channel_trim", {"channel": 4, "trim": 0.5}, osc("/ch/04/preamp/trim", ("f", 0.5)))
binary(X, "set_aux_trim", {"aux": 4, "trim": 1.0}, osc("/auxin/04/preamp/trim", ("f", 1.0)))
binary(X, "set_channel_invert", {"channel": 4, "inverted": True}, osc("/ch/04/preamp/invert", ("i", 1)))
binary(X, "set_aux_invert", {"aux": 4, "inverted": False}, osc("/auxin/04/preamp/invert", ("i", 0)))
binary(X, "set_channel_source", {"channel": 17, "source": 64}, osc("/ch/17/config/source", ("i", 64)))
binary(X, "set_aux_source", {"aux": 7, "source": 39}, osc("/auxin/07/config/source", ("i", 39)))

# DCAs: single-digit numbers, on/fader directly under /dca/N (p.38).
binary(X, "set_dca_fader", {"dca": 2, "level": 0.5}, osc("/dca/2/fader", ("f", 0.5)))
binary(X, "mute_dca", {"dca": 2, "muted": False}, osc("/dca/2/on", ("i", 1)))
binary(X, "get_dca_fader", {"dca": 8}, osc("/dca/8/fader"))
binary(X, "get_dca_on", {"dca": 8}, osc("/dca/8/on"),
       device_reply_hex=hexs(osc("/dca/8/on", ("i", 0))), expect_result={"ok": {"kind": "value", "value": 0}})
binary(X, "get_dca_name", {"dca": 1}, osc("/dca/1/config/name"))
binary(X, "set_dca_name", {"dca": 1, "name": "Band"}, osc("/dca/1/config/name", ("s", "Band")))
binary(X, "set_dca_color", {"dca": 1, "color": 2}, osc("/dca/1/config/color", ("i", 2)))
binary(X, "set_dca_icon", {"dca": 1, "icon": 12}, osc("/dca/1/config/icon", ("i", 12)))

# Solo switches: /-stat/solosw/01-80 (p.64).
binary(X, "solo_channel", {"channel": 5, "soloed": True}, osc("/-stat/solosw/05", ("i", 1)))
binary(X, "solo_aux", {"aux": 1, "soloed": True}, osc("/-stat/solosw/33", ("i", 1)))
binary(X, "solo_fx_return", {"fx_return": 8, "soloed": True}, osc("/-stat/solosw/48", ("i", 1)))
binary(X, "solo_bus", {"bus": 1, "soloed": False}, osc("/-stat/solosw/49", ("i", 0)))
binary(X, "solo_matrix", {"matrix": 6, "soloed": True}, osc("/-stat/solosw/70", ("i", 1)))
binary(X, "solo_main", {"soloed": True}, osc("/-stat/solosw/71", ("i", 1)))
binary(X, "solo_mono", {"soloed": True}, osc("/-stat/solosw/72", ("i", 1)))
binary(X, "solo_dca", {"dca": 8, "soloed": True}, osc("/-stat/solosw/80", ("i", 1)))
binary(X, "clear_solo", {}, osc("/-action/clearsolo", ("i", 1)))
binary(X, "get_solo_active", {}, osc("/-stat/solo"),
       device_reply_hex=hexs(osc("/-stat/solo", ("i", 1))), expect_result={"ok": {"kind": "value", "value": 1}})

# Mute groups: /config/mute/1-6, 1 = engaged (p.21).
binary(X, "mute_group", {"group": 6, "muted": True}, osc("/config/mute/6", ("i", 1)))
binary(X, "get_mute_group", {"group": 1}, osc("/config/mute/1"))

# Headamps: three-digit index 000-127 (p.42); numbered from 1 here.
binary(X, "set_headamp_gain", {"headamp": 1, "gain": 0.5}, osc("/headamp/000/gain", ("f", 0.5)))
binary(X, "set_headamp_phantom", {"headamp": 128, "enabled": True}, osc("/headamp/127/phantom", ("i", 1)))
binary(X, "get_headamp_gain", {"headamp": 33}, osc("/headamp/032/gain"))
binary(X, "get_headamp_phantom", {"headamp": 10}, osc("/headamp/009/phantom"),
       device_reply_hex=hexs(osc("/headamp/009/phantom", ("i", 1))), expect_result={"ok": {"kind": "value", "value": 1}})

# Talkback (p.21, p.64).
binary(X, "set_talkback", {"talkback": "A", "enabled": True}, osc("/-stat/talk/A", ("i", 1)))
binary(X, "get_talkback", {"talkback": "B"}, osc("/-stat/talk/B"))
binary(X, "set_talkback_level", {"talkback": "B", "level": 0.75}, osc("/config/talk/B/level", ("f", 0.75)))
binary(X, "set_talkback_latch", {"talkback": "A", "latched": False}, osc("/config/talk/A/latch", ("i", 0)))
binary(X, "set_talkback_destinations", {"talkback": "A", "destinations": 262143},
       osc("/config/talk/A/destmap", ("i", 262143)))
binary(X, "set_talkback_source", {"source": 1}, osc("/config/talk/source", ("i", 1)))
binary(X, "set_oscillator", {"enabled": False}, osc("/-stat/osc/on", ("i", 0)))

# Scenes, snippets, cues (p.44-51, p.68).
binary(X, "recall_scene", {"scene": 12}, osc("/-action/goscene", ("i", 12)),
       expect_result={"ok": {"kind": "unverified"}})
binary(X, "recall_snippet", {"snippet": 0}, osc("/-action/gosnippet", ("i", 0)))
binary(X, "recall_cue", {"cue": 99}, osc("/-action/gocue", ("i", 99)))
binary(X, "load_scene", {"scene": 99}, osc("/load", ("s", "scene"), ("i", 99)),
       device_reply_hex=hexs(osc("/load", ("s", "scene"), ("i", 1))),
       expect_result={"ok": {"kind": "value", "value": 1}})
binary(X, "load_snippet", {"snippet": 3}, osc("/load", ("s", "snippet"), ("i", 3)))
binary(X, "get_scene_name", {"scene": 5}, osc("/-show/showfile/scene/005/name"),
       device_reply_hex=hexs(osc("/-show/showfile/scene/005/name", ("s", "Act 2"))),
       expect_result={"ok": {"kind": "value", "value": "Act 2"}})
binary(X, "get_snippet_name", {"snippet": 42}, osc("/-show/showfile/snippet/042/name"))
binary(X, "get_cue_name", {"cue": 0}, osc("/-show/showfile/cue/000/name"))
binary(X, "get_show_position", {}, osc("/-show/prepos/current"))
binary(X, "create_undo_point", {}, osc("/-action/undopt", ("i", 1)))
binary(X, "undo", {}, osc("/-action/doundo", ("i", 1)))

# Effects (p.39).
binary(X, "set_fx_type", {"slot": 1, "type": 0}, osc("/fx/1/type", ("i", 0)))
binary(X, "get_fx_type", {"slot": 8}, osc("/fx/8/type"))
binary(X, "set_fx_parameter", {"slot": 4, "parameter": 23, "value": 0.5}, osc("/fx/4/par/23", ("f", 0.5)))
# The document's own example reply, p.12: /fx/4/par/23 ,f 0.5.
binary(X, "get_fx_parameter", {"slot": 4, "parameter": 23}, osc("/fx/4/par/23"),
       device_reply_hex="2f66782f342f7061722f3233000000002c6600003f000000",
       expect_result={"ok": {"kind": "value", "value": 0.5}})

# User controls (p.23-24, p.60-65, p.116-119).
binary(X, "set_user_button", {"set": "A", "button": 5, "assignment": "O00"},
       osc("/config/userctrl/A/btn/5", ("s", "O00")))
binary(X, "set_user_encoder", {"set": "C", "encoder": 4, "assignment": "F70"},
       osc("/config/userctrl/C/enc/4", ("s", "F70")))
binary(X, "set_user_set_color", {"set": "B", "color": 4}, osc("/config/userctrl/B/color", ("i", 4)))
binary(X, "select_user_set", {"set": 2}, osc("/-stat/userbank", ("i", 2)))
binary(X, "set_user_control_value", {"control": 25, "value": 127}, osc("/-stat/userpar/25/value", ("i", 127)))

# Identity: /info answers ,ssss version, server name, model, firmware (p.11).
binary(X, "get_console_model", {}, osc("/info"),
       device_reply_hex=hexs(osc("/info", ("s", "V2.05"), ("s", "osc-server"), ("s", "X32C"), ("s", "2.12"))),
       expect_result={"ok": {"kind": "value", "value": "X32C"}})
binary(X, "get_firmware_version", {}, osc("/info"),
       device_reply_hex=hexs(osc("/info", ("s", "V2.05"), ("s", "osc-server"), ("s", "X32"), ("s", "4.02"))),
       expect_result={"ok": {"kind": "value", "value": "4.02"}})

# ── Telemetry ─────────────────────────────────────────────────────────────
# /xremote on connect, then the first of the one-at-a-time queries for current
# values; pushed changes arrive on the same addresses as sets (p.9-10).
telemetry(X, "channel-mute", expect_connect_wire_hex=[hexs(osc("/xremote")), hexs(osc("/ch/01/mix/on"))],
          inbound_hex=hexs(osc("/ch/07/mix/on", ("i", 0))),
          expect_state={"channels": {"7": {"mute": True}}})
telemetry(X, "channel-name", inbound_hex=hexs(osc("/ch/12/config/name", ("s", "Vox"))),
          expect_state={"channels": {"12": {"name": "Vox"}}})
telemetry(X, "main-fader", inbound_hex=hexs(osc("/main/st/mix/fader", ("f", 0.5))),
          expect_state={"main": {"fader": 0.5}})
# The document's example change notifications, p.10.
telemetry(X, "solo-switch", inbound_hex=hexs(osc("/-stat/solosw/01", ("i", 1))),
          expect_state={"channels": {"1": {"solo": True}}})
telemetry(X, "solo-active", inbound_hex=hexs(osc("/-stat/solo", ("i", 1))), expect_state={"solo_active": True})
telemetry(X, "channel-send-level", inbound_hex=hexs(osc("/ch/03/mix/16/level", ("f", 0.25))),
          expect_state={"channels": {"3": {"sends": {"16": {"level": 0.25}}}}})
telemetry(X, "channel-send-on", inbound_hex=hexs(osc("/ch/03/mix/02/on", ("i", 0))),
          expect_state={"channels": {"3": {"sends": {"2": {"on": False}}}}})
telemetry(X, "channel-pan", inbound_hex=hexs(osc("/ch/02/mix/pan", ("f", 0.75))),
          expect_state={"channels": {"2": {"pan": 0.75}}})
telemetry(X, "channel-color", inbound_hex=hexs(osc("/ch/05/config/color", ("i", 3))),
          expect_state={"channels": {"5": {"color": 3}}})
telemetry(X, "channel-main-assign", inbound_hex=hexs(osc("/ch/05/mix/st", ("i", 1))),
          expect_state={"channels": {"5": {"main_assign": True}}})
telemetry(X, "channel-dca-groups", inbound_hex=hexs(osc("/ch/05/grp/dca", ("i", 5))),
          expect_state={"channels": {"5": {"dca_groups": 5}}})
telemetry(X, "aux-mute", inbound_hex=hexs(osc("/auxin/08/mix/on", ("i", 1))),
          expect_state={"aux": {"8": {"mute": False}}})
telemetry(X, "fx-return-fader", inbound_hex=hexs(osc("/fxrtn/02/mix/fader", ("f", 0.25))),
          expect_state={"fx_returns": {"2": {"fader": 0.25}}})
telemetry(X, "bus-name", inbound_hex=hexs(osc("/bus/16/config/name", ("s", "Wedge 1"))),
          expect_state={"buses": {"16": {"name": "Wedge 1"}}})
telemetry(X, "bus-matrix-send", inbound_hex=hexs(osc("/bus/04/mix/06/level", ("f", 0.75))),
          expect_state={"buses": {"4": {"matrix_sends": {"6": {"level": 0.75}}}}})
telemetry(X, "matrix-mute", inbound_hex=hexs(osc("/mtx/06/mix/on", ("i", 0))),
          expect_state={"matrices": {"6": {"mute": True}}})
telemetry(X, "mono-fader", inbound_hex=hexs(osc("/main/m/mix/fader", ("f", 0.75))),
          expect_state={"mono": {"fader": 0.75}})
telemetry(X, "main-matrix-send-on", inbound_hex=hexs(osc("/main/st/mix/03/on", ("i", 1))),
          expect_state={"main": {"matrix_sends": {"3": {"on": True}}}})
telemetry(X, "dca-mute", inbound_hex=hexs(osc("/dca/3/on", ("i", 0))),
          expect_state={"dcas": {"3": {"mute": True}}})
telemetry(X, "dca-name", inbound_hex=hexs(osc("/dca/3/config/name", ("s", "Drums"))),
          expect_state={"dcas": {"3": {"name": "Drums"}}})
telemetry(X, "solo-aux", inbound_hex=hexs(osc("/-stat/solosw/33", ("i", 1))),
          expect_state={"aux": {"1": {"solo": True}}})
telemetry(X, "solo-bus", inbound_hex=hexs(osc("/-stat/solosw/64", ("i", 1))),
          expect_state={"buses": {"16": {"solo": True}}})
telemetry(X, "solo-main", inbound_hex=hexs(osc("/-stat/solosw/71", ("i", 0))),
          expect_state={"main": {"solo": False}})
telemetry(X, "solo-dca", inbound_hex=hexs(osc("/-stat/solosw/73", ("i", 1))),
          expect_state={"dcas": {"1": {"solo": True}}})
telemetry(X, "mute-group", inbound_hex=hexs(osc("/config/mute/4", ("i", 1))),
          expect_state={"mute_groups": {"4": {"muted": True}}})
telemetry(X, "headamp-gain", inbound_hex=hexs(osc("/headamp/000/gain", ("f", 0.5))),
          expect_state={"headamps": {"1": {"gain": 0.5}}})
telemetry(X, "headamp-phantom", inbound_hex=hexs(osc("/headamp/031/phantom", ("i", 1))),
          expect_state={"headamps": {"32": {"phantom": True}}})
telemetry(X, "talkback", inbound_hex=hexs(osc("/-stat/talk/A", ("i", 1))),
          expect_state={"talkback": {"A": {"on": True}}})
telemetry(X, "show-position", inbound_hex=hexs(osc("/-show/prepos/current", ("i", 4))),
          expect_state={"show": {"position": 4}})
telemetry(X, "scene-name", inbound_hex=hexs(osc("/-show/showfile/scene/012/name", ("s", "Act 2"))),
          expect_state={"scenes": {"12": {"name": "Act 2"}}})
telemetry(X, "fx-parameter", inbound_hex="2f66782f342f7061722f3233000000002c6600003f000000",
          expect_state={"fx": {"4": {"parameters": {"23": 0.5}}}})
telemetry(X, "user-control", inbound_hex=hexs(osc("/-stat/userpar/01/value", ("i", 127))),
          expect_state={"user_controls": {"1": {"value": 127}}})


# ── Levels in dB ──────────────────────────────────────────────────────────
# Maillot p.128, both directions, written here from the document's C-like
# code rather than from the spec's points.
def _x32_db(f):
    if f >= 0.5:
        return f * 40.0 - 30.0
    if f >= 0.25:
        return f * 80.0 - 50.0
    if f >= 0.0625:
        return f * 160.0 - 70.0
    return f * 480.0 - 90.0


def _x32_position(d):
    if d < -60.0:
        return (d + 90.0) / 480.0
    if d < -30.0:
        return (d + 70.0) / 160.0
    if d < -10.0:
        return (d + 50.0) / 80.0
    return (d + 30.0) / 40.0


def _fader_db_vectors(spec_id, db=-20.0):
    """A vector for every *_db command, from its 0-1 sibling's vector: the
    same address with the level given in dB, or the same reply read as dB;
    and every telemetry vector's fader and send levels also stated in dB."""
    commands = yaml.safe_load((ROOT / "specs" / f"{spec_id}.yaml").read_text(encoding="utf-8"))["commands"]
    have = {v["command"] for v in V if v.get("spec") == spec_id and "command" in v}
    for v in list(V):
        if v.get("spec") != spec_id or "command" not in v:
            continue
        name = v["command"] + "_db"
        if name not in commands or name in have:
            continue
        have.add(name)
        wire = bytes.fromhex(v["expect_wire_hex"])
        address = wire[:wire.index(0)].decode()
        new = {"spec": spec_id, "command": name, "input": dict(v["input"])}
        if "level" in new["input"]:
            del new["input"]["level"]
            new["input"]["level_db"] = db
            new["expect_wire_hex"] = hexs(osc(address, ("f", _x32_position(db))))
        else:
            new["expect_wire_hex"] = v["expect_wire_hex"]
            if "device_reply_hex" in v:
                reply = bytes.fromhex(v["device_reply_hex"])
                (position,) = struct.unpack(">f", reply[-4:])
                new["device_reply_hex"] = v["device_reply_hex"]
                new["expect_result"] = {"ok": {"kind": "value", "value": _x32_db(position)}}
        V.append(new)

    def add_db(node, trail):
        if not isinstance(node, dict):
            return
        for key in list(node):
            value = node[key]
            if (key in ("fader", "level", "mono_level") and isinstance(value, float)
                    and trail[:1] not in (["talkback"], ["monitor"])):
                node[key + "_db"] = round(_x32_db(value), 1)
            else:
                add_db(value, trail + [key])

    for v in V:
        if v.get("spec") == spec_id and "expect_state" in v:
            add_db(v["expect_state"], [])


_fader_db_vectors(X)
telemetry(X, "channel-fader-db", inbound_hex=hexs(osc("/ch/05/mix/fader", ("f", 0.375))),
          expect_state={"channels": {"5": {"fader": 0.375, "fader_db": -20.0}}})
telemetry(X, "dca-fader-minus-infinity", inbound_hex=hexs(osc("/dca/1/fader", ("f", 0.0))),
          expect_state={"dcas": {"1": {"fader": 0.0, "fader_db": -90.0}}})


# ── Any parameter by its address ─────────────────────────────────────────
# Set: the address with one argument of the type chosen; get: the bare
# address, answered on it with the value (p.8). /node with the node as a
# string is answered on "node" (no leading /) with one line of text; a /
# set is echoed back (p.78).
binary(X, "set_parameter_float", {"path": "ch/01/mix/fader", "value": 0.75}, osc("/ch/01/mix/fader", ("f", 0.75)),
       expect_result={"ok": {"kind": "unverified"}})
binary(X, "set_parameter_int", {"path": "ch/01/mix/on", "value": 0}, osc("/ch/01/mix/on", ("i", 0)))
binary(X, "set_parameter_string", {"path": "ch/01/config/name", "value": "Vox"},
       osc("/ch/01/config/name", ("s", "Vox")))
binary(X, "get_parameter", {"path": "ch/01/mix/fader"}, osc("/ch/01/mix/fader"),
       device_reply_hex=hexs(osc("/ch/01/mix/fader", ("f", 0.5))),
       expect_result={"ok": {"kind": "value", "value": 0.5}})
binary(X, "get_node", {"node": "headamp/124"}, osc("/node", ("s", "headamp/124")),
       device_reply_hex=hexs(osc("node", ("s", "/headamp/124 +0.0 OFF\n"))),
       expect_result={"ok": {"kind": "value", "value": "/headamp/124 +0.0 OFF\n"}})
binary(X, "set_node", {"text": "ch/01/config Vox 1 RD 1"}, osc("/", ("s", "ch/01/config Vox 1 RD 1")),
       device_reply_hex=hexs(osc("/", ("s", "ch/01/config Vox 1 RD 1"))),
       expect_result={"ok": {"kind": "ack"}})
telemetry(X, "param-float", inbound_hex=hexs(osc("/ch/01/gate/thr", ("f", 0.5))),
          expect_state={"channels": {"1": {"gate": {"threshold": 0.5}}}})
telemetry(X, "param-int", inbound_hex=hexs(osc("/-prefs/rta/peakhold", ("i", 1))), expect_state={})
telemetry(X, "param-string", inbound_hex=hexs(osc("/-prefs/style", ("s", "Patrick"))), expect_state={})


# ── EQ bands (p.26-37) ────────────────────────────────────────────────────
# /<strip>/eq/<band>/type ,i; f, g and q ,f as the 0-1 wire position.
_X32_EQ = [("channel", "channel", "/ch/05", 4, "channels", "5"),
           ("aux", "aux", "/auxin/02", 4, "aux", "2"),
           ("fx_return", "fx_return", "/fxrtn/08", 4, "fx_returns", "8"),
           ("bus", "bus", "/bus/16", 6, "buses", "16"),
           ("matrix", "matrix", "/mtx/06", 6, "matrices", "6"),
           ("main", None, "/main/st", 6, "main", None),
           ("mono", None, "/main/m", 6, "mono", None)]
for stem, p, pre, nb, key, n in _X32_EQ:
    idx = {p: int(n)} if p else {}
    top = 13 if stem in ("matrix", "main", "mono") else 5
    binary(X, f"set_{stem}_eq_band_type", {**idx, "band": nb, "type": top}, osc(f"{pre}/eq/{nb}/type", ("i", top)))
    binary(X, f"set_{stem}_eq_band_frequency", {**idx, "band": 1, "frequency": 0.25}, osc(f"{pre}/eq/1/f", ("f", 0.25)))
    binary(X, f"set_{stem}_eq_band_gain", {**idx, "band": 2, "gain": 0.75}, osc(f"{pre}/eq/2/g", ("f", 0.75)))
    binary(X, f"set_{stem}_eq_band_q", {**idx, "band": 3, "q": 0.5}, osc(f"{pre}/eq/3/q", ("f", 0.5)))
    tkey = f"{stem.replace('_', '-')}-eq-band"
    for wire, field, val, t in [("type", "type", 2, "i"), ("f", "frequency", 0.5, "f"),
                                ("g", "gain", 0.25, "f"), ("q", "q", 1.0, "f")]:
        st = {"eq": {"bands": {"2": {field: val}}}}
        telemetry(X, f"{tkey}-{field}", inbound_hex=hexs(osc(f"{pre}/eq/2/{wire}", (t, val))),
                  expect_state={key: {n: st}} if n else {key: st})


# ── Dynamics and gate (p.25-37) ───────────────────────────────────────────
# /<strip>/dyn/... and /ch/NN/gate/...: enumerations and switches ,i, the
# rest ,f as the 0-1 wire position.
_X32_DYN_STRIPS = [("channel", "channel", "/ch/07", "channels", "7"), ("bus", "bus", "/bus/03", "buses", "3"),
                   ("matrix", "matrix", "/mtx/02", "matrices", "2"), ("main", None, "/main/st", "main", None),
                   ("mono", None, "/main/m", "mono", None)]
# (command key, address leaf, wire type, value sent, value pushed back)
_X32_DYN = [("mode", "mode", "i", 1, 0), ("detector", "det", "i", 1, 1), ("envelope", "env", "i", 0, 1),
            ("threshold", "thr", "f", 0.5, 0.25), ("ratio", "ratio", "i", 11, 6), ("knee", "knee", "f", 0.2, 0.25),
            ("makeup_gain", "mgain", "f", 0.125, 0.5), ("attack", "attack", "f", 0.25, 0.75),
            ("hold", "hold", "f", 0.5, 0.5), ("release", "release", "f", 1.0, 0.0),
            ("position", "pos", "i", 1, 0), ("key_source", "keysrc", "i", 64, 33), ("mix", "mix", "f", 0.5, 1.0),
            ("auto", "auto", "b", True, 1), ("filter_on", "filter/on", "b", False, 0),
            ("filter_type", "filter/type", "i", 8, 4), ("filter_frequency", "filter/f", "f", 0.75, 0.5)]
_X32_GATE = [("mode", "mode", "i", 3, 4), ("threshold", "thr", "f", 0.5, 0.75), ("range", "range", "f", 0.25, 1.0),
             ("attack", "attack", "f", 0.0, 0.5), ("hold", "hold", "f", 0.5, 0.25), ("release", "release", "f", 0.75, 0.5),
             ("key_source", "keysrc", "i", 1, 64), ("filter_on", "filter/on", "b", True, 1),
             ("filter_type", "filter/type", "i", 0, 8), ("filter_frequency", "filter/f", "f", 0.25, 1.0)]


def _x32_section(stem, p, pre, key, n, section, table):
    idx = {p: int(n)} if p else {}
    for cmd, leaf, t, sent, pushed in table:
        if t == "b":
            binary(X, f"set_{stem}_{section}_{cmd}", {**idx, "enabled": sent},
                   osc(f"{pre}/{section}/{leaf}", ("i", 1 if sent else 0)))
            value = pushed == 1
            wire = ("i", pushed)
        else:
            binary(X, f"set_{stem}_{section}_{cmd}", {**idx, "value": sent}, osc(f"{pre}/{section}/{leaf}", (t, sent)))
            value = pushed
            wire = (t, pushed)
        st = {section: {cmd: value}}
        telemetry(X, f"{stem.replace('_', '-')}-{section}-{cmd.replace('_', '-')}",
                  inbound_hex=hexs(osc(f"{pre}/{section}/{leaf}", wire)),
                  expect_state={key: {n: st}} if n else {key: st})


for stem, p, pre, key, n in _X32_DYN_STRIPS:
    _x32_section(stem, p, pre, key, n, "dyn", _X32_DYN)
_x32_section("channel", "channel", "/ch/12", "channels", "12", "gate", _X32_GATE)


# ── Preamp low cut, delay, automix, insert position (p.24-37) ─────────────
binary(X, "set_channel_low_cut", {"channel": 3, "enabled": True}, osc("/ch/03/preamp/hpon", ("i", 1)))
binary(X, "set_channel_low_cut_slope", {"channel": 3, "slope": 2}, osc("/ch/03/preamp/hpslope", ("i", 2)))
binary(X, "set_channel_low_cut_frequency", {"channel": 3, "frequency": 0.25}, osc("/ch/03/preamp/hpf", ("f", 0.25)))
binary(X, "set_channel_delay", {"channel": 32, "enabled": False}, osc("/ch/32/delay/on", ("i", 0)))
binary(X, "set_channel_delay_time", {"channel": 32, "time": 0.5}, osc("/ch/32/delay/time", ("f", 0.5)))
binary(X, "set_channel_automix_group", {"channel": 8, "group": 2}, osc("/ch/08/automix/group", ("i", 2)))
binary(X, "set_channel_automix_weight", {"channel": 8, "weight": 0.5}, osc("/ch/08/automix/weight", ("f", 0.5)))
binary(X, "set_automix_enable", {"group": "Y", "enabled": True}, osc("/config/amixenable/Y", ("i", 1)))
for stem, p, pre in [("channel", "channel", "/ch/02"), ("bus", "bus", "/bus/09"), ("matrix", "matrix", "/mtx/04"),
                     ("main", None, "/main/st"), ("mono", None, "/main/m")]:
    binary(X, f"set_{stem}_insert_position", {**({p: int(pre[-2:])} if p else {}), "position": 1},
           osc(f"{pre}/insert/pos", ("i", 1)))
telemetry(X, "channel-low-cut", inbound_hex=hexs(osc("/ch/03/preamp/hpon", ("i", 1))),
          expect_state={"channels": {"3": {"low_cut": True}}})
telemetry(X, "channel-low-cut-slope", inbound_hex=hexs(osc("/ch/03/preamp/hpslope", ("i", 1))),
          expect_state={"channels": {"3": {"low_cut_slope": 1}}})
telemetry(X, "channel-low-cut-frequency", inbound_hex=hexs(osc("/ch/03/preamp/hpf", ("f", 0.5))),
          expect_state={"channels": {"3": {"low_cut_frequency": 0.5}}})
telemetry(X, "channel-delay-on", inbound_hex=hexs(osc("/ch/04/delay/on", ("i", 1))),
          expect_state={"channels": {"4": {"delay_on": True}}})
telemetry(X, "channel-delay-time", inbound_hex=hexs(osc("/ch/04/delay/time", ("f", 0.25))),
          expect_state={"channels": {"4": {"delay_time": 0.25}}})
telemetry(X, "channel-automix-group", inbound_hex=hexs(osc("/ch/01/automix/group", ("i", 1))),
          expect_state={"channels": {"1": {"automix_group": 1}}})
telemetry(X, "channel-automix-weight", inbound_hex=hexs(osc("/ch/01/automix/weight", ("f", 0.75))),
          expect_state={"channels": {"1": {"automix_weight": 0.75}}})
telemetry(X, "automix-enable", inbound_hex=hexs(osc("/config/amixenable/X", ("i", 1))),
          expect_state={"automix": {"X": {"enabled": True}}})
for stem, pre, key, n in [("channel", "/ch/02", "channels", "2"), ("bus", "/bus/09", "buses", "9"),
                          ("matrix", "/mtx/04", "matrices", "4"), ("main", "/main/st", "main", None),
                          ("mono", "/main/m", "mono", None)]:
    st = {"insert_position": 0}
    telemetry(X, f"{stem}-insert-position", inbound_hex=hexs(osc(f"{pre}/insert/pos", ("i", 0))),
              expect_state={key: {n: st}} if n else {key: st})

# ── Send pan, tap and pan follow on the odd send of a pair (p.26-37) ──────
for stem, cstem, p, pre, dp, key, n, skey in [
        ("channel", "send", "channel", "/ch/10", "bus", "channels", "10", "sends"),
        ("aux", "send", "aux", "/auxin/01", "bus", "aux", "1", "sends"),
        ("fx_return", "send", "fx_return", "/fxrtn/01", "bus", "fx_returns", "1", "sends"),
        ("bus", "matrix_send", "bus", "/bus/16", "matrix", "buses", "16", "matrix_sends"),
        ("main", "matrix_send", None, "/main/st", "matrix", "main", None, "matrix_sends"),
        ("mono", "matrix_send", None, "/main/m", "matrix", "mono", None, "matrix_sends")]:
    idx = {p: int(n)} if p else {}
    last = 15 if dp == "bus" else 5
    binary(X, f"set_{stem}_{cstem}_pan", {**idx, dp: last, "pan": 0.25}, osc(f"{pre}/mix/{last:02d}/pan", ("f", 0.25)))
    binary(X, f"set_{stem}_{cstem}_tap", {**idx, dp: 1, "tap": 3}, osc(f"{pre}/mix/01/type", ("i", 3)))
    binary(X, f"set_{stem}_{cstem}_pan_follow", {**idx, dp: 3, "enabled": True}, osc(f"{pre}/mix/03/panFollow", ("i", 1)))
    for leaf, field, wire, value in [("pan", "pan", ("f", 0.75), 0.75), ("type", "tap", ("i", 4), 4),
                                     ("panFollow", "pan_follow", ("i", 0), False)]:
        st = {skey: {"5": {field: value}}}
        telemetry(X, f"{stem.replace('_', '-')}-{cstem.replace('_', '-')}-{field.replace('_', '-')}",
                  inbound_hex=hexs(osc(f"{pre}/mix/05/{leaf}", wire)),
                  expect_state={key: {n: st}} if n else {key: st})


# ── Routing and output patching (p.22-24, p.40-42) ────────────────────────
binary(X, "set_routing_mode", {"mode": 1}, osc("/config/routing/routswitch", ("i", 1)))
binary(X, "set_routing_input", {"block": "9-16", "source": 23}, osc("/config/routing/IN/9-16", ("i", 23)))
binary(X, "set_routing_input_aux", {"source": 15}, osc("/config/routing/IN/AUX", ("i", 15)))
binary(X, "set_routing_playback", {"block": "25-32", "source": 0}, osc("/config/routing/PLAY/25-32", ("i", 0)))
binary(X, "set_routing_playback_aux", {"source": 3}, osc("/config/routing/PLAY/AUX", ("i", 3)))
binary(X, "set_routing_aes50", {"port": "B", "block": "41-48", "source": 35}, osc("/config/routing/AES50B/41-48", ("i", 35)))
binary(X, "set_routing_card", {"block": "1-8", "source": 20}, osc("/config/routing/CARD/1-8", ("i", 20)))
binary(X, "set_routing_output", {"block": "13-16", "source": 24}, osc("/config/routing/OUT/13-16", ("i", 24)))
binary(X, "set_user_routing_output", {"slot": 48, "source": 208}, osc("/config/userrout/out/48", ("i", 208)))
binary(X, "set_user_routing_input", {"slot": 1, "source": 168}, osc("/config/userrout/in/01", ("i", 168)))
for stem, kind, n in [("output", "main", 16), ("aux_output", "aux", 6), ("p16_output", "p16", 16),
                      ("aes_output", "aes", 2), ("rec_output", "rec", 2)]:
    binary(X, f"set_{stem}_source", {"output": n, "source": 76}, osc(f"/outputs/{kind}/{n:02d}/src", ("i", 76)))
    binary(X, f"set_{stem}_tap", {"output": 1, "tap": 8}, osc(f"/outputs/{kind}/01/pos", ("i", 8)))
    if kind != "rec":
        binary(X, f"set_{stem}_invert", {"output": 2, "inverted": True}, osc(f"/outputs/{kind}/02/invert", ("i", 1)))
binary(X, "set_output_delay", {"output": 5, "enabled": True}, osc("/outputs/main/05/delay/on", ("i", 1)))
binary(X, "set_output_delay_time", {"output": 5, "time": 0.5}, osc("/outputs/main/05/delay/time", ("f", 0.5)))
binary(X, "set_p16_output_iq_group", {"output": 3, "group": 2}, osc("/outputs/p16/03/iQ/group", ("i", 2)))
binary(X, "set_p16_output_iq_speaker", {"output": 3, "speaker": 6}, osc("/outputs/p16/03/iQ/speaker", ("i", 6)))
binary(X, "set_p16_output_iq_eq", {"output": 3, "eq": 4}, osc("/outputs/p16/03/iQ/eq", ("i", 4)))
binary(X, "set_p16_output_iq_model", {"output": 3, "model": 7}, osc("/outputs/p16/03/iQ/model", ("i", 7)))
telemetry(X, "routing-mode", inbound_hex=hexs(osc("/config/routing/routswitch", ("i", 0))), expect_state={"routing": {"mode": 0}})
telemetry(X, "routing-input", inbound_hex=hexs(osc("/config/routing/IN/17-24", ("i", 2))),
          expect_state={"routing": {"input": {"17-24": 2}}})
telemetry(X, "routing-playback-aux", inbound_hex=hexs(osc("/config/routing/PLAY/AUX", ("i", 0))),
          expect_state={"routing": {"playback": {"AUX": 0}}})
telemetry(X, "routing-aes50", inbound_hex=hexs(osc("/config/routing/AES50A/33-40", ("i", 26))),
          expect_state={"routing": {"aes50": {"A": {"33-40": 26}}}})
telemetry(X, "routing-card", inbound_hex=hexs(osc("/config/routing/CARD/25-32", ("i", 3))),
          expect_state={"routing": {"card": {"25-32": 3}}})
telemetry(X, "routing-output", inbound_hex=hexs(osc("/config/routing/OUT/5-8", ("i", 1))),
          expect_state={"routing": {"outputs": {"5-8": 1}}})
telemetry(X, "user-routing-out", inbound_hex=hexs(osc("/config/userrout/out/12", ("i", 207))),
          expect_state={"routing": {"user_out": {"12": 207}}})
telemetry(X, "user-routing-in", inbound_hex=hexs(osc("/config/userrout/in/32", ("i", 1))),
          expect_state={"routing": {"user_in": {"32": 1}}})
for leaf, field, wire, value in [("src", "source", ("i", 4), 4), ("pos", "tap", ("i", 6), 6), ("invert", "invert", ("i", 1), True),
                                 ("delay/on", "delay_on", ("i", 0), False), ("delay/time", "delay_time", ("f", 0.25), 0.25),
                                 ("iQ/group", "iq_group", ("i", 1), 1), ("iQ/speaker", "iq_speaker", ("i", 2), 2),
                                 ("iQ/eq", "iq_eq", ("i", 3), 3), ("iQ/model", "iq_model", ("i", 0), 0)]:
    kind = "p16" if leaf.startswith("iQ") else "main"
    telemetry(X, f"output-{field.replace('_', '-')}", inbound_hex=hexs(osc(f"/outputs/{kind}/16/{leaf}", wire)),
              expect_state={"outputs": {kind: {"16": {field: value}}}})


# ── Monitor, solo, talkback, oscillator, links, DP48 (p.20-24) ────────────
# (command, input, address, wire argument, state key under monitor, pushed value, state value)
for cmd, inp, leaf, wire, key, pushed, value in [
        ("set_monitor_level", {"level": 0.75}, "level", ("f", 0.75), "level", ("f", 0.5), 0.5),
        ("set_monitor_source", {"source": 6}, "source", ("i", 6), "source", ("i", 3), 3),
        ("set_monitor_source_trim", {"trim": 0.5}, "sourcetrim", ("f", 0.5), "source_trim", ("f", 1.0), 1.0),
        ("set_monitor_channel_mode", {"mode": 1}, "chmode", ("i", 1), "channel_mode", ("i", 0), 0),
        ("set_monitor_bus_mode", {"mode": 0}, "busmode", ("i", 0), "bus_mode", ("i", 1), 1),
        ("set_monitor_dca_mode", {"mode": 1}, "dcamode", ("i", 1), "dca_mode", ("i", 1), 1),
        ("set_solo_exclusive", {"enabled": True}, "exclusive", ("i", 1), "exclusive", ("i", 1), True),
        ("set_solo_follows_select", {"enabled": False}, "followsel", ("i", 0), "follows_select", ("i", 1), True),
        ("set_select_follows_solo", {"enabled": True}, "followsolo", ("i", 1), "select_follows_solo", ("i", 0), False),
        ("set_monitor_dim", {"enabled": True}, "dim", ("i", 1), "dim", ("i", 1), True),
        ("set_monitor_mono", {"enabled": False}, "mono", ("i", 0), "mono", ("i", 0), False),
        ("set_monitor_delay", {"enabled": True}, "delay", ("i", 1), "delay", ("i", 1), True),
        ("set_monitor_master_control", {"enabled": True}, "masterctrl", ("i", 1), "master_control", ("i", 0), False),
        ("set_monitor_pfl_dim", {"enabled": True}, "dimpfl", ("i", 1), "pfl_dim", ("i", 1), True),
        ("mute_monitor", {"enabled": True}, "mute", ("i", 1), "mute", ("i", 1), True),
        ("set_monitor_dim_gain", {"attenuation": 0.25}, "dimatt", ("f", 0.25), "dim_gain", ("f", 0.75), 0.75),
        ("set_monitor_delay_time", {"time": 1.0}, "delaytime", ("f", 1.0), "delay_time", ("f", 0.0), 0.0)]:
    binary(X, cmd, inp, osc(f"/config/solo/{leaf}", wire))
    telemetry(X, f"monitor-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"/config/solo/{leaf}", pushed)),
              expect_state={"monitor": {key: value}})
binary(X, "set_talkback_enable", {"enabled": True}, osc("/config/talk/enable", ("i", 1)))
binary(X, "set_talkback_dim", {"talkback": "B", "enabled": False}, osc("/config/talk/B/dim", ("i", 0)))
telemetry(X, "talkback-enable", inbound_hex=hexs(osc("/config/talk/enable", ("i", 0))), expect_state={"talkback": {"enabled": False}})
telemetry(X, "talkback-dim", inbound_hex=hexs(osc("/config/talk/A/dim", ("i", 1))), expect_state={"talkback": {"A": {"dim": True}}})
telemetry(X, "talkback-latch", inbound_hex=hexs(osc("/config/talk/B/latch", ("i", 1))), expect_state={"talkback": {"B": {"latch": True}}})
telemetry(X, "talkback-destinations", inbound_hex=hexs(osc("/config/talk/A/destmap", ("i", 3))),
          expect_state={"talkback": {"A": {"destinations": 3}}})
telemetry(X, "talkback-source", inbound_hex=hexs(osc("/config/talk/source", ("i", 1))), expect_state={"talkback": {"source": 1}})
binary(X, "set_oscillator_level", {"level": 0.5}, osc("/config/osc/level", ("f", 0.5)))
binary(X, "set_oscillator_frequency", {"slot": "f2", "frequency": 0.5}, osc("/config/osc/f2", ("f", 0.5)))
binary(X, "set_oscillator_frequency_select", {"selection": 1}, osc("/config/osc/fsel", ("i", 1)))
binary(X, "set_oscillator_type", {"type": 1}, osc("/config/osc/type", ("i", 1)))
binary(X, "set_oscillator_destination", {"destination": 25}, osc("/config/osc/dest", ("i", 25)))
telemetry(X, "oscillator-level", inbound_hex=hexs(osc("/config/osc/level", ("f", 0.25))), expect_state={"oscillator": {"level": 0.25}})
telemetry(X, "oscillator-frequency", inbound_hex=hexs(osc("/config/osc/f1", ("f", 0.75))),
          expect_state={"oscillator": {"frequencies": {"f1": 0.75}}})
telemetry(X, "oscillator-frequency-select", inbound_hex=hexs(osc("/config/osc/fsel", ("i", 0))),
          expect_state={"oscillator": {"frequency_select": 0}})
telemetry(X, "oscillator-type", inbound_hex=hexs(osc("/config/osc/type", ("i", 2))), expect_state={"oscillator": {"type": 2}})
telemetry(X, "oscillator-destination", inbound_hex=hexs(osc("/config/osc/dest", ("i", 18))),
          expect_state={"oscillator": {"destination": 18}})
binary(X, "set_mono_mode", {"mode": 1}, osc("/config/mono/mode", ("i", 1)))
binary(X, "set_mono_link", {"enabled": True}, osc("/config/mono/link", ("i", 1)))
telemetry(X, "mono-mode", inbound_hex=hexs(osc("/config/mono/mode", ("i", 0))), expect_state={"mono": {"mode": 0}})
telemetry(X, "mono-link", inbound_hex=hexs(osc("/config/mono/link", ("i", 0))), expect_state={"mono": {"link": False}})
for stem, leaf, pair, key in [("channel", "chlink", "31-32", "channel_links"), ("aux", "auxlink", "7-8", "aux_links"),
                              ("fx_return", "fxlink", "1-2", "fx_return_links"), ("bus", "buslink", "15-16", "bus_links"),
                              ("matrix", "mtxlink", "5-6", "matrix_links")]:
    binary(X, f"set_{stem}_link", {"pair": pair, "enabled": True}, osc(f"/config/{leaf}/{pair}", ("i", 1)))
    telemetry(X, f"{stem.replace('_', '-')}-link", inbound_hex=hexs(osc(f"/config/{leaf}/{pair}", ("i", 1))),
              expect_state={key: {pair: True}})
binary(X, "set_link_preference", {"element": "fdrmute", "enabled": False}, osc("/config/linkcfg/fdrmute", ("i", 0)))
telemetry(X, "link-preference", inbound_hex=hexs(osc("/config/linkcfg/eq", ("i", 1))), expect_state={"link_preferences": {"eq": True}})
binary(X, "set_dp48_assign", {"channel": 48, "group": 12}, osc("/config/dp48/assign/48", ("i", 12)))
binary(X, "set_dp48_group_name", {"group": 1, "name": "Drums"}, osc("/config/dp48/grpname/01", ("s", "Drums")))
binary(X, "set_dp48_scope", {"scope": 15}, osc("/config/dp48/scope", ("i", 15)))
binary(X, "broadcast_dp48", {}, osc("/config/dp48/broadcast", ("i", 1)))
telemetry(X, "dp48-assign", inbound_hex=hexs(osc("/config/dp48/assign/07", ("i", 3))), expect_state={"dp48": {"assign": {"7": 3}}})
telemetry(X, "dp48-group-name", inbound_hex=hexs(osc("/config/dp48/grpname/12", ("s", "Vox"))),
          expect_state={"dp48": {"group_names": {"12": "Vox"}}})
telemetry(X, "dp48-scope", inbound_hex=hexs(osc("/config/dp48/scope", ("i", 3))), expect_state={"dp48": {"scope": 3}})


# ── Show, scene, snippet, cue and library management (p.43-52) ────────────
# /save, /load, /rename, /delete, /copy and /add are answered on their own
# address with ,si <type> <status>: the document's examples, p.49-52.
def _op(cmd, inp, verb, args, kind, status=1):
    binary(X, cmd, inp, osc(f"/{verb}", *args), device_reply_hex=hexs(osc(f"/{verb}", ("s", kind), ("i", status))),
           expect_result={"ok": {"kind": "value", "value": status}})


_op("save_scene", {"index": 45, "name": "test", "note": "note"}, "save",
    [("s", "scene"), ("i", 45), ("s", "test"), ("s", "note")], "scene")
_op("save_snippet", {"index": 0, "name": "Aaa"}, "save", [("s", "snippet"), ("i", 0), ("s", "Aaa")], "snippet")
_op("save_channel_preset", {"index": 3, "name": "Kick", "strip": 1}, "save",
    [("s", "libchan"), ("i", 3), ("s", "Kick"), ("i", 0)], "libchan")
_op("save_fx_preset", {"index": 99, "name": "Hall", "slot": 8}, "save", [("s", "libfx"), ("i", 99), ("s", "Hall"), ("i", 7)], "libfx")
_op("save_routing_preset", {"index": 1, "name": "Tour"}, "save", [("s", "librout"), ("i", 1), ("s", "Tour")], "librout", 0)
_op("save_dp48_preset", {"index": 2, "name": "Band"}, "save", [("s", "libmon"), ("i", 2), ("s", "Band")], "libmon")
_op("load_channel_preset", {"index": 3, "strip": 72, "scope": 63}, "load",
    [("s", "libchan"), ("i", 3), ("i", 71), ("i", 63)], "libchan")
_op("load_fx_preset", {"index": 5, "slot": 1}, "load", [("s", "libfx"), ("i", 5), ("i", 0)], "libfx")
_op("load_routing_preset", {"index": 0}, "load", [("s", "librout"), ("i", 0)], "librout")
_op("load_dp48_preset", {"index": 9}, "load", [("s", "libmon"), ("i", 9)], "libmon")
_op("rename_library_item", {"kind": "scene", "index": 99, "name": "myScene"}, "rename",
    [("s", "scene"), ("i", 99), ("s", "myScene")], "scene")
_op("delete_library_item", {"kind": "scene", "index": 99}, "delete", [("s", "scene"), ("i", 99)], "scene")
_op("copy_library_item", {"kind": "libchan", "source": 45, "destination": 48}, "copy",
    [("s", "libchan"), ("i", 45), ("i", 48)], "libchan")
_op("add_cue", {"number": 1252, "name": "Ccc"}, "add", [("s", "cue"), ("i", 1252), ("s", "Ccc")], "cue")
binary(X, "set_show_name", {"name": "MyShow"}, osc("/-show/showfile/show/name", ("s", "MyShow")))
binary(X, "set_scene_safes", {"group": "inputs", "bitmap": 36}, osc("/-show/showfile/show/inputs", ("i", 36)))
for cmd, inp, leaf, wire in [("set_cue_number", {"number": 10327}, "numb", ("i", 10327)),
                             ("set_cue_name", {"name": "Intro"}, "name", ("s", "Intro")),
                             ("set_cue_skip", {"skip": True}, "skip", ("i", 1)),
                             ("set_cue_scene", {"scene": -1}, "scene", ("i", -1)),
                             ("set_cue_snippet", {"snippet": 3}, "bit", ("i", 3)),
                             ("set_cue_midi_type", {"type": 1}, "miditype", ("i", 1)),
                             ("set_cue_midi_channel", {"channel": 16}, "midichan", ("i", 16)),
                             ("set_cue_midi_param1", {"value": 127}, "midipara1", ("i", 127)),
                             ("set_cue_midi_param2", {"value": 0}, "midipara2", ("i", 0))]:
    binary(X, cmd, {"index": 2, **inp}, osc(f"/-show/showfile/cue/002/{leaf}", wire))
binary(X, "set_scene_name", {"index": 1, "name": "AAA"}, osc("/-show/showfile/scene/001/name", ("s", "AAA")))
binary(X, "set_scene_notes", {"index": 1, "notes": "aaa"}, osc("/-show/showfile/scene/001/notes", ("s", "aaa")))
binary(X, "set_scene_safe_groups", {"index": 1, "bitmap": 262}, osc("/-show/showfile/scene/001/safes", ("i", 262)))
binary(X, "set_snippet_name", {"index": 0, "name": "Aaa"}, osc("/-show/showfile/snippet/000/name", ("s", "Aaa")))
binary(X, "set_snippet_filters", {"index": 0, "bitmap": 1}, osc("/-show/showfile/snippet/000/eventtyp", ("i", 1)))
binary(X, "set_snippet_channels", {"index": 0, "bitmap": -1}, osc("/-show/showfile/snippet/000/channels", ("i", -1)))
binary(X, "set_snippet_aux_buses", {"index": 0, "bitmap": 65536}, osc("/-show/showfile/snippet/000/auxbuses", ("i", 65536)))
binary(X, "set_snippet_main_groups", {"index": 0, "bitmap": 32768}, osc("/-show/showfile/snippet/000/maingrps", ("i", 32768)))
telemetry(X, "show-name", inbound_hex=hexs(osc("/-show/showfile/show/name", ("s", "Sunday"))), expect_state={"show": {"name": "Sunday"}})
telemetry(X, "show-safes", inbound_hex=hexs(osc("/-show/showfile/show/effects", ("i", 255))),
          expect_state={"show": {"safes": {"effects": 255}}})
for leaf, key, wire, value in [("numb", "number", ("i", 200), 200), ("skip", "skip", ("i", 1), True), ("scene", "scene", ("i", 2), 2),
                               ("bit", "snippet", ("i", -1), -1), ("miditype", "midi_type", ("i", 3), 3),
                               ("midichan", "midi_channel", ("i", 1), 1), ("midipara1", "midi_param1", ("i", 64), 64),
                               ("midipara2", "midi_param2", ("i", 127), 127)]:
    telemetry(X, f"cue-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"/-show/showfile/cue/099/{leaf}", wire)),
              expect_state={"cues": {"99": {key: value}}})
telemetry(X, "scene-notes", inbound_hex=hexs(osc("/-show/showfile/scene/002/notes", ("s", "bbb"))),
          expect_state={"scenes": {"2": {"notes": "bbb"}}})
telemetry(X, "scene-safes", inbound_hex=hexs(osc("/-show/showfile/scene/002/safes", ("i", 2))), expect_state={"scenes": {"2": {"safes": 2}}})
telemetry(X, "scene-has-data", inbound_hex=hexs(osc("/-show/showfile/scene/002/hasdata", ("i", 1))),
          expect_state={"scenes": {"2": {"has_data": True}}})
for leaf, key in [("eventtyp", "filters"), ("channels", "channels"), ("auxbuses", "aux_buses"), ("maingrps", "main_groups")]:
    telemetry(X, f"snippet-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"/-show/showfile/snippet/000/{leaf}", ("i", 3))),
              expect_state={"snippets": {"0": {key: 3}}})
telemetry(X, "snippet-has-data", inbound_hex=hexs(osc("/-show/showfile/snippet/000/hasdata", ("i", 0))),
          expect_state={"snippets": {"0": {"has_data": False}}})
for lib, key in [("ch", "channel"), ("fx", "fx"), ("r", "routing"), ("mon", "dp48")]:
    telemetry(X, f"preset-{key}-name", inbound_hex=hexs(osc(f"/-libs/{lib}/001/name", ("s", "Preset"))),
              expect_state={"presets": {key: {"1": {"name": "Preset"}}}})
    telemetry(X, f"preset-{key}-has-data", inbound_hex=hexs(osc(f"/-libs/{lib}/100/hasdata", ("i", 1))),
              expect_state={"presets": {key: {"100": {"has_data": True}}}})


# ── Effects sources, graphic EQ, preferences, status, recorders (p.24, p.39,
# p.55-70, p.101) ──────────────────────────────────────────────────────────
binary(X, "set_fx_source", {"slot": 4, "side": "r", "source": 17}, osc("/fx/4/source/r", ("i", 17)))
binary(X, "set_fx_parameter_int", {"slot": 2, "parameter": 5, "value": 1}, osc("/fx/2/par/05", ("i", 1)))
binary(X, "set_geq_band", {"slot": 1, "band": 31, "gain": 0.5}, osc("/fx/1/par/31", ("f", 0.5)))
binary(X, "set_geq_band_b", {"slot": 1, "band": 1, "gain": 0.75}, osc("/fx/1/par/33", ("f", 0.75)))
binary(X, "set_geq_master", {"slot": 8, "level": 0.5}, osc("/fx/8/par/32", ("f", 0.5)))
binary(X, "set_geq_master_b", {"slot": 8, "level": 0.25}, osc("/fx/8/par/64", ("f", 0.25)))
telemetry(X, "fx-source", inbound_hex=hexs(osc("/fx/1/source/l", ("i", 1))), expect_state={"fx": {"1": {"source": {"l": 1}}}})
binary(X, "set_console_name", {"name": "FOH"}, osc("/-prefs/name", ("s", "FOH")))
telemetry(X, "console-name", inbound_hex=hexs(osc("/-prefs/name", ("s", "X32-02-4A-53"))), expect_state={"console": {"name": "X32-02-4A-53"}})
binary(X, "set_preference_switch", {"preference": "scene_advance", "enabled": True}, osc("/-prefs/scene_advance", ("i", 1)))
telemetry(X, "preference-switch", inbound_hex=hexs(osc("/-prefs/hardmute", ("i", 1))), expect_state={"preferences": {"switches": {"hardmute": True}}})
for cmd, inp, leaf, wire, pushed, value in [
        ("set_show_control", {"mode": 1}, "show_control", ("i", 1), ("i", 2), 2),
        ("set_sample_rate", {"rate": 0}, "clockrate", ("i", 0), ("i", 1), 1),
        ("set_clock_source", {"source": 3}, "clocksource", ("i", 3), ("i", 1), 1),
        ("set_clock_mode", {"mode": 1}, "clockmode", ("i", 1), ("i", 0), 0),
        ("set_mute_led_mode", {"mode": 1}, "invertmutes", ("i", 1), ("i", 1), 1),
        ("set_recorder_type", {"type": 1}, "rec_control", ("i", 1), ("i", 0), 0),
        ("set_headamp_flags", {"flags": 15}, "haflags", ("i", 15), ("i", 2), 2),
        ("set_screen_brightness", {"value": 1.0}, "bright", ("f", 1.0), ("f", 0.5), 0.5),
        ("set_lcd_contrast", {"value": 0.5}, "lcdcont", ("f", 0.5), ("f", 0.25), 0.25),
        ("set_led_brightness", {"value": 0.0}, "ledbright", ("f", 0.0), ("f", 1.0), 1.0),
        ("set_lamp_level", {"value": 0.75}, "lamp", ("f", 0.75), ("f", 0.75), 0.75)]:
    binary(X, cmd, inp, osc(f"/-prefs/{leaf}", wire))
    telemetry(X, f"preference-{leaf.replace('_', '-')}", inbound_hex=hexs(osc(f"/-prefs/{leaf}", pushed)),
              expect_state={"preferences": {leaf: value}})
for cmd, inp, leaf, wire, key, pushed, value in [
        ("set_remote_enable", {"enabled": True}, "enable", ("i", 1), "enabled", ("i", 1), True),
        ("set_remote_protocol", {"protocol": 2}, "protocol", ("i", 2), "protocol", ("i", 1), 1),
        ("set_remote_port", {"port": 2}, "port", ("i", 2), "port", ("i", 0), 0),
        ("set_remote_io", {"features": 16383}, "ioenable", ("i", 16383), "io", ("i", 33), 33)]:
    binary(X, cmd, inp, osc(f"/-prefs/remote/{leaf}", wire))
    telemetry(X, f"remote-{key}", inbound_hex=hexs(osc(f"/-prefs/remote/{leaf}", pushed)), expect_state={"remote": {key: value}})
binary(X, "select_strip", {"strip": 72}, osc("/-stat/selidx", ("i", 71)))
telemetry(X, "selected-strip", inbound_hex=hexs(osc("/-stat/selidx", ("i", 0))), expect_state={"selected_strip": 0})
for cmd, inp, leaf, wire, key, pushed, value in [
        ("set_channel_fader_bank", {"bank": 3}, "chfaderbank", ("i", 3), "channel_fader_bank", ("i", 1), 1),
        ("set_group_fader_bank", {"bank": 5}, "grpfaderbank", ("i", 5), "group_fader_bank", ("i", 0), 0),
        ("set_bus_sends_bank", {"bank": 2}, "bussendbank", ("i", 2), "bus_sends_bank", ("i", 3), 3),
        ("set_screen", {"screen": 9}, "screen/screen", ("i", 9), "screen", ("i", 10), 10),
        ("set_eq_band_selection", {"band": 5}, "eqband", ("i", 5), "eq_band_selection", ("i", 0), 0),
        ("set_sends_on_fader", {"enabled": True}, "sendsonfader", ("i", 1), "sendsonfader", ("i", 0), False)]:
    binary(X, cmd, inp, osc(f"/-stat/{leaf}", wire))
    telemetry(X, f"status-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"/-stat/{leaf}", pushed)), expect_state={"status": {key: value}})
binary(X, "set_screen_page", {"screen": "SCENE", "page": 5}, osc("/-stat/screen/SCENE/page", ("i", 5)))
telemetry(X, "status-screen-page", inbound_hex=hexs(osc("/-stat/screen/CHAN/page", ("i", 4))),
          expect_state={"status": {"screen_pages": {"CHAN": 4}}})
for leaf, path, wire, value in [("userbank", {"user_controls": {"bank": 2}}, ("i", 2), None),
                                ("usbmounted", {"usb": {"mounted": True}}, ("i", 1), None),
                                ("xcardtype", {"status": {"card_type": 10}}, ("i", 10), None),
                                ("autosave", {"status": {"autosave": False}}, ("i", 0), None),
                                ("remote", {"status": {"daw_mode": True}}, ("i", 1), None),
                                ("lock", {"status": {"lock": 1}}, ("i", 1), None)]:
    telemetry(X, f"stat-{leaf}", inbound_hex=hexs(osc(f"/-stat/{leaf}", wire)), expect_state=path)
telemetry(X, "user-button", inbound_hex=hexs(osc("/config/userctrl/A/btn/12", ("s", "O00"))),
          expect_state={"user_controls": {"sets": {"A": {"buttons": {"12": "O00"}}}}})
telemetry(X, "user-encoder", inbound_hex=hexs(osc("/config/userctrl/C/enc/4", ("s", "F70"))),
          expect_state={"user_controls": {"sets": {"C": {"encoders": {"4": "F70"}}}}})
telemetry(X, "user-set-color", inbound_hex=hexs(osc("/config/userctrl/B/color", ("i", 4))),
          expect_state={"user_controls": {"sets": {"B": {"color": 4}}}})
binary(X, "set_usb_recorder_state", {"state": 4}, osc("/-stat/tape/state", ("i", 4)))
binary(X, "set_usb_recorder_gain", {"side": "R", "gain": 0.2}, osc("/config/tape/gainR", ("f", 0.2)))
binary(X, "set_usb_recorder_autoplay", {"enabled": True}, osc("/config/tape/autoplay", ("i", 1)))
binary(X, "usb_recorder_track", {"direction": -1}, osc("/-action/playtrack", ("i", -1)))
binary(X, "usb_recorder_select", {"record": 6}, osc("/-action/recselect", ("i", 6)))
for leaf, path, wire in [("/-stat/tape/state", {"usb_recorder": {"state": 2}}, ("i", 2)),
                         ("/-stat/tape/file", {"usb_recorder": {"file": "/dir000/R_20130105-205752.wav"}}, ("s", "/dir000/R_20130105-205752.wav")),
                         ("/-stat/tape/etime", {"usb_recorder": {"elapsed": 42}}, ("i", 42)),
                         ("/-stat/tape/rtime", {"usb_recorder": {"remaining": -1}}, ("i", -1)),
                         ("/config/tape/gainL", {"usb_recorder": {"gain": {"L": 0.5}}}, ("f", 0.5)),
                         ("/config/tape/autoplay", {"usb_recorder": {"autoplay": False}}, ("i", 0)),
                         ("/-usb/path", {"usb": {"path": "Dblues 48kHz"}}, ("s", "Dblues 48kHz")),
                         ("/-usb/title", {"usb": {"title": "Candy-DB"}}, ("s", "Candy-DB")),
                         ("/-usb/dir/maxpos", {"usb": {"entries": 16}}, ("i", 16)),
                         ("/-usb/dir/006/name", {"usb": {"dir": {"6": {"name": "Candy.wav"}}}}, ("s", "Candy.wav")),
                         ("/-stat/urec/state", {"xlive": {"state": 3}}, ("i", 3)),
                         ("/-stat/urec/etime", {"xlive": {"elapsed": 869}}, ("i", 869)),
                         ("/-stat/urec/rtime", {"xlive": {"remaining": 2615103}}, ("i", 2615103)),
                         ("/-urec/sessionmax", {"xlive": {"sessions": 2}}, ("i", 2)),
                         ("/-urec/markermax", {"xlive": {"markers": 3}}, ("i", 3)),
                         ("/-urec/sessionpos", {"xlive": {"session": 6}}, ("i", 6)),
                         ("/-urec/sessionlen", {"xlive": {"session_length": 4970}}, ("i", 4970))]:
    telemetry(X, "rec" + leaf.replace("/", "-").replace("--", "-"), inbound_hex=hexs(osc(leaf, wire)), expect_state=path)
binary(X, "set_xlive_state", {"state": 3}, osc("/-stat/urec/state", ("i", 3)))
for cmd, leaf, n in [("select_xlive_session", "selsession", 6), ("delete_xlive_session", "delsession", 100),
                     ("select_xlive_marker", "selmarker", 1), ("delete_xlive_marker", "delmarker", 2),
                     ("save_xlive_marker", "savemarker", 3)]:
    binary(X, cmd, {"index": n}, osc(f"/-action/{leaf}", ("i", n)))
binary(X, "add_xlive_marker", {}, osc("/-action/addmarker", ("i", 1)))
binary(X, "set_xlive_position", {"position": 86399999}, osc("/-action/setposition", ("i", 86399999)))
binary(X, "clear_xlive_alert", {}, osc("/-action/clearalert", ("i", 1)))
binary(X, "save_console_state", {}, osc("/-action/savestate", ("i", 1)))


def _osc_args(packet):
    """Address, type tags and i/f/s arguments of one OSC message."""
    def string(pos):
        end = packet.index(b"\0", pos)
        return packet[pos:end].decode(), (end + 4) & ~3
    address, pos = string(0)
    tags, pos = string(pos)
    args = []
    for t in tags[1:]:
        if t == "i":
            args.append(struct.unpack(">i", packet[pos:pos + 4])[0]); pos += 4
        elif t == "f":
            args.append(struct.unpack(">f", packet[pos:pos + 4])[0]); pos += 4
        elif t == "s":
            v, pos = string(pos); args.append(v)
        else:
            return address, tags[1:], None
    return address, tags[1:], args


def _with_params(spec):
    """Every single-argument parameter the console reports is also kept
    under params, keyed by its address without the leading /, in the field
    of its OSC type (the generic rules)."""
    import re
    for v in V:
        if v.get("spec") != spec or "inbound_hex" not in v or "expect_state" not in v:
            continue
        address, tags, args = _osc_args(bytes.fromhex(v["inbound_hex"]))
        if args is None or tags not in ("f", "i", "s"):
            continue
        if not re.fullmatch(r"/[A-Za-z0-9_-][A-Za-z0-9_/-]*", address):
            continue
        field = {"f": "float", "i": "int", "s": "string"}[tags]
        v["expect_state"] = {**v["expect_state"], "params": {address[1:]: {field: args[0]}}}


_with_params(X)
