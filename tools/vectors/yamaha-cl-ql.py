YCLQL = "yamaha-cl-ql"
# Yamaha CL/QL over RCP (TCP 49280, LF-terminated). Wire formats from Yamaha's QLab Setup Guide
# for CL/QL/TF and its Python Script Template V1.00 (command_list.pdf, command.py, recall.py);
# replies from the RCP grammar of Yamaha's DME7 specification (see the spec's quirks).


def _yclql_vectors():
    S = YCLQL
    # (command base, RCP address, X (param, max) or None, Y (param, max) or None,
    #  value kind, state path or None, set-only), transcribed from the document.
    rows = [
        ('input_fader_level', 'MIXER:Current/InCh/Fader/Level', ('channel', 72), None, ('level', 1000), 'inputs.{x}.fader_level', False),
        ('input_on', 'MIXER:Current/InCh/Fader/On', ('channel', 72), None, ('bool',), 'inputs.{x}.on', False),
        ('input_stereo_pan', 'MIXER:Current/InCh/ToSt/Pan', ('channel', 72), None, ('pan',), 'inputs.{x}.stereo_pan', False),
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

    done = set(_yclql_EXPLICIT)
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
    for args, kw in _yclql_EXPLICIT.values():
        if args[0] == "text":
            text(S, *args[1:], **kw)
        else:
            telemetry(S, *args[1:], **kw)


_yclql_EXPLICIT = {}


def _yclql_explicit(kind, *args, **kw):
    key = args[0] if kind == "text" else "telemetry-" + args[0]
    _yclql_EXPLICIT[key] = ((kind,) + args, kw)


# Yamaha's own examples (QLab guide p.12-15, recall.py, command_list.pdf).
_yclql_explicit("text", "set_input_on", {"channel": 1, "enabled": True}, "set MIXER:Current/InCh/Fader/On 0 0 1\n",
                device_reply="OK set MIXER:Current/InCh/Fader/On 0 0 1 \"ON\"\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "set_input_stereo_pan", {"channel": 1, "pan": -63}, "set MIXER:Current/InCh/ToSt/Pan 0 0 -63\n",
                device_reply="OK set MIXER:Current/InCh/ToSt/Pan 0 0 -63 \"L63\"\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "set_input_fader_level", {"channel": 2, "level": -1000}, "set MIXER:Current/InCh/Fader/Level 1 0 -1000\n",
                device_reply="ERROR set InvalidArgument\n", expect_result={"error": {"error": "device_error"}})
_yclql_explicit("text", "recall_scene", {"scene": 1}, "ssrecall_ex MIXER:Lib/Scene 1\n",
                device_reply="OK ssrecall_ex MIXER:Lib/Scene 1\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "get_current_scene", {}, "sscurrent_ex MIXER:Lib/Scene\n",
                device_reply="OK sscurrent_ex MIXER:Lib/Scene 12 modified\n", expect_result={"ok": {"kind": "value", "value": "12"}})
_yclql_explicit("telemetry", "scene-current", inbound="NOTIFY sscurrent_ex MIXER:Lib/Scene 12\n", expect_state={"scene": {"current": 12}})
_yclql_explicit("telemetry", "scene-modified", inbound="OK sscurrent_ex MIXER:Lib/Scene 12 modified\n",
                expect_state={"scene": {"current": 12, "modified": True}})
_yclql_explicit("telemetry", "scene-recalled", inbound="OK ssrecall_ex MIXER:Lib/Scene 7\n", expect_state={"scene": {"current": 7}})

# Generic and device commands (DME7 spec grammar; console replies corroborated, see quirks).
_yclql_explicit("text", "set_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 0, "y": 0, "value": -1000},
     "set MIXER:Current/InCh/Fader/Level 0 0 -1000\n", device_reply='OKm set MIXER:Current/InCh/Fader/Level 0 0 -1000 "-10.00"\n',
     expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("text", "set_parameter_text", {"address": "MIXER:Current/InCh/Label/Name", "x": 4, "y": 0, "value": "Kick In"},
     'set MIXER:Current/InCh/Label/Name 4 0 "Kick In"\n', device_reply="ERROR set UnknownAddress\n",
     expect_result={"error": {"error": "device_error"}})
_yclql_explicit("text", "get_parameter", {"address": "MIXER:Current/InCh/Fader/Level", "x": 2, "y": 0}, "get MIXER:Current/InCh/Fader/Level 2 0\n",
     device_reply="OK get MIXER:Current/InCh/Fader/Level 2 0 -32768\n", expect_result={"ok": {"kind": "value", "value": "-32768"}})
_yclql_explicit("text", "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "CL5"\n',
     expect_result={"ok": {"kind": "value", "value": "CL5"}})
_yclql_explicit("text", "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "FOH"\n',
     expect_result={"ok": {"kind": "value", "value": "FOH"}})
_yclql_explicit("text", "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
_yclql_explicit("text", "set_keepalive", {"interval_ms": 10000}, "scpmode keepalive 10000\n",
     device_reply="OK scpmode keepalive 10000\n", expect_result={"ok": {"kind": "ack"}})
_yclql_explicit("telemetry", "product-name", expect_connect_wire=["devinfo productname\n"],
     inbound='OK devinfo productname "CL5"\n', expect_state={"device": {"product_name": "CL5"}})
_yclql_explicit("telemetry", "device-name", inbound='OK devinfo devicename "FOH"\n', expect_state={"device": {"name": "FOH"}})
_yclql_explicit("telemetry", "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
_yclql_explicit("telemetry", "error-reply-is-not-state", inbound="ERROR get InvalidArgument\n", expect_state={})
_yclql_explicit("text", "get_error_status", {}, "devstatus error\n", device_reply='OK devstatus error "none"\n',
     expect_result={"ok": {"kind": "value", "value": "none"}})
_yclql_explicit("telemetry", "error-status", inbound='NOTIFY devstatus error "wrn/Word Clock Error// x22 on (1) ID-001 2024/1/2 10:00:00"\n',
     expect_state={"device": {"error": "wrn/Word Clock Error// x22 on (1) ID-001 2024/1/2 10:00:00"}})


_yclql_vectors()
_yclql_EXPLICIT.clear()
