# Panasonic KAIROS (panasonic-kairos): the Simple Control Protocol on TCP
# 3005 (path=value, path, subscribe:path, CR LF; answers OK, Error and
# path=value) and the REST API on 1234 (GET lists, PATCH merge-patch), from
# Panasonic's protocol definition and API Specifications V1.0.
PK = "panasonic-kairos"
_ok = {"device_reply": "OK\r\n", "expect_result": {"ok": {"kind": "ack"}}}
text(PK, "set_attribute", {"path": "AUX1.source", "value": "IN1"}, "AUX1.source=IN1\r\n", **_ok)
text(PK, "get_attribute", {"path": "AUX1.source"}, "AUX1.source\r\n", device_reply="AUX1.source=IN1\r\n",
     expect_result={"ok": {"kind": "value", "value": "IN1"}})
text(PK, "call_function", {"path": "MV1.clear_layout"}, "MV1.clear_layout=\r\n", **_ok)
text(PK, "call_function_with", {"path": "MV1.recall_layout", "argument": "1"}, "MV1.recall_layout=1\r\n", **_ok)
text(PK, "subscribe", {"path": "IN1.tally"}, "subscribe:IN1.tally\r\n", **_ok)
text(PK, "unsubscribe", {"path": "IN1.tally"}, "unsubscribe:IN1.tally\r\n", **_ok)
text(PK, "scene_cut", {"scene": "SCENES.Main"}, "SCENES.Main.cut=\r\n", **_ok)
text(PK, "scene_auto", {"scene": "SCENES.Templates.2Box"}, "SCENES.Templates.2Box.auto=\r\n", **_ok)
text(PK, "set_aux_source", {"aux": "AUX1", "source": "IN1"}, "AUX1.source=IN1\r\n", **_ok)
text(PK, "set_aux_source", {"aux": "AUX1", "source": "IN99"}, "AUX1.source=IN99\r\n", device_reply="Error\r\n",
     expect_result={"error": {"error": "device_error"}}, file="set_aux_source-error")
text(PK, "recall_multiviewer_layout", {"multiviewer": "MV1", "layout": 1}, "MV1.recall_layout=1\r\n", **_ok)
text(PK, "clear_multiviewer_layout", {"multiviewer": "MV2"}, "MV2.clear_layout=\r\n", **_ok)


def _rest(command, input, target, body):
    V.append({"spec": PK, "command": command, "input": input,
              "expect_request": {"method": "PATCH", "port": 1234, "target": target, "body": body}})


_rest("set_layer_source_a", {"scene": "Main", "layer": "Background", "source": "IN1"}, "/scenes/Main/Background", '{"sourceA":"IN1"}')
_rest("set_layer_source_b", {"scene": "Main", "layer": "Background", "source": "Black"}, "/scenes/Main/Background", '{"sourceB":"Black"}')
_rest("play_macro", {"macro": "Macro1"}, "/macros/Macro1", '{"state":"play"}')
_rest("set_multiviewer_preset", {"multiviewer": "0", "preset": 11}, "/multiviewers/0", '{"preset":11}')
telemetry(PK, "attribute", inbound="IN1.tally=1\r\n", expect_state={"attributes": {"IN1.tally": "1"}})
telemetry(PK, "new-production", inbound="APPLICATION:NEW\r\n", expect_state={"production": {"reloads": 1}},
          expect_then_send=[{"method": "GET", "port": 1234, "target": t}
                            for t in ("/scenes", "/inputs", "/aux", "/macros", "/multiviewers")])
telemetry(PK, "scenes", inbound_http={"path": "/scenes", "body": json.dumps([{
    "layers": [{"name": "Background", "sourceA": "IN1", "sourceB": "Black", "sources": ["Black"], "uuid": "ce41"},
               {"name": "Layer-1", "sourceA": "Black", "sources": ["Black", "White"], "uuid": "6150"}],
    "name": "Main", "tally": 1, "uuid": "f1a5"}])},
    state_before={"scenes": {"old": {"name": "Gone"}}},
    expect_state={"scenes": {"f1a5": {"name": "Main", "tally": 1, "layers": {
        "ce41": {"name": "Background", "index": 0, "source_a": "IN1", "source_b": "Black"},
        "6150": {"name": "Layer-1", "index": 1, "source_a": "Black"}}}}})
telemetry(PK, "inputs", inbound_http={"path": "/inputs", "body": json.dumps([
    {"index": 0, "name": "IN1", "tally": 1, "uuid": "e532"}])},
    expect_state={"inputs": {"e532": {"name": "IN1", "index": 0, "tally": 1}}})
telemetry(PK, "aux", inbound_http={"path": "/aux", "body": json.dumps([
    {"name": "AUX1", "source": "IN1", "sources": ["Black", "IN1"], "uuid": "5d15"}])},
    expect_state={"aux": {"5d15": {"name": "AUX1", "source": "IN1"}}})
telemetry(PK, "macros", inbound_http={"path": "/macros", "body": json.dumps([
    {"name": "Macro1", "state": None, "uuid": "eb71"}])},
    expect_state={"macros": {"eb71": {"name": "Macro1"}}})
telemetry(PK, "multiviewers", inbound_http={"path": "/multiviewers", "body": json.dumps([
    {"index": 0, "name": "Multiviewer1", "preset": None, "presets": [{"id": 0, "name": "Full", "usr": False}],
     "sdp": "v=0\nc=IN IP4 239.168.50.115\n", "uuid": "fd83"}])},
    expect_state={"multiviewers": {"fd83": {"name": "Multiviewer1", "index": 0, "sdp": "v=0\nc=IN IP4 239.168.50.115\n",
                                            "presets": {"0": {"name": "Full", "user": False}}}}})
