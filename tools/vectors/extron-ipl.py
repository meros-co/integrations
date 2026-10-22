K = "extron-ipl"
# Extron IP Link control interfaces (IPL 250 Reference Manual 68-1715-01
# Rev. A, IPL T S Series User Guide 68-1218-01 Rev. G). Every command is sent
# followed by CR; Esc is 0x1B. Ports are sent as two digits. Replies are the
# verbose mode 3 (tagged) forms, or the bare forms tied to their query.
E = "\x1b"
P2 = {"model": "ipl-250"}
TS = {"model": "ipl-t-s"}
ACK = {"ok": {"kind": "ack"}}
ERR = {"error": {"error": "device_error"}}


def val(v):
    return {"ok": {"kind": "value", "value": v}}


_seen = {}


def t(command, *args, **extra):
    # A second vector of one command gets a file of its own.
    n = _seen.get(command, 0) + 1
    _seen[command] = n
    if n > 1:
        extra["file"] = f"{command}-{n}"
    text(K, command, *args, **extra)


# Serial ports (IPL 250 p.4-8 to 4-10, IPL T S p.28-30)
t("send_serial_data", {"port": 2, "data": "PWR ON"}, E + "02RS\rPWR ON\r", **P2)
t("send_serial_data_timed", {"port": 1, "first_wait": 4, "between_wait": 7, "end": "3L", "data": "?VOL"},
  E + "01*4*7*3LRS\r?VOL\r", **TS)
t("set_serial_port", {"port": 2, "baud": "9600", "parity": "N", "data_bits": "8", "stop_bits": "1"},
  E + "02*9600,N,8,1CP\r", device_reply="Cpn02 Ccp9600,N,8,1\r\n", expect_result=ACK, **P2)
t("get_serial_port", {"port": 4}, E + "04CP\r", device_reply="Cpn04 Ccp38400,E,7,2\r\n",
  expect_result=val("38400,E,7,2"), **TS)
t("set_serial_port_type", {"port": 3, "type": 2}, E + "03*2CY\r", device_reply="Cpn03 Cty2\r\n", expect_result=ACK, **TS)
t("set_serial_port_type", {"port": 1, "type": 1}, E + "01*1CY\r", device_reply="E13\r\n", expect_result=ERR, **P2)
t("get_serial_port_type", {"port": 1}, E + "01CY\r", device_reply="0\r\n", expect_result=val("0"), **P2)
t("set_serial_flow_control", {"port": 1, "flow": "H", "pacing": 20}, E + "01*H,20CF\r",
  device_reply="Cpn01 CflH,0020\r\n", expect_result=ACK, **TS)
t("get_serial_flow_control", {"port": 1}, E + "01CF\r", device_reply="N,0000\r\n", expect_result=val("N,0000"), **P2)
t("set_serial_receive_timeout", {"port": 2, "first_wait": 10, "between_wait": 2, "priority": 1, "end": "13D"},
  E + "02*10*2*1*13DCE\r", device_reply="Cpn02 Cce00010,00002,1,00013D\r\n", expect_result=ACK, **P2)
t("get_serial_receive_timeout", {"port": 2}, E + "02CE\r", device_reply="Cpn02 Cce00010,00002,0,00000L\r\n",
  expect_result=val("00010,00002,0,00000L"), **P2)
t("set_serial_passthrough", {"port": 2, "redirect": 1, "timeout": 10, "end": "0L"}, E + "02*1*10*0LCD\r",
  device_reply="Cpn02 Ccd00001,00010,00000L\r\n", expect_result=ACK, **TS)
t("end_serial_passthrough", {"port": 2}, E + "02*0CD\r", device_reply="Cpn02 Ccd00000,00000,00000L\r\n",
  expect_result=ACK, **TS)
t("end_serial_passthrough", {"port": 3}, E + "03*0CD\r", device_reply="E31\r\n", expect_result=ERR, **P2)
t("get_serial_passthrough", {"port": 2}, E + "02CD\r", device_reply="00001,00010,00000L\r\n",
  expect_result=val("00001,00010,00000L"), **TS)

