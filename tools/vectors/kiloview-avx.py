# Kiloview AVX24-4 Media HUB: JSON over HTTP under /v1 (AVX24-4 Media HUB
# WEB API Documentation v1.0). Requests follow the document's request
# examples: JSON bodies for POSTs, query strings for GETs (the document's
# URL examples), multipart form data for the icon upload; replies its
# {code, message, data} examples.
_KA = "kiloview-avx"
_KA_OK = {"status": 200, "body": '{"code":200,"message":"ok","data":{}}'}
_KA_ACK = {"ok": {"kind": "ack"}}


def _ka_j(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def _ka_data(data, **top):
    return {"status": 200, "body": _ka_j({"code": 200, "message": "ok", "data": data, **top})}


def _ka_post(command, input, path, body, **extra):
    V.append({"spec": _KA, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": path, "body": body,
                                 "content_type": "application/json"}, **extra})


def _ka_get(command, input, target, **extra):
    http(_KA, command, input, "GET", target, **extra)


def _ka_form(parts):
    """multipart/form-data as the core writes it (SPEC.md §4, "Binary
    request bodies"): parts of (name, bytes, filename, content type)."""
    import hashlib
    digest = hashlib.sha256(b"".join(n.encode() + b"\0" + d + b"\0" for n, d, _, _ in parts)).hexdigest()
    boundary = "----form-" + digest[:16]
    body = b""
    for name, data, filename, ctype in parts:
        body += f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"".encode()
        if filename:
            body += f"; filename=\"{filename}\"".encode()
        body += b"\r\n"
        if ctype:
            body += f"Content-Type: {ctype}\r\n".encode()
        body += b"\r\n" + data + b"\r\n"
    body += f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


# ── 1 Users ─────────────────────────────────────────────────────────────
_ka_post("login", {"username": "admin", "password": "admin"}, "/v1/user/login",
         '{"username":"admin","password":"admin"}',
         http_reply=_ka_data({"alias": "admin", "token": "eyJ.avx.tok"}),
         expect_result={"ok": {"kind": "value", "value": "eyJ.avx.tok"}})
# A refused write: a code other than 200 in the envelope.
_ka_post("add_user", {"username": "csadmin", "alias": "csadmin", "password": "123", "role": "admin"},
         "/v1/user/add", '{"alias":"csadmin","password":"123","role":"admin","username":"csadmin"}',
         http_reply=_KA_OK, expect_result=_KA_ACK)
_ka_post("add_user", {"username": "x", "alias": "x", "password": "1"}, "/v1/user/add",
         '{"alias":"x","password":"1","role":"common","username":"x"}', file="add_user.error",
         http_reply={"status": 200, "body": '{"code":70001,"message":"exists","data":null}'},
         expect_result={"error": {"error": "device_error", "code": "70001"}})
_ka_post("delete_user", {"user": 2}, "/v1/user/delete", '{"ids":[2]}')
_ka_post("delete_users", {"ids": [2, 3, 4]}, "/v1/user/delete", '{"ids":[2,3,4]}')
_ka_post("modify_user", {"user": 0, "alias": "csadmin_new", "password": "newpassword123", "role": "common"},
         "/v1/user/modify", '{"alias":"csadmin_new","id":0,"password":"newpassword123","role":"common"}')
_ka_get("get_users", {}, "/v1/user/userlist")

# ── 2 System ────────────────────────────────────────────────────────────
_ka_get("set_hostname", {"hostname": "AVX-Core-Switch-001"}, "/v1/sys/sethostname?hostname=AVX-Core-Switch-001",
        http_reply=_KA_OK, expect_result=_KA_ACK)
_ka_get("factory_reset", {}, "/v1/sys/refactory")
_ka_get("get_system_log", {}, "/v1/sys/syslog", http_reply=_ka_data("2024-01-01 00:00:00 system boot success"),
        expect_result={"ok": {"kind": "value", "value": "2024-01-01 00:00:00 system boot success"}})
_ka_get("reboot", {}, "/v1/sys/reboot")
_ka_post("upload_firmware", {"file": "AQIDBA==", "filename": "AVX-Firmware.bin"}, "/v1/sys/upload",
         '{"u_file":"AQIDBA==","filename":"AVX-Firmware.bin"}')
_ka_post("upgrade_firmware", {"filename": "AVX-Firmware.bin"}, "/v1/sys/upgrade", '{"filename":"AVX-Firmware.bin"}')
_ka_get("get_runtime_log", {}, "/v1/sys/getlog")
_ka_get("export_operation_log", {}, "/v1/sys/operation/export", http_reply=_ka_data("/data/operation/operation.txt"),
        expect_result={"ok": {"kind": "value", "value": "/data/operation/operation.txt"}})
_ka_get("get_time", {}, "/v1/systime/get")
_ka_post("set_timezone", {"timezone": "Asia/Shanghai", "offset": 8}, "/v1/systime/setLocation",
         '{"timezone":"Asia/Shanghai","offset":8}')
_ka_post("set_time_ntp", {"offset": 8, "ntp": "time.windows.com time.apple.com pool.ntp.org"},
         "/v1/systime/setLocation", '{"timetype":"ntp","offset":8,"ntp":"time.windows.com time.apple.com pool.ntp.org"}')
_ka_post("set_time_manual", {"offset": 8, "time": "2026-04-16 15:07:44"}, "/v1/systime/setLocation",
         '{"timetype":"manual","offset":8,"time":"2026-04-16 15:07:44"}')
_ka_post("set_time_pc", {"offset": 8}, "/v1/systime/setLocation", '{"timetype":"pc","offset":8}')
_ka_get("get_ptp", {}, "/v1/ptp/query")
_ka_post("enable_ptp_clock", {"clock": 2, "enabled": False}, "/v1/ptp/save", '{"ptps":[{"id":2,"enable":false}]}')
_ka_post("enable_ptp_clocks", {"ptps": [{"id": 1, "enable": True}, {"id": 2, "enable": False}]}, "/v1/ptp/save",
         '{"ptps":[{"id":1,"enable":true},{"id":2,"enable":false}]}')
_ka_get("get_ptp_clock", {"clock": 1}, "/v1/ptp/detail/query?id=1")
_ka_post("modify_ptp_clock", {"clock": 1, "type": "BC", "domain": 0, "two_step": True, "priority1": 128,
                              "priority2": 128, "leap_second": False,
                              "ports": [{"id": 1, "mode": "e2e", "master": True}]},
         "/v1/ptp/modify",
         '{"id":1,"type":"BC","domain":0,"two_step":true,"priority1":128,"priority2":128,"leap_s":false,'
         '"ports":[{"id":1,"mode":"e2e","master":true}]}')

# ── 3 Presets, virtual NICs, mode ───────────────────────────────────────
_ka_get("get_presets", {}, "/v1/preset/getlist")
_ka_post("save_preset", {"name": "high_performance_v2"}, "/v1/preset/save", '{"prname":"high_performance_v2"}')
_ka_post("load_preset", {"name": "default_config"}, "/v1/preset/read", '{"prname":"default_config"}')
_KA_DEL = {"delete_list": ["old_config"], "notfound": [], "failed": [], "message": "ok"}
_ka_post("delete_preset", {"name": "old_config"}, "/v1/preset/delete", '{"namelist":["old_config"]}',
         http_reply={"status": 200, "body": _ka_j({"code": 200, "message": "ok", "delete_info": _KA_DEL})},
         expect_result={"ok": {"kind": "value", "value": _KA_DEL}})
_ka_post("delete_presets", {"names": ["old_config", "test_mode_v1", "invalid_preset"]}, "/v1/preset/delete",
         '{"namelist":["old_config","test_mode_v1","invalid_preset"]}')
_ka_post("rename_preset", {"name": "performance_mode", "new_name": "high_performance_v1"}, "/v1/preset/modify",
         '{"old_name":"performance_mode","new_name":"high_performance_v1"}')
_ka_get("get_virtual_nic", {"vlan": 10}, "/v1/preset/vlan/interface/get?vlan_id=10")
_ka_get("get_virtual_nics", {}, "/v1/preset/vlan/interface/list/get")
_ka_post("set_virtual_nic", {"vlan": 10, "enabled": True, "ip": "192.168.10.1", "netmask": "255.255.255.0",
                             "type": "static", "default_route": False},
         "/v1/preset/vlan/interface/set",
         '{"vlan_id":10,"interface":{"enable":true,"ip":"192.168.10.1","netmask":"255.255.255.0","type":"static",'
         '"is_default_route":false}}')
_ka_get("get_interface_ips", {}, "/v1/switch/interface/ip/get")
_ka_get("get_switch_mode", {}, "/v1/preset/mode/get", http_reply=_ka_data({"mode": "layer3"}),
        expect_result={"ok": {"kind": "value", "value": "layer3"}})
_ka_post("set_switch_mode", {"mode": "layer3"}, "/v1/preset/mode/set", '{"mode":"layer3"}')

# ── 4.1 Discovery ───────────────────────────────────────────────────────
_ka_post("add_discovery_server", {"server_ip": "192.168.30.125"}, "/v1/input/server/add",
         '{"dis_g_name":"public","dis_s_ip":"192.168.30.125"}')
_ka_post("add_manual_discovery", {"device_ip": "192.168.40.23"}, "/v1/input/manual/add",
         '{"manuals":[{"dis_g_name":"public","dis_d_ip":"192.168.40.23"}]}')
_ka_post("add_manual_discoveries", {"manuals": [{"dis_g_name": "public", "dis_d_ip": "192.168.40.23"}]},
         "/v1/input/manual/add", '{"manuals":[{"dis_g_name":"public","dis_d_ip":"192.168.40.23"}]}')
_ka_post("add_auto_discovery", {"group": "public"}, "/v1/input/auto/add", '{"dis_g_name":"public"}')
_ka_get("discover_server", {"group": "group1", "server_ip": "192.168.1.1"},
        "/v1/input/server/discovery?dis_g_name=group1&dis_s_ip=192.168.1.1")
_ka_get("discover_manual", {}, "/v1/input/manual/discovery")
_ka_get("discover_auto", {"group": "group 1"}, "/v1/input/auto/discovery?dis_g_name=group%201")
_ka_post("delete_discovery", {"discovery": 3}, "/v1/input/discovery/delete", '{"ids":[3]}')
_ka_post("delete_discoveries", {"ids": [1, 2, 3]}, "/v1/input/discovery/delete", '{"ids":[1,2,3]}')
_ka_get("get_discovery_methods", {}, "/v1/input/discovery/all")

# ── 4.2 Inputs, 4.3 Outputs ─────────────────────────────────────────────
_ka_post("modify_input", {"input": 1, "alias": "Camera 1"}, "/v1/input/modify", '{"id":1,"alias":"Camera 1"}')
_ka_post("add_output", {"name": "output1", "alias": "Output 1", "channel_name": "Channel 1", "group": "group1",
                        "routed": True, "discovery_server_ip": "192.168.1.1", "vlan": 100},
         "/v1/output/add",
         '{"outputs":[{"name":"output1","alias":"Output 1","c_name":"Channel 1","g_name":"group1","router":true,'
         '"dis_s_ip":"192.168.1.1","vlan_id":100}]}')
_ka_post("add_outputs", {"outputs": [{"name": "o2", "alias": "o2", "c_name": "c2", "g_name": "public",
                                      "router": False, "dis_s_ip": "", "vlan_id": 1}]},
         "/v1/output/add",
         '{"outputs":[{"name":"o2","alias":"o2","c_name":"c2","g_name":"public","router":false,"dis_s_ip":"",'
         '"vlan_id":1}]}')
_ka_post("delete_output", {"output": 2}, "/v1/output/delete", '{"ids":[2]}')
_ka_post("delete_outputs", {"ids": [1, 2, 3]}, "/v1/output/delete", '{"ids":[1,2,3]}')
_ka_post("modify_output", {"output": 1, "name": "new_name", "group": "new_group", "discovery_server_ip": "",
                           "routed": False, "vlan": 200},
         "/v1/output/modify",
         '{"id":1,"name":"new_name","g_name":"new_group","dis_s_ip":"","router":false,"vlan_id":200}')
_ka_get("get_output", {"output": 1}, "/v1/output/query?id=1")
_ka_get("get_outputs", {}, "/v1/output/query/all")
_ka_get("get_default_output", {}, "/v1/output/query/default", http_reply=_ka_data({"name": "output1"}),
        expect_result={"ok": {"kind": "value", "value": "output1"}})

# ── 4.4-4.8 Panels ──────────────────────────────────────────────────────
_ka_post("add_panel", {"name": "panel1"}, "/v1/panel/add", '{"name":"panel1"}')
_ka_post("delete_panel", {"panel": 3}, "/v1/panel/delete", '{"ids":[3]}')
_ka_post("delete_panels", {"ids": [1, 2, 3]}, "/v1/panel/delete", '{"ids":[1,2,3]}')
_ka_post("rename_panel", {"panel": 1, "name": "new_panel_name"}, "/v1/panel/modify",
         '{"id":1,"name":"new_panel_name"}')
