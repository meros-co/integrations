# Behringer X AIR (XR18, X18, XR16, XR12) / Midas M AIR (MR18, MR12): OSC over
# UDP 10024. Protocol rules from Behringer's "X AIR Mixer Series Remote Control
# Protocol (fw 1.11)" p.1-2: a get is the bare address, the mixer answers on the
# same address; floats 0-1, booleans as ints. Addresses from the community
# Behringer World Wiki "X-Air / M-Air OSC Commands" list. mix/on is 1 = passing
# audio, as on the X32.
XA = "behringer-xair"

UNVERIFIED = {"ok": {"kind": "unverified"}}


def _val(v):
    return {"ok": {"kind": "value", "value": v}}


# Strips with a number: (command stem, parameter, wire prefix for number n).
_XA_NUMBERED = [
    ("channel", "channel", lambda n: f"/ch/{n:02d}"),
    ("fx_return", "fx_return", lambda n: f"/rtn/{n}"),
    ("bus", "bus", lambda n: f"/bus/{n}"),
    ("fx_send", "fx_send", lambda n: f"/fxsend/{n}"),
]
# Single strips: (command stem, wire prefix).
_XA_SINGLE = [("aux", "/rtn/aux"), ("main", "/lr")]

# Fader, mute, name and colour on every strip.
for stem, p, pre in _XA_NUMBERED:
    n = 4 if stem != "channel" else 16
    binary(XA, f"set_{stem}_fader", {p: n, "level": 0.75}, osc(f"{pre(n)}/mix/fader", ("f", 0.75)))
    binary(XA, f"mute_{stem}", {p: 1, "muted": True}, osc(f"{pre(1)}/mix/on", ("i", 0)),
           expect_result=UNVERIFIED)
    binary(XA, f"get_{stem}_on", {p: 2}, osc(f"{pre(2)}/mix/on"),
           device_reply_hex=hexs(osc(f"{pre(2)}/mix/on", ("i", 1))), expect_result=_val(1))
    binary(XA, f"set_{stem}_name", {p: 3, "name": "Vox"}, osc(f"{pre(3)}/config/name", ("s", "Vox")))
    binary(XA, f"set_{stem}_color", {p: 4, "color": 9}, osc(f"{pre(4)}/config/color", ("i", 9)))
for stem, pre in _XA_SINGLE:
    binary(XA, f"set_{stem}_fader", {"level": 1.0}, osc(f"{pre}/mix/fader", ("f", 1.0)))
    binary(XA, f"mute_{stem}", {"muted": False}, osc(f"{pre}/mix/on", ("i", 1)))
    binary(XA, f"get_{stem}_fader", {}, osc(f"{pre}/mix/fader"),
           device_reply_hex=hexs(osc(f"{pre}/mix/fader", ("f", 0.75))), expect_result=_val(0.75))
    binary(XA, f"get_{stem}_on", {}, osc(f"{pre}/mix/on"))
    binary(XA, f"get_{stem}_name", {}, osc(f"{pre}/config/name"),
           device_reply_hex=hexs(osc(f"{pre}/config/name", ("s", "PA"))), expect_result=_val("PA"))
    binary(XA, f"set_{stem}_name", {"name": "PA"}, osc(f"{pre}/config/name", ("s", "PA")))
    binary(XA, f"set_{stem}_color", {"color": 0}, osc(f"{pre}/config/color", ("i", 0)))
# The document's example echo address, /ch/01/mix/fader (p.2).
binary(XA, "get_channel_fader", {"channel": 1}, osc("/ch/01/mix/fader"),
       device_reply_hex=hexs(osc("/ch/01/mix/fader", ("f", 0.5))), expect_result=_val(0.5))
binary(XA, "get_fx_return_fader", {"fx_return": 4}, osc("/rtn/4/mix/fader"))
binary(XA, "get_bus_fader", {"bus": 6}, osc("/bus/6/mix/fader"),
       device_reply_hex=hexs(osc("/bus/6/mix/fader", ("f", 0.25))), expect_result=_val(0.25))
binary(XA, "get_fx_send_fader", {"fx_send": 1}, osc("/fxsend/1/mix/fader"))
binary(XA, "get_channel_name", {"channel": 12}, osc("/ch/12/config/name"),
       device_reply_hex=hexs(osc("/ch/12/config/name", ("s", "Kick"))), expect_result=_val("Kick"))
binary(XA, "get_fx_return_name", {"fx_return": 1}, osc("/rtn/1/config/name"))
binary(XA, "get_fx_send_name", {"fx_send": 2}, osc("/fxsend/2/config/name"))
# A message on another address is not the reply: bus 2's name is still pending.
binary(XA, "get_bus_name", {"bus": 2}, osc("/bus/2/config/name"),
       device_reply_hex=hexs(osc("/bus/3/config/name", ("s", "Wedge"))), expect_result=None)

# Pan: channels, aux, FX returns, buses, main.
binary(XA, "set_channel_pan", {"channel": 2, "pan": 0.25}, osc("/ch/02/mix/pan", ("f", 0.25)))
binary(XA, "set_aux_pan", {"pan": 0.5}, osc("/rtn/aux/mix/pan", ("f", 0.5)))
binary(XA, "set_fx_return_pan", {"fx_return": 3, "pan": 1.0}, osc("/rtn/3/mix/pan", ("f", 1.0)))
binary(XA, "set_bus_pan", {"bus": 5, "pan": 0.0}, osc("/bus/5/mix/pan", ("f", 0.0)))
binary(XA, "set_main_pan", {"pan": 0.75}, osc("/lr/mix/pan", ("f", 0.75)))

