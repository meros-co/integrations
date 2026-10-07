C70 = "sennheiser-chg-70n"
# Sennheiser SSC (v1) on the CHG 70N: one JSON message per UDP datagram to
# port 45, answered to the sender with the method's value, or with an
# /osc/error entry. Stated from the SSC developer's guide for EW-DX
# (firmware 3.0.x), §5, §6 and §12.
OK = {"ok": {"kind": "ack"}}


def t(*args, device_reply=None, **extra):
    """A command whose answer is one datagram."""
    if device_reply is not None:
        extra["device_reply_hex"] = device_reply.encode().hex()
    text(*args, **extra)


def datagram(name, inbound, **extra):
    """A datagram from the charger and the state it must produce."""
    telemetry(C70, name, inbound_hex=inbound.encode().hex(), **extra)


ERR = {"error": {"error": "device_error"}}

t(C70, "identify_bay", {"bay": 1}, '{"bays":{"identify":[true,null]}}',
     device_reply='{"bays":{"identify":[true,false]}}', expect_result=OK)
t(C70, "identify_bay", {"bay": 2, "enabled": False}, '{"bays":{"identify":[null,false]}}',
     device_reply='{"osc":{"error":[{"bays":{"identify":[423]}}]}}', expect_result=ERR)
t(C70, "identify_device", {}, '{"device":{"identification":{"visual":true}}}',
     device_reply='{"device":{"identification":{"visual":true}}}', expect_result=OK)
t(C70, "set_storage_mode", {"enabled": True}, '{"device":{"storage_mode":true}}',
     device_reply='{"device":{"storage_mode":true}}', expect_result=OK)
t(C70, "set_device_name", {"name": "CHG-1"}, '{"device":{"name":"CHG-1"}}',
     device_reply='{"device":{"name":"CHG-1"}}', expect_result=OK)
t(C70, "set_location", {"location": "Rack \"B\""}, '{"device":{"location":"Rack \\"B\\""}}',
     device_reply='{"device":{"location":"Rack \\"B\\""}}', expect_result=OK)
t(C70, "set_network_dhcp", {}, '{"device":{"network":{"ipv4":{"auto":true}}}}',
     device_reply='{"device":{"network":{"ipv4":{"auto":true}}}}', expect_result=OK)
t(C70, "set_network_manual", {"ip": "192.168.1.30", "netmask": "255.255.255.0", "gateway": "192.168.1.1"},
     '{"device":{"network":{"ipv4":{"auto":false,"manual_ipaddr":"192.168.1.30","manual_netmask":"255.255.255.0","manual_gateway":"192.168.1.1"}}}}',
     device_reply='{"device":{"network":{"ipv4":{"auto":false,"manual_ipaddr":"192.168.1.30","manual_netmask":"255.255.255.0","manual_gateway":"192.168.1.1"}}}}',
     expect_result=OK)
t(C70, "set_mdns", {"enabled": False}, '{"device":{"network":{"mdns":false}}}',
     device_reply='{"device":{"network":{"mdns":false}}}', expect_result=OK)
t(C70, "sync_transmitters", {"settings": [{"bay_id": 0, "name": "LEAD", "lock": True}]},
     '{"bays":{"sync_settings":[{"bay_id":0,"name":"LEAD","lock":true}]}}',
     device_reply='{"bays":{"sync_settings":[{"bay_id":0,"name":"LEAD","lock":true}]}}', expect_result=OK)
t(C70, "restart", {}, '{"device":{"restart":true}}', expect_result={"ok": {"kind": "unverified"}})
t(C70, "restore_factory_defaults", {}, '{"device":{"restore":"FACTORY_DEFAULTS"}}',
     device_reply='{"device":{"restore":"FACTORY_DEFAULTS"}}', expect_result=OK)

datagram("bays",
          '{"bays":{"state":["NORMAL","DFU_MODE"],"device_type":["BA70","NONE"],'
                  '"identify":[false,true],"bat_gauge":[80,null],"bat_health":[97,null],'
                  '"bat_cycles":[41,null],"bat_timetofull":[35,null],'
                  '"warnings":["","BatteryComError"],"version":["1.6.22",""],'
                  '"serial":["123456",""],"sync_error":["","SyncFailed"]}}',
          expect_state={"bays": {
              "1": {"state": "normal", "device_type": "BA70", "identifying": False,
                    "battery_percent": 80, "battery_health_pct": 97, "battery_cycles": 41,
                    "time_to_full_min": 35, "warning": "", "version": "1.6.22",
                    "serial": "123456", "sync_error": ""},
              "2": {"state": "dfu_mode", "device_type": "NONE", "identifying": True,
                    "warning": "BatteryComError", "version": "", "serial": "",
                    "sync_error": "SyncFailed"}}})
datagram("bay-update",
          '{"bays":{"update":{"progress":[100,40],"error":["NONE","Invalid firmware"],"enable":[false,true]}}}',
          expect_state={"bays": {
              "1": {"update": {"progress_pct": 100, "error": "NONE", "enabled": False}},
              "2": {"update": {"progress_pct": 40, "error": "Invalid firmware", "enabled": True}}}})
datagram("device",
          '{"device":{"name":"CHG70N","location":"FOH rack","identification":{"visual":false},'
                  '"storage_mode":false,"warnings":"CascadeComError","cascade":["00:1B:66:00:00:01"],'
                  '"identity":{"version":"3.0.4","serial":"5120000123","product":"CHG 70N-C",'
                  '"vendor":"Sennheiser electronic SE & Co. KG"}}}',
          expect_state={"device": {
              "name": "CHG70N", "location": "FOH rack", "identifying": False, "storage_mode": False,
              "warnings": "CascadeComError", "cascade": '["00:1B:66:00:00:01"]', "version": "3.0.4",
              "serial": "5120000123", "product": "CHG 70N-C",
              "vendor": "Sennheiser electronic SE & Co. KG"}})
datagram("network",
          '{"device":{"network":{"ipv4":{"auto":false,"ipaddr":["192.168.1.30"],'
                  '"netmask":"255.255.255.0","gateway":["192.168.1.1"],"manual_ipaddr":"192.168.1.30",'
                  '"manual_netmask":"255.255.255.0","manual_gateway":"192.168.1.1"},"mdns":true,'
                  '"ether":{"macs":["00:1B:66:00:00:01"]}}}}',
          expect_state={"device": {"network": {
              "auto": False, "ip": "192.168.1.30", "netmask": "255.255.255.0",
              "gateway": "192.168.1.1", "manual_ip": "192.168.1.30",
              "manual_netmask": "255.255.255.0", "manual_gateway": "192.168.1.1",
              "mdns": True, "mac": "00:1B:66:00:00:01"}}})