_ka_post("expand_panel", {"panel": 1}, "/v1/panel/expand", '{"id":1,"expand":true}')
_ka_get("get_panel", {"panel": 1}, "/v1/panel/query?id=1")
_ka_get("get_panels", {}, "/v1/panel/query/all")
_KA_SRC = {"name": "input1", "alias": "Input 1", "c_name": "Channel 1", "g_name": "group1", "d_ip": "192.168.1.100",
           "d_port": 554, "url": "192.168.1.100:554", "enable": True, "dis_method": "manual", "dis_g_name": "public",
           "dis_d_ip": "192.168.1.100", "dis_s_ip": "", "vlan_id": 100, "switcher_id": "0000000000000"}
_ka_post("add_panel_inputs", {"panel": 1, "inputs": [_KA_SRC]}, "/v1/panel/input/add",
         '{"id":1,"inputs":[' + _ka_j(_KA_SRC) + "]}")
_ka_post("remove_panel_input", {"panel": 1, "input": 2}, "/v1/panel/input/remove", '{"id":1,"input_ids":[2]}')
_ka_post("remove_panel_inputs", {"panel": 1, "ids": [1, 2, 3]}, "/v1/panel/input/remove",
         '{"id":1,"input_ids":[1,2,3]}')
_ka_post("sort_panel_inputs", {"panel": 1, "ids": [3, 1, 2]}, "/v1/panel/input/sort", '{"id":1,"input_ids":[3,1,2]}')
_ka_post("add_panel_output", {"panel": 1, "output": 4}, "/v1/panel/output/add", '{"id":1,"output_ids":[4]}')
_ka_post("add_panel_outputs", {"panel": 1, "ids": [1, 2, 3]}, "/v1/panel/output/add", '{"id":1,"output_ids":[1,2,3]}')
_ka_post("remove_panel_output", {"panel": 1, "output": 2}, "/v1/panel/output/remove", '{"id":1,"output_ids":[2]}')
_ka_post("remove_panel_outputs", {"panel": 1, "ids": [1, 2]}, "/v1/panel/output/remove",
         '{"id":1,"output_ids":[1,2]}')
_ka_post("sort_panel_outputs", {"panel": 1, "ids": [3, 1, 2]}, "/v1/panel/output/sort",
         '{"id":1,"output_ids":[3,1,2]}')
_ka_get("get_panel_detail", {"panel": 1}, "/v1/panel/detail?id=1")
_ka_post("route", {"panel": 1, "output": 1, "input": 2}, "/v1/panel/cross/output/input",
         '{"id":1,"output_id":1,"input_id":2}', http_reply=_KA_OK, expect_result=_KA_ACK)
_ka_post("select_input_all", {"panel": 1, "input": 1}, "/v1/panel/cross/input/select",
         '{"id":1,"input_id":1,"select":true}')
_ka_get("get_panel_input_config", {"panel": 1}, "/v1/panel/cfg/input/all?id=1")
_ka_get("get_panel_output_config", {"panel": 1}, "/v1/panel/cfg/output/all?id=1")
_ka_post("apply_input_config", {"panel": 1, "input": 1, "icon": 1, "front_content": "input1"},
         "/v1/panel/cfg/input/apply", '{"id":1,"input_id":1,"icon_id":1,"front_content":"input1"}')
_ka_post("apply_output_config", {"panel": 1, "output": 1, "icon": 1, "front_content": "output1"},
         "/v1/panel/cfg/output/apply", '{"id":1,"output_id":1,"icon_id":1,"front_content":"output1"}')
_ka_post("enable_physical_panel", {"panel": 1}, "/v1/panel/cfg/enable/set", '{"id":1,"enable":true}')
_ka_get("get_physical_panel_enabled", {"panel": 1}, "/v1/panel/cfg/enable/query?id=1",
        http_reply=_ka_data({"enable": True}), expect_result={"ok": {"kind": "value", "value": True}})

# ── 5 KLS panels ────────────────────────────────────────────────────────
_ka_post("kls_add_discovery_server", {"serial": "KLSSERIAL001", "server_ip": "192.168.1.1"},
         "/v1/kls/input/server/add", '{"serial_number":"KLSSERIAL001","dis_g_name":"public","dis_s_ip":"192.168.1.1"}')
_ka_post("kls_add_manual_discovery", {"serial": "KLSSERIAL001", "device_ip": "192.168.1.100"},
         "/v1/kls/input/manual/add",
         '{"serial_number":"KLSSERIAL001","manuals":[{"dis_g_name":"public","dis_d_ip":"192.168.1.100"}]}')
_ka_post("kls_add_manual_discoveries", {"serial": "KLSSERIAL001",
                                        "manuals": [{"dis_g_name": "public", "dis_d_ip": "192.168.1.100"}]},
         "/v1/kls/input/manual/add",
         '{"serial_number":"KLSSERIAL001","manuals":[{"dis_g_name":"public","dis_d_ip":"192.168.1.100"}]}')
_ka_post("kls_add_auto_discovery", {"serial": "KLSSERIAL001", "group": "public"}, "/v1/kls/input/auto/add",
         '{"serial_number":"KLSSERIAL001","dis_g_name":"public"}')
_KA_KD = [{"dis_method": "manual", "dis_g_name": "public", "dis_d_ip": "192.168.1.100", "dis_s_ip": ""}]
_ka_post("kls_delete_discoveries", {"serial": "KLSSERIAL001", "discoveries": _KA_KD}, "/v1/kls/input/discovery/delete",
         '{"serial_number":"KLSSERIAL001","discoverys":' + _ka_j(_KA_KD) + "}")
_KA_KO = {"uuid": "485441684654546484654", "name": "kls-output-1", "alias": "KLS Output 1", "c_name": "Channel 1",
          "g_name": "group1", "router": True, "dis_s_ip": "192.168.1.1", "vlan_id": 100,
          "switcher_id": "0000000000000"}
_ka_post("kls_add_outputs", {"outputs": [_KA_KO]}, "/v1/kls/output/add", '{"outputs":[' + _ka_j(_KA_KO) + "]}",
         http_reply=_ka_data({"outputs": [_KA_KO], "matrixs": []}),
         expect_result={"ok": {"kind": "value", "value": {"outputs": [_KA_KO], "matrixs": []}}})
_ka_post("kls_delete_output", {"uuid": "485441684654546484654"}, "/v1/kls/output/delete",
         '{"uuids":["485441684654546484654"]}')
_ka_post("kls_delete_outputs", {"uuids": ["a", "b"]}, "/v1/kls/output/delete", '{"uuids":["a","b"]}')
_ka_post("kls_modify_output", {"uuid": "485441684654546484654", "name": "new-name", "vlan": 200, "group": "new-group",
                               "discovery_server_ip": "192.168.1.2"},
         "/v1/kls/output/modify",
         '{"uuid":"485441684654546484654","name":"new-name","vlan_id":200,"g_name":"new-group","dis_s_ip":"192.168.1.2"}')
_ka_get("kls_get_output", {"uuid": "485441684654546484654"}, "/v1/kls/output/query?uuid=485441684654546484654")
_ka_get("kls_get_outputs", {}, "/v1/kls/output/query/all")
_ka_post("kls_add_panel", {"name": "kls-panel-1", "uuid": "panel-uuid-1", "serial": "KLSSERIAL001"},
         "/v1/kls/panel/add", '{"name":"kls-panel-1","uuid":"panel-uuid-1","serial_number":"KLSSERIAL001"}')
_ka_post("kls_delete_panel", {"uuid": "panel-uuid-1", "serial": "KLSSERIAL001"}, "/v1/kls/panel/delete",
         '{"uuid":"panel-uuid-1","serial_number":"KLSSERIAL001"}')
_ka_post("kls_enable_switcher", {"serial": "KLSSERIAL001", "discoveries": [], "inputs": [], "outputs": [],
                                 "panels": [{"uuid": "panel-uuid-1", "name": "Default", "Icon": ""}],
                                 "panel_inputs": [], "panel_outputs": [],
                                 "panel_matrix": [{"panel_uuid": "panel-uuid-1", "output_uuid": "output-uuid-1",
                                                   "input_name": "input1", "rl_uuid": "", "rl_loop": False}]},
         "/v1/kls/panel/switcher/enable",
         '{"serial_number":"KLSSERIAL001","discoverys":[],"inputs":[],"outputs":[],'
         '"panels":[{"uuid":"panel-uuid-1","name":"Default","Icon":""}],"panel_inputs":[],"panel_outputs":[],'
         '"panel_matrixs":[{"panel_uuid":"panel-uuid-1","output_uuid":"output-uuid-1","input_name":"input1",'
         '"rl_uuid":"","rl_loop":false}]}')
_ka_post("kls_delete_device", {"serial": "KLSSERIAL001"}, "/v1/kls/panel/device/delete",
         '{"serial_number":"KLSSERIAL001"}')
_ka_post("kls_add_panel_inputs", {"panel": "panel-uuid-1", "inputs": [_KA_SRC]}, "/v1/kls/panel/input/add",
         '{"panel_uuid":"panel-uuid-1","inputs":[' + _ka_j(_KA_SRC) + "]}")
_ka_post("kls_remove_panel_input", {"panel": "panel-uuid-1", "input": "input1"}, "/v1/kls/panel/input/remove",
         '{"panel_uuid":"panel-uuid-1","input_names":["input1"]}')
_ka_post("kls_remove_panel_inputs", {"panel": "panel-uuid-1", "names": ["input1", "input2"]},
         "/v1/kls/panel/input/remove", '{"panel_uuid":"panel-uuid-1","input_names":["input1","input2"]}')
_ka_post("kls_add_panel_output", {"panel": "panel-uuid-1", "output": "output-uuid-1"}, "/v1/kls/panel/output/add",
         '{"panel_uuid":"panel-uuid-1","output_uuids":["output-uuid-1"]}')
_ka_post("kls_add_panel_outputs", {"panel": "panel-uuid-1", "uuids": ["output-uuid-1", "output-uuid-2"]},
         "/v1/kls/panel/output/add", '{"panel_uuid":"panel-uuid-1","output_uuids":["output-uuid-1","output-uuid-2"]}')
_ka_post("kls_remove_panel_output", {"panel": "panel-uuid-1", "output": "output-uuid-1"},
         "/v1/kls/panel/output/remove", '{"panel_uuid":"panel-uuid-1","output_uuids":["output-uuid-1"]}')
_ka_post("kls_remove_panel_outputs", {"panel": "panel-uuid-1", "uuids": ["output-uuid-1"]},
         "/v1/kls/panel/output/remove", '{"panel_uuid":"panel-uuid-1","output_uuids":["output-uuid-1"]}')
_ka_get("kls_get_panel", {"panel": "panel-uuid-1"}, "/v1/kls/panel/detail?panel_uuid=panel-uuid-1")
_ka_post("kls_route", {"panel": "panel-uuid-1", "output": "output-uuid-1", "input": "input1"},
         "/v1/kls/panel/cross/output/input",
         '{"panel_uuid":"panel-uuid-1","output_uuid":"output-uuid-1","input_name":"input1"}')
_ka_post("kls_select_input_all", {"panel": "panel-uuid-1", "input": "input1"}, "/v1/kls/panel/cross/input/select",
         '{"panel_uuid":"panel-uuid-1","input_name":"input1","select":true}')

# ── 6 KLS binding, 7 Icons, 8 Front panel, 9 VLAN profiles ──────────────
_ka_post("bind_kls", {"ip": "192.168.1.100", "ifname": "1", "location": 1}, "/v1/bindkls/tobind",
         '{"ip":"192.168.1.100","ifname":"1","location":1}')
_ka_get("unbind_kls", {"location": 1}, "/v1/bindkls/unbind?id=1")
_ka_get("get_kls_vlans", {}, "/v1/bindkls/vlan", http_reply=_ka_data(["eth1.1"]),
        expect_result={"ok": {"kind": "value", "value": ["eth1.1"]}})
_ka_get("get_kls_binding", {}, "/v1/bindkls/bindinfo")
_ka_get("get_kls_binding_status", {}, "/v1/bindkls/status", http_reply=_ka_data({"status": 1}),
        expect_result={"ok": {"kind": "value", "value": 1}})
_KA_ICON, _KA_ICON_CT = _ka_form([("index", b"1", None, None),
                                  ("icon_file", b"\x89PNG", "custom_icon.png", "image/png"),
                                  ("icon_name", b"custom_icon.png", None, None),
                                  ("mime_type", b"image/png", None, None),
                                  ("width", b"64", None, None), ("height", b"64", None, None)])
V.append({"spec": _KA, "command": "upload_icon",
          "input": {"index": 1, "file": "iVBORw==", "name": "custom_icon.png", "width": 64, "height": 64},
          "expect_request": {"method": "POST", "target": "/v1/icon/upload", "body_hex": _KA_ICON.hex(),
                             "content_type": _KA_ICON_CT}})
_ka_post("modify_icons", {"ids": [1, 2, 3]}, "/v1/icon/modify", '{"ids":[1,2,3]}')
_ka_get("get_icons", {}, "/v1/icon/all")
_ka_post("notify_icon_update", {"name": "custom_icon.png"}, "/v1/icon/notify/update",
         '{"icon_name":"custom_icon.png"}')
_ka_post("preview_input", {"input": 1}, "/v1/front/input/preview", '{"input_id":1,"enable":true}')
_ka_post("take", {"panel": 1, "input": 1, "output": 2}, "/v1/front/input/output/take",
         '{"panel_id":1,"input_id":1,"output_ids":[2]}')
