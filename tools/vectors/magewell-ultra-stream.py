# Magewell Ultra Stream (magewell-ultra-stream): one vector per command, and
# telemetry vectors for every state rule. Requests are as the Ultra Stream API
# gives them: GET /usapi?method=<name>&<args> with values percent-encoded
# (RFC 3986 unreserved characters kept), JSON bodies for the file lists,
# multipart/form-data for uploads (the vendor's demo), and /cloud-api on port
# 8070. Passwords go as the MD5 of their text.
import hashlib as _us_hashlib

US = "magewell-ultra-stream"


def _us_md5(text):
    return _us_hashlib.md5(text.encode()).hexdigest()


def _us(command, input, query, model=None, **extra):
    v = {"spec": US, "command": command, "input": input,
         "expect_request": {"method": "GET", "target": "/usapi?method=" + query}, **extra}
    if model:
        v["model"] = model
    V.append(v)


def _us_post(command, input, query, body, model=None, **extra):
    v = {"spec": US, "command": command, "input": input,
         "expect_request": {"method": "POST", "target": "/usapi?method=" + query, "body": body}, **extra}
    if model:
        v["model"] = model
    V.append(v)


def _us_form(name, data, filename, content_type):
    """One-part multipart/form-data body; the boundary is the core's, derived
    from the part's name and bytes (SPEC.md §4, "Binary request bodies")."""
    digest = _us_hashlib.sha256(name.encode() + b"\0" + data + b"\0").hexdigest()
    boundary = "----form-" + digest[:16]
    body = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n"
            f"Content-Type: {content_type}\r\n\r\n").encode() + data + f"\r\n--{boundary}--\r\n".encode()
    return body.hex(), f"multipart/form-data; boundary={boundary}"


# Universal interfaces.
_us("ping", {}, "ping", http_reply={"status": 200, "body": '{"result":0,"cur-status":65552}'},
    expect_result={"ok": {"kind": "ack"}})
_us("get_info", {}, "get-info",
    http_reply={"status": 200, "body": '{"result":0,"product":{"sn":"C301181214002"}}'},
    expect_result={"ok": {"kind": "value", "value": {"result": 0, "product": {"sn": "C301181214002"}}}})
_us("get_status", {}, "get-status")
_us("get_settings", {}, "get-settings")
_us("get_signal_info", {}, "get-signal-info")
_us("get_report", {}, "get-report",
    http_reply={"status": 200, "body": '<div class="report-summary"><h2>DEVICE</h2></div>'},
    expect_result={"ok": {"kind": "value", "value": '<div class="report-summary"><h2>DEVICE</h2></div>'}})
_us("start_recording", {}, "start-rec")
_us("stop_recording", {}, "stop-rec", http_reply={"status": 200, "body": '{"result":0}'},
    expect_result={"ok": {"kind": "ack"}})
_us("start_live", {}, "start-live", file="start_live")
_us("start_live", {}, "start-live", file="start_live-hevc",
    http_reply={"status": 200, "body": '{"result":-32}'},
    expect_result={"error": {"error": "device_error", "code": "-32"}})
_us("stop_live", {}, "stop-live", file="stop_live")
_us("stop_live", {}, "stop-live", file="stop_live-repeat",
    http_reply={"status": 200, "body": '{"result":1}'},
    expect_result={"error": {"error": "device_error", "code": "1"}})
_us("clear_recording_status", {}, "clear-rec")
_us("clear_live_status", {}, "clear-live")

# General settings.
_us("set_device_name", {"name": "Stage Left (1)"}, "set-name&name=Stage%20Left%20%281%29")
_us("set_app_settings_allowed", {"allowed": False}, "set-app-settings&is-settings=0")
_us("enable_app_password", {"new_password": "pair1234"},
    "set-app-passwd&is-pass=1&new-pass=" + _us_md5("pair1234"))
_us("disable_app_password", {"password": "pair1234"},
    "set-app-passwd&is-pass=0&pass=" + _us_md5("pair1234"))
_us("change_app_password", {"password": "pair1234", "new_password": "pair5678"},
    "set-app-passwd&is-pass=1&pass=" + _us_md5("pair1234") + "&new-pass=" + _us_md5("pair5678"))
_us("complete_first_setup", {"name": "Booth", "timezone": "Europe/London", "auto_time": True},
    "set-first-over&name=Booth&enable-passwd=0&timezone=Europe%2FLondon&is-auto=1&time=")
_us("complete_first_setup_with_password",
    {"name": "Booth", "password": "pair1234", "timezone": "UTC", "auto_time": False, "time": "2026-10-23 15:10:00"},
    "set-first-over&name=Booth&enable-passwd=1&passwd=" + _us_md5("pair1234")
    + "&timezone=UTC&is-auto=0&time=2026-10-23%2015%3A10%3A00")
_us("set_recording_options",
    {"usb": "normal", "sd": "loop", "file_type": "mov", "folder": "Shows/Day 1", "file_name_mode": "time",
     "file_minutes": 30},
    "set-rec-control&usb-option=1&sd-option=2&mime-type=1&root-folder=Shows%2FDay%201&file-name-mode=1&time-unit=30")
_us("set_softap", {"ssid": "C301181214002", "password": "stream pass"},
    "set-softap&is-softap=1&is-visible=1&softap-ssid=C301181214002&softap-passwd=stream%20pass")
_us("set_date_time", {"timezone": "Asia/Shanghai", "auto_time": False, "time": "2019-09-10 15:00:00"},
    "set-date-time&timezone=Asia%2FShanghai&is-auto=0&time=2019-09-10%2015%3A00%3A00")
_us("set_stop_on_signal_loss", {"enabled": True}, "set-signal-lost-ctrl&is-signal-lost=1")
_us("set_ssdp", {"enabled": False}, "set-ssdp&is-ssdp=0")
_us("set_auto_record", {"enabled": True}, "set-auto-rec&is-auto-rec=1")
_us("set_volume", {"mic_enabled": True, "mic_gain": 12.5, "input_enabled": True, "input_gain": -6.0,
                   "headphone_enabled": False, "headphone_gain": 0.0},
    "set-volume&is-mic=1&mic-gain=12.50&is-spi=1&spi-gain=-6.00&is-phone=0&phone-gain=0.00")
