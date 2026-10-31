# Rational Acoustics Smaart (rationalacoustics-smaart): the JSON requests the
# MIT-licensed Companion modules send on Smaart's API WebSocket, each with its
# fixed sequenceNumber, which the answer carries back. A refused request
# answers with response.error.
SM = "rationalacoustics-smaart"
SM_OK = {"ok": {"kind": "ack"}}


def sm(command, input, wire, seq, **extra):
    text(SM, command, input, wire, device_reply=f'{{"sequenceNumber":{seq},"response":{{}}}}',
         expect_result=SM_OK, **extra)


def key(command, input, keypress, **extra):
    sm(command, input, '{"sequenceNumber":16,"action":"issueCommand","properties":[{"keypress":"' + keypress + '"}]}',
       16, **extra)


text(SM, "get_status", {}, '{"sequenceNumber":2,"action":"get"}',
     device_reply='{"sequenceNumber":2,"response":{"authenticationRequired":false}}',
     expect_result={"ok": {"kind": "value", "value": {"authenticationRequired": False}}})
sm("reset_averages", {},
   '{"sequenceNumber":10,"action":"set","target":"activeMeasurements","properties":[{"runningAverage":0}]}', 10)
text(SM, "reset_averages", {},
     '{"sequenceNumber":10,"action":"set","target":"activeMeasurements","properties":[{"runningAverage":0}]}',
     device_reply='{"sequenceNumber":10,"response":{"error":"unknown target"}}',
     expect_result={"error": {"error": "device_error"}}, file="reset_averages-error")
sm("select_tab", {"tab": "Main PA"},
   '{"sequenceNumber":11,"action":"set","target":"tabs","properties":[{"activeTab":"Main PA"}]}', 11,
   model="smaart-v8")
sm("start_tab_measurements", {"tab": "Main PA"},
   '{"sequenceNumber":12,"action":"set","target":{"tabName":"Main PA","measurementName":"allMeasurements"},'
   '"properties":[{"active":true}]}', 12)
sm("set_generator", {"active": True},
   '{"sequenceNumber":13,"action":"set","target":"signalGenerator","properties":[{"active":true}]}', 13)
sm("set_generator_level", {"level": -12.5},
   '{"sequenceNumber":14,"action":"set","target":"signalGenerator","properties":[{"gain":-12.5}]}', 14)
sm("set_delay_tracking", {"tracking": False},
   '{"sequenceNumber":15,"action":"set","target":{"measurementName":"allTransferFunctionMeasurements"},'
   '"properties":[{"trackingDelay":false}]}', 15)
sm("capture_trace", {"measurement": "Mic 1"},
   '{"sequenceNumber":17,"action":"capture","target":{"measurementName":"Mic 1"}}', 17, model="smaart-v8")
sm("rename_trace", {"path": "C:/Smaart/Traces/Mic 1.trf", "name": "FOH"},
   '{"sequenceNumber":18,"action":"set","target":{"traceFilePath":"C:/Smaart/Traces/Mic 1.trf"},'
   '"properties":[{"name":"FOH"}]}', 18, model="smaart-di")
key("press_key", {"keypress": "shift + command + H"}, "shift + command + H")
key("zoom_x", {"direction": "in"}, "option + command+")
key("zoom_y", {"direction": "out"}, "-")
key("zoom_xy", {"direction": "in"}, "command+")
key("zoom_preset", {"preset": "zoom_3"}, "option + 3")
key("cursor_key", {"direction": "left"}, "cursor left")
key("cycle_z_order", {"direction": "backward"}, "shift + Z")
key("hide_trace", {}, "H")
key("hide_all_traces", {}, "shift + command + H")
key("toggle_peak_hold", {}, "P")
key("toggle_input_meters", {}, "shift + E")
key("toggle_input_meter_orientation", {}, "shift + option + E")
key("real_time_mode", {}, "R")
key("impulse_mode", {}, "I")
key("spl_mode", {}, "option + H")
key("toggle_spl_meters", {}, "E")
key("select_view", {"view": "multi_spectrum"}, "0")
key("top_trace_y_offset", {"direction": "down"}, "command + cursor down")
key("clear_top_trace_y_offset", {}, "Y")
key("clear_all_y_offsets", {}, "command + Y")
key("toggle_bar", {"bar": "data"}, "B")
key("lock_cursor_to_peak", {}, "command + P")
key("clear_locked_cursor", {}, "command + X")
key("move_locked_cursor", {"direction": "right"}, "command + cursor right")
key("cycle_preferred_plot", {}, "M")

telemetry(SM, "authentication", inbound_ws='{"sequenceNumber":2,"response":{"authenticationRequired":true}}',
          expect_state={"authentication_required": True})
telemetry(SM, "error", inbound_ws='{"sequenceNumber":13,"response":{"error":"signal generator required"}}',
          expect_state={"last_error": "signal generator required"})
telemetry(SM, "login", settings={"password": "s3cret"},
          expect_connect_wire='{"sequenceNumber":1,"action":"set","properties":[{"password":"s3cret"}]}',
          inbound_ws='{"sequenceNumber":2,"response":{"authenticationRequired":false}}',
          expect_state={"authentication_required": False})
