SJ = "symetrix-jupiter"
# Symetrix Jupiter control protocol: one command per UDP datagram to 48630,
# ending in CR; answers to the sender; pushes "#nnnnn=vvvvv".
JACK = {"ok": {"kind": "ack"}}

text(SJ, "set_controller", {"controller": 1, "position": 754}, "CS 1 754\r",
     device_reply_hex=b"ACK\r".hex(), expect_result=JACK)
text(SJ, "increment_controller", {"controller": 7, "amount": 10}, "CC 7 1 10\r")
text(SJ, "decrement_controller", {"controller": 7, "amount": 10}, "CC 7 0 10\r")
text(SJ, "get_controller", {"controller": 1}, "GS 1\r", device_reply_hex=b"65535\r".hex(),
     expect_result={"ok": {"kind": "value", "value": "65535"}})
text(SJ, "read_controller_block", {"controller": 9, "count": 3}, "GSB2 9 3\r",
     expect_result={"ok": {"kind": "unverified"}})
text(SJ, "set_fader_db", {"controller": 3, "level_db": -72.0}, "CS 3 0\r")
text(SJ, "get_fader_db", {"controller": 3}, "GS 3\r", device_reply_hex=b"65535\r".hex(),
     expect_result={"ok": {"kind": "value", "value": 12.0}})
text(SJ, "button_on", {"controller": 20}, "CS 20 65535\r")
text(SJ, "button_off", {"controller": 20}, "CS 20 0\r")
text(SJ, "load_preset", {"preset": 50}, "LP 50\r", device_reply_hex=b"NAK\r".hex(),
     expect_result={"error": {"error": "device_error"}})
text(SJ, "get_preset", {}, "GPR D\r", device_reply_hex=b"PrstD=0007\r".hex(),
     expect_result={"ok": {"kind": "value", "value": "7"}})
text(SJ, "flash_unit", {}, "FU\r")
text(SJ, "set_push_global", {"enabled": True}, "PU 1\r")
text(SJ, "push_enable", {}, "PUE\r")
text(SJ, "push_enable_range", {"low": 1, "high": 100}, "PUE 1 100\r")
text(SJ, "push_disable", {}, "PUD\r")
text(SJ, "push_disable_range", {"low": 1, "high": 2}, "PUD 1 2\r")
text(SJ, "push_refresh", {}, "PUR\r")
text(SJ, "push_refresh_range", {"low": 1, "high": 2}, "PUR 1 2\r")
text(SJ, "push_clear", {}, "PUC\r")
text(SJ, "set_push_interval", {"interval_ms": 20}, "PUI 20\r")
text(SJ, "set_push_threshold", {"parameter_threshold": 0, "meter_threshold": 500}, "PUT 0 500\r")

telemetry(SJ, "push", inbound_hex=b"#00009=32321\r".hex(),
          expect_state={"controllers": {"9": {"value": 32321}}})
