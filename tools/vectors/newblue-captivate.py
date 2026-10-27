# NewBlue Captivate (newblue-captivate): one vector per command, over the
# Advanced API on TCP 9025. Each scheduleCommand line is computed here from
# the command name and parameter names of NewBlue's OpenAPI document
# (API_VERSION 250721), written as compact JSON by json.dumps:
# {"command": <name>, "parameters": {...}, "variables": {}} and CR LF. Flags
# documented as "set to 1" go as the number 1 or 0, or the text "1"/"0" where
# the document types them as strings; booleans the document types as boolean
# go as true/false.
NB = "newblue-captivate"
_NB_OK = {"device_reply": '{"result":{"command":"x","reply":"x","success":true}}\r\n',
          "expect_result": {"ok": {"kind": "ack"}}}


def _nb_line(obj):
    return json.dumps(obj, separators=(",", ":"), ensure_ascii=False) + "\r\n"


def _nb(command, input, cmd, parameters, **extra):
    text(NB, command, input, _nb_line({"command": cmd, "parameters": parameters, "variables": {}}), **extra)


_TI = "{218f46f9-e881-49a8-ab17-03c8f9cc8461}"
_IN = "API Tour: JSON Command Tester"

# scheduleAction, updateInputDefinition and settings (the document's own examples).
text(NB, "schedule_action", {"action": "update", "title": "My Title", "variables": {"Time": "00:00", "Message": "Go!"}},
     _nb_line({"action": "update", "input": "", "title": "My Title", "variables": {"Time": "00:00", "Message": "Go!"}}),
     device_reply='{"result":{"command":"schedule","reply":"schedule","success":true}}\r\n',
     expect_result={"ok": {"kind": "ack"}})
text(NB, "schedule_input_action", {"action": "animateIn", "input": "Scoreboard OCR: Basketball", "variables": {"Home Score": 13}},
     _nb_line({"action": "animateIn", "input": "Scoreboard OCR: Basketball", "title": "", "variables": {"Home Score": 13}}),
     device_reply='{"error":"scheduleCommand failed","data":{}}\r\n',
     expect_result={"error": {"error": "device_error"}})
text(NB, "custom_message", {"to": "Some Listener", "data": {"score": 3}},
     _nb_line({"command": "customMessage", "parameters": {"to": "Some Listener", "sender": "", "data": {"score": 3}},
               "variables": {}}))
text(NB, "update_input_definition", {"input": "My Input", "definition": {"method": "merge",
                                     "variables": {"Clock": {"pattern": "[0-9][0-9]:[0-9][0-9]"}}}},
     _nb_line({"redefine": "My Input", "definition": {"method": "merge",
                                                      "variables": {"Clock": {"pattern": "[0-9][0-9]:[0-9][0-9]"}}}}))
text(NB, "save_input_settings", {"input": _IN, "settings": {"name": "value"}},
     _nb_line({"input": _IN, "settings": {"name": "value"}}), device_reply='{"result":null}\r\n',
     expect_result={"ok": {"kind": "ack"}})
text(NB, "load_input_settings", {"input": _IN}, _nb_line({"input": _IN, "settings": "load"}),
     device_reply='{"result":{"name":"value"}}\r\n', expect_result={"ok": {"kind": "value", "value": {"name": "value"}}})
V.append({"spec": NB, "command": "get_title_xml", "input": {"id": "{b960a28b-e583-4f3f-b54b-851ef481314d}"},
          "expect_request": {"method": "GET", "port": 9022,
                             "target": "/api/title/%7Bb960a28b-e583-4f3f-b54b-851ef481314d%7D"}})

