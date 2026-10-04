# Focusrite RedNet MP8R over Yamaha RCP (TCP 49280, LF-terminated). Addresses and ranges from the unit's own
# prminfo answers in the Companion module's MP8R Parameters-1.txt; the grammar (set/get <address> <x> <y>
# [value], OK/OKm/ERROR replies, NOTIFY set) from Yamaha's DME7 RCP specification 3.1-3.5. Channel n is X = n-1.
FR = "focusrite-rednet-rcp"

_FR_FLAGS = [
    ("phantom_power", "IO:Current/InCh/48VOn"),
    ("high_pass_filter", "IO:Current/InCh/HPFOn"),
    ("gain_compensation", "IO:Current/InCh/GainCompOn"),
]
_FR_LEVELS = [
    ("preamp_gain", "IO:Current/InCh/HAGain", 42, "preamp_gain_db"),
    ("compensated_gain", "IO:Current/InCh/CompGain", -6, "compensated_gain_db"),
]

for _name, _addr in _FR_FLAGS:
    text(FR, "set_" + _name, {"channel": 3, "enabled": True}, f"set {_addr} 2 0 1\n",
         device_reply=f"OK set {_addr} 2 0 1\n", expect_result={"ok": {"kind": "ack"}})
    text(FR, "get_" + _name, {"channel": 8}, f"get {_addr} 7 0\n",
         device_reply=f"OK get {_addr} 7 0 0\n", expect_result={"ok": {"kind": "value", "value": "0"}})
    telemetry(FR, _name.replace("_", "-"), inbound=f"NOTIFY set {_addr} 0 0 1\n",
              expect_state={"channels": {"1": {_name: True}}})

for _name, _addr, _value, _state in _FR_LEVELS:
    text(FR, "set_" + _name, {"channel": 1, "gain_db": _value}, f"set {_addr} 0 0 {_value}\n",
         device_reply=f"OK set {_addr} 0 0 {_value}\n", expect_result={"ok": {"kind": "ack"}})
    text(FR, "get_" + _name, {"channel": 5}, f"get {_addr} 4 0\n",
         device_reply=f"OK get {_addr} 4 0 {_value}\n", expect_result={"ok": {"kind": "value", "value": str(_value)}})
    telemetry(FR, _name.replace("_", "-"), inbound=f"OK get {_addr} 4 0 {_value}\n",
              expect_state={"channels": {"5": {_state: _value}}})

for _name, _addr, _value in [("system_status", "IO:Current/Dev/SystemStatus", 2),
                             ("sync_status", "IO:Current/Dev/SyncStatus", 9),
                             ("exec_mode", "IO:Current/Dev/ExecMode", 0)]:
    text(FR, "get_" + _name, {}, f"get {_addr} 0 0\n",
         device_reply=f"OK get {_addr} 0 0 {_value}\n", expect_result={"ok": {"kind": "value", "value": str(_value)}})
    telemetry(FR, _name.replace("_", "-"), inbound=f"NOTIFY set {_addr} 0 0 {_value}\n",
              expect_state={"device": {_name: _value}})

# A gain the unit clamps is acknowledged OKm (DME7 spec 3.3.5).
text(FR, "set_parameter", {"address": "IO:Current/InCh/HAGain", "x": 6, "y": 0, "value": 70},
     "set IO:Current/InCh/HAGain 6 0 70\n", device_reply="OKm set IO:Current/InCh/HAGain 6 0 66\n",
     expect_result={"ok": {"kind": "ack"}})
text(FR, "get_parameter", {"address": "IO:Current/Dev/IPSelectMode", "x": 0, "y": 0},
     "get IO:Current/Dev/IPSelectMode 0 0\n", device_reply="OK get IO:Current/Dev/IPSelectMode 0 0 1\n",
     expect_result={"ok": {"kind": "value", "value": "1"}})
text(FR, "get_product_name", {}, "devinfo productname\n", device_reply='OK devinfo productname "MP8R"\n',
     expect_result={"ok": {"kind": "value", "value": "MP8R"}})
text(FR, "get_device_name", {}, "devinfo devicename\n", device_reply='OK devinfo devicename "Stage MP8R"\n',
     expect_result={"ok": {"kind": "value", "value": "Stage MP8R"}})
text(FR, "get_run_mode", {}, "devstatus runmode\n", device_reply='OK devstatus runmode "normal"\n',
     expect_result={"ok": {"kind": "value", "value": "normal"}})
text(FR, "get_error_status", {}, "devstatus error\n", device_reply='OK devstatus error "nothing"\n',
     expect_result={"ok": {"kind": "value", "value": "nothing"}})
text(FR, "set_keepalive", {"interval_ms": 20000}, "scpmode keepalive 20000\n",
     device_reply="OK scpmode keepalive 20000\n", expect_result={"ok": {"kind": "ack"}})
telemetry(FR, "product-name", expect_connect_wire=["devinfo productname\n"],
          inbound='OK devinfo productname "MP8R"\n', expect_state={"device": {"product_name": "MP8R"}})
telemetry(FR, "run-mode", inbound='NOTIFY devstatus runmode "normal"\n', expect_state={"device": {"run_mode": "normal"}})
telemetry(FR, "error-status", inbound='OK devstatus error "nothing"\n', expect_state={"device": {"error": "nothing"}})
telemetry(FR, "error-reply-is-not-state", inbound="ERROR get UnknownAddress\n", expect_state={})
