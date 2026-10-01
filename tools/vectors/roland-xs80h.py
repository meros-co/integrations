R80 = "roland-xs80h"
# Roland XS-82H/83H/84H (Reference Manual v3.0, edition 03, p.21-24): code +
# ":" arguments + ";", followed by CR over LAN; replies "ACK;", "ERR:n;" or a
# status frame. Inputs, outputs and presets are 0-based on the wire.
R80_OK = {"ok": {"kind": "ack"}}


def r80(command, input, wire, **extra):
    text(R80, command, input, wire, **extra)


r80("set_input_type", {"input": 1, "type": 8}, "VIS:0,8;\r", device_reply="ACK;", expect_result=R80_OK)
r80("set_input_color_space", {"input": 8, "space": 4}, "VIC:7,4;\r", device_reply="ERR:5;",
    expect_result={"error": {"error": "device_error"}})
r80("set_input_hdcp", {"input": 2, "enabled": True}, "VIH:1,1;\r")
r80("set_input_aspect", {"input": 3, "aspect": 4}, "VIA:2,4;\r")
r80("set_output_select", {"output": 4, "select": 2}, "VOS:3,2;\r")
r80("set_output_resolution", {"output": 1, "resolution": 16}, "VOR:0,16;\r")
r80("set_output_hdcp", {"output": 2, "enabled": False}, "VOH:1,0;\r")
r80("set_output_color_space", {"output": 3, "space": 3}, "VOC:2,3;\r")
r80("set_output_signal", {"output": 1, "hdmi": True}, "VOD:0,1;\r")
r80("route_av", {"output": 2, "input": 5}, "OAV:1,4;\r", device_reply="ACK;\r", expect_result=R80_OK)
r80("route_video", {"output": 1, "input": 8}, "OVS:0,7;\r")
r80("route_audio", {"output": 4, "input": 1}, "OAS:3,0;\r")
r80("set_output_off", {"output": 1, "pressed": True}, "OFS:0,1;\r")
r80("set_hdmi_input_level", {"input": 1, "level": 127}, "IDL:0,127;\r")
r80("set_analog_input_level", {"input": 8, "level": 0}, "IAL:7,0;\r")
r80("set_output_level", {"output": 2, "level": 100}, "OAL:1,100;\r")
r80("set_hdmi_input_mute", {"input": 4, "enabled": True}, "IDM:3,1;\r")
r80("set_analog_input_mute", {"input": 4, "enabled": False}, "IAM:3,0;\r")
r80("set_output_mute", {"output": 3, "enabled": True}, "OAM:2,1;\r")
r80("set_output_delay", {"output": 1, "delay": 170}, "ADT:0,170;\r")
r80("set_mode", {"mode": 18}, "MOD:18;\r")
r80("recall_preset", {"preset": 32}, "PSE:31;\r")
r80("get_input_status", {"input": 1}, "ITS:0;\r", device_reply="ITS:0,0,0,1,0;",
    expect_result={"ok": {"kind": "value", "value": "0,0,0,1,0"}})
r80("get_output_status", {"output": 2}, "OTS:1;\r", device_reply="OTS:1,1,6,1,0,1;",
    expect_result={"ok": {"kind": "value", "value": "1,1,6,1,0,1"}})
r80("get_crosspoint", {"output": 1}, "CTS:0;\r", device_reply="CTS:0,3,3,0;",
    expect_result={"ok": {"kind": "value", "value": "0,3,3,0"}})
r80("auto_take", {"output": 3}, "ATO:2,1;\r")
r80("set_output1_send", {"input": 16, "rate": 100}, "ASA:15,100;\r")
r80("set_output2_send", {"input": 9, "rate": 0}, "ASB:8,0;\r")
r80("set_output3_send", {"input": 1, "rate": 50}, "ASC:0,50;\r")
r80("set_output4_send", {"input": 8, "rate": 75}, "ASD:7,75;\r")
r80("set_rgb_edid", {"input": 1, "edid": 10}, "AED:0,10;\r")
r80("set_hdmi_edid", {"input": 2, "edid": 25}, "DED:1,25;\r")
r80("set_panel_lock", {"enabled": True}, "PLS:1;\r")
r80("set_key_lock_mode", {"mode": 9, "enabled": True}, "KLM:9,1;\r")
r80("get_key_lock", {}, "KLS;\r", device_reply="KLS:1,1,0,0,0,0,0,0,0,0,0;",
    expect_result={"ok": {"kind": "value", "value": "1,1,0,0,0,0,0,0,0,0,0"}})
r80("get_version", {}, "VER;\r", device_reply="VER:XS-84H,3.15;",
    expect_result={"ok": {"kind": "value", "value": "XS-84H,3.15"}})
r80("active_sense", {}, "ACS;\r", device_reply="ACK;", expect_result=R80_OK)

telemetry(R80, "crosspoint", inbound="CTS:0,3,4,1;",
          expect_state={"outputs": {"1": {"video": 4, "audio": 5, "off": True}}})
telemetry(R80, "input-status", inbound="ITS:7,4,2,0,1;",
          expect_state={"inputs": {"8": {"type": 4, "color_space": 2, "hdcp": False, "aspect": 1}}})
telemetry(R80, "output-status", inbound="OTS:3,2,16,1,4,0;",
          expect_state={"outputs": {"4": {"select": 2, "resolution": 16, "hdcp": True, "color_space": 4, "hdmi": False}}})
telemetry(R80, "key-lock", inbound="KLS:1,1,0,0,0,0,0,0,0,0,1;",
          expect_state={"panel_lock": True, "key_lock_modes": "1,0,0,0,0,0,0,0,0,1"})
telemetry(R80, "version", inbound="VER:XS-82H,3.15;", expect_state={"device": {"model": "XS-82H", "version": "3.15"}})
