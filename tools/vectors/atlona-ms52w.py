# Atlona AT-OME-MS52W Telnet API (atlona-ms52w): commands as the API document
# (40139-R5) writes them, ended by CR; answers as compact JSON lines (OutputMode
# j) holding result and methodreturn, or a line of text.
AMS = "atlona-ms52w"


def ams(command, input, wire, **extra):
    text(AMS, command, input, wire, **extra)


def ok(method):
    return json.dumps({"result": {"success": True}, "methodreturn": method}, separators=(",", ":")) + "\r\n"


_ACK = {"expect_result": {"ok": {"kind": "ack"}}}
ams("get_bass", {}, "Audio:GetBass\r",
    device_reply='{"result":{"bass":{"units":"dB","value":2}},"methodreturn":"Audio: GetBass"}\r\n',
    expect_result={"ok": {"kind": "value", "value": "2"}})
ams("set_bass", {"db": 2}, "Audio:SetBass 2\r", device_reply=ok("audio: setbass 2"), **_ACK)
ams("get_treble", {}, "Audio:GetTreble\r")
ams("set_treble", {"db": -5}, "Audio:SetTreble -5\r", device_reply=ok("audio: settreble -5"), **_ACK)
ams("get_audio_source", {}, "Audio:GetSource\r")
ams("set_audio_source", {"source": "analog"}, "Audio:SetSource analog\r", device_reply=ok("audio: setsource analog"), **_ACK)
ams("get_mute", {}, "Audio:Mute:Get\r")
ams("set_mute", {"output": "hdmi", "muted": True}, "Audio:Mute:Set hdmi true\r",
    device_reply=ok("audio: mute: set hdmi true"), **_ACK)
ams("get_volume", {}, "Audio:Volume:Get\r")
ams("set_volume", {"db": -10}, "Audio:Volume:Set -10\r", device_reply=ok("audio: volume: set - 10"), **_ACK)
ams("volume_up", {"db": 25}, "Audio:Volume:Increase 25\r")
ams("volume_down", {"db": 20}, "Audio:Volume:Decrease 20\r")
ams("volume_step_up", {}, "SetVol +\r", device_reply='{"result":{"volume":-9,"success":true},"methodreturn":"setvol +"}\r\n',
    expect_result={"ok": {"kind": "value", "value": "-9"}})
ams("volume_step_down", {}, "SetVol -\r")
ams("refresh_calendar", {}, "Calendar:Refresh\r")
ams("cec_power", {"output": "hdmi", "on": True}, "CEC:Trig 1 1\r", device_reply=ok("cec: trig 1 1"), **_ACK)
ams("kick_byod", {}, "Display:BYOD:Kick\r")
ams("get_display_control", {}, "Display:Control:IP:Get\r")
ams("set_display_control", {"ip": "10.20.50.58", "port": 23}, "Display:Control:IP:Set 10.20.50.58 23\r")
ams("get_inputs", {}, "Display:Input:All:Get\r")
ams("get_input", {}, "Display:Input:Get\r")
ams("set_input", {"input": "displayport"}, ["Display:Input:Set 1\r", "Display:Input:Get\r"],
    device_reply='{"result":{"activeinput":1},"methodreturn":"display: input: set 1","jsonrpc":"2.0"}\r\n')
ams("get_hdcp", {"input": "usb_c"}, "Display:Input:HDCP:State:Get 0\r")
ams("set_hdcp", {"input": "hdmi_3", "enabled": False},
    ["Display:Input:HDCP:State:Set 2 0\r", "Display:Input:HDCP:State:Get 2\r"],
    device_reply=ok("display: input: hdcp: state: set 2 0"))
ams("get_input_state", {"input": "byod"}, "Display:InputState:Get 4\r")
ams("get_route", {"output": "hdmi"}, "Display:Matrix:Get 1\r")
ams("set_route", {"input": "usb_c", "output": "hdbaset"}, ["Display:Matrix:Set 0 0\r", "Display:Matrix:Get 0\r"],
    device_reply=ok("Display: Matrix: Set 0 0"))
ams("get_matrix_mode", {}, "Display:Matrix:Mode:Get\r")
ams("set_matrix_mode", {"mode": "static"}, ["Display:Matrix:Mode:Set 2\r", "Display:Matrix:Mode:Get\r"],
    device_reply=ok("display: matrix: mode: set 2"))
ams("get_display_power", {}, "Display:Minimal:Get\r", device_reply="on\r\n",
    expect_result={"ok": {"kind": "value", "value": "on"}})
ams("set_display_power", {"on": False}, ["Display:Minimal:Set 0\r", "Display:Minimal:Get\r"],
    device_reply=ok("display: minimal: set 0"))
