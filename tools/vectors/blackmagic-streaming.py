# Blackmagic Web Presenter / Streaming Encoder: blocks of "Key: value" lines
# ended by a blank line, LF line endings, answered with "ACK" or "NACK" and a
# blank line (Blackmagic Streaming Ethernet Protocol v1.2, January 2026, p.2-3).
# The wire forms below follow the document's own examples: "STREAM SETTINGS:\n
# Video Mode: 1080p59.94\n\n" (p.3), "STREAM SETTINGS:\n\n" requests a status
# dump (p.4), "IDENTITY:\nLabel: My Streaming Encoder\n\n" (p.4), the network
# examples (p.7), the Twitch example (p.9), the stream XML examples (p.10-11),
# "STREAM STATE:\nAction: Start\n\n" (p.12) and "SHUTDOWN:\nAction: Reboot\n\n" (p.14).
BS = "blackmagic-streaming"

ACK = "ACK\n\n"

# Identity and version (p.4-5). Doc example: "Label: My Streaming Encoder".
text(BS, "set_label", {"label": "My Streaming Encoder"}, "IDENTITY:\nLabel: My Streaming Encoder\n\n",
     device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "get_identity", {}, "IDENTITY:\n\n")
text(BS, "get_version", {}, "VERSION:\n\n")

# Network (p.5-7). Doc examples on interface 0.
text(BS, "get_network", {}, "NETWORK:\n\n")
text(BS, "get_network_interface", {"interface": 1}, "NETWORK INTERFACE 1:\n\n")
text(BS, "set_network_dhcp", {"interface": 0}, "NETWORK INTERFACE 0:\nDynamic IP: true\n\n",
     device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "set_network_static",
     {"interface": 0, "addresses": "192.168.1.2/255.255.255.0", "gateway": "192.168.1.1",
      "dns_servers": "8.8.8.8, 8.8.4.4"},
     "NETWORK INTERFACE 0:\nDynamic IP: false\nStatic Addresses: 192.168.1.2/255.255.255.0\n"
     "Static Gateway: 192.168.1.1\nStatic DNS Servers: 8.8.8.8, 8.8.4.4\n\n")
text(BS, "set_network_interface_priority", {"interface": 1, "priority": 2},
     "NETWORK INTERFACE 1:\nPriority: 2\n\n")

# UI settings (p.7-8).
text(BS, "get_ui_settings", {}, "UI SETTINGS:\n\n")
text(BS, "set_locale", {"locale": "ja_JP.UTF-8"}, "UI SETTINGS:\nCurrent Locale: ja_JP.UTF-8\n\n")
text(BS, "set_audio_meter", {"meter": "VU -18dB"}, "UI SETTINGS:\nCurrent Audio Meter: VU -18dB\n\n")

# Stream settings (p.3-4, p.8-9).
text(BS, "get_stream_settings", {}, "STREAM SETTINGS:\n\n",
     device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "set_video_mode", {"mode": "1080p59.94"}, "STREAM SETTINGS:\nVideo Mode: 1080p59.94\n\n",
     device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "set_platform", {"platform": "YouTube SRT (Beta)"},
     "STREAM SETTINGS:\nCurrent Platform: YouTube SRT (Beta)\n\n")
# A server name holding ": " (p.9) goes out unchanged.
text(BS, "set_server", {"server": "US West: Los Angeles, CA"},
     "STREAM SETTINGS:\nCurrent Server: US West: Los Angeles, CA\n\n")
text(BS, "set_quality_level", {"quality": "Streaming Medium"},
     "STREAM SETTINGS:\nCurrent Quality Level: Streaming Medium\n\n")
text(BS, "set_stream_key", {"key": "abc1-def2-ghi3-jkl4-mno5"},
     "STREAM SETTINGS:\nStream Key: abc1-def2-ghi3-jkl4-mno5\n\n")
text(BS, "set_stream_password", {"password": "pp-abcd-efgh-ijkl-mnop"},
     "STREAM SETTINGS:\nPassword: pp-abcd-efgh-ijkl-mnop\n\n")
