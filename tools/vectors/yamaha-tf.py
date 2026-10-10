YTF = "yamaha-tf"
# Yamaha TF over RCP (TCP 49280, LF-terminated). Wire formats from Yamaha's QLab Setup Guide for
# CL/QL/TF and its Python Script Template V1.00 (command_list.pdf, recall_a.py, TFxQLab/MultiCh.py,
# TFxQLab/recallb1.py); replies from the RCP grammar of Yamaha's DME7 specification.


def _ytf_vectors():
    S = YTF
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 40), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 40), None, ('bool',), 'inputs.{x}.on', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 40), None, ('pan',), 'inputs.{x}.stereo_pan', False),
        ('stereo_input_fader_level', 'MIXER:Current/StInCh/Fader/Level', ('stereo_input', 4), None, ('level', 1000), 'stereo_inputs.{x}.fader_level', False),
        ('stereo_input_on', 'MIXER:Current/StInCh/Fader/On', ('stereo_input', 4), None, ('bool',), 'stereo_inputs.{x}.on', False),
        ('stereo_input_stereo_pan', 'MIXER:Current/StInCh/ToSt/Pan', ('stereo_input', 4), None, ('pan',), 'stereo_inputs.{x}.stereo_pan', False),
        ('fx_return_fader_level', 'MIXER:Current/FxRtnCh/Fader/Level', ('fx_return', 4), None, ('level', 1000), 'fx_returns.{x}.fader_level', False),
        ('fx_return_on', 'MIXER:Current/FxRtnCh/Fader/On', ('fx_return', 4), None, ('bool',), 'fx_returns.{x}.on', False),
        ('fx_return_stereo_pan', 'MIXER:Current/FxRtnCh/ToSt/Pan', ('fx_return', 4), None, ('pan',), 'fx_returns.{x}.stereo_pan', False),
        ('dca_fader_level', 'MIXER:Current/DCA/Fader/Level', ('dca', 8), None, ('level', 1000), 'dcas.{x}.fader_level', False),
        ('dca_on', 'MIXER:Current/DCA/Fader/On', ('dca', 8), None, ('bool',), 'dcas.{x}.on', False),
        ('mix_fader_level', 'MIXER:Current/Mix/Fader/Level', ('mix', 20), None, ('level', 1000), 'mixes.{x}.fader_level', False),
        ('mix_on', 'MIXER:Current/Mix/Fader/On', ('mix', 20), None, ('bool',), 'mixes.{x}.on', False),
        ('mix_balance', 'MIXER:Current/Mix/Out/Balance', ('mix', 20), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'mixes.{x}.balance', False),
        ('matrix_fader_level', 'MIXER:Current/Mtrx/Fader/Level', ('matrix', 4), None, ('level', 1000), 'matrices.{x}.fader_level', False),
        ('matrix_on', 'MIXER:Current/Mtrx/Fader/On', ('matrix', 4), None, ('bool',), 'matrices.{x}.on', False),
        ('stereo_fader_level', 'MIXER:Current/St/Fader/Level', ('stereo', 2), None, ('level', 1000), 'stereo.{x}.fader_level', False),
        ('stereo_on', 'MIXER:Current/St/Fader/On', ('stereo', 2), None, ('bool',), 'stereo.{x}.on', False),
        ('stereo_balance', 'MIXER:Current/St/Out/Balance', ('stereo', 2), None, ('int', 'balance', -63, 63, '-63 (L63) to 63 (R63), 0 centre'), 'stereo.{x}.balance', False),
        ('mix_pan_link', 'MIXER:Current/Mix/PanLink', ('mix', 20), None, ('bool',), 'mixes.{x}.pan_link', False),
        ('mono_fader_level', 'MIXER:Current/Mono/Fader/Level', None, None, ('level', 1000), 'mono.fader_level', False),
        ('mono_on', 'MIXER:Current/Mono/Fader/On', None, None, ('bool',), 'mono.on', False),
        ('mute_group_on', 'MIXER:Current/MuteMaster/On', ('mute_group', 6), None, ('bool',), 'mute_groups.{x}.on', False),
    ]

    # Expected wire stated independently of the spec's templates: X and Y go
    # on the wire numbered from 0, a parameter without X or Y sends 0, strings
    # are double-quoted with \\ and \" escapes, every line ends with LF.
    def sample(kind):
        """(input value, wire text, state value)"""
        k = kind[0]
        if k == "level":
            return -1000, "-1000", -1000
        if k in ("bool", "flag"):
            return True, "1", True
        if k == "int":
            return kind[3], str(kind[3]), kind[3]
        if k == "pan":
            return -35, "-35", -35
        if k == "enum":
            v = kind[1][-1]
            return v, '"' + v + '"', v
        if k == "name":
            return 'Lead "V"', '"Lead \\"V\\""', None
        return "MIX1", '"MIX1"', "MIX1"

    def pname(kind):
        return {"level": "level", "bool": "enabled", "pan": "pan", "enum": "value", "name": "name",
                "text": "value"}.get(kind[0]) or kind[1]

    def nest(path, value):
        out = cur = {}
        parts = path.split(".")
        for p in parts[:-1]:
            cur[p] = {}
            cur = cur[p]
        cur[parts[-1]] = value
        return out

    done = set(_ytf_EXPLICIT)
    for base, addr, x, y, kind, state, set_only in rows:
        inp, xw, yw, xn, yn = {}, 0, 0, None, None
        if x:
            xn = min(3, x[1]); inp[x[0]] = xn; xw = xn - 1
        if y:
            yn = min(2, y[1]); inp[y[0]] = yn; yw = yn - 1
        val, wire, sval = sample(kind)
        if "set_" + base not in done:
            text(S, "set_" + base, dict(inp, **{pname(kind): val}), f"set {addr} {xw} {yw} {wire}\n",
                 device_reply=f"OK set {addr} {xw} {yw} {wire}\n", expect_result={"ok": {"kind": "ack"}})
        if not set_only and "get_" + base not in done:
            plain = wire[1:-1] if wire.startswith('"') else wire
            text(S, "get_" + base, inp, f"get {addr} {xw} {yw}\n",
                 device_reply=f"OK get {addr} {xw} {yw} {wire}\n",
                 expect_result={"ok": {"kind": "value", "value": plain}})
        if state:
            if kind[0] == "name":
                wire, sval = '"Vox 1"', "Vox 1"
            path = state.replace("{x}", str(xn)).replace("{y}", str(yn))
            tail = "" if wire.startswith('"') else f' "{wire}"'
            telemetry(S, base, inbound=f"NOTIFY set {addr} {xw} {yw} {wire}{tail}\n", expect_state=nest(path, sval))
    for args, kw in _ytf_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_ytf_EXPLICIT = {}


