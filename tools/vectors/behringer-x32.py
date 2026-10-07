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
telemetry(X, "param-int", inbound_hex=hexs(osc("/-prefs/clocksource", ("i", 1))), expect_state={})
telemetry(X, "param-string", inbound_hex=hexs(osc("/-show/showfile/show/name", ("s", "Sunday"))), expect_state={})


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