_us("set_video_color", {"contrast": 110, "brightness": -5, "saturation": 100, "hue": 0},
    "set-video-color&contrast=110&brightness=-5&saturation=100&hue=0")
_us("set_low_latency_live", {"enabled": True}, "set-low-latency-live&is-low-latency=1")
_us("set_live_lock", {"locked": False}, "set-auto-live&is-auto-live=0")
_us("set_video_input_format", {"custom_color_space": True, "color_space": "bt709", "custom_quant_range": False,
                               "quant_range": "limited"},
    "set-video-input-format&is-color-fmt=1&color-fmt=3&is-quant-range=0&quant-range=2")
_us("set_video_output_format", {"custom_color_space": False, "color_space": "bt601", "custom_quant_range": True,
                                "quant_range": "full", "custom_sat_range": True, "sat_range": "extended"},
    "set-video-output-format&is-color-fmt=0&color-fmt=2&is-quant-range=1&quant-range=1&is-sat-range=1&sat-range=3")
_us("set_audio_sync_offset", {"ms": -40}, "set-sync-offset&audio-sync-offset=-40")
_IMG_HEX, _IMG_TYPE = _us_form("file", b"\xff\xd8\xff\xe0", "no-signal.jpg", "image/jpeg")
V.append({"spec": US, "command": "add_no_signal_image", "input": {"image": "/9j/4A=="},
          "expect_request": {"method": "POST", "target": "/usapi?method=add-nosignal-file", "body_hex": _IMG_HEX,
                             "content_type": _IMG_TYPE}})
_us("delete_no_signal_image", {"id": 2}, "del-nosignal-file&id=2")
_us("select_no_signal_image", {"id": 1}, "set-nosignal-file&id=1")
_us("set_no_signal_image_enabled", {"enabled": True}, "use-nosignal-file&use-nosignal-file=1")

# Storage devices.
_us("start_usb_format", {}, "start-format-usb")
_us("start_sd_format", {}, "start-format-sd")
_us("clear_usb_format_status", {}, "clear-format-usb")
_us("clear_sd_format_status", {}, "clear-format-sd")
_us("start_usb_test", {}, "start-test-usb")
_us("stop_usb_test", {}, "stop-test-usb")
_us("clear_usb_test_status", {}, "clear-test-usb")
_us("get_media_files", {"disk": "sd", "start": 0, "count": 3}, "get-media-files&disk-type=1&start=0&count=3")
_us_post("delete_media_files", {"disk": "sd", "files": ["VID_6_14.mp4", "VID_9.mp4"]}, "del-media-files",
         '{"disk-type":1,"media-files":["VID_6_14.mp4","VID_9.mp4"]}')

# Upload.
_FTP_Q = ("&net-mode=1&proto=0&url=ftp.example.com&port=21&dir-name=uploads&encryption=1&login-type=1"
          "&user-name=cam&passwd=p%40ss&mode=1")
_FTP_IN = {"network": "ethernet", "protocol": "ftp", "url": "ftp.example.com", "port": 21, "directory": "uploads",
           "encryption": "explicit-tls", "login": "normal", "user": "cam", "password": "p@ss", "mode": "passive"}
_us("add_upload_server_ftp", _FTP_IN, "send-file-add-server&type=0" + _FTP_Q)
_us("set_upload_server_ftp", {"id": 1, **_FTP_IN}, "send-file-set-server&id=1&type=0" + _FTP_Q)
_us("start_upload_server_test", _FTP_IN, "send-file-start-test-server&type=0" + _FTP_Q)
_OAUTH_IN = {"network": "wifi", "account_id": "dbid:AAB", "user": "Stage Crew", "access_token": "at.1",
             "refresh_token": "rt.1"}
_OAUTH_Q = "&net-mode=2&account-id=dbid%3AAAB&photo-path=&user-name=Stage%20Crew&access-token=at.1&refresh-token=rt.1"
_us("add_upload_server_google_drive", {**_OAUTH_IN, "directory": "Shows", "parents": "root"},
    "send-file-add-server&type=1" + _OAUTH_Q + "&dir-name=Shows&parents=root")
_us("set_upload_server_google_drive", {"id": 0, **_OAUTH_IN},
    "send-file-set-server&id=0&type=1" + _OAUTH_Q + "&dir-name=&parents=")
_us("add_upload_server_dropbox", {**_OAUTH_IN, "directory": "Shows"},
    "send-file-add-server&type=2" + _OAUTH_Q + "&dir-name=Shows")
_us("set_upload_server_dropbox", {"id": 3, **_OAUTH_IN}, "send-file-set-server&id=3&type=2" + _OAUTH_Q + "&dir-name=")
_us("add_upload_server_youtube", {**_OAUTH_IN, "privacy": "private"},
    "send-file-add-server&type=3" + _OAUTH_Q + "&privacy=private")
_us("set_upload_server_youtube", {"id": 2, **_OAUTH_IN, "privacy": "unlisted"},
    "send-file-set-server&id=2&type=3" + _OAUTH_Q + "&privacy=unlisted")
_us("delete_upload_server", {"id": 0}, "send-file-del-server&id=0")
_us("enable_upload_server", {"id": 1, "enabled": True}, "send-file-enable-server&id=1&is-use=1")
_us("stop_upload_server_test", {}, "send-file-stop-test-server")
_us("clear_upload_test_status", {}, "clear-test-send-file")
_us_post("add_upload_files", {"disk": "sd", "files": ["VID_6_14.mp4"]}, "send-file-add",
         '{"disk-type":1,"media-files":["VID_6_14.mp4"]}')
_us_post("remove_upload_files", {"disk": "usb", "files": ["VID_6_14.mp4"]}, "send-file-del",
         '{"disk-type":0,"media-files":["VID_6_14.mp4"]}')
_us("clear_upload_list", {}, "send-file-clear")
_us("get_upload_status", {"disk": "sd"}, "send-file-get-status&disk-type=1&start=0&count=0")
_us("set_auto_upload", {"enabled": False}, "send-file-set-is-auto&is-auto=0")
_us("start_upload", {}, "send-file-start")
_us("stop_upload", {}, "send-file-stop")