_ka_post("take_all", {"panel": 1, "input": 1, "outputs": [1, 2, 3]}, "/v1/front/input/output/take",
         '{"panel_id":1,"input_id":1,"output_ids":[1,2,3]}')
_ka_get("get_front_panels", {}, "/v1/front/panel/query")
_ka_get("get_front_panel", {"panel": 1}, "/v1/front/panel/detail?panel_id=1")
V.append({"spec": _KA, "command": "stop_ndi", "input": {},
          "expect_request": {"method": "POST", "target": "/v1/front/ndi/stop"},
          "http_reply": {"status": 200, "body": '{"code":200,"message":"Success","data":null}'},
          "expect_result": _KA_ACK})
_ka_get("get_vlan_profiles", {}, "/v1/vlan/cfg/all")
_ka_post("modify_vlan_profile", {"profile": 1, "vlans": "1"}, "/v1/vlan/cfg/modify", '{"id":1,"vlan_ids":"1"}')

# ── 10 Topology, 11 Diagnostics ─────────────────────────────────────────
_ka_get("get_topology", {}, "/v1/topology/query")
_ka_get("get_topology_status", {}, "/v1/topology/status/query",
        http_reply={"status": 200, "body": '{"data":{"nodes":[],"edges":[]}}'},
        expect_result={"ok": {"kind": "value", "value": {"nodes": [], "edges": []}}})
_ka_post("get_topology_device_by_port", {"port": 13}, "/v1/topology/dev/info/query",
         '{"data":{"port_id":13,"dev_mac":""}}')
_ka_post("get_topology_device_by_mac", {"mac": "18:C3:F4:20:64:E1"}, "/v1/topology/dev/info/query",
         '{"data":{"port_id":0,"dev_mac":"18:C3:F4:20:64:E1"}}')
_KA_P2P = {"from_dev": "AVX-001", "to_dev": "DEV-002", "from_mac": "00:11:22:33:44:55", "to_mac": "66:77:88:99:AA:BB",
           "from_ip": "192.168.1.10", "to_ip": "192.168.1.20"}
_ka_post("kls_start_diagnostic", dict(_KA_P2P, task=1001), "/v1/diagnose/task/rfppadd",
         '{"task_id":1001,"type":0,"from_dev":"AVX-001","to_dev":"DEV-002","from_mac":"00:11:22:33:44:55",'
         '"to_mac":"66:77:88:99:AA:BB","from_ip":"192.168.1.10","to_ip":"192.168.1.20","from_type":0,"to_type":1}')
_ka_post("kls_start_diagnostic_full", dict(_KA_P2P, task=1001, type="performance", role="1222333"),
         "/v1/diagnose/task/rfppadd",
         '{"task_id":1001,"type":2,"role":"1222333","packet_len":1400,"speed":900,"duration":120,"from_dev":"AVX-001",'
         '"to_dev":"DEV-002","from_mac":"00:11:22:33:44:55","to_mac":"66:77:88:99:AA:BB","from_ip":"192.168.1.10",'
         '"to_ip":"192.168.1.20","from_type":0,"to_type":1}')
_ka_get("get_diagnostic_devices", {}, "/v1/diagnose/task/alldev/list")
_ka_post("delete_diagnostic_task", {"task": 2001}, "/v1/diagnose/task/del", '{"ids":[2001]}')
_ka_post("delete_diagnostic_tasks", {"ids": [2001, 2002, 2003]}, "/v1/diagnose/task/del",
         '{"ids":[2001,2002,2003]}')
_ka_post("get_diagnostic_result", {"task": 2001, "task_type": "global"}, "/v1/diagnose/task/device",
         '{"task_id":2001,"task_type":0}')
_ka_post("start_diagnostic", _KA_P2P, "/v1/diagnose/task/p2pdev/add",
         '{"from_dev":"AVX-001","from_ip":"192.168.1.10","from_mac":"00:11:22:33:44:55","to_dev":"DEV-002",'
         '"to_ip":"192.168.1.20","to_mac":"66:77:88:99:AA:BB","type":0}',
         http_reply=_ka_data({"task_id": 3001}), expect_result={"ok": {"kind": "value", "value": 3001}})
_ka_post("start_diagnostic_full", dict(_KA_P2P, type="performance", speed=1000, duration=180, packet_len=1500),
         "/v1/diagnose/task/p2pdev/add",
         '{"from_dev":"AVX-001","from_ip":"192.168.1.10","from_mac":"00:11:22:33:44:55","to_dev":"DEV-002",'
         '"to_ip":"192.168.1.20","to_mac":"66:77:88:99:AA:BB","type":2,"role":"sendrecv","speed":1000,'
         '"duration":180,"packet_len":1500}')
_ka_get("get_diagnostic_tasks", {}, "/v1/diagnose/task/query/type")

# ── 12 Switch ───────────────────────────────────────────────────────────
_ka_get("get_switch_info", {}, "/v1/switch/sys/info")
_ka_post("set_device_alias", {"mac": "AA:BB:CC:DD:EE:FF", "alias": "Camera-01"}, "/v1/switch/dev/alias/set",
         '{"mac":"AA:BB:CC:DD:EE:FF","alias":"Camera-01"}',
         http_reply={"status": 200, "body": '{"code":200,"message":"ok","data":""}'}, expect_result=_KA_ACK)
_ka_get("get_ports", {}, "/v1/switch/port/dump")
_ka_post("port_control", {"port": 1, "command": "restart"}, "/v1/switch/port/control",
         '{"switch_id":0,"port_id":1,"command":"restart"}')
_ka_post("set_port", {"settings": {"port": 1, "alias": "port1", "poe": {"enable": True}}}, "/v1/switch/port/set",
         '{"msg":{"port":1,"alias":"port1","poe":{"enable":true}}}')
_ka_post("set_ports", {"settings": {"eee_enable": True, "poe_enable": True, "port_enable": True,
                                    "speed_duplex": "auto", "ports": [1, 3]}},
         "/v1/switch/port/set/batch",
         '{"msg":{"eee_enable":true,"poe_enable":true,"port_enable":true,"speed_duplex":"auto","ports":[1,3]}}')
_ka_post("set_port_enabled", {"port": 3, "enabled": False}, "/v1/switch/port/set/batch",
         '{"msg":{"ports":[3],"port_enable":false}}')
_ka_post("set_port_poe", {"port": 5}, "/v1/switch/port/set/batch", '{"msg":{"ports":[5],"poe_enable":true}}')
_ka_post("set_port_eee", {"port": 5, "enabled": False}, "/v1/switch/port/set/batch",
         '{"msg":{"ports":[5],"eee_enable":false}}')
_ka_post("set_port_speed", {"port": 33, "speed_duplex": "10G"}, "/v1/switch/port/set/batch",
         '{"msg":{"ports":[33],"speed_duplex":"10G"}}')
_ka_post("set_port_alias", {"port": 1, "alias": "port-1"}, "/v1/switch/port/alias/set", '{"port":1,"alias":"port-1"}')
_ka_post("get_port_traffic", {"port": 1}, "/v1/switch/port/traffic", '{"port":1}')
_ka_get("get_traffic_top", {}, "/v1/switch/port/traffic/query/top")
_ka_post("watch_ports", {"ports": [1, 11, 13, 23, 33, 4]}, "/v1/switch/port/display/set",
         '{"type":"select","ports":[1,11,13,23,33,4]}')
_ka_post("unwatch_ports", {"ports": [4]}, "/v1/switch/port/display/del", '{"type":"select","ports":[4]}')
_ka_get("get_watched_ports", {}, "/v1/switch/port/display/query",
        http_reply={"status": 200, "body": '{"cascade_ports":[],"select_ports":[]}'},
        expect_result={"ok": {"kind": "value", "value": {"cascade_ports": [], "select_ports": []}}})
_ka_post("remove_device", {"port": 1, "duration": 10}, "/v1/switch/device/remove",
         '{"switch_sn":"","port":1,"duration":10}')
_ka_post("add_aggregation", {"group": 1, "alias": "Uplink", "type": "lacp_active", "members": [1, 2]},
         "/v1/switch/aggr/add", '{"msg":[{"aggre_id":1,"alias":"Uplink","type":"lacp_active","members":[1,2]}]}',
         http_reply={"status": 200, "body": '{"code":200,"message":"ok"}'}, expect_result=_KA_ACK)
_ka_post("delete_aggregation", {"group": 1}, "/v1/switch/aggr/del", '{"aggre_id":[1]}')
_ka_post("delete_aggregations", {"ids": [1, 2]}, "/v1/switch/aggr/del", '{"aggre_id":[1,2]}')
_ka_get("get_aggregations", {}, "/v1/switch/aggr/dump")
_ka_post("get_aggregation", {"ids": [1]}, "/v1/switch/aggr/get", '{"aggre_id":[1]}')
_ka_post("add_aggregation_ports", {"group": 1, "members": [3, 4]}, "/v1/switch/aggr/port/add",
         '{"aggre_id":1,"members":[3,4]}')
_ka_post("set_aggregation", {"group": 1, "alias": "Uplink A", "type": "lacp_passive", "members": [1, 2, 3]},
         "/v1/switch/aggr/set",
         '{"msg":[{"aggre_id":1,"alias":"Uplink A","type":"lacp_passive","members":[1,2,3]}]}')
_ka_get("get_vlans", {}, "/v1/switch/vlan/dump")
_ka_post("add_vlan", {"vlan": 10, "alias": "vlan10", "color": "#38bb70"}, "/v1/switch/vlan/add",
         '{"data":[{"id":10,"alias":"vlan10","color":"#38bb70"}]}')
_KA_VL = {"id": 10, "configs": [1], "color": "#38bb70", "alias": "vlan10", "ports": []}
_ka_post("add_vlans", {"vlans": [_KA_VL]}, "/v1/switch/vlan/add", '{"data":[' + _ka_j(_KA_VL) + "]}")
_ka_post("set_vlan", {"vlan": _KA_VL}, "/v1/switch/vlan/set", '{"data":' + _ka_j(_KA_VL) + "}")
_ka_post("get_vlan", {"vlan": 100}, "/v1/switch/vlan/query", '{"vlan_id":100}')
_ka_post("delete_vlan", {"vlan": 10}, "/v1/switch/vlan/del", '{"vlans":[10]}')
_ka_post("delete_vlans", {"vlans": [10, 20]}, "/v1/switch/vlan/del", '{"vlans":[10,20]}')
_ka_post("set_vlan_ports", {"vlan": 10, "ports": [36, 35]}, "/v1/switch/vlan/port/set",
         '{"msg":[{"vlan_id":10,"ports":[36,35]}]}')
_ka_post("set_vlans_ports", {"settings": [{"vlan_id": 10, "ports": [36, 35]}]}, "/v1/switch/vlan/port/set",
         '{"msg":[{"vlan_id":10,"ports":[36,35]}]}')
_ka_post("get_dhcp_clients", {}, "/v1/switch/dhcps/clients/dump", '{"vlan_id":[]}')
_ka_post("get_dhcp_clients_of", {"vlans": [1, 3]}, "/v1/switch/dhcps/clients/dump", '{"vlan_id":[1,3]}')
_ka_get("get_dhcp", {"vlan": 10}, "/v1/switch/dhcps/get?vlan_id=10")
_ka_post("set_dhcp", {"vlan": 1, "interface_enabled": True, "interface_ip": "192.168.1.1",
                      "interface_netmask": "255.255.255.0", "enabled": True, "pool_start": "192.168.1.100",
                      "pool_end": "192.168.1.200", "dns": "8.8.8.8", "lease_days": 1, "lease_hours": 0,
                      "lease_minutes": 0},
         "/v1/switch/dhcps/set",
         '{"vlan_if":{"vlan_id":1,"enable":true,"ip":"192.168.1.1","netmask":"255.255.255.0"},"dhcp":{"vlan_id":1,'
         '"enable":true,"ip_pool":{"start":"192.168.1.100","end":"192.168.1.200"},"gateway":{"type":"default","ip":""},'
         '"dns":"8.8.8.8","lease_time":{"days":1,"hours":0,"minutes":0}}}')
_ka_post("enable_dhcp", {"vlan": 10}, "/v1/switch/dhcps/enable/set", '{"vlan_id":10,"enable":true}')
_ka_post("get_static_bindings", {}, "/v1/switch/dhcps/static/get", "{}")
_ka_post("get_static_bindings_of", {"vlans": [10, 20]}, "/v1/switch/dhcps/static/get", '{"vlan_id":[10,20]}')
_ka_post("set_static_binding", {"vlan": 10, "ip": "192.168.10.50", "mac": "00:11:22:33:44:55", "port": 5},
         "/v1/switch/dhcps/static/set",
         '{"data":[{"vlan_id":10,"ip":"192.168.10.50","mac":"00:11:22:33:44:55","bind_port":5,"op":1,"bind_type":1}]}')
_KA_SB = [{"vlan_id": 10, "ip": "192.168.10.50", "mac": "00:11:22:33:44:55", "bind_port": 5, "op": 1,
           "bind_type": 3}]
_ka_post("set_static_bindings", {"bindings": _KA_SB}, "/v1/switch/dhcps/static/set",
         '{"data":' + _ka_j(_KA_SB) + "}")
