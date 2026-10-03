SC = "symetrix-composer"
# Symetrix Composer Control Protocol (Composer v7.0 Control Protocol):
# commands end in CR (p.7); answers ACK / NAK, values, strings; pushes and
# GSB2 lines "#nnnnn=vvvvv" (p.10, p.13). Faders -72 + 84 * position / 65535.
ACK = {"ok": {"kind": "ack"}}

text(SC, "set_controller", {"controller": 1, "position": 754}, "CS 1 754\r",
     device_reply="ACK\r", expect_result=ACK)
text(SC, "set_controller", {"controller": 9999, "position": 0}, "CS 9999 0\r",
     device_reply="NAK\r", expect_result={"error": {"error": "device_error"}})
text(SC, "set_controller_quick", {"controller": 140, "position": 65535}, "CSQ 140 65535\r",
     device_reply="ACK\r", expect_result=ACK)
text(SC, "increment_controller", {"controller": 7, "amount": 780}, "CC 7 1 780\r",
     device_reply="ACK\r", expect_result=ACK)
text(SC, "decrement_controller", {"controller": 7, "amount": 780}, "CC 7 0 780\r")
text(SC, "get_controller", {"controller": 368}, "GS2 368\r", device_reply="368 47114\r",
     expect_result={"ok": {"kind": "value", "value": "47114"}})
