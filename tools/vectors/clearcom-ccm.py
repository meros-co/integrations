CCM = "clearcom-ccm"
# Clear-Com CCM REST API (port 80, JSON), as the MIT-licensed Companion modules
# clearcom-rest (Arcadia, commit 34e908ec) and clearcom-freespeakii (FreeSpeak
# II base, commit 8b822aa4) send it. Device id 1 (the device_id default).


def ccm(command, input, method, target, body=None, model=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
    v = {"spec": CCM, "command": command, "input": input, "expect_request": req, **extra}
    if model:
        v["model"] = model
    V.append(v)


FS = "freespeak-ii"
ccm("get", {"path": "1/devices/1/liveStatus"}, "GET", "/api/1/devices/1/liveStatus",
    http_reply={"status": 200, "body": '{"uptime":3600,"state":"ok"}'},
    expect_result={"ok": {"kind": "value", "value": {"uptime": 3600, "state": "ok"}}})
ccm("put", {"path": "1/devices/1/interfaces/1/ports/65793", "body": {"gid": "p1", "port_label": "PL1"}},
    "PUT", "/api/1/devices/1/interfaces/1/ports/65793", body='{"gid":"p1","port_label":"PL1"}')
ccm("post", {"path": "1/devices/1/endpoints/12/rmk", "body": {}}, "POST", "/api/1/devices/1/endpoints/12/rmk", body="{}")
ccm("delete", {"path": "1/devices/1/gpi/1/events/3"}, "DELETE", "/api/1/devices/1/gpi/1/events/3",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
ccm("get_schema", {}, "GET", "/api/1/schemas/clearcom_api.json")
ccm("get_device", {}, "GET", "/api/1/devices/1")
ccm("get_device_live_status", {}, "GET", "/api/1/devices/1/liveStatus")
ccm("list_endpoints", {}, "GET", "/api/1/devices/endpoints")
ccm("list_endpoints", {}, "GET", "/api/1/devices/1/endpoints", model=FS, file="list_endpoints-freespeak-ii")
ccm("list_roles", {}, "GET", "/api/2/rolesets")
ccm("list_roles", {}, "GET", "/api/1/roles", model=FS, file="list_roles-freespeak-ii")
ccm("list_keysets", {}, "GET", "/api/2/keysets")
ccm("list_connections", {}, "GET", "/api/1/connections")
ccm("get_connections_live_status", {}, "GET", "/api/1/connections/liveStatus")
ccm("list_ports", {}, "GET", "/api/1/devices/interfaces/ports")
ccm("list_interface_ports", {"interface": 1}, "GET", "/api/1/devices/1/interfaces/1/ports", model=FS)
ccm("list_gpio", {}, "GET", "/api/1/devices/1/gpio")
ccm("call_endpoint", {"endpoint": 12, "active": True, "text": "Cue"}, "POST", "/api/1/devices/1/endpoints/12/call",
    body='{"active":true,"text":"Cue"}')
ccm("call_all_endpoints", {"active": False}, "POST", "/api/1/devices/1/endpoints/call", body='{"active":false,"text":""}')
ccm("rmk_endpoint", {"endpoint": 12}, "POST", "/api/1/devices/1/endpoints/12/rmk", body="{}")
ccm("rmk_all_endpoints", {}, "POST", "/api/1/devices/1/endpoints/rmk", body="{}")
ccm("change_role", {"endpoint": 12, "role": 4}, "POST", "/api/1/devices/1/endpoints/12/changerole", body='{"id":4}', model=FS)
ccm("change_association", {"endpoint": 12, "endpoint_gid": "ep-12", "roleset_gid": "rs-4"}, "POST",
    "/api/1/devices/1/endpoints/12/changeassociation", body='{"gid":"ep-12","association":{"gid":"rs-4"}}')
# A port id is device << 16 | interface << 8 | index: 65793 is device 1, interface 1, port 1.
ccm("port_join", {"interface": 1, "port": 65793, "connection": 3}, "POST",
    "/api/1/devices/1/interfaces/1/ports/65793/join", body='{"target":"/api/1/connections/3"}')
ccm("port_leave", {"interface": 1, "port": 65793, "connection": 3}, "POST",
    "/api/1/devices/1/interfaces/1/ports/65793/leave", body='{"target":"/api/1/connections/3"}')
ccm("port_call", {"interface": 1, "port": 65793}, "POST", "/api/1/devices/1/interfaces/1/ports/65793/gpo",
    body='{"enabled":true,"timeout":4000}', model=FS)
ccm("start_port_nulling", {"interface": 1, "port": 65793}, "POST", "/api/1/devices/1/interfaces/1/ports/65793/nulling",
    body="{}", http_reply={"status": 200, "body": '{"ok":true,"nulling":"Nulling"}'},
    expect_result={"ok": {"kind": "value", "value": "Nulling"}})
ccm("get_port_nulling", {"interface": 1, "port": 65793}, "GET", "/api/1/devices/1/interfaces/1/ports/65793/nulling",
    http_reply={"status": 200, "body": '{"ok":true,"nulling":"Idle"}'},
    expect_result={"ok": {"kind": "value", "value": "Idle"}})
ccm("set_gpo", {"gpo": 2, "enabled": True}, "POST", "/api/1/devices/1/setGPO", body='{"id":2,"enabled":true}', model=FS)
ccm("release_gpo", {"gpo": 2}, "POST", "/api/1/devices/1/setGPO", body='{"id":2,"enabled":null}', model=FS)
ccm("set_gpi", {"gpi": 1, "enabled": True}, "POST", "/api/1/devices/1/setGPI", body='{"id":1,"enabled":true}')
ccm("add_gpi_event", {"gpi": 1, "source": "/api/1/connections/3", "destination": "/api/1/devices/1/interfaces/1/ports/65793",
                      "action": "xpt"}, "POST", "/api/1/devices/1/gpi/1/events",
    body='{"source":"/api/1/connections/3","destination":"/api/1/devices/1/interfaces/1/ports/65793","action":"xpt"}')
ccm("update_gpi_event", {"gpi": 1, "event": 3, "source": "/api/1/connections/3", "destination": "/api/1/connections/4",
                         "action": "call"}, "PUT", "/api/1/devices/1/gpi/1/events/3",
    body='{"source":"/api/1/connections/3","destination":"/api/1/connections/4","action":"call"}')
ccm("delete_gpi_event", {"gpi": 1, "event": 3}, "DELETE", "/api/1/devices/1/gpi/1/events/3")

ENDPOINT = ('{"id":12,"label":"BP 12","type":"FSII-BP","gid":"ep-12","role":{"id":4,"label":"Camera 4"},'
            '"liveStatus":{"status":"online","role":4,"antennaIndex":2,"keyState":[{"keysetIndex":0,"currentState":"talk+listen","volume":0}],'
            '"batteryType":"Li-Ion","batteryLevel":80,"longevity":{"hours":6,"minutes":30},"RSSI":-60,"linkQuality":5,'
            '"frameErrorRate":0.5,"callState":"none"}}')
ENDPOINT_STATE = {"label": "BP 12", "type": "FSII-BP", "gid": "ep-12", "role_label": "Camera 4", "status": "online",
                  "role": 4, "battery_level": 80, "battery_type": "Li-Ion", "rssi": -60, "link_quality": 5,
                  "frame_error_rate": 0.5, "antenna": 2, "call_state": "none",
                  "key_state": '[{"keysetIndex":0,"currentState":"talk+listen","volume":0}]',
                  "longevity_hours": 6, "longevity_minutes": 30}
telemetry(CCM, "device", inbound_http={"path": "/api/1/devices/1", "body":
          '{"device_id":1,"device_label":"Arcadia","deviceType_name":"NEP-ARCADIA","versionSW":"2.4.0"}'},
          expect_state={"device": {"label": "Arcadia", "type": "NEP-ARCADIA", "version": "2.4.0"}})
telemetry(CCM, "device-live", inbound_http={"path": "/api/1/devices/1/liveStatus", "body":
          '{"uptime":3600,"version":"5.1.0","label":"FSII Base","state":"running"}'},
          expect_state={"device": {"uptime": 3600, "version": "5.1.0", "label": "FSII Base", "state": "running"}})
telemetry(CCM, "endpoints", inbound_http={"path": "/api/1/devices/1/endpoints", "body": "[" + ENDPOINT + "]"},
          state_before={"endpoints": {"99": {"label": "Gone"}}},
          expect_state={"endpoints": {"12": ENDPOINT_STATE}})
telemetry(CCM, "roles", inbound_http={"path": "/api/1/roles", "body": '[{"id":4,"type":"FSII-BP","label":"Camera 4"}]'},
          expect_state={"roles": {"4": {"label": "Camera 4", "type": "FSII-BP"}}})
telemetry(CCM, "rolesets", inbound_http={"path": "/api/2/rolesets", "body": '[{"id":7,"name":"Director","label":"DIR","gid":"rs-7"}]'},
          expect_state={"roles": {"7": {"label": "DIR", "name": "Director", "gid": "rs-7"}}})
telemetry(CCM, "connections", inbound_http={"path": "/api/1/connections", "body": '[{"id":3,"label":"PL 1","type":"partyline"}]'},
          expect_state={"connections": {"3": {"label": "PL 1", "type": "partyline"}}})
telemetry(CCM, "connections-live", inbound_http={"path": "/api/1/connections/liveStatus", "body":
          '[{"id":3,"label":"PL 1","participants":[{"id":12,"label":"BP 12","joinState":"Talk"}]}]'},
          expect_state={"connections": {"3": {"label": "PL 1",
                                               "participants": '[{"id":12,"label":"BP 12","joinState":"Talk"}]'}}})
telemetry(CCM, "ports", inbound_http={"path": "/api/1/devices/1/interfaces/1/ports", "body":
          '[{"port_id":65793,"port_hwIndex":1,"port_label":"2W-1","port_desc":"Stage","port_config_type":"2W",'
          '"liveStatus":{"online":true}}]'},
          expect_state={"ports": {"65793": {"label": "2W-1", "description": "Stage", "config_type": "2W", "online": True}}})
telemetry(CCM, "gpio", inbound_http={"path": "/api/1/devices/1/gpio", "body":
          '[{"id":1,"label":"GPI 1","type":0,"liveStatus":{"status":false}},'
          '{"id":2,"label":"GPO 1","type":1,"liveStatus":{"status":true,"forced":true}}]'},
          expect_state={"gpis": {"1": {"label": "GPI 1", "status": False}},
                        "gpos": {"2": {"label": "GPO 1", "status": True, "forced": True}}})
telemetry(CCM, "endpoint-init", inbound_ws='42["EndpointInit",{"result":[' + ENDPOINT + ']}]',
          expect_state={"endpoints": {"12": ENDPOINT_STATE}})
telemetry(CCM, "endpoint-updated", inbound_ws='42["EndpointUpdated",{"endpointId":12,"path":"liveStatus",'
          '"value":{"status":"offline","role":65535}}]',
          expect_state={"endpoints": {"12": {"status": "offline", "role": 65535}}})
telemetry(CCM, "endpoint-keys", inbound_ws='42["EndpointUpdated",{"endpointId":12,"path":"liveStatus.keyState",'
          '"value":[{"keysetIndex":1,"currentState":"listen","volume":2}]}]',
          expect_state={"endpoints": {"12": {"key_state": '[{"keysetIndex":1,"currentState":"listen","volume":2}]'}}})
telemetry(CCM, "endpoint-updated-list", inbound_ws='42["EndpointUpdated",[{"endpointId":5,"path":"liveStatus.callState",'
          '"value":"calling"},{"endpointId":6,"path":"liveStatus","value":{"status":"online","batteryLevel":40,"rssi":-70}}]]',
          expect_state={"endpoints": {"5": {"call_state": "calling"},
                                      "6": {"status": "online", "battery_level": 40, "rssi": -70}}})
telemetry(CCM, "gpi-init", inbound_ws='42["GpiInit",{"result":[{"id":1,"label":"GPI 1","liveStatus":{"status":true}}]}]',
          expect_state={"gpis": {"1": {"label": "GPI 1", "status": True}}})
telemetry(CCM, "gpo-updated", inbound_ws='42["GpoUpdated",{"result":{"id":2,"liveStatus":{"status":false,"forced":false}}}]',
          expect_state={"gpos": {"2": {"status": False, "forced": False}}})
telemetry(CCM, "gpi-updated-arcadia", inbound_ws='42["GpiUpdated",{"gpiId":1,"path":"liveStatus","value":{"status":false}}]',
          expect_state={"gpis": {"1": {"status": False}}})
telemetry(CCM, "live-connections", inbound_ws='42["live:connections",{"updated":true}]',
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/api/1/connections/liveStatus"}])
telemetry(CCM, "live-devices", inbound_ws='42["live:devices",{"updated":true}]',
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/api/1/devices/1"}, {"method": "GET", "target": "/api/1/devices/1/liveStatus"}])
