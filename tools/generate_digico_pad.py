"""Generate specs/digico-pad.yaml and tools/vectors/digico-pad.py: DiGiCo SD and
Quantum consoles over the DiGiCo Pad (iPad) OSC command set.

    python tools/generate_digico_pad.py

Edit this file, not its output: tools/test_generate_digico_pad.py fails when
the committed spec or vector module differs from what it writes.

DiGiCo publishes no copy of the command set. Its addresses, value types and
ranges are restated here from the parameter table of DiGiCo's MIT-licensed
Bitfocus Companion module (digico_osc.csv and digico_entities.csv), and the
query, list and change-notice behaviour from that module's code: every
address below a strip number is a parameter, a query is the address with
"/?" appended and no arguments (or one int, an index), the console answers
on the address itself and sends "<path>/!" when something under <path>
changed. Times are sent in seconds (the module divides milliseconds by
1000), levels in dB, frequencies in Hz. Nothing of it is documented by
DiGiCo, so every behaviour is a quirk and a VERIFICATION.md item.
"""
import json
import struct
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent
SID = "digico-pad"


def q(s):
    return json.dumps(s, ensure_ascii=False)


# ── Value kinds ───────────────────────────────────────────────────────────
class B:
    """On/off, sent as a float 0 or 1 (int with wire 'i')."""
    def __init__(self, wire="float"):
        self.wire = wire


class N:
    """A number in the console's own unit."""
    def __init__(self, lo, hi, unit=None):
        self.lo, self.hi, self.unit = lo, hi, unit


class R(N):
    """A number whose meaning the source does not give: kept as sent."""


class E:
    """Named values, each a number on the wire."""
    def __init__(self, values):
        self.values = values  # wire number -> name


class S:
    """Text."""


BOOL_MAP = '{ "0": false, "1": true, "0.0": false, "1.0": true }'


def enum_map(values):
    pairs = []
    for wire, name in values.items():
        pairs.append(f'"{wire}": {q(name)}')
        pairs.append(f'"{wire}.0": {q(name)}')
    return "{ " + ", ".join(pairs) + " }"


class P:
    """One parameter of a strip: its address below the strip ({b} a band,
    {d} a destination strip), its state key below the strip (* for a band or
    destination), its kind, a label and a description, the command that sets
    it, and whether the source itself marks its meaning as a guess."""
    def __init__(self, leaf, key, kind, label, desc, cmd, bands=None, maybe=False):
        self.leaf, self.key, self.kind, self.label, self.desc = leaf, key, kind, label, desc
        self.cmd, self.bands, self.maybe = cmd, bands, maybe


# ── The strips ────────────────────────────────────────────────────────────
SECTIONS = {
    "input": dict(prefix="/Input_Channels", param="channel", max=384, root="inputs",
                  noun="input channel", plural="input channels", label="Channel", name_leaf="Channel_Input/name"),
    "aux": dict(prefix="/Aux_Outputs", param="aux", max=192, root="aux_outputs",
                noun="aux output", plural="aux outputs", label="Aux output", name_leaf="Buss_Trim/name"),
    "group": dict(prefix="/Group_Outputs", param="group", max=192, root="group_outputs",
                  noun="group output", plural="group outputs", label="Group output", name_leaf="Buss_Trim/name"),
    "matrix": dict(prefix="/Matrix_Outputs", param="matrix", max=64, root="matrix_outputs",
                   noun="matrix output", plural="matrix outputs", label="Matrix output", name_leaf="Buss_Trim/name"),
    "matrix_input": dict(prefix="/Matrix_Inputs", param="matrix_input", max=64, root="matrix_inputs",
                         noun="matrix input", plural="matrix inputs", label="Matrix input",
                         name_leaf="Channel_Input/name"),
    "control_group": dict(prefix="/Control_Groups", param="control_group", max=36, root="control_groups",
                          noun="control group", plural="control groups", label="Control group", name_leaf="name"),
    "geq": dict(prefix="/Graphic_EQ", param="geq", max=48, root="graphic_eqs",
                noun="graphic EQ", plural="graphic EQs", label="Graphic EQ", name_leaf="name"),
    "multi": dict(prefix="/Multis", param="multi", max=256, root="multis",
                  noun="multi", plural="multis", label="Multi", name_leaf="name"),
}


def main_params(fader=True):
    rows = []
    if fader:
        rows.append(P("fader", "fader", N(-150, 10, "dB"), "Fader level",
                      "Fader level in dB, -150 (the bottom of the fader) to +10.", "fader"))
    rows += [
        P("mute", "mute", B(), "Muted", "True mutes the strip, false unmutes it.", "mute"),
        P("solo", "solo", B(), "Soloed", "True solos the strip, false clears its solo.", "solo"),
    ]
    return rows


def cg_send():
    return [
        P("CGs_level", "control_group_level", N(-150, 10, "dB"), "Control group level",
          "Level the strip takes from its control groups, in dB, -150 to +10.", "cgs_level"),
        P("CGs_mute", "control_group_mute", B(), "Control group mute",
          "True when the strip's control groups mute it.", "cgs_mute"),
    ]


def delay():
    return [
        P("Channel_Delay/delay_on", "delay.enabled", B(), "Delay on", "True switches the channel delay in.",
          "delay_on"),
        P("Channel_Delay/delay", "delay.time", N(0, 1.3, "s"), "Delay time",
          "Channel delay in seconds, 0 to 1.3 (1300 ms).", "delay"),
        P("Channel_Delay/fine_delay", "delay.fine", N(-127, 127, "samples"), "Fine delay",
          "Fine delay adjustment in samples, -127 to +127.", "fine_delay"),
    ]


def filters():
    return [
        P("Filters/hi_filter_in", "filters.hi.enabled", B(), "Hi filter on",
          "True switches the hi filter in (the source reads it as the high-pass filter).", "hi_filter_in"),
        P("Filters/hi_filter_freq", "filters.hi.frequency", N(20, 20000, "Hz"), "Hi filter frequency",
          "Hi filter frequency in Hz, 20 to 20000.", "hi_filter_freq"),
        P("Filters/lo_filter_in", "filters.lo.enabled", B(), "Lo filter on",
          "True switches the lo filter in (the source reads it as the low-pass filter).", "lo_filter_in"),
        P("Filters/lo_filter_freq", "filters.lo.frequency", N(20, 20000, "Hz"), "Lo filter frequency",
          "Lo filter frequency in Hz, 20 to 20000.", "lo_filter_freq"),
    ]


def inserts():
    return [
        P("Insert/insert_A_in", "inserts.a.enabled", B(), "Insert A in", "True switches insert A in.", "insert_a_in"),
        P("Insert/insert_B_in", "inserts.b.enabled", B(), "Insert B in", "True switches insert B in.", "insert_b_in"),
    ]


def eq(attack_lo, release_lo):
    return [
        P("EQ/eq_in", "eq.enabled", B(), "EQ in", "True switches the strip's EQ in, false bypasses it.", "eq_in"),
        P("EQ/eq_on_{b}", "eq.bands.*.enabled", B(), "Band on", "True switches the EQ band on.", "eq_band_on",
          bands=4),
        P("EQ/eq_curve_{b}", "eq.bands.*.curve", R(1, 4), "Band curve",
          "Curve number as the console numbers it: 1 bell, and the source names 4 high shelf on band 1 and 3 "
          "low shelf on band 4; 2 is not named.", "eq_band_curve", bands=4),
        P("EQ/eq_freq_{b}", "eq.bands.*.frequency", N(20, 20000, "Hz"), "Band frequency",
          "EQ band frequency in Hz, 20 to 20000.", "eq_band_frequency", bands=4),
        P("EQ/eq_gain_{b}", "eq.bands.*.gain", N(-18, 18, "dB"), "Band gain",
          "EQ band gain in dB, -18 to +18.", "eq_band_gain", bands=4),
        P("EQ/eq_Q_{b}", "eq.bands.*.q", N(0.1, 20), "Band Q", "EQ band Q (bandwidth), 0.1 to 20.", "eq_band_q",
          bands=4),
        P("EQ/eq_symm_Q_{b}", "eq.bands.*.symmetric_q", B(), "Symmetric Q",
          "True sets the band's symmetric Q mode.", "eq_band_symmetric_q", bands=4),
        P("EQ/dynamic_eq_on_{b}", "eq.bands.*.dynamic", B(), "Dynamic EQ on",
          "True makes the band a dynamic EQ band.", "eq_band_dynamic", bands=4),
        P("EQ/eq_over-under_{b}", "eq.bands.*.dynamic_over_under", R(0, 1), "Dynamic over/under",
          "Dynamic EQ over/under mode, 0 or 1 as the console numbers it; the source does not say which is which.",
          "eq_band_dynamic_over_under", bands=4),
        P("EQ/eq_thresh_{b}", "eq.bands.*.dynamic_threshold", N(-60, 0, "dB"), "Dynamic threshold",
          "Dynamic EQ threshold in dB, -60 to 0.", "eq_band_dynamic_threshold", bands=4),
        P("EQ/eq_ratio_{b}", "eq.bands.*.dynamic_ratio", N(1, 10), "Dynamic ratio",
          "Dynamic EQ ratio, 1 to 10 (:1).", "eq_band_dynamic_ratio", bands=4),
        P("EQ/eq_attack_{b}", "eq.bands.*.dynamic_attack", N(attack_lo, 0.1, "s"), "Dynamic attack",
          f"Dynamic EQ attack in seconds, {attack_lo:g} to 0.1.", "eq_band_dynamic_attack", bands=4),
        P("EQ/eq_release_{b}", "eq.bands.*.dynamic_release", N(release_lo, 10, "s"), "Dynamic release",
          f"Dynamic EQ release in seconds, {release_lo:g} to 10.", "eq_band_dynamic_release", bands=4),
    ]


DYN_MODE = {0: "compressor", 1: "multiband", 2: "de_esser"}
KNEE = {0: "hard", 1: "medium", 2: "soft"}