text(SC, "get_controller_value", {"controller": 1}, "GS 1\r", device_reply="0\r",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(SC, "read_controller_block", {"controller": 9, "count": 3}, "GSB2 9 3\r",
     expect_result={"ok": {"kind": "unverified"}})
# -8 dB: (-8 + 72) * 65535 / 84 = 49931.4 (p.5's example).
text(SC, "set_fader_db", {"controller": 12, "level_db": -8.0}, "CS 12 49931\r",
     device_reply="ACK\r", expect_result=ACK)
text(SC, "set_fader_db", {"controller": 12, "level_db": 12.0}, "CS 12 65535\r")
text(SC, "get_fader_db", {"controller": 12}, "GS 12\r", device_reply="0\r",
     expect_result={"ok": {"kind": "value", "value": -72.0}})
text(SC, "button_on", {"controller": 140}, "CS 140 65535\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "button_off", {"controller": 140}, "CS 140 0\r")
text(SC, "load_preset", {"preset": 7}, "LP 7\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "get_preset", {}, "GPR\r", device_reply="0007\r",
     expect_result={"ok": {"kind": "value", "value": "7"}})
text(SC, "flash_unit", {}, "FU 8\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "stop_flashing", {}, "FU 0\r")
text(SC, "nop", {}, "NOP\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "get_ip", {}, "RI\r", device_reply="192.168.100.150\r",
     expect_result={"ok": {"kind": "value", "value": "192.168.100.150"}})
text(SC, "reboot", {}, "R!\r", expect_result={"ok": {"kind": "unverified"}})
# System strings (p.11-12).
text(SC, "set_system_string", {"unit": 1, "resource": "1001", "entry": 2, "card": 0, "value": "Acme Inc."},
     "SSYSS 1.1001.2.0.0=Acme Inc.\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "get_system_string", {"unit": 1, "resource": "1001", "entry": 11, "card": 3},
     "GSYSS 1.1001.11.3.0\r", device_reply="Conference Room\r",
     expect_result={"ok": {"kind": "value", "value": "Conference Room"}})
text(SC, "set_dial_number", {"unit": 1, "card": 3, "number": "14257787728"},
     "SSYSS 1.1004.0.3.0=14257787728\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "set_speed_dial_number", {"unit": 1, "card": 3, "entry": 11, "number": "555-1234"},
     "SSYSS 1.1000.11.3.0=555-1234\r")
text(SC, "set_speed_dial_name", {"unit": 1, "card": 3, "entry": 11, "name": "Conference Rm 1"},
     "SSYSS 1.1001.11.3.0=Conference Rm 1\r")
text(SC, "get_speed_dial_number", {"unit": 1, "card": 3, "entry": 0}, "GSYSS 1.1000.0.3.0\r")
text(SC, "get_speed_dial_name", {"unit": 1, "card": 3, "entry": 11}, "GSYSS 1.1001.11.3.0\r")
text(SC, "get_dialed_number", {"unit": 1, "card": 3}, "GSYSS 1.1002.0.3.0\r")
text(SC, "get_caller_id", {"unit": 2, "card": 3, "channel": 1}, "GSYSS 2.1003.0.3.1\r",
     device_reply="NAK\r", expect_result={"error": {"error": "device_error"}})
text(SC, "get_call_status", {"unit": 1, "card": 4 - 1}, "GSYSS 1.1005.0.3.0\r",
     device_reply="In Call: 710\r", expect_result={"ok": {"kind": "value", "value": "In Call: 710"}})
text(SC, "get_call_elapsed", {"unit": 1, "card": 3}, "GSYSS 1.1006.0.3.0\r",
     device_reply="Time 0:00:35\r", expect_result={"ok": {"kind": "value", "value": "Time 0:00:35"}})
# Push (p.15-18).
text(SC, "set_push_global", {"enabled": False}, "PU 0\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "push_enable", {}, "PUE\r")
text(SC, "push_enable_range", {"low": 100, "high": 200}, "PUE 100 200\r")
text(SC, "push_disable", {}, "PUD\r")
text(SC, "push_disable_range", {"low": 5, "high": 5}, "PUD 5 5\r")
text(SC, "push_refresh", {}, "PUR\r")
text(SC, "push_refresh_range", {"low": 1, "high": 64}, "PUR 1 64\r")
text(SC, "push_clear", {}, "PUC\r")
text(SC, "set_push_interval", {"interval_ms": 250}, "PUI 250\r")
text(SC, "set_push_threshold", {"parameter_threshold": 1, "meter_threshold": 1000}, "PUT 1 1000\r")
# Super Matrix Mixer (p.21-24).
text(SC, "matrix_set", {"feature": "CPGain", "enumerator": "I3O6", "value": 4.1},
     "CMV Set 0.1.CPGain.I3O6 4.10\r", device_reply="ACK\r", expect_result=ACK)
text(SC, "matrix_set", {"feature": "CPConnect", "enumerator": "{I1O1:I3O20}", "value": 1},
     "CMV Set 0.1.CPConnect.{I1O1:I3O20} 1.00\r", device_reply="NAK Enumerator\r",
     expect_result={"error": {"error": "device_error"}})
text(SC, "matrix_get", {"feature": "CPConnect", "enumerator": "I13O76"}, "CMV Get 0.1.CPConnect.I13O76\r",
     device_reply="1\r", expect_result={"ok": {"kind": "value", "value": "1"}})
text(SC, "matrix_get", {"unit": 2, "module": 1, "feature": "OGain", "enumerator": "O1"},
     "CMV Get 2.1.OGain.O1\r", device_reply="-4.50\r", expect_result={"ok": {"kind": "value", "value": "-4.50"}})
text(SC, "matrix_modify", {"feature": "CPDelay", "enumerator": "I1O1", "offset": -3.7},
     "CMV Modify 0.1.CPDelay.I1O1 -3.70\r")
text(SC, "matrix_toggle", {"feature": "OMute", "enumerator": "O2"}, "CMV Toggle 0.1.OMute.O2 1\r")
text(SC, "matrix_reset", {"feature": "OGain", "enumerator": "{O5:O31}"}, "CMV Reset 0.1.OGain.{O5:O31}\r")
text(SC, "get_matrix_change_count", {}, "GSYSC 0.3060.-1,0\r", device_reply="42\r",
     expect_result={"ok": {"kind": "value", "value": "42"}})

telemetry(SC, "push", inbound="#00007=12321\r#00324=00128\r#10000=65535\r",
          expect_connect_wire=["EH 0\r"],
          expect_state={"controllers": {"7": {"value": 12321}, "324": {"value": 128}, "10000": {"value": 65535}}})
telemetry(SC, "missing-controller", inbound="#00011=-0001\r",
          expect_state={"controllers": {"11": {"value": -1}}})
telemetry(SC, "gs2-answer", inbound="368 47114\r",
          expect_state={"controllers": {"368": {"value": 47114}}})
