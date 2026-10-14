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
    "^((25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])\\.){3}(25[0-5]|2[0-4][0-9]|1?[0-9]?[0-9])$": "192.168.0.20",
    "^([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}$": "AA:BB:CC:22:14:FF",
    "^/[^\\\\{}#%() ]*$": "/putxml",
    "^[0-9A-Fa-f]+$": "0000006D0022",
    "^[A-Za-z0-9 ._-]*$": "Welcome to Room 1",
    "^[A-Za-z0-9 _-]*$": "Projector on",
    "^[A-Za-z0-9.-]{1,253}$": "syslog.example.com",
    "^[A-Za-z0-9]([A-Za-z0-9-]{0,62}[A-Za-z0-9])?$": "room-1-ucx",
    "^[A-Za-z0-9][A-Za-z0-9.-]{0,63}$": "umx.room-1",
    "^[FUDE][1-9][0-9]{0,2}$": "D1",
    "^[^\\\\{}#%()\\s]+$": "Secret2",
    "^[^{}#%()]*$": "PWR ON\\x0D",
    "^\\d{4}-\\d\\d-\\d\\dT\\d\\d:\\d\\d:\\d\\d(\\.\\d{1,3})?Z$": "2020-11-01T04:39:54.000Z",
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
        if directive.startswith("map."):
            key = ("true" if v else "false") if isinstance(v, bool) else str(v)
            return _LW_DOC["maps"][directive[4:]][key]
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


# Full control: GPIO, serial, network, Event Manager, variables, message
# sending (answers from the manuals' examples: UCX 8.5-8.20, MMX2 7.9-7.15,
# MX2 9.16-9.19, UMX 7.4-7.23).
text(LW, "set_gpio_direction", {"port": 1, "direction": "Input"}, "SET /V1/MEDIA/GPIO/P1.Direction=Input\r\n",
     device_reply="pw /V1/MEDIA/GPIO/P1.Direction=Input\r\n", expect_result=ACK, **UCX)
text(LW, "gpio_interval", {"port": 1, "level": "Low", "seconds": 1}, "CALL /V1/MEDIA/GPIO/P1:interval(Low;1)\r\n",
     device_reply="mO /V1/MEDIA/GPIO/P1:interval=\r\n", expect_result=ACK, **UCX)
text(LW, "gpio_toggle", {"port": 1}, "CALL /V1/MEDIA/GPIO/P1:toggle()\r\n",
     device_reply="mO /V1/MEDIA/GPIO/P1:toggle\r\n", expect_result=ACK, **V1)
text(LW, "set_serial_baudrate", {"port": 1, "baud": "57600"}, "SET /V1/MEDIA/SERIAL/P1.Baudrate=57600\r\n",
     device_reply="pw /V1/MEDIA/SERIAL/P1.Baudrate=57600\r\n", expect_result=ACK, **UCX)
text(LW, "apply_network_settings", {}, "CALL /V1/MANAGEMENT/NETWORK:applySettings()\r\n",
     device_reply="mO /V1/MANAGEMENT/NETWORK:applySettings\r\n", expect_result=ACK, **UCX)
text(LW, "set_time", {"time": "2020-11-01T04:39:54.000Z"}, "CALL /V1/MANAGEMENT/DATETIME:setTime(2020-11-01T04:39:54.000Z)\r\n",
     device_reply="mO /V1/MANAGEMENT/DATETIME:setTime=\r\n", expect_result=ACK, **UCX)
text(LW, "get_edid_preferred_resolution", {"bank": "U", "slot": 2}, "GET /V1/EDID/U/U2.PreferredResolution\r\n",
     device_reply="pr /V1/EDID/U/U2.PreferredResolution=1920x1080p60.00Hz\r\n", expect_result=_v("1920x1080p60.00Hz"), **UCX)
text(LW, "mx2_apply_network_settings", {}, "CALL /MANAGEMENT/NETWORK:applySettings()\r\n",
     device_reply="mO /MANAGEMENT/NETWORK:applySettings=\r\n", expect_result=ACK, **MX)
text(LW, "mx2_set_serial_protocol", {"protocol": "LW3"}, "SET /MANAGEMENT/SERIAL.Protocol=LW3\r\n",
     device_reply="pw /MANAGEMENT/SERIAL.Protocol=LW3\r\n", expect_result=ACK, **MX)
text(LW, "umx_set_gpio_direction", {"port": 1, "direction": "I"}, "SET /MEDIA/GPIO/P1.Direction=I\r\n",
     device_reply="pw /MEDIA/GPIO/P1.Direction=I\r\n", expect_result=ACK, **UMX)
text(LW, "umx_set_serial_baudrate", {"baud": "57600"}, "SET /MEDIA/UART/P1.Baudrate=6\r\n",
     device_reply="pw /MEDIA/UART/P1.Baudrate=6\r\n", expect_result=ACK, **UMX)
