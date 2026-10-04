# Blackmagic Ultimatte 12: the Ultimatte 12 Ethernet Protocol 2.3 on TCP 9998,
# "Name: value" blocks ended by a blank line (Ultimatte 12 manual, February
# 2026, Developer Information p.85-100). The wire forms follow the manual's
# own examples: "control:\nbacking color: blue" and "matte density: 273"
# (p.85, written with the table's capitalisation), "CONTROL:\nMatte Density:
# 100" and "Offset Matte Density: 10" (p.89), the header-only status request
# "CONTROL:" (p.90), the FILE and GPI blocks (p.90-91), "BG 1 Frame Buffer
# Index: 1" (p.91) and "CCU Camera Id: 5" (p.91). Functions are sent as Yes
# (p.100 footnote 5).
UL = "blackmagic-ultimatte"
U4K = {"model": "ultimatte-12-4k"}
MINI = {"model": "ultimatte-12-hd-mini"}


def ul(command, input, body, **extra):
    text(UL, command, input, body + "\n\n", **extra)


ul("set_level", {"control": "Matte Density", "value": 100}, "CONTROL:\nMatte Density: 100",
   device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
ul("offset_level", {"control": "Matte Density", "delta": 10}, "CONTROL:\nOffset Matte Density: 10")
ul("set_window_position", {"control": "Window Position Top", "value": 120}, "CONTROL:\nWindow Position Top: 120")
ul("set_correct_size", {"control": "Veil Correct Horizontal Size", "value": 6}, "CONTROL:\nVeil Correct Horizontal Size: 6")
ul("set_matte_process", {"control": "GM Process Vertical", "value": 3}, "CONTROL:\nGM Process Vertical: 3")
ul("set_gpi_delay", {"control": "GP 1 Input Delay", "value": 120}, "CONTROL:\nGP 1 Input Delay: 120")
ul("set_transition_rate", {"value": 30}, "CONTROL:\nTransition Rate: 30")
ul("set_fg_input_frame_delay", {"value": 14}, "CONTROL:\nFG Input Frame Delay: 14")
ul("set_output_offset", {"value": -1500}, "CONTROL:\nOutput Offset: -1500")
ul("set_switch", {"control": "Matte Enable", "state": "Off"}, "CONTROL:\nMatte Enable: Off",
   device_reply="NAK\n\n", expect_result={"error": {"error": "device_error"}})
ul("quickload", {"slot": 2}, "CONTROL:\nQuickload 2: On")
ul("quicksave", {"slot": 5}, "CONTROL:\nQuicksave 5: On")
ul("set_backing_color", {"value": "Blue"}, "CONTROL:\nBacking Color: Blue")
ul("set_monitor_out", {"value": "Combined Matte"}, "CONTROL:\nMonitor Out: Combined Matte")
ul("set_layer_order", {"value": "BG Layer/FG/Layer/BG"}, "CONTROL:\nLayer Order: BG Layer/FG/Layer/BG")
ul("set_video_format", {"value": "1080p59.94"}, "CONTROL:\nVideo Format: 1080p59.94")
ul("set_ly_in_mix_mode", {"value": "Additive"}, "CONTROL:\nLY In Mix Mode: Additive")
ul("set_filter_mode", {"value": "Average"}, "CONTROL:\nFilter Mode: Average")
ul("set_filter_median", {"value": 4}, "CONTROL:\nFilter Median: 4")
ul("set_filter_average", {"value": 0}, "CONTROL:\nFilter Average: 0")
ul("set_3g_sdi_level", {"value": "B"}, "CONTROL:\n3G SDI level: B")
ul("set_color_space", {"value": "Rec.2020"}, "CONTROL:\nColor Space: Rec.2020")
ul("set_gp_out_level", {"value": "Low"}, "CONTROL:\nGP Out Level: Low")
ul("set_output_range", {"value": "Full"}, "CONTROL:\nOutput Range: Full")
ul("run_function", {"function": "Sample Wall"}, "CONTROL:\nSample Wall: Yes")
ul("factory_defaults", {}, "CONTROL:\nFactory Defaults: Yes")
ul("user_defaults", {}, "CONTROL:\nUser Defaults: Yes")
ul("set_input_source", {"value": "IP2110"}, "IP VIDEO:\nInput Source: IP2110", **U4K)
ul("set_ip_output_enable", {"value": "On"}, "IP VIDEO:\nOutput Enable: On", **U4K)
ul("load_file", {"name": "Studio A"}, "FILE:\nLoad: Studio A")
ul("save_file", {"name": "Studio A"}, "FILE:\nSave: Studio A")
ul("delete_file", {"name": "Old Set"}, "FILE:\nDelete: Old Set")
ul("rename_file", {"name": "File 1", "new_name": "News"}, "FILE:\nRename: File 1\nTo: News")
ul("gpi_insert_event", {"id": 1, "name": "File 2"}, "GPI:\nID: 1\nInsert: File 2\nAt: -1")
ul("gpi_remove_event", {"id": 1, "index": 0}, "GPI:\nID: 1\nRemove: 0")
ul("gpi_set_index", {"id": 1, "index": 1}, "GPI:\nID: 1\nIndex: 1")
ul("set_frame_buffer", {"buffer": "BG 1", "image": 1},
   "FRAME BUFFER:\nBG 1 Frame Buffer Index: 1\nBG 1 Frame Buffer Enable: on")
ul("set_frame_buffer_enable", {"buffer": "BG 1", "state": "off"}, "FRAME BUFFER:\nBG 1 Frame Buffer Enable: off")
ul("set_ccu_camera_id", {"value": 5}, "CAMERA CONTROL:\nCCU Camera Id: 5", **MINI)
ul("request_status", {"block": "CONTROL"}, "CONTROL:", device_reply="ACK\n\n", expect_result={"ok": {"kind": "ack"}})
ul("ping", {}, "PING:")


# ── Telemetry, from the document's status dump examples (p.86-89) ──
telemetry(UL, "preamble", inbound="PROTOCOL PREAMBLE:\nVersion: 2.3\n\n",
          expect_state={"device": {"protocol_version": "2.3"}})
telemetry(UL, "identity", inbound="IDENTITY:\nModel: Ultimatte 12 8K\nLabel: Ultimatte 12 8K\nUnique ID: 12345678\n\n",
          expect_state={"device": {"model": "Ultimatte 12 8K", "label": "Ultimatte 12 8K", "unique_id": "12345678"}})
telemetry(UL, "network-interface", inbound="NETWORK INTERFACE 0:\nName: Cadence GigE Ethernet MAC\nPriority: 0\n"
          "DynamicIP: false\nCurrent Addresses: 10.0.0.2/255.255.255.0\nCurrent Gateway: 10.0.0.1\n\n",
          expect_state={"network": {"interface": {"name": "Cadence GigE Ethernet MAC", "dynamic_ip": False,
                                                  "current_addresses": "10.0.0.2/255.255.255.0",
                                                  "current_gateway": "10.0.0.1"}}})
telemetry(UL, "version", inbound="VERSION:\nProduct ID: BE85\nHardware Version: 0100\nSoftware Version: 1135E1AF\n"
          "Software Release: 2.3\n\n",
          expect_state={"device": {"product_id": "BE85", "hardware_version": "0100", "software_version": "1135E1AF",
                                   "software_release": "2.3"}})
telemetry(UL, "device", inbound="DEVICE:\nVideo Format: 1080p60\nReference Source: Foreground\nFG In: Locked\n"
          "BG In: Locked\nG MATTE In: Locked\n\n",
          expect_state={"device": {"video_format": "1080p60", "reference_source": "Foreground"},
                        "inputs": {"fg": "Locked", "bg": "Locked", "g_matte": "Locked"}})
telemetry(UL, "ip-video", inbound="IP VIDEO:\nInput Source: ip2110\nOutput Enable: on\n\n",
          expect_state={"ip_video": {"input_source": "ip2110", "output_enable": True}})
telemetry(UL, "control", inbound="CONTROL:\nMatte Density: 0\nRed Density: 0\nMatte Enable: On\nBacking Color: Green\n"
          "Tally Active: Off\nOutput Offset: -20\n\n",
          expect_state={"control": {"matte_density": 0, "red_density": 0, "matte_enable": True,
                                    "backing_color": "Green", "tally_active": False, "output_offset": -20}})
telemetry(UL, "current-file", inbound="CURRENT FILE:\nName: Ultimatte Defaults\nStatus: Clean\n\n",
          expect_state={"file": {"current": "Ultimatte Defaults", "status": "Clean"}})
telemetry(UL, "image-list", inbound="IMAGE LIST:\nCapacity: 6736052224\nAvailable: 6722727936\nImage 1\nImage 2\n\n",
          expect_state={"media": {"capacity": 6736052224, "available": 6722727936}})
telemetry(UL, "frame-buffer", inbound="FRAME BUFFER:\nBackground 1:\nBackground Mix: 0\nLayer Transition Duration: 500\n\n",
          expect_state={"frame_buffer": {"background_1": "", "background_mix": 0, "layer_transition_ms": 500}})
telemetry(UL, "camera-control", inbound="CAMERA CONTROL:\nCCU Camera Id: 1\nCEC Camera Id: 0\nCamera Control Mode: idle\n\n",
          expect_state={"camera_control": {"ccu_camera_id": 1, "cec_camera_id": 0, "mode": "idle"}})
telemetry(UL, "message", inbound="MESSAGE:\nWarning: Event limit exceeded\n\n",
          expect_state={"message": {"warning": "Event limit exceeded"}})
