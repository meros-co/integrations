K = "kramer-p3000"
# Kramer Protocol 3000 (Protocol 3000 Reference Guide v3.0, P/N 2900-300489
# Rev 5): host messages "#" + command + CR; device messages "~nn@...CR LF".
# ROUTE is layer,destination,source; VID/AUD/AV are input>output.
KX = {"model": "p3000-extended"}

# System (p.9-13, p.21-28, p.30, p.36)
text(K, "handshake", {}, "#\r", device_reply="~01@ OK\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "get_model", {}, "#MODEL?\r", device_reply="~01@MODEL VS-88UT\r\n",
     expect_result={"ok": {"kind": "value", "value": "VS-88UT"}})
text(K, "get_version", {}, "#VERSION?\r", device_reply="~01@VERSION 1.12.0034\r\n",
     expect_result={"ok": {"kind": "value", "value": "1.12.0034"}})
text(K, "get_serial", {}, "#SN?\r", device_reply="~01@SN 00000123456789\r\n",
     expect_result={"ok": {"kind": "value", "value": "00000123456789"}})
text(K, "get_protocol_version", {}, "#PROT-VER?\r", device_reply="~01@PROT-VER 3000:1.0\r\n",
     expect_result={"ok": {"kind": "value", "value": "1.0"}})
text(K, "get_build_date", {}, "#BUILD-DATE?\r", device_reply="~01@BUILD-DATE 2017/07/26 10:15:00\r\n",
     expect_result={"ok": {"kind": "value", "value": "2017/07/26 10:15:00"}})
text(K, "get_io_count", {}, "#INFO-IO?\r", device_reply="~01@INFO-IO? IN 8,OUT 8\r\n",
     expect_result={"ok": {"kind": "value", "value": "IN 8,OUT 8"}})
text(K, "get_preset_count", {}, "#INFO-PRST?\r", device_reply="~01@INFO-PRST VID 60,AUD 60\r\n",
     expect_result={"ok": {"kind": "value", "value": "VID 60,AUD 60"}})
text(K, "get_name", {}, "#NAME?\r", device_reply="~01@NAME? ROOM-A\r\n",
     expect_result={"ok": {"kind": "value", "value": "ROOM-A"}})
