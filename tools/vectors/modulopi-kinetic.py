# Modulo Kinetic (modulopi-kinetic): JSON-RPC 2.0 over the Designer's
# websocket, as the manual's websocket pages write the requests
# ({"jsonrpc":"2.0","method":"doaction.timeline","params":{"uuid":...,
# "action":"play","timecode":60000}}), each with an id naming what it does.
MK = "modulopi-kinetic"
ACK = {"ok": {"kind": "ack"}}
TL = "{b9ff8f5b-5a15-4422-9e25-b8eeed9f8f9a}"
TK = "{45c8566e-435d-4abb-ad3a-2800cb1ce1c5}"
VR = "{06db974b-8811-4dc8-ac41-5fbf4c17634d}"


def _req(method, params, id_):
    p = "" if params is None else ',"params":' + params
    return '{"jsonrpc":"2.0","method":"%s"%s,"id":%s}' % (method, p, json.dumps(id_))


def _mk(command, input, method, params, id_, result="true", value=None):
    reply = '{"jsonrpc":"2.0","result":%s,"id":%s}' % (result, json.dumps(id_))
    expect = ACK if value is None else {"ok": {"kind": "value", "value": value}}
    text(MK, command, input, _req(method, params, id_), device_reply=reply, expect_result=expect)


TASKS = [{"keyword": "", "name": "Task6", "objectType": "task", "taskEnable": True, "triggerEnable": True, "triggered": False,
          "uiColor": "#ff0000", "uuid": TK}]
TLS = [{"name": "Main", "objectType": "timeline", "uiColor": "#00ff00", "uuid": TL, "timecode": 60000, "on": True, "play": True,
        "grandMasterFader": 100, "lockToTc": False, "audioMaster": 80}]
VARS = [{"name": "count", "objectType": "variable", "variableType": "int", "value": 12, "uiColor": "", "uuid": VR}]
VN = [{"blind": False, "name": "Node 1", "objectType": "vnode", "renderSurfaceUuid": "{1}", "state": 2, "uiColor": "",
       "uuid": TK, "ip": "10.0.0.21", "fps": 60, "version": "6.1.0"}]
PCS = [{"name": "DESIGNER", "objectType": "Computer", "status": "online", "isLockedEmbedded": False, "isEmbedded": False,
        "uiColor": "", "uuid": VR, "cpu": 12.5, "mem": 40.2, "version": "6.1", "fullVersionString": "6.1.0.1234"}]

_mk("call", {"method": "get.object.timeline", "params": {"name": "Main"}}, "get.object.timeline", '{"name":"Main"}', "call",
    json.dumps(TLS[0]), TLS[0])
_mk("list_tasks", {}, "get.list.tasks", None, "tasks", json.dumps(TASKS), TASKS)
_mk("get_task", {"uuid": TK}, "get.object.task", '{"uuid":"%s"}' % TK, "task " + TK, json.dumps(TASKS[0]), TASKS[0])
_mk("get_task_by_name", {"name": "Task6"}, "get.object.task", '{"name":"Task6"}', "task by name", json.dumps(TASKS[0]), TASKS[0])
_mk("launch_task", {"uuid": TK}, "doaction.task", '{"uuid":"%s","action":"launch"}' % TK, "launch " + TK)
_mk("set_task_enabled", {"uuid": TK, "enabled": True}, "set.property.task",
    '{"uuid":"%s","property":"taskEnable","value":true}' % TK, "task enable " + TK)
_mk("set_task_trigger_enabled", {"uuid": TK, "enabled": False}, "set.property.task",
    '{"uuid":"%s","property":"triggerEnable","value":false}' % TK, "task trigger " + TK)
_mk("rename_task", {"uuid": TK, "name": "Opening"}, "set.property.task", '{"uuid":"%s","property":"name","value":"Opening"}' % TK,
    "task name " + TK)
_mk("set_task_keyword", {"uuid": TK, "keyword": "go"}, "set.property.task", '{"uuid":"%s","property":"keyword","value":"go"}' % TK,
    "task keyword " + TK)
_mk("set_task_color", {"uuid": TK, "color": "#55aaff"}, "set.property.task", '{"uuid":"%s","property":"uiColor","value":"#55aaff"}' % TK,
    "task color " + TK)
_mk("list_timelines", {}, "get.list.timelines", None, "timelines", json.dumps(TLS), TLS)
_mk("get_timeline", {"uuid": TL}, "get.object.timeline", '{"uuid":"%s"}' % TL, "timeline " + TL, json.dumps(TLS[0]), TLS[0])
_mk("get_timeline_by_name", {"name": "Main"}, "get.object.timeline", '{"name":"Main"}', "timeline by name", json.dumps(TLS[0]), TLS[0])
_mk("timeline_play", {"uuid": TL}, "doaction.timeline", '{"uuid":"%s","action":"play"}' % TL, "play " + TL)
_mk("timeline_play_at", {"uuid": TL, "timecode_ms": 60000}, "doaction.timeline", '{"uuid":"%s","action":"play","timecode":60000}' % TL,
    "play at " + TL)
_mk("timeline_locate", {"uuid": TL}, "doaction.timeline", '{"uuid":"%s","action":"locate"}' % TL, "locate " + TL)
_mk("timeline_locate_at", {"uuid": TL, "timecode_ms": 0}, "doaction.timeline", '{"uuid":"%s","action":"locate","timecode":0}' % TL,
    "locate at " + TL)
_mk("timeline_grand_master", {"uuid": TL, "percent": 0.0, "fade_ms": 3000}, "doaction.timeline",
    '{"uuid":"%s","action":"grandMasterFader","value":0.00,"timecode":3000}' % TL, "grand master " + TL)
