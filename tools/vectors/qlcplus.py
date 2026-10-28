QLC = "qlcplus"
# QLC+ Web API (QLC+ Documentation, Version 5, Advanced > Web Interface >
# Web API): pipe-separated text messages on ws://host:9999/qlcplusWS, typed
# here from the page's tables.


def qlc(command, input, message, **extra):
    text(QLC, command, input, message, **extra)


qlc("is_project_loaded", {}, "QLC+API|isProjectLoaded", device_reply="QLC+API|isProjectLoaded|true",
    expect_result={"ok": {"kind": "value", "value": "true"}})
qlc("get_functions_number", {}, "QLC+API|getFunctionsNumber", device_reply="QLC+API|getFunctionsNumber|12",
    expect_result={"ok": {"kind": "value", "value": "12"}})
qlc("get_functions_list", {}, "QLC+API|getFunctionsList", device_reply="QLC+API|getFunctionsList|0|Scene 1|1|Chaser|",
    expect_result={"ok": {"kind": "value", "value": "0|Scene 1|1|Chaser|"}})
qlc("get_function_type", {"function": 1}, "QLC+API|getFunctionType|1", device_reply="QLC+API|getFunctionType|Chaser",
    expect_result={"ok": {"kind": "value", "value": "Chaser"}})
qlc("get_function_status", {"function": 1}, "QLC+API|getFunctionStatus|1", device_reply="QLC+API|getFunctionStatus|Running",
    expect_result={"ok": {"kind": "value", "value": "Running"}})
qlc("set_function_status", {"function": 3, "running": True}, "QLC+API|setFunctionStatus|3|1",
    expect_result={"ok": {"kind": "unverified"}})
qlc("get_widgets_number", {}, "QLC+API|getWidgetsNumber")
qlc("get_widgets_list", {}, "QLC+API|getWidgetsList")
qlc("get_widget_type", {"widget": 4}, "QLC+API|getWidgetType|4")
qlc("get_widget_function", {"widget": 4}, "QLC+API|getWidgetFunction|4")
qlc("get_widget_status", {"widget": 4}, "QLC+API|getWidgetStatus|4", device_reply="QLC+API|getWidgetStatus|255",
    expect_result={"ok": {"kind": "value", "value": "255"}})
qlc("get_channels_values", {"universe": 1, "start": 1, "count": 16}, "QLC+API|getChannelsValues|1|1|16")
qlc("reset_channel", {"channel": 513}, "QLC+API|sdResetChannel|513")
qlc("reset_universe", {"universe": 2}, "QLC+API|sdResetUniverse|2")
qlc("set_widget_value", {"widget": 7, "value": 255}, "7|255")
qlc("set_slider_override", {"widget": 7, "enabled": True}, "7|SLIDER_OVERRIDE|1")
qlc("set_slider_cng_preset", {"widget": 7, "value": 32}, "7|CNG_PRESET|32")
qlc("set_slider_cng_colors", {"widget": 7, "primary": "#ff0000", "secondary": "#00ff00"}, "7|CNG_COLORS|#ff0000|#00ff00")
qlc("cue_list", {"widget": 9, "action": "next"}, "9|NEXT")
qlc("cue_list_step", {"widget": 9, "index": 3}, "9|STEP|3")
qlc("cue_list_side_fader", {"widget": 9, "level": 128}, "9|CUE_SIDECHANGE|128")
qlc("frame_page", {"widget": 2, "direction": "previous"}, "2|PREV_PG")
qlc("frame_go_to_page", {"widget": 2, "page": 1}, "2|PAGE|1")
qlc("set_frame_disabled", {"widget": 2, "disabled": False}, "2|FRAME_DISABLE|0")
qlc("set_frame_collapsed", {"widget": 2, "collapsed": True}, "2|COLLAPSE|1")
qlc("set_matrix_slider", {"widget": 11, "value": 200}, "11|MATRIX_SLIDER|200")
qlc("set_matrix_color", {"widget": 11, "index": 1, "color": "#00ff00"}, "11|MATRIX_COLOR_1|#00ff00")
qlc("set_matrix_combo", {"widget": 11, "index": 2}, "11|MATRIX_COMBO|2")
qlc("set_xy_pad", {"widget": 12, "x": 100, "y": 50}, "12|XYPAD|100|50")
qlc("set_xy_pad_range_h", {"widget": 12, "min": 0, "max": 255}, "12|XYPAD_RANGE_H|0|255")
qlc("set_xy_pad_range_v", {"widget": 12, "min": 10, "max": 20}, "12|XYPAD_RANGE_V|10|20")
qlc("set_xy_pad_preset", {"widget": 12, "preset": 1}, "12|XYPAD_PRESET|1")
qlc("speed_dial", {"widget": 13, "action": "apply"}, "13|SPEED_APPLY")
qlc("set_speed_time", {"widget": 13, "ms": 500}, "13|SPEED_TIME|500")
qlc("set_speed_factor", {"widget": 13, "factor": "1/4"}, "13|SPEED_FACTOR|1/4")
qlc("set_audio_capture", {"widget": 14, "enabled": True}, "14|1")
qlc("set_audio_volume", {"widget": 14, "volume": 80}, "14|AUDIO_VOLUME|80")
qlc("set_clock_running", {"widget": 15, "running": False}, "15|CLOCK_PLAY|0")
qlc("reset_clock", {"widget": 15}, "15|CLOCK_RESET")
qlc("set_channel", {"channel": 513, "value": 255}, "CH|513|255")
qlc("set_vc_page", {"page": 2}, "VC_PAGE|2")
qlc("set_grand_master", {"value": 128}, "GM_VALUE|128")
V.append({"spec": QLC, "command": "get_vc_json", "input": {},
          "expect_request": {"method": "GET", "target": "/vc.json", "port": 9999},
          "http_reply": {"status": 200, "body": '{"widgets":[]}'},
          "expect_result": {"ok": {"kind": "value", "value": {"widgets": []}}}})

