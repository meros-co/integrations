# Kiloview P3 / P3 Mini (kiloview-p3): one vector per command over the web
# API of the "P3/P3 Mini WEB API Manual en v1.03" (firmware 1.22.0010). Each
# request is stated from the document's request address, method and example
# body; JSON bodies are written with json.dumps in the document's key order,
# query values and path segments percent-encoded per RFC 3986. Replies use the
# document's examples. Paths keep the document's typos (systemctr, ststemctrl,
# networkamanager), as the spec does.
from urllib.parse import quote as _kp3_quote

KP3 = "kiloview-p3"


def _kp3_json(obj):
    return json.dumps(obj, separators=(",", ":"))


def _kp3(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body if isinstance(body, str) else _kp3_json(body)
    V.append({"spec": KP3, "command": command, "input": input, "expect_request": request, **extra})


def _kp3_q(value):
    return _kp3_quote(str(value), safe="")


_KP3_OK_BODY = {"result": "ok", "msg": None}
_KP3_OK = {"http_reply": {"status": 200, "body": _kp3_json(_KP3_OK_BODY)},
           "expect_result": {"ok": {"kind": "ack"}}}


def _kp3_value(body, value):
    return {"http_reply": {"status": 200, "body": _kp3_json(body)},
            "expect_result": {"ok": {"kind": "value", "value": value}}}


# A reply whose result is error fails the command.
_KP3_ERR = {"http_reply": {"status": 200, "body": _kp3_json({"result": "error", "msg": "failed"})},
            "expect_result": {"error": {"error": "device_error"}}}


def _kp3_form(parts):
    """multipart/form-data as the core writes it (SPEC.md §4): the boundary
    from a SHA-256 of the parts' names and data."""
    import hashlib
    digest = hashlib.sha256(b"".join(n.encode() + b"\0" + d + b"\0" for n, d, _ in parts)).hexdigest()
    boundary = "----form-" + digest[:16]
    body = b""
    for name, data, filename in parts:
        body += f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"".encode()
        if filename:
            body += f"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream".encode()
        body += b"\r\n\r\n" + data + b"\r\n"
    body += f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


def _kp3_upload(command, input, target, parts, **extra):
    body, ctype = _kp3_form(parts)
    V.append({"spec": KP3, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": target, "body_hex": body.hex(), "content_type": ctype},
              **extra})


_KP3_FILE = "AAH+/w=="          # four made-up bytes
_KP3_BYTES = b"\x00\x01\xfe\xff"

# ── 1.1 Users and authentication ──
_TOKEN = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.e30.sig"
_kp3("login", {"username": "admin", "password": "Admin123"}, "POST", "/api/systemctrl/user/login",
     {"username": "admin", "password": "Admin123"},
     **_kp3_value({"result": "ok", "data": {"token": _TOKEN, "alias": "Admin", "changed": True}, "msg": None}, _TOKEN))
_kp3("check_session", {}, "GET", "/api/systemctrl/user/session", **_KP3_OK)
_kp3("get_api_authentication", {}, "GET", "/api/systemctrl/user/auth_info",
     **_kp3_value({"result": "ok", "data": {"enable": False}}, {"enable": False}))
_kp3("set_api_authentication", {"enabled": True}, "POST", "/api/systemctrl/user/auth_info", {"enable": True}, **_KP3_OK)
_kp3("get_users", {}, "GET", "/api/systemctrl/user/list")
_kp3("add_user", {"id": "lisi", "alias": "lisi", "password": "Admin123"}, "POST", "/api/systemctr/user/add",
     {"id": "lisi", "alias": "lisi", "password": "Admin123", "confirmNewPassword": "Admin123", "create_time": "",
      "role": "", "enable_web": True, "enable_api": True}, **_KP3_OK)
_kp3("remove_user", {"id": "lisi"}, "POST", "/api/systemctrl/user/remove", {"ids": ["lisi"]}, **_KP3_OK)
_kp3("modify_user", {"id": "zhangsan", "alias": "xiaosan", "enable_api": False}, "POST", "/api/systemctrl/user/modify",
     {"id": "zhangsan", "alias": "xiaosan", "role": "", "enable_web": True, "enable_api": False}, **_KP3_ERR)

# ── 1.2 Time ──
_kp3("set_timezone", {"timezone": "Asia/Shanghai", "offset": 8}, "POST", "/api/systemctrl/systime/setLocation",
     {"timezone": "Asia/Shanghai", "offset": 8}, **_KP3_OK)
_kp3("set_time", {"time": "2025-06-26 14:31:01", "mode": "pc", "offset": 8}, "POST", "/api/systemctrl/systime/setTime",
     {"time": "2025-06-26 14:31:01", "timetype": "pc", "offset": 8}, **_KP3_OK)
_kp3("set_time_ntp", {"servers": "time.windows.com pool.ntp.org", "offset": -5}, "POST", "/api/systemctrl/systime/setTime",
     {"timetype": "ntp", "offset": -5, "ntp": "time.windows.com pool.ntp.org"}, **_KP3_OK)
_kp3("get_time", {}, "GET", "/api/systemctrl/systime/get")
_kp3("get_ntp_status", {}, "GET", "/api/systemctrl/systime/ntpstatus")

# ── 1.3 System ──
_kp3("get_components", {}, "GET", "/api/systemctrl/system/components")
_kp3("get_hostname", {}, "GET", "/api/systemctrl/system/getHostname",
     **_kp3_value({"result": "ok", "msg": None, "data": {"hostname": "P3-20240806P3001"}}, "P3-20240806P3001"))
_kp3("set_hostname", {"name": "P3123"}, "GET", "/api/systemctrl/system/setHostname?name=P3123", **_KP3_OK)
_kp3("get_web_protocols", {}, "GET", "/api/systemctrl/system/getProtocol")
_kp3("set_web_protocols", {"http": True, "https": True}, "POST", "/api/systemctrl/system/setProtocol",
     {"http": True, "https": True, "httpPort": 80, "httpsPort": 443}, **_KP3_OK)
_kp3("get_ssl_certificate", {}, "GET", "/api/systemctrl/system/ssl")
_kp3("apply_ssl_certificate", {}, "POST", "/api/systemctrl/system/ssl")
_kp3("reset_ssl_certificate", {}, "DELETE", "/api/systemctrl/system/ssl")
_kp3_upload("upload_ssl_file", {"file_type": "key", "file": _KP3_FILE, "filename": "server.key"},
            "/api/systemctrl/system/ssl/key", [("file", _KP3_BYTES, "server.key")])
_kp3("factory_reset", {}, "GET", "/api/systemctrl/system/reFactory", **_KP3_OK)
_kp3("restart_codec", {}, "GET", "/api/systemctrl/system/restore", **_KP3_OK)
_kp3("reboot", {}, "GET", "/api/systemctrl/system/reboot", **_KP3_OK)
_kp3("ping", {"target": "192.168.35.215"}, "POST", "/api/systemctrl/system/ping", {"target_addr": "192.168.35.215"},
     **_kp3_value({"result": "ok", "data": {"loss_rate": 0, "reachable": True}, "msg": None},
                  {"loss_rate": 0, "reachable": True}))
_kp3("get_nic_speed", {}, "GET", "/api/systemctrl/system/getNetworkSpeed?ifname=eth0")
_kp3("get_system_info", {}, "GET",
     "/api/systemctrl/system/getSystemInfo?version=true&cpu=true&memory=true&disk=true&persisTime=true&imei=true")
_kp3_upload("upload_firmware", {"file": _KP3_FILE, "filename": "P3-1.22.0010.bin"}, "/api/systemctrl/system/upload",
            [("file", _KP3_BYTES, "P3-1.22.0010.bin")], **_KP3_OK)
_FW_ID = "firmware-P3-kiloview-1.21.0009.0002-20250626"
_kp3_upload("upload_firmware_chunk", {"identifier": _FW_ID, "number": 27, "file": _KP3_FILE},
            "/api/systemctrl/system/uploadfile",
            [("identifier", _FW_ID.encode(), None), ("file", _KP3_BYTES, _FW_ID), ("number", b"27", None),
             ("file_type", b"bin", None)])
_kp3("merge_firmware", {"identifier": _FW_ID, "count": 235}, "POST", "/api/systemctrl/system/mergefile",
     {"identifier": _FW_ID, "number": 235, "file_type": "bin"}, **_KP3_OK)
_kp3("set_production_test", {"enabled": False}, "POST", "/api/systemctrl/system/production_off", {"value": False})
_kp3("get_production_test", {}, "GET", "/api/ststemctrl/system/production_stat",
     **_kp3_value({"result": "ok", "data": {"value": True}}, True))

# ── 1.4 Logs ──
_kp3("get_log", {"service": "Kilolink"}, "GET", "/api/systemctrl/system_log/get/Kilolink",
     **_kp3_value({"result": "ok", "data": "..."}, "..."))

# ── 2 Recording ──
_REC = "/run/media/sdb1/RECORD/MAIN20250521_162924_517475/REC-MAIN-202505211629245174751.mp4"
_kp3("delete_recording_file", {"path": _REC}, "POST", "/api/record/record/remove_file", {"path": _REC}, **_KP3_OK)
_kp3("get_recording_files", {"device": "/run/media/sdb1"}, "GET",
     "/api/record/record/get_disk_files?device=%2Frun%2Fmedia%2Fsdb1")
_kp3("choose_usb_drive", {"id": "sdb1", "chosen": False}, "POST", "/api/record/record/choose_usb",
     {"id": "sdb1", "choose": False}, **_KP3_OK)
_kp3("get_storage", {}, "GET", "/api/record/record/disk_list")
_kp3("get_recording_settings", {}, "GET", "/api/record/record/get_record_info")
_kp3("set_recording_settings", {"format": "mp4", "limit_type": "time_loop"}, "POST", "/api/record/record/settings",
     {"format": "mp4", "file_prefix": "REC", "limit_type": "time_loop", "limit_size": 51200, "limit_time": 60,
      "disk_policy": "noSpace", "follow_timecode_start": False}, **_KP3_OK)
_kp3("start_recording", {}, "GET", "/api/record/v1/get_recording_status", {"start": True}, **_KP3_OK)
_kp3("stop_recording", {}, "GET", "/api/record/v1/get_recording_status", {"start": False}, **_KP3_OK)
_kp3("get_recording_status", {}, "GET", "/api/record/v1/get_recording_status",
     **_kp3_value({"result": "ok", "data": {"status": False}}, False))

# ── 3.1 Streams ──
_SID = "11213c00ab3bf668"
_kp3("get_streams", {}, "GET", "/api/streamer/v1/stream/list")
_SRT_IN = {"name": "SRT1", "connection_mode": "Listener", "listener_port": 1025}
_SRT_BODY = {"type": "srt", "name": "SRT1", "bindAudio": 1, "bindNetwork": "auto", "connectionMode": "Listener",
             "address": "", "listenerPort": 1025, "latency": 125, "bandwidth": 25, "payloadSize": 1316,
             "encryption": "0", "passphrase": "", "srt_stream_id": "", "ts_null_multiple": 0, "ts_pcr_period": 20,
             "ts_pts_pcr_delay": 200, "ts_transport_stream_id": 101}
_kp3("add_srt_stream", _SRT_IN, "POST", "/api/streamer/v1/stream", _SRT_BODY, **_KP3_OK)
_kp3("modify_srt_stream", {**_SRT_IN, "id": _SID, "bind_network": "modem1"}, "PUT", "/api/streamer/v1/stream/" + _SID,
     {"id": _SID, **{**_SRT_BODY, "bindNetwork": "modem1"}}, **_KP3_OK)
_RTMP_IN = {"name": "YouTube", "address": "rtmp://a.rtmp.youtube.com/live2/key", "bind_audio": 2}
_RTMP_BODY = {"type": "rtmp", "name": "YouTube", "bindAudio": 2, "bindNetwork": "auto",
              "address": "rtmp://a.rtmp.youtube.com/live2/key", "user": "", "password": "", "connIntv": 5,
              "connTimeout": 10}
_kp3("add_rtmp_stream", _RTMP_IN, "POST", "/api/streamer/v1/stream", _RTMP_BODY, **_KP3_OK)
_kp3("modify_rtmp_stream", {**_RTMP_IN, "id": _SID}, "PUT", "/api/streamer/v1/stream/" + _SID,
     {"id": _SID, **_RTMP_BODY}, **_KP3_OK)
_NDI_BODY = {"group": "", "channel_name": "", "connection": "disable_rudp", "netprefix": "", "netmask": "", "ttl": 1,
             "discovery_server": "", "bindNetwork": "auto", "name": "NDI", "type": "ndi_hx", "bindAudio": 1}
_kp3("add_ndi_hx_stream", {"name": "NDI"}, "POST", "/api/streamer/v1/stream", _NDI_BODY, **_KP3_OK)
_kp3("modify_ndi_hx_stream", {"name": "NDI", "id": _SID}, "PUT", "/api/streamer/v1/stream/" + _SID,
     {"id": _SID, **_NDI_BODY}, **_KP3_OK)
_RTP_ID = "542b4dcb8d15d59b"
_kp3("add_rtp_stream", {"name": "RTP", "address": "192.168.35.215", "port": 1026}, "POST", "/api/streamer/v1/stream",
     {"address": "192.168.35.215", "port": 1026, "load_type": "ts", "ts_service_name": "Encoder",
      "ts_service_provider": "Encoder device", "bindNetwork": "auto", "name": "RTP", "type": "rtp", "bindAudio": 1},
     **_KP3_OK)
# The document's own Modify Stream example (3.1.3).
_kp3("modify_rtp_stream", {"name": "RTP", "address": "192.168.35.215", "port": 1026, "id": _RTP_ID}, "PUT",
     "/api/streamer/v1/stream/" + _RTP_ID,
     {"address": "192.168.35.215", "port": 1026, "load_type": "ts", "ts_service_name": "Encoder",
      "ts_service_provider": "Encoder device", "id": _RTP_ID, "bindNetwork": "auto", "name": "RTP", "type": "rtp",
      "bindAudio": 1}, **_KP3_OK)
_HLS = {"type": "hls", "name": "HLS", "bindAudio": 1, "bindNetwork": "auto", "mode": "server", "segmentTime": 2,
        "maxSegments": 5, "session": "live", "media_playlist_url": ""}
_kp3("add_stream", {"config": _HLS}, "POST", "/api/streamer/v1/stream", _HLS, **_KP3_OK)
_kp3("modify_stream", {"id": _SID, "config": {"id": _SID, **_HLS}}, "PUT", "/api/streamer/v1/stream/" + _SID,
     {"id": _SID, **_HLS}, **_KP3_OK)
_kp3("delete_stream", {"id": _RTP_ID}, "DELETE", "/api/streamer/v1/stream/" + _RTP_ID, **_KP3_OK)
_kp3("set_stream_enabled", {"id": "17ac289ffbd0062b", "enabled": True}, "POST", "/api/streamer/v1/stream/enable",
     {"id": "17ac289ffbd0062b", "enable": True}, **_KP3_OK)
_kp3("reset_ndi", {}, "POST", "/api/streamer/v1/stream/ndi/reset", **_KP3_OK)
_kp3("get_webrtc", {}, "GET", "/api/streamer/v1/stream/get_webrtc?mode=0")
_kp3("set_webrtc", {"mode": "1", "stream_id": "25a61cd4248f6b46"}, "GET",
     "/api/streamer/v1/stream/set_webrtc?mode=1&stream_id=25a61cd4248f6b46")
_kp3("get_media_users", {}, "GET", "/api/streamer/v1/stream/media_user",
     **_kp3_value({"result": "ok", "user_list": ["admin"]}, ["admin"]))
_kp3("add_media_user", {"username": "zhangyu", "password": "Admin123"}, "POST", "/api/streamer/v1/stream/media_user",
     {"oper": "add", "username": "zhangyu", "password": "Admin123"}, **_KP3_OK)
_kp3("delete_media_user", {"username": "zhangyu"}, "DELETE", "/api/streamer/v1/stream/media_user/zhangyu", **_KP3_OK)

# ── 3.2 PTZ ──
_kp3("ptz_move", {"direction": "right", "speed": 50}, "GET", "/api/streamer/ptz/control?action=right&speed=50", **_KP3_OK)
_kp3("get_ptz", {}, "GET", "/api/streamer/ptz/info")
_kp3("get_ptz_protocols", {}, "GET", "/api/streamer/ptz/protocol",
     **_kp3_value({"result": "ok", "msg": None, "data": ["Sony Visca", "Pelco-d", "Pelco-p"]},
                  ["Sony Visca", "Pelco-d", "Pelco-p"]))
_kp3("get_ptz_serial_devices", {}, "GET", "/api/streamer/ptz/serial_device")
_kp3("set_ptz_network", {"addr": "192.168.43.225"}, "POST", "/api/streamer/ptz/modify",
     {"enable": 1, "typ": "network", "ptz_addr": 1, "ptz_protocol": "Sony Visca", "addr": "192.168.43.225",
      "port": 52381, "protocol": "TCP"}, **_KP3_OK)
# The document's example (3.2.5).
_kp3("set_ptz_serial", {"device": "none"}, "POST", "/api/streamer/ptz/modify",
     {"enable": 1, "typ": "serial", "ptz_addr": 1, "ptz_protocol": "Sony Visca", "rtscts": 0, "startBits": 8,
      "endBits": 1, "xonxoff": 0, "device": "none", "parity": "none", "baudrate": 9600}, **_KP3_OK)
_kp3("set_ptz_usb", {"enabled": False, "control_type": "USB"}, "POST", "/api/streamer/ptz/modify",
     {"enable": 0, "typ": "usb", "ptz_addr": 1, "ptz_protocol": "v4l2-ctl", "control_type": "USB"}, **_KP3_OK)

# ── 4.1 Network ──
_kp3("get_interfaces", {}, "GET", "/api/networkmanager/network/GetLinkinfo")
_kp3("get_link_list", {}, "GET", "/api/networkmanager/network/GetLinkList")
_kp3("set_interface", {"method": "dhcp"}, "POST", "/api/networkmanager/network/ChangeLinkinfo",
     {"ifname": "eth0", "cfg": {"enable": True, "method": "dhcp", "address": "", "netmask": "", "gw": "", "mac": "",
                                "dns": ""}}, **_KP3_OK)
_kp3("set_interface_enabled", {"enabled": True}, "POST", "/api/networkmanager/network/SetSwitch",
     {"ifname": "eth0", "enable": True}, **_KP3_OK)
_kp3("get_ethernet", {}, "GET", "/api/networkmanager/network/GetEthernets")
# The document's example (4.1.6).
_kp3("set_ethernet", {"method": "static", "address": "192.168.35.178", "netmask": "255.255.255.0",
                      "gateway": "192.168.35.254", "dns": "8.8.8.8;8.8.4.4", "mac": "4A:53:F6:E8:CF:12"},
     "POST", "/api/networkmanager/network/SetEthernets",
     {"ifname": "eth0", "method": "static", "address": "192.168.35.178", "netmask": "255.255.255.0",
      "gw": "192.168.35.254", "dns": "8.8.8.8;8.8.4.4", "mac": "4A:53:F6:E8:CF:12"}, **_KP3_OK)
_kp3("reset_mdns", {}, "GET", "/api/networkmanager/network/ResetMDNS", **_KP3_OK)
_kp3("get_link_rates", {}, "GET", "/api/networkmanager/network/getNetworkSpeed")
_kp3("get_link_data", {}, "GET", "/api/networkmanager/network/getNetworkInterfaceAllData")
_kp3("get_interface_map", {}, "GET", "/api/networkmanager")
_kp3("get_traffic", {}, "GET", "/api/networkmanager/network/traffic_data")
_kp3("clear_traffic", {"key": "wlan0"}, "POST", "/api/networkmanager/network/traffic_data", {"traffic_keys": ["wlan0"]},
     **_KP3_OK)
_kp3("get_current_networks", {}, "GET", "/api/networkmanager/network/current_networks")
_kp3("bind_network", {"pid": 1234, "network_name": "eth0"}, "POST", "/api/networkmanager/network/bind_network",
     {"pid": 1234, "network_name": "eth0"}, **_KP3_OK)
_kp3("get_network_choices", {}, "GET", "/api/networkmanager/network/list_network")

# ── 4.2 Static routes ──
_ROUTE_IN = {"interface": "eth0", "target": "192.168.43.1", "netmask": "255.255.255.255", "gateway": "192.168.43.254"}
_ROUTE = {"interface": "eth0", "target": "192.168.43.1", "netmask": "255.255.255.255", "gateway": "192.168.43.254",
          "metric": 0, "mtu": 1500}
_kp3("add_static_route", _ROUTE_IN, "POST", "/api/networkmanager/static/routing/add", _ROUTE, **_KP3_OK)
_kp3("get_static_routes", {}, "GET", "/api/networkmanager/static/routing/get")
_kp3("modify_static_route", {"id": 2, **_ROUTE_IN, "mtu": 1400}, "POST", "/api/networkmanager/static/routing/modify",
     {"id": 2, **{**_ROUTE, "mtu": 1400}}, **_KP3_OK)
_kp3("delete_static_route", {"id": 2}, "POST", "/api/networkmanager/static/routing/del", {"ids": 2}, **_KP3_OK)
_kp3("set_static_route_enabled", {"id": 2, "enabled": True}, "POST", "/api/networkmanager/static/routing/set",
     {"id": 2, "status": 1}, **_KP3_OK)

# ── 4.3 Wi-Fi ──
_WIFI_BODY = {"wifi_ssid": "KILOVIEW", "wifi_bssid": "", "wifi_encryption": 4, "wifi_password": "Kiloview001",
              "param_autoconn": True, "param_addr_solv": "dhcp", "ip_address": "", "netmask": "", "gateway": "",
              "dns": "", "dns2": "", "mac_address": ""}
_kp3("add_wifi", {"ssid": "KILOVIEW", "password": "Kiloview001"}, "POST", "/api/networkmanager/wifi/add", _WIFI_BODY,
     **_KP3_OK)
_kp3("get_saved_wifi", {}, "GET", "/api/networkmanager/wifi/get")
_kp3("get_current_wifi", {}, "GET", "/api/networkmanager/wifi/getCurrentWifi")
_kp3("get_wifi", {"ssid": "KILOVIEW_5G"}, "GET", "/api/networkamanager/wifi/getWifiName?wifi_ssid=KILOVIEW_5G")
_kp3("modify_wifi", {"ssid": "KILOVIEW", "password": "Kiloview001", "autoconnect": False}, "POST",
     "/api/networkmanager/wifi/modify", {**_WIFI_BODY, "param_autoconn": False}, **_KP3_OK)
_kp3("forget_wifi", {"ssid": "KILOVIEW"}, "POST", "/api/networkmanager/wifi/del", {"wifi_ssid": "KILOVIEW"}, **_KP3_OK)
_kp3("set_wifi_connected", {"ssid": "KILOVIEW_5G", "connected": False}, "POST", "/api/networkmanager/wifi/set",
     {"enable": False, "wifi_ssid": "KILOVIEW_5G"},
     http_reply={"status": 200, "body": _kp3_json({"result": "ok", "msg": "WiFi disconnected successfully!"})},
     expect_result={"ok": {"kind": "ack"}})
_kp3("scan_wifi", {}, "GET", "/api/networkmanager/wifi/getScanWIFI")
_kp3("get_wifi_country_codes", {}, "GET", "/api/networkmanager/wifi/getWifiNationalCode")

# ── 4.4 Hotspot ──
# The document's example (4.4.1).
_kp3("set_hotspot", {"enabled": True, "ssid": "P3_ed95fd", "password": "12345678"}, "POST",
     "/api/networkmanager/hotspot/set",
     {"hotspot_ssid": "P3_ed95fd", "hotspot_hidden": 0, "hotspot_password": "12345678", "ipRange": "192.168.140.",
      "hotspot_channel": 6, "enable": True}, **_KP3_OK)
_kp3("get_hotspot", {}, "GET", "/api/networkmanager/hotspot/get")
_kp3("get_wifi_parameters", {}, "GET", "/api/networkmanager/hotspot/getWIFIParameter")
_kp3("set_wifi_parameters", {}, "POST", "/api/networkmanager/hotspot/setWIFIParameter",
     {"country_code": "ALL", "roaming": False, "roaming_rssi_thres": 5, "roaming_scan_intv": 5}, **_KP3_OK)
_kp3("get_wifi_channels", {}, "GET", "/api/networkmanager/hotspot/getWiFiChannel")
_kp3("get_hotspot_bands", {}, "GET", "/api/networkmanager/hotspot/getHotspotFrequencyBand")

# ── 4.5 Modems ──
_kp3("get_modems", {}, "GET", "/api/networkmanager/modem/get")
_kp3("get_modem", {"modem": "modem1"}, "GET", "/api/networkmanager/modem/getModemByNumber?modem_number=modem1")
_kp3("get_apn_list", {}, "GET", "/api/networkmanager/modem/getAPN")
# The document's example (4.5.4).
_kp3("set_modem_parameters", {"modem": "modem3", "version": "5G", "device_name": "/dev/ttyUSB3"}, "POST",
     "/api/networkmanager/modem/setModemParameter",
     {"version": "5G", "modem_name": "modem3", "modem_number": "", "device_name": "/dev/ttyUSB3", "ip_address": "",
      "enable_5G": False, "network_mode": "AUTO", "apn": "", "username": "", "password": "", "pin": "", "auth": 1},
     http_reply={"status": 200, "body": _kp3_json({"result": "ok", "msg": "Successfully set Modem parameters!"})},
     expect_result={"ok": {"kind": "ack"}})
_kp3("reset_modem", {"modem": "modem3", "wwan_name": "usb0"}, "POST", "/api/networkmanager/modem/modemRestore",
     {"modem_name": "modem3", "wwan_name": "usb0"}, **_KP3_OK)
_kp3("set_modem_enabled", {"modem": "modem3", "wwan_name": "usb0", "enabled": False}, "POST",
     "/api/networkmanager/modem/modemEnable", {"enable_card": False, "modem_name": "modem3", "wwan_name": "usb0"},
     **_KP3_OK)
_kp3("get_imei", {}, "GET", "/api/networkmanager/modem/imei_info")

# ── 5 KiloLink ──
_kp3("update_bonding_codec_info", {}, "GET", "/api/kilolink/update_codec_info", **_KP3_OK)
_kp3("set_bonding_streaming", {"enabled": True}, "POST", "/api/kilolink/bonding/setting_stream", {"enable": True},
     **_KP3_OK)
_kp3("get_bonding_streaming", {}, "GET", "/api/kilolink/bonding/stream_state",
     **_kp3_value({"result": "ok", "data": {"enable": True}}, True))
_kp3("set_bonding_connection", {"enabled": True}, "POST", "/api/kilolink/bonding/setting", {"enable": True}, **_KP3_OK)
_kp3("get_bonding_connection", {}, "GET", "/api/kilolink/bonding/state")
_kp3("get_bonding_server_stream", {}, "GET", "/api/kilolink/bonding/server_stream_state")
_kp3("set_bonding_server", {"ip": "192.168.35.140", "key": "3dc92ca6"}, "POST", "/api/kilolink/bonding/connect",
     {"ip": "192.168.35.140", "port": 60000, "crypto": False, "dynamic_bitrate": 1, "key": "3dc92ca6", "enable": True},
     **_KP3_OK)
_kp3("get_bonding_server", {}, "GET", "/api/kilolink/bonding/server")
_kp3("get_bonding_server_history", {}, "GET", "/api/kilolink/bonding/server_conf")
_kp3("set_bonding_interface", {"ifname": "eth0", "enabled": False}, "POST", "/api/kilolink/network/select",
     {"ifname": "eth0", "enable": False}, **_KP3_OK)

# ── 6.1 Video input ──
_kp3("get_edid", {}, "GET", "/api/codec/vin/edid")
_kp3("set_edid_priority", {"priority": "2"}, "POST", "/api/codec/vin/edid/priority", {"edid_priority": "2"},
     http_reply={"status": 200, "body": _kp3_json({"result": "ok", "msg": ""})}, expect_result={"ok": {"kind": "ack"}})
_kp3_upload("upload_edid", {"file": _KP3_FILE}, "/api/codec/vin/edid/upload", [("file", _KP3_BYTES, "edid.bin")])
_kp3("reset_edid", {}, "GET", "/api/codec/vin/edid/reset", **_KP3_OK)
_kp3("get_video_input", {}, "GET", "/api/codec/v1/vin/detail")
_kp3("get_video_sources", {}, "GET", "/api/codec/v1/vin/list_source")
_kp3("get_video_formats", {}, "GET", "/api/codec/v1/vin/list_format")
_kp3("set_video_source", {"source": "sdi"}, "POST", "/api/codec/v1/vin/source", {"source": "sdi"}, **_KP3_OK)

# ── 6.2 Audio ──
_kp3("get_audio_input", {}, "GET", "/api/codec/v1/audio/input")
# The document's example (6.2.2).
_kp3("set_audio_input", {"source": "hdmi", "current_source": "sdi"}, "POST", "/api/codec/v1/audio/input/source",
     {"source": "hdmi", "gain": {"hdmi": 0, "sdi": 0, "linein": 0, "uvc": 0}, "current_source": "sdi"}, **_KP3_OK)
_kp3("get_audio_sources", {}, "GET", "/api/codec/v1/audio/input/list_source")
_kp3("get_audio_encoders", {}, "GET", "/api/codec/v1/audio/encode/list")
# The document's example (6.2.5).
_kp3("add_audio_encoder", {"name": "AAC", "bitrate": 64000}, "POST", "/api/codec/v1/audio/encode",
     {"id": "", "name": "AAC", "codec": "AAC", "resample": "fastest", "sampling": 48000, "channels": 2,
      "bitrate": 64000, "g711Law": "PCMU", "chn_map": [0, 1, 2, 3]}, **_KP3_OK)
# The document's example (6.2.6).
_kp3("modify_audio_encoder", {"id": 3, "name": "G711", "codec": "G711", "sampling": 8000, "channels": 1}, "PUT",
     "/api/codec/v1/audio/encode/3",
     {"id": 3, "name": "G711", "codec": "G711", "resample": "fastest", "sampling": 8000, "channels": 1,
      "bitrate": 128000, "g711Law": "PCMU", "chn_map": [0, 1, 2, 3]}, **_KP3_OK)
_kp3("remove_audio_encoder", {"id": 3}, "DELETE", "/api/codec/v1/audio/encode/3", **_KP3_OK)

# ── 6.3 Video encoder ──
_kp3("get_video_encoder", {}, "GET", "/api/codec/v1/venc/settings")
# The document's example (6.3.2).
_kp3("set_video_encoder", {}, "POST", "/api/codec/v1/venc/settings",
     {"codec": "H264", "grey": False, "profile": 0, "picHeight": 1, "picWidth": 1, "mode": "CBR", "gop": 60,
      "fps": "auto", "bitrate": 6000, "maxBitrate": 6000, "minQp": 10, "maxQp": 40, "minIQp": 10, "minIp": 1,
      "maxIp": 20, "deinterlaceMode": "line_doubling", "timecode_type": 1}, **_KP3_OK)
_kp3("set_video_bitrate", {"bitrate": 4000}, "POST", "/api/codec/v1/venc/bitrate", {"bitrate": 4000}, **_KP3_OK)

# ── 6.4 Video processing ──
_IMG = "f232aece686c11f0b6df4a53f6e8cf12"
_kp3("get_images", {}, "GET", "/api/codec/vpss/image")
_kp3_upload("upload_image", {"file": _KP3_FILE, "name": "123", "filename": "logo.png"}, "/api/codec/vpss/image",
            [("file", _KP3_BYTES, "logo.png"), ("name", b"123", None)])
_kp3("delete_image", {"id": _IMG}, "DELETE", "/api/codec/vpss/image/" + _IMG, **_KP3_OK)
_kp3("get_still_frames", {}, "GET", "/api/codec/vpss/static_frame")
_kp3_upload("upload_still_frame", {"file": _KP3_FILE}, "/api/codec/vpss/static_frame",
            [("file", _KP3_BYTES, "static_frame.png")])
_kp3("reset_still_frame", {}, "POST", "/api/codec/vpss/static_frame/reset", **_KP3_OK)
_kp3("get_input_transform", {}, "GET", "/api/codec/v1/vpss/input/rmf")
_kp3("set_input_transform", {"mirror": True}, "POST", "/api/codec/v1/vpss/input/rmf",
     {"rotate": 0, "mirror_en": True, "flip_en": False}, **_KP3_OK)
_kp3("get_osd", {}, "GET", "/api/codec/v1/vpss/osd")
_kp3("set_osd_enabled", {"enabled": True}, "PUT", "/api/codec/v1/vpss/osd", {"enable": True}, **_KP3_OK)
# The document's example (6.4.11).
_kp3("add_osd_item", {"type": "text", "text": "1234"}, "POST", "/api/codec/v1/vpss/osd",
     {"id": "", "type": "text", "file_id": "", "text": "1234", "x": 0, "y": 0, "position": "LeftTop", "font_size": 1,
      "style": "normal", "outline": 0, "font_color": 1, "outline_color": 0, "enable": True}, **_KP3_OK)
_OSD = "67ad55e868f711f0b6df4a53f6e8cf12"
_kp3("get_osd_item", {"id": _OSD}, "GET", "/api/codec/v1/vpss/osd/" + _OSD)
# The document's example (6.4.13).
_kp3("modify_osd_item", {"id": _OSD, "type": "text", "file_id": "5d87689e68f511f0b6df4a53f6e8cf12", "text": "1234",
                         "x": 1, "y": 1, "position": "Manual", "style": "bold", "font_color": 2}, "PUT",
     "/api/codec/v1/vpss/osd/" + _OSD,
     {"id": _OSD, "type": "text", "file_id": "5d87689e68f511f0b6df4a53f6e8cf12", "text": "1234", "x": 1, "y": 1,
      "position": "Manual", "font_size": 1, "style": "bold", "outline": 0, "font_color": 2, "outline_color": 0,
      "enable": True}, **_KP3_OK)
_kp3("delete_osd_item", {"id": _OSD}, "DELETE", "/api/codec/v1/vpss/osd/" + _OSD, **_KP3_OK)
# The document's example (6.4.15).
_kp3("set_screen_output", {"left": 800, "right": 800, "top": 480, "bottom": 480}, "POST", "/api/codec/v1/vpss/output",
     {"left": 800, "right": 800, "top": 480, "down": 480, "rotation": 0}, **_KP3_OK)

# ── 6.5 Panel ──
_kp3("get_panel_settings", {}, "GET", "/api/codec/panel/settings")
# The document's example (6.5.2).
_kp3("set_panel_settings", {"video_source": "HDMI", "codec": "H265", "fps": "30", "audio_source": "HDMI"}, "POST",
     "/api/codec/panel/settings",
     {"video_source": "HDMI", "codec": "H265", "height": 1080, "width": 1920, "video_bitrate": 4000, "fps": "30",
      "audio_source": "HDMI", "deinterlaceMode": "line_doubling", "gain": 0}, **_KP3_OK)


# ── Telemetry: the document's example replies ──
def _kp3_tel(name, path, body, expect_state, **extra):
    telemetry(KP3, name, inbound_http={"path": path, "body": body if isinstance(body, str) else _kp3_json(body),
                                       **({"request": extra.pop("request")} if "request" in extra else {})},
              expect_state=expect_state, **extra)


_kp3_tel("hostname", "/api/systemctrl/system/getHostname",
         {"result": "ok", "msg": None, "data": {"hostname": "P3-20240806P3001"}},
         {"device": {"hostname": "P3-20240806P3001"}})
_SYSINFO = {"result": "ok", "data": {
    "version": {"ndiVersion": "5.1.3", "hardwareVersion": "2.0", "softwareVersion": "1.21.0009.0002",
                "serialNumber": "20240806P3001", "product": "P3", "oem": "kiloview"},
    "cpu": {"precent": 57.944}, "mem": {"precent": 33.6, "total": 1029500, "used": 345972},
    "disk": {"precent": 31, "free": 1288652, "used": 588780, "total": 1998672},
    "persisTime": {"uptime": 56019.12}, "imei": {"IMEI1": "869841060061295", "IMEI2": "", "IMEI3": "", "IMEI4": ""}}}
_SYSINFO_PATH = ("/api/systemctrl/system/getSystemInfo?version=true&cpu=true&memory=true&disk=true&persisTime=true"
                 "&imei=true")
_kp3_tel("system-info", _SYSINFO_PATH, _SYSINFO,
         {"device": {"product": "P3", "oem": "kiloview", "serial_number": "20240806P3001",
                     "firmware": "1.21.0009.0002", "hardware_version": "2.0", "ndi_version": "5.1.3",
                     "cpu_percent": 57.944, "memory_percent": 33.6, "memory_total": 1029500, "memory_used": 345972,
                     "disk_percent": 31.0, "disk_total": 1998672, "disk_used": 588780, "disk_free": 1288652,
                     "uptime": 56019.12, "imei": {"1": "869841060061295", "2": "", "3": "", "4": ""}}})
# The table's spelling of the load fields.
_kp3_tel("system-info-percent", _SYSINFO_PATH,
         {"result": "ok", "data": {"cpu": {"percent": 12.5}, "mem": {"percent": 40.0}, "disk": {"percent": 7}}},
         {"device": {"cpu_percent": 12.5, "memory_percent": 40.0, "disk_percent": 7.0}})
_kp3_tel("api-authentication", "/api/systemctrl/user/auth_info", {"result": "ok", "data": {"enable": False}},
         {"auth": {"api_authentication": False}})
_kp3_tel("users", "/api/systemctrl/user/list",
         {"result": "ok", "data": [
             {"id": "admin", "alias": "Admin", "web": True, "api": True, "create_time": "1970-01-01 00:00:00"},
             {"id": "zhangsan", "alias": "zhangsan", "web": True, "api": False, "create_time": "2025-06-25 14:28:03"}]},
         {"users": {"admin": {"alias": "Admin", "web": True, "api": True, "created": "1970-01-01 00:00:00"},
                    "zhangsan": {"alias": "zhangsan", "web": True, "api": False, "created": "2025-06-25 14:28:03"}}},
         state_before={"users": {"lisi": {"alias": "lisi", "web": True, "api": True, "created": "2025-06-26 10:00:00"}}})
_TIME = {"time": "2025-06-26 14:47:32", "timetype": "pc", "offset": 8,
         "ntp": "time.windows.com time.apple.com pool.ntp.org"}
_kp3_tel("time", "/api/systemctrl/systime/get", {"result": "ok", "msg": None, "data": {**_TIME, "Timezone": "Asia/Shanghai"}},
         {"time": {"time": "2025-06-26 14:47:32", "mode": "pc", "timezone": "Asia/Shanghai", "offset": 8,
                   "ntp_servers": "time.windows.com time.apple.com pool.ntp.org"}})
_kp3_tel("time-lower-case", "/api/systemctrl/systime/get",
         {"result": "ok", "msg": None, "data": {**_TIME, "timezone": "Europe/London"}},
         {"time": {"time": "2025-06-26 14:47:32", "mode": "pc", "timezone": "Europe/London", "offset": 8,
                   "ntp_servers": "time.windows.com time.apple.com pool.ntp.org"}})
_kp3_tel("ntp-status", "/api/systemctrl/systime/ntpstatus",
         [{"remote": "127.127.1.0", "refid": ".LOCL.", "st": "10", "t": "l", "when": "26", "poll": "64", "reach": "1",
           "delay": "0.000", "offset": "0.000", "jitter": "0.001"},
          {"remote": "*20.189.79.72", "refid": "10.0.6.24", "st": "3", "t": "u", "when": "24", "poll": "64",
           "reach": "1", "delay": "98.516", "offset": "-3.859", "jitter": "0.001"}],
         {"time": {"ntp_peers": {
             "0": {"remote": "127.127.1.0", "refid": ".LOCL.", "stratum": 10, "type": "l", "when": 26, "poll": 64,
                   "reach": "1", "delay": 0.0, "offset": 0.0, "jitter": 0.001},
             "1": {"remote": "*20.189.79.72", "refid": "10.0.6.24", "stratum": 3, "type": "u", "when": 24, "poll": 64,
                   "reach": "1", "delay": 98.516, "offset": -3.859, "jitter": 0.001}}}})
_kp3_tel("web-protocols", "/api/systemctrl/system/getProtocol",
         {"result": "ok", "msg": None, "data": {"http": True, "https": True, "httpPort": 80, "httpsPort": 443}},
         {"web": {"http": True, "https": True, "http_port": 80, "https_port": 443}})
# The key in the reply never reaches state.
_kp3_tel("ssl", "/api/systemctrl/system/ssl",
         {"result": "ok", "msg": None, "data": {"ssl_crt": "", "ssl_key": "secret", "enable_reset": False}},
         {"ssl": {"certificate": "", "reset_allowed": False}})
_kp3_tel("production-test", "/api/ststemctrl/system/production_stat", {"result": "ok", "data": {"value": True}},
         {"system": {"production_test": True}})
_kp3_tel("nic-speed", "/api/systemctrl/system/getNetworkSpeed",
         {"result": "ok", "data": {"eth0": {"receive": 8.92, "transmit": 4308.7, "speed": 1000}}},
         {"nic": {"eth0": {"receive": 8.92, "transmit": 4308.7, "speed": 1000}}})
_kp3_tel("recording-status", "/api/record/v1/get_recording_status", {"result": "ok", "data": {"status": True}},
         {"recording": {"active": True}})
_kp3_tel("recording-settings", "/api/record/record/get_record_info",
         {"result": "ok", "data": {"disk_type": "largest", "format": "ts", "file_prefix": "REC",
                                   "limit_type": "time-loop", "limit_size": 51200, "limit_time": 60,
                                   "disk_policy": "noSpace", "follow_timecode_start": False}},
         {"recording": {"disk_type": "largest", "format": "ts", "file_prefix": "REC", "limit_type": "time-loop",
                        "limit_size": 51200, "limit_time": 60, "disk_policy": "noSpace", "follow_timecode": False}})
_DISK = {"id": "sdb1", "name": "sdb1", "type": "", "host": "", "volume": "/dev/sdb1", "options": "",
         "mounted": "/run/media/sdb1", "filesys": "vfat", "status": "mounted", "choose": True, "disk_type": "usb",
         "total": 953867, "used": 437021, "free": 516845, "percent": 45}
_DISK_STATE = {"storage": {"usb": {"sdb1": {"name": "sdb1", "type": "", "volume": "/dev/sdb1",
                                            "mounted": "/run/media/sdb1", "filesystem": "vfat", "status": "mounted",
                                            "chosen": True, "disk_type": "usb", "total": 953867, "used": 437021,
                                            "free": 516845, "percent": 45}}}}
_DISK_FILES = [{"method": "GET", "target": "/api/record/record/get_disk_files?device=%2Frun%2Fmedia%2Fsdb1"}]
_kp3_tel("storage", "/api/record/record/disk_list", {"result": "ok", "data": {"nas": [], "USB": [_DISK]}},
         _DISK_STATE, state_before={"storage": {"usb": {"sdc1": {"name": "sdc1"}}}}, expect_then_send=_DISK_FILES)
_kp3_tel("storage-lower-case", "/api/record/record/disk_list", {"result": "ok", "data": {"nas": [], "usb": [_DISK]}},
         _DISK_STATE, expect_then_send=_DISK_FILES)
_kp3_tel("recording-files", "/api/record/record/get_disk_files?device=%2Frun%2Fmedia%2Fsdb1",
         {"result": "ok", "data": [{"path": "/run/media/sdb1/RECORD/MAIN20250521_161544_719240/REC-MAIN-1.mp4",
                                    "name": "REC-MAIN-1.mp4", "size": 6.75, "current": False, "stream": "main",
                                    "start_time": "2025-05-21 16:15:44", "end_time": "2025-05-21 16:16:01"}]},
         {"recording": {"files": {"/run/media/sdb1": {"0": {
             "path": "/run/media/sdb1/RECORD/MAIN20250521_161544_719240/REC-MAIN-1.mp4", "name": "REC-MAIN-1.mp4",
             "size": 6.75, "current": False, "stream": "main", "start_time": "2025-05-21 16:15:44",
             "end_time": "2025-05-21 16:16:01"}}}}})
_kp3_tel("streams", "/api/streamer/v1/stream/list",
         [{"id": "11213c00ab3bf668", "type": "srt", "name": "SRT1", "enable": False, "status": "stop", "bindAudio": 3,
           "address": "", "connectionMode": "Listener", "encryption": "0", "passphrase": "", "latency": 125,
           "bandwidth": 25, "payloadSize": 1316, "listenerPort": 1025, "srt_stream_id": "", "ts_null_multiple": 0,
           "ts_pcr_period": 20, "ts_pts_pcr_delay": 200, "ts_transport_stream_id": 101, "auto_enable": False,
           "bindNetwork": "auto", "shortcut_stream": False, "bool": False, "network_enable": True,
           "addressUrl": "offline"},
          {"id": "542b4dcb8d15d59b", "type": "rtp", "name": "RTP", "enable": True, "status": "start", "bindAudio": 1,
           "address": "192.168.35.215", "port": 1026, "bindNetwork": "eth0"}],
         {"streams": {
             "11213c00ab3bf668": {"type": "srt", "name": "SRT1", "enabled": False, "status": "stop", "bind_audio": 3,
                                  "address": "", "connection_mode": "Listener", "encryption": "0", "latency": 125,
                                  "bandwidth": 25, "payload_size": 1316, "listener_port": 1025, "srt_stream_id": "",
                                  "auto_enable": False, "bind_network": "auto", "network_enable": True,
                                  "address_url": "offline"},
             "542b4dcb8d15d59b": {"type": "rtp", "name": "RTP", "enabled": True, "status": "start", "bind_audio": 1,
                                  "address": "192.168.35.215", "port": 1026, "bind_network": "eth0"}}},
         state_before={"streams": {"deadbeef": {"type": "rtmp", "name": "Old"}}})
_kp3_tel("media-users", "/api/streamer/v1/stream/media_user", {"result": "ok", "user_list": ["admin", "zhangyu"]},
         {"media_users": {"0": "admin", "1": "zhangyu"}})
_PTZ = {"enable": 1, "typ": "network",
        "network": {"protocol": "TCP", "addr": "192.168.43.225", "port": 52381, "ptz_protocol": "Sony Visca",
                    "ptz_addr": "1"},
        "serial": {"device": "none", "baudrate": 9600, "parity": "none", "startBits": 8, "endBits": 1, "xonxoff": 0,
                   "rtscts": 0, "ptz_protocol": "Sony Visca", "ptz_addr": "1"}}
_PTZ_USB = {"ptz_protocol": "v4l2-ctl", "ptz_addr": "1", "port": 1, "control_type": "virtual", "device": "/dev/video0"}
_PTZ_STATE = {"enabled": True, "type": "network",
              "network": {"protocol": "TCP", "address": "192.168.43.225", "port": 52381, "ptz_protocol": "Sony Visca",
                          "ptz_address": "1"},
              "serial": {"device": "none", "baudrate": 9600, "parity": "none", "data_bits": 8, "stop_bits": 1,
                         "xonxoff": False, "rtscts": False, "ptz_protocol": "Sony Visca", "ptz_address": "1"},
              "usb": {"ptz_protocol": "v4l2-ctl", "ptz_address": "1", "port": 1, "control_type": "virtual",
                      "device": "/dev/video0"}}
_kp3_tel("ptz", "/api/streamer/ptz/info", {"result": "ok", "data": {**_PTZ, "USB": _PTZ_USB}}, {"ptz": _PTZ_STATE})
_kp3_tel("ptz-lower-case", "/api/streamer/ptz/info", {"result": "ok", "data": {**_PTZ, "usb": _PTZ_USB}},
         {"ptz": _PTZ_STATE})
_kp3_tel("ptz-protocols", "/api/streamer/ptz/protocol",
         {"result": "ok", "msg": None, "data": ["Sony Visca", "Pelco-d", "Pelco-p"]},
         {"ptz": {"protocols": {"0": "Sony Visca", "1": "Pelco-d", "2": "Pelco-p"}}})
_kp3_tel("ptz-serial-devices", "/api/streamer/ptz/serial_device", {"result": "ok", "data": ["/dev/ttyS1"]},
         {"ptz": {"serial_devices": {"0": "/dev/ttyS1"}}}, state_before={"ptz": {"serial_devices": {"1": "/dev/ttyS2"}}})
_ETH = {"status": "up", "address": "192.168.35.178", "netmask": "255.255.255.0", "gw": "192.168.35.254",
        "mac": "4A:53:F6:E8:CF:12", "method": "static", "enable": True, "device": "eth0"}
_ETH_STATE = {"status": "up", "address": "192.168.35.178", "netmask": "255.255.255.0", "gateway": "192.168.35.254",
              "mac": "4A:53:F6:E8:CF:12", "method": "static", "enabled": True, "dns": "8.8.8.8;8.8.4.4"}
_kp3_tel("interfaces", "/api/networkmanager/network/GetLinkinfo",
         {"result": "ok", "data": [{**_ETH, "dns": "8.8.8.8;8.8.4.4"}]},
         {"network": {"interfaces": {"eth0": _ETH_STATE}}})
# The translated example's upper-case DNS key.
_kp3_tel("interfaces-dns-upper-case", "/api/networkmanager/network/GetLinkinfo",
         {"result": "ok", "data": [{**_ETH, "DNS": "8.8.8.8;8.8.4.4"}]},
         {"network": {"interfaces": {"eth0": _ETH_STATE}}})
_kp3_tel("ethernet", "/api/networkmanager/network/GetEthernets", {"result": "ok", "data": {**_ETH, "dns": "8.8.8.8;8.8.4.4"}},
         {"network": {"interfaces": {"eth0": _ETH_STATE}}})
_kp3_tel("ethernet-dns-upper-case", "/api/networkmanager/network/GetEthernets",
         {"result": "ok", "data": {**_ETH, "DNS": "8.8.8.8;8.8.4.4"}},
         {"network": {"interfaces": {"eth0": _ETH_STATE}}})
_kp3_tel("link-list", "/api/networkmanager/network/GetLinkList",
         {"result": "ok", "data": [{"device": "eth0", "device_name": "ETH0", "status": "up", "address": "192.168.35.178",
                                    "netmask": "255.255.255.0", "mac": "4A:53:F6:E8:CF:12", "gw": "192.168.35.254"}]},
         {"network": {"interfaces": {"eth0": {"name": "ETH0", "status": "up", "address": "192.168.35.178",
                                              "netmask": "255.255.255.0", "gateway": "192.168.35.254",
                                              "mac": "4A:53:F6:E8:CF:12"}}}})
_LINKS = [{"enable": True, "modems": "total", "name": "total", "type": "total", "receive": 437.02, "transmit": 24.86,
           "sort": 1},
          {"modems": "eth0", "address": "192.168.35.178", "name": "eth0", "type": "eth", "enable": True,
           "enable_card": True, "version": "", "signal_version": "", "modem_Operator": "eth0", "signal": 0, "rsrp": 0,
           "sinr": 0, "receive": 30.95, "transmit": 24.86, "sort": 2, "insert_card": "no", "connect_number": 0,
           "wwan_name": "eth0", "pin_status": 10},
          {"modems": "wlan0", "address": "192.168.230.113", "name": "KILOVIEW_5G", "type": "wlan", "enable": True,
           "enable_card": True, "version": "", "signal_version": "", "modem_Operator": "wlan0", "signal": -100,
           "rsrp": 0, "sinr": 0, "receive": 406.07, "transmit": 0.5, "sort": 3, "insert_card": "no",
           "connect_number": 0, "wwan_name": "wlan0", "pin_status": 10}]
_LINKS_STATE = {"links": {
    "total": {"name": "total", "type": "total", "enabled": True, "receive": 437.02, "transmit": 24.86, "sort": 1},
    "eth0": {"name": "eth0", "type": "eth", "address": "192.168.35.178", "enabled": True, "card_enabled": True,
             "version": "", "signal_version": "", "operator": "eth0", "signal": 0, "rsrp": 0, "sinr": 0,
             "receive": 30.95, "transmit": 24.86, "sort": 2, "card_inserted": "no", "connections": 0,
             "wwan_name": "eth0", "pin_status": 10},
    "wlan0": {"name": "KILOVIEW_5G", "type": "wlan", "address": "192.168.230.113", "enabled": True,
              "card_enabled": True, "version": "", "signal_version": "", "operator": "wlan0", "signal": -100,
              "rsrp": 0, "sinr": 0, "receive": 406.07, "transmit": 0.5, "sort": 3, "card_inserted": "no",
              "connections": 0, "wwan_name": "wlan0", "pin_status": 10}}}
# A modem that left the list (its slot switched off) leaves state.
_kp3_tel("link-rates", "/api/networkmanager/network/getNetworkSpeed", {"result": "ok", "data": _LINKS}, _LINKS_STATE,
         state_before={"links": {"wwan0": {"name": "CHN-UNICOM", "signal": -67}}})
_kp3_tel("link-data", "/api/networkmanager/network/getNetworkInterfaceAllData", {"result": "ok", "data": _LINKS},
         _LINKS_STATE)
_kp3_tel("current-networks", "/api/networkmanager/network/current_networks",
         {"result": "ok", "data": [{"name": "eth0", "wwan_name": "eth0", "version": "", "alias": "eth0",
                                    "sub_wwan": "eth0", "address": "192.168.35.178", "weight": 252}]},
         {"network": {"current": {"eth0": {"wwan_name": "eth0", "version": "", "alias": "eth0", "sub_wwan": "eth0",
                                           "address": "192.168.35.178", "weight": 252}}}})
_kp3_tel("traffic", "/api/networkmanager/network/traffic_data",
         {"result": "ok", "data": [{"receive": 371494655, "transmit": 1403095098, "start_time": "2025/06/04 14:17:44",
                                    "type": "eth", "name": "eth0", "sort": 2, "traffic_key": "eth0"}]},
         {"network": {"traffic": {"eth0": {"receive": 371494655.0, "transmit": 1403095098.0,
                                           "start_time": "2025/06/04 14:17:44", "type": "eth", "name": "eth0"}}}})
_kp3_tel("network-choices", "/api/networkmanager/network/list_network",
         {"result": "ok", "data": [{"name": "AUTO", "value": "auto"},
                                   {"name": "SIM1", "value": "modem1", "type": "", "address": "", "modem_Operator": "",
                                    "enable": False}]},
         {"network": {"choices": {"auto": {"name": "AUTO"},
                                  "modem1": {"name": "SIM1", "type": "", "address": "", "operator": "",
                                             "enabled": False}}}})
_kp3_tel("static-routes", "/api/networkmanager/static/routing/get",
         {"result": "ok", "data": [{"interface": "eth0", "target": "192.168.43.1", "netmask": "255.255.255.255",
                                    "gateway": "192.168.43.254", "metric": 0, "mtu": 1500, "status": 0, "id": 2}]},
         {"routes": {"2": {"interface": "eth0", "target": "192.168.43.1", "netmask": "255.255.255.255",
                           "gateway": "192.168.43.254", "metric": 0, "mtu": 1500, "enabled": False}}})
_WIFI = {"wifi_ssid": "KILOVIEW_5G", "wifi_bssid": "78:45:58:4b:74:fa", "wifi_password": "kiloview001",
         "wifi_encryption": 4, "wifi_signal": -65, "param_autoconn": True, "mac_address": "2c:c3:e6:ed:95:fd",
         "param_addr_solv": "dhcp", "netmask": "255.255.255.0", "ip_address": "192.168.230.113",
         "gateway": "192.168.230.254", "dns": "", "dns2": "", "hw_mode": "2.4G/5G", "enable": True, "state": 1,
         "active_disconnect": False, "network_id": 1}
_kp3_tel("saved-wifi", "/api/networkmanager/wifi/get", {"result": "ok", "data": [_WIFI]},
         {"wifi": {"saved": {"KILOVIEW_5G": {"bssid": "78:45:58:4b:74:fa", "encryption": 4, "signal": -65,
                                             "autoconnect": True, "mac": "2c:c3:e6:ed:95:fd", "method": "dhcp",
                                             "netmask": "255.255.255.0", "address": "192.168.230.113",
                                             "gateway": "192.168.230.254", "dns": "", "dns2": "", "hw_mode": "2.4G/5G",
                                             "enabled": True, "state": 1, "active_disconnect": False,
                                             "network_id": 1}}}})
_kp3_tel("current-wifi", "/api/networkmanager/wifi/getCurrentWifi", {"result": "ok", "data": _WIFI},
         {"wifi": {"current": {"ssid": "KILOVIEW_5G", "bssid": "78:45:58:4b:74:fa", "encryption": 4, "signal": -65,
                               "method": "dhcp", "address": "192.168.230.113", "netmask": "255.255.255.0",
                               "gateway": "192.168.230.254", "mac": "2c:c3:e6:ed:95:fd", "hw_mode": "2.4G/5G",
                               "state": 1}}})
_kp3_tel("wifi-scan", "/api/networkmanager/wifi/getScanWIFI",
         {"result": "ok", "data": [{"wifi_signal": -41, "akm": True, "wifi_ssid": "KILOVIEW", "save": False},
                                   {"wifi_signal": -49, "akm": False, "wifi_ssid": "NND3D4_2.4G", "save": True}]},
         {"wifi": {"scan": {"0": {"ssid": "KILOVIEW", "signal": -41, "secured": True, "saved": False},
                            "1": {"ssid": "NND3D4_2.4G", "signal": -49, "secured": False, "saved": True}}}})
_kp3_tel("hotspot", "/api/networkmanager/hotspot/get",
         {"result": "ok", "data": {"hotspot_ssid": "P3_ed95fd", "hotspot_password": "12345678", "hotspot_hidden": 0,
                                   "hw_mode": "g", "hw_mode_name": "2.4G/5G", "hotspot_channel": 6,
                                   "ipRange": "192.168.140.", "ipPrefix": "24", "enable": True,
                                   "gateway": "192.168.140.254", "mac_address": "2e:c3:e6:ed:95:fd",
                                   "connect_number": 0}},
         {"hotspot": {"ssid": "P3_ed95fd", "hidden": False, "hw_mode": "g", "hw_mode_name": "2.4G/5G", "channel": 6,
                      "ip_range": "192.168.140.", "ip_prefix": "24", "enabled": True, "gateway": "192.168.140.254",
                      "mac": "2e:c3:e6:ed:95:fd", "connections": 0}})
_kp3_tel("wifi-parameters", "/api/networkmanager/hotspot/getWIFIParameter",
         {"result": "ok", "data": {"country_code": "ALL", "roaming": False, "roaming_rssi_thres": 5,
                                   "roaming_scan_intv": 5}},
         {"wifi": {"country_code": "ALL", "roaming": False, "roaming_rssi_threshold": 5, "roaming_scan_interval": 5}})
_MODEM = {"modem_name": "modem1", "modem_Operator": "CHN-UNICOM", "insert_card": True,
          "modem_number": "89852351023050349492", "modem_net_play": "1", "ip_address": "10.215.72.17", "apn": "",
          "username": "", "password": "", "pin": "", "version": "4G", "enable": True, "id": 1, "modem_signal": -67,
          "sort": 5, "wwan_name": "wwan0", "network_mode": "AUTO", "process_pid": "15989", "network_card": True,
          "auth": 1, "enable_5G": False, "device_name": "/dev/ttyUSB2", "enable_card": True, "signal_version": "4G"}
_MODEM_STATE = {"operator": "CHN-UNICOM", "card_inserted": True, "ccid": "89852351023050349492", "internet": "1",
                "address": "10.215.72.17", "apn": "", "version": "4G", "enabled": True, "signal": -67,
                "wwan_name": "wwan0", "network_mode": "AUTO", "network_card": True, "auth": 1, "enable_5g": False,
                "device_name": "/dev/ttyUSB2", "card_enabled": True, "signal_version": "4G"}
_kp3_tel("modems", "/api/networkmanager/modem/get",
         {"result": "ok", "data": [{**_MODEM, "modem_rsrp": -95, "modem_sinr": 12, "imei": "869841060061295",
                                    "pin_status": 10}]},
         {"modems": {"modem1": {**_MODEM_STATE, "rsrp": -95, "sinr": 12, "imei": "869841060061295",
                                "pin_status": 10}}})
_kp3_tel("modem", "/api/networkmanager/modem/getModemByNumber?modem_number=modem1", {**_MODEM, "module_name": "EG25-G"},
         {"modems": {"modem1": {**_MODEM_STATE, "module_name": "EG25-G"}}})
_kp3_tel("imei", "/api/networkmanager/modem/imei_info",
         {"result": "ok", "data": [{"modem": "modem3", "version": "5G", "IMEI_code": "869841060061295",
                                    "insert_card": True, "modem_signal": 0, "modem_Operator": "", "network_card": True}]},
         {"modems": {"modem3": {"imei": "869841060061295", "version": "5G", "card_inserted": True, "signal": 0,
                                "operator": "", "network_card": True}}})
_kp3_tel("bonding-streaming", "/api/kilolink/bonding/stream_state", {"result": "ok", "data": {"enable": True}},
         {"bonding": {"streaming": True}})
_kp3_tel("bonding-connected", "/api/kilolink/bonding/state", {"result": "ok", "data": {"enable": False}},
         {"bonding": {"connected": False}})
_kp3_tel("bonding-server-stream", "/api/kilolink/bonding/server_stream_state", {"result": "ok", "data": {"enable": True}},
         {"bonding": {"server_stream": True}})
_KL = {"port": 60000, "crypto": False, "dynamic_bitrate": 1, "key": "3dc92ca6", "enable": True}
_KL_STATE = {"bonding": {"server": {"address": "192.168.35.140", "port": 60000, "crypto": False,
                                    "dynamic_bitrate": True, "enabled": True}}}
_kp3_tel("bonding-server", "/api/kilolink/bonding/server", {"result": "ok", "data": {"ip": "192.168.35.140", **_KL}},
         _KL_STATE)
_kp3_tel("bonding-server-upper-case", "/api/kilolink/bonding/server",
         {"result": "ok", "data": {"IP": "192.168.35.140", **_KL}}, _KL_STATE)
_kp3_tel("bonding-history", "/api/kilolink/bonding/server_conf",
         {"result": "ok", "data": [{"ip": "175.178.97.152", "port": 50000, "key": "23699c64"},
                                   {"ip": "192.168.35.140", "port": 60000, "key": "3dc92ca6"}]},
         {"bonding": {"history": {"0": {"address": "175.178.97.152", "port": 50000},
                                  "1": {"address": "192.168.35.140", "port": 60000}}}})
_kp3_tel("bonding-history-upper-case", "/api/kilolink/bonding/server_conf",
         {"result": "ok", "data": [{"IP": "175.178.97.152", "port": 50000, "key": "23699c64"}]},
         {"bonding": {"history": {"0": {"address": "175.178.97.152", "port": 50000}}}})
_kp3_tel("edid", "/api/codec/vin/edid", {"edid_priority": "2", "edid": "AP///////wA=", "reset": False},
         {"input": {"edid_priority": "2", "edid_reset_allowed": False}})
_kp3_tel("video-input", "/api/codec/v1/vin/detail",
         {"signal": "locked", "format": "1920x1080p 60Hz", "width": 1920, "height": 1080, "interlaced": False,
          "fps": 60, "type": "HDMI", "interface": "v1", "max_width": 3840, "max_height": 2160, "max_fps": 60,
          "source": "hdmi", "input_encode": None, "input_resolution": None, "input_fps": None},
         {"input": {"signal": "locked", "format": "1920x1080p 60Hz", "width": 1920, "height": 1080,
                    "interlaced": False, "fps": 60.0, "type": "HDMI", "interface": "v1", "max_width": 3840,
                    "max_height": 2160, "max_fps": 60.0, "source": "hdmi"}})
_kp3_tel("video-sources", "/api/codec/v1/vin/list_source",
         [{"label": "HDMI", "value": "hdmi"}, {"label": "SDI", "value": "sdi"}, {"label": "USB Camera", "value": "uvc"}],
         {"input": {"sources": {"HDMI": "hdmi", "SDI": "sdi", "USB Camera": "uvc"}}})
_AUDIO_IN = {"source": "auto", "sampling": 48000, "channels": 2, "current_source": "sdi",
             "gain": {"hdmi": 0, "sdi": -6, "linein": 3, "uvc": 0}, "type": "AUTO"}
_kp3_tel("audio-input", "/api/codec/v1/audio/input", _AUDIO_IN,
         {"audio": {"input": {"source": "auto", "sampling": 48000, "channels": 2, "current_source": "sdi",
                              "type": "AUTO", "gain": {"hdmi": 0, "sdi": -6, "linein": 3, "uvc": 0}}}})
_kp3_tel("audio-encoders", "/api/codec/v1/audio/encode/list",
         [{"name": "Bonding-Only", "volume": 0, "resample": "fastest", "channels": 2, "sampleSize": 1024,
           "sampling": 48000, "codec": "AAC", "bitrate": 128000, "sampleWidth": 16, "g711Law": "PCMU",
           "chn_map": [0, 1, 2, 3], "audio_channel_id": 1, "id": 1, "real_sampling": 48000, "audio_source": "hdmi"}],
         {"audio": {"encoders": {"1": {"name": "Bonding-Only", "volume": 0, "resample": "fastest", "channels": 2,
                                       "sample_size": 1024, "sampling": 48000, "codec": "AAC", "bitrate": 128000,
                                       "sample_width": 16, "g711_law": "PCMU", "channel_id": 1,
                                       "real_sampling": 48000, "source": "hdmi"}}}},
         state_before={"audio": {"encoders": {"3": {"name": "G711"}}}})
_kp3_tel("video-encoder", "/api/codec/v1/venc/settings",
         {"source": "sdi", "codec": "H264", "enable": True, "grey": False, "profile": 0, "picHeight": 1, "picWidth": 1,
          "mode": "CBR", "gop": 60, "fps": "auto", "bitrate": 6000, "maxBitrate": 6000, "minQp": 10, "maxQp": 40,
          "minIQp": 10, "minIp": 1, "maxIp": 20, "deinterlaceMode": "line_doubling", "timecode_type": 1,
          "srcHeight": 1080, "srcWidth": 1920, "subHeight": 360, "subWidth": 640, "realHeight": 1080,
          "realWidth": 1920, "real_fps": 30},
         {"video": {"encoder": {"source": "sdi", "codec": "H264", "enabled": True, "grey": False, "profile": 0,
                                "pic_height": 1, "pic_width": 1, "mode": "CBR", "gop": 60, "fps": "auto",
                                "bitrate": 6000, "max_bitrate": 6000, "min_qp": 10, "max_qp": 40, "min_iqp": 10,
                                "min_ip": 1, "max_ip": 20, "deinterlace": "line_doubling", "timecode_type": 1,
                                "source_height": 1080, "source_width": 1920, "sub_height": 360, "sub_width": 640,
                                "real_height": 1080, "real_width": 1920, "real_fps": 30.0}}})
_kp3_tel("images", "/api/codec/vpss/image",
         [{"size_h": 1080, "size_w": 1920, "url": "/img/f232aece686c11f0b6df4a53f6e8cf12.png", "name": "123",
           "path": "/data/storage/img/f232aece686c11f0b6df4a53f6e8cf12.png", "id": "f232aece686c11f0b6df4a53f6e8cf12"}],
         {"vpss": {"images": {"f232aece686c11f0b6df4a53f6e8cf12": {
             "name": "123", "url": "/img/f232aece686c11f0b6df4a53f6e8cf12.png", "width": 1920, "height": 1080}}}})
_kp3_tel("still-frames", "/api/codec/vpss/static_frame",
         [{"size_h": 1080, "size_w": 1920, "url": "/frame/static_frame.png", "path": "/data/storage/frame/static_frame.png",
           "yuv_path": "/data/storage/frame/static_frame.yuv", "id": "static_frame", "pixel_format": 0}],
         {"vpss": {"still_frames": {"static_frame": {"url": "/frame/static_frame.png", "width": 1920,
                                                     "height": 1080}}}})
_kp3_tel("input-transform", "/api/codec/v1/vpss/input/rmf", {"rotate": 90, "mirror_en": False, "flip_en": True},
         {"vpss": {"rotate": 90, "mirror": False, "flip": True}})
_OSD_ITEM = {"enable": True, "type": "image", "position": "Manual", "x": 1, "y": 1, "font_size": 1, "font_color": 2,
             "style": "bold", "outline": 0, "outline_color": 0, "text": "1234",
             "file_id": "5d87689e68f511f0b6df4a53f6e8cf12", "row_spacing": 1, "id": "67ad55e868f711f0b6df4a53f6e8cf12",
             "name": "123", "pic_url": "/img/5d87689e68f511f0b6df4a53f6e8cf12.png"}
_OSD_STATE = {"enabled": True, "type": "image", "position": "Manual", "x": 1, "y": 1, "font_size": 1, "font_color": 2,
              "style": "bold", "outline": 0, "outline_color": 0, "text": "1234",
              "file_id": "5d87689e68f511f0b6df4a53f6e8cf12", "row_spacing": 1}
_kp3_tel("osd", "/api/codec/v1/vpss/osd", {"enable": False, "items": [_OSD_ITEM]},
         {"osd": {"enabled": False, "items": {"67ad55e868f711f0b6df4a53f6e8cf12": _OSD_STATE}}},
         state_before={"osd": {"items": {"1cd4b9f068f511f0b6df4a53f6e8cf12": {"type": "text"}}}})
_kp3_tel("osd-item", "/api/codec/v1/vpss/osd/67ad55e868f711f0b6df4a53f6e8cf12", _OSD_ITEM,
         {"osd": {"items": {"67ad55e868f711f0b6df4a53f6e8cf12": _OSD_STATE}}})
_kp3_tel("panel", "/api/codec/panel/settings",
         {"video_source": "sdi", "width": 1, "height": 1, "video_bitrate": 4000, "codec": "H264", "fps": "auto",
          "audio_source": "hdmi", "current_source": "hdmi", "gain": {"hdmi": 0, "sdi": 0, "linein": 0, "uvc": 0},
          "deinterlaceMode": "line_doubling"},
         {"panel": {"video_source": "sdi", "width": 1, "height": 1, "video_bitrate": 4000, "codec": "H264",
                    "fps": "auto", "audio_source": "hdmi", "current_source": "hdmi", "deinterlace": "line_doubling",
                    "gain": {"hdmi": 0, "sdi": 0, "linein": 0, "uvc": 0}}})

# ── Telemetry: read-back after a write answered ok ──
_KP3_OKS = _kp3_json(_KP3_OK_BODY)


def _kp3_rb(name, path, reads, request=None, body=_KP3_OKS):
    extra = {"request": request} if request is not None else {}
    _kp3_tel("readback-" + name, path, body, {},
             expect_then_send=[{"method": "GET", "target": r} for r in reads], **extra)


_kp3_rb("auth-info", "/api/systemctrl/user/auth_info", ["/api/systemctrl/user/auth_info"], {"enable": True})
_kp3_rb("users", "/api/systemctr/user/add", ["/api/systemctrl/user/list"])
_kp3_rb("time", "/api/systemctrl/systime/setTime", ["/api/systemctrl/systime/get"])
_kp3_rb("hostname", "/api/systemctrl/system/setHostname?name=P3123", ["/api/systemctrl/system/getHostname"])
_kp3_rb("web-protocols", "/api/systemctrl/system/setProtocol", ["/api/systemctrl/system/getProtocol"])
_kp3_rb("ssl-upload", "/api/systemctrl/system/ssl/crt", ["/api/systemctrl/system/ssl"])
_kp3_rb("production-test", "/api/systemctrl/system/production_off", ["/api/ststemctrl/system/production_stat"])
_kp3_rb("storage", "/api/record/record/choose_usb", ["/api/record/record/disk_list"])
_kp3_rb("recording-settings", "/api/record/record/settings", ["/api/record/record/get_record_info"])
_kp3_rb("recording", "/api/record/v1/get_recording_status", ["/api/record/v1/get_recording_status"], {"start": True})
_kp3_rb("streams", "/api/streamer/v1/stream/enable", ["/api/streamer/v1/stream/list"])
_kp3_rb("media-users", "/api/streamer/v1/stream/media_user", ["/api/streamer/v1/stream/media_user"],
        {"oper": "add", "username": "zhangyu", "password": "Admin123"})
_kp3_rb("media-user-deleted", "/api/streamer/v1/stream/media_user/zhangyu", ["/api/streamer/v1/stream/media_user"])
_kp3_rb("ptz", "/api/streamer/ptz/modify", ["/api/streamer/ptz/info"])
_kp3_rb("interfaces", "/api/networkmanager/network/SetSwitch",
        ["/api/networkmanager/network/GetLinkinfo", "/api/networkmanager/network/GetEthernets"],
        body=_kp3_json({"result": "ok", "data": []}))
_kp3_rb("traffic", "/api/networkmanager/network/traffic_data", ["/api/networkmanager/network/traffic_data"],
        {"traffic_keys": ["eth0"]}, body=_kp3_json({"result": "ok"}))
_kp3_rb("bind-network", "/api/networkmanager/network/bind_network", ["/api/networkmanager/network/current_networks"])
_kp3_rb("static-routes", "/api/networkmanager/static/routing/del", ["/api/networkmanager/static/routing/get"])
_kp3_rb("wifi", "/api/networkmanager/wifi/set",
        ["/api/networkmanager/wifi/get", "/api/networkmanager/wifi/getCurrentWifi"])
_kp3_rb("hotspot", "/api/networkmanager/hotspot/set", ["/api/networkmanager/hotspot/get"])
_kp3_rb("wifi-parameters", "/api/networkmanager/hotspot/setWIFIParameter",
        ["/api/networkmanager/hotspot/getWIFIParameter"])
_kp3_rb("modems", "/api/networkmanager/modem/modemEnable", ["/api/networkmanager/modem/get"])
_kp3_rb("bonding-streaming", "/api/kilolink/bonding/setting_stream", ["/api/kilolink/bonding/stream_state"])
_kp3_rb("bonding-server", "/api/kilolink/bonding/connect",
        ["/api/kilolink/bonding/state", "/api/kilolink/bonding/server"])
_kp3_rb("edid", "/api/codec/vin/edid/reset", ["/api/codec/vin/edid"])
_kp3_rb("video-source", "/api/codec/v1/vin/source",
        ["/api/codec/v1/vin/detail", "/api/codec/v1/venc/settings", "/api/codec/v1/audio/input"])
_kp3_rb("audio-input", "/api/codec/v1/audio/input/source", ["/api/codec/v1/audio/input"])
_kp3_rb("audio-encoders", "/api/codec/v1/audio/encode/3", ["/api/codec/v1/audio/encode/list"])
_kp3_rb("video-encoder", "/api/codec/v1/venc/bitrate", ["/api/codec/v1/venc/settings"])
_kp3_rb("images", "/api/codec/vpss/image/f232aece686c11f0b6df4a53f6e8cf12", ["/api/codec/vpss/image"])
_kp3_rb("still-frames", "/api/codec/vpss/static_frame/reset", ["/api/codec/vpss/static_frame"])
_kp3_rb("input-transform", "/api/codec/v1/vpss/input/rmf", ["/api/codec/v1/vpss/input/rmf"])
_kp3_rb("osd", "/api/codec/v1/vpss/osd/67ad55e868f711f0b6df4a53f6e8cf12", ["/api/codec/v1/vpss/osd"])
_kp3_rb("panel", "/api/codec/panel/settings", ["/api/codec/panel/settings"])
# A read is not a write: the status read (no body) asks for nothing more.
_kp3_tel("no-readback-on-read", "/api/record/v1/get_recording_status", {"result": "ok", "data": {"status": False}},
         {"recording": {"active": False}}, expect_then_send=[])
