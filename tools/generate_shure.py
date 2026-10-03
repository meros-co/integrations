"""Generate the Shure command-string specs (P300, IntelliMix Room, ANI, MXA,
MXN5) and their vector modules from one table, since they share the protocol.

    python tools/generate_shure.py
"""
import json, sys
from pathlib import Path

ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent

ONOFF_MAP = '{ "ON": true, "OFF": false, "On": true, "Off": false, "on": true, "off": false }'
COLORS = ["RED", "ORANGE", "GOLD", "YELLOW", "YELLOWGREEN", "GREEN", "TURQUOISE", "POWDERBLUE", "CYAN",
          "SKYBLUE", "BLUE", "PURPLE", "LIGHTPURPLE", "VIOLET", "ORCHID", "PINK", "WHITE"]

CONVERSIONS = {
    "shure_gain": ("[[0, -110.0], [1400, 30.0]]", "AUDIO_GAIN_HI_RES and similar: tenths of a dB offset by 1100 (0-1400 = -110.0 to +30.0 dB)"),
    "shure_off_att": ("[[0, -110.0], [107, -3.0]]", "automixer off attenuation: dB offset by 110 (0-107 = -110 to -3 dB)"),
    "shure_agc_cut": ("[[0, -20.0], [200, 0.0]]", "AGC maximum cut: tenths of a dB offset by 200 (0-200 = -20.0 to 0.0 dB)"),
    "shure_agc_boost": ("[[0, 0.0], [200, 20.0]]", "AGC maximum boost: tenths of a dB (0-200 = 0.0 to +20.0 dB)"),
    "shure_agc_target": ("[[0, -50.0], [500, 0.0]]", "AGC target: tenths of a dBFS offset by 500 (0-500 = -50.0 to 0.0 dBFS)"),
    "shure_comp_threshold": ("[[0, -60.0], [600, 0.0]]", "compressor threshold: tenths of a dB offset by 600 (0-600 = -60.0 to 0.0 dB)"),
    "shure_comp_ratio": ("[[10, 1.0], [1000, 100.0]]", "compressor ratio: tenths (0010-1000 = 1.0:1 to 100.0:1)"),
    "shure_xy": ("[[0, -1524.0], [3048, 1524.0]]", "lobe X/Y position: cm offset by 1524 (0-3048 = -1524 to +1524 cm)"),
}


def q(s):
    return json.dumps(s)


class Spec:
    def __init__(self, sid, name, header, sources, models, inc, chan_fmt, all_cmd, quirks):
        self.sid, self.name, self.header, self.sources = sid, name, header, sources
        self.models, self.inc, self.chan_fmt, self.all_cmd = models, inc, chan_fmt, all_cmd
        self.extra_quirks = quirks
        self.commands = {}      # name -> yaml text
        self.state = {}         # path -> (type, unit, description)
        self.rules = []         # yaml text
        self.vectors = []       # python lines
        self.conversions = set()

    # ── helpers ──
    def ch(self, var="channel"):
        return "{" + var + (":" + self.chan_fmt if self.chan_fmt else "") + "}"

    def chv(self, n):
        return format(n, self.chan_fmt) if self.chan_fmt else str(n)

    def cmd(self, name, summary, params, send, expect, returns):
        lines = [f"  {name}:", f"    summary: {q(summary)}"]
        if params:
            lines.append("    params:")
            for k, v in params.items():
                lines.append(f"      {k}: {v}")
        lines.append(f"    send: {q(send)}")
        if expect:
            lines.append(f"    expect: {expect}")
        lines.append(f"    returns: {returns}")
        self.commands[name] = "\n".join(lines)

    def vec(self, name, inp, wire, reply=None, result=None):
        extra = ""
        if reply is not None:
            extra += f", device_reply={q(reply)}"
        if result is not None:
            extra += f", expect_result={result}"
        self.vectors.append(f"text(S, {q(name)}, {inp!r}, {q(wire)}{extra})")


CH = "{ type: int, min: 0, max: 99, required: true }"
CH1 = "{ type: int, min: 1, max: 99, required: true }"


def get_expect(param):
    return "{ reply_contains: " + q(f" {param} ") + ", matches: " + q(f" {param} " + r"\{?(.*?) *\}? *>$") + " }"


def set_expect(param):
    return "{ reply_contains: " + q(f" {param} ") + ", contains: " + q(f" {param} ") + " }"


VALUE = lambda v: "{'ok': {'kind': 'value', 'value': " + repr(v) + "}}"
ACK = "{'ok': {'kind': 'ack'}}"
ERR = "{'error': {'error': 'device_error'}}"


def add_param(s, p):
    """p: dict(key, P, scope dev|ch, kind, ...)."""
    key, P, scope, kind = p["key"], p["P"], p["scope"], p["kind"]
    desc = p.get("desc", P)
    dev = scope == "dev"
    prefix = f"GET {P}" if dev else f"GET {s.ch()} {P}"
    params_get = {} if dev else {"channel": p.get("chan", CH)}
    path = f"device.{key}" if dev else f"channels.*.{key}"
    rule_re = (f"^< REP {P} " if dev else f"^< REP (\\d+) {P} ")
    target = f"device.{key}" if dev else "channels.{1}." + key
    cap = "{1}" if dev else "{2}"
    ex_ch = p.get("example_channel", 1)
    exin = {} if dev else {"channel": ex_ch}
    rep_head = f"< REP {P} " if dev else f"< REP {s.chv(ex_ch)} {P} "
    wire_get = (f"< GET {P} >" if dev else f"< GET {s.chv(ex_ch)} {P} >")

    # GET
    if kind != "wo":
        s.cmd(f"get_{key}", f"Read {desc} ({P})", params_get, prefix, get_expect(P), "value")
        ex = p.get("example", "ON")
        s.vec(f"get_{key}", exin, wire_get, rep_head + f"{ex} >", VALUE(p.get("example_value", ex)))

    unit = p.get("unit")
    if kind in ("onoff", "onoff_toggle", "ro_onoff"):
        s.state[path] = ("bool", None, f"{desc} (on/true)")
        s.rules.append(f"    - match: {q(rule_re + '(ON|OFF|On|Off) >$')}\n      state: {{ {q(target)}: {{ value: {q(cap)}, map: {ONOFF_MAP} }} }}")
    elif kind in ("enum", "ro_text", "text"):
        s.state[path] = ("string", None, desc)
        m = q(rule_re + r'\{?(.*?) *\}? >$')
        s.rules.append(f"    - match: {m}\n      state: {{ {q(target)}: {q(cap)} }}")
    elif kind in ("int", "ro_int"):
        conv = p.get("conv")
        if conv:
            s.conversions.add(conv)
            s.state[path] = ("float", unit, desc)
            m = q(rule_re + r'(\d+) >$')
            v = q(cap[:-1] + ':from.' + conv + '}')
            s.rules.append(f"    - match: {m}\n      state: {{ {q(target)}: {v} }}")
        else:
            s.state[path] = ("int", unit, desc)
            m = q(rule_re + r'(-?\d+) >$')
            s.rules.append(f"    - match: {m}\n      state: {{ {q(target)}: {q(cap)} }}")

    head = f"SET {P}" if dev else f"SET {s.ch()} {P}"
    wire_head = f"< SET {P}" if dev else f"< SET {s.chv(ex_ch)} {P}"
    params_set = dict(params_get)
    if kind in ("onoff", "onoff_toggle"):
        word = p.get("set_key", "enabled")
        s.cmd(f"set_{key}", f"Turn {desc} on or off ({P})", {**params_set, word: "{ type: bool, default: true }"},
              f"{head} {{{word}:on_off}}", set_expect(P), "ack")
        s.vec(f"set_{key}", {**exin, word: True}, wire_head + " ON >", rep_head + "ON >", ACK)
        if kind == "onoff_toggle":
            s.cmd(f"toggle_{key}", f"Toggle {desc} ({P} TOGGLE)", params_set, f"{head} TOGGLE", set_expect(P), "ack")
            s.vec(f"toggle_{key}", exin, wire_head + " TOGGLE >", rep_head + "OFF >", ACK)
    elif kind == "enum":
        vals = p["values"]
        s.cmd(f"set_{key}", f"Set {desc} ({P})", {**params_set, "value": "{ type: enum, values: [" + ", ".join(q(v) for v in vals) + "], required: true }"},
              f"{head} {{value}}", set_expect(P), "ack")
        s.vec(f"set_{key}", {**exin, "value": vals[-1]}, wire_head + f" {vals[-1]} >", rep_head + f"{vals[-1]} >", ACK)
    elif kind == "int":
        lo, hi = p["min"], p["max"]
        width = p.get("width")
        off = p.get("offset", 0)
        fmt = ""
        if off:
            fmt += f":{off:+d}"
        if width:
            fmt += f":0{width}d"
        name = p.get("set_param", "value")
        s.cmd(f"set_{key}", f"Set {desc} ({P}){', ' + unit if unit else ''}", {**params_set, name: f"{{ type: int, min: {lo}, max: {hi}, required: true }}"},
              f"{head} {{{name}{fmt}}}", set_expect(P), "ack")
        exv = p.get("set_example", hi)
        wv = exv + off
        wtxt = format(wv, f"0{width}d") if width else str(wv)
        s.vec(f"set_{key}", {**exin, name: exv}, wire_head + f" {wtxt} >", rep_head + f"{wtxt} >", ACK)
        if p.get("incdec"):
            for verb, word in (("increase", s.inc[0]), ("decrease", s.inc[1])):
                s.cmd(f"{verb}_{key}", f"{verb.capitalize()} {desc} by a step ({P} {word})",
                      {**params_set, "step": p["incdec"]}, f"{head} {word} {{step}}", set_expect(P), "ack")
                s.vec(f"{verb}_{key}", {**exin, "step": 10}, wire_head + f" {word} 10 >", rep_head + f"{wtxt} >", ACK)