# Sends: two-digit bus 01-06 mix buses, 07-10 FX sends 1-4.
for stem, p, pre, n in [("channel", "channel", "/ch/07", 7), ("fx_return", "fx_return", "/rtn/2", 2)]:
    binary(XA, f"set_{stem}_send_level", {p: n, "bus": 10, "level": 0.75}, osc(f"{pre}/mix/10/level", ("f", 0.75)))
    binary(XA, f"get_{stem}_send_level", {p: n, "bus": 1}, osc(f"{pre}/mix/01/level"),
           device_reply_hex=hexs(osc(f"{pre}/mix/01/level", ("f", 0.5))), expect_result=_val(0.5))
    binary(XA, f"set_{stem}_send_pan", {p: n, "bus": 6, "pan": 0.5}, osc(f"{pre}/mix/06/pan", ("f", 0.5)))
    binary(XA, f"set_{stem}_send_tap", {p: n, "bus": 3, "tap": 4}, osc(f"{pre}/mix/03/tap", ("i", 4)))
    binary(XA, f"set_{stem}_send_group_on", {p: n, "bus": 2, "enabled": True}, osc(f"{pre}/mix/02/grpon", ("i", 1)))
binary(XA, "set_aux_send_level", {"bus": 7, "level": 0.25}, osc("/rtn/aux/mix/07/level", ("f", 0.25)))
binary(XA, "get_aux_send_level", {"bus": 2}, osc("/rtn/aux/mix/02/level"))
binary(XA, "set_aux_send_pan", {"bus": 1, "pan": 0.0}, osc("/rtn/aux/mix/01/pan", ("f", 0.0)))
binary(XA, "set_aux_send_tap", {"bus": 9, "tap": 0}, osc("/rtn/aux/mix/09/tap", ("i", 0)))
binary(XA, "set_aux_send_group_on", {"bus": 4, "enabled": False}, osc("/rtn/aux/mix/04/grpon", ("i", 0)))

# Main LR assignment (mix/lr).
binary(XA, "assign_channel_to_main", {"channel": 9, "assigned": False}, osc("/ch/09/mix/lr", ("i", 0)))
binary(XA, "assign_aux_to_main", {"assigned": True}, osc("/rtn/aux/mix/lr", ("i", 1)))
binary(XA, "assign_fx_return_to_main", {"fx_return": 1, "assigned": True}, osc("/rtn/1/mix/lr", ("i", 1)))
binary(XA, "assign_bus_to_main", {"bus": 6, "assigned": True}, osc("/bus/6/mix/lr", ("i", 1)))

# DCA and mute-group bitmaps, four groups each.
for stem, p, pre in [_XA_NUMBERED[0], _XA_NUMBERED[2], _XA_NUMBERED[3]]:
    binary(XA, f"set_{stem}_dca_groups", {p: 1, "groups": 15}, osc(f"{pre(1)}/grp/dca", ("i", 15)))
    binary(XA, f"set_{stem}_mute_groups", {p: 1, "groups": 5}, osc(f"{pre(1)}/grp/mute", ("i", 5)))

# EQ on/off and mode.
binary(XA, "set_channel_eq_on", {"channel": 16, "enabled": True}, osc("/ch/16/eq/on", ("i", 1)))
binary(XA, "set_aux_eq_on", {"enabled": False}, osc("/rtn/aux/eq/on", ("i", 0)))
binary(XA, "set_bus_eq_on", {"bus": 1, "enabled": True}, osc("/bus/1/eq/on", ("i", 1)))
binary(XA, "set_main_eq_on", {"enabled": True}, osc("/lr/eq/on", ("i", 1)))
binary(XA, "set_bus_eq_mode", {"bus": 2, "mode": 1}, osc("/bus/2/eq/mode", ("i", 1)))
binary(XA, "set_main_eq_mode", {"mode": 2}, osc("/lr/eq/mode", ("i", 2)))

# EQ bands: /eq/<band>/type|f|g|q; 4 bands on inputs, 6 on buses and main.
for stem, inp, pre, band in [("channel", {"channel": 5}, "/ch/05", 4), ("aux", {}, "/rtn/aux", 1),
                             ("fx_return", {"fx_return": 4}, "/rtn/4", 2), ("bus", {"bus": 3}, "/bus/3", 6),
                             ("main", {}, "/lr", 5)]:
    binary(XA, f"set_{stem}_eq_band_type", {**inp, "band": band, "type": 2}, osc(f"{pre}/eq/{band}/type", ("i", 2)))
    binary(XA, f"set_{stem}_eq_band_frequency", {**inp, "band": band, "frequency": 0.5},
           osc(f"{pre}/eq/{band}/f", ("f", 0.5)))
    binary(XA, f"set_{stem}_eq_band_gain", {**inp, "band": band, "gain": 0.75}, osc(f"{pre}/eq/{band}/g", ("f", 0.75)))
    binary(XA, f"set_{stem}_eq_band_q", {**inp, "band": band, "q": 0.25}, osc(f"{pre}/eq/{band}/q", ("f", 0.25)))

# Compressor: channels, buses, main. Gate: channels.
_XA_DYN = [("on", "on", "b"), ("mode", "mode", 1), ("detector", "det", 1), ("envelope", "env", 0),
           ("threshold", "thr", 0.5), ("ratio", "ratio", 11), ("knee", "knee", 0.25),
           ("makeup_gain", "mgain", 0.0), ("attack", "attack", 0.125), ("hold", "hold", 0.5),
           ("release", "release", 0.75), ("mix", "mix", 1.0), ("key_source", "keysrc", 22),
           ("auto", "auto", "b"), ("filter_on", "filter/on", "b"), ("filter_type", "filter/type", 8),
           ("filter_frequency", "filter/f", 0.5)]
_XA_GATE = [("on", "on", "b"), ("mode", "mode", 3), ("threshold", "thr", 0.25), ("range", "range", 1.0),
            ("attack", "attack", 0.0), ("hold", "hold", 0.5), ("release", "release", 0.5),
            ("key_source", "keysrc", 0), ("filter_on", "filter/on", "b"), ("filter_type", "filter/type", 4),
            ("filter_frequency", "filter/f", 0.75)]


