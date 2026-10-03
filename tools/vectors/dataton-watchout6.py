# Dataton WATCHOUT 6 control protocol (dataton-watchout6): one vector per
# command. Lines are the syntax of the WATCHOUT 6.6 User's Guide, Appendix C
# (gotoTime 5000, setInput "uno" +0.1, setInputs 100 "Yo" 0.8 "Man" 0.5,
# standBy true 1000, hitTest 1200 250, load "C:/Samples/ExampleShow.watch" 3
# false), each with the command ID tag [m] of p.187 and CRLF. Replies are the
# guide's examples with the same tag ([23]Ready "2.0" "WATCHPOINT" "Windows"
# true, Reply true "Image01" 1154 212 0), WATCHOUT 7's empty tagged reply for
# a successful action, and the Error format of p.185.
WO6 = "dataton-watchout6"
_A = {"device_reply": "[m]\r\n", "expect_result": {"ok": {"kind": "ack"}}}


def _wo6(command, input, line, **extra):
    text(WO6, command, input, line + "\r\n", **extra)


def _wo6_err(reply):
    return {"device_reply": reply + "\r\n", "expect_result": {"error": {"error": "device_error"}}}


_wo6("ping", {}, "[m]ping", device_reply='[m]Ready "2.0" "WATCHPOINT" "Windows" true\r\n',
     expect_result={"ok": {"kind": "value", "value": '"2.0" "WATCHPOINT" "Windows" true'}})
_wo6("get_status", {}, "[m]getStatus", device_reply='[m]Reply "WO2Launch" false 0 true true false 122 true\r\n',
     expect_result={"ok": {"kind": "value", "value": '"WO2Launch" false 0 true true false 122 true'}})
_wo6("run", {}, "[m]run", **_A)
_wo6("halt", {}, "[m]halt", **_A)
_wo6("run_timeline", {"timeline": "Intro loop"}, '[m]run "Intro loop"', **_A)
_wo6("halt_timeline", {"timeline": "Intro loop"}, '[m]halt "Intro loop"')
_wo6("kill_timeline", {"timeline": "Intro loop"}, '[m]kill "Intro loop"',
     **_wo6_err('[m]Error 7 0 "Timeline not found"'))
_wo6("goto_time", {"time_ms": 5000}, "[m]gotoTime 5000", **_A)
_wo6("goto_time_timecode", {"time": "00:01:30.500"}, '[m]gotoTime "00:01:30.500"')
_wo6("goto_time_timeline", {"time_ms": 0, "timeline": "Intro loop"}, '[m]gotoTime 0 "Intro loop"')
_wo6("goto_control_cue", {"cue": "Act 2"}, '[m]gotoControlCue "Act 2" false', **_A)
_wo6("goto_control_cue_timeline", {"cue": "Start", "reverse_only": True, "timeline": "Intro loop"},
     '[m]gotoControlCue "Start" true "Intro loop"')
_wo6("reset", {}, "[m]reset")
_wo6("set_input", {"input": "uno", "value": 0.75, "transition_ms": 1000}, '[m]setInput "uno" 0.7500 1000', **_A)
_wo6("adjust_input", {"input": "uno", "delta": "+0.1"}, '[m]setInput "uno" +0.1 0')
_wo6("set_inputs_2", {"transition_ms": 100, "input1": "Yo", "value1": 0.8, "input2": "Man", "value2": 0.5},
     '[m]setInputs 100 "Yo" 0.8000 "Man" 0.5000')
_wo6("enable_layer_conditions", {"conditions": 3}, "[m]enableLayerCond 3")
_wo6("standby", {"enter": True, "fade_ms": 1000}, "[m]standBy true 1000", **_A)
_wo6("hit_test", {"x": 1200, "y": 250}, "[m]hitTest 1200 250", model="display",
     device_reply='[m]Reply true "Image01" 1154 212 0\r\n',
     expect_result={"ok": {"kind": "value", "value": 'true "Image01" 1154 212 0'}})
_wo6("online", {"online": False}, "[m]online false")
_wo6("load_production", {"path": "C:/Samples/ExampleShow.watch", "conditions": 3, "go_online": False},
     '[m]load "C:/Samples/ExampleShow.watch" 3 false', **_A)