def _ytf_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _ytf_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (MultiCh.py, recalla0.py, recallb1.py, Readme_EN.pdf).
_ytf_explicit("text", "set_input_fader_level", {"channel": 5, "level": 500}, "set MIXER:Current/InCh/Fader/Level 4 0 500\n",
              device_reply="OK set MIXER:Current/InCh/Fader/Level 4 0 500 \"5.00\"\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "set_input_on", {"channel": 2, "enabled": False}, "set MIXER:Current/InCh/Fader/On 1 0 0\n",
              device_reply="OK set MIXER:Current/InCh/Fader/On 1 0 0\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "recall_scene", {"bank": "a", "scene": 0}, "ssrecall_ex scene_a 0\n",
              device_reply="OK ssrecall_ex scene_a 0\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "get_current_scene", {"bank": "b"}, "sscurrent_ex scene_b\n",
              device_reply="ERROR sscurrent_ex InvalidArgument\n", expect_result={"error": {"error": "device_error"}})
_ytf_explicit("telemetry", "scene-current", inbound="NOTIFY sscurrent_ex scene_b 1\n", expect_state={"scenes": {"b": {"current": 1}}})
_ytf_explicit("telemetry", "scene-modified", inbound="OK sscurrent_ex scene_a 3 unmodified\n",
              expect_state={"scenes": {"a": {"current": 3, "modified": False}}})
_ytf_explicit("telemetry", "scene-recalled", inbound="OK ssrecall_ex scene_a 5\n", expect_state={"scenes": {"a": {"current": 5}}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_ytf_explicit("text", "set_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/InCh/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/InCh/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_ytf_explicit("text", "get_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/InCh/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/InCh/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_ytf_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "TF5"\n',
     expect_result={"ok": {"kind": "value", "value": "TF5"}})
_ytf_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_ytf_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_ytf_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_ytf_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "TF5"\n', expect_state={"device": {"product_name": "TF5"}})
_ytf_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_ytf_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_ytf_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})


_ytf_vectors()
_ytf_EXPLICIT.clear()
