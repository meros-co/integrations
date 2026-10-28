RVM = "roland-vmixer"
# Roland V-Mixers (V-Mixer RS-232C Reference v2.2; M-200i Telnet Reference):
# on RS-232C stx (02H) + three letters + ":" parameters + ";", nothing after
# it, and ack is the byte 06H. Only the M-200i also takes the set over Telnet,
# where stx is dropped, lines end in CR LF and ack is the text OK. The first
# model, the M-480, has only the serial port, so its vectors play there.
RVM_OK = {"ok": {"kind": "ack"}}
RVM_ERR = {"error": {"error": "device_error"}}


def rvm(command, input, wire, **extra):
    text(RVM, command, input, "\x02" + wire, **extra)


def rvm200(command, input, wire, **extra):
    text(RVM, command, input, wire + "\r\n", model="m-200i", **extra)


def rvm_value(v):
    return {"ok": {"kind": "value", "value": v}}


rvm("set_phantom", {"channel": "I1", "enabled": True}, "PTC:I1,1;", device_reply="\x06", expect_result=RVM_OK)
rvm200("set_phantom", {"channel": "I1", "enabled": True}, "PTC:I1,1;", device_reply="OK\r\n", expect_result=RVM_OK,
       file="set_phantom-telnet")
rvm("get_phantom", {"channel": "R1L"}, "PTQ:R1L;", device_reply="\x02PTS:R1L,1;", expect_result=rvm_value("1"))
rvm("set_polarity", {"channel": "I48", "enabled": False}, "PSC:I48,0;")
rvm("get_polarity", {"channel": "I2"}, "PSQ:I2;", device_reply="\x02PSS:I2,0;", expect_result=rvm_value("0"))
rvm("set_gain", {"channel": "I1", "pad": True, "gain": 4}, "PGC:I1,1,4;")                         # reference example
rvm("get_gain", {"channel": "I1"}, "PGQ:I1;", device_reply="\x02PGS:I1,0,-55;", expect_result=rvm_value("-55"))
rvm("set_filter", {"channel": "U72", "enabled": True}, "FLC:U72,1;")
rvm("get_filter", {"channel": "I3"}, "FLQ:I3;", device_reply="\x02FLS:I3,1;", expect_result=rvm_value("1"))
rvm("set_eq", {"channel": "MAC", "enabled": True}, "EQC:MAC,1;")
rvm("get_eq", {"channel": "MX8"}, "EQQ:MX8;", device_reply="\x02EQS:MX8,1;", expect_result=rvm_value("1"))
rvm("set_gate", {"channel": "I5", "enabled": True}, "GTC:I5,1;", device_reply="\x02ERR:5;", expect_result=RVM_ERR)
rvm("get_gate", {"channel": "I5"}, "GTQ:I5;", device_reply="\x02GTS:I5,0;", expect_result=rvm_value("0"))
rvm("set_compressor", {"channel": "AX16", "enabled": False}, "CPC:AX16,0;")
rvm("get_compressor", {"channel": "AX1"}, "CPQ:AX1;", device_reply="\x02CPS:AX1,1;", expect_result=rvm_value("1"))
rvm("set_aux_send", {"channel": "I1", "bus": 2, "level": -10.5, "pan": -63}, "AXC:I1,AX2,-10.5,L63;")
rvm("set_aux_send_off", {"channel": "R3", "bus": 16, "pan": 0}, "AXC:R3,AX16,INF,C;")
rvm("get_aux_send", {"channel": "I1", "bus": 2}, "AXQ:I1,AX2;", device_reply="\x02AXS:I1,AX2,-10.5,L63;",
    expect_result=rvm_value("-10.5"))
rvm("set_matrix_send", {"channel": "MAL", "bus": 1, "level": 0.0, "pan": 20}, "MXC:MAL,MX1,0.0,R20;")
rvm("set_matrix_send_off", {"channel": "AX4", "bus": 8, "pan": 0}, "MXC:AX4,MX8,INF,C;")
rvm("get_matrix_send", {"channel": "AX4", "bus": 8}, "MXQ:AX4,MX8;", device_reply="\x02MXS:AX4,MX8,INF,C;",
    expect_result=rvm_value("INF"))