def _xa_proc(stem, inp, pre, block, items):
    for key, wire, sample in items:
        if sample == "b":
            binary(XA, f"set_{stem}_{block}_{key}", {**inp, "enabled": True}, osc(f"{pre}/{block}/{wire}", ("i", 1)))
        elif isinstance(sample, int):
            binary(XA, f"set_{stem}_{block}_{key}", {**inp, "value": sample},
                   osc(f"{pre}/{block}/{wire}", ("i", sample)))
        else:
            binary(XA, f"set_{stem}_{block}_{key}", {**inp, "value": sample},
                   osc(f"{pre}/{block}/{wire}", ("f", sample)))


_xa_proc("channel", {"channel": 3}, "/ch/03", "dyn", _XA_DYN)
_xa_proc("bus", {"bus": 4}, "/bus/4", "dyn", _XA_DYN)
_xa_proc("main", {}, "/lr", "dyn", _XA_DYN)
_xa_proc("channel", {"channel": 11}, "/ch/11", "gate", _XA_GATE)

# Inserts: insert/on and insert/fxslot 0-8.
binary(XA, "set_channel_insert_on", {"channel": 1, "enabled": True}, osc("/ch/01/insert/on", ("i", 1)))
binary(XA, "set_channel_insert_select", {"channel": 1, "insert": 8}, osc("/ch/01/insert/fxslot", ("i", 8)))
binary(XA, "set_bus_insert_on", {"bus": 2, "enabled": False}, osc("/bus/2/insert/on", ("i", 0)))
binary(XA, "set_bus_insert_select", {"bus": 2, "insert": 1}, osc("/bus/2/insert/fxslot", ("i", 1)))

# Preamp and source of the input strips.
binary(XA, "set_channel_usb_trim", {"channel": 4, "trim": 0.5}, osc("/ch/04/preamp/rtntrim", ("f", 0.5)))
binary(XA, "set_aux_usb_trim", {"trim": 1.0}, osc("/rtn/aux/preamp/rtntrim", ("f", 1.0)))
binary(XA, "set_fx_return_usb_trim", {"fx_return": 2, "trim": 0.0}, osc("/rtn/2/preamp/rtntrim", ("f", 0.0)))
binary(XA, "set_channel_usb_return", {"channel": 4, "enabled": True}, osc("/ch/04/preamp/rtnsw", ("i", 1)))
binary(XA, "set_aux_usb_return", {"enabled": False}, osc("/rtn/aux/preamp/rtnsw", ("i", 0)))
binary(XA, "set_fx_return_usb_return", {"fx_return": 3, "enabled": True}, osc("/rtn/3/preamp/rtnsw", ("i", 1)))
binary(XA, "set_aux_usb_source", {"source": 8}, osc("/rtn/aux/config/rtnsrc", ("i", 8)))
binary(XA, "set_fx_return_usb_source", {"fx_return": 1, "source": 0}, osc("/rtn/1/config/rtnsrc", ("i", 0)))
binary(XA, "set_channel_invert", {"channel": 8, "inverted": True}, osc("/ch/08/preamp/invert", ("i", 1)))
binary(XA, "set_channel_low_cut", {"channel": 8, "enabled": True}, osc("/ch/08/preamp/hpon", ("i", 1)))
binary(XA, "set_channel_low_cut_frequency", {"channel": 8, "frequency": 0.25}, osc("/ch/08/preamp/hpf", ("f", 0.25)))
binary(XA, "set_channel_source", {"channel": 13, "source": 15}, osc("/ch/13/config/insrc", ("i", 15)))
binary(XA, "set_channel_usb_source", {"channel": 13, "source": 17}, osc("/ch/13/config/rtnsrc", ("i", 17)))
binary(XA, "set_channel_automix_group", {"channel": 6, "group": 2}, osc("/ch/06/automix/group", ("i", 2)))
binary(XA, "set_channel_automix_weight", {"channel": 6, "weight": 0.5}, osc("/ch/06/automix/weight", ("f", 0.5)))

# DCAs 1-4: on and fader directly under /dca/N.
binary(XA, "set_dca_fader", {"dca": 4, "level": 0.5}, osc("/dca/4/fader", ("f", 0.5)))
binary(XA, "mute_dca", {"dca": 1, "muted": True}, osc("/dca/1/on", ("i", 0)))
binary(XA, "get_dca_fader", {"dca": 2}, osc("/dca/2/fader"))
binary(XA, "get_dca_on", {"dca": 3}, osc("/dca/3/on"),
       device_reply_hex=hexs(osc("/dca/3/on", ("i", 0))), expect_result=_val(0))
binary(XA, "get_dca_name", {"dca": 1}, osc("/dca/1/config/name"))
binary(XA, "set_dca_name", {"dca": 1, "name": "Band"}, osc("/dca/1/config/name", ("s", "Band")))
binary(XA, "set_dca_color", {"dca": 2, "color": 15}, osc("/dca/2/config/color", ("i", 15)))

# Solo, mute groups 1-4, headphone/monitor (/config/solo).
binary(XA, "solo_channel", {"channel": 16, "soloed": True}, osc("/-stat/solosw/16", ("i", 1)))
binary(XA, "clear_solo", {}, osc("/-action/clearsolo", ("i", 1)))
binary(XA, "get_solo_active", {}, osc("/-stat/solo"),
       device_reply_hex=hexs(osc("/-stat/solo", ("i", 0))), expect_result=_val(0))
binary(XA, "mute_group", {"group": 4, "muted": True}, osc("/config/mute/4", ("i", 1)))
binary(XA, "get_mute_group", {"group": 1}, osc("/config/mute/1"),
       device_reply_hex=hexs(osc("/config/mute/1", ("i", 1))), expect_result=_val(1))
