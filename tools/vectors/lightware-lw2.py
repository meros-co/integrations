L2 = "lightware-lw2"
# Lightware LW2 (MX8x8HDMI-Pro manual v3.0 chapter 7; UMX-HDMI-140 manual
# chapter 6). Commands are sent in curly brackets followed by CR LF; answers
# are in round brackets, from the manuals' examples.
MXP = {"model": "mx-pro"}
UMX2 = {"model": "umx-hdmi-140"}
OK2 = {"ok": {"kind": "ack"}}
ER2 = {"error": {"error": "device_error"}}


def v2(x):
    return {"ok": {"kind": "value", "value": x}}


# General (MX 7.2, UMX 6.3)
text(L2, "get_product_type", {}, "{i}\r\n", device_reply="(MX8x8DVI FRAME)\r\n", expect_result=v2("MX8x8DVI FRAME"), **MXP)
text(L2, "get_firmware", {}, "{f}\r\n", device_reply="(FW:2.5.0)\r\n", expect_result=v2("2.5.0"), **MXP)
text(L2, "get_serial", {}, "{s}\r\n", device_reply="(SN:33004291)\r\n", expect_result=v2("33004291"), **MXP)
text(L2, "get_compile_time", {}, "{ct}\r\n", device_reply="(Compiled: Nov 25 2013 12:40:07, build: 1737)\r\n",
     expect_result=v2("Nov 25 2013 12:40:07, build: 1737"), **MXP)
text(L2, "get_health", {}, "{st}\r\n", device_reply="(STAT 3.3V 5.0V 29V)\r\n", expect_result=v2("3.3V 5.0V 29V"), **MXP)
text(L2, "get_protocol", {}, "{P_?}\r\n", device_reply="(CONTROL PROTOCOL = #1)\r\n", expect_result=v2("1"), **MXP)
text(L2, "ping", {}, "{ping}\r\n", device_reply="(PONG!)\r\n", expect_result=OK2, **UMX2)
text(L2, "get_label", {}, "{label}\r\n", device_reply="(LABEL=UMX-HDMI-140_ConferenceRoom)\r\n",
     expect_result=v2("UMX-HDMI-140_ConferenceRoom"), **UMX2)
text(L2, "restart", {}, "{rst}\r\n", **MXP)
text(L2, "factory_defaults", {}, "{factory=all}\r\n", device_reply="(FACTORY ALL...)\r\n", expect_result=OK2, **UMX2)

# Crosspoint (MX 7.3.1-7.3.8)
text(L2, "switch", {"input": 1, "output": 2}, "{1@2}\r\n", device_reply="(O02 I01)\r\n", expect_result=OK2, **MXP)
text(L2, "switch_all", {"input": 1}, "{1@O}\r\n", device_reply="(I01 ALL)\r\n", expect_result=OK2, **MXP)
text(L2, "disconnect", {"output": 3}, "{0@3}\r\n", device_reply="(O03 I00)\r\n", expect_result=OK2, **MXP)
text(L2, "mute_output", {"output": 1}, "{#01}\r\n", device_reply="(1MT01)\r\n", expect_result=OK2, **MXP)
text(L2, "unmute_output", {"output": 1}, "{+01}\r\n", device_reply="(0MT01)\r\n", expect_result=OK2, **MXP)
text(L2, "lock_output", {"output": 1}, "{#>01}\r\n", device_reply="(1LO01)\r\n", expect_result=OK2, **MXP)
text(L2, "unlock_output", {"output": 1}, "{+<01}\r\n", device_reply="(0LO01)\r\n", expect_result=OK2, **MXP)
text(L2, "get_connections", {}, "{VC}\r\n", device_reply="(ALL M01 L02 U03 04 05 06 07 08)\r\n",
     expect_result=v2("M01 L02 U03 04 05 06 07 08"), **MXP)
text(L2, "get_mutes", {}, "{VM}\r\n", device_reply="(MUT 1 0 1 0 0 0 0 0)\r\n", expect_result=v2("1 0 1 0 0 0 0 0"), **MXP)

# Layers (UMX 6.4)
text(L2, "switch_layer", {"input": 2, "output": 1, "layer": "AV"}, "{2@1 AV}\r\n", device_reply="(O01 I02 AV)\r\n",
     expect_result=OK2, **UMX2)
