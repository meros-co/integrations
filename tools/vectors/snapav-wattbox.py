# SnapAV WattBox (snapav-wattbox): one vector per command, from the Wattbox
# Integration Protocol v1.7: requests "?Name", controls "!Name=args", each
# ended by a line feed; answers "?Name=value", "OK" or "#Error". Replies are
# the document's examples where it gives them. An outlet list read back
# after a switch has each outlet's metering read after it; the first of
# those reads is part of what the vector sees go out.
WB = "snapav-wattbox"
_WOK = {"ok": {"kind": "ack"}}


def _wb(command, input, wire, **extra):
    wires = [w + "\n" for w in wire] if isinstance(wire, list) else wire + "\n"
    text(WB, command, input, wires, **extra)


def _wval(v):
    return {"ok": {"kind": "value", "value": v}}


_wb("get_firmware", {}, "?Firmware", device_reply="?Firmware=1.0.0.0\n", expect_result=_wval("1.0.0.0"))
_wb("get_hostname", {}, "?Hostname", device_reply="?Hostname=Wattbox\n", expect_result=_wval("Wattbox"))
_wb("get_serial", {}, "?Serial", device_reply="?Serial=12345678\n", expect_result=_wval("12345678"))
_wb("get_service_tag", {}, "?ServiceTag", device_reply="?ServiceTag=ST1234567890\n",
    expect_result=_wval("ST1234567890"))
_wb("get_model", {}, "?Model", device_reply="?Model=WB-700-IPV-12\n", expect_result=_wval("WB-700-IPV-12"))
_wb("get_outlet_count", {}, "?OutletCount", device_reply="?OutletCount=16\n", expect_result=_wval("16"))
_wb("get_outlet_status", {}, "?OutletStatus", device_reply="?OutletStatus=0,0,0,0,0,0,0,0,0,0,0,0\n",
    expect_result=_wval("0,0,0,0,0,0,0,0,0,0,0,0"))
_wb("get_outlet_power_status", {"outlet": 1}, "?OutletPowerStatus=1",
    device_reply="?OutletPowerStatus=1,1.01,0.02,116.50\n", expect_result=_wval("1,1.01,0.02,116.50"))
_wb("get_power_status", {}, "?PowerStatus", device_reply="?PowerStatus=60.00,600.00,110.00,1\n",
    expect_result=_wval("60.00,600.00,110.00,1"))
_wb("get_auto_reboot", {}, "?AutoReboot", device_reply="?AutoReboot=1\n", expect_result=_wval("1"))
_wb("get_outlet_names", {}, "?OutletName", device_reply="?OutletName={Outlet 1},{Outlet 2}\n",
    expect_result=_wval("{Outlet 1},{Outlet 2}"))
_wb("get_ups_status", {}, "?UPSStatus", device_reply="?UPSStatus=50,0,Good,False,25,True,False\n",
    expect_result=_wval("50,0,Good,False,25,True,False"))
_wb("get_ups_connection", {}, "?UPSConnection", device_reply="?UPSConnection=0\n", expect_result=_wval("0"))
_wb("set_outlet_name", {"outlet": 3, "name": "Projector"}, ["!OutletNameSet=3,Projector", "?OutletName"],
    device_reply="OK\n?OutletName={Outlet 1},{Outlet 2},{Projector}\n", expect_result=_WOK)
_wb("set_all_outlet_names", {"names": "{Rack},{Display}"}, ["!OutletNameSetAll={Rack},{Display}", "?OutletName"],
    device_reply="OK\n?OutletName={Rack},{Display}\n", expect_result=_WOK)
_wb("set_outlet", {"outlet": 2, "action": "on"}, ["!OutletSet=2,ON", "?OutletStatus", "?OutletPowerStatus=1"],
    device_reply="OK\n?OutletStatus=0,1\n", expect_result=_WOK)
_wb("set_outlet", {"outlet": 2, "action": "reset"}, ["!OutletSet=2,RESET"],
    device_reply="#Error\n", expect_result={"error": {"error": "device_error"}}, file="set_outlet-error")
_wb("reset_outlet_after", {"outlet": 4, "delay_s": 30}, ["!OutletSet=4,RESET,30", "?OutletStatus", "?OutletPowerStatus=1"],
    device_reply="OK\n?OutletStatus=1,1,1,0\n", expect_result=_WOK)
_wb("reset_all_outlets", {}, ["!OutletSet=0,RESET", "?OutletStatus", "?OutletPowerStatus=1"],
    device_reply="OK\n?OutletStatus=0,0\n", expect_result=_WOK)
_wb("set_outlet_power_on_delay", {"outlet": 1, "delay_s": 10}, "!OutletPowerOnDelaySet=1,10", device_reply="OK\n",
    expect_result=_WOK)
_wb("set_outlet_mode", {"outlet": 5, "mode": "reset_only"}, "!OutletModeSet=5,2", device_reply="OK\n",
    expect_result=_WOK)