binary(XA, "set_monitor_level", {"value": 0.75}, osc("/config/solo/level", ("f", 0.75)))
binary(XA, "set_monitor_source", {"value": 14}, osc("/config/solo/source", ("i", 14)))
binary(XA, "set_monitor_source_trim", {"value": 0.5}, osc("/config/solo/sourcetrim", ("f", 0.5)))
binary(XA, "set_monitor_channel_mode", {"value": 1}, osc("/config/solo/chmode", ("i", 1)))
binary(XA, "set_monitor_bus_mode", {"value": 0}, osc("/config/solo/busmode", ("i", 0)))
binary(XA, "set_monitor_dim", {"enabled": True}, osc("/config/solo/dim", ("i", 1)))
binary(XA, "set_monitor_dim_gain", {"value": 0.25}, osc("/config/solo/dimatt", ("f", 0.25)))
binary(XA, "set_monitor_pfl_dim", {"enabled": False}, osc("/config/solo/dimpfl", ("i", 0)))
binary(XA, "set_monitor_mono", {"enabled": True}, osc("/config/solo/mono", ("i", 1)))
binary(XA, "mute_monitor", {"muted": True}, osc("/config/solo/mute", ("i", 1)))

# Links and automix.
binary(XA, "set_channel_link", {"pair": "15-16", "linked": True}, osc("/config/chlink/15-16", ("i", 1)))
binary(XA, "set_bus_link", {"pair": "1-2", "linked": False}, osc("/config/buslink/1-2", ("i", 0)))
binary(XA, "set_link_preference", {"element": "fdrmute", "enabled": True}, osc("/config/linkcfg/fdrmute", ("i", 1)))
binary(XA, "set_automix_enable", {"group": "X", "enabled": True}, osc("/config/amixenable/X", ("i", 1)))
binary(XA, "set_automix_lock", {"group": "Y", "locked": False}, osc("/config/amixlock/Y", ("i", 0)))

# Headamps /headamp/01-16, numbered from 1 on the wire.
binary(XA, "set_headamp_gain", {"headamp": 1, "gain": 0.5}, osc("/headamp/01/gain", ("f", 0.5)))
binary(XA, "set_headamp_phantom", {"headamp": 16, "enabled": True}, osc("/headamp/16/phantom", ("i", 1)))
binary(XA, "get_headamp_gain", {"headamp": 9}, osc("/headamp/09/gain"),
       device_reply_hex=hexs(osc("/headamp/09/gain", ("f", 0.25))), expect_result=_val(0.25))
binary(XA, "get_headamp_phantom", {"headamp": 2}, osc("/headamp/02/phantom"))

# Effects slots 1-4.
binary(XA, "set_fx_type", {"slot": 1, "type": 60}, osc("/fx/1/type", ("i", 60)))
binary(XA, "get_fx_type", {"slot": 4}, osc("/fx/4/type"),
       device_reply_hex=hexs(osc("/fx/4/type", ("i", 3))), expect_result=_val(3))
binary(XA, "set_fx_insert", {"slot": 2, "enabled": True}, osc("/fx/2/insert", ("i", 1)))
binary(XA, "get_fx_parameter", {"slot": 3, "parameter": 1}, osc("/fx/3/par/01"))

# Routing.
binary(XA, "set_aux_out_source", {"output": 6, "source": 55}, osc("/routing/aux/06/src", ("i", 55)))
binary(XA, "set_aux_out_tap", {"output": 1, "tap": 10}, osc("/routing/aux/01/pos", ("i", 10)))
binary(XA, "set_main_out_source", {"source": 0}, osc("/routing/main/01/src", ("i", 0)))
binary(XA, "set_phones_source", {"source": 1}, osc("/routing/main/02/src", ("i", 1)))
binary(XA, "set_ultranet_source", {"output": 16, "source": 0}, osc("/routing/p16/16/src", ("i", 0)))
binary(XA, "set_ultranet_tap", {"output": 2, "tap": 8}, osc("/routing/p16/02/pos", ("i", 8)))
binary(XA, "set_usb_out_source", {"output": 18, "source": 37}, osc("/routing/usb/18/src", ("i", 37)))
binary(XA, "set_usb_out_tap", {"output": 1, "tap": 2}, osc("/routing/usb/01/pos", ("i", 2)))

# Snapshots, protocol document p.4.
binary(XA, "load_snapshot", {"snapshot": 64}, osc("/-snap/load", ("i", 64)), expect_result=UNVERIFIED)
binary(XA, "save_snapshot", {"snapshot": 1}, osc("/-snap/save", ("i", 1)))
binary(XA, "delete_snapshot", {"snapshot": 12}, osc("/-snap/delete", ("i", 12)))
binary(XA, "set_snapshot_save_name", {"name": "Act 2"}, osc("/-snap/name", ("s", "Act 2")))
binary(XA, "get_snapshot_index", {}, osc("/-snap/index"),
       device_reply_hex=hexs(osc("/-snap/index", ("i", 7))), expect_result=_val(7))
binary(XA, "get_snapshot_name", {}, osc("/-snap/name"))
binary(XA, "get_snapshot_slot_name", {"snapshot": 5}, osc("/-snap/05/name"),
       device_reply_hex=hexs(osc("/-snap/05/name", ("s", "Opener"))), expect_result=_val("Opener"))
binary(XA, "set_snapshot_slot_name", {"snapshot": 5, "name": "Opener"}, osc("/-snap/05/name", ("s", "Opener")))
binary(XA, "get_snapshot_scope", {"snapshot": 64}, osc("/-snap/64/scope"))
binary(XA, "set_snapshot_scope", {"snapshot": 2, "scope": "+" * 58 + "-"},
       osc("/-snap/02/scope", ("s", "+" * 58 + "-")))