# IR, relays and contact inputs (IPL 250 p.4-11 to 4-12)
t("play_ir", {"port": 1, "file": 3, "function": 1}, E + "01,3,1,0IR\r", device_reply="Irs01,03,001,00\r\n",
  expect_result=ACK, **P2)
t("play_ir", {"port": 4, "file": 12, "function": 25, "mode": "continuous"}, E + "04,12,25,1IR\r",
  device_reply="Irs04,12,025,01\r\n", expect_result=ACK, **P2)
t("play_ir", {"port": 2, "file": 3, "function": 9}, E + "02,3,9,0IR\r", device_reply="E28\r\n", expect_result=ERR, **P2)
t("set_relay", {"relay": 2, "state": "closed"}, "02*1O\r", device_reply="Cpn02 Rly1\r\n", expect_result=ACK, **P2)
t("set_relay", {"relay": 3, "state": "toggle"}, "03*2O\r", device_reply="Cpn03 Rly0\r\n", expect_result=ACK, **P2)
t("pulse_relay", {"relay": 1, "ticks": 25}, "01*3*25O\r", device_reply="Cpn01 Rly1\r\n", expect_result=ACK, **P2)
t("get_relay", {"relay": 4}, "04O\r", device_reply="1\r\n", expect_result=val("1"), **P2)
t("get_contact_input", {"input": 3}, "03]\r", device_reply="0\r\n", expect_result=val("0"), **P2)

# Information (IPL 250 p.4-12 to 4-14, IPL T S p.31-32)
t("get_firmware", {}, "Q\r", device_reply="Ver01*1.15\r\n", expect_result=val("1.15"), **P2)
t("get_firmware_versions", {}, "0Q\r",
  device_reply="Ver00*2.20-1.14(1.77-IPL Series -Wed, 16 Jan 2003 00:00:00 GMT)-1.15*(1.78-IPL Series -Fri, 17 Jul 2009"
               " 21:47:29 GMT)\r\n",
  expect_result=val("2.20-1.14(1.77-IPL Series -Wed, 16 Jan 2003 00:00:00 GMT)-1.15*(1.78-IPL Series -Fri, 17 Jul 2009"
                    " 21:47:29 GMT)"), **P2)
t("get_part_number", {}, "N\r", device_reply="Pno60-1026-81\r\n", expect_result=val("60-1026-81"), **P2)
t("get_model_name", {}, "1I\r", device_reply="Inf01*IPL T S4\r\n", expect_result=val("IPL T S4"), **TS)
t("get_model_description", {}, "2I\r", device_reply="Inf02*Four Bi-Directional Serial Ports [RS232/422/485]\r\n",
  expect_result=val("Four Bi-Directional Serial Ports [RS232/422/485]"), **TS)
t("get_system_memory", {}, "3I\r", device_reply="Inf03*12800 Bytes Used out of 1024 KBytes\r\n",
  expect_result=val("12800 Bytes Used out of 1024 KBytes"), **P2)
t("get_user_memory", {}, "4I\r", device_reply="Inf04*217856 Bytes Used out of 7232 KBytes\r\n",
  expect_result=val("217856 Bytes Used out of 7232 KBytes"), **TS)

# Network, time and verbose mode (IPL 250 p.4-14 to 4-16)
t("set_unit_name", {"name": "Rm316-AVcenter"}, E + "Rm316-AVcenterCN\r", device_reply="Ipn Rm316-AVcenter\r\n",
  expect_result=ACK, **P2)
t("reset_unit_name", {}, E + " CN\r", device_reply="Ipn IPL-250-02-74-62\r\n", expect_result=ACK, **P2)
t("get_unit_name", {}, E + "CN\r", device_reply="Ipn IPL-T-S2-00-02-3D\r\n", expect_result=val("IPL-T-S2-00-02-3D"), **TS)
t("set_date_time", {"date_time": "07/17/09-10:54:00"}, E + "07/17/09-10:54:00CT\r",
  device_reply="Ipt07/17/09-10:54:00\r\n", expect_result=ACK, **P2)
