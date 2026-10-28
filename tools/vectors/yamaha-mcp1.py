MC = "yamaha-mcp1"
# Yamaha MCP1 Remote Control Protocol (V1.0.0): one LF-ended text line per
# command, answered OK or ERROR <command> <code>.


def mc(command, input, wire, reply=None, result=None, **extra):
    if reply is not None:
        extra["device_reply"] = reply + "\n"
        extra["expect_result"] = result
    text(MC, command, input, wire + "\n", **extra)


def mcv(v):
    return {"ok": {"kind": "value", "value": v}}


ACK = {"ok": {"kind": "ack"}}
mc("get_run_mode", {}, "devstatus runmode", 'OK devstatus runmode "normal"', mcv("normal"))
mc("get_error_status", {}, "devstatus error", 'OK devstatus error "none"', mcv("none"))
mc("set_emergency_mode", {"emergency": True}, "devmode emergency", "OK devmode emergency", ACK)
mc("set_encoding", {"encoding": "utf8"}, "scpmode encoding utf8", "OK scpmode encoding utf8", ACK)
mc("set_keepalive", {"interval_ms": 5000}, "scpmode keepalive 5000", "OK scpmode keepalive 5000", ACK)
mc("get_current_preset", {}, "sscurrent", "OK sscurrent 3 modified", mcv("3"))
mc("recall_preset", {"index": 2}, "ssrecall 2", "OK ssrecall 2", ACK)
mc("recall_preset", {"index": 2}, "ssrecall 2", "ERROR ssrecall AccessDenied", {"error": {"error": "device_error"}}, file="recall_preset-denied")
mc("get_protocol_version", {}, "devinfo protocolver", 'OK devinfo protocolver "1.0.0"', mcv("1.0.0"))
mc("get_firmware_version", {}, "devinfo version", 'OK devinfo version "5.0.0"', mcv("5.0.0"))
mc("get_product_name", {}, "devinfo productname", 'OK devinfo productname "MCP1"', mcv("MCP1"))
mc("get_serial_number", {}, "devinfo serialno", 'OK devinfo serialno "ABC123"', mcv("ABC123"))
mc("get_device_id", {}, "devinfo deviceid", 'OK devinfo deviceid "001"', mcv("001"))
mc("get_device_name", {}, "devinfo devicename", 'OK devinfo devicename "Hall"', mcv("Hall"))
mc("get_preset_count", {}, "ssnum", "OK ssnum 20", mcv("20"))
mc("get_preset_info", {"index": 1}, "ssinfo 1", 'OK ssinfo 1 "1" user "Lecture" ""', mcv("Lecture"))


def mct(name, inbound, state):
    telemetry(MC, name, inbound=inbound + "\n", expect_state=state)


mct("run-mode", 'NOTIFY devstatus runmode "emergency"', {"device": {"run_mode": "emergency"}})
mct("error", 'NOTIFY devstatus error "err/DCP[0] communication error// x53 on (1) ID-001 2013/1/22 11:38:23"',
    {"device": {"error": "err/DCP[0] communication error// x53 on (1) ID-001 2013/1/22 11:38:23"}})
mct("devmode", "OK devmode normal", {"device": {"run_mode": "normal"}})
mct("protocol", 'OK devinfo protocolver "1.0.0"', {"device": {"protocol_version": "1.0.0"}})
mct("firmware", 'OK devinfo version "5.0.0"', {"device": {"firmware": "5.0.0"}})
mct("product", 'OK devinfo productname "MCP1"', {"device": {"product": "MCP1"}})
mct("serial", 'OK devinfo serialno "ABC123"', {"device": {"serial": "ABC123"}})
mct("device-id", 'OK devinfo deviceid "001"', {"device": {"device_id": "001"}})
mct("name", 'OK devinfo devicename "Hall"', {"device": {"name": "Hall"}})
mct("current", "OK sscurrent 3 modified", {"presets": {"current": 3, "modified": True}})
mct("current-push", "NOTIFY sscurrent 4", {"presets": {"current": 4}})
mct("recall", "NOTIFY ssrecall 4", {"presets": {"last_recalled": 4}})
mct("count", "OK ssnum 20", {"presets": {"count": 20}})
mct("info", 'OK ssinfo 1 "1" user "Lecture" ""', {"presets": {"1": {"number": "1", "kind": "user", "title": "Lecture"}}})
mct("error-answer", "ERROR ssrecall AccessDenied", {"error": {"command": "ssrecall", "code": "AccessDenied"}})