# USB recorder (XR16, XR12, MR12).
binary(XA, "set_recorder_state", {"state": 4}, osc("/-stat/tape/state", ("i", 4)), model="xr16")
binary(XA, "get_recorder_state", {}, osc("/-stat/tape/state"), model="xr12",
       device_reply_hex=hexs(osc("/-stat/tape/state", ("i", 2))), expect_result=_val(2))
binary(XA, "get_usb_mounted", {}, osc("/-stat/usbmounted"), model="mr12")

# Console. /info answers ,ssss: server version, name, model, version (p.2).
binary(XA, "save_state", {}, osc("/-action/savestate", ("i", 1)))
binary(XA, "get_mixer_name", {}, osc("/-prefs/name"),
       device_reply_hex=hexs(osc("/-prefs/name", ("s", "XR18-1A-2B-3C"))), expect_result=_val("XR18-1A-2B-3C"))
_XA_INFO = hexs(osc("/info", ("s", "V0.04"), ("s", "XR18-1A-2B-3C"), ("s", "XR18"), ("s", "1.22")))
binary(XA, "get_console_model", {}, osc("/info"), device_reply_hex=_XA_INFO, expect_result=_val("XR18"))
binary(XA, "get_firmware_version", {}, osc("/info"), device_reply_hex=_XA_INFO, expect_result=_val("1.22"))
binary(XA, "get_console_name", {}, osc("/info"), device_reply_hex=_XA_INFO, expect_result=_val("XR18-1A-2B-3C"))
binary(XA, "get_status", {}, osc("/status"),
       device_reply_hex=hexs(osc("/status", ("s", "active"), ("s", "192.168.1.20"), ("s", "XR18-1A-2B-3C"))),
       expect_result=_val("active"))

# ── Telemetry ─────────────────────────────────────────────────────────────
# /xremote on connect, then the first of the one-at-a-time queries for current
# values; pushed changes arrive on the same addresses as sets.
telemetry(XA, "channel-mute", expect_connect_wire_hex=[hexs(osc("/xremote")), hexs(osc("/ch/01/mix/on"))],
          inbound_hex=hexs(osc("/ch/07/mix/on", ("i", 0))), expect_state={"channels": {"7": {"mute": True}}})


def _t(name, address, arg, state):
    telemetry(XA, name, inbound_hex=hexs(osc(address, arg)), expect_state=state)


_t("channel-fader", "/ch/16/mix/fader", ("f", 0.75), {"channels": {"16": {"fader": 0.75}}})
_t("channel-name", "/ch/12/config/name", ("s", "Vox"), {"channels": {"12": {"name": "Vox"}}})
_t("channel-color", "/ch/05/config/color", ("i", 3), {"channels": {"5": {"color": 3}}})
_t("channel-pan", "/ch/02/mix/pan", ("f", 0.25), {"channels": {"2": {"pan": 0.25}}})
_t("channel-send-level", "/ch/03/mix/10/level", ("f", 0.5), {"channels": {"3": {"sends": {"10": {"level": 0.5}}}}})
_t("channel-send-pan", "/ch/03/mix/01/pan", ("f", 0.5), {"channels": {"3": {"sends": {"1": {"pan": 0.5}}}}})
_t("channel-send-tap", "/ch/03/mix/02/tap", ("i", 3), {"channels": {"3": {"sends": {"2": {"tap": 3}}}}})
_t("channel-send-group", "/ch/03/mix/06/grpon", ("i", 1), {"channels": {"3": {"sends": {"6": {"group_on": True}}}}})
_t("channel-main-assign", "/ch/04/mix/lr", ("i", 0), {"channels": {"4": {"main_assign": False}}})
_t("channel-dca-groups", "/ch/04/grp/dca", ("i", 9), {"channels": {"4": {"dca_groups": 9}}})
_t("channel-mute-groups", "/ch/04/grp/mute", ("i", 2), {"channels": {"4": {"mute_groups": 2}}})
_t("channel-eq-on", "/ch/01/eq/on", ("i", 1), {"channels": {"1": {"eq_on": True}}})
_t("channel-eq-band-type", "/ch/01/eq/4/type", ("i", 5), {"channels": {"1": {"eq": {"bands": {"4": {"type": 5}}}}}})
_t("channel-eq-band-frequency", "/ch/01/eq/2/f", ("f", 0.5), {"channels": {"1": {"eq": {"bands": {"2": {"frequency": 0.5}}}}}})
_t("channel-eq-band-gain", "/ch/01/eq/3/g", ("f", 0.75), {"channels": {"1": {"eq": {"bands": {"3": {"gain": 0.75}}}}}})
_t("channel-eq-band-q", "/ch/01/eq/1/q", ("f", 0.25), {"channels": {"1": {"eq": {"bands": {"1": {"q": 0.25}}}}}})
_t("channel-insert-on", "/ch/09/insert/on", ("i", 1), {"channels": {"9": {"insert_on": True}}})
_t("channel-insert-select", "/ch/09/insert/fxslot", ("i", 2), {"channels": {"9": {"insert_select": 2}}})
_t("channel-usb-trim", "/ch/09/preamp/rtntrim", ("f", 0.5), {"channels": {"9": {"usb_trim": 0.5}}})
_t("channel-usb-return", "/ch/09/preamp/rtnsw", ("i", 1), {"channels": {"9": {"usb_return": True}}})
_t("channel-invert", "/ch/09/preamp/invert", ("i", 1), {"channels": {"9": {"invert": True}}})
_t("channel-low-cut", "/ch/09/preamp/hpon", ("i", 0), {"channels": {"9": {"low_cut": False}}})
_t("channel-low-cut-frequency", "/ch/09/preamp/hpf", ("f", 0.25), {"channels": {"9": {"low_cut_frequency": 0.25}}})
_t("channel-source", "/ch/13/config/insrc", ("i", 4), {"channels": {"13": {"source": 4}}})
_t("channel-usb-source", "/ch/13/config/rtnsrc", ("i", 12), {"channels": {"13": {"usb_source": 12}}})
_t("channel-automix-group", "/ch/06/automix/group", ("i", 1), {"channels": {"6": {"automix_group": 1}}})
_t("channel-automix-weight", "/ch/06/automix/weight", ("f", 0.5), {"channels": {"6": {"automix_weight": 0.5}}})
_t("channel-solo", "/-stat/solosw/16", ("i", 1), {"channels": {"16": {"solo": True}}})
_XA_PROC_T = {"on": ("i", 1, True), "mode": ("i", 1, 1), "detector": ("i", 1, 1), "envelope": ("i", 0, 0),
              "threshold": ("f", 0.5, 0.5), "ratio": ("i", 6, 6), "knee": ("f", 0.25, 0.25),
              "makeup_gain": ("f", 0.0, 0.0), "attack": ("f", 0.125, 0.125), "hold": ("f", 0.5, 0.5),
              "release": ("f", 0.75, 0.75), "mix": ("f", 1.0, 1.0), "key_source": ("i", 17, 17),
              "auto": ("i", 0, False), "filter_on": ("i", 1, True), "filter_type": ("i", 2, 2),
              "filter_frequency": ("f", 0.5, 0.5), "range": ("f", 0.25, 0.25)}
