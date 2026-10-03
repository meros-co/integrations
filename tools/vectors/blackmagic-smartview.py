# Blackmagic SmartView / SmartScope: the SmartView Ethernet Protocol v1.4 on TCP
# 9992, "Name: value" blocks ended by a blank line (Blackmagic SmartView and
# SmartScope manual, July 2024, p.38-41). The wire forms below follow the
# manual's own examples: "NETWORK:\nDynamic IP: true", the static block with
# 192.168.2.2 / 255.255.255.0 / 192.168.2.1 (p.39), "MONITOR A:\nBrightness: 127"
# (p.39), "WidescreenSD: ON" / "OFF", "Identify: true", "MONITOR B:\nBorder:
# green", "ScopeMode: Picture" (p.40), "AudioChannel: 0" and "LUT: 0" (p.41).
# The ACK reply, status requests and PING are inferred (see the spec's quirks).
SV = "blackmagic-smartview"
SCOPE = {"model": "smartscope-duo-4k"}
SV4K = {"model": "smartview-4k"}

# Network and name (p.39).
text(SV, "set_network_dhcp", {}, "NETWORK:\nDynamic IP: true\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(SV, "set_network_static", {"address": "192.168.2.2", "netmask": "255.255.255.0", "gateway": "192.168.2.1"},
     "NETWORK:\nDynamic IP: false\nStatic address: 192.168.2.2\nStatic netmask: 255.255.255.0\n"
     "Static gateway: 192.168.2.1\n\n")
text(SV, "set_device_name", {"name": "StageFront"}, "SMARTVIEW DEVICE:\nName: StageFront\n\n")

# Monitor settings (p.39-41).
text(SV, "set_brightness", {"monitor": "A", "level": 127}, "MONITOR A:\nBrightness: 127\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(SV, "set_contrast", {"monitor": "B", "level": 127}, "MONITOR B:\nContrast: 127\n\n")
text(SV, "set_saturation", {"monitor": "A", "level": 0}, "MONITOR A:\nSaturation: 0\n\n",
     device_reply="NAK\n\n", expect_result={"error": {"error": "device_error"}})
text(SV, "identify", {"monitor": "A"}, "MONITOR A:\nIdentify: true\n\n")
text(SV, "set_border", {"monitor": "B", "color": "green"}, "MONITOR B:\nBorder: green\n\n")
text(SV, "set_widescreen_sd", {"monitor": "A", "enabled": True}, "MONITOR A:\nWidescreenSD: ON\n\n")
text(SV, "set_scope_mode", {"monitor": "A", "mode": "Picture"}, "MONITOR A:\nScopeMode: Picture\n\n", **SCOPE)
text(SV, "set_audio_channel", {"monitor": "B", "pair": 1}, "MONITOR B:\nAudioChannel: 0\n\n", **SCOPE)
text(SV, "set_lut", {"monitor": "A", "lut": 2}, "MONITOR A:\nLUT: 1\n\n", **SV4K)
text(SV, "disable_lut", {"monitor": "A"}, "MONITOR A:\nLUT: NONE\n\n", **SV4K)
# Community-observed (Companion module).
text(SV, "set_monitor_input", {"monitor": "A", "input": "SDI B"}, "MONITOR A:\nMonitorInput: SDI B\n\n", **SV4K)

# Status requests and PING, in the Videohub form.
text(SV, "get_device_info", {}, "SMARTVIEW DEVICE:\n\n")
text(SV, "get_network", {}, "NETWORK:\n\n")
text(SV, "get_monitor", {"monitor": "B"}, "MONITOR B:\n\n")
text(SV, "ping", {}, "PING:\n\n", device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})


# ── Telemetry ──
telemetry(SV, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 1.4\n\n",
          expect_state={"device": {"protocol_version": "1.4"}})
# The p.38 device block.
telemetry(SV, "device", inbound="SMARTVIEW DEVICE:\nModel: SmartView Duo\nHostname: stagefront.studio.example.com\n"
          "Name: StageFront\nMonitors: 2\nInverted: false\n\n",
          expect_state={"device": {"model": "SmartView Duo", "hostname": "stagefront.studio.example.com",
                                   "name": "StageFront", "monitors": 2, "inverted": False}})
# The p.39 network block.
telemetry(SV, "network", inbound="NETWORK:\nDynamic IP: true\nStatic address: 192.168.2.2\n"
          "Static netmask: 255.255.255.0\nStatic gateway: 192.168.2.1\nCurrent address: 192.168.1.101\n"
          "Current netmask: 255.255.255.0\nCurrent gateway: 192.168.1.1\n\n",
          expect_state={"network": {"dynamic_ip": True, "static_address": "192.168.2.2",
                                    "static_netmask": "255.255.255.0", "static_gateway": "192.168.2.1",
                                    "current_address": "192.168.1.101", "current_netmask": "255.255.255.0",
                                    "current_gateway": "192.168.1.1"}})
telemetry(SV, "monitor-a", inbound="MONITOR A:\nBrightness: 255\nContrast: 127\nSaturation: 127\nIdentify: false\n"
          "Border: RED\nWidescreenSD: ON\nLUT: NONE\n\n",
          expect_state={"monitors": {"A": {"brightness": 255, "contrast": 127, "saturation": 127,
                                           "identify": False, "border": "red", "widescreen_sd": True,
                                           "lut": "none"}}})
telemetry(SV, "monitor-b", inbound="MONITOR B:\nBorder: green\nScopeMode: AudioDbvu\nAudioChannel: 0\n"
          "WidescreenSD: OFF\n\n",
          expect_state={"monitors": {"B": {"border": "green", "scope_mode": "AudioDbvu", "audio_channel": 1,
                                           "widescreen_sd": False}}})
telemetry(SV, "monitor-lut-input", inbound="MONITOR A:\nLUT: 1\nMonitorInput: SDI A\n\n",
          expect_state={"monitors": {"A": {"lut": "2", "input": "SDI A"}}})