def common_device(s, extra_get=()):
    for key, P, desc, ex in [("model", "MODEL", "the model", "{P300                            }"),
                             ("serial_number", "SERIAL_NUM", "the serial number", "{3TA1234567                      }"),
                             ("firmware", "FW_VER", "the firmware version", "{4.4.10            }"),
                             ("device_id", "DEVICE_ID", "the device ID (name)", "{Room 101                       }"),
                             ("ip_address", "IP_ADDR_NET_AUDIO_PRIMARY", "the primary audio network IP address", "{192.168.1.20   }"),
                             ("subnet", "IP_SUBNET_NET_AUDIO_PRIMARY", "the primary audio network subnet mask", "{255.255.255.0  }"),
                             ("gateway", "IP_GATEWAY_NET_AUDIO_PRIMARY", "the primary audio network gateway", "{192.168.1.1    }")] + list(extra_get):
        exv = ex.strip("{}").strip()
        add_param(s, dict(key=key, P=P, scope="dev", kind="ro_text", desc=desc, example=ex, example_value=exv))


def presets_etc(s, legacy_names, flash_get=True, defaults=True):
    s.cmd("get_preset", "The preset last recalled (PRESET), 1-10", {}, "GET PRESET", get_expect("PRESET"), "value")
    s.vec("get_preset", {}, "< GET PRESET >", "< REP PRESET 03 >", VALUE("03"))
    s.state["device.preset"] = ("int", None, "The preset last recalled, 1-10")
    s.rules.append("    - match: \"^< REP PRESET (\\\\d+) >$\"\n      state: { \"device.preset\": \"{1}\" }")
    s.cmd("recall_preset", "Recall a preset (SET PRESET), 1-10", {"preset": "{ type: int, min: 1, max: 10, required: true }"},
          "SET PRESET {preset:02d}", set_expect("PRESET"), "ack")
    s.vec("recall_preset", {"preset": 3}, "< SET PRESET 03 >", "< REP PRESET 03 >", ACK)
    s.vec("recall_preset", {"preset": 10}, "< SET PRESET 10 >", "< REP ERR >", ERR)
    if legacy_names:
        s.cmd("get_preset_name", "A preset's name (PRESET1-PRESET10)", {"preset": "{ type: int, min: 1, max: 10, required: true }"},
              "GET PRESET{preset}", "{ reply_contains: \" PRESET\", matches: \" PRESET\\\\d+ \\\\{?(.*?) *\\\\}? *>$\" }", "value")
        s.vec("get_preset_name", {"preset": 2}, "< GET PRESET2 >", "< REP PRESET2 {Lecture                  } >", VALUE("Lecture"))
    else:
        s.cmd("get_preset_name", "A preset's name (PRESET_NAME); {empty} for an empty preset", {"preset": "{ type: int, min: 1, max: 10, required: true }"},
              "GET PRESET_NAME {preset:02d}", "{ reply_contains: \" PRESET_NAME \", matches: \" PRESET_NAME \\\\d+ (.*?) *>$\" }", "value")
        s.vec("get_preset_name", {"preset": 2}, "< GET PRESET_NAME 02 >", "< REP PRESET_NAME 02 Lecture >", VALUE("Lecture"))
    s.cmd("flash", "Flash the device's lights to identify it (FLASH); it stops by itself after about 30 s", {"enabled": "{ type: bool, default: true }"},
          "SET FLASH {enabled:on_off}", set_expect("FLASH"), "ack")
    s.vec("flash", {"enabled": True}, "< SET FLASH ON >", "< REP FLASH ON >", ACK)
    s.cmd("reboot", "Reboot the device (SET REBOOT); not acknowledged", {}, "SET REBOOT", None, "none")
    s.vec("reboot", {}, "< SET REBOOT >", None, "{'ok': {'kind': 'unverified'}}")
    if defaults:
        s.cmd("restore_defaults", "Restore default settings (SET DEFAULT_SETTINGS)", {}, "SET DEFAULT_SETTINGS",
              "{ reply_contains: \"DEFAULT_SETTINGS\", contains: \"DEFAULT_SETTINGS\" }", "ack")
        s.vec("restore_defaults", {}, "< SET DEFAULT_SETTINGS >", "< REP DEFAULT_SETTINGS 00 >", ACK)
    s.cmd("get_all", "Ask for every parameter of a channel (0: the device and all channels); the answers update state", {"channel": CH},
          f"GET {s.ch()} ALL", None, "none")
    s.vec("get_all", {"channel": 0}, f"< GET {s.chv(0)} ALL >", None, "{'ok': {'kind': 'unverified'}}")
    s.cmd("get_parameter", "Read any documented channel parameter by name (GET <channel> <PARAM>), returning its value text", {"channel": CH, "parameter": "{ type: string, pattern: \"^[A-Z][A-Z0-9_]*$\", max_length: 40, required: true }"},
          f"GET {s.ch()} {{parameter}}", "{ reply_contains: \" {parameter} \", matches: \"^< REP (?:\\\\d+ )?[A-Z][A-Z0-9_]* \\\\{?(.*?) *\\\\}? *>$\" }", "value")
    s.vec("get_parameter", {"channel": 1, "parameter": "AUDIO_MUTE"}, f"< GET {s.chv(1)} AUDIO_MUTE >", f"< REP {s.chv(1)} AUDIO_MUTE OFF >", VALUE("OFF"))
    s.cmd("set_parameter", "Set any documented channel parameter by name (SET <channel> <PARAM> <value>), the value in the document's form", {"channel": CH, "parameter": "{ type: string, pattern: \"^[A-Z][A-Z0-9_]*$\", max_length: 40, required: true }", "value": "{ type: string, pattern: \"^[^<>]+$\", max_length: 64, required: true }"},
          f"SET {s.ch()} {{parameter}} {{value}}", "{ reply_contains: \" {parameter} \", not_contains: \"REP ERR\" }", "ack")
    s.vec("set_parameter", {"channel": 1, "parameter": "AUDIO_MUTE", "value": "ON"}, f"< SET {s.chv(1)} AUDIO_MUTE ON >", f"< REP {s.chv(1)} AUDIO_MUTE ON >", ACK)
    s.cmd("get_device_parameter", "Read any documented device parameter by name (GET <PARAM>)", {"parameter": "{ type: string, pattern: \"^[A-Z][A-Z0-9_]*$\", max_length: 40, required: true }"},
          "GET {parameter}", "{ reply_contains: \" {parameter} \", matches: \"^< REP (?:\\\\d+ )?[A-Z][A-Z0-9_]* \\\\{?(.*?) *\\\\}? *>$\" }", "value")
    s.vec("get_device_parameter", {"parameter": "ENCRYPTION"}, "< GET ENCRYPTION >", "< REP ENCRYPTION OFF >", VALUE("OFF"))
    s.cmd("set_device_parameter", "Set any documented device parameter by name (SET <PARAM> <value>)", {"parameter": "{ type: string, pattern: \"^[A-Z][A-Z0-9_]*$\", max_length: 40, required: true }", "value": "{ type: string, pattern: \"^[^<>]+$\", max_length: 128, required: true }"},
          "SET {parameter} {value}", "{ reply_contains: \" {parameter} \", not_contains: \"REP ERR\" }", "ack")
    s.vec("set_device_parameter", {"parameter": "LED_BRIGHTNESS", "value": "2"}, "< SET LED_BRIGHTNESS 2 >", "< REP LED_BRIGHTNESS 2 >", ACK)


def gain(s, P="AUDIO_GAIN_HI_RES", key="gain", desc="a channel's digital gain"):
    add_param(s, dict(key=key, P=P, scope="ch", kind="int", desc=desc, conv="shure_gain", unit="dB",
                      min=-1100, max=300, offset=1100, width=4, set_param="gain_tenth_db", set_example=-60,
                      example="1040", example_value="1040", incdec="{ type: int, min: 1, max: 1400, required: true }"))
    s.state[f"channels.*.{key}"] = ("float", "dB", desc[0].upper() + desc[1:] + ", -110.0 to +30.0 dB (wire 0-1400 in tenths offset by 1100); set_" + key + " takes tenths of a dB, -1100 to 300")


def meter(s, P, key, desc):
    add_param(s, dict(key=key, P=P, scope="dev", kind="int", desc=desc + " metering interval, ms (0: off; 1-99 refused)",
                      min=0, max=99999, width=5, set_param="rate_ms", set_example=1000, example="01000", example_value="01000", unit="ms"))


