# Barco Encore (barco-encore), the original Encore presentation system: one
# vector per command. Lines are written from what the MIT-licensed Bitfocus
# Companion module sends ("ATRN " or "PRESET R " plus a number and a line
# feed); Barco publishes no syntax and no reply, so every write is unverified.
BE = "barco-encore"
UE = {"ok": {"kind": "unverified"}}
text(BE, "auto_transition_time", {"duration": 30}, "ATRN 30\n", expect_result=UE)
text(BE, "recall_preset", {"preset": 12}, "PRESET R 12\n", expect_result=UE)
