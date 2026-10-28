# AMWA NMOS Node: the IS-04 v1.3 Node API, IS-05 Connection API v1.1, IS-08
# Channel Mapping API v1.0 and IS-07 Events API v1.0. Requests follow the RAML
# of each specification (AMWA-TV/is-04 v1.3.3, is-05 v1.1.2, is-08 v1.0.1,
# is-07 v1.0.1) and bodies their schemas and examples: the IS-05 PATCH with an
# SDP transport file (receiver-patch-transportfile.json), sender_id null to
# disconnect, the IS-08 activation (map-activations-post.json). Replies are the
# specifications' examples, shortened. A Node has no standard port: vectors
# open it on the runner's stand-in port, and the Connection, Channel Mapping
# and Events APIs go there too unless their port setting is given.
NN = "nmos-node"
_OK = {"ok": {"kind": "ack"}}
RX = "0a174530-e3cf-11e6-bf01-fe55135034f3"
TX = "5709255c-c0ae-4e1e-99a0-e872e83e48e0"
NODE = "/x-nmos/node/v1.3"
CONN = "/x-nmos/connection/v1.1/single"
CM = "/x-nmos/channelmapping/v1.0"
EV = "/x-nmos/events/v1.0"
SDP = ("v=0\r\no=- 1496222842 1496222842 IN IP4 172.29.226.25\r\ns=IP Studio Stream\r\nt=0 0\r\n"
       "m=video 5010 RTP/AVP 103\r\nc=IN IP4 232.250.98.80/32\r\na=rtpmap:103 raw/90000\r\n")