def matrix(s):
    io = "{ type: int, min: 0, max: 99, required: true }"
    s.cmd("get_matrix_route", "Matrix mixer: whether an input is routed to an output (MATRIX_MXR_ROUTE)", {"input": io, "output": io},
          f"GET {s.ch('input')} MATRIX_MXR_ROUTE {s.ch('output')}", get_expect("MATRIX_MXR_ROUTE").replace('" MATRIX_MXR_ROUTE \\\\{?', '" MATRIX_MXR_ROUTE \\\\d+ \\\\{?'), "value")
    s.vec("get_matrix_route", {"input": 1, "output": 15}, f"< GET {s.chv(1)} MATRIX_MXR_ROUTE {s.chv(15)} >", f"< REP {s.chv(1)} MATRIX_MXR_ROUTE {s.chv(15)} ON >", VALUE("ON"))
    s.cmd("set_matrix_route", "Matrix mixer: route or unroute an input to an output", {"input": io, "output": io, "enabled": "{ type: bool, default: true }"},
          f"SET {s.ch('input')} MATRIX_MXR_ROUTE {s.ch('output')} {{enabled:on_off}}", set_expect("MATRIX_MXR_ROUTE"), "ack")
    s.vec("set_matrix_route", {"input": 1, "output": 15, "enabled": False}, f"< SET {s.chv(1)} MATRIX_MXR_ROUTE {s.chv(15)} OFF >", f"< REP {s.chv(1)} MATRIX_MXR_ROUTE {s.chv(15)} OFF >", ACK)
    s.cmd("set_matrix_gain", "Matrix mixer: a crosspoint's gain in tenths of a dB, -1100 to 300 (-110.0 to +30.0 dB)", {"input": io, "output": io, "gain_tenth_db": "{ type: int, min: -1100, max: 300, required: true }"},
          f"SET {s.ch('input')} MATRIX_MXR_GAIN {s.ch('output')} {{gain_tenth_db:+1100:04d}}", set_expect("MATRIX_MXR_GAIN"), "ack")
    s.vec("set_matrix_gain", {"input": 21, "output": 17, "gain_tenth_db": -35}, f"< SET {s.chv(21)} MATRIX_MXR_GAIN {s.chv(17)} 1065 >", f"< REP {s.chv(21)} MATRIX_MXR_GAIN {s.chv(17)} 1065 >", ACK)
    s.cmd("get_matrix_gain", "Matrix mixer: a crosspoint's gain (wire value, tenths of a dB offset by 1100)", {"input": io, "output": io},
          f"GET {s.ch('input')} MATRIX_MXR_GAIN {s.ch('output')}", "{ reply_contains: \" MATRIX_MXR_GAIN \", matches: \" MATRIX_MXR_GAIN \\\\d+ ?(\\\\d{4}) *>$\", convert: shure_gain }", "value")
    s.vec("get_matrix_gain", {"input": 21, "output": 17}, f"< GET {s.chv(21)} MATRIX_MXR_GAIN {s.chv(17)} >", f"< REP {s.chv(21)} MATRIX_MXR_GAIN {s.chv(17)} 1100 >", VALUE(0.0))
    for verb, word in (("increase", s.inc[0]), ("decrease", s.inc[1])):
        s.cmd(f"{verb}_matrix_gain", f"Matrix mixer: {verb} a crosspoint's gain by a step in tenths of a dB", {"input": io, "output": io, "step": "{ type: int, min: 1, max: 1400, required: true }"},
              f"SET {s.ch('input')} MATRIX_MXR_GAIN {s.ch('output')} {word} {{step}}", set_expect("MATRIX_MXR_GAIN"), "ack")
        s.vec(f"{verb}_matrix_gain", {"input": 21, "output": 17, "step": 25}, f"< SET {s.chv(21)} MATRIX_MXR_GAIN {s.chv(17)} {word} 25 >", f"< REP {s.chv(21)} MATRIX_MXR_GAIN {s.chv(17)} 1125 >", ACK)
    s.conversions.add("shure_gain")
    s.state["matrix.*.*.routed"] = ("bool", None, "Matrix mixer crosspoint routed, keyed by input then output channel")
    s.state["matrix.*.*.gain"] = ("float", "dB", "Matrix mixer crosspoint gain, keyed by input then output channel")
    s.rules.append("    - match: \"^< REP (\\\\d+) MATRIX_MXR_ROUTE (\\\\d+) (ON|OFF) >$\"\n      state: { \"matrix.{1}.{2}.routed\": { value: \"{3}\", map: " + ONOFF_MAP + " } }")
    s.rules.append("    - match: \"^< REP (\\\\d+) MATRIX_MXR_GAIN (\\\\d+) ?(\\\\d{4}) >$\"\n      state: { \"matrix.{1}.{2}.gain\": \"{3:from.shure_gain}\" }")


def peq(s):
    io = "{ type: int, min: 0, max: 99, required: true }"
    flt = "{ type: int, min: 0, max: 16, required: true }"
    s.cmd("get_peq_filter", "Whether a PEQ filter is enabled (PEQ <block> <filter>)", {"block": io, "filter": flt},
          f"GET {s.ch('block')} PEQ {{filter:02d}}", "{ reply_contains: \" PEQ \", matches: \" PEQ \\\\d+ (ON|OFF) *>$\" }", "value")
    s.vec("get_peq_filter", {"block": 1, "filter": 2}, f"< GET {s.chv(1)} PEQ 02 >", f"< REP {s.chv(1)} PEQ 02 ON >", VALUE("ON"))
    s.cmd("set_peq_filter", "Enable, disable or toggle a PEQ filter (0: every block or filter)", {"block": io, "filter": flt, "state": "{ type: enum, values: [\"ON\", \"OFF\", \"TOGGLE\"], required: true }"},
          f"SET {s.ch('block')} PEQ {{filter:02d}} {{state}}", set_expect("PEQ"), "ack")
    s.vec("set_peq_filter", {"block": 1, "filter": 2, "state": "OFF"}, f"< SET {s.chv(1)} PEQ 02 OFF >", f"< REP {s.chv(1)} PEQ 02 OFF >", ACK)
    s.state["peq.*.*.enabled"] = ("bool", None, "A PEQ filter's enable, keyed by block then filter")
    s.rules.append("    - match: \"^< REP (\\\\d+) PEQ (\\\\d+) (ON|OFF) >$\"\n      state: { \"peq.{1}.{2}.enabled\": { value: \"{3}\", map: " + ONOFF_MAP + " } }")


def standard_channel(s, name_set=False):
    add_param(s, dict(key="name", P="CHAN_NAME", scope="ch", kind="ro_text", desc="a channel's name", example="{Podium                         }", example_value="Podium"))
    add_param(s, dict(key="mute", P="AUDIO_MUTE", scope="ch", kind="onoff_toggle", desc="a channel's mute", set_key="muted"))
    gain(s)


def emit(s):
    out = []
    out.append("spec: 1")
    out.append(f"id: {s.sid}")
    out.append(f"name: {q(s.name)}")
    out.append("vendor: Shure")
    out.append("category: mixer")
    out.append("")
    out.append(s.header.rstrip())
    out.append("source:")
    for title, url, note in s.sources:
        out.append(f"  - title: {q(title)}")
        out.append("    author: Shure Incorporated")
        out.append(f"    url: {q(url)}")
        out.append("    note: >")
        for line in wrap(note, 6):
            out.append(line)
    out.append("""
transport:
  type: line-tcp
  port: 2202
  framing: delimited
  open: "< "
  close: " >"
  encoding: ascii
  timeout_ms: 2000
  reply: expected
  # Answers and change reports are both REP; SAMPLE (metering) never answers.
  # Commands name the parameter their REP carries (expect.reply_contains), so
  # a change pushed meanwhile is never taken for an answer.
  reply_match: "^< REP "
  error_match: "^< REP ERR"
  probe: "GET MODEL"
""")
    out.append("conversions:")
    for c in sorted(s.conversions):
        pts, why = CONVERSIONS[c]
        out.append(f"  # {why}")
        out.append(f"  {c}:")
        out.append(f"    points: {pts}")
    out.append("")
    out.append("models:")
    names = list(s.commands)
    for m in s.models:
        out.append(f"  - id: {m['id']}")
        out.append(f"    name: {q(m['name'])}")
        supports = [n for n in names if n not in m.get("exclude", ())]
        out.append("    supports:")
        for n in supports:
            out.append(f"      - {n}")
        out.append("    verification: none")
        if m.get("notes"):
            out.append("    notes: >")
            out.extend(wrap(m["notes"], 6))
    out.append("")
    out.append("commands:")
    for n in names:
        out.append(s.commands[n])
    out.append("")
    out.append("state:")
    for path, (typ, unit, desc) in s.state.items():
        u = f", unit: {q(unit)}" if unit else ""
        out.append(f"  {path}: {{ type: {typ}{u}, description: {q(desc)} }}")
    out.append("")
    out.append("telemetry:")
    out.append("  # Every parameter is reported once on connecting; after that the device")
    out.append("  # sends a REP whenever a value changes, so nothing is polled.")
    out.append("  poll:")
    out.append(f"    send: [{q(s.all_cmd)}]")
    out.append("  updates:")
    out.extend(s.rules)
    out.append("")
    out.append("quirks:")
    for sev, text in COMMON_QUIRKS + s.extra_quirks:
        out.append("  - models: [all]")
        out.append(f"    severity: {sev}")
        out.append("    text: >")
        out.extend(wrap(text, 6))
    return "\n".join(out) + "\n"


def wrap(text, indent):
    import textwrap
    return [" " * indent + l for l in textwrap.wrap(" ".join(text.split()), 100 - indent)]


