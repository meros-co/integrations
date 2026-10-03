# Brompton Tessera (brompton-tessera): one vector per command over the IP
# Control API (API 3.5.2 section 4). A write is PUT /api/<path> with the body
# {"data": value}; a read is GET /api/<path>. The commands mirror the
# reference's endpoint tree one for one, so, as for ProPresenter, the command
# list and each command's path come from the spec, and the expected request is
# worked out here: example values put into the path with RFC 3986 encoding and
# the body written with json.dumps (floats with the reference's decimal places,
# taken from the spec's .Nf directive). The commands the reference gives an
# example for, and the replies, are written by hand below from its examples.
from urllib.parse import quote as _bt_quote
import re as _bt_re

BT = "brompton-tessera"
_bt_doc = yaml.safe_load((ROOT / "specs" / f"{BT}.yaml").read_text(encoding="utf-8"))
_BT_HAND = {"set_output_brightness", "set_blackout_enabled", "set_lut_3d_strength", "set_genlock_source",
            "set_input_source", "recall_preset", "set_test_pattern_type", "set_test_pattern_frame_store",
            "reboot", "shutdown", "request_failover", "set_hidden_markers_frames_enabled_on",
            "set_frame_store_frame_name", "get_software_version", "get_override", "get_all",
            "set_curves_red_points", "get_active_preset_name"}
_BT_DYN = {"group": 3, "port": 1, "loop": 2, "frame": 2, "preset": 5, "serial": "AB12345",
           "panel_type": "BP2 V2"}


def _bt_example(name, p):
    if name in _BT_DYN and p.get("type") in ("int", "string"):
        return _BT_DYN[name]
    t = p["type"]
    if t == "bool":
        return True
    if t == "int":
        return p.get("max", p.get("min", 1))
    if t == "float":
        return p["max"]
    if t == "enum":
        return p["values"][-1]
    if t == "string":
        return "Main wall"
    if t == "json":
        return [2, 4]
    raise ValueError(t)


def _bt_vectors():
    for name, command in _bt_doc["commands"].items():
        if name in _BT_HAND:
            continue
        params = command.get("params") or {}
        values = {k: _bt_example(k, p) for k, p in params.items()}
        send = command["send"]
        target = send["path"]
        for k, v in values.items():
            target = target.replace("{" + k + "}", _bt_quote(str(v), safe=""))
        request = {"method": send["method"], "target": target}
        if send["method"] == "PUT":
            body = send["body"]
            m = _bt_re.search(r'\{"data":(.*)\}$', body)
            inner = m.group(1)
            value_names = [k for k in params if "{" + k in inner]
            if not value_names:
                request["body"] = body.replace("{settings.processor_password:json}", '""')
            else:
                k = value_names[0]
                v = values[k]
                dec = _bt_re.search(r":\.(\d)f\}", inner)
                if dec:
                    text = f"{v:.{int(dec.group(1))}f}"
                else:
                    text = json.dumps(v, separators=(",", ":"))
                request["body"] = '{"data":' + text + "}"
        V.append({"spec": BT, "command": name, "input": values, "expect_request": request})


_bt_vectors()