def dynamics(mode_leaf, gate_mode=False):
    rows = [
        P("Dynamics/comp_in", "dynamics.compressor.enabled", B(), "Compressor in",
          "True switches the compressor in.", "comp_in"),
        P("Dynamics/gate_in", "dynamics.gate.enabled", B(), "Gate in", "True switches the gate in.", "gate_in"),
        P(f"Dynamics/{mode_leaf}", "dynamics.mode", E(DYN_MODE), "Dynamics mode",
          "compressor, multiband or de_esser (0, 1, 2 on the wire).", "dynamics_mode"),
    ]
    if gate_mode:
        rows.append(P("Dynamics/gate-duck-comp", "dynamics.gate.mode", R(0, 2), "Gate mode",
                      "Gate, ducker or compressor mode, 0 to 2 as the console numbers it; the source marks this "
                      "parameter as a guess and names no value.", "gate_mode", maybe=True))
    rows += [
        P("Dynamics/comp_all_gain", "dynamics.compressor.gain", N(0, 40, "dB"), "Compressor gain",
          "Overall compressor make-up gain in dB, 0 to 40.", "comp_gain"),
        P("Dynamics/comp_all_thresh", "dynamics.compressor.threshold", N(-60, 0, "dB"), "Compressor threshold",
          "Overall compressor threshold in dB, -60 to 0.", "comp_threshold"),
        P("Dynamics/comp_band_in_{b}", "dynamics.compressor.bands.*.enabled", B(), "Band in",
          "True switches the compressor band in.", "comp_band_in", bands=3),
        P("Dynamics/comp_gain_{b}", "dynamics.compressor.bands.*.gain", N(0, 40, "dB"), "Band gain",
          "Compressor band make-up gain in dB, 0 to 40.", "comp_band_gain", bands=4),
        P("Dynamics/comp_thresh_{b}", "dynamics.compressor.bands.*.threshold", N(-60, 0, "dB"), "Band threshold",
          "Compressor band threshold in dB, -60 to 0.", "comp_band_threshold", bands=3),
        P("Dynamics/comp_ratio_{b}", "dynamics.compressor.bands.*.ratio", N(1, 50), "Band ratio",
          "Compressor band ratio, 1 to 50 (:1).", "comp_band_ratio", bands=4),
        P("Dynamics/comp_attack_{b}", "dynamics.compressor.bands.*.attack", N(0.00002, 0.1, "s"), "Band attack",
          "Compressor band attack in seconds, 0.00002 to 0.1.", "comp_band_attack", bands=3),
        P("Dynamics/comp_release_{b}", "dynamics.compressor.bands.*.release", N(0.001, 10, "s"), "Band release",
          "Compressor band release in seconds, 0.001 to 10.", "comp_band_release", bands=3),
        P("Dynamics/comp_knee_{b}", "dynamics.compressor.bands.*.knee", E(KNEE), "Band knee",
          "hard, medium or soft (0, 1, 2 on the wire).", "comp_band_knee", bands=4),
        P("Dynamics/comp_auto-gain_{b}", "dynamics.compressor.bands.*.auto_gain", B(), "Band auto gain",
          "True switches the band's auto gain on.", "comp_band_auto_gain", bands=4),
        P("Dynamics/comp_listen_{b}", "dynamics.compressor.bands.*.listen", B(), "Band listen",
          "True listens to the compressor band.", "comp_band_listen", bands=3),
        P("Dynamics/comp_LP_crossover", "dynamics.compressor.lp_crossover", N(20, 20000, "Hz"), "LP crossover",
          "Multiband compressor low-pass crossover in Hz, 20 to 20000.", "comp_lp_crossover"),
        P("Dynamics/comp_HP_crossover", "dynamics.compressor.hp_crossover", N(20, 20000, "Hz"), "HP crossover",
          "Multiband compressor high-pass crossover in Hz, 20 to 20000.", "comp_hp_crossover"),
        P("Dynamics/desser_centre_freq", "dynamics.de_esser.centre", R(0, 255), "De-esser centre",
          "De-esser centre frequency as the console's number, 0 to 255; the source gives no unit.",
          "de_esser_centre"),
        P("Dynamics/desser_freq_width", "dynamics.de_esser.width", R(0, 255), "De-esser width",
          "De-esser frequency width as the console's number, 0 to 255; the source gives no unit.",
          "de_esser_width"),
        P("Dynamics/gate_centre_freq", "dynamics.gate.key_centre", R(0, 255), "Gate key centre",
          "Gate or ducker key filter centre frequency as the console's number, 0 to 255; the source gives no "
          "unit.", "gate_key_centre"),
        P("Dynamics/gate_freq_width", "dynamics.gate.key_width", R(0, 255), "Gate key width",
          "Gate or ducker key filter width as the console's number, 0 to 255; the source gives no unit.",
          "gate_key_width"),
        P("Dynamics/gate_attack", "dynamics.gate.attack", N(0.00005, 0.1, "s"), "Gate attack",
          "Gate attack in seconds, 0.00005 to 0.1.", "gate_attack"),
        P("Dynamics/gate_hold", "dynamics.gate.hold", N(0.002, 2, "s"), "Gate hold",
          "Gate hold in seconds, 0.002 to 2.", "gate_hold"),
        P("Dynamics/gate_range", "dynamics.gate.range", N(0, 90, "dB"), "Gate range",
          "Gate range in dB, 0 to 90.", "gate_range"),
        P("Dynamics/gate_release", "dynamics.gate.release", N(0.005, 10, "s"), "Gate release",
          "Gate release in seconds, 0.005 to 10.", "gate_release"),
        P("Dynamics/gate_thresh", "dynamics.gate.threshold", N(-60, 0, "dB"), "Gate threshold",
          "Gate threshold in dB, -60 to 0.", "gate_threshold"),
        P("Dynamics/key_solo", "dynamics.key_solo", B(), "Key solo",
          "True solos (listens to) the dynamics side-chain key.", "key_solo"),
    ]
    return rows


INPUT_ONLY = [
    P("Channel_Input/name", "name", S(), "Name", "Channel name as the console shows it.", "name"),
    P("Channel_Input/trim", "trim", N(-40, 60, "dB"), "Trim", "Digital input trim in dB, -40 to +60.", "trim"),
    P("Channel_Input/phase", "phase", B(), "Phase", "True inverts the channel's polarity.", "phase"),
    P("Channel_Input/phantom", "phantom", B(), "Phantom power", "True switches 48 V phantom power on.", "phantom"),
    P("Channel_Input/alt_phantom", "alt_phantom", B(), "Alternate phantom power",
      "True switches 48 V phantom power on for the alternate input.", "alt_phantom"),
    P("Channel_Input/analog_gain", "analog_gain", N(-20, 60, "dB"), "Analogue gain",
      "Analogue preamp gain in dB, -20 to +60.", "analog_gain"),
    P("Channel_Input/alt_analog_gain", "alt_analog_gain", N(-20, 60, "dB"), "Alternate analogue gain",
      "Analogue preamp gain of the alternate input in dB, -20 to +60.", "alt_analog_gain"),
    P("Channel_Input/input_pad", "pad", B(), "Pad", "True switches the input pad in.", "pad"),
    P("Channel_Input/alt_input_pad", "alt_pad", B(), "Alternate pad",
      "True switches the alternate input's pad in.", "alt_pad"),
    P("Channel_Input/main/alt_in", "alt_input", B(), "Alternate input",
      "True selects the alternate input, false the main input.", "alt_input"),
    P("Channel_Input/input_type", "input_type", R(0, 2), "Input type",
      "Input source type, 0 to 2 as the console numbers it; the source marks this parameter as a guess (mic or "
      "line) and names no value.", "input_type", maybe=True),
    P("Channel_Input/stereo_mode", "stereo_mode", E({1: "mono", 2: "stereo"}), "Stereo mode",
      "mono or stereo (1, 2 on the wire).", "stereo_mode"),
    P("Panner/pan", "pan", N(-1, 1), "Pan", "Pan position, -1 (left) to +1 (right).", "pan"),
    P("Panner/f-b", "front_back", N(0, 1), "Front-back pan",
      "Front-to-back pan position, 0 to 1; the source does not say which end is front.", "front_back"),
]

AUTO_SOLO = {0: "off", 1: "solo_1", 2: "solo_2", 3: "solo_1_and_2"}


def buss(auto_solo, sends_to):
    rows = [
        P("Buss_Trim/name", "name", S(), "Name", "Output name as the console shows it.", "name"),
        P("Buss_Trim/phase", "phase", B(), "Phase", "True inverts the output's polarity.", "phase"),
        P("Buss_Trim/trim", "trim", N(-40, 60, "dB"), "Buss trim", "Buss trim in dB, -40 to +60.", "trim"),
        P("hard_mute", "hard_mute", B(), "Hard mute", "True switches the output's hard mute on.", "hard_mute"),
        P("solo_1_or_2", "solo_bus", R(0, 2), "Solo bus",
          "Solo bus the output's solo goes to, 0 to 2 as the console numbers it (Solo 1, Solo 2 or both, in an "
          "order the source does not give).", "solo_bus"),
        P("alternate_solo", "alternate_solo", B(), "Alternate solo",
          "True switches the output's alternate solo on; the source marks this parameter as a guess.",
          "alternate_solo", maybe=True),
    ]
    if auto_solo:
        rows.append(P("auto_solo", "auto_solo", E(AUTO_SOLO), "Auto solo",
                      "off, solo_1, solo_2 or solo_1_and_2 (0 to 3 on the wire).", "auto_solo"))
    if sends_to:
        rows += [
            P("sends_to_faders", "sends_to_faders", B(), "Sends on faders",
              "True puts this output's sends on the faders.", "sends_to_faders"),
            P("sends_to_rotaries", "sends_to_rotaries", B(), "Sends on rotaries",
              "True puts this output's sends on the rotaries.", "sends_to_rotaries"),
        ]
    return rows