COMMON_QUIRKS = [
    ("info", """Commands are matched to their REP by the parameter name, not by the channel: a change to
     the same parameter on another channel arriving in the same moment could be taken for the answer.
     An error answer is < REP ERR >, which names nothing; it fails the command waiting."""),
    ("info", """Text values (names, model, serial number, addresses) are padded with spaces to a fixed
     width, sometimes in braces; the spaces and braces are removed. Channel 0 (or 00) means every
     channel, answered with one REP per channel."""),
    ("info", """Gains are sent and reported in tenths of a dB offset by 1100 (0000-1400 = -110.0 to
     +30.0 dB); set commands take tenths of a dB (-1100 to 300), and state holds dB. Increase and
     decrease steps are in tenths of a dB."""),
    ("info", """Metering (METER_RATE and its siblings) sends SAMPLE messages at the chosen interval
     until set to 0; it is never started here, and SAMPLE values (000-060 = -60 to 0 dBFS) are not
     kept in state. Values 1-99 ms are refused by the device."""),
    ("info", """On connecting the session sends one GET ALL; the device answers with a REP for every
     parameter, and thereafter reports every change unasked, so state stays current without polling.
     Opened for commands only, the GET ALL is not sent."""),
    ("info", """get_parameter, set_parameter, get_device_parameter and set_device_parameter reach any
     parameter the command-string document lists, with the value in the document's form, for those not
     given a command of their own."""),
    ("critical", """recall_preset replaces the device's whole configuration with the preset's, live,
     and cannot be undone except by recalling another preset."""),
    ("critical", """restore_defaults erases the device's configuration (presets aside) and cannot be
     undone; reboot restarts the device, interrupting its audio, and is not acknowledged."""),
]

# ── P300 ──────────────────────────────────────────────────────────────────
PUBS = "https://pubs.shure.com/command-strings/{p}/en-US"
PDF = "https://pubs2-api.prod.shureweb.eu/documents/public/download/{d}.pdf"


def src(title, product, doc, note):
    return (title, PUBS.format(p=product), f"Manufacturer document, also as a PDF at {PDF.format(d=doc)} . " + note)


def p300():
    s = Spec("shure-p300", "Shure P300 IntelliMix Audio Conferencing Processor",
             "# Written from Shure's P300 command strings. The channel numbers are the\n# P300's (document p.3-4): 01-08 Dante mic inputs, 09-10 Dante inputs, 11-12\n# analog inputs, 13 USB in, 14 mobile in, 15-16 Dante outputs 1-2, 17-18\n# analog outputs, 19 USB out, 20 mobile out, 21 automixer output, 22 AEC\n# reference, 23-28 Dante outputs 3-8 (firmware 4.1 and later); 00 is all.",
             [src("P300 Command Strings, Version 4.4 (2024-D)", "P300", 7171,
                  "TCP 2202; GET, SET, REP and SAMPLE; REP sent on change; channel table p.3-4; common commands p.5-41 (GET ALL, MODEL, SERIAL_NUM, CHAN_NAME, DEVICE_ID, FW_VER, PRESET and PRESET1-10, AUDIO_GAIN_HI_RES with INC/DEC, input and output level switches, AUDIO_MUTE, DEVICE_AUDIO_MUTE, FLASH, metering, LED_BRIGHTNESS, network, ENCRYPTION, REBOOT, PEQ, meter modes, USB_CONNECT, matrix routing and gain, CONTROL_MAC_ADDR, NA names, DEFAULT_SETTINGS, AEC, AEC_REF, ERLE metering, AEC_NLP, NOISE_RED, AGC, gate inhibit, automixer, compressor, delay, direct out tap point, call status).")],
             [dict(id="p300", name="Shure P300 IntelliMix Audio Conferencing Processor")],
             ("INC", "DEC"), "02d", "GET 00 ALL", [
                 ("info", "AUTOMXR_MUTE on channel 21 is the system mute Shure recommends for conferencing (mute after the AEC, not at the microphones)."),
                 ("info", "GATE_INHIBIT works only on firmware before 4.1; channels 23-28 (Dante outputs 3-8) need firmware 4.1 or later."),
             ])
    common_device(s, [("na_device_name", "NA_DEVICE_NAME", "the Dante device name", "{P300-ab12cd                    }"),
                      ("mac_address", "CONTROL_MAC_ADDR", "the control MAC address", "00:0E:DD:FF:F1:63")])
    add_param(s, dict(key="encryption", P="ENCRYPTION", scope="dev", kind="ro_onoff", desc="audio encryption"))
    add_param(s, dict(key="usb_connected", P="USB_CONNECT", scope="dev", kind="ro_onoff", desc="a USB host connected"))
    presets_etc(s, legacy_names=True)
    standard_channel(s)
    add_param(s, dict(key="device_mute", P="DEVICE_AUDIO_MUTE", scope="dev", kind="onoff_toggle", desc="the device mute", set_key="muted"))
    add_param(s, dict(key="input_level", P="AUDIO_IN_LVL_SWITCH", scope="ch", kind="enum", values=["LINE_LVL", "AUX_LVL"], desc="an analog input's level switch (11-12)", example="LINE_LVL", example_channel=11))
    add_param(s, dict(key="output_level", P="AUDIO_OUT_LVL_SWITCH", scope="ch", kind="enum", values=["LINE_LVL", "AUX_LVL", "MIC_LVL"], desc="an analog output's level switch (17-18)", example="LINE_LVL", example_channel=17))
    add_param(s, dict(key="led_brightness", P="LED_BRIGHTNESS", scope="dev", kind="int", min=0, max=2, desc="the LED brightness (0 off, 1 dim, 2 default)", example="2", example_value="2"))
    add_param(s, dict(key="input_meter_mode", P="INPUT_METER_MODE", scope="dev", kind="enum", values=["PRE_FADER", "POST_FADER"], desc="the input meter tap", example="PRE_FADER"))
    add_param(s, dict(key="output_meter_mode", P="OUTPUT_METER_MODE", scope="dev", kind="enum", values=["PRE_FADER", "POST_FADER"], desc="the output meter tap", example="PRE_FADER"))
    for P, key, d in [("METER_RATE_IN", "meter_rate_in", "the input"), ("METER_RATE_OUT", "meter_rate_out", "the output"),
                      ("METER_RATE_PROC", "meter_rate_proc", "the processing-block"), ("METER_RATE_ERLE", "meter_rate_erle", "the AEC ERLE"),
                      ("METER_RATE_AGC", "meter_rate_agc", "the AGC gain")]:
        meter(s, P, key, d)
    add_param(s, dict(key="aec", P="AEC", scope="ch", kind="onoff_toggle", desc="the AEC of a Dante mic channel (01-08)"))
    add_param(s, dict(key="aec_reference", P="AEC_REF", scope="ch", kind="enum", values=[f"DANTEOUT{i}" for i in range(1, 9)] + ["ANALOGOUT1", "ANALOGOUT2", "DANTEIN9", "DANTEIN10", "ANALOGIN1", "ANALOGIN2", "USBIN", "MOBILEIN"], desc="the AEC reference source (channel 22)", example="DANTEOUT1", example_channel=22))
    add_param(s, dict(key="aec_nlp", P="AEC_NLP", scope="ch", kind="enum", values=["LOW", "MEDIUM", "HIGH"], desc="the AEC non-linear processing", example="MEDIUM"))
    add_param(s, dict(key="noise_reduction", P="NOISE_RED", scope="ch", kind="onoff", desc="noise reduction"))
    add_param(s, dict(key="noise_reduction_level", P="NOISE_RED_LVL", scope="ch", kind="enum", values=["LOW", "MEDIUM", "HIGH"], desc="the noise reduction level", example="LOW"))
    add_param(s, dict(key="agc", P="AGC", scope="ch", kind="onoff_toggle", desc="automatic gain control"))
    add_param(s, dict(key="agc_max_cut", P="AGC_MAX_CUT", scope="ch", kind="int", conv="shure_agc_cut", unit="dB", min=-200, max=0, offset=200, width=3, set_param="cut_tenth_db", set_example=-123, desc="the AGC maximum cut, tenths of a dB (-200 to 0)", example="077", example_value="077"))
    add_param(s, dict(key="agc_max_boost", P="AGC_MAX_BOOST", scope="ch", kind="int", conv="shure_agc_boost", unit="dB", min=0, max=200, width=3, set_param="boost_tenth_db", set_example=123, desc="the AGC maximum boost, tenths of a dB (0 to 200)", example="123", example_value="123"))
    add_param(s, dict(key="agc_target", P="AGC_TARGET", scope="ch", kind="int", conv="shure_agc_target", unit="dBFS", min=-500, max=0, offset=500, width=3, set_param="target_tenth_dbfs", set_example=-123, desc="the AGC target, tenths of a dBFS (-500 to 0)", example="377", example_value="377"))
    add_param(s, dict(key="gate_inhibit", P="GATE_INHIBIT", scope="ch", kind="onoff_toggle", desc="the gate inhibit (channel 22; firmware before 4.1)", example_channel=22))
    add_param(s, dict(key="automixer_mode", P="AUTOMXR_MODE", scope="ch", kind="enum", values=["MANUAL", "GAINSHARE", "GATING"], desc="the automixer mode (channel 21)", example="GATING", example_channel=21))
    add_param(s, dict(key="automixer_off_attenuation", P="AUTOMXR_OFF_ATT", scope="ch", kind="int", conv="shure_off_att", unit="dB", min=-110, max=-3, offset=110, width=3, set_param="attenuation_db", set_example=-10, desc="the automixer off attenuation, dB (-110 to -3)", example="100", example_value="100", example_channel=21))
    add_param(s, dict(key="automixer_gate_sensitivity", P="AUTOMXR_GATE_SEN", scope="ch", kind="int", min=1, max=9, desc="the automixer gating sensitivity (1-9)", example="5", example_value="5", example_channel=21))
    add_param(s, dict(key="automixer_max_open", P="AUTOMXR_MAX_NOM", scope="ch", kind="int", min=1, max=8, desc="the automixer maximum number of open mics (1-8)", example="4", example_value="4", example_channel=21))
    add_param(s, dict(key="automixer_last_mic_lock", P="AUTOMXR_LMLO", scope="ch", kind="onoff_toggle", desc="the automixer last mic lock-on", example_channel=21))
    add_param(s, dict(key="automixer_hold_time", P="AUTOMXR_HOLDTIME", scope="ch", kind="int", min=100, max=1500, width=4, unit="ms", desc="the automixer hold time, ms", example="0400", example_value="0400", example_channel=21))
    add_param(s, dict(key="automixer_always_on", P="AUTOMXR_ALWAYS_ON", scope="ch", kind="onoff_toggle", desc="a channel always on in the automixer"))
    add_param(s, dict(key="automixer_priority", P="AUTOMXR_PRIORITY", scope="ch", kind="onoff_toggle", desc="a channel's automixer priority"))
    add_param(s, dict(key="automixer_mute", P="AUTOMXR_MUTE", scope="ch", kind="onoff_toggle", desc="the automixer post-gate mute (channel 21: the system mute)", set_key="muted", example_channel=21))
    add_param(s, dict(key="automixer_gate", P="AUTOMXR_GATE", scope="ch", kind="ro_onoff", desc="a channel's automixer gate (open)"))
    add_param(s, dict(key="compressor", P="COMPRESSOR", scope="ch", kind="onoff_toggle", desc="the compressor (channel 21)", example_channel=21))
    add_param(s, dict(key="compressor_threshold", P="COMP_THRESHOLD", scope="ch", kind="int", conv="shure_comp_threshold", unit="dB", min=-600, max=0, offset=600, width=3, set_param="threshold_tenth_db", set_example=-123, desc="the compressor threshold, tenths of a dB (-600 to 0)", example="477", example_value="477", example_channel=21))
    add_param(s, dict(key="compressor_ratio", P="COMP_RATIO", scope="ch", kind="int", conv="shure_comp_ratio", min=10, max=1000, width=4, set_param="ratio_tenths", set_example=123, desc="the compressor ratio in tenths (10 = 1.0:1 to 1000 = 100.0:1)", example="0123", example_value="0123", example_channel=21))
    add_param(s, dict(key="delay", P="DELAY", scope="ch", kind="int", min=0, max=1000, width=4, unit="ms", desc="an output's delay, ms (0 off; channels 17-19)", example="0010", example_value="0010", example_channel=17))
    add_param(s, dict(key="direct_out_point", P="DIRECTOUT_POINT", scope="ch", kind="int", min=0, max=3, desc="the direct output tap point (0 pre-gate/pre-processing, 1 pre-gate/post, 2 post-gate/pre, 3 post-gate/post; firmware 4.1 and later)", example="3", example_value="3"))
    add_param(s, dict(key="call_status_enabled", P="ONHOOK_ENABLE", scope="dev", kind="onoff", desc="the call status feature"))
    add_param(s, dict(key="call_state", P="ONHOOK_STATE", scope="dev", kind="ro_text", desc="the call status (ONHOOK or OFFHOOK)", example="OFFHOOK"))
    matrix(s)
    peq(s)
    return s


