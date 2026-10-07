LW = "lightware-lw3"
# Lightware LW3 (MX2 v1.11.3, MMX2 v2.16, Taurus UCX v2.22, UMX-HDMI-140
# v1.3.1 user manuals, LW3 Programmers' Reference chapters). Commands end with
# CR LF; replies are "<prefix> <path>..." lines, changes "CHG <path>=<value>".
#
# Every command first gets a vector rendered here from example values with a
# small renderer of its own (bool as true/false, .Nf floats, plain values),
# as the ProPresenter vectors are; the hand-stated vectors after them, with
# device replies from the manuals' examples, replace those of the same name.
import re as _re

_LW_DOC = yaml.safe_load((ROOT / "specs" / f"{LW}.yaml").read_text(encoding="utf-8"))
_LW_STRINGS = {
    "^([0-9A-Fa-f]{2})+$": "0400",
    "^[ -~]*$": "Room 1",
    "^[A-Za-z0-9_-]+$": "Preset_1",
    "^[^\\\\{}#%()]*$": "Lectern",
    "^(U[1-4]|H1)$": "U2",
    "^[IO][1-9][0-9]?$": "I2",
    "^[ED][FPLS]?$": "E",
    "^[FUD][1-9][0-9]{0,2}$": "F47",
    "^((I[1-9][0-9]?|0)?;){0,47}(I[1-9][0-9]?|0)?$": "I1;I2;0;I3",
    "^/[A-Za-z0-9_/@-]*\\.[A-Za-z0-9_]+$": "/V1/MEDIA/VIDEO/I1.SignalPresent",
    "^/[A-Za-z0-9_/@-]*:[A-Za-z0-9_]+$": "/V1/MEDIA/VIDEO/XP:switch",
    "^/[A-Za-z0-9_/@-]*[.:][A-Za-z0-9_]+$": "/MEDIA/PORTS/VIDEO/I1/SETTINGS.EnablePower",
    "^/[A-Za-z0-9_/@-]*(/\\*)?$": "/V1/MEDIA/VIDEO/*",
}


def _lw_example(p):
    t = p["type"]
    if t == "enum":
        return p["values"][0]
    if t == "bool":
        return True
    if t == "string":
        return _LW_STRINGS[p["pattern"]]
    if t == "float":
        return float(p.get("min", 0) if p.get("min", 0) > -20 else -20.0)
    lo = p.get("min", 1)
    hi = p.get("max")
    return 2 if (lo <= 2 and (hi is None or hi >= 2)) else lo


def _lw_render(template, values):
    def sub(m):
        name, _, directive = m.group(1).partition(":")
        v = values[name]
        if directive.startswith(".") and directive.endswith("f"):
            return f"{v:.{int(directive[1:-1])}f}"
        if isinstance(v, bool):
            return "true" if v else "false"
        return str(v)
    return _re.sub(r"\{([A-Za-z0-9_.]+(?::[^{}]*)?)\}", sub, template) + "\r\n"


for _name, _cmd in _LW_DOC["commands"].items():
    _values = {k: _lw_example(p) for k, p in (_cmd.get("params") or {}).items()}
    _send = _cmd["send"]
    _model = next(m["id"] for m in _LW_DOC["models"] if _name in m["supports"])
    _extra = {}
    if "{settings.password}" in str(_send):
        _values_s = {**_values, "settings.password": "Secret1"}
        _extra["settings"] = {"password": "Secret1"}
    else:
        _values_s = _values
    if isinstance(_send, list):
        _wire = [_lw_render(s, _values_s) for s in _send]
    else:
        _wire = _lw_render(_send, _values_s)
    text(LW, _name, _values, _wire, model=_model, **_extra)

ACK = {"ok": {"kind": "ack"}}
ERR = {"error": {"error": "device_error"}}


def _v(x):
    return {"ok": {"kind": "value", "value": x}}


V1 = {"model": "mmx2"}
UCX = {"model": "taurus-ucx"}
MX = {"model": "mx2"}
UMX = {"model": "umx"}

# Shared (MX2 9.4, MMX2 7.5)
text(LW, "get_serial", {}, "GET /.SerialNumber\r\n", device_reply="pr /.SerialNumber=87654321\r\n",
     expect_result=_v("87654321"), **V1)
text(LW, "get_product_name", {}, "GET /.ProductName\r\n", device_reply="pr /.ProductName=MX2-8x8-HDMI20\r\n",
     expect_result=_v("MX2-8x8-HDMI20"), **MX)