text(L2, "mute_output_layer", {"output": 1, "layer": "A"}, "{#1 A}\r\n", device_reply="(1MT01 A)\r\n",
     expect_result=OK2, **UMX2)
text(L2, "unmute_output_layer", {"output": 1, "layer": "AV"}, "{+1 AV}\r\n", device_reply="(0MT01)\r\n",
     expect_result=OK2, **UMX2)
text(L2, "lock_output_layer", {"output": 1, "layer": "V"}, "{#>1 V}\r\n", device_reply="(1LO01 V)\r\n",
     expect_result=OK2, **UMX2)
text(L2, "unlock_output_layer", {"output": 1, "layer": "AV"}, "{+<1 AV}\r\n", device_reply="(0LO01)\r\n",
     expect_result=OK2, **UMX2)
text(L2, "get_connection_layer", {"layer": "V"}, "{VC V}\r\n", device_reply="(ALLV 03)\r\n", expect_result=v2("03"), **UMX2)
text(L2, "get_crosspoint_size", {"layer": "V"}, "{GETSIZE V}\r\n", device_reply="(SIZE=6x1 V)\r\n",
     expect_result=v2("6x1"), **UMX2)
text(L2, "get_autoselect", {"layer": "V"}, "{AS_V1=?}\r\n", device_reply="(AS_V1=D;P)\r\n", expect_result=v2("D;P"), **UMX2)
text(L2, "set_autoselect", {"layer": "A", "state": "E", "mode": "F"}, "{AS_A1=E;F}\r\n", device_reply="(AS_A1=E;F)\r\n",
     expect_result=OK2, **UMX2)
text(L2, "get_autoselect_priority", {"layer": "V"}, "{PRIO_V1=?}\r\n", device_reply="(PRIO_V1=0;1;2;3;4)\r\n",
     expect_result=v2("0;1;2;3;4"), **UMX2)
text(L2, "set_autoselect_priority", {"layer": "A", "priorities": "1;0;2;3;4;5"}, "{PRIO_A1=1;0;2;3;4;5}\r\n",
     device_reply="(PRIO_A1=1;0;2;3;4;5)\r\n", expect_result=OK2, **UMX2)

# Presets and names (MX 7.3.9-7.3.20)
text(L2, "save_preset", {"preset": 1}, "{$1}\r\n", device_reply="(SPR01)\r\n", expect_result=OK2, **MXP)
text(L2, "load_preset", {"preset": 1}, "{%1}\r\n", device_reply="(LPR01)\r\n", expect_result=OK2, **MXP)
text(L2, "load_preset", {"preset": 9}, "{%9}\r\n", device_reply="(ERR04)\r\n", expect_result=ER2, **MXP)
text(L2, "view_preset", {"preset": 1}, "{VP#1=?}\r\n", device_reply="(VP#1= M01 02 M03 04 05 06 07 08)\r\n",
     expect_result=v2("M01 02 M03 04 05 06 07 08"), **MXP)
text(L2, "set_preset_name", {"preset": 1, "name": "first preset"}, "{PNAME#1=first preset}\r\n",
     device_reply="(PNAME#1=FIRST PRESET)\r\n", expect_result=OK2, **MXP)
text(L2, "get_preset_name", {"preset": 1}, "{PNAME#1=?}\r\n", device_reply="(PNAME#1=FIRST PRESET)\r\n",
     expect_result=v2("FIRST PRESET"), **MXP)
text(L2, "reset_preset_names", {}, "{PNAME#1=!}\r\n", device_reply="(PNAME#1=Preset 1)\r\n", expect_result=OK2, **MXP)
text(L2, "set_input_name", {"input": 1, "name": "first input"}, "{INAME#1=first input}\r\n",
     device_reply="(INAME#1=FIRST INPUT)\r\n", expect_result=OK2, **MXP)
text(L2, "get_input_name", {"input": 1}, "{INAME#1=?}\r\n", device_reply="(INAME#1=FIRST INPUT)\r\n",
     expect_result=v2("FIRST INPUT"), **MXP)
