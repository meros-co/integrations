# EEG / AI-Media caption encoders (eeg-smart-encoder): one vector per
# command. Each command is written by hand from the HD492 and HD1492 manuals'
# command tables: CTRL+A (01), the command letter and arguments separated by
# spaces, then CR (0D); caption text ends with CR, and a mode ends with
# CTRL+C (03) alone. The manuals' own examples are used where they give
# them (P 103 -1 {Evening News}, P 105 00.30.00 4844). Nothing is
# acknowledged, so every result is unverified.
EG = "eeg-smart-encoder"
_U = {"expect_result": {"ok": {"kind": "unverified"}}}


def _eg(command, input, wire, **extra):
    text(EG, command, input, wire, **{**_U, **extra})


_eg("set_vanc_regeneration", {"enabled": False}, "\x01! OFF\r")
_eg("ignore_line21_channel", {"channel": "CC3"}, "\x016 CC3\r")
_eg("return_line21_channel", {"channel": "T1"}, "\x017 T1\r")
_eg("start_passthru", {}, "\x013 F1\r")
_eg("start_realtime", {"channel": "CC1", "rollup": 3, "base": 15}, "\x012 CC1 3 b15\r")
_eg("send_caption_line", {"text": "GOOD EVENING AND WELCOME"}, "GOOD EVENING AND WELCOME\r")
_eg("end_mode", {}, "\x03")
_eg("set_vanc_insertion", {"enabled": True}, "\x01# ON\r")
_eg("set_vanc_line", {"line": 12}, "\x01f 334 12\r")
_eg("set_vanc_line_no333", {"line": 12}, "\x01f vanc 12\r")
_eg("use_serial_output_333", {}, "\x01f 333\r")
_eg("use_serial_output_ga", {}, "\x01f ga\r")
_eg("enable_xds_entry", {}, "\x01O XDS O\r")
_eg("set_xds_active_port", {"port": "P2"}, "\x01O P2 XDS O\r")
_eg("load_xds_text", {"packet": "103", "content": "Evening News"}, "\x01P 103 -1 {Evening News}\r")
_eg("load_xds_text_priority", {"packet": "0501", "duration": "-1", "content": "EEG", "priority": 30},
    "\x01P 0501 -1 {EEG} 30\r")
_eg("load_xds_hex", {"packet": "105", "duration": "00.30.00", "hex": "4844"}, "\x01P 105 00.30.00 4844\r")
_eg("load_default_xds_text", {"packet": "0502", "duration": "-1", "content": "WEEG"}, "\x01P L0502 -1 {WEEG}\r")
_eg("store_xds_text", {"packet": "0501", "content": "EEG"}, "\x01w \x01P 0501 -1 {EEG}\r")
_eg("delete_xds_packet", {"packet": "103"}, "\x01P 103\r")
_eg("delete_all_xds_packets", {}, "\x01L -all\r")
_eg("block_xds_class", {"class": "01"}, "\x01T -0100\r")
_eg("unblock_xds_class", {"class": "01"}, "\x01T 0100\r")
_eg("set_serial_port_p2", {"baud": "9600", "bits": "8", "parity": "n"}, "\x01I P2 9600 8 n\r")
