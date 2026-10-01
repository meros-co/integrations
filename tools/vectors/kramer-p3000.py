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
