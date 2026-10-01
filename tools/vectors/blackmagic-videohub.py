# Videohub: a block ended by a blank line; every port is 0-based on the wire,
# so chassis port N is written N-1 (Blackmagic Videohub Ethernet Protocol v2.3,
# November 2023). The wire forms below follow the document's own examples:
# "VIDEO OUTPUT ROUTING:\n7 2\n\n" routes input 3 to output 8 (p.6), "SERIAL
# PORT LOCKS:\n7 O\n\n" locks, "7 F" force-unlocks (p.7), "OUTPUT LABELS:\n\n"
# requests a status dump (p.8) and "PING:\n\n" checks the connection (p.8).
VH = "blackmagic-videohub"

# Routing (p.4, p.6). Doc example: output port 8 <- input port 3 is "7 2".
text(VH, "set_route", {"input": 3, "output": 8}, "VIDEO OUTPUT ROUTING:\n7 2\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(VH, "set_monitoring_route", {"input": 8, "monitoring_output": 1},
     "VIDEO MONITORING OUTPUT ROUTING:\n0 7\n\n")
text(VH, "set_serial_route", {"port": 1, "source_port": 13}, "SERIAL PORT ROUTING:\n0 12\n\n")
text(VH, "set_processing_unit_route", {"unit": 2, "source": 4}, "PROCESSING UNIT ROUTING:\n1 3\n\n",
     model="workgroup-videohub")
text(VH, "set_frame_buffer_route", {"frame": 1, "source": 8}, "FRAME BUFFER ROUTING:\n0 7\n\n",
     model="workgroup-videohub")

# Labels (p.3-4). Doc example: output 7 relabelled is "6 new output label seven".
text(VH, "set_input_label", {"input": 1, "label": "VTR 1"}, "INPUT LABELS:\n0 VTR 1\n\n")
text(VH, "set_output_label", {"output": 7, "label": "new output label seven"},
     "OUTPUT LABELS:\n6 new output label seven\n\n",
     device_reply="NAK\n\n", expect_result={"error": {"error": "device_error"}})
text(VH, "set_monitoring_output_label", {"monitoring_output": 2, "label": "Monitor feed 2"},
     "MONITORING OUTPUT LABELS:\n1 Monitor feed 2\n\n")
text(VH, "set_serial_port_label", {"port": 1, "label": "Deck 1"}, "SERIAL PORT LABELS:\n0 Deck 1\n\n")
text(VH, "set_frame_label", {"frame": 2, "label": "Frame two"}, "FRAME LABELS:\n1 Frame two\n\n",
     model="workgroup-videohub")