def imx():
    s = Spec("shure-imx-room", "Shure IntelliMix Room (software DSP)",
             "# Written from Shure's IntelliMix Room command strings. Channels: 01-16\n# Dante mic inputs (8 or 16 licensed), 17-24 Dante line inputs, 25 virtual\n# audio in, 26 PC in, 27-34 Dante outputs, 35 virtual audio out, 36 PC out,\n# 37 automix output, 55 VAD input right; 00 is all.",
             [src("IntelliMix Room Command Strings, Version 7.5 (2024-G)", "IntelliMixRoom", 7549,
                  "TCP 2202 on the computer running IntelliMix Room; channel table p.3; device information (GET ALL, MODEL, FW_VER, DEVICE_ID, NA_DEVICE_NAME, PRESET, call status), AUDIO_GAIN_HI_RES with inc/dec, DEVICE_AUDIO_MUTE, AUDIO_MUTE, matrix routing and gain, AUTOMXR_MUTE, AUDIO_GAIN_POSTGATE, AUTOMXR_GATE, licence commands (CHAN_CONFIG, CHAN_COUNT, LIC_EXP_DATE, LIC_TYPE, LIC_VALID) and DENOISER_ENABLE / DENOISER_LEVEL.")],
             [dict(id="intellimix-room", name="Shure IntelliMix Room (8 or 16 channels)")],
             ("inc", "dec"), "02d", "GET 00 ALL", [
                 ("info", "Commands for channels beyond the licensed count (8-channel licences) fail with REP ERR."),
             ])
    for key, P, desc, ex in [("model", "MODEL", "the model", "{IntelliMix Room                 }"), ("firmware", "FW_VER", "the software version", "{7.5.0             }"),
                             ("device_id", "DEVICE_ID", "the device ID", "{Room 101                       }"), ("na_device_name", "NA_DEVICE_NAME", "the Dante device name", "{IMXR-ab12                      }")]:
        add_param(s, dict(key=key, P=P, scope="dev", kind="ro_text", desc=desc, example=ex, example_value=ex.strip("{}").strip()))
    s.cmd("get_preset", "The preset last recalled (PRESET)", {}, "GET PRESET", get_expect("PRESET"), "value")
    s.vec("get_preset", {}, "< GET PRESET >", "< REP PRESET 03 >", VALUE("03"))
    s.state["device.preset"] = ("int", None, "The preset last recalled")
    s.rules.append("    - match: \"^< REP PRESET (\\\\d+) >$\"\n      state: { \"device.preset\": \"{1}\" }")
    s.cmd("recall_preset", "Recall a preset (SET PRESET), 1-10", {"preset": "{ type: int, min: 1, max: 10, required: true }"}, "SET PRESET {preset:02d}", set_expect("PRESET"), "ack")
    s.vec("recall_preset", {"preset": 3}, "< SET PRESET 03 >", "< REP PRESET 03 >", ACK)
    s.cmd("get_all", "Ask for every parameter of a channel (0: all); the answers update state", {"channel": CH}, "GET {channel:02d} ALL", None, "none")
    s.vec("get_all", {"channel": 0}, "< GET 00 ALL >", None, "{'ok': {'kind': 'unverified'}}")
    add_param(s, dict(key="device_mute", P="DEVICE_AUDIO_MUTE", scope="dev", kind="onoff_toggle", desc="the device mute", set_key="muted"))
    add_param(s, dict(key="mute", P="AUDIO_MUTE", scope="ch", kind="onoff_toggle", desc="a channel's mute", set_key="muted"))
    gain(s)
    add_param(s, dict(key="postgate_gain", P="AUDIO_GAIN_POSTGATE", scope="ch", kind="int", conv="shure_gain", unit="dB", min=-1099, max=300, offset=1100, width=4, set_param="gain_tenth_db", set_example=0, desc="a channel's post-gate gain (01-16, 37)", example="1100", example_value="1100"))
    add_param(s, dict(key="automixer_mute", P="AUTOMXR_MUTE", scope="ch", kind="onoff_toggle", desc="the automixer post-gate mute (01-16, 37)", set_key="muted", example_channel=37))
    add_param(s, dict(key="automixer_gate", P="AUTOMXR_GATE", scope="ch", kind="ro_onoff", desc="a channel's automixer gate (open)"))
    add_param(s, dict(key="channels_configured", P="CHAN_CONFIG", scope="dev", kind="ro_text", desc="whether the channels are configured (TRUE/FALSE)", example="TRUE"))
    add_param(s, dict(key="licensed_channels", P="CHAN_COUNT", scope="dev", kind="ro_int", desc="the licensed channel count (8 or 16)", example="16", example_value="16"))
    add_param(s, dict(key="licence_expiry", P="LIC_EXP_DATE", scope="dev", kind="ro_text", desc="the licence expiry date (yyyy-mm-dd)", example="2026-12-31"))
    add_param(s, dict(key="licence_type", P="LIC_TYPE", scope="dev", kind="ro_text", desc="the licence type (DEMO, PAID, TRIAL)", example="PAID"))
    add_param(s, dict(key="licence_valid", P="LIC_VALID", scope="dev", kind="ro_text", desc="whether the licence is valid (TRUE/FALSE)", example="TRUE"))
    add_param(s, dict(key="denoiser", P="DENOISER_ENABLE", scope="ch", kind="onoff", desc="the denoiser (channel 00 or 37)", example_channel=37))
    add_param(s, dict(key="denoiser_level", P="DENOISER_LEVEL", scope="ch", kind="enum", values=["LOW", "MEDIUM", "HIGH"], desc="the denoiser level (channel 00 or 37)", example="LOW", example_channel=37))
    add_param(s, dict(key="call_status_enabled", P="ONHOOK_ENABLE", scope="dev", kind="onoff", desc="the call status feature"))
    add_param(s, dict(key="call_state", P="ONHOOK_STATE", scope="dev", kind="ro_text", desc="the call status (ONHOOK or OFFHOOK)", example="ONHOOK"))
    matrix(s)
    return s


