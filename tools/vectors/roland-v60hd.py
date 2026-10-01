R60 = "roland-v60hd"
# Roland V-60HD (Reference Manual v3.1, edition 04, p.20-23): commands are
# STX (02H) + code + ":" arguments + ";" with no line ending; a command is
# acknowledged by the single byte ACK (06H). Channels and memories are
# 0-based on the wire.
R60_OK = {"ok": {"kind": "ack"}}
R60_ERR = {"error": {"error": "device_error"}}


def r60(command, input, wire, **extra):
    text(R60, command, input, wire, **extra)


# Video (p.21)
r60("select_pgm", {"xpt": 1}, "\x02PGM:0;", device_reply="\x06", expect_result=R60_OK)
r60("select_pst", {"xpt": 8}, "\x02PST:7;", device_reply="\x02ERR:5;", expect_result=R60_ERR)
r60("select_aux", {"xpt": 6}, "\x02AUX:5;")
r60("set_transition", {"effect": 2}, "\x02TRS:2;")
r60("set_transition_time", {"time": 40}, "\x02TIM:40;")
r60("cut", {}, "\x02CUT;", device_reply="\x06", expect_result=R60_OK)
r60("auto", {}, "\x02ATO;")
r60("press_pinp1", {}, "\x02P1S;")
r60("press_pinp2", {}, "\x02P2S;")
r60("press_split", {}, "\x02SPS;")
r60("press_dsk", {}, "\x02DSK;")
r60("press_dsk_pvw", {}, "\x02DVW;")
r60("press_auto_mixing", {}, "\x02ATM;")
r60("press_output_fade", {}, "\x02FDE;")
r60("set_pinp1_position", {"h": -450, "v": 400}, "\x02PP1:-450,400;")
r60("set_pinp2_position", {"h": 0, "v": -400}, "\x02PP2:0,-400;")
r60("set_split_position", {"pgm": -250, "pst": 250}, "\x02SPT:-250,250;")
r60("set_dsk_source", {"xpt": 7}, "\x02DSS:6;")
r60("set_dsk_level", {"level": 255}, "\x02KYL:255;")
r60("set_dsk_gain", {"gain": 0}, "\x02KYG:0;")
r60("set_ch6_input", {"rgb": True}, "\x02IPS:1;")
r60("set_sdi_out1_bus", {"bus": 0}, "\x02OS1:0;")
r60("set_sdi_out2_bus", {"bus": 1}, "\x02OS2:1;")
r60("set_hdmi_out1_bus", {"bus": 2}, "\x02OH1:2;")
r60("set_hdmi_out2_bus", {"bus": 0}, "\x02OH2:0;")
# Audio (p.22)
r60("set_input_level", {"input": 10, "level": -801}, "\x02IAL:10,-801;")
r60("set_master_level", {"level": 100}, "\x02OAL:100;")
r60("set_aux_level", {"level": -800}, "\x02OAX:-800;")
r60("set_input_delay", {"input": 4, "delay": 120}, "\x02ADT:4,120;")
r60("get_audio_level", {"channel": 13}, "\x02QAL:13;",
    device_reply="\x02QAL:100,80,70,60,50,40,30,20,100,80,70,60,50;\x06", expect_result=R60_OK)   # p.22 example
r60("press_input_mute", {"input": 5}, "\x02IAM:5;")
r60("press_input_solo", {"input": 0}, "\x02IAS:0;")
# System (p.23)
r60("set_hdcp", {"enabled": False}, "\x02HCP:0;")
r60("set_test_pattern", {"pattern": 5}, "\x02TPT:5;")
r60("set_test_tone", {"tone": 6}, "\x02TTN:6;")
r60("recall_memory", {"memory": 8}, "\x02MEM:7;")
r60("get_panel_status", {"item": 7}, "\x02QPL:7;",
    device_reply="\x02QPL:0,1,0,1,1,0,2047;\x06", expect_result=R60_OK)
r60("get_tally", {}, "\x02TLY;", device_reply="\x02TLY:1, 2, 0, 0, 0, 0, 0, 0;\x06", expect_result=R60_OK)  # p.23 example
r60("active_sense", {}, "\x02ACS;", device_reply="\x06", expect_result=R60_OK)
r60("get_version", {}, "\x02VER;", device_reply="\x02VER:V-60HD,3.10;",
    expect_result={"ok": {"kind": "value", "value": "3.10"}})

telemetry(R60, "panel", inbound="\x02QPL:0,1,0,1,1,0,2047;",
          expect_state={"program": {"xpt": 1}, "preview": {"xpt": 2}, "aux": {"xpt": 1}, "composition": 1,
                        "dsk": True, "output_fade": False, "fader": {"level": 2047}})
telemetry(R60, "tally", inbound="\x02TLY:1, 2, 0, 0, 0, 0, 0, 0;",
          expect_state={"tally": "1, 2, 0, 0, 0, 0, 0, 0"})
telemetry(R60, "audio-levels", inbound="\x02QAL:100,80,70,60,50,40,30,20,100,80,70,60,50;",
          expect_state={"audio": {"levels": "100,80,70,60,50,40,30,20,100,80,70,60,50"}})
telemetry(R60, "version", inbound="\x02VER:V-60HD,3.10;",
          expect_state={"device": {"model": "V-60HD", "version": "3.10"}})