text(BS, "set_stream_url", {"url": "srt://192.168.8.51"}, "STREAM SETTINGS:\nCurrent URL: srt://192.168.8.51\n\n")
# The document's Twitch example (p.9).
text(BS, "set_stream_settings",
     {"mode": "1080p59.94", "platform": "Twitch", "server": "US West: Los Angeles, CA",
      "quality": "Streaming Medium", "key": "live_123456789_1aB2cD3eF4gH5iJ6kL7mN8oP9qR0sT"},
     "STREAM SETTINGS:\nVideo Mode: 1080p59.94\nCurrent Platform: Twitch\n"
     "Current Server: US West: Los Angeles, CA\nCurrent Quality Level: Streaming Medium\n"
     "Stream Key: live_123456789_1aB2cD3eF4gH5iJ6kL7mN8oP9qR0sT\n\n",
     device_reply="NACK\n\n", expect_result={"error": {"error": "device_error"}})

# Streaming XML (p.10-11).
text(BS, "get_stream_xml", {}, "STREAM XML:\n\n")
text(BS, "add_stream_xml",
     {"filename": "Custom.xml",
      "xml": '<?xml version="1.0" encoding="UTF-8"?><streaming><service><name>My Custom Platform</name>'
             '</service></streaming>'},
     'STREAM XML Custom.xml:\n<?xml version="1.0" encoding="UTF-8"?><streaming><service>'
     '<name>My Custom Platform</name></service></streaming>\n\n',
     device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "remove_stream_xml", {"filename": "Custom.xml"}, "STREAM XML:\nAction: Remove\nFiles: Custom.xml\n\n")
text(BS, "remove_all_stream_xml", {}, "STREAM XML:\nAction: Remove All\n\n")

# Streaming (p.12-13).
text(BS, "get_stream_state", {}, "STREAM STATE:\n\n")
text(BS, "start_stream", {}, "STREAM STATE:\nAction: Start\n\n",
     device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "stop_stream", {}, "STREAM STATE:\nAction: Stop\n\n",
     device_reply="NACK\n\n", expect_result={"error": {"error": "device_error"}})

# Audio (p.13). Doc example: monitor output audio source set to remote.
text(BS, "get_audio_settings", {}, "AUDIO SETTINGS:\n\n")
text(BS, "set_monitor_audio_source", {"source": "Remote Source"},
     "AUDIO SETTINGS:\nCurrent Monitor Out Audio Source: Remote Source\n\n")

# Power (p.14).
text(BS, "reboot", {}, "SHUTDOWN:\nAction: Reboot\n\n", device_reply=ACK, expect_result={"ok": {"kind": "ack"}})
text(BS, "factory_reset", {}, "SHUTDOWN:\nAction: Factory Reset\n\n")


# ── Telemetry (blocks pushed on connecting, after changes, and after a status request) ──
telemetry(BS, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 1.2\n\n",
          expect_state={"device": {"protocol_version": "1.2"}})
telemetry(BS, "end-prelude", inbound="END PRELUDE:\n\n", expect_state={"device": {"prelude_complete": True}})
telemetry(BS, "identity", inbound="IDENTITY:\nModel: Blackmagic Streaming Encoder HD\n"
          "Label: Blackmagic Streaming Encoder HD\nUnique ID: 00112233445566778899AABBCCDDEEFF\n\n",
          expect_state={"identity": {"model": "Blackmagic Streaming Encoder HD",
                                     "label": "Blackmagic Streaming Encoder HD",
                                     "unique_id": "00112233445566778899AABBCCDDEEFF"}})
telemetry(BS, "version", inbound="VERSION:\nProduct ID: BE73\nHardware Version: 0100\n"
          "Software Version: 0123ABCD\nSoftware Release: 3.5\n\n",
          expect_state={"version": {"product_id": "BE73", "hardware_version": "0100",
                                    "software_version": "0123ABCD", "software_release": "3.5"}})
telemetry(BS, "network", inbound="NETWORK:\nInterface Count: 2\nDefault Interface: 0\n\n",
          expect_state={"network": {"interface_count": 2, "default_interface": 0}})