# Stream servers.
_us("add_stream_server_rtmp", {"type": "youtube", "name": "YouTube main", "url": "rtmp://a.rtmp.youtube.com/live2",
                               "key": "abcd-1234"},
    "add-server&type=2&name=YouTube%20main&url=rtmp%3A%2F%2Fa.rtmp.youtube.com%2Flive2&key=abcd-1234&is-auth=0"
    "&user=&passwd=&token=&event-data=&net-mode=1")
_us("add_stream_server_rtsp", {"name": "Local RTSP", "main": True, "main_name": "main", "sub": False,
                               "sub_name": "sub", "audio": True},
    "add-server&type=100&name=Local%20RTSP&port=554&is-main=1&main-stream-name=main&is-sub=0"
    "&sub-stream-name=sub&is-audio=1")
_us("set_stream_server_rtmp", {"id": 0, "type": "rtmp", "name": "CDN", "url": "192.168.1.123:345/live", "key": "aa",
                               "auth": True, "user": "op", "password": "pw", "network": "mobile"},
    "set-server&id=0&type=0&name=CDN&url=192.168.1.123%3A345%2Flive&key=aa&is-auth=1&user=op&passwd=pw&token="
    "&event-data=&net-mode=0")
_us("set_stream_server_rtsp", {"id": 4, "name": "RTSP", "port": 8554, "main": True, "main_name": "live",
                               "sub": True, "sub_name": "preview", "audio": False},
    "set-server&id=4&type=100&name=RTSP&port=8554&is-main=1&main-stream-name=live&is-sub=1"
    "&sub-stream-name=preview&is-audio=0")
_us("delete_stream_server", {"id": 1}, "del-server&id=1")
_us("enable_stream_server", {"id": 1, "enabled": True}, "enable-server&id=1&is-use=1")
_us("start_stream_test", {"type": "twitch", "url": "rtmp://live.twitch.tv/app", "key": "live_1",
                          "network": "wifi"},
    "start-test-server&type=1&url=rtmp%3A%2F%2Flive.twitch.tv%2Fapp&key=live_1&is-auth=0&user=&passwd=&token="
    "&net-mode=2")
_us("stop_stream_test", {}, "stop-test-server")
_us("clear_stream_test_status", {}, "clear-test-server")

# Encoding parameters.
_us("set_stream_assignment", {"record": "main", "live": "sub", "preview": "sub"}, "set-stream&rec=0&live=1&mws=1")
_us("set_main_stream_video", {"follow_input": False, "codec": "h264", "width": 1280, "height": 720,
                              "frame_interval": 333333, "kbps": 1024, "gop_seconds": 1, "profile": 2,
                              "stat_seconds": 60, "range": "full"},
    "set-video&stream=0&is-auto=0&cx=1280&cy=720&duration=333333&kbps=1024&gop=1&fourcc=0&profile=2&cbrstat=60"
    "&fullrange=1&codec=0")
_us("set_sub_stream_video", {"width": 640, "height": 360, "frame_interval": 400000, "kbps": 512, "gop_seconds": 2,
                             "profile": 1, "stat_seconds": 10, "range": "limited"},
    "set-video&stream=1&is-auto=0&cx=640&cy=360&duration=400000&kbps=512&gop=2&fourcc=0&profile=1&cbrstat=10"
    "&fullrange=0&codec=0")
_us("set_audio_bitrate", {"kbps": 128}, "set-audio&kbps=128")

# Reset and reboot, login.
_us("reboot", {}, "reboot")
_us("reset_all_settings", {}, "reset-all-settings")
_us("set_reset_key_enabled", {"enabled": False}, "set-enable-key-reset&enable-key-reset=0")
_us("set_reboot_key_enabled", {"enabled": True}, "set-enable-key-reboot&enable-key-reboot=1")
_us("logout", {}, "logout")

# Users.
_us("get_users", {}, "get-users")
_us("add_user", {"user": "Test", "password": "Test1"}, "add-user&id=Test&pass=" + _us_md5("Test1"))
_us("delete_user", {"user": "Test"}, "del-user&id=Test")
_us("change_password", {"password": "Admin", "new_password": "N3w"},
    "ch-password&pass=e3afed0047b08059d0fada10f400c1e5&new-pass=" + _us_md5("N3w"))
_us("set_user_password", {"user": "Test", "password": "Other"}, "set-password&id=Test&pass=" + _us_md5("Other"))

# Network.
_us("set_ethernet_dhcp", {}, "set-net&type=1&is-dhcp=1")
_us("set_ethernet_static", {"ip_address": "10.10.107.212", "netmask": "255.255.0.0", "gateway": "10.10.0.1",
                            "dns": "10.0.0.3"},
    "set-net&type=1&is-dhcp=0&ip=10.10.107.212&mask=255.255.0.0&router=10.10.0.1&dns=10.0.0.3")
_us("set_wifi_dhcp", {}, "set-net&type=0&is-dhcp=1")
_us("set_wifi_static", {"ip_address": "192.168.8.249", "netmask": "255.255.255.0", "gateway": "192.168.8.1",
                        "dns": "192.168.8.1"},
    "set-net&type=0&is-dhcp=0&ip=192.168.8.249&mask=255.255.255.0&router=192.168.8.1&dns=192.168.8.1")
_us("disconnect_wifi", {}, "disconn-wifi")
_us("open_softap", {}, "open-softap")
_us("close_softap", {}, "close-softap")

# Firmware.
_FW_HEX, _FW_TYPE = _us_form("file", b"MWF\x01", "ultra_stream_hdmi_rev_c_1_2_123.mwf", "application/octet-stream")
V.append({"spec": US, "command": "upload_firmware",
          "input": {"firmware": "TVdGAQ==", "filename": "ultra_stream_hdmi_rev_c_1_2_123.mwf"},
          "expect_request": {"method": "POST", "target": "/usapi?method=upload-update-file", "body_hex": _FW_HEX,
                             "content_type": _FW_TYPE},
          "http_reply": {"status": 200, "body": '{"status":0,"up-to-date":true,"version":"1.2.123","size":12494463}'},
          "expect_result": {"ok": {"kind": "value", "value": {"status": 0, "up-to-date": True, "version": "1.2.123",
                                                              "size": 12494463}}}})