_wb("set_outlet_reboot_operations", {"operations": "0,1,0,0"}, "!OutletRebootSet=0,1,0,0", device_reply="OK\n",
    expect_result=_WOK)
_wb("set_auto_reboot", {"enabled": True}, ["!AutoReboot=1", "?AutoReboot"], device_reply="OK\n?AutoReboot=1\n",
    expect_result=_WOK)
_wb("set_auto_reboot_timeouts", {"timeout_s": 30, "count": 3, "ping_delay_min": 5, "reboot_attempts": 0},
    "!AutoRebootTimeoutSet=30,3,5,0", device_reply="OK\n", expect_result=_WOK)
_wb("update_firmware", {"url": "http://10.0.0.5/wattbox.bin"}, "!FirmwareUpdate=http://10.0.0.5/wattbox.bin",
    device_reply="OK\n", expect_result=_WOK)
_wb("reboot", {}, "!Reboot", device_reply="OK\n", expect_result=_WOK)
_wb("set_account", {"user": "admin", "password": "n3wpass"}, "!AccountSet=admin,n3wpass", device_reply="OK\n",
    expect_result=_WOK)
_wb("set_network_dhcp", {"hostname": "wattbox-rack1"}, "!NetworkSet=wattbox-rack1", device_reply="OK\n",
    expect_result=_WOK)
_wb("set_network_static", {"hostname": "wattbox-rack1", "ip": "192.168.0.20", "subnet": "255.255.255.0",
                           "gateway": "192.168.0.1", "dns1": "192.168.0.1"},
    "!NetworkSet=wattbox-rack1,192.168.0.20,255.255.255.0,192.168.0.1,192.168.0.1,8.8.8.8", device_reply="OK\n",
    expect_result=_WOK)
_wb("add_schedule_once", {"name": "Close", "outlets": "1,2,3", "action": "off", "date": "2018/09/28", "time": "13:30"},
    "!ScheduleAdd={Close},{1,2,3},{0},{0},{2018/09/28},{13:30}", device_reply="OK\n", expect_result=_WOK)
_wb("add_schedule_recurring", {"name": "Open", "outlets": "4", "action": "on", "days": "0,1,0,1,0,1,0",
                               "time": "08:00"},
    "!ScheduleAdd={Open},{4},{1},{1},{0,1,0,1,0,1,0},{08:00}", device_reply="OK\n", expect_result=_WOK)
_wb("add_host", {"name": "Router", "address": "192.168.0.1", "outlets": "1,2"}, "!HostAdd=Router,192.168.0.1,{1,2}",
    device_reply="OK\n", expect_result=_WOK)

# Telemetry: the answers the polls get, as the document shows them.
telemetry(WB, "outlet-status", inbound="?OutletStatus=1,0,1\n",
          expect_state={"outlets": {"1": {"on": True}, "2": {"on": False}, "3": {"on": True}}},
          expect_then_send=["?OutletPowerStatus=1\n", "?OutletPowerStatus=2\n", "?OutletPowerStatus=3\n"])
telemetry(WB, "outlet-power", inbound="?OutletPowerStatus=1,1.01,0.02,116.50\n",
          expect_state={"outlets": {"1": {"power": 1.01, "current": 0.02, "voltage": 116.5}}})
telemetry(WB, "outlet-names", inbound="?OutletName={Outlet 1},{Rack fan}\n",
          expect_state={"outlets": {"1": {"name": "Outlet 1"}, "2": {"name": "Rack fan"}}})
telemetry(WB, "power-status", inbound="?PowerStatus=60.00,600.00,110.00,1\n",
          expect_state={"power": {"current": 60.0, "power": 600.0, "voltage": 110.0, "safe_voltage": True}})
telemetry(WB, "ups-status", inbound="?UPSStatus=50,0,Good,False,25,True,False\n",
          expect_state={"ups": {"battery_charge": 50, "battery_load": 0, "battery_health": "Good", "power_lost": False,
                                "runtime": 25, "alarm_enabled": True, "alarm_muted": False}})
telemetry(WB, "ups-connection", inbound="?UPSConnection=0\n", expect_state={"ups": {"connected": False}})
telemetry(WB, "auto-reboot", inbound="?AutoReboot=1\n", expect_state={"auto_reboot": True})
telemetry(WB, "identity", inbound="?Model=WB-700-IPV-12\n", expect_state={"device": {"model": "WB-700-IPV-12"}})
telemetry(WB, "firmware", inbound="?Firmware=1.0.0.0\n", expect_state={"device": {"firmware": "1.0.0.0"}})
telemetry(WB, "hostname", inbound="?Hostname=Wattbox\n", expect_state={"device": {"hostname": "Wattbox"}})
telemetry(WB, "serial", inbound="?Serial=12345678\n", expect_state={"device": {"serial": "12345678"}})
telemetry(WB, "service-tag", inbound="?ServiceTag=ST1234567890\n", expect_state={"device": {"service_tag": "ST1234567890"}})
telemetry(WB, "outlet-count", inbound="?OutletCount=12\n", expect_state={"outlet_count": 12})