for block, items in (("dyn", _XA_DYN), ("gate", _XA_GATE)):
    for key, wire, _ in items:
        t, wv, sv = _XA_PROC_T[key]
        state = {f"{block}_on": sv} if key == "on" else {block: {key: sv}}
        _t(f"channel-{block}-{key.replace('_', '-')}", f"/ch/03/{block}/{wire}", (t, wv), {"channels": {"3": state}})
for key, wire, _ in _XA_DYN:
    t, wv, sv = _XA_PROC_T[key]
    state = {"dyn_on": sv} if key == "on" else {"dyn": {key: sv}}
    _t(f"bus-dyn-{key.replace('_', '-')}", f"/bus/2/dyn/{wire}", (t, wv), {"buses": {"2": state}})
    _t(f"main-dyn-{key.replace('_', '-')}", f"/lr/dyn/{wire}", (t, wv), {"main": state})

_t("aux-mute", "/rtn/aux/mix/on", ("i", 1), {"aux": {"mute": False}})
_t("aux-fader", "/rtn/aux/mix/fader", ("f", 0.5), {"aux": {"fader": 0.5}})
_t("aux-name", "/rtn/aux/config/name", ("s", "USB"), {"aux": {"name": "USB"}})
_t("aux-color", "/rtn/aux/config/color", ("i", 1), {"aux": {"color": 1}})
_t("aux-pan", "/rtn/aux/mix/pan", ("f", 0.5), {"aux": {"pan": 0.5}})
_t("aux-send-level", "/rtn/aux/mix/07/level", ("f", 0.25), {"aux": {"sends": {"7": {"level": 0.25}}}})
_t("aux-send-pan", "/rtn/aux/mix/07/pan", ("f", 0.25), {"aux": {"sends": {"7": {"pan": 0.25}}}})
_t("aux-send-tap", "/rtn/aux/mix/07/tap", ("i", 5), {"aux": {"sends": {"7": {"tap": 5}}}})
_t("aux-send-group", "/rtn/aux/mix/07/grpon", ("i", 0), {"aux": {"sends": {"7": {"group_on": False}}}})
_t("aux-main-assign", "/rtn/aux/mix/lr", ("i", 1), {"aux": {"main_assign": True}})
_t("aux-eq-on", "/rtn/aux/eq/on", ("i", 1), {"aux": {"eq_on": True}})
_t("aux-eq-band-type", "/rtn/aux/eq/1/type", ("i", 0), {"aux": {"eq": {"bands": {"1": {"type": 0}}}}})
_t("aux-eq-band-frequency", "/rtn/aux/eq/1/f", ("f", 0.5), {"aux": {"eq": {"bands": {"1": {"frequency": 0.5}}}}})
_t("aux-eq-band-gain", "/rtn/aux/eq/1/g", ("f", 0.5), {"aux": {"eq": {"bands": {"1": {"gain": 0.5}}}}})
_t("aux-eq-band-q", "/rtn/aux/eq/1/q", ("f", 0.5), {"aux": {"eq": {"bands": {"1": {"q": 0.5}}}}})
_t("aux-usb-trim", "/rtn/aux/preamp/rtntrim", ("f", 0.5), {"aux": {"usb_trim": 0.5}})
_t("aux-usb-return", "/rtn/aux/preamp/rtnsw", ("i", 1), {"aux": {"usb_return": True}})
_t("aux-usb-source", "/rtn/aux/config/rtnsrc", ("i", 8), {"aux": {"usb_source": 8}})

