# BrightSign Local DWS (brightsign-dws): the LDWS endpoints under /api/v1, as
# the BrightSign developer documentation's endpoint pages write them, every
# answer wrapped in {"data": {"result": ...}}.
BS = "brightsign-dws"


def _r(result):
    return {"status": 200, "body": json.dumps({"data": {"result": result}})}


def _bs(command, input, method, target, result=None, body=None, model=None, file=None):
    result = {"success": True} if result is None else result
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
    v = {"spec": BS, "command": command, "input": input, "expect_request": req, "http_reply": _r(result),
         "expect_result": {"ok": {"kind": "value", "value": result}}}
    if model:
        v["model"] = model
    if file:
        v["file"] = file
    V.append(v)


INFO = {"serial": "RE433D006644", "upTime": "30 minutes", "upTimeSeconds": 1832, "model": "XD1035", "FWVersion": "9.0.97",
        "bootVersion": "9.0.85", "family": "cobra", "power": {"result": {"source": "AC"}}, "poe": {"result": {"status": "inactive"}},
        "networking": {"result": {"description": "Test_player", "name": "XD5-RE433D006644"}}, "connectionType": "eth0"}
_bs("get_info", {}, "GET", "/api/v1/info/", INFO)
_bs("get_health", {}, "GET", "/api/v1/health/", {"status": "active", "statusTime": "2026-10-27 10:00:00 "})
TIME = {"time": "2018-10-18 14:34:34 PDT", "timezone_name": "America/Los_Angeles", "year": 2018}
_bs("get_time", {}, "GET", "/api/v1/time/", TIME)
_bs("set_time", {"date": "2026-10-27", "time": "14:30"}, "PUT", "/api/v1/time/",
    body='{"date":"2026-10-27","time":"14:30","applyTimezone":false}')
VM = {"isAutoMode": False, "name": "1920x1080x60p", "width": "1920", "height": "1080", "frames": "60", "scan": "p",
      "mode": {"colorDepth": "8bit", "colorSpace": "rgb", "frequency": 60, "height": 1080, "width": 1920, "modeName": "1920x1080x60p"}}
_bs("get_video_mode", {}, "GET", "/api/v1/video-mode/", VM)
_bs("get_video_output", {"output": 1}, "GET", "/api/v1/video/hdmi/output/1/", {"bestMode": "1920x1080x60p"})
_bs("get_edid", {}, "GET", "/api/v1/video/hdmi/output/0/edid/", {"edid": "00ffffffffffff00"})
_bs("get_power_save", {}, "GET", "/api/v1/video/hdmi/output/0/power-save/", {"is_connected": True, "is_powered": True, "enabled": False})
_bs("set_power_save", {"enabled": True}, "PUT", "/api/v1/video/hdmi/output/0/power-save/", True, body='{"enabled":true}')
_bs("get_video_modes", {}, "GET", "/api/v1/video/hdmi/output/0/modes/", [{"modeName": "1920x1080x60p"}])
_bs("get_output_mode", {"which": "best"}, "GET", "/api/v1/video/hdmi/output/0/mode/?best=1", VM)
_bs("set_output_mode", {"mode": "3840x2160x30p"}, "PUT", "/api/v1/video/hdmi/output/0/mode/",
    {"success": True, "reboot": True}, body='{"modeName":"3840x2160x30p"}')
SNAP = {"filename": "sd/remote_snapshots/img-2021-12-29-10-49-27.jpg", "width": 1920, "height": 1080}
_bs("take_snapshot", {}, "POST", "/api/v1/snapshot/", SNAP, body='{"width":1920,"height":1080,"shouldCaptureFullResolution":false}')
_bs("list_files", {"path": "sd/My Show/autorun.brs"}, "GET", "/api/v1/files/sd/My%20Show/autorun.brs/",
    {"name": "sd/autorun.brs", "type": "file", "path": "sd/autorun.brs", "fileSize": 1144340, "streamable": True})
_bs("read_file", {"path": "sd/autorun.brs"}, "GET", "/api/v1/files/sd/autorun.brs/?contents",
    {"mimeType": None, "name": "autorun.brs", "contents": "TGlicmFyeSAi"})