_ka_get("get_power", {}, "/v1/switch/power/status/query")
_ka_post("add_route", {"dest_ip": "192.168.2.0", "dest_mask": 24, "next_hop": "192.168.1.1", "priority": 100},
         "/v1/switch/iproutes/add", '{"dest_ip":"192.168.2.0","dest_mask":24,"next_hop":"192.168.1.1","priority":100}')
_ka_post("delete_route", {"dest_ip": "192.168.2.0", "dest_mask": 24, "next_hop": "192.168.1.1", "priority": 100},
         "/v1/switch/iproutes/delete",
         '{"dest_ip":"192.168.2.0","dest_mask":24,"next_hop":"192.168.1.1","priority":100}')
_KA_RT = [{"dest_ip": "192.168.2.0", "dest_mask": 24, "next_hop": "192.168.1.1", "priority": 100}]
_ka_get("get_routes", {}, "/v1/switch/iproutes/get",
        http_reply={"status": 200, "body": _ka_j({"code": 200, "message": "ok", "routes": _KA_RT})},
        expect_result={"ok": {"kind": "value", "value": _KA_RT}})
V.append({"spec": _KA, "command": "save_network", "input": {},
          "expect_request": {"method": "POST", "target": "/v1/network/save"}})
_ka_get("get_network_status", {}, "/v1/network/query", http_reply=_ka_data({"status": False}),
        expect_result={"ok": {"kind": "value", "value": False}})

# ── 13 Alarms ───────────────────────────────────────────────────────────
_ka_get("get_device_alarms", {"mac": "18:C3:F4:20:64:E1"}, "/v1/alarm/dev/events/get?mac=18%3AC3%3AF4%3A20%3A64%3AE1")
_ka_get("get_alarms", {}, "/v1/alarm/events/levels/get?alarm_levels=1&alarm_levels=2&alarm_levels=3")
_ka_post("delete_alarm", {"alarm": 1}, "/v1/alarm/events/del", '{"ids":[1]}')
_ka_post("delete_alarms", {"ids": [1, 2, 3]}, "/v1/alarm/events/del", '{"ids":[1,2,3]}')
_ka_post("get_alarm_page", {}, "/v1/alarm/events/get", '{"page_index":1,"page_size":20}')
_KA_AF = {"alarm_level": [1, 2], "start_time": "2024-01-01 00:00:00", "end_time": "2024-01-31 23:59:59",
          "status": 1, "page_index": 1, "page_size": 20, "port_id": [1, 2], "keyword": "CPU"}
_ka_post("find_alarms", {"filter": _KA_AF}, "/v1/alarm/events/get", _ka_j(_KA_AF))
_ka_get("get_alarms_by_level", {"level": "error"}, "/v1/alarm/events/levels/get?alarm_levels=1")
_ka_post("set_alarm_status", {"alarm": 4}, "/v1/alarm/events/status/set", '{"ids":[4],"status":2}')
_ka_post("set_alarms_status", {"ids": [1, 2, 3], "status": "unread"}, "/v1/alarm/events/status/set",
         '{"ids":[1,2,3],"status":1}')
_ka_get("export_alarms", {}, "/v1/alarm/list/export", http_reply=_ka_data({"file_path": "alarm_export_1.xlsx"}),
        expect_result={"ok": {"kind": "value", "value": "alarm_export_1.xlsx"}})
_ka_get("export_alarms_by_level", {"level": "warning"}, "/v1/alarm/list/export?alarm_level=2")
_ka_get("get_alarm_config", {}, "/v1/alarm/config/get")
_ka_post("set_alarm_config", {"cpu": True, "memory": True, "temperature": True, "interval": 5, "disconnect": True,
                              "disconnect_duration": 30},
         "/v1/alarm/config/set",
         '{"cpu_enabled":true,"mem_enabled":true,"temp_enabled":true,"alarm_interval":5,'
         '"disconnect_alarm_enabled":true,"disconnect_duration":30}')
_ka_get("get_port_alarms", {}, "/v1/alarm/ports/events/get")
_ka_post("get_port_alarm", {"port": 1}, "/v1/alarm/debug/port", '{"port_id":1}')

# ── 14 Production test, 15 Page interface, 16 Reverse proxy ─────────────
_ka_get("test_factory_reset", {}, "/v1/test/factory_reset")
_ka_get("get_fans", {}, "/v1/test/fan_list")
_ka_get("get_resources", {}, "/v1/test/get_resources")
_ka_get("get_test_port_status", {}, "/v1/test/port_status")
_ka_post("set_serial_number", {"serial": "SN20240001"}, "/v1/test/set_serial_number",
         '{"serial_number":"SN20240001"}')
_ka_get("start_fan_test", {}, "/v1/test/start_fan_test")
_ka_get("stop_fan_test", {}, "/v1/test/stop_fan_test")
_ka_get("get_usb_status", {}, "/v1/test/usb_port_status")
_ka_get("get_system_info", {}, "/api/systemctrl/report/get_system_info")
_ka_get("get_network", {}, "/api/networkmanager/report/get_network")
_ka_get("get_device_page", {"mac": "18:C3:F4:21:D7:2E"}, "/v1/local/klnk/forward?id=18%3AC3%3AF4%3A21%3AD7%3A2E")


# ── Telemetry ───────────────────────────────────────────────────────────
def _ka_tel(name, path, data, expect_state, **extra):
    body = extra.pop("body", None) or _ka_j({"code": 200, "message": "ok", "data": data})
    inbound = {"path": path, "body": body}
    if "request" in extra:
        inbound["request"] = extra.pop("request")
    telemetry(_KA, name, inbound_http=inbound, expect_state=expect_state, **extra)


def _ka_then(*targets):
    return [{"method": "GET", "target": t} for t in targets]


_ka_tel("users", "/v1/user/userlist", [
    {"id": 1, "username": "admin", "alias": "admin", "role": "admin", "online": True, "first_login": True,
     "create_time": "2026-02-04 18:51:27", "update_time": "2026-02-04 18:51:27"}],
    {"users": {"1": {"username": "admin", "alias": "admin", "role": "admin", "online": True, "first_login": True,
                     "created": "2026-02-04 18:51:27", "updated": "2026-02-04 18:51:27"}}},
    state_before={"users": {"7": {"username": "gone"}}})
_ka_tel("time", "/v1/systime/get", {"Timezone": "Asia/Shanghai", "ntp": "time.windows.com pool.ntp.org",
                                    "offset": 8, "time": "2026-03-18 15:54:48", "timetype": "pc"},
        {"time": {"timezone": "Asia/Shanghai", "ntp": "time.windows.com pool.ntp.org", "offset": 8,
                  "now": "2026-03-18 15:54:48", "source": "pc"}})
_KA_PTP_PORT = {"id": 1, "ptp_id": 1, "mode": "e2e", "master": True,
                "vlanInfo": {"type": "access", "pvid": 1, "vid": "1", "alias": "vlan1", "color": "#1D2024",
                             "config_file": ""}}
_ka_tel("ptp", "/v1/ptp/query", {
    "ports": [_KA_PTP_PORT],
    "ptps": [{"id": 1, "enable": True, "domain": 0, "type": "BC", "identity": "18c3f4.fffe.2ffbee", "pport_id": "1",
              "status": 1, "two_step": True, "priority1": 128, "priority2": 128, "leap_s": False,
              "de_mainclock": "12ns", "average_delay": "300ns", "upstream_equipment": "GM", "upstream_port": "3",
              "ports": [_KA_PTP_PORT]}]},
    {"ptp": {"clocks": {"1": {"enabled": True, "domain": 0, "type": "BC", "identity": "18c3f4.fffe.2ffbee",
                              "parent_port": "1", "status": "running", "two_step": True, "priority1": 128,
                              "priority2": 128, "leap_second": False, "offset_from_master": "12ns",
                              "mean_path_delay": "300ns", "upstream_device": "GM", "upstream_port": "3"}},
             "ports": {"1": {"clock": 1, "mode": "e2e", "master": True}}}},
    state_before={"ptp": {"clocks": {"3": {"type": "TC"}}, "ports": {"9": {"clock": 3}}}})
_ka_tel("ptp-clock", "/v1/ptp/detail/query?id=2", {"id": 2, "enable": False, "domain": 1, "type": "TC",
                                                   "two_step": False, "priority1": 100, "priority2": 120,
                                                   "leap_s": True, "ports": []},
        {"ptp": {"clocks": {"2": {"enabled": False, "domain": 1, "type": "TC", "two_step": False, "priority1": 100,
                                  "priority2": 120, "leap_second": True}}}})
_ka_tel("presets", "/v1/preset/getlist", [{"prname": "default_config", "version": "1.0.0", "apply": True},
                                          {"prname": "show.v2", "version": "1.2.0", "apply": False}],
        {"presets": {"default_config": {"version": "1.0.0", "apply": True},
                     "show.v2": {"version": "1.2.0", "apply": False}}},
        state_before={"presets": {"old": {"version": "0.1"}}})
_ka_tel("virtual-nics", "/v1/preset/vlan/interface/list/get",
        [{"vlan_id": 1, "ip": "192.168.1.1", "netmask": "255.255.255.0", "alias": "vlan1", "color": "#1D2024"}],
        {"virtual_nics": {"1": {"ip": "192.168.1.1", "netmask": "255.255.255.0", "alias": "vlan1",
                                "color": "#1D2024"}}})
_ka_tel("virtual-nic", "/v1/preset/vlan/interface/get?vlan_id=10", {
    "interface": {"enable": True, "ip": "192.168.1.1", "netmask": "255.255.255.0", "type": "static",
                  "referenced": True, "is_default_route": True},
    "recommend": {"dhcp": {"is_recommend": False, "recommend_type": "dhcp"},
                  "manual": {"is_recommend": True, "recommend_type": "manual", "vlanif_ip": "192.168.1.254",
                             "recommend_range": "192.168.1.1-192.168.1.200"}}},
    {"virtual_nics": {"10": {"enabled": True, "ip": "192.168.1.1", "netmask": "255.255.255.0", "type": "static",
                             "referenced": True, "default_route": True, "recommend_dhcp": False,
                             "recommend_manual": True, "recommended_ip": "192.168.1.254",
                             "recommended_range": "192.168.1.1-192.168.1.200"}}})
_ka_tel("interface-ips", "/v1/switch/interface/ip/get",
        {"interface_ip_list": [{"vlan_id": 0, "ip": "192.168.0.168"}, {"vlan_id": 1, "ip": "192.168.1.1"}]},
        {"interface_ips": {"0": "192.168.0.168", "1": "192.168.1.1"}}, state_before={"interface_ips": {"5": "10.0.0.1"}})
_ka_tel("mode", "/v1/preset/mode/get", {"mode": "layer3"}, {"switch": {"mode": "layer3"}})
# 4.1.8, the document's example.
_ka_tel("discovery-methods", "/v1/input/discovery/all", {
    "manuals": [{"dis_id": 3, "dis_g_name": "public", "dis_d_ip": "192.168.40.23"}],
    "auto_servers": [{"dis_id": 1, "dis_method": "auto", "dis_g_name": "public", "dis_d_ip": "", "dis_s_ip": ""},
                     {"dis_id": 4, "dis_method": "server", "dis_g_name": "public", "dis_d_ip": "",
                      "dis_s_ip": "192.43.56.2"}]},
    {"discovery": {"3": {"method": "manual", "group": "public", "device_ip": "192.168.40.23"},
                   "1": {"method": "auto", "group": "public", "device_ip": "", "server_ip": ""},
                   "4": {"method": "server", "group": "public", "device_ip": "", "server_ip": "192.43.56.2"}}},
    state_before={"discovery": {"8": {"method": "auto", "group": "gone"}}},
    expect_then_send=_ka_then("/v1/input/manual/discovery", "/v1/input/auto/discovery?dis_g_name=public",
                              "/v1/input/server/discovery?dis_g_name=public&dis_s_ip=192.43.56.2"))


def _ka_found(n, method, dip, sip):
    return {"name": f"input{n}", "alias": f"Input {n}", "c_name": f"Channel {n}", "d_name": f"Device {n}",
            "d_ip": f"192.168.1.10{n}", "d_port": 554, "url": f"192.168.1.10{n}:554", "enable": True,
            "dis_method": method, "dis_g_name": "group1", "dis_d_ip": dip, "dis_s_ip": sip, "vlan_id": 100,
            "switcher_id": "0000000000000"}


def _ka_founds(n, **more):
    return {"name": f"input{n}", "alias": f"Input {n}", "channel_name": f"Channel {n}", "device_name": f"Device {n}",
            "device_ip": f"192.168.1.10{n}", "device_port": 554, "enabled": True, "vlan": 100,
            "switch_serial": "0000000000000", **more}


_ka_tel("discovered-manual", "/v1/input/manual/discovery", {"inputs": [_ka_found(1, "manual", "192.168.1.101", "")]},
        {"discovered": {"manual": {"192.168.1.101:554": _ka_founds(1, group="group1")}}},
        state_before={"discovered": {"manual": {"10.0.0.9:5961": {"name": "gone"}}}})
_ka_tel("discovered-auto", "/v1/input/auto/discovery?dis_g_name=group1",
        {"inputs": [_ka_found(2, "auto", "192.168.1.102", "")]},
        {"discovered": {"auto": {"group1": {"192.168.1.102:554": _ka_founds(2)}}}})
