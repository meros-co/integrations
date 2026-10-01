CQU = "chamsys-magicq-udp"
# ChamSys Remote Ethernet Protocol without the CREP header: one ASCII command
# per UDP datagram to port 6553, nothing appended (MagicQ Manual 1.9.9.x,
# ChamSys Remote Protocol Commands pp. 471-474 and Controlling MagicQ Using
# UDP/IP pp. 475-480; QuickQ Manual 12.0, Ethernet Remote Control p. 74).
# Expected strings are typed from those tables, using the manual's examples
# ("4,50I", "8,2,50J", "1A2A1S2G3,4I", "4,50L", "10A"), not taken from the spec.


def cqu(command, input, wire, **extra):
    text(CQU, command, input, wire, **extra)


# Several commands back to back in one datagram.
cqu("command", {"commands": "1A2A1S2G3,4I"}, "1A2A1S2G3,4I")
# <playback number> A/R/T/U/G/S/B/F
# QuickQ p. 74: "sending 10A will activate Playback 10".
cqu("activate", {"playback": 10}, "10A", expect_result={"ok": {"kind": "unverified"}})
cqu("release", {"playback": 1}, "1R")
cqu("test", {"playback": 2}, "2T")
cqu("untest", {"playback": 2}, "2U")
cqu("go", {"playback": 2}, "2G")
cqu("stop", {"playback": 1}, "1S")
cqu("fast_back", {"playback": 5}, "5B")
cqu("fast_forward", {"playback": 202}, "202F")
# QuickQ p. 74: "4,50L to set Playback 4 to 50%".
cqu("level", {"playback": 4, "level": 50}, "4,50L")
# "To jump to Cue id 2.5 on playback 8 you would use: 8,2,50J"
cqu("jump_cue", {"playback": 8, "cue": 2, "cue_dec": 50}, "8,2,50J")
cqu("page", {"page": 3}, "3P")
# "to set dimmer channel 4 to 50% you would use: 4,50I"
cqu("channel_intensity", {"channel": 4, "level": 50}, "4,50I")
cqu("tenscene_button", {"button": 5}, "5X")
cqu("tenscene_button_state", {"button": 5, "state": "4"}, "5,4X")
cqu("tenscene_zone_button_state", {"zone": 2, "button": 5, "state": "10"}, "2,5,10X")
# Remote programming commands: <number>, <params> H.
cqu("select_head", {"head": 1}, "01,1H")
cqu("select_head_range", {"start": 1, "end": 12}, "01,1,12H")
cqu("deselect_head", {"head": 3}, "02,3H")
cqu("deselect_head_range", {"start": 3, "end": 6}, "02,3,6H")
cqu("deselect_all", {}, "03H")
cqu("select_group", {"group": 200}, "04,200H")
cqu("set_intensity", {"level": 75}, "05,75H")
cqu("set_intensity_timed", {"level": 0, "time": 5}, "05,0,5H")
# Attribute numbers: Pan (4), Tilt (5), Cyan (16), Gobo1 (8), Zoom (13).
cqu("set_attribute", {"attribute": 4, "value": 128}, "06,4,128H")
cqu("set_attribute_timed", {"attribute": 5, "value": 64, "time": 3}, "06,5,64,3H")
cqu("increase_attribute", {"attribute": 16, "value": 10}, "07,16,10H")
cqu("increase_attribute_res", {"attribute": 4, "value": 256, "high_res": True}, "07,4,256,1H")
cqu("decrease_attribute", {"attribute": 8, "value": 1}, "08,8,1H")
cqu("decrease_attribute_res", {"attribute": 13, "value": 5, "high_res": False}, "08,13,5,0H")
cqu("clear_programmer", {}, "09H")
cqu("include_position_palette", {"palette": 1}, "10,1H")
cqu("include_colour_palette", {"palette": 2}, "11,2H")
cqu("include_beam_palette", {"palette": 1024}, "12,1024H")
cqu("include_cue", {"cue": 7}, "13,7H")
cqu("update", {}, "19H")
cqu("record_position_palette", {"palette": 5}, "20,5H")
cqu("record_colour_palette", {"palette": 6}, "21,6H")
cqu("record_beam_palette", {"palette": 7}, "22,7H")
cqu("record_cue", {"cue": 12}, "23,12H")
cqu("next_head", {}, "30H")
cqu("previous_head", {}, "31H")
cqu("all_heads", {}, "32H")
cqu("locate", {}, "40H")
cqu("lamp_on", {}, "41H")
cqu("lamp_off", {}, "42H")
cqu("reset_heads", {}, "43H")
cqu("remote_trigger", {"state": "2"}, "71,2H")
cqu("test_cue", {"cue": 10000}, "80,10000H")
cqu("untest_cue", {"cue": 1}, "81,1H")
cqu("test_cue_stack", {"stack": 3}, "82,3H")
cqu("test_cue_stack_level", {"stack": 3, "level": 50}, "82,3,50H")
cqu("test_cue_stack_level_cue", {"stack": 3, "level": 100, "cue": 4}, "82,3,100,4H")
cqu("untest_cue_stack", {"stack": 3}, "83,3H")
# <show file id>: four digit decimal number between 0000 and 9999.
cqu("save_show", {"show": 12}, "90,0012H")
cqu("load_show", {"show": 9999}, "91,9999H")
cqu("load_import", {"file": 1}, "92,0001H")
cqu("load_grid", {"grid": 0}, "93,0000H")
# \<94> , <shutdown type> , 81, 117 , 105, 116 H ; 3 is reboot.
cqu("shutdown", {"mode": "3"}, "94,3,81,117,105,116H")
cqu("emergency_hot_takeover", {"enabled": True}, "112,1H")

# QuickQ only (p. 74).
# <playback number>,<cue number>J
cqu("quickq_jump_cue", {"playback": 1, "cue": 5}, "1,5J")
# X: <button ID>,<state> (2 activate, 3 release, 4 toggle), and <zone>,<button ID>,<state>, zone 0 all.
cqu("quickq_tenscene_button_state", {"button": 3, "state": "2"}, "3,2X")
cqu("quickq_tenscene_zone_button_state", {"zone": 0, "button": 3, "state": "4"}, "0,3,4X")
