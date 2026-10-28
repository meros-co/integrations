AX = "axia-lwrp"
# Axia Livewire Routing Protocol 2.0.2: ASCII commands ended by a line ending,
# sent without waiting (the device answers with indications only).


def lw(command, input, wire, **extra):
    text(AX, command, input, wire + "\r\n", **extra)


def lwt(name, inbound, **extra):
    telemetry(AX, name, inbound=inbound + "\r\n", **extra)


lw("login", {"password": "secret"}, "LOGIN secret")
lw("login_read_only", {}, "LOGIN")
lw("change_password", {"old_password": "old", "new_password": "new"}, "PASSWD old new")
lw("get_version", {}, "VER")
lw("get_ip", {}, "IP")
lw("set_ip", {"address": "192.168.2.20", "netmask": "255.255.255.0", "gateway": "192.168.2.1", "hostname": "STUDIO-A"},
   "IP address 192.168.2.20 netmask 255.255.255.0 gateway 192.168.2.1 hostname STUDIO-A")
lw("get_globals", {}, "SET")
lw("set_globals", {"values": "CLK:1 MODE:2"}, "SET CLK:1 MODE:2")
lw("save", {}, "SAVE")
lw("get_sources", {}, "SRC")
lw("set_source_name", {"port": 1, "name": "CD 1"}, 'SRC 1 PSNM:"CD 1"')            # §3.2.1 example
lw("set_source_rtp", {"port": 1, "enabled": True}, "SRC 1 RTPE:1")
lw("set_source_address", {"port": 1, "address": "239.192.0.1"}, 'SRC 1 RTPA:"239.192.0.1"')
lw("set_source_gain", {"port": 1, "gain": -100}, "SRC 1 INGN:-100")
lw("set_source_label", {"port": 2, "label": "MIC 2"}, 'SRC 2 LABL:"MIC 2"')
lw("set_source_channels", {"port": 3, "channels": 1}, "SRC 3 NCHN:1")
lw("get_destinations", {}, "DST")
lw("set_destination_address", {"port": 1, "address": "239.192.0.1 <CYGNUS 1@CYGNUS>"},
   'DST 1 ADDR:"239.192.0.1 <CYGNUS 1@CYGNUS>"')                                      # §3.2.2 example
lw("set_destination_name", {"port": 1, "name": "Omnia"}, 'DST 1 NAME:"Omnia"')
lw("set_destination_channels", {"port": 1, "channels": 2}, "DST 1 NCHN:2")
lw("get_meters", {}, "MTR")
lw("get_input_meters", {}, "MTR ICH")
lw("get_output_meters", {}, "MTR OCH")
lw("set_level_detector", {"direction": "OCH", "port": 1, "clip_level": -100, "clip_time": 3000,
                          "low_level": -600, "low_time": 3000},
   "LVL OCH 1 CLIP.LEVEL:-100 CLIP.TIME:3000 LOW.LEVEL:-600 LOW.TIME:3000")               # §3.2.6 example
lw("subscribe_gpi", {}, "ADD GPI")
lw("subscribe_gpi_ports", {"ports": "1 3 5-8"}, "ADD GPI 1 3 5-8")
lw("unsubscribe_gpi", {"ports": "2"}, "DEL GPI 2")
lw("subscribe_gpo", {}, "ADD GPO")
lw("subscribe_gpo_ports", {"ports": "1-4"}, "ADD GPO 1-4")
lw("unsubscribe_gpo", {"ports": "1-4"}, "DEL GPO 1-4")
lw("set_gpo", {"port": 2, "pins": "xxxxH"}, "GPO 2 xxxxH")                                # §3.3 example
lw("set_gpo_after", {"port": 2, "pins": "xxxxL", "duration": 250}, "GPO 2 xxxxL DURATION:250")
lw("set_gpi", {"port": 1, "pins": "lxxxx"}, "GPI 1 lxxxx")
lw("send_gpi_message", {"port": 7, "message": "BREAK 100 DURATION 3000"}, 'GPI 7 CMD:"BREAK 100 DURATION 3000"')
lw("send_gpo_message", {"port": 3, "message": "PRI ANA"}, 'GPO 3 CMD:"PRI ANA"')
lw("get_gpo_config", {"port": 1}, "CFG GPO 1")
lw("set_gpo_source", {"port": 1, "source": "192.168.2.21/3"}, "CFG GPO 1 SRCA:192.168.2.21/3")
lw("set_gpo_follow", {"port": 1}, "CFG GPO 1 FUNC:FOLLOW")
lw("set_gpo_pulse", {"port": 1, "change": "H", "polarity": "L", "duration": 500}, "CFG GPO 1 FUNC:PULSE/H/L/500")
lw("get_mixes", {}, "MIX")
lw("set_mix_point", {"mix": 1, "point": 7, "gain": -30}, "MIX 1 7:-30")                  # mixer doc example
lw("clear_mix_point", {"mix": 1, "point": 7}, "MIX 1 7")
lw("begin_batch", {}, "BEGIN")
lw("end_batch", {}, "END")
lw("execute_batch", {}, "EXE")