_bs("rename_file", {"path": "sd/test.txt", "name": "newname.txt"}, "POST", "/api/v1/files/sd/test.txt/", body='{"name":"newname.txt"}')
_bs("create_folder", {"path": "sd/testfolder"}, "PUT", "/api/v1/files/sd/testfolder/")
_bs("delete_file", {"path": "sd/test.txt"}, "DELETE", "/api/v1/files/sd/test.txt/")
_bs("format_storage", {"device": "sd"}, "DELETE", "/api/v1/storage/sd/", body='{"fs":"exfat"}')
_bs("encrypt_storage", {"path": "/sd", "type": "unique-key"}, "PUT", "/api/v1/storage/encryption/",
    body='{"filePath":"/sd","settings":{"type":"unique-key","password":""},"format":true,"doProvision":false}')


# Upload: multipart/form-data (RFC 7578), one part named file with the file's
# name; the boundary is the core's, derived from the parts (SPEC.md §4).
def _bs_form(parts):
    import hashlib
    digest = hashlib.sha256(b"".join(n.encode() + b"\0" + d + b"\0" for n, d, _ in parts)).hexdigest()
    boundary = "----form-" + digest[:16]
    out = b""
    for name, data, filename in parts:
        out += f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"".encode()
        if filename:
            out += f"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream".encode()
        out += b"\r\n\r\n" + data + b"\r\n"
    out += f"--{boundary}--\r\n".encode()
    return out, f"multipart/form-data; boundary={boundary}"


_UP, _UPT = _bs_form([("file", b"\x00\x01\xfe\xff", "clip.mp4")])
V.append({"spec": BS, "command": "upload_file", "input": {"folder": "sd/media", "file": "AAH+/w==", "name": "clip.mp4"},
          "expect_request": {"method": "PUT", "target": "/api/v1/files/sd/media/", "body_hex": _UP.hex(), "content_type": _UPT},
          "http_reply": _r({"success": True, "results": ["clip.mp4"]}),
          "expect_result": {"ok": {"kind": "value", "value": {"success": True, "results": ["clip.mp4"]}}}})

_bs("get_registry", {}, "GET", "/api/v1/registry/", {"success": True, "value": {"networking": {}}})
_bs("get_registry_key", {"section": "html", "key": "use-brightsign-media-player"}, "GET",
    "/api/v1/registry/html/use-brightsign-media-player/", {"success": True, "value": "0"})
_bs("set_registry_key", {"section": "html", "key": "use-brightsign-media-player", "value": "0"}, "PUT",
    "/api/v1/registry/html/use-brightsign-media-player/", body='{"value":"0"}')
_bs("delete_registry_key", {"section": "html", "key": "use-brightsign-media-player"}, "DELETE",
    "/api/v1/registry/html/use-brightsign-media-player/")
_bs("delete_registry_section", {"section": "html"}, "DELETE", "/api/v1/registry/html/")
_bs("get_recovery_url", {}, "GET", "/api/v1/registry/recovery_url/", {"success": True, "value": "www.google.com"})
_bs("set_recovery_url", {"url": "www.google.com"}, "PUT", "/api/v1/registry/recovery_url/", body='{"url":"www.google.com"}')
_bs("flush_registry", {}, "PUT", "/api/v1/registry/flush/")
_bs("get_logs", {}, "GET", "/api/v1/logs/", "line 1\nline 2")
_bs("get_diagnostics", {}, "GET", "/api/v1/diagnostics/", {"ethernet": {"diagnosis": "present"}})
_bs("dns_lookup", {"address": "example.com"}, "GET", "/api/v1/diagnostics/dns-lookup/example.com/", {"records": {"ipv4": ["192.0.2.1"]}})
_bs("ping", {"address": "192.0.2.1"}, "GET", "/api/v1/diagnostics/ping/192.0.2.1/", {"success": True})
_bs("trace_route", {"address": "192.0.2.1", "resolve": True}, "GET", "/api/v1/diagnostics/trace-route/192.0.2.1/?resolveAddress=true",
    {"address": "192.0.2.1"})
_bs("get_network_neighborhood", {}, "GET", "/api/v1/diagnostics/network-neighborhood/", {"success": True, "response": []})
_bs("get_network_configuration", {}, "GET", "/api/v1/diagnostics/network-configuration/eth0/", {"text": "eth0"})
_bs("set_network_configuration", {"interface": "eth0", "config": {"dnsServerList": ["192.0.2.53"]}}, "PUT",
    "/api/v1/diagnostics/network-configuration/eth0/", body='{"dnsServerList":["192.0.2.53"]}')