INPUT = (INPUT_ONLY + main_params() + cg_send() + delay() + filters() + inserts() + eq(0, 0)
         + dynamics("comp-multiband-desser", gate_mode=True))
AUX = (buss(True, True) + main_params() + cg_send() + delay() + filters() + inserts() + eq(0.0005, 0.01)
       + dynamics("comp-multiband-desser"))
GROUP = (buss(True, True) + main_params() + cg_send() + delay() + filters() + inserts() + eq(0.0005, 0.01)
         + dynamics("comp-multiband-desser"))
# The source spells the matrix outputs' dynamics mode comp_multiband-desser.
MATRIX = (buss(False, False) + main_params() + cg_send() + delay() + filters() + inserts() + eq(0.0005, 0.01)
          + dynamics("comp_multiband-desser"))
MATRIX_INPUT = [P("Channel_Input/name", "name", S(), "Name", "Matrix input name as the console shows it.", "name")]
CONTROL_GROUP = [
    P("name", "name", S(), "Name", "Control group name as the console shows it.", "name"),
    *main_params(),
    P("aux_send", "aux_send", N(-18, 18, "dB"), "Aux send",
      "Control group aux send level in dB, -18 to +18.", "aux_send"),
]
GEQ = [
    P("name", "name", S(), "Name", "Graphic EQ name as the console shows it.", "name"),
    P("geq_in", "enabled", B(), "Graphic EQ in", "True switches the graphic EQ in.", "geq_in"),
    P("geq_gain_{b}", "bands.*.gain", N(-12, 12, "dB"), "Band gain",
      "Graphic EQ band gain in dB, -12 to +12.", "geq_band_gain", bands=32),
]
MULTI = [P("name", "name", S(), "Name", "Multi name as the console shows it.", "name"), *main_params()]

STRIPS = {
    "input": INPUT, "aux": AUX, "group": GROUP, "matrix": MATRIX, "matrix_input": MATRIX_INPUT,
    "control_group": CONTROL_GROUP, "geq": GEQ, "multi": MULTI,
}

# Sends: strip section, destination section, rows ({d} the destination).
SENDS = [
    ("input", "aux", [
        P("Aux_Send/{d}/send_level", "aux_sends.*.level", N(-150, 10, "dB"), "Send level",
          "Aux send level in dB, -150 to +10.", "aux_send_level"),
        P("Aux_Send/{d}/send_on", "aux_sends.*.enabled", B(), "Send on", "True switches the aux send on.",
          "aux_send_on"),
        P("Aux_Send/{d}/send_pan", "aux_sends.*.pan", N(-1, 1), "Send pan",
          "Aux send pan, -1 (left) to +1 (right).", "aux_send_pan"),
    ]),
    ("group", "aux", [
        P("Aux_Send/{d}/send_level", "aux_sends.*.level", N(-150, 10, "dB"), "Send level",
          "Aux send level in dB, -150 to +10.", "aux_send_level"),
        P("Aux_Send/{d}/send_on", "aux_sends.*.enabled", B(), "Send on", "True switches the aux send on.",
          "aux_send_on"),
        P("Aux_Send/{d}/send_pan", "aux_sends.*.pan", N(-1, 1), "Send pan",
          "Aux send pan, -1 (left) to +1 (right).", "aux_send_pan"),
        P("Aux_Send/{d}/send_pre-post", "aux_sends.*.tap", R(0, 2), "Send tap",
          "Pre- or post-fader tap point of the send, 0 to 2 as the console numbers it; the source names no value.",
          "aux_send_tap"),
    ]),
    ("input", "group", [
        P("Group_Send/{d}/group", "group_sends.*.assigned", B(), "Routed to group",
          "True routes the channel to the group; the source marks this parameter as a guess.", "group_send",
          maybe=True),
    ]),
    ("group", "group", [
        P("Group_Send/{d}/group", "group_sends.*.assigned", B(), "Routed to group",
          "True routes the group to the other group; the source marks this parameter as a guess.", "group_send",
          maybe=True),
    ]),
    ("matrix_input", "matrix", [
        P("Matrix_Send/{d}/send_level", "matrix_sends.*.level", N(-150, 10, "dB"), "Send level",
          "Matrix send level in dB, -150 to +10.", "matrix_send_level"),
        P("Matrix_Send/{d}/send_on", "matrix_sends.*.enabled", B(), "Send on",
          "True switches the matrix send on.", "matrix_send_on"),
    ]),
]

# /Console/<section>/modes: one int per strip.
MODES = [("input", "Input_Channels", "1 mono, 2 stereo"), ("aux", "Aux_Outputs", "1 mono, 2 stereo"),
         ("group", "Group_Outputs", "1 to 6; the source names 1 mono and 2 stereo")]
COUNTS = [("Input_Channels", "input_channels"), ("Aux_Outputs", "aux_outputs"),
          ("Group_Outputs", "group_outputs"), ("Matrix_Inputs", "matrix_inputs"),
          ("Matrix_Outputs", "matrix_outputs"), ("Control_Groups", "control_groups"),
          ("Graphic_EQ", "graphic_eqs"), ("Multis", "multis"), ("Talkback_Outputs", "talkback_outputs")]

MODELS = [("sd5", "DiGiCo SD5"), ("sd7", "DiGiCo SD7"), ("sd8", "DiGiCo SD8"), ("sd9", "DiGiCo SD9"),
          ("sd10", "DiGiCo SD10"), ("sd11", "DiGiCo SD11"), ("sd12", "DiGiCo SD12"),
          ("quantum225", "DiGiCo Quantum225"), ("quantum338", "DiGiCo Quantum338"),
          ("quantum5", "DiGiCo Quantum5"), ("quantum7", "DiGiCo SD7 Quantum / Quantum7"),
          ("quantum852", "DiGiCo Quantum852")]


# ── Building the spec ─────────────────────────────────────────────────────
class Out:
    def __init__(self):
        self.commands = []      # (name, yaml lines)
        self.state = {}         # path -> (type, unit, description)
        self.rules = []         # yaml text blocks
        self.vectors = []       # python lines
        self.maybe = []         # addresses whose meaning the source marks as a guess


O = Out()


def dest_param(sec, dest):
    """The destination's parameter name: its own, or to_<name> when it is the source's."""
    p = SECTIONS[dest]["param"]
    return f"to_{p}" if dest == sec else p


def strip_param(sec, name=None):
    s = SECTIONS[sec]
    desc = "The " + s["noun"] + ", numbered from 1 as on the console (up to " + str(s["max"]) + ")."
    return (f"{name or s['param']}: {{ type: int, min: 1, max: {s['max']}, required: true, label: {s['label']}, "
            f"description: {q(desc)} }}")


def band_param(n):
    return (f"band: {{ type: int, min: 1, max: {n}, required: true, label: Band, "
            f"description: {q(f'Band, 1 to {n}.')} }}")


def value_param(row):
    k = row.kind
    label = row.label
    if isinstance(k, B):
        name = {"mute": "muted", "solo": "soloed"}.get(row.cmd, "enabled")
        return name, (f"{name}: {{ type: bool, default: true, label: {q(label)}, description: {q(row.desc)} }}"), \
            f"{{{name}:bool01}}", "int" if k.wire == "int" else "float"
    if isinstance(k, S):
        return "name", (f"name: {{ type: string, max_length: 64, required: true, label: {q(label)}, "
                        f"description: {q(row.desc + ' Up to 64 characters.')} }}"), "{name}", "string"
    if isinstance(k, E):
        vals = ", ".join(k.values.values())
        return "value", (f"value: {{ type: enum, values: [{vals}], required: true, label: {q(label)}, "
                         f"description: {q(row.desc)} }}"), None, "float"
    return "value", (f"value: {{ type: float, min: {k.lo:g}, max: {k.hi:g}, required: true, label: {q(label)}, "
                     f"description: {q(row.desc)} }}"), "{value}", "float"


MAPS = {}


def map_name(kind):
    for name, values in MAPS.items():
        if values == kind.values:
            return name
    name = {tuple(DYN_MODE.values()): "dynamics_mode", tuple(KNEE.values()): "knee",
            tuple(AUTO_SOLO.values()): "auto_solo", ("mono", "stereo"): "stereo_mode"}[tuple(kind.values.values())]
    MAPS[name] = kind.values
    return name


def leaf_address(leaf, band="{band}", dest=None):
    return leaf.replace("{b}", band).replace("{d}", dest or "{d}")


def leaf_regex(leaf):
    return leaf.replace("{b}", r"(\d+)").replace("{d}", r"(\d+)")


def state_desc(sec, row, dest=None):
    s = SECTIONS[sec]
    what = f"{row.label} of the {s['noun']}" + (f", to {SECTIONS[dest]['noun']} *" if dest else "") + \
        (", band *" if row.bands else "")
    return f"{what}: {row.desc}"


def state_type(kind):
    if isinstance(kind, B):
        return "bool"
    if isinstance(kind, S):
        return "string"
    if isinstance(kind, E):
        return "string"
    return "float"


def add_command(sec, row, dest=None):
    s = SECTIONS[sec]
    stem = sec
    if row.cmd in ("mute", "solo"):
        name = f"{row.cmd}_{stem}"
    else:
        name = f"set_{stem}_{row.cmd}"
    vname, vline, value_tpl, wire = value_param(row)
    params = [strip_param(sec)]
    if dest:
        params.append(strip_param(dest, dest_param(sec, dest)))
    if row.bands:
        params.append(band_param(row.bands))
    params.append(vline)
    addr = f"{s['prefix']}/{{{s['param']}}}/" + leaf_address(
        row.leaf, "{band}", "{" + dest_param(sec, dest) + "}" if dest else None)
    if isinstance(row.kind, E):
        value_tpl = f"{{value:map.{map_name(row.kind)}}}"
    target = s["noun"] + (f"'s send to a {SECTIONS[dest]['noun']}" if dest else "")
    article = "an" if target[0] in "aeiou" else "a"
    where = row.leaf.replace("{b}", "<band>").replace("{d}", "<n>")
    summary = f"Set the {row.label.lower()} of {article} {target} ({where})"
    lines = [f"    summary: {q(summary)}", "    params:"]
    lines += [f"      {p}" for p in params]
    lines.append(f"    send: {{ address: {q(addr)}, args: [ {{ value: {q(value_tpl)}, type: {wire} }} ] }}")
    lines.append("    returns: none")
    O.commands.append((name, lines))
    return name