t("get_date_time", {}, E + "CT\r", device_reply="Ipt Fri, 17 Jul 2009 15:17:40\r\n",
  expect_result=val("Fri, 17 Jul 2009 15:17:40"), **P2)
t("set_gmt_offset", {"offset": "-05:00"}, E + "-05:00CZ\r", device_reply="Ipz-05:00\r\n", expect_result=ACK, **P2)
t("get_gmt_offset", {}, E + "CZ\r", device_reply="Ipz05:30\r\n", expect_result=val("05:30"), **TS)
t("set_daylight_saving", {"mode": 2}, E + "2CX\r", device_reply="Ipx2\r\n", expect_result=ACK, **P2)
t("get_daylight_saving", {}, E + "CX\r", device_reply="Ipx1\r\n", expect_result=val("1"), **TS)
t("set_dhcp", {"enabled": False}, E + "0DH\r", device_reply="Idh0\r\n", expect_result=ACK, **P2)
t("get_dhcp", {}, E + "DH\r", device_reply="Idh1\r\n", expect_result=val("1"), **TS)
t("set_ip_address", {"address": "192.168.254.254"}, E + "192.168.254.254CI\r", device_reply="Ipi 192.168.254.254\r\n",
  expect_result=ACK, **P2)
t("get_ip_address", {}, E + "CI\r", device_reply="Ipi 10.0.0.25\r\n", expect_result=val("10.0.0.25"), **TS)
t("get_mac_address", {}, E + "CH\r", device_reply="Iph 00-05-A6-02-74-62\r\n", expect_result=val("00-05-A6-02-74-62"),
  **P2)
t("set_subnet_mask", {"netmask": "255.255.0.0"}, E + "255.255.0.0CS\r", device_reply="Ips 255.255.0.0\r\n",
  expect_result=ACK, **P2)
t("get_subnet_mask", {}, E + "CS\r", device_reply="Ips 255.255.255.0\r\n", expect_result=val("255.255.255.0"), **TS)
t("set_gateway", {"gateway": "10.0.0.1"}, E + "10.0.0.1CG\r", device_reply="Ipg 10.0.0.1\r\n", expect_result=ACK, **TS)
t("get_gateway", {}, E + "CG\r", device_reply="Ipg 0.0.0.0\r\n", expect_result=val("0.0.0.0"), **P2)
t("set_verbose_mode", {"mode": 3}, E + "3CV\r", device_reply="Vrb3\r\n", expect_result=ACK, **P2)
t("get_verbose_mode", {}, E + "CV\r", device_reply="Vrb3\r\n", expect_result=val("3"), **TS)
t("get_connection_count", {}, E + "CC\r", device_reply="Icc002\r\n", expect_result=val("2"), **P2)
t("set_port_timeout", {"scope": "global", "steps": 60}, E + "1*60TC\r", device_reply="Pti1*00060\r\n",
  expect_result=ACK, **P2)
t("get_port_timeout", {"scope": "this"}, E + "0TC\r", device_reply="Pti0*00030\r\n", expect_result=val("30"), **TS)

