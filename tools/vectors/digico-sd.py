# DiGiCo SD and Quantum (digico-sd): OSC over UDP to the console's Other OSC
# Receive port. Addresses, type tags and ranges are written here from DiGiCo's
# "OSC Command List for Other OSC" (17/11/2014): every address starts /sd/,
# "*" is the strip number, Int controls take i, Float controls f 0.0-1.0 and
# names s (page 1 examples: /sd/Input_Channels/1/mute, i,1 is mute on). The
# console never answers, so every write is unverified.
DG = "digico-sd"

UNVERIFIED = {"ok": {"kind": "unverified"}}

# (command stem, parameter name, number used, address prefix)
_DG_STRIPS = {
    "input": ("channel", 12, "/sd/Input_Channels/12"),
    "aux": ("aux", 3, "/sd/Aux_Outputs/3"),
    "group": ("group", 192, "/sd/Group_Outputs/192"),
    "matrix": ("matrix", 64, "/sd/Matrix_Outputs/64"),
}

# The EQ and Dynamics sections, the same on every strip apart from the band
# counts: (command suffix, list leaf, kind, bands). kind: b = Int 0-1 switch,
# f = Float 0-1, i<lo>-<hi> = Int range.
_DG_EQ_IN = [
    ("eq_in", "EQ/eq_in", "b", None), ("eq_on", "EQ/eq_on_", "b", 4), ("eq_curve", "EQ/eq_curve_", "i1-4", 4),
    ("eq_freq", "EQ/eq_freq_", "f", 4), ("eq_q", "EQ/eq_Q_", "f", 4), ("eq_gain", "EQ/eq_gain_", "f", 4),
    ("eq_symm_q", "EQ/eq_symm_Q_", "b", 4),
]
_DG_EQ_OUT = [
    ("eq_in", "EQ/eq_in", "b", None), ("eq_on", "EQ/eq_on_", "b", 8), ("eq_curve", "EQ/eq_curve_", "i1-4", 8),
    ("eq_freq", "EQ/eq_freq_", "f", 8), ("eq_q", "EQ/eq_Q_", "f", 8), ("eq_gain", "EQ/eq_gain_", "f", 8),
    ("eq_symm_q", "EQ/eq_symm_Q_", "b", 8), ("eq_pre_ins", "EQ/eq_pre-ins", "b", None),
]
_DG_DYN_EQ = [
    ("eq_over_under", "EQ/eq_over-under_", "i0-1", 4), ("eq_thresh", "EQ/eq_thresh_", "f", 4),
    ("eq_ratio", "EQ/eq_ratio_", "f", 4), ("eq_attack", "EQ/eq_attack_", "f", 4),
    ("eq_release", "EQ/eq_release_", "f", 4), ("dynamic_eq_on", "EQ/dynamic_eq_on_", "b", 4),
]
_DG_DYN = [
    ("comp_in", "Dynamics/comp_in", "b", None), ("gate_in", "Dynamics/gate_in", "b", None),
    ("comp_band_in", "Dynamics/comp_band_in_", "b", 3),
    ("comp_lp_crossover", "Dynamics/comp_LP_crossover", "f", None),
    ("comp_hp_crossover", "Dynamics/comp_HP_crossover", "f", None),
    ("comp_gain", "Dynamics/comp_gain_", "f", 4), ("gate_range", "Dynamics/gate_range", "f", None),
    ("comp_thresh", "Dynamics/comp_thresh_", "f", 3), ("gate_thresh", "Dynamics/gate_thresh", "f", None),
    ("comp_knee", "Dynamics/comp_knee_", "i0-2", 4), ("comp_ratio", "Dynamics/comp_ratio_", "f", 4),
    ("comp_attack", "Dynamics/comp_attack_", "f", 3), ("gate_attack", "Dynamics/gate_attack", "f", None),
    ("comp_release", "Dynamics/comp_release_", "f", 3), ("gate_release", "Dynamics/gate_release", "f", None),
    ("gate_hold", "Dynamics/gate_hold", "f", None), ("key_solo", "Dynamics/key_solo", "b", None),
    ("comp_listen", "Dynamics/comp_listen_", "b", 3), ("comp_all_gain", "Dynamics/comp_all_gain", "f", None),
    ("comp_all_thresh", "Dynamics/comp_all_thresh", "f", None),
    ("comp_auto_gain", "Dynamics/comp_auto-gain_", "b", 4),
    ("desser_centre_freq", "Dynamics/desser_centre_freq", "f", None),
    ("gate_centre_freq", "Dynamics/gate_centre_freq", "f", None),
    ("desser_freq_width", "Dynamics/desser_freq_width", "f", None),
    ("gate_freq_width", "Dynamics/gate_freq_width", "f", None),
]
_DG_COMMON = [
    ("insert_a_in", "Insert/insert_A_in", "b", None), ("insert_b_in", "Insert/insert_B_in", "b", None),
    ("delay_on", "Channel_Delay/delay_on", "b", None), ("delay", "Channel_Delay/delay", "f", None),
    ("fine_delay", "Channel_Delay/fine_delay", "f", None),
]
_DG_INPUT_ONLY = [
    ("trim", "Channel_Input/trim", "f", None), ("phase", "Channel_Input/phase", "i0-3", None),
    ("phantom", "Channel_Input/phantom", "b", None), ("alt_phantom", "Channel_Input/alt_phantom", "b", None),
    ("analog_gain", "Channel_Input/analog_gain", "f", None),
    ("alt_analog_gain", "Channel_Input/alt_analog_gain", "f", None),
    ("pad", "Channel_Input/input_pad", "b", None), ("alt_pad", "Channel_Input/alt_input_pad", "b", None),
    ("hi_filter_in", "Filters/hi_filter_in", "b", None), ("lo_filter_in", "Filters/lo_filter_in", "b", None),
    ("hi_filter_freq", "Filters/hi_filter_freq", "f", None), ("lo_filter_freq", "Filters/lo_filter_freq", "f", None),
    ("pan", "Panner/pan", "f", None), ("front_back", "Panner/f-b", "f", None),
]
_DG_BUSS = [("buss_trim", "Buss_Trim/trim", "f", None), ("buss_phase", "Buss_Trim/phase", "i0-3", None)]


