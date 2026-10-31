RM5 = "roland-m5000"
# Roland M-5000 (RS-232C/Telnet Reference v1.20, p.6-21): over Telnet a command
# is the three letters, ":" parameters and ";" ended by CR LF, ack is the text
# OK; on RS-232C stx (02H) comes first, nothing follows the ";", and ack is
# the byte 06H.
RM5_OK = {"ok": {"kind": "ack"}}
RM5_ERR = {"error": {"error": "device_error"}}


def rm5(command, input, wire, **extra):
    text(RM5, command, input, wire + "\r\n", **extra)


def rm5t(name, inbound, **extra):
    telemetry(RM5, name, inbound=inbound + "\r\n", **extra)


def rm5_value(v):
    return {"ok": {"kind": "value", "value": v}}


rm5("set_phantom", {"channel": "I1", "enabled": True}, "PTC:I1,1;", device_reply="OK\r\n", expect_result=RM5_OK)
rm5("get_phantom", {"channel": "I128"}, "PTQ:I128;", device_reply="PTS:I128,0;\r\n", expect_result=rm5_value("0"))
rm5("set_eq", {"channel": "MA2", "enabled": False}, "EQC:MA2,0;")
rm5("get_eq", {"channel": "SG64"}, "EQQ:SG64;", device_reply="EQS:SG64,1;\r\n", expect_result=rm5_value("1"))
rm5("set_aux_send", {"channel": "I1", "aux": 1, "level": 4.0, "pan": 30}, "AXC:I1,AX1,4.0,R30;")   # p.10 example
rm5("set_aux_send_off", {"channel": "U192", "aux": 64, "pan": -100}, "AXC:U192,AX64,INF,L100;")
rm5("get_aux_send", {"channel": "I1", "aux": 3}, "AXQ:I1,AX3;",
    device_reply="AXS:I1,AX3,-6.5,C;\r\n", expect_result=rm5_value("-6.5"))                      # p.15 example
rm5("set_pan", {"channel": "I1", "pan": -45}, "PNC:I1,L45;")                                     # p.10 example
rm5("get_pan", {"channel": "I1"}, "PNQ:I1;", device_reply="PNS:I1,C;\r\n", expect_result=rm5_value("C"))
rm5("set_mute", {"channel": "MG8", "muted": True}, "MUC:MG8,1;", device_reply="ERR:5;\r\n", expect_result=RM5_ERR)
rm5("get_mute", {"channel": "DCA24"}, "MUQ:DCA24;", device_reply="MUS:DCA24,1;\r\n", expect_result=rm5_value("1"))
rm5("set_fader", {"channel": "I1", "level": -25.0}, "FDC:I1,-25.0;", device_reply="OK\r\n", expect_result=RM5_OK)
rm5("set_fader_off", {"channel": "I1"}, "FDC:I1,INF;")                                           # p.11 example
rm5("adjust_fader", {"channel": "MON2", "change": -3.5}, "RFC:MON2,-3.5;")
rm5("get_fader", {"channel": "I1"}, "FDQ:I1;", device_reply="FDS:I1,-6.0;\r\n", expect_result=rm5_value("-6.0"))
rm5("get_channel_name", {"channel": "I1"}, "CNQ:I1;", device_reply='CNS:I1,"A.BASS";\r\n',
    expect_result=rm5_value("A.BASS"))                                                           # p.14 example
rm5("recall_scene", {"scene": 1.5}, "SCC:1.50;")                                                 # p.11 example
rm5("recall_scene_relative", {"offset": -2}, "RSC:-2;")
rm5("store_scene", {"scene": 300.0}, "SSC:300.00;")
rm5("store_scene_named", {"scene": 1.0, "name": "SCENE"}, "SSC:1.00,SCENE;")
rm5("store_scene_with_m48", {"scene": 1.0, "name": "SCENE", "memory": 5}, "SSC:1.00,SCENE,05;")   # p.12 example
rm5("new_scene", {"name": "Band"}, "SNC:Band;")
rm5("new_scene_with_m48", {"name": "Band", "memory": 0}, "SNC:Band,0;")
rm5("get_scene", {}, "SCQ;", device_reply='SCS:23.00,"JAZZ BAND MUSIC";\r\n', expect_result=rm5_value("23.00"))
rm5("set_display_brightness", {"brightness": 100}, "DBC:100;")
rm5("get_display_brightness", {}, "DBQ;", device_reply="DBS:80;\r\n", expect_result=rm5_value("80"))
rm5("set_panel_brightness", {"brightness": 0}, "PBC:0;")
rm5("get_panel_brightness", {}, "PBQ;", device_reply="PBS:50;\r\n", expect_result=rm5_value("50"))
rm5("set_lamp_brightness", {"brightness": 40}, "LBC:40;")
rm5("get_lamp_brightness", {}, "LBQ;", device_reply="LBS:40;\r\n", expect_result=rm5_value("40"))
rm5("set_monitor_dimmer", {"monitor": "MON1", "enabled": True}, "DMC:MON1,1;")                  # p.13 example
rm5("get_monitor_dimmer", {"monitor": "MON2"}, "DMQ:MON2;", device_reply="DMS:MON2,0;\r\n", expect_result=rm5_value("0"))
rm5("recorder_transport", {"action": "rec_pause"}, "RTC:R;")
rm5("recorder_locate", {"hours": 0, "minutes": 2, "seconds": 34}, "RLC:00h02m34s;")
rm5("recorder_select_song", {"song": 12}, "RIC:12;")
rm5("recorder_move_song", {"offset": 1}, "RIC:+1;")
rm5("recorder_next_song", {}, "RIC:N;")
rm5("recorder_previous_song", {}, "RIC:P;")
rm5("get_recorder_status", {}, "RTQ;", device_reply="RTS:-1;\r\n", expect_result=rm5_value("-1"))
rm5("get_recorder_position", {}, "RLQ;", device_reply="RLS:02m34s;\r\n", expect_result=rm5_value("02m34s"))
rm5("get_recorder_song", {}, "RIQ;", device_reply="RIS:3,10;\r\n", expect_result=rm5_value("3"))
rm5("get_recorder_song_name", {}, "RNQ;", device_reply='RNS:1,"SONG01.WAV";\r\n', expect_result=rm5_value("SONG01.WAV"))
rm5("get_recorder_song_name_of", {"song": 1}, "RNQ:1;", device_reply='RNS:1,"SONG01.WAV;\r\n',
    expect_result=rm5_value("SONG01.WAV"))                                                       # p.19 example, as printed
