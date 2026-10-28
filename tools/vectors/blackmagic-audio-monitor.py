# Blackmagic Audio Monitor 12G G3: the Audio Monitor 12G Ethernet Protocol v1.4
# on TCP 9996, blocks ended by a blank line (Blackmagic Audio Monitor manual,
# 2025, p.16-19). The wire forms follow the manual's own examples:
# "AUDIOMONITOR DEVICE:\nLabel: My new name", "AUDIO METER:\nMeter Mode:
# Loudness (EBU +18 scale)", "AUDIO INPUT:\nRouting: Speaker Stereo XLR AES/EBU
# Stereo 1-2", "AUDIO OUTPUT:\nGain: Speaker Stereo 125", "Solo: Right", each
# answered with "ACK:" and the fields set (p.17-18), and "PING:" (p.19).
AM = "blackmagic-audio-monitor"
ACK = {"device_reply": "ACK:\n\n", "expect_result": {"ok": {"kind": "ack"}}}

text(AM, "set_label", {"label": "My new name"}, "AUDIOMONITOR DEVICE:\nLabel: My new name\n\n",
     device_reply="ACK:\nAUDIOMONITOR DEVICE:\nLabel: My new name\n\n", expect_result={"ok": {"kind": "ack"}})
text(AM, "set_meter_mode", {"mode": "Loudness (EBU +18 scale)"},
     "AUDIO METER:\nMeter Mode: Loudness (EBU +18 scale)\n\n", **ACK)
text(AM, "set_input", {"input": "XLR AES/EBU Stereo 1-2"},
     "AUDIO INPUT:\nRouting: Speaker Stereo XLR AES/EBU Stereo 1-2\n\n", **ACK)
text(AM, "set_speaker_gain", {"gain": 125}, "AUDIO OUTPUT:\nGain: Speaker Stereo 125\n\n", **ACK)
text(AM, "set_headphone_gain", {"gain": 0}, "AUDIO OUTPUT:\nGain: Headphone Stereo 0\n\n")
text(AM, "set_mute", {"muted": True}, "AUDIO OUTPUT:\nMute: true\n\n",
     device_reply="NAK\n\n", expect_result={"error": {"error": "device_error"}})
text(AM, "set_solo", {"solo": "Right"}, "AUDIO OUTPUT:\nSolo: Right\n\n", **ACK)

for command, header in [
    ("get_device_info", "AUDIOMONITOR DEVICE:"),
    ("get_network", "NETWORK:"),
    ("get_audio_meter", "AUDIO METER:"),
    ("get_audio_input", "AUDIO INPUT:"),
    ("get_st2110", "ST2110:"),
    ("get_audio_output", "AUDIO OUTPUT:"),
]:
    text(AM, command, {}, header + "\n\n")
text(AM, "ping", {}, "PING:\n\n", **ACK)


# ── Telemetry: the p.17-18 blocks ──
telemetry(AM, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 1.4\n\n",
          expect_state={"device": {"protocol_version": "1.4"}})
telemetry(AM, "device", inbound="AUDIOMONITOR DEVICE:\nModel: Blackmagic Audio Monitor 12G\n"
          "Label: Blackmagic Audio Monitor 12G\nUnique ID: 7c2e0d0a1b2c\n\n",
          expect_state={"device": {"model": "Blackmagic Audio Monitor 12G",
                                   "label": "Blackmagic Audio Monitor 12G", "unique_id": "7c2e0d0a1b2c"}})
telemetry(AM, "network", inbound="NETWORK:\nDynamic IP: 1\nStatic address: 0.0.0.0\nStatic subnet: 0.0.0.0\n"
          "Static gateway: 0.0.0.0\nCurrent address: 192.168.1.154\nCurrent subnet: 255.255.255.0\n"
          "Current gateway: 192.168.1.1\n\n",
          expect_state={"network": {"dynamic_ip": True, "static_address": "0.0.0.0",
                                    "static_subnet": "0.0.0.0", "static_gateway": "0.0.0.0",
                                    "current_address": "192.168.1.154", "current_subnet": "255.255.255.0",
                                    "current_gateway": "192.168.1.1"}})
telemetry(AM, "meter", inbound="AUDIO METER:\nMeter Mode: VU (-20dBFS Ref)\n\n",
          expect_state={"meter": {"mode": "VU (-20dBFS Ref)"}})
telemetry(AM, "input", inbound="AUDIO INPUT:\nRouting: Speaker Stereo SDI Stereo 1-2\n\n",
          expect_state={"input": {"routing": "SDI Stereo 1-2"}})
telemetry(AM, "st2110", inbound="ST2110:\nSDI Output Level: Auto\n\n",
          expect_state={"st2110": {"sdi_output_level": "Auto"}})
telemetry(AM, "output", inbound="AUDIO OUTPUT:\nGain: Speaker Stereo 0\nGain: Headphone Stereo 12\nMute: false\n"
          "Solo: Off\nAudio delay in ms: 0\nAudio delay in frames: 0\nAudio delay unit selected: Milliseconds\n\n",
          expect_state={"output": {"speaker_gain": 0, "headphone_gain": 12, "mute": False, "solo": "Off",
                                   "delay_ms": 0, "delay_frames": 0, "delay_unit": "Milliseconds"}})
# The ACK answer repeats the fields set (p.18).
telemetry(AM, "ack-output", inbound="ACK:\nAUDIO OUTPUT:\nGain: Speaker Stereo 125\nSolo: Right\n\n",
          expect_state={"output": {"speaker_gain": 125, "solo": "Right"}})
telemetry(AM, "ack-input", inbound="ACK:\nAUDIO INPUT:\nRouting: Speaker Stereo XLR AES/EBU Stereo 1-2\n\n",
          expect_state={"input": {"routing": "XLR AES/EBU Stereo 1-2"}})
telemetry(AM, "ack-meter", inbound="ACK:\nAUDIO METER:\nMeter Mode: Loudness (EBU +18 scale)\n\n",
          expect_state={"meter": {"mode": "Loudness (EBU +18 scale)"}})