_mk("timeline_audio_master", {"uuid": TL, "percent": 50.0}, "doaction.timeline",
    '{"uuid":"%s","action":"audioMaster","value":50.00,"timecode":0}' % TL, "audio master " + TL)
_mk("set_timeline_on", {"uuid": TL, "enabled": True}, "set.property.timeline", '{"uuid":"%s","property":"on","value":true}' % TL,
    "timeline on " + TL)
_mk("set_timeline_lock_to_tc", {"uuid": TL, "enabled": True}, "set.property.timeline",
    '{"uuid":"%s","property":"lockToTc","value":true}' % TL, "timeline lock " + TL)
_mk("rename_timeline", {"uuid": TL, "name": "Show"}, "set.property.timeline", '{"uuid":"%s","property":"name","value":"Show"}' % TL,
    "timeline name " + TL)
_mk("set_timeline_color", {"uuid": TL, "color": "#00ff00"}, "set.property.timeline",
    '{"uuid":"%s","property":"uiColor","value":"#00ff00"}' % TL, "timeline color " + TL)
_mk("list_variables", {}, "get.list.variables", None, "variables", json.dumps(VARS), VARS)
_mk("get_variable", {"uuid": VR}, "get.object.variable", '{"uuid":"%s"}' % VR, "variable " + VR, json.dumps(VARS[0]), VARS[0])
_mk("get_variable_by_name", {"name": "count"}, "get.object.variable", '{"name":"count"}', "variable by name", json.dumps(VARS[0]), VARS[0])
_mk("set_variable_value", {"uuid": VR, "value": [100, 200]}, "set.property.variable",
    '{"uuid":"%s","property":"value","value":[100,200]}' % VR, "variable value " + VR)
_mk("rename_variable", {"uuid": VR, "name": "score"}, "set.property.variable", '{"uuid":"%s","property":"name","value":"score"}' % VR,
    "variable name " + VR)
_mk("set_variable_color", {"uuid": VR, "color": "#55aaff"}, "set.property.variable",
    '{"uuid":"%s","property":"uiColor","value":"#55aaff"}' % VR, "variable color " + VR)
_mk("list_vnodes", {}, "get.list.vnodes", None, "vnodes", json.dumps(VN), VN)
_mk("get_vnode", {"uuid": TK}, "get.object.vnode", '{"uuid":"%s"}' % TK, "vnode " + TK, json.dumps(VN[0]), VN[0])
_mk("vnode_action", {"uuid": TK, "action": "online"}, "doaction.vnode", '{"uuid":"%s","action":"online"}' % TK, "vnode online " + TK)
_mk("set_vnode_blind", {"uuid": TK, "blind": True}, "set.property.vnode", '{"uuid":"%s","property":"blind","value":true}' % TK,
    "vnode blind " + TK)
_mk("rename_vnode", {"uuid": TK, "name": "Left"}, "set.property.vnode", '{"uuid":"%s","property":"name","value":"Left"}' % TK,
    "vnode name " + TK)
_mk("set_vnode_color", {"uuid": TK, "color": "#123456"}, "set.property.vnode", '{"uuid":"%s","property":"uiColor","value":"#123456"}' % TK,
    "vnode color " + TK)
_mk("list_computers", {}, "get.list.computers", None, "computers", json.dumps(PCS), PCS)
_mk("get_computer", {"uuid": VR}, "get.object.computer", '{"uuid":"%s"}' % VR, "computer " + VR, json.dumps(PCS[0]), PCS[0])
_mk("kinetic_action", {"uuid": VR, "action": "save"}, "doaction.kinetic", '{"uuid":"%s","action":"save"}' % VR, "kinetic save")
# An error reply fails the command.
V.append({"spec": MK, "command": "launch_task", "file": "launch_task.error", "input": {"uuid": TK},
          "expect_wire": _req("doaction.task", '{"uuid":"%s","action":"launch"}' % TK, "launch " + TK),
          "device_reply": '{"jsonrpc":"2.0","error":{"code":-32602,"message":"Invalid params"},"id":"launch %s"}' % TK,
          "expect_result": {"error": {"error": "device_error"}}})


def _poll(name, id_, result, state):
    telemetry(MK, name, inbound_ws='{"jsonrpc":"2.0","result":%s,"id":"%s"}' % (json.dumps(result), id_), expect_state=state)


_poll("tasks", "poll tasks", TASKS, {"tasks": {TK: {"name": "Task6", "keyword": "", "enabled": True, "trigger_enabled": True,
                                                    "triggered": False, "color": "#ff0000"}}})
_poll("timelines", "poll timelines", TLS, {"timelines": {TL: {"name": "Main", "timecode": 60000, "on": True, "playing": True,
                                                              "grand_master": 100.0, "audio_master": 80.0, "lock_to_tc": False,
                                                              "color": "#00ff00"}}})
_poll("variables", "poll variables", VARS, {"variables": {VR: {"name": "count", "object_type": "variable", "type": "int",
                                                              "value": "12", "color": ""}}})
_poll("vnodes", "poll vnodes", VN, {"vnodes": {TK: {"name": "Node 1", "state": 2, "blind": False, "ip": "10.0.0.21", "fps": 60.0,
                                                    "version": "6.1.0", "render_surface": "{1}"}}})
_poll("computers", "poll computers", PCS, {"computers": {VR: {"name": "DESIGNER", "status": "online", "cpu": 12.5, "memory": 40.2,
                                                             "version": "6.1", "full_version": "6.1.0.1234", "embedded": False}}})
