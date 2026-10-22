VR4 = "roland-vr4hd"
# Roland VR-4HD (Reference Manual v2.100, p.21-23): RS-232 commands are STX
# (02H) + code + ":" arguments + ";" with no line ending; a command is
# acknowledged by the single byte ACK (06H), the status requests and VER by
# their frame. Channels and memories are 0-based on the wire.
VR4_OK = {"ok": {"kind": "ack"}}
VR4_ERR = {"error": {"error": "device_error"}}


def vr4(command, input, wire, **extra):
    text(VR4, command, input, wire, **extra)


vr4("select_channel", {"channel": 4}, "\x02PGM:3;", device_reply="\x06", expect_result=VR4_OK)
vr4("set_input4_connector", {"connector": 2}, "\x02IS4:2;", device_reply="\x02ERR:4;", expect_result=VR4_ERR)
vr4("set_transition", {"effect": 1}, "\x02TRS:1;")
vr4("set_transition_time", {"time": 40}, "\x02TIM:40;")
vr4("set_pinp", {"enabled": True}, "\x02PIP:1;")
vr4("set_pinp_channel", {"channel": 1}, "\x02PIS:0;")
vr4("set_key", {"enabled": False}, "\x02DSK:0;")
vr4("set_key_source", {"channel": 0}, "\x02DSS:-1;")
vr4("set_output_fade", {"enabled": True}, "\x02FDE:1;")
vr4("set_freeze", {"enabled": True}, "\x02FRZ:1;")
vr4("set_audio_input_level", {"channel": 3, "level": 100}, "\x02LM3:100;")
vr4("set_audio_pair_level", {"pair": 2, "level": 127}, "\x02LS2:127;")
vr4("set_hdmi_level", {"input": 1, "level": 0}, "\x02LH1:0;")
vr4("set_usb_level", {"level": 64}, "\x02LUB:64;")
vr4("set_main_level", {"level": 90}, "\x02LMN:90;")
vr4("recall_memory", {"memory": 8}, "\x02MEM:7;")
vr4("reset_usb", {}, "\x02RUB;", device_reply="\x06", expect_result=VR4_OK)
vr4("get_version", {}, "\x02VER;", device_reply="\x02VER:VR-4HD,2.10;",
    expect_result={"ok": {"kind": "value", "value": "2.10"}})
vr4("get_pinp", {}, "\x02QPI;", device_reply="\x02QPI:1,2;", expect_result={"ok": {"kind": "value", "value": "1,2"}})
vr4("get_key", {}, "\x02QDK;", device_reply="\x02QDK:0,-1;", expect_result={"ok": {"kind": "value", "value": "0,-1"}})
vr4("get_channel", {}, "\x02QPG;", device_reply="\x02QPG:3;", expect_result={"ok": {"kind": "value", "value": "3"}})
vr4("get_usb", {}, "\x02QUB;", device_reply="\x02QUB:3;", expect_result={"ok": {"kind": "value", "value": "3"}})

telemetry(VR4, "channel", expect_connect_wire=["\x02VER;"], inbound="\x02QPG:2;",
          expect_state={"program": {"channel": 3}})
telemetry(VR4, "pinp", inbound="\x02QPI:1,0;", expect_state={"pinp": {"on": True, "channel": 1}})
telemetry(VR4, "key", inbound="\x02QDK:1,-1;", expect_state={"key": {"on": True, "source": 0}})
telemetry(VR4, "usb", inbound="\x02QUB:2;", expect_state={"usb": {"connection": 2}})
telemetry(VR4, "version", inbound="\x02VER:VR-4HD,2.10;",
          expect_state={"device": {"model": "VR-4HD", "version": "2.10"}})