# Protocol 1.0 (April 2021 manual, p.27): DNS servers in the NETWORK block.
telemetry(BS, "network-v1-0", inbound="NETWORK:\nInterface Count: 2\nDefault Interface: 0\n"
          "Static DNS Servers: 8.8.8.8, 8.8.4.4\nCurrent DNS Servers: 192.168.1.1, 8.8.4.4\n\n",
          expect_state={"network": {"interface_count": 2, "default_interface": 0,
                                    "static_dns_servers": "8.8.8.8, 8.8.4.4",
                                    "current_dns_servers": "192.168.1.1, 8.8.4.4"}})
telemetry(BS, "network-interface-0", inbound="NETWORK INTERFACE 0:\nName: Ethernet\nPriority: 1\n"
          "MAC Address: 00:11:22:33:44:55\nDynamic IP: true\nCurrent Addresses: 192.168.1.10/255.255.255.0\n"
          "Current Gateway: 192.168.1.1\nCurrent DNS Servers: 192.168.1.1, 8.8.8.8, 8.8.4.4\n"
          "Static Addresses: 10.0.0.2/255.255.255.0\nStatic Gateway: 10.0.0.1\n"
          "Static DNS Servers: 8.8.8.8, 8.8.4.4\n\n",
          expect_state={"network": {"interfaces": {"0": {
              "name": "Ethernet", "priority": 1, "mac_address": "00:11:22:33:44:55", "dynamic_ip": True,
              "current_addresses": "192.168.1.10/255.255.255.0", "current_gateway": "192.168.1.1",
              "current_dns_servers": "192.168.1.1, 8.8.8.8, 8.8.4.4",
              "static_addresses": "10.0.0.2/255.255.255.0", "static_gateway": "10.0.0.1",
              "static_dns_servers": "8.8.8.8, 8.8.4.4"}}}})
# The p.6 example: the tethered smartphone interface, with no current DNS servers.
telemetry(BS, "network-interface-1", inbound="NETWORK INTERFACE 1:\nName: USBEthernet\nPriority: 0\n"
          "MAC Address: 00:00:00:00:00:00\nDynamic IP: true\nCurrent Addresses: 0.0.0.0/255.255.0.0\n"
          "Current Gateway: 0.0.0.0\nCurrent DNS Servers: \nStatic Addresses: 10.0.0.2/255.255.255.0\n"
          "Static Gateway: 10.0.0.1\nStatic DNS Servers: 8.8.8.8, 8.8.4.4\n\n",
          expect_state={"network": {"interfaces": {"1": {
              "name": "USBEthernet", "priority": 0, "mac_address": "00:00:00:00:00:00", "dynamic_ip": True,
              "current_addresses": "0.0.0.0/255.255.0.0", "current_gateway": "0.0.0.0",
              "current_dns_servers": "", "static_addresses": "10.0.0.2/255.255.255.0",
              "static_gateway": "10.0.0.1", "static_dns_servers": "8.8.8.8, 8.8.4.4"}}}})
# The p.7 change notice after a static address request.
telemetry(BS, "network-interface-static", inbound="NETWORK INTERFACE 0:\nDynamic IP: false\n"
          "Static Addresses: 192.168.1.2/255.255.255.0\nStatic Gateway: 192.168.1.1\n"
          "Static DNS Servers: 8.8.8.8, 8.8.4.4\n\n",
          expect_state={"network": {"interfaces": {"0": {
              "dynamic_ip": False, "static_addresses": "192.168.1.2/255.255.255.0",
              "static_gateway": "192.168.1.1", "static_dns_servers": "8.8.8.8, 8.8.4.4"}}}})
telemetry(BS, "ui-settings", inbound="UI SETTINGS:\nAvailable Locales: en_US.UTF-8, zh_CN.UTF-8, ja_JP.UTF-8\n"
          "Current Locale: en_US.UTF-8\nAvailable Audio Meters: PPM -18dB, PPM -20dB, VU -18dB, VU -20dB\n"
          "Current Audio Meter: PPM -20dB\n\n",
          expect_state={"ui": {"available_locales": "en_US.UTF-8, zh_CN.UTF-8, ja_JP.UTF-8",
                               "locale": "en_US.UTF-8",
                               "available_audio_meters": "PPM -18dB, PPM -20dB, VU -18dB, VU -20dB",
                               "audio_meter": "PPM -20dB"}})
