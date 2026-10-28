# AMWA NMOS registry: the IS-04 v1.3 Query API (AMWA-TV/is-04 v1.3.3,
# APIs/QueryAPI.raml and the subscription schemas). State arrives through the
# nmos-query-websocket extension, which offers each subscription grain to the
# rules as {"nmos": <type>, "op": "sync" | "set", "items": [...]} or
# {"nmos": <type>, "op": "delete", "ids": [...]}; the telemetry vectors give
# those messages. A registry has no standard port: vectors open it on the
# runner's stand-in port.
NR = "nmos-registry"
Q = "/x-nmos/query/v1.3"
TX = "5709255c-c0ae-4e1e-99a0-e872e83e48e0"
SUB = "6a2b4a4b-4c35-4f6c-9d5e-2e1f4c0b7f11"


def _nr(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": NR, "command": command, "input": input, "expect_request": req, **extra})


for t in ["nodes", "devices", "sources", "flows", "senders", "receivers"]:
    _nr(f"get_{t}", {}, "GET", f"{Q}/{t}")
    _nr(f"get_{t[:-1]}", {"id": TX}, "GET", f"{Q}/{t}/{TX}")
_nr("get_senders", {}, "GET", f"{Q}/senders", file="get_senders.reply",
    http_reply={"status": 200, "body": '[{"id":"' + TX + '","label":"Camera 1"}]'},
    expect_result={"ok": {"kind": "value", "value": [{"id": TX, "label": "Camera 1"}]}})
_nr("query_resources", {"type": "senders", "query": "label=Camera%201&paging.limit=100"}, "GET",
    f"{Q}/senders?label=Camera%201&paging.limit=100")
_nr("list_subscriptions", {}, "GET", f"{Q}/subscriptions")
_nr("get_subscription", {"id": SUB}, "GET", f"{Q}/subscriptions/{SUB}")
_nr("create_subscription", {"resource_path": "/senders", "persist": True}, "POST", f"{Q}/subscriptions",
    '{"max_update_rate_ms":100,"resource_path":"/senders","params":{},"persist":true,"secure":false}',
    http_reply={"status": 201, "body": '{"id":"' + SUB + '","ws_href":"ws://172.29.80.52:8870/x-nmos/query/v1.3/subscriptions/'
                + SUB + '","max_update_rate_ms":100,"persist":true,"secure":false,"resource_path":"/senders","params":{}}'},
    expect_result={"ok": {"kind": "value", "value": {
        "id": SUB, "ws_href": f"ws://172.29.80.52:8870/x-nmos/query/v1.3/subscriptions/{SUB}",
        "max_update_rate_ms": 100, "persist": True, "secure": False, "resource_path": "/senders", "params": {}}}})
_nr("delete_subscription", {"id": SUB}, "DELETE", f"{Q}/subscriptions/{SUB}",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_nr("delete_subscription", {"id": SUB}, "DELETE", f"{Q}/subscriptions/{SUB}", file="delete_subscription.refused",
    http_reply={"status": 403, "body": '{"code":403,"error":"non-persistent","debug":null}'},
    expect_result={"error": {"error": "device_error", "code": "403"}})


# ── Telemetry: the extension's messages ──
def _ws(name, message, expect_state, **extra):
    telemetry(NR, name, inbound_ws=json.dumps(message, separators=(",", ":")), expect_state=expect_state, **extra)


_SENDER = {"id": TX, "version": "1441812152:154331951", "label": "Camera 1", "description": "", "tags": {},
           "flow_id": "5fbec3b1-1b0f-417d-9059-8b94a47197ed", "transport": "urn:x-nmos:transport:rtp.mcast",
           "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "manifest_href": None, "interface_bindings": [],
           "subscription": {"receiver_id": None, "active": False}, "caps": {}}
_SENDER_STATE = {"label": "Camera 1", "description": "", "version": "1441812152:154331951", "tags": "{}",
                 "resource": json.dumps(_SENDER, separators=(",", ":")),
                 "flow_id": "5fbec3b1-1b0f-417d-9059-8b94a47197ed", "transport": "urn:x-nmos:transport:rtp.mcast",
                 "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "manifest_href": "",
                 "interface_bindings": "[]", "subscription": {"receiver_id": "", "active": False}, "caps": "{}"}
# The sync on (re)connecting replaces the whole list.
_ws("sync", {"nmos": "senders", "op": "sync", "items": [_SENDER]}, {"senders": {TX: _SENDER_STATE}},
    state_before={"senders": {"9d3a1c20-1111-4c4c-8d8d-000000000001": {"label": "Gone while away"}}})
_ws("set", {"nmos": "senders", "op": "set", "items": [dict(_SENDER, label="Camera 1 ISO")]},
    {"senders": {TX: dict(_SENDER_STATE, label="Camera 1 ISO",
                          resource=json.dumps(dict(_SENDER, label="Camera 1 ISO"), separators=(",", ":")))}})
_ws("delete", {"nmos": "senders", "op": "delete", "ids": [TX]}, {"senders": {}},
    state_before={"senders": {TX: {"label": "Camera 1"}}})
_NODE = {"id": "3b8be755-08ff-452b-b217-c9151eb21193", "version": "1:5", "label": "host1", "description": "",
         "tags": {}, "href": "http://172.29.80.65:12345/", "hostname": "host1.example.com",
         "api": {"versions": ["v1.3"], "endpoints": [{"host": "172.29.80.65", "port": 12345, "protocol": "http"}]},
         "caps": {}, "services": [], "clocks": [{"name": "clk0", "ref_type": "internal"}], "interfaces": []}
_ws("nodes", {"nmos": "nodes", "op": "set", "items": [_NODE]},
    {"nodes": {_NODE["id"]: {"label": "host1", "description": "", "version": "1:5", "tags": "{}",
                             "resource": json.dumps(_NODE, separators=(",", ":")), "hostname": "host1.example.com",
                             "href": "http://172.29.80.65:12345/", "api_versions": '["v1.3"]',
                             "api_endpoints": '[{"host":"172.29.80.65","port":12345,"protocol":"http"}]',
                             "caps": "{}", "services": "[]", "clocks": '[{"name":"clk0","ref_type":"internal"}]',
                             "interfaces": "[]"}}})
_RX = {"id": "0a174530-e3cf-11e6-bf01-fe55135034f3", "version": "1:6", "label": "Monitor", "description": "",
       "tags": {}, "format": "urn:x-nmos:format:audio", "transport": "urn:x-nmos:transport:rtp",
       "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "interface_bindings": ["eth0"],
       "subscription": {"sender_id": TX, "active": True}, "caps": {"media_types": ["audio/L24"]}}
_ws("receivers", {"nmos": "receivers", "op": "set", "items": [_RX]},
    {"receivers": {_RX["id"]: {"label": "Monitor", "description": "", "version": "1:6", "tags": "{}",
                               "resource": json.dumps(_RX, separators=(",", ":")), "format": "urn:x-nmos:format:audio",
                               "transport": "urn:x-nmos:transport:rtp",
                               "device_id": "c501ae9a-7e5d-4a6f-9f0d-c1b4b1b8e7f2", "interface_bindings": '["eth0"]',
                               "subscription": {"sender_id": TX, "active": True},
                               "caps": '{"media_types":["audio/L24"]}'}}})