def _nn(command, input, method, target, body=None, port=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    if port is not None:
        req["port"] = port
    V.append({"spec": NN, "command": command, "input": input, "expect_request": req, **extra})


def _seq(command, input, requests, **extra):
    """A command of several requests: over HTTP the next goes only after the
    reply to the one before, so the vector states the first (the change);
    the read-back after it is a plain GET."""
    method, target, body = requests[0]
    _nn(command, input, method, target, body, **extra)


# ── IS-04 Node API ──
_nn("get_self", {}, "GET", NODE + "/self",
    http_reply={"status": 200, "body": '{"id":"3b8be755-08ff-452b-b217-c9151eb21193","label":"host1"}'},
    expect_result={"ok": {"kind": "value", "value": {"id": "3b8be755-08ff-452b-b217-c9151eb21193", "label": "host1"}}})
for t in ["devices", "sources", "flows", "senders", "receivers"]:
    _nn(f"get_{t}", {}, "GET", f"{NODE}/{t}")
    _nn(f"get_{t[:-1]}", {"id": RX}, "GET", f"{NODE}/{t}/{RX}")
_seq("set_receiver_target", {"receiver_id": RX, "sender": {"id": TX, "transport": "urn:x-nmos:transport:rtp.mcast"}},
     [("PUT", f"{NODE}/receivers/{RX}/target", '{"id":"' + TX + '","transport":"urn:x-nmos:transport:rtp.mcast"}'),
      ("GET", f"{NODE}/receivers", None)])
_seq("clear_receiver_target", {"receiver_id": RX},
     [("PUT", f"{NODE}/receivers/{RX}/target", "{}"), ("GET", f"{NODE}/receivers", None)])

# ── IS-05 Connection API ──
_nn("get_connection_senders", {}, "GET", CONN + "/senders")
_nn("get_connection_receivers", {}, "GET", CONN + "/receivers")
# The Connection API on a port of its own.
_nn("get_connection_receivers", {}, "GET", CONN + "/receivers", port=3215, settings={"connection_port": 3215},
    file="get_connection_receivers.port")
for t, i in (("sender", TX), ("receiver", RX)):
    for r in ["constraints", "staged", "active", "transporttype"]:
        _nn(f"get_{t}_{r}", {"id": i}, "GET", f"{CONN}/{t}s/{i}/{r}")
_nn("get_sender_transport_file", {"id": TX}, "GET", f"{CONN}/senders/{TX}/transportfile",
    http_reply={"status": 200, "body": SDP}, expect_result={"ok": {"kind": "value", "value": SDP.strip()}})

IMM = '"activation":{"mode":"activate_immediate"}'
_seq("connect_receiver", {"receiver_id": RX, "sender_id": TX, "sdp": SDP},
     [("PATCH", f"{CONN}/receivers/{RX}/staged",
       '{"sender_id":"' + TX + '","master_enable":true,' + IMM + ',"transport_file":{"data":'
       + json.dumps(SDP) + ',"type":"application/sdp"}}'),
      ("GET", f"{CONN}/receivers/{RX}/active", None)])
_seq("connect_receiver_with_params", {"receiver_id": RX, "sender_id": TX,
                                      "transport_params": [{"multicast_ip": "232.250.98.80", "destination_port": 5010}]},
     [("PATCH", f"{CONN}/receivers/{RX}/staged",
       '{"sender_id":"' + TX + '","master_enable":true,' + IMM
       + ',"transport_params":[{"multicast_ip":"232.250.98.80","destination_port":5010}]}'),
      ("GET", f"{CONN}/receivers/{RX}/active", None)])
_seq("disconnect_receiver", {"receiver_id": RX},
     [("PATCH", f"{CONN}/receivers/{RX}/staged", '{"sender_id":null,"master_enable":false,' + IMM + "}"),
      ("GET", f"{CONN}/receivers/{RX}/active", None)])
_seq("set_receiver_enabled", {"receiver_id": RX, "enabled": True},
     [("PATCH", f"{CONN}/receivers/{RX}/staged", '{"master_enable":true,' + IMM + "}"),
      ("GET", f"{CONN}/receivers/{RX}/active", None)])
_seq("set_sender_enabled", {"sender_id": TX, "enabled": False},
     [("PATCH", f"{CONN}/senders/{TX}/staged", '{"master_enable":false,' + IMM + "}"),
      ("GET", f"{CONN}/senders/{TX}/active", None)])
_seq("set_sender_receiver", {"sender_id": TX, "receiver_id": RX},
     [("PATCH", f"{CONN}/senders/{TX}/staged", '{"receiver_id":"' + RX + '",' + IMM + "}"),
      ("GET", f"{CONN}/senders/{TX}/active", None)])
_SCHED = {"activation": {"mode": "activate_scheduled_absolute", "requested_time": "1496759200:0"}}
_seq("stage_receiver", {"receiver_id": RX, "staged": _SCHED},
     [("PATCH", f"{CONN}/receivers/{RX}/staged",
       '{"activation":{"mode":"activate_scheduled_absolute","requested_time":"1496759200:0"}}'),
      ("GET", f"{CONN}/receivers/{RX}/active", None)])
_seq("stage_sender", {"sender_id": TX, "staged": {"activation": {"mode": None}}},
     [("PATCH", f"{CONN}/senders/{TX}/staged", '{"activation":{"mode":null}}'),
      ("GET", f"{CONN}/senders/{TX}/active", None)])
_nn("bulk_stage_receivers", {"items": [{"id": RX, "params": {"master_enable": False}}]}, "POST",
    "/x-nmos/connection/v1.1/bulk/receivers", '[{"id":"' + RX + '","params":{"master_enable":false}}]')
_nn("bulk_stage_senders", {"items": [{"id": TX, "params": {"master_enable": True}}]}, "POST",
    "/x-nmos/connection/v1.1/bulk/senders", '[{"id":"' + TX + '","params":{"master_enable":true}}]')

# ── IS-08 Channel Mapping API ──
_nn("get_channelmapping_io", {}, "GET", CM + "/io")
_nn("get_active_map", {}, "GET", CM + "/map/active")
_nn("get_map_activations", {}, "GET", CM + "/map/activations")
_seq("route_channel", {"output": "OutA", "channel": 0, "input": "input1", "input_channel": 2},
     [("POST", CM + "/map/activations",
       '{"activation":{"mode":"activate_immediate"},"action":{"OutA":{"0":{"input":"input1","channel_index":2}}}}'),
      ("GET", CM + "/map/active", None)])
_seq("unroute_channel", {"output": "OutA", "channel": 3},
     [("POST", CM + "/map/activations",
       '{"activation":{"mode":"activate_immediate"},"action":{"OutA":{"3":{"input":null,"channel_index":null}}}}'),
      ("GET", CM + "/map/active", None)])
_ACT = {"activation": {"mode": "activate_scheduled_absolute", "requested_time": "1544448739:0"},
        "action": {"OutA": {"0": {"input": "input1", "channel_index": 2}}}}
_seq("activate_map", {"activation": _ACT},
     [("POST", CM + "/map/activations",
       '{"activation":{"mode":"activate_scheduled_absolute","requested_time":"1544448739:0"},'
       '"action":{"OutA":{"0":{"input":"input1","channel_index":2}}}}'),
      ("GET", CM + "/map/activations", None)])
_seq("cancel_map_activation", {"id": "3"},
     [("DELETE", CM + "/map/activations/3", None), ("GET", CM + "/map/activations", None)])

# ── IS-07 Events API ──
_nn("get_event_sources", {}, "GET", EV + "/sources")
_nn("get_event_state", {"id": "1ea39324-a32b-4e1d-86e9-33f9956ebc60"}, "GET",
    EV + "/sources/1ea39324-a32b-4e1d-86e9-33f9956ebc60/state")
_nn("get_event_type", {"id": "1ea39324-a32b-4e1d-86e9-33f9956ebc60"}, "GET",
    EV + "/sources/1ea39324-a32b-4e1d-86e9-33f9956ebc60/type")


# ── Telemetry ──
def _tel(name, path, body, expect_state, **extra):
    text = body if isinstance(body, str) else json.dumps(body, separators=(",", ":"))
    telemetry(NN, name, inbound_http={"path": path, "body": text}, expect_state=expect_state, **extra)


_SENDER = {"id": TX, "version": "1441812152:154331951", "label": "Camera 1", "description": "", "tags": {},
           "flow_id": None, "transport": "urn:x-nmos:transport:rtp.mcast", "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2",
           "manifest_href": "http://172.29.80.65/x-manifest/", "interface_bindings": ["eth0"],
           "subscription": {"receiver_id": None, "active": True}, "caps": {}}
_tel("senders", NODE + "/senders", [_SENDER],
     {"senders": {TX: {"label": "Camera 1", "description": "", "version": "1441812152:154331951", "tags": "{}",
                       "resource": json.dumps(_SENDER, separators=(",", ":")),
                       "flow_id": "", "transport": "urn:x-nmos:transport:rtp.mcast",
                       "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2",
                       "manifest_href": "http://172.29.80.65/x-manifest/", "interface_bindings": '["eth0"]',
                       "subscription": {"receiver_id": "", "active": True}, "caps": "{}"}}},
     expect_then_send=[{"method": "GET", "target": f"{CONN}/senders/{TX}/transportfile"}])
_RECEIVER = {"id": RX, "version": "1441812152:154331951", "label": "Monitor 2", "description": "Rack 4",
             "tags": {"location": ["MCR"]}, "format": "urn:x-nmos:format:video", "transport": "urn:x-nmos:transport:rtp",
             "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "interface_bindings": [],
             "subscription": {"sender_id": TX, "active": True}, "caps": {"media_types": ["video/raw"]}}
_tel("receivers", NODE + "/receivers", [_RECEIVER],
     {"receivers": {RX: {"label": "Monitor 2", "description": "Rack 4", "version": "1441812152:154331951",
                         "tags": '{"location":["MCR"]}', "resource": json.dumps(_RECEIVER, separators=(",", ":")),
                         "format": "urn:x-nmos:format:video", "transport": "urn:x-nmos:transport:rtp",
                         "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "interface_bindings": "[]",
                         "subscription": {"sender_id": TX, "active": True}, "caps": '{"media_types":["video/raw"]}'}}},
     state_before={"receivers": {"gone": {"label": "Old"}}})
_FLOW = {"id": "5fbec3b1-1b0f-417d-9059-8b94a47197ed", "version": "1:2", "label": "Video", "description": "",
         "tags": {}, "format": "urn:x-nmos:format:video", "source_id": "2aa143ac-0ab7-4d75-bc32-5c00c13d186f",
         "device_id": "9126cc2f-4c26-4c9b-a6cd-93c4381c9be5", "parents": [], "media_type": "video/raw",
         "grain_rate": {"numerator": 25, "denominator": 1}, "frame_width": 1920, "frame_height": 1080,
         "interlace_mode": "interlaced_tff", "colorspace": "BT709", "transfer_characteristic": "SDR"}
_tel("flows", NODE + "/flows", [_FLOW],
     {"flows": {_FLOW["id"]: {"label": "Video", "description": "", "version": "1:2", "tags": "{}",
                             "resource": json.dumps(_FLOW, separators=(",", ":")), "format": "urn:x-nmos:format:video",
                             "source_id": "2aa143ac-0ab7-4d75-bc32-5c00c13d186f",
                             "device_id": "9126cc2f-4c26-4c9b-a6cd-93c4381c9be5", "parents": "[]",
                             "media_type": "video/raw", "grain_rate": '{"numerator":25,"denominator":1}',
                             "frame_width": 1920, "frame_height": 1080, "interlace_mode": "interlaced_tff",
                             "colorspace": "BT709", "transfer_characteristic": "SDR"}}})
_DEV = {"id": "9126cc2f-4c26-4c9b-a6cd-93c4381c9be5", "version": "1:3", "label": "Gateway", "description": "",
        "tags": {}, "type": "urn:x-nmos:device:pipeline", "node_id": "3b8be755-08ff-452b-b217-c9151eb21193",
        "senders": [], "receivers": [],
        "controls": [{"href": "http://172.29.80.65:3215/x-nmos/connection/v1.1/", "type": "urn:x-nmos:control:sr-ctrl/v1.1"}]}
_tel("devices", NODE + "/devices", [_DEV],
     {"devices": {_DEV["id"]: {"label": "Gateway", "description": "", "version": "1:3", "tags": "{}",
                               "resource": json.dumps(_DEV, separators=(",", ":")), "type": "urn:x-nmos:device:pipeline",
                               "node_id": "3b8be755-08ff-452b-b217-c9151eb21193",
                               "controls": json.dumps(_DEV["controls"], separators=(",", ":"))}}})
_SRC = {"id": "2aa143ac-0ab7-4d75-bc32-5c00c13d186f", "version": "1:4", "label": "Mic", "description": "",
        "tags": {}, "format": "urn:x-nmos:format:audio", "caps": {}, "device_id": _DEV["id"], "parents": [],
        "clock_name": None, "channels": [{"label": "L"}, {"label": "R"}]}
_tel("sources", NODE + "/sources", [_SRC],
     {"sources": {_SRC["id"]: {"label": "Mic", "description": "", "version": "1:4", "tags": "{}",
                               "resource": json.dumps(_SRC, separators=(",", ":")), "format": "urn:x-nmos:format:audio",
                               "device_id": _DEV["id"], "clock_name": "", "caps": "{}", "parents": "[]",
                               "channels": '[{"label":"L"},{"label":"R"}]'}}})
_NODE = {"id": "3b8be755-08ff-452b-b217-c9151eb21193", "version": "1:5", "label": "host1", "description": "",
         "tags": {}, "href": "http://172.29.80.65:12345/", "hostname": "host1.example.com",
         "api": {"versions": ["v1.2", "v1.3"], "endpoints": [{"host": "172.29.80.65", "port": 12345, "protocol": "http"}]},
         "caps": {}, "services": [], "clocks": [], "interfaces": []}
_tel("self", NODE + "/self", _NODE,
     {"nodes": {_NODE["id"]: {"label": "host1", "description": "", "version": "1:5", "tags": "{}",
                              "resource": json.dumps(_NODE, separators=(",", ":")), "hostname": "host1.example.com",
                              "href": "http://172.29.80.65:12345/", "api_versions": '["v1.2","v1.3"]',
                              "api_endpoints": '[{"host":"172.29.80.65","port":12345,"protocol":"http"}]',
                              "caps": "{}", "services": "[]", "clocks": "[]", "interfaces": "[]"}}})

# IS-05: the list of ids asks for each one's active parameters.
_tel("connection-list", CONN + "/receivers", [RX + "/"], {},
     expect_then_send=[{"method": "GET", "target": f"{CONN}/receivers/{RX}/active"}])
_tel("receiver-active", f"{CONN}/receivers/{RX}/active",
     {"sender_id": TX, "master_enable": True,
      "activation": {"mode": "activate_immediate", "requested_time": "1496739062:0", "activation_time": "1496739062:263900"},
      "transport_file": {"data": SDP, "type": "application/sdp"},
      "transport_params": [{"multicast_ip": "232.250.98.80", "destination_port": 5010, "rtp_enabled": True}]},
     {"connection": {"receivers": {RX: {"active": {
         "sender_id": TX, "master_enable": True, "activation_mode": "activate_immediate",
         "requested_time": "1496739062:0", "activation_time": "1496739062:263900",
         "transport_params": '[{"multicast_ip":"232.250.98.80","destination_port":5010,"rtp_enabled":true}]',
         "transport_file": SDP, "transport_file_type": "application/sdp"}}}}})
# Uninitialized (receiver-active-get-200-uninit.json): every null written as "".
_tel("receiver-active-uninit", f"{CONN}/receivers/{RX}/active",
     {"sender_id": None, "master_enable": False,
      "activation": {"mode": None, "requested_time": None, "activation_time": None},
      "transport_file": {"data": None, "type": None}, "transport_params": [{}]},
     {"connection": {"receivers": {RX: {"active": {
         "sender_id": "", "master_enable": False, "activation_mode": "", "requested_time": "", "activation_time": "",
         "transport_params": "[{}]", "transport_file": "", "transport_file_type": ""}}}}})
_tel("sender-staged", f"{CONN}/senders/{TX}/staged",
     {"receiver_id": None, "master_enable": True,
      "activation": {"mode": "activate_scheduled_absolute", "requested_time": "1496759200:0", "activation_time": None},
      "transport_params": [{"destination_ip": "232.250.98.80"}]},
     {"connection": {"senders": {TX: {"staged": {
         "receiver_id": "", "master_enable": True, "activation_mode": "activate_scheduled_absolute",
         "requested_time": "1496759200:0", "activation_time": "",
         "transport_params": '[{"destination_ip":"232.250.98.80"}]'}}}}})
_tel("sender-transport-file", f"{CONN}/senders/{TX}/transportfile", SDP,
     {"connection": {"senders": {TX: {"transport_file": SDP.strip()}}}})

# IS-08 (io-get-200.json and map-active-get-200.json, shortened).
_tel("io", CM + "/io",
     {"inputs": {"input1": {"parent": {"id": "f6b34356-9885-4db8-bdeb-3141e23f443c", "type": "source"},
                            "channels": [{"label": "outA"}], "caps": {"reordering": False, "block_size": 2},
                            "properties": {"name": "Input 2", "description": "Four channel input 2"}},
                 "input2": {"parent": {"id": None, "type": None}, "channels": [], "caps": {},
                            "properties": {"name": "Spare", "description": ""}}},
      "outputs": {"output1": {"source_id": "bdebfc73-6f43-497a-a9c3-c822ea34a4c9", "channels": [{"label": "outA"}],
                              "caps": {"routable_inputs": ["input1"]},
                              "properties": {"name": "Output 1", "description": "RTP Output One"}},
                  "output2": {"source_id": None, "channels": [], "caps": {"routable_inputs": None},
                              "properties": {"name": "Output 2", "description": ""}}}},
     {"channelmapping": {
         "inputs": {"input1": {"name": "Input 2", "description": "Four channel input 2",
                               "parent_id": "f6b34356-9885-4db8-bdeb-3141e23f443c", "parent_type": "source",
                               "channels": '[{"label":"outA"}]', "caps": '{"reordering":false,"block_size":2}'},
                    "input2": {"name": "Spare", "description": "", "parent_id": "", "parent_type": "",
                               "channels": "[]", "caps": "{}"}},
         "outputs": {"output1": {"name": "Output 1", "description": "RTP Output One",
                                 "source_id": "bdebfc73-6f43-497a-a9c3-c822ea34a4c9", "channels": '[{"label":"outA"}]',
                                 "routable_inputs": '["input1"]', "caps": '{"routable_inputs":["input1"]}'},
                     "output2": {"name": "Output 2", "description": "", "source_id": "", "channels": "[]",
                                 "routable_inputs": "any", "caps": '{"routable_inputs":null}'}}}})
_tel("map-active", CM + "/map/active",
     {"activation": {"mode": None, "requested_time": None, "activation_time": None},
      "map": {"outA": {"0": {"input": "input1", "channel_index": 1}, "1": {"input": None, "channel_index": None}}}},
     {"channelmapping": {"map": {"outA": {"0": {"input": "input1", "input_channel": 1},
                                          "1": {"input": "", "input_channel": -1}}}}},
     state_before={"channelmapping": {"map": {"outB": {"0": {"input": "x", "input_channel": 0}}}}})
_tel("map-activations", CM + "/map/activations/",
     {"3": {"activation": {"mode": "activate_scheduled_absolute", "requested_time": "1544448739:0",
                           "activation_time": "1544448739:0"}, "action": {}}},
     {"channelmapping": {"activations": '{"3":{"activation":{"mode":"activate_scheduled_absolute",'
                                        '"requested_time":"1544448739:0","activation_time":"1544448739:0"},"action":{}}}'}})

# IS-07 (eventsapi-sources-get-200.json, eventsapi-state-*.json).
_E = "f9c7b88b-1846-43d9-9e53-c230e77d91ac"
_tel("event-sources", EV + "/sources", [_E + "/"], {},
     expect_then_send=[{"method": "GET", "target": f"{EV}/sources/{_E}/state"}])
_tel("event-state", f"{EV}/sources/{_E}/state",
     {"identity": {"source_id": _E}, "event_type": "number/temperature/C",
      "timing": {"creation_timestamp": "1532504241:104000200"}, "payload": {"value": 201, "scale": 10},
      "message_type": "state"},
     {"events": {_E: {"event_type": "number/temperature/C", "value": "201", "scale": 10,
                      "payload": '{"value":201,"scale":10}', "creation_timestamp": "1532504241:104000200"}}})
_tel("event-type", f"{EV}/sources/{_E}/type", {"type": "boolean"}, {"events": {_E: {"type": '{"type":"boolean"}'}}})

# IS-07 messages on an event Sender's websocket or MQTT broker, as the
# nmos-event-transports extension offers them (IS-07 v1.0.1, Message types).
def nn_ws(name, message, expect_state, **extra):
    telemetry(NN, name, inbound_ws=json.dumps(message, separators=(",", ":")), expect_state=expect_state, **extra)


nn_ws("is07-state", {"identity": {"source_id": "6cbd0441-7882-44cd-9557-842243a0d618", "flow_id": "ff455cf3-fb17-448b-8f4c-e6c0e294f5ed"},
                     "event_type": "boolean", "timing": {"creation_timestamp": "1531680501:280709600"},
                     "payload": {"value": True}, "message_type": "state"},
      {"events": {"6cbd0441-7882-44cd-9557-842243a0d618": {
          "event_type": "boolean", "value": "true", "payload": "{\"value\":true}",
          "creation_timestamp": "1531680501:280709600", "announced": "none"}}})
nn_ws("is07-number-state", {"identity": {"source_id": "s2"}, "event_type": "number/temperature/C",
                            "timing": {"creation_timestamp": "1:0"}, "payload": {"value": 215, "scale": 10},
                            "message_type": "state"},
      {"events": {"s2": {"event_type": "number/temperature/C", "value": "215", "scale": 10,
                         "payload": "{\"value\":215,\"scale\":10}", "creation_timestamp": "1:0", "announced": "none"}}})
nn_ws("is07-reboot", {"identity": {"source_id": "s2", "flow_id": "f"}, "timing": {"creation_timestamp": "1:0"},
                      "message_type": "reboot"},
      {"events": {"s2": {"announced": "reboot"}}})
nn_ws("is07-connection-status", {"message_type": "connection_status", "active": False,
                                 "sender_id": "772116e0-b4ba-43b1-9ffc-70287c17cb9e"},
      {"event_senders": {"772116e0-b4ba-43b1-9ffc-70287c17cb9e": {"broker_connected": False}}})