rvm200("set_main_send", {"channel": "I1", "enabled": False}, "MAC:I1,0;")
rvm200("set_main_side_send", {"channel": "AX8", "side": "MAR", "enabled": True}, "MAC:AX8,MAR,1;")
rvm200("get_main_send", {"channel": "U32", "side": "MAL"}, "MAQ:U32,MAL;", device_reply="MAS:U32,MAL,1;\r\n",
       expect_result=rvm_value("1"))
rvm("set_pan", {"channel": "I1", "pan": 63}, "PNC:I1,R63;")
rvm("get_pan", {"channel": "MAL"}, "PNQ:MAL;", device_reply="\x02PNS:MAL,L10;", expect_result=rvm_value("L10"))
rvm("set_mute", {"channel": "MG8", "muted": True}, "MUC:MG8,1;")
rvm("get_mute", {"channel": "DCA24"}, "MUQ:DCA24;", device_reply="\x02MUS:DCA24,0;", expect_result=rvm_value("0"))
rvm("set_fader", {"channel": "I1", "level": 10.0}, "FDC:I1,10.0;")
rvm("set_fader_off", {"channel": "MAC"}, "FDC:MAC,INF;")
rvm("adjust_fader", {"channel": "DCA1", "change": 99.9}, "RFC:DCA1,99.9;")
rvm("get_fader", {"channel": "I1"}, "FDQ:I1;", device_reply="\x02FDS:I1,-6.0;", expect_result=rvm_value("-6.0"))
rvm200("set_delay", {"channel": "MAL", "enabled": True}, "DSC:MAL,1;")
rvm200("get_delay", {"channel": "AX8"}, "DSQ:AX8;", device_reply="DSS:AX8,0;\r\n", expect_result=rvm_value("0"))
rvm200("set_delay_time", {"channel": "MAL", "time": "1.2", "unit": "MS"}, "DTC:MAL,1.2,MS;")    # reference example
rvm200("set_delay_time_system_unit", {"channel": "MX4", "time": "120"}, "DTC:MX4,120;")
rvm200("get_delay_time", {"channel": "MAL", "unit": "FT"}, "DTQ:MAL,FT;", device_reply="DTS:MAL,12,FT;\r\n",
       expect_result=rvm_value("12"))
rvm200("get_delay_time_system_unit", {"channel": "MAR"}, "DTQ:MAR;", device_reply="DTS:MAR,1.2,MS;\r\n",
       expect_result=rvm_value("1.2"))
rvm("recall_scene", {"scene": 10}, "SCC:010;")                                                    # reference example
rvm("recall_scene_relative", {"offset": 1}, "RSC:1;")
rvm("store_scene", {"scene": 299}, "SSC:299;")
rvm("store_scene_named", {"scene": 0, "name": "Opening"}, "SSC:000,Opening;")
rvm("store_scene_with_m48", {"scene": 23, "name": "Band", "memory": 16}, "SSC:023,Band,16;")
rvm("get_scene", {}, "SCQ;", device_reply='\x02SCS:023,"JAZZ BAND MUSIC";', expect_result=rvm_value("023"))
rvm("set_input_patch", {"channel": "I1", "port": "RAI21"}, "PIC:I1,RAI21;")                       # reference example
rvm("get_input_patch", {"channel": "CSAX16"}, "PIQ:CSAX16;", device_reply="\x02PIS:CSAX16,OFF;",
    expect_result=rvm_value("OFF"))
