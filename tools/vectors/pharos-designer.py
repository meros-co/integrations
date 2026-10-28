PH = "pharos-designer"
# Pharos Controller API, Release 12.0: HTTP API requests with JSON bodies,
# and WebSocket broadcasts on /query. Bodies use the reference's own examples
# where it gives one; optional attributes the spec always sends (fade, delay,
# path) are added in the order the spec writes them.


def ph(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": PH, "command": command, "input": input, "expect_request": request, **extra})


PH_OK = {"status": 200, "body": ""}
ph("toggle_beacon", {}, "POST", "/api/beacon", http_reply=PH_OK, expect_result={"ok": {"kind": "ack"}})
ph("set_beacon", {"enabled": True}, "PUT", "/api/beacon", '{"enable":true}')
ph("park_channels", {"universe": "dmx:1", "channels": "1-3,5", "level": 128}, "POST", "/api/channel",
   '{"universe":"dmx:1","channels":"1-3,5","level":128}')
ph("unpark_channels", {"universe": "rio-dmx:rio44:1", "channels": "1-3,5"}, "DELETE", "/api/channel",
   '{"universe":"rio-dmx:rio44:1","channels":"1-3,5"}')
ph("set_cloud_device_key", {"key": "abc123"}, "POST", "/api/cloud", '{"action":"set_device_key","cloud_device_key":"abc123"}')
ph("clear_cloud_device_key", {}, "POST", "/api/cloud", '{"action":"clear_device_key"}')
# "tl = 1 get_timeline(tl):start()"; answered "Executed".
ph("run_command", {"input": "tl = 1 get_timeline(tl):start()"}, "POST", "/api/cmdline",
   '{"input":"tl = 1 get_timeline(tl):start()"}', http_reply={"status": 200, "body": "Executed"},
   expect_result={"ok": {"kind": "value", "value": "Executed"}})
ph("set_ip", {"ip": "192.168.1.3", "subnet_mask": "255.255.255.0", "gateway": "192.168.1.1"}, "POST", "/api/config",
   '{"ip":"192.168.1.3","subnet_mask":"255.255.255.0","gateway":"192.168.1.1"}',
   http_reply={"status": 200, "body": '{"restart":true}'}, expect_result={"ok": {"kind": "value", "value": True}})
ph("set_dhcp", {"enabled": True}, "POST", "/api/config", '{"dhcp_enabled":true}')
ph("set_name_servers", {"primary": "192.168.1.1", "secondary": "8.8.8.8"}, "POST", "/api/config",
   '{"name_server_1":"192.168.1.1","name_server_2":"8.8.8.8"}')
ph("set_web_ports", {"http_port": 80, "https_port": 443}, "POST", "/api/config", '{"http_port":80,"https_port":443}')
ph("set_datetime", {"datetime": "2024-06-27T09:30"}, "POST", "/api/config", '{"datetime":"2024-06-27T09:30"}')
ph("set_watchdog", {"enabled": True}, "POST", "/api/config", '{"watchdog_enabled":true}')
ph("set_log_level", {"level": 3}, "POST", "/api/config", '{"log_level":3}')
ph("set_syslog", {"enabled": False, "ip": "192.168.1.2"}, "POST", "/api/config", '{"syslog_enabled":false,"syslog_ip":"192.168.1.2"}')
ph("set_ntp", {"enabled": True, "ip": "192.168.1.1"}, "POST", "/api/config", '{"ntp_enabled":true,"ntp_ip":"192.168.1.1"}')
ph("mark_dali_fixed", {"interface": 0, "address": 12}, "POST", "/api/dali", '{"interface":0,"address":12,"action":"mark_fixed"}')
ph("refresh_dali", {"interface": 1, "address": 0}, "POST", "/api/dali", '{"interface":1,"address":0,"action":"refresh"}')
ph("factory_reset", {}, "POST", "/api/factory_reset")
ph("set_content_target_master", {"type": "secondary", "level": 0.5, "fade": 2.0, "delay": 2.0}, "POST", "/api/content_target",
   '{"action":"master_intensity","type":"secondary","level":0.500,"fade":2.000,"delay":2.000}', model="vlc-plus")
ph("set_group_master", {"group": 1, "level": 0.5, "fade": 2.0}, "POST", "/api/group",
   '{"action":"master_intensity","num":1,"level":0.500,"fade":2.000,"delay":0.000}')
ph("set_output_enabled", {"protocol": "art-net", "enabled": False}, "POST", "/api/output", '{"protocol":"art-net","action":"disable"}')
ph("override_intensity", {"target": "fixture", "num": 1, "intensity": 128}, "PUT", "/api/override",
   '{"target":"fixture","num":1,"intensity":128,"fade":0.000,"path":"Default"}')
# "Override group 1 ... intensity 255, colour red 255, green 255, blue 0".
ph("override_rgb", {"target": "group", "num": 1, "intensity": 255, "red": 255, "green": 255, "blue": 0}, "PUT", "/api/override",
   '{"target":"group","num":1,"intensity":255,"colour":{"red":255,"green":255,"blue":0},"fade":0.000,"path":"Default"}')
ph("override_hue_saturation", {"target": "fixture", "num": 2, "intensity": 128, "hue": 200, "saturation": 240}, "PUT",
   "/api/override",
   '{"target":"fixture","num":2,"intensity":128,"colour":{"hue":200,"saturation":240},"fade":0.000,"path":"Default"}')
# "Override group 3 colour temperature of 44 with a fade time of 5 seconds".
ph("override_temperature", {"target": "group", "num": 3, "intensity": 255, "temperature": 44, "fade": 5.0, "path": "Braked"}, "PUT",
   "/api/override", '{"target":"group","num":3,"intensity":255,"temperature":44,"fade":5.000,"path":"Braked"}')
ph("override_snapshot", {"target": "group", "num": 0}, "PUT", "/api/override",
   '{"target":"group","num":0,"intensity":"snapshot","colour":"snapshot"}')
ph("release_override", {"target": "fixture", "num": 1, "fade": 2.0}, "DELETE", "/api/override", '{"target":"fixture","num":1,"fade":2.000}')
ph("release_all_overrides", {"fade": 0.0}, "DELETE", "/api/override", '{"fade":0.000}')
V.append({"spec": PH, "command": "upload_project", "input": {"file": "UEhBUk9T"},
          "expect_request": {"method": "POST", "target": "/api/project/file", "body_hex": b"PHAROS".hex(),
                             "content_type": "application/vnd.pharos"}})
# "to start a full discovery on DMX universe 2 ... {"universe": "dmx:2"}"; 202.
ph("rdm_discovery", {"universe": "dmx:2"}, "POST", "/api/rdm/discovery", '{"universe":"dmx:2"}',
   http_reply={"status": 202, "body": ""}, expect_result={"ok": {"kind": "ack"}})
ph("rdm_discovery_update", {"universe": "edn:edn20:4:1"}, "PUT", "/api/rdm/discovery", '{"universe":"edn:edn20:4:1"}')
ph("rdm_patch_assign", {"uid": "072c:0004fe02", "fixture": 123}, "PUT", "/api/rdm_devices/072c%3A0004fe02/patch",
   '{"action":"assign","fixture_number":123}')
ph("rdm_patch_replace", {"uid": "072c:0004fe02", "target": "1234:56789abc"}, "PUT", "/api/rdm_devices/072c%3A0004fe02/patch",
   '{"action":"replace","target_device_id":"1234:56789abc"}')
ph("rdm_get", {"request": {"universe": "dmx:2", "destination_uid": "072c:0004fe02", "pid": "DEVICE_INFO"}}, "POST", "/api/rdm/get",
   '{"universe":"dmx:2","destination_uid":"072c:0004fe02","pid":"DEVICE_INFO"}')
ph("rdm_set", {"request": {"universe": "dmx:2", "destination_uid": "072c:0004fe02", "pid": "IDENTIFY_DEVICE", "data": True}},
   "POST", "/api/rdm/set", '{"universe":"dmx:2","destination_uid":"072c:0004fe02","pid":"IDENTIFY_DEVICE","data":true}')
ph("reboot", {}, "POST", "/api/reset")
# Scene examples from the reference.
ph("start_scene", {"scene": 2}, "POST", "/api/scene", '{"action":"start","num":2}')
ph("start_scene_release_others", {"scene": 2, "group": "B", "fade": 2.0}, "POST", "/api/scene",
   '{"action":"start_release_others","num":2,"group":"B","fade":2.000}')
ph("start_scene_release_same_group", {"scene": 2}, "POST", "/api/scene",
   '{"action":"start_release_others","num":2,"same_group":true,"fade":0.000}')
ph("release_scene", {"scene": 2, "fade": 3.5}, "POST", "/api/scene", '{"action":"release","num":2,"fade":3.500}')
ph("toggle_scene", {"scene": 2, "fade": 2.0}, "POST", "/api/scene", '{"action":"toggle","num":2,"fade":2.000}')
ph("release_all_scenes", {"fade": 2.0}, "POST", "/api/scene", '{"action":"release","fade":2.000}')
ph("release_scenes_in_group", {"group": "!B", "fade": 2.0}, "POST", "/api/scene", '{"action":"release","group":"!B","fade":2.000}')
# Timeline examples from the reference.
ph("start_timeline", {"timeline": 2}, "POST", "/api/timeline", '{"action":"start","num":2}')
ph("start_timeline_release_others", {"timeline": 2, "group": "B", "fade": 2.0}, "POST", "/api/timeline",
   '{"action":"start_release_others","num":2,"group":"B","fade":2.000}')
ph("start_timeline_release_same_group", {"timeline": 2}, "POST", "/api/timeline",
   '{"action":"start_release_others","num":2,"same_group":true,"fade":0.000}')
ph("release_timeline", {"timeline": 2, "fade": 3.5}, "POST", "/api/timeline", '{"action":"release","num":2,"fade":3.500}')
ph("toggle_timeline", {"timeline": 2, "fade": 2.0}, "POST", "/api/timeline", '{"action":"toggle","num":2,"fade":2.000}')
ph("release_all_timelines", {"fade": 2.0}, "POST", "/api/timeline", '{"action":"release","fade":2.000}')
ph("release_timelines_in_group", {"group": "!B", "fade": 2.0}, "POST", "/api/timeline", '{"action":"release","group":"!B","fade":2.000}')
ph("pause_timeline", {"timeline": 4}, "POST", "/api/timeline", '{"action":"pause","num":4}')
ph("resume_timeline", {"timeline": 4}, "POST", "/api/timeline", '{"action":"resume","num":4}')
ph("pause_all_timelines", {}, "POST", "/api/timeline", '{"action":"pause"}')
ph("resume_all_timelines", {}, "POST", "/api/timeline", '{"action":"resume"}')
ph("set_timeline_rate", {"timeline": 5, "rate": "0.5"}, "POST", "/api/timeline", '{"action":"set_rate","num":5,"rate":"0.5"}')
ph("set_timeline_position", {"timeline": 1, "position": "1:3"}, "POST", "/api/timeline",
   '{"action":"set_position","num":1,"position":"1:3"}')
ph("set_timeline_time", {"timeline": 4, "time": 12.34}, "POST", "/api/timeline", '{"action":"set_position","num":4,"time":12.340}')
ph("set_timeline_flag", {"timeline": 5, "flag": "Start sparkle"}, "POST", "/api/timeline",
   '{"action":"set_position","num":5,"flag":"Start sparkle"}')
ph("refresh_status_monitor", {}, "POST", "/api/status_monitor", '{"action":"refresh"}')
ph("refresh_fixture_status", {"fixture": 1}, "POST", "/api/status_monitor", '{"action":"refresh","fixture_number":1}')
ph("set_text_slot", {"name": "myTextSlot", "value": "Hello World!"}, "PUT", "/api/text_slot",
   '{"name":"myTextSlot","value":"Hello World!"}')
ph("fire_trigger", {"trigger": 2}, "POST", "/api/trigger", '{"num":2}')
ph("fire_trigger_with_var", {"trigger": 2, "var": "2,4,5", "conditions": False}, "POST", "/api/trigger",
   '{"num":2,"var":"2,4,5","conditions":false}')
ph("add_user", {"session_password": "my_password", "username": "bob", "password": "bobs_password", "access": ["Control", "Status"]},
   "POST", "/api/user",
   '{"session_password":"my_password","username":"bob","password":"bobs_password","access":["Control","Status"]}')
ph("update_user", {"session_password": "my_password", "username": "bob", "password": "new_password", "access": ["Status"]},
   "PUT", "/api/user", '{"session_password":"my_password","username":"bob","password":"new_password","access":["Status"]}')
ph("delete_user", {"session_password": "my_password", "username": "bob"}, "DELETE", "/api/user",
   '{"session_password":"my_password","username":"bob"}')
ph("get_api_version", {}, "GET", "/api/api_version", http_reply={"status": 200, "body": '{"version": 12}'},
   expect_result={"ok": {"kind": "value", "value": 12}})
ph("get_lua_variables", {"variables": "bob,alice"}, "GET", "/api/lua?variables=bob%2Calice",
   http_reply={"status": 200, "body": '{"alice": 1234, "bob": "a string variable"}'},
   expect_result={"ok": {"kind": "value", "value": {"alice": 1234, "bob": "a string variable"}}})
ph("get_log", {}, "GET", "/api/log")
ph("get_output_levels", {"universe": "dmx:1"}, "GET", "/api/output?universe=dmx%3A1")
ph("get_fixtures", {}, "GET", "/api/fixtures?with_custom_properties=true&with_rdm_devices=true")
ph("get_fixture", {"fixture": 12}, "GET", "/api/fixtures/12")
ph("get_rdm_devices", {"filter": "offline"}, "GET", "/api/rdm_devices?offline=true")
ph("get_rdm_device", {"uid": "1234:56789abc"}, "GET", "/api/rdm_devices/1234%3A56789abc")
ph("get_rdm_discovery", {"universe": "dmx:2"}, "GET", "/api/rdm/discovery?universe=dmx%3A2")
ph("get_rdm_unpatched", {}, "GET", "/api/rdm/discovery/unpatched")
ph("get_rdm_parameter_descriptions", {}, "GET", "/api/rdm/parameter_descriptions")
ph("get_dali_interfaces", {}, "GET", "/api/dali_interface")
ph("get_dali", {"interface": 0}, "GET", "/api/dali?interface=0")
ph("get_user_groups", {}, "GET", "/api/user_groups", http_reply={"status": 200, "body": '{"user_groups": ["Admin", "Control", "Status"]}'},
   expect_result={"ok": {"kind": "value", "value": ["Admin", "Control", "Status"]}})
ph("get_guest_groups", {}, "GET", "/api/guest_groups")


def ph_poll(name, path, body, state, **extra):
    telemetry(PH, name, inbound_http={"path": path, "body": json.dumps(body)}, expect_state=state, **extra)


ph_poll("scenes", "/api/scene", {"scenes": [
    {"num": 1, "name": "Scene 1", "group": "Back of House", "group_num": 1, "state": "none", "onstage": True}]},
    {"scenes": {"1": {"name": "Scene 1", "group": "Back of House", "group_num": 1, "state": "none", "onstage": True}}},
    state_before={"scenes": {"9": {"name": "Deleted"}}})
ph_poll("timelines", "/api/timeline", {"timelines": [
    {"num": 1, "name": "Timeline 1", "group": "Back of House", "group_num": 1, "length": 10000, "source_bus": "internal",
     "timecode_format": "SMPTE30", "audio_band": 0, "audio_channel": "combined", "audio_peak": False, "time_offset": 5000,
     "state": "running", "onstage": True, "position": 10000, "rate_adjusted": True, "priority": "normal",
     "custom_properties": {}}]},
    {"timelines": {"1": {"name": "Timeline 1", "group": "Back of House", "group_num": 1, "length": 10000,
                         "source_bus": "internal", "timecode_format": "SMPTE30", "audio_band": 0, "audio_channel": "combined",
                         "audio_peak": False, "time_offset": 5000, "state": "running", "onstage": True, "position": 10000,
                         "rate_adjusted": True, "priority": "normal"}}})
ph_poll("groups", "/api/group", {"groups": [{"name": "All Fixtures", "level": 100}, {"num": 1, "name": "Group 1", "level": 100}]},
        {"groups": {"1": {"name": "Group 1", "level": 100}}})
ph_poll("content-targets", "/api/content_target", {"content_targets": [{"name": "Primary", "level": 100}, {"name": "Target 3", "level": 40}]},
        {"content_targets": {"Primary": {"level": 100}, "Target 3": {"level": 40}}}, model="vlc-plus")
ph_poll("inputs", "/api/input", {"gpio": [{"input": 1, "type": "Contact Closure", "value": True},
                                          {"input": 2, "type": "Analog", "value": 55}]},
        {"inputs": {"1": {"type": "Contact Closure", "active": True}, "2": {"type": "Analog", "level": 55}}})
ph_poll("system", "/api/system", {
    "hardware_type": "LPC", "channel_capacity": 512, "serial_number": "006321", "memory_total": "12790Kb",
    "memory_used": "24056Kb", "memory_available": "103884Kb", "lua_memory_used": "40Kb", "lua_memory_allowed": "8912Kb",
    "storage_size": "1914MB", "bootloader_version": "0.9.0", "firmware_version": "2.8.0", "reset_reason": "Software Reset",
    "last_boot_time": "01 Jan 2017 09:09:38", "ip_address": "192.168.1.3", "subnet_mask": "255.255.255.0",
    "broadcast_address": "192.168.1.255", "default_gateway": "192.168.1.3", "host_name": "lpc-006321",
    "domain_name": "lighting.lan", "dns_servers": ["1.0.0.1", "1.1.1.1"], "vlan_tag": 100},
    {"system": {"hardware_type": "LPC", "channel_capacity": 512, "serial_number": "006321", "memory_total": "12790Kb",
                "memory_used": "24056Kb", "memory_available": "103884Kb", "lua_memory_used": "40Kb",
                "lua_memory_allowed": "8912Kb", "storage_size": "1914MB", "bootloader_version": "0.9.0",
                "firmware_version": "2.8.0", "reset_reason": "Software Reset", "last_boot_time": "01 Jan 2017 09:09:38",
                "ip_address": "192.168.1.3", "subnet_mask": "255.255.255.0", "default_gateway": "192.168.1.3",
                "host_name": "lpc-006321", "domain_name": "lighting.lan", "vlan_tag": 100}})
ph_poll("temperature", "/api/temperature", {"sys_temp": 40.2, "core1_temp": 44, "cc_temp": 44.1, "gpu_temp": 38.2},
        {"temperature": {"sys_temp": 40.2, "core1_temp": 44.0, "cc_temp": 44.1, "gpu_temp": 38.2}})
ph_poll("fans", "/api/fan_speed", {"fan_speed": {"fan_1": 2368, "fan_2": 3605}}, {"fans": {"fan_1": 2368, "fan_2": 3605}})
ph_poll("time", "/api/time", {"datetime": "01 Feb 2017 13:44:42", "local_time": 1485956682, "uptime": 493347},
        {"time": {"datetime": "01 Feb 2017 13:44:42", "local_time": 1485956682, "uptime": 493347}})
ph_poll("project", "/api/project", {"name": "Help Project", "author": "Contoso", "filename": "help_project_v1.pd2",
                                    "unique_id": "{6b48627a-1d5e-4b2f-81e2-481e092a6a79}", "upload_date": "2017-01-30T15:19:08"},
        {"project": {"name": "Help Project", "author": "Contoso", "filename": "help_project_v1.pd2",
                     "unique_id": "{6b48627a-1d5e-4b2f-81e2-481e092a6a79}", "upload_date": "2017-01-30T15:19:08"}})
ph_poll("replication", "/api/replication", {"name": "Help Project", "unique_id": "{6b48627a}"},
        {"replication": {"name": "Help Project", "unique_id": "{6b48627a}"}})
ph_poll("controllers", "/api/controller", {"controllers": [
    {"num": 1, "type": "LPC", "name": "Controller 1", "serial": "009060", "ip_address": "192.168.1.3", "online": True,
     "is_network_primary": True}]},
    {"controllers": {"1": {"name": "Controller 1", "type": "LPC", "serial": "009060", "ip_address": "192.168.1.3",
                           "online": True, "is_network_primary": True}}})
ph_poll("remote-devices", "/api/remote_device", {"remote_devices": [
    {"name": "Ballroom", "name_with_type": "Ballroom (RIO 44)", "num": 1, "type": "RIO 44", "online": True}]},
    {"remote_devices": {"1": {"name": "Ballroom", "type": "RIO 44", "online": True}}})
ph_poll("protocols", "/api/protocol", {"outputs": [{"type": 1, "name": "DMX", "disabled": False,
                                                    "universes": [{"key": {"index": 1}, "name": "1"}]}]},
        {"protocols": {"1": {"name": "DMX", "disabled": False}}})
ph_poll("text-slots", "/api/text_slot", {"text_slots": [{"name": "text", "value": "example"}]},
        {"text_slots": {"text": {"value": "example"}}})
ph_poll("playback-groups", "/api/playback_group", {"playback_groups": [{"num": 1, "name": "Back of House"}]},
        {"playback_groups": {"1": {"name": "Back of House"}}})
ph_poll("triggers", "/api/trigger", {"triggers": [
    {"type": "Startup", "num": 1, "name": "Initialise", "group": "#e18383", "description": "", "trigger_text": "At startup",
     "conditions": [], "actions": [{"text": "Start Timeline 1"}]}]},
    {"triggers": {"1": {"name": "Initialise", "type": "Startup", "description": "", "trigger_text": "At startup"}}})
ph_poll("cloud", "/api/cloud", {"connected": False, "connecting": True}, {"cloud": {"connected": False, "connecting": True}})
ph_poll("config", "/api/config", {
    "ip": "192.168.1.3", "subnet_mask": "255.255.255.0", "gateway": "192.168.1.1", "dhcp_enabled": True,
    "name_server_1": "192.168.1.1", "name_server_2": "8.8.8.8", "http_port": 80, "https_port": 443,
    "datetime": "2024-06-27T09:30", "watchdog_enabled": True, "log_level": 3, "syslog_enabled": False,
    "syslog_ip": "192.168.1.2", "ntp_enabled": True, "ntp_ip": "192.168.1.1"},
    {"config": {"ip": "192.168.1.3", "subnet_mask": "255.255.255.0", "gateway": "192.168.1.1", "dhcp_enabled": True,
                "name_server_1": "192.168.1.1", "name_server_2": "8.8.8.8", "http_port": 80, "https_port": 443,
                "datetime": "2024-06-27T09:30", "watchdog_enabled": True, "log_level": 3, "syslog_enabled": False,
                "syslog_ip": "192.168.1.2", "ntp_enabled": True, "ntp_ip": "192.168.1.1"}})
ph_poll("status-monitor", "/api/status_monitor", {"latest_refresh_all": {
    "completed_at": "2024-06-27T09:30", "discovered_device_count": 50, "unpatched_device_count": 3}},
    {"status_monitor": {"completed_at": "2024-06-27T09:30", "discovered_device_count": 50, "unpatched_device_count": 3}})


def ph_ws(name, message, state, **extra):
    telemetry(PH, name, inbound_ws=json.dumps(message), expect_state=state, **extra)


# The reference's change messages.
ph_ws("ws-scene", {"broadcast": "scene", "data": {"num": 1, "onstage": True, "state": "started"}},
      {"scenes": {"1": {"state": "started", "onstage": True}}})
ph_ws("ws-timeline", {"broadcast": "timeline", "data": {"num": 1, "onstage": True, "state": "running", "position": 5000}},
      {"timelines": {"1": {"state": "running", "onstage": True, "position": 5000}}})
ph_ws("ws-group", {"broadcast": "group", "data": {"level": 67, "name": "Group 1", "num": 1}},
      {"groups": {"1": {"name": "Group 1", "level": 67}}})
ph_ws("ws-beacon", {"broadcast": "beacon", "data": {"on": False}}, {"beacon": False})
ph_ws("ws-content-target", {"request": "content_target", "id": 5, "data": {"content_targets": [{"name": "Primary", "level": 100}]}},
      {"content_targets": {"Primary": {"level": 100}}})
ph_ws("ws-remote-device", {"data": {"remote_devices": [
    {"name": "EDN 20 1", "name_with_type": "EDN 20 1", "num": 1, "online": False, "serial": [], "type": "EDN 20"}]}},
    {"remote_devices": {"1": {"name": "EDN 20 1", "type": "EDN 20", "online": False}}})
ph_ws("ws-lua", {"broadcast": "lua", "data": {"slider_level": "56"}}, {"lua": {"slider_level": "56"}})

# The WebSocket's subscriptions on opening, without security (no token
# attribute; WebSocket API, Authentication and Keep Alive).
telemetry(PH, "ws-connect", inbound_ws='{"broadcast":"beacon","data":{"on":true}}',
          expect_connect_ws=['{"subscribe":"scene"}', '{"subscribe":"timeline"}', '{"subscribe":"group"}',
                             '{"subscribe":"beacon"}', '{"subscribe":"remote_device"}', '{"subscribe":"lua"}',
                             '{"subscribe":"content_target"}', '{}'],
          expect_state={"beacon": True})