text(LW, "umx_set_serial_stop_bits", {"stop_bits": "1.5"}, "SET /MEDIA/UART/P1.StopBits=1\r\n",
     device_reply="pw /MEDIA/UART/P1.StopBits=1\r\n", expect_result=ACK, **UMX)
text(LW, "umx_serial_send", {"message": "C00\\x0D"}, "CALL /MEDIA/UART/P1:sendMessage(C00\\x0D)\r\n",
     device_reply="mO /MEDIA/UART/P1:sendMessage\r\n", expect_result=ACK, **UMX)
text(LW, "umx_serial_send_hex", {"hex": "433030"}, "CALL /MEDIA/UART/P1:sendBinaryMessage(433030)\r\n",
     device_reply="mO /MEDIA/UART/P1:sendBinaryMessage\r\n", expect_result=ACK, **UMX)
text(LW, "umx_set_event_condition", {"event": 1, "condition": "/MEDIA/VIDEO/I1.SignalPresent=1"},
     "SET /EVENTS/E1.Condition=/MEDIA/VIDEO/I1.SignalPresent=1\r\n",
     device_reply="pw /EVENTS/E1.Condition=/MEDIA/VIDEO/I1.SignalPresent=1\r\n", expect_result=ACK, **UMX)
text(LW, "umx_set_event_action", {"event": 1, "action": "/MEDIA/VIDEO/XP.switch=I4:O1"},
     "SET /EVENTS/E1.Action=/MEDIA/VIDEO/XP.switch=I4:O1\r\n",
     device_reply="pw /EVENTS/E1.Action=/MEDIA/VIDEO/XP.switch=I4:O1\r\n", expect_result=ACK, **UMX)
text(LW, "umx_trigger_event_condition", {"event": 1}, "CALL /EVENTS/E1:triggerCondition(1)\r\n",
     device_reply="mO /EVENTS/E1:triggerCondition\r\n", expect_result=ACK, **UMX)
text(LW, "umx_add_to_variable", {"variable": 1, "operand": 5, "min": 0, "max": 100},
     "CALL /CTRL/VARS/V1:add(5;0;100)\r\n", device_reply="mO /CTRL/VARS/V1:add\r\n", expect_result=ACK, **UMX)
text(LW, "umx_tcp_message", {"address": "192.168.0.20", "port": 5555, "message": "C00\\x0a\\x0d"},
     "CALL /MEDIA/ETHERNET:tcpMessage(192.168.0.20:5555=C00\\x0a\\x0d)\r\n",
     device_reply="mO /MEDIA/ETHERNET:tcpMessage\r\n", expect_result=ACK, **UMX)
text(LW, "umx_wake_on_lan", {"mac": "AA:BB:CC:22:14:FF"}, "CALL /MEDIA/ETHERNET:wakeOnLan(AA:BB:CC:22:14:FF)\r\n",
     device_reply="mO /MEDIA/ETHERNET:wakeOnLan\r\n", expect_result=ACK, **UMX)
text(LW, "umx_apply_network_settings", {}, "CALL /MANAGEMENT/NETWORK:applySettings(1)\r\n",
     device_reply="mO /MANAGEMENT/NETWORK:applySettings\r\n", expect_result=ACK, **UMX)
text(LW, "umx_edid_switch", {"source": "F49", "input": 1}, "CALL /EDID:switch(F49:E1)\r\n",
     device_reply="mO /EDID:switch\r\n", expect_result=ACK, **UMX)

telemetry(LW, "v1-gpio", inbound="CHG /V1/MEDIA/GPIO/P1.Output=High\r\n",
          expect_state={"gpio": {"1": {"output": "High"}}})
telemetry(LW, "umx-gpio", inbound="pw /MEDIA/GPIO/P2.Direction=O\r\n",
          expect_state={"gpio": {"2": {"direction": "Output"}}})
telemetry(LW, "v1-ocs", inbound="pr /V1/MEDIA/OCS/P1.State=Occupied\r\n", expect_state={"ocs": {"1": {"state": "Occupied"}}})
telemetry(LW, "v1-serial", inbound="pw /V1/MEDIA/SERIAL/P2.Parity=Even\r\n", expect_state={"serial": {"2": {"parity": "Even"}}})
telemetry(LW, "umx-uart-baud", inbound="pw /MEDIA/UART/P1.Baudrate=7\r\n", expect_state={"serial": {"1": {"baudrate": 115200}}})
telemetry(LW, "umx-uart-mode", inbound="CHG /MEDIA/UART/P1.Rs232Mode=2\r\n",
          expect_state={"serial": {"1": {"mode": "command_injection"}}})