# /V1 tree (MMX2 7.5-7.9, UCX 8.5-8.11)
text(LW, "get_firmware_version", {}, "GET /V1/MANAGEMENT/UID/PACKAGE.Version\r\n",
     device_reply="pr /V1/MANAGEMENT/UID/PACKAGE.Version=2.16.0b6\r\n", expect_result=_v("2.16.0b6"), **V1)
text(LW, "set_device_label", {"label": "Room 1"}, "SET /V1/MANAGEMENT/LABEL.DeviceLabel=Room 1\r\n",
     device_reply="pw /V1/MANAGEMENT/LABEL.DeviceLabel=Room 1\r\n", expect_result=ACK, **V1)
text(LW, "switch", {"layer": "VIDEO", "input": 2, "output": 1}, "CALL /V1/MEDIA/VIDEO/XP:switch(I2:O1)\r\n",
     device_reply="mO /V1/MEDIA/VIDEO/XP:switch=\r\n", expect_result=ACK, **V1)
text(LW, "switch", {"layer": "VIDEO", "input": 5, "output": 1}, "CALL /V1/MEDIA/VIDEO/XP:switch(I5:O1)\r\n",
     device_reply="mE /V1/MEDIA/VIDEO/XP:switch %E006:Illegal operation\r\n", expect_result=ERR, **UCX)
text(LW, "disconnect", {"layer": "AUDIO", "output": 2}, "CALL /V1/MEDIA/AUDIO/XP:switch(0:O2)\r\n",
     device_reply="mO /V1/MEDIA/AUDIO/XP:switch=\r\n", expect_result=ACK, **V1)
text(LW, "set_port_lock", {"layer": "VIDEO", "port": "O1", "locked": True},
     "SET /V1/MEDIA/VIDEO/XP/O1.Lock=true\r\n", device_reply="pw /V1/MEDIA/VIDEO/XP/O1.Lock=true\r\n",
     expect_result=ACK, **V1)
text(LW, "get_connected_source", {"layer": "VIDEO", "output": 1}, "GET /V1/MEDIA/VIDEO/XP/O1.ConnectedSource\r\n",
     device_reply="pw /V1/MEDIA/VIDEO/XP/O1.ConnectedSource=I3\r\n", expect_result=_v("I3"), **V1)
text(LW, "get_signal_present", {"layer": "VIDEO", "port": "I2"}, "GET /V1/MEDIA/VIDEO/I2.SignalPresent\r\n",
     device_reply="nE /V1/MEDIA/VIDEO/I9 %E002:Not exist\r\n", expect_result=ERR, **V1)

# MX2 tree (MX2 9.5, 9.6, 9.11)
text(LW, "mx2_switch", {"input": 3, "output": 2}, "CALL /MEDIA/XP/VIDEO:switch(I3:O2)\r\n",
     device_reply="mO /MEDIA/XP/VIDEO:switch\r\n", expect_result=ACK, **MX)
text(LW, "mx2_mute_output", {"output": 2}, "CALL /MEDIA/XP/VIDEO:muteDestination(O2)\r\n",
     device_reply="mO /MEDIA/XP/VIDEO:muteDestination\r\n", expect_result=ACK, **MX)
text(LW, "mx2_get_connections", {}, "GET /MEDIA/XP/VIDEO.DestinationConnectionStatus\r\n",
     device_reply="pr /MEDIA/XP/VIDEO.DestinationConnectionStatus=I1;I1;0;I4\r\n",
     expect_result=_v("I1;I1;0;I4"), **MX)

# Subscriptions and change notifications (MX2 9.3.9-9.3.10, MMX2 7.4.8-7.4.10)
telemetry(LW, "v1-connected-source", inbound="CHG /V1/MEDIA/VIDEO/XP/O1.ConnectedSource=I3\r\n",
          expect_state={"video": {"outputs": {"1": {"input": 3}}}})
telemetry(LW, "v1-disconnected", inbound="pw /V1/MEDIA/AUDIO/XP/O2.ConnectedSource=0\r\n",
          expect_state={"audio": {"outputs": {"2": {"input": 0}}}})
telemetry(LW, "v1-lock", inbound="CHG /V1/MEDIA/VIDEO/XP/O1.Lock=true\r\n",
          expect_state={"video": {"outputs": {"1": {"locked": True}}}})
telemetry(LW, "v1-signal", inbound="CHG /V1/MEDIA/VIDEO/I2.SignalPresent=false\r\n",
          expect_state={"video": {"inputs": {"2": {"signal": False}}}})
telemetry(LW, "v1-usb", inbound="CHG /V1/MEDIA/USB/XP/H1.ConnectedSource=U2\r\n",
          expect_state={"usb": {"hubs": {"1": {"host": 2}}}})