rvm("set_output_patch", {"port": "RAO1", "source": "MAL"}, "POC:RAO1,MAL;")                      # reference example
rvm("get_output_patch", {"port": "DOR"}, "POQ:DOR;", device_reply="\x02POS:DOR,MONR;", expect_result=rvm_value("MONR"))
rvm("set_lock", {"locked": True}, "LCC:1;")
rvm("get_lock", {}, "LCQ;", device_reply="\x02LCS:1;", expect_result=rvm_value("1"))
rvm("set_blackout", {"enabled": False}, "BLC:0;")
rvm("get_blackout", {}, "BLQ;", device_reply="\x02BLS:0;", expect_result=rvm_value("0"))
rvm("set_display_brightness", {"brightness": 50}, "DBC:50;")
rvm("get_display_brightness", {}, "DBQ;", device_reply="\x02DBS:50;", expect_result=rvm_value("50"))
rvm("set_panel_brightness", {"brightness": 100}, "PBC:100;")
rvm("get_panel_brightness", {}, "PBQ;", device_reply="\x02PBS:100;", expect_result=rvm_value("100"))
rvm("set_lamp_brightness", {"brightness": 0}, "LBC:0;")
rvm("get_lamp_brightness", {}, "LBQ;", device_reply="\x02LBS:0;", expect_result=rvm_value("0"))
rvm("set_monitor_dimmer", {"enabled": True}, "DMC:1;")
rvm("get_monitor_dimmer", {}, "DMQ;", device_reply="\x02DMS:1;", expect_result=rvm_value("1"))
rvm("recorder_transport", {"action": "play"}, "RTC:P;")
rvm("recorder_locate", {"hours": 1, "minutes": 0, "seconds": 5}, "RLC:01h00m05s;")
rvm("recorder_select_song", {"song": 0}, "RIC:0;")
rvm("recorder_move_song", {"offset": -3}, "RIC:-3;")
rvm("recorder_next_song", {}, "RIC:N;")
rvm("recorder_previous_song", {}, "RIC:P;")
rvm("get_recorder_status", {}, "RTQ;", device_reply="\x02RTS:1;", expect_result=rvm_value("1"))
rvm("get_recorder_position", {}, "RLQ;", device_reply="\x02RLS:01h00m05s;", expect_result=rvm_value("01h00m05s"))
rvm("get_recorder_song", {}, "RIQ;", device_reply="\x02RIS:2,5;", expect_result=rvm_value("2"))
rvm("get_recorder_remaining", {}, "RRQ;", device_reply="\x02RRS:10h00m00s;", expect_result=rvm_value("10h00m00s"))
rvm("get_recorder_song_name", {}, "RNQ;", device_reply='\x02RNS:2,"SONG02.WAV";', expect_result=rvm_value("SONG02.WAV"))
rvm("get_recorder_song_name_of", {"song": 4}, "RNQ:4;", device_reply='\x02RNS:4,"SONG04.WAV";',
    expect_result=rvm_value("SONG04.WAV"))
rvm("get_channel_name", {"channel": "I1"}, "CNQ:I1;", device_reply='\x02CNS:I1,"VOCAL ";', expect_result=rvm_value("VOCAL "))
rvm("get_reac_status", {"port": "B"}, "RCQ:B;", device_reply="\x02RCS:B,0;", expect_result=rvm_value("0"))
rvm("get_version", {}, "VRQ;", device_reply="\x02VRS:1.500,1.100,1.000;", expect_result=rvm_value("1.500"))
rvm("resend_last_reply", {}, "ERR:0;")


def rvmt(name, inbound, **extra):
    telemetry(RVM, name, inbound="\x02" + inbound, **extra)


