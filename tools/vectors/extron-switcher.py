K = "extron-switcher"
# Extron IN-series scaling presentation switchers (IN1806/IN1808 68-3131-01
# Rev. B, IN1606/IN1608 68-2290-01 Rev. J). Every command is sent followed by
# CR; Esc is 0x1B. Replies are the verbose mode 3 (tagged) forms of the
# command and response tables.
E = "\x1b"
N8 = {"model": "in1808-series"}
N6 = {"model": "in1608-series"}
ACK = {"ok": {"kind": "ack"}}
ERR = {"error": {"error": "device_error"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


# Input selection (IN1808 p.63, IN1608 p.50)
text(K, "select_input", {"input": 3}, "3*1!\r", device_reply="In03*1 All\r\n", expect_result=ACK, **N8)
text(K, "select_input", {"input": 8}, "8*1!\r", device_reply="E01\r\n", expect_result=ERR, **N8)
text(K, "select_video_input", {"input": 2}, "2*1%\r", device_reply="In02*1 Vid\r\n", expect_result=ACK, **N8)
text(K, "select_audio_input", {"input": 9}, "9*1$\r", device_reply="In09*1 Aud\r\n", expect_result=ACK, **N8)
text(K, "get_input", {}, "1!\r", device_reply="In04*1 All\r\n", expect_result=val("4"), **N8)
text(K, "get_video_input", {}, "1%\r", device_reply="05\r\n", expect_result=val("5"), **N8)
text(K, "get_audio_input", {}, "1$\r", device_reply="In09*1 Aud\r\n", expect_result=val("9"), **N8)
text(K, "select_input_in1600", {"input": 4}, "4!\r", device_reply="In4 All\r\n", expect_result=ACK, **N6)
text(K, "select_video_input_in1600", {"input": 1}, "1&\r", device_reply="In1 RGB\r\n", expect_result=ACK, **N6)
text(K, "select_audio_input_in1600", {"input": 5}, "5$\r", device_reply="E17\r\n", expect_result=ERR, **N6)
text(K, "get_input_in1600", {}, "!\r", device_reply="In6 All\r\n", expect_result=val("6"), **N6)
text(K, "get_video_input_in1600", {}, "&\r", device_reply="3\r\n", expect_result=val("3"), **N6)
text(K, "get_audio_input_in1600", {}, "$\r", device_reply="In2 Aud\r\n", expect_result=val("2"), **N6)

# Loop out and names (IN1808 p.63, p.69; IN1608 p.51)
text(K, "set_loop_out_input", {"input": 5}, E + "5LOUT\r", device_reply="Lout05\r\n", expect_result=ACK, **N8)
text(K, "get_loop_out_input", {}, E + "LOUT\r", device_reply="Lout01\r\n", expect_result=val("1"), **N8)
text(K, "set_input_name", {"input": 2, "name": "Lectern PC"}, E + "I2*Lectern PCVNAM\r",
     device_reply="VnamI02*Lectern PC\r\n", expect_result=ACK, **N8)
text(K, "get_input_name", {"input": 2}, E + "I2VNAM\r", device_reply="VnamI02*Lectern PC\r\n",
     expect_result=val("Lectern PC"), **N8)
text(K, "set_output_name", {"output": 1, "name": "Projector"}, E + "O1*ProjectorVNAM\r",
     device_reply="VnamO01*Projector\r\n", expect_result=ACK, **N8)
text(K, "get_output_name", {"output": 3}, E + "O3VNAM\r", device_reply="VnamO03*Video Output 3\r\n",
     expect_result=val("Video Output 3"), **N8)
text(K, "set_input_name_in1600", {"input": 3, "name": "Laptop"}, E + "3,LaptopNI\r",
     device_reply="Nmi3,Laptop\r\n", expect_result=ACK, **N6)
text(K, "get_input_name_in1600", {"input": 3}, E + "3NI\r", device_reply="Nmi3,Laptop\r\n",
     expect_result=val("Laptop"), **N6)

# Input configuration (IN1808 p.64-66, IN1608 p.52-53, p.60)
text(K, "auto_image", {"mode": "1"}, "1*1A\r", device_reply="Img1*1\r\n", expect_result=ACK, **N8)
text(K, "auto_image", {}, "1*0A\r", **N8)
text(K, "auto_image_in1600", {}, "A\r", device_reply="Img0\r\n", expect_result=ACK, **N6)
text(K, "auto_image_fill_in1600", {}, "1*A\r", device_reply="Img1\r\n", expect_result=ACK, **N6)
text(K, "auto_image_keep_aspect_in1600", {}, "2*A\r", device_reply="Img2\r\n", expect_result=ACK, **N6)
text(K, "set_auto_image", {"input": 4, "enabled": True}, "4*1A\r", device_reply="Img4*1\r\n", expect_result=ACK, **N6)
text(K, "get_auto_image", {"input": 4}, "4A\r", device_reply="Img4*0\r\n", expect_result=val("0"), **N6)
text(K, "set_input_hdcp_authorized", {"input": 3, "allowed": False}, E + "E3*0HDCP\r",
     device_reply="HdcpE03*0\r\n", expect_result=ACK, **N8)
text(K, "get_input_hdcp_authorized", {"input": 3}, E + "E3HDCP\r", device_reply="HdcpE3*1\r\n",
     expect_result=val("1"), **N6)
text(K, "set_aspect_ratio", {"input": 2, "mode": 2}, E + "2*2ASPR\r", device_reply="Aspr02*2\r\n",
     expect_result=ACK, **N8)
text(K, "get_aspect_ratio", {"input": 2}, E + "2ASPR\r", device_reply="Aspr2*1\r\n", expect_result=val("1"), **N6)
text(K, "set_film_mode", {"input": 5, "enabled": False}, E + "5*0FILM\r", device_reply="Film05*0\r\n",
     expect_result=ACK, **N8)
text(K, "get_film_mode", {"input": 5}, E + "5FILM\r", device_reply="Film05*1\r\n", expect_result=val("1"), **N8)
text(K, "set_auto_switch_mode", {"mode": 2}, E + "2AUSW\r", device_reply="Ausw2\r\n", expect_result=ACK, **N8)
text(K, "get_auto_switch_mode", {}, E + "AUSW\r", device_reply="Ausw0\r\n", expect_result=val("0"), **N6)
text(K, "set_auto_switch_timeout", {"seconds": 10}, E + "T10AUSW\r", device_reply="AuswT010\r\n",
     expect_result=ACK, **N8)
text(K, "get_auto_switch_timeout", {}, E + "TAUSW\r", device_reply="AuswT003\r\n", expect_result=val("3"), **N8)

# Picture (IN1808 p.66-68, IN1608 p.54-55, p.60)
text(K, "set_freeze", {"frozen": True}, "1*1F\r", device_reply="Frz1*1\r\n", expect_result=ACK, **N8)
text(K, "get_freeze", {}, "1F\r", device_reply="Frz1*0\r\n", expect_result=val("0"), **N8)
text(K, "set_freeze_in1600", {"frozen": False}, "0F\r", device_reply="Frz0\r\n", expect_result=ACK, **N6)
text(K, "get_freeze_in1600", {}, "F\r", device_reply="Frz1\r\n", expect_result=val("1"), **N6)
text(K, "set_contrast", {"input": 2, "level": 70}, E + "2*70CONT\r", device_reply="Cont02*070\r\n",
     expect_result=ACK, **N8)
text(K, "get_contrast", {"input": 2}, E + "2CONT\r", device_reply="Cont02*070\r\n", expect_result=val("70"), **N8)
text(K, "set_brightness", {"input": 1, "level": 0}, E + "1*0BRIT\r", device_reply="Brit01*000\r\n",
     expect_result=ACK, **N8)
text(K, "get_brightness", {"input": 1}, E + "1BRIT\r", device_reply="Brit01*064\r\n", expect_result=val("64"), **N8)
text(K, "set_detail", {"level": 80}, E + "1*80HDET\r", device_reply="Hdet1*080\r\n", expect_result=ACK, **N8)
text(K, "get_detail", {}, E + "1HDET\r", device_reply="Hdet1*064\r\n", expect_result=val("64"), **N8)
text(K, "set_image_position", {"horizontal": -10, "vertical": 20, "width": 1920, "height": 1080},
     E + "1,-10*20*1920*1080XIMG\r", device_reply="Ximg1,-0010*+0020*01920*01080\r\n", expect_result=ACK, **N8)
text(K, "get_image_position", {}, E + "1XIMG\r", device_reply="Ximg1,+0000*+0000*01920*01080\r\n",
     expect_result=val("+0000*+0000*01920*01080"), **N8)
text(K, "set_contrast_current_input", {"level": 60}, E + "60CONT\r", device_reply="Cont3*060\r\n",
     expect_result=ACK, **N6)
text(K, "get_contrast_current_input", {}, E + "CONT\r", device_reply="Cont3*060\r\n", expect_result=val("60"), **N6)
text(K, "set_brightness_current_input", {"level": 64}, E + "64BRIT\r", device_reply="Brit3*064\r\n",
     expect_result=ACK, **N6)
text(K, "get_brightness_current_input", {}, E + "BRIT\r", device_reply="064\r\n", expect_result=val("64"), **N6)
text(K, "set_detail_current_input", {"level": 50}, E + "50HDET\r", device_reply="Hdet3*050\r\n",
     expect_result=ACK, **N6)
text(K, "get_detail_current_input", {}, E + "HDET\r", device_reply="Hdet3*050\r\n", expect_result=val("50"), **N6)
text(K, "set_color_current_input", {"level": 64}, E + "64COLR\r", device_reply="Colr1*064\r\n",
     expect_result=ACK, **N6)
text(K, "get_color_current_input", {}, E + "COLR\r", device_reply="Colr1*064\r\n", expect_result=val("64"), **N6)
text(K, "set_tint_current_input", {"level": 64}, E + "64TINT\r", device_reply="E17\r\n", expect_result=ERR, **N6)
text(K, "get_tint_current_input", {}, E + "TINT\r", device_reply="Tint1*064\r\n", expect_result=val("64"), **N6)
text(K, "set_image_position_in1600", {"horizontal": 0, "vertical": 0, "width": 1280, "height": 720},
     E + "0*0*1280*720XIMG\r", device_reply="Ximg+0000*+0000*01280*00720\r\n", expect_result=ACK, **N6)
text(K, "get_image_position_in1600", {}, E + "XIMG\r", device_reply="Ximg+0000*+0000*01280*00720\r\n",
     expect_result=val("+0000*+0000*01280*00720"), **N6)

# Output (IN1808 p.69-71, IN1608 p.54, p.56)
text(K, "set_video_mute", {"output": 2, "mode": 1}, "2*1B\r", device_reply="Vmt2*1\r\n", expect_result=ACK, **N8)
text(K, "get_video_mute", {"output": 2}, "2*B\r", device_reply="Vmt2*1\r\n", expect_result=val("1"), **N8)
text(K, "set_video_mute_all", {"mode": 2}, "2B\r", device_reply="Vmt2\r\n", expect_result=ACK, **N8)
text(K, "get_video_mute_all", {}, "B\r", device_reply="Vmt0 1 0\r\n", expect_result=val("0 1 0"), **N8)
text(K, "set_output_rate", {"rate": 45}, E + "1*45RATE\r", device_reply="Rate1*045\r\n", expect_result=ACK, **N8)
text(K, "get_output_rate", {}, E + "1RATE\r", device_reply="Rate1*037\r\n", expect_result=val("37"), **N8)
text(K, "set_output_rate_in1600", {"rate": 73}, E + "73RATE\r", device_reply="Rate73\r\n", expect_result=ACK, **N6)
text(K, "get_output_rate_in1600", {}, E + "RATE\r", device_reply="Rate73\r\n", expect_result=val("73"), **N6)
text(K, "set_hdmi_output_format", {"output": 1, "format": 3}, E + "1*3VTPO\r", device_reply="Vtpo1*3\r\n",
     expect_result=ACK, **N8)
text(K, "get_hdmi_output_format", {"output": 1}, E + "1VTPO\r", device_reply="Vtpo1*0\r\n", expect_result=val("0"), **N6)
text(K, "set_power_save", {"mode": 0}, E + "0PSAV\r", device_reply="Psav0\r\n", expect_result=ACK, **N8)
text(K, "get_power_save", {}, E + "PSAV\r", device_reply="Psav1\r\n", expect_result=val("1"), **N6)
text(K, "set_osd_duration", {"seconds": 501}, E + "501MDUR\r", device_reply="Mdur501\r\n", expect_result=ACK, **N6)
text(K, "get_osd_duration", {}, E + "MDUR\r", device_reply="Mdur060\r\n", expect_result=val("60"), **N8)

# Presets (IN1808 p.74, IN1608 p.59)
text(K, "recall_input_preset", {"preset": 4}, "2*4.\r", device_reply="2Rpr004\r\n", expect_result=ACK, **N8)
text(K, "recall_input_preset", {"preset": 17}, "2*17.\r", device_reply="E11\r\n", expect_result=ERR, **N8)
text(K, "save_input_preset", {"preset": 4}, "2*4,\r", device_reply="2Spr004\r\n", expect_result=ACK, **N8)
text(K, "recall_user_preset", {"preset": 2}, "1*2.\r", device_reply="1Rpr02\r\n", expect_result=ACK, **N6)
text(K, "save_user_preset", {"preset": 2}, "1*2,\r", device_reply="1Spr02\r\n", expect_result=ACK, **N6)
text(K, "set_auto_memory", {"input": 3, "enabled": False}, E + "3*0AMEM\r", device_reply="Amem03*0\r\n",
     expect_result=ACK, **N8)
text(K, "get_auto_memory", {"input": 3}, E + "3AMEM\r", device_reply="Amem03*1\r\n", expect_result=val("1"), **N8)

# Audio (IN1808 p.75-77, p.89-92; IN1608 p.57-58)
text(K, "set_audio_input_format", {"input": 2, "format": 2}, E + "I2*2AFMT\r", device_reply="AfmtI02*2\r\n",
     expect_result=ACK, **N8)
text(K, "get_audio_input_format", {"input": 2}, E + "I2AFMT\r", device_reply="AfmtI02*4\r\n",
     expect_result=val("4"), **N8)
text(K, "set_group_level", {"group": 3, "level": -293}, E + "D3*-293GRPM\r", device_reply="GrpmD3*-00293\r\n",
     expect_result=ACK, **N8)
text(K, "increase_group_level", {"group": 3, "step": 30}, E + "D3*30+GRPM\r", device_reply="GrpmD3*-00263\r\n",
     expect_result=ACK, **N8)
text(K, "decrease_group_level", {"group": 1, "step": 100}, E + "D1*100-GRPM\r", device_reply="GrpmD1*-00100\r\n",
     expect_result=ACK, **N6)
text(K, "get_group_level", {"group": 3}, E + "D3GRPM\r", device_reply="GrpmD3*-00263\r\n",
     expect_result=val("-00263"), **N8)
text(K, "set_group_mute", {"group": 4, "muted": True}, E + "D4*1GRPM\r", device_reply="GrpmD4*1\r\n",
     expect_result=ACK, **N8)
text(K, "get_group_mute", {"group": 2}, E + "D2GRPM\r", device_reply="GrpmD2*0\r\n", expect_result=val("0"), **N6)
text(K, "set_dsp_gain", {"oid": 40000, "level": 120}, E + "G40000*120AU\r", device_reply="DsG40000*120\r\n",
     expect_result=ACK, **N8)
text(K, "get_dsp_gain", {"oid": 60000}, E + "G60000AU\r", device_reply="DsG60000*-55\r\n",
     expect_result=val("-55"), **N8)
text(K, "set_dsp_mute", {"oid": 60002, "muted": True}, E + "M60002*1AU\r", device_reply="DsM60002*1\r\n",
     expect_result=ACK, **N6)
text(K, "get_dsp_mute", {"oid": 40001}, E + "M40001AU\r", device_reply="DsM40001*0\r\n", expect_result=val("0"), **N6)
text(K, "set_phantom_power", {"oid": "40000", "enabled": True}, E + "Z40000*1AU\r", device_reply="DsZ40000*1\r\n",
     expect_result=ACK, **N8)
text(K, "set_volume_knob_group", {"group": "8"}, E + "1*8KNOB\r", device_reply="Knob1*8\r\n", expect_result=ACK, **N6)
text(K, "get_volume_knob_group", {}, E + "1KNOB\r", device_reply="Knob1*1\r\n", expect_result=val("1"), **N6)
text(K, "play_audio_file", {"slot": 2}, E + "2*1PLAY\r", device_reply="Play2*1\r\n", expect_result=ACK, **N8)
text(K, "play_audio_file", {"slot": 9}, E + "9*1PLAY\r", device_reply="E22\r\n", expect_result=ERR, **N8)
text(K, "stop_audio_file", {"slot": 2}, E + "2*0PLAY\r", device_reply="Play2*0\r\n", expect_result=ACK, **N8)
text(K, "get_playing_audio_file", {}, E + "PLAY\r", device_reply="Play2*1\r\n", expect_result=val("2"), **N8)

# Advanced (IN1808 p.78-79, IN1608 p.60-62)
text(K, "set_test_pattern", {"pattern": 4}, E + "1*4TEST\r", device_reply="Test1*04\r\n", expect_result=ACK, **N8)
text(K, "get_test_pattern", {}, E + "1TEST\r", device_reply="Test1*00\r\n", expect_result=val("0"), **N8)
text(K, "set_test_pattern_in1600", {"pattern": 3}, E + "3TEST\r", device_reply="Test3\r\n", expect_result=ACK, **N6)
text(K, "get_test_pattern_in1600", {}, E + "TEST\r", device_reply="Test0\r\n", expect_result=val("0"), **N6)
text(K, "set_switch_effect", {"effect": 3}, E + "O1*3SWEF\r", device_reply="SwefO1*3\r\n", expect_result=ACK, **N8)
text(K, "get_switch_effect", {}, E + "O1SWEF\r", device_reply="SwefO1*2\r\n", expect_result=val("2"), **N8)
text(K, "set_switch_effect_in1600", {"effect": 0}, E + "0SWEF\r", device_reply="Swef0\r\n", expect_result=ACK, **N6)
text(K, "get_switch_effect_in1600", {}, E + "SWEF\r", device_reply="Swef1\r\n", expect_result=val("1"), **N6)
text(K, "get_signal_status", {}, E + "0LS\r", device_reply="In00 1*0*1*0*0*0*1*0\r\n",
     expect_result=val("1*0*1*0*0*0*1*0"), **N8)
text(K, "set_front_panel_lock", {"mode": 2}, "2X\r", device_reply="Exe2\r\n", expect_result=ACK, **N8)
text(K, "get_front_panel_lock", {}, "X\r", device_reply="Exe0\r\n", expect_result=val("0"), **N6)
text(K, "set_output_hdcp_mode", {"output": 1, "mode": 2}, E + "S1*2HDCP\r", device_reply="HdcpS1*2\r\n",
     expect_result=ACK, **N8)
text(K, "get_output_hdcp_mode", {"output": 1}, E + "S1HDCP\r", device_reply="HdcpS1*1\r\n",
     expect_result=val("1"), **N8)
text(K, "set_output_hdcp_mode_in1600", {"mode": 4}, E + "S4HDCP\r", device_reply="HdcpS4\r\n",
     expect_result=ACK, **N6)
text(K, "get_output_hdcp_mode_in1600", {}, E + "SHDCP\r", device_reply="HdcpS0\r\n", expect_result=val("0"), **N6)
text(K, "set_hdcp_notification", {"mode": 2}, E + "N1*2HDCP\r", device_reply="HdcpN1*2\r\n",
     expect_result=ACK, **N8)
text(K, "get_hdcp_notification", {}, E + "N1HDCP\r", device_reply="HdcpN1*1\r\n", expect_result=val("1"), **N8)
text(K, "set_hdcp_notification_in1600", {"enabled": False}, E + "N0HDCP\r", device_reply="HdcpN0\r\n",
     expect_result=ACK, **N6)
text(K, "get_hdcp_notification_in1600", {}, E + "NHDCP\r", device_reply="HdcpN1\r\n", expect_result=val("1"), **N6)
text(K, "get_input_hdcp_status", {"input": 2}, E + "I2HDCP\r", device_reply="HdcpI02*2\r\n",
     expect_result=val("2"), **N8)
text(K, "get_output_hdcp_status", {"output": 1}, E + "O1HDCP\r", device_reply="HdcpO01*1\r\n",
     expect_result=val("1"), **N8)

# Information (IN1808 p.83-84, IN1608 p.63-64)
text(K, "get_input_info", {"input": 2}, "2*I\r",
     device_reply="Inf00*Vid02 Typ2 Amt0 Vmt0 Hrt067.43 Vrt059.94\r\n",
     expect_result=val("Vid02 Typ2 Amt0 Vmt0 Hrt067.43 Vrt059.94"), **N8)
text(K, "get_info_in1600", {}, "I\r", device_reply="Vid3 Aud3 Typ6 Std- Blk0 Hrt067.5 Vrt060.0\r\n",
     expect_result=val("Vid3 Aud3 Typ6 Std- Blk0 Hrt067.5 Vrt060.0"), **N6)
text(K, "get_model_name", {}, "1I\r", device_reply="Inf01*IN1808 IPCP SA\r\n", expect_result=val("IN1808 IPCP SA"), **N8)
text(K, "get_firmware", {}, "Q\r", device_reply="Ver01*1.02\r\n", expect_result=val("1.02"), **N8)
text(K, "get_firmware_full", {}, "*Q\r", device_reply="Bld1.02.0003\r\n", expect_result=val("1.02.0003"), **N8)
text(K, "get_part_number", {}, "N\r", device_reply="Pno60-1615-02\r\n", expect_result=val("60-1615-02"), **N8)
text(K, "get_temperature", {}, E + "20STAT\r", device_reply="20Stat 41\r\n", expect_result=val("41"), **N8)
text(K, "set_verbose_mode", {"mode": 3}, E + "3CV\r", device_reply="Vrb3\r\n", expect_result=ACK, **N6)
text(K, "get_verbose_mode", {}, E + "CV\r", device_reply="Vrb3\r\n", expect_result=val("3"), **N8)

# Telemetry: the banner, verbose-mode change reports and pushed messages.
telemetry(K, "banner",
          inbound="(c) Copyright 2022, Extron Electronics, IN1808 IPCP SA, V1.02, 60-1615-02\r\n",
          expect_state={"device": {"model": "IN1808 IPCP SA", "firmware": "1.02", "part_number": "60-1615-02"}})
telemetry(K, "login", inbound="Login User\r\n", expect_state={"device": {"login": "User"}})
telemetry(K, "input-all", inbound="In03*1 All\r\n",
          expect_state={"input": 3, "video_input": 3, "audio_input": 3})
telemetry(K, "input-front-panel", inbound="In5 All\r\n",
          expect_state={"input": 5, "video_input": 5, "audio_input": 5})
telemetry(K, "input-video", inbound="In1 RGB\r\n", expect_state={"video_input": 1})
telemetry(K, "input-audio", inbound="In09*1 Aud\r\n", expect_state={"audio_input": 9})
telemetry(K, "loop-out", inbound="Lout04\r\n", expect_state={"loop_out_input": 4})
telemetry(K, "video-mute", inbound="Vmt2*1\r\n", expect_state={"outputs": {"2": {"video_mute": 1}}})
telemetry(K, "video-mute-all", inbound="Vmt0\r\n", expect_state={"video_mute_all": 0})
telemetry(K, "input-name", inbound="VnamI02*Lectern PC\r\n", expect_state={"inputs": {"2": {"name": "Lectern PC"}}})
telemetry(K, "input-name-in1600", inbound="Nmi3,Laptop\r\n", expect_state={"inputs": {"3": {"name": "Laptop"}}})
telemetry(K, "output-name", inbound="VnamO01*Projector\r\n", expect_state={"outputs": {"1": {"name": "Projector"}}})
telemetry(K, "audio-format", inbound="AfmtI02*4\r\n", expect_state={"inputs": {"2": {"audio_format": 4}}})
telemetry(K, "hdcp-input", inbound="HdcpI 02*2\r\n", expect_state={"inputs": {"2": {"hdcp_status": 2}}})
telemetry(K, "hdcp-output", inbound="HdcpO 01*1\r\n", expect_state={"outputs": {"1": {"hdcp_status": 1}}})
telemetry(K, "hdcp-authorized", inbound="HdcpE03*0\r\n", expect_state={"inputs": {"3": {"hdcp_authorized": False}}})
telemetry(K, "aspect", inbound="Aspr02*2\r\n", expect_state={"inputs": {"2": {"aspect_ratio": 2}}})
telemetry(K, "contrast", inbound="Cont02*070\r\n", expect_state={"inputs": {"2": {"contrast": 70}}})
telemetry(K, "brightness", inbound="Brit01*064\r\n", expect_state={"inputs": {"1": {"brightness": 64}}})
telemetry(K, "group", inbound="GrpmD3*-00263\r\n", expect_state={"groups": {"3": {"value": -263}}})
telemetry(K, "dsp-gain", inbound="DsG60000*-55\r\n", expect_state={"dsp": {"60000": {"gain": -55}}})
telemetry(K, "dsp-mute", inbound="DsM60002*1\r\n", expect_state={"dsp": {"60002": {"mute": True}}})
telemetry(K, "freeze", inbound="Frz1*1\r\n", expect_state={"freeze": True})
telemetry(K, "front-panel", inbound="Exe3\r\n", expect_state={"front_panel_lock": 3})
telemetry(K, "power-save", inbound="Psav0\r\n", expect_state={"power_save": 0})
telemetry(K, "rate", inbound="Rate1*037\r\n", expect_state={"output_rate": 37})
telemetry(K, "test-pattern", inbound="Test1*04\r\n", expect_state={"test_pattern": 4})
telemetry(K, "switch-effect", inbound="SwefO1*2\r\n", expect_state={"switch_effect": 2})
telemetry(K, "auto-switch", inbound="Ausw1\r\n", expect_state={"auto_switch_mode": 1})
telemetry(K, "verbose", inbound="Vrb3\r\n", expect_state={"verbose_mode": 3})
telemetry(K, "playback", inbound="Play2*0\r\n", expect_state={"audio_playback_slot": 2, "audio_playing": False})
telemetry(K, "signal", inbound="IN00 1*0*1*0*0*0*1*0\r\n",
          expect_state={"inputs": {"1": {"signal": True}, "2": {"signal": False}, "3": {"signal": True},
                                   "4": {"signal": False}, "5": {"signal": False}, "6": {"signal": False},
                                   "7": {"signal": True}, "8": {"signal": False}}})
telemetry(K, "signal-six", inbound="In00 0*1*0*0*0*1\r\n",
          expect_state={"inputs": {"1": {"signal": False}, "2": {"signal": True}, "3": {"signal": False},
                                   "4": {"signal": False}, "5": {"signal": False}, "6": {"signal": True}}})
telemetry(K, "reconfig", inbound="Reconfig\r\n", expect_state={})