def ani():
    s = Spec("shure-ani", "Shure ANI audio network interfaces (ANI4IN, ANI4OUT, ANIUSB-MATRIX, ANI22)",
             "# Written from Shure's command strings for each interface. Channels:\n# ANI4IN 1-4 inputs; ANI4OUT 1-4 outputs; ANI22 01-02 analog inputs, 03-04\n# analog outputs; ANIUSB-MATRIX 00-10 (see its document); 0 or 00 is all.",
             [src("ANI4IN Command Strings, Version 2.2 (2024-F)", "ANI4IN", 7317, "TCP 2202; GET ALL, device information, CHAN_NAME, presets, AUDIO_GAIN_HI_RES and the analog AUDIO_GAIN (00-51 in 3 dB steps) with INC/DEC, AUDIO_MUTE, FLASH, METER_RATE, sig/clip LED, LED_BRIGHTNESS, PHANTOM_PWR_ENABLE, HW_GATING_LOGIC, CHAN_LED_IN_STATE (no answer), REBOOT, INPUT_METER_MODE, LIMITER_ENGAGED, AUDIO_SUMMING_MODE, levels, NA names, DEFAULT_SETTINGS, PEQ, ENCRYPTION."),
              src("ANI4OUT Command Strings, Version 3 (2019-G)", "ANI4OUT", 7106, "TCP 2202; as the ANI4IN for outputs, with AUDIO_OUT_LVL_SWITCH (LINE_LVL, AUX_LVL, MIC_LVL), LAST_ERROR_EVENT and OUTPUT_METER_MODE."),
              src("ANIUSB-Matrix Command Strings, Version 4.7 (2024-E)", "ANIUSB-MATRIX", 7125, "TCP 2202; GET ALL (V1 only), presets incl. PRESET_AUDIO_ROUTE, gain, level switches, AUDIO_MUTE, DEVICE_AUDIO_MUTE, metering, LED, limiter, ENCRYPTION_CH, PEQ, meter modes, USB_CONNECT, matrix routing and gain, LOGIC_MUTE, call status."),
              src("ANI22 Command Strings, Version 2.2 (2024-L)", "ANI22", 7123, "TCP 2202; channel table p.3; gain (digital and analog), AUDIO_MUTE, DEVICE_AUDIO_MUTE, output level switch, metering, LED, PHANTOM_PWR_ENABLE, AUDIO_SUMMING_MODE, logic, PEQ.")],
             [dict(id="ani4in", name="Shure ANI4IN (XLR and BLOCK)", exclude=("set_output_level", "get_output_level", "set_device_mute", "toggle_device_mute", "get_device_mute", "get_matrix_route", "set_matrix_route", "set_matrix_gain", "get_matrix_gain", "increase_matrix_gain", "decrease_matrix_gain", "get_usb_connected", "set_logic_mute", "toggle_logic_mute", "get_logic_mute", "get_last_error", "get_output_meter_mode", "set_output_meter_mode")),
              dict(id="ani4out", name="Shure ANI4OUT (XLR and BLOCK)", exclude=("set_analog_gain", "get_analog_gain", "increase_analog_gain", "decrease_analog_gain", "set_phantom", "get_phantom", "get_logic_out", "set_led_in", "get_device_mute", "set_device_mute", "toggle_device_mute", "get_matrix_route", "set_matrix_route", "set_matrix_gain", "get_matrix_gain", "increase_matrix_gain", "decrease_matrix_gain", "get_usb_connected", "set_logic_mute", "toggle_logic_mute", "get_logic_mute", "get_input_meter_mode", "set_input_meter_mode")),
              dict(id="aniusb-matrix", name="Shure ANIUSB-MATRIX", exclude=("set_analog_gain", "get_analog_gain", "increase_analog_gain", "decrease_analog_gain", "set_phantom", "get_phantom", "get_logic_out", "set_led_in", "set_summing_mode", "get_summing_mode", "get_last_error")),
              dict(id="ani22", name="Shure ANI22 (XLR and BLOCK)", exclude=("get_matrix_route", "set_matrix_route", "set_matrix_gain", "get_matrix_gain", "increase_matrix_gain", "decrease_matrix_gain", "get_usb_connected", "set_logic_mute", "toggle_logic_mute", "get_logic_mute", "get_last_error", "get_output_meter_mode", "set_output_meter_mode"))],
             ("INC", "DEC"), "02d", "GET 00 ALL", [
                 ("info", "The ANI4IN and ANI4OUT documents write channel numbers as one digit (x); the session sends two (01), as the ANI22 and ANIUSB-MATRIX documents do."),
                 ("info", "set_led_in (CHAN_LED_IN_STATE) is not answered on the ANI4IN, so it is sent without waiting."),
                 ("info", "Phantom power and the analog gain (00-51 dB in 3 dB steps) are on the analog inputs only."),
             ])
    common_device(s)
    add_param(s, dict(key="encryption", P="ENCRYPTION", scope="dev", kind="ro_onoff", desc="audio encryption"))
    presets_etc(s, legacy_names=True)
    standard_channel(s)
    add_param(s, dict(key="analog_gain", P="AUDIO_GAIN", scope="ch", kind="int", min=0, max=51, width=2, unit="dB", set_param="gain_db", set_example=24, desc="an analog input's preamp gain, dB in 3 dB steps (00-51)", example="24", example_value="24", incdec="{ type: int, min: 3, max: 51, required: true }"))
    add_param(s, dict(key="device_mute", P="DEVICE_AUDIO_MUTE", scope="dev", kind="onoff_toggle", desc="the device mute", set_key="muted"))
    add_param(s, dict(key="output_level", P="AUDIO_OUT_LVL_SWITCH", scope="ch", kind="enum", values=["LINE_LVL", "AUX_LVL", "MIC_LVL"], desc="an analog output's level switch", example="LINE_LVL"))
    add_param(s, dict(key="phantom", P="PHANTOM_PWR_ENABLE", scope="ch", kind="onoff", desc="an input's phantom power"))
    add_param(s, dict(key="logic_out", P="HW_GATING_LOGIC", scope="ch", kind="ro_onoff", desc="an input's mic logic switch out"))
    s.cmd("set_led_in", "Set an input's mic logic LED in (CHAN_LED_IN_STATE); not answered on the ANI4IN", {"channel": CH, "enabled": "{ type: bool, default: true }"}, "SET {channel:02d} CHAN_LED_IN_STATE {enabled:on_off}", None, "none")
    s.vec("set_led_in", {"channel": 1, "enabled": True}, "< SET 01 CHAN_LED_IN_STATE ON >", None, "{'ok': {'kind': 'unverified'}}")
    add_param(s, dict(key="clip_indicator", P="AUDIO_OUT_CLIP_INDICATOR", scope="ch", kind="ro_onoff", desc="a channel's clip indicator"))
    add_param(s, dict(key="limiter_engaged", P="LIMITER_ENGAGED", scope="ch", kind="ro_onoff", desc="a channel's limiter engaged"))
    add_param(s, dict(key="sig_clip_led", P="LED_COLOR_SIG_CLIP", scope="ch", kind="ro_text", desc="a channel's sig/clip LED colour (OFF, GREEN, AMBER, RED)", example="GREEN"))
    add_param(s, dict(key="led_brightness", P="LED_BRIGHTNESS", scope="dev", kind="int", min=0, max=2, desc="the LED brightness (0 off, 1 dim, 2 default)", example="2", example_value="2"))
    add_param(s, dict(key="summing_mode", P="AUDIO_SUMMING_MODE", scope="dev", kind="enum", values=["OFF", "1+2", "3+4", "1+2/3+4", "1+2+3+4"], desc="the audio summing mode", example="OFF"))
    add_param(s, dict(key="input_meter_mode", P="INPUT_METER_MODE", scope="dev", kind="enum", values=["PRE_FADER", "POST_FADER"], desc="the input meter tap", example="PRE_FADER"))
    add_param(s, dict(key="output_meter_mode", P="OUTPUT_METER_MODE", scope="dev", kind="enum", values=["PRE_FADER", "POST_FADER"], desc="the output meter tap", example="PRE_FADER"))
    add_param(s, dict(key="usb_connected", P="USB_CONNECT", scope="dev", kind="ro_onoff", desc="a USB host connected (ANIUSB-MATRIX)"))
    add_param(s, dict(key="logic_mute", P="LOGIC_MUTE", scope="dev", kind="onoff_toggle", desc="the logic mute (ANIUSB-MATRIX)", set_key="muted"))
    add_param(s, dict(key="last_error", P="LAST_ERROR_EVENT", scope="dev", kind="ro_text", desc="the last error event (ANI4OUT)", example="{none}", example_value="none"))
    meter(s, "METER_RATE", "meter_rate", "the")
    matrix(s)
    peq(s)
    return s