_wo6("load_display", {"show": "ExampleShow"}, '[m]load "ExampleShow" true true 0', model="display",
     device_reply='[m]Busy "Loading" "" 0\r\n', expect_result={"ok": {"kind": "ack"}})
_wo6("set_logo_string", {"text": "Main stage"}, '[m]setLogoString "Main stage"', model="display")
_wo6("timecode_mode", {"mode": 2, "offset_ms": -3600000}, "[m]timecodeMode 2 -3600000", model="display")
_wo6("power_down", {}, "[m]powerDown", model="display",
     **_wo6_err('[m]Error 8 2 "Authority insufficient"'))
_wo6("subscribe_timeline", {"timeline": "Intro loop"},
     '[m]getStatus 1 "TaskList:mItemList:mItems:TimelineTask \\"Intro loop\\""', **_A)
_wo6("unsubscribe_timeline", {"timeline": "Intro loop"},
     '[m]getStatus 0 "TaskList:mItemList:mItems:TimelineTask \\"Intro loop\\""')
_wo6("get_aux_timelines", {}, "[m]getAuxTimelines tree",
     device_reply='[m]Reply {"ItemList":[{"Name":"Intro loop","Duration":60000}]}\r\n',
     expect_result={"ok": {"kind": "value", "value": '{"ItemList":[{"Name":"Intro loop","Duration":60000}]}'}})
_wo6("get_control_cues", {"filter": 4}, "[m]getControlCues 4")
_wo6("get_control_cues_timeline", {"timeline": "Intro loop"}, '[m]getControlCues 0 "Intro loop"')
_wo6("get_inputs", {}, "[m]getInputs")

# ── Telemetry ──
WO6_CONNECT = ["[m]authenticate 1\r\n"]
telemetry(WO6, "status-reply", expect_connect_wire=WO6_CONNECT,
          inbound='[m]Reply "WO2Launch" false 0 true true false 122 true 1 false 1695024000000\r\n',
          expect_state={"show": {"name": "WO2Launch"},
                        "status": {"busy": False, "cluster_health": "ok", "display_open": True,
                                   "show_active": True, "programmer_online": False, "standby": False,
                                   "world_time_ms": 1695024000000},
                        "main": {"position_ms": 122, "playing": True, "rate": 1.0}})
telemetry(WO6, "status-inactive", inbound='Status "" "" false 3 false false true\r\n',
          expect_state={"show": {"name": ""},
                        "status": {"busy": False, "cluster_health": "dead", "display_open": False,
                                   "show_active": False, "programmer_online": True}})
telemetry(WO6, "timeline-status",
          inbound='Status "TaskList:mItemList:mItems:TimelineTask \\"Intro loop\\"" 2 4500 1695024000000\r\n',
          expect_state={"timelines": {"Intro loop": {"state": "running", "position_ms": 4500}}})
telemetry(WO6, "timeline-in-folder",
          inbound='Status "TaskList:mItemList:mItems:TaskFolder \\"Act 1\\" :mItemList:mItems:TimelineTask '
                  '\\"Fog\\"" 1 0 1695024000000\r\n',
          expect_state={"timelines": {"Fog": {"state": "paused", "position_ms": 0}}})
telemetry(WO6, "ready", inbound='Ready "6.6.5" "WATCHPOINT" "Windows" true\r\n',
          expect_state={"system": {"version": "6.6.5", "program": "WATCHPOINT", "os": "Windows",
                                   "license_ok": True}})
telemetry(WO6, "busy", inbound='Busy "Transferring" "Media/Wilfred.jpg" 76\r\n',
          expect_state={"busy": {"activity": "Transferring", "subject": "Media/Wilfred.jpg", "percent": 76}})
telemetry(WO6, "error", inbound='Error 5 2 "Media/Wilfred.jpg"\r\n',
          expect_state={"last_error": {"kind": "file_server", "text": '2 "Media/Wilfred.jpg"'}})
telemetry(WO6, "warning", inbound='Warning "Low Memory: Primary Video 1960 KB"\r\n',
          expect_state={"last_warning": "Low Memory: Primary Video 1960 KB"})
telemetry(WO6, "information", inbound='Information "Cluster established"\r\n',
          expect_state={"last_information": "Cluster established"})
telemetry(WO6, "quit", inbound="Quit\r\n", expect_state={"system": {"quitting": True}})