_us("install_uploaded_firmware", {}, "update&mode=upload")
_us("install_online_firmware", {}, "update&mode=online")
_us("cancel_firmware_download", {}, "cancel-download")
_us("check_for_firmware_update", {}, "online-update-check")
_us("clear_firmware_update_status", {}, "clear-upgrade")
_us("clear_firmware_check_status", {}, "clear-check-update")
_us("set_auto_update_check", {"enabled": True}, "set-enable-check-update&is-check-update=1")

# Cloud, on port 8070.
V.append({"spec": US, "command": "register_cloud",
          "input": {"slot": 1, "code": "0427", "address": "10.10.8.233", "https": True},
          "expect_request": {"method": "GET", "port": 8070,
                             "target": "/cloud-api?method=cloud-reg-ex&id=1&cloud-code=0427&cloud-ip-addr=10.10.8.233"
                                       "&cloud-http-port=80&cloud-enable-https=1&cloud-https-port=443"}})
V.append({"spec": US, "command": "unregister_cloud", "input": {"slot": 0},
          "expect_request": {"method": "GET", "port": 8070, "target": "/cloud-api?method=cloud-unreg-ex&id=0"}})
V.append({"spec": US, "command": "get_cloud_status", "input": {},
          "expect_request": {"method": "GET", "port": 8070, "target": "/cloud-api?method=cloud-status&version=1"}})

# EDID (HDMI model).
_us("get_edid_config", {}, "get-edid-config")
_us("set_edid_config", {"smart_edid": True, "keep_last": False, "add_audio": True, "limit_pixel_clock": False},
    "set-edid-config&smart-edid=1&keep-last=0&add-audio=1&limit-pixel-clock=0")
_us("reset_edid", {}, "set-default-edid")
_EDID_HEX, _EDID_TYPE = _us_form("file", b"\x00\xff\xff\xff\xff\xff\xff\x00", "edid.bin",
                                 "application/octet-stream")
V.append({"spec": US, "command": "import_edid", "input": {"edid": "AP///////wA="},
          "expect_request": {"method": "POST", "target": "/usapi?method=import-edid", "body_hex": _EDID_HEX,
                             "content_type": _EDID_TYPE}})
_us("get_loop_through_edid", {}, "get-loop-through-edid")


# ── Telemetry ─────────────────────────────────────────────────────────────
def _ust(name, path, body, state, **extra):
    telemetry(US, name, inbound_http={"path": path, "body": body if isinstance(body, str) else json.dumps(body),
                                      **({"request": extra.pop("request")} if "request" in extra else {})},
              expect_state=state, **extra)


def _us_mask(mask):
    bits = {"first_boot": 0, "recording": 1, "live": 2, "usb_ready": 4, "rtmp_ready": 5, "softap": 6,
            "usb_testing": 12, "upgrading": 14, "stream_testing": 15, "password_set": 16, "app_locked": 17,
            "formatting_usb": 20, "formatting_sd": 21, "wifi_searching": 22, "wifi_connecting": 23,
            "checking_update": 25, "resetting": 26, "rebooting": 29, "upload_testing": 30}
    return {"mask": mask, **{k: bool(mask >> b & 1) for k, b in bits.items()}}


# ping: the status mask only (USB ready and password set: 0x10010).
_ust("ping", "/usapi?method=ping", {"result": 0, "cur-status": 65552}, {"status": _us_mask(65552)})
_ust("ping-failed", "/usapi?method=ping", {"result": -9, "cur-status": 65552}, {})

# get-status, as the document's examples give its parts; recording and live
# (0x06) with the USB drive ready.
_US_DISK = {"disk-status": 1, "total-size": 61638148096, "used-size": 3559489536, "free-size": 57944440832,
            "block-size": 32768, "usage": 6, "sys-path": "/dev/sda1", "fs-type": "vfat",
            "mount-path": "/usr/share/web-data/media/disk0", "fs-label": "C472-731D", "write-bps": 14255282,
            "read-bps": 0, "free-sec": 208038, "file-count": 47, "beign-time": "2019-09-24 08:04:39",
            "end-time": "2019-09-28 07:12:44", "total-cache-time": 0}
