# Barco Event Master ASCII interface (barco-eventmaster-telnet): one vector per
# command. Lines are written from the command formats and examples of
# R5919184/00 chapter 1 ("ATRN 200", "ATRN 0", "ATRN", "PRESET -s 1",
# "PRESET -r 10", "PRESET -a 10"), ended with CR LF as a telnet client sends
# Enter. Barco documents no reply, so every write is unverified.
BT = "barco-eventmaster-telnet"
UT = {"ok": {"kind": "unverified"}}
text(BT, "auto_transition", {}, "ATRN\r\n", expect_result=UT)                      # guide example
text(BT, "auto_transition_time", {"frames": 200}, "ATRN 200\r\n", expect_result=UT)  # guide example
text(BT, "cut", {}, "ATRN 0\r\n", expect_result=UT)                                # guide example
text(BT, "save_preset", {"preset": 1}, "PRESET -s 1\r\n", expect_result=UT)        # guide example
text(BT, "recall_preset", {"preset": 10}, "PRESET -r 10\r\n", expect_result=UT)    # guide example
text(BT, "recall_preset_auto_transition", {"preset": 10}, "PRESET -a 10\r\n",
     expect_result=UT)                                                             # guide example