rvmt("name", 'CNS:I1,"VOCAL ";', expect_state={"channels": {"I1": {"name": "VOCAL "}}})
rvmt("fader", "FDS:R1L,-12.5;", expect_state={"channels": {"R1L": {"fader": "-12.5"}}})
rvmt("fader-inf", "FDS:MAC,INF;", expect_state={"channels": {"MAC": {"fader": "-inf"}}})
rvmt("mute", "MUS:MG2,1;", expect_state={"channels": {"MG2": {"mute": True}}})
rvmt("phantom", "PTS:I4,1;", expect_state={"channels": {"I4": {"phantom": True}}})
rvmt("polarity", "PSS:I4,1;", expect_state={"channels": {"I4": {"polarity": True}}})
rvmt("filter", "FLS:I4,0;", expect_state={"channels": {"I4": {"filter": False}}})
rvmt("eq", "EQS:MAL,1;", expect_state={"channels": {"MAL": {"eq": True}}})
rvmt("gate", "GTS:I4,1;", expect_state={"channels": {"I4": {"gate": True}}})
rvmt("compressor", "CPS:AX2,1;", expect_state={"channels": {"AX2": {"compressor": True}}})
rvmt("gain", "PGS:I1,0,-55;", expect_state={"channels": {"I1": {"pad": False, "gain": -55}}})
rvmt("pan-left", "PNS:I1,L63;", expect_state={"channels": {"I1": {"pan": -63}}})
rvmt("pan-centre", "PNS:I1,C;", expect_state={"channels": {"I1": {"pan": 0}}})
rvmt("pan-right", "PNS:MX3,R5;", expect_state={"channels": {"MX3": {"pan": 5}}})
rvmt("input-patch", "PIS:I1,RAI21;", expect_state={"channels": {"I1": {"input_patch": "RAI21"}}})
rvmt("output-patch", "POS:RAO1,MAL;", expect_state={"output_patch": {"RAO1": "MAL"}})
rvmt("aux-send", "AXS:I1,AX2,-10.5,L63;",
     expect_state={"channels": {"I1": {"aux_sends": {"AX2": {"level": "-10.5", "pan": -63}}}}})
rvmt("matrix-send", "MXS:AX4,MX8,INF,C;",
     expect_state={"channels": {"AX4": {"matrix_sends": {"MX8": {"level": "-inf", "pan": 0}}}}})
rvmt("matrix-send-right", "MXS:MAL,MX1,0.0,R20;",
     expect_state={"channels": {"MAL": {"matrix_sends": {"MX1": {"level": "0.0", "pan": 20}}}}})
rvmt("scene", 'SCS:023,"JAZZ BAND MUSIC";', expect_state={"scene": {"number": 23, "name": "JAZZ BAND MUSIC"}})
rvmt("lock", "LCS:1;", expect_state={"console": {"locked": True}})
rvmt("blackout", "BLS:0;", expect_state={"console": {"blackout": False}})
rvmt("dimmer", "DMS:1;", expect_state={"console": {"monitor_dimmer": True}})
rvmt("display-brightness", "DBS:50;", expect_state={"brightness": {"display": 50}})
rvmt("panel-brightness", "PBS:20;", expect_state={"brightness": {"panel": 20}})
rvmt("lamp-brightness", "LBS:0;", expect_state={"brightness": {"lamp": 0}})
rvmt("recorder-status", "RTS:0;", expect_state={"recorder": {"status": "stopped"}})
rvmt("recorder-position", "RLS:34s;", expect_state={"recorder": {"position": "34s"}})
rvmt("recorder-song", "RIS:2,5;", expect_state={"recorder": {"song": 2, "song_count": 5}})
rvmt("recorder-song-name", 'RNS:2,"SONG02.WAV";', expect_state={"recorder": {"songs": {"2": {"name": "SONG02.WAV"}}}})
rvmt("recorder-remaining", "RRS:10h00m00s;", expect_state={"recorder": {"remaining": "10h00m00s"}})
rvmt("reac", "RCS:A,2;", expect_state={"reac": {"A": {"status": "connected"}}})
rvmt("reac-error", "EPS:B,0;", expect_state={"reac": {"B": {"error": "fan_stopped"}}})
rvmt("console-error", "ERS:99;", expect_state={"device": {"error": 99}})
rvmt("version", "VRS:1.500,1.100,1.000;",
     expect_state={"device": {"version": "1.500", "panel_version": "1.100", "fader_version": "1.000"}})
telemetry(RVM, "main-send", inbound="MAS:I1,MAL,1;\r\n", model="m-200i",
          expect_state={"channels": {"I1": {"main_sends": {"MAL": True}}}})
telemetry(RVM, "delay", inbound="DSS:AX8,1;\r\n", model="m-200i", expect_state={"channels": {"AX8": {"delay": True}}})
telemetry(RVM, "delay-time", inbound="DTS:MAL,1.2,MS;\r\n", model="m-200i",
          expect_state={"channels": {"MAL": {"delay_time": "1.2", "delay_unit": "MS"}}})
