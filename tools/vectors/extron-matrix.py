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
text(K, "tie", {"input": 0, "output": 4}, "0*4!\r", device_reply="Out04 In00 All\r\n", expect_result=ACK, **DV)
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
text(K, "set_audio_mute_mode", {"output": 9, "mode": 3}, "9*3Z\r", device_reply="E12\r\n", expect_result=ERR, **XT)
text(K, "get_audio_mute", {"output": 1}, "1Z\r", device_reply="Amt1*2\r\n", expect_result=val("2"))
text(K, "set_audio_mute_all", {"muted": True}, "1*Z\r", device_reply="Amt1\r\n", expect_result=ACK)
text(K, "get_mutes", {}, E + "VM\r", device_reply="Mut01002300\r\n", expect_result=val("01002300"), **DV)

# Presets and rooms (DXP HD 4K PLUS p.48-50, DXP DVI p.58-60, XTP II p.52)
text(K, "save_global_preset", {"preset": 9}, "9,\r", device_reply="Spr09\r\n", expect_result=ACK)
text(K, "recall_global_preset", {"preset": 5}, "5.\r", device_reply="Rpr05\r\n", expect_result=ACK)
text(K, "recall_global_preset", {"preset": 6}, "6.\r", device_reply="E11\r\n", expect_result=ERR, **SM)
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
text(K, "set_output_volume", {"output": 1, "level": 90}, "1*90V\r", device_reply="E13\r\n", expect_result=ERR, **XT)
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
text(K, "get_firmware", {}, "Q\r", device_reply="Ver01*1.14\r\n", expect_result=val("1.14"), **SM)
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