def state_path(sec, row, dest=None):
    s = SECTIONS[sec]
    return f"{s['root']}.*.{row.key}"


def rule_value(row, arg="{arg0}"):
    k = row.kind
    if isinstance(k, B):
        return f'{{ value: "{arg}", map: {BOOL_MAP} }}'
    if isinstance(k, E):
        return f'{{ value: "{arg}", map: {enum_map(k.values)} }}'
    return q(arg)


def add_param(sec, row, dest=None):
    s = SECTIONS[sec]
    path = state_path(sec, row, dest)
    unit = getattr(row.kind, "unit", None) if isinstance(row.kind, N) else None
    O.state[path] = (state_type(row.kind), unit, state_desc(sec, row, dest))
    # The rule: the strip's number is {1}; a band or destination is {2}.
    addr = f"^{s['prefix']}/(\\d+)/{leaf_regex(row.leaf)}$"
    key = row.key.replace("*", "{2}")
    O.rules.append(f"    - address: '{addr}'\n      state: {{ \"{s['root']}.{{1}}.{key}\": {rule_value(row)} }}")
    if row.maybe:
        O.maybe.append(f"{s['prefix']}/*/{row.leaf.replace('{b}', '*').replace('{d}', '*')}")
    return add_command(sec, row, dest)


def query(address, reply=None):
    reply = reply or "^" + address + "$"
    return f"{{ address: {q(address + '/?')}, reply_address: {q(reply)} }}"


def strip_reads(sec, n):
    """The queries that read every parameter of strip n (a template)."""
    s = SECTIONS[sec]
    items = []
    for row in STRIPS[sec]:
        if row.leaf == s["name_leaf"]:
            continue
        for band in (range(1, row.bands + 1) if row.bands else [None]):
            leaf = row.leaf.replace("{b}", str(band)) if band else row.leaf
            items.append(query(f"{s['prefix']}/{n}/{leaf}"))
    return items


# Strip parameters, their commands and state.
for sec, rows in STRIPS.items():
    for row in rows:
        add_param(sec, row)
for sec, dest, rows in SENDS:
    for row in rows:
        add_param(sec, row, dest)

# Strip counts and modes.
for wire, key in COUNTS:
    O.state[f"console.counts.{key}"] = ("int", None, f"How many {key.replace('_', ' ')} the session has, as the "
                                        f"console reports it (/Console/{wire}).")
    O.rules.append(f"    - address: '^/Console/{wire}$'\n      state: {{ console.counts.{key}: \"{{arg0}}\" }}")
for sec, wire, meaning in MODES:
    s = SECTIONS[sec]
    O.state[f"{s['root']}.*.mode"] = ("int", None, f"Format of the {s['noun']} from /Console/{wire}/modes, one "
                                      f"number per strip in strip order: {meaning}.")
    assigns = ", ".join(f'"{s["root"]}.{i + 1}.mode": "{{arg{i}}}"' for i in range(s["max"]))
    O.rules.append(f"    - address: '^/Console/{wire}/modes$'\n      state: {{ {assigns} }}")


# ── Commands that are not a strip parameter ───────────────────────────────
def cmd(name, summary, params, send, returns="none", expect=None):
    lines = [f"    summary: {q(summary)}"]
    if params:
        lines.append("    params:")
        lines += [f"      {p}" for p in params]
    if isinstance(send, list):
        lines.append("    send:")
        lines += [f"      - {m}" for m in send]
    else:
        lines.append(f"    send: {send}")
    if expect:
        lines.append(f"    expect: {expect}")
    lines.append(f"    returns: {returns}")
    O.commands.append((name, lines))


def osc_msg(address, *args):
    if not args:
        return f"{{ address: {q(address)} }}"
    return f"{{ address: {q(address)}, args: [ {', '.join(f'{{ value: {q(v)}, type: {t} }}' for v, t in args)} ] }}"


SNAP = ("snapshot: { type: int, min: 0, max: 9999, required: true, label: Snapshot, "
        "description: \"The snapshot's index in the console's list, as snapshots.list keys it (not its number).\" }")
PRESET = ("preset: { type: int, min: 0, max: 9999, required: true, label: Preset, "
          "description: \"The preset's index, as presets.list keys it.\" }")
MACRO = ("macro: { type: int, min: 0, max: 9999, required: true, label: Macro, "
         "description: \"The macro's index, as macros.list keys it.\" }")
NAME = ("name: { type: string, max_length: 64, required: true, label: Name, "
        "description: \"The new name. Up to 64 characters.\" }")
ENABLED = "enabled: {{ type: bool, default: true, label: {label}, description: {desc} }}"
SECTION = ("section: { type: string, max_length: 64, pattern: '^[A-Za-z0-9 _-]+$', required: true, "
           "label: Section, description: \"The console section the preset applies to, as presets.list.*.section "
           "gives it (Input Channels, Aux Outputs, ...).\" }")
TARGET = ("strip: { type: int, min: 1, max: 384, required: true, label: Strip, "
          "description: \"The strip of that section, numbered from 1 as on the console.\" }")

cmd("recall_snapshot", "Recall a snapshot on the live console (Snapshots/Recall_Snapshot/<index>)",
    [SNAP], osc_msg("/Snapshots/Recall_Snapshot/{snapshot}"))
cmd("fire_next_snapshot", "Fire the snapshot after the current one (Snapshots/Fire_Next_Snapshot)", [],
    osc_msg("/Snapshots/Fire_Next_Snapshot"))
cmd("fire_previous_snapshot", "Fire the snapshot before the current one (Snapshots/Fire_Prev_Snapshot)", [],
    osc_msg("/Snapshots/Fire_Prev_Snapshot"))
cmd("fire_selected_snapshot", "Fire the snapshot selected on the console surface (Snapshots/Fire_Selected_Snapshot)",
    [], osc_msg("/Snapshots/Fire_Selected_Snapshot"))
cmd("select_surface_snapshot", "Select a snapshot on the console surface without firing it "
    "(Snapshots/Change_Surface_Snapshot/<index>)", [SNAP], osc_msg("/Snapshots/Change_Surface_Snapshot/{snapshot}"))
cmd("new_snapshot_before", "Add a new snapshot before a snapshot (Snapshots/New_Snapshot/<index>)", [SNAP],
    osc_msg("/Snapshots/New_Snapshot/{snapshot}"))
cmd("insert_new_snapshot", "Insert a new snapshot after the current one (Snapshots/Insert_New_Snapshot)", [],
    osc_msg("/Snapshots/Insert_New_Snapshot"))
cmd("update_current_snapshot", "Update the current snapshot with the console's state "
    "(Snapshots/Update_Current_Snapshot)", [], osc_msg("/Snapshots/Update_Current_Snapshot"))
cmd("update_snapshot", "Overwrite a snapshot with the console's state (Snapshots/Update_Snapshot/<index>)", [SNAP],
    osc_msg("/Snapshots/Update_Snapshot/{snapshot}"))
cmd("delete_snapshot", "Delete a snapshot (Snapshots/Delete_Snapshot/<index>)", [SNAP],
    osc_msg("/Snapshots/Delete_Snapshot/{snapshot}"))
cmd("rename_snapshot", "Rename a snapshot (Snapshots/Rename_Snapshot/<index>, s)", [SNAP, NAME],
    osc_msg("/Snapshots/Rename_Snapshot/{snapshot}", ("{name}", "string")))
cmd("renumber_snapshot", "Change a snapshot's number (Snapshots/Renumber_Snapshot/<index>, i in hundredths)",
    [SNAP, "number: { type: float, min: 0, max: 327.67, required: true, label: Number, description: "
     "\"The snapshot's new number as the console shows it, 0 to 327.67 in steps of 0.01; sent in hundredths.\" }"],
    osc_msg("/Snapshots/Renumber_Snapshot/{snapshot}", ("{number:to.snapshot_number:.0f}", "int")))
cmd("move_snapshot", "Move a snapshot to another place in the list (Snapshots/Move_Snapshot/<index>, i)",
    [SNAP, "to: { type: int, min: 0, max: 9999, required: true, label: To, "
     "description: \"The index of the snapshot whose place it takes, as snapshots.list keys it.\" }"],
    osc_msg("/Snapshots/Move_Snapshot/{snapshot}", ("{to}", "int")))
cmd("undo_snapshot", "Undo the last snapshot recall (Snapshots/Undo_Snapshot)", [],
    osc_msg("/Snapshots/Undo_Snapshot"))
cmd("set_snapshot_sends_midi", "Set whether firing a snapshot sends MIDI (Snapshots/Fire_Snapshot_sends_MIDI, i)",
    [ENABLED.format(label="Snapshots send MIDI", desc=q("True makes a fired snapshot send its MIDI."))],
    osc_msg("/Snapshots/Fire_Snapshot_sends_MIDI", ("{enabled:bool01}", "int")))
cmd("set_midi_fires_snapshots", "Set whether MIDI received fires snapshots (Snapshots/MIDI_fires_Snapshots, i)",
    [ENABLED.format(label="MIDI fires snapshots", desc=q("True lets received MIDI recall snapshots."))],
    osc_msg("/Snapshots/MIDI_fires_Snapshots", ("{enabled:bool01}", "int")))