_US_STATUS = {
    "result": 0, "cur-status": 65558, "last-rec-status": 0, "cur-time": "2019-09-26 10:00:00",
    "box-name": "Ultra Stream C301181214002", "lock-user": [],
    "sysstat": {"cpu": 1500, "cpu-usage": 19, "eth-rx": 106248, "eth-tx": 4339448, "mem-free": 195612,
                "mem-total": 524288, "mobile-rx": 0, "mobile-tx": 0, "sd-rd": 0, "sd-wr": 0, "uptime": 331255,
                "usb-rd": 0, "usb-wr": 1782000, "wlan-rx": 0, "wlan-tx": 0},
    "input-signal": {"status": 1, "cx": 1920, "cy": 1080, "interlaced": 0, "frame-rate": 59.94,
                     "channel-valid": 3, "is-lpcm": 1, "bits-per-sample": 16, "sample-rate": 48000},
    "rec-status": {"result": 0, "run-ms": 65000, "cur-bps": 4300000, "avg-bps": 4200000, "client-id": ""},
    "live-status": {"result": 22, "run-ms": 30000, "cur-bps": 4100000, "avg-bps": 4000000, "net": 1,
                    "result2": 27, "cur-bps2": 0, "net2": 0, "client-id": ""},
    "living-test": {"upload-bps": 0, "percent": 0, "result": 27, "net": 0, "client-id": ""},
    "check-upgrade": {"result": 0, "client-id": ""},
    "upgrade": {"ver": "1.2.123", "date": "2012-1-1 00:00:00", "size-byte": 12004784,
                "info": [{"version": "1.2.123", "changelog": "## Develop version 1.2."}]},
    "upgrade-status": {"step": 0, "percent": 0, "result": 27, "client-id": "", "mode": "none"},
    "send-file": {"total-count-ongoing": 0, "total-count-done": 4, "disk-type": 1, "name": "", "result": 31,
                  "message": "", "left-time": 0, "percent": 0},
    "send-file-test": {"result": 27, "client-id": "", "id": 0},
    "usb": _US_DISK,
    "sd": {"disk-status": 0},
    "disk-test": {"read-bps": 0, "write-bps": 0, "percent": 0, "result": 27, "client-id": ""},
    "format-status": {"percent": 0, "result": 27, "client-id": ""},
    "wifi": {"name": "MWL1", "level": 3, "ip": "192.168.8.249", "mask": "255.255.255.0", "router": "192.168.8.1",
             "dns": "192.168.8.1"},
    "eth": {"ip": "10.10.107.212", "mask": "255.255.0.0", "router": "10.10.0.1", "dns": "10.0.0.3"},
    "mobile": {"ip": "", "mask": "", "router": "", "dns": ""},
}
_ust("status", "/usapi?method=get-status", _US_STATUS, {
    "status": {**_us_mask(65558), "last_recording_result": 0, "locked_by": "[]"},
    "device": {"time": "2019-09-26 10:00:00", "name": "Ultra Stream C301181214002"},
    "system": {"cpu": 1500, "cpu_usage": 19, "eth_rx": 106248, "eth_tx": 4339448, "memory_free": 195612,
               "memory_total": 524288, "mobile_rx": 0, "mobile_tx": 0, "sd_read": 0, "sd_write": 0,
               "uptime": 331255, "usb_read": 0, "usb_write": 1782000, "wlan_rx": 0, "wlan_tx": 0},
    "input": {"status": 1, "width": 1920, "height": 1080, "interlaced": False, "frame_rate": 59.94,
              "audio_channels_valid": 3, "audio_lpcm": True, "audio_bits": 16, "audio_sample_rate": 48000},
    "recording": {"result": 0, "run_ms": 65000, "bps": 4300000, "avg_bps": 4200000},
    "live": {"result": 22, "run_ms": 30000, "bps": 4100000, "avg_bps": 4000000, "network": 1, "result2": 27,
             "bps2": 0, "network2": 0},
    "stream_test": {"upload_bps": 0, "percent": 0, "result": 27, "network": 0},
    "firmware": {"check": {"result": 0},
                 "available": {"version": "1.2.123", "date": "2012-1-1 00:00:00", "size_bytes": 12004784,
                               "notes": '[{"version":"1.2.123","changelog":"## Develop version 1.2."}]'},
                 "update": {"step": 0, "percent": 0, "result": 27, "mode": "none"}},
    "upload": {"ongoing": 0, "done": 4, "disk": "sd", "file": "", "result": 31, "message": "", "seconds_left": 0,
               "percent": 0},
    "upload_test": {"result": 27, "server_id": 0},
    "usb_test": {"read_bps": 0, "write_bps": 0, "percent": 0, "result": 27},
    "format": {"percent": 0, "result": 27},
    "storage": {
        "usb": {"status": "valid", "status_code": 1, "total_bytes": 61638148096, "used_bytes": 3559489536,
                "free_bytes": 57944440832, "block_size": 32768, "usage_percent": 6, "device_path": "/dev/sda1",
                "filesystem": "vfat", "mount_path": "/usr/share/web-data/media/disk0", "label": "C472-731D",
                "write_bps": 14255282, "read_bps": 0, "free_seconds": 208038, "file_count": 47,
                "begin_time": "2019-09-24 08:04:39", "end_time": "2019-09-28 07:12:44", "cache_seconds": 0},
        "sd": {"status": "none", "status_code": 0}},
    "network": {"wifi": {"name": "MWL1", "level": 3, "ip_address": "192.168.8.249", "netmask": "255.255.255.0",
                         "gateway": "192.168.8.1", "dns": "192.168.8.1"},
                "ethernet": {"ip_address": "10.10.107.212", "netmask": "255.255.0.0", "gateway": "10.10.0.1",
                             "dns": "10.0.0.3"},
                "mobile": {"ip_address": "", "netmask": "", "gateway": "", "dns": ""}},
})
_ust("status-not-logged-in", "/usapi?method=get-status", {"result": -17}, {})

# get-settings, the document's example (less its lists, read below).
_US_SETTINGS = {
    "result": 0, "name": "Ultra Stream C301191111100", "passwd": 0, "is-settings": 1, "is-ssdp": 1,
    "is-startup-rec": 0, "is-startup-live": 1, "is-signal-lost": 0, "is-hdmi-rec-ctrl": 0,
    "is-auto-send-file": 0, "is-auto-rec": 0, "is-low-latency": 0, "is-auto-live": 0, "is-check-update": 1,
    "audio-sync-offset": -20, "enable-key-reset": 1, "enable-key-reboot": 1,
    "softap": {"is-softap": 1, "is-visible": 1, "softap-ssid": "C301181214002", "softap-passwd": "81214002"},
    "date-time": {"timezone": "UTC", "is-auto": 1},
    "rec-control": {"mime-type": 0, "usb-option": 1, "sd-option": 2, "root-folder": "REC_Folder",
                    "file-prefix": "VID", "time-unit": 5},
    "video-color": {"contrast": 100, "brightness": 0, "saturation": 100, "hue": 0},
    "volume": {"is-mic": 1, "mic-gain": 0, "is-spi": 1, "spi-gain": -6.5, "is-phone": 0, "phone-gain": 0},
    "rec-stream": 0, "live-stream": 1, "mws-stream": 1,
    "main-stream": {"is-auto": 0, "codec": 1, "cx": 1920, "cy": 1080, "duration": 166667, "kbps": 4096, "gop": 1,
                    "fourcc": 0, "profile": 0, "cbrstat": 60, "fullrange": 0},
    "sub-stream": {"cx": 1024, "cy": 768, "duration": 333333, "kbps": 1024, "gop": 1, "fourcc": 0, "profile": 2,
                   "cbrstat": 60, "fullrange": 1},
    "audio": {"sample-rate": 48000, "channels": 2, "kbps": 128},
    "eth": {"is-dhcp": 0, "ip": "10.10.107.212", "mask": "255.255.0.0", "router": "10.10.0.1", "dns": "10.0.0.3"},
    "wifi": {"is-dhcp": 1, "ip": "", "mask": "", "router": "", "dns": ""},
    "video-input-format": {"is-color-fmt": 0, "color-fmt": 1, "is-quant-range": 0, "quant-range": 1},
    "video-output-format": {"is-color-fmt": 0, "color-fmt": 3, "is-quant-range": 0, "quant-range": 2,
                            "is-sat-range": 1, "sat-range": 3},
    "use-nosignal-file": 1,
}
_US_SETTINGS_STATE = {
    "device": {"name": "Ultra Stream C301191111100"},
    "app": {"password_enabled": False, "settings_allowed": True},
    "network": {"ssdp": True,
                "ethernet": {"config": {"dhcp": False, "ip_address": "10.10.107.212", "netmask": "255.255.0.0",
                                        "gateway": "10.10.0.1", "dns": "10.0.0.3"}},
                "wifi": {"config": {"dhcp": True, "ip_address": "", "netmask": "", "gateway": "", "dns": ""}}},
    "control": {"startup_record": False, "startup_live": True, "stop_on_signal_loss": False,
                "hdmi_record_control": False, "auto_record": False, "live_lock": False, "low_latency_live": False},
    "upload": {"auto": False},
    "firmware": {"auto_check": True},
    "keys": {"reset_enabled": True, "reboot_enabled": True},
    "audio": {"sync_offset_ms": -20, "mic": {"enabled": True, "gain": 0.0}, "input": {"enabled": True, "gain": -6.5},
              "headphone": {"enabled": False, "gain": 0.0}, "sample_rate": 48000, "channels": 2, "kbps": 128},
    "softap": {"enabled": True, "visible": True, "ssid": "C301181214002"},
    "time": {"timezone": "UTC", "auto": True},
    "recording": {"options": {"file_type": "mp4", "usb": "normal", "sd": "loop", "folder": "REC_Folder",
                              "file_prefix": "VID", "file_minutes": 5}},
    "video": {"contrast": 100, "brightness": 0, "saturation": 100, "hue": 0,
              "input_format": {"custom_color_space": False, "color_space": "rgb", "custom_quant_range": False,
                               "quant_range": "full"},
              "output_format": {"custom_color_space": False, "color_space": "bt709", "custom_quant_range": False,
                                "quant_range": "limited", "custom_sat_range": True, "sat_range": "extended"}},
    "streams": {"recording": "main", "live": "sub", "preview": "sub"},
    "encoder": {"main": {"follow_input": False, "codec": "hevc", "width": 1920, "height": 1080,
                         "frame_interval": 166667, "kbps": 4096, "gop_seconds": 1, "profile": 0, "stat_seconds": 60,
                         "range": "limited"},
                "sub": {"width": 1024, "height": 768, "frame_interval": 333333, "kbps": 1024, "gop_seconds": 1,
                        "profile": 2, "stat_seconds": 60, "range": "full"}},
    "no_signal": {"enabled": True},
}
_ust("settings", "/usapi?method=get-settings", _US_SETTINGS, _US_SETTINGS_STATE)