_t("fx-return-mute", "/rtn/2/mix/on", ("i", 0), {"fx_returns": {"2": {"mute": True}}})
_t("fx-return-fader", "/rtn/2/mix/fader", ("f", 0.25), {"fx_returns": {"2": {"fader": 0.25}}})
_t("fx-return-name", "/rtn/2/config/name", ("s", "Verb"), {"fx_returns": {"2": {"name": "Verb"}}})
_t("fx-return-color", "/rtn/2/config/color", ("i", 4), {"fx_returns": {"2": {"color": 4}}})
_t("fx-return-pan", "/rtn/2/mix/pan", ("f", 0.5), {"fx_returns": {"2": {"pan": 0.5}}})
_t("fx-return-send-level", "/rtn/4/mix/01/level", ("f", 0.75), {"fx_returns": {"4": {"sends": {"1": {"level": 0.75}}}}})
_t("fx-return-send-pan", "/rtn/4/mix/01/pan", ("f", 0.75), {"fx_returns": {"4": {"sends": {"1": {"pan": 0.75}}}}})
_t("fx-return-send-tap", "/rtn/4/mix/01/tap", ("i", 1), {"fx_returns": {"4": {"sends": {"1": {"tap": 1}}}}})
_t("fx-return-send-group", "/rtn/4/mix/01/grpon", ("i", 1), {"fx_returns": {"4": {"sends": {"1": {"group_on": True}}}}})
_t("fx-return-main-assign", "/rtn/4/mix/lr", ("i", 1), {"fx_returns": {"4": {"main_assign": True}}})
_t("fx-return-eq-band-type", "/rtn/4/eq/2/type", ("i", 3), {"fx_returns": {"4": {"eq": {"bands": {"2": {"type": 3}}}}}})
_t("fx-return-eq-band-frequency", "/rtn/4/eq/2/f", ("f", 0.5), {"fx_returns": {"4": {"eq": {"bands": {"2": {"frequency": 0.5}}}}}})
_t("fx-return-eq-band-gain", "/rtn/4/eq/2/g", ("f", 0.5), {"fx_returns": {"4": {"eq": {"bands": {"2": {"gain": 0.5}}}}}})
_t("fx-return-eq-band-q", "/rtn/4/eq/2/q", ("f", 0.5), {"fx_returns": {"4": {"eq": {"bands": {"2": {"q": 0.5}}}}}})
_t("fx-return-usb-trim", "/rtn/1/preamp/rtntrim", ("f", 0.5), {"fx_returns": {"1": {"usb_trim": 0.5}}})
_t("fx-return-usb-return", "/rtn/1/preamp/rtnsw", ("i", 0), {"fx_returns": {"1": {"usb_return": False}}})
_t("fx-return-usb-source", "/rtn/1/config/rtnsrc", ("i", 3), {"fx_returns": {"1": {"usb_source": 3}}})

_t("bus-mute", "/bus/6/mix/on", ("i", 0), {"buses": {"6": {"mute": True}}})
_t("bus-fader", "/bus/6/mix/fader", ("f", 0.75), {"buses": {"6": {"fader": 0.75}}})
_t("bus-name", "/bus/1/config/name", ("s", "Wedge 1"), {"buses": {"1": {"name": "Wedge 1"}}})
_t("bus-color", "/bus/1/config/color", ("i", 2), {"buses": {"1": {"color": 2}}})
_t("bus-pan", "/bus/1/mix/pan", ("f", 0.5), {"buses": {"1": {"pan": 0.5}}})
_t("bus-main-assign", "/bus/5/mix/lr", ("i", 1), {"buses": {"5": {"main_assign": True}}})
_t("bus-dca-groups", "/bus/5/grp/dca", ("i", 1), {"buses": {"5": {"dca_groups": 1}}})
_t("bus-mute-groups", "/bus/5/grp/mute", ("i", 8), {"buses": {"5": {"mute_groups": 8}}})
_t("bus-eq-on", "/bus/5/eq/on", ("i", 1), {"buses": {"5": {"eq_on": True}}})
_t("bus-eq-mode", "/bus/5/eq/mode", ("i", 1), {"buses": {"5": {"eq_mode": 1}}})
_t("bus-eq-band-type", "/bus/5/eq/6/type", ("i", 4), {"buses": {"5": {"eq": {"bands": {"6": {"type": 4}}}}}})
_t("bus-eq-band-frequency", "/bus/5/eq/6/f", ("f", 0.5), {"buses": {"5": {"eq": {"bands": {"6": {"frequency": 0.5}}}}}})
_t("bus-eq-band-gain", "/bus/5/eq/6/g", ("f", 0.5), {"buses": {"5": {"eq": {"bands": {"6": {"gain": 0.5}}}}}})
_t("bus-eq-band-q", "/bus/5/eq/6/q", ("f", 0.5), {"buses": {"5": {"eq": {"bands": {"6": {"q": 0.5}}}}}})
_t("bus-insert-on", "/bus/5/insert/on", ("i", 1), {"buses": {"5": {"insert_on": True}}})
_t("bus-insert-select", "/bus/5/insert/fxslot", ("i", 7), {"buses": {"5": {"insert_select": 7}}})

_t("fx-send-mute", "/fxsend/3/mix/on", ("i", 1), {"fx_sends": {"3": {"mute": False}}})
_t("fx-send-fader", "/fxsend/3/mix/fader", ("f", 0.5), {"fx_sends": {"3": {"fader": 0.5}}})
_t("fx-send-name", "/fxsend/3/config/name", ("s", "Delay"), {"fx_sends": {"3": {"name": "Delay"}}})
_t("fx-send-color", "/fxsend/3/config/color", ("i", 6), {"fx_sends": {"3": {"color": 6}}})
_t("fx-send-dca-groups", "/fxsend/3/grp/dca", ("i", 3), {"fx_sends": {"3": {"dca_groups": 3}}})
_t("fx-send-mute-groups", "/fxsend/3/grp/mute", ("i", 0), {"fx_sends": {"3": {"mute_groups": 0}}})

