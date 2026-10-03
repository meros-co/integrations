# NovaStar COEX (novastar-coex): one vector per command over the HTTP API on
# port 8001. Most commands get a generated vector: example values put into the
# spec's path and body with an independent renderer written here (JSON strings
# with json.dumps, fixed decimals with Python's format, booleans as true/false
# or 1/0), so the core's template engine is checked against it. The requests
# the documents give an example for are written by hand below from those
# examples (COEX OpenAPI pages; COEX Series Interface API User Manual 2023), and
# replies use the documented {"code", "data", "message"} envelope.
import re as _nc_re

NC = "novastar-coex"
_nc_doc = yaml.safe_load((ROOT / "specs" / f"{NC}.yaml").read_text(encoding="utf-8"))
_NC_EX = {"screen": "{8C6F6B1D-0001}", "cabinets": [93138183199495], "canvases": [1, 2], "input": 1,
          "tables": [{"screenId": "{8C6F6B1D-0001}", "gammaTable": [0, 1, 2]}],
          "gamuts": [{"screenId": "{8C6F6B1D-0001}", "colorGamutInfo": {"colorTemperature": 6500}}],
          "data": [{"screenId": "{8C6F6B1D-0001}"}], "layout": {"screenID": "{8C6F6B1D-0001}", "canvases": []},
          "strategies": ["s1"], "name": "Show", "timezone": "Europe/London", "backup_file": "QUJD"}


def _nc_value(name, p):
    if name in _NC_EX and not (p.get("type") == "enum" and name == "name"):
        return _NC_EX[name]
    t = p["type"]
    if t == "bool":
        return p.get("default", True)
    if t == "int":
        return p.get("default", p.get("max", p.get("min", 1)))
    if t == "float":
        return p.get("max", 1.0)
    if t == "enum":
        return p["values"][0]
    if t == "string":
        return "Show"
    if t == "json":
        return [1]
    raise ValueError(t)


def _nc_render(template, values):
    def sub(m):
        name, directive = m.group(1), m.group(2) or ""
        v = values[name]
        if directive == ":json":
            return json.dumps(v)
        if directive == ":bool01":
            return "1" if v else "0"
        if directive.startswith(":.") and directive.endswith("f"):
            return f"{v:{directive[1:]}}"
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, (list, dict)):
            return json.dumps(v, separators=(",", ":"))
        return str(v)
    return _nc_re.sub(r"\{([a-z_]+)(:[^{}]+)?\}", sub, template)


_NC_HAND = {"set_brightness", "set_display_mode", "recall_preset", "set_cabinet_brightness_nits", "set_edid",
            "set_cabinet_color_temperature", "set_cabinet_gamma", "set_cabinet_gamut", "legacy_recall_preset",
            "set_working_mode", "legacy_set_test_pattern", "set_input_color_space", "get_snmp", "get_display_params",
            "set_display_mode_all"}
for _name, _cmd in _nc_doc["commands"].items():
    if _name in _NC_HAND:
        continue
    _params = _cmd.get("params") or {}
    _values = {k: _nc_value(k, p) for k, p in _params.items()}
    _send = _cmd["send"]
    _target = _send["path"]
    for _k, _v in _values.items():
        _target = _target.replace("{" + _k + "}", _nc_re.sub(r"[^A-Za-z0-9._~-]", lambda m: "%%%02X" % ord(m.group()), str(_v)))
    if "raw_query" in _send:
        _target += "?" + _send["raw_query"]
    _req = {"method": _send["method"], "target": _target}
    if "body" in _send:
        _req["body"] = _nc_render(_send["body"], _values)
    V.append({"spec": NC, "command": _name, "input": _values, "expect_request": _req})


