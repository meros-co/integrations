DD = "dandugan-model-e"
# Dan Dugan Model E ASCII remote control (User Guide 2007, chapter 6): a command
# ended by CR, answered by its echo with the value and no spaces.
DD_ACK = {"ok": {"kind": "ack"}}


def dd(command, input, wire, reply=None, value=None, **extra):
    if reply is not None:
        extra["device_reply"] = reply + "\r"
        extra["expect_result"] = DD_ACK if value is None else {"ok": {"kind": "value", "value": value}}
    text(DD, command, input, wire + "\r", **extra)


dd("set_bypass", {"channel": 2, "bypassed": True}, "BP 2,1", "BP2,1")
dd("get_bypass", {"channel": 2}, "BP 2", "BP2,1", "1")
dd("set_mode", {"channel": 3, "mode": "auto"}, "CM 3,1", "CM3,1")
dd("get_mode", {"channel": 3}, "CM 3", "CM3,2", "2")
dd("set_name", {"channel": 1, "name": "Podium"}, "CN 1,Podium", "CN1,Podium")
dd("get_name", {"channel": 1}, "CN 1", "CN1,Podium", "Podium")
dd("set_override", {"channel": 8, "enabled": False}, "CO 8,0", "CO8,0")
dd("get_override", {"channel": 8}, "CO 8", "CO8,0", "0")
dd("set_preset_mode", {"channel": 4, "mode": "mute"}, "CP 4,2", "CP4,2")
dd("get_preset_mode", {"channel": 4}, "CP 4", "CP4,1", "1")
dd("set_weight", {"channel": 2, "weight_db": -4.55}, "CW 2,-4.55", "CW2,-4.55")      # guide example value
dd("get_weight", {"channel": 2}, "CW 2", "CW2,-4.55", "-4.55")
dd("set_unit_name", {"name": "Council"}, "NA Council", "NACouncil")
dd("get_unit_name", {}, "NA", "NACouncil", "Council")
dd("set_system_mute", {"muted": True}, "SM 1", "SM1")
dd("get_system_mute", {}, "SM", "SM0", "0")
dd("set_system_override", {"enabled": True}, "SO 1", "SO1")
dd("get_system_override", {}, "SO", "SO1", "1")
dd("recall_presets", {}, "SP 1", "SP OK")
dd("release_presets", {}, "SP 0", "SP OK")
dd("factory_reset", {}, "FP", "FP OK")
dd("set_dhcp", {"enabled": False}, "DH 0", "DH0")
dd("get_dhcp", {}, "DH", "DH1", "1")
dd("set_ip_address", {"address": "192.168.1.50"}, "IP 192.168.1.50", "IP192.168.1.50")
dd("get_ip_address", {}, "IP", "IP192.168.1.50", "192.168.1.50")
dd("set_gateway", {"address": "192.168.1.1"}, "GW 192.168.1.1", "GW192.168.1.1")
dd("get_gateway", {}, "GW", "GW192.168.1.1", "192.168.1.1")
dd("set_netmask", {"address": "255.255.255.0"}, "NM 255.255.255.0", "NM255.255.255.0")
dd("get_netmask", {}, "NM", "NM255.255.255.0", "255.255.255.0")
dd("get_headroom", {}, "HR", "HR1", "1")
dd("get_master", {}, "MM", "MM1", "1")
dd("get_versions", {}, "VE", "VE 1.0, 1.0", "1.0, 1.0")                                 # guide example
text(DD, "set_bypass", {"channel": 1, "bypassed": False}, "BP 1,0\r", device_reply="BP Syntax error\r",
     expect_result={"error": {"error": "device_error"}}, file="set_bypass-error")


def ddt(name, inbound, state):
    telemetry(DD, name, inbound=inbound + "\r", expect_state=state)


ddt("bypass", "BP2,1", {"channels": {"2": {"bypassed": True}}})
ddt("mode", "CM3,1", {"channels": {"3": {"mode": "auto"}}})
ddt("name", "CN1,Podium", {"channels": {"1": {"name": "Podium"}}})
ddt("override", "CO8,1", {"channels": {"8": {"override": True}}})
ddt("preset-mode", "CP4,2", {"channels": {"4": {"preset_mode": "mute"}}})
ddt("weight", "CW2,-4.55", {"channels": {"2": {"weight_db": -4.55}}})
ddt("unit-name", "NACouncil", {"unit": {"name": "Council"}})
ddt("system-mute", "SM1", {"unit": {"system_mute": True}})
ddt("system-override", "SO0", {"unit": {"system_override": False}})
ddt("dhcp", "DH1", {"unit": {"dhcp": True}})
ddt("ip", "IP192.168.1.50", {"unit": {"ip_address": "192.168.1.50"}})
ddt("gateway", "GW192.168.1.1", {"unit": {"gateway": "192.168.1.1"}})
ddt("netmask", "NM255.255.255.0", {"unit": {"netmask": "255.255.255.0"}})
ddt("headroom", "HR1", {"unit": {"headroom": "+4dBu"}})
ddt("master", "MM0", {"unit": {"master": False}})
ddt("versions", "VE 1.0, 1.0", {"unit": {"versions": "1.0, 1.0"}})