# Security and port maps (IPL 250 p.4-16 to 4-17, IPL T S p.36-37)
t("get_security_level", {}, E + "CK\r", device_reply="Pvl12\r\n", expect_result=val("12"), **P2)
t("set_admin_password", {"password": "Room316"}, E + "Room316CA\r", device_reply="Ipa ****\r\n", expect_result=ACK, **P2)
t("set_admin_password", {"password": " "}, E + " CA\r", device_reply="Ipa \r\n", expect_result=ACK, **TS)
t("set_user_password", {"password": "guest1"}, E + "guest1CU\r", device_reply="E14\r\n", expect_result=ERR, **TS)
t("set_telnet_port", {"port": 2323}, E + "2323MT\r", device_reply="Pmt02323\r\n", expect_result=ACK, **P2)
t("get_telnet_port", {}, E + "MT\r", device_reply="Pmt00023\r\n", expect_result=val("23"), **TS)
t("set_web_port", {"port": 0}, E + "0MH\r", device_reply="Pmh00000\r\n", expect_result=ACK, **TS)
t("get_web_port", {}, E + "MH\r", device_reply="00080\r\n", expect_result=val("80"), **P2)
t("set_direct_port", {"port": 2001}, E + "2001MD\r", device_reply="Pmd02001\r\n", expect_result=ACK, **P2)
t("get_direct_port", {}, E + "MD\r", device_reply="Pmd03001\r\n", expect_result=val("3001"), **TS)

# State: banner and login, tagged replies, bare replies tied to their query.
telemetry(K, "banner", inbound="(c) Copyright 2009, Extron Electronics, IPL 250, V1.15, 60-1026-81\r\n",
          expect_state={"device": {"model": "IPL 250", "firmware": "1.15", "part_number": "60-1026-81"}})
telemetry(K, "login", inbound="Login Administrator\r\n", expect_state={"device": {"login": "Administrator"}})
telemetry(K, "verbose", inbound="Vrb3\r\n", expect_state={"verbose_mode": 3})
telemetry(K, "bare-verbose", inbound="3\r\n", request=E + "CV", expect_state={"verbose_mode": 3})
telemetry(K, "firmware", inbound="1.15\r\n", request="Q", expect_state={"device": {"firmware": "1.15"}})
telemetry(K, "part-number", inbound="Pno60-1218-02\r\n", expect_state={"device": {"part_number": "60-1218-02"}})
telemetry(K, "model-name", inbound="Inf01*IPL 250\r\n", expect_state={"device": {"model_name": "IPL 250"}})
telemetry(K, "model-description", inbound="Inf02*Two Bi-Directional Serial Ports [RS232/422/485]\r\n",
          expect_state={"device": {"description": "Two Bi-Directional Serial Ports [RS232/422/485]"}})
telemetry(K, "security-level", inbound="Pvl11\r\n", expect_state={"security_level": 11})
telemetry(K, "bare-security-level", inbound="12\r\n", request=E + "CK", expect_state={"security_level": 12})
telemetry(K, "serial-settings", inbound="Cpn02 Ccp9600,N,8,1\r\n", expect_state={"serial": {"2": {"settings": "9600,N,8,1"}}})
telemetry(K, "bare-serial-settings", inbound="115200,O,7,2\r\n", request=E + "03CP",
          expect_state={"serial": {"3": {"settings": "115200,O,7,2"}}})
telemetry(K, "serial-type", inbound="Cpn01 Cty1\r\n", expect_state={"serial": {"1": {"type": 1}}})
telemetry(K, "bare-serial-type", inbound="2\r\n", request=E + "04CY", expect_state={"serial": {"4": {"type": 2}}})
telemetry(K, "flow-control", inbound="Cpn01 CflH,0020\r\n",
          expect_state={"serial": {"1": {"flow_control": "H", "pacing": 20}}})
telemetry(K, "bare-flow-control", inbound="S,0000\r\n", request=E + "02CF",
          expect_state={"serial": {"2": {"flow_control": "S", "pacing": 0}}})
telemetry(K, "receive-timeout", inbound="Cpn02 Cce00010,00002,0,00000L\r\n",
          expect_state={"serial": {"2": {"receive_timeout": "00010,00002,0,00000L"}}})
telemetry(K, "bare-receive-timeout", inbound="00001*00002,1,00013D\r\n", request=E + "01CE",
          expect_state={"serial": {"1": {"receive_timeout": "00001*00002,1,00013D"}}})