telemetry(LW, "umx-recognizer", inbound="pr /MEDIA/UART/RECOGNIZER.Rx=Login:\r\n",
          expect_state={"serial": {"1": {"received": "Login:"}}})
telemetry(LW, "mx2-serial", inbound="pw /MANAGEMENT/SERIAL.Protocol=LW2\r\n", expect_state={"serial": {"1": {"protocol": "LW2"}}})
telemetry(LW, "network-dhcp", inbound="pw /V1/MANAGEMENT/NETWORK.DhcpEnabled=false\r\n",
          expect_state={"network": {"dhcp": False}})
telemetry(LW, "network-static", inbound="pw /MANAGEMENT/NETWORK.StaticIpAddress=192.168.0.20\r\n",
          expect_state={"network": {"static_address": "192.168.0.20"}})
telemetry(LW, "mx2-address", inbound="pr /MANAGEMENT/NETWORK.IpAddress=192.168.0.101\r\n",
          expect_state={"network": {"address": "192.168.0.101"}})
telemetry(LW, "v1-service", inbound="pr /V1/MANAGEMENT/NETWORK/SERVICES/LW3.Port=6107\r\n",
          expect_state={"network": {"services": {"LW3": {"port": 6107}}}})
telemetry(LW, "v1-ethernet", inbound="CHG /V1/MEDIA/ETHERNET/P3.Enabled=false\r\n",
          expect_state={"network": {"ethernet": {"3": {"enabled": False}}}})
telemetry(LW, "v1-5v", inbound="pw /V1/MEDIA/VIDEO/O1.Output5VMode=Follow signal present\r\n",
          expect_state={"video": {"outputs": {"1": {"output_5v_mode": "Follow signal present"}}}})
telemetry(LW, "v1-control-lock", inbound="pw /V1/MANAGEMENT/UI.ControlLock=Force locked\r\n",
          expect_state={"ui": {"control_lock": "Force locked"}})
telemetry(LW, "v1-autoselect", inbound="CHG /V1/MEDIA/VIDEO/AUTOSELECT/O1/I2.Priority=50\r\n",
          expect_state={"video": {"outputs": {"1": {"autoselect": {"2": {"priority": 50}}}}}})
telemetry(LW, "v1-edid-slot", inbound="pr /V1/EDID/U/U2.PreferredResolution=1920x1080p60.00Hz\r\n",
          expect_state={"edid": {"slots": {"U2": {"preferred_resolution": "1920x1080p60.00Hz"}}}})
telemetry(LW, "mx2-port-setting", inbound="pw /MEDIA/PORTS/VIDEO/O2/SETTINGS.HdcpMode=ALWAYS\r\n",
          expect_state={"video": {"outputs": {"2": {"hdcp_mode": "ALWAYS"}}}})
telemetry(LW, "umx-test-pattern", inbound="pw /MEDIA/VIDEO/I3.FreeRunMode=2\r\n",
          expect_state={"video": {"inputs": {"3": {"test_pattern_mode": 2}}}})
telemetry(LW, "umx-event", inbound="pw /EVENTS/E1.Action=/MEDIA/VIDEO/XP.switch=I4:O1\r\n",
          expect_state={"events": {"1": {"action": "/MEDIA/VIDEO/XP.switch=I4:O1"}}})
telemetry(LW, "umx-event-enabled", inbound="CHG /EVENTS/E12.Enabled=true\r\n",
          expect_state={"events": {"12": {"enabled": True}}})
telemetry(LW, "umx-variable", inbound="pw /CTRL/VARS/V3.Value=42\r\n", expect_state={"variables": {"3": {"value": "42"}}})
telemetry(LW, "umx-cec", inbound="CHG /MEDIA/CEC/O1.LastReceivedMessage=0F87000000\r\n",
          expect_state={"cec": {"O1": {"last_received": "0F87000000"}}})
telemetry(LW, "umx-login-enable", inbound="pw /LOGIN.LoginEnable=true\r\n", expect_state={"device": {"login_enabled": True}})
telemetry(LW, "mx2-preset-load", inbound="mO /MEDIA/PRESET/My_pres:load=Ok\r\n",
          expect_state={"presets": {"last_loaded": "My_pres", "My_pres": {"exists": True}}})
telemetry(LW, "mx2-preset-create", inbound="mO /MEDIA/PRESET:create=\r\n",
          request="CALL /MEDIA/PRESET:create(Stage_A,VIDEO)",
          expect_state={"presets": {"Stage_A": {"exists": True}}})
telemetry(LW, "mx2-preset-rename", inbound="mO /MEDIA/PRESET:rename=\r\n",
          request="CALL /MEDIA/PRESET:rename(Stage_A,Stage_B)",
          state_before={"presets": {"Stage_A": {"exists": True}}},
          expect_state={"presets": {"Stage_B": {"exists": True}}})


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
