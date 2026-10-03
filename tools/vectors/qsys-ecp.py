Q = "qsys-ecp"
# Q-SYS External Control Protocol (Q-SYS Help, ECP Commands): ASCII lines
# ending in LF; strings with spaces in double quotes, " and \ escaped.
QACK = {"ok": {"kind": "ack"}}
QNONE = {"ok": {"kind": "unverified"}}

text(Q, "status_get", {}, "sg\n", device_reply='sr "Room 1" "a1b2c3" 1 1\r\n',
     expect_result={"ok": {"kind": "value", "value": 'sr "Room 1" "a1b2c3" 1 1'}})
text(Q, "control_get", {"name": "gain1"}, 'cg "gain1"\n',
     device_reply='cv "gain1" "-12.0dB" -12 0.75\r\n',
     expect_result={"ok": {"kind": "value", "value": 'cv "gain1" "-12.0dB" -12 0.75'}})
text(Q, "control_get", {"name": "Room Mute"}, 'cg "Room Mute"\n',
     device_reply='bad_id "Room Mute"\r\n', expect_result={"error": {"error": "device_error"}})
text(Q, "control_get_metadata", {"name": "gain1"}, 'cgm "gain1"\n', expect_result=QNONE)
text(Q, "control_set_value", {"name": "gain1", "value": -6.5}, 'csv "gain1" -6.500000\n',
     device_reply='cv "gain1" "-6.5dB" -6.5 0.8125\r\n', expect_result=QACK)
text(Q, "control_set_value_ramp", {"name": "gain1", "value": -20.0, "ramp": 2.5},
     'csvr "gain1" -20.000000 2.500\n', expect_result=QNONE)
text(Q, "control_set_position", {"name": "gain1", "position": 0.5}, 'csp "gain1" 0.500000\n',
     device_reply='cv "gain1" "-20.0dB" -20 0.5\r\n', expect_result=QACK)
text(Q, "control_set_position_ramp", {"name": "gain1", "position": 1.0, "ramp": 0.5},
     'cspr "gain1" 1.000000 0.500\n')
text(Q, "control_set_string", {"name": "text1", "text": "Say \"hi\""}, 'css "text1" "Say \\"hi\\""\n',
     device_reply='cv "text1" "Say \\"hi\\"" 0 0\r\n', expect_result=QACK)
text(Q, "control_set_string", {"name": "mute1", "text": "muted"}, 'css "mute1" "muted"\n',
     device_reply='control_read_only "mute1"\r\n', expect_result={"error": {"error": "device_error"}})
text(Q, "control_set_value_vector", {"name": "eq1", "count": 3, "values": "1 -2.5 3"},
     'csvv "eq1" 3 1 -2.5 3\n', expect_result=QNONE)
text(Q, "control_set_value_vector_ramp", {"name": "eq1", "count": 2, "values": "1 2", "ramp": 1.0},
     'csvvr "eq1" 2 1 2 1.000\n')
text(Q, "control_set_position_vector", {"name": "eq1", "count": 2, "positions": "0.5 0.25"},
     'cspv "eq1" 2 0.5 0.25\n')
text(Q, "control_set_position_vector_ramp", {"name": "eq1", "count": 1, "positions": "1", "ramp": 0.25},
     'cspvr "eq1" 1 1 0.250\n')
text(Q, "control_set_string_vector", {"name": "names", "count": 2, "strings": '"a b" "c"'},
     'cssv "names" 2 "a b" "c"\n')
text(Q, "control_trigger", {"name": "play"}, 'ct "play"\n', expect_result=QNONE)
text(Q, "change_group_create", {}, "cgc 1\n", expect_result=QNONE)
text(Q, "change_group_add", {"name": "gain1"}, 'cga 1 "gain1"\n')
text(Q, "change_group_add", {"group": 2, "name": "Room Mute"}, 'cga 2 "Room Mute"\n')
text(Q, "change_group_remove", {"name": "gain1"}, 'cgr 1 "gain1"\n')
text(Q, "change_group_clear", {"group": 2}, "cgclr 2\n")
text(Q, "change_group_destroy", {"group": 2}, "cgd 2\n")
text(Q, "change_group_invalidate", {}, "cgi 1\n")
text(Q, "change_group_poll", {"group": 2}, "cgp 2\n", device_reply="cgpa\r\n", expect_result=QACK)
text(Q, "change_group_poll_no_ack", {}, "cgpna 1\n")
text(Q, "change_group_schedule", {"group": 2, "period_ms": 100}, "cgs 2 100\n")
text(Q, "change_group_schedule_no_ack", {"period_ms": 0}, "cgsna 1 0\n")
text(Q, "snapshot_load", {"bank": "Room Presets", "number": 2, "ramp": 1.5}, 'ssl "Room Presets" 2 1.500\n',
     expect_result=QNONE)
text(Q, "snapshot_load", {"bank": "Lights", "number": 1}, 'ssl "Lights" 1 0.000\n')
text(Q, "snapshot_save", {"bank": "Room Presets", "number": 2}, 'sss "Room Presets" 2\n')
text(Q, "command", {"line": "cg gain1"}, "cg gain1\n", expect_result=QNONE)

telemetry(Q, "control-value", inbound='cv "gain1" "-12.0dB" -12 0.75\r\n',
          expect_state={"controls": {"gain1": {"string": "-12.0dB", "value": -12.0, "position": 0.75}}})
telemetry(Q, "vector-control", inbound='cvv "meter1" 2 "-20.5dB" "-30.0dB" 2 -20.5 -30 2 0.4 0.3\r\n',
          expect_state={"controls": {"meter1": {"string": "-20.5dB", "value": -20.5}}})
telemetry(Q, "metadata", inbound='cmv "mute1" 6 "true" 1 1\r\n',
          expect_state={"controls": {"mute1": {"disabled": True}}})
telemetry(Q, "status", inbound='sr "Room 1" "a1b2c3" 1 0\r\n',
          expect_state={"core": {"design_name": "Room 1", "design_id": "a1b2c3", "primary": True, "active": False}})
telemetry(Q, "error", inbound='bad_id "gain9"\r\n',
          expect_state={"errors": {"last": {"kind": "bad_id", "detail": '"gain9"'}}})