_t("main-mute", "/lr/mix/on", ("i", 0), {"main": {"mute": True}})
_t("main-fader", "/lr/mix/fader", ("f", 0.5), {"main": {"fader": 0.5}})
_t("main-name", "/lr/config/name", ("s", "LR"), {"main": {"name": "LR"}})
_t("main-color", "/lr/config/color", ("i", 7), {"main": {"color": 7}})
_t("main-pan", "/lr/mix/pan", ("f", 0.5), {"main": {"pan": 0.5}})
_t("main-eq-on", "/lr/eq/on", ("i", 0), {"main": {"eq_on": False}})
_t("main-eq-mode", "/lr/eq/mode", ("i", 2), {"main": {"eq_mode": 2}})
_t("main-eq-band-type", "/lr/eq/5/type", ("i", 1), {"main": {"eq": {"bands": {"5": {"type": 1}}}}})
_t("main-eq-band-frequency", "/lr/eq/5/f", ("f", 0.25), {"main": {"eq": {"bands": {"5": {"frequency": 0.25}}}}})
_t("main-eq-band-gain", "/lr/eq/5/g", ("f", 0.25), {"main": {"eq": {"bands": {"5": {"gain": 0.25}}}}})
_t("main-eq-band-q", "/lr/eq/5/q", ("f", 0.25), {"main": {"eq": {"bands": {"5": {"q": 0.25}}}}})

_t("dca-mute", "/dca/3/on", ("i", 0), {"dcas": {"3": {"mute": True}}})
_t("dca-fader", "/dca/3/fader", ("f", 0.75), {"dcas": {"3": {"fader": 0.75}}})
_t("dca-name", "/dca/3/config/name", ("s", "Drums"), {"dcas": {"3": {"name": "Drums"}}})
_t("dca-color", "/dca/3/config/color", ("i", 5), {"dcas": {"3": {"color": 5}}})
_t("solo-active", "/-stat/solo", ("i", 1), {"solo_active": True})
_t("mute-group", "/config/mute/4", ("i", 1), {"mute_groups": {"4": {"muted": True}}})
for wire, key, arg, sv in [("level", "level", ("f", 0.75), 0.75), ("source", "source", ("i", 4), 4),
                           ("sourcetrim", "source_trim", ("f", 0.5), 0.5), ("chmode", "channel_mode", ("i", 1), 1),
                           ("busmode", "bus_mode", ("i", 0), 0), ("dim", "dim", ("i", 1), True),
                           ("dimatt", "dim_gain", ("f", 0.25), 0.25), ("dimpfl", "pfl_dim", ("i", 0), False),
                           ("mono", "mono", ("i", 1), True), ("mute", "mute", ("i", 1), True)]:
    _t(f"monitor-{wire}", f"/config/solo/{wire}", arg, {"monitor": {key: sv}})
_t("channel-link", "/config/chlink/1-2", ("i", 1), {"channel_links": {"1-2": True}})
_t("bus-link", "/config/buslink/5-6", ("i", 0), {"bus_links": {"5-6": False}})
_t("link-preference", "/config/linkcfg/preamp", ("i", 1), {"link_preferences": {"preamp": True}})
_t("automix-enable", "/config/amixenable/X", ("i", 1), {"automix": {"X": {"enabled": True}}})
_t("automix-lock", "/config/amixlock/Y", ("i", 1), {"automix": {"Y": {"locked": True}}})
_t("headamp-gain", "/headamp/01/gain", ("f", 0.5), {"headamps": {"1": {"gain": 0.5}}})
_t("headamp-phantom", "/headamp/16/phantom", ("i", 1), {"headamps": {"16": {"phantom": True}}})
_t("fx-type", "/fx/2/type", ("i", 11), {"fx": {"2": {"type": 11}}})
_t("fx-insert", "/fx/2/insert", ("i", 0), {"fx": {"2": {"insert": False}}})
_t("fx-parameter", "/fx/2/par/05", ("f", 0.5), {"fx": {"2": {"parameters": {"5": 0.5}}}})
_t("routing-aux-source", "/routing/aux/03/src", ("i", 26), {"routing": {"aux": {"3": {"source": 26}}}})
_t("routing-aux-tap", "/routing/aux/03/pos", ("i", 8), {"routing": {"aux": {"3": {"tap": 8}}}})
_t("routing-main", "/routing/main/01/src", ("i", 0), {"routing": {"main_source": 0}})
_t("routing-phones", "/routing/main/02/src", ("i", 1), {"routing": {"phones_source": 1}})
_t("routing-ultranet-source", "/routing/p16/16/src", ("i", 15), {"routing": {"ultranet": {"16": {"source": 15}}}})
_t("routing-ultranet-tap", "/routing/p16/16/pos", ("i", 10), {"routing": {"ultranet": {"16": {"tap": 10}}}})
_t("routing-usb-source", "/routing/usb/18/src", ("i", 37), {"routing": {"usb": {"18": {"source": 37}}}})
_t("routing-usb-tap", "/routing/usb/18/pos", ("i", 2), {"routing": {"usb": {"18": {"tap": 2}}}})
_t("snapshot-index", "/-snap/index", ("i", 12), {"snapshot": {"index": 12}})
_t("snapshot-name", "/-snap/name", ("s", "Act 2"), {"snapshot": {"name": "Act 2"}})
_t("snapshot-slot-name", "/-snap/12/name", ("s", "Act 2"), {"snapshots": {"12": {"name": "Act 2"}}})
_t("snapshot-scope", "/-snap/12/scope", ("s", "+" * 59), {"snapshots": {"12": {"scope": "+" * 59}}})
_t("recorder-state", "/-stat/tape/state", ("i", 4), {"recorder": {"state": 4}})
_t("recorder-elapsed", "/-stat/tape/etime", ("i", 125), {"recorder": {"elapsed": 125}})
_t("recorder-remaining", "/-stat/tape/rtime", ("i", -3), {"recorder": {"remaining": -3}})
_t("recorder-file", "/-stat/tape/file", ("s", "R_20260101-120000.wav"), {"recorder": {"file": "R_20260101-120000.wav"}})
_t("usb-mounted", "/-stat/usbmounted", ("i", 1), {"recorder": {"usb_mounted": True}})
_t("console-name", "/-prefs/name", ("s", "XR18-1A-2B-3C"), {"console": {"name": "XR18-1A-2B-3C"}})