# Locks (p.4-5, p.7): O to lock, U to unlock, F to force-unlock.
text(VH, "lock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 O\n\n")
text(VH, "unlock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 U\n\n")
text(VH, "force_unlock_output", {"output": 3}, "VIDEO OUTPUT LOCKS:\n2 F\n\n")
text(VH, "lock_monitoring_output", {"monitoring_output": 1}, "MONITORING OUTPUT LOCKS:\n0 O\n\n")
text(VH, "unlock_monitoring_output", {"monitoring_output": 1}, "MONITORING OUTPUT LOCKS:\n0 U\n\n")
text(VH, "force_unlock_monitoring_output", {"monitoring_output": 1}, "MONITORING OUTPUT LOCKS:\n0 F\n\n")
text(VH, "lock_serial_port", {"port": 8}, "SERIAL PORT LOCKS:\n7 O\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
text(VH, "unlock_serial_port", {"port": 6}, "SERIAL PORT LOCKS:\n5 U\n\n")
text(VH, "force_unlock_serial_port", {"port": 8}, "SERIAL PORT LOCKS:\n7 F\n\n")
text(VH, "lock_processing_unit", {"unit": 1}, "PROCESSING UNIT LOCKS:\n0 O\n\n", model="workgroup-videohub")
text(VH, "unlock_processing_unit", {"unit": 1}, "PROCESSING UNIT LOCKS:\n0 U\n\n", model="workgroup-videohub")
text(VH, "force_unlock_processing_unit", {"unit": 1}, "PROCESSING UNIT LOCKS:\n0 F\n\n",
     model="workgroup-videohub")
text(VH, "lock_frame_buffer", {"frame": 2}, "FRAME BUFFER LOCKS:\n1 O\n\n", model="workgroup-videohub")
text(VH, "unlock_frame_buffer", {"frame": 2}, "FRAME BUFFER LOCKS:\n1 U\n\n", model="workgroup-videohub")
text(VH, "force_unlock_frame_buffer", {"frame": 2}, "FRAME BUFFER LOCKS:\n1 F\n\n",
     model="workgroup-videohub")

# Serial port directions (p.5): control, slave or auto.
text(VH, "set_serial_port_direction", {"port": 3, "direction": "auto"}, "SERIAL PORT DIRECTIONS:\n2 auto\n\n")

# Take mode, protocol v2.8 (Videohub manual, May 2025, p.50): the CONFIGURATION
# block's "Take Mode: true", and a TAKE MODE line "<output-1> true|false".
text(VH, "set_take_mode", {"enabled": True}, "CONFIGURATION:\nTake Mode: true\n\n",
     model="smart-videohub-cleanswitch-12x12")
text(VH, "set_output_take_mode", {"output": 4, "enabled": False}, "TAKE MODE:\n3 false\n\n",
     model="videohub-40x40-12g")

# Status dump requests (p.8): the header, then a blank line.
for command, header, model in [
    ("get_device_info", "VIDEOHUB DEVICE:", None),
    ("get_input_labels", "INPUT LABELS:", None),
    ("get_monitoring_output_labels", "MONITORING OUTPUT LABELS:", "studio-videohub"),
    ("get_serial_port_labels", "SERIAL PORT LABELS:", "studio-videohub"),
    ("get_frame_labels", "FRAME LABELS:", "workgroup-videohub"),
    ("get_video_output_routing", "VIDEO OUTPUT ROUTING:", None),
    ("get_monitoring_output_routing", "VIDEO MONITORING OUTPUT ROUTING:", "studio-videohub"),
    ("get_serial_port_routing", "SERIAL PORT ROUTING:", "studio-videohub"),
    ("get_processing_unit_routing", "PROCESSING UNIT ROUTING:", "workgroup-videohub"),
    ("get_frame_buffer_routing", "FRAME BUFFER ROUTING:", "workgroup-videohub"),
    ("get_video_output_locks", "VIDEO OUTPUT LOCKS:", None),
    ("get_monitoring_output_locks", "MONITORING OUTPUT LOCKS:", "studio-videohub"),
    ("get_serial_port_locks", "SERIAL PORT LOCKS:", "studio-videohub"),
    ("get_processing_unit_locks", "PROCESSING UNIT LOCKS:", "workgroup-videohub"),
    ("get_frame_buffer_locks", "FRAME BUFFER LOCKS:", "workgroup-videohub"),
    ("get_serial_port_directions", "SERIAL PORT DIRECTIONS:", "studio-videohub"),
    ("get_video_input_status", "VIDEO INPUT STATUS:", "universal-videohub-288"),
    ("get_video_output_status", "VIDEO OUTPUT STATUS:", "universal-videohub-288"),
    ("get_serial_port_status", "SERIAL PORT STATUS:", "universal-videohub-288"),
    ("get_configuration", "CONFIGURATION:", "smart-videohub-cleanswitch-12x12"),
    ("get_take_mode", "TAKE MODE:", "videohub-40x40-12g"),
    ("get_network", "NETWORK:", "videohub-mini-8x4-12g"),
    ("get_network_interface", "NETWORK INTERFACE:", "videohub-mini-8x4-12g"),
    ("get_alarm_status", "ALARM STATUS:", "universal-videohub-288"),
]:
    extra = {"model": model} if model else {}
    text(VH, command, {}, header + "\n\n", **extra)
# The p.8 dialogue: ACK first; the block itself follows and goes to telemetry.
text(VH, "get_output_labels", {}, "OUTPUT LABELS:\n\n",
     device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})

text(VH, "ping", {}, "PING:\n\n", device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})


# ── Telemetry (status blocks; 0-based on the wire, 1-based in the state) ──
telemetry(VH, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 2.3\n\n",
          expect_state={"device": {"protocol_version": "2.3"}})