telemetry(LW, "label", inbound="pr /V1/MANAGEMENT/LABEL.DeviceLabel=Room 1\r\n",
          expect_state={"device": {"label": "Room 1"}})
telemetry(LW, "mx2-connections", inbound="CHG /MEDIA/XP/VIDEO.DestinationConnectionStatus=I1;I1;0;I4\r\n",
          expect_state={"video": {"outputs": {"1": {"input": 1}, "2": {"input": 1}, "3": {"input": 0},
                                              "4": {"input": 4}}}})
telemetry(LW, "mx2-port-status", inbound="pr /MEDIA/XP/VIDEO.SourcePortStatus=TFF;MAA\r\n",
          expect_state={"video": {"inputs": {
              "1": {"locked": False, "muted": False, "embedded_audio": True, "hdcp": True, "signal": True,
                    "connected": True},
              "2": {"locked": False, "muted": True, "embedded_audio": False, "hdcp": False, "signal": False,
                    "connected": False}}}})
telemetry(LW, "mx2-power", inbound="CHG /MANAGEMENT/POWER.Operation=STANDBY\r\n",
          expect_state={"device": {"power_mode": "STANDBY"}})
telemetry(LW, "error-not-state", inbound="pE /V1/MEDIA/VIDEO/I9.SignalPresent %E002:Not exist\r\n",
          expect_state={})

# Any node, property or method by its path (MX2 9.3.4, 9.3.9; answers from
# the manual's examples).
text(LW, "get_property", {"path": "/.SerialNumber"}, "GET /.SerialNumber\r\n",
     device_reply="pr /.SerialNumber=87654321\r\n", expect_result=_v("87654321"), **MX)
text(LW, "set_property", {"path": "/MEDIA/PORTS/VIDEO/I1/SETTINGS.Conversion", "value": "OFF"},
     "SET /MEDIA/PORTS/VIDEO/I1/SETTINGS.Conversion=OFF\r\n",
     device_reply="pw /MEDIA/PORTS/VIDEO/I1/SETTINGS.Conversion=OFF\r\n", expect_result=ACK, **MX)
text(LW, "call_method", {"method": "/MEDIA/XP/VIDEO:switch", "arguments": "I1:O1"},
     "CALL /MEDIA/XP/VIDEO:switch(I1:O1)\r\n", device_reply="mO /MEDIA/XP/VIDEO:switch\r\n",
     expect_result=_v(None), **MX)
text(LW, "call_method", {"method": "/MEDIA/XP/VIDEO:switch", "arguments": "IA:O1"},
     "CALL /MEDIA/XP/VIDEO:switch(IA:O1)\r\n", device_reply="mE /MEDIA/XP/VIDEO:switch %E004:Invalid value\r\n",
     expect_result=ERR, file="call_method-error", **MX)
text(LW, "get_manual", {"path": "/MEDIA/PORTS/VIDEO/I1/SETTINGS.EnablePower"},
     "MAN /MEDIA/PORTS/VIDEO/I1/SETTINGS.EnablePower\r\n",
     device_reply="pm /MEDIA/PORTS/VIDEO/I1/SETTINGS.EnablePower [true|false] Enables or disables 3v3 powering on DP_PWR pin\r\n",
     expect_result=_v("[true|false] Enables or disables 3v3 powering on DP_PWR pin"), **MX)
text(LW, "open_node", {"node": "/MEDIA/VIDEO/*"}, "OPEN /MEDIA/VIDEO/*\r\n",
     device_reply="o- /MEDIA/VIDEO/*\r\n", expect_result=ACK, **MX)
text(LW, "close_node", {"node": "/MEDIA/VIDEO"}, "CLOSE /MEDIA/VIDEO\r\n",
     device_reply="c- /MEDIA/VIDEO\r\n", expect_result=ACK, **MX)
telemetry(LW, "any-property", inbound="CHG /MEDIA/AUDIO/O3.VolumePercent=50.00\r\n", expect_state={})
telemetry(LW, "any-property-root", inbound="pr /.SerialNumber=87654321\r\n",
          expect_state={"device": {"serial": "87654321"}})


def _lw_nodes():
    """Every property line (GET and SET answers, CHG) is also kept under
    nodes, by node path and property name (the generic rule)."""
    for v in V:
        if v.get("spec") != LW or "inbound" not in v:
            continue
        m = _re.match(r"^(?:CHG|p[rw]) (/[^=.\s]*)\.([A-Za-z0-9_]+)=(.*)$", v["inbound"].rstrip("\r\n"))
        if m:
            v["expect_state"] = {**v["expect_state"], "nodes": {m.group(1): {m.group(2): m.group(3)}}}


_lw_nodes()
