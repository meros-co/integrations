# DSAN Limitimer (dsan-limitimer) and PerfectCue (dsan-perfectcue): one
# vector per command and telemetry vectors for the LED and time messages.
# Each message is built here from DSAN's documented format, ">" + command +
# space + checksum + CR, the checksum computed independently as the low
# byte of the sum of the command and the space (DSAN's worked examples:
# PRG1 -> 3A, FORWARD -> 35); played through a serial-to-Ethernet gateway's
# TCP port.
def _dsan(command):
    return ">%s %02X\r" % (command, sum((command + " ").encode()) & 0xFF)


LIM = "dsan-limitimer"
for _name, _cmd in [("program_1", "PRG1"), ("program_2", "PRG2"), ("program_3", "PRG3"), ("session", "SESS"),
                    ("total_time_up", "TTUP"), ("total_time_down", "TTDN"), ("sum_up_time_up", "STUP"),
                    ("sum_up_time_down", "STDN"), ("set_seconds", "SSEC"), ("clear", "CLR"), ("beep", "BEEP"),
                    ("blink", "BLNK"), ("repeat", "REPT"), ("enable_signal_light_timer", "ENTIME"),
                    ("disable_signal_light_timer", "DISTIME")]:
    text(LIM, _name, {}, _dsan(_cmd))
text(LIM, "start_stop", {}, _dsan("STOP"))
assert _dsan("PRG1") == ">PRG1 3A\r"

telemetry(LIM, "program-leds",
          inbound=_dsan("P1LEDON") + _dsan("P2LEDDM") + _dsan("P3LEDOF") + _dsan("SESLEDDM"),
          expect_state={"leds": {"program_1": "on", "program_2": "dim", "program_3": "off", "session": "dim"}})
telemetry(LIM, "indicator-leds",
          inbound=_dsan("BPLEDON") + _dsan("BKLEDOF") + _dsan("GRNLEDON") + _dsan("YELLEDOF")
          + _dsan("REDLEDOF") + _dsan("SMON") + _dsan("BEEP"),
          expect_state={"leds": {"beep": True, "blink": False, "green": True, "yellow": False, "red": False,
                                 "seconds_mode": True}})
# ">TTSTR= 0:00 E8"
telemetry(LIM, "time-strings",
          inbound=_dsan("TTSTR= 5:00") + _dsan("STSTR= 1:00") + _dsan("RTSTR=-0:12") + _dsan("RTSTRSZ= 0:00"),
          expect_state={"times": {"total": "5:00", "sum_up": "1:00", "remaining": "-0:12", "remaining_held": "0:00"}})
assert _dsan("TTSTR= 0:00") == ">TTSTR= 0:00 E8\r"

PC = "dsan-perfectcue"
for _name, _cmd in [("forward", "FORWARD"), ("reverse", "REVERSE"), ("blank_on", "BLANKON")]:
    text(PC, _name, {}, _dsan(_cmd))
text(PC, "blank_off", {}, _dsan("BLANKOF"))
assert _dsan("FORWARD") == ">FORWARD 35\r"
telemetry(PC, "leds",
          inbound=_dsan("FRWLEDON") + _dsan("RVSLEDOF") + _dsan("BLKLEDON"),
          expect_state={"leds": {"forward": True, "reverse": False, "blank": True}})