telemetry(QLC, "project-loaded", inbound_ws="QLC+API|isProjectLoaded|false", expect_state={"project_loaded": False})
telemetry(QLC, "functions-number", inbound_ws="QLC+API|getFunctionsNumber|3", expect_state={"functions_count": 3})
telemetry(QLC, "widgets-number", inbound_ws="QLC+API|getWidgetsNumber|5", expect_state={"widgets_count": 5})
telemetry(QLC, "functions-list", inbound_ws="QLC+API|getFunctionsList|0|Scene 1|1|Chaser|",
          state_before={"functions": {"7": {"name": "Deleted"}}},
          expect_state={"functions": {"0": {"name": "Scene 1"}, "1": {"name": "Chaser"}}})
telemetry(QLC, "widgets-list", inbound_ws="QLC+API|getWidgetsList|4|Blackout|9|Cue List|",
          expect_state={"widgets": {"4": {"caption": "Blackout"}, "9": {"caption": "Cue List"}}})
# "QLC+ also pushes GM_VALUE|<value>|<display> when it changes."
telemetry(QLC, "grand-master", inbound_ws="GM_VALUE|128|50%",
          expect_state={"grand_master": {"value": 128, "display": "50%"}})
# Each list item has its details read in turn; their answers name nothing,
# so each is tied to the request it answers.
telemetry(QLC, "functions-list-reads", inbound_ws="QLC+API|getFunctionsList|0|Scene 1|",
          expect_then_send=["QLC+API|getFunctionType|0", "QLC+API|getFunctionStatus|0"],
          expect_state={"functions": {"0": {"name": "Scene 1"}}})
telemetry(QLC, "widgets-list-reads", inbound_ws="QLC+API|getWidgetsList|4|Blackout|",
          expect_then_send=["QLC+API|getWidgetType|4", "QLC+API|getWidgetFunction|4", "QLC+API|getWidgetStatus|4"],
          expect_state={"widgets": {"4": {"caption": "Blackout"}}})
telemetry(QLC, "function-type", inbound="QLC+API|getFunctionType|Chaser", request="QLC+API|getFunctionType|1",
          expect_state={"functions": {"1": {"type": "Chaser"}}})
telemetry(QLC, "function-status", inbound="QLC+API|getFunctionStatus|Running", request="QLC+API|getFunctionStatus|1",
          expect_state={"functions": {"1": {"status": "Running", "running": True}}})
telemetry(QLC, "function-stopped", inbound="QLC+API|getFunctionStatus|Stopped", request="QLC+API|getFunctionStatus|3",
          expect_state={"functions": {"3": {"status": "Stopped", "running": False}}})
telemetry(QLC, "widget-type", inbound="QLC+API|getWidgetType|Slider", request="QLC+API|getWidgetType|7",
          expect_state={"widgets": {"7": {"type": "Slider"}}})
telemetry(QLC, "widget-function", inbound="QLC+API|getWidgetFunction|2|Chaser|Intro", request="QLC+API|getWidgetFunction|7",
          expect_state={"widgets": {"7": {"function": "2|Chaser|Intro"}}})
telemetry(QLC, "widget-status", inbound="QLC+API|getWidgetStatus|PLAY|3", request="QLC+API|getWidgetStatus|9",
          expect_state={"widgets": {"9": {"status": "PLAY|3"}}})
