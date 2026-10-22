V12 = "roland-v1200hd"
# Roland V-1200HD (Remote Control Guide v1.4, p.89-91): RS-232 commands are
# STX (02H) + code + ":" arguments + ";" with no line ending; a command is
# acknowledged by the single byte ACK (06H). Stills, cross-points, buses,
# memories and STS groups are 0-based on the wire; input connectors are the
# guide's numbers 0-20.
V12_OK = {"ok": {"kind": "ack"}}
V12_ERR = {"error": {"error": "device_error"}}


def v12(command, input, wire, **extra):
    text(V12, command, input, wire, **extra)


v12("select_me_pgm", {"me": 1, "source": 0}, "\x02PM1:0;", device_reply="\x06", expect_result=V12_OK)
v12("select_me_pst", {"me": 2, "source": 20}, "\x02PT2:20;", device_reply="\x02ERR:6;", expect_result=V12_ERR)
v12("me_transition", {"me": 1, "type": 2}, "\x02PP1:2;")
v12("select_still", {"still": 2, "memory": 16}, "\x02ST2:15;")
v12("select_composition_source", {"composition": 4, "source": 13}, "\x02CS4:13;")
v12("set_composition_output", {"composition": 1, "type": 1, "enabled": True}, "\x02CP1:1,1;")
v12("select_aux", {"aux": 2, "source": 18}, "\x02AX2:18;")
v12("select_444_hdmi_output", {"output": 2, "xpt": 4, "type": 1}, "\x024X2:3,1;")
v12("select_444_composition_source", {"source": 3}, "\x024CS:3;")
v12("set_444_composition_output", {"type": 0, "enabled": False}, "\x024CP:0,0;")
v12("set_hdmi_fade", {"output": 1, "enabled": True}, "\x024F1:1;")
v12("set_output_fade", {"bus": 6, "enabled": True}, "\x02FDE:5,1;")
v12("recall_memory", {"memory": 1}, "\x02MEM:0;", device_reply="\x06", expect_result=V12_OK)
v12("get_status", {"group": 5}, "\x02STS:4;", device_reply="\x02STS:4,8;",
    expect_result={"ok": {"kind": "value", "value": "8"}})
v12("get_version", {}, "\x02VER;", device_reply="\x02VER:V-1200HD,1.40,0003;",
    expect_result={"ok": {"kind": "value", "value": "1.40,0003"}})
v12("active_sense", {}, "\x02ACS;", device_reply="\x06", expect_result=V12_OK)

telemetry(V12, "me-and-stills", expect_connect_wire=["\x02VER;"], inbound="\x02STS:0,0,1,10,20,0,15;",
          expect_state={"me": {"1": {"pgm": 0, "pst": 1}, "2": {"pgm": 10, "pst": 20}},
                        "stills": {"1": {"memory": 1}, "2": {"memory": 16}}})
telemetry(V12, "compositions-and-aux", inbound="\x02STS:1,2,1,3,0,4,1,5,0,6,7;",
          expect_state={"compositions": {"1": {"source": 2, "on": True}, "2": {"source": 3, "on": False},
                                         "3": {"source": 4, "on": True}, "4": {"source": 5, "on": False}},
                        "aux": {"1": {"source": 6}, "2": {"source": 7}}})
telemetry(V12, "process-444", inbound="\x02STS:2,0,3,2,1;",
          expect_state={"hdmi": {"1": {"xpt": 1}, "2": {"xpt": 4}},
                        "process_444": {"composition_source": 2, "composition_on": True}})
telemetry(V12, "fades", inbound="\x02STS:3,1,0,0,0,0,1,0,1;",
          expect_state={"buses": {"1": {"fade": True}, "2": {"fade": False}, "3": {"fade": False},
                                  "4": {"fade": False}, "5": {"fade": False}, "6": {"fade": True}},
                        "hdmi": {"1": {"fade": False}, "2": {"fade": True}}})
telemetry(V12, "memory", inbound="\x02STS:4,8;", expect_state={"memory": 0})
telemetry(V12, "memory-recalled", inbound="\x02STS:4,2;", expect_state={"memory": 3})
telemetry(V12, "version", inbound="\x02VER:V-1200HD,1.40,0003;",
          expect_state={"device": {"model": "V-1200HD", "version": "1.40", "build": "0003"}})