text(K, "set_name", {"name": "ROOM-A"}, "#NAME ROOM-A\r", device_reply="~01@NAME ROOM-A\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "reset_name", {}, "#NAME-RST\r", device_reply="~01@NAME-RST OK\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "identify", {}, "#IDV\r", device_reply="~01@IDV OK\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "reset", {}, "#RESET\r", device_reply="~01@RESET OK\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "factory_reset", {}, "#FACTORY\r", device_reply="~01@FACTORY OK\r\n", expect_result={"ok": {"kind": "ack"}})

# Routing: ROUTE layer,dest,src (p.54); layers 1 video .. 5 USB (p.140)
text(K, "route_video", {"input": 3, "output": 2}, "#ROUTE 1,2,3\r",
     device_reply="~01@ROUTE 1,2,3\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "route_audio", {"input": 0, "output": 1}, "#ROUTE 2,1,0\r",
     device_reply="~01@ROUTE 2,1,0 ERR 003\r\n", expect_result={"error": {"error": "device_error"}})
text(K, "route_data", {"input": 2, "output": 4}, "#ROUTE 3,4,2\r")
text(K, "route_ir", {"input": 1, "output": 3}, "#ROUTE 4,3,1\r")
text(K, "route_usb", {"input": 5, "output": 6}, "#ROUTE 5,6,5\r")
text(K, "route_video_all", {"input": 3}, "#ROUTE 1,*,3\r")
text(K, "route_audio_all", {"input": 4}, "#ROUTE 2,*,4\r")
text(K, "get_route_video", {"output": 2}, "#ROUTE? 1,2\r", device_reply="~01@ROUTE 1,2,7\r\n",
     expect_result={"ok": {"kind": "value", "value": "7"}})
text(K, "get_route_audio", {"output": 2}, "#ROUTE? 2,2\r", device_reply="~01@ROUTE 2,2,5\r\n",
     expect_result={"ok": {"kind": "value", "value": "5"}})
text(K, "get_route_data", {"output": 1}, "#ROUTE? 3,1\r")
text(K, "get_route_ir", {"output": 1}, "#ROUTE? 4,1\r")
text(K, "get_route_usb", {"output": 1}, "#ROUTE? 5,1\r", device_reply="~01@ROUTE 5,1 ERR 002\r\n",
     expect_result={"error": {"error": "device_error"}})

# Legacy switching (p.51-55)
text(K, "legacy_route_video", {"input": 4, "output": 1}, "#VID 4>1\r", device_reply="~01@VID 4>1\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "legacy_route_audio", {"input": 4, "output": 1}, "#AUD 4>1\r", device_reply="~01@AV 4>1\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "legacy_route_av", {"input": 0, "output": 3}, "#AV 0>3\r")
text(K, "legacy_get_route_video", {"output": 2}, "#VID? 2\r", device_reply="~01@VID 6>2\r\n",
     expect_result={"ok": {"kind": "value", "value": "6"}})
text(K, "legacy_get_route_audio", {"output": 2}, "#AUD? 2\r", device_reply="~01@AUD 0>2\r\n",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(K, "set_audio_follow_video", {"follow": True}, "#AFV 0\r", device_reply="~01@AFV 0\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "get_audio_follow_video", {}, "#AFV?\r", device_reply="~01@AFV 1\r\n",
     expect_result={"ok": {"kind": "value", "value": "1"}})
text(K, "set_auto_switch_mode", {"layer": 1, "output": 2, "mode": 2}, "#AV-SW-MODE 1,2,2\r")
text(K, "get_auto_switch_mode", {"layer": 1, "output": 2}, "#AV-SW-MODE? 1,2\r",
     device_reply="~01@AV-SW-MODE 1,2,1\r\n", expect_result={"ok": {"kind": "value", "value": "1"}})

# Presets (p.32-34)
text(K, "store_preset", {"preset": 5}, "#PRST-STO 5\r", device_reply="~01@PRST-STO 5\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "recall_preset", {"preset": 3}, "#PRST-RCL 3\r", device_reply="~PRST-RCL 3\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "get_preset_list", {}, "#PRST-LST?\r", device_reply="~01@PRST-LST 1,2,5\r\n",
     expect_result={"ok": {"kind": "value", "value": "1,2,5"}})
text(K, "get_preset_video", {"preset": 3, "output": 2}, "#PRST-VID? 3,2\r", device_reply="~PRST-VID 3, 4>2\r\n",
     expect_result={"ok": {"kind": "value", "value": "4"}})
text(K, "get_preset_audio", {"preset": 3, "output": 2}, "#PRST-AUD? 3,2\r", device_reply="~01@PRST-AUD 3, 1>2\r\n",
     expect_result={"ok": {"kind": "value", "value": "1"}})
text(K, "set_preset_lock", {"preset": 2, "locked": True}, "#PRST-LOCK 2,ON\r", device_reply="~01@PRST-LOCK 2,ON\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "get_preset_lock", {"preset": 1}, "#PRST-LOCK? 1\r", device_reply="~01@PRST-LOCK 1,OFF\r\n",
     expect_result={"ok": {"kind": "value", "value": "OFF"}})

# Labels (p.23)
text(K, "set_input_label", {"input": 2, "label": "Laptop"}, "#LABEL 0,2,1,Laptop\r")
text(K, "set_output_label", {"output": 1, "label": "Projector"}, "#LABEL 1,1,1,Projector\r",
     device_reply="~01@LABEL 1,1,1,Projector\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "get_label", {"port": 1}, "#LABEL? 1\r", device_reply="~01@LABEL 0,1,1,Camera\r\n",
     expect_result={"ok": {"kind": "value", "value": "Camera"}})

# Mutes (p.64, p.83)
text(K, "set_video_mute", {"output": 3, "muted": True}, "#VMUTE 3,1\r", device_reply="~01@VMUTE 3,1\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "get_video_mute", {"output": 3}, "#VMUTE? 3\r", device_reply="~01@VMUTE 3,0\r\n",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(K, "set_audio_mute", {"output": 1, "muted": False}, "#MUTE 1,0\r", device_reply="~01@MUTE 1,0\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "get_audio_mute", {"output": 1}, "#MUTE? 1\r", device_reply="~01@MUTE 1,1\r\n",
     expect_result={"ok": {"kind": "value", "value": "1"}})

# Audio levels (p.73, p.76)
text(K, "set_input_audio_level", {"channel": 1, "level": -10}, "#AUD-LVL 0,1,-10\r")
text(K, "set_output_audio_level", {"channel": 2, "level": 5}, "#AUD-LVL 1,2,5\r",
     device_reply="~01@AUD-LVL 1,2,5\r\n", expect_result={"ok": {"kind": "ack"}})
text(K, "set_input_audio_level_keep_mute", {"channel": 1, "level": -80}, "#AUD-LVL 0,1,-80,1\r")
text(K, "set_output_audio_level_keep_mute", {"channel": 1, "level": 0}, "#AUD-LVL 1,1,0,1\r")
text(K, "get_input_audio_level", {"channel": 1}, "#AUD-LVL? 0,1\r", device_reply="~01@AUD-LVL 0,1,-20\r\n",
     expect_result={"ok": {"kind": "value", "value": "-20"}})
text(K, "get_output_audio_level", {"channel": 1}, "#AUD-LVL? 1,1\r", device_reply="~01@AUD-LVL 1,1,3\r\n",
     expect_result={"ok": {"kind": "value", "value": "3"}})

# Video output controls (p.62-63)
text(K, "set_freeze", {"output": 1, "frozen": True}, "#VFRZ 1,1\r")
text(K, "get_freeze", {"output": 1}, "#VFRZ? 1\r", device_reply="~01@VFRZ 1,0\r\n",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(K, "set_test_pattern", {"output": 2, "pattern": 4}, "#VID-PATTERN 2,4\r")
text(K, "get_test_pattern", {"output": 2}, "#VID-PATTERN? 2\r", device_reply="~01@VID-PATTERN 2,4\r\n",
     expect_result={"ok": {"kind": "value", "value": "4"}})

# Signal, HDCP and sink status (p.17-20, p.35, p.76)
text(K, "get_signal", {"input": 3}, "#SIGNAL? 3\r", device_reply="~01@SIGNAL 3,1\r\n",
     expect_result={"ok": {"kind": "value", "value": "1"}})
text(K, "get_audio_signal", {"input": 3}, "#AUD-SIGNAL? 3\r", device_reply="~01@AUD-SIGNAL 3,0\r\n",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(K, "get_hdcp_status_input", {"input": 1}, "#HDCP-STAT? 0,1\r", device_reply="~01@HDCP-STAT 0,1,ON\r\n",
     expect_result={"ok": {"kind": "value", "value": "ON"}})
text(K, "get_hdcp_status_output", {"output": 2}, "#HDCP-STAT? 1,2\r", device_reply="~01@HDCP-STAT 1,2,OFF\r\n",
     expect_result={"ok": {"kind": "value", "value": "OFF"}})
text(K, "set_hdcp_mode", {"input": 1, "mode": 0}, "#HDCP-MOD 1,0\r")
text(K, "get_hdcp_mode", {"input": 1}, "#HDCP-MOD? 1\r", device_reply="~01@HDCP-MOD 1,3\r\n",
     expect_result={"ok": {"kind": "value", "value": "3"}})
text(K, "get_display", {"output": 4}, "#DISPLAY? 4\r", device_reply="~01@DISPLAY 4,2\r\n",
     expect_result={"ok": {"kind": "value", "value": "2"}})

# Front panel lock, standby, power save (p.25, p.30, p.36)
text(K, "set_front_panel_lock", {"locked": True}, "#LOCK-FP 1\r", device_reply="~01@LOCK-FP 1\r\n",
     expect_result={"ok": {"kind": "ack"}})
text(K, "get_front_panel_lock", {}, "#LOCK-FP?\r", device_reply="~01@LOCK-FP 0\r\n",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(K, "set_standby", {"standby": True}, "#STANDBY 1\r")
text(K, "get_standby", {}, "#STANDBY?\r", device_reply="~01@STANDBY 0\r\n",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(K, "set_power_save", {"enabled": False}, "#POWER-SAVE 0\r")
text(K, "get_power_save", {}, "#POWER-SAVE?\r", device_reply="~01@POWER-SAVE OFF\r\n",
     expect_result={"ok": {"kind": "value", "value": "OFF"}})

# Extended Protocol 3000 (p.4-7 and each X- entry); examples from the guide.
text(K, "route_signal", {"output_signal": "OUT.HDBT.4.VIDEO.1", "input_signal": "IN.HDMI.1.VIDEO.1"},
     "#X-ROUTE OUT.HDBT.4.VIDEO.1,IN.HDMI.1.VIDEO.1\r",
     device_reply="~01@X-ROUTE OUT.HDBT.4.VIDEO.1,IN.HDMI.1.VIDEO.1\r\n", expect_result={"ok": {"kind": "ack"}}, **KX)
text(K, "get_signal_route", {"output_signal": "OUT.SDI.5.VIDEO.1"}, "#X-ROUTE? OUT.SDI.5.VIDEO.1\r",
     device_reply="~01@X-ROUTE OUT.SDI.5.VIDEO.1,IN.SDI.1.VIDEO.1\r\n",
     expect_result={"ok": {"kind": "value", "value": "IN.SDI.1.VIDEO.1"}}, **KX)
text(K, "get_matrix_status", {}, "#MATRIX-STATUS?\r",
     device_reply="~01@MATRIX-STATUS [[OUT.SDI.5.VIDEO.1,IN.SDI.1.VIDEO.1]]\r\n",
     expect_result={"ok": {"kind": "value", "value": "[[OUT.SDI.5.VIDEO.1,IN.SDI.1.VIDEO.1]]"}}, **KX)
text(K, "get_signals_list", {}, "#SIGNALS-LIST?\r", **KX)
text(K, "get_ports_list", {}, "#PORTS-LIST?\r",
     device_reply="~01@PORTS-LIST [IN.SDI.1,OUT.SDI.5]\r\n",
     expect_result={"ok": {"kind": "value", "value": "[IN.SDI.1,OUT.SDI.5]"}}, **KX)
text(K, "mute_signal", {"signal": "OUT.HDMI.1.VIDEO.1", "muted": True}, "#X-MUTE OUT.HDMI.1.VIDEO.1,ON\r",
     device_reply="~01@X-MUTE OUT.HDMI.1.VIDEO.1,ON\r\n", expect_result={"ok": {"kind": "ack"}}, **KX)
text(K, "get_signal_mute", {"signal": "OUT.ANALOG_AUDIO.1.AUDIO.1"}, "#X-MUTE? OUT.ANALOG_AUDIO.1.AUDIO.1\r",
     device_reply="~01@X-MUTE OUT.ANALOG_AUDIO.1.AUDIO.1,OFF\r\n",
     expect_result={"ok": {"kind": "value", "value": "OFF"}}, **KX)
text(K, "set_signal_audio_level", {"signal": "OUT.ANALOG_AUDIO.1.AUDIO.1", "level": -10},
     "#X-AUD-LVL OUT.ANALOG_AUDIO.1.AUDIO.1,-10\r", **KX)
text(K, "get_signal_audio_level", {"signal": "OUT.ANALOG_AUDIO.1.AUDIO.1"}, "#X-AUD-LVL? OUT.ANALOG_AUDIO.1.AUDIO.1\r",
     device_reply="~01@X-AUD-LVL OUT.ANALOG_AUDIO.1.AUDIO.1,-10.00\r\n",
     expect_result={"ok": {"kind": "value", "value": "-10.00"}}, **KX)
text(K, "set_port_label", {"port": "OUT.HDMI.5", "label": "LG-28D"}, "#X-LABEL OUT.HDMI.5,LG-28D\r",
     device_reply="~01@X-LABEL OUT.HDMI.5,LG-28D\r\n", expect_result={"ok": {"kind": "ack"}}, **KX)
text(K, "get_port_label", {"port": "OUT.HDMI.5"}, "#X-LABEL? OUT.HDMI.5\r", **KX)
text(K, "set_signal_afv", {"signal": "OUT.HDMI.1.VIDEO.1", "follow": True}, "#X-AFV OUT.HDMI.1.VIDEO.1,ON\r", **KX)
text(K, "get_signal_afv", {"signal": "OUT.HDMI.1.VIDEO.1"}, "#X-AFV? OUT.HDMI.1.VIDEO.1\r",
     device_reply="~01@X-AFV OUT.HDMI.1.VIDEO.1,ON\r\n",
     expect_result={"ok": {"kind": "value", "value": "ON"}}, **KX)
text(K, "get_signal_status", {"signal": "IN.HDMI.1.VIDEO.1"}, "#X-SIGNAL? IN.HDMI.1.VIDEO.1\r",
     device_reply="~01@ X-SIGNAL? IN.HDMI.1.VIDEO.1,1\r\n",
     expect_result={"ok": {"kind": "value", "value": "1"}}, **KX)

# Telemetry: identity asked on connect (MODEL? first); everything else is the
# device's own long-syntax message, pushed on change from any controller or the
# front panel (p.3, p.132) or answering a query.
telemetry(K, "model", expect_connect_wire=["#MODEL?\r"], inbound="~01@MODEL VS-88UT\r\n",
          expect_state={"device": {"model": "VS-88UT"}})
telemetry(K, "identity", inbound="~01@INFO-IO? IN 8,OUT 8\r\n", expect_state={"device": {"inputs": 8, "outputs": 8}})
telemetry(K, "name", inbound="~01@NAME? ROOM-A\r\n", expect_state={"device": {"name": "ROOM-A"}})
telemetry(K, "name-error", inbound="~01@NAME %66yy ERR 003\r\n", expect_state={})
telemetry(K, "protocol-start", inbound="~01@Protocol Start\r\n", expect_state={"device": {"started": True}})
telemetry(K, "route-video", inbound="~01@ROUTE 1,2,3\r\n", expect_state={"video": {"outputs": {"2": {"input": 3}}}})
telemetry(K, "route-audio", inbound="~01@ROUTE 2,4,0\r\n", expect_state={"audio": {"outputs": {"4": {"input": 0}}}})
telemetry(K, "route-usb", inbound="~01@ROUTE 5,1,2\r\n", expect_state={"usb": {"outputs": {"1": {"input": 2}}}})
telemetry(K, "switched-av", inbound="~01@AV 3>1\r\n",
          expect_state={"video": {"outputs": {"1": {"input": 3}}}, "audio": {"outputs": {"1": {"input": 3}}}})
telemetry(K, "switched-vid", inbound="~01@VID 5>2\r\n", expect_state={"video": {"outputs": {"2": {"input": 5}}}})
telemetry(K, "switched-aud", inbound="~01@AUD 6>2\r\n", expect_state={"audio": {"outputs": {"2": {"input": 6}}}})
telemetry(K, "afv", inbound="~01@AFV 1\r\n", expect_state={"audio_follow_video": False})
telemetry(K, "preset-recalled", inbound="~01@PRST-RCL 3\r\n", expect_state={"presets": {"last_recalled": 3}})
telemetry(K, "preset-lock", inbound="~01@PRST-LOCK 2,ON\r\n", expect_state={"presets": {"2": {"locked": True}}})
telemetry(K, "label", inbound="~01@LABEL 1,2,1,Projector\r\n", expect_state={"outputs": {"2": {"label": "Projector"}}})
telemetry(K, "video-mute", inbound="~01@VMUTE 3,1\r\n", expect_state={"outputs": {"3": {"video_mute": True}}})
telemetry(K, "audio-mute", inbound="~01@MUTE 1,0\r\n", expect_state={"outputs": {"1": {"audio_mute": False}}})
telemetry(K, "audio-level", inbound="~01@AUD-LVL 1,2,-12\r\n", expect_state={"audio": {"outputs": {"2": {"level": -12.0}}}})
telemetry(K, "signal", inbound="~01@SIGNAL 4,0\r\n", expect_state={"inputs": {"4": {"signal": False}}})
telemetry(K, "hdcp", inbound="~01@HDCP-STAT 0,1,ON\r\n", expect_state={"inputs": {"1": {"hdcp": True}}})
telemetry(K, "display", inbound="~01@DISPLAY 2,2\r\n", expect_state={"outputs": {"2": {"display": 2}}})
telemetry(K, "front-panel-lock", inbound="~01@LOCK-FP 1\r\n", expect_state={"front_panel_locked": True})
telemetry(K, "standby", inbound="~01@STANDBY 1\r\n", expect_state={"standby": True})
telemetry(K, "x-route", inbound="~01@X-ROUTE OUT.HDBT.4.VIDEO.1,IN.HDMI.1.VIDEO.1\r\n",
          expect_state={"signals": {"OUT": {"HDBT": {"4": {"VIDEO": {"1": {"source": "IN.HDMI.1.VIDEO.1"}}}}}}})
telemetry(K, "x-mute", inbound="~01@X-MUTE OUT.ANALOG_AUDIO.1.AUDIO.1,OFF\r\n",
          expect_state={"signals": {"OUT": {"ANALOG_AUDIO": {"1": {"AUDIO": {"1": {"mute": False}}}}}}})
telemetry(K, "x-audio-level", inbound="~01@X-AUD-LVL OUT.ANALOG_AUDIO.1.AUDIO.1,-10.00\r\n",
          expect_state={"signals": {"OUT": {"ANALOG_AUDIO": {"1": {"AUDIO": {"1": {"level": -10.0}}}}}}})

# EDID, multiviewer and scaler, step-in, I/O gateway, streamer, network, time
# and audio DSP (guide p.16-131). Wire forms from the guide's syntax boxes and
# examples, "#CMD p1,p2" + CR; replies "~nn@CMD p1,p2" + CR LF.
_OK = {"ok": {"kind": "ack"}}


def _kv(x):
    return {"ok": {"kind": "value", "value": x}}


_FULL = [
    ("copy_edid", {"src_type": 1, "src_id": 1, "dst_type": 0, "dest_bitmap": "0x13", "safe_mode": True},
     "#CPEDID 1,1,0,0x13,1", "~01@CPEDID 1,1,0,0x13,1", _OK),
    ("get_edid_size", {"stage": 0, "stage_id": 1}, "#GEDID? 0,1", "~01@GEDID 0,1,256", _kv("256")),
    ("set_edid_audio", {"input": 1, "format": 2}, "#EDID-AUDIO 1,2", "~01@EDID-AUDIO 1,2", _OK),
    ("get_edid_audio", {"input": 1}, "#EDID-AUDIO? 1", "~01@EDID-AUDIO 1,2", _kv("2")),
    ("set_edid_color_space", {"input": 2, "color_space": 4}, "#EDID-CS 2,4", "~01@EDID-CS 2,4", _OK),
    ("get_edid_color_space", {"input": 2}, "#EDID-CS? 2", "~01@EDID-CS 2,0", _kv("0")),
    ("set_edid_deep_color", {"input": 1, "remove": True}, "#EDID-DC 1,1", "~01@EDID-DC 1,1", _OK),
    ("get_edid_deep_color", {"input": 1}, "#EDID-DC? 1", "~01@EDID-DC 1,1", _kv("1")),
    ("set_edid_lock", {"input": 2, "locked": False}, "#LOCK-EDID 2,0", "~01@LOCK-EDID 2,0", _OK),
    ("get_edid_lock", {"input": 2}, "#LOCK-EDID? 2", "~01@LOCK-EDID 2,ON", _kv("ON")),
    ("set_view_mode", {"mode": 3}, "#VIEW-MOD 3", "~01@VIEW-MOD 3", _OK),
    ("get_view_mode", {}, "#VIEW-MOD?", "~01@VIEW-MOD 3", _kv("3")),
    ("set_window_source", {"window": 2, "value": 4}, "#W-SRC 2,4", "~01@W-SRC 2,4", _OK),
    ("get_window_source", {"window": 2}, "#W-SRC? 2", "~01@W-SRC 2,4", _kv("4")),
    ("set_window_layer", {"window": 1, "value": 4}, "#W-LAYER 1,4", "~01@W-LAYER 1,4", _OK),
    ("get_window_layer", {"window": 1}, "#W-LAYER? 1", "~01@W-LAYER 1,4", _kv("4")),
    ("set_window_color", {"window": 1, "value": 2}, "#W-COLOR 1,2", "~01@W-COLOR 1,2", _OK),
    ("get_window_color", {"window": 1}, "#W-COLOR? 1", "~01@W-COLOR 1,2", _kv("2")),
    ("set_window_hue", {"window": 1, "value": 50}, "#W-HUE 1,50", "~01@W-HUE 1,50", _OK),
    ("get_window_hue", {"window": 1}, "#W-HUE? 1", "~01@W-HUE 1,50", _kv("50")),
    ("set_window_saturation", {"window": 3, "value": 60}, "#W-SATURATION 3,60", "~01@W-SATURATION 3,60", _OK),
    ("get_window_saturation", {"window": 3}, "#W-SATURATION? 3", "~01@W-SATURATION 3,60", _kv("60")),
    ("set_window_sharpness", {"window": 3, "value": 10}, "#W-SHARP 3,10", "~01@W-SHARP 3,10", _OK),
    ("get_window_sharpness", {"window": 3}, "#W-SHARP? 3", "~01@W-SHARP 3,10", _kv("10")),
    ("set_window_zoom", {"window": 1, "value": 150}, "#W-ZOOM 1,150", "~01@W-ZOOM 1,150", _OK),
    ("get_window_zoom", {"window": 1}, "#W-ZOOM? 1", "~01@W-ZOOM 1,150", _kv("150")),
    ("set_brightness", {"window": 1, "value": 55}, "#BRIGHTNESS 1,55", "~01@BRIGHTNESS 1,55", _OK),
    ("get_brightness", {"window": 1}, "#BRIGHTNESS? 1", "~01@BRIGHTNESS 1,55", _kv("55")),
    ("set_contrast", {"window": 2, "value": 45}, "#CONTRAST 2,45", "~01@CONTRAST 2,45", _OK),
    ("get_contrast", {"window": 2}, "#CONTRAST? 2", "~01@CONTRAST 2,45", _kv("45")),
    ("set_window_enabled", {"window": 4, "enabled": False}, "#W-ENABLE 4,0", "~01@W-ENABLE 4,0", _OK),
    ("get_window_enabled", {"window": 4}, "#W-ENABLE? 4", "~01@W-ENABLE 4,0", _kv("0")),
    ("set_window_freeze", {"window": 1, "enabled": True}, "#W-FRZ 1,1", "~01@W-FRZ 1,1", _OK),
    ("get_window_freeze", {"window": 1}, "#W-FRZ? 1", "~01@W-FRZ 1,1", _kv("1")),
    ("set_window_border", {"window": 1, "enabled": True}, "#WND-BRD 1,1", "~01@WND-BRD 1,1", _OK),
    ("get_window_border", {"window": 1}, "#WND-BRD? 1", "~01@WND-BRD 1,1", _kv("1")),
    ("set_window_position", {"window": 1, "x": 205, "y": 117, "width": 840, "height": 472}, "#W-POS 1,205,117,840,472",
     "~01@W-POS 1,205,117,840,472", _OK),
    ("get_window_position", {"window": 1}, "#W-POS? 1", "~01@W-POS 1,205,117,840,472", _kv("205,117,840,472")),
    ("set_image_mode", {"scaler": 1, "mode": 2}, "#IMAGE-PROP 1,2", "~01@IMAGE-PROP 1,2", _OK),
    ("get_image_mode", {"scaler": 1}, "#IMAGE-PROP? 1", "~01@IMAGE-PROP 1,2", _kv("2")),
    ("set_auto_sync", {"scaler": 1, "mode": 1}, "#SCLR-AS 1,1", "~01@SCLR-AS 1,1", _OK),
    ("get_auto_sync", {"scaler": 1}, "#SCLR-AS? 1", "~01@SCLR-AS 1,1", _kv("1")),
    ("set_scaler_audio_delay", {"output": 1, "delay": 2}, "#SCLR-AUDIO-DELAY 1,2", "~01@SCLR-AUDIO-DELAY 1,2", _OK),
    ("get_scaler_audio_delay", {"output": 1}, "#SCLR-AUDIO-DELAY? 1", "~01@SCLR-AUDIO-DELAY 1,2", _kv("2")),
    ("scaler_pc_auto_adjust", {"scaler": 1}, "#SCLR-PCAUTO 1,1", "~01@SCLR-PCAUTO 1,1", _OK),
    ("set_osd", {"id": 1, "mode": 2}, "#SHOW-OSD 1,2", "~01@SHOW-OSD 1,2", _OK),
    ("get_osd", {"id": 1}, "#SHOW-OSD? 1", "~01@SHOW-OSD 1,0", _kv("0")),
    ("set_output_resolution", {"output": 1, "resolution": 225}, "#VID-RES 1,1,0,225", "~01@VID-RES 1,1,0,225", _OK),
    ("get_output_resolution", {"output": 1}, "#VID-RES? 1,1,0", "~01@VID-RES 1,1,0,225", _kv("225")),
    ("get_input_resolution", {"input": 2}, "#VID-RES? 0,2,0", "~01@VID-RES 0,2,0,16", _kv("16")),
    ("set_stepin_button", {"button": 1, "mode": 1}, "#BTN 1,1", "~01@BTN 1,1", _OK),
    ("get_stepin_button", {"button": 1}, "#BTN? 1", "~01@BTN 1,255", _kv("255")),
    ("get_stepin_capabilities", {}, "#STEPIN-CP?", "~01@STEPIN-CP 1,4,4", _kv("1,4,4")),
    ("set_gpio_config", {"pin": 1, "type": 1, "direction": 1, "pullup": False}, "#GPIO-CFG 1,1,1,0", "~01@GPIO-CFG 1,1,1", _OK),
    ("get_gpio_config", {"pin": 1}, "#GPIO-CFG? 1", "~01@GPIO-CFG 1,1,0", _kv("1,0")),
    ("set_gpio_state", {"pin": 2, "state": 1}, "#GPIO-STATE 2,1", "~01@GPIO-STATE 2,1", _OK),
    ("get_gpio_state", {"pin": 2}, "#GPIO-STATE? 2", "~01@GPIO-STATE 2,1", _kv("1")),
    ("set_gpio_thresholds", {"pin": 3, "low_mv": 1000, "high_mv": 4000}, "#GPIO-THR 3,1000,4000", "~01@GPIO-THR 3,1000,4000", _OK),
    ("get_gpio_thresholds", {"pin": 3}, "#GPIO-THR? 3", "~01@GPIO-THR 3,1000,4000", _kv("1000,4000")),
    ("get_gpio_voltage", {"pin": 3}, "#GPIO-VOLT? 3", "~01@GPIO-VOLT 3,4800", _kv("4800")),
    ("set_relay", {"relay": 1, "closed": True}, "#RELAY-STATE 1,1", "~01@RELAY-STATE 1,1", _OK),
    ("get_relay", {"relay": 1}, "#RELAY-STATE? 1", "~01@RELAY-STATE 1,1", _kv("1")),
    ("set_uart", {"port": 1, "baud": "9600", "data_bits": 8, "parity": 0, "stop_bits": "1", "serial_type": 0},
     "#UART 1,9600,8,0,1,0", "~01@UART 1,9600,8,0,1,0", _OK),
    ("get_uart", {"port": 1}, "#UART? 1", "~01@UART 1,9600,8,0,1,0", _kv("9600,8,0,1,0")),
    ("set_baud", {"rate": "115200"}, "#BAUD 115200", "~01@BAUD 115200", _OK),
    ("get_baud", {}, "#BAUD?", "~01@BAUD 115200", _kv("115200")),
    ("set_port_direction", {"port": 5, "direction": "OUT"}, "#PORT-DIRECTION 5,OUT", "~01@PORT-DIRECTION 5,OUT", _OK),
    ("get_port_direction", {"port": 5}, "#PORT-DIRECTION? 5", "~01@PORT-DIRECTION 5,OUT", _kv("OUT")),
    ("set_com_route", {"com": 1, "port_type": 0, "eth_port": 5001, "replies": True, "keepalive": 60},
     "#COM-ROUTE 1,0,5001,1,60", "~01@COM-ROUTE 1,0,5001,1,60", _OK),
    ("get_com_route", {"com": 1}, "#COM-ROUTE? 1", "~01@COM-ROUTE 1,0,5001,1,60", _kv("0,5001,1,60")),
    ("remove_com_route", {"com": 1}, "#COM-ROUTE-REMOVE 1", "~01@COM-ROUTE-REMOVE 1", _OK),
    ("get_eth_tunnel", {"id": 1}, "#ETH-TUNNEL? 1", "~01@ETH-TUNNEL 1,1,0,5001,192.168.1.10,6000,1,1",
     _kv("1,1,0,5001,192.168.1.10,6000,1,1")),
    ("set_kds_action", {"value": 1}, "#KDS-ACTION 1", "~01@KDS-ACTION 1", _OK),
    ("get_kds_action", {}, "#KDS-ACTION?", "~01@KDS-ACTION 1", _kv("1")),
    ("set_kds_audio", {"value": 0}, "#KDS-AUD 0", "~01@KDS-AUD 0", _OK),
    ("get_kds_audio", {}, "#KDS-AUD?", "~01@KDS-AUD 0", _kv("0")),
    ("set_kds_encoding", {"value": 0}, "#KDS-EN 0", "~01@KDS-EN 0", _OK),
    ("get_kds_encoding", {}, "#KDS-EN?", "~01@KDS-EN 0", _kv("0")),
    ("set_kds_latency", {"value": 10}, "#KDS-LATENCY 10", "~01@KDS-LATENCY 10", _OK),
    ("get_kds_latency", {}, "#KDS-LATENCY?", "~01@KDS-LATENCY 10", _kv("10")),
    ("set_kds_scale", {"value": 1}, "#KDS-SCALE 1", "~01@KDS-SCALE 1", _OK),
    ("get_kds_scale", {}, "#KDS-SCALE?", "~01@KDS-SCALE 1", _kv("1")),
    ("set_kds_connection", {"address": "192.168.1.39", "port": 554, "folder": "live"}, "#KDS-CONN 192.168.1.39,554,live",
     "~01@KDS-CONN 192.168.1.39,554,live", _OK),
    ("get_kds_connection", {}, "#KDS-CONN?", "~01@KDS-CONN 192.168.1.39,554,live", _kv("192.168.1.39,554,live")),
    ("get_kds_status", {}, "#KDS-OP-STAT?", "~01@KDS-OP-STAT 0", _kv("0")),
    ("get_kds_bitrate", {}, "#KDS-BR?", "~01@KDS-BR 8000", _kv("8000")),
    ("get_kds_frame_rate", {}, "#KDS-FR?", "~01@KDS-FR 30", _kv("30")),
    ("get_kds_clients", {}, "#KDS-ACTIVE-CLNT?", "~01@KDS-ACTIVE-CLNT 2", _kv("2")),
    ("get_kds_gop", {}, "#KDS-GOP?", "~01@KDS-GOP 30", _kv("30")),
    ("get_kds_method", {}, "#KDS-METHOD?", "~01@KDS-METHOD 1", _kv("1")),
    ("get_kds_protocol", {}, "#KDS-PROT?", "~01@KDS-PROT RTSP", _kv("RTSP")),
    ("get_kds_mode", {}, "#KDS-MOD?", "~01@KDS-MOD 3", _kv("3")),
    ("set_dhcp", {"mode": 1}, "#NET-DHCP 1", "~01@NET-DHCP 1", _OK),
    ("get_dhcp", {}, "#NET-DHCP?", "~01@NET-DHCP 0", _kv("0")),
    ("set_ip_address", {"value": "192.168.1.39"}, "#NET-IP 192.168.1.39", "~01@NET-IP 192.168.1.39", _OK),
    ("get_ip_address", {}, "#NET-IP?", "~01@NET-IP 192.168.001.039", _kv("192.168.001.039")),
    ("set_netmask", {"value": "255.255.0.0"}, "#NET-MASK 255.255.0.0", "~01@NET-MASK 255.255.0.0", _OK),
    ("get_netmask", {}, "#NET-MASK?", "~01@NET-MASK 255.255.000.000", _kv("255.255.000.000")),
    ("set_gateway", {"value": "192.168.1.1"}, "#NET-GATE 192.168.1.1", "~01@NET-GATE 192.168.1.1", _OK),
    ("get_gateway", {}, "#NET-GATE?", "~01@NET-GATE 192.168.000.001", _kv("192.168.000.001")),
    ("set_net_config", {"id": 0, "address": "192.168.1.39", "netmask": "255.255.255.0", "gateway": "192.168.1.1"},
     "#NET-CONFIG 0,192.168.1.39,255.255.255.0,192.168.1.1", "~01@NET-CONFIG 0,192.168.1.39,255.255.255.0,192.168.1.1", _OK),
    ("get_net_config", {"id": 0}, "#NET-CONFIG? 0", "~01@NET-CONFIG 0,192.168.1.39,255.255.255.0,192.168.1.1",
     _kv("192.168.1.39,255.255.255.0,192.168.1.1")),
    ("get_mac_address", {}, "#NET-MAC?", "~01@NET-MAC 00-1D-56-00-12-34", _kv("00-1D-56-00-12-34")),
    ("get_dns", {"id": 0}, "#NET-DNS? 0", "~01@NET-DNS 0,8.8.8.8", _kv("8.8.8.8")),
    ("set_control_port", {"protocol": "TCP", "port": 5000}, "#ETH-PORT TCP,5000", "~01@ETH-PORT TCP,5000", _OK),
    ("get_control_port", {"protocol": "UDP"}, "#ETH-PORT? UDP", "~01@ETH-PORT UDP,50000", _kv("50000")),
    ("set_time", {"day": "TUE", "date": "25-04-2017", "time": "16:30:00"}, "#TIME TUE,25-04-2017,16:30:00",
     "~01@TIME TUE,25-04-2017,16:30:00", _OK),
    ("get_time", {}, "#TIME?", "~01@TIME TUE,25-04-2017,16:30:00", _kv("TUE,25-04-2017,16:30:00")),
    ("set_time_location", {"utc_offset": 2, "dst": True}, "#TIME-LOC 2,1", "~01@TIME-LOC 2,1", _OK),
    ("get_time_location", {}, "#TIME-LOC?", "~01@TIME-LOC 2,1", _kv("2,1")),
    ("set_time_server", {"enabled": True, "address": "128.138.140.44", "sync_hour": 0}, "#TIME-SRV 1,128.138.140.44,0",
     "~01@TIME-SRV 1,128.138.140.44,0", _OK),
    ("get_time_server", {}, "#TIME-SRV?", "~01@TIME-SRV 1,128.138.140.44,0,ON", _kv("1,128.138.140.44,0,ON")),
    ("set_audio_embedding", {"input": 1, "output": 0, "mode": 1}, "#AUD-EMB 1,0,1", "~01@AUD-EMB 1,0,1", _OK),
    ("get_audio_embedding", {"input": 1, "output": 0}, "#AUD-EMB? 1,0", "~01@AUD-EMB 1,0,1", _kv("1")),
    ("set_audio_filter", {"channel": 1, "type": 0, "frequency": 3, "enabled": True}, "#AUD-FILTER 1,0,3,1",
     "~01@AUD-FILTER 1,0,3,1", _OK),
    ("get_audio_filter", {"channel": 1}, "#AUD-FILTER? 1", "~01@AUD-FILTER 1,0,3,1", _kv("0,3,1")),
    ("set_audio_mix", {"channel": 1, "knob": 2, "level": -20}, "#AUD-MIX 1,2,-20", "~01@AUD-MIX 1,2,-20", _OK),
    ("get_audio_mix", {"channel": 1, "knob": 2}, "#AUD-MIX? 1,2", "~01@AUD-MIX 1,2,-20", _kv("-20")),
    ("set_eq_frequency", {"stage": 1, "channel": 1, "band": 0, "index": 2}, "#EQ-FREQ 1,1,0,2", "~01@EQ-FREQ 1,1,0,2", _OK),
    ("get_eq_frequency", {"stage": 1, "channel": 1, "band": 0}, "#EQ-FREQ? 1,1,0", "~01@EQ-FREQ 1,1,0,2", _kv("2")),
    ("set_eq_level", {"stage": 1, "channel": 1, "band": 2, "level": 25}, "#EQ-LVL 1,1,2,25", "~01@EQ-LVL 1,1,2,25", _OK),
    ("get_eq_level", {"stage": 1, "channel": 1, "band": 2}, "#EQ-LVL? 1,1,2", "~01@EQ-LVL 1,1,2,25", _kv("25")),
    ("set_eq_q", {"channel": 1, "band": 1, "q": 7}, "#EQ-Q 1,1,7", "~01@EQ-Q 1,1,7", _OK),
    ("get_eq_q", {"channel": 1, "band": 1}, "#EQ-Q? 1,1", "~01@EQ-Q 1,1,7", _kv("7")),
    ("set_mic_gain", {"input": 0, "level": 70}, "#MIC-GAIN 0,70", "~01@MIC-GAIN 0,70", _OK),
    ("get_mic_gain", {"input": 0}, "#MIC-GAIN? 0", "~01@MIC-GAIN 0,70", _kv("70")),
    ("set_talkover_parameter", {"channel": 0, "parameter": 0, "value": 80}, "#MIC-TLK 0,0,80", "~01@MIC-TLK 0,0,80", _OK),
    ("get_talkover_parameter", {"channel": 0, "parameter": 0}, "#MIC-TLK? 0,0", "~01@MIC-TLK 0,0,80", _kv("80")),
    ("set_talkover_mode", {"channel": 1, "mode": 2}, "#TLK 1,2", "~01@TLK 1,2", _OK),
    ("get_talkover_mode", {"channel": 1}, "#TLK? 1", "~01@TLK 1,2", _kv("2")),
    ("set_bass", {"channel": 1, "value": 15}, "#BASS 1,15", "~01@BASS 1,15", _OK),
    ("get_bass", {"channel": 1}, "#BASS? 1", "~01@BASS 1,15", _kv("15")),
    ("set_treble", {"channel": 1, "value": 12}, "#TREBLE 1,12", "~01@TREBLE 1,12", _OK),
    ("get_treble", {"channel": 1}, "#TREBLE? 1", "~01@TREBLE 1,12", _kv("12")),
    ("set_midrange", {"channel": 1, "value": -3}, "#MIDRANGE 1,-3", "~01@MIDRANGE 1,-3", _OK),
    ("get_midrange", {"channel": 1}, "#MIDRANGE? 1", "~01@MIDRANGE 1,-3", _kv("-3")),
    ("set_balance", {"channel": 1, "value": -10}, "#BALANCE 1,-10", "~01@BALANCE 1,-10", _OK),
    ("get_balance", {"channel": 1}, "#BALANCE? 1", "~01@BALANCE 1,-10", _kv("-10")),
    ("set_loudness", {"channel": 1, "enabled": True}, "#LOUDNESS 1,1", "~01@LOUDNESS 1,1", _OK),
    ("get_loudness", {"channel": 1}, "#LOUDNESS? 1", "~01@LOUDNESS 1,1", _kv("1")),
    ("set_mic_delay", {"input": 1, "ms": 40}, "#MIC-DELAY 1,40", "~01@MIC-DELAY 1,40", _OK),
    ("get_mic_delay", {"input": 1}, "#MIC-DELAY? 1", "~01@MIC-DELAY 1,40", _kv("40")),
]
for _name, _input, _wire, _reply, _result in _FULL:
    text(K, _name, _input, _wire + "\r", device_reply=_reply + "\r\n", expect_result=_result)
text(K, "set_edid_lock", {"input": 9, "locked": True}, "#LOCK-EDID 9,1\r", device_reply="~01@LOCK-EDID 9,1 ERR 003\r\n",
     expect_result={"error": {"error": "device_error"}}, file="set_edid_lock-error")

telemetry(K, "edid-audio", inbound="~01@EDID-AUDIO 1,2\r\n", expect_state={"edid": {"inputs": {"1": {"audio_format": 2}}}})
telemetry(K, "edid-lock", inbound="~01@LOCK-EDID 2,ON\r\n", expect_state={"edid": {"inputs": {"2": {"locked": True}}}})
telemetry(K, "view-mode", inbound="~01@VIEW-MOD 3\r\n", expect_state={"view_mode": 3})
telemetry(K, "window-source", inbound="~01@W-SRC 2,4\r\n", expect_state={"windows": {"2": {"source": 4}}})
telemetry(K, "window-enabled", inbound="~01@W-ENABLE 4,0\r\n", expect_state={"windows": {"4": {"enabled": False}}})
telemetry(K, "window-position", inbound="~01@W-POS 1,205,117,840,472\r\n",
          expect_state={"windows": {"1": {"position": "205,117,840,472"}}})
telemetry(K, "image-mode", inbound="~01@IMAGE-PROP 1,2\r\n", expect_state={"scalers": {"1": {"image_mode": 2}}})
telemetry(K, "output-resolution", inbound="~01@VID-RES 1,1,0,225\r\n", expect_state={"outputs": {"1": {"resolution": 225}}})
telemetry(K, "gpio-state", inbound="~01@GPIO-STATE 2,1\r\n", expect_state={"gpio": {"2": {"state": 1}}})
telemetry(K, "gpio-config", inbound="~01@GPIO-CFG 1,1,0\r\n", expect_state={"gpio": {"1": {"type": 1, "direction": 0}}})
telemetry(K, "relay", inbound="~01@RELAY-STATE 1,1\r\n", expect_state={"relays": {"1": {"closed": True}}})
telemetry(K, "uart", inbound="~01@UART 1,9600,8,0,1,0\r\n", expect_state={"serial": {"1": {"settings": "9600,8,0,1,0"}}})
telemetry(K, "streamer-status", inbound="~01@KDS-OP-STAT 2\r\n", expect_state={"streamer": {"status": 2}})
telemetry(K, "streamer-connection", inbound="~01@KDS-CONN 192.168.1.39,554,live\r\n",
          expect_state={"streamer": {"connection": "192.168.1.39,554,live"}})
telemetry(K, "dhcp", inbound="~01@NET-DHCP 1\r\n", expect_state={"network": {"dhcp": True}})
telemetry(K, "ip", inbound="~01@NET-IP 192.168.001.039\r\n", expect_state={"network": {"address": "192.168.001.039"}})
telemetry(K, "net-config", inbound="~01@NET-CONFIG 0,192.168.1.39,255.255.255.0,192.168.1.1\r\n",
          expect_state={"network": {"interfaces": {"0": {"address": "192.168.1.39", "netmask": "255.255.255.0",
                                                         "gateway": "192.168.1.1"}}}})
telemetry(K, "mac", inbound="~01@NET-MAC 00-1D-56-00-12-34\r\n", expect_state={"network": {"mac": "00-1D-56-00-12-34"}})
telemetry(K, "control-port", inbound="~01@ETH-PORT TCP,5000\r\n", expect_state={"network": {"control_ports": {"TCP": 5000}}})
telemetry(K, "time", inbound="~01@TIME TUE,25-04-2017,16:30:00\r\n", expect_state={"clock": {"time": "TUE,25-04-2017,16:30:00"}})
telemetry(K, "time-server", inbound="~01@TIME-SRV 1,128.138.140.44,0,ON\r\n",
          expect_state={"clock": {"server": "1,128.138.140.44,0,ON"}})
telemetry(K, "audio-embedding", inbound="~01@AUD-EMB 1,0,1\r\n", expect_state={"audio": {"embedding": {"1": {"0": 1}}}})
telemetry(K, "eq-level", inbound="~01@EQ-LVL 1,1,2,25\r\n", expect_state={"audio": {"eq": {"1": {"1": {"2": {"level": 25}}}}}})
telemetry(K, "talkover", inbound="~01@MIC-TLK 0,0,80\r\n", expect_state={"audio": {"talkover": {"0": {"0": 80}}}})
telemetry(K, "mix", inbound="~01@AUD-MIX 1,2,-20\r\n", expect_state={"audio": {"channels": {"1": {"mix": {"2": -20.0}}}}})
telemetry(K, "bass", inbound="~01@BASS 1,15\r\n", expect_state={"audio": {"channels": {"1": {"bass": 15}}}})
telemetry(K, "edid-error-not-state", inbound="~01@LOCK-EDID 9,1 ERR 003\r\n", expect_state={})

# ── Product-specific commands, from each product's user manual: MV-4X
# (2900-301566 Rev 5, p.60-70), KDS-EN7/DEC7 (2900-301543 Rev 9, p.86-103),
# DSP-62-AEC/UC (2900-301432 Rev 3, p.59-83). The manuals write no space
# after the command name; Protocol 3000 puts one there (guide p.2). ──
_ACKR = {"ok": {"kind": "ack"}}
_MV = {"model": "mv-4x"}
_EN = {"model": "kds-en7"}
_DE = {"model": "kds-dec7"}
_DS = {"model": "dsp-62-aec"}
_A2 = "IN.ANALOG_AUDIO.2.AUDIO.1"
_A3 = "IN.ANALOG_AUDIO.3.AUDIO.1"
_OS = "OUT.ANALOG_STEREO.1.AUDIO.1"
_PRODUCT = [
    # MV-4X
    ("set_rotation", {"output": 1, "input": 1, "angle": 3}, "#ROTATE 1,1,3", _MV),
    ("get_rotation", {"output": 1, "input": 3}, "#ROTATE? 1,3", _MV),
    ("reset_window", {"window": 2}, "#RSTWIN 2", _MV),
    ("reset_window_parameters", {"window": 4}, "#W-P-DEFAULT 4", _MV),
    ("set_window_sharpness_h", {"window": 1, "value": 60}, "#W-SHARP-H 1,60", _MV),
    ("get_window_sharpness_h", {"window": 1}, "#W-SHARP-H? 1", _MV),
    ("set_window_sharpness_v", {"window": 3, "value": 40}, "#W-SHARP-V 3,40", _MV),
    ("get_window_sharpness_v", {"window": 3}, "#W-SHARP-V? 3", _MV),
    ("set_io_hdcp_mode", {"io": 1, "port": 2, "mode": 3}, "#HDCP-MOD 1,2,3", _MV),
    ("get_io_hdcp_mode", {"io": 0, "port": 4}, "#HDCP-MOD? 0,4", _MV),
    # KDS-EN7 / KDS-DEC7
    ("set_kds_channel", {"channel": 1}, "#KDS-CHANNEL-SELECT [video,audio,rs232,ir,usb,cec],1", _DE),
    ("set_kds_signal_channel", {"signal": "ir", "channel": 12}, "#KDS-CHANNEL-SELECT [ir],12", _DE),
    ("get_kds_channel", {"signal": "video"}, "#KDS-CHANNEL-SELECT? video", _DE),
    ("set_kds_define_channel", {"channel": 7}, "#KDS-DEFINE-CHANNEL 7", _EN),
    ("get_kds_define_channel", {}, "#KDS-DEFINE-CHANNEL?", _EN),
    ("set_kds_method", {"method": 2}, "#KDS-METHOD 2", _EN),
    ("set_kds_multicast", {"group": "0.0.0.0", "ttl": 64}, "#KDS-MULTICAST 0.0.0.0,64", _EN),
    ("get_kds_multicast", {}, "#KDS-MULTICAST?", _DE),
    ("set_kds_multicast_prefix", {"prefix": 224}, "#KDS-MULTICAST-PREFIX 224", _EN),
    ("get_kds_multicast_prefix", {}, "#KDS-MULTICAST-PREFIX?", _DE),
    ("get_kds_ratio", {}, "#KDS-RATIO?", _EN),
    ("get_kds_resolution", {"io": 0, "port": 1, "native": False}, "#KDS-RESOL? 0,1,0", _EN),
    ("set_kds_vlan_tag", {"gateway": 1, "tag": 33}, "#KDS-VLAN-TAG 1,33", _EN),
    ("get_kds_vlan_tag", {"gateway": 1}, "#KDS-VLAN-TAG? 1", _EN),
    ("set_service_vlan", {"service": 1, "enabled": True, "vlan": 100, "cos": 5}, "#NET-SRV-802-1Q 1,1,100,5", _DE),
    ("get_service_vlan", {"service": 0}, "#NET-SRV-802-1Q? 0", _DE),
    ("set_service_dscp", {"service": 0, "dscp": 46}, "#NET-SRV-DSCP 0,46", _EN),
    ("get_service_dscp", {"service": 1}, "#NET-SRV-DSCP? 1", _EN),
    ("set_video_wall_grid", {"columns": 2, "rows": 2}, "#VIEW-MOD 15,2,2", _DE),
    ("set_video_wall_mosaic", {"x1": 0, "y1": 0, "x2": 5000, "y2": 5000}, "#VIEW-MOD 16,0,0,5000,5000", _DE),
    ("set_video_wall_position", {"position": 3, "rotation": 2}, "#VIDEO-WALL-SETUP 3,2", _DE),
    ("get_video_wall_position", {}, "#VIDEO-WALL-SETUP?", _DE),
    ("set_video_wall_bezel", {"vw": 1180, "ow": 1210, "vh": 664, "oh": 694}, "#KDS-VW-BEZEL 1180,1210,664,694", _DE),
    ("get_video_wall_bezel", {}, "#KDS-VW-BEZEL?", _DE),
    ("set_video_wall_pattern", {"enabled": True}, "#KDS-VW-PATTERN 1", _DE),
    ("get_video_wall_pattern", {}, "#KDS-VW-PATTERN?", _DE),
    ("set_video_wall_stretch", {"output": 1, "mode": 1}, "#WND-STRETCH 1,1", _DE),
    ("get_video_wall_stretch", {"output": 1}, "#WND-STRETCH? 1", _DE),
    ("set_kds_osd_display", {"mode": 2}, "#KDS-OSD-DISPLAY 2", _DE),
    ("get_kds_osd_display", {}, "#KDS-OSD-DISPLAY?", _DE),
    ("set_kds_daisy_chain", {"enabled": False}, "#KDS-DAISY-CHAIN 0", _DE),
    ("get_kds_daisy_chain", {}, "#KDS-DAISY-CHAIN?", _DE),
    ("set_kds_sleep_image", {"enabled": True}, "#KDS-SLEEP-IMAGE 1", _DE),
    ("get_kds_sleep_image", {}, "#KDS-SLEEP-IMAGE?", _DE),
    ("set_hdcp_output_mode", {"output": 1, "mode": 0}, "#HDCP-OUT 1,0", _EN),
    ("get_hdcp_output_mode", {"output": 1}, "#HDCP-OUT? 1", _DE),
    ("set_color_space_convert", {"output": 1, "mode": 1}, "#CS-CONVERT 1,1", _DE),
    ("get_color_space_convert", {"output": 1}, "#CS-CONVERT? 1", _DE),
    ("set_interface_name", {"interface": 0, "name": "LOBBY-DEC"}, "#NAME 0,LOBBY-DEC", _DE),
    ("get_interface_name", {"interface": 0}, "#NAME? 0", _EN),
    # DSP-62
    ("set_dsp_aec", {"field": "bypass", "signal": _A3, "value": "0"}, f"#DSP-AEC bypass,{_A3},0", _DS),
    ("get_dsp_aec", {"field": "low-freq", "signal": _A3}, f"#DSP-AEC? low-freq,{_A3}", _DS),
    ("set_dsp_compressor", {"field": "attack", "signal": _A3, "value": "15"}, f"#DSP-COMP attack,{_A3},15", _DS),
    ("get_dsp_compressor", {"field": "attack", "signal": _A3}, f"#DSP-COMP? attack,{_A3}", _DS),
    ("set_dsp_delay", {"field": "delay", "signal": "OUT.USB_B.1.AUDIO.1", "value": "75"},
     "#DSP-DELAY delay,OUT.USB_B.1.AUDIO.1,75", _DS),
    ("get_dsp_delay", {"field": "bypass", "signal": "IN.HDMI_AUDIO.1.AUDIO.1"},
     "#DSP-DELAY? bypass,IN.HDMI_AUDIO.1.AUDIO.1", _DS),
    ("set_dsp_expander", {"field": "attack", "signal": _A2, "value": "50"}, f"#DSP-EXP attack,{_A2},50", _DS),
    ("get_dsp_expander", {"field": "attack", "signal": _A2}, f"#DSP-EXP? attack,{_A2}", _DS),
    ("set_dsp_high_pass", {"field": "bypass", "signal": _A2, "value": "0"}, f"#DSP-HPF bypass,{_A2},0", _DS),
    ("get_dsp_high_pass", {"field": "bypass", "signal": _A2}, f"#DSP-HPF? bypass,{_A2}", _DS),
    ("set_dsp_limiter", {"field": "bypass", "signal": _OS, "value": "1"}, f"#DSP-LIMITER bypass,{_OS},1", _DS),
    ("get_dsp_limiter", {"field": "bypass", "signal": "OUT.ANALOG_STEREO.2.AUDIO.1"},
     "#DSP-LIMITER? bypass,OUT.ANALOG_STEREO.2.AUDIO.1", _DS),
    ("set_dsp_post", {"field": "mute", "signal": _A2, "value": "1"}, f"#DSP-POST mute,{_A2},1", _DS),
    ("get_dsp_post", {"field": "mute", "signal": _A2}, f"#DSP-POST? mute,{_A2}", _DS),
    ("set_dsp_eq", {"field": "level", "signal": _A2, "band": 2, "value": "12"}, f"#DSP-EQ level,{_A2},2,12", _DS),
    ("get_dsp_eq", {"field": "band-bypass", "signal": _A2, "band": 0}, f"#DSP-EQ? band-bypass,{_A2},0", _DS),
    ("set_dsp_invert", {"signal": _A2, "inverted": False}, f"#DSP-INVERT {_A2},0", _DS),
    ("get_dsp_invert", {"signal": _A2}, f"#DSP-INVERT? {_A2}", _DS),
    ("set_signal_generator", {"field": "mode", "value": "2"}, "#DSP-SIGGEN mode,IN.GENERATOR.1.AUDIO.1,2", _DS),
    ("get_signal_generator", {"field": "tone-freq"}, "#DSP-SIGGEN? tone-freq,IN.GENERATOR.1.AUDIO.1", _DS),
    ("get_dsp_meter", {"signal": _OS, "type": 5}, f"#DSP-METER? {_OS},5", _DS),
    ("set_mix_level", {"output_signal": _OS, "input_signal": "IN.USB_B.1.AUDIO.2", "level": -25.0},
     f"#X-MIX-LVL {_OS},IN.USB_B.1.AUDIO.2,-25.0", _DS),
    ("get_mix_level", {"output_signal": _OS, "input_signal": "IN.HDMI_AUDIO.1.AUDIO.1"},
     f"#X-MIX-LVL? {_OS},IN.HDMI_AUDIO.1.AUDIO.1", _DS),
    ("set_mix_mute", {"output_signal": _OS, "input_signal": "IN.HDMI_AUDIO.1.AUDIO.1", "muted": False},
     f"#X-MIX-MUTE {_OS},IN.HDMI_AUDIO.1.AUDIO.1,0", _DS),
    ("get_mix_mute", {"output_signal": _OS, "input_signal": "IN.HDMI_AUDIO.1.AUDIO.1"},
     f"#X-MIX-MUTE? {_OS},IN.HDMI_AUDIO.1.AUDIO.1", _DS),
    ("set_audio_input_mode", {"signal": "IN.ANALOG_AUDIO.5.AUDIO.1", "mode": 2},
     "#X-AUD-MODE IN.ANALOG_AUDIO.5.AUDIO.1,2", _DS),
    ("get_audio_input_mode", {"signal": "IN.ANALOG_AUDIO.5.AUDIO.1"}, "#X-AUD-MODE? IN.ANALOG_AUDIO.5.AUDIO.1", _DS),
    ("set_mic_type", {"signal": _A3, "type": "condenser"}, f"#X-MIC-TYPE {_A3},condenser", _DS),
    ("get_mic_type", {"signal": _A3}, f"#X-MIC-TYPE? {_A3}", _DS),
    ("set_link_group", {"group": 1, "linked": False}, "#X-LINK-GROUP 1,OFF", _DS),
    ("get_link_group", {"group": 1}, "#X-LINK-GROUP? 1", _DS),
    ("set_signal_pattern", {"pattern": 2}, "#X-PATTERN OUT.HDMI.1.VIDEO.1,2", _DS),
    ("get_signal_pattern", {}, "#X-PATTERN? OUT.HDMI.1.VIDEO.1", _DS),
    ("recall_config_preset", {"type": "IOCONFIG.SYSTEM.MIXER", "preset": 3}, "#X-PRST-RCL IOCONFIG.SYSTEM.MIXER,3", _DS),
    ("store_config_preset", {"type": "IOCONFIG.SYSTEM", "preset": 2}, "#X-PRST-STO IOCONFIG.SYSTEM,2", _DS),
]
for _cmd, _in, _wire, _m in _PRODUCT:
    text(K, _cmd, _in, _wire + "\r", **_m)
# Replies: an ack and a value.
text(K, "set_rotation", {"output": 1, "input": 2, "angle": 4}, "#ROTATE 1,2,4\r", device_reply="~01@ROTATE 1,2,4\r\n",
     expect_result=_ACKR, file="set_rotation_reply", **_MV)
text(K, "get_kds_channel", {"signal": "audio"}, "#KDS-CHANNEL-SELECT? audio\r",
     device_reply="~01@KDS-CHANNEL-SELECT audio,12\r\n",
     expect_result={"ok": {"kind": "value", "value": "audio,12"}}, file="get_kds_channel_reply", **_DE)
# The products report their state the same way (bidirectional commands, guide p.3).
telemetry(K, "mv-rotation", model="mv-4x", inbound="~01@#ROTATE 1,3,2\r\n", expect_state={"mv": {"rotation": {"1": {"3": 2}}}})
telemetry(K, "kds-channel", model="kds-dec7", inbound="~01@KDS-CHANNEL-SELECT video,5\r\n",
          expect_state={"kds": {"channels": {"video": 5}}})
telemetry(K, "kds-channel-list", model="kds-dec7", inbound="~01@KDS-CHANNEL-SELECT [video,audio,IR],8\r\n",
          expect_state={"kds": {"channels": {"video": 8, "audio": 8, "ir": 8}}})
telemetry(K, "kds-define-channel", model="kds-en7", inbound="~01@KDS-DEFINE-CHANNEL 7\r\n",
          expect_state={"kds": {"encoder_channel": 7}})
telemetry(K, "kds-vlan", model="kds-en7", inbound="~01@KDS-VLAN-TAG 0,33\r\n", expect_state={"kds": {"vlan_tags": {"0": 33}}})
telemetry(K, "video-wall", model="kds-dec7", inbound="~01@VIDEO-WALL-SETUP 3,1\r\n",
          expect_state={"video_wall": {"position": 3, "rotation": 1}})
telemetry(K, "mix-level", model="dsp-62-aec", inbound=f"~01@X-MIX-LVL {_OS},IN.USB_B.1.AUDIO.2,-25.5\r\n",
          expect_state={"mixer": {_OS: {"IN.USB_B.1.AUDIO.2": {"level": -25.5}}}})
telemetry(K, "mix-mute", model="dsp-62-aec", inbound=f"~01@X-MIX-MUTE {_OS},IN.HDMI_AUDIO.1.AUDIO.1,1\r\n",
          expect_state={"mixer": {_OS: {"IN.HDMI_AUDIO.1.AUDIO.1": {"mute": True}}}})
telemetry(K, "dsp-stage", model="dsp-62-aec", inbound=f"~01@DSP-COMP attack,{_A3},15\r\n",
          expect_state={"dsp": {"comp": {_A3: {"attack": "15"}}}})
telemetry(K, "dsp-eq", model="dsp-62-aec", inbound=f"~01@DSP-EQ level,{_A2},2,-3.5\r\n",
          expect_state={"dsp": {"eq": {_A2: {"2": {"level": "-3.5"}}}}})
telemetry(K, "dsp-invert", model="dsp-62-aec", inbound=f"~01@DSP-INVERT {_A2},1\r\n",
          expect_state={"dsp": {"invert": {_A2: {"inverted": "1"}}}})
telemetry(K, "dsp-meter", model="dsp-62-aec", inbound=f"~01@DSP-METER {_OS},5,-12.5\r\n",
          expect_state={"meters": {_OS: {"5": -12.5}}})
telemetry(K, "dsp-error-not-state", model="dsp-62-aec", inbound=f"~01@DSP-COMP attack,{_A3},150 ERR 003\r\n",
          expect_state={})