_bs("get_network_interfaces", {}, "GET", "/api/v1/diagnostics/interfaces/", ["eth0"])
_bs("get_packet_capture", {}, "GET", "/api/v1/diagnostics/packet-capture/", {"statusCode": 200, "is_running": False})
_bs("start_packet_capture", {"duration": 60, "filter": "port 80"}, "POST", "/api/v1/diagnostics/packet-capture/",
    body='{"filename":"capture.pcap","net_interface":"eth0","duration":60,"maxpackets":0,"snaplen":0,"filter":"port 80"}')
_bs("stop_packet_capture", {}, "DELETE", "/api/v1/diagnostics/packet-capture/")
V.append({"spec": BS, "command": "get_telnet", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1/diagnostics/telnet/"},
          "http_reply": {"status": 200, "body": '{"enabled":true,"portNumber":23}'},
          "expect_result": {"ok": {"kind": "value", "value": {"enabled": True, "portNumber": 23}}}})
V.append({"spec": BS, "command": "set_telnet", "input": {"enabled": True},
          "expect_request": {"method": "PUT", "target": "/api/v1/diagnostics/telnet/", "body": '{"enabled":true,"portNumber":23,"reboot":true}'},
          "http_reply": {"status": 200, "body": '{"success":true,"reboot":true}'},
          "expect_result": {"ok": {"kind": "value", "value": {"success": True, "reboot": True}}}})
V.append({"spec": BS, "command": "get_ssh", "input": {}, "expect_request": {"method": "GET", "target": "/api/v1/diagnostics/ssh/"},
          "http_reply": {"status": 200, "body": '{"enabled":true,"portNumber":22,"password":true}'},
          "expect_result": {"ok": {"kind": "value", "value": {"enabled": True, "portNumber": 22, "password": True}}}})
V.append({"spec": BS, "command": "set_ssh", "input": {"enabled": True, "password": "s3cret", "reboot": False},
          "expect_request": {"method": "PUT", "target": "/api/v1/diagnostics/ssh/",
                             "body": '{"enabled":true,"portNumber":22,"password":"s3cret","reboot":false}'},
          "http_reply": {"status": 200, "body": '{"success":true,"reboot":false}'},
          "expect_result": {"ok": {"kind": "value", "value": {"success": True, "reboot": False}}}})
_bs("get_chromium_debug", {}, "GET", "/api/v1/diagnostics/chromium-debug/", {"enableWebInspector": False})
_bs("set_chromium_debug", {"enabled": True}, "PUT", "/api/v1/diagnostics/chromium-debug/", body='{"enableWebInspector":true}')
_bs("get_property_lock", {}, "GET", "/api/v1/advanced/property-lock/", {"settingsHandlerEnabled": True, "bsnce": False})
_bs("set_property_lock", {"force_registry": True, "registry_enable": False}, "PUT", "/api/v1/advanced/property-lock/",
    body='{"forceRegistrySettings":true,"registryEnableSettings":false}')
_bs("get_supervisor_logging", {}, "GET", "/api/v1/system/supervisor/logging/", {"level": 2, "name": "info"})
_bs("set_supervisor_logging", {"level": 3}, "PUT", "/api/v1/system/supervisor/logging/", body='{"level":3}')
V.append({"spec": BS, "command": "send_cec", "input": {"hex": "4004"},
          "expect_request": {"method": "POST", "target": "/api/v1/sendCecX/", "body": '{"hexCommand":"4004"}'},
          "http_reply": {"status": 200, "body": "{}"}, "expect_result": {"ok": {"kind": "ack"}}})
