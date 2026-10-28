# Adder XDIP receivers (adder-xdip): one vector per command over the public
# REST API (public-rest-api.yaml 1.05), base path /api on HTTPS 8443. The
# Access password is left empty, so no login goes first; the login and its
# Bearer token are covered by one vector with a password and by the engine's
# session tests. Replies follow the definition's schemas. A command of two
# requests is compared on its first, which the HTTP vector runner sends.
XD = "adder-xdip"
_XD_NO_LOGIN = {"settings": {"password": ""}}


def _xd(command, input, method, target, **extra):
    extra = {**_XD_NO_LOGIN, **extra}
    http(XD, command, input, method, target, **extra)


_xd("get_channels", {}, "GET", "/api/channels",
    http_reply={"status": 200, "body": '[{"id":1,"nodes":[{"nodeUuid":"0f6d"}]}]'},
    expect_result={"ok": {"kind": "value", "value": [{"id": 1, "nodes": [{"nodeUuid": "0f6d"}]}]}})
_xd("get_channel", {"id": 3}, "GET", "/api/channels/3")
_xd("get_connected_channel", {}, "GET", "/api/channels/connected",
    http_reply={"status": 200, "body": '{"id":2147483647}'},
    expect_result={"ok": {"kind": "value", "value": 2147483647}})
_xd("get_channel_status", {}, "GET", "/api/channels/status")
_xd("get_max_channels", {}, "GET", "/api/channels/maximumAllowed",
    http_reply={"status": 200, "body": '{"count":16}'}, expect_result={"ok": {"kind": "value", "value": 16}})
V.append({"spec": XD, "command": "switch_channel", "input": {"id": 4}, **_XD_NO_LOGIN,
          "expect_request": {"method": "POST", "target": "/api/channels/4/switch"}})
# With the Access password: the login goes first, its body the password.
V.append({"spec": XD, "command": "switch_channel", "input": {"id": 4}, "settings": {"password": "secret"},
          "file": "switch_channel-login",
          "expect_request": [{"method": "POST", "target": "/api/nodes/self/access",
                              "body": '{"accessPassword":"secret"}', "content_type": "application/json"}]})
V.append({"spec": XD, "command": "disconnect", "input": {}, **_XD_NO_LOGIN,
          "expect_request": {"method": "POST", "target": "/api/channels/2147483647/switch"}})
_xd("get_self", {}, "GET", "/api/nodes/self")
_xd("get_node", {"uuid": "0f6d1c2e-1111-2222-3333-444455556666"}, "GET", "/api/nodes/0f6d1c2e-1111-2222-3333-444455556666")
_xd("get_selected_nodes", {}, "GET", "/api/nodes/selected")
_xd("get_local_computer", {}, "GET", "/api/localComputer")

telemetry(XD, "connected", **_XD_NO_LOGIN, inbound_http={"path": "/api/channels/connected", "body": '{"id":5}'},
          expect_state={"channel": {"connected": 5}})
telemetry(XD, "status", **_XD_NO_LOGIN, inbound_http={"path": "/api/channels/status", "body": json.dumps(
    {"summary": "noVideo", "connections": [{"id": "video1", "state": "connected", "status": "noInput"}]})},
    expect_state={"channel": {"status": "noVideo", "connections": {"0": {"id": "video1", "state": "connected",
                                                                         "status": "noInput"}}}})
telemetry(XD, "channels", **_XD_NO_LOGIN, inbound_http={"path": "/api/channels", "body": json.dumps(
    [{"id": 1, "nodes": [{"nodeUuid": "aa"}]}, {"id": 2, "nodes": [{"nodeUuid": "bb"}]}])},
    state_before={"channels": {"9": {"node": "old"}}},
    expect_state={"channels": {"1": {"node": "aa"}, "2": {"node": "bb"}}})
telemetry(XD, "self", **_XD_NO_LOGIN, inbound_http={"path": "/api/nodes/self", "body": json.dumps(
    {"uuid": "cc", "name": "Desk 4", "description": "Gallery", "type": "receiver", "active": True,
     "serialNumber": "2412A0449220", "macAddress": "00:0F:58:0B:D4:D8"})},
    expect_state={"receiver": {"uuid": "cc", "name": "Desk 4", "description": "Gallery", "active": True,
                               "serial_number": "2412A0449220", "mac_address": "00:0F:58:0B:D4:D8"}})
telemetry(XD, "selected", **_XD_NO_LOGIN, inbound_http={"path": "/api/nodes/selected", "body": json.dumps(
    [{"uuid": "aa", "name": "Server 1", "description": "", "type": "transmitter", "active": False,
      "serialNumber": "1", "macAddress": "00:0F:58:00:00:01"}])},
    expect_state={"nodes": {"aa": {"name": "Server 1", "description": "", "type": "transmitter", "active": False,
                                   "serial_number": "1", "mac_address": "00:0F:58:00:00:01"}}})
telemetry(XD, "computer", **_XD_NO_LOGIN, inbound_http={"path": "/api/localComputer",
                                                        "body": '{"name":"Edit 2","description":"Suite B"}'},
          expect_state={"local_computer": {"name": "Edit 2", "description": "Suite B"}})
telemetry(XD, "max", **_XD_NO_LOGIN, inbound_http={"path": "/api/channels/maximumAllowed", "body": '{"count":16}'},
          expect_state={"max_channels": 16})