def _dg_param(stem, suffix, leaf, kind, bands, k):
    p, n, prefix = _DG_STRIPS[stem]
    inp = {p: n}
    if bands:
        band = 1 + k % bands
        inp["band"] = band
        leaf = leaf + str(band)
    address = f"{prefix}/{leaf}"
    if kind == "b":
        on = k % 2 == 0
        inp["enabled"] = on
        return inp, address, ("i", 1 if on else 0)
    if kind == "f":
        v = [0.0, 0.25, 0.5, 0.75, 1.0][k % 5]
        inp["value"] = v
        return inp, address, ("f", v)
    lo, hi = (int(x) for x in kind[1:].split("-"))
    v = hi if k % 2 else lo
    inp["value"] = v
    return inp, address, ("i", v)


for stem, table in [
        ("input", _DG_INPUT_ONLY + _DG_EQ_IN + _DG_DYN_EQ + _DG_DYN + _DG_COMMON),
        ("aux", _DG_EQ_OUT + _DG_DYN_EQ + _DG_DYN + _DG_COMMON + _DG_BUSS),
        ("group", _DG_EQ_OUT + _DG_DYN_EQ + _DG_DYN + _DG_COMMON + _DG_BUSS),
        ("matrix", _DG_EQ_OUT + _DG_DYN_EQ + _DG_DYN + _DG_COMMON + _DG_BUSS)]:
    for k, (suffix, leaf, kind, bands) in enumerate(table):
        inp, address, arg = _dg_param(stem, suffix, leaf, kind, bands, k)
        binary(DG, f"set_{stem}_{suffix}", inp, osc(address, arg), expect_result=UNVERIFIED)

# Fader, mute, solo and name on every strip, control groups included. The
# names sit under Channel_Input on inputs and Buss_Trim on output busses.
for stem, p, n, prefix, name_leaf in [
        ("input", "channel", 1, "/sd/Input_Channels/1", "Channel_Input/name"),
        ("aux", "aux", 16, "/sd/Aux_Outputs/16", "Buss_Trim/name"),
        ("group", "group", 2, "/sd/Group_Outputs/2", "Buss_Trim/name"),
        ("matrix", "matrix", 1, "/sd/Matrix_Outputs/1", "Buss_Trim/name"),
        ("control_group", "control_group", 36, "/sd/Control_Groups/36", "name")]:
    binary(DG, f"set_{stem}_fader", {p: n, "level": 1.0}, osc(f"{prefix}/fader", ("f", 1.0)),
           expect_result=UNVERIFIED)
    binary(DG, f"mute_{stem}", {p: n, "muted": True}, osc(f"{prefix}/mute", ("i", 1)))
    binary(DG, f"solo_{stem}", {p: n, "soloed": False}, osc(f"{prefix}/solo", ("i", 0)))
    # The list's example renames input 1 to FRED (page 1).
    binary(DG, f"set_{stem}_name", {p: n, "name": "FRED"}, osc(f"{prefix}/{name_leaf}", ("s", "FRED")))

# Sends.
binary(DG, "set_input_aux_send_level", {"channel": 5, "aux": 2, "value": 0.75},
       osc("/sd/Input_Channels/5/Aux_Send/2/send_level", ("f", 0.75)))