def _nc(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
    V.append({"spec": NC, "command": command, "input": input, "expect_request": req, **extra})


_NC_OK = {"status": 200, "body": '{"code":0,"data":null,"message":"Success"}'}
_NC_ACK = {"ok": {"kind": "ack"}}
_S = "{8C6F6B1D-0001}"
# OpenAPI "Set Screen Brightness" example: brightness 0 for one screen.
_nc("set_brightness", {"screen": _S, "brightness": 0}, "PUT", "/api/v1/screen/brightness",
    '{"screenIdList":["' + _S + '"],"brightness":0.0000}', http_reply=_NC_OK, expect_result=_NC_ACK)
# A device-busy answer (error code 5) fails the command.
_nc("set_display_mode", {"screen": _S, "mode": 1}, "PUT", "/api/v1/screen/output/displaymode",
    '{"value":1,"screenIdList":["' + _S + '"]}',
    http_reply={"status": 200, "body": '{"code":5,"data":null,"message":"Busying"}'},
    expect_result={"error": {"error": "device_error"}})
_nc("set_display_mode_all", {"mode": 0}, "PUT", "/api/v1/screen/output/displaymode", '{"value":0,"screenIdList":[]}')
_nc("recall_preset", {"screen": _S, "preset": 2}, "POST", "/api/v1/preset/current/update",
    '{"sequenceNumber":2,"screenID":"' + _S + '"}', http_reply=_NC_OK, expect_result=_NC_ACK)
# 2023 manual examples.
_nc("set_cabinet_brightness_nits", {"cabinets": [93138183199495], "ratio": 1.0, "nits": 1000}, "PUT",
    "/api/v1/device/cabinet/brightness", '{"idList":[93138183199495],"ratio":1.0000,"nit":1000}')
_nc("set_cabinet_gamma", {"cabinets": [93138183199495], "channel": 3, "gamma": 2.8}, "PUT", "/api/v1/device/cabinet/gamma",
    '{"idList":[93138183199495],"type":3,"value":2.80}')
_nc("set_cabinet_color_temperature", {"cabinets": [93138183199495], "kelvin": 6500}, "PUT",
    "/api/v1/device/cabinet/colortemperature", '{"idList":[93138183199495],"value":6500}')
_nc("set_cabinet_gamut", {"name": "DCI-P3"}, "PUT", "/api/v1/device/correctionop/cabinets/gamut", '{"name":"DCI-P3"}')
_nc("set_edid", {"input": 1, "width": 3840, "height": 2160, "rate": 60}, "PUT", "/api/v1/device/input/1/edid",
    '{"para":{"resolution":{"width":3840,"height":2160},"refreshRate":60.00,"isCustom":false}}')
_nc("set_input_color_space", {"input": 1, "space": "0"}, "PUT", "/api/v1/device/input/1/colorspace", '{"colorSpace":0}')
_nc("legacy_recall_preset", {"preset": 1}, "PUT", "/api/v1/device/currentpreset", '{"sequenceNumber":1}')
_nc("set_working_mode", {"mode": 2}, "PUT", "/api/v1/device/hw/mode", '{"mode":2}')
# Pure white (the manual's example).
_nc("legacy_set_test_pattern", {"mode": 0, "grid_width": 1}, "PUT", "/api/v1/device/screen/controller/pattern/test",
    '{"mode":0,"parameters":{"red":4095,"green":4095,"blue":4095,"gray":4095,"gridWidth":1,"moveSpeed":50,'
    '"gradientStretch":8,"state":0}}')
_nc("get_snmp", {}, "GET", "/api/v1/device/snmpstate",
    http_reply={"status": 200, "body": '{"code":0,"data":{"state":true},"message":"Success"}'},
    expect_result={"ok": {"kind": "value", "value": True}})
_nc("get_display_params", {}, "GET", "/api/v1/screen/displayparams")

# Telemetry: the polled reads.
telemetry(NC, "display-params", inbound_http={"path": "/api/v1/screen/displayparams", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"list": [
        {"screenId": "{A}", "brightness": 0.8, "colorTemperature": 6500, "gamma": 2.8}]}})},
    expect_state={"screens": {"{A}": {"brightness": 0.8, "color_temperature": 6500, "gamma": 2.8}}})
telemetry(NC, "display-state", inbound_http={"path": "/api/v1/screen/output/display/state", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"mappingState": [{"canvasID": 1, "enable": False}],
                                                "displayState": [{"canvasID": 1, "displayMode": 2}]}})},
    expect_state={"canvases": {"1": {"display_mode": 2, "mapping": False}}})
telemetry(NC, "monitor", inbound_http={"path": "/api/v1/device/monitor/info?isNeedCabinetInfo=1", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"name": "MX40 Pro", "runtime": 3600, "backupStatus": 109,
                                                "mainBoardTemperature": {"value": 41.5, "status": 0}}})},
    expect_state={"device": {"runtime": 3600, "backup_status": 109, "mainboard_temperature": 41.5,
                             "mainboard_temperature_status": 0}})
telemetry(NC, "device", inbound_http={"path": "/api/v1/device/hw", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"name": "MX40 Pro", "sn": "ABC123", "swVersion": "V1.5.0", "mode": 2}})},
    expect_state={"device": {"name": "MX40 Pro", "serial": "ABC123", "software_version": "V1.5.0", "working_mode": 2}})