RB = {"success": True, "message": "A reboot has been initiated"}
_bs("reboot", {}, "PUT", "/api/v1/control/reboot/", RB)
_bs("reboot_with_crash_report", {}, "PUT", "/api/v1/control/reboot/", RB, body='{"crash_report":true}')
_bs("reboot_disable_autorun", {}, "PUT", "/api/v1/control/reboot/", RB, body='{"autorun":"disable"}')
_bs("factory_reset", {}, "PUT", "/api/v1/control/reboot/", RB, body='{"factory_reset":true}')
_bs("get_dws_password", {}, "GET", "/api/v1/control/dws-password/", {"success": True, "password": {"isResultValid": True, "isBlank": True}})
_bs("set_dws_password", {"password": "newpassword", "previous": "oldpassword"}, "PUT", "/api/v1/control/dws-password/",
    {"success": True, "reboot": False}, body='{"password":"newpassword","previous_password":"oldpassword"}')
_bs("get_local_dws", {}, "GET", "/api/v1/control/local-dws/", {"success": True, "value": True})
_bs("set_local_dws", {"enabled": False}, "PUT", "/api/v1/control/local-dws/", {"success": True, "reboot": True}, body='{"enable":false}')
_bs("reset_ssh_host_keys", {}, "PUT", "/api/v1/control/ssh-host-keys/reset/")
_bs("reset_dws_certificates", {}, "PUT", "/api/v1/control/dws-default-certs/reset/")
_bs("update_firmware", {"url": "https://example.com/fw/xd5-9.0.97.bsfw"}, "GET",
    "/api/v1/download-firmware/?url=https%3A%2F%2Fexample.com%2Ffw%2Fxd5-9.0.97.bsfw", {"success": True, "reboot": True})
DC = {"whiteBalance": {"redBalance": 120, "greenBalance": 121, "blueBalance": 122}, "volume": 50, "brightness": 60,
      "contrast": 45, "idleStandbyTimeout": 600, "powerSetting": "on", "videoOutput": "HDMI1", "sdConnection": "brightsign",
      "alwaysConnectedEnabled": True}
_bs("get_display_settings", {}, "GET", "/api/v1/display-control/", DC, model="display")
_bs("get_display_info", {}, "GET", "/api/v1/display-control/info/", {"serialNo": "1234567890", "hwRevision": 2}, model="display")
for cmd, seg, key, val in [("set_display_brightness", "brightness", "brightness", 100), ("set_display_contrast", "contrast", "contrast", 45),
                           ("set_display_volume", "volume", "volume", 30), ("set_display_standby_timeout", "standby-timeout", "seconds", 60)]:
    _bs(cmd, {key if key != "seconds" else "seconds": val}, "PUT", f"/api/v1/display-control/{seg}/",
        {"success": True, key: val}, body=json.dumps({key: val}, separators=(",", ":")), model="display")
_bs("set_display_always_connected", {"enabled": True}, "PUT", "/api/v1/display-control/always-connected/",
    {"success": True, "enable": True}, body='{"enable":true}', model="display")
_bs("set_display_always_on", {"enabled": False}, "PUT", "/api/v1/display-control/always-on/",
    {"success": True, "enable": False}, body='{"enable":false}', model="display")
_bs("set_display_power", {"setting": "standby"}, "PUT", "/api/v1/display-control/power-settings/",
    {"success": True, "setting": "standby"}, body='{"setting":"standby"}', model="display")
_bs("set_display_sd_connection", {"connection": "display"}, "PUT", "/api/v1/display-control/sd-connection/",
    {"success": True, "connection": "display"}, body='{"connection":"display"}', model="display")
_bs("set_display_video_output", {"output": "HDMI2"}, "PUT", "/api/v1/display-control/video-output/",
    {"success": True, "output": "HDMI2"}, body='{"output":"HDMI2"}', model="display")
_bs("set_display_white_balance", {"red": 120, "green": 120, "blue": 120}, "PUT", "/api/v1/display-control/white-balance/",
    {"success": True, "redBalance": 120, "greenBalance": 120, "blueBalance": 120},
    body='{"redBalance":120,"greenBalance":120,"blueBalance":120}', model="display")
_bs("update_display_firmware_url", {"url": "https://example.com/tv-firmware.zip"}, "PUT", "/api/v1/display-control/firmware/",
    {"success": True, "reboot": True}, body='{"url":"https://example.com/tv-firmware.zip"}', model="display")
_bs("update_display_firmware_file", {"path": "path/to/firmware.zip"}, "PUT", "/api/v1/display-control/firmware/",
    {"success": True, "reboot": True}, body='{"filepath":"path/to/firmware.zip"}', model="display")
