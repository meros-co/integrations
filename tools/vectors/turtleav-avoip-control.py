# Turtle AV DARWIN Control / CHAZY Control / CHAZY Control Pro: the
# controllers' command line on Telnet 23, one command per line (CR LF
# assumed), replies [SUCCESS]... or [ERROR]... (DARWIN Control API Reference,
# Chazy Control API Reference, CHAZY Control Pro help listing). Wire forms
# follow the documents' examples: "SET DEC 1 SWITCH 3 ALL", "SET DEC 1 SWITCH
# 0 VIDEO", "SET DEC 1 OUTPUT RESOLUTION 2", "SET DEC 1 OUTPUT FLIP HOR",
# "SET GPIO 1 LEVEL High", "SET DEC 1 RELAY 1 CLOSE", "SET DEC 1 CEC SEND 4F
# 36", "APPLY WALL 1 PRESET 1".
TA = "turtleav-avoip-control"
DW = {"model": "darwin-control"}
CZ = {"model": "chazy-control"}
CP = {"model": "chazy-control-pro"}


def ta(command, input, line, **extra):
    text(TA, command, input, line + "\r\n", **extra)


ta("route", {"decoder": 1, "encoder": 3}, "SET DEC 1 SWITCH 3 ALL",
   device_reply="[SUCCESS]Set decoder 001 from encoder 003.\r\n", expect_result={"ok": {"kind": "ack"}})
ta("route_signal", {"decoder": 1, "encoder": 0, "signal": "VIDEO"}, "SET DEC 1 SWITCH 0 VIDEO",
   device_reply="[ERROR]Decoder 100 does not exist.\r\n", expect_result={"error": {"error": "device_error"}})
ta("route_signal_chazy", {"decoder": 1, "encoder": 3, "signal": "AUDIO"}, "SET DEC 1 SWITCH 3 AUDIO", **CZ)
ta("set_decoder_output", {"decoder": 1, "enabled": True}, "SET DEC 1 OUTPUT ON")
ta("set_decoder_mute", {"decoder": 1, "muted": False}, "SET DEC 1 OUTPUT MUTE OFF")
ta("set_decoder_osd", {"decoder": 1, "visible": True}, "SET DEC 1 OUTPUT OSD ON")
ta("flash_decoder_led", {"decoder": 1, "flash": True}, "SET DEC 1 LED ON")
ta("flash_encoder_led", {"encoder": 2, "flash": False}, "SET ENC 2 LED OFF")
ta("set_decoder_resolution", {"decoder": 1, "code": 1}, "SET DEC 1 OUTPUT RESOLUTION 1", **DW)
ta("set_decoder_resolution_chazy", {"decoder": 1, "code": 2}, "SET DEC 1 OUTPUT RESOLUTION 2", **CZ)
ta("set_decoder_resolution_pro", {"decoder": 1, "code": 17}, "SET DEC 1 OUTPUT RESOLUTION 17", **CP)
ta("set_decoder_rotate", {"decoder": 1, "rotation": 1}, "SET DEC 1 OUTPUT ROTATE 1")
ta("set_decoder_flip", {"decoder": 1, "flip": "HOR"}, "SET DEC 1 OUTPUT FLIP HOR", **CZ)
ta("set_decoder_pause", {"decoder": 1, "paused": True}, "SET DEC 1 OUTPUT PAUSE ON", **DW)
ta("set_decoder_freeze", {"decoder": 1, "frozen": True}, "SET DEC 1 OUTPUT FREEZE ON", **CP)
ta("set_decoder_mode", {"decoder": 1, "mode": "MX"}, "SET DEC 1 MODE MX")
ta("set_decoder_name", {"decoder": 1, "name": "TEST1"}, "SET DEC 1 NAME TEST1")
ta("set_encoder_name", {"encoder": 1, "name": "TX1"}, "SET ENC 1 NAME TX1")
ta("set_encoder_edid", {"encoder": 1, "code": 0}, "SET ENC 1 EDID DEFAULT 0", **DW)
ta("set_encoder_edid_chazy", {"encoder": 1, "code": 26}, "SET ENC 1 EDID DEFAULT 26", **CZ)
ta("set_encoder_edid_pro", {"encoder": 1, "code": 101}, "SET ENC 1 EDID DEFAULT 101", **CP)
ta("set_encoder_audio_input", {"encoder": 1, "source": "HDMI"}, "SET ENC 1 AUDIO INPUT HDMI", **DW)
ta("set_encoder_bitrate", {"encoder": 1, "code": 2}, "SET ENC 1 STREAM BITRATE 2", **DW)
ta("set_encoder_main_stream", {"encoder": 1, "codec": 1, "audio": True}, "SET ENC 1 MAINSTREAM E 1 A ON", **DW)
ta("apply_wall_preset", {"wall": 1, "preset": 1}, "APPLY WALL 1 PRESET 1",
   device_reply="[SUCCESS]Apply preset: Preset 1.\r\n", expect_result={"ok": {"kind": "ack"}})