telemetry(VH, "device", inbound="VIDEOHUB DEVICE:\nDevice present: true\nModel name: Blackmagic Smart Videohub\n"
          "Video inputs: 16\nVideo processing units: 0\nVideo outputs: 16\nVideo monitoring outputs: 0\n"
          "Serial ports: 0\n\n",
          expect_state={"device": {"present": "true", "model": "Blackmagic Smart Videohub", "inputs": 16,
                                   "processing_units": 0, "outputs": 16, "monitoring_outputs": 0,
                                   "serial_ports": 0}})
telemetry(VH, "device-absent", inbound="VIDEOHUB DEVICE:\nDevice present: false\n\n",
          expect_state={"device": {"present": "false"}})
# Protocol v2.8 device block, the May 2025 manual's own example (p.46).
telemetry(VH, "device-identity", inbound="VIDEOHUB DEVICE:\nDevice present: true\n"
          "Model name: Blackmagic Videohub 10x10 12G\nFriendly name: Blackmagic Videohub 10x10 12G\n"
          "Unique ID: A654D3524FD4493FAB12105BACF1299F\nVideo inputs: 10\nVideo outputs: 10\n\n",
          expect_state={"device": {"present": "true", "model": "Blackmagic Videohub 10x10 12G",
                                   "friendly_name": "Blackmagic Videohub 10x10 12G",
                                   "unique_id": "A654D3524FD4493FAB12105BACF1299F",
                                   "inputs": 10, "outputs": 10}})
telemetry(VH, "input-labels", inbound="INPUT LABELS:\n0 VTR 1\n1 VTR 2\n\n",
          expect_state={"inputs": {"1": {"label": "VTR 1"}, "2": {"label": "VTR 2"}}})
# A change notice carries only the changed lines (p.6).
telemetry(VH, "output-labels", inbound="OUTPUT LABELS:\n7 New output 8 label\n10 New output 11 label\n\n",
          expect_state={"outputs": {"8": {"label": "New output 8 label"}, "11": {"label": "New output 11 label"}}})
telemetry(VH, "monitoring-output-labels", inbound="MONITORING OUTPUT LABELS:\n0 Monitor feed 1\n1 Monitor feed 2\n\n",
          expect_state={"monitoring_outputs": {"1": {"label": "Monitor feed 1"}, "2": {"label": "Monitor feed 2"}}})
telemetry(VH, "serial-port-labels", inbound="SERIAL PORT LABELS:\n0 Deck 1\n1 Deck 2\n\n",
          expect_state={"serial_ports": {"1": {"label": "Deck 1"}, "2": {"label": "Deck 2"}}})
telemetry(VH, "frame-labels", inbound="FRAME LABELS:\n0 Frame one\n1 Frame two\n\n",
          expect_state={"frame_buffers": {"1": {"label": "Frame one"}, "2": {"label": "Frame two"}}})
telemetry(VH, "routing", inbound="VIDEO OUTPUT ROUTING:\n0 5\n1 3\n\n",
          expect_state={"outputs": {"1": {"input": 6}, "2": {"input": 4}}})
telemetry(VH, "monitoring-routing", inbound="VIDEO MONITORING OUTPUT ROUTING:\n0 7\n1 8\n\n",
          expect_state={"monitoring_outputs": {"1": {"input": 8}, "2": {"input": 9}}})
telemetry(VH, "serial-routing", inbound="SERIAL PORT ROUTING:\n0 12\n1 11\n\n",
          expect_state={"serial_ports": {"1": {"source_port": 13}, "2": {"source_port": 12}}})
telemetry(VH, "processing-unit-routing", inbound="PROCESSING UNIT ROUTING:\n0 5\n1 3\n\n",
          expect_state={"processing_units": {"1": {"source": 6}, "2": {"source": 4}}})
telemetry(VH, "frame-buffer-routing", inbound="FRAME BUFFER ROUTING:\n0 7\n1 8\n\n",
          expect_state={"frame_buffers": {"1": {"source": 8}, "2": {"source": 9}}})
