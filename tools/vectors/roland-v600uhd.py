RV6 = "roland-v600uhd"
# Roland V-600UHD (Reference Manual edition 07, LAN/RS-232 Command Reference
# p.58-59). Over LAN commands omit stx and replies are "ACK" LF; commands are
# sent ended CR LF as a Telnet client does. Cross-points, the transition
# pattern and memories are 0-based on the wire.
RV6_OK = {"ok": {"kind": "ack"}}


def rv6(command, input, wire, **extra):
    text(RV6, command, input, wire, **extra)


rv6("select_pgm", {"xpt": 1}, "PGM:0;\r\n", device_reply="ACK\n", expect_result=RV6_OK)
rv6("select_pst", {"xpt": 8}, "PST:7;\r\n", device_reply="ERR:5;\n",
    expect_result={"error": {"error": "device_error"}})
rv6("select_aux", {"xpt": 3}, "AUX:2;\r\n")
rv6("set_transition_pattern", {"pattern": 3}, "TRS:3;\r\n")
rv6("set_transition_time", {"time": 40}, "TIM:40;\r\n")
rv6("auto", {}, "ATO;\r\n", device_reply="ACK\n", expect_result=RV6_OK)
rv6("cut", {}, "CUT;\r\n")
rv6("set_composition_type", {"type": 4}, "CTY:4;\r\n")
rv6("set_dsk_type", {"type": 3}, "DTY:3;\r\n")
rv6("toggle_composition", {}, "CMP;\r\n")
rv6("toggle_dsk", {}, "DSK;\r\n")
rv6("set_output_fade", {"enabled": True}, "FDE:1;\r\n")
rv6("set_output_fade_time", {"time": 100}, "FDT:100;\r\n")
rv6("set_input_level", {"input": 7, "level": -801}, "IAL:7,-801;\r\n")
rv6("set_master_level", {"level": 0}, "OAL:0;\r\n")
rv6("set_aux_level", {"level": -800}, "OAX:-800;\r\n")
rv6("set_hdcp", {"enabled": False}, "HCP:0;\r\n")
rv6("recall_memory", {"memory": 64}, "MEM:63;\r\n")
rv6("active_sense", {}, "ACS;\r\n", device_reply="ACK\n", expect_result=RV6_OK)
rv6("get_version", {}, "VER;\r\n", device_reply="VER:V-600UHD,3.00;\n",
    expect_result={"ok": {"kind": "value", "value": "3.00"}})

telemetry(RV6, "version", inbound="VER:V-600UHD,3.00;\n",
          expect_state={"device": {"model": "V-600UHD", "version": "3.00"}})