lwt("version", 'VER LWRP:1.4.2 DEVN:"LiveIO" SYSV:2.0.1 NSRC:8/1 NDST:8 NGPI:4 NGPO:4',
    expect_state={"device": {"lwrp_version": "1.4.2", "name": "LiveIO", "system_version": "2.0.1",
                             "sources": "8/1", "destinations": "8", "gpi_ports": 4, "gpo_ports": 4}})
lwt("globals", "SET CLK:1", expect_state={"device": {"globals": "CLK:1"}})
lwt("ip", "IP address 192.168.2.20 netmask 255.255.255.0 gateway 192.168.2.1 hostname STUDIO-A",
    expect_state={"network": {"address": "192.168.2.20", "netmask": "255.255.255.0", "gateway": "192.168.2.1",
                              "hostname": "STUDIO-A"}})
lwt("source", 'SRC 1 PSNM:"CD 1" RTPE:1 RTPA:"239.192.0.1" INGN:-100 LABL:"CD" NCHN:2',
    expect_state={"sources": {"1": {"name": "CD 1", "rtp_enabled": True, "address": "239.192.0.1", "gain": -100,
                                    "label": "CD", "channels": 2}}})
lwt("destination", 'DST 1 ADDR:"239.192.0.1 <CYGNUS 1@CYGNUS>" NAME:"Omnia" NCHN:2',
    expect_state={"destinations": {"1": {"address": "239.192.0.1 <CYGNUS 1@CYGNUS>", "name": "Omnia", "channels": 2}}})
lwt("destination-none", "DST 2 ADDR:0", expect_state={"destinations": {"2": {"address": "0"}}})
lwt("meter-input", "MTR ICH 1 PEEK:-120:-130 RMS:-200:-210",
    expect_state={"meters": {"inputs": {"1": {"peak_left": -120, "peak_right": -130, "rms_left": -200, "rms_right": -210}}}})
lwt("meter-output", "MTR OCH 2 PEEK:-1000:-1000 RMS:-1000:-1000",
    expect_state={"meters": {"outputs": {"2": {"peak_left": -1000, "peak_right": -1000, "rms_left": -1000,
                                               "rms_right": -1000}}}})
lwt("level-clip", "LVL OCH 1.L CLIP", expect_state={"levels": {"outputs": {"1": {"L": {"clip": True}}}}})
lwt("level-low", "LVL ICH 3.R NO-LOW", expect_state={"levels": {"inputs": {"3": {"R": {"low": False}}}}})
lwt("level-input-clip", "LVL ICH 2.L NO-CLIP", expect_state={"levels": {"inputs": {"2": {"L": {"clip": False}}}}})
lwt("level-output-low", "LVL OCH 4.R LOW", expect_state={"levels": {"outputs": {"4": {"R": {"low": True}}}}})
lwt("gpi", "GPI 7 hLhhh", expect_state={"gpi": {"7": {"pins": "hLhhh"}}})                 # §3.3 example
lwt("gpi-message", 'GPI 7 CMD:"BREAK 100 DURATION 3000"', expect_state={"gpi": {"7": {"message": "BREAK 100 DURATION 3000"}}})
lwt("gpo", "GPO 2 lllll", expect_state={"gpo": {"2": {"pins": "lllll"}}})
lwt("gpo-message", 'GPO 2 CMD:"ON AIR"', expect_state={"gpo": {"2": {"message": "ON AIR"}}})
lwt("gpo-config", "CFG GPO 1 SRCA:192.168.2.21/3 FUNC:FOLLOW",
    expect_state={"gpo": {"1": {"config": "SRCA:192.168.2.21/3 FUNC:FOLLOW"}}})
lwt("mix", "MIX 1 1:0 7:-30", state_before={"mixes": {"1": {"points": {"3": {"gain": -60}}}}},
    expect_state={"mixes": {"1": {"points": {"1": {"gain": 0}, "7": {"gain": -30}}}}})
lwt("error", "ERROR 1000 bad command", expect_state={"error": {"code": 1000, "message": "bad command"}})
