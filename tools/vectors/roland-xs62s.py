R62 = "roland-xs62s"
# Roland XS-62S (Reference Manual v3.1, edition 02, p.26-30): STX (02H) +
# code + ":" arguments + ";" with no line ending, acknowledged by the byte
# ACK (06H). Channels, memories and GPOs are 0-based on the wire.
R62_OK = {"ok": {"kind": "ack"}}


def r62(command, input, wire, **extra):
    text(R62, command, input, wire, **extra)


# Video (p.27-28)
r62("select_pgm", {"xpt": 2}, "\x02PGM:1;", device_reply="\x06", expect_result=R62_OK)
r62("select_pst", {"xpt": 3}, "\x02PST:2;", device_reply="\x02ERR:4;",
    expect_result={"error": {"error": "device_error"}})          # DISSOLVE mode, p.30
r62("select_aux", {"xpt": 8}, "\x02AUX:7;")
r62("set_transition", {"effect": 2}, "\x02TRS:2;")
r62("set_transition_time", {"time": 10}, "\x02TIM:10;")
r62("cut", {}, "\x02CUT;")
r62("take", {}, "\x02TAK;", device_reply="\x06", expect_result=R62_OK)
r62("set_pinp", {"state": 2}, "\x02PPS:2;")
r62("set_split", {"state": 1}, "\x02SPS:1;")
r62("set_dsk", {"enabled": True}, "\x02DSK:1;")
r62("set_dsk_preview", {"enabled": False}, "\x02DVW:0;")
r62("set_auto_mixing", {"enabled": True}, "\x02ATM:1;")
r62("set_freeze", {"enabled": True}, "\x02FRZ:1;")
r62("get_bus_channel", {"bus": 2}, "\x02QVC:2;", device_reply="\x02QVC:2,5;\x06", expect_result=R62_OK)
r62("set_edid", {"input": 2, "edid": 8}, "\x02EDD:2,8;")
r62("set_input_scaling", {"input": 0, "type": 4}, "\x02VIA:0,4;")
r62("set_scaler_resolution", {"resolution": 10}, "\x02VOR:10;")
r62("get_scaler_resolution", {}, "\x02QVR;")
r62("set_scaler_scaling", {"type": 1}, "\x02VOA:1;")
r62("set_hdmi_color_space", {"output": 2, "space": 1}, "\x02VOC:2,1;")
r62("set_hdmi_signal", {"output": 0, "hdmi": True}, "\x02VOD:0,1;")
r62("set_pinp_position", {"h": 450, "v": -400}, "\x02PIP:450,-400;")
r62("set_split_position", {"pgm": 0, "pst": -250}, "\x02SPT:0,-250;")
r62("set_dsk_source", {"xpt": 7}, "\x02DSS:6;")
r62("set_dsk_level", {"level": 128}, "\x02KYL:128;")
r62("set_dsk_gain", {"gain": 64}, "\x02KYG:64;")
r62("set_ch6_input", {"rgb": False}, "\x02IPS:0;")
r62("get_ch6_input", {}, "\x02QIP;")
r62("set_output_bus", {"bus": 2}, "\x02VOS:2;")
r62("get_output_bus", {"output": 4}, "\x02QVS:4;")
# Audio (p.28-29)
r62("set_pgm_input_level", {"input": 0, "level": -801}, "\x02IL1:0,-801;")
r62("set_pvw_input_level", {"input": 10, "level": 100}, "\x02IL2:10,100;")
r62("set_master_level", {"level": 0}, "\x02OL1:0;")
r62("set_pvw_level", {"level": -800}, "\x02OL2:-800;")
r62("set_aux_level", {"level": 50}, "\x02OL3:50;")
r62("set_input_delay", {"input": 0, "delay": 60}, "\x02ADT:0,60;")
r62("get_audio_level", {"channel": 14}, "\x02QAL:14;")
r62("set_audio_output_bus", {"output": 2, "bus": 1}, "\x02AOS:2,1;")
r62("get_audio_output_bus", {"output": 0}, "\x02QAS:0;")
r62("set_input_mute", {"input": 9, "enabled": True}, "\x02IAM:9,1;")
r62("set_input_solo", {"input": 1, "enabled": False}, "\x02IAS:1,0;")
# System (p.29)
r62("set_hdcp", {"enabled": True}, "\x02HCP:1;")
r62("set_test_pattern", {"pattern": 1}, "\x02TPT:1;")
r62("set_test_tone", {"tone": 4}, "\x02TTN:4;")
r62("recall_memory", {"memory": 1}, "\x02MEM:0;")
r62("get_panel_status", {"item": 7}, "\x02QPL:7;", device_reply="\x02QPL:0,1,0,1,1,0,0;\x06", expect_result=R62_OK)  # p.29 example
r62("pulse_gpo", {"gpo": 4}, "\x02GPO:3,1;")
r62("set_gpo", {"gpo": 1, "enabled": False}, "\x02GPO:0,0;")
r62("set_mode", {"mode": 2}, "\x02MOD:2;")
r62("camera_recall", {"camera": 6, "memory": 8}, "\x02CAM:6,7;")
r62("get_tally", {}, "\x02TLY;")
r62("get_version", {}, "\x02VER;", device_reply="\x02VER:,3.10;\x06", expect_result=R62_OK)
r62("active_sense", {}, "\x02ACS;", device_reply="\x06", expect_result=R62_OK)

telemetry(R62, "panel", inbound="\x02QPL:0,1,0,1,1,0,0;",
          expect_state={"program": {"xpt": 1}, "preview": {"xpt": 2}, "aux": {"xpt": 1}, "composition": 1,
                        "dsk": True, "freeze": False, "fader": {"level": 0}})
telemetry(R62, "bus-channel", inbound="\x02QVC:2,5;", expect_state={"buses": {"3": {"xpt": 6}}})
telemetry(R62, "scaler-resolution", inbound="\x02QVR:2;", expect_state={"scaler": {"resolution": 2}})
telemetry(R62, "ch6-input", inbound="\x02QIP:2;", expect_state={"ch6_input": 2})
telemetry(R62, "output-bus", inbound="\x02QVS:4,3;", expect_state={"video_outputs": {"4": {"bus": 3}}})
telemetry(R62, "audio-output-bus", inbound="\x02QAS:0,2;", expect_state={"audio_outputs": {"0": {"bus": 2}}})
telemetry(R62, "tally", inbound="\x02TLY:1,2,0,0,0,0,0,0;", expect_state={"tally": "1,2,0,0,0,0,0,0"})
# QAL:14 in the parameter list's order (p.28): inputs 0-10, MASTER OUT, PVW/2, AUX/3.
telemetry(R62, "audio-levels", inbound="\x02QAL:-801, 80, 70, 60, 50, 40, 30, 20, 100, 80, 70, 60, 50, 0;",
          expect_state={"audio": {"levels": "-801, 80, 70, 60, 50, 40, 30, 20, 100, 80, 70, 60, 50, 0",
                                  "inputs": {"0": {"level": -801}, "1": {"level": 80}, "2": {"level": 70},
                                             "3": {"level": 60}, "4": {"level": 50}, "5": {"level": 40},
                                             "6": {"level": 30}, "7": {"level": 20}, "8": {"level": 100},
                                             "9": {"level": 80}, "10": {"level": 70}},
                                  "master_level": 60, "pvw_level": 50, "aux_level": 0}})
telemetry(R62, "version", inbound="\x02VER:,3.10;", expect_state={"device": {"model": "", "version": "3.10"}})