# get-settings' lists, each replaced whole: a server and an image removed on
# the device since the last read disappear.
_ust("settings-lists", "/usapi?method=get-settings", {
    "result": 0,
    "stream-server": [
        {"id": 0, "type": 0, "url": "192.168.1.123:345/live", "key": "aa", "is-auth": 1, "user": "op",
         "passwd": "pw", "is-use": 1, "token": "", "net-mode": 1, "name": "192.168.1.123"},
        {"id": 2, "type": 100, "name": "Local RTSP", "port": 554, "is-main": 1, "main-stream-name": "main",
         "is-sub": 0, "sub-stream-name": "sub", "is-audio": 1, "is-use": 0, "net-mode": 1}],
    "send-file-cloud": [
        {"id": 0, "is-use": 1, "type": 2, "account-id": "dbid:AABnr7BK6RO5", "photo-path": "", "user-name": "qin qiu",
         "access-token": "secret", "refresh-token": "secret", "dir-name": "()", "net-mode": 2},
        {"id": 1, "is-use": 0, "type": 0, "proto": 1, "url": "ftp.example.com", "port": 22, "dir-name": "up",
         "encryption": 0, "login-type": 1, "user-name": "cam", "passwd": "secret", "mode": 1, "net-mode": 1}],
    "nosignal-files": [
        {"id": 0, "is-use": 0, "is-edit": 0, "file-path": "/no-signal/default0.jpg", "time": 0},
        {"id": 1, "is-use": 1, "is-edit": 1, "file-path": "/no-signal/user1.jpg", "time": 1569393321}]},
    state_before={"stream_servers": {"5": {"name": "Old", "type": "rtmp"}},
                  "no_signal": {"images": {"3": {"path": "/no-signal/gone.jpg"}}}},
    state={
        "stream_servers": {
            "0": {"type": "rtmp", "name": "192.168.1.123", "url": "192.168.1.123:345/live", "auth": True,
                  "user": "op", "enabled": True, "network": "ethernet"},
            "2": {"type": "rtsp", "name": "Local RTSP", "enabled": False, "network": "ethernet", "port": 554,
                  "main": True, "main_name": "main", "sub": False, "sub_name": "sub", "audio": True}},
        "upload_servers": {
            "0": {"type": "dropbox", "enabled": True, "network": "wifi", "account_id": "dbid:AABnr7BK6RO5",
                  "user": "qin qiu", "photo_path": "", "directory": "()"},
            "1": {"type": "ftp", "enabled": False, "network": "ethernet", "user": "cam", "directory": "up",
                  "protocol": "sftp", "url": "ftp.example.com", "port": 22, "encryption": "plain",
                  "login": "normal", "mode": "passive"}},
        "no_signal": {"images": {
            "0": {"selected": False, "editable": False, "path": "/no-signal/default0.jpg", "time": 0},
            "1": {"selected": True, "editable": True, "path": "/no-signal/user1.jpg", "time": 1569393321}}}})

