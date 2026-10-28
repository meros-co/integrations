# Kiloview KiloLink Server Pro: /api/tools/*.json (KiloLink Server Pro API,
# 2025.6). Parameters are the document's tables, sent as JSON bodies; the
# replies follow its examples and Appendix 1.
KL = "kiloview-kilolink-server"
_KL_OK = {"status": 200, "body": '{"result":"ok","msg":""}'}
_KL_ACK = {"ok": {"kind": "ack"}}
_KL_DEV = "cn=device1,ou=KVDevices,cn=admin,dc=kiloview,dc=com"


def _kl(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
        req["content_type"] = "application/json"
    V.append({"spec": KL, "command": command, "input": input, "expect_request": req, **extra})


_kl("login", {"username": "admin", "password": "kiloview001"}, "POST", "/api/tools/login.json",
    '{"username":"admin","password":"kiloview001"}',
    http_reply={"status": 200, "body": '{"token":"sdaf3!AS413@"}'},
    expect_result={"ok": {"kind": "value", "value": "sdaf3!AS413@"}})
V.append({"spec": KL, "command": "get_own_menu", "input": {}, "settings": {"token": "sdaf3!AS413@", "username": "ocl"},
          "expect_request": {"method": "GET", "target": "/api/tools/getSelfMenu.json"}})
_kl("get_group_menus", {}, "GET", "/api/tools/getRdControl.json")
_kl("set_group_menu", {"group": "cn=kiloview,ou=KVGroups,cn=admin,dc=kiloview,dc=com", "device_control": True,
                       "user_control": True, "upload_control": True, "send_control": True}, "POST",
    "/api/tools/setRdControl.json",
    '{"deviceControl":true,"userControl":true,"uploadControl":true,"sendControl":true,'
    '"dn":"cn=kiloview,ou=KVGroups,cn=admin,dc=kiloview,dc=com"}', http_reply=_KL_OK, expect_result=_KL_ACK)
# An Appendix 4 error fails the command.
_kl("set_group_menu", {"group": "cn=x", "device_control": False, "user_control": False, "upload_control": False,
                       "send_control": False}, "POST", "/api/tools/setRdControl.json",
    '{"deviceControl":false,"userControl":false,"uploadControl":false,"sendControl":false,"dn":"cn=x"}',
    file="set_group_menu.error", http_reply={"status": 200, "body": '{"result":"error","msg":"1116"}'},
    expect_result={"error": {"error": "device_error"}})
_kl("get_tree", {}, "POST", "/api/tools/searchFix.json", "{}")
_kl("get_tree_under", {"dn": "cn=admin,dc=kiloview,dc=com"}, "POST", "/api/tools/searchFix.json",
    '{"dn":"cn=admin,dc=kiloview,dc=com"}')
_kl("add_node", {"parent": "ou=KVDevices,cn=admin,dc=kiloview,dc=com", "type": "device",
                 "cfg": {"cn": "111", "ipHostNumber": "192.168.0.1", "serialNumber": "2021121616",
                         "description": "123123"}}, "POST", "/api/tools/add.json",
    '{"dn":"ou=KVDevices,cn=admin,dc=kiloview,dc=com","cfg":{"cn":"111","ipHostNumber":"192.168.0.1",'
    '"serialNumber":"2021121616","description":"123123"},"type":"device"}')
_kl("add_device", {"name": "111", "serial_number": "2021121616", "key": "123123"}, "POST", "/api/tools/add.json",
    '{"dn":"ou=KVDevices,cn=admin,dc=kiloview,dc=com","cfg":{"cn":"111","ipHostNumber":"192.168.0.1",'
    '"serialNumber":"2021121616","description":"123123","o":""},"type":"device"}')
_kl("add_group", {"name": "g1", "members": ["uid=ocl,ou=KVUsers,cn=admin,dc=kiloview,dc=com"]}, "POST",
    "/api/tools/add.json",
    '{"dn":"ou=KVGroups,cn=admin,dc=kiloview,dc=com","cfg":{"cn":"g1",'
    '"uniqueMember":["uid=ocl,ou=KVUsers,cn=admin,dc=kiloview,dc=com"]},"type":"group"}')
_kl("add_organization", {"parent": "cn=admin,dc=kiloview,dc=com", "name": "Studio"}, "POST", "/api/tools/add.json",
    '{"dn":"cn=admin,dc=kiloview,dc=com","cfg":{"ou":"Studio"},"type":"organization"}')
_kl("delete_node", {"dn": _KL_DEV}, "POST", "/api/tools/delete.json", '{"dn":"' + _KL_DEV + '"}')
_kl("delete_nodes", {"list": ["dn1", "dn2"]}, "POST", "/api/tools/deleteMany.json", '{"list":["dn1","dn2"]}')
_kl("modify_node", {"dn": _KL_DEV, "type": "device",
                    "cfg": {"cn": "device1", "ipHostNumber": "192.168.0.1", "serialNumber": "1",
                            "description": "k"}}, "POST", "/api/tools/modify.json",
    '{"dn":"' + _KL_DEV + '","cfg":{"cn":"device1","ipHostNumber":"192.168.0.1","serialNumber":"1",'
    '"description":"k"},"type":"device"}')
_kl("rename_node", {"dn": _KL_DEV, "name": "device2"}, "POST", "/api/tools/rename.json",
    '{"dn":"' + _KL_DEV + '","new":"device2"}')
_kl("move_node", {"dn": _KL_DEV, "parent": "ou=Studio,cn=admin,dc=kiloview,dc=com"}, "POST", "/api/tools/rename.json",
    '{"dn":"' + _KL_DEV + '","parent":"ou=Studio,cn=admin,dc=kiloview,dc=com"}')
_kl("move_devices", {"dns": [_KL_DEV], "parent": "ou=Studio,cn=admin,dc=kiloview,dc=com"}, "POST",
    "/api/tools/move.json", '{"dns":["' + _KL_DEV + '"],"parent":"ou=Studio,cn=admin,dc=kiloview,dc=com"}')
_kl("set_device_access", {"dn": _KL_DEV, "aci": ["aci-1"]}, "POST", "/api/tools/aciSet.json",
    '{"dn":"' + _KL_DEV + '","OpenLDAPaci":["aci-1"]}')
_kl("get_firmware_list", {}, "POST", "/api/tools/getFirmwareList.json", "{}")
_kl("find_firmware", {"search": "E1", "page": 1, "limit": 10}, "POST", "/api/tools/getFirmwareList.json",
    '{"search":"E1","page":1,"limit":10}')
_kl("get_firmware_types", {}, "POST", "/api/tools/getFirmwareTypes.json", "{}",
    http_reply={"status": 200, "body": '{"result":"ok","msg":"","data":["E1","E2"]}'},
    expect_result={"ok": {"kind": "value", "value": ["E1", "E2"]}})
_kl("set_firmware_description", {"type": "E1", "version": "4.7.2515", "description": "E1 Firmware"}, "POST",
    "/api/tools/modifyFirmware.json", '{"desc":"E1 Firmware","type":"E1","version":"4.7.2515"}')
_kl("get_devices", {}, "POST", "/api/tools/getDeviceList.json", "{}")
_kl("get_devices_under", {"dn": "ou=KVDevices,cn=admin,dc=kiloview,dc=com"}, "POST",
    "/api/tools/getDeviceList.json", '{"dn":"ou=KVDevices,cn=admin,dc=kiloview,dc=com","device_type":"all"}')
_kl("get_devices_by_mode", {"device_type": "N40", "device_mode": "encoder"}, "POST",
    "/api/tools/getDeviceList.json",
    '{"dn":"cn=admin,dc=kiloview,dc=com","device_type":"N40","device_mode":"encoder"}')
_kl("change_own_password", {"password": "Kiloview002"}, "POST", "/api/tools/changeSelfPassword.json",
    '{"new":"Kiloview002"}')
_kl("change_own_nickname", {"nickname": "new_nickname"}, "POST", "/api/tools/changeSelfInfo.json",
    '{"cfg":{"cn":"new_nickname"}}')
_kl("reboot_devices", {"list": ["dn1", "dn2"]}, "POST", "/api/tools/rebootDevices.json", '{"list":["dn1","dn2"]}')
_kl("reset_devices", {"list": ["dn1"]}, "POST", "/api/tools/resetDevices.json", '{"list":["dn1"]}')
_kl("upgrade_devices", {"list": ["dn1"], "type": "E1", "version": "4.7.2515"}, "POST",
    "/api/tools/upgradeDevices.json", '{"list":["dn1"],"type":"E1","version":"4.7.2515"}')
_kl("cancel_upgrade", {"list": ["dn1"]}, "POST", "/api/tools/closeUpgrade.json", '{"list":["dn1"]}')
_kl("set_stream", {"dn": _KL_DEV, "key": "k1", "stream": "1", "enabled": True}, "POST",
    "/api/tools/controlDeviceStream.json",
    '{"dn":"' + _KL_DEV + '","key":"k1","stream":"1","enabled":true,"all":false}')
_kl("set_all_streams", {"dn": _KL_DEV, "enabled": False}, "POST", "/api/tools/controlDeviceStream.json",
    '{"dn":"' + _KL_DEV + '","enabled":false,"all":true}')
_kl("get_device_modes", {}, "POST", "/api/tools/getDeviceModes.json", "{}",
    http_reply={"status": 200, "body": '{"result":"ok","data":["encoder","decoder","IO"]}'},
    expect_result={"ok": {"kind": "value", "value": ["encoder", "decoder", "IO"]}})
_kl("get_device_types", {}, "POST", "/api/tools/getDeviceTypes.json", "{}")
_kl("create_random_key", {}, "POST", "/api/tools/createRandomKey.json", "{}",
    http_reply={"status": 200, "body": '{"result":"ok","data":"8918e4fdfd7d7d079ef502782cc5f572"}'},
    expect_result={"ok": {"kind": "value", "value": "8918e4fdfd7d7d079ef502782cc5f572"}})
_kl("create_device_key", {"serial_number": "2021121616"}, "POST", "/api/tools/createRandomKey.json",
    '{"serialNumber":"2021121616"}')


# ── Telemetry ───────────────────────────────────────────────────────────
def _kl_tel(name, path, body, expect_state, request=None, **extra):
    inbound = {"path": path, "body": json.dumps(body, separators=(",", ":"), ensure_ascii=False)}
    if request is not None:
        inbound["request"] = request
    telemetry(KL, name, inbound_http=inbound, expect_state=expect_state, **extra)


_kl_tel("own-menu", "/api/tools/getSelfMenu.json",
        {"deviceControl": True, "userControl": False, "uploadControl": False, "sendControl": True},
        {"menu": {"device_control": True, "user_control": False, "upload_control": False, "send_control": True}})
_kl_tel("group-menus", "/api/tools/getRdControl.json",
        {"groupName1": {"deviceControl": True, "userControl": False, "uploadControl": False, "sendControl": True}},
        {"group_menus": {"groupName1": {"device_control": True, "user_control": False, "upload_control": False,
                                        "send_control": True}}},
        state_before={"group_menus": {"old": {"device_control": True}}})
_kl_tel("device-counts", "/api/tools/getDeviceList.json",
        {"result": "ok", "data": {"total": 12, "online": 9, "offline": 3, "list": []}},
        {"devices": {"total": 12, "online": 9, "offline": 3}}, request={})
# A filtered list keeps the counts of the whole directory as they were.
_kl_tel("device-counts-filtered", "/api/tools/getDeviceList.json",
        {"result": "ok", "data": {"total": 2, "online": 2, "offline": 0, "list": []}},
        {}, request={"dn": "ou=Studio,cn=admin,dc=kiloview,dc=com", "device_type": "all", "device_mode": "encoder"})
_kl_tel("firmware-types", "/api/tools/getFirmwareTypes.json", {"result": "ok", "msg": "", "data": ["E1", "E2"]},
        {"firmware_types": '["E1","E2"]'})
_kl_tel("firmware-list", "/api/tools/getFirmwareList.json", {"result": "ok", "msg": "", "data": [
    {"type": "E1", "children": [{"version": "4.7.2515", "desc": "E1 Firmware", "uploadTime": 1649303521}]}]},
    {"firmware": {"E1": {"0": {"version": "4.7.2515", "description": "E1 Firmware", "uploaded": 1649303521}}}},
    request={}, state_before={"firmware": {"E2": {"0": {"version": "1.0"}}}})
# Appendix 1, shortened to one node in each organization.
_kl_tel("tree", "/api/tools/searchFix.json", {"result": "ok", "data": {
    "ou=KVGroups": {"ou": "KVGroups", "objectClass": ["organizationalUnit"], "parentDN": "cn=admin,dc=kiloview,dc=com",
                    "type": "organization", "dn": "ou=KVGroups,cn=admin,dc=kiloview,dc=com", "children": [
                        {"description": "desc", "uniqueMember": ["uid=ocl,ou=KVUsers,cn=admin,dc=kiloview,dc=com"],
                         "cn": "g1", "children": {}, "parentDN": "ou=KVGroups,cn=admin,dc=kiloview,dc=com",
                         "objectClass": ["groupOfUniqueNames", "top"], "o": "1649389337", "type": "group",
                         "dn": "cn=g1,ou=KVGroups,cn=admin,dc=kiloview,dc=com", "userControl": True}]},
    "ou=KVDevices": {"ou": "KVDevices", "objectClass": ["organizationalUnit"], "parentDN": "cn=admin,dc=kiloview,dc=com",
                     "type": "organization", "dn": "ou=KVDevices,cn=admin,dc=kiloview,dc=com", "children": [
                         {"description": "123123", "ipHostNumber": "192.168.0.1", "cn": "111", "children": {},
                          "parentDN": "ou=KVDevices,cn=admin,dc=kiloview,dc=com", "objectClass": ["device", "ipHost"],
                          "type": "device", "dn": "cn=111,ou=KVDevices,cn=admin,dc=kiloview,dc=com",
                          "serialNumber": "2021121616"},
                         {"ou": "Studio", "children": {}, "parentDN": "ou=KVDevices,cn=admin,dc=kiloview,dc=com",
                          "type": "organization", "dn": "ou=Studio,ou=KVDevices,cn=admin,dc=kiloview,dc=com"}]}}},
    {"organizations": {"ou=KVGroups": {"dn": "ou=KVGroups,cn=admin,dc=kiloview,dc=com"},
                       "ou=KVDevices": {"dn": "ou=KVDevices,cn=admin,dc=kiloview,dc=com"}},
     "tree": {"cn=g1,ou=KVGroups,cn=admin,dc=kiloview,dc=com": {
                  "name": "g1", "type": "group", "parent": "ou=KVGroups,cn=admin,dc=kiloview,dc=com",
                  "description": "desc", "remark": "1649389337",
                  "members": '["uid=ocl,ou=KVUsers,cn=admin,dc=kiloview,dc=com"]'},
              "cn=111,ou=KVDevices,cn=admin,dc=kiloview,dc=com": {
                  "name": "111", "type": "device", "parent": "ou=KVDevices,cn=admin,dc=kiloview,dc=com",
                  "description": "123123", "ip": "192.168.0.1", "serial_number": "2021121616"},
              "ou=Studio,ou=KVDevices,cn=admin,dc=kiloview,dc=com": {
                  "name": "Studio", "type": "organization", "parent": "ou=KVDevices,cn=admin,dc=kiloview,dc=com"}}},
    request={}, state_before={"tree": {"cn=gone,ou=KVDevices,cn=admin,dc=kiloview,dc=com": {"name": "gone"}}})


_KL_LIST = {"method": "POST", "target": "/api/tools/getDeviceList.json", "body": "{}"}
_KL_TREE = {"method": "POST", "target": "/api/tools/searchFix.json", "body": "{}"}
_KL_FW = {"method": "POST", "target": "/api/tools/getFirmwareList.json", "body": "{}"}
_kl_tel("reread-add", "/api/tools/add.json", {"result": "ok", "msg": ""}, {}, request={"dn": "x"},
        expect_then_send=[_KL_TREE, _KL_LIST])
_kl_tel("reread-upgrade", "/api/tools/upgradeDevices.json", {"result": "ok", "msg": ""}, {}, request={"list": []},
        expect_then_send=[_KL_FW, _KL_LIST])
_kl_tel("reread-stream", "/api/tools/controlDeviceStream.json", {"result": "ok", "msg": ""}, {},
        request={"dn": "x"}, expect_then_send=[_KL_LIST])
_kl_tel("reread-menu", "/api/tools/setRdControl.json", {"result": "ok", "msg": ""}, {}, request={"dn": "x"},
        expect_then_send=[{"method": "GET", "target": "/api/tools/getRdControl.json"},
                          {"method": "GET", "target": "/api/tools/getSelfMenu.json"}])
_kl_tel("reread-refused", "/api/tools/delete.json", {"result": "error", "msg": "1053"}, {}, request={"dn": "x"},
        expect_then_send=[])