binary(DG, "set_input_aux_send_on", {"channel": 5, "aux": 2, "enabled": False},
       osc("/sd/Input_Channels/5/Aux_Send/2/send_on", ("i", 0)))
binary(DG, "set_input_aux_send_pan", {"channel": 5, "aux": 2, "value": 0.5},
       osc("/sd/Input_Channels/5/Aux_Send/2/send_pan", ("f", 0.5)))
binary(DG, "set_input_group_send", {"channel": 7, "group": 1},
       osc("/sd/Input_Channels/7/Group_Send/1/group", ("i", 1)))
binary(DG, "set_group_group_send", {"group": 3, "destination": 4, "enabled": True},
       osc("/sd/Group_Outputs/3/Group_Send/4/group", ("i", 1)))

# Graphic EQ.
binary(DG, "set_geq_in", {"geq": 1, "enabled": True}, osc("/sd/Graphic_EQ/1/geq_in", ("i", 1)))
binary(DG, "set_geq_band_gain", {"geq": 2, "band": 32, "value": 0.5},
       osc("/sd/Graphic_EQ/2/geq_gain_32", ("f", 0.5)))

# Session, snapshots and macros: Int arguments; next/previous and save take 0
# (their range is 0-0).
binary(DG, "save_session", {}, osc("/sd/Filing/Save_current_Session", ("i", 0)))
binary(DG, "fire_snapshot", {"snapshot": 12}, osc("/sd/Snapshots/Fire_Snapshot_number", ("i", 12)),
       expect_result=UNVERIFIED)
binary(DG, "fire_next_snapshot", {}, osc("/sd/Snapshots/Fire_Next_Snapshot", ("i", 0)))
binary(DG, "fire_previous_snapshot", {}, osc("/sd/Snapshots/Fire_Prev_Snapshot", ("i", 0)))
# Macro 1 is button 0 on the wire (range 0-255).
binary(DG, "press_macro", {"macro": 1}, osc("/sd/Macros/Buttons/press", ("i", 0)))

# The families the list explains by name only (quirks): main/alt_in, the
# strips' CGs_level and CGs_mute, Matrix_Inputs sends and Multis, as the
# list's rows read (pages 1-12).
binary(DG, "set_input_alt_in", {"channel": 3, "alternate": True},
       osc("/sd/Input_Channels/3/Channel_Input/main/alt_in", ("i", 1)), expect_result=UNVERIFIED)
for stem, p, n, prefix in [("input", "channel", 1, "/sd/Input_Channels/1"), ("aux", "aux", 192, "/sd/Aux_Outputs/192"),
                           ("group", "group", 2, "/sd/Group_Outputs/2"), ("matrix", "matrix", 64, "/sd/Matrix_Outputs/64")]:
    binary(DG, f"set_{stem}_cgs_level", {p: n, "level": 0.25}, osc(f"{prefix}/CGs_level", ("f", 0.25)))
    binary(DG, f"set_{stem}_cgs_mute", {p: n, "muted": False}, osc(f"{prefix}/CGs_mute", ("i", 0)))
binary(DG, "set_matrix_input_send_level", {"input": 2, "matrix": 64, "value": 0.5},
       osc("/sd/Matrix_Inputs/2/Matrix_Send/64/send_level", ("f", 0.5)))
binary(DG, "set_matrix_input_send_on", {"input": 64, "matrix": 1, "enabled": True},
       osc("/sd/Matrix_Inputs/64/Matrix_Send/1/send_on", ("i", 1)))
binary(DG, "solo_multi", {"multi": 1, "soloed": True}, osc("/sd/Multis/1/solo", ("i", 1)))
binary(DG, "set_multi_fader", {"multi": 4, "level": 0.75}, osc("/sd/Multis/4/fader", ("f", 0.75)))
binary(DG, "mute_multi", {"multi": 4, "muted": True}, osc("/sd/Multis/4/mute", ("i", 1)))
binary(DG, "set_multi_name", {"multi": 2, "name": "Band"}, osc("/sd/Multis/2/name", ("s", "Band")))

# Any address of the list with an argument of the row's type.
binary(DG, "set_parameter_float", {"path": "sd/Input_Channels/1/fader", "value": 1.0},
       osc("/sd/Input_Channels/1/fader", ("f", 1.0)), expect_result=UNVERIFIED)
binary(DG, "set_parameter_int", {"path": "sd/Input_Channels/1/mute", "value": 1},
       osc("/sd/Input_Channels/1/mute", ("i", 1)))
binary(DG, "set_parameter_string", {"path": "sd/Input_Channels/1/Channel_Input/name", "value": "FRED"},
       osc("/sd/Input_Channels/1/Channel_Input/name", ("s", "FRED")))