rm5("get_recorder_remaining", {}, "RRQ;", device_reply="RRS:01h02m34s;\r\n", expect_result=rm5_value("01h02m34s"))
rm5("get_reac_status", {"port": "A"}, "RCQ:A;", device_reply="RCS:A,2;\r\n", expect_result=rm5_value("2"))
rm5("get_version", {}, "VRQ;", device_reply="VRS:1.200;\r\n", expect_result=rm5_value("1.200"))
rm5("resend_last_reply", {}, "ERR:0;")

rm5t("name", 'CNS:I1,"A.BASS";', expect_state={"channels": {"I1": {"name": "A.BASS"}}})
rm5t("fader", "FDS:I1,-6.0;", expect_state={"channels": {"I1": {"fader": "-6.0"}}})
rm5t("fader-inf", "FDS:MA1,INF;", expect_state={"channels": {"MA1": {"fader": "-inf"}}})
rm5t("mute", "MUS:DCA3,1;", expect_state={"channels": {"DCA3": {"mute": True}}})
rm5t("pan-left", "PNS:I2,L45;", expect_state={"channels": {"I2": {"pan": -45}}})
rm5t("pan-centre", "PNS:I2,C;", expect_state={"channels": {"I2": {"pan": 0}}})
rm5t("pan-right", "PNS:AX4,R100;", expect_state={"channels": {"AX4": {"pan": 100}}})
rm5t("eq", "EQS:MX7,0;", expect_state={"channels": {"MX7": {"eq": False}}})
rm5t("phantom", "PTS:I9,1;", expect_state={"channels": {"I9": {"phantom": True}}})
rm5t("aux-send", "AXS:I1,AX3,-6.5,C;",
     expect_state={"channels": {"I1": {"aux_sends": {"AX3": {"level": "-6.5", "pan": 0}}}}})
rm5t("aux-send-inf", "AXS:I1,AX4,INF,L30;",
     expect_state={"channels": {"I1": {"aux_sends": {"AX4": {"level": "-inf", "pan": -30}}}}})
rm5t("aux-send-mono", "AXS:I1,AX5,0.0,0;",
     expect_state={"channels": {"I1": {"aux_sends": {"AX5": {"level": "0.0", "pan": 0}}}}})
rm5t("scene", 'SCS:23.00,"JAZZ BAND MUSIC";', expect_state={"scene": {"number": "23.00", "name": "JAZZ BAND MUSIC"}})
rm5t("brightness", "DBS:80;", expect_state={"brightness": {"display": 80}})
rm5t("panel-brightness", "PBS:50;", expect_state={"brightness": {"panel": 50}})
rm5t("lamp-brightness", "LBS:30;", expect_state={"brightness": {"lamp": 30}})
rm5t("dimmer", "DMS:MON1,1;", expect_state={"monitors": {"MON1": {"dimmer": True}}})
rm5t("recorder-status", "RTS:4;", expect_state={"recorder": {"status": "rec_pause"}})
rm5t("recorder-position", "RLS:02m34s;", expect_state={"recorder": {"position": "02m34s"}})
rm5t("recorder-song", "RIS:3,10;", expect_state={"recorder": {"song": 3, "song_count": 10}})
rm5t("recorder-song-name", 'RNS:1,"SONG01.WAV";', expect_state={"recorder": {"songs": {"1": {"name": "SONG01.WAV"}}}})
rm5t("recorder-remaining", "RRS:01h02m34s;", expect_state={"recorder": {"remaining": "01h02m34s"}})
rm5t("reac", "RCS:B,1;", expect_state={"reac": {"B": {"status": "establishing"}}})
rm5t("reac-error", "EPS:D1,2;", expect_state={"reac": {"D1": {"error": "transmission"}}})
rm5t("console-error", "ERS:1;", expect_state={"device": {"error": 1}})
rm5t("version", "VRS:1.200;", expect_state={"device": {"version": "1.200"}})
