# Analog Way Midra TPP (Midra TPP Programmer's Guide for v02.00.15 and the TPP
# commands document): ASCII command + LF on TCP 10500, answered with the
# register name, indexes and value + CR LF. Expected lines are written from the
# guide's syntax lines ("<scrn>,1GCtakLF", "<scrnF>,<mem>,<scrnT>,<progPrev>,
# <fltr>,1GClrq") with the 0-based indexes the guide and the document state.
MD = "analogway-midra"
ACK = {"ok": {"kind": "ack"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


text(MD, "take", {"screen": 1}, "0,1GCtak\n", device_reply="GCtak0,1\r\n", expect_result=ACK)  # guide 3.4
text(MD, "take_all", {}, "1GCtal\n", device_reply="GCtal1\r\n", expect_result=ACK)
text(MD, "set_tbar", {"screen": 2, "position": 5000}, "1,5000GCtba\n", device_reply="GCtba1,5000\r\n",
     expect_result=ACK)
# Guide 3.6: memory 3 stored from screen 1, to screen 2's preview, nothing filtered.
text(MD, "recall_preset", {"memory": 3, "screen": 2}, "0,2,1,1,0,1GClrq\n",
     device_reply="GClrq0,2,1,1,0,1\r\n", expect_result=ACK)
# Guide 3.3: layer PIP1 of screen 1 preview to input 3, then PUscu.
text(MD, "set_layer_source", {"screen": 1, "layer": 1, "source": 3}, "0,1,1,3PRinp\n",
     device_reply="PRinp0,1,1,3\r\nPUscu0,1\r\n", expect_result=ACK)
text(MD, "update_screen", {"screen": 2}, "1,1PUscu\n", device_reply="PUscu1,1\r\n", expect_result=ACK)
text(MD, "step_back", {"screen": 1}, "0,1GCsba\n", device_reply="GCsba0,1\r\n", expect_result=ACK)
text(MD, "reload_program", {"screen": 1}, "0,1GCrpr\n", device_reply="GCrpr0,1\r\n", expect_result=ACK)
text(MD, "freeze_screen", {"screen": 2}, "1,1GCfsc\n", device_reply="GCfsc1,1\r\n", expect_result=ACK)
text(MD, "freeze_all", {"frozen": False}, "0GCfra\n", device_reply="GCfra0\r\n", expect_result=ACK)
text(MD, "freeze_layer", {"screen": 1, "layer": 2}, "0,2,1GCfrl\n", device_reply="GCfrl0,2,1\r\n",
     expect_result=ACK)
text(MD, "freeze_input", {"input": 10}, "9,1INfrz\n", device_reply="INfrz9,1\r\n", expect_result=ACK)
text(MD, "quick_frame", {"screen": 1}, "0,1CTqfa\n", device_reply="CTqfa0,1\r\n",
     expect_result=ACK)                                                 # guide 3.8
text(MD, "quick_frame_all", {"enabled": False}, "0CTqfl\n", device_reply="CTqfl0\r\n",
     expect_result=ACK)                                                 # guide 3.9
text(MD, "auto_quick_frame", {}, "1CTaqf\n", device_reply="CTaqf1\r\n", expect_result=ACK)
text(MD, "standby", {"standby": False}, "0SBreq\n", device_reply="SBreq0\r\n", expect_result=ACK)
text(MD, "set_switcher_mode", {"mode": 1}, "1SGswm\n", device_reply="SGswm1\r\n", expect_result=ACK)
text(MD, "set_input_plug", {"input": 7, "plug": 2}, "6,2INplg\n", device_reply="INplg6,2\r\n",
     expect_result=ACK)
text(MD, "autoset_input", {"input": 1}, "0,1INasi\n", device_reply="INasi0,1\r\n", expect_result=ACK)
text(MD, "set_audio_input_level", {"audio_input": 19, "level": 192}, "19,192AUile\n",
     device_reply="AUile19,192\r\n", expect_result=ACK)
text(MD, "set_audio_output_volume", {"output": 1, "volume": 128}, "0,128AUomv\n",
     device_reply="AUomv0,128\r\n", expect_result=ACK)
text(MD, "mute_audio_input", {"audio_input": 1}, "1,1AUimu\n", device_reply="AUimu1,1\r\n",
     expect_result=ACK)
text(MD, "mute_audio_output", {"output": 2, "muted": False}, "1,0AUomu\n", device_reply="AUomu1,0\r\n",
     expect_result=ACK)
text(MD, "set_audio_output_input", {"output": 1, "input": 5}, "0,5AUodi\n", device_reply="AUodi0,5\r\n",
     expect_result=ACK)
# Guide 3.2: sending 170 returns 4294967125.
text(MD, "ping", {"value": 170}, "170SYpig\n", device_reply="SYpig4294967125\r\n",
     expect_result=val("4294967125"))
text(MD, "get_ready", {}, "*\n", device_reply="*1\r\n", expect_result=val("1"))           # guide 3.1
text(MD, "get_device_type", {}, "?\n", device_reply="DEV259\r\n", expect_result=val("259"))
text(MD, "get_firmware", {}, "VEupd\n", device_reply="VEupd33554447\r\n",
     expect_result=val("33554447"))                       # document example: v02.00.15
text(MD, "get_command_set_version", {}, "VEvar\n", device_reply="VEvar1\r\n", expect_result=val("1"))
text(MD, "get_take_available", {"screen": 1}, "0,GCtav\n", device_reply="GCtav0,1\r\n",
     expect_result=val("1"))
text(MD, "get_layer_source", {"screen": 2, "layer": 1, "preview": False}, "1,0,1,PRinp\n",
     device_reply="PRinp1,0,1,11\r\n", expect_result=val("11"))
text(MD, "get_tbar", {"screen": 1}, "0,GCtba\n", device_reply="GCtba0,10000\r\n", expect_result=val("10000"))
text(MD, "read_back", {"mode": 1}, "1#\n", device_reply="#1\r\n", expect_result=ACK)           # guide 3.1
# Guide 2.4.5: E11, index out of range, fails the command.
text(MD, "send_raw", {"command": "5,1GCtak"}, "5,1GCtak\n", device_reply="E11\r\n",
     expect_result={"error": {"error": "device_error"}})

# ── Telemetry ──
telemetry(MD, "ready", inbound="*1\r\n", expect_state={"device": {"ready": True}})
telemetry(MD, "device-type", inbound="DEV257\r\n", expect_state={"device": {"type": 257}})
telemetry(MD, "command-set", inbound="VEvar11\r\n", expect_state={"device": {"command_set_version": 11}})
telemetry(MD, "firmware", inbound="VEupd33554447\r\n", expect_state={"device": {"firmware": 33554447}})
telemetry(MD, "serial", inbound="DIdsn100\r\n", expect_state={"device": {"serial_number": 100}})
telemetry(MD, "switcher-mode", inbound="SGswm2\r\n", expect_state={"device": {"switcher_mode": 2}})
telemetry(MD, "standby", inbound="SBsta1\r\n", expect_state={"device": {"standby": True}})
telemetry(MD, "take-available", inbound="GCtav1,0\r\n",
          expect_state={"screens": {"2": {"take_available": False}}})
telemetry(MD, "taking", inbound="GCtak0,0\r\n", expect_state={"screens": {"1": {"taking": False}}})
telemetry(MD, "take-kind", inbound="GCtio0,2\r\n", expect_state={"screens": {"1": {"take_kind": 2}}})
telemetry(MD, "tbar", inbound="GCtba0,10000\r\n", expect_state={"screens": {"1": {"tbar": 10000}}})
telemetry(MD, "max-layers", inbound="SCmly0,4\r\n", expect_state={"screens": {"1": {"max_layers": 4}}})
telemetry(MD, "screen-frozen", inbound="GCfsc1,1\r\n", expect_state={"screens": {"2": {"frozen": True}}})
telemetry(MD, "layer-frozen", inbound="GCfrl0,2,1\r\n",
          expect_state={"screens": {"1": {"layer_frozen": {"2": True}}}})
telemetry(MD, "quick-frame", inbound="CTqfa0,1\r\n", expect_state={"screens": {"1": {"quick_frame": True}}})
telemetry(MD, "layer-program", inbound="PRinp0,0,1,3\r\n",
          expect_state={"screens": {"1": {"program": {"layers": {"1": {"source": 3}}}}}})
telemetry(MD, "layer-preview", inbound="PRinp1,1,0,2\r\n",
          expect_state={"screens": {"2": {"preview": {"layers": {"0": {"source": 2}}}}}})
telemetry(MD, "freeze-all", inbound="GCfra1\r\n", expect_state={"freeze_all": True})
telemetry(MD, "quick-frame-all", inbound="CTqfl0\r\n", expect_state={"quick_frame_all": False})
telemetry(MD, "auto-quick-frame", inbound="CTaqf1\r\n", expect_state={"auto_quick_frame": True})
telemetry(MD, "previewed-layer", inbound="GCply1\r\n", expect_state={"previewed_layer": 1})
telemetry(MD, "input-available", inbound="INava9,1\r\n", expect_state={"inputs": {"10": {"available": True}}})
telemetry(MD, "input-frozen", inbound="INfrz2,1\r\n", expect_state={"inputs": {"3": {"frozen": True}}})
telemetry(MD, "input-plug", inbound="INplg6,2\r\n", expect_state={"inputs": {"7": {"plug": 2}}})
telemetry(MD, "input-signal", inbound="ISsva4,3,1\r\n",
          expect_state={"inputs": {"5": {"plugs": {"3": {"signal": True}}}}})
telemetry(MD, "audio-input-level", inbound="AUile19,192\r\n",
          expect_state={"audio": {"inputs": {"19": {"level": 192}}}})
telemetry(MD, "audio-input-mute", inbound="AUimu1,1\r\n", expect_state={"audio": {"inputs": {"1": {"muted": True}}}})
telemetry(MD, "audio-output-volume", inbound="AUomv0,192\r\n",
          expect_state={"audio": {"outputs": {"1": {"volume": 192}}}})
telemetry(MD, "audio-output-mute", inbound="AUomu1,0\r\n",
          expect_state={"audio": {"outputs": {"2": {"muted": False}}}})
telemetry(MD, "audio-output-input", inbound="AUodi0,5\r\n",
          expect_state={"audio": {"outputs": {"1": {"input": 5}}}})

# Registers from the commands document: borders (applied with PUscu), audio
# mode, input plug settings and the clock. Commands sending several registers
# state the first line and feed every answer.
text(MD, "set_layer_border", {"screen": 1, "layer": 2, "style": 4}, "0,1,2,4PRbst\n",
     device_reply="PRbst0,1,2,4\r\nPUscu0,1\r\n", expect_result=ACK)
text(MD, "set_layer_border_alpha", {"screen": 2, "layer": 1, "alpha": 128, "preview": False}, "1,0,1,128PRbal\n",
     device_reply="PRbal1,0,1,128\r\nPUscu1,1\r\n", expect_result=ACK)
text(MD, "set_audio_mode", {"mode": 2}, "2SGsam\n", device_reply="SGsam2\r\n", expect_result=ACK)
text(MD, "autocenter_input", {"input": 1, "mode": 1}, "0,1INain\n", device_reply="INain0,1\r\n", expect_result=ACK)
text(MD, "set_input_hdcp", {"input": 5, "plug": 3, "enabled": False}, "4,3,0INhdc\n", device_reply="INhdc4,3,0\r\n",
     expect_result=ACK)
text(MD, "set_input_long_reach", {"input": 10, "plug": 4, "enabled": True}, "9,4,1INlre\n",
     device_reply="INlre9,4,1\r\n", expect_result=ACK)
text(MD, "set_output_long_reach", {"output": 2, "enabled": True}, "1,1OUlre\n", device_reply="OUlre1,1\r\n",
     expect_result=ACK)
text(MD, "set_input_aspect", {"input": 1, "plug": 3, "aspect": 5}, "0,3,5IEain\n", device_reply="IEain0,3,5\r\n",
     expect_result=ACK)
text(MD, "set_input_aspect_mode", {"input": 1, "plug": 3, "mode": 1}, "0,3,1IEaou\n", device_reply="IEaou0,3,1\r\n",
     expect_result=ACK)
text(MD, "set_input_crop", {"input": 2, "plug": 2, "crop": 3}, "1,2,3IEpcr\n", device_reply="IEpcr1,2,3\r\n",
     expect_result=ACK)
text(MD, "set_clock", {"year": 2026, "month": 10, "day": 14, "hour": 9, "minute": 30, "second": 0}, "2026RTyea\n",
     device_reply="RTyea2026\r\nRTmon9\r\nRTday14\r\nRThou9\r\nRTmin30\r\nRTsec0\r\nRTreq2\r\n", expect_result=ACK)
text(MD, "refresh_clock", {}, "1RTreq\n", device_reply="RTreq1\r\n", expect_result=ACK)
telemetry(MD, "border", inbound="PRbst0,1,2,4\r\n",
          expect_state={"screens": {"1": {"preview": {"layers": {"2": {"border": 4}}}}}})
telemetry(MD, "border-program", inbound="PRbst0,0,1,1\r\n",
          expect_state={"screens": {"1": {"program": {"layers": {"1": {"border": 1}}}}}})
telemetry(MD, "border-alpha", inbound="PRbal1,0,1,128\r\n",
          expect_state={"screens": {"2": {"program": {"layers": {"1": {"border_alpha": 128}}}}}})
telemetry(MD, "border-alpha-preview", inbound="PRbal1,1,1,255\r\n",
          expect_state={"screens": {"2": {"preview": {"layers": {"1": {"border_alpha": 255}}}}}})
telemetry(MD, "audio-mode", inbound="SGsam1\r\n", expect_state={"device": {"audio_mode": 1}})
telemetry(MD, "standby-progress", inbound="SBprg40\r\n", expect_state={"device": {"standby_progress": 40}})
telemetry(MD, "autocenter-progress", inbound="INaip0,80\r\n", expect_state={"inputs": {"1": {"autocenter_progress": 80}}})
telemetry(MD, "autoset-progress", inbound="INapi0,100\r\n", expect_state={"inputs": {"1": {"autoset_progress": 100}}})
telemetry(MD, "input-hdcp", inbound="INhdc4,3,0\r\n",
          expect_state={"inputs": {"5": {"plugs": {"3": {"hdcp_enabled": False}}}}})
telemetry(MD, "input-long-reach", inbound="INlre9,4,1\r\n",
          expect_state={"inputs": {"10": {"plugs": {"4": {"long_reach": True}}}}})
telemetry(MD, "output-long-reach", inbound="OUlre1,0\r\n", expect_state={"outputs": {"2": {"long_reach": False}}})
telemetry(MD, "aspect", inbound="IEain0,3,5\r\n", expect_state={"inputs": {"1": {"plugs": {"3": {"aspect": 5}}}}})
telemetry(MD, "aspect-mode", inbound="IEaou0,3,2\r\n",
          expect_state={"inputs": {"1": {"plugs": {"3": {"aspect_mode": 2}}}}})
telemetry(MD, "crop", inbound="IEpcr1,2,3\r\n", expect_state={"inputs": {"2": {"plugs": {"2": {"crop": 3}}}}})
telemetry(MD, "plug-available", inbound="INpav0,4,0\r\n",
          expect_state={"inputs": {"1": {"plugs": {"4": {"available": False}}}}})
telemetry(MD, "plug-hdcp", inbound="IShdc0,3,1\r\n", expect_state={"inputs": {"1": {"plugs": {"3": {"hdcp": True}}}}})
telemetry(MD, "plug-audio", inbound="ISaud0,3,1\r\n",
          expect_state={"inputs": {"1": {"plugs": {"3": {"embedded_audio": True}}}}})
telemetry(MD, "plug-width", inbound="ISiwi0,3,1920\r\n", expect_state={"inputs": {"1": {"plugs": {"3": {"width": 1920}}}}})
telemetry(MD, "plug-height", inbound="ISihe0,3,1080\r\n",
          expect_state={"inputs": {"1": {"plugs": {"3": {"height": 1080}}}}})
telemetry(MD, "plug-format", inbound="IScfo0,3,30\r\n", expect_state={"inputs": {"1": {"plugs": {"3": {"format": 30}}}}})
telemetry(MD, "output-hdcp", inbound="OUihc0,6,1\r\n", expect_state={"outputs": {"1": {"plugs": {"6": {"hdcp": True}}}}})
telemetry(MD, "clock-year", inbound="RTyea2026\r\n", expect_state={"clock": {"year": 2026}})
telemetry(MD, "clock-month", inbound="RTmon9\r\n", expect_state={"clock": {"month": 10}})
telemetry(MD, "clock-day", inbound="RTday14\r\n", expect_state={"clock": {"day": 14}})
telemetry(MD, "clock-hour", inbound="RThou9\r\n", expect_state={"clock": {"hour": 9}})
telemetry(MD, "clock-minute", inbound="RTmin30\r\n", expect_state={"clock": {"minute": 30}})
telemetry(MD, "clock-second", inbound="RTsec5\r\n", expect_state={"clock": {"second": 5}})
telemetry(MD, "clock-valid", inbound="RTval1\r\n", expect_state={"clock": {"valid": True}})
telemetry(MD, "transitions", inbound="DFtra2,1,1\r\n", expect_state={"transitions": {"2": {"layers": {"1": True}}}})