# scheduleCommand: example values per parameter name, and the wire name and
# form of each, from the OpenAPI document.
_EX = {"title": _TI, "input": _IN, "channel": 1, "file": "C:/Shows/show.nbtlproj", "video": "NDI Video",
       "width": 320, "height": 180, "position": 10.3, "name": "New Title", "variable": "Name",
       "input_variable": "Score", "title_variable": "Home", "profile": "Google: Sheets.Sheet 1", "cache": 10,
       "save": 2, "device": "NDI", "channels": "0,1", "frame_rate": 29.97, "service": "Facebook",
       "url": "rtmps://live-api-s.facebook.com:443/rtmp/", "key": "FB-1234", "folder": "C:/pngout",
       "subfolder": "take1", "settings": {"bufferLength": 15, "playSpeed": 0.5}, "speed": 0.5,
       "inputs": ["USB Camera VID:1133 PID:2075"], "provider": "zoom", "meeting": "81234567890",
       "password": "1234", "event": "in", "inputs_list": "Input A,Input B"}
_WIRE = {"name": "newName", "input_variable": "inputVar", "title_variable": "titleVar", "cache": "maxCacheCount",
         "save": "maxSaveCount", "frame_rate": "frameRate", "service": "choice", "url": "serverUrl",
         "key": "streamKey", "auto_cache": "autoCache", "video_source": "videoSource", "provider": "meetingProvider",
         "meeting": "meetingId", "show_ui": "showUI", "send_video": "sendVideo"}
_CMD = {}


def _nb_vectors():
    doc = yaml.safe_load((ROOT / "specs" / "newblue-captivate.yaml").read_text(encoding="utf-8"))
    for name, c in doc["commands"].items():
        send = c.get("send")
        if not isinstance(send, str) or not send.startswith('{"command":') or name == "custom_message":
            continue
        cmd = json.loads(send.split('"parameters"')[0][:-1] + "}")["command"]
        params = c.get("params") or {}
        inp, wire = {}, {}
        for p, s in params.items():
            w = _WIRE.get(p, p)
            if p == "position" and s["type"] == "int":
                inp[p], wire["position" if name == "set_title_position" else "newPosition"] = 2, 2
                continue
            if p == "inputs" and s["type"] == "string":
                inp[p], wire["input"] = "Input A,Input B", "Input A,Input B"
                continue
            if p == "save" and name == "close_project":
                inp[p], wire["saveProject"] = True, 1
                continue
            if p == "confirm":
                wire["closeProject"] = "1"
                continue
            if p == "name" and name == "join_meeting":
                w = "name"
            if p == "video" and name == "get_video_input_info":
                w = "videoIn"
            if s["type"] == "bool":
                v = True
                inp[p] = v
                if p in ("merge", "auto_cache"):
                    wire[w] = "1"
                elif p in ("dynamic", "verbose", "show_ui", "send_video"):
                    wire[w] = 1
                else:
                    wire[w] = True
                continue
            if s["type"] == "enum":
                v = _EX.get(p, s["values"][0])
            elif "default" in s and p not in _EX:
                v = s["default"]
            else:
                v = _EX[p]
            inp[p] = v
            if s["type"] == "float":
                wire[w] = float(f"{v:.3f}")
            else:
                wire[w] = v
        line = _nb_line({"command": cmd, "parameters": wire, "variables": {}})
        if s_float := any(s["type"] == "float" for s in params.values()):
            # Floats are written with three decimals (29.970), which json.dumps does not keep.
            for p, s in params.items():
                if s["type"] == "float":
                    w = _WIRE.get(p, p)
                    line = line.replace(f'"{w}":{float(f"{inp[p]:.3f}")}', f'"{w}":{inp[p]:.3f}')
        extra = _NB_OK if name in ("macro", "delete_title", "start_streaming") else {}
        if c["returns"] == "value" and name == "get_time":
            extra = {"device_reply": '{"result":{"command":"getTime","reply":"getTime","success":true,"time":"911.01"}}\r\n',
                     "expect_result": {"ok": {"kind": "value", "value": {"command": "getTime", "reply": "getTime",
                                                                         "success": True, "time": "911.01"}}}}
        text(NB, name, inp, line, **extra)