cmd("recall_macro", "Run a macro (Macros/Recall_Macro/<index>)", [MACRO], osc_msg("/Macros/Recall_Macro/{macro}"))
BUTTON = ("button: { type: int, min: 0, max: 255, required: true, label: Button, "
          "description: \"The OSC macro button, as macros.buttons keys it (0 to 255).\" }")
cmd("press_macro_button", "Press an OSC macro button (Macros/Buttons/press, i)", [BUTTON],
    osc_msg("/Macros/Buttons/press", ("{button}", "int")))
cmd("set_macro_button_state", "Set an OSC macro button's state (Macros/Buttons/state/<button>, i)",
    [BUTTON, "state: { type: int, min: 0, max: 2, required: true, label: State, "
     "description: \"The button's state, 0 to 2 as the console numbers it; the source names no value.\" }"],
    osc_msg("/Macros/Buttons/state/{button}", ("{state}", "int")))
SCOPE = ("scope: { type: enum, values: [all, eq, dynamics], default: all, label: Recall scope, "
         "description: \"What the preset recalls: all of it, its EQ only or its dynamics only.\" }")
MAPS["recall_scope"] = {0: "all", 1: "eq", 2: "dynamics"}
cmd("recall_preset", "Recall a preset onto one strip (Presets/Recall_Scope, i, then Presets/Recall_Preset/<index>, s)",
    [PRESET, SECTION, TARGET, SCOPE],
    [osc_msg("/Presets/Recall_Scope", ("{scope:map.recall_scope}", "int")),
     osc_msg("/Presets/Recall_Preset/{preset}", ("/{section}/{strip}", "string"))])
cmd("update_preset", "Update a preset from one strip (Presets/Update_Preset/<index>, s)",
    [PRESET, SECTION, TARGET], osc_msg("/Presets/Update_Preset/{preset}", ("/{section}/{strip}", "string")))
cmd("new_preset", "Create a preset from one strip in a preset group (Presets/New_Preset, s s)",
    ["group: { type: string, max_length: 64, required: true, label: Group, description: \"The preset group to "
     "create it in, as presets.list.*.group gives it; empty for none.\" }", SECTION, TARGET],
    osc_msg("/Presets/New_Preset", ("{group}", "string"), ("/{section}/{strip}", "string")))
cmd("delete_preset", "Delete a preset (Presets/Delete_Preset/<index>)", [PRESET],
    osc_msg("/Presets/Delete_Preset/{preset}"))
cmd("lock_preset", "Lock or unlock a preset (Presets/Lock_Preset/<index>, i)",
    [PRESET, "locked: { type: bool, default: true, label: Locked, description: \"True locks the preset.\" }"],
    osc_msg("/Presets/Lock_Preset/{preset}", ("{locked:bool01}", "int")))
cmd("rename_preset", "Rename a preset (Presets/Rename_Preset/<index>, s)", [PRESET, NAME],
    osc_msg("/Presets/Rename_Preset/{preset}", ("{name}", "string")))
cmd("rename_preset_group", "Rename the preset group a preset is in (Presets/Rename_Preset_Group/<index>, s)",
    [PRESET, NAME], osc_msg("/Presets/Rename_Preset_Group/{preset}", ("{name}", "string")))
cmd("set_fader_bank", "Select the surface's fader bank layout (Layout/Layout/Banks, s)",
    ["banks: { type: string, max_length: 256, required: true, label: Banks, description: \"The bank selection as "
     "layout.banks reports it.\" }"], osc_msg("/Layout/Layout/Banks", ("{banks}", "string")))
PATH = ("path: { type: string, max_length: 128, pattern: '^[A-Za-z][A-Za-z0-9_/.-]*$', required: true, "
        "label: Address, description: \"The parameter's address without the leading slash, as the command set "
        "writes it (Input_Channels/1/fader).\" }")
cmd("set_parameter_float", "Send any parameter of the command set a float (any address, f)",
    [PATH, "value: { type: float, required: true, label: Value, description: \"The value in the parameter's own "
     "unit.\" }"], osc_msg("/{path}", ("{value}", "float")))
cmd("set_parameter_int", "Send any parameter of the command set an int (any address, i)",
    [PATH, "value: { type: int, required: true, label: Value, description: \"The value as the console numbers "
     "it.\" }"], osc_msg("/{path}", ("{value}", "int")))
cmd("set_parameter_string", "Send any parameter of the command set a string (any address, s)",
    [PATH, "value: { type: string, max_length: 256, required: true, label: Value, description: \"The text.\" }"],
    osc_msg("/{path}", ("{value}", "string")))
cmd("query_parameter", "Read any parameter of the command set (its address with /? appended)", [PATH],
    osc_msg("/{path}/?"), returns="value", expect='{ address: "/{path}", arg: 0 }')

# ── Lists, the console and change notices ─────────────────────────────────
O.state.update({
    "console.name": ("string", None, "The console's name (/Console/Name)."),
    "console.session_file": ("string", None, "The file name of the session loaded (/Console/Session/Filename)."),
    "snapshots.count": ("int", None, "How many snapshots the session has (/Snapshots/count)."),
    "snapshots.current": ("int", None, "Index of the current snapshot, as snapshots.list keys it; -1 for none "
                          "(/Snapshots/Current_Snapshot)."),
    "snapshots.list.*.number": ("float", None, "The snapshot's number as the console shows it (sent in hundredths)."),
    "snapshots.list.*.group": ("string", None, "The snapshot's group, empty for none."),
    "snapshots.list.*.name": ("string", None, "The snapshot's name."),
    "snapshots.list.*.notes": ("string", None, "The snapshot's notes (/Snapshots/notes)."),
    "macros.count": ("int", None, "How many macros the session has (/Macros/count)."),
    "macros.list.*.name": ("string", None, "The macro's name, keyed by its index."),
    "macros.buttons.count": ("int", None, "How many OSC macro buttons there are (/Macros/Buttons/count)."),
    "macros.buttons.*.state": ("int", None, "The OSC macro button's state, 0 to 2 as the console numbers it."),
    "macros.buttons.*.name": ("string", None, "The OSC macro button's name."),
    "presets.count": ("int", None, "How many presets the session has (/Presets/count)."),
    "presets.list.*.channel_count": ("int", None, "How many channels the preset holds, as the source reads its "
                                     "second field."),
    "presets.list.*.section": ("string", None, "The console section the preset applies to."),
    "presets.list.*.group": ("string", None, "The preset's group, empty for none."),
    "presets.list.*.name": ("string", None, "The preset's name."),
    "presets.list.*.locked": ("bool", None, "True when the preset is locked (/Presets/Lock_Preset/<index>)."),
    "layout.banks": ("string", None, "The surface's fader bank selection, as the console sends it "
                     "(/Layout/Layout/Banks)."),
})

ENTITY = [("Snapshots", "snapshots", "^/Snapshots/name$"), ("Macros", "macros", "^/Macros/name$"),
          ("Presets", "presets", "^/Presets/name$")]
LIST_READS = [query("/Snapshots/count"), query("/Snapshots/Current_Snapshot"), query("/Macros/count"),
              query("/Macros/Buttons/count"), query("/Presets/count")]
CONSOLE_READS = [query("/Console/Name"), query("/Console/Session/Filename"),
                 query("/Console/Channels", "^/Console/"),
                 *[query(f"/Console/{wire}/modes") for _, wire, _ in MODES],
                 query("/Layout/Layout/Banks")]
STRIP_STARTS = [query(f"{SECTIONS[sec]['prefix']}/1/{SECTIONS[sec]['name_leaf']}") for sec in STRIPS]

LIST_RULES = f"""    - address: '^/Console/Name$'
      state: {{ console.name: "{{arg0}}" }}
    - address: '^/Console/Session/Filename$'
      state: {{ console.session_file: "{{arg0}}" }}
    - address: '^/Layout/Layout/Banks$'
      state: {{ layout.banks: "{{arg0}}" }}
    - address: '^/Snapshots/Current_Snapshot$'
      state: {{ snapshots.current: "{{arg0}}" }}
    - address: '^/Snapshots/name$'
      state: {{ "snapshots.list.{{arg0}}.number": "{{arg1:from.snapshot_number}}", "snapshots.list.{{arg0}}.group": "{{arg2}}", "snapshots.list.{{arg0}}.name": "{{arg3}}" }}
      then_send:
        - {{ address: /Snapshots/notes/?, args: [ {{ value: "{{arg0}}", type: int }} ], reply_address: "^/Snapshots/notes$" }}
    - address: '^/Snapshots/notes$'
      state: {{ "snapshots.list.{{arg0}}.notes": "{{arg4}}" }}
    - address: '^/Macros/name$'
      state: {{ "macros.list.{{arg0}}.name": "{{arg1}}" }}
    - address: '^/Macros/Buttons/count$'
      state: {{ macros.buttons.count: "{{arg0}}" }}
    - address: '^/Macros/Buttons/count$'
      request_match: '^/Macros/Buttons/count/\\?$'
      replace: macros.buttons
      then_send:
        - {{ address: /Macros/Buttons/?, reply_address: "^/Macros/Buttons/" }}
    - address: '^/Macros/Buttons/state$'
      state: {{ "macros.buttons.{{arg0}}.state": "{{arg1}}", "macros.buttons.{{arg0}}.name": "{{arg2}}" }}
    - address: '^/Macros/Buttons/state/(\\d+)$'
      state: {{ "macros.buttons.{{1}}.state": "{{arg0}}" }}
    - address: '^/Presets/name$'
      state: {{ "presets.list.{{arg0}}.channel_count": "{{arg1}}", "presets.list.{{arg0}}.section": "{{arg3}}", "presets.list.{{arg0}}.group": "{{arg4}}", "presets.list.{{arg0}}.name": "{{arg5}}" }}
      then_send:
        - {{ address: "/Presets/Lock_Preset/{{arg0}}/?", reply_address: "^/Presets/Lock_Preset/{{arg0}}$" }}
    - address: '^/Presets/Lock_Preset/(\\d+)$'
      state: {{ "presets.list.{{1}}.locked": {{ value: "{{arg0}}", map: {BOOL_MAP} }} }}"""