_ka_tel("discovered-server", "/v1/input/server/discovery?dis_g_name=group1&dis_s_ip=192.168.1.1",
        {"inputs": [_ka_found(3, "server", "", "192.168.1.1")]},
        {"discovered": {"server": {"192.168.1.1": {"group1": {"192.168.1.103:554": _ka_founds(3)}}}}})
_KA_OUT = {"id": 1, "name": "output1", "alias": "Output 1", "c_name": "Channel 1", "g_name": "group1", "smooth": 0,
           "router": True, "codec_k": 0, "dis_s_ip": "192.168.1.1", "locked": False, "enable": True, "vlan_id": 100,
           "vlan_alias": "VLAN100", "vlan_color": "#0000FF", "switcher_id": "0000000000000", "connect_num": 0,
           "create_time": "2026-04-21 00:00:00.000000", "update_time": "2026-04-21 00:00:00.000000"}
_KA_OUTS = {"name": "output1", "alias": "Output 1", "channel_name": "Channel 1", "group": "group1", "smooth": 0,
            "routed": True, "codec_keepalive": 0, "discovery_server_ip": "192.168.1.1", "locked": False,
            "enabled": True, "vlan": 100, "vlan_alias": "VLAN100", "vlan_color": "#0000FF",
            "switch_serial": "0000000000000", "connections": 0, "created": "2026-04-21 00:00:00.000000",
            "updated": "2026-04-21 00:00:00.000000"}
_ka_tel("outputs", "/v1/output/query/all", {"outputs": [_KA_OUT]}, {"outputs": {"1": _KA_OUTS}},
        state_before={"outputs": {"9": {"name": "output9"}}})
_ka_tel("output", "/v1/output/query?id=1", dict(_KA_OUT, locked=True),
        {"outputs": {"1": dict(_KA_OUTS, locked=True)}})
_ka_tel("default-output", "/v1/output/query/default", {"name": "output1"}, {"output_default": "output1"})
_ka_tel("panels", "/v1/panel/query/all", {"panels": [
    {"id": 1, "name": "Default", "input_num": 2, "output_num": 1, "create_time": "2026-04-20 21:29:57.832819",
     "update_time": "2026-04-20 21:29:57.832820"}]},
    {"panels": {"1": {"name": "Default", "input_count": 2, "output_count": 1, "created": "2026-04-20 21:29:57.832819",
                      "updated": "2026-04-20 21:29:57.832820"}}},
    state_before={"panels": {"5": {"name": "gone"}}},
    expect_then_send=_ka_then("/v1/panel/detail?id=1", "/v1/panel/cfg/input/all?id=1", "/v1/panel/cfg/output/all?id=1",
                              "/v1/panel/cfg/enable/query?id=1"))
_ka_tel("panel", "/v1/panel/query?id=2", {"id": 2, "name": "ccc", "input_num": 0, "output_num": 0,
                                          "create_time": "2026-04-21 13:46:41.195032",
                                          "update_time": "2026-04-21 13:46:41.195032"},
        {"panels": {"2": {"name": "ccc", "input_count": 0, "output_count": 0, "created": "2026-04-21 13:46:41.195032",
                          "updated": "2026-04-21 13:46:41.195032"}}})
# 4.7.1, the document's example, with another input among other_inputs.
_KA_PIN = {"id": 1, "name": "AVX24-4 (hhyoutput1)", "alias": "AVX24-4 (hhyoutput1)", "c_name": "AVX24-4 (hhyoutput1)",
           "g_name": "", "d_ip": "172.30.46.14", "d_port": 5961, "url": "172.30.46.14:5961", "enable": True,
           "dis_method": "auto", "dis_g_name": "public", "dis_d_ip": "", "dis_s_ip": "", "vlan_id": 1,
           "switcher_id": "000000000234D2", "resolution": "0x0@0HZ", "vb": "0.00Mbps", "ab": "0.00Mbps",
           "multicast": True, "icon_type": "", "icon_name": ""}
_KA_PINS = {"name": "AVX24-4 (hhyoutput1)", "alias": "AVX24-4 (hhyoutput1)", "channel_name": "AVX24-4 (hhyoutput1)",
            "group": "", "device_ip": "172.30.46.14", "device_port": 5961, "url": "172.30.46.14:5961",
            "enabled": True, "discovery": "auto", "discovery_group": "public", "discovery_device_ip": "",
            "discovery_server_ip": "", "vlan": 1, "switch_serial": "000000000234D2", "resolution": "0x0@0HZ",
            "video_bitrate": "0.00Mbps", "audio_bitrate": "0.00Mbps", "multicast": True, "icon_type": "",
            "icon_name": ""}
_ka_tel("panel-detail", "/v1/panel/detail?id=1", {
    "id": 1, "name": "Default", "expand": False,
    "panel_inputs": [_KA_PIN],
    "panel_outputs": [{"id": 1, "name": "output1", "alias": "output1", "c_name": "output1", "g_name": "public",
                       "router": False, "enable": True, "dis_s_ip": "", "vlan_id": 1, "vlan_alias": "vlan1",
                       "vlan_color": "#1D2024", "switcher_id": "000000000234D2", "connect_num": 0, "lock": False,
                       "resolution": "0x0@0HZ", "vb": "0.00Mbps", "ab": "0.00Mbps", "input_id": 0, "linked": False,
                       "r_id": 0, "r_loop": False, "r_src_id": 0, "r_src_uuid": "", "r_src_progress": 0,
                       "icon_type": "", "icon_name": "", "d_ip": "172.30.46.175"}],
    "other_inputs": [dict(_KA_PIN, id=2, name="N60 (cam)", alias="Cam", multicast=False)],
    "panel_templates": [], "rotationlists": []},
    {"panel_info": {"1": {"expanded": False}},
     "panel_inputs": {"1": {"1": {"position": 0}}},
     "inputs": {"1": _KA_PINS, "2": dict(_KA_PINS, name="N60 (cam)", alias="Cam", multicast=False)},
     "panel_outputs": {"1": {"1": {"position": 0, "input": 0, "linked": False, "locked": False, "connections": 0,
                                   "resolution": "0x0@0HZ", "video_bitrate": "0.00Mbps", "audio_bitrate": "0.00Mbps",
                                   "rotation_list": 0, "rotation_loop": False, "rotation_source": 0,
                                   "rotation_source_uuid": "", "rotation_progress": 0, "icon_type": "",
                                   "icon_name": "", "device_ip": "172.30.46.175"}}}},
    state_before={"panel_inputs": {"1": {"9": {"position": 3}}}, "panel_outputs": {"1": {"7": {"input": 2}}}})
_ka_tel("panel-input-config", "/v1/panel/cfg/input/all?id=1",
        {"cfg_inputs": [{"id": 1, "name": "input1", "alias": "Input 1", "icon_id": 1, "front_content": "input1"}]},
        {"panel_config": {"1": {"inputs": {"1": {"name": "input1", "alias": "Input 1", "icon": 1,
                                                 "front_content": "input1"}}}}})
_ka_tel("panel-output-config", "/v1/panel/cfg/output/all?id=1",
        {"outputs": [{"id": 1, "name": "output1", "alias": "Output 1", "icon_id": 2, "front_content": "output1"}]},
        {"panel_config": {"1": {"outputs": {"1": {"name": "output1", "alias": "Output 1", "icon": 2,
                                                  "front_content": "output1"}}}}},
        state_before={"panel_config": {"1": {"outputs": {"4": {"name": "gone"}}}}})
_ka_tel("panel-output-config-table", "/v1/panel/cfg/output/all?id=2",
        {"cfg_outputs": [{"id": 3, "name": "output3", "alias": "Output 3", "icon_id": 2, "front_content": "o3"}]},
        {"panel_config": {"2": {"outputs": {"3": {"name": "output3", "alias": "Output 3", "icon": 2,
                                                  "front_content": "o3"}}}}})
_ka_tel("physical-panel", "/v1/panel/cfg/enable/query?id=1", {"enable": True},
        {"panel_config": {"1": {"physical_panel": True}}})
_KA_KOS = {"name": "kls-output-1", "alias": "KLS Output 1", "channel_name": "Channel 1", "group": "group1",
           "routed": True, "discovery_server_ip": "192.168.1.1", "vlan": 100, "switch_serial": "0000000000000"}
_ka_tel("kls-outputs", "/v1/kls/output/query/all", {"outputs": [_KA_KO]},
        {"kls_outputs": {"485441684654546484654": _KA_KOS}}, state_before={"kls_outputs": {"old": {"name": "x"}}})
_ka_tel("kls-outputs-added", "/v1/kls/output/add", {"outputs": [dict(_KA_KO, uuid="u2")], "matrixs": []},
        {"kls_outputs": {"u2": _KA_KOS}})
_ka_tel("kls-output", "/v1/kls/output/query?uuid=u3", dict(_KA_KO, uuid="u3"), {"kls_outputs": {"u3": _KA_KOS}})
_ka_tel("kls-panel", "/v1/kls/panel/detail?panel_uuid=panel-uuid-1", {
    "uuid": "panel-uuid-1",
    "panel_inputs": [{"name": "input1", "alias": "Input 1", "c_name": "Channel 1", "g_name": "group1",
                      "d_ip": "192.168.1.100", "d_port": 5961, "url": "192.168.1.100:5961", "enable": True,
                      "dis_method": "auto", "dis_g_name": "public", "dis_d_ip": "192.168.1.100",
                      "dis_s_ip": "192.168.1.1", "vlan_id": 1, "switcher_id": "KLSSERIAL001",
                      "resolution": "1920x1080@60HZ", "vb": "50.00Mbps", "ab": "1.50Mbps", "multicast": True}],
    "panel_outputs": [{"uuid": "output-uuid-1", "name": "output1", "alias": "Output 1", "c_name": "Channel 1",
                       "g_name": "group1", "router": True, "enable": True, "dis_s_ip": "192.168.1.1", "vlan_id": 1,
                       "vlan_alias": "VLAN1", "vlan_color": "#0000FF", "switcher_id": "KLSSERIAL001",
                       "connect_num": 0, "lock": False, "resolution": "1920x1080@60HZ", "vb": "50.00Mbps",
                       "ab": "1.50Mbps", "input_name": "input1", "linked": False, "r_uuid": "", "r_loop": False,
                       "r_src_id": "", "r_src_uuid": "", "r_src_progress": 0, "d_ip": "192.168.1.200"}],
    "other_inputs": [], "panel_templates": [], "rotationlists": []},
    {"kls_panels": {"panel-uuid-1": {
        "inputs": {"input1": {"alias": "Input 1", "channel_name": "Channel 1", "group": "group1",
                              "device_ip": "192.168.1.100", "device_port": 5961, "url": "192.168.1.100:5961",
                              "enabled": True, "discovery": "auto", "vlan": 1, "switch_serial": "KLSSERIAL001",
                              "resolution": "1920x1080@60HZ", "video_bitrate": "50.00Mbps",
                              "audio_bitrate": "1.50Mbps", "multicast": True}},
        "outputs": {"output-uuid-1": {"name": "output1", "alias": "Output 1", "channel_name": "Channel 1",
                                      "group": "group1", "routed": True, "enabled": True, "vlan": 1,
                                      "connections": 0, "locked": False, "resolution": "1920x1080@60HZ",
                                      "video_bitrate": "50.00Mbps", "audio_bitrate": "1.50Mbps", "input": "input1",
                                      "linked": False, "playlist": "", "playlist_loop": False,
                                      "playlist_progress": 0, "device_ip": "192.168.1.200"}}}}},
    state_before={"kls_panels": {"panel-uuid-1": {"inputs": {"input9": {"alias": "gone"}}}}})
_ka_tel("kls-vlans", "/v1/bindkls/vlan", ["eth1.1"], {"kls": {"vlans": '["eth1.1"]'}})
_ka_tel("kls-binding", "/v1/bindkls/bindinfo",
        {"status": 1, "bind_list": [{"id": 1, "ip": "192.168.1.100", "ifname": "1", "bind_status": 1}]},
        {"kls": {"binding_status": "bound", "bindings": {"1": {"ip": "192.168.1.100", "ifname": "1",
                                                               "status": "bound"}}}},
        state_before={"kls": {"bindings": {"2": {"ip": "10.0.0.2", "status": "failed"}}}})
_ka_tel("kls-binding-status", "/v1/bindkls/status", {"status": 2}, {"kls": {"binding_status": "binding"}})
_ka_tel("icons", "/v1/icon/all", {"official_icons": [{"id": 1, "name": "icon1.png", "type": "official"}],
                                  "custom_icons": [{"id": 3, "name": "icon3.png", "type": "custom"}]},
        {"icons": {"1": {"name": "icon1.png", "type": "official"}, "3": {"name": "icon3.png", "type": "custom"}}},
        state_before={"icons": {"9": {"name": "gone.png", "type": "custom"}}})
_ka_tel("front-panels", "/v1/front/panel/query", {"panels": [{"id": 1, "name": "panel1", "alias": "panel1"},
                                                             {"id": 2, "name": "panel2", "alias": "panel2"}]},
        {"front_panels": {"1": {"name": "panel1", "alias": "panel1"}, "2": {"name": "panel2", "alias": "panel2"}}},
        expect_then_send=_ka_then("/v1/front/panel/detail?panel_id=1", "/v1/front/panel/detail?panel_id=2"))
