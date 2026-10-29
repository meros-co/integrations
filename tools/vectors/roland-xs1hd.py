XS1 = "roland-xs1hd"
# Roland XS-1HD (Reference Manual edition 02, p.9-10): RS-232 commands are
# STX (02H) + code + ":" arguments + ";" with no line ending; a command is
# acknowledged by the single byte ACK (06H). Inputs and memories are 0-based
# on the wire.
XS1_OK = {"ok": {"kind": "ack"}}
XS1_ERR = {"error": {"error": "device_error"}}


def xs1(command, input, wire, **extra):
    text(XS1, command, input, wire, **extra)


xs1("select_xpt", {"xpt": 1, "input": 1}, "\x02XP1:0;", device_reply="\x06", expect_result=XS1_OK)
xs1("set_transition_time", {"time": 20}, "\x02TIM:20;", device_reply="\x02ERR:5;", expect_result=XS1_ERR)
xs1("auto", {}, "\x02ATO;", device_reply="\x06", expect_result=XS1_OK)
xs1("set_output_level", {"level": 127}, "\x02MVL:127;")
xs1("set_channel_level", {"channel": 4, "level": 0}, "\x02CV4:0;")
xs1("recall_memory", {"memory": 16}, "\x02MEM:15;")
xs1("get_version", {}, "\x02VER;", device_reply="\x02VER:XS-1HD,1.01;",
    expect_result={"ok": {"kind": "value", "value": "1.01"}})
xs1("active_sense", {}, "\x02ACS;", device_reply="\x06", expect_result=XS1_OK)

telemetry(XS1, "version", expect_connect_wire=["\x02VER;"], inbound="\x02VER:XS-1HD,1.01;",
          expect_state={"device": {"model": "XS-1HD", "version": "1.01"}})
# Levels in dB, by Roland's 100 = 0.0 dB and 127 = +6.0 dB (p.6), rounded to
# the unit's step.
xs1("set_output_level_db", {"level_db": 6.0}, "\x02MVL:127;")
xs1("set_channel_level_db", {"channel": 2, "level_db": 3.0}, "\x02CV2:114;")