def mxa():
    s = Spec("shure-mxa", "Shure Microflex Advance array microphones (MXA910, MXA920, MXA710, MXA310, MXA902, MXA901)",
             "# Written from Shure's command strings for each microphone. Channel numbers\n# differ by model (each document's Channel Number Assignments): typically the\n# lobes or Dante outputs first, then the automixer output (MXA910 and MXA920:\n# 09; MXA310: 05) and the AEC reference; 0 is all.",
             [src("MXA910 Command Strings, Version 6.2 (2021-J)", "MXA910", 6999, "TCP 2202; channels 0-9; device information, CHAN_NAME, gains incl. post-gate and echo reduction, mutes, clip indicator, FLASH, metering rates, levels, presets, gate outputs, LED colours, states and brightness, lobe steering BEAM_X/Y/Z/W and autofocus positions, REBOOT, LAST_ERROR_EVENT, filters, IntelliMix bypass, NA names, NUM_ACTIVE_MICS, PEQ, solo, ENCRYPTION."),
              src("MXA920 Command Strings, Version 1.4 (2025-J)", "MXA920", 8372, "TCP 2202; Telnet negotiation not supported; channel table and automatic coverage on/off p.3-5; device information, presets incl. PRESET_NAME, gains, mutes incl. post-gate and coverage areas, speech gating, noise filter, metering, LEDs (brightness 0-5, 17 colours, ON/FLASHING/OFF), PEQ, EQ contour, lobe width, ARRAY_HEIGHT, AUTO_COVERAGE, VAB, talker positions, array groups, gate outputs, lobe steering, AUTOFOCUS, coverage areas."),
              src("MXA710 Command Strings, Version 3.8 (2025-G)", "MXA710", 7643, "TCP 2202; 2 ft and 4 ft channel tables; DEVICE_INSTALLATION; BEAM_ANGLE (-90 to +90) and BEAM_W per lobe; as the MXA920 otherwise."),
              src("MXA310 Command Strings, Version 3.2 (2023-C)", "MXA310", 7094, "TCP 2202; channels 0-5; polar pattern, lobe angle, mute button and LED states, external switch, low-cut filter, mute control function, as the MXA910 otherwise."),
              src("MXA902 Command Strings, Version 0.5 (2025-G)", "MXA902", 9932, "TCP 2202; automixer output 09, speaker output 10, Dante inputs 11-12; SPEAKER, DELAY (1-160 ms on 10), COMPRESSOR, signal generator."),
              src("MXA901-R Command Strings, Version 0.3 (2025-F)", "MXA901-R", 10256, "TCP 2202; output 09, AEC reference 10; ARRAY_HEIGHT, talker positions, AUTOFOCUS.")],
             [dict(id="mxa910", name="Shure MXA910 ceiling array", notes="Channels 1-8 lobes, 9 automixer output. Its document writes the gain step keywords INC and DEC in capitals."),
              dict(id="mxa920", name="Shure MXA920 ceiling array (round and square)", notes="With automatic coverage on, channels 1-8 are coverage areas; off, Dante outputs and post-gate channels 1-8; 9 automixer output, 10 AEC reference."),
              dict(id="mxa710", name="Shure MXA710 linear array (2 ft and 4 ft)", notes="2 ft: lobes 1-4, automixer output 5; 4 ft: lobes 1-8 and the automixer output after them."),
              dict(id="mxa310", name="Shure MXA310 table array", notes="Channels 1-4 lobes, 5 automixer output. Its document writes INC and DEC in capitals."),
              dict(id="mxa902", name="Shure MXA902 integrated conferencing ceiling array", notes="Automixer output 09, speaker output 10, Dante inputs 11-12."),
              dict(id="mxa901", name="Shure MXA901 ceiling array (round and square)", notes="Output 09, AEC reference 10.")],
             ("inc", "dec"), "02d", "GET 00 ALL", [
                 ("warning", "The MXA910 and MXA310 documents write the gain step keywords as INC and DEC, the newer documents as inc and dec; the session sends the lower-case form to every model."),
                 ("info", "Which parameters a microphone answers depends on its model and, on the MXA920, on whether automatic coverage is on; others are answered REP ERR."),
                 ("info", "Lobe X/Y positions are sent and reported offset by 1524 cm (0-3048 = -1524 to +1524 cm); set commands take cm relative to the centre."),
             ])
    common_device(s, [("na_device_name", "NA_DEVICE_NAME", "the Dante device name", "{MXA920-ab12                    }"),
                      ("mac_address", "CONTROL_MAC_ADDR", "the control MAC address", "00:0E:DD:FF:F1:63")])
    add_param(s, dict(key="encryption", P="ENCRYPTION", scope="dev", kind="ro_onoff", desc="audio encryption"))
    presets_etc(s, legacy_names=False)
    standard_channel(s)
    add_param(s, dict(key="postgate_gain", P="AUDIO_GAIN_POSTGATE", scope="ch", kind="int", conv="shure_gain", unit="dB", min=-1099, max=300, offset=1100, width=4, set_param="gain_tenth_db", set_example=0, desc="a channel's post-gate gain", example="1100", example_value="1100"))
    add_param(s, dict(key="device_mute", P="DEVICE_AUDIO_MUTE", scope="dev", kind="onoff_toggle", desc="the device mute", set_key="muted"))
    add_param(s, dict(key="postgate_mute", P="AUDIO_MUTE_POSTGATE", scope="ch", kind="onoff_toggle", desc="a channel's post-gate mute (MXA920, coverage off)", set_key="muted"))
    add_param(s, dict(key="coverage_mute", P="CA_MUTE", scope="ch", kind="onoff_toggle", desc="a coverage area's mute (MXA920, coverage on)", set_key="muted"))
    add_param(s, dict(key="coverage_gain", P="CA_GAIN", scope="ch", kind="int", conv="shure_gain", unit="dB", min=-1100, max=300, offset=1100, width=4, set_param="gain_tenth_db", set_example=-60, desc="a coverage area's gain (MXA920, coverage on)", example="1100", example_value="1100"))
    add_param(s, dict(key="clip_indicator", P="AUDIO_OUT_CLIP_INDICATOR", scope="ch", kind="ro_onoff", desc="a channel's clip indicator"))
    add_param(s, dict(key="active_mics", P="NUM_ACTIVE_MICS", scope="dev", kind="ro_int", desc="the number of active channels", example="2", example_value="2"))
    add_param(s, dict(key="solo", P="CHAN_AUTOMIX_SOLO_EN", scope="ch", kind="enum", values=["ENABLE", "DISABLE"], desc="a channel's automix solo", example="DISABLE"))
    add_param(s, dict(key="gate_out", P="AUTOMIX_GATE_OUT_EXT_SIG", scope="ch", kind="ro_onoff", desc="a lobe's automixer gate out"))
    add_param(s, dict(key="coverage_gate_out", P="AUTOMIX_GATE_OUT_CA", scope="ch", kind="ro_onoff", desc="a coverage area's automixer gate out (MXA920)"))
    add_param(s, dict(key="speech_gating", P="SPEECH_GATING", scope="ch", kind="enum", values=["Off", "Low", "Medium", "High", "ON", "OFF"], desc="the automixer speech gating (MXA920: Off/Low/Medium/High; MXA902 and MXA901: ON/OFF)", example="Low", example_channel=9))
    add_param(s, dict(key="noise_filter", P="NOISE_FILTER", scope="ch", kind="enum", values=["Off", "Low", "Medium", "High", "ON", "OFF"], desc="the enhanced noise filtering (MXA920: Off/Low/Medium/High; MXA902 and MXA901: ON/OFF)", example="Off", example_channel=9))
    for P, key, d in [("METER_RATE", "meter_rate", "the output"), ("METER_RATE_POSTGATE", "meter_rate_postgate", "the post-gate"),
                      ("METER_RATE_MXR_GAIN", "meter_rate_mixer_gain", "the automixer gain"), ("METER_RATE_PRECOMP", "meter_rate_precomp", "the pre-compressor"),
                      ("METER_RATE_AECREF", "meter_rate_aec_reference", "the AEC reference"), ("CA_METER_RATE", "meter_rate_coverage", "the coverage area")]:
        meter(s, P, key, d)
    add_param(s, dict(key="mute_led", P="DEV_MUTE_STATUS_LED_STATE", scope="dev", kind="ro_onoff", desc="the mute LED (on = muted)"))
    add_param(s, dict(key="led_brightness", P="LED_BRIGHTNESS", scope="dev", kind="int", min=0, max=5, desc="the LED brightness (0 off, 1-5 = 20-100%; older firmware 0-2)", example="5", example_value="5"))
    add_param(s, dict(key="led_color_unmuted", P="LED_COLOR_UNMUTED", scope="dev", kind="enum", values=COLORS, desc="the LED colour when unmuted", example="GREEN"))
    add_param(s, dict(key="led_color_muted", P="LED_COLOR_MUTED", scope="dev", kind="enum", values=COLORS, desc="the LED colour when muted", example="RED"))
    add_param(s, dict(key="led_state_muted", P="LED_STATE_MUTED", scope="dev", kind="enum", values=["ON", "FLASHING", "OFF"], desc="the LED behaviour when muted", example="ON"))
    add_param(s, dict(key="led_state_unmuted", P="LED_STATE_UNMUTED", scope="dev", kind="enum", values=["ON", "FLASHING", "OFF"], desc="the LED behaviour when unmuted", example="ON"))
    add_param(s, dict(key="led_in", P="DEV_LED_IN_STATE", scope="dev", kind="onoff", desc="the LED-in state (on = unmuted)"))
    add_param(s, dict(key="bypass_eq", P="BYPASS_ALL_EQ", scope="dev", kind="onoff_toggle", desc="bypass of all EQ"))
    add_param(s, dict(key="bypass_intellimix", P="BYPASS_IMX", scope="dev", kind="onoff_toggle", desc="bypass of the IntelliMix DSP"))
    add_param(s, dict(key="eq_contour", P="EQ_CONTOUR", scope="dev", kind="onoff", desc="the EQ contour"))
    add_param(s, dict(key="lobe_width", P="BEAM_W", scope="ch", kind="enum", values=["NARROW", "MEDIUM", "WIDE"], desc="a lobe's width", example="MEDIUM"))
    for axis in ("X", "Y"):
        add_param(s, dict(key=f"lobe_{axis.lower()}", P=f"BEAM_{axis}", scope="ch", kind="int", conv="shure_xy", unit="cm", min=-1524, max=1524, offset=1524, width=4, set_param="position_cm", set_example=-457, desc=f"a lobe's {axis} position, cm from the centre", example="1067", example_value="1067"))
        add_param(s, dict(key=f"autofocus_{axis.lower()}", P=f"BEAM_{axis}_AF", scope="ch", kind="ro_int", conv="shure_xy", unit="cm", desc=f"a lobe's autofocus {axis} position, cm from the centre", example="1524", example_value="1524"))
    add_param(s, dict(key="lobe_z", P="BEAM_Z", scope="ch", kind="int", min=0, max=914, width=4, unit="cm", set_param="height_cm", set_example=152, desc="a lobe's height below the array, cm", example="0152", example_value="0152"))
    add_param(s, dict(key="autofocus_z", P="BEAM_Z_AF", scope="ch", kind="ro_int", unit="cm", desc="a lobe's autofocus height below the array, cm", example="0152", example_value="0152"))
    add_param(s, dict(key="autofocus", P="AUTOFOCUS", scope="dev", kind="onoff", desc="autofocus"))
    add_param(s, dict(key="array_height", P="ARRAY_HEIGHT", scope="dev", kind="int", min=122, max=914, unit="cm", set_param="height_cm", set_example=274, desc="the array's height above the floor, cm", example="274", example_value="274"))
    add_param(s, dict(key="automatic_coverage", P="AUTO_COVERAGE", scope="dev", kind="onoff", desc="automatic coverage (MXA920)"))
    add_param(s, dict(key="acoustic_boundary", P="VAB", scope="dev", kind="int", min=0, max=20, desc="the virtual acoustic boundary strength (0 off, MXA920)", example="5", example_value="5"))
    add_param(s, dict(key="talker_position_rate", P="TALKER_POSITION_RATE", scope="dev", kind="int", min=0, max=99999, width=5, unit="ms", set_param="rate_ms", set_example=1000, desc="the talker position reporting interval, ms (0 off)", example="01000", example_value="01000"))
    add_param(s, dict(key="talker_sensitivity", P="TALKER_POSITION_SENSITIVITY", scope="dev", kind="int", min=0, max=11, desc="the talker position sensitivity setting (0-2 localisation, 4-7 voice detection, 8/9/11 reflection and height correction)", example="0", example_value="0"))
    add_param(s, dict(key="installation", P="DEVICE_INSTALLATION", scope="dev", kind="enum", values=["CEILING", "WALL_HORIZONTAL", "WALL_VERTICAL", "TABLE"], desc="the installation position (MXA710)", example="TABLE"))
    add_param(s, dict(key="lobe_angle", P="BEAM_ANGLE", scope="ch", kind="int", min=-90, max=90, unit="degrees", set_param="angle", set_example=-30, desc="a lobe's angle (MXA710)", example="-30", example_value="-30"))
    add_param(s, dict(key="speaker", P="SPEAKER", scope="dev", kind="onoff", desc="the loudspeaker (MXA902)"))
    add_param(s, dict(key="delay", P="DELAY", scope="ch", kind="int", min=0, max=160, width=4, unit="ms", desc="the loudspeaker delay, ms (MXA902 channel 10; 0 off)", example="0010", example_value="0010", example_channel=10))
    add_param(s, dict(key="compressor", P="COMPRESSOR", scope="ch", kind="onoff_toggle", desc="the compressor (MXA902)", example_channel=9))
    add_param(s, dict(key="signal_generator_type", P="SIG_GEN_TYPE", scope="ch", kind="enum", values=["PINK", "WHITE", "TONE"], desc="the signal generator type (MXA902)", example="PINK", example_channel=10))
    add_param(s, dict(key="signal_generator_frequency", P="SIG_GEN_FREQ", scope="ch", kind="int", min=100, max=20000, unit="Hz", set_param="frequency_hz", set_example=1000, desc="the signal generator tone frequency, Hz (MXA902)", example="1000", example_value="1000", example_channel=10))
    add_param(s, dict(key="signal_generator", P="SIG_GEN", scope="ch", kind="enum", values=["START", "STOP", "TOGGLE"], desc="the signal generator (MXA902)", example="STOP", example_channel=10))
    add_param(s, dict(key="polar_pattern", P="POLAR_PATTERN", scope="ch", kind="enum", values=["TOROID", "OMNI", "CARDIOID", "SUPER", "HYPER", "BIDIRECTION"], desc="a lobe's polar pattern (MXA310)", example="CARDIOID"))
    add_param(s, dict(key="mute_button", P="MUTE_BUTTON_STATUS", scope="dev", kind="ro_onoff", desc="the mute button pressed (MXA310)"))
    peq(s)
    return s