# get-info, from the document's example.
_US_INFO = {
    "result": 0, "mac-addr": {"eth": "70:b3:d5:75:d5:fc", "wifi": "70:b3:d5:75:d5:fd", "blue": "70:b3:d5:75:d5:fe"},
    "snapshot": "/tmp/sbox-snapshot/sbox-quarter.jpg",
    "product": {"sn": "C301181214002", "product-id": 769, "hardware-ver": "C", "firmware-id": 0,
                "firmware-ver-s": "1.2.122", "factory-firmware-ver-s": "1.2.115", "product-name": "Ultra Stream",
                "module-name": "Ultra Stream HDMI", "manu-name": "MAGEWELL", "features": 1, "max-lock-count": 2},
    "nosignal": {"max-count": 4, "max-width": 1920, "max-height": 1080, "max-size-kb": 512},
    "audio-range": {"hdmi": {"max": 6.00, "min": -100.00, "def": 0.00}, "mic": {"max": 55.25, "min": -12.00, "def": 0.00},
                    "phone": {"max": 6.00, "min": -57.00, "def": 0.00}},
    "codec-cap": {"resolutions": [{"w": 1280, "h": 720}, {"w": 1920, "h": 1080}],
                  "durations": [{"name": "30 FPS", "value": 333333}],
                  "profile": [{"name": "High profile", "value": 2}], "hevc-profile": [{"name": "Main profile", "value": 0}],
                  "video-kbps": [{"name": "4 Mbps", "value": 4096}], "audio-kbps": [{"name": "128 Kbps", "value": 128}],
                  "gop-sec": [{"name": " 1 sec", "value": 1}],
                  "video-range": [{"name": "Full range (0-255)", "value": 1}],
                  "stat-sec": [{"name": "60 sec", "value": 60}], "video-codec": [{"name": "HEVC", "value": 1}]},
    "rec-control": {"usb-option": [{"name": "Ordinary recording", "value": 1}],
                    "sd-option": [{"name": "Loop recording", "value": 2}],
                    "time-unit": [{"name": "5 minutes", "value": 5}], "file-ext": [{"name": "mov", "value": 1}]},
    "color-range": {"contrast": {"max": 200, "min": 50, "def": 100}, "brightness": {"max": 100, "min": -100, "def": 0},
                    "saturation": {"max": 200, "min": 0, "def": 100}, "hue": {"max": 90, "min": -90, "def": 0}},
    "video-format": {"input-color-fmt": [{"name": "RGB", "value": 1}], "output-color-fmt": [{"name": "YUV BT.709", "value": 3}],
                     "quant-range": [{"name": "Limited range (16-235)", "value": 2}],
                     "sat-range": [{"name": "Extended GAMUT range (1-254)", "value": 3}]},
    "send-file": {"server": [{"name": "Dropbox", "value": 2}], "ftp-proto": [{"name": "SFTP - SSH File Transfer Protocol", "value": 1}],
                  "ftp-encryption": [{"name": "Require implicit FTP over TLS", "value": 2}],
                  "ftp-transfer-mode": [{"name": "Passive", "value": 1}]},
}


def _us_compact(v):
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


_cc, _rc, _vf, _sf = _US_INFO["codec-cap"], _US_INFO["rec-control"], _US_INFO["video-format"], _US_INFO["send-file"]
_ust("info", "/usapi?method=get-info", _US_INFO, {
    "device": {"serial": "C301181214002", "product_id": 769, "hardware_revision": "C", "firmware_id": 0,
               "firmware": "1.2.122", "factory_firmware": "1.2.115", "product": "Ultra Stream",
               "model": "Ultra Stream HDMI", "manufacturer": "MAGEWELL", "features": 1, "max_app_locks": 2,
               "mac": {"ethernet": "70:b3:d5:75:d5:fc", "wifi": "70:b3:d5:75:d5:fd", "bluetooth": "70:b3:d5:75:d5:fe"},
               "snapshot_path": "/tmp/sbox-snapshot/sbox-quarter.jpg"},
    "capabilities": {
        "no_signal": {"max_count": 4, "max_width": 1920, "max_height": 1080, "max_size_kb": 512},
        "audio": {"input": {"min": -100.0, "max": 6.0, "default": 0.0},
                  "mic": {"min": -12.0, "max": 55.25, "default": 0.0},
                  "headphone": {"min": -57.0, "max": 6.0, "default": 0.0}},
        "color": {"contrast": {"min": 50, "max": 200, "default": 100},
                  "brightness": {"min": -100, "max": 100, "default": 0},
                  "saturation": {"min": 0, "max": 200, "default": 100},
                  "hue": {"min": -90, "max": 90, "default": 0}},
        "encoding": {"resolutions": _us_compact(_cc["resolutions"]), "frame_intervals": _us_compact(_cc["durations"]),
                     "profiles": _us_compact(_cc["profile"]), "hevc_profiles": _us_compact(_cc["hevc-profile"]),
                     "video_kbps": _us_compact(_cc["video-kbps"]), "audio_kbps": _us_compact(_cc["audio-kbps"]),
                     "gop_seconds": _us_compact(_cc["gop-sec"]), "ranges": _us_compact(_cc["video-range"]),
                     "stat_seconds": _us_compact(_cc["stat-sec"]), "codecs": _us_compact(_cc["video-codec"])},
        "recording": {"usb_options": _us_compact(_rc["usb-option"]), "sd_options": _us_compact(_rc["sd-option"]),
                      "file_minutes": _us_compact(_rc["time-unit"]), "file_types": _us_compact(_rc["file-ext"])},
        "video_format": {"input_color_spaces": _us_compact(_vf["input-color-fmt"]),
                         "output_color_spaces": _us_compact(_vf["output-color-fmt"]),
                         "quant_ranges": _us_compact(_vf["quant-range"]), "sat_ranges": _us_compact(_vf["sat-range"])},
        "upload": {"servers": _us_compact(_sf["server"]), "ftp_protocols": _us_compact(_sf["ftp-proto"]),
                   "ftp_encryption": _us_compact(_sf["ftp-encryption"]),
                   "ftp_transfer_modes": _us_compact(_sf["ftp-transfer-mode"])}},
})

# get-users, replaced whole.
_ust("users", "/usapi?method=get-users",
     {"result": 0, "users": [{"id": "Admin", "type": 1}, {"id": "Test.1", "type": 2}]},
     {"users": {"Admin": {"role": "admin"}, "Test.1": {"role": "user"}}},
     state_before={"users": {"Gone": {"role": "user"}}})

# EDID.
_US_EDID = "AP///////wA09wEAAQAAAAEaAQOAAAB4Au6Vo1RMmSYPUFT//4AxQEVAYUBxQIGA0QDhwAEACOgAMPJwWoCwWIoAUB10AAAe"
_ust("edid-config", "/usapi?method=get-edid-config",
     {"result": 0, "smart-edid": 1, "keep-last": 0, "add-audio": 1, "limit-pixel-clock": 1, "data": _US_EDID},
     {"edid": {"smart": True, "keep_last": False, "add_audio": True, "limit_pixel_clock": True, "data": _US_EDID}})