for root, key, name_reply in ENTITY:
    LIST_RULES += f"""
    - address: '^/{root}/count$'
      state: {{ {key}.count: "{{arg0}}" }}
    - address: '^/{root}/count$'
      request_match: '^/{root}/count/\\?$'
      replace: {key}.list
      then_send:
        - {{ address: /{root}/names/?, reply_address: "{name_reply}" }}
    - address: '^/{root}/!$'
      then_send:
        - {query(f'/{root}/count')}"""

# A change notice: read what is under the path again. A session loaded reads
# everything again.
LIST_RULES += """
    - address: '^(/.+)/!$'
      then_send:
        - { address: "{1}/?", reply_address: "^{1}(/|$)" }
    - address: '^/Console/Session/!$'
      then_send:
""" + "\n".join(f"        - {i}" for i in CONSOLE_READS + LIST_READS) + "\n" + \
    "\n".join(f"        - {{ when_set: read_on_connect, {i[2:]}" for i in STRIP_STARTS)

# Walking the strips: the reply to a strip's name query reads that strip's
# parameters and asks for the next strip's name; the walk ends at the first
# strip the console does not answer for. request_match keeps a name the
# console pushes (a strip renamed) from walking again.
CHAIN_RULES = []
DEST_STARTS = {
    "aux": [("input", "Aux_Send/{n}/send_level"), ("group", "Aux_Send/{n}/send_level")],
    "group": [("input", "Group_Send/{n}/group"), ("group", "Group_Send/{n}/group")],
    "matrix": [("matrix_input", "Matrix_Send/{n}/send_level")],
}
for sec in STRIPS:
    s = SECTIONS[sec]
    leaf = s["name_leaf"]
    items = [query(f"{s['prefix']}/{{2:+1}}/{leaf}")] + strip_reads(sec, "{2}")
    for src, dleaf in DEST_STARTS.get(sec, []):
        items.append(query(f"{SECTIONS[src]['prefix']}/1/{dleaf.replace('{n}', '{2}')}"))
    CHAIN_RULES.append(
        f"    - address: '^{s['prefix']}/(\\d+)/{leaf}$'\n"
        f"      request_match: '^{s['prefix']}/(\\d+)/{leaf}/\\?$'\n"
        f"      then_send:\n" + "\n".join(f"        - {i}" for i in items))
# The sends: walking the source strips for each destination.
for sec, dest, rows in SENDS:
    s = SECTIONS[sec]
    first, rest = rows[0], rows[1:]
    lf = first.leaf.replace("{d}", "(\\d+)")
    items = [query(f"{s['prefix']}/{{3:+1}}/{first.leaf.replace('{d}', '{4}')}")]
    items += [query(f"{s['prefix']}/{{3}}/{r.leaf.replace('{d}', '{4}')}") for r in rest]
    CHAIN_RULES.append(
        f"    - address: '^{s['prefix']}/(\\d+)/{lf}$'\n"
        f"      request_match: '^{s['prefix']}/(\\d+)/{lf}/\\?$'\n"
        f"      then_send:\n" + "\n".join(f"        - {i}" for i in items))


# ── The document ──────────────────────────────────────────────────────────
HEADER = """spec: 1
id: digico-pad
name: DiGiCo SD and Quantum consoles (DiGiCo Pad remote)
vendor: DiGiCo
category: mixer

# The command set DiGiCo's iPad app uses: in Setup > External Control the
# console adds a "DiGiCo Pad" device with this host's IP address and a Send and
# a Receive port, loads the iPad command file for its model, and then takes
# OSC on its Receive port and sends every change, and the answer to every
# query, to this host's address on its Send port. Unlike Other OSC (the
# digico-sd spec), it reads back: state is read on opening and kept current
# from what the console sends. DiGiCo does not publish the command set; it is
# taken from DiGiCo's own MIT-licensed Bitfocus Companion module, and every
# behaviour is inferred (quirks, VERIFICATION.md). This file is written by
# tools/generate_digico_pad.py.

source:
  - title: "TN324: Use of SD Series External Control for iPad (Oct 2014, rev 5 May 2018)"
    author: DiGiCo (UK) Ltd
    url: https://digico.biz/wp-content/uploads/2020/02/TN324-SD-Series-iPad-External-Control.pdf
    note: >
      Manufacturer technical note, the only DiGiCo document on the DiGiCo Pad device. Page 2: Master
      Screen > Setup > External Control, Enable External Control, Add device > DiGiCo Pad, the iPad's IP
      address, the console's Send and Receive port numbers (example Send = 9000, Receive = 8000), unique
      ports per device, one DiGiCo Pad device enabled at a time with V685 software and app 1.2.0, the
      Enable column, and Load: the iPad command set for the console (one for SD8, SD9 and SD11, another
      for SD5, SD7 and SD10), loaded once, Clear All before loading another. The console's Local IP
      address at the bottom of the panel (pages 1-2). It does not list the commands.
  - title: companion-module-digico-osc (Bitfocus Companion module, MIT licence)
    author: DiGiCo and Bitfocus AS contributors
    url: https://github.com/bitfocus/companion-module-digico-osc/tree/0d5290d3ef85bad58057aa6917f84e5435ab8ed9
    note: >
      DiGiCo's Companion module for the DiGiCo Pad command set, read at commit 0d5290d3 (October 2026)
      as the only source of the command set, filling what DiGiCo's documents leave out. digico_osc.csv:
      every strip parameter's address (Input_Channels, Aux_Outputs, Group_Outputs, Matrix_Outputs,
      Matrix_Inputs, Control_Groups, Graphic_EQ, Multis, Console), its type (Int, Float, String; BInt
      and BFloat for on/off), range, unit and scale (times in ms with scale 1000: sent in seconds), and
      which rows it marks "MAYBE?". digico_entities.csv and src/entity-schemas.json: the Snapshots,
      Macros and Presets addresses, the record fields of /Snapshots/name (index, number in hundredths,
      group, name), /Snapshots/notes, /Macros/name, /Macros/Buttons/state and /Presets/name.
      src/mixers.ts and src/main.ts: a query is the address with "/?" appended and no argument (or an
      int index), answered on the address; /<root>/names/? answers with one name record per item;
      /Console/Channels/? answers with the /Console/ counts; "<path>/!" means something under <path>
      changed and is read again; the module walks strip names from 1 until one is not answered.
      src/actions.ts: preset recall sends Presets/Recall_Scope then Recall_Preset with "/<section>/<n>";
      New_Preset takes the group and the target. src/value-mappings.json: on/off, auto solo (0 off, 1
      Solo 1, 2 Solo 2, 3 both), stereo mode (1 mono, 2 stereo), knee (0 hard, 1 medium, 2 soft),
      dynamics mode (0 compressor, 1 multiband, 2 de-esser), recall scope (0 all, 1 EQ, 2 dynamics),
      EQ curves (1 bell, 4 high shelf on band 1, 3 low shelf on band 4). companion/HELP.md: the device
      type DiGiCo Pad, the Commands Allowed file (iPadv2sd8-9-11-12, ipad_q3 for Quantum), Bundles off
      and Suppress OSC Retransmit on. src/osc.ts: some meter messages arrive without an OSC type-tag
      string.
"""


def emit():
    out = [HEADER]
    out.append("""# The console's Receive port (commands and queries go there) and the Send port
# it sends to this host: TN324's example ports. Both are the operator's choice
# in the External Control panel.
transport:
  type: osc-udp
  port: 8000
  timeout_ms: 500
  reply: to-source
  listen_port: { setting: feedback_port }
  probe: /Console/Name/?

settings:
  feedback_port: { type: int, min: 1, max: 65535, default: 9000,
    label: Feedback port,
    description: "Local UDP port this host listens on for the console; enter it as the DiGiCo Pad device's Send port in the console's External Control panel, with this host's IP address." }
  read_on_connect: { type: bool, default: true,
    label: Read every strip,
    description: "Read every parameter of every strip on opening and when the console loads a session, one query at a time; turn off to read only the console, snapshot, macro and preset lists and keep the rest from what the console sends." }

ports:
  - { port: 8000, protocol: udp, role: control }
  - { port: 9000, protocol: udp, role: feedback, listener: core, setting: feedback_port, note: "the DiGiCo Pad device's Send port: where the console sends changes and answers" }
""")
    names = [n for n, _ in O.commands]
    out.append("models:")
    for i, (mid, mname) in enumerate(MODELS):
        out.append(f"  - id: {mid}")
        out.append(f"    name: {mname}")
        if i == 0:
            out.append("    supports: &all")
            line = "      ["
            for j, n in enumerate(names):
                piece = n + (", " if j < len(names) - 1 else "]")
                if len(line) + len(piece) > 110:
                    out.append(line.rstrip())
                    line = "      "
                line += piece
            out.append(line)
        else:
            out.append("    supports: *all")
        out.append("    verification: none")
    out.append("")
    out.append("commands:")
    for n, lines in O.commands:
        out.append(f"  {n}:")
        out += lines
        out.append("")
    out.append(QUIRKS)
    out.append("conversions:")
    out.append("  snapshot_number:   # a snapshot's number, sent in hundredths (0-32767 = 0.00 to 327.67)")
    out.append("    points: [[0, 0.0], [32767, 327.67]]")
    out.append("")
    out.append("maps:")
    for name, values in MAPS.items():
        out.append(f"  {name}: {{ {', '.join(f'{v}: {q(str(k))}' for k, v in values.items())} }}")
    out.append("")
    out.append("state:")
    for path, (t, unit, desc) in O.state.items():
        u = f", unit: {unit}" if unit else ""
        out.append(f"  {path}: {{ type: {t}{u}, description: {q(desc)} }}")
    out.append("")
    out.append("telemetry:")
    out.append("  poll:")
    out.append("    send:")
    for i in CONSOLE_READS + LIST_READS:
        out.append(f"      - {i}")
    for i in STRIP_STARTS:
        out.append(f"      - {{ when_set: read_on_connect, {i[2:]}")
    out.append("  updates:")
    out.append(LIST_RULES)
    out += O.rules
    out += CHAIN_RULES
    return "\n".join(out) + "\n"


