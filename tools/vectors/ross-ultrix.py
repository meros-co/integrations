RU = "ross-ultrix"
# Ross Ultrix RossTalk (Ultrix User Guide 2101DR-004-28 Table 31; RossTalk
# Commands 4802DR-403-35 Table 5): ASCII command + CR LF on TCP 7788, never
# acknowledged. Expected lines are written from the guides' syntax and
# examples (GPI 23, TIMER 4:END, XPT D:5 S:16 I:7 L:1,6,10-13,
# TXTLABEL ID:25; TEXT:CAM1; BGCLR:...; TXTCLR:...).
RU_U = {"ok": {"kind": "unverified"}}

text(RU, "fire_salvo", {"salvo": 23}, "GPI 23\r\n", expect_result=RU_U)
text(RU, "timer_run", {"timer": 3}, "TIMER 3:RUN\r\n")
text(RU, "timer_stop", {"timer": 2}, "TIMER 2:STOP\r\n")
text(RU, "timer_pause", {"timer": 1}, "TIMER 1:PAUSE\r\n")
text(RU, "timer_end", {"timer": 4}, "TIMER 4:END\r\n")
text(RU, "set_timer", {"timer": 2, "hours": 0, "minutes": 5, "seconds": 30, "tenths": 5},
     "TIMER 2:SET:00:05:30.5\r\n")
text(RU, "take", {"destination": 2, "source": 1, "user": 7}, "XPT D:2 S:1 I:7\r\n")
text(RU, "take_levels", {"destination": 5, "source": 16, "user": 7, "levels": "1,6,10-13"},
     "XPT D:5 S:16 I:7 L:1,6,10-13\r\n", expect_result=RU_U)
text(RU, "set_text_label", {"id": 25, "text": "CAM1"}, "TXTLABEL ID:25; TEXT:CAM1;\r\n")
text(RU, "set_text_label_colors",
     {"id": 25, "text": "CAM1", "background_red": 0, "background_green": 0, "background_blue": 0,
      "text_red": 255, "text_green": 255, "text_blue": 255},
     "TXTLABEL ID:25; TEXT:CAM1; BGCLR:0,0,0; TXTCLR:255,255,255\r\n")
