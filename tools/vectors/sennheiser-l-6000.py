L60 = "sennheiser-l-6000"
# Sennheiser SSC (v1) on the L 6000: one JSON message per UDP datagram to
# port 45, answered to the sender with the method's value, or with an
# /osc/error entry. Stated from the SSC developer's guide for Digital 6000
# (TI 1109 v2.2), §4.3, §5, §6.1 and §10-12.
OK = {"ok": {"kind": "ack"}}
ERR = {"error": {"error": "device_error"}}


def t(*args, device_reply=None, **extra):
    """A command whose answer is one datagram."""
    if device_reply is not None:
        extra["device_reply_hex"] = device_reply.encode().hex()
    text(*args, **extra)


def datagram(name, inbound, **extra):
    """A datagram from the charger and the state it must produce."""
    telemetry(L60, name, inbound_hex=inbound.encode().hex(), **extra)


t(L60, "identify_bay", {"slot": 1, "bay": 1}, '{"slot1":{"subslot1":{"identify":null}}}',
  device_reply='{"slot1":{"subslot1":{"identify":true}}}', expect_result=OK)
t(L60, "identify_bay", {"slot": 4, "bay": 2}, '{"slot4":{"subslot2":{"identify":null}}}',
  device_reply='{"osc":{"error":[{"slot4":{"subslot2":{"identify":[404,{"desc":"address not found"}]}}}]}}',
  expect_result=ERR, file="identify_bay-error")
t(L60, "identify_device", {}, '{"device":{"identify":null}}',
  device_reply='{"device":{"identify":true}}', expect_result=OK)
t(L60, "set_storage_mode", {"enabled": True}, '{"device":{"storage_mode":true}}',
  device_reply='{"device":{"storage_mode":true}}', expect_result=OK)
t(L60, "set_device_name", {"name": "L6000-RACK"}, '{"device":{"name":"L6000-RACK"}}',
  device_reply='{"device":{"name":"L6000-RACK-001b66000001"}}', expect_result=OK)
t(L60, "set_network_dhcp", {}, '{"device":{"network":{"ipv4":{"auto":true}}}}',
  device_reply='{"device":{"network":{"ipv4":{"auto":true}}}}', expect_result=OK)
t(L60, "set_network_manual", {"ip": "192.168.1.40", "netmask": "255.255.255.0", "gateway": "192.168.1.1"},
  '{"device":{"network":{"ipv4":{"auto":false,"static_ipaddr":"192.168.1.40","static_netmask":"255.255.255.0","static_gateway":"192.168.1.1"}}}}',
  device_reply='{"device":{"network":{"ipv4":{"auto":false,"static_ipaddr":"192.168.1.40","static_netmask":"255.255.255.0","static_gateway":"192.168.1.1"}}}}',
  expect_result=OK)
t(L60, "set_mdns", {"enabled": False}, '{"device":{"network":{"ipv4":{"mdns":false}}}}',
  device_reply='{"device":{"network":{"ipv4":{"mdns":false}}}}', expect_result=OK)

datagram("slots",
         '{"slot1":{"type":1,"subslot1":{"led":"GREEN","accu_detection":true},'
         '"subslot2":{"led":"OFF","accu_detection":false}},'
         '"slot2":{"type":2,"subslot1":{"led":"YELLOW_GREEN_FLASHING","accu_detection":true},'
         '"subslot2":{"led":"DEV_IDENTIFY","accu_detection":true}},'
         '"slot3":{"type":0},"slot4":{"type":3,"subslot1":{"led":"RED_FLASHING"}}}',
         expect_state={"slots": {
             "1": {"type": "lm6060", "type_code": 1,
                   "bays": {"1": {"led": "green", "battery_detected": True},
                            "2": {"led": "off", "battery_detected": False}}},
             "2": {"type": "lm6061", "type_code": 2,
                   "bays": {"1": {"led": "yellow_green_flashing", "battery_detected": True},
                            "2": {"led": "identify", "battery_detected": True}}},
             "3": {"type": "none", "type_code": 0},
             "4": {"type": "unknown_fault", "type_code": 3,
                   "bays": {"1": {"led": "red_flashing"}}}}})
# A slot type the guide does not list keeps its number only.
datagram("slot-type-unlisted", '{"slot2":{"type":5}}',
         expect_state={"slots": {"2": {"type_code": 5}}})
# The guide's own accu_parameter example (§11.3), and one for slot 4 bay 2.
datagram("accu-parameter",
         '{"slot1":{"subslot1":{"accu_parameter":[24.0,3939,1459,0,5235,4,0,69,0,100,340,30]}},'
         '"slot4":{"subslot2":{"accu_parameter":[31.5,4105,2300,850,8410,7,12,88,42,97,0,25]}}}',
         expect_state={"slots": {
             "1": {"bays": {"1": {
                 "temperature_c": 24.0, "voltage_mv": 3939, "capacity_mah": 1459, "current_ma": 0,
                 "energy_mwh": 5235, "operating_time_h": 4, "operating_time_min": 0,
                 "battery_percent": 69, "cycles": 0, "health_pct": 100,
                 "time_to_full_h": 340, "time_to_full_min": 30}}},
             "4": {"bays": {"2": {
                 "temperature_c": 31.5, "voltage_mv": 4105, "capacity_mah": 2300, "current_ma": 850,
                 "energy_mwh": 8410, "operating_time_h": 7, "operating_time_min": 12,
                 "battery_percent": 88, "cycles": 42, "health_pct": 97,
                 "time_to_full_h": 0, "time_to_full_min": 25}}}}})
datagram("warnings", '{"device":{"warnings":[3,5,24,28,36]}}',
         expect_state={"device": {
             "warning_codes": "[3,5,24,28,36]",
             "warnings": {"0": "slot2_bay2_too_hot", "1": "slot3_bay2_too_hot", "2": "fan1_defect",
                          "3": "device_too_hot", "4": "slot4_bay2_too_cold"}}})
datagram("warnings-cleared", '{"device":{"warnings":[]}}',
         state_before={"device": {"warning_codes": "[16]", "warnings": {"0": "slot1_bay1_defect"}}},
         expect_state={"device": {"warning_codes": "[]"}})
datagram("device",
         '{"device":{"name":"L6000-001b66000001","storage_mode":false,"language":["en_GB"],'
         '"identity":{"version":"3.0.2","vendor":"Sennheiser electronic GmbH & Co. KG","product":"L6000"}}}',
         expect_state={"device": {
             "name": "L6000-001b66000001", "storage_mode": False, "language": '["en_GB"]',
             "version": "3.0.2", "vendor": "Sennheiser electronic GmbH & Co. KG", "product": "L6000"}})
datagram("network",
         '{"device":{"network":{"ether":{"macs":["00:1b:66:00:00:01"],"interfaces":["CONTROL"]},'
         '"ipv4":{"auto":false,"mdns":true,"interfaces":[0],"static_ipaddr":"192.168.1.40",'
         '"static_netmask":"255.255.255.0","static_gateway":"192.168.1.1","ipaddr":"192.168.1.40",'
         '"netmask":"255.255.255.0","gateway":"192.168.1.1"}}}}',
         expect_state={"device": {"network": {
             "mac": "00:1b:66:00:00:01", "interfaces": '["CONTROL"]', "ipv4_interfaces": "[0]",
             "auto": False, "mdns": True, "manual_ip": "192.168.1.40",
             "manual_netmask": "255.255.255.0", "manual_gateway": "192.168.1.1",
             "ip": "192.168.1.40", "netmask": "255.255.255.0", "gateway": "192.168.1.1"}}})