ta("apply_wall_preset_pro", {"wall": 200, "preset": 9}, "APPLY WALL 200 PRESET 9", **CP)
ta("set_wall_preset_source", {"wall": 1, "preset": 2, "group": "A", "encoder": 3}, "SET WALL 1 PRESET 2 CLASS A SOURCE 3")
ta("set_gpio_direction", {"gpio": 1, "direction": "OUT"}, "SET GPIO 1 DIR OUT")
ta("set_gpio_level", {"gpio": 1, "level": "High"}, "SET GPIO 1 LEVEL High")
ta("set_relay", {"type": "DEC", "id": 1, "relay": 1, "state": "CLOSE"}, "SET DEC 1 RELAY 1 CLOSE", **CZ)
ta("set_io_output", {"type": "DEC", "id": 1, "pin": 1, "level": 0}, "SET DEC 1 IO 1 OUT 0", **CZ)
ta("send_cec", {"type": "DEC", "id": 1, "data": "4F 36"}, "SET DEC 1 CEC SEND 4F 36", **CZ)
ta("send_ir", {"type": "ENC", "id": 2, "data": "0000006D0022"}, "SET ENC 2 IR SEND 0000006D0022", **CZ)
ta("reboot_decoder", {"decoder": 1}, "SET DEC 1 REBOOT")
ta("reboot_encoder", {"encoder": 1}, "SET ENC 1 REBOOT")
ta("reboot_controller", {}, "SET REBOOT", **CZ)
ta("dante_subscribe", {"device": "Decoder-001", "channel": 1, "tx_device": "TX1", "tx_channel": 1},
   "SET DANTE DEV Decoder-001 AUDIO RXCHN 1 SOURCE TX1 CHN 1", **CZ)
ta("set_dante_latency", {"device": "Decoder-001", "latency": 5000}, "SET DANTE DEV Decoder-001 LATENCY 5000", **CZ)
ta("apply_config_preset", {"preset": 3}, "APPLY CONFIG PRESET 03", **CP)
ta("route_group", {"group": 2, "encoder": 5}, "SET GROUP 2 SWITCH 5 ALL", **CP)
ta("apply_dante_preset", {"preset": 1}, "APPLY DANTE PRESET 1", **CP)
ta("set_ultra_low_latency", {"decoder": 1, "enabled": True}, "SET DEC 1 ULL ON", **CP)
ta("send_serial", {"decoder": 1, "message": "PWR ON"}, "SET DEC 1 SENDGUEST ASCII PWR ON", **CP)

# ── Telemetry: this session's [SUCCESS] replies (the documents' examples) ──
telemetry(TA, "route", inbound="[SUCCESS]Set decoder 001 from encoder 003.\r\n",
          expect_state={"decoders": {"1": {"source": 3}}})
telemetry(TA, "unroute", inbound="[SUCCESS]Set decoder 001 VARSUC unselect encoder.\r\n",
          expect_state={"decoders": {"1": {"source": 0}}})
telemetry(TA, "output", inbound="[SUCCESS]Set decoder 001 output mute on.\r\n",
          expect_state={"decoders": {"1": {"mute": True}}})
telemetry(TA, "rotate", inbound="[SUCCESS]Set decoder 001 rotate 90 degree.\r\n",
          expect_state={"decoders": {"1": {"rotation": 90}}})
telemetry(TA, "wall", inbound="[SUCCESS]Apply preset: Preset 1.\r\n", expect_state={"walls": {"last_preset": "Preset 1"}})
