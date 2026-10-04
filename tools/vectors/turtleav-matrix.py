# Turtle AV 4x4 / 8x8 4K60 video wall, matrix and multiviewer: lower-case
# commands ended by "!" (the documents' delimiter), sent as lines with CR LF
# (4x4 manual §13, 8x8 manual §10). Wire forms follow the documents'
# examples: "s output 1 in source 2!", "r output 1 in source!", "s recall
# preset 1!", "recall mx preset 1!", "s cec in 1 on!", "s reboot!".
TM = "turtleav-matrix"
M8 = {"model": "mx88vw"}


def tm(command, input, line, **extra):
    text(TM, command, input, line + "\r\n", **extra)


tm("set_power", {"powered": True}, "s power 1!", device_reply="power on\r\n", expect_result={"ok": {"kind": "ack"}})
tm("route", {"output": 1, "input": 2}, "s output 1 in source 2!",
   device_reply="output1->input2\r\n", expect_result={"ok": {"kind": "ack"}})
tm("route_8x8", {"output": 8, "input": 5}, "s output 8 in source 5!",
   device_reply="E01\r\n", expect_result={"error": {"error": "device_error"}}, **M8)
tm("get_route", {"output": 1}, "r output 1 in source!",
   device_reply="output1->input3\r\n", expect_result={"ok": {"kind": "value", "value": "3"}})
tm("recall_preset", {"preset": 1}, "s recall preset 1!")
tm("save_preset", {"preset": 8}, "s save preset 8!")
tm("recall_matrix_preset", {"preset": 2}, "recall mx preset 2!", **M8)
tm("save_matrix_preset", {"preset": 2}, "save mx preset 2!", **M8)
tm("recall_wall_preset", {"preset": 3}, "recall vw preset 3!", **M8)
tm("set_display_mode", {"mode": 2}, "s display mode 2!")
tm("set_display_mode_8x8", {"mode": 1}, "s display mode 1!", **M8)
tm("set_output_resolution", {"output": 1, "code": 3}, "s output 1 res 3!")
tm("set_output_resolution_8x8", {"output": 0, "code": 24}, "s output 0 res 24!", **M8)
tm("set_output_hdcp", {"output": 1, "mode": 3}, "s output 1 hdcp 3!")
tm("set_output_hdcp_8x8", {"output": 1, "mode": 5}, "s output 1 hdcp 5!", **M8)
tm("set_input_edid", {"input": 1, "code": 1}, "s input 1 edid 1!")
tm("set_input_edid_8x8", {"input": 8, "code": 22}, "s input 8 edid 22!", **M8)
tm("set_multiview_layout", {"layout": 5}, "s multiview 5!")
tm("set_window_source", {"window": 1, "input": 4}, "s window 1 in 4!")
tm("set_wall_layout", {"mode": 1}, "s tw mode 1!")
tm("set_wall_group_input", {"group": 1, "input": 2}, "s tw group 1 input 2!")
tm("set_wall_group_source_8x8", {"group": 1, "input": 6}, "s vw group 1 source 6!", **M8)
tm("route_ext_audio", {"output": 1, "input": 1}, "s output 1 exa in source 1!")
tm("cec_input", {"input": 1, "action": "on"}, "s cec in 1 on!")
tm("cec_output", {"output": 2, "action": "vol+"}, "s cec hdmi out 2 vol+!")
tm("set_panel_lock", {"locked": True}, "s lock 1!")
tm("reboot", {}, "s reboot!")

telemetry(TM, "route", inbound="output1->input1\r\n", expect_state={"outputs": {"1": {"input": 1}}})
telemetry(TM, "power", inbound="power on\r\n", expect_state={"power": True})