_nb_vectors()

# Telemetry: getTitleControlInfo (the title object the scheduleAction page
# shows), a play notification (the document's example), and the status reads.
telemetry(NB, "titles", inbound=_nb_line({"result": {"command": "getTitleControlInfo", "reply": "getTitleControlInfo",
                                                     "success": True, "titleCount": "1", "titles": [
    {"channel": "0", "id": "{adf82825-daea-4db4-a919-b1394d257f1e}", "inSwitcher": "0", "input": _IN,
     "inputs": [{"name": _IN, "url": "/API Tour/tour/JsonCommands.html"}], "name": "New Title", "playStatus": "Off",
     "renderProgress": "1", "status": "Done", "type": "title", "url": "/API Tour/tour/JsonCommands.html",
     "variables": [{"value": "Name", "variable": "Name"}, {"value": "Subtitle", "variable": "Subtitle"}]}]}}),
    state_before={"titles": {"{gone}": {"name": "Deleted"}}},
    expect_state={"titles": {"{adf82825-daea-4db4-a919-b1394d257f1e}": {
        "name": "New Title", "channel": 0, "input": _IN, "play_status": "Off", "render_status": "Done",
        "render_progress": 1.0, "type": "title", "variables": {"Name": "Name", "Subtitle": "Subtitle"}}}})
telemetry(NB, "play-notification", inbound=_nb_line({"notification": {
    "channel": -1, "command": "notify", "event": "play", "id": "{cd654eba-4afe-4838-ad75-65ff823ac0dd}", "input": "",
    "play": "Running", "sender": "", "title": "Acrylic Baseball Scoreboard", "to": ""}}),
    expect_state={"titles": {"{cd654eba-4afe-4838-ad75-65ff823ac0dd}": {"play_status": "Running"}}})
telemetry(NB, "product-info", inbound=_nb_line({"result": {
    "api_version": "240910", "buildDate": "Feb 21 2025", "buildTime": "16:30:45", "command": "getProductInfo",
    "licensed": True, "product": "Captivate", "reply": "getProductInfo", "sha": "d74bf9", "sku": "SKUTL5BR",
    "success": True, "version": "2024.0.241118"}}),
    expect_state={"system": {"product": "Captivate", "version": "2024.0.241118", "api_version": "240910",
                             "licensed": True, "sku": "SKUTL5BR", "build_date": "Feb 21 2025"}})
telemetry(NB, "video-format", inbound=_nb_line({"result": {
    "command": "getVideoFormat", "frameRate": "29.97", "height": "1080", "interlaced": "0", "reply": "getVideoFormat",
    "success": True, "width": "1920"}}),
    expect_state={"video_format": {"width": 1920, "height": 1080, "frame_rate": 29.97, "interlaced": False}})
telemetry(NB, "output-device", inbound=_nb_line({"result": {
    "command": "getOutputDevice", "device": "NDI", "reply": "getOutputDevice", "success": True}}),
    expect_state={"output": {"device": "NDI"}})
telemetry(NB, "project-edited", inbound=_nb_line({"result": {
    "command": "checkProjectEdited", "filePath": "", "hasChanged": "1", "isCached": "0", "isCustomized": "0",
    "isLibraryScene": "0", "newFilePath": "", "reply": "checkProjectEdited", "success": True}}),
    expect_state={"project": {"file": "", "changed": True, "cached": False, "customized": False,
                              "library_scene": False}})
telemetry(NB, "channels", inbound=_nb_line({"result": {
    "command": "readProject", "project": [{"playlist": [{"channel": "0", "name": "Channel 1"},
                                                        {"channel": "1", "name": "Lower Thirds"}]}],
    "reply": "readProject", "success": True}}),
    expect_state={"channels": {"0": {"name": "Channel 1"}, "1": {"name": "Lower Thirds"}}})