telemetry(K, "passthrough", inbound="Cpn02 Ccd00001,00010,00000L\r\n",
          expect_state={"serial": {"2": {"passthrough": "00001,00010,00000L"}}})
telemetry(K, "bare-passthrough", inbound="00000,00000,00000L\r\n", request=E + "03CD",
          expect_state={"serial": {"3": {"passthrough": "00000,00000,00000L"}}})
telemetry(K, "ir", inbound="Irs04,12,025,01\r\n", expect_state={"ir": {"4": {"file": 12, "function": 25, "mode": 1}}})
telemetry(K, "relay", inbound="Cpn03 Rly1\r\n", expect_state={"relays": {"3": {"closed": True}}})
telemetry(K, "bare-relay", inbound="0\r\n", request="02O", expect_state={"relays": {"2": {"closed": False}}})
telemetry(K, "bare-contact", inbound="1\r\n", request="04]", expect_state={"contacts": {"4": {"closed": True}}})
telemetry(K, "unit-name", inbound="Ipn Rm316-AVcenter\r\n", expect_state={"network": {"unit_name": "Rm316-AVcenter"}})
telemetry(K, "dhcp", inbound="Idh1\r\n", expect_state={"network": {"dhcp": True}})
telemetry(K, "bare-dhcp", inbound="0\r\n", request=E + "DH", expect_state={"network": {"dhcp": False}})
telemetry(K, "address", inbound="Ipi 10.0.0.25\r\n", expect_state={"network": {"address": "10.0.0.25"}})
telemetry(K, "bare-address", inbound="192.168.254.254\r\n", request=E + "CI",
          expect_state={"network": {"address": "192.168.254.254"}})
telemetry(K, "netmask", inbound="Ips 255.255.0.0\r\n", expect_state={"network": {"netmask": "255.255.0.0"}})
telemetry(K, "bare-netmask", inbound="255.255.255.0\r\n", request=E + "CS",
          expect_state={"network": {"netmask": "255.255.255.0"}})
telemetry(K, "gateway", inbound="Ipg 10.0.0.1\r\n", expect_state={"network": {"gateway": "10.0.0.1"}})
telemetry(K, "bare-gateway", inbound="0.0.0.0\r\n", request=E + "CG", expect_state={"network": {"gateway": "0.0.0.0"}})
telemetry(K, "mac", inbound="Iph 00-05-A6-02-74-62\r\n", expect_state={"network": {"mac": "00-05-A6-02-74-62"}})
telemetry(K, "connections", inbound="Icc002\r\n", expect_state={"network": {"connections": 2}})
telemetry(K, "bare-connections", inbound="003\r\n", request=E + "CC", expect_state={"network": {"connections": 3}})
telemetry(K, "telnet-port", inbound="Pmt00023\r\n", request=E + "MT", expect_state={"network": {"telnet_port": 23}})
telemetry(K, "web-port", inbound="00080\r\n", request=E + "MH", expect_state={"network": {"web_port": 80}})
telemetry(K, "direct-port", inbound="Pmd02001\r\n", request=E + "2001MD", expect_state={"network": {"direct_port": 2001}})
telemetry(K, "date-time", inbound="Ipt Fri, 17 Jul 2009 15:17:40\r\n",
          expect_state={"clock": {"date_time": "Fri, 17 Jul 2009 15:17:40"}})
telemetry(K, "gmt-offset", inbound="Ipz-05:00\r\n", request=E + "CZ", expect_state={"clock": {"gmt_offset": "-05:00"}})
telemetry(K, "daylight-saving", inbound="Ipx2\r\n", request=E + "2CX", expect_state={"clock": {"daylight_saving": 2}})
telemetry(K, "connection-timeout", inbound="Pti0*00030\r\n", request=E + "0TC",
          expect_state={"port_timeout": {"connection": 30}})
telemetry(K, "global-timeout", inbound="00060\r\n", request=E + "1TC", expect_state={"port_timeout": {"global": 60}})