telemetry(VH, "locks", inbound="VIDEO OUTPUT LOCKS:\n0 O\n1 L\n2 U\n\n",
          expect_state={"outputs": {"1": {"lock": "ours"}, "2": {"lock": "other"}, "3": {"lock": "unlocked"}}})
telemetry(VH, "monitoring-output-locks", inbound="MONITORING OUTPUT LOCKS:\n0 U\n1 L\n\n",
          expect_state={"monitoring_outputs": {"1": {"lock": "unlocked"}, "2": {"lock": "other"}}})
# The p.6 example: serial port 6 unlocked.
telemetry(VH, "serial-port-locks", inbound="SERIAL PORT LOCKS:\n5 U\n\n",
          expect_state={"serial_ports": {"6": {"lock": "unlocked"}}})
telemetry(VH, "processing-unit-locks", inbound="PROCESSING UNIT LOCKS:\n0 O\n\n",
          expect_state={"processing_units": {"1": {"lock": "ours"}}})
telemetry(VH, "frame-buffer-locks", inbound="FRAME BUFFER LOCKS:\n1 L\n\n",
          expect_state={"frame_buffers": {"2": {"lock": "other"}}})
telemetry(VH, "serial-port-directions", inbound="SERIAL PORT DIRECTIONS:\n0 control\n1 slave\n2 auto\n\n",
          expect_state={"serial_ports": {"1": {"direction": "control"}, "2": {"direction": "slave"},
                                         "3": {"direction": "auto"}}})
telemetry(VH, "video-input-status", inbound="VIDEO INPUT STATUS:\n0 BNC\n1 Optical\n2 None\n\n",
          expect_state={"inputs": {"1": {"hardware": "BNC"}, "2": {"hardware": "Optical"}, "3": {"hardware": "None"}}})
telemetry(VH, "video-output-status", inbound="VIDEO OUTPUT STATUS:\n0 BNC\n1 None\n\n",
          expect_state={"outputs": {"1": {"hardware": "BNC"}, "2": {"hardware": "None"}}})
telemetry(VH, "serial-port-status", inbound="SERIAL PORT STATUS:\n0 RS422\n1 None\n\n",
          expect_state={"serial_ports": {"1": {"hardware": "RS422"}, "2": {"hardware": "None"}}})
# Protocol v2.8 blocks (May 2025 manual, p.46 and p.50).
telemetry(VH, "network", inbound="NETWORK:\nInterface Count: 1\nDefault Interface: 0\n\n",
          expect_state={"network": {"interface_count": 1, "default_interface": 0}})
telemetry(VH, "network-interface", inbound="NETWORK INTERFACE:\nName: 1 GbE\nPriority: 1\n"
          "MAC Address: 7c:2e:0d:07:26:a0\nDynamic IP: false\nCurrent Addresses: 192.168.25.253/255.255.255.0\n"
          "Current Gateway: 192.168.25.1\nStatic Addresses: 192.168.25.253/255.255.255.0\n"
          "Static Gateway: 192.168.25.1\n\n",
          expect_state={"network": {"interface": {
              "name": "1 GbE", "priority": 1, "mac_address": "7c:2e:0d:07:26:a0", "dynamic_ip": False,
              "current_addresses": "192.168.25.253/255.255.255.0", "current_gateway": "192.168.25.1",
              "static_addresses": "192.168.25.253/255.255.255.0", "static_gateway": "192.168.25.1"}}})
telemetry(VH, "take-mode", inbound="CONFIGURATION:\nTake Mode: true\n\n", expect_state={"take_mode": True})
# The p.50 mixed example: outputs alternate take on and off.
telemetry(VH, "output-take-mode", inbound="TAKE MODE:\n0 true\n1 false\n2 true\n\n",
          expect_state={"outputs": {"1": {"take_mode": True}, "2": {"take_mode": False},
                                    "3": {"take_mode": True}}})
telemetry(VH, "end-prelude", inbound="END PRELUDE:\n\n", expect_state={"device": {"prelude_complete": True}})
# Not in Blackmagic's documents (community parser).
telemetry(VH, "alarm-status", inbound="ALARM STATUS:\nFan 0: ok\nFan 1: ok\n\n",
          expect_state={"alarms": {"Fan 0": "ok", "Fan 1": "ok"}})
