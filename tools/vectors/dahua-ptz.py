# Dahua (dahua-ptz): one vector per command, written by hand from the request
# forms a public integration uses for /cgi-bin/ptz.cgi (Dahua's own HTTP API
# document is for partners only): action, channel, code and three arguments.
DH = "dahua-ptz"
DQ = "/cgi-bin/ptz.cgi?"

http(DH, "move", {"direction": "LeftUp", "speed": 8}, "GET", DQ + "action=start&channel=1&code=LeftUp&arg1=8&arg2=8&arg3=0",
     http_reply={"status": 200, "body": "OK\r\n"}, expect_result={"ok": {"kind": "ack"}})
http(DH, "move", {"direction": "Up"}, "GET", DQ + "action=start&channel=2&code=Up&arg1=4&arg2=4&arg3=0",
     settings={"channel": 2}, file="move-channel-2")
http(DH, "stop", {}, "GET", DQ + "action=stop&channel=1&code=Left&arg1=1&arg2=1&arg3=0")
http(DH, "zoom_in", {}, "GET", DQ + "action=start&channel=1&code=ZoomTele&arg1=0&arg2=0&arg3=0")
http(DH, "zoom_out", {}, "GET", DQ + "action=start&channel=1&code=ZoomWide&arg1=0&arg2=0&arg3=0")
http(DH, "zoom_stop", {}, "GET", DQ + "action=stop&channel=1&code=ZoomWide&arg1=0&arg2=0&arg3=0")
http(DH, "focus_near", {}, "GET", DQ + "action=start&channel=1&code=FocusNear&arg1=0&arg2=0&arg3=0")
http(DH, "focus_far", {}, "GET", DQ + "action=start&channel=1&code=FocusFar&arg1=0&arg2=0&arg3=0")
http(DH, "focus_stop", {}, "GET", DQ + "action=stop&channel=1&code=FocusFar&arg1=0&arg2=0&arg3=0")
http(DH, "goto_preset", {"preset": 3}, "GET", DQ + "action=start&channel=1&code=GotoPreset&arg1=0&arg2=3&arg3=0")
http(DH, "goto_preset", {"preset": 300}, "GET", DQ + "action=start&channel=1&code=GotoPreset&arg1=0&arg2=300&arg3=0",
     http_reply={"status": 400, "body": "Error\r\nBad Request!\r\n"},
     expect_result={"error": {"error": "device_error", "code": "400"}}, file="goto_preset-refused")