# A refused credential: digest authentication fails.
V.append({"spec": BS, "command": "get_info", "file": "get_info.unauthorized", "input": {},
          "expect_request": {"method": "GET", "target": "/api/v1/info/"},
          "http_reply": {"status": 401, "body": ""}, "expect_result": {"error": {"error": "auth"}}})


def _bt(name, path, result, state, model=None):
    t = {"inbound_http": {"path": path, "body": json.dumps({"data": {"result": result}})}, "expect_state": state}
    if model:
        t["model"] = model
    telemetry(BS, name, **t)


_bt("info", "/api/v1/info/", INFO, {"info": {
    "serial": "RE433D006644", "model": "XD1035", "family": "cobra", "firmware": "9.0.97", "boot_version": "9.0.85",
    "uptime_s": 1832, "connection_type": "eth0", "name": "XD5-RE433D006644", "description": "Test_player",
    "power_source": "AC", "poe_status": "inactive"}})
_bt("health", "/api/v1/health/", {"status": "active", "statusTime": "2026-10-27 10:00:00 "},
    {"health": {"status": "active", "time": "2026-10-27 10:00:00 "}})
_bt("time", "/api/v1/time/", TIME, {"time": {"now": "2018-10-18 14:34:34 PDT", "timezone": "America/Los_Angeles"}})
_bt("video-mode", "/api/v1/video-mode/", VM, {"video": {"mode": "1920x1080x60p", "width": 1920, "height": 1080, "frame_rate": 60,
                                                        "auto": False, "color_space": "rgb", "color_depth": "8bit"}})
_bt("power-save", "/api/v1/video/hdmi/output/0/power-save/", {"is_connected": True, "is_powered": False, "enabled": True},
    {"outputs": {"0": {"connected": True, "powered": False, "power_save": True}}})
_bt("local-dws", "/api/v1/control/local-dws/", {"success": True, "value": True}, {"access": {"local_dws": True}})
_bt("telnet", "/api/v1/diagnostics/telnet/", {"enabled": True, "portNumber": 23}, {"access": {"telnet": True, "telnet_port": 23}})
telemetry(BS, "telnet-bare", inbound_http={"path": "/api/v1/diagnostics/telnet/", "body": '{"enabled":false,"portNumber":23}'},
          expect_state={"access": {"telnet": False, "telnet_port": 23}})
telemetry(BS, "ssh-bare", inbound_http={"path": "/api/v1/diagnostics/ssh/", "body": '{"enabled":true,"portNumber":22,"password":true}'},
          expect_state={"access": {"ssh": True, "ssh_port": 22}})
_bt("ssh", "/api/v1/diagnostics/ssh/", {"enabled": True, "portNumber": 2222}, {"access": {"ssh": True, "ssh_port": 2222}})
_bt("chromium-debug", "/api/v1/diagnostics/chromium-debug/", {"enableWebInspector": True}, {"access": {"web_inspector": True}})
_bt("display-control", "/api/v1/display-control/", DC, {"display": {
    "brightness": 60, "contrast": 45, "volume": 50, "power": "on", "standby_timeout_s": 600, "video_output": "HDMI1",
    "sd_connection": "brightsign", "always_connected": True, "white_balance": {"red": 120, "green": 121, "blue": 122}}}, model="display")
for name, seg, result, state in [
        ("display-brightness", "brightness", {"success": True, "brightness": 80}, {"brightness": 80}),
        ("display-contrast", "contrast", {"contrast": 40}, {"contrast": 40}),
        ("display-volume", "volume", {"volume": 20}, {"volume": 20}),
        ("display-power", "power-settings", {"setting": "standby"}, {"power": "standby"}),
        ("display-standby", "standby-timeout", {"seconds": 60}, {"standby_timeout_s": 60}),
        ("display-video-output", "video-output", {"output": "HDMI2"}, {"video_output": "HDMI2"}),
        ("display-sd", "sd-connection", {"connection": "display"}, {"sd_connection": "display"}),
        ("display-white-balance", "white-balance", {"redBalance": 1, "greenBalance": 2, "blueBalance": 3},
         {"white_balance": {"red": 1, "green": 2, "blue": 3}})]:
    _bt(name, f"/api/v1/display-control/{seg}/", result, {"display": state}, model="display")