# The p.8 dump, its typeset line wraps joined.
telemetry(BS, "stream-settings", inbound="STREAM SETTINGS:\n"
          "Available Video Modes: Auto, 1080p23.98, 1080p24, 1080p25, 1080p29.97, 1080p30, 1080p50, "
          "1080p59.94, 1080p60, 720p25, 720p30, 720p50, 720p60\n"
          "Video Mode: 1080p59.94\nCurrent Platform: YouTube\nCurrent Server: Primary\n"
          "Current Quality Level: Streaming Medium\nStream Key: abc1-def2-ghi3-jkl4-mno5\nPassword: \n"
          "Current URL: srt://192.168.8.51\nCustomizable URL: true\n"
          "Available Default Platforms: YouTube RTMP, YouTube SRT (Beta), Facebook, Twitch\n"
          "Available Custom Platforms: My Platform\nAvailable Servers: Primary, Secondary\n"
          "Available Quality Levels: Streaming High, Streaming Medium, Streaming Low\n\n",
          expect_state={"stream": {
              "available_video_modes": "Auto, 1080p23.98, 1080p24, 1080p25, 1080p29.97, 1080p30, 1080p50, "
                                       "1080p59.94, 1080p60, 720p25, 720p30, 720p50, 720p60",
              "video_mode": "1080p59.94", "platform": "YouTube", "server": "Primary",
              "quality_level": "Streaming Medium", "key": "abc1-def2-ghi3-jkl4-mno5", "password": "",
              "url": "srt://192.168.8.51", "customizable_url": True,
              "available_default_platforms": "YouTube RTMP, YouTube SRT (Beta), Facebook, Twitch",
              "available_custom_platforms": "My Platform", "available_servers": "Primary, Secondary",
              "available_quality_levels": "Streaming High, Streaming Medium, Streaming Low"}})
# The p.9 change notice: a server name holding ": " is split at the first colon only.
telemetry(BS, "stream-settings-change", inbound="STREAM SETTINGS:\nVideo Mode: 1080p59.94\n"
          "Current Platform: Twitch\nCurrent Server: US West: Los Angeles, CA\n"
          "Current Quality Level: Streaming Medium\nStream Key: live_123456789_1aB2cD3eF4gH5iJ6kL7mN8oP9qR0sT\n\n",
          expect_state={"stream": {"video_mode": "1080p59.94", "platform": "Twitch",
                                   "server": "US West: Los Angeles, CA", "quality_level": "Streaming Medium",
                                   "key": "live_123456789_1aB2cD3eF4gH5iJ6kL7mN8oP9qR0sT"}})
telemetry(BS, "stream-xml", inbound="STREAM XML:\nFiles: Custom.xml\n\n",
          expect_state={"stream_xml": {"files": "Custom.xml"}})
# The p.11 notice after Remove: "Files:" with nothing after it.
telemetry(BS, "stream-xml-empty", inbound="STREAM XML:\nFiles:\n\n", expect_state={"stream_xml": {"files": ""}})
# The p.12 block; the poll asks for it on connecting.
telemetry(BS, "stream-state", expect_connect_wire=["STREAM STATE:\n\n"],
          inbound="STREAM STATE:\nStatus: Idle\nBitrate: 161672\nDuration: 00:00:00:00\nCache Used: 0\n\n",
          expect_state={"stream_state": {"status": "Idle", "bitrate": 161672, "duration": "00:00:00:00",
                                         "cache_used": 0}})
# The p.12 push after a start request.
telemetry(BS, "stream-state-status", inbound="STREAM STATE:\nStatus: Streaming\n\n",
          expect_state={"stream_state": {"status": "Streaming"}})
telemetry(BS, "audio-settings", inbound="AUDIO SETTINGS:\nCurrent Monitor Out Audio Source: Auto\n"
          "Available Monitor Out Audio Sources: Auto, SDI In, Remote Source\n\n",
          expect_state={"audio": {"monitor_out_source": "Auto",
                                  "available_monitor_out_sources": "Auto, SDI In, Remote Source"}})