QUIRKS = """quirks:
  - models: [all]
    severity: info
    text: >
      DiGiCo does not document the DiGiCo Pad command set. Every address, type, range, unit and
      behaviour here is taken from DiGiCo's MIT-licensed Bitfocus Companion module (its parameter table
      and code); TN324 documents only how the device is set up. Each is listed in VERIFICATION.md to be
      checked on a console. Where DiGiCo's own documents say otherwise, they win.

  - models: [all]
    severity: critical
    text: >
      Nothing is accepted until the console is set up for it (TN324 p.2): Setup > External Control
      enabled, Add device > DiGiCo Pad with this host's IP address, a Send port (the feedback_port
      setting, 9000 in TN324's example) and a Receive port (the device's port, 8000), the device's Enable
      box ticked, and the iPad command file for the console loaded with Load (the module's help names
      iPadv2sd8-9-11-12 for SD8, SD9, SD11 and SD12, and ipad_q3 for Quantum); if the panel says no
      commands are enabled, Clear All and load it again. The module's help also asks for Bundles off and
      Suppress OSC Retransmit on. With software V685 and app 1.2.0 only one DiGiCo Pad device can be
      enabled at a time, so this host may take the place of an iPad.

  - models: [all]
    severity: critical
    text: >
      recall_snapshot, fire_next_snapshot, fire_previous_snapshot, fire_selected_snapshot,
      undo_snapshot, recall_preset and recall_macro change the live console at once (a snapshot's
      recall scope, faders and mutes included), and only another recall changes it back.
      update_snapshot, update_current_snapshot, update_preset, delete_snapshot and delete_preset
      overwrite or remove what the session holds.

  - models: [all]
    severity: warning
    text: >
      Reading: the console answers a query (the address with /? appended) on the address itself, and
      sends the same message when the value changes there, so state is read on opening and kept
      current from what the console sends. With read_on_connect on, the strips are read by walking
      their names from 1: each name answered reads that strip's every parameter and asks for the next
      name, and the walk ends at the first strip not answered (so at the session's strip count, plus
      one query that times out after 500 ms). Sends are walked the same way, per destination. A large
      session takes thousands of queries, sent one at a time behind any command; a session loaded on
      the console (/Console/Session/!) reads everything again. To read everything again otherwise,
      open the device again.

  - models: [all]
    severity: warning
    text: >
      Units: levels and gains are in dB, frequencies in Hz, times in seconds (the module's table gives
      milliseconds with a scale of 1000, so the console takes and sends seconds), fine delay in samples,
      pans from -1 to +1. A fader at -150 dB is the bottom of its travel. De-esser and gate key centre
      and width, the EQ curve, dynamic EQ over/under, the solo bus, a group's send tap, the input type,
      the gate mode and macro button states are kept as the console's numbers: the source does not
      say what they mean.

  - models: [all]
    severity: warning
    text: >
      The module's table marks some parameters "MAYBE?": their address or meaning is its author's
      guess. They are built as the table reads them and listed in VERIFICATION.md: input channel
      input_type and Dynamics gate-duck-comp, the outputs' alternate_solo, and the group sends of
      input channels and group outputs (Group_Send/<n>/group). The table's other MAYBE rows (the
      /Console/.../group_sends and send_levels lists, /Console/Session/!) are not built.

  - models: [all]
    severity: warning
    text: >
      Snapshots, macros and presets are keyed by their index in the console's list, which is what
      recall_snapshot, rename_snapshot, recall_macro and the preset commands take; a snapshot's number
      (snapshots.list.*.number) is what the console shows, sent in hundredths. A preset is recalled
      onto, updated from or created from a strip named by the preset's section, as presets.list.*.section
      gives it, and the strip's number. Inserting, deleting or moving an item renumbers the list; the
      console then sends /Snapshots/! (or /Macros/!, /Presets/!) and the list is read again whole.

  - models: [all]
    severity: info
    text: >
      Meters are not read. The command set has meter requests (Meters/request/<n> with a string, and
      Meters/values) whose argument the source does not give, and the module's code notes that some
      meter messages arrive with no OSC type-tag string, which is not valid OSC 1.0.
"""


def vectors():
    v = ['S = "digico-pad"',
         "# Generated by tools/generate_digico_pad.py from the command set as DiGiCo's Companion module",
         "# gives it: floats for levels and on/off, ints for snapshot and preset settings, strings for",
         "# names; /? appended for a query."]
    v.append('UNV = {"ok": {"kind": "unverified"}}')
    samples = {"input": 12, "aux": 3, "group": 2, "matrix": 64, "matrix_input": 5, "control_group": 36,
               "geq": 48, "multi": 7}
    k = 0
    for sec, rows in STRIPS.items():
        s = SECTIONS[sec]
        n = samples[sec]
        for row in rows:
            k += 1
            inp = {s["param"]: n}
            band = None
            if row.bands:
                band = 1 + k % row.bands
                inp["band"] = band
            addr = f"{s['prefix']}/{n}/" + leaf_address(row.leaf, str(band))
            vname, arg, state_val = vector_value(row, k)
            inp[vname] = vname_value(row, k)
            name = command_name(sec, row)
            v.append(f"binary(S, {q(name)}, {inp!r}, osc({q(addr)}, {arg}), expect_result=UNV)")
            # The same address arriving reads into state.
            path = f"{s['root']}.{n}.{row.key.replace('*', str(band))}"
            v.append(f"telemetry(S, {q(name.replace('_', '-') + '-state')}, inbound_hex=hexs(osc({q(addr)}, {arg})), "
                     f"expect_state={nest(path, state_val)!r})")
    for sec, dest, rows in SENDS:
        s, d = SECTIONS[sec], SECTIONS[dest]
        for row in rows:
            k += 1
            inp = {s["param"]: 4, dest_param(sec, dest): 2}
            addr = f"{s['prefix']}/4/" + leaf_address(row.leaf, "", "2")
            vname, arg, state_val = vector_value(row, k)
            inp[vname] = vname_value(row, k)
            name = command_name(sec, row)
            v.append(f"binary(S, {q(name)}, {inp!r}, osc({q(addr)}, {arg}), expect_result=UNV)")
            path = f"{s['root']}.4.{row.key.replace('*', '2')}"
            v.append(f"telemetry(S, {q(name.replace('_', '-') + '-state')}, inbound_hex=hexs(osc({q(addr)}, {arg})), "
                     f"expect_state={nest(path, state_val)!r})")
    v.append(VECTOR_TAIL)
    return "\n".join(v) + "\n"


def command_name(sec, row):
    return f"{row.cmd}_{sec}" if row.cmd in ("mute", "solo") else f"set_{sec}_{row.cmd}"


def vname_value(row, k):
    kind = row.kind
    if isinstance(kind, B):
        return k % 2 == 0
    if isinstance(kind, S):
        return "Lead Vox"
    if isinstance(kind, E):
        return list(kind.values.values())[k % len(kind.values)]
    return [kind.lo, kind.hi][k % 2]


def vector_value(row, k):
    kind = row.kind
    val = vname_value(row, k)
    if isinstance(kind, B):
        name = {"mute": "muted", "solo": "soloed"}.get(row.cmd, "enabled")
        return name, f'("f", {1.0 if val else 0.0})', val
    if isinstance(kind, S):
        return "name", f'("s", {q(val)})', val
    if isinstance(kind, E):
        wire = [w for w, nme in kind.values.items() if nme == val][0]
        return "value", f'("f", {float(wire)})', val
    return "value", f'("f", {float(val)})', f32(val)


def f32(x):
    """The value as an OSC float carries it."""
    return struct.unpack(">f", struct.pack(">f", x))[0]


def nest(path, value):
    out = value
    for part in reversed(path.split(".")):
        out = {part: out}
    return out


