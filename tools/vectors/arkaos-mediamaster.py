# ArKaos MediaMaster (arkaos-mediamaster): the remote player's Telnet command
# texts, each a line ended by CR LF on TCP 45678, as the Companion module the
# MediaMaster 6 user guide points to sends them (Cue:12, Controller:Play,
# Fullscreen:On, Properties:Mixing, View:Remote Player). Nothing is answered.
AK = "arkaos-mediamaster"
U = {"ok": {"kind": "unverified"}}

text(AK, "run_cue", {"cue": 12}, "Cue:12\r\n", expect_result=U)
for cmd, wire in [("controller_play", "Controller:Play"), ("controller_pause", "Controller:Pause"),
                  ("controller_stop", "Controller:Stop"), ("controller_first", "Controller:First"),
                  ("controller_previous", "Controller:Previous"), ("controller_next", "Controller:Next"),
                  ("controller_last", "Controller:Last"), ("controller_final_15", "Controller:Final15"),
                  ("controller_final_30", "Controller:Final30"), ("orientation", "Orientation")]:
    text(AK, cmd, {}, wire + "\r\n", expect_result=U)
text(AK, "fullscreen", {"enabled": True}, "Fullscreen:On\r\n", expect_result=U)
V.append({"spec": AK, "command": "fullscreen", "file": "fullscreen.off", "input": {"enabled": False},
          "expect_wire": "Fullscreen:Off\r\n", "expect_result": U})
text(AK, "properties_tab", {"tab": "Mixing"}, "Properties:Mixing\r\n", expect_result=U)
text(AK, "view", {"view": "Remote Player"}, "View:Remote Player\r\n", expect_result=U)
text(AK, "send_command", {"command": "Cue:4095"}, "Cue:4095\r\n", expect_result=U)