_ka_tel("front-panel", "/v1/front/panel/detail?panel_id=1", {
    "id": 1, "name": "panel1", "alias": "panel1",
    "panel_inputs": [{"id": 1, "name": "input1", "alias": "input1", "icon_name": "input1", "front_content": "input1"}],
    "panel_outputs": [{"id": 1, "name": "output1", "alias": "output1", "lock": False, "input_id": 1, "linked": False,
                       "icon_name": "output1", "front_content": "output1"}]},
    {"front_panels": {"1": {"inputs": {"1": {"name": "input1", "alias": "input1", "icon_name": "input1",
                                             "front_content": "input1"}},
                            "outputs": {"1": {"name": "output1", "alias": "output1", "locked": False, "input": 1,
                                              "linked": False, "icon_name": "output1", "front_content": "output1"}}}}})
_ka_tel("vlan-profiles", "/v1/vlan/cfg/all", {"vlan_cfgs": [{"id": 1, "type": True, "name": "NDI",
                                                             "priority": "Highest", "description": "Default VLAN NDI",
                                                             "vlans": [1, 3]}]},
        {"vlan_profiles": {"1": {"name": "NDI", "official": True, "priority": "Highest",
                                 "description": "Default VLAN NDI", "vlans": "[1,3]"}}})
_ka_tel("topology", "/v1/topology/query", {
    "nodes": [{"id": "18:C3:F4:2F:FB:EE", "name": "AVX24-4-chen666", "type": "kiloview", "label": "AVX24-4",
               "parent_id": "", "shape": "router", "image": "./static/img/AVX24_4.png", "valid": False, "x": 320,
               "y": 560, "connection_port": 0}],
    "wires": [{"id": "18:C3:F4:2F:FB:EE:3", "shape": "edge", "color": ""}]},
    {"topology_nodes": {"18:C3:F4:2F:FB:EE": {"name": "AVX24-4-chen666", "type": "kiloview", "label": "AVX24-4",
                                              "parent": "", "shape": "router", "image": "./static/img/AVX24_4.png",
                                              "valid": False, "x": 320, "y": 560, "port": 0}},
     "topology_wires": {"18:C3:F4:2F:FB:EE:3": {"shape": "edge", "color": ""}}})
# 10.2, the document's example: no code.
_ka_tel("topology-status", "/v1/topology/status/query", None, {
    "node_status": {"18:C3:F4:2F:FB:EE": {"main": True, "status": "online", "alarm_status": "",
                                          "ports": {"3": "online"}},
                    "V:18:C3:F4:2F:FB:EE-3": {"main": False, "status": "online", "alarm_status": ""}},
    "link_status": {"18:C3:F4:2F:FB:EE:3": {"status": "online", "fill": "", "source": "18:C3:F4:2F:FB:EE",
                                            "source_port": "3", "source_is_device": False,
                                            "target": "V:18:C3:F4:2F:FB:EE-3", "target_port": "",
                                            "target_is_device": True}}},
    body=_ka_j({"data": {"nodes": [
        {"id": "V:18:C3:F4:2F:FB:EE-3", "isMain": False, "status": "online", "alarm_status": "", "ports": []},
        {"id": "18:C3:F4:2F:FB:EE", "isMain": True, "status": "online", "alarm_status": "",
         "ports": [{"port_id": "3", "status": "online"}]}],
        "edges": [{"id": "18:C3:F4:2F:FB:EE:3", "fill": "", "status": "online",
                   "source": {"is_dev": False, "cell": "18:C3:F4:2F:FB:EE", "port": "3"},
                   "target": {"is_dev": True, "cell": "V:18:C3:F4:2F:FB:EE-3", "port": ""}}]}}),
    state_before={"link_status": {"old": {"status": "offline"}}})
_ka_tel("topology-status-wires", "/v1/topology/status/query", None,
        {"link_status": {"w1": {"status": "offline", "fill": "red", "source": "a", "source_port": "1",
                                "source_is_device": False, "target": "b", "target_port": "",
                                "target_is_device": True}}},
        body=_ka_j({"code": 200, "message": "ok", "data": {"nodes": [], "wires": [
            {"id": "w1", "fill": "red", "status": "offline", "source": {"is_dev": False, "cell": "a", "port": "1"},
             "target": {"is_dev": True, "cell": "b", "port": ""}}]}}))
_ka_tel("topology-device", "/v1/topology/dev/info/query", {
    "switch_id": 0, "port_id": 13, "dev_name": "18:C3:F4:20:64:E1", "dev_mac": "18:C3:F4:20:64:E1",
    "dev_ip": "192.168.3.225", "is_static_ip": False, "image": "./static/img/N50.png",
    "rx": {"packets": "2", "bytes": "1203", "error": "0", "loss": "0"},
    "tx": {"packets": "1", "bytes": "491", "error": "0", "loss": "0"}, "media": [], "model": "N50",
    "sn": "3202112006A41", "software_version": "2.43.0011.kdds.0297", "hardware_version": "", "company": "Kiloview",
    "alias": "N50-32021120064E1", "sw_serial_number": "18c3f425640e", "diagnose_status": 0, "parent_id": "",
    "cpu_usage": "16.20%", "disk_usage": "48.75%", "mem_usage": "21.44%", "temperature": "", "server_count": 1,
    "dev_proxy_url": "", "switch_link_status": "online"},
    {"topology_devices": {"18:C3:F4:20:64:E1": {
        "name": "18:C3:F4:20:64:E1", "switch": 0, "port": 13, "ip": "192.168.3.225", "static_ip": False,
        "image": "./static/img/N50.png", "rx_bytes": 1203, "tx_bytes": 491, "rx_packets": 2, "tx_packets": 1,
        "rx_errors": 0, "tx_errors": 0, "rx_loss": 0, "tx_loss": 0, "media": "[]", "model": "N50",
        "serial": "3202112006A41", "software_version": "2.43.0011.kdds.0297", "hardware_version": "",
        "company": "Kiloview", "alias": "N50-32021120064E1", "switch_serial": "18c3f425640e", "diagnosing": False,
        "parent": "", "cpu": "16.20%", "disk": "48.75%", "memory": "21.44%", "temperature": "", "server_count": 1,
        "proxy_url": "", "link_status": "online"}}},
    request={"data": {"port_id": 13, "dev_mac": ""}})
_ka_tel("diagnostic-devices", "/v1/diagnose/task/alldev/list",
        [{"name": "AVX-001", "mac": "00:11:22:33:44:55", "serial_number": "SN20240001", "ip": "192.168.1.10"}],
        {"diagnostics": {"devices": {"00:11:22:33:44:55": {"name": "AVX-001", "serial": "SN20240001",
                                                           "ip": "192.168.1.10"}}}})
_ka_tel("diagnostic-tasks", "/v1/diagnose/task/query/type", [
    {"id": 3001, "name": "e2e", "type": 1, "from_dev": "AVX-001", "to_dev": "DEV-002", "seq": 1, "speed": 1000,
     "duration": 180, "pkg_len": 1500, "vlan_id": 100, "status": 1, "switcher_sn": "SN20240001",
     "create_time": "2024-01-01 10:00:00", "update_time": "2024-01-01 10:03:00"}],
    {"diagnostics": {"tasks": {"3001": {"name": "e2e", "type": "end_to_end", "from": "AVX-001", "to": "DEV-002",
                                        "seq": 1, "speed": 1000, "duration": 180, "packet_len": 1500, "vlan": 100,
                                        "status": "completed", "switch_serial": "SN20240001",
                                        "created": "2024-01-01 10:00:00", "updated": "2024-01-01 10:03:00"}}}},
    state_before={"diagnostics": {"tasks": {"2001": {"name": "old"}}}})
# 11.1.4, the document's example.
_ka_tel("diagnostic-result", "/v1/diagnose/task/device", None, {"diagnostics": {"results": {"2001": {
    "message": "done", "device_count": 5, "progress": 100, "time": "2024-01-01 10:03:00", "abnormal": 0,
    "normal_count": 5, "recognized": 5, "unrecognized": 0, "step": 2, "complete": True,
    "devices": {"1": {"from": "AVX-001", "to": "DEV-002", "ping": "connected", "delay": "5ms", "loss": "0.1%",
                      "rx": "950Mbps", "tx": "940Mbps", "ip": "192.168.1.20", "mac": "66:77:88:99:AA:BB",
                      "device_type": "regular", "identified": True, "delay_ok": True, "loss_ok": True,
                      "tx_ok": True, "device_ok": True, "ping_ok": True}}}}}},
    body=_ka_j({"code": 200, "message": "ok", "total": 5, "data": {
        "result_msg": "done", "device_info_list": [{
            "id": 1, "task_id": 2001, "result_type": 0, "from_dev": "AVX-001", "to_dev": "DEV-002", "ping_result": 1,
            "delay": "5ms", "jitter": "1ms", "loss": "0.1%", "rx": "950Mbps", "tx": "940Mbps", "ip": "192.168.1.20",
            "devmac": "66:77:88:99:AA:BB", "switcher_sn": "SN20240001", "create_time": "2024-01-01 10:00:00",
            "update_time": "2024-01-01 10:03:00",
            "check_device_data": {"check_delay": True, "check_loss": True, "check_tx": True, "check_device": True,
                                  "check_ping": True},
            "total_info": "1000/1", "dev_type": "regular", "identify": 1}],
        "device_count": 5, "done": 100, "diagnose_time": "2024-01-01 10:03:00",
        "all_er": {"er_delay": 0, "er_loss": 0, "er_tx": 0, "er_device": 0, "er_ping": 0}, "normal": True,
        "diagnose_info": {"abnormal": 0, "dtime": "180s", "from_dev": "AVX-001", "from_ip": "192.168.1.10", "id": 1,
                          "mac_process": 100, "macping_test": 1, "normal": 5, "ping_process": 100, "ping_test": 1,
                          "recognize": 5, "unrecognized": 0, "step": 2, "task_id": 2001, "to_dev": "DEV-002",
                          "to_ip": "192.168.1.20", "network_diagnose": 1}}}),
    request={"task_id": 2001, "task_type": 0},
    state_before={"diagnostics": {"results": {"2001": {"devices": {"7": {"from": "old"}}}}}})
# 12.1.1, the document's example.
_ka_tel("switch-info", "/v1/switch/sys/info", {
    "basic_info": {"loc_mac": "18:C3:F4:2F:FB:F3", "uptime": "1d 07:10:14", "soft_version": "0.03.0268",
                   "hw_version": "V0.0-V1-0.0-0.0"},
    "dev_info": {"cpu_usage": "10%", "mem_usage": "12%", "temperature": "73", "fan": "87%"},
    "model": "AVX24-4", "alias": "AVX24-4", "sn": "18c3f425640e", "kls_link_status": "disconnected",
    "hostname": "AVX24-4-lion", "used_port": 6, "total_port": 28, "mode": "layer3"},
    {"switch": {"mac": "18:C3:F4:2F:FB:F3", "uptime": "1d 07:10:14", "software_version": "0.03.0268",
                "hardware_version": "V0.0-V1-0.0-0.0", "cpu": "10%", "memory": "12%", "temperature": 73.0,
                "fan": "87%", "model": "AVX24-4", "alias": "AVX24-4", "serial": "18c3f425640e",
                "kls_link_status": "disconnected", "hostname": "AVX24-4-lion", "used_ports": 6, "total_ports": 28,
                "mode": "layer3"}})
# 12.2.1, the document's example port.
_KA_PORT = {"port": 11, "alias": "port11",
            "vlan": {"type": "access", "pvid": 3, "vid": "3", "alias": "vlan3", "color": "#4298ED", "config_file": ""},
            "link": {"enable": True, "link": True, "media": "copper", "speed_duplex": "1G", "autonego": False,
                     "flow_control": True},
            "aggregation": {"aggre_id": 0, "type": "-"},
            "rate": {"rx": {"packets": "5", "bytes": "1398", "error": "0", "loss": "0"},
                     "tx": {"packets": "3", "bytes": "620", "error": "0", "loss": "0"}},
            "eee": {"actiphy": False, "perfect_reach": False, "enable": False},
            "poe": {"enable": True, "type": "PoE", "power": "14.85", "voltage": "54.02", "current": "275"},
            "bind_ip": {"enable": False, "ip": ""}, "igmp": {"port": 11, "router": False, "uplink": True},
            "ptp_clock_domain": 0, "device_ip": "192.168.3.38", "model": "N60", "image": "./static/img/N60.png",
            "company": "Kiloview"}
_ka_tel("ports", "/v1/switch/port/dump", [_KA_PORT], {"ports": {"11": {
    "alias": "port11",
    "vlan": {"type": "access", "pvid": 3, "vid": "3", "alias": "vlan3", "color": "#4298ED", "config_file": ""},
    "link": {"enabled": True, "up": True, "media": "copper", "speed_duplex": "1G", "autonego": False,
             "flow_control": True},
    "aggregation": {"id": 0, "type": "-"},
    "rx": {"packets": 5, "bytes": 1398, "errors": 0, "loss": 0}, "tx": {"packets": 3, "bytes": 620, "errors": 0, "loss": 0},
    "eee": {"actiphy": False, "perfect_reach": False, "enabled": False},
    "poe": {"enabled": True, "type": "PoE", "power": 14.85, "voltage": 54.02, "current": 275.0},
    "bind_ip": {"enabled": False, "ip": ""}, "igmp": {"router": False, "uplink": True}, "ptp_clock_domain": 0,
    "device_ip": "192.168.3.38", "device_model": "N60", "device_image": "./static/img/N60.png",
    "device_company": "Kiloview"}}},
    state_before={"ports": {"12": {"alias": "port12"}}})
