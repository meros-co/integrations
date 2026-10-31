K = "extron-matrix"
# Extron SIS matrices (DXP HD 4K PLUS 68-2939-01 Rev. A, DXP DVI/HDMI
# 68-2192-01 Rev. B, XTP II CrossPoint 68-1736-02 Rev. P, SMX 68-1451-01
# Rev. E). Every command is sent followed by CR; Esc is 0x1B. Replies are the
# verbose mode 3 (tagged) forms of the command and response tables.
E = "\x1b"
D4 = {"model": "dxp-hd-4k-plus"}
DV = {"model": "dxp-dvi-hdmi"}
XT = {"model": "xtp-ii-crosspoint"}
SM = {"model": "smx"}
ACK = {"ok": {"kind": "ack"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


ERR = {"error": {"error": "device_error"}}

# Ties (DXP HD 4K PLUS p.43, DXP DVI p.57, XTP II p.51)
text(K, "tie", {"input": 3, "output": 4}, "3*4!\r", device_reply="Out04 In03 All\r\n", expect_result=ACK)
text(K, "tie", {"input": 0, "output": 4}, "0*4!\r", device_reply="Out04 In00 All\r\n", expect_result=ACK, **DV,
     file="tie-dxp-dvi")
text(K, "tie_video", {"input": 7, "output": 5}, "7*5%\r", device_reply="Out05 In07 Vid\r\n", expect_result=ACK)
text(K, "tie_audio", {"input": 6, "output": 4}, "6*4$\r", device_reply="Out04 In06 Aud\r\n", expect_result=ACK)
text(K, "tie_input_to_all", {"input": 5}, "5*!\r", device_reply="In05 All\r\n", expect_result=ACK)
text(K, "tie_video_to_all", {"input": 2}, "2*%\r", device_reply="In02 Vid\r\n", expect_result=ACK)
text(K, "tie_audio_to_all", {"input": 2}, "2*$\r", device_reply="In02 Aud\r\n", expect_result=ACK)
text(K, "quick_tie", {"ties": "3*4%6*1$3*2!"}, E + "+Q3*4%6*1$3*2!\r", device_reply="Qik\r\n", expect_result=ACK)
text(K, "get_tie", {"output": 2}, "2!\r", device_reply="Out02 In07 All\r\n", expect_result=val("07"))
text(K, "get_video_tie", {"output": 2}, "2%\r", device_reply="Out02 In03 Vid\r\n", expect_result=val("03"))
text(K, "get_audio_tie", {"output": 2}, "2$\r", device_reply="E13\r\n", expect_result=ERR)

# Mutes (DXP HD 4K PLUS p.46-47, XTP II p.52)
text(K, "set_video_mute", {"output": 2, "muted": True}, "2*1B\r", device_reply="Vmt2*1\r\n", expect_result=ACK)
text(K, "set_video_sync_mute", {"output": 2}, "2*2B\r", device_reply="Vmt2*2\r\n", expect_result=ACK)
text(K, "get_video_mute", {"output": 2}, "2B\r", device_reply="Vmt2*0\r\n", expect_result=val("0"))
text(K, "set_video_mute_all", {"muted": False}, "0*B\r", device_reply="Vmt0\r\n", expect_result=ACK)
text(K, "set_audio_mute", {"output": 1, "muted": True}, "1*1Z\r", device_reply="Amt1*1\r\n", expect_result=ACK)
text(K, "set_audio_mute_mode", {"output": 1, "mode": 7}, "1*7Z\r", device_reply="Amt1*7\r\n", expect_result=ACK)
text(K, "set_audio_mute_mode", {"output": 9, "mode": 3}, "9*3Z\r", device_reply="E12\r\n", expect_result=ERR, **XT,
     file="set_audio_mute_mode-error")
text(K, "get_audio_mute", {"output": 1}, "1Z\r", device_reply="Amt1*2\r\n", expect_result=val("2"))
text(K, "set_audio_mute_all", {"muted": True}, "1*Z\r", device_reply="Amt1\r\n", expect_result=ACK)
text(K, "get_mutes", {}, E + "VM\r", device_reply="Mut01002300\r\n", expect_result=val("01002300"), **DV)

# Presets and rooms (DXP HD 4K PLUS p.48-50, DXP DVI p.58-60, XTP II p.52)
text(K, "save_global_preset", {"preset": 9}, "9,\r", device_reply="Spr09\r\n", expect_result=ACK)
text(K, "recall_global_preset", {"preset": 5}, "5.\r", device_reply="Rpr05\r\n", expect_result=ACK)
text(K, "recall_global_preset", {"preset": 6}, "6.\r", device_reply="E11\r\n", expect_result=ERR, **SM,
     file="recall_global_preset-error")
text(K, "recall_global_preset_prst", {"preset": 3}, E + "R3PRST\r", device_reply="PrstR03\r\n", expect_result=ACK)
text(K, "get_global_preset_video_ties", {"preset": 0}, E + "0*1*1VC\r",
     device_reply="Vgp00*Out01 8 8 2 8 8 7 7 0 -- -- -- -- -- -- -- -- Vid\r\n",
     expect_result=val("8 8 2 8 8 7 7 0 -- -- -- -- -- -- -- --"))
text(K, "get_global_preset_audio_ties", {"preset": 15}, E + "15*1*2VC\r")
text(K, "set_global_preset_name", {"preset": 1, "name": "Security1"}, E + "1,Security1NG\r",
     device_reply="Nmg01,Security1\r\n", expect_result=ACK)
text(K, "get_global_preset_name", {"preset": 2}, E + "2NG\r", device_reply="Nmg02,[unassigned]\r\n",
     expect_result=val("[unassigned]"))
text(K, "save_room_preset", {"room": 3, "preset": 9}, "3*9,\r", device_reply="Rmm03 Spr09\r\n", expect_result=ACK)
text(K, "recall_room_preset", {"room": 7, "preset": 3}, "7*3.\r", device_reply="Rmm07 Rpr03\r\n", expect_result=ACK)
text(K, "set_room_name", {"room": 1, "name": "Classrm1"}, E + "1,Classrm1NR\r", device_reply="Nmr01,Classrm1\r\n",
     expect_result=ACK)
text(K, "get_room_name", {"room": 1}, E + "1NR\r", device_reply="Nmr01,Classrm1\r\n", expect_result=val("Classrm1"))

# Names (DXP HD 4K PLUS p.44-45, DXP DVI p.62)
text(K, "set_input_name", {"input": 1, "name": "PodiumCam"}, E + "1,PodiumCamNI\r", device_reply="Nmi1,PodiumCam\r\n",
     expect_result=ACK)
text(K, "get_input_name", {"input": 1}, E + "1NI\r", device_reply="Nmi1,PodiumCam\r\n", expect_result=val("PodiumCam"))
text(K, "set_output_name", {"output": 1, "name": "Main PJ1"}, E + "1,Main PJ1NO\r", device_reply="Nmo1,Main PJ1\r\n",
     expect_result=ACK)
text(K, "get_output_name", {"output": 1}, E + "1NO\r", device_reply="Nmo1,Main PJ1\r\n", expect_result=val("Main PJ1"))

# Audio levels (DXP HD 4K PLUS p.47, XTP II p.53-54)
text(K, "set_input_attenuation_db", {"input": 2, "level": -10}, "2*-10G\r", device_reply="In2 Aud-10\r\n",
     expect_result=ACK)
text(K, "set_input_gain", {"input": 1, "gain": 7}, "1*7G\r", device_reply="In01 Aud+07\r\n", expect_result=ACK)
text(K, "set_input_attenuation", {"input": 1, "attenuation": 18}, "1*18g\r", device_reply="In01 Aud-18\r\n",
     expect_result=ACK)
text(K, "increase_input_level", {"input": 4}, "4+G\r", device_reply="In04 Aud+08\r\n", expect_result=ACK)
text(K, "decrease_input_level", {"input": 4}, "4-G\r", **XT)
text(K, "get_input_level", {"input": 1}, "1G\r", device_reply="In01 Aud-03\r\n", expect_result=val("-03"))
text(K, "set_output_volume", {"output": 1, "level": 64}, "1*64V\r", device_reply="Out1 Vol64\r\n", expect_result=ACK)
text(K, "set_output_volume", {"output": 1, "level": 90}, "1*90V\r", device_reply="E13\r\n", expect_result=ERR, **XT,
     file="set_output_volume-error")
text(K, "increase_output_volume", {"output": 2}, "2+V\r", device_reply="Out2 Vol65\r\n", expect_result=ACK)
text(K, "decrease_output_volume", {"output": 2}, "2-V\r")
text(K, "get_output_volume", {"output": 2}, "2V\r", device_reply="Out2 Vol100\r\n", expect_result=val("100"))

# Front panel (DXP HD 4K PLUS p.50, XTP II p.62)
text(K, "set_front_panel_lock", {"mode": 1}, "1X\r", device_reply="Exe1\r\n", expect_result=ACK)
text(K, "get_front_panel_lock", {}, "X\r", device_reply="Exe2\r\n", expect_result=val("2"))
text(K, "set_executive_mode", {"mode": 0}, E + "0EXEC\r", device_reply="Exec0\r\n", expect_result=ACK)
text(K, "get_executive_mode", {}, E + "EXEC\r", device_reply="Exec2\r\n", expect_result=val("2"))

# Status and information
text(K, "get_signal_status", {}, "0LS\r", device_reply="Frq00 01100000\r\n", expect_result=val("01100000"))
text(K, "get_input_hdcp_status", {"input": 3}, E + "I3HDCP\r", device_reply="HdcpI3*1\r\n", expect_result=val("1"))
text(K, "get_output_hdcp_status", {"output": 2}, E + "O2HDCP\r", device_reply="HdcpO2*3\r\n", expect_result=val("3"))
text(K, "set_power_save", {"mode": 0}, E + "0PSAV\r", device_reply="Psav0\r\n", expect_result=ACK)
text(K, "get_power_save", {}, E + "PSAV\r", device_reply="1\r\n", expect_result=val("1"))
text(K, "set_test_pattern", {"pattern": 3}, E + "03TEST\r", device_reply="Tst03\r\n", expect_result=ACK)
text(K, "get_test_pattern", {}, E + "TEST\r", device_reply="00\r\n", expect_result=val("00"))
text(K, "set_verbose_mode", {"mode": 3}, E + "3CV\r", device_reply="Vrb3\r\n", expect_result=ACK)
text(K, "get_verbose_mode", {}, E + "CV\r", device_reply="Vrb3\r\n", expect_result=val("3"))
text(K, "get_firmware", {}, "Q\r", device_reply="1.14\r\n", expect_result=val("1.14"))
text(K, "get_firmware", {}, "Q\r", device_reply="Ver01*1.14\r\n", expect_result=val("1.14"), **SM,
     file="get_firmware-smx")
text(K, "get_firmware_full", {}, "*Q\r", device_reply="1.14.0002\r\n", expect_result=val("1.14.0002"))
text(K, "get_part_number", {}, "N\r", device_reply="Pno60-1495-01\r\n", expect_result=val("60-1495-01"))
text(K, "get_info", {}, "I\r", device_reply="V32X32 A32X32\r\n", expect_result=val("V32X32 A32X32"), **XT)
text(K, "get_status", {}, "S\r", device_reply="Sts00*12.03 3.31 +40.00 03305\r\n",
     expect_result=val("12.03 3.31 +40.00 03305"))

# SMX: plane-addressed (SMX p.42-44)
text(K, "smx_tie", {"plane": 1, "input": 3, "output": 4}, "01*3*4!\r", device_reply="01 Out04 In03 All\r\n",
     expect_result=ACK)
text(K, "smx_tie_video", {"plane": 1, "input": 3, "output": 5}, "01*3*5%\r", device_reply="01 Out05 In03 Vid\r\n",
     expect_result=ACK)
text(K, "smx_tie_audio", {"plane": 0, "input": 3, "output": 6}, "00*3*6$\r", device_reply="00 Out06 In03 Aud\r\n",
     expect_result=ACK)
text(K, "smx_tie_input_to_all", {"plane": 2, "input": 1}, "02*1*!\r", device_reply="02 In01 All\r\n", expect_result=ACK)
text(K, "smx_quick_tie", {"ties": "01*3*4!01*3*5%01*3*6$"}, E + "+Q01*3*4!01*3*5%01*3*6$\r",
     device_reply="Qik\r\n", expect_result=ACK)
text(K, "smx_get_video_tie", {"plane": 1, "output": 4}, "01*4%\r", device_reply="01 Out04 In03 Vid\r\n",
     expect_result=val("03"))
text(K, "smx_get_audio_tie", {"plane": 1, "output": 4}, "01*4$\r", device_reply="E14\r\n", expect_result=ERR)
text(K, "smx_set_video_mute", {"plane": 1, "output": 4, "muted": True}, "01*4*1B\r", device_reply="01 Vmt4*1\r\n",
     expect_result=ACK)
text(K, "smx_get_video_mute", {"plane": 1, "output": 4}, "01*4B\r", device_reply="1\r\n", expect_result=val("1"))
text(K, "smx_set_plane_video_mute", {"plane": 1, "muted": False}, "01*0*B\r", device_reply="01 Vmt00*0\r\n",
     expect_result=ACK)
text(K, "smx_set_audio_mute", {"plane": 2, "output": 1, "muted": False}, "02*1*0Z\r", device_reply="02 Amt1*0\r\n",
     expect_result=ACK)
text(K, "smx_get_audio_mute", {"plane": 2, "output": 1}, "02*1Z\r", device_reply="0\r\n", expect_result=val("0"))
text(K, "smx_set_plane_audio_mute", {"plane": 2, "muted": True}, "02*1*Z\r", device_reply="02 Amt00*1\r\n",
     expect_result=ACK)
text(K, "smx_get_mutes", {"plane": 1}, E + "01VM\r", device_reply="Mut01*0123\r\n", expect_result=val("0123"))
text(K, "smx_save_plane_preset", {"plane": 1, "preset": 2}, "01*2*0,\r", device_reply="01 Spr2\r\n", expect_result=ACK)
text(K, "smx_recall_plane_preset", {"plane": 1, "preset": 2}, "01*2*0.\r", device_reply="01 Rpr2\r\n",
     expect_result=ACK)

# Telemetry: verbose mode 3 is asked for on connecting; every message after
# goes through the rules, replies and change reports alike.
telemetry(K, "connect", expect_connect_wire=[E + "3CV\r"], inbound="Vrb3\r\n",
          expect_state={"device": {"verbose_mode": 3}})
telemetry(K, "connect-password", settings={"password": "extron"},
          expect_connect_wire=["extron\r", E + "3CV\r"], inbound="Vrb3\r\n",
          expect_state={"device": {"verbose_mode": 3}})
telemetry(K, "banner",
          inbound="(c) Copyright 2018, Extron Electronics, DXP 88 HD 4K Plus, V1.00, 60-1495-03\r\n",
          expect_state={"device": {"model": "DXP 88 HD 4K Plus", "firmware": "1.00", "part_number": "60-1495-03"}})
telemetry(K, "banner-smx", inbound="(c) Copyright 2009, Extron Electronics SMX, V1.14, 60-857-01\r\n",
          expect_state={"device": {"model": "SMX", "firmware": "1.14", "part_number": "60-857-01"}})
telemetry(K, "login", inbound="\r\nLogin Administrator\r\n", expect_state={"device": {"login": "Administrator"}})
telemetry(K, "part-number", inbound="Pno60-1495-01\r\n", expect_state={"device": {"part_number": "60-1495-01"}})
telemetry(K, "info", inbound="V8X8 A8X2\r\n",
          expect_state={"device": {"video_inputs": 8, "video_outputs": 8, "audio_inputs": 8, "audio_outputs": 2}})
telemetry(K, "tie-all", inbound="Out04 In03 All\r\n",
          expect_state={"outputs": {"4": {"video_input": 3, "audio_input": 3}}})
telemetry(K, "tie-video", inbound="Out5 In7 Vid\r\n", expect_state={"outputs": {"5": {"video_input": 7}}})
telemetry(K, "tie-rgb", inbound="Out4 In8 RGB\r\n", expect_state={"outputs": {"4": {"video_input": 8}}})
telemetry(K, "tie-audio", inbound="Out04 In06 Aud\r\n", expect_state={"outputs": {"4": {"audio_input": 6}}})
telemetry(K, "untie", inbound="Out03 In00 All\r\n",
          expect_state={"outputs": {"3": {"video_input": 0, "audio_input": 0}}})
telemetry(K, "error-line", inbound="E01\r\n", expect_state={})
telemetry(K, "video-mute", inbound="Vmt2*2\r\n", expect_state={"outputs": {"2": {"video_mute": True, "video_mute_mode": 2}}})
telemetry(K, "audio-mute", inbound="Amt1*0\r\n", expect_state={"outputs": {"1": {"audio_mute": False, "audio_mute_mode": 0}}})
telemetry(K, "input-level", inbound="In01 Aud-18\r\n", expect_state={"inputs": {"1": {"audio_level": -18}}})
telemetry(K, "output-volume", inbound="Out02 Vol64\r\n", expect_state={"outputs": {"2": {"volume": 64}}})
telemetry(K, "preset-recalled", inbound="Rpr05\r\n", expect_state={"presets": {"last_recalled": 5}})
telemetry(K, "preset-saved", inbound="Spr09\r\n", expect_state={"presets": {"last_saved": 9}})
telemetry(K, "xtp-preset-recalled", inbound="PrstR12\r\n", expect_state={"presets": {"last_recalled": 12}})
telemetry(K, "room-preset", inbound="Rmm07 Rpr03\r\n", expect_state={"rooms": {"7": {"last_recalled": 3}}})
telemetry(K, "input-name", inbound="Nmi1,PodiumCam\r\n", expect_state={"inputs": {"1": {"name": "PodiumCam"}}})
telemetry(K, "output-name", inbound="Nmo2,Main PJ1\r\n", expect_state={"outputs": {"2": {"name": "Main PJ1"}}})
telemetry(K, "preset-name", inbound="Nmg01,Security1\r\n", expect_state={"presets": {"1": {"name": "Security1"}}})
telemetry(K, "room-name", inbound="Nmr01,Classrm1\r\n", expect_state={"rooms": {"1": {"name": "Classrm1"}}})
telemetry(K, "front-panel", inbound="Exe1\r\n", expect_state={"front_panel_lock": 1})
telemetry(K, "executive-mode", inbound="Exec2\r\n", expect_state={"front_panel_lock": 2})
telemetry(K, "hdcp", inbound="HdcpI3*2\r\n", expect_state={"inputs": {"3": {"hdcp_status": 2}}})
telemetry(K, "vc-video", inbound="Vgp00*Out01 8 8 2 8 -- -- -- -- -- -- -- -- -- -- -- -- Vid\r\n",
          expect_state={"outputs": {"1": {"video_input": 8}, "2": {"video_input": 8}, "3": {"video_input": 2},
                                    "4": {"video_input": 8}}})
telemetry(K, "vc-audio", inbound="Vgp00*Out01 2 6 1 0 -- -- -- -- -- -- -- -- -- -- -- -- Aud\r\n",
          expect_state={"outputs": {"1": {"audio_input": 2}, "2": {"audio_input": 6}, "3": {"audio_input": 1},
                                    "4": {"audio_input": 0}}})
telemetry(K, "vc-preset-ignored", inbound="Vgp04*Out01 8 8 2 8 -- -- -- -- -- -- -- -- -- -- -- -- Vid\r\n",
          expect_state={})
telemetry(K, "mutes", inbound="Mut0123\r\n",
          expect_state={"outputs": {"1": {"video_mute": False, "audio_mute": False},
                                    "2": {"video_mute": True, "audio_mute": False},
                                    "3": {"video_mute": False, "audio_mute": True},
                                    "4": {"video_mute": True, "audio_mute": True}}})
telemetry(K, "mutes-xtp", inbound="Mut00 0746\r\n",
          expect_state={"outputs": {"1": {"video_mute": False, "audio_mute": False},
                                    "2": {"video_mute": True, "audio_mute": True},
                                    "3": {"video_mute": False, "audio_mute": True},
                                    "4": {"video_mute": False, "audio_mute": True}}})
telemetry(K, "signal", inbound="Frq00 0110\r\n",
          expect_state={"inputs": {"1": {"signal": False}, "2": {"signal": True}, "3": {"signal": True},
                                   "4": {"signal": False}}})
telemetry(K, "smx-tie", inbound="01 Out04 In03 All\r\n",
          expect_state={"planes": {"1": {"outputs": {"4": {"video_input": 3, "audio_input": 3}}}}})
telemetry(K, "smx-tie-audio", inbound="00 Out06 In03 Aud\r\n",
          expect_state={"planes": {"0": {"outputs": {"6": {"audio_input": 3}}}}})
telemetry(K, "smx-video-mute", inbound="01 Vmt4*1\r\n",
          expect_state={"planes": {"1": {"outputs": {"4": {"video_mute": True}}}}})
telemetry(K, "smx-plane-mute-ignored", inbound="01 Vmt00*1\r\n", expect_state={})
telemetry(K, "smx-plane-preset", inbound="01 Rpr2\r\n", expect_state={"planes": {"1": {"last_recalled": 2}}})

# Full control (DXP HD 4K PLUS p.44-56, DXP DVI p.56-71, XTP II Rev. Q
# p.51-66, SMX p.41-58). Replies are the verbose mode 3 tagged forms.
text(K, "edid_assign", {"input": 3, "edid": 15}, E + "A3*15EDID\r", device_reply="EdidA03*15\r\n", expect_result=ACK, **DV)
text(K, "edid_assign_all", {"edid": 32}, E + "A32*EDID\r", device_reply="EdidA00*32\r\n", expect_result=ACK, **DV)
text(K, "get_edid_assignment", {"input": 3}, E + "A3EDID\r", device_reply="EdidA03*15\r\n", expect_result=val("15"), **DV)
text(K, "edid_save_output", {"edid": 37}, E + "S37EDID\r", device_reply="EdidS37\r\n", expect_result=ACK, **DV)
text(K, "smx_edid_assign", {"slot": 2, "input": 4, "edid": 38}, E + "A2*4*38EDID\r", device_reply="02EdidA04*38\r\n",
     expect_result=ACK, **SM)
text(K, "smx_edid_assign_all", {"slot": 2, "edid": 0}, E + "A2*0*EDID\r", device_reply="02EdidA00*00\r\n",
     expect_result=ACK, **SM)
text(K, "smx_get_edid_assignment", {"slot": 2, "input": 4}, E + "A2*4EDID\r", device_reply="02EdidA04*38\r\n",
     expect_result=val("38"), **SM)
text(K, "smx_edid_save_output", {"slot": 2, "edid": 40}, E + "S2*40EDID\r", device_reply="02EdidS40\r\n",
     expect_result=ACK, **SM)
text(K, "get_edid_hex", {"slot": 1}, E + "R1EDID\r", device_reply="EdidR" + "00FFFFFFFFFFFF00" * 16 + "\r\n",
     expect_result=val("00FFFFFFFFFFFF00" * 16), **D4)
text(K, "get_edid_native_resolution", {"slot": 1}, E + "N1EDID\r", device_reply="EdidN1920x1080 @ 60Hz\r\n",
     expect_result=val("1920x1080 @ 60Hz"), **D4)
text(K, "edid_import_file", {"slot": 2, "file": "EXN_HDMI_1080p60_2Ch.bin"}, E + "I2,EXN_HDMI_1080p60_2Ch.bin" + "EDID\r",
     device_reply="EdidI02\r\n", expect_result=ACK, **D4)
text(K, "edid_export_file", {"slot": 9, "file": "out1.bin"}, E + "E9,out1.binEDID\r", device_reply="EdidE09\r\n",
     expect_result=ACK, **D4)
text(K, "set_input_hdcp_authorization", {"input": 2, "enabled": False}, E + "E2*0HDCP\r", device_reply="HdcpE2*0\r\n",
     expect_result=ACK, **D4)
text(K, "get_input_hdcp_authorization", {"input": 2}, E + "E2HDCP\r", device_reply="HdcpE2*1\r\n", expect_result=val("1"), **XT)
text(K, "set_output_hdcp_mode", {"output": 1, "mode": 1}, E + "S1*1HDCP\r", device_reply="HdcpS1*1\r\n", expect_result=ACK, **D4)
text(K, "set_output_format", {"output": 2, "format": 1}, E + "2*1VTPO\r", device_reply="Vtpo2*1\r\n", expect_result=ACK, **D4)
text(K, "get_output_format", {"output": 2}, E + "2VTPO\r", device_reply="Vtpo2*1\r\n", expect_result=val("1"), **D4)
text(K, "set_room_outputs", {"room": 1, "outputs": "1,2,5"}, E + "1,1,2,5MR\r", device_reply="Mpr1,1,2,5\r\n",
     expect_result=ACK, **DV)
text(K, "get_room_outputs", {"room": 1}, E + "1MR\r", device_reply="Mpr1,1,2,5\r\n", expect_result=val("1,2,5"), **DV)
text(K, "set_room_preset_name", {"room": 1, "preset": 2, "name": "Lecture"}, E + "1*2,LectureNP\r",
     device_reply="Nmp1*2,Lecture\r\n", expect_result=ACK, **D4)
text(K, "set_input_endpoint_power", {"input": 3, "enabled": True}, E + "I3*1POEC\r", device_reply="PoecI03*1*1*1\r\n",
     expect_result=ACK, **XT)
text(K, "set_output_endpoint_power", {"output": 2, "enabled": False}, E + "O2*0POEC\r", device_reply="PoecO02*0*1*2\r\n",
     expect_result=ACK, **XT)
text(K, "get_input_endpoint_power", {"input": 3}, E + "I3POEC\r", device_reply="1*1*1\r\n", expect_result=val("1*1*1"), **XT)
text(K, "get_output_endpoint_power", {"output": 2}, E + "O2POEC\r", device_reply="0*1*2\r\n", expect_result=val("0*1*2"), **XT)
text(K, "get_endpoint_power_budget", {}, E + "TPOEC\r", device_reply="PoecT1*0\r\n", expect_result=val("1*0"), **XT)
text(K, "set_receiver_relay", {"output": 4, "relay": 1, "state": "closed"}, E + "4*1*1RELY\r", device_reply="Rely4*1*1\r\n",
     expect_result=ACK, **XT)
text(K, "pulse_receiver_relay", {"output": 4, "relay": 2, "ticks": 31}, E + "4*2*3*31RELY\r", device_reply="Rely4*2*3\r\n",
     expect_result=ACK, **XT)
text(K, "get_receiver_relay", {"output": 4, "relay": 1}, E + "4*1RELY\r", device_reply="Rely4*1*1\r\n", expect_result=val("1"), **XT)
text(K, "set_transmitter_input", {"input": 5, "sub_input": 2, "what": "all"}, E + "5*2*3ETIE\r", device_reply="Etie5*2*2\r\n",
     expect_result=ACK, **XT)
text(K, "set_receiver_input", {"output": 3, "sub_input": 1, "what": "video"}, E + "R3*1*1ETIE\r",
     device_reply="EtieR3*1*2\r\n", expect_result=ACK, **XT)
text(K, "get_transmitter_inputs", {}, E + "ETIE\r", device_reply="Etie00*1*1 2*2 3*1\r\n", expect_result=val("1*1 2*2 3*1"), **XT)
text(K, "set_receiver_video_mute", {"output": 3, "muted": True}, E + "R3*1*1B\r", device_reply="VmtR3*1*1\r\n",
     expect_result=ACK, **XT)
text(K, "set_receiver_audio_mute", {"output": 3, "mode": 2}, E + "R3*1*2Z\r", device_reply="AmtR3*1*2\r\n",
     expect_result=ACK, **XT)
text(K, "set_transmitter_audio_source", {"input": 5, "sub_input": 2, "source": 1}, E + "T5*2*1AFMT\r",
     device_reply="AfmtT5*2*1\r\n", expect_result=ACK, **XT)
text(K, "set_transmitter_gain", {"input": 5, "sub_input": 1, "gain": 6}, E + "T5*1*6G\r", device_reply="AudT5*1*+6\r\n",
     expect_result=ACK, **XT)
text(K, "set_transmitter_attenuation", {"input": 5, "sub_input": 1, "attenuation": 3}, E + "T5*1*3g\r",
     device_reply="AudT5*1*-3\r\n", expect_result=ACK, **XT)
text(K, "set_receiver_volume", {"output": 3, "volume": 50}, E + "R3*1*50V\r", device_reply="VolR3*1*50\r\n",
     expect_result=ACK, **XT)
text(K, "set_receiver_hdcp_authorization", {"output": 3, "sub_input": 1, "enabled": True}, E + "RE3*1*1HDCP\r",
     device_reply="HdcpRE3*1*1\r\n", expect_result=ACK, **XT)
text(K, "set_transmitter_hdcp_authorization", {"input": 5, "sub_input": 2, "enabled": False}, E + "TE5*2*0HDCP\r",
     device_reply="HdcpTE5*2*0\r\n", expect_result=ACK, **XT)
text(K, "set_receiver_freeze", {"output": 3, "frozen": True}, E + "R3*1*1F\r", device_reply="FrzR3*1*1\r\n",
     expect_result=ACK, **XT)
text(K, "set_output_freeze", {"output": 3, "frozen": False}, E + "O3*0F\r", device_reply="FrzO3*0\r\n", expect_result=ACK, **XT)
text(K, "transmitter_image_reset", {"input": 5, "sub_input": 1}, E + "T5*1*2AADJ\r", device_reply="AadjT5*1*2\r\n",
     expect_result=ACK, **XT)
text(K, "receiver_image_reset", {"output": 3}, E + "R3*1*2AADJ\r", device_reply="AadjR3*1*2\r\n", expect_result=ACK, **XT)
text(K, "set_transmitter_front_panel_lock", {"input": 5, "mode": 1}, E + "T5*1X\r", device_reply="ExeT5*1\r\n",
     expect_result=ACK, **XT)
text(K, "set_receiver_front_panel_lock", {"output": 3, "mode": 0}, E + "R3*0X\r", device_reply="ExeR3*0\r\n",
     expect_result=ACK, **XT)
text(K, "reset_transmitter", {"input": 5}, E + "T5ZXXX\r", device_reply="ZpxT5\r\n", expect_result=ACK, **XT)
text(K, "reset_receiver", {"output": 3}, E + "R3ZXXX\r", device_reply="ZpxR3\r\n", expect_result=ACK, **XT)
text(K, "set_input_audio_source", {"input": 2, "source": 2}, E + "I2*2AFMT\r", device_reply="AfmtI2*2\r\n",
     expect_result=ACK, **XT)
text(K, "set_dhcp", {"enabled": False}, E + "0DH\r", device_reply="Idh0\r\n", expect_result=ACK, **D4)
text(K, "get_dhcp", {}, E + "DH\r", device_reply="Idh1\r\n", expect_result=val("1"), **DV)
text(K, "set_ip_address", {"address": "192.168.254.254"}, E + "192.168.254.254CI\r", device_reply="Ipi 192.168.254.254\r\n",
     expect_result=ACK, **D4)
text(K, "get_ip_address", {}, E + "CI\r", device_reply="Ipi192.168.254.254\r\n", expect_result=val("192.168.254.254"), **XT)
text(K, "set_subnet_mask", {"netmask": "255.255.0.0"}, E + "255.255.0.0CS\r", device_reply="Ips 255.255.0.0\r\n",
     expect_result=ACK, **SM)
text(K, "get_subnet_mask", {}, E + "CS\r", device_reply="Ips255.255.0.0\r\n", expect_result=val("255.255.0.0"), **DV)
text(K, "set_gateway", {"gateway": "192.168.254.1"}, E + "192.168.254.1CG\r", device_reply="Ipg192.168.254.1\r\n",
     expect_result=ACK, **XT)
text(K, "get_gateway", {}, E + "CG\r", device_reply="Ipg 0.0.0.0\r\n", expect_result=val("0.0.0.0"), **D4)
text(K, "get_mac_address", {}, E + "CH\r", device_reply="Iph 00-05-A6-12-34-56\r\n", expect_result=val("00-05-A6-12-34-56"), **D4)
text(K, "get_connection_count", {}, E + "CC\r", device_reply="Icc3\r\n", expect_result=val("3"), **DV)
text(K, "set_unit_name", {"name": "Rack-A-Matrix"}, E + "Rack-A-MatrixCN\r", device_reply="Ipn Rack-A-Matrix\r\n",
     expect_result=ACK, **XT)
text(K, "get_unit_name", {}, E + "CN\r", device_reply="Ipn DXP-HD-4K-12-34-56\r\n", expect_result=val("DXP-HD-4K-12-34-56"), **D4)
text(K, "set_date_time", {"date_time": "10/14/26 09:30:00"}, E + "10/14/26 09:30:00CT\r",
     device_reply="Ipt10/14/26 09:30:00\r\n", expect_result=ACK, **DV)
text(K, "get_date_time", {}, E + "CT\r", device_reply="Ipt Wed, 14 Oct 2026 09:30:00\r\n",
     expect_result=val("Wed, 14 Oct 2026 09:30:00"), **D4)
text(K, "set_gmt_offset", {"offset": "8.30"}, E + "8.30CZ\r", device_reply="Ipz+08:30\r\n", expect_result=ACK, **XT)
text(K, "get_gmt_offset", {}, E + "CZ\r", device_reply="Ipz-05:00\r\n", expect_result=val("-05:00"), **D4)
text(K, "set_daylight_saving", {"mode": 2}, E + "2CX\r", device_reply="Ipx2\r\n", expect_result=ACK, **SM)
text(K, "set_serial_port", {"port": 1, "baud": "38400", "parity": "N", "data_bits": "8", "stop_bits": "1"},
     E + "1*38400,N,8,1CP\r", device_reply="Cpn01 Ccp38400,N,8,1\r\n", expect_result=ACK, **XT)
text(K, "get_serial_port", {"port": 1}, E + "1CP\r", device_reply="Cpn01 Ccp9600,N,8,1\r\n", expect_result=val("9600,N,8,1"), **D4)
text(K, "set_serial_port_type", {"port": 1, "type": 1}, E + "1*1CY\r", device_reply="Cpn01 Cty1\r\n", expect_result=ACK, **DV)
text(K, "set_port_timeout", {"scope": "global", "steps": 60}, E + "1*60TC\r", device_reply="Pti1*60\r\n",
     expect_result=ACK, **D4)
text(K, "restart_network", {}, E + "2BOOT\r", device_reply="Boot2\r\n", expect_result=ACK, **D4)
text(K, "set_admin_password", {"password": "extron"}, E + "extronCA\r", device_reply="Ipa ****\r\n", expect_result=ACK, **XT)
text(K, "set_user_password", {"password": " "}, E + " CU\r", device_reply="Ipu \r\n", expect_result=ACK, **DV)
text(K, "smx_define_virtual_plane", {"plane": 90, "planes": "01*02*05"}, E + "90,01*02*05MP\r",
     device_reply="Mpv90,01,02,05\r\n", expect_result=ACK, **SM)
text(K, "smx_get_virtual_plane", {"plane": 90}, E + "90MP\r", device_reply="01,02,05\r\n", expect_result=val("01,02,05"), **SM)
text(K, "smx_get_slot_planes", {}, E + "STAT\r", device_reply="Stat00 01 -- 02 -- -- 03 -- 03 03\r\n",
     expect_result=val("00 01 -- 02 -- -- 03 -- 03 03"), **SM)
text(K, "smx_set_input_name", {"plane": 1, "input": 3, "name": "Camera 3"}, E + "1*3,Camera 3NI\r",
     device_reply="01Nmi03,Camera 3\r\n", expect_result=ACK, **SM)
text(K, "smx_set_output_name", {"plane": 1, "output": 2, "name": "Proj"}, E + "1*2,ProjNO\r",
     device_reply="01Nmo02,Proj\r\n", expect_result=ACK, **SM)
text(K, "smx_set_plane_preset_name", {"plane": 1, "preset": 4, "name": "Panel"}, E + "1*4,PanelNG\r",
     device_reply="01Nmg04,Panel\r\n", expect_result=ACK, **SM)
text(K, "smx_set_input_gain", {"plane": 1, "input": 3, "gain": 6}, "01*3*6G\r", device_reply="01In03 Aud+06\r\n",
     expect_result=ACK, **SM)
text(K, "smx_set_input_attenuation", {"plane": 1, "input": 3, "attenuation": 4}, "01*3*4g\r",
     device_reply="01In03 Aud-04\r\n", expect_result=ACK, **SM)
text(K, "smx_set_output_volume", {"plane": 1, "output": 2, "volume": 60}, "01*2*60V\r", device_reply="01Out02 Vol60\r\n",
     expect_result=ACK, **SM)
text(K, "smx_set_rgb_delay", {"plane": 2, "output": 1, "steps": 3}, E + "02*1*3D\r", device_reply="02Out01 Dly03\r\n",
     expect_result=ACK, **SM)
text(K, "smx_set_input_hdcp_authorization", {"slot": 3, "input": 2, "enabled": True}, E + "E3*2*1HDCP\r",
     device_reply="03HdcpE02*1\r\n", expect_result=ACK, **SM)
text(K, "smx_get_slot_signal", {"slot": 3}, "3*0LS\r", device_reply="03FRQ00*3 0 1 0\r\n", expect_result=val("3 0 1 0"), **SM)

telemetry(K, "edid-assignment", inbound="EdidA03*15\r\n", expect_state={"inputs": {"3": {"edid": 15}}})
telemetry(K, "smx-edid-assignment", inbound="02EdidA04*38\r\n",
          expect_state={"slots": {"2": {"inputs": {"4": {"edid": 38}}}}})
telemetry(K, "hdcp-authorization", inbound="HdcpE2*0\r\n", expect_state={"inputs": {"2": {"hdcp_authorized": False}}})
telemetry(K, "output-format", inbound="Vtpo2*3\r\n", expect_state={"outputs": {"2": {"format": 3}}})
telemetry(K, "room-outputs", inbound="Mpr1,1,2,5\r\n", expect_state={"rooms": {"1": {"outputs": "1,2,5"}}})
telemetry(K, "endpoint-power", inbound="PoecI03*1*2*1\r\n",
          expect_state={"inputs": {"3": {"endpoint_power_enabled": True, "endpoint_power_request": 2,
                                         "endpoint_power_status": 1}}})
telemetry(K, "relay", inbound="Rely4*2*1\r\n", expect_state={"outputs": {"4": {"relays": {"2": {"closed": True}}}}})
telemetry(K, "transmitter-inputs", inbound="Etie00*1*1 2*3\r\n",
          expect_state={"inputs": {"1": {"endpoint_video_input": 1, "endpoint_audio_input": 1},
                                   "2": {"endpoint_video_input": 2, "endpoint_audio_input": 3}}})
telemetry(K, "receiver-input", inbound="EtieR3*1*2\r\n",
          expect_state={"outputs": {"3": {"receiver_video_input": 1, "receiver_audio_input": 2}}})
telemetry(K, "receiver-mute", inbound="VmtR3*1*1\r\n", expect_state={"outputs": {"3": {"receiver_video_mute": True}}})
telemetry(K, "transmitter-level", inbound="AudT5*1*-3\r\n",
          expect_state={"inputs": {"5": {"endpoints": {"1": {"audio_level": -3}}}}})
telemetry(K, "transmitter-signal", inbound="LsT5*2*1\r\n", expect_state={"inputs": {"5": {"endpoints": {"2": {"signal": True}}}}})
telemetry(K, "receiver-volume", inbound="VolR3*1*50\r\n", expect_state={"outputs": {"3": {"receiver_volume": 50}}})
telemetry(K, "network", inbound="Ipi 192.168.254.254\r\n", expect_state={"network": {"address": "192.168.254.254"}})
telemetry(K, "mac", inbound="Iph 00-05-A6-12-34-56\r\n", expect_state={"network": {"mac": "00-05-A6-12-34-56"}})
telemetry(K, "dhcp", inbound="Idh1\r\n", expect_state={"network": {"dhcp": True}})
telemetry(K, "gmt", inbound="Ipz+08:30\r\n", expect_state={"clock": {"gmt_offset": "+08:30"}})
telemetry(K, "serial-port", inbound="Cpn01 Ccp9600,N,8,1\r\n", expect_state={"serial": {"1": {"settings": "9600,N,8,1"}}})
telemetry(K, "port-timeout", inbound="Pti1*60\r\n", expect_state={"port_timeout": {"global": 60}})
telemetry(K, "virtual-plane", inbound="Mpv90,01,02,05\r\n", expect_state={"planes": {"90": {"virtual_members": "01,02,05"}}})
telemetry(K, "slot-planes", inbound="Stat00 01 -- 02\r\n",
          expect_state={"slots": {"1": {"plane": "00"}, "2": {"plane": "01"}, "3": {"plane": "--"}, "4": {"plane": "02"}}})
telemetry(K, "smx-name", inbound="01Nmi03,Camera 3\r\n", expect_state={"planes": {"1": {"inputs": {"3": {"name": "Camera 3"}}}}})
telemetry(K, "smx-level", inbound="01In03 Aud+06\r\n", expect_state={"planes": {"1": {"inputs": {"3": {"audio_level": 6}}}}})
telemetry(K, "smx-volume", inbound="01Out02 Vol60\r\n", expect_state={"planes": {"1": {"outputs": {"2": {"volume": 60}}}}})
telemetry(K, "signal-dvi", inbound="Frq00 3 0 3 3\r\n",
          expect_state={"inputs": {"1": {"signal": True}, "2": {"signal": False}, "3": {"signal": True},
                                   "4": {"signal": True}}})
telemetry(K, "part-number-dvi", inbound="Pno43-265-01\r\n", expect_state={"device": {"part_number": "43-265-01"}})

# ── MMX 42/62 (68-778-01 Rev. C, p.4-5 to 4-6): bare replies, no verbose mode ──
_seen_mmx = {}


def _tm(command, *args, **extra):
    # A second file per command, so the other models' vectors are kept.
    n = _seen_mmx.get(command, 0) + 1
    _seen_mmx[command] = n
    text(K, command, *args, file=f"{command}.mmx" if n == 1 else f"{command}.mmx-{n}", **extra)


MM = {"model": "mmx-42-62"}
# The MMX has an RS-232 port only: these vectors play over a
# serial-to-Ethernet interface's TCP port.
_tm("tie", {"input": 5, "output": 2}, "5*2!\r", device_reply="Out02 In05 All\r\n", expect_result=ACK, **MM)
_tm("tie_video", {"input": 3, "output": 1}, "3*1%\r", device_reply="Out01 In03 Vid\r\n", expect_result=ACK, **MM)
_tm("tie_audio", {"input": 6, "output": 1}, "6*1$\r", device_reply="Out01 In06 Aud\r\n", expect_result=ACK, **MM)
_tm("tie", {"input": 7, "output": 1}, "7*1!\r", device_reply="E01\r\n", expect_result=ERR, **MM)
_tm("get_video_tie", {"output": 2}, "2%\r", device_reply="In05\r\n", expect_result=val("05"), **MM)
_tm("get_audio_tie", {"output": 1}, "1$\r", device_reply="In00\r\n", expect_result=val("00"), **MM)
_tm("set_video_mute", {"output": 1, "muted": True}, "1*1B\r", device_reply="Vmt1*1\r\n", expect_result=ACK, **MM)
_tm("get_video_mute", {"output": 1}, "1B\r", device_reply="1\r\n", expect_result=val("1"), **MM)
_tm("set_video_mute_all", {"muted": True}, "1*B\r", device_reply="Vmt1\r\n", expect_result=ACK, **MM)
_tm("set_audio_mute", {"output": 2, "muted": False}, "2*0Z\r", device_reply="Amt2*0\r\n", expect_result=ACK, **MM)
_tm("get_audio_mute", {"output": 2}, "2Z\r", device_reply="0\r\n", expect_result=val("0"), **MM)
_tm("set_audio_mute_all", {"muted": False}, "0*Z\r", device_reply="Amt0\r\n", expect_result=ACK, **MM)
_tm("set_input_gain", {"input": 4, "gain": 3}, "4*3G\r", device_reply="In04 Aud+03\r\n", expect_result=ACK, **MM)
_tm("set_input_attenuation", {"input": 2, "attenuation": 6}, "2*6g\r", device_reply="In02 Aud-06\r\n",
     expect_result=ACK, **MM)
_tm("increase_input_level", {"input": 4}, "4+G\r", device_reply="In04 Aud+04\r\n", expect_result=ACK, **MM)
_tm("decrease_input_level", {"input": 4}, "4-G\r", device_reply="In04 Aud+02\r\n", expect_result=ACK, **MM)
_tm("get_input_level", {"input": 4}, "4G\r", device_reply="+03\r\n", expect_result=val("+03"), **MM)
_tm("set_front_panel_lock", {"mode": 1}, "1X\r", device_reply="Exe1\r\n", expect_result=ACK, **MM)
_tm("get_front_panel_lock", {}, "X\r", device_reply="0\r\n", expect_result=val("0"), **MM)
_tm("get_info", {}, "I\r", device_reply="V1*03 A1*03 V2*05 A2*06 Vmt1*0 Amt1*0 Vmt2*1 Amt2*0\r\n",
     expect_result=val("V1*03 A1*03 V2*05 A2*06 Vmt1*0 Amt1*0 Vmt2*1 Amt2*0"), **MM)
_tm("get_part_number", {}, "N\r", device_reply="60-655-01\r\n", expect_result=val("60-655-01"), **MM)
_tm("get_firmware", {}, "Q\r", device_reply="1.02\r\n", expect_result=val("1.02"), **MM)
_tm("mmx_reset_audio_levels", {}, E + "ZA\r", device_reply="Zpa\r\n", expect_result=ACK, **MM)
_tm("mmx_unmute_all_video", {}, E + "ZB\r", device_reply="Zpb\r\n", expect_result=ACK, **MM)
_tm("mmx_unmute_all_audio", {}, E + "ZZ\r", device_reply="Zpz\r\n", expect_result=ACK, **MM)

telemetry(K, "mmx-info", inbound="V1*03 A1*03 V2*05 A2*06 Vmt1*0 Amt1*0 Vmt2*1 Amt2*0\r\n", model="mmx-42-62",
          expect_state={"outputs": {"1": {"video_input": 3, "audio_input": 3, "video_mute": False, "audio_mute": False},
                                    "2": {"video_input": 5, "audio_input": 6, "video_mute": True,
                                          "audio_mute": False}}})
telemetry(K, "mmx-front-panel-tie", inbound="Out02 In04 All\r\n", model="mmx-42-62",
          expect_state={"outputs": {"2": {"video_input": 4, "audio_input": 4}}})
telemetry(K, "mmx-bare-video-tie", inbound="In05\r\n", request="2%", model="mmx-42-62",
          expect_state={"outputs": {"2": {"video_input": 5}}})
telemetry(K, "mmx-bare-audio-tie", inbound="In01\r\n", request="1$", model="mmx-42-62",
          expect_state={"outputs": {"1": {"audio_input": 1}}})
telemetry(K, "mmx-bare-video-mute", inbound="1\r\n", request="2B", model="mmx-42-62",
          expect_state={"outputs": {"2": {"video_mute": True}}})
telemetry(K, "mmx-bare-audio-mute", inbound="0\r\n", request="1Z", model="mmx-42-62",
          expect_state={"outputs": {"1": {"audio_mute": False}}})
telemetry(K, "mmx-bare-level", inbound="-06\r\n", request="2G", model="mmx-42-62",
          expect_state={"inputs": {"2": {"audio_level": -6}}})
telemetry(K, "mmx-bare-lock", inbound="1\r\n", request="X", model="mmx-42-62", expect_state={"front_panel_lock": 1})
telemetry(K, "mmx-global-mute", inbound="Vmt1\r\n", model="mmx-42-62", expect_state={}, expect_then_send=["I\r"])
telemetry(K, "mmx-reset-video-mutes", inbound="Zpb\r\n", model="mmx-42-62",
          expect_state={"outputs": {"1": {"video_mute": False}, "2": {"video_mute": False}}})
telemetry(K, "mmx-reset-audio-mutes", inbound="Zpz\r\n", model="mmx-42-62",
          expect_state={"outputs": {"1": {"audio_mute": False}, "2": {"audio_mute": False}}})
telemetry(K, "mmx-reset-levels", inbound="Zpa\r\n", model="mmx-42-62",
          expect_state={"inputs": {str(n): {"audio_level": 0} for n in range(1, 7)}})

# ── MTPX Plus (68-1383-01 Rev. B), MVX VGA A (68-877-01 Rev. E, 68-521-31
# Rev. C) and MMX 32 VGA A (68-789-01 Rev. D): commands written from the
# guides' syntax lines and examples ──
XM_MT = {"model": "mtpx-plus"}
XM_MV = {"model": "mvx-vga-a"}
XM_ML = {"model": "mvx-vga-a-large"}
XM_M3 = {"model": "mmx-32-vga-a"}
_seen_xm = {}


def _xm(command, *args, **extra):
    # A second file per command and model, so earlier vectors are kept.
    tag = extra.get("model", "x")
    key = (command, tag)
    n = _seen_xm.get(key, 0) + 1
    _seen_xm[key] = n
    text(K, command, *args, file=f"{command}.{tag}" if n == 1 else f"{command}.{tag}-{n}", **extra)


# New commands (their own files).
text(K, "get_preset_ties", {"preset": 4}, E + "4VC\r", device_reply="6 5 6 8 3 3 1 0 Vid 8 1 1 1 8 8 8 8 Aud\r\n",
     expect_result=val("6 5 6 8 3 3 1 0 Vid 8 1 1 1 8 8 8 8 Aud"))
text(K, "write_global_preset", {"preset": 27, "ties": "12*5!10*09%3*2$3*8&"}, E + "+27P12*5!10*09%3*2$3*8&\r",
     device_reply="Spr27\r\n", expect_result=ACK)
text(K, "write_global_preset", {"preset": 16, "ties": "8*5!07*04%3*6$3*8&"}, E + "*16P8*5!07*04%3*6$3*8&\r",
     device_reply="Spr16\r\n", expect_result=ACK, file="write_global_preset-mvx", **XM_MV)
text(K, "write_room_preset", {"room": 7, "preset": 3, "ties": "12*7&11*5$4*5%6*6!"}, E + "+7*3P12*7&11*5$4*5%6*6!\r",
     device_reply="Rmm07 Spr03\r\n", expect_result=ACK)
text(K, "get_room_preset_ties", {"room": 3, "preset": 2, "plane": "1"}, E + "3*2*1*1VC\r")
text(K, "set_output_audio_level", {"output": 3, "level": "consumer"}, "3*0*40#\r", device_reply="Out3 Lvl0\r\n",
     expect_result=ACK)
text(K, "get_output_audio_level", {"output": 3}, "3*40#\r", device_reply="1\r\n", expect_result=val("1"))
text(K, "set_rgb_delay", {"output": 5, "steps": 7}, E + "5*7D\r", device_reply="Out5 Dly07\r\n", expect_result=ACK)
text(K, "get_rgb_delay", {"output": 6}, E + "6D\r", device_reply="05\r\n", expect_result=val("05"))
text(K, "get_firmware_verbose", {}, "0Q\r", device_reply="1.23-1.00(1.06-16x16 Series -Tue, 04 Dec 2007 00:00:00 GMT)\r\n",
     expect_result=val("1.23-1.00(1.06-16x16 Series -Tue, 04 Dec 2007 00:00:00 GMT)"))
text(K, "clear_global_presets", {}, E + "ZG\r", device_reply="Zpg\r\n", expect_result=ACK)
text(K, "clear_global_preset", {"preset": 5}, E + "5ZG\r", device_reply="Zpg05\r\n", expect_result=ACK)
text(K, "reset_output_volumes", {}, E + "ZV\r", device_reply="Zpv\r\n", expect_result=ACK)
text(K, "reset_all_mutes", {}, E + "ZZ\r", device_reply="Zpz\r\n", expect_result=ACK)
text(K, "reset_rgb_delays", {}, E + "ZD\r", device_reply="Zpd\r\n", expect_result=ACK)
text(K, "reset_level_peaking", {}, E + "ZT\r", device_reply="Zpt\r\n", expect_result=ACK)
text(K, "reset_skew", {}, E + "ZK\r", device_reply="Zpk\r\n", expect_result=ACK)
text(K, "clear_rooms", {}, E + "ZR\r", device_reply="Zpr\r\n", expect_result=ACK)
text(K, "clear_room", {"room": 4}, E + "4ZR\r", device_reply="Zpr04\r\n", expect_result=ACK)
text(K, "clear_room_presets", {}, E + "ZP\r", device_reply="Zpp\r\n", expect_result=ACK)
text(K, "clear_room_preset", {"room": 2, "preset": 3}, E + "2*3ZP\r", device_reply="Zpp02*03\r\n", expect_result=ACK)
text(K, "set_tp_input_type", {"input": 4, "type": "rs232"}, "4*1\\\r", device_reply="Typ04*1\r\n", expect_result=ACK)
text(K, "get_tp_input_type", {"input": 4}, "4\\\r", device_reply="1\r\n", expect_result=val("1"))
text(K, "set_serial_insert", {"port": 2, "enabled": True}, E + "2*1Lrpt\r", device_reply="Lrpt02*1\r\n", expect_result=ACK)
text(K, "get_serial_insert", {"port": 2}, E + "2Lrpt\r", device_reply="0\r\n", expect_result=val("0"))
text(K, "set_input_peaking", {"input": 3, "level": 128}, E + "3*128Ipek\r", device_reply="Ipek03*128\r\n",
     expect_result=ACK)
text(K, "increase_input_peaking", {"input": 3}, E + "3+Ipek\r", device_reply="Ipek03*129\r\n", expect_result=ACK)
text(K, "decrease_input_peaking", {"input": 3}, E + "3-Ipek\r", device_reply="Ipek03*127\r\n", expect_result=ACK)
text(K, "get_input_peaking", {"input": 3}, E + "3Ipek\r", device_reply="127\r\n", expect_result=val("127"))
text(K, "auto_calibrate_input", {"input": 2}, E + "2*0AADJ\r", device_reply="Aadj02*2\r\n", expect_result=ACK)
text(K, "set_input_skew", {"input": 2, "red": 0, "green": 0, "blue": 4}, E + "2*0*0*4Iseq\r",
     device_reply="Iseq02*00*00*04\r\n", expect_result=ACK)
text(K, "adjust_input_skew", {"input": 2, "plane": "2", "direction": "+"}, E + "2*2+Iseq\r",
     device_reply="Iseq02*00*00*05\r\n", expect_result=ACK)
text(K, "get_input_skew", {"input": 2}, E + "2Iseq\r", device_reply="00*00*05\r\n", expect_result=val("00*00*05"))
text(K, "set_output_skew", {"output": 2, "red": 0, "green": 0, "blue": 4}, E + "2*0*0*4Oseq\r",
     device_reply="Oseq02*0*0*4\r\n", expect_result=ACK)
text(K, "adjust_output_skew", {"output": 2, "plane": "2", "direction": "-"}, E + "2*2-Oseq\r",
     device_reply="Oseq02*0*0*3\r\n", expect_result=ACK)
text(K, "get_output_skew", {"output": 2}, E + "2Oseq\r", device_reply="0*0*3\r\n", expect_result=val("0*0*3"))
text(K, "set_output_prepeak", {"output": 4, "enabled": True}, E + "4*1Opek\r", device_reply="Opek04*1\r\n",
     expect_result=ACK)
text(K, "get_output_prepeak", {"output": 4}, E + "4Opek\r", device_reply="1\r\n", expect_result=val("1"))
text(K, "set_local_sync_polarity", {"polarity": 0}, E + "0Opol\r", device_reply="Opol00\r\n", expect_result=ACK)
text(K, "set_local_output_sync_polarity", {"output": 2, "polarity": 0}, E + "2*0Opol\r", device_reply="Opol2*0\r\n",
     expect_result=ACK)
text(K, "get_local_sync_polarity", {}, E + "Opol\r", device_reply="3\r\n", expect_result=val("3"))
text(K, "get_local_output_sync_polarity", {"output": 2}, E + "2Opol\r", device_reply="4\r\n", expect_result=val("4"))
text(K, "set_input_groups", {"groups": "4013300004440443"}, E + "4013300004440443I\r",
     device_reply="GrI 4 0 1 3 3 0 0 0 0 4 4 4 0 4 4 3\r\n", expect_result=ACK)
text(K, "set_output_groups", {"groups": "1111222200000000"}, E + "1111222200000000O\r",
     device_reply="Gro1111222200000000\r\n", expect_result=ACK)
text(K, "get_input_groups", {}, E + "I\r", device_reply="1 1 1 3 3 0 0 0 0 4 4 4 0 4 4 4\r\n",
     expect_result=val("1 1 1 3 3 0 0 0 0 4 4 4 0 4 4 4"))
text(K, "get_output_groups", {}, E + "O\r", device_reply="0000000000000000\r\n", expect_result=val("0000000000000000"))
text(K, "get_input_select_switches", {}, E + "Stat\r", device_reply="001*1\r\n", expect_result=val("001*1"))

# Existing commands on the new models.
_xm("tie", {"input": 1, "output": 3}, "1*3!\r", device_reply="Out03 In01 All\r\n", expect_result=ACK, **XM_MT)
_xm("tie", {"input": 1, "output": 3}, "1*3!\r", device_reply="Out3 In1 All\r\n", expect_result=ACK, **XM_MV)
_xm("tie", {"input": 1, "output": 2}, "1*2!\r", device_reply="OUT02 IN01 ALL\r\n", expect_result=ACK, **XM_M3)
_xm("tie_video", {"input": 3, "output": 1}, "3*1%\r", device_reply="OUT01 IN3 RGB\r\n", expect_result=ACK, **XM_M3)
_xm("tie_audio", {"input": 1, "output": 2}, "1*2$\r", device_reply="OUT02 IN1 AUD\r\n", expect_result=ACK, **XM_M3)
_xm("set_video_mute", {"output": 2, "muted": True}, "2*1B\r", device_reply="VMT2*1\r\n", expect_result=ACK, **XM_M3)
_xm("set_audio_mute_all", {"muted": True}, "1*Z\r", device_reply="AMT1\r\n", expect_result=ACK, **XM_M3)
_xm("get_video_tie", {"output": 7}, "7%\r", device_reply="02\r\n", expect_result=val("02"), **XM_ML)
_xm("get_mutes", {}, E + "VM\r", device_reply="0132Mut\r\n", expect_result=val("0132"), **XM_MV)
_xm("set_output_volume", {"output": 1, "level": 50}, "1*50V\r", device_reply="Out01 Vol50\r\n", expect_result=ACK, **XM_ML)
_xm("set_input_gain", {"input": 1, "gain": 2}, "1*2G\r", device_reply="In1 Aud+02\r\n", expect_result=ACK, **XM_MV)
_xm("recall_room_preset", {"room": 3, "preset": 2}, "3*2.\r", device_reply="Rmm03 Rpr02\r\n", expect_result=ACK, **XM_MT)

# Telemetry.
telemetry(K, "mvx-ties", inbound="6 5 6 8 3 3 1 0 Vid 8 1 1 1 8 8 8 8 Aud\r\n", request=E + "0VC", model="mvx-vga-a",
          expect_state={"outputs": {"1": {"video_input": 6, "audio_input": 8}, "2": {"video_input": 5, "audio_input": 1},
                                    "3": {"video_input": 6, "audio_input": 1}, "4": {"video_input": 8, "audio_input": 1},
                                    "5": {"video_input": 3, "audio_input": 8}, "6": {"video_input": 3, "audio_input": 8},
                                    "7": {"video_input": 1, "audio_input": 8}, "8": {"video_input": 0, "audio_input": 8}}})
telemetry(K, "mvx-large-video-ties", inbound="08 08 12 08 08 11 00 00 -- -- -- -- -- -- -- -- Vid\r\n",
          request=E + "0*1*1VC", model="mvx-vga-a-large",
          expect_state={"outputs": {"1": {"video_input": 8}, "2": {"video_input": 8}, "3": {"video_input": 12},
                                    "4": {"video_input": 8}, "5": {"video_input": 8}, "6": {"video_input": 11},
                                    "7": {"video_input": 0}, "8": {"video_input": 0}}})
telemetry(K, "mtpx-upper-audio-ties", inbound="01 01 01 01 02 12 12 00 00 00 00 00 01 08 15 16 Aud\r\n",
          request=E + "0*17*2VC", model="mtpx-plus",
          expect_state={"outputs": {str(17 + i): {"audio_input": v} for i, v in
                                    enumerate([1, 1, 1, 1, 2, 12, 12, 0, 0, 0, 0, 0, 1, 8, 15, 16])}})
telemetry(K, "mvx-mutes", inbound="0132Mut\r\n", model="mvx-vga-a",
          expect_state={"outputs": {"1": {"video_mute": False, "audio_mute": False},
                                    "2": {"video_mute": True, "audio_mute": False},
                                    "3": {"video_mute": True, "audio_mute": True},
                                    "4": {"video_mute": False, "audio_mute": True}}})
telemetry(K, "mvx-large-mutes", inbound="0 2 3 0\r\n", request=E + "VM", model="mvx-vga-a-large",
          expect_state={"outputs": {"1": {"video_mute": False, "audio_mute": False},
                                    "2": {"video_mute": False, "audio_mute": True},
                                    "3": {"video_mute": True, "audio_mute": True},
                                    "4": {"video_mute": False, "audio_mute": False}}})
telemetry(K, "mvx-front-panel-tie", inbound="Qik\r\n", model="mvx-vga-a", expect_state={},
          expect_then_send=[E + "0VC\r"])
telemetry(K, "mvx-large-front-panel-tie", inbound="Qik\r\n", model="mvx-vga-a-large", expect_state={},
          expect_then_send=[E + "0*1*1VC\r", E + "0*1*2VC\r"])
telemetry(K, "mtpx-front-panel-tie", inbound="Qik\r\n", model="mtpx-plus", expect_state={},
          expect_then_send=[E + "0*1*1VC\r", E + "0*1*2VC\r", E + "0*17*1VC\r", E + "0*17*2VC\r"])
telemetry(K, "mvx-recall", inbound="Rpr05\r\n", model="mvx-vga-a", expect_state={"presets": {"last_recalled": 5}},
          expect_then_send=[E + "0VC\r"])
telemetry(K, "mvx-reset-mutes", inbound="Zpz\r\n", model="mvx-vga-a", expect_state={},
          expect_then_send=[E + "VM\r"])
telemetry(K, "mmx32-reset-mutes", inbound="Zpz\r\n", model="mmx-32-vga-a", expect_state={},
          expect_then_send=["I\r"])
telemetry(K, "mmx32-tie", inbound="OUT02 IN01 ALL\r\n", model="mmx-32-vga-a",
          expect_state={"outputs": {"2": {"video_input": 1, "audio_input": 1}}})
telemetry(K, "mmx32-info", inbound="V1*2 A1*2 V2*3 A2*1 Vmt1*0 Amt1*1 Vmt2*0 Amt2*0\r\n", model="mmx-32-vga-a",
          expect_state={"outputs": {"1": {"video_input": 2, "audio_input": 2, "video_mute": False, "audio_mute": True},
                                    "2": {"video_input": 3, "audio_input": 1, "video_mute": False, "audio_mute": False}}})
telemetry(K, "mvx-level", inbound="Out3 Lvl1\r\n", model="mvx-vga-a", expect_state={"outputs": {"3": {"audio_output_level": 1}}})
telemetry(K, "mvx-bare-level", inbound="0\r\n", request="2*40#", model="mvx-vga-a",
          expect_state={"outputs": {"2": {"audio_output_level": 0}}})
telemetry(K, "mvx-delay", inbound="Out5 Dly07\r\n", model="mvx-vga-a", expect_state={"outputs": {"5": {"rgb_delay": 7}}})
telemetry(K, "mvx-bare-delay", inbound="05\r\n", request=E + "6D", model="mvx-vga-a",
          expect_state={"outputs": {"6": {"rgb_delay": 5}}})
telemetry(K, "mvx-bare-video-tie", inbound="12\r\n", request="5&", model="mvx-vga-a-large",
          expect_state={"outputs": {"5": {"video_input": 12}}})
telemetry(K, "mvx-bare-audio-tie", inbound="06\r\n", request="3$", model="mvx-vga-a-large",
          expect_state={"outputs": {"3": {"audio_input": 6}}})
telemetry(K, "mvx-bare-gain", inbound="-02\r\n", request="4G", model="mvx-vga-a-large",
          expect_state={"inputs": {"4": {"audio_level": -2}}})
telemetry(K, "mvx-bare-volume", inbound="55\r\n", request="7V", model="mvx-vga-a-large",
          expect_state={"outputs": {"7": {"volume": 55}}})
telemetry(K, "mtpx-tp-type", inbound="Typ04*1\r\n", model="mtpx-plus", expect_state={"inputs": {"4": {"tp_type": 1}}})
telemetry(K, "mtpx-bare-tp-type", inbound="0\r\n", request="4\\", model="mtpx-plus",
          expect_state={"inputs": {"4": {"tp_type": 0}}})
telemetry(K, "mtpx-insert", inbound="Lrpt02*1\r\n", model="mtpx-plus", expect_state={"serial_inserts": {"2": {"enabled": True}}})
telemetry(K, "mtpx-bare-insert", inbound="0\r\n", request=E + "2Lrpt", model="mtpx-plus",
          expect_state={"serial_inserts": {"2": {"enabled": False}}})
telemetry(K, "mtpx-peaking", inbound="Ipek03*128\r\n", model="mtpx-plus", expect_state={"inputs": {"3": {"peaking": 128}}})
telemetry(K, "mtpx-bare-peaking", inbound="127\r\n", request=E + "3Ipek", model="mtpx-plus",
          expect_state={"inputs": {"3": {"peaking": 127}}})
telemetry(K, "mtpx-calibration", inbound="Aadj02*1\r\n", model="mtpx-plus", expect_state={"inputs": {"2": {"calibration": 1}}})
telemetry(K, "mtpx-input-skew", inbound="Iseq02*00*00*04\r\n", model="mtpx-plus",
          expect_state={"inputs": {"2": {"skew": "00*00*04"}}})
telemetry(K, "mtpx-bare-input-skew", inbound="00*00*05\r\n", request=E + "2Iseq", model="mtpx-plus",
          expect_state={"inputs": {"2": {"skew": "00*00*05"}}})
telemetry(K, "mtpx-output-skew", inbound="Oseq02*0*0*4\r\n", model="mtpx-plus",
          expect_state={"outputs": {"2": {"skew": "0*0*4"}}})
telemetry(K, "mtpx-bare-output-skew", inbound="0*0*3\r\n", request=E + "2Oseq", model="mtpx-plus",
          expect_state={"outputs": {"2": {"skew": "0*0*3"}}})
telemetry(K, "mtpx-prepeak", inbound="Opek04*1\r\n", model="mtpx-plus", expect_state={"outputs": {"4": {"prepeak": True}}})
telemetry(K, "mtpx-bare-prepeak", inbound="0\r\n", request=E + "4Opek", model="mtpx-plus",
          expect_state={"outputs": {"4": {"prepeak": False}}})
telemetry(K, "mtpx-polarity", inbound="Opol00\r\n", model="mtpx-plus", expect_state={"local_outputs": {"1": {"sync_polarity": 0}}})
telemetry(K, "mtpx-polarity-2", inbound="Opol2*3\r\n", model="mtpx-plus", expect_state={"local_outputs": {"2": {"sync_polarity": 3}}})
telemetry(K, "mtpx-bare-polarity", inbound="3\r\n", request=E + "Opol", model="mtpx-plus",
          expect_state={"local_outputs": {"1": {"sync_polarity": 3}}})
telemetry(K, "mtpx-bare-polarity-2", inbound="4\r\n", request=E + "2Opol", model="mtpx-plus",
          expect_state={"local_outputs": {"2": {"sync_polarity": 4}}})
telemetry(K, "mtpx-input-groups", inbound="Gri4013300004440443\r\n", model="mtpx-plus",
          expect_state={"groups": {"inputs": "4013300004440443"}})
telemetry(K, "mtpx-output-groups", inbound="Gro1111222200000000\r\n", model="mtpx-plus",
          expect_state={"groups": {"outputs": "1111222200000000"}})
telemetry(K, "mtpx-bare-input-groups", inbound="1 1 1 3\r\n", request=E + "I", model="mtpx-plus",
          expect_state={"groups": {"inputs": "1 1 1 3"}})
telemetry(K, "mtpx-bare-output-groups", inbound="0000\r\n", request=E + "O", model="mtpx-plus",
          expect_state={"groups": {"outputs": "0000"}})
telemetry(K, "mtpx-switches", inbound="001*1\r\n", request=E + "Stat", model="mtpx-plus",
          expect_state={"input_select_switches": "001"})
telemetry(K, "mtpx-room-recall", inbound="Rmm03 Rpr02\r\n", model="mtpx-plus",
          expect_state={"rooms": {"3": {"last_recalled": 2}}},
          expect_then_send=[E + "0*1*1VC\r", E + "0*1*2VC\r", E + "0*17*1VC\r", E + "0*17*2VC\r"])