def _bt(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": BT, "command": command, "input": input, "expect_request": request, **extra})


_BT_ACK = {"ok": {"kind": "ack"}}
_BT_ERR = {"error": {"error": "device_error"}}

# API 3.5.2 section 4, "Writing data": PUT /api/output/global-colour/brightness
# {"data": 5000}, answered {"brightness": 5000}.
_bt("set_output_brightness", {"brightness": 5000}, "PUT", "/api/output/global-colour/brightness", '{"data":5000}',
    http_reply={"status": 200, "body": '{"brightness":5000}'}, expect_result=_BT_ACK)
# Appendix A, "Bool(ean)": {"data": true} to override/blackout/enabled.
_bt("set_blackout_enabled", {"enabled": True}, "PUT", "/api/override/blackout/enabled", '{"data":true}',
    http_reply={"status": 200, "body": '{"enabled":true}'}, expect_result=_BT_ACK)
# Appendix A, "Float": 33.3 to processing/3d-lut/strength (one decimal place).
_bt("set_lut_3d_strength", {"strength": 33.3}, "PUT", "/api/processing/3d-lut/strength", '{"data":33.3}')
# Appendix A, "Enum": "sdi" to output/network/genlock/source.
_bt("set_genlock_source", {"source": "sdi"}, "PUT", "/api/output/network/genlock/source", '{"data":"sdi"}')
# Section 4, "With GET request": ?set=1&port-type=sdi&port-number=1.
_bt("set_input_source", {"port_type": "sdi", "port": 1}, "GET",
    "/api/input/active/source?set=1&port-type=sdi&port-number=1")
# A failure answers with a response-code (section 4).
_bt("recall_preset", {"number": 128}, "PUT", "/api/presets/active/number", '{"data":128}',
    http_reply={"status": 200, "body": '{"response-code":"Object not found"}'}, expect_result=_BT_ERR)
_bt("set_test_pattern_type", {"type": "colour-bars"}, "PUT", "/api/override/test-pattern/type", '{"data":"colour-bars"}')
# Appendix A, "TestPatternType": a frame store user number is an integer.
_bt("set_test_pattern_frame_store", {"frame": 7}, "PUT", "/api/override/test-pattern/type", '{"data":7}')
# system/actions: the password in the body, or a blank string.
_bt("reboot", {}, "PUT", "/api/system/actions/reboot", '{"data":""}')
_bt("shutdown", {}, "PUT", "/api/system/actions/shutdown", '{"data":"s3cret"}', settings={"processor_password": "s3cret"})
_bt("request_failover", {}, "PUT", "/api/output/network/failover/actions/request-failover", '{"data":""}')
# Section 4, "Passing an array of values into JSON": frames 2 and 4.
_bt("set_hidden_markers_frames_enabled_on", {"frames_enabled_on": [2, 4]}, "PUT",
    "/api/output/network/hidden-markers/frames-enabled-on", '{"data":[2,4]}')
# Appendix A, "String": "Holding Card" as frame 1's name.
_bt("set_frame_store_frame_name", {"frame": 1, "name": "Holding Card"}, "PUT",
    "/api/override/test-pattern/frame-store/frames/1/name", '{"data":"Holding Card"}')
_bt("set_curves_red_points", {"points": [{"x": 0.2, "y": 0.2}, {"x": 0.64, "y": 0.77}]}, "PUT",
    "/api/processing/curves/red/points", '{"data":[{"x":0.2,"y":0.2},{"x":0.64,"y":0.77}]}')
_bt("get_software_version", {}, "GET", "/api/system/software-version",
    http_reply={"status": 200, "body": '{"software-version":"3.5.2"}'},
    expect_result={"ok": {"kind": "value", "value": "3.5.2"}})
_bt("get_active_preset_name", {}, "GET", "/api/presets/active/name",
    http_reply={"status": 200, "body": '{"name":"Show"}'}, expect_result={"ok": {"kind": "value", "value": "Show"}})
_bt("get_override", {}, "GET", "/api/override",
    http_reply={"status": 200, "body": '{"override":{"blackout":{"enabled":false,"fade-time":1.5},"freeze":{"enabled":true}}}'},
    expect_result={"ok": {"kind": "value", "value": {"blackout": {"enabled": False, "fade-time": 1.5},
                                                     "freeze": {"enabled": True}}}})
_bt("get_all", {}, "GET", "/api/")

# Telemetry: the whole-tree poll (keys as the Companion module reads them,
# under "api"), and single-endpoint replies.
telemetry(BT, "tree", inbound_http={"path": "/api/", "body": json.dumps({"api": {
    "override": {"blackout": {"enabled": True, "fade-time": 2.0}, "freeze": {"enabled": False},
                 "test-pattern": {"enabled": False, "type": "smpte-bars", "format": "from-input"}},
    "presets": {"active": {"name": "Show", "number": 3}},
    "output": {"global-colour": {"brightness": 1500, "colour-temperature": 6504,
                                 "gains": {"red": 100.0, "green": 98.5, "blue": 100.0, "intensity": 100.0}},
               "network": {"failover": {"state": {"is-active": True, "is-partner-present": False}},
                           "cable-redundancy": {"loops": {"1": {"state": "loop-found: A1->B1"}}}}},
    "input": {"active": {"source": {"port-type": "hdmi", "port-number": 1}},
              "ports": {"hdmi": {"1": {"meta-data": {"refresh-rate": 50.0,
                                                     "resolution": {"width": 3840, "height": 2160}}}}}},
    "groups": {"items": {"2": {"name": "Upstage", "brightness": 900}}},
    "devices": {"statistics": {"online-count": 240, "error-count": 1}},
    "system": {"software-version": "3.5.2", "processor-type": "sx40", "temperature": {"gpu": 51.5}},
}})}, expect_state={
    "override": {"blackout": {"enabled": True, "fade_time": 2.0}, "freeze": {"enabled": False},
                 "test_pattern": {"enabled": False, "type": "smpte-bars", "format": "from-input"}},
    "presets": {"active": {"name": "Show", "number": 3}},
    "output": {"global_colour": {"brightness": 1500, "colour_temperature": 6504,
                                 "gains": {"red": 100.0, "green": 98.5, "blue": 100.0, "intensity": 100.0}},
               "network": {"failover": {"state": {"is_active": True, "is_partner_present": False}},
                           "cable_redundancy": {"loops": {"1": {"state": "loop-found: A1->B1"}}}}},
    "input": {"active": {"source": {"port_type": "hdmi", "port_number": 1}},
              "ports": {"hdmi": {"1": {"meta_data": {"refresh_rate": 50.0,
                                                     "resolution": {"width": 3840, "height": 2160}}}}}},
    "groups": {"2": {"name": "Upstage", "brightness": 900}},
    "devices": {"statistics": {"online_count": 240, "error_count": 1}},
    "system": {"software_version": "3.5.2", "processor_type": "sx40", "temperature": {"gpu": 51.5}},
})
# The reply to a write of one group's brightness: any group number.
telemetry(BT, "group-brightness-reply", inbound_http={"path": "/api/groups/items/12/brightness",
                                                      "body": '{"brightness":4000}'},
          expect_state={"groups": {"12": {"brightness": 4000}}})
telemetry(BT, "blackout-reply", inbound_http={"path": "/api/override/blackout/enabled", "body": '{"enabled":false}'},
          expect_state={"override": {"blackout": {"enabled": False}}})