VECTOR_TAIL = '''
# Console, lists and generic commands.
binary(S, "recall_snapshot", {"snapshot": 7}, osc("/Snapshots/Recall_Snapshot/7"), expect_result=UNV)
binary(S, "fire_next_snapshot", {}, osc("/Snapshots/Fire_Next_Snapshot"), expect_result=UNV)
binary(S, "fire_previous_snapshot", {}, osc("/Snapshots/Fire_Prev_Snapshot"), expect_result=UNV)
binary(S, "fire_selected_snapshot", {}, osc("/Snapshots/Fire_Selected_Snapshot"), expect_result=UNV)
binary(S, "select_surface_snapshot", {"snapshot": 3}, osc("/Snapshots/Change_Surface_Snapshot/3"), expect_result=UNV)
binary(S, "new_snapshot_before", {"snapshot": 0}, osc("/Snapshots/New_Snapshot/0"), expect_result=UNV)
binary(S, "insert_new_snapshot", {}, osc("/Snapshots/Insert_New_Snapshot"), expect_result=UNV)
binary(S, "update_current_snapshot", {}, osc("/Snapshots/Update_Current_Snapshot"), expect_result=UNV)
binary(S, "update_snapshot", {"snapshot": 2}, osc("/Snapshots/Update_Snapshot/2"), expect_result=UNV)
binary(S, "delete_snapshot", {"snapshot": 9}, osc("/Snapshots/Delete_Snapshot/9"), expect_result=UNV)
binary(S, "rename_snapshot", {"snapshot": 4, "name": "Act 2"}, osc("/Snapshots/Rename_Snapshot/4", ("s", "Act 2")),
       expect_result=UNV)
# Number 12.5 is sent in hundredths.
binary(S, "renumber_snapshot", {"snapshot": 4, "number": 12.5}, osc("/Snapshots/Renumber_Snapshot/4", ("i", 1250)),
       expect_result=UNV)
binary(S, "move_snapshot", {"snapshot": 4, "to": 1}, osc("/Snapshots/Move_Snapshot/4", ("i", 1)), expect_result=UNV)
binary(S, "undo_snapshot", {}, osc("/Snapshots/Undo_Snapshot"), expect_result=UNV)
binary(S, "set_snapshot_sends_midi", {"enabled": False}, osc("/Snapshots/Fire_Snapshot_sends_MIDI", ("i", 0)),
       expect_result=UNV)
binary(S, "set_midi_fires_snapshots", {"enabled": True}, osc("/Snapshots/MIDI_fires_Snapshots", ("i", 1)),
       expect_result=UNV)
binary(S, "recall_macro", {"macro": 5}, osc("/Macros/Recall_Macro/5"), expect_result=UNV)
binary(S, "press_macro_button", {"button": 0}, osc("/Macros/Buttons/press", ("i", 0)), expect_result=UNV)
binary(S, "set_macro_button_state", {"button": 3, "state": 2}, osc("/Macros/Buttons/state/3", ("i", 2)),
       expect_result=UNV)
binary(S, "recall_preset", {"preset": 2, "section": "Input Channels", "strip": 5, "scope": "eq"},
       [osc("/Presets/Recall_Scope", ("i", 1)), osc("/Presets/Recall_Preset/2", ("s", "/Input Channels/5"))],
       expect_result=UNV)
binary(S, "update_preset", {"preset": 2, "section": "Aux Outputs", "strip": 1},
       osc("/Presets/Update_Preset/2", ("s", "/Aux Outputs/1")), expect_result=UNV)
binary(S, "new_preset", {"group": "Vocals", "section": "Input Channels", "strip": 12},
       osc("/Presets/New_Preset", ("s", "Vocals"), ("s", "/Input Channels/12")), expect_result=UNV)
binary(S, "delete_preset", {"preset": 6}, osc("/Presets/Delete_Preset/6"), expect_result=UNV)
binary(S, "lock_preset", {"preset": 6, "locked": True}, osc("/Presets/Lock_Preset/6", ("i", 1)), expect_result=UNV)
binary(S, "rename_preset", {"preset": 6, "name": "Kick"}, osc("/Presets/Rename_Preset/6", ("s", "Kick")),
       expect_result=UNV)
binary(S, "rename_preset_group", {"preset": 6, "name": "Drums"}, osc("/Presets/Rename_Preset_Group/6", ("s", "Drums")),
       expect_result=UNV)
binary(S, "set_fader_bank", {"banks": "1"}, osc("/Layout/Layout/Banks", ("s", "1")), expect_result=UNV)
binary(S, "set_parameter_float", {"path": "Input_Channels/1/fader", "value": -10.0},
       osc("/Input_Channels/1/fader", ("f", -10.0)), expect_result=UNV)
binary(S, "set_parameter_int", {"path": "Presets/Recall_Scope", "value": 2}, osc("/Presets/Recall_Scope", ("i", 2)),
       expect_result=UNV)
binary(S, "set_parameter_string", {"path": "Multis/1/name", "value": "Band"}, osc("/Multis/1/name", ("s", "Band")),
       expect_result=UNV)
binary(S, "query_parameter", {"path": "Input_Channels/3/fader"}, osc("/Input_Channels/3/fader/?"),
       device_reply_hex=hexs(osc("/Input_Channels/3/fader", ("f", -5.0))),
       expect_result={"ok": {"kind": "value", "value": -5.0}})

# What opening sends first: the console and list queries, then the first name of each strip kind.
telemetry(S, "console-name", inbound_hex=hexs(osc("/Console/Name", ("s", "FOH SD12"))),
          expect_state={"console": {"name": "FOH SD12"}})
telemetry(S, "session-file", inbound_hex=hexs(osc("/Console/Session/Filename", ("s", "Tour.ses"))),
          expect_state={"console": {"session_file": "Tour.ses"}})
telemetry(S, "input-count", inbound_hex=hexs(osc("/Console/Input_Channels", ("i", 96))),
          expect_state={"console": {"counts": {"input_channels": 96}}})
telemetry(S, "aux-modes", inbound_hex=hexs(osc("/Console/Aux_Outputs/modes", ("i", 1), ("i", 2), ("i", 2))),
          expect_state={"aux_outputs": {"1": {"mode": 1}, "2": {"mode": 2}, "3": {"mode": 2}}})
telemetry(S, "fader-bank", inbound_hex=hexs(osc("/Layout/Layout/Banks", ("s", "2"))),
          expect_state={"layout": {"banks": "2"}})
telemetry(S, "current-snapshot", inbound_hex=hexs(osc("/Snapshots/Current_Snapshot", ("i", 4))),
          expect_state={"snapshots": {"current": 4}})
telemetry(S, "snapshot-record", inbound_hex=hexs(osc("/Snapshots/name", ("i", 4), ("i", 1250), ("s", "Band"), ("s", "Act 2"))),
          expect_state={"snapshots": {"list": {"4": {"number": 12.5, "group": "Band", "name": "Act 2"}}}},
          expect_then_send_hex=[hexs(osc("/Snapshots/notes/?", ("i", 4)))])
telemetry(S, "snapshot-notes", inbound_hex=hexs(osc("/Snapshots/notes", ("i", 4), ("i", 0), ("i", 0), ("i", 0), ("s", "Pyro cue"))),
          expect_state={"snapshots": {"list": {"4": {"notes": "Pyro cue"}}}})
# The count answering its query clears the list and reads the names.
telemetry(S, "snapshot-count", inbound_hex=hexs(osc("/Snapshots/count", ("i", 12))), request="/Snapshots/count/?",
          state_before={"snapshots": {"count": 13, "list": {"13": {"name": "Old"}}}},
          expect_state={"snapshots": {"count": 12, "list": None}},
          expect_then_send_hex=[hexs(osc("/Snapshots/names/?"))])
telemetry(S, "snapshots-changed", inbound_hex=hexs(osc("/Snapshots/!")),
          expect_then_send_hex=[hexs(osc("/Snapshots/count/?")), hexs(osc("/Snapshots/?"))])
telemetry(S, "macro-record", inbound_hex=hexs(osc("/Macros/name", ("i", 2), ("s", "Band on"))),
          expect_state={"macros": {"list": {"2": {"name": "Band on"}}}})
telemetry(S, "macro-button", inbound_hex=hexs(osc("/Macros/Buttons/state", ("i", 0), ("i", 1), ("s", "Talk"))),
          expect_state={"macros": {"buttons": {"0": {"state": 1, "name": "Talk"}}}})
telemetry(S, "macro-button-count", inbound_hex=hexs(osc("/Macros/Buttons/count", ("i", 8))), request="/Macros/Buttons/count/?",
          expect_state={"macros": {"buttons": {"count": 8}}},
          expect_then_send_hex=[hexs(osc("/Macros/Buttons/?"))])
telemetry(S, "preset-record", inbound_hex=hexs(osc("/Presets/name", ("i", 3), ("i", 1), ("i", 0), ("s", "Input Channels"), ("s", "Vox"), ("s", "Lead"))),
          expect_state={"presets": {"list": {"3": {"channel_count": 1, "section": "Input Channels", "group": "Vox", "name": "Lead"}}}},
          expect_then_send_hex=[hexs(osc("/Presets/Lock_Preset/3/?"))])
telemetry(S, "preset-lock", inbound_hex=hexs(osc("/Presets/Lock_Preset/3", ("i", 1))),
          expect_state={"presets": {"list": {"3": {"locked": True}}}})
# A strip's name answering its query reads the strip and asks for the next
# name; the same name pushed (a rename) only reads into state.
telemetry(S, "multi-name-walk", inbound_hex=hexs(osc("/Multis/1/name", ("s", "Band"))), request="/Multis/1/name/?",
          expect_state={"multis": {"1": {"name": "Band"}}},
          expect_then_send_hex=[hexs(osc("/Multis/2/name/?")), hexs(osc("/Multis/1/fader/?")),
                                hexs(osc("/Multis/1/mute/?")), hexs(osc("/Multis/1/solo/?"))])
telemetry(S, "multi-name-pushed", inbound_hex=hexs(osc("/Multis/1/name", ("s", "Band"))),
          expect_state={"multis": {"1": {"name": "Band"}}})
telemetry(S, "matrix-name-walk", inbound_hex=hexs(osc("/Matrix_Inputs/2/Channel_Input/name", ("s", "PA L"))),
          request="/Matrix_Inputs/2/Channel_Input/name/?",
          expect_state={"matrix_inputs": {"2": {"name": "PA L"}}},
          expect_then_send_hex=[hexs(osc("/Matrix_Inputs/3/Channel_Input/name/?"))])
telemetry(S, "aux-send-walk", inbound_hex=hexs(osc("/Input_Channels/4/Aux_Send/2/send_level", ("f", -10.0))),
          request="/Input_Channels/4/Aux_Send/2/send_level/?",
          expect_state={"inputs": {"4": {"aux_sends": {"2": {"level": -10.0}}}}},
          expect_then_send_hex=[hexs(osc("/Input_Channels/5/Aux_Send/2/send_level/?")),
                                hexs(osc("/Input_Channels/4/Aux_Send/2/send_on/?")),
                                hexs(osc("/Input_Channels/4/Aux_Send/2/send_pan/?"))])
telemetry(S, "layout-changed", inbound_hex=hexs(osc("/Layout/Layout/Banks/!")),
          expect_then_send_hex=[hexs(osc("/Layout/Layout/Banks/?"))])
'''


def main():
    (ROOT / "specs" / f"{SID}.yaml").write_bytes(emit().encode())
    (ROOT / "tools" / "vectors" / f"{SID}.py").write_bytes(vectors().encode())
    print(SID, len(O.commands), "commands,", len(O.state), "state paths,", len(O.rules), "rules")


main()