_ka_tel("port-traffic", "/v1/switch/port/traffic", {"port": "1", "traffic": [
    {"rx": {"packets": "8438", "bytes": "12236088", "error": "0", "loss": "0"},
     "tx": {"packets": "29", "bytes": "10757", "error": "0", "loss": "0"}}]},
    {"port_traffic": {"1": '[{"rx":{"packets":"8438","bytes":"12236088","error":"0","loss":"0"},'
                           '"tx":{"packets":"29","bytes":"10757","error":"0","loss":"0"}}]'}},
    request={"port": 1})
_ka_tel("traffic-top", "/v1/switch/port/traffic/query/top", {
    "sum_rx_rate": 93.395, "sum_tx_rate": 0.24,
    "desc_stat": [{"port": "1", "rx_rate": 93.318, "tx_rate": 0.036, "top_rate": 93.318, "top_usage": 0.093},
                  {"port": "23", "rx_rate": 0.032, "tx_rate": 0.09, "top_rate": 0.09, "top_usage": 0}]},
    {"traffic": {"rx_rate_total": 93.395, "tx_rate_total": 0.24, "top": {
        "0": {"port": 1, "rx_rate": 93.318, "tx_rate": 0.036, "top_rate": 93.318, "top_usage": 0.093},
        "1": {"port": 23, "rx_rate": 0.032, "tx_rate": 0.09, "top_rate": 0.09, "top_usage": 0.0}}}},
    state_before={"traffic": {"top": {"2": {"port": 4}}}})
_ka_tel("watched-ports", "/v1/switch/port/display/query", None,
        {"watched_ports": {"select": {"11": True}}},
        body=_ka_j({"cascade_ports": [], "select_ports": [_KA_PORT]}),
        state_before={"watched_ports": {"select": {"4": True}, "cascade": {"2": True}}})
_ka_tel("watched-ports-envelope", "/v1/switch/port/display/query",
        {"cascade_ports": [dict(_KA_PORT, port=33)], "select_ports": [_KA_PORT]},
        {"watched_ports": {"select": {"11": True}, "cascade": {"33": True}}})
_ka_tel("aggregations", "/v1/switch/aggr/dump",
        [{"aggre_id": 1, "alias": "Group 1", "type": "static", "configured": [1, 2, 3], "aggregated": [1, 2]}],
        {"aggregations": {"1": {"alias": "Group 1", "type": "static", "configured": "[1,2,3]", "aggregated": "[1,2]"}}},
        state_before={"aggregations": {"2": {"alias": "gone"}}})
_ka_tel("aggregation", "/v1/switch/aggr/get",
        [{"aggre_id": 2, "alias": "B", "type": "lacp_active", "configured": [5], "aggregated": []}],
        {"aggregations": {"2": {"alias": "B", "type": "lacp_active", "configured": "[5]", "aggregated": "[]"}}},
        request={"aggre_id": [2]})
_KA_VLAN = {"id": 1, "alias": "vlan1", "color": "#1D2024", "configs": [], "config_names": [], "ports": [1, 2, 33],
            "interface": {"enable": True, "ip": "172.30.46.107", "netmask": "255.255.255.0", "type": "dhcp",
                          "referenced": True, "is_default_route": False},
            "dhcp_enable": False, "vlan_if": {"vlan_id": 1, "enable": True, "ip": "172.30.46.254",
                                              "netmask": "255.255.255.0"}, "dhcp_conflict": False}
_KA_VLANS = {"alias": "vlan1", "color": "#1D2024", "configs": "[]", "config_names": "[]", "ports": "[1,2,33]",
             "interface": {"enabled": True, "ip": "172.30.46.107", "netmask": "255.255.255.0", "type": "dhcp",
                           "referenced": True, "default_route": False},
             "dhcp_enabled": False, "dhcp_conflict": False,
             "vlan_if": {"enabled": True, "ip": "172.30.46.254", "netmask": "255.255.255.0"}}
_ka_tel("vlans", "/v1/switch/vlan/dump", [_KA_VLAN], {"vlans": {"1": _KA_VLANS}},
        state_before={"vlans": {"20": {"alias": "gone"}}},
        expect_then_send=_ka_then("/v1/switch/dhcps/get?vlan_id=1"))
_ka_tel("vlan", "/v1/switch/vlan/query", dict(_KA_VLAN, id=3), {"vlans": {"3": _KA_VLANS}}, request={"vlan_id": 3})
_ka_tel("dhcp", "/v1/switch/dhcps/get?vlan_id=1", {
    "vlan_if": {"vlan_id": 1, "enable": True, "ip": "192.168.1.1", "netmask": "255.255.255.0"},
    "dhcp": {"vlan_id": 1, "enable": True, "ip_pool": {"start": "192.168.1.100", "end": "192.168.1.200"},
             "gateway": {"type": "default", "ip": "192.168.1.1"}, "dns": "8.8.8.8",
             "lease_time": {"days": 1, "hours": 0, "minutes": 0}}},
    {"dhcp_servers": {"1": {"interface_enabled": True, "interface_ip": "192.168.1.1",
                            "interface_netmask": "255.255.255.0", "enabled": True, "pool_start": "192.168.1.100",
                            "pool_end": "192.168.1.200", "gateway_type": "default", "gateway_ip": "192.168.1.1",
                            "dns": "8.8.8.8", "lease_days": 1, "lease_hours": 0, "lease_minutes": 0}}})
_KA_CLIENT = {"vlan_id": 1, "ip": "192.168.1.50", "netmask": "255.255.255.0", "type": "Manual",
              "host_name": "device-001", "mac": "00:11:22:33:44:55", "expired_time": {"days": 0, "hours": 12,
                                                                                       "minutes": 0},
              "brand": "kiloview", "mode": "N60", "sn": "DSGK123456789", "connect_port": 5, "bind_type": 3,
              "color": "#FF0000"}
_KA_CLIENTS = {"vlan": 1, "ip": "192.168.1.50", "netmask": "255.255.255.0", "type": "Manual",
               "host_name": "device-001", "lease_days": 0, "lease_hours": 12, "lease_minutes": 0, "brand": "kiloview",
               "model": "N60", "serial": "DSGK123456789", "port": 5, "binding": "ip_mac", "color": "#FF0000"}
_ka_tel("dhcp-clients", "/v1/switch/dhcps/clients/dump", [_KA_CLIENT],
        {"dhcp_clients": {"00:11:22:33:44:55": _KA_CLIENTS}}, request={"vlan_id": []},
        state_before={"dhcp_clients": {"AA:AA:AA:AA:AA:AA": {"ip": "10.0.0.9"}}})
_ka_tel("dhcp-clients-filtered", "/v1/switch/dhcps/clients/dump", [_KA_CLIENT],
        {"dhcp_clients": {"AA:AA:AA:AA:AA:AA": {"ip": "10.0.0.9"}, "00:11:22:33:44:55": _KA_CLIENTS}},
        request={"vlan_id": [1]}, state_before={"dhcp_clients": {"AA:AA:AA:AA:AA:AA": {"ip": "10.0.0.9"}}})
_ka_tel("dhcp-static", "/v1/switch/dhcps/static/get", [dict(_KA_CLIENT, bind_type=0)],
        {"dhcp_static": {"00:11:22:33:44:55": dict(_KA_CLIENTS, binding="none")}}, request={},
        state_before={"dhcp_static": {"AA:AA:AA:AA:AA:AA": {"ip": "10.0.0.9"}}})
_ka_tel("dhcp-static-filtered", "/v1/switch/dhcps/static/get", [dict(_KA_CLIENT, bind_type=2)],
        {"dhcp_static": {"AA:AA:AA:AA:AA:AA": {"ip": "10.0.0.9"},
                         "00:11:22:33:44:55": dict(_KA_CLIENTS, binding="mac")}},
        request={"vlan_id": [1]}, state_before={"dhcp_static": {"AA:AA:AA:AA:AA:AA": {"ip": "10.0.0.9"}}})
_ka_tel("power", "/v1/switch/power/status/query", [
    {"slot": "slot1", "name": "AVX-KILOLINK-920-A", "status": "Slot Empty", "power": ""},
    {"slot": "PoE", "name": "PoE Total Power(Real-time)", "status": "Power On", "power": "23.77W"}],
    {"power": {"slot1": {"name": "AVX-KILOLINK-920-A", "status": "Slot Empty", "power": ""},
               "PoE": {"name": "PoE Total Power(Real-time)", "status": "Power On", "power": "23.77W"}}})
_ka_tel("routes", "/v1/switch/iproutes/get", None,
        {"routes": {"0": {"dest_ip": "192.168.2.0", "dest_mask": 24, "next_hop": "192.168.1.1", "priority": 100}}},
        body=_ka_j({"code": 200, "message": "ok", "routes": _KA_RT}),
        state_before={"routes": {"1": {"dest_ip": "10.0.0.0"}}})
_ka_tel("network-status", "/v1/network/query", {"status": False}, {"network_config": {"status": False}})
_KA_ALARM = {"id": 1, "status": 1, "alarm_level": 1, "time": "2024-01-01 10:00:00", "message": "CPU over 80%",
             "source_port": 1}
_KA_ALARMS = {"status": "unread", "level": "error", "time": "2024-01-01 10:00:00", "message": "CPU over 80%", "port": 1}
_ka_tel("alarms", "/v1/alarm/events/levels/get?alarm_levels=1&alarm_levels=2&alarm_levels=3", None,
        {"alarms": {"count": 50, "unread_errors": 10, "unread_warnings": 30, "unread_info": 10,
                    "events": {"1": _KA_ALARMS}}},
        body=_ka_j({"code": 200, "message": "ok", "events_num": 50, "error_total": 10, "warning_total": 30,
                    "info_total": 10, "data": [_KA_ALARM]}),
        state_before={"alarms": {"events": {"9": {"status": "read"}}}})
_ka_tel("alarms-level", "/v1/alarm/events/levels/get?alarm_levels=2", None,
        {"alarms": {"count": 1, "unread_errors": 0, "unread_warnings": 1, "unread_info": 0,
                    "events": {"9": {"status": "read"}, "2": dict(_KA_ALARMS, level="warning")}}},
        body=_ka_j({"code": 200, "message": "ok", "events_num": 1, "error_total": 0, "warning_total": 1,
                    "info_total": 0, "data": [dict(_KA_ALARM, id=2, alarm_level=2)]}),
        state_before={"alarms": {"events": {"9": {"status": "read"}}}})
_ka_tel("device-alarms", "/v1/alarm/dev/events/get?mac=AA", [dict(_KA_ALARM, id=5, status=2)],
        {"alarms": {"events": {"5": dict(_KA_ALARMS, status="read")}}})
_ka_tel("alarm-page", "/v1/alarm/events/get", None, {"alarms": {"events": {"6": dict(_KA_ALARMS, level="info")}}},
        body=_ka_j({"code": 200, "message": "ok", "total": 100, "page_index": 1, "page_size": 20,
                    "data": [dict(_KA_ALARM, id=6, alarm_level=3)]}))
_ka_tel("alarm-config", "/v1/alarm/config/get", {"cpu_enabled": True, "mem_enabled": True, "temp_enabled": False,
                                                 "alarm_interval": 1, "disconnect_alarm_enabled": True,
                                                 "disconnect_duration": 1},
        {"alarms": {"config": {"cpu": True, "memory": True, "temperature": False, "interval": 1, "disconnect": True,
                               "disconnect_duration": 1}}})
_ka_tel("port-alarms", "/v1/alarm/ports/events/get", {
    "1": {"has_alarm": True, "has_error": True, "port_alarm_events": [_KA_ALARM]},
    "2": {"has_alarm": False, "has_error": False, "port_alarm_events": []}},
    {"port_alarms": {"1": {"has_alarm": True, "has_error": True}, "2": {"has_alarm": False, "has_error": False}}},
    state_before={"port_alarms": {"3": {"has_alarm": True}}})
_ka_tel("port-alarm", "/v1/alarm/debug/port", {"code": 1, "message": "string", "has_alarm": True, "has_error": False,
                                               "alarm_events": [], "error_events": []},
        {"port_alarms": {"4": {"has_alarm": True, "has_error": False}}}, request={"port_id": 4})
_KA_BOARD = {"cpu_usage": "12%", "mem_usage": "30%", "temperature": "45", "software_version": "1.0",
             "hardware_version": "V1"}
_KA_BOARDS = {"cpu": "12%", "memory": "30%", "temperature": "45", "software_version": "1.0",
              "hardware_version": "V1"}
_ka_tel("resources", "/v1/test/get_resources", {"exchange_board": _KA_BOARD, "ev4": _KA_BOARD, "a311d2": _KA_BOARD,
                                                "serial_number": "SN1", "firmware_version": "2.0",
                                                "hardware_version": "V1.0"},
        {"boards": {"exchange_board": _KA_BOARDS, "ev4": _KA_BOARDS, "a311d2": _KA_BOARDS},
         "device": {"serial": "SN1", "firmware_version": "2.0", "hardware_version": "V1.0"}})
