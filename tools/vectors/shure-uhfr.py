UR = "shure-uhfr"
# UHF-R command strings (technical bulletin, 2011): one datagram per message,
# "* " and " *" around it, answered with REPORT.
UR_ACK = {"ok": {"kind": "ack"}}


def ur(command, input, wire, **extra):
    # The answer is one datagram, given as hex.
    if "device_reply" in extra:
        extra["device_reply_hex"] = extra.pop("device_reply").encode("ascii").hex()
    text(UR, command, input, wire, **extra)


def ur_value(v):
    return {"ok": {"kind": "value", "value": v}}


ur("get_channel_name", {"channel": 1}, "* GET 1 CHAN_NAME *", device_reply="* REPORT 1 CHAN_NAME Vocal 1       *",
     expect_result=ur_value("Vocal 1"))
ur("set_channel_name", {"channel": 2, "name": "Guitar"}, "* SET 2 CHAN_NAME Guitar *", device_reply="* REPORT 2 CHAN_NAME Guitar       *",
     expect_result=UR_ACK, model="ur4d")
ur("get_mute", {"channel": 1}, "* GET 1 MUTE *", device_reply="* REPORT 1 MUTE OFF *", expect_result=ur_value("OFF"))
ur("set_mute", {"channel": 1, "muted": True}, "* SET 1 MUTE ON *", device_reply="* REPORT 1 MUTE ON *", expect_result=UR_ACK)
ur("toggle_mute", {"channel": 1}, "* SET 1 MUTE TOGGLE *", device_reply="* REPORT 1 MUTE OFF *", expect_result=UR_ACK)
ur("get_audio_gain", {"channel": 1}, "* GET 1 AUDIO_GAIN *", device_reply="* REPORT 1 AUDIO_GAIN 12 *", expect_result=ur_value("12"))
ur("set_audio_gain", {"channel": 1, "value": 5}, "* SET 1 AUDIO_GAIN 05 *", device_reply="* REPORT 1 AUDIO_GAIN 05 *", expect_result=UR_ACK)
ur("gain_up", {"channel": 1, "db": 3}, "* SET 1 GAIN_UP 3 *", device_reply="* REPORT 1 AUDIO_GAIN 09 *", expect_result=UR_ACK)
ur("gain_down", {"channel": 1, "db": 5}, "* SET 1 GAIN_DOWN 5 *")
ur("get_group_channel", {"channel": 1}, "* GET 1 GROUP_CHAN *", device_reply="* REPORT 1 GROUP_CHAN 03 12 *", expect_result=ur_value("03 12"))
ur("set_group_channel", {"channel": 1, "group": 3, "group_channel": 12}, "* SET 1 GROUP_CHAN 02 11 *")
ur("group_up", {"channel": 1}, "* SET 1 GROUP_UP *")
ur("group_down", {"channel": 1}, "* SET 1 GROUP_DOWN *")
ur("channel_up", {"channel": 1}, "* SET 1 CHAN_UP *")
ur("channel_down", {"channel": 1}, "* SET 1 CHAN_DOWN *")
ur("get_battery", {"channel": 1}, "* GET 1 TX_BAT *", device_reply="* REPORT 1 TX_BAT U *", expect_result=ur_value("U"))
ur("get_transmitter_type", {"channel": 1}, "* GET 1 TX_TYPE *", device_reply="* Report 1 TX_TYPE UR2 *", expect_result=ur_value("UR2"))
ur("start_metering", {"channel": 1, "interval": 40}, "* METER 1 ALL 040 *", device_reply="* REPORT 1 CHAN_NAME Vocal 1       *",
     expect_result=UR_ACK)
ur("stop_metering", {"channel": 1}, "* METER 1 ALL STOP *")

telemetry(UR, "name", inbound_hex=hexs("* REPORT 1 CHAN_NAME Vocal 1       *".encode()), expect_state={"channels": {"1": {"name": "Vocal 1"}}})
telemetry(UR, "mute", inbound_hex=hexs("* REPORT 2 MUTE ON *".encode()), expect_state={"channels": {"2": {"muted": True}}})
telemetry(UR, "audio-gain", inbound_hex=hexs("* REPORT 1 AUDIO_GAIN 32 *".encode()), expect_state={"channels": {"1": {"audio_gain": 32}}})
telemetry(UR, "group-channel", inbound_hex=hexs("* REPORT 1 GROUP_CHAN 03 12 *".encode()), expect_state={"channels": {"1": {"group": "03", "group_channel": "12"}}})
telemetry(UR, "group-channel-off-plan", inbound_hex=hexs("* REPORT 1 GROUP_CHAN -- -- *".encode()), expect_state={"channels": {"1": {"group": "--", "group_channel": "--"}}})
telemetry(UR, "frequency", inbound_hex=hexs("* REPORT 1 FREQUENCY 518125 *".encode()), expect_state={"channels": {"1": {"frequency_khz": 518125}}})
telemetry(UR, "battery", inbound_hex=hexs("* REPORT 1 TX_BAT 4 *".encode()), expect_state={"channels": {"1": {"battery": "4"}}})
telemetry(UR, "transmitter", inbound_hex=hexs("* Report 1 TX_TYPE UR1H *".encode()), expect_state={"channels": {"1": {"transmitter": "UR1H"}}})
telemetry(UR, "sample", inbound_hex=hexs("* SAMPLE 1 ALL AB 070 085 4 128 *".encode()),
          expect_state={"channels": {"1": {"antennas": "AB", "rf_a": 70, "rf_b": 85, "battery": "4", "audio_level": 128}}})
