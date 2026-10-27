# Irisdown Countdown Timer (irisdown-countdown-timer): one vector per
# command and telemetry vectors for both UPDATES modes. The wire text is
# written by hand from "Countdown Timer v2.0.10.0 Remote Control Protocol":
# ASCII commands ended by an EOL (sent as CRLF), and the update lines' two
# documented forms, using the document's own examples.
ICT = "irisdown-countdown-timer"
text(ICT, "go", {}, "GO\r\n")
text(ICT, "pause", {}, "PAUSE\r\n")
text(ICT, "toggle_pause", {}, "TOGGLEPAUSE\r\n")
text(ICT, "reset", {}, "RESET\r\n")
# "RESET 00:05:30"
text(ICT, "reset_to", {"minutes": 5, "seconds": 30}, "RESET 00:05:30\r\n")
# "RESET 25"
text(ICT, "reset_minutes", {"minutes": 25}, "RESET 25\r\n")
# "JOG 5", "JOG -5"
text(ICT, "jog", {"minutes": -5}, "JOG -5\r\n")
text(ICT, "set_display", {"mode": "CLOCK"}, "DISPLAY CLOCK\r\n")
text(ICT, "show_message", {"text": "Lunch has been brought forward 15 minutes"},
     'MESSAGE "Lunch has been brought forward 15 minutes"\r\n')
text(ICT, "clear_message", {}, "MESSAGE CLEAR\r\n")

# "TIME=+00:10:00&STATE=STOPPED&DISPLAY=TIMER&MESSAGE=TRUE"
telemetry(ICT, "update-mode-2", inbound="TIME=+00:10:00&STATE=STOPPED&DISPLAY=TIMER&MESSAGE=TRUE\r\n",
          expect_connect_wire=["UPDATEMODE 2\r\n", "UPDATES ON\r\n"],
          expect_state={"timer": {"remaining": "00:10:00", "hours": 0, "minutes": 10, "seconds": 0,
                                  "overtime": False, "state": "STOPPED"},
                        "display": {"mode": "TIMER"}, "message": {"showing": True}})
telemetry(ICT, "update-mode-2-overtime", inbound="TIME=-00:01:30&STATE=PLAYING&DISPLAY=BLACK&MESSAGE=FALSE\r\n",
          expect_state={"timer": {"remaining": "00:01:30", "hours": 0, "minutes": 1, "seconds": 30,
                                  "overtime": True, "state": "PLAYING"},
                        "display": {"mode": "BLACK"}, "message": {"showing": False}})
# "00:10:00 STOPPED TIMER"
telemetry(ICT, "update-mode-1", inbound="00:10:00 STOPPED TIMER\r\n",
          expect_state={"timer": {"remaining": "00:10:00", "hours": 0, "minutes": 10, "seconds": 0,
                                  "state": "STOPPED"}, "display": {"mode": "TIMER"}})
telemetry(ICT, "ok-reply", inbound="OK\r\n", expect_state={})