def mxn5():
    s = Spec("shure-mxn5", "Shure MXN5-C networked ceiling loudspeaker",
             "# Written from Shure's MXN5-C command strings. Channels: 01-02 Dante inputs,\n# 03 summed input, 04 Dante output; 00 is all.",
             [src("MXN5-C Command Strings, Version 1.5 (2024-E)", "MXN5-C", 7644, "TCP 2202; channel table p.3; GET ALL and GET x ALL (MXN5-C-V1 only); device information; CHAN_NAME and NA names; FLASH; clip indicator; METER_RATE with 4-channel SAMPLE; AUDIO_GAIN_HI_RES with inc/dec; DEVICE_AUDIO_MUTE; AUDIO_MUTE; PRESET and PRESET_NAME; DEFAULT_SETTINGS; LIMITER_ENGAGED; ENCRYPTION; REBOOT; PEQ (block 03); DELAY (1-160 ms on 03); BYPASS_DSP; signal generator type, frequency (125-20000 Hz), gain (0-1310) and start/stop.")],
             [dict(id="mxn5-c", name="Shure MXN5-C networked ceiling loudspeaker")],
             ("inc", "dec"), "02d", "GET 00 ALL", [
                 ("info", "GET x ALL works only on MXN5-C-V1 units; on others the connect-time read is answered with an error and state fills as values change."),
                 ("warning", "The signal generator plays pink or white noise, a tone or a sweep through the loudspeaker; set its gain before starting it."),
             ])
    common_device(s, [("na_device_name", "NA_DEVICE_NAME", "the Dante device name", "{MXN5-ab12                      }"),
                      ("mac_address", "CONTROL_MAC_ADDR", "the control MAC address", "00:0E:DD:FF:F1:63")])
    add_param(s, dict(key="encryption", P="ENCRYPTION", scope="dev", kind="ro_onoff", desc="audio encryption"))
    presets_etc(s, legacy_names=False)
    standard_channel(s)
    add_param(s, dict(key="device_mute", P="DEVICE_AUDIO_MUTE", scope="dev", kind="onoff_toggle", desc="the device mute", set_key="muted"))
    add_param(s, dict(key="clip_indicator", P="AUDIO_OUT_CLIP_INDICATOR", scope="ch", kind="ro_onoff", desc="a channel's clip indicator"))
    add_param(s, dict(key="limiter_engaged", P="LIMITER_ENGAGED", scope="ch", kind="ro_onoff", desc="the limiter engaged"))
    meter(s, "METER_RATE", "meter_rate", "the")
    add_param(s, dict(key="delay", P="DELAY", scope="ch", kind="int", min=0, max=160, width=4, unit="ms", desc="the delay, ms (channel 03; 0 off)", example="0010", example_value="0010", example_channel=3))
    add_param(s, dict(key="bypass_dsp", P="BYPASS_DSP", scope="dev", kind="onoff_toggle", desc="bypass of the EQ, delay and limiter"))
    add_param(s, dict(key="signal_generator_type", P="SIG_GEN_TYPE", scope="ch", kind="enum", values=["PINK", "WHITE", "TONE", "SWEEP"], desc="the signal generator type (channel 03)", example="PINK", example_channel=3))
    add_param(s, dict(key="signal_generator_frequency", P="SIG_GEN_FREQ", scope="ch", kind="int", min=125, max=20000, unit="Hz", set_param="frequency_hz", set_example=1000, desc="the signal generator tone frequency, Hz", example="1000", example_value="1000", example_channel=3))
    add_param(s, dict(key="signal_generator_gain", P="SIG_GEN_GAIN", scope="ch", kind="int", conv="shure_gain", unit="dB", min=-1100, max=210, offset=1100, width=4, set_param="gain_tenth_db", set_example=-200, desc="the signal generator gain, tenths of a dB (-1100 to 210)", example="0900", example_value="0900", example_channel=3))
    s.cmd("set_signal_generator", "Start, stop or toggle the signal generator (SIG_GEN)", {"channel": CH, "state": "{ type: enum, values: [START, STOP, TOGGLE], required: true }"},
          "SET {channel:02d} SIG_GEN {state}", set_expect("SIG_GEN"), "ack")
    s.vec("set_signal_generator", {"channel": 3, "state": "STOP"}, "< SET 03 SIG_GEN STOP >", "< REP 03 SIG_GEN STOP >", ACK)
    peq(s)
    return s


for spec in (p300(), imx(), ani(), mxa(), mxn5()):
    (ROOT / "specs" / f"{spec.sid}.yaml").write_bytes(emit(spec).encode())
    vec = [f"S = {q(spec.sid)}", "# Generated with the spec from Shure's command-string documents (see the spec's sources)."] + spec.vectors
    vec.append(f"text(S, \"get_mute\", {{'channel': 2}}, \"< GET {spec.chv(2)} AUDIO_MUTE >\", device_reply=\"< REP ERR >\", expect_result={ERR})")
    name_rep = f"< REP {spec.chv(1)} CHAN_NAME {{Podium     }} >" if "get_name" in spec.commands else ""
    name_state = ", 'name': 'Podium'" if "get_name" in spec.commands else ""
    vec.append(f"telemetry(S, \"mute-gain-name\", inbound=\"< REP {spec.chv(1)} AUDIO_MUTE ON >< REP {spec.chv(1)} AUDIO_GAIN_HI_RES 1040 >{name_rep}< REP PRESET 03 >\", expect_state={{'channels': {{'1': {{'mute': True, 'gain': -6.0{name_state}}}}}, 'device': {{'preset': 3}}}})")
    (ROOT / "tools" / "vectors" / f"{spec.sid}.py").write_bytes(("\n".join(vec) + "\n").encode())
    print(spec.sid, len(spec.commands), "commands")
