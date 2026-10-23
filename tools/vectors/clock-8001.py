# Clock-8001 (clock-8001): one vector per command and telemetry vectors for
# the state feedback, written by hand from the project's "OSC API commands"
# (v4/osc.md) and encoded with this script's own OSC encoder. The feedback
# arrives as one OSC bundle; booleans are OSC's T and F type tags.
C8 = "clock-8001"


def _bundle(*messages):
    out = b"#bundle\0" + struct.pack(">II", 0, 1)
    for m in messages:
        out += struct.pack(">i", len(m)) + m
    return out


binary(C8, "timer_countdown", {"timer": 1, "seconds": 300}, osc("/clock/timer/1/countdown", ("i", 300)))
binary(C8, "timer_countdown_to", {"timer": 2, "time": "18:30:00"},
       osc("/clock/timer/2/countdown/target", ("s", "18:30:00")))
binary(C8, "timer_countup", {"timer": 0}, osc("/clock/timer/0/countup"))
binary(C8, "timer_countup_from", {"timer": 3, "time": "09:00:00"}, osc("/clock/timer/3/countup/target", ("s", "09:00:00")))
binary(C8, "timer_modify", {"timer": 1, "seconds": -60}, osc("/clock/timer/1/modify", ("i", -60)))
binary(C8, "timer_pause", {"timer": 1}, osc("/clock/timer/1/pause"))
binary(C8, "timer_resume", {"timer": 1}, osc("/clock/timer/1/resume"))
binary(C8, "timer_restart", {"timer": 9}, osc("/clock/timer/9/restart"))
binary(C8, "timer_set", {"timer": 4, "seconds": 125}, osc("/clock/timer/4/set", ("i", 125)))
binary(C8, "timer_stop", {"timer": 1}, osc("/clock/timer/1/stop"))
binary(C8, "timer_signal", {"timer": 1, "red": 255, "green": 0, "blue": 0},
       osc("/clock/timer/1/signal", ("i", 255), ("i", 0), ("i", 0), ("i", 255)))
binary(C8, "pause_all", {}, osc("/clock/pause"))
binary(C8, "resume_all", {}, osc("/clock/resume"))
binary(C8, "source_hide", {"source": 2}, osc("/clock/source/2/hide"))
binary(C8, "source_show", {"source": 2}, osc("/clock/source/2/show"))
binary(C8, "source_title", {"source": 1, "title": "Studio A"}, osc("/clock/source/1/title", ("s", "Studio A")))
binary(C8, "source_colors", {"source": 1, "text_red": 255, "text_green": 255, "text_blue": 255,
                             "back_red": 0, "back_green": 0, "back_blue": 0, "back_alpha": 128},
       osc("/clock/source/1/colors", ("i", 255), ("i", 255), ("i", 255), ("i", 255),
           ("i", 0), ("i", 0), ("i", 0), ("i", 128)))
binary(C8, "hide_all", {}, osc("/clock/hide"))
binary(C8, "show_all", {}, osc("/clock/show"))
binary(C8, "show_info", {"seconds": 10}, osc("/clock/info", ("i", 10)))
binary(C8, "set_background", {"number": 1}, osc("/clock/background", ("i", 1)))
binary(C8, "show_text", {"text_red": 255, "text_green": 255, "text_blue": 0, "back_red": 0, "back_green": 0,
                         "back_blue": 0, "seconds": 30, "text": "Wrap up"},
       osc("/clock/text", ("i", 255), ("i", 255), ("i", 0), ("i", 255), ("i", 0), ("i", 0), ("i", 0), ("i", 255),
           ("i", 30), ("s", "Wrap up")))
binary(C8, "title_colors", {"text_red": 0, "text_green": 0, "text_blue": 0, "back_red": 255, "back_green": 255,
                            "back_blue": 255},
       osc("/clock/titlecolors", ("i", 0), ("i", 0), ("i", 0), ("i", 255), ("i", 255), ("i", 255), ("i", 255),
           ("i", 255)))
binary(C8, "seconds_on", {}, osc("/clock/seconds/on"))
binary(C8, "seconds_off", {}, osc("/clock/seconds/off"))
# "/clock/time/set ... format 01:02:03"
binary(C8, "set_time", {"time": "01:02:03"}, osc("/clock/time/set", ("s", "01:02:03")))
binary(C8, "flash", {}, osc("/clock/flash"))
binary(C8, "set_signal", {"group": 1, "red": 0, "green": 255, "blue": 0},
       osc("/clock/signal/1", ("i", 0), ("i", 255), ("i", 0)))
binary(C8, "set_automation", {"enabled": True}, osc("/clock/automation", ("T", None)))
binary(C8, "cue_right", {}, osc("/clock/cue/right", ("s", "")))
binary(C8, "cue_left", {}, osc("/clock/cue/left", ("s", "")))
binary(C8, "cue_blank", {"blank": False}, osc("/clock/cue/blank", ("s", ""), ("F", None)))
binary(C8, "relay_ltc", {"input": "ltc2", "timecode": "10:00:00:12"}, osc("/clock/ltc2", ("s", "10:00:00:12")))

_UUID = "3f1a2b4c-5d6e-4f70-8192-a3b4c5d6e7f8"
telemetry(C8, "state-bundle", inbound_hex=hexs(_bundle(
    osc("/clock/source/1/state", ("s", _UUID), ("F", None), ("s", "12:34:56"), ("s", "12:34"), ("s", "clock"),
        ("f", 0.5), ("F", None), ("F", None), ("s", "Time of day"), ("i", 1)),
    osc("/clock/timer/2/state", ("s", _UUID), ("T", None), ("s", "00:04:30"), ("s", "4:30"), ("s", "countdown"),
        ("f", 0.25), ("F", None), ("T", None)))),
    expect_state={
        "sources": {"1": {"uuid": _UUID, "hidden": False, "output": "12:34:56", "compact": "12:34", "icon": "clock",
                          "progress": 0.5, "expired": False, "paused": False, "title": "Time of day", "mode": 1}},
        "timers": {"2": {"uuid": _UUID, "active": True, "output": "00:04:30", "compact": "4:30", "icon": "countdown",
                         "progress": 0.25, "expired": False, "paused": True}}})