ams("get_broadcast", {}, "GetBroadcast\r", device_reply="Off\r\n", expect_result={"ok": {"kind": "value", "value": "Off"}})
ams("set_broadcast", {"enabled": True}, "SetBroadcast 1\r", device_reply="Broadcast on\r\n", **_ACK)
ams("get_hostname", {}, "GetHostName\r")
ams("get_no_signal_timeout", {}, "GetNoSignalTimeout\r")
ams("get_temperature", {}, "Instruments:Temperature:Get c\r")
ams("get_model", {}, "Misc:Model:Get\r")
ams("get_versions", {}, "Misc:Versions:Get\r")
ams("get_network", {}, "Net:GetInfo\r")
ams("factory_reset", {}, "Platform:Reset all\r")
ams("factory_reset", {"network": True}, "Platform:Reset all network\r", file="factory_reset-network")
ams("restart", {}, "Platform:Restart\r")
ams("shutdown", {}, "Platform:Shutdown\r")
ams("get_moderator", {}, "Moderator:Enable:Get\r")
ams("set_moderator", {"enabled": True}, "Moderator:Enable:Set 1\r")
ams("get_moderator_status", {}, "Moderator:Status:Get\r")
ams("moderator_kick", {"client": 2}, "Moderator:Kick 2\r")
ams("moderator_show", {"client": 2}, "Moderator:Show 2\r")
ams("get_relay_pulse", {}, "Relay:PulseDuration:Get\r")
ams("set_relay_pulse", {"seconds": 2}, ["Relay:PulseDuration:Set 2\r", "Relay:PulseDuration:Get\r"],
    device_reply=ok("relay: pulseduration: set 2"))
ams("get_relay", {"relay": 0}, "Relay:State:Get 0\r")
ams("set_relay", {"relay": 1, "closed": True}, ["Relay:State:Set 1 1\r", "Relay:State:Get 1\r"],
    device_reply=ok("relay: state: set 1 1"))
ams("usb_follow_input", {"input": 2, "host": "host1"}, "USBRouting:Follow:Input 2 1\r")
ams("get_usb_input", {}, "USBRouting:Input:Get\r")
ams("set_usb_input", {"port": "usb_c"}, ["USBRouting:Input:Set 4\r", "USBRouting:Input:Get\r"],
    device_reply=ok("usbrouting:input:set 4"))
ams("get_usb_mode", {}, "USBRouting:Mode:Get\r")
ams("set_usb_mode", {"mode": "manual"}, ["USBRouting:Mode:Set MANUAL\r", "USBRouting:Mode:Get\r"],
    device_reply=ok("usbrouting:mode:set manual"))
ams("get_zone", {"zone": 1}, "Zone:PortParams 1\r")
ams("set_zone", {"zone": 1, "baud": "19200"}, ["Zone:PortSetup 1 19200 8 0 1\r", "Zone:PortParams 1\r"],
    device_reply=ok("zone: portsetup 1 19200 8 0 1"))
ams("send_zone_command", {"zone": 1, "command": "vol+"}, "Zone:SendCmd 1 vol+\r")


def jl(obj):
    return json.dumps(obj, separators=(",", ":")) + "\r\n"


telemetry(AMS, "bass", inbound=jl({"result": {"bass": {"units": "dB", "value": 3}}, "methodreturn": "Audio: GetBass"}),
          expect_state={"audio": {"bass_db": 3}})
telemetry(AMS, "mute", inbound=jl({"result": {"outputmute": {"analog": False, "hdmi": True}}, "methodreturn": "Audio: Mute: Get"}),
          expect_state={"audio": {"mute": {"hdmi": True, "analog": False}}})
telemetry(AMS, "volume", inbound=jl({"result": {"volume": {"units": "dB", "value": -12}}, "methodreturn": "Audio:Volume:Get"}),
          expect_state={"audio": {"volume_db": -12}})
telemetry(AMS, "volume-step", inbound=jl({"result": {"volume": -20, "success": True}, "methodreturn": "audio: volume: decrease 20"}),
          expect_state={"audio": {"volume_db": -20}})
telemetry(AMS, "inputs", inbound=jl({"result": {"0": {"status": False}, "1": {"status": True}, "4": {"type": "airplay", "status": False}},
                                     "methodreturn": "display: input: all: get", "jsonrpc": "2.0"}),
          expect_state={"inputs": {"0": {"connected": False}, "1": {"connected": True}, "4": {"connected": False, "type": "airplay"}}})
telemetry(AMS, "active-input", inbound=jl({"result": {"input": 4, "type": "airplay"}, "methodreturn": "display: input: get", "jsonrpc": "2.0"}),
          expect_state={"active_input": "byod", "active_input_type": "airplay"})
telemetry(AMS, "hdcp", inbound=jl({"result": {"state": True}, "methodreturn": "display: input: hdcp: state: get 0"}),
          expect_state={"inputs": {"0": {"hdcp": True}}})
telemetry(AMS, "route-hdmi", inbound=jl({"result": {"input": 4}, "methodreturn": "display: matrix: get 1", "jsonrpc": "2.0"}),
          expect_state={"outputs": {"hdmi": {"input": "byod"}}})
telemetry(AMS, "matrix-mode", inbound=jl({"result": {"mode": True, "subtype": "MATRIX_MODE_STATIC"}, "methodreturn": "display: matrix: mode: get"}),
          expect_state={"matrix_mode": {"enabled": True, "subtype": "MATRIX_MODE_STATIC"}})