_ka_tel("fans", "/v1/test/fan_list", ["fan1", "fan2"], {"test": {"fans": '["fan1","fan2"]'}})
_ka_tel("usb", "/v1/test/usb_port_status", "connected", {"test": {"usb": "connected"}})
_ka_tel("fan-test", "/v1/test/start_fan_test", {"fans": [{"name": "fan1", "status": "ok", "rpm": 5200}]},
        {"test": {"fan_test": {"fan1": {"status": "ok", "rpm": 5200}}}},
        state_before={"test": {"fan_test": {"fan9": {"status": "bad"}}}})
_ka_tel("port-test", "/v1/test/port_status",
        [{"port": 1, "speed_duplex": "1G", "link": True, "poe_power": "3.2", "test_result": "pass"}],
        {"test": {"ports": {"1": {"speed_duplex": "1G", "link": True, "poe_power": "3.2", "result": "pass"}}}})
# 15.1, 15.2, the document's examples.
_ka_tel("system-info", "/api/systemctrl/report/get_system_info", {
    "hostname": "AVX24-4", "version": {"hardwareVersion": "V1.0", "softwareVersion": "V2.0.0",
                                       "firmwareVersion": "V1.5.0", "serialnumber": "SN000000000001",
                                       "ndiVersion": "NDI 5.5", "product": "AVX24-4", "oem": "Brand"},
    "mac": "00:11:22:33:44:55", "mode": "L2", "cpu": {"percent": 25}, "temperature": 45,
    "mem": {"percent": 60, "total": 4096, "used": 2458}, "disk": {"percent": 30, "total": 10240, "used": 3072}},
    {"system": {"hostname": "AVX24-4", "hardware_version": "V1.0", "software_version": "V2.0.0",
                "firmware_version": "V1.5.0", "serial": "SN000000000001", "ndi_version": "NDI 5.5",
                "product": "AVX24-4", "oem": "Brand", "mac": "00:11:22:33:44:55", "mode": "L2", "cpu": 25,
                "temperature": 45, "memory": 60, "memory_total": 4096, "memory_used": 2458, "disk": 30,
                "disk_total": 10240, "disk_used": 3072}})
_ka_tel("network", "/api/networkmanager/report/get_network", [
    {"name": "eth1.1", "type": "eth", "mac": "00:11:22:33:44:55", "address": "192.168.1.100",
     "netmask": "255.255.255.0", "gw": "192.168.1.1", "dns": "8.8.8.8", "method": "static", "status": "up",
     "multicast": "yes", "receive": 102400, "transmit": 51200}],
    {"network": {"interfaces": {"eth1.1": {"type": "eth", "mac": "00:11:22:33:44:55", "ip": "192.168.1.100",
                                           "netmask": "255.255.255.0", "gateway": "192.168.1.1", "dns": "8.8.8.8",
                                           "method": "static", "status": "up", "multicast": "yes",
                                           "received": 102400, "transmitted": 51200}}}},
    state_before={"network": {"interfaces": {"eth0": {"status": "down"}}}})

# Read-backs after a change the device accepted.
_KA_OKB = '{"code":200,"message":"ok","data":null}'


def _ka_reread(name, path, request, then, **extra):
    inbound = {"path": path, "body": extra.pop("body", _KA_OKB)}
    if request is not None:
        inbound["request"] = request
    telemetry(_KA, name, inbound_http=inbound, expect_then_send=then, expect_state=extra.pop("expect_state", {}),
              **extra)


_KA_LEVELS = "/v1/alarm/events/levels/get?alarm_levels=1&alarm_levels=2&alarm_levels=3"
_ka_reread("reread-user", "/v1/user/add", {"username": "u"}, _ka_then("/v1/user/userlist"))
_ka_reread("reread-hostname", "/v1/sys/sethostname?hostname=AVX", None,
           _ka_then("/v1/switch/sys/info", "/api/systemctrl/report/get_system_info"))
_ka_reread("reread-time", "/v1/systime/setLocation", {"timetype": "pc", "offset": 8}, _ka_then("/v1/systime/get"))
_ka_reread("reread-ptp", "/v1/ptp/modify", {"id": 1}, _ka_then("/v1/ptp/query"))
_ka_reread("reread-preset", "/v1/preset/save", {"prname": "a"}, _ka_then("/v1/preset/getlist"))
_ka_reread("reread-preset-load", "/v1/preset/read", {"prname": "a"},
           _ka_then("/v1/preset/getlist", "/v1/panel/query/all", "/v1/output/query/all", "/v1/switch/vlan/dump",
                    "/v1/switch/port/dump"))
_ka_reread("reread-virtual-nic", "/v1/preset/vlan/interface/set", {"vlan_id": 10, "interface": {}},
           _ka_then("/v1/preset/vlan/interface/get?vlan_id=10", "/v1/preset/vlan/interface/list/get",
                    "/v1/switch/vlan/dump", "/v1/switch/interface/ip/get"),
           body='{"code":200,"message":"ok"}')
_ka_reread("reread-mode", "/v1/preset/mode/set", {"mode": "layer2"},
           _ka_then("/v1/preset/mode/get", "/v1/switch/sys/info"))
_ka_reread("reread-discovery", "/v1/input/auto/add", {"dis_g_name": "public"}, _ka_then("/v1/input/discovery/all"))
_ka_reread("reread-input", "/v1/input/modify", {"id": 1, "alias": "a"}, _ka_then("/v1/panel/query/all"))
_ka_reread("reread-output", "/v1/output/delete", {"ids": [1]}, _ka_then("/v1/output/query/all", "/v1/panel/query/all"))
_ka_reread("reread-panel", "/v1/panel/expand", {"id": 1, "expand": True}, _ka_then("/v1/panel/query/all"))
_ka_reread("reread-panel-delete", "/v1/panel/delete", {"ids": [2]}, _ka_then("/v1/panel/query/all"),
           state_before={"panel_info": {"1": {"expanded": True}, "2": {"expanded": False}},
                         "panel_outputs": {"2": {"1": {"input": 3}}}},
           expect_state={"panel_info": {"1": {"expanded": True}}, "panel_inputs": {}, "panel_outputs": {},
                         "panel_config": {}})
_ka_reread("reread-panel-inputs", "/v1/panel/input/sort", {"id": 2, "input_ids": [3, 1]},
           _ka_then("/v1/panel/query/all", "/v1/panel/detail?id=2"))
_ka_reread("reread-route", "/v1/panel/cross/output/input", {"id": 3, "output_id": 1, "input_id": 2},
           _ka_then("/v1/panel/detail?id=3"))
_ka_reread("reread-route-error", "/v1/panel/cross/output/input", {"id": 3, "output_id": 1, "input_id": 2}, [],
           body='{"code":500,"message":"locked","data":null}')
_ka_reread("reread-input-config", "/v1/panel/cfg/input/apply", {"id": 2, "input_id": 1},
           _ka_then("/v1/panel/cfg/input/all?id=2"))
_ka_reread("reread-output-config", "/v1/panel/cfg/output/apply", {"id": 2, "output_id": 1},
           _ka_then("/v1/panel/cfg/output/all?id=2"))
_ka_reread("reread-physical-panel", "/v1/panel/cfg/enable/set", {"id": 2, "enable": False},
           _ka_then("/v1/panel/cfg/enable/query?id=2", "/v1/front/panel/query"))
_ka_reread("reread-kls-output", "/v1/kls/output/modify", {"uuid": "u"}, _ka_then("/v1/kls/output/query/all"))
_ka_reread("reread-kls-route", "/v1/kls/panel/cross/output/input",
           {"panel_uuid": "panel uuid", "output_uuid": "o", "input_name": "i"},
           _ka_then("/v1/kls/panel/detail?panel_uuid=panel%20uuid"))
_ka_reread("reread-kls-panel-delete", "/v1/kls/panel/delete", {"uuid": "panel-uuid-1", "serial_number": "S"}, [],
           state_before={"kls_panels": {"panel-uuid-1": {"inputs": {"a": {"alias": "a"}}},
                                        "p2": {"inputs": {"b": {"alias": "b"}}}}},
           expect_state={"kls_panels": {"p2": {"inputs": {"b": {"alias": "b"}}}}})
_ka_reread("reread-kls-switcher", "/v1/kls/panel/switcher/enable", {"serial_number": "S"},
           _ka_then("/v1/kls/output/query/all"))
_ka_reread("reread-kls-bind", "/v1/bindkls/tobind", {"ip": "1.2.3.4"}, _ka_then("/v1/bindkls/bindinfo"))
_ka_reread("reread-kls-unbind", "/v1/bindkls/unbind?id=1", None, _ka_then("/v1/bindkls/bindinfo"))
_ka_reread("reread-icons", "/v1/icon/notify/update", {"icon_name": "a.png"}, _ka_then("/v1/icon/all"))
_ka_reread("reread-take", "/v1/front/input/output/take", {"panel_id": 2, "input_id": 1, "output_ids": [1]},
           _ka_then("/v1/front/panel/detail?panel_id=2", "/v1/panel/query/all"))
_ka_reread("reread-vlan-profile", "/v1/vlan/cfg/modify", {"id": 1, "vlan_ids": "1"}, _ka_then("/v1/vlan/cfg/all"))
_ka_reread("reread-device-alias", "/v1/switch/dev/alias/set", {"mac": "a"}, _ka_then("/v1/topology/query"),
           body='{"code":200,"message":"ok","data":""}')
_ka_reread("reread-diagnostic", "/v1/diagnose/task/p2pdev/add", {"type": 0}, _ka_then("/v1/diagnose/task/query/type"),
           body='{"code":200,"message":"ok","data":{"task_id":3001}}')
_ka_reread("reread-port", "/v1/switch/port/set/batch", {"msg": {}}, _ka_then("/v1/switch/port/dump"),
           body='{"code":200,"message":"ok","data":""}')
_ka_reread("reread-watched", "/v1/switch/port/display/set", {"type": "select", "ports": [1]},
           _ka_then("/v1/switch/port/display/query"))
_ka_reread("reread-device-remove", "/v1/switch/device/remove", {"port": 1},
           _ka_then("/v1/switch/port/dump", "/v1/topology/query"))
_ka_reread("reread-aggregation", "/v1/switch/aggr/set", {"msg": []},
           _ka_then("/v1/switch/aggr/dump", "/v1/switch/port/dump"), body='{"code":200,"message":"ok","data":{}}')
_ka_reread("reread-vlan", "/v1/switch/vlan/del", {"vlans": [10]},
           _ka_then("/v1/switch/vlan/dump", "/v1/switch/port/dump"))
_ka_reread("reread-dhcp", "/v1/switch/dhcps/set", {"vlan_if": {"vlan_id": 7}, "dhcp": {}},
           _ka_then("/v1/switch/dhcps/get?vlan_id=7", "/v1/switch/vlan/dump"), body='{"code":200,"message":"ok"}')
_ka_reread("reread-dhcp-enable", "/v1/switch/dhcps/enable/set", {"vlan_id": 10, "enable": False},
           _ka_then("/v1/switch/dhcps/get?vlan_id=10", "/v1/switch/vlan/dump"), body='{"code":200,"message":"ok"}')
_ka_reread("reread-static", "/v1/switch/dhcps/static/set", {"data": []},
           [{"method": "POST", "target": "/v1/switch/dhcps/static/get", "body": "{}",
             "content_type": "application/json"}], body='{"code":200,"message":"ok"}')
_ka_reread("reread-route-table", "/v1/switch/iproutes/add", {"dest_ip": "a"}, _ka_then("/v1/switch/iproutes/get"),
           body='{"code":200,"message":"ok"}')
_ka_reread("reread-network", "/v1/network/save", None, _ka_then("/v1/network/query"))
_ka_reread("reread-alarms", "/v1/alarm/events/status/set", {"ids": [1], "status": 2}, _ka_then(_KA_LEVELS),
           body='{"code":200,"message":"ok"}')
_ka_reread("reread-alarm-config", "/v1/alarm/config/set", {"cpu_enabled": True}, _ka_then("/v1/alarm/config/get"),
           body='{"code":200,"message":"ok"}')
_ka_reread("reread-serial", "/v1/test/set_serial_number", {"serial_number": "S"},
           _ka_then("/v1/test/get_resources", "/v1/switch/sys/info"),
           body='{"code":200,"message":"ok","data":{"code":1,"message":"string"}}')
# Downloads saved to the host file named (never written by a vector).
V.append({"spec": "kiloview-avx", "command": "download_alarm_export", "input": {"file_path": "alarm_export_1.xlsx", "to": {"file": "alarms.xlsx"}}, "expect_request": {"method": "POST", "target": "/v1/alarm/file/download", "body": "{\"file_path\":\"alarm_export_1.xlsx\"}", "content_type": "application/json"}})


# The session: with a password, a command's first request is the login, its
# token then sent as a bearer token (the engine's session tests cover the
# header and the login again on 401). The other vectors leave the password
# empty, so each command's own request is the one compared.
V.append({"spec": _KA, "command": "get_users", "input": {}, "file": "get_users-login",
          "settings": {"password": "admin"},
          "expect_request": {"method": "POST", "target": "/v1/user/login",
                             "body": '{"username":"admin","password":"admin"}', "content_type": "application/json"}})
