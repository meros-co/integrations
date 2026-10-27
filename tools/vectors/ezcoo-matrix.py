EZ = "ezcoo-matrix"
# Ezcoo EZ-MX44HAS2 (User Manual V1.1): EZS/EZG commands ended by Enter, sent
# here with CR LF; no answers documented, so commands are unverified.
EZ_U = {"ok": {"kind": "unverified"}}


def ez(command, input, line, **extra):
    text(EZ, command, input, line + "\r\n", **extra)


ez("route", {"output": 1, "input": 2}, "EZS OUT1 VS IN2", expect_result=EZ_U)
ez("set_extracted_audio", {"output": 0, "enabled": True}, "EZS OUT0 EXA EN")
ez("set_stream", {"output": 3, "enabled": False}, "EZS OUT3 STREAM OFF")
ez("set_input_edid", {"input": 2, "edid": 14}, "EZS IN2 EDID 14")
ez("set_cascade_mode", {"enabled": False}, "EZS DEBUG MODE1 DIS")
ez("set_ip_address", {"address": "192.168.101.20"}, "EZS HIP 192.168.101.20")
ez("get_ip_address", {}, "EZG HIP")
ez("get_status", {}, "EZSTA")
ez("factory_reset", {}, "EZS RST")


def ez_t(name, inbound, state):
    telemetry(EZ, name, inbound=inbound + "\r\n", expect_state=state)


ez_t("address", "ADDR 00", {"address": 0})
ez_t("signal", "IN2 SIG STA 1", {"inputs": {"2": {"signal": True}}})
ez_t("edid", "IN1 EDID 0", {"inputs": {"1": {"edid": 0}}})
ez_t("route", "OUT1 VS IN3", {"outputs": {"1": {"input": 3}}})
ez_t("extracted-audio", "OUT2 EXA DIS", {"outputs": {"2": {"extracted_audio": False}}})
ez_t("stream", "OUT1 STREAM ON", {"outputs": {"1": {"stream": True}}})
ez_t("video", "OUT3 VIDEO1", {"outputs": {"3": {"video_mode": 1}}})
ez_t("gateway", "RIP 192.168.001.001", {"network": {"gateway": "192.168.001.001"}})
ez_t("ip", "HIP 192.168.001.002", {"network": {"ip_address": "192.168.001.002"}})
ez_t("netmask", "NMK 255.255.255.000", {"network": {"netmask": "255.255.255.000"}})
ez_t("tcp-port", "TIP 23", {"network": {"tcp_port": 23}})
ez_t("dhcp", "DHCP 1", {"network": {"dhcp": True}})
ez_t("mac", "MAC 00.01.02.03.04.05", {"network": {"mac_address": "00.01.02.03.04.05"}})