telemetry(AMS, "display-power", request="Display:Minimal:Get", inbound="on\r\n", expect_state={"display_power": True})
telemetry(AMS, "broadcast", request="GetBroadcast", inbound="Off\r\n", expect_state={"broadcast": False})
telemetry(AMS, "versions", inbound=jl({"result": {"versions": {"mcu": "V1.0.00", "master": "2.8.1"}}, "methodreturn": "misc: versions: get"}),
          expect_state={"versions": {"master": "2.8.1", "mcu": "V1.0.00"}})
telemetry(AMS, "moderator-event", inbound=jl({"jsonrpc": "2.0", "event": {"moderator": {"streams": [], "activeindex": -1, "enabled": True}}}),
          expect_state={"moderator": {"enabled": True, "active_index": -1, "streams": "[]"}})
telemetry(AMS, "network", inbound=jl({"result": {"netinfo": {"eth0": {"mode": "dynamic", "netmask": "255.255.255.0", "ip": "10.20.20.32",
                                                                       "mac": "00:1e:06:36:b5:00"},
                                                              "wlan0": {"mode": "access point", "netmask": "255.255.255.0", "ip": "192.168.43.1",
                                                                        "mac": "74:da:38:ef:db:e8", "ap": {"channel": 165, "ssid": "ms52w-b500"}}}},
                                      "methodreturn": "net: getinfo"}),
          expect_state={"network": {"eth0": {"ip": "10.20.20.32", "netmask": "255.255.255.0", "mac": "00:1e:06:36:b5:00", "mode": "dynamic"},
                                    "wlan0": {"ip": "192.168.43.1", "netmask": "255.255.255.0", "mac": "74:da:38:ef:db:e8",
                                              "mode": "access point", "ssid": "ms52w-b500", "channel": 165}}})
telemetry(AMS, "relay", inbound=jl({"result": {"state": True}, "methodreturn": "relay: state: get 0"}),
          expect_state={"relays": {"0": {"closed": True}}})
telemetry(AMS, "usb-mode", inbound=jl({"result": {"mode": "AUTOSWITCH"}, "methodreturn": "usbrouting:mode:get", "jsonrpc": "2.0"}),
          expect_state={"usb": {"mode": "autoswitch"}})
telemetry(AMS, "zone", inbound=jl({"result": {"zone": 1, "params": "[9600, 8, 0, 1]"}, "methodreturn": "zone: portparams 1"}),
          expect_state={"zones": {"1": {"params": "[9600, 8, 0, 1]"}}})

# AT-UHD-SW-510W (API 40xxx version 13: SetVol -61, Input:Set 1, Misc:Version:Get mcu, OSD:State:Set 1)
SW = {"model": "at-uhd-sw-510w"}
ams("set_volume_level", {"db": -61}, "SetVol -61\r", device_reply=ok("setvol -61"), **_ACK, **SW)
ams("get_volume_level", {}, "SetVol sta\r", device_reply=jl({"result": {"volume": -61, "success": True}, "methodreturn": "setvol sta"}),
    expect_result={"ok": {"kind": "value", "value": "-61"}}, **SW)
ams("get_active_input", {}, "Input:Get\r",
    device_reply=jl({"result": {"input": 4, "type": "airplay"}, "methodreturn": "input:get", "jsonrpc": "2.0"}),
    expect_result={"ok": {"kind": "value", "value": "4"}}, **SW)
ams("select_input", {"input": "displayport"}, "Input:Set 1\r", **SW)
ams("get_version", {"system": "mcu"}, "Misc:Version:Get mcu\r",
    device_reply=jl({"result": {"version": {"mcu": "V1.1.05"}}, "methodreturn": "misc:version:get mcu"}),
    expect_result={"ok": {"kind": "value", "value": "V1.1.05"}}, **SW)
ams("get_osd", {}, "OSD:State:Get\r", device_reply=jl({"result": {"state": True}, "methodreturn": "osd:state:get"}),
    expect_result={"ok": {"kind": "value", "value": "true"}}, **SW)
ams("set_osd", {"shown": True}, "OSD:State:Set 1\r", **SW)
ams("set_route", {"input": "usb_c", "output": "hdbaset"}, "Display:Matrix:Set 0 0\r", file="set_route-sw510w", **SW)
telemetry(AMS, "osd-sw510w", model="at-uhd-sw-510w", inbound=jl({"result": {"state": True}, "methodreturn": "osd:state:get"}),
          expect_state={"osd_shown": True})
telemetry(AMS, "version-mcu-sw510w", model="at-uhd-sw-510w",
          inbound=jl({"result": {"version": {"mcu": "V1.1.05"}}, "methodreturn": "misc:version:get mcu"}),
          expect_state={"versions": {"mcu": "V1.1.05"}})
telemetry(AMS, "active-input-sw510w", model="at-uhd-sw-510w",
          inbound=jl({"result": {"input": 4, "type": "airplay"}, "methodreturn": "input:get", "jsonrpc": "2.0"}),
          expect_state={"active_input": "byod", "active_input_type": "airplay"})
