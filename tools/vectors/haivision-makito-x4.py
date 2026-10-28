# Haivision Makito X4 encoder and decoder (haivision-makito-x4): the REST
# API under /apis as the units' API references give it (encoder 1.10.0,
# decoder 1.7.0), JSON bodies in the references' field names, written here
# compact. Every write is followed by a read of the list it changed; only the
# write is stated. The session login is answered by the vector runner.
# Telemetry from the references' example replies, shortened.
HMX = "haivision-makito-x4"
_HMX_SET = {"password": "Secret-123"}
_HMX_DEC = "makito-x4-decoder"
_HMX_RUG = "makito-x4-rugged-encoder"


def _hmx_j(obj):
    return json.dumps(obj, separators=(",", ":"))


def _hmx(command, input, method, target, body=None, model=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body if isinstance(body, str) else _hmx_j(body)
    v = {"spec": HMX, "command": command, "input": input, "settings": _HMX_SET, "expect_request": request, **extra}
    if model:
        v["model"] = model
    V.append(v)


def _hmx_form(parts):
    """multipart/form-data as the core writes it (SPEC.md §4): the boundary
    derived from the parts' names and bytes."""
    import hashlib
    digest = hashlib.sha256(b"".join(n.encode() + b"\0" + d + b"\0" for n, d, _, _ in parts)).hexdigest()
    boundary = "----form-" + digest[:16]
    body = b""
    for name, data, filename, ctype in parts:
        body += f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"".encode()
        if filename:
            body += f"; filename=\"{filename}\"\r\nContent-Type: {ctype}".encode()
        body += b"\r\n\r\n" + data + b"\r\n"
    body += f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


def _hmx_up(command, input, target, parts, model=None, **extra):
    body, ctype = _hmx_form(parts)
    v = {"spec": HMX, "command": command, "input": input, "settings": _HMX_SET,
         "expect_request": {"method": "POST", "target": target, "body_hex": body.hex(), "content_type": ctype}, **extra}
    if model:
        v["model"] = model
    V.append(v)


_OS = "application/octet-stream"

# ── Reads ────────────────────────────────────────────────────────────────
for _c, _t in [("get_session", "/apis/authentication"), ("get_status", "/apis/status"),
               ("get_system_info", "/apis/system_info"), ("get_messages", "/apis/messages"),
               ("get_leds", "/apis/leds"), ("list_accounts", "/apis/accounts"), ("get_policies", "/apis/policies"),
               ("get_audit", "/apis/audit"), ("get_banner", "/apis/banner"), ("list_ca_certificates", "/apis/ca"),
               ("list_identity_certificates", "/apis/identity"), ("get_license_features", "/apis/license"),
               ("list_licenses", "/apis/licenses"), ("get_upgrade_state", "/apis/upgrade"),
               ("get_network", "/apis/network"), ("get_datetime", "/apis/datetime"), ("get_nmos", "/apis/nmos"),
               ("get_services", "/apis/services"), ("get_services_status", "/apis/services/status"),
               ("get_ems", "/apis/ems"), ("get_preview_service", "/apis/services/preview"),
               ("list_presets", "/apis/presets"), ("list_streams", "/apis/streams"),
               ("list_video_encoders", "/apis/videnc"), ("list_downscale_resolutions", "/apis/videnc/0/resolutions"),
               ("list_video_inputs", "/apis/vidin"), ("list_audio_encoders", "/apis/audenc"),
               ("list_metadata_sources", "/apis/metadata"), ("list_ad_insertion_sources", "/apis/scte"),
               ("list_essences", "/apis/essences"), ("list_essence_resolutions", "/apis/essences/resolutions"),
               ("list_snapshots", "/apis/snapshots"), ("get_snapshot_settings", "/apis/snapshotsettings"),
               ("list_logos", "/apis/logos"), ("list_logo_settings", "/apis/logosettings")]:
    _hmx(_c, {}, "GET", _t)
_hmx("get_comport", {}, "GET", "/apis/comport", model=_HMX_RUG)
_hmx("list_decoders", {}, "GET", "/apis/decoders", model=_HMX_DEC)
_hmx("list_st2110_outputs", {}, "GET", "/apis/st2110", model=_HMX_DEC)
_hmx("get_status", {}, "GET", "/apis/status", file="get_status-reply",
     http_reply={"status": 200, "body": '{"cardStatus":"OK","firmwareVersion":"1.7.0-43","temperature":"53"}'},
     expect_result={"ok": {"kind": "value", "value": {"cardStatus": "OK", "firmwareVersion": "1.7.0-43",
                                                      "temperature": "53"}}})
_hmx("get_status", {}, "GET", "/apis/status", file="get_status-error",
     http_reply={"status": 500, "body": "Error"}, expect_result={"error": {"error": "device_error", "code": "500"}})
for _c, _t, _k, _v in [("get_ca_certificate", "/apis/ca/new%20certificate%202020", "id", "new certificate 2020"),
                       ("get_identity_certificate", "/apis/identity/srv1", "id", "srv1"),
                       ("get_stream", "/apis/streams/3", "id", 3), ("get_video_encoder", "/apis/videnc/0", "id", 0),
                       ("get_video_input", "/apis/vidin/1", "id", 1), ("get_audio_encoder", "/apis/audenc/2", "id", 2),
                       ("get_metadata_source", "/apis/metadata/1", "id", 1),
                       ("get_ad_insertion_source", "/apis/scte/1", "id", 1), ("get_recording", "/apis/recordings/1", "id", 1)]:
    _hmx(_c, {_k: _v}, "GET", _t)
_hmx("get_decoder", {"id": 0}, "GET", "/apis/decoders/0", model=_HMX_DEC)
_hmx("get_license", {"name": "HAI-032004020015.lic"}, "GET", "/apis/licenses/HAI-032004020015.lic",
     http_reply={"status": 200, "body": "[AUDENC]\nQty=4\n"},
     expect_result={"ok": {"kind": "value", "value": "[AUDENC]\nQty=4"}})
_hmx("get_upgrade_progress", {"filename": "makitox4_enc_v1.9.0-40.hai"}, "GET", "/makitox4_enc_v1.9.0-40.hai")
_hmx("get_stream_srt_csv", {"id": 4, "interval": "last5min"}, "GET", "/apis/streams/4/stats?type=srt&interval=last5min")
_hmx("get_stream_srt_csv_path", {"id": 4, "interval": "last60min", "path": 2}, "GET",
     "/apis/streams/4/stats?type=srt&interval=last60min&redundancyPath=2")
_hmx("check_preset_saved", {}, "PUT", "/apis/presets/current/issaved",
     http_reply={"status": 200, "body": '{"isSaved":false}'}, expect_result={"ok": {"kind": "value", "value": {"isSaved": False}}})
_hmx("check_preset_saved", {}, "GET", "/apis/presets/current/issaved", model=_HMX_DEC, file="check_preset_saved-decoder")
_hmx("reboot", {}, "POST", "/apis/reboot", http_reply={"status": 200, "body": '{"upgrade":0}'},
     expect_result={"ok": {"kind": "value", "value": {"upgrade": 0}}})

# ── System ───────────────────────────────────────────────────────────────
_hmx("set_device_name", {"name": "Truck 4 encoder"}, "PUT", "/apis/system_info", {"deviceName": "Truck 4 encoder"})
_hmx("set_analytics", {"analytics": "declined"}, "PUT", "/apis/system_info", {"analytics": "declined"})
_hmx("set_personality", {"personality": "2110"}, "PUT", "/apis/system_info", {"personality": "2110"})
_hmx("set_decoder_personality", {"personality": "SDI"}, "PUT", "/apis/system_info", {"personality": "SDI"}, model=_HMX_DEC)
_hmx("clear_messages", {}, "DELETE", "/apis/messages")
_hmx("blink_leds", {"duration": 5}, "POST", "/apis/leds", {"duration": 5})
_hmx("stop_blinking_leds", {}, "DELETE", "/apis/leds")

# ── Accounts and security ───────────────────────────────────────────────
_hmx("create_account", {"username": "john", "role": "Operator", "password": "32rFE2f0343_8989"}, "POST", "/apis/accounts",
     {"name": "john", "role": "Operator", "password": "32rFE2f0343_8989", "passwordConfirm": "32rFE2f0343_8989"})
_hmx("change_password", {"username": "john", "password": "haivision"}, "PUT", "/apis/accounts/john",
     {"oldPassword": "", "password": "haivision", "passwordConfirm": "haivision"})
_hmx("delete_account", {"username": "john"}, "DELETE", "/apis/accounts/john")
for _c in ("enable", "lock", "unlock"):
    _hmx(f"{_c}_account", {"username": "john"}, "PUT", f"/apis/accounts/john/{_c}")
_hmx_up("upload_public_key", {"username": "john", "key": "c3NoLXJzYQ==", "filename": "id_rsa.pub"},
        "/apis/accounts/john/pubkeys", [("file", b"ssh-rsa", "id_rsa.pub", _OS)])
_hmx("delete_public_key", {"username": "john", "filename": "id_rsa.pub"}, "DELETE", "/apis/accounts/john/pubkeys/id_rsa.pub")
_HMX_POL = {"minLen": 6, "quality": "Strong", "uppercase": 1, "digits": 1, "symbols": 1, "rememberedPasswords": 5,
            "minimumLifetime": 7, "expiration": True, "expirationDays": 90, "hsts": True, "autoLogout": False,
            "autoLogoutTime": 20, "limitPwdRetries": False, "limitPwdRetriesMaxAttempts": 3,
            "limitPwdRetriesInterval": 15, "inactiveLock": False, "inactiveLockTime": 90, "compliance": "FIPS140",
            "ssl3": False, "tls1": True, "tls1_1": True, "tls1_2": True}
_hmx("configure_policies", {"policies": _HMX_POL}, "PUT", "/apis/policies", _HMX_POL)
_hmx("configure_audit", {"enabled": True, "transport": "TLS", "server": "10.65.10.173", "trusted": "Self-signed",
                         "fingerprint": "SHA1:AD:4C:D2:94"}, "PUT", "/apis/audit",
     {"enabled": 1, "transport": "TLS", "server": "10.65.10.173", "trusted": "Self-signed", "fingerprint": "SHA1:AD:4C:D2:94"})
_hmx("configure_audit", {"enabled": False, "transport": "UDP", "server": "10.65.10.173"}, "PUT", "/apis/audit",
     {"enabled": 0, "transport": "UDP", "server": "10.65.10.173", "trusted": "All", "fingerprint": ""}, file="configure_audit-udp")
_hmx_up("upload_banner", {"file": "SGVsbG8="}, "/apis/banner", [("file", b"Hello", "banner.txt", _OS)])
_hmx("set_banner_enabled", {"enabled": False}, "PUT", "/apis/banner", {"enabled": False})
_hmx("delete_banner", {}, "DELETE", "/apis/banner")
_hmx_up("upload_ca_certificate", {"file": "LS0tLS0=", "name": "chicago-server-ca"}, "/apis/ca",
        [("name", b"chicago-server-ca", None, None), ("type", b"ca", None, None), ("infmt", b"auto", None, None),
         ("password", b"", None, None), ("file", b"-----", "chicago-server-ca", _OS)])
_hmx("delete_ca_certificate", {"id": "new certificate 2020"}, "DELETE", "/apis/ca/new%20certificate%202020")
_hmx("set_default_ca_certificate", {"id": "root"}, "PUT", "/apis/ca/root/default")
_hmx("generate_identity_certificate", {"name": "new certificate 2020", "sign": "self",
                                       "subject": "/C=US/ST=Illinois/CN=localhost.localdomain"}, "POST", "/apis/identity",
     {"name": "new certificate 2020", "sign": "self", "subject": "/C=US/ST=Illinois/CN=localhost.localdomain"})
_hmx_up("upload_identity_certificate", {"file": "MIIB", "name": "srv1", "format": "p12", "file_password": "pw"},
        "/apis/identity", [("name", b"srv1", None, None), ("type", b"id", None, None), ("infmt", b"p12", None, None),
                           ("password", b"pw", None, None), ("file", bytes.fromhex("308201"), "srv1", _OS)])
_hmx("delete_identity_certificate", {"id": "srv1"}, "DELETE", "/apis/identity/srv1")
_hmx("set_default_identity_certificate", {"id": "srv1"}, "PUT", "/apis/identity/srv1/default")

# ── Licenses and upgrade ─────────────────────────────────────────────────
_hmx_up("upload_license", {"file": "W0FWSU5d", "filename": "HAI-032004020015.lic"}, "/apis/licenses",
        [("file", b"[AVIN]", "HAI-032004020015.lic", _OS)])
_hmx("delete_license", {"name": "HAI-032004020015.lic"}, "DELETE", "/apis/licenses/HAI-032004020015.lic")
_hmx_up("upload_upgrade_package", {"package": "SEFJ", "filename": "makitox4_enc_v1.9.0-40.hai"}, "/apis/upgrade",
        [("file", b"HAI", "makitox4_enc_v1.9.0-40.hai", _OS)])
_hmx_up("upload_upgrade_file", {"package": "SEFJ", "filename": "makitox4_enc_v1.9.0-40.hai"}, "/apis/upgrade/upload",
        [("file", b"HAI", "makitox4_enc_v1.9.0-40.hai", _OS)])
_hmx("install_upgrade_package", {}, "POST", "/apis/upgrade/install")
_hmx("delete_upgrade_package", {"filename": "makitox4_enc_v1.9.0-40.hai"}, "DELETE", "/apis/upgrade/makitox4_enc_v1.9.0-40.hai")

# ── Network, time, services ──────────────────────────────────────────────
_HMX_NET = {"hostname": "mx4e-studio", "autoDns": 1, "eth0": {"dhcp": 1, "ipv4": True}}
_hmx("configure_network", {"network": _HMX_NET}, "PUT", "/apis/network", _HMX_NET)
_hmx("set_time_ntp", {"timezone": "America/Montreal", "server": "ca.pool.ntp.org"}, "PUT", "/apis/datetime",
     {"timezone": "America/Montreal", "ntp": True, "server": "ca.pool.ntp.org"})
_hmx("set_time_ntp", {"timezone": "America/Montreal", "server": "ca.pool.ntp.org"}, "PUT", "/apis/datetime",
     {"timezone": "America/Montreal", "ntp": 1, "server": "ca.pool.ntp.org"}, model=_HMX_DEC, file="set_time_ntp-decoder")
_hmx("set_time_manual", {"timezone": "America/Montreal", "time": "08 04 2020 16:07:00"}, "PUT", "/apis/datetime",
     {"timezone": "America/Montreal", "ntp": False, "time_text": "08 04 2020 16:07:00"})
_hmx("set_time_manual", {"timezone": "Europe/London", "time": "12 31 2026 23:59:00"}, "PUT", "/apis/datetime",
     {"timezone": "Europe/London", "ntp": 0, "time_text": "12 31 2026 23:59:00"}, model=_HMX_DEC, file="set_time_manual-decoder")
_hmx("set_ptp", {"domain": 0, "announce_interval": -2, "announce_timeout": 3, "delay_mechanism": "e2e", "sync_interval": -3},
     "PUT", "/apis/datetime", {"domainNumber": 0, "announcementInterval": -2, "announcementTimeout": 3,
                                "delayMechanism": 1, "syncInterval": -3})
_hmx("validate_ntp_server", {"server": "ca.pool.ntp.org"}, "POST", "/apis/ntpvalidate", {"server": "ca.pool.ntp.org"},
     http_reply={"status": 200, "body": '{"error":false,"apply":true}'},
     expect_result={"ok": {"kind": "value", "value": {"error": False, "apply": True}}})
_hmx("validate_hostname", {"use": "audit", "hostname": "syslog.example.com"}, "POST", "/apis/hostnamevalidate/audit",
     {"hostname": "syslog.example.com"}, model=_HMX_DEC)
_hmx("configure_nmos", {"node_port": 8001, "mode": "manual", "is04_address": "10.65.10.169", "is04_port": 8010,
                        "is09": True, "is09_address": "10.65.10.169", "is09_port": 3001}, "PUT", "/apis/nmos",
     {"nodePort": 8001, "mode": "manual", "is04Address": "10.65.10.169", "is04Port": 8010, "is09": True,
      "is09Address": "10.65.10.169", "is09Port": 3001, "version": "1.3"})
_hmx("set_service", {"service": "onvif", "enabled": True}, "PUT", "/apis/services", {"onvif": 1})
_hmx("set_service", {"service": "telnet", "enabled": False}, "PUT", "/apis/services", {"telnet": 0}, file="set_service-off")
_hmx("set_service_interface", {"service": "snmp", "interface": "eth1"}, "PUT", "/apis/services", {"nic": {"snmp": "eth1"}})
_hmx("set_passthrough", {"port": 7788}, "PUT", "/apis/services", {"passthrough": 1, "passthroughPort": 7788}, model=_HMX_RUG)
_hmx("restart_service", {"service": "ems"}, "PUT", "/apis/services/ems/restart")
_HMX_CODE = "CABSdwoMMTAuNjUuMTEuMTg3"
_hmx("decode_ems_pairing_code", {"code": _HMX_CODE}, "PUT", "/apis/ems/decode", {"pairingCode": _HMX_CODE})
_hmx("pair_ems", {"code": _HMX_CODE, "hostname": "10.65.11.187", "keepalive": 56669}, "PUT", "/apis/ems/pair",
     {"emsPort": 8883, "pairingCode": _HMX_CODE, "emsHostname": "10.65.11.187", "emsKeepalive": 56669})
_hmx("unpair_ems", {}, "PUT", "/apis/ems/unpair")
_hmx("set_preview_service", {"enabled": False, "interval": 1}, "PUT", "/apis/services/preview", {"enabled": False, "interval": 1})
_hmx("set_decoder_preview_service", {"enabled": True, "decoders": [{"interval": 5, "enabled": True}]}, "PUT",
     "/apis/services/preview", {"enabled": True, "decoders": [{"interval": 5, "enabled": True}]}, model=_HMX_DEC)
_hmx("set_comport_mode", {"mode": "passthrough"}, "PUT", "/apis/comport", {"mode": 2}, model=_HMX_RUG)
_hmx("set_comport_baud_rate", {"baud_rate": "115200"}, "PUT", "/apis/comport", {"baudRate": 115200}, model=_HMX_RUG)

# ── Presets ──────────────────────────────────────────────────────────────
_hmx("create_preset", {"name": "yolo2.cfg"}, "POST", "/apis/presets", {"name": "yolo2.cfg", "startup": False, "changeActive": False})
_hmx_up("upload_preset", {"file": "W3N5c10=", "name": "default.cfg"}, "/apis/presets", [("file", b"[sys]", "default.cfg", _OS)])
_hmx("set_preset_autosave", {"enabled": True}, "PUT", "/apis/presets", {"autosave": True})
_hmx("set_preset_startup", {"name": "test.cfg", "startup": False}, "PUT", "/apis/presets/test.cfg", {"startup": False})
_hmx("make_startup_preset", {"name": "test.cfg"}, "PUT", "/apis/presets/test.cfg/startup")
_hmx("load_preset", {"name": "Show A.cfg"}, "PUT", "/apis/presets/Show%20A.cfg/load")
_hmx("duplicate_preset", {"name": "test.cfg"}, "PUT", "/apis/presets/test.cfg/duplicate")
_hmx("rename_preset", {"name": "test.cfg", "new_name": "test2"}, "PUT", "/apis/presets/test.cfg/rename",
     {"name": "test2", "overwriteIfAlreadyExists": False})
_hmx("delete_preset", {"name": "test.cfg"}, "DELETE", "/apis/presets/test.cfg")

# ── Streams and recordings ───────────────────────────────────────────────
_HMX_STREAM = {"name": "camera 2", "encapsulation": 2, "address": "10.65.10.175", "port": 12000,
               "video": [{"id": 0}], "audio": [{"id": 0}, {"id": 1}]}
_hmx("create_stream", {"stream": _HMX_STREAM}, "POST", "/apis/streams", _HMX_STREAM)
_HMX_SRT = {"encapsulation": 34, "name": "SRT listener", "port": 22222, "srtMode": 1, "latency": 130}
_hmx("create_stream", {"stream": _HMX_SRT}, "POST", "/apis/streams", _HMX_SRT, model=_HMX_DEC, file="create_stream-decoder")
_hmx("configure_stream", {"id": 3, "stream": {"port": 15002, "encapsulation": 2}}, "PUT", "/apis/streams/3",
     {"port": 15002, "encapsulation": 2})
_hmx("delete_stream", {"id": 3}, "DELETE", "/apis/streams/3")
_hmx("start_stream", {"id": 3}, "PUT", "/apis/streams/3/start")
_hmx("stop_stream", {"id": 3}, "PUT", "/apis/streams/3/stop")
_HMX_REC = {"streamId": 1, "autoStart": True, "format": "ts", "name": "rec_1", "limitType": "size", "limitSize": "1048576",
            "segmentType": "time", "segmentSize": "600", "recycleSegments": True}
_hmx("create_recording", {"id": 1, "name": "rec_1", "limit": "size", "limit_value": 1048576, "segments": "time",
                          "segment_value": 600, "recycle": True}, "POST", "/apis/recordings", _HMX_REC)
_hmx("configure_recording", {"id": 1, "name": "rec_1"}, "PUT", "/apis/recordings/1",
     {"streamId": 1, "autoStart": True, "format": "ts", "name": "rec_1", "limitType": "none", "limitSize": "0",
      "segmentType": "0", "segmentSize": "0", "recycleSegments": False})
_hmx("start_recording", {"id": 1}, "PUT", "/apis/recordings/1/start")
_hmx("stop_recording", {"id": 1}, "PUT", "/apis/recordings/1/stop")
_hmx("delete_recording", {"id": 1}, "DELETE", "/apis/recordings/1")

# ── Encoders, inputs, sources ────────────────────────────────────────────
_hmx("configure_video_encoder", {"id": 0, "settings": {"bitrate": 5000, "gopSize": 60}}, "PUT", "/apis/videnc/0",
     {"bitrate": 5000, "gopSize": 60})
_hmx("start_video_encoder", {"id": 0}, "PUT", "/apis/videnc/0/start")
_hmx("stop_video_encoder", {"id": 0}, "PUT", "/apis/videnc/0/stop")
_hmx("configure_video_input", {"id": 0, "fir_filter": True}, "PUT", "/apis/vidin/0",
     {"firFilter": True, "squareDivisionCapture": False})
_hmx("configure_audio_encoder", {"id": 0, "name": "Audio Encoder 0", "interface": 1, "algorithm": "aac_adts", "bitrate": 320,
                                 "mode": "stereo", "language": "eng"}, "PUT", "/apis/audenc/0",
     {"name": "Audio Encoder 0", "interface": 1, "algorithm": 10, "bitRate": 320, "sampleRate": 48, "mode": 0,
      "inputLevel": 6, "lang": "eng"})
_hmx("configure_audio_encoder", {"id": 1, "name": "Surround", "interface": 9, "algorithm": "pcm_passthrough", "bitrate": 0,
                                 "mode": "5.1"}, "PUT", "/apis/audenc/1",
     {"name": "Surround", "interface": 9, "algorithm": 1, "bitRate": 0, "sampleRate": 48, "mode": 6, "inputLevel": 6,
      "lang": ""}, file="configure_audio_encoder-surround")
_hmx("start_audio_encoder", {"id": 0}, "PUT", "/apis/audenc/0/start")
_hmx("stop_audio_encoder", {"id": 0}, "PUT", "/apis/audenc/0/stop")
_HMX_META = {"name": "network created", "type": 1, "format": 0, "port": 8755, "decimationMode": "none", "decimation": 1}
_hmx("create_metadata_source", {"source": _HMX_META}, "POST", "/apis/metadata", _HMX_META)
_hmx("configure_metadata_source", {"id": 5, "source": _HMX_META}, "PUT", "/apis/metadata/5", _HMX_META)
_hmx("start_metadata_source", {"id": 5}, "PUT", "/apis/metadata/5/start")
_hmx("stop_metadata_source", {"id": 5}, "PUT", "/apis/metadata/5/stop")
_hmx("delete_metadata_source", {"id": 5}, "DELETE", "/apis/metadata/5")
_hmx("configure_ad_insertion_source", {"id": 1, "name": "HD-SDI-BNC-1", "synchronization": "auto", "leap_seconds": 37,
                                       "time_descriptor_period": 1, "pre_event_reminders": 3, "message_delta": 500},
     "PUT", "/apis/scte/1", {"name": "HD-SDI-BNC-1", "synchronizationType": "auto", "leapSeconds": 37,
                             "timeDescriptorPeriodSecs": 1, "preEventReminders": 3, "msgDeltaMs": 500})
_hmx("start_ad_insertion_source", {"id": 1}, "PUT", "/apis/scte/1/start")
_hmx("stop_ad_insertion_source", {"id": 1}, "PUT", "/apis/scte/1/stop")
_HMX_ESS = {"input_channel": 0, "address": "239.11.129.10", "port": 12000, "rtp_payload": 96}
_hmx("create_video_essence", dict(_HMX_ESS, name="building 2"), "POST", "/apis/essences",
     {"type": "video", "name": "building 2", "autoStart": True, "address": "239.11.129.10", "port": 12000,
      "sourceAddress": "", "rtpPayload": 96, "inputChannel": 0, "inputFormat": "0"})
_hmx("create_audio_essence", dict(_HMX_ESS, name="camera 3", rtp_payload=98, channels=4), "POST", "/apis/essences",
     {"type": "audio", "name": "camera 3", "autoStart": True, "address": "239.11.129.10", "port": 12000,
      "sourceAddress": "", "rtpPayload": 98, "inputChannel": 0, "numOfChannels": 4, "packetTime": 1})
_hmx("create_data_essence", dict(_HMX_ESS, rtp_payload=100, auto_start=False), "POST", "/apis/essences",
     {"type": "data", "name": "", "autoStart": False, "address": "239.11.129.10", "port": 12000, "sourceAddress": "",
      "rtpPayload": 100, "inputChannel": 0})
_hmx("configure_video_essence", dict(_HMX_ESS, id=0, rtp_payload=102, format="9223372044908392568"), "PUT",
     "/apis/essences/0", {"type": "video", "name": "", "address": "239.11.129.10", "port": 12000, "sourceAddress": "",
                          "rtpPayload": 102, "inputChannel": 0, "inputFormat": "9223372044908392568"})
_hmx("configure_audio_essence", dict(_HMX_ESS, id=2, channels=2, packet_time="125us"), "PUT", "/apis/essences/2",
     {"type": "audio", "name": "", "address": "239.11.129.10", "port": 12000, "sourceAddress": "", "rtpPayload": 96,
      "inputChannel": 0, "numOfChannels": 2, "packetTime": 2})
_hmx("configure_data_essence", dict(_HMX_ESS, id=3, source_address="10.65.251.66"), "PUT", "/apis/essences/3",
     {"type": "data", "name": "", "address": "239.11.129.10", "port": 12000, "sourceAddress": "10.65.251.66",
      "rtpPayload": 96, "inputChannel": 0})
_hmx("start_essence", {"id": 0}, "PUT", "/apis/essences/0/start")
_hmx("stop_essence", {"id": 0}, "PUT", "/apis/essences/0/stop")
_hmx("delete_essence", {"id": 0}, "DELETE", "/apis/essences/0")
_hmx("take_snapshot", {"encoder": 1, "quality": 90}, "POST", "/apis/snapshots", {"videnc": 1, "quality": 90},
     http_reply={"status": 200, "body": '{"data":{"filename":"snap-1.png","id":"snap-1.png"}}'},
     expect_result={"ok": {"kind": "value", "value": {"data": {"filename": "snap-1.png", "id": "snap-1.png"}}}})
_hmx("take_snapshot", {}, "POST", "/apis/snapshots", {"videnc": 0, "quality": 90}, file="take_snapshot-stopped",
     http_reply={"status": 500, "body": '{"message":"Failed to take snapshot: \'Encoder not started\'"}'},
     expect_result={"error": {"error": "device_error", "code": "500"}})
_hmx("delete_snapshot", {"id": "snap-2021-10-20-17h36m35s.png"}, "DELETE", "/apis/snapshots/snap-2021-10-20-17h36m35s.png")
_hmx("configure_snapshot_settings", {"quality": 65, "format": "png", "max_storage_kb": 100000}, "PUT",
     "/apis/snapshotsettings", {"quality": 65, "format": "png", "maxStorageKb": 100000})
_hmx_up("upload_logo", {"image": "iVBORw==", "filename": "IMG_2445.png", "content_type": "image/png"}, "/apis/logos",
        [("file", bytes.fromhex("89504e47"), "IMG_2445.png", "image/png")])
_hmx("delete_logo", {"name": "IMG_2445"}, "DELETE", "/apis/logos/IMG_2445")
_hmx("configure_logo", {"interface": 0, "display": True, "file": "HaivisionMakitoX4", "opacity": 78, "scaling": 124,
                        "x": 768, "y": 606}, "PUT", "/apis/logosettings/0",
     {"display": True, "interface": 0, "opacity": 78, "scaling": 124, "relative": False, "positioning": 0, "x": 768,
      "y": 606, "fileName": "HaivisionMakitoX4"})

# ── Decoders ─────────────────────────────────────────────────────────────
_hmx("set_decoder_stream", {"id": 0, "stream": 3}, "PUT", "/apis/decoders/0", {"streamId": 3}, model=_HMX_DEC)
_hmx("set_decoder_stream", {"id": 1, "stream": -1}, "PUT", "/apis/decoders/1", {"streamId": -1}, model=_HMX_DEC,
     file="set_decoder_stream-none")
_hmx("configure_decoder", {"id": 0, "name": "Decoder 0", "stream": 3, "still_image": "custom", "still_image_delay": 3,
                           "still_image_file": "slate.png", "buffering": "fixed", "buffering_delay": 1000,
                           "multisync_delay": 1010, "hdr": "off", "output1": False, "output2": False, "output3": True,
                           "output4": False, "frame_rate": "59.94", "quad": "off", "drop_corrupted": False},
     "PUT", "/apis/decoders/0",
     {"name": "Decoder 0", "streamId": 3, "stillImage": 6, "stillImageDelay": 3, "stillImageFilename": "slate.png",
      "bufferingMode": 2, "bufferingDelay": 1000, "multisyncBufferingDelay": 1010, "hdrDynamicRange": -1,
      "outputs": [False, False, True, False], "outputFrameRate": 59, "quadMode": 0, "dropCorruptedFrames": False},
     model=_HMX_DEC)
_hmx("start_decoder", {"id": 0}, "PUT", "/apis/decoders/0/start", model=_HMX_DEC)
_hmx("stop_decoder", {"id": 0}, "PUT", "/apis/decoders/0/stop", model=_HMX_DEC)
_hmx("configure_st2110_output", {"id": 10, "name": "Video Output 1", "address": "239.78.0.10", "port": 40000,
                                 "rtp_payload": 96}, "PUT", "/apis/st2110/10",
     {"name": "Video Output 1", "address": "239.78.0.10", "port": 40000, "rtpPayload": 96, "tos": "0x88", "ttl": 64},
     model=_HMX_DEC)
_hmx("configure_st2110_audio_output", {"id": 11, "address": "239.78.0.11", "port": 40000, "rtp_payload": 98,
                                       "packet_time": "1ms"}, "PUT", "/apis/st2110/11",
     {"name": "", "address": "239.78.0.11", "port": 40000, "rtpPayload": 98, "tos": "0x88", "ttl": 64, "packetTime": 1},
     model=_HMX_DEC)
_hmx("start_st2110_output", {"id": 10}, "PUT", "/apis/st2110/10/start", model=_HMX_DEC)
_hmx("stop_st2110_output", {"id": 10}, "PUT", "/apis/st2110/10/stop", model=_HMX_DEC)


# ── Telemetry ────────────────────────────────────────────────────────────
def _hmx_t(name, path, body, expect_state, model=None, **extra):
    v = {"spec": HMX, "telemetry": name, "settings": _HMX_SET,
         "inbound_http": {"path": path, "body": body if isinstance(body, str) else _hmx_j(body)},
         "expect_state": expect_state, **extra}
    if model:
        v["model"] = model
    V.append(v)


_hmx_t("status", "/apis/status", {"cardStatus": "OK", "serialNumber": "HAI-032004020015", "cardType": "Makito X4 SDI Encoder",
                                  "firmwareVersion": "1.7.0-43", "uptime": "0 days 19:44:14", "httpdUptime": "66374.81",
                                  "chipsetLoad": "63", "temperature": "53"},
       {"system": {"card_status": "OK", "serial_number": "HAI-032004020015", "card_type": "Makito X4 SDI Encoder",
                   "firmware_version": "1.7.0-43", "uptime": "0 days 19:44:14", "httpd_uptime_s": 66374.81,
                   "chipset_load": 63.0, "temperature": 53.0}})
_hmx_t("system-info", "/apis/system_info", {"hostname": "mx4e-hai", "videoEncoders": 8, "hevc": True, "rugged": False,
                                            "personality": "SDI", "nextPersonality": "2110", "snapshot": 1},
       {"info": {"hostname": "mx4e-hai", "video_encoders": 8, "hevc": True, "rugged": False, "personality": "SDI",
                 "next_personality": "2110", "snapshot": 1}})
_hmx_t("streams-encoder", "/apis/streams", {"data": [{
    "info": {"name": "UDP to 15000", "id": 3, "type": 1, "encapsulation": 2, "address": "10.65.11.149", "port": 15000,
             "pcrPid": 33, "pcrPidAutoAssigned": True, "video": [{"id": 0, "name": "Video Encoder 0", "pid": 33,
                                                                     "autoassigned": True}],
             "tos": "0x80", "shaping": "vbr", "blockAligned": "1",
             "recording": {"enabled": True, "status": "RECORDING", "percentageDone": 12}},
    "stats": {"uptime": "1h2m", "sentBytes": "37,972,184", "bitrateInKbps": 12807, "state": 1, "stateString": "STREAMING",
              "srt": {"state": 1, "rtt": "1200"}}}]},
       {"streams": {"3": {"name": "UDP to 15000", "type": 1, "encapsulation": 2, "address": "10.65.11.149", "port": 15000,
                          "pcr_pid": 33, "pcr_pid_auto": True,
                          "video": '[{"id":0,"name":"Video Encoder 0","pid":33,"autoassigned":true}]',
                          "tos": "0x80", "shaping": "vbr", "fec_block_aligned": True,
                          "recording": {"enabled": True, "status": "RECORDING", "percent_done": 12},
                          "stats": {"uptime": "1h2m", "sent_bytes": "37,972,184", "bitrate_kbps": 12807, "state": 1,
                                    "state_name": "STREAMING", "srt": {"state": 1, "rtt": "1200"}}}}},
       state_before={"streams": {"9": {"name": "Deleted"}}})
_hmx_t("streams-decoder", "/apis/streams", {"data": [{
    "info": {"name": "SRT encrypted", "id": 4, "decoderId": 2, "encapsulation": 34, "port": 3333, "srtMode": 1,
             "passphraseSet": True, "srtToUdp": False, "audioPids": []},
    "stats": {"state": 3, "receivedBytes": "110,571,836", "bitrateInKbps": 591, "streamSummary": "1 video, 0 audio, 1 KLV, 0 filtered",
              "srt": {"state": "ACTIVE", "remotePort": "40001"}}}]},
       {"streams": {"4": {"name": "SRT encrypted", "decoder_id": 2, "encapsulation": 34, "port": 3333, "srt_mode": 1,
                          "passphrase_set": True, "srt_to_udp": False, "audio_pids": "[]",
                          "stats": {"state": 3, "received_bytes": "110,571,836", "bitrate_kbps": 591,
                                    "summary": "1 video, 0 audio, 1 KLV, 0 filtered",
                                    "srt": {"state_name": "ACTIVE", "remote_port_text": "40001"}}}}},
       model=_HMX_DEC)
_hmx_t("video-encoders", "/apis/videnc", {"data": [{
    "info": {"name": "Video Encoder 0", "id": 0, "codecAlgorithm": 1, "resolution": "9223372042223853568",
             "resolutionText": "1280x720p", "bitrate": 1200, "gopSize": 100, "dailyResync": True, "partialFrameSkip": 1},
    "stats": {"inputPresent": 1, "inputFormat": "1920x1080i29", "encodedFrameRate": 59.94, "encodedBitRate": "1,291",
              "encoderLoad": 11, "state": 7},
    "associatedStreams": [{"id": 1, "state": 1, "shaping": "vbr"}]}]},
       {"video_encoders": {"0": {"name": "Video Encoder 0", "codec": 1, "resolution": "9223372042223853568",
                                 "resolution_text": "1280x720p", "bitrate": 1200, "gop_size": 100, "daily_resync": True,
                                 "partial_frame_skip": True,
                                 "associated_streams": '[{"id":1,"state":1,"shaping":"vbr"}]',
                                 "stats": {"input_present": 1, "input_format": "1920x1080i29", "encoded_frame_rate": 59.94,
                                           "encoded_bitrate": "1,291", "encoder_load": 11, "state": 7}}}})
_hmx_t("audio-encoders", "/apis/audenc", {"data": [{
    "info": {"name": "Audio Encoder 0", "id": 0, "interface": 1, "algorithm": 10, "bitRate": 128, "sampleRate": 48, "mode": 0,
             "inputLevel": 6, "lang": "fra"},
    "stats": {"encodedFramerate": "46.89", "maximumVuLevel": "-5.7 dB", "state": 3, "lastReset": "3s ago"}}]},
       {"audio_encoders": {"0": {"name": "Audio Encoder 0", "interface": 1, "algorithm": 10, "bitrate": 128, "sample_rate": 48,
                                 "mode": 0, "input_level": 6, "language": "fra",
                                 "stats": {"encoded_frame_rate": 46.89, "max_vu_level": "-5.7 dB", "state": 3,
                                           "last_reset": "3s ago"}}}})
_hmx_t("video-inputs", "/apis/vidin", {"data": [{"name": "BNC-1 or 2SI", "id": 0, "interfaceType": 1, "firFilter": False,
                                                  "squareDivisionCapture": False, "state": "ACTIVE"}]},
       {"video_inputs": {"0": {"name": "BNC-1 or 2SI", "interface_type": 1, "fir_filter": False, "square_division": False,
                               "state": "ACTIVE"}}})
_hmx_t("metadata", "/apis/metadata", {"data": [{
    "info": {"name": "HD-SDI-BNC-1", "id": 1, "status": 0, "type": 2, "format": 0, "securityInsertionEnabled": 0,
             "klvFilteringEnabled": True, "uasTags": [1, 2, 3]},
    "stats": {"rxBytes": "0", "klvBitrate": "0", "state": 0, "stateTextual": "stopped"}}]},
       {"metadata": {"1": {"name": "HD-SDI-BNC-1", "status": 0, "type": 2, "format": 0, "security_insertion": False,
                           "klv_filtering": True, "uas_tags": "[1,2,3]",
                           "stats": {"received_bytes": "0", "klv_bitrate": 0, "state": 0, "state_name": "stopped"}}}})
_hmx_t("ad-insertion", "/apis/scte", {"data": [{
    "info": {"name": "HD-SDI-BNC-1", "id": 1, "status": 1, "autoStart": False, "synchronizationType": "auto", "leapSeconds": 37,
             "timeDescriptorPeriodSecs": "0", "msgDeltaMs": "2000"},
    "stats": {"rxOKMessages": "112", "state": 1, "lastScte104ErrorCode": "AS & IJ are not time synced (1004)"}}]},
       {"ad_insertion": {"1": {"name": "HD-SDI-BNC-1", "status": 1, "auto_start": False, "synchronization": "auto",
                               "leap_seconds": 37, "time_descriptor_period_s": 0, "message_delta_ms": 2000,
                               "stats": {"ok_messages": "112", "state": 1,
                                         "last_error_code": "AS & IJ are not time synced (1004)"}}}})
_hmx_t("essences", "/apis/essences", {"data": [{
    "info": {"name": "v1", "id": 1, "autoStart": True, "inputChannel": 0, "type": "video", "rtpPayload": 96,
             "address": "239.4.1.10", "port": 50000, "inputFormat": "0"},
    "stats": {"streamSync": True, "rxPackets": "34,909,932", "state": 1, "stateString": "RUNNING",
              "detectedVideoFormat": "1280x720p60"}}]},
       {"essences": {"1": {"name": "v1", "auto_start": True, "input_channel": 0, "type": "video", "rtp_payload": 96,
                           "address": "239.4.1.10", "port": 50000, "input_format": "0",
                           "stats": {"stream_sync": True, "received_packets": "34,909,932", "state": 1,
                                     "state_name": "RUNNING", "detected_format": "1280x720p60"}}}})
_hmx_t("decoders", "/apis/decoders", {"data": [{
    "info": {"id": 0, "name": "Decoder 0", "streamId": 3, "stillImage": 6, "bufferingMode": 2, "bufferingDelay": 1000,
             "hdrDynamicRange": -1, "outputs": [False, False, True, False], "outputFrameRate": 0, "quadMode": 0,
             "dropCorruptedFrames": False},
    "stats": {"state": 2, "troubleCode": -2, "streamState": 3, "videoAlgorithm": "H.264", "videoLatency": "11662 ms",
              "loadPercentage": 11, "klv": False}}]},
       {"decoders": {"0": {"name": "Decoder 0", "stream_id": 3, "still_image": 6, "buffering_mode": 2,
                           "buffering_delay_ms": 1000, "hdr": -1, "outputs": "[false,false,true,false]",
                           "output_frame_rate": 0, "quad_mode": 0, "drop_corrupted_frames": False,
                           "stats": {"state": 2, "trouble_code": -2, "stream_state": 3, "video_algorithm": "H.264",
                                     "video_latency": "11662 ms", "load": 11, "klv": False}}}},
       model=_HMX_DEC)
_hmx_t("st2110-outputs", "/apis/st2110", {"data": [{
    "info": {"name": "Video Output 1", "id": 10, "output": 1, "state": 1, "stateString": "STARTED", "type": "video",
             "rtpPayload": 96, "tos": "0x88", "ttl": 64, "address": "239.78.0.10", "port": 40000},
    "stats": {"streamSync": True, "txPackets": "1,767,615,165", "stateString": "RUNNING", "videoFormat": "1280x720p59"}}]},
       {"st2110": {"10": {"name": "Video Output 1", "output": 1, "state": 1, "state_name": "STARTED", "type": "video",
                          "rtp_payload": 96, "tos": "0x88", "ttl": 64, "address": "239.78.0.10", "port": 40000,
                          "stats": {"stream_sync": True, "sent_packets": "1,767,615,165", "state_name": "RUNNING",
                                    "video_format": "1280x720p59"}}}},
       model=_HMX_DEC)
_hmx_t("network", "/apis/network", {"eth0": {"name": "eth0", "ipv4": True, "ipv6": False, "currentSpeed": "1000",
                                             "mac": "5c:77:57:00:e5:89", "dhcp": 1, "ip": "10.65.11.143"},
                                    "dnsPrecedence": "ipv6", "hostname": "mx4e-hai-032004020015", "autoDns": 1,
                                    "mDnsEnabled": 1, "mDnsName": "MX4E-MTL-003"},
       {"network": {"interfaces": {"eth0": {"ipv4": True, "ipv6": False, "current_speed": "1000", "mac": "5c:77:57:00:e5:89",
                                            "dhcp": True, "ip": "10.65.11.143"}},
                    "dns_precedence": "ipv6", "hostname": "mx4e-hai-032004020015", "auto_dns": True, "mdns": True,
                    "mdns_name": "MX4E-MTL-003"}})
_hmx_t("datetime", "/apis/datetime", {"timezone": "America/Montreal", "ntp": True, "server": "23.133.168.245",
                                      "offset": -18000, "dst": False, "time": 1763496208, "clockLocked": True,
                                      "delayMechanism": 0, "syncInterval": -3},
       {"datetime": {"timezone": "America/Montreal", "ntp": True, "ntp_server": "23.133.168.245", "utc_offset_s": -18000,
                     "dst": False, "time": 1763496208, "ptp_locked": True, "ptp_delay_mechanism": 0, "ptp_sync_interval": -3}})
_hmx_t("datetime-decoder", "/apis/datetime", {"timezone": "UTC", "ntp": 1},
       {"datetime": {"timezone": "UTC", "ntp": True}}, model=_HMX_DEC)
_hmx_t("nmos", "/apis/nmos", {"data": {"nodePort": 8001, "mode": "manual", "is04Address": "10.65.10.109", "is04Port": 8010,
                                       "is09": True, "version": "1.3"}},
       {"nmos": {"node_port": 8001, "mode": "manual", "is04_address": "10.65.10.109", "is04_port": 8010, "is09": True,
                 "version": "1.3"}})
_hmx_t("services", "/apis/services/status", {"http": 1, "onvif": 0, "passthroughPort": 7788,
                                             "status": {"http": True, "onvif": False, "ems": 2}, "nic": {"snmp": "eth1"}},
       {"services": {"http": {"requested": True, "active": True}, "onvif": {"requested": False, "active": False},
                     "ems": {"active_state": 2}, "snmp": {"interface": "eth1"}, "passthrough": {"port": 7788}}})
_hmx_t("preview", "/apis/services/preview", {"enabled": True, "interval": 30, "width": 456, "height": 256},
       {"preview": {"enabled": True, "interval_s": 30, "width": 456, "height": 256}})
_hmx_t("snapshot-settings", "/apis/snapshotsettings", {"data": {"quality": 100, "format": "png", "lastInput": 0,
                                                                "systemType": "SDI", "maxStorageKb": 102400}},
       {"snapshots": {"settings": {"quality": 100, "format": "png", "last_input": 0, "system_type": "SDI",
                                   "max_storage_kb": 102400}}})
_hmx_t("snapshots", "/apis/snapshots", {"data": [{"filename": "snap-1.png", "id": "snap-1.png",
                                                  "ctime": "20 Oct 2021 20:17:48 GMT", "age": 3551}]},
       {"snapshots": {"files": {"snap-1.png": {"created": "20 Oct 2021 20:17:48 GMT", "age_s": 3551}}}})
_hmx_t("logos", "/apis/logos", {"data": [{"name": "Haivision", "mtime": "1698791830",
                                          "thumbnail": "/apis/logos/.Haivision.oly.png?t=1698791830"}]},
       {"logos": {"files": {"Haivision": {"modified": 1698791830, "thumbnail": "/apis/logos/.Haivision.oly.png?t=1698791830"}}}})
_hmx_t("logo-settings", "/apis/logosettings", {"data": [{"display": True, "interface": 0, "if_name": "BNC-1", "opacity": 100,
                                                         "scaling": 100, "relative": False, "positioning": 0, "x": 1664,
                                                         "y": 869, "fileName": "C++"}]},
       {"logos": {"interfaces": {"0": {"display": True, "interface_name": "BNC-1", "opacity": 100, "scaling": 100,
                                       "relative": False, "positioning": 0, "x": 1664, "y": 869, "file": "C++"}}}})
_hmx_t("comport", "/apis/comport", {"mode": 0, "standard": 0, "baudRate": 115200, "flowControl": 0, "parity": 0,
                                    "stopBits": 1, "dataBits": 8, "passthroughActive": False},
       {"comport": {"mode": 0, "standard": 0, "baud_rate": 115200, "flow_control": 0, "parity": 0, "stop_bits": 1,
                    "data_bits": 8, "passthrough_active": False}}, model=_HMX_RUG)
_hmx_t("ems", "/apis/ems", {"data": {"deviceID": "", "emsHostname": "10.65.11.187", "emsKeepalive": 56669, "emsPort": 8883,
                                     "lastPairingError": "", "serviceEnabled": 1, "state": "UNPAIRED", "waitingToUnpair": 0}},
       {"ems": {"device_id": "", "hostname": "10.65.11.187", "keepalive_s": 56669, "port": 8883, "last_pairing_error": "",
                "service_enabled": True, "state": "UNPAIRED", "unpairing": False}})
_hmx_t("leds", "/apis/leds", {"statusLED": "Green", "statusTransitions": 1, "blinkingWillStopIn": "14m55s"},
       {"leds": {"status": "Green", "blinking_stops_in": "14m55s"}})
_hmx_t("presets", "/apis/presets", {"data": [{"name": "frank.cfg", "startup": True, "mtime": "1706889318"}],
                                    "autosave": False, "active": "frank.cfg", "activeIsStartup": True,
                                    "activeWasModified": False},
       {"presets": {"files": {"frank.cfg": {"startup": True, "modified": 1706889318}}, "autosave": False,
                    "active": "frank.cfg", "active_is_startup": True, "active_modified": False}},
       state_before={"presets": {"files": {"old.cfg": {"startup": False}}}})
_hmx_t("preset-saved", "/apis/presets/current/issaved", {"isSaved": False}, {"presets": {"saved": False}})
_hmx_t("accounts", "/apis/accounts", {"data": [{"info": {"name": "admin", "role": "Administrator", "state": 0, "expiration": 0,
                                                         "expirationDays": 0, "pwd": True},
                                                "keys": {"accountName": "admin", "data": []}}]},
       {"accounts": {"admin": {"role": "Administrator", "state": 0, "expiration": 0, "expiration_days": 0,
                               "password_set": True, "public_keys": "[]"}}})
_hmx_t("policies", "/apis/policies", {"minLen": 6, "quality": "Strong", "hsts": True, "autoLogoutTime": 2,
                                      "compliance": "NONE", "tls1_2": True},
       {"policies": {"min_length": 6, "quality": "Strong", "hsts": True, "auto_logout_min": 2, "compliance": "NONE",
                     "tls1_2": True}})
_hmx_t("audit", "/apis/audit", {"enabled": 1, "transport": "TLS", "server": "10.65.10.173", "trusted": "Self-signed",
                                "fingerprint": "SHA1:AD"},
       {"audit": {"enabled": True, "transport": "TLS", "server": "10.65.10.173", "trusted": "Self-signed",
                  "fingerprint": "SHA1:AD"}})
_hmx_t("banner", "/apis/banner", {"banner": "Hi&#32;there", "enabled": True, "status": 1},
       {"banner": {"text": "Hi&#32;there", "enabled": True}})
_hmx_t("ca", "/apis/ca", {"data": [{"id": "root", "name": "root", "type": "CA", "default": 0}]},
       {"certificates": {"ca": {"root": {"name": "root", "type": "CA", "default": False}}}})
_hmx_t("identity", "/apis/identity", {"data": [{"id": "srv1", "name": "srv1", "type": "Self-signed", "default": 1}]},
       {"certificates": {"identity": {"srv1": {"name": "srv1", "type": "Self-signed", "default": True}}}})
_hmx_t("license-features", "/apis/license", {"data": [{"feature": "Number of Video Encoders", "value": "2", "category": "video"}]},
       {"license": {"features": {"Number of Video Encoders": {"value": "2", "category": "video"}}}})
_hmx_t("licenses", "/apis/licenses", {"data": [{"name": "MX4E-complete-HAI-032004020015.lic"}]},
       {"license": {"files": {"MX4E-complete-HAI-032004020015.lic": {"name": "MX4E-complete-HAI-032004020015.lic"}}}})
_hmx_t("upgrade", "/apis/upgrade", {"data": {"name": "makitox4_enc_v1.9.0-40.hai"}, "currentVersion": "1.8.0-22"},
       {"upgrade": {"package": "makitox4_enc_v1.9.0-40.hai", "current_version": "1.8.0-22"}})
_hmx_t("messages", "/apis/messages", {"messages": "Thu May 8 04:08:04 WST 2025: Software installed.&#10;"},
       {"messages": {"text": "Thu May 8 04:08:04 WST 2025: Software installed.&#10;"}})
_hmx_t("session", "/apis/authentication", {"username": "admin", "uid": 500, "gid": 511, "passwordExp": False},
       {"session": {"username": "admin", "uid": 500, "gid": 511, "password_expired": False}})
# Binary answers saved to the host file named (never written by a vector).
_hmx("download_snapshot", {"id": "snap-1.png", "to": {"file": "snap-1.png"}}, "GET", "/apis/snapshots/snap-1.png")
_hmx("download_input_preview", {"id": "0", "to": {"file": "preview.jpg"}}, "GET", "/apis/preview/0")
_hmx("download_logo", {"file": "station.png", "to": {"file": "station.png"}}, "GET", "/apis/logos/station.png")
_hmx("download_decoder_preview", {"id": 0, "to": {"file": "decoder.jpg"}}, "GET", "/apis/decoders/0/preview",
     model="makito-x4-decoder")
