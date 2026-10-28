# Kiloview RF02 Cradle chassis: the management platform's Web API on port
# 8000 (Kiloview-RF02-WebAPI 2025-06-27). Requests follow the document's
# body examples; replies its {requestId, code, msg, data} examples.
KR = "kiloview-rf02"
_KR_ACK = {"ok": {"kind": "ack"}}
_KR_EMPTY = {"status": 200, "body": "{}"}


def _kr(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": KR, "command": command, "input": input, "expect_request": req, **extra})


def _kr_data(data, msg="OK"):
    return {"status": 200, "body": json.dumps({"requestId": "8ee8ebb9", "code": 200, "msg": msg, "data": data},
                                              separators=(",", ":"))}


# ── Login and users ─────────────────────────────────────────────────────
_kr("login", {"username": "admin", "password": "admin"}, "POST", "/api/v1/login",
    '{"userName":"admin","passWord":"admin"}',
    http_reply=_kr_data({"expire": "2124-12-09T11:43:48Z", "token": "eyJ.rf02.tok",
                         "user": {"nickname": "nickname", "userId": 3, "username": "qss"}}, "Login successful"),
    expect_result={"ok": {"kind": "value", "value": "eyJ.rf02.tok"}})
# With a password, the session logs in first (the engine's session tests
# cover the bearer token).
V.append({"spec": "kiloview-rf02", "command": "get_self", "input": {}, "settings": {"password": "admin"},
          "expect_request": {"method": "POST", "target": "/api/v1/login",
                             "body": '{"userName":"admin","passWord":"admin"}', "content_type": "application/json"}})
_kr("get_user", {"user": 9}, "GET", "/api/v1/sys-user/9")
_kr("add_user", {"username": "lenovo", "nickname": "lenovo", "password": "Admin123"}, "POST", "/api/v1/sys-user",
    '{"username":"lenovo","nickName":"lenovo","password":"Admin123"}', http_reply=_KR_EMPTY, expect_result=_KR_ACK)
# A refused write: the document's 500 answer.
_kr("add_device", {"name": "P3", "serial_number": "2023110200011", "license": "a1bad332"}, "POST",
    "/api/v1/klnk/addDevice",
    '{"name":"P3","serialnumber":"2023110200011","license":"a1bad332","note":"","serial_number":"2023110200011"}',
    file="add_device.error",
    http_reply={"status": 500, "body": '{"requestId":"4865a0ec","code":500,"msg":"Device Already Exists",'
                                       '"status":"error","data":null}'},
    expect_result={"error": {"error": "device_error", "code": "500"}})
_kr("set_user_nickname", {"user": 3, "nickname": "nickname", "remark": "eu"}, "PUT", "/api/v1/sys-user",
    '{"userId":3,"nickName":"nickname","remark":"eu"}')
_kr("get_users", {"page": 1, "page_size": 10}, "GET", "/api/v1/sys-user?pageIndex=1&pageSize=10")
_kr("delete_user", {"user": 7}, "DELETE", "/api/v1/sys-user/delUsers", '{"ids":[7]}')
_kr("delete_users", {"ids": [7, 83]}, "DELETE", "/api/v1/sys-user/delUsers", '{"ids":[7,83]}')
_kr("set_user_password", {"user": 3, "password": "123456"}, "PUT", "/api/v1/user/pwd/reset",
    '{"userId":3,"password":"123456"}')
_kr("get_modules", {}, "GET", "/api/v1/sys-module?page=1&pageSize=10")
_kr("get_roles", {"page": 2, "page_size": 20}, "GET", "/api/v1/role?pageIndex=2&pageSize=20")
_kr("delete_role", {"role": 3}, "DELETE", "/api/v1/role", '{"ids":[3]}')
_kr("delete_roles", {"ids": [3, 4]}, "DELETE", "/api/v1/role", '{"ids":[3,4]}')
_kr("set_role", {"role": 2, "name": "XI SHI Yu", "remark": "User 1", "users": [4, 9, 1, 3]}, "POST", "/api/v1/role/2",
    '{"roleId":2,"roleName":"XI SHI Yu","remark":"User 1","userIds":[4,9,1,3]}')
_kr("rename_role", {"role": 2, "name": "Ordinary users"}, "POST", "/api/v1/role/rename",
    '{"roleId":2,"roleName":"Ordinary users"}')
_kr("set_role_modules", {"role": 2, "modules": ["devMgr", "usrMgr", "firmwareUpload", "firmwareDispatch", "streamMgr"]},
    "POST", "/api/v1/role/update",
    '{"roleId":2,"modules":["devMgr","usrMgr","firmwareUpload","firmwareDispatch","streamMgr"]}')

# ── Directory service ───────────────────────────────────────────────────
_kr("create_directory", {"name": "MGTV", "parent": 2}, "POST", "/api/v1/directory", '{"dirName":"MGTV","parentId":2}')
_kr("rename_directory", {"directory": 1, "name": "Root22"}, "PUT", "/api/v1/directory/1", '{"dirName":"Root22"}')
_kr("delete_directory", {"directory": 5}, "DELETE", "/api/v1/directory/5", "{}")
_kr("move_to_directory", {"directory": 4, "item": "000000002", "target": 8}, "PUT", "/api/v1/directory/4/position",
    '{"dirId":"000000002","targetId":8,"type":"device"}')
_kr("move_batch", {"target": 2, "ids": ["4"]}, "POST", "/api/v1/directory/move/batch", '{"targetId":2,"ids":["4"]}')
_kr("set_directory_permissions", {"directory": 4, "permissions": [{"roleId": 2, "permission": "write"},
                                                                  {"roleId": 3, "permission": "read"}]},
    "POST", "/api/v1/directory/permission",
    '{"directoryId":"4","permissions":[{"roleId":2,"permission":"write"},{"roleId":3,"permission":"read"}]}')
_kr("check_directory_permission", {"role": 2, "directory": 4, "permission": "write"}, "POST",
    "/api/v1/directory/check", '{"roleId":2,"directoryId":4,"permission":"write"}')
_kr("get_directory_permissions", {"directory": 4}, "GET", "/api/v1/directory/permission/?directoryId=4")
_kr("clear_directory_permissions", {}, "DELETE", "/api/v1/directory/deleteAllPermission")
_kr("get_directory_tree", {}, "GET", "/api/v1/directory/tree")
_kr("get_directory_devices", {"directory": 2}, "GET", "/api/v1/directory/2/devices",
    http_reply=_kr_data({"list": None, "online_count": 0, "total_count": 0}),
    expect_result={"ok": {"kind": "value", "value": {"list": None, "online_count": 0, "total_count": 0}}})
_kr("get_directory_encoders", {"directory": "2"}, "GET", "/api/v1/directory/2/encode")
_kr("get_directory_decoders", {"directory": "202501161047"}, "GET", "/api/v1/directory/202501161047/decode")
_kr("get_directory_gateways", {"directory": "2"}, "GET", "/api/v1/directory/2/mgw")
_kr("get_directory_bonding", {"directory": "2"}, "GET", "/api/v1/directory/2/bonding")
_kr("delete_gateway_channel", {"serial_number": "1234567891234", "channel": "2"}, "POST",
    "/api/v1/klnk/gateway/channels/del", '[{"channelid":"2","serial_number":"1234567891234"}]')
_kr("delete_gateway_channels", {"channels": [{"channelid": "2", "serial_number": "1234567891234"},
                                             {"channelid": "7", "serial_number": "1234567891234"}]},
    "POST", "/api/v1/klnk/gateway/channels/del",
    '[{"channelid":"2","serial_number":"1234567891234"},{"channelid":"7","serial_number":"1234567891234"}]')

# ── Network, system, maintenance ────────────────────────────────────────
_kr("get_network", {}, "GET", "/api/v1/netmgr")
_kr("set_network", {"interface": "enP1p1s0", "ip": "192.168.35.105", "netmask": "255.255.255.0", "dns": "1.2.4.8",
                    "mac": "6e:6e:4f:4f:4f:2c"}, "POST", "/api/v1/netmgr",
    '{"ifname":"enP1p1s0","ip":"192.168.35.105","netmask":"255.255.255.0","gw":"","dns":"1.2.4.8",'
    '"method":"static","mac":"6e:6e:4f:4f:4f:2c"}')
_kr("set_network_dhcp", {"interface": "eth0", "mac": "6e:6e:4f:4f:4f:2c"}, "POST", "/api/v1/netmgr",
    '{"ifname":"eth0","ip":"","netmask":"","gw":"","dns":"","method":"dhcp","mac":"6e:6e:4f:4f:4f:2c"}')
_kr("get_system", {}, "GET", "/api/v1/systemctrl/get")
_kr("set_system", {"name": "Device258", "ntp": "ntp1.aliyun.com ntp2.aliyun.com ntp3.aliyun.com",
                   "timezone": "Asia/Shanghai", "time": "2024-12-23 12:37:03", "offset": 8}, "POST",
    "/api/v1/systemctrl/baseConf",
    '{"name":"Device258","timetype":"ntp","ntp":"ntp1.aliyun.com ntp2.aliyun.com ntp3.aliyun.com",'
    '"timezone":"Asia/Shanghai","time":"2024-12-23 12:37:03","offset":8}')
_kr("get_maintenance", {}, "GET", "/api/v1/systemctrl/system_maintenance")
_kr("set_maintenance", {"enable": 2, "policy": "day", "day": 1, "days": [], "months": [], "weeks": [],
                        "start": "2024-12-13T16:15:00.000Z"}, "POST", "/api/v1/systemctrl/system_maintenance",
    '{"enable":2,"day":1,"days":[],"months":[],"policy":"day","start_date":"2024-12-13T16:15:00.000Z","weeks":[]}')
_kr("factory_reset", {}, "POST", "/api/v1/systemctrl/reset", http_reply=_KR_EMPTY, expect_result=_KR_ACK)
_kr("reboot", {}, "POST", "/api/v1/systemctrl/reboot")
_kr("get_system_version", {}, "GET", "/api/v1/systemctrl/sys_ver")
_kr("get_logs", {"date": "2025-06-01,2025-06-02"}, "GET", "/api/v1/systemctrl/sysLog?date=2025-06-01%2C2025-06-02")
_kr("export_logs", {"date": "2025-06-01"}, "GET", "/api/v1/systemctrl/expLog?date=2025-06-01")

# ── Card firmware ───────────────────────────────────────────────────────
_kr("get_firmware_types", {}, "GET", "/api/v1/firmware/type?page=1&page_size=10")
_kr("get_firmware_list", {"device_type": "3"}, "GET", "/api/v1/firmware/list?devicetypeid=3")
_kr("get_firmware_by_model", {"model": "P3"}, "GET", "/api/v1/firmware/model?model=P3")
_kr("check_main_control_firmware", {}, "GET", "/api/v1/firmware/maincontrol/exists")
_kr("delete_firmware", {"ids": [1, 2]}, "DELETE", "/api/v1/firmware", '{"ids":[1,2]}')
_kr("set_firmware_description", {"firmware": 2, "description": "description"}, "PUT", "/api/v1/firmware",
    '{"id":2,"description":"description"}',
    http_reply=_kr_data("OK", "Update Successfully"), expect_result=_KR_ACK)
# Multipart, written by the E3 generator's form encoder (kiloview-e3.py).
_kr_body, _kr_ctype = _ke3_form([("identifier", b"fw1", None), ("number", b"0", None), ("file_type", b"bin", None),
                                 ("file", bytes([0, 1, 2]), "fw1.0")])
V.append({"spec": KR, "command": "upload_firmware_chunk",
          "input": {"identifier": "fw1", "number": 0, "file_type": "bin", "file": "AAEC"},
          "expect_request": {"method": "POST", "target": "/api/v1/firmware/upload/chunk",
                             "body_hex": _kr_body.hex(), "content_type": _kr_ctype}})

# ── Front screen ────────────────────────────────────────────────────────
_kr("get_lcd_addresses", {}, "GET", "/api/v1/netmgr/lcd-quick")
_kr("get_lcd_config", {}, "GET", "/api/v1/netmgr/lcd-checkConf")
_kr("set_lcd_address", {"interface": "enp1s0", "ip": "192.168.35.101", "netmask": "255.255.255.0", "dns": "1.2.4.8",
                        "mac": "6e:6e:4f:4f:4f:2c"}, "POST", "/api/v1/netmgr/setIP",
    '{"ifname":"enp1s0","ip":"192.168.35.101","netmask":"255.255.255.0","gw":"","dns":"1.2.4.8",'
    '"method":"static","mac":"6e:6e:4f:4f:4f:2c"}')
_kr("lcd_factory_reset", {}, "POST", "/api/v1/systemctrl/reset-lcd", '{"password":"kiloview"}')

# ── Boards and external devices ─────────────────────────────────────────
_kr("set_card_network", {"serial_numbers": ["000000003", "000000108", "000000002"], "start_ip": "192.168.35.134",
                         "netmask": "255.255.255.0", "gateway": "192.168.35.254", "dns": "8.8.8.8;8.8.4.4"},
    "POST", "/api/v1/klnk/network",
    '{"serial_numbers":["000000003","000000108","000000002"],"network":{"mode":"static",'
    '"start_ip":"192.168.35.134","ip_increment":1,"netmask":"255.255.255.0","gateway":"192.168.35.254",'
    '"dns":"8.8.8.8;8.8.4.4"}}')
_kr("set_card_network_dhcp", {"serial_numbers": ["000000003"]}, "POST", "/api/v1/klnk/network",
    '{"serial_numbers":["000000003"],"network":{"mode":"dhcp","start_ip":"","ip_increment":1,"netmask":"",'
    '"gateway":"","dns":""}}')
_kr("rename_card", {"serial_number": "000000108", "name": "N60-Test"}, "POST", "/api/v1/klnk/rename",
    '{"serial_number":"000000108","friendly_name":"N60-Test"}')
_kr("reboot_cards", {"serial_numbers": ["000000108"]}, "POST", "/api/v1/klnk/reboot",
    '{"serial_numbers":["000000108"]}')
_kr("reset_cards", {"serial_numbers": ["000000002"]}, "POST", "/api/v1/klnk/reset", '{"serial_numbers":["000000002"]}')
_kr("upgrade_cards", {"version": 8, "serial_numbers": ["000000002"]}, "POST", "/api/v1/klnk/upgrade",
    '{"version_id":8,"serial_numbers":["000000002"]}')
_kr("debug_forwarding_ports", {"serial_numbers": ["000000002"]}, "POST", "/api/v1/klnk/ports",
    '{"serial_numbers":["000000002"]}')
_kr("debug_forwarding", {"list": [{"serial_number": "000000002", "channel": ""}]}, "POST", "/api/v1/klnk/forward",
    '{"list":[{"serial_number":"000000002","channel":""}]}')
_kr("set_card_mode", {"serial_number": "202501161047", "mode": "encode"}, "POST", "/api/v1/klnk/changemode",
    '{"serial_number":"202501161047","mode":"encode"}')
_kr("get_device_config_map", {"serial_number": "2023110200011", "channel": "1"}, "GET",
    "/api/v1/klnk/getDeviceConfigMap?serial_number=2023110200011&id=1")
_kr("get_device_config", {"serial_number": "2023110200011", "channel": "1"}, "GET",
    "/api/v1/klnk/getDeviceConfig?serial_number=2023110200011&id=1")
_KR_CFG = {"video": {"signal": "hdmi", "codec": "h264", "resolution": "1920x1080", "bitrate": "8000",
                     "framerate_mode": "custom", "framerate": "30"}, "audio": {"signal": "hdmi"}}
_kr("set_device_config", {"serial_number": "2023110200011", "channel": "1", "config": _KR_CFG}, "POST",
    "/api/v1/klnk/setDeviceConfig",
    '{"serial_number":"2023110200011","id":"1","config":' + json.dumps(_KR_CFG, separators=(",", ":")) + "}")
_kr("add_device", {"name": "P3", "serial_number": "2023110200011", "license": "a1bad332"}, "POST",
    "/api/v1/klnk/addDevice",
    '{"name":"P3","serialnumber":"2023110200011","license":"a1bad332","note":"","serial_number":"2023110200011"}')
_kr("get_device_license", {"serial_number": "2023110200011"}, "GET",
    "/api/v1/klnk/getDeviceLicense?serial_number=2023110200011",
    http_reply=_kr_data("a1bad332", "Success"), expect_result={"ok": {"kind": "value", "value": "a1bad332"}})
_kr("get_gateway_output_devices", {}, "GET", "/api/v1/klnk/gateway/output-devices")
_kr("delete_devices", {"serial_numbers": ["000000108"]}, "POST", "/api/v1/klnk/delDevice",
    '{"serial_numbers":["000000108"]}')
_kr("get_bonding_traffic", {"serial_number": "2023110200011"}, "GET",
    "/api/v1/klnk/getBondingTraffic?serial_number=2023110200011")

# ── OPS, switch, dashboard ──────────────────────────────────────────────
_kr("get_metrics", {}, "GET", "/api/v1/metrics",
    http_reply={"status": 200, "body": "rf02_devices_total 20\n"},
    expect_result={"ok": {"kind": "value", "value": "rf02_devices_total 20"}})
_kr("reset_switch", {}, "POST", "/api/v1/klnk/rstSwitchBoard")
_kr("get_aggregation_ports", {}, "GET", "/api/v1/sw/aggr/ports")
_kr("get_aggregation", {}, "GET", "/api/v1/sw/aggr/dump")
_kr("set_aggregation", {"groups": [{"aggre-id": 7, "name": "-", "type": "lacp", "enable": 1, "members": [20, 22]}]},
    "POST", "/api/v1/sw/aggr/set", '[{"aggre-id":7,"name":"-","type":"lacp","enable":1,"members":[20,22]}]')
_kr("get_dashboard", {}, "GET", "/api/v1/dashboard/data")
_kr("get_dashboard_of", {"type": "external"}, "GET", "/api/v1/dashboard/data?type=external")
_kr("get_notification_summary", {}, "GET", "/api/v1/noti/summary")
_kr("get_notifications", {}, "GET", "/api/v1/noti/list")
_kr("get_notifications_of", {"type": "warning"}, "GET", "/api/v1/noti/list?msg_type=warning")
_kr("mark_notification_read", {"message": 1}, "POST", "/api/v1/noti/close", '{"msg_id":1}')
_kr("clear_notifications", {"type": "warning"}, "POST", "/api/v1/noti/del", '{"alert_type":"warning"}')


# ── Telemetry ───────────────────────────────────────────────────────────
def _kr_tel(name, path, data, expect_state, **extra):
    body = json.dumps({"requestId": "8ee8ebb9", "code": 200, "msg": "OK", "data": data}, separators=(",", ":"))
    inbound = {"path": path, "body": body}
    if "request" in extra:
        inbound["request"] = extra.pop("request")
    telemetry(KR, name, inbound_http=inbound, expect_state=expect_state, **extra)


_kr_tel("self", "/api/v1/getinfo", {"modules": ["devMgr", "usrMgr"], "name": "admin", "roles": ["admin"], "userId": 1,
                                    "userName": "admin"},
        {"self": {"user_id": 1, "username": "admin", "name": "admin", "roles": '["admin"]',
                  "modules": '["devMgr","usrMgr"]'}})
_KR_USERS = {"count": 1, "pageIndex": 1, "pageSize": 1000, "list": [
    {"userId": 4, "username": "mtest", "nickName": "mtest", "status": "2", "tokenVersion": 3, "roleIds": [2, 1],
     "createBy": 1, "updateBy": 0, "createdAt": "2025-06-04T06:17:45.768388447Z",
     "updatedAt": "2025-06-04T14:26:01.791013418+08:00"}]}
_KR_USER4 = {"username": "mtest", "nickname": "mtest", "status": "2", "roles": "[2,1]",
             "created": "2025-06-04T06:17:45.768388447Z", "updated": "2025-06-04T14:26:01.791013418+08:00"}
_kr_tel("users", "/api/v1/sys-user?pageIndex=1&pageSize=1000", _KR_USERS, {"users": {"4": _KR_USER4}},
        state_before={"users": {"9": {"username": "gone"}}})
_kr_tel("users-page", "/api/v1/sys-user?pageIndex=2&pageSize=10", _KR_USERS,
        {"users": {"9": {"username": "kept"}, "4": _KR_USER4}}, state_before={"users": {"9": {"username": "kept"}}})
_kr_tel("roles", "/api/v1/role?pageIndex=1&pageSize=1000", {"count": 1, "pageIndex": 1, "pageSize": 1000, "list": [
    {"roleId": 1, "roleName": "System Administrators Group", "roleKey": "admin", "remark": "System default administrator",
     "modules": ["devMgr", "usrMgr"], "user_ids": [1, 4], "createdAt": "2025-01-17T08:38:47.277Z",
     "updatedAt": "2025-06-04T14:26:01.772113934+08:00"}]},
    {"roles": {"1": {"name": "System Administrators Group", "key": "admin", "remark": "System default administrator",
                     "modules": '["devMgr","usrMgr"]', "users": "[1,4]"}}},
    state_before={"roles": {"5": {"name": "gone"}}})
_kr_tel("network", "/api/v1/netmgr", [
    {"ifname": "eth0", "ip": "", "netmask": "", "gw": "", "dns": "1.2.4.8", "method": "manual",
     "mac": "6e:6e:4f:4f:4f:2c"},
    {"ifname": "enp1s0", "ip": "192.168.35.101", "netmask": "255.255.255.0", "gw": "", "dns": "1.2.4.8",
     "method": "manual", "mac": "de:0b:d1:ac:81:7a"}],
    {"network": {"interfaces": {
        "eth0": {"ip": "", "netmask": "", "gateway": "", "dns": "1.2.4.8", "method": "manual", "mac": "6e:6e:4f:4f:4f:2c"},
        "enp1s0": {"ip": "192.168.35.101", "netmask": "255.255.255.0", "gateway": "", "dns": "1.2.4.8",
                   "method": "manual", "mac": "de:0b:d1:ac:81:7a"}}}})
_kr_tel("lcd", "/api/v1/netmgr/lcd-checkConf", {"ifname": "enp1s0", "ip": "192.168.35.101", "netmask": "255.255.255.0",
                                                "gw": "", "dns": "1.2.4.8", "method": "static",
                                                "mac": "de:0b:d1:ac:81:7a"},
        {"lcd": {"interface": "enp1s0", "ip": "192.168.35.101", "netmask": "255.255.255.0", "gateway": "",
                 "dns": "1.2.4.8", "method": "static"}})
_kr_tel("system", "/api/v1/systemctrl/get", {"name": "Device258", "timetype": "ntp", "ntp": "ntp1.aliyun.com",
                                             "Timezone": "Asia/Shanghai", "time": "2024-12-23 12:37:03", "offset": 8},
        {"system": {"name": "Device258", "time_type": "ntp", "ntp": "ntp1.aliyun.com", "timezone": "Asia/Shanghai",
                    "time": "2024-12-23 12:37:03", "offset": 8}})
_kr_tel("devices", "/api/v1/klnk/gateway/output-devices", [
    {"id": "1234567891234", "slotid": 6, "model": "FMG400", "name": "FMG400", "devicemodel": "FMG400",
     "serialnumber": "1234567891234", "firmware": "1.00.0042", "versiontips": False, "status": 1,
     "network": [{"if": "eth0", "ip": "192.168.35.191", "mask": "255.255.255.0", "gw": "192.168.35.254",
                  "model": "", "tx": {"speed": 1.5, "pkt": 100, "error": 0},
                  "rx": {"speed": 2.5, "pkt": 200, "error": 1}}],
     "cpu": 12.5, "memory": 40, "uptime": 3600, "temp": 55, "disk": 10, "directoryId": 2, "devicetype": "mgw"}],
    {"devices": {"1234567891234": {
        "id": "1234567891234", "slot": 6, "model": "FMG400", "name": "FMG400", "device_model": "FMG400",
        "firmware": "1.00.0042", "update_available": False, "status": 1, "cpu": 12.5, "memory": 40, "uptime": 3600.0,
        "temperature": 55, "disk": 10, "directory": 2, "device_type": "mgw",
        "network": {"eth0": {"ip": "192.168.35.191", "netmask": "255.255.255.0", "gateway": "192.168.35.254",
                             "model": "", "tx_speed": 1.5, "rx_speed": 2.5, "tx_packets": 100, "rx_packets": 200,
                             "tx_errors": 0, "rx_errors": 1}}}}},
    state_before={"devices": {"000000108": {"name": "removed"}}})
_kr_tel("dashboard", "/api/v1/dashboard/data", {
    "total_count": 20,
    "model_stats": {"FMG400": {"units": 3, "offline": 0, "external_device": False},
                    "N60": {"units": 1, "offline": 0, "external_device": True}},
    "encoding": {"total_devices": 10, "online_devices": 10, "offline_devices": 0, "total_channels": 15,
                 "error_devices": 0, "update_time": "0001-01-01T00:00:00Z"},
    "bonding": {"total_devices": 1, "online_devices": 1, "offline_devices": 0, "total_channels": 1,
                "error_devices": 0, "update_time": "0001-01-01T00:00:00Z"},
    "update_time": "2025-06-27T18:08:26.012939403+08:00"},
    {"dashboard": {"total_count": 20, "updated": "2025-06-27T18:08:26.012939403+08:00",
                   "models": {"FMG400": {"units": 3, "offline": 0, "external": False},
                              "N60": {"units": 1, "offline": 0, "external": True}},
                   "services": {"encoding": {"total_devices": 10, "online_devices": 10, "offline_devices": 0,
                                             "total_channels": 15, "error_devices": 0,
                                             "updated": "0001-01-01T00:00:00Z"},
                                "bonding": {"total_devices": 1, "online_devices": 1, "offline_devices": 0,
                                            "total_channels": 1, "error_devices": 0,
                                            "updated": "0001-01-01T00:00:00Z"}}}})
_kr_tel("notification-summary", "/api/v1/noti/summary",
        {"warning_count": 80, "error_count": 0, "info_count": 27, "unread_count": 107},
        {"notifications_summary": {"warnings": 80, "errors": 0, "info": 27, "unread": 107}})
_KR_NOTI = [{"id": 2, "device_id": "device-001", "type": "warning",
             "content": "The CPU usage has exceeded the threshold.", "is_read": False,
             "timestamp": "2025-02-20T17:21:25.677014171+08:00"}]
_KR_NOTI_S = {"2": {"device": "device-001", "type": "warning", "content": "The CPU usage has exceeded the threshold.",
                    "read": False, "time": "2025-02-20T17:21:25.677014171+08:00"}}
_kr_tel("notifications", "/api/v1/noti/list", _KR_NOTI, {"notifications": _KR_NOTI_S},
        state_before={"notifications": {"1": {"type": "info"}}})
_kr_tel("notifications-of", "/api/v1/noti/list?msg_type=warning", _KR_NOTI,
        {"notifications": dict(_KR_NOTI_S, **{"1": {"type": "info"}})},
        state_before={"notifications": {"1": {"type": "info"}}})
_kr_tel("aggregation", "/api/v1/sw/aggr/dump", {"aggre-id": 0, "aggregated": [], "configured": [22, 23, 24],
                                                "name": "147", "type": "lacp", "enable": 1},
        {"aggregation": {"id": 0, "name": "147", "type": "lacp", "enabled": True, "configured": "[22,23,24]",
                         "aggregated": "[]"}})
_kr_tel("aggregation-ports", "/api/v1/sw/aggr/ports", [{"port": 22, "name": "port22", "type": "ge"}],
        {"aggregation_ports": {"22": {"name": "port22", "type": "ge"}}},
        state_before={"aggregation_ports": {"9": {"name": "gone"}}})


def _kr_reread(name, path, request, then):
    inbound = {"path": path, "body": "{}"}
    if request is not None:
        inbound["request"] = request
    telemetry(KR, name, inbound_http=inbound, expect_state={},
              expect_then_send=[{"method": "GET", "target": t} for t in then])


_KR_U = "/api/v1/sys-user?pageIndex=1&pageSize=1000"
_KR_R = "/api/v1/role?pageIndex=1&pageSize=1000"
_kr_reread("reread-user", "/api/v1/sys-user", {"username": "lenovo"}, [_KR_U, _KR_R])
_kr_reread("reread-role", "/api/v1/role/2", {"roleId": 2}, [_KR_R, "/api/v1/getinfo"])
_kr_reread("reread-network", "/api/v1/netmgr", {"ifname": "eth0"}, ["/api/v1/netmgr", "/api/v1/netmgr/lcd-checkConf"])
# The poll's own read of the interfaces queues nothing.
_kr_tel("network-no-reread", "/api/v1/netmgr", [], {}, expect_then_send=[])
_kr_reread("reread-system", "/api/v1/systemctrl/baseConf", {"name": "x"}, ["/api/v1/systemctrl/get"])
_kr_reread("reread-devices", "/api/v1/klnk/rename", {"serial_number": "1"},
           ["/api/v1/klnk/gateway/output-devices", "/api/v1/dashboard/data"])
_kr_reread("reread-move", "/api/v1/directory/move/batch", {"targetId": 2}, ["/api/v1/klnk/gateway/output-devices"])
_kr_reread("reread-aggregation", "/api/v1/sw/aggr/set", None, ["/api/v1/sw/aggr/dump"])
_kr_reread("reread-notifications", "/api/v1/noti/close", {"msg_id": 1}, ["/api/v1/noti/summary", "/api/v1/noti/list"])

# Every other vector leaves the password empty, so each command's own
# request is the one compared.
for _kr_v in V:
    if _kr_v["spec"] == "kiloview-rf02" and _kr_v.get("command") != "get_self":
        _kr_v.setdefault("settings", {}).setdefault("password", "")