text(L2, "reset_input_names", {}, "{INAME#1=!}\r\n", device_reply="(INAME#1=Input 1)\r\n", expect_result=OK2, **MXP)
text(L2, "set_output_name", {"output": 1, "name": "first output"}, "{ONAME#1=first output}\r\n",
     device_reply="(ONAME#1=FIRST OUTPUT)\r\n", expect_result=OK2, **MXP)
text(L2, "get_output_name", {"output": 1}, "{ONAME#1=?}\r\n", device_reply="(ONAME#1=FIRST OUTPUT)\r\n",
     expect_result=v2("FIRST OUTPUT"), **MXP)
text(L2, "reset_output_names", {}, "{ONAME#1=!}\r\n", device_reply="(ONAME#1=Output 1)\r\n", expect_result=OK2, **MXP)

# EDID and port status (MX 7.5-7.6)
text(L2, "edid_switch", {"input": 5, "location": 10}, "{5:10}\r\n", device_reply="(E_SW_OK)\r\n", expect_result=OK2, **MXP)
text(L2, "edid_switch_all", {"location": 2}, "{A:2}\r\n", device_reply="(E_SW_OK)\r\n", expect_result=OK2, **MXP)
text(L2, "edid_learn", {"location": 4, "output": 3}, "{4>3}\r\n", device_reply="(E_SW_OK)\r\n", expect_result=OK2, **MXP)
text(L2, "get_emulated_edids", {}, "{VEDID}\r\n", device_reply="(VEDID 025 101 006 102 024 101 101 024)\r\n",
     expect_result=v2("025 101 006 102 024 101 101 024"), **MXP)
text(L2, "get_edid_header", {"location": 7}, "{WH7}\r\n", device_reply="(EH#7 NEC 1280x1024@60 LCD1970NXp)\r\n",
     expect_result=v2("NEC 1280x1024@60 LCD1970NXp"), **MXP)
text(L2, "get_input_status", {}, "{:ISD}\r\n", device_reply="(ISD 31000000)\r\n", expect_result=v2("31000000"), **MXP)
text(L2, "get_output_status", {}, "{:OSD}\r\n", device_reply="(OSD 10000000)\r\n", expect_result=v2("10000000"), **MXP)
text(L2, "clear_hdcp_key_cache", {}, "{:HDCPRESET}\r\n", device_reply="(DONE)\r\n", expect_result=OK2, **MXP)
text(L2, "get_ip_config", {}, "{IP_CONFIG=?}\r\n", device_reply="(IP_CONFIG=7 192.168.0.103 10001 255.255.255.0 192.168.0.1)\r\n",
     expect_result=v2("7 192.168.0.103 10001 255.255.255.0 192.168.0.1"), **MXP)

# Telemetry: the polled connection view and replies.
telemetry(L2, "connections", inbound="(ALL M01 L02 U03 04 05 06 07 00)\r\n",
          expect_state={"outputs": {
              "1": {"input": 1, "muted": True, "locked": False},
              "2": {"input": 2, "muted": False, "locked": True},
              "3": {"input": 3, "muted": True, "locked": True},
              "4": {"input": 4, "muted": False, "locked": False},
              "5": {"input": 5, "muted": False, "locked": False},
              "6": {"input": 6, "muted": False, "locked": False},
              "7": {"input": 7, "muted": False, "locked": False},
              "8": {"input": 0, "muted": False, "locked": False}}})
telemetry(L2, "switch", inbound="(O02 I01)\r\n", expect_state={"outputs": {"2": {"input": 1}}})
telemetry(L2, "mute", inbound="(1MT01)\r\n", expect_state={"outputs": {"1": {"muted": True}}})
telemetry(L2, "unlock", inbound="(0LO03)\r\n", expect_state={"outputs": {"3": {"locked": False}}})
telemetry(L2, "names", inbound="(INAME#1=FIRST INPUT)\r\n", expect_state={"inputs": {"1": {"name": "FIRST INPUT"}}})
telemetry(L2, "preset", inbound="(LPR01)\r\n", expect_state={"last_preset_loaded": 1})
telemetry(L2, "umx-layer", inbound="(ALLV M03)\r\n",
          expect_state={"layers": {"V": {"input": 3, "muted": True, "locked": False}}})
telemetry(L2, "product", inbound="(I:UMX-HDMI-140)\r\n", expect_state={"device": {"product_type": "UMX-HDMI-140"}})