_ust("edid-default", "/usapi?method=set-default-edid", {"result": 0, "data": _US_EDID},
     {"edid": {"data": _US_EDID}},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-edid-config"}])
_ust("edid-loop-through", "/usapi?method=get-loop-through-edid", {"result": 0, "data": _US_EDID},
     {"edid": {"loop_through_data": _US_EDID}})

# cloud-status (port 8070), the document's example.
_ust("cloud-status", "/cloud-api?method=cloud-status&version=1", {
    "device_id": "A305200908004", "number": 2, "version": 1, "result": 0,
    "status": [{"cloud-code": "", "cloud-date": 0, "cloud-enable-https": 0, "cloud-http-port": 80,
                "cloud-https-port": 443, "cloud-ip-addr": "10.0.1.32", "cloud-reg-status": 101, "cloud-status": 35,
                "id": 0, "is-cloud-set": 1},
               {"cloud-code": "0427", "cloud-date": 0, "cloud-enable-https": 1, "cloud-http-port": 80,
                "cloud-https-port": 443, "cloud-ip-addr": "10.10.8.233", "cloud-reg-status": 103,
                "cloud-status": 36, "id": 1, "is-cloud-set": 1}]},
    {"cloud": {"device_id": "A305200908004", "slots": 2, "platforms": {
        "0": {"code": "", "date": 0, "https": False, "http_port": 80, "https_port": 443, "address": "10.0.1.32",
              "registration": "accepted", "connection": "online", "configured": True},
        "1": {"code": "0427", "date": 0, "https": True, "http_port": 80, "https_port": 443,
              "address": "10.10.8.233", "registration": "waiting", "connection": "offline", "configured": True}}}})

# get-media-files: the first page replaces the list.
_ust("media-usb", "/usapi?method=get-media-files&disk-type=0&start=0&count=100", {
    "result": 0, "path": "/media/disk0/REC_Folder", "thumbnail": "thumbnail",
    "media-files": [{"name": "VID_9.mp4", "thumbnail-name": "VID_9.mp4_1569393321", "status": 1,
                     "create-time": "2019-09-25 06:35:21", "size-bytes": 4025670, "duration": 14480,
                     "width": 1280, "height": 720, "interval": 200000, "codec": 0}]},
    {"media": {"usb": {"path": "/media/disk0/REC_Folder", "thumbnail_dir": "thumbnail", "files": {
        "VID_9.mp4": {"thumbnail": "VID_9.mp4_1569393321", "status": "normal", "created": "2019-09-25 06:35:21",
                      "size_bytes": 4025670, "duration": 14480, "width": 1280, "height": 720,
                      "frame_interval": 200000, "codec": "h264"}}}}},
    state_before={"media": {"usb": {"files": {"VID_1.mp4": {"status": "normal"}}}}})
# A later page adds to the list.
_ust("media-sd-page", "/usapi?method=get-media-files&disk-type=1&start=100&count=100", {
    "result": 0, "path": "/media/disk1/REC_Folder", "thumbnail": "thumbnail",
    "media-files": [{"name": "VID_200.mov", "thumbnail-name": "VID_200.mov_1", "status": 0,
                     "create-time": "2019-09-26 08:00:00", "size-bytes": 1000, "duration": 2, "width": 3840,
                     "height": 2160, "interval": 166667, "codec": 1}]},
    {"media": {"sd": {"path": "/media/disk1/REC_Folder", "thumbnail_dir": "thumbnail", "files": {
        "VID_1.mov": {"status": "normal"},
        "VID_200.mov": {"thumbnail": "VID_200.mov_1", "status": "recording", "created": "2019-09-26 08:00:00",
                        "size_bytes": 1000, "duration": 2, "width": 3840, "height": 2160,
                        "frame_interval": 166667, "codec": "hevc"}}}}},
    state_before={"media": {"sd": {"files": {"VID_1.mov": {"status": "normal"}}}}})

# upload-update-file: the uploaded file, then the status read again.
_ust("firmware-upload", "/usapi?method=upload-update-file",
     {"status": 0, "up-to-date": False, "version": "1.2.123", "size": 12494463},
     {"firmware": {"upload": {"up_to_date": False, "version": "1.2.123", "size_bytes": 12494463}}},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-status"}])

# Re-reads after a change.
_ust("reread-settings", "/usapi?method=set-video&stream=0&is-auto=0&cx=1280&cy=720&duration=333333&kbps=1024"
     "&gop=1&fourcc=0&profile=2&cbrstat=60&fullrange=1&codec=0", {"result": 0}, {},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-settings"}])
_ust("reread-settings-refused", "/usapi?method=set-name&name=x", {"result": -28}, {},
     expect_then_send=[])
_ust("reread-status", "/usapi?method=start-live", {"result": 0}, {},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-status"}])
_ust("reread-live-lock", "/usapi?method=set-auto-live&is-auto-live=1", {"result": 0}, {},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-settings"},
                       {"method": "GET", "target": "/usapi?method=get-status"}])
_ust("reread-first-setup", "/usapi?method=set-first-over&name=Booth&enable-passwd=0&timezone=UTC&is-auto=1&time=",
     {"result": 0}, {},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-settings"},
                       {"method": "GET", "target": "/usapi?method=get-status"}])
_ust("reread-edid", "/usapi?method=set-edid-config&smart-edid=1&keep-last=0&add-audio=1&limit-pixel-clock=0",
     {"result": 0}, {}, expect_then_send=[{"method": "GET", "target": "/usapi?method=get-edid-config"}])
_ust("reread-users", "/usapi?method=add-user&id=Test&pass=" + _us_md5("Test1"), {"result": 0}, {},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-users"}])
_ust("reread-cloud", "/cloud-api?method=cloud-unreg-ex&id=0", {"result": 0}, {},
     expect_then_send=[{"method": "GET", "port": 8070, "target": "/cloud-api?method=cloud-status&version=1"}])
_ust("reread-media", "/usapi?method=del-media-files", {"result": 0}, {},
     request={"disk-type": 1, "media-files": ["VID_9.mp4"]},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-media-files&disk-type=1&start=0&count=100"},
                       {"method": "GET", "target": "/usapi?method=get-status"}])
