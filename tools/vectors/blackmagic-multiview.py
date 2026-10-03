# Blackmagic MultiView 16 / MultiView 4: the Videohub Ethernet Protocol v2.3 on
# TCP 9990, blocks ended by a blank line, every port 0-based on the wire
# (Blackmagic MultiView manual, January 2022, p.35-38). The wire forms below
# follow the manual's own examples: "VIDEO OUTPUT ROUTING:\n7 2\n\n" routes input
# 3 to output 8, "CONFIGURATION:\nSolo enabled: true\n\n" enables solo, "16 10"
# makes input 11 the solo source, "17 0" (MultiView 16) and "5 0" (MultiView 4)
# embed audio from input 1 (p.37), "OUTPUT LABELS:\n6 new output label seven"
# and "OUTPUT LABELS:\n\n" requests a status dump (p.38), "PING:\n\n" (p.38).
MV = "blackmagic-multiview"
MV4 = {"model": "multiview-4"}

# Routing (p.37).
text(MV, "set_route", {"input": 3, "output": 8}, "VIDEO OUTPUT ROUTING:\n7 2\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(MV, "set_solo_source", {"input": 11}, "VIDEO OUTPUT ROUTING:\n16 10\n\n")
text(MV, "set_audio_source", {"input": 1}, "VIDEO OUTPUT ROUTING:\n17 0\n\n")
text(MV, "set_solo_source_4", {"input": 2}, "VIDEO OUTPUT ROUTING:\n4 1\n\n", **MV4)
text(MV, "set_audio_source_4", {"input": 1}, "VIDEO OUTPUT ROUTING:\n5 0\n\n", **MV4)

# Labels (p.36-38).
text(MV, "set_input_label", {"input": 1, "label": "VTR 1"}, "INPUT LABELS:\n0 VTR 1\n\n")
text(MV, "set_output_label", {"output": 7, "label": "new output label seven"},
     "OUTPUT LABELS:\n6 new output label seven\n\n",
     device_reply="NAK\n\n", expect_result={"error": {"error": "device_error"}})

# Locks (p.36): O to lock, U to unlock; F as in the Videohub protocol.
text(MV, "lock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 O\n\n")
text(MV, "unlock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 U\n\n")
text(MV, "force_unlock_output", {"output": 18}, "VIDEO OUTPUT LOCKS:\n17 F\n\n")

# Configuration (p.36-37).
text(MV, "set_solo_enabled", {"enabled": True}, "CONFIGURATION:\nSolo enabled: true\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(MV, "set_layout", {"layout": "4x4"}, "CONFIGURATION:\nLayout: 4x4\n\n")
text(MV, "set_output_format", {"format": "50p"}, "CONFIGURATION:\nOutput format: 50p\n\n")
text(MV, "set_widescreen_sd", {"enabled": False}, "CONFIGURATION:\nWidescreen SD enable: false\n\n")
text(MV, "set_display_border", {"enabled": True}, "CONFIGURATION:\nDisplay border: true\n\n")
text(MV, "set_display_labels", {"enabled": False}, "CONFIGURATION:\nDisplay labels: false\n\n")
text(MV, "set_display_audio_meters", {"enabled": True}, "CONFIGURATION:\nDisplay audio meters: true\n\n")
text(MV, "set_display_sdi_tally", {"enabled": True}, "CONFIGURATION:\nDisplay SDI tally: true\n\n")

# Status dump requests (p.38): the header, then a blank line.
for command, header in [
    ("get_device_info", "MULTIVIEW DEVICE:"),
    ("get_input_labels", "INPUT LABELS:"),
    ("get_video_output_routing", "VIDEO OUTPUT ROUTING:"),
    ("get_video_output_locks", "VIDEO OUTPUT LOCKS:"),
    ("get_configuration", "CONFIGURATION:"),
]:
    text(MV, command, {}, header + "\n\n")
text(MV, "get_output_labels", {}, "OUTPUT LABELS:\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(MV, "ping", {}, "PING:\n\n", device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})


# ── Telemetry (status blocks; 0-based on the wire, 1-based in the state) ──
telemetry(MV, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 2.3\n\n",
          expect_state={"device": {"protocol_version": "2.3"}})
# The p.35-36 device block, with its empty fields.
telemetry(MV, "device", inbound="MULTIVIEW DEVICE:\nDevice present: true\nModel name: Blackmagic MultiView 16\n"
          "Video inputs: 16\nFriendly name:\nUnique ID:\nVideo processing units: 0\nVideo outputs: 16\n"
          "Video monitoring outputs: 0\nSerial Ports:\n\n",
          expect_state={"device": {"present": "true", "model": "Blackmagic MultiView 16", "inputs": 16,
                                   "friendly_name": "", "unique_id": "", "processing_units": 0,
                                   "outputs": 16, "monitoring_outputs": 0}})
telemetry(MV, "input-labels", inbound="INPUT LABELS:\n0 VTR 1\n1 VTR 2\n\n",
          expect_state={"inputs": {"1": {"label": "VTR 1"}, "2": {"label": "VTR 2"}}})
# A change notice carries only the changed lines (p.37).
telemetry(MV, "output-labels", inbound="OUTPUT LABELS:\n7 New output 8 label\n10 New output 11 label\n\n",
          expect_state={"outputs": {"8": {"label": "New output 8 label"}, "11": {"label": "New output 11 label"}}})
telemetry(MV, "routing", inbound="VIDEO OUTPUT ROUTING:\n0 5\n1 3\n16 10\n17 0\n\n",
          expect_state={"outputs": {"1": {"input": 6}, "2": {"input": 4}, "17": {"input": 11},
                                    "18": {"input": 1}}})
telemetry(MV, "locks", inbound="VIDEO OUTPUT LOCKS:\n0 U\n1 O\n2 L\n\n",
          expect_state={"outputs": {"1": {"lock": "unlocked"}, "2": {"lock": "ours"}, "3": {"lock": "other"}}})
telemetry(MV, "configuration", inbound="CONFIGURATION:\nLayout: 4x4\nOutput format: 60i\nSolo enabled: false\n"
          "Widescreen SD enable: true\nDisplay border: true\nDisplay labels: true\nDisplay audio meters: false\n"
          "Display SDI tally: true\n\n",
          expect_state={"config": {"layout": "4x4", "output_format": "60i", "solo_enabled": False,
                                   "widescreen_sd": True, "display_border": True, "display_labels": True,
                                   "display_audio_meters": False, "display_sdi_tally": True}})
# The p.36 listing spells the booleans "True or False".
telemetry(MV, "configuration-capitalised", inbound="CONFIGURATION:\nLayout: SOLO\nSolo enabled: True\n"
          "Display border: False\n\n",
          expect_state={"config": {"layout": "SOLO", "solo_enabled": True, "display_border": False}})
