# Magewell Ultra Encode (magewell-ultra-encode): one vector per command, a
# second where the AIO and Plus models send another request than the HDMI and
# SDI models, and telemetry vectors for every rule, from the documents'
# example replies. Requests are as the Ultra Encode API, Ultra Encode AIO API
# and Ultra Encode Plus API give them: GET /usapi?method=<name>&<args> with
# values percent-encoded (RFC 3986 unreserved characters kept), uploads as
# multipart/form-data with the file in a part named file, and the Control Hub
# methods on port 8070 under /cloud-api.
from urllib.parse import quote as _ue_quote

UE = "magewell-ultra-encode"
_UE_V2 = "aio"


def _ue_target(method, pairs):
    query = "".join(f"&{k}={_ue_quote(str(v), safe='-._~')}" for k, v in pairs)
    return f"/usapi?method={method}{query}"


def _ue(command, input, method, pairs=(), model=None, file=None, port=None, path="/usapi", **extra):
    target = _ue_target(method, pairs).replace("/usapi", path, 1)
    request = {"method": "GET", "target": target}
    if port:
        request["port"] = port
    v = {"spec": UE, "command": command, "input": input, "expect_request": request, **extra}
    if model:
        v["model"] = model
    if file:
        v["file"] = file
    V.append(v)


def _ue2(command, input, method, pairs=(), **extra):
    """The same command on an AIO, where its request differs."""
    _ue(command, input, method, pairs, model=_UE_V2, file=f"{command}-aio", **extra)


# The multipart body the core writes for one binary part (SPEC.md §4,
# "Binary request bodies"): the boundary is derived from the parts.
def _ue_form(data, filename):
    import hashlib
    digest = hashlib.sha256(b"file\0" + data + b"\0").hexdigest()
    boundary = "----form-" + digest[:16]
    body = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n"
            "Content-Type: application/octet-stream\r\n\r\n").encode() + data + b"\r\n" + f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


def _ue_upload(command, method, data_b64, data, filename, model=None, **extra):
    body, ctype = _ue_form(data, filename)
    v = {"spec": UE, "command": command, "input": {("image" if command in ("add_nosignal_image", "add_overlay_image") else "file"): data_b64},
         "expect_request": {"method": "POST", "target": f"/usapi?method={method}", "body_hex": body.hex(), "content_type": ctype},
         **extra}
    if model:
        v["model"] = model
    V.append(v)


# ── Universal Interfaces ──
_ue("ping", {}, "ping", http_reply={"status": 200, "body": '{"result":0,"cur-status":65552}'},
    expect_result={"ok": {"kind": "ack"}})
_ue("get_info", {}, "get-info", http_reply={"status": 200, "body": '{"result":0,"product":{"sn":"A305200908002"}}'},
    expect_result={"ok": {"kind": "value", "value": {"result": 0, "product": {"sn": "A305200908002"}}}})
_ue("get_status", {}, "get-status")
_ue("get_settings", {}, "get-settings")
_ue("get_signal_info", {}, "get-signal-info")
_ue2("get_signal_info", {"input": "sdi"}, "get-signal-info", [("input-device", 1)])
_ue("get_report", {}, "get-report", http_reply={"status": 200, "body": "<div class=\"report-summary\"></div>"},
    expect_result={"ok": {"kind": "value", "value": "<div class=\"report-summary\"></div>"}})
_ue("export_settings", {}, "export-settings", [("file-name", "settings.json")], model=_UE_V2)
_ue_upload("import_settings", "import-settings", "e30=", b"{}", "settings.json", model=_UE_V2)
_ue("reboot", {}, "reboot")
_ue("reset_all_settings", {}, "reset-all-settings", http_reply={"status": 200, "body": '{"result":-9}'},
    expect_result={"error": {"error": "device_error", "code": "-9"}})
_ue("get_logs", {"types": "warn,error"}, "get-logs", [("types", "warn,error")], model=_UE_V2)
_ue("clear_logs", {}, "clear-logs", model=_UE_V2)
for _c, _m in [("start_live", "start-live"), ("stop_live", "stop-live"), ("clear_live", "clear-live"),
               ("start_recording", "start-rec"), ("stop_recording", "stop-rec"), ("clear_recording", "clear-rec")]:
    _ue(_c, {}, _m, model=_UE_V2)

# ── General settings ──
_ue("set_name", {"name": "Stage (Left)"}, "set-name", [("name", "Stage (Left)")])
_ue("set_first_over", {"name": "Encoder-1"}, "set-first-over", [("name", "Encoder-1")])
_ue("set_softap", {"ssid": "A304201201001", "password": "01201001"}, "set-softap",
    [("is-softap", 1), ("is-visible", 1), ("softap-ssid", "A304201201001"), ("softap-passwd", "01201001")])
_ue("open_softap", {}, "open-softap")
_ue("close_softap", {}, "close-softap")
_ue("set_ssdp", {"enabled": True}, "set-ssdp", [("is-ssdp", 1)])
_UE_VOL = {"input_on": True, "input_gain": -6.5, "line_in_on": False, "line_in_gain": 12.0, "line_out_on": True, "line_out_gain": 0.0}
_ue("set_volume", _UE_VOL, "set-volume", [("is-mic", 0), ("mic-gain", "12.00"), ("is-spi", 1), ("spi-gain", "-6.50"),
                                          ("is-phone", 1), ("phone-gain", "0.00")])
_ue2("set_volume", _UE_VOL, "set-volume", [("is-linein", 0), ("linein-gain", 1200), ("is-spi", 1), ("spi-gain", -650),
                                           ("is-lineout", 1), ("lineout-gain", 0)])
_ue("set_mic_bias", {"enabled": True}, "set-analog-mic-bias", [("enable-mic-bias", 1)], model=_UE_V2)
_ue("set_audio_mixer", {"input_audio": True, "line_audio": False}, "set-audio-mixer-config",
    [("enable-spi-mix", 1), ("enable-lineout-mix", 0)], model=_UE_V2)
_ue("set_video_color", {"contrast": 100, "brightness": -10, "saturation": 120, "hue": 0}, "set-video-color",
    [("contrast", 100), ("brightness", -10), ("saturation", 120), ("hue", 0)])
_ue("set_video_input_format", {"color_auto": False, "color_format": "rgb", "quant_auto": True}, "set-video-input-format",
    [("is-color-fmt", 1), ("color-fmt", 1), ("is-quant-range", 0), ("quant-range", 2)])
_ue2("set_video_input_format", {"input": "sdi", "color_auto": True, "quant_auto": False, "quant_range": "full"},
     "set-video-input-format", [("input-device", 1), ("is-color-fmt", 0), ("color-fmt", 3), ("is-quant-range", 1), ("quant-range", 1)])
_ue("set_video_output_format", {"color_auto": False, "color_format": "bt.601", "quant_auto": True, "sat_auto": False,
                                "sat_range": "extended"}, "set-video-output-format",
    [("is-color-fmt", 1), ("color-fmt", 2), ("is-quant-range", 0), ("quant-range", 2), ("is-sat-range", 1), ("sat-range", 3)])
_ue2("set_video_output_format", {"color_auto": True, "quant_auto": True, "sat_auto": True}, "set-video-output-format",
     [("input-device", 2), ("is-color-fmt", 0), ("color-fmt", 3), ("is-quant-range", 0), ("quant-range", 2), ("is-sat-range", 0),
      ("sat-range", 2)])
_ue("set_audio_sync_offset", {"ms": -40}, "set-sync-offset", [("audio-sync-offset", -40)])
_ue_upload("add_nosignal_image", "add-nosignal-file", "/9j/4A==", b"\xff\xd8\xff\xe0", "nosignal.jpg")
_ue("delete_nosignal_image", {"id": 2}, "del-nosignal-file", [("id", 2)])
_ue("select_nosignal_image", {"id": 1}, "set-nosignal-file", [("id", 1)])
_ue("set_nosignal_image_enabled", {"enabled": False}, "use-nosignal-file", [("use-nosignal-file", 0)])
_ue("set_time_zone", {"timezone": "America/New_York"}, "set-time-zone", [("timezone", "America/New_York")])
_ue("set_time_auto", {"ntp_server": "0.pool.ntp.org", "ntp_backup": "1.pool.ntp.org"}, "set-date-time",
    [("is-auto", 1), ("time", ""), ("ntp-server", "0.pool.ntp.org"), ("ntp-server-backup", "1.pool.ntp.org")])
_ue("set_time_manual", {"time": "2026-10-23 15:10:00"}, "set-date-time", [("is-auto", 0), ("time", "2026-10-23 15:10:00")])
_ue("set_udp_mtu", {"mtu": 1316}, "set-udp-mtu", [("udp-mtu", 1316)])
_ue("set_web", {"http_port": 8000, "https": True}, "set-web", [("http-port", 8000), ("is-https", 1), ("https-port", 443)], model=_UE_V2)
_ue_upload("upload_certificate", "upload-cert", "LS0t", b"---", "server.crt", model=_UE_V2)
_ue_upload("upload_certificate_key", "upload-cert-key", "LS0t", b"---", "server.key", model=_UE_V2)
_ue("generate_certificate", {}, "generate-cert-key", model=_UE_V2)
_ue("set_theme", {"theme": "dark"}, "set-theme", [("theme", 1)], model=_UE_V2)
_ue("set_hls_push_segments", {"count": 3, "duration": 2}, "set-hls-push-segment", [("seg-count", 3), ("seg-duration", 2)], model=_UE_V2)
_ue("set_lcd_control", {"touch": True, "flip": False, "page": "live"}, "set-lcd-control",
    [("no-touch", 1), ("no-flip", 0), ("page-idx", 1)], model=_UE_V2)
_ue("set_live_keep_last", {"enabled": True}, "set-live-keep-last", [("live-keep-last", 1)], model=_UE_V2)
_ue("set_ndi_bridge", {"enabled": True, "name": "Bridge", "address": "10.0.0.5", "port": 5990, "key": "k3y"}, "set-ndi-bridge-config",
    [("enable-ndi-bridge", 1), ("groups", "public"), ("bridge-name", "Bridge"), ("ip-addr", "10.0.0.5"), ("port", 5990),
     ("encryp-key", "k3y")], model=_UE_V2)
_ue("add_token", {"token": "Q7kMpTN7", "expire": -1, "user": "Admin"}, "add-token",
    [("token", "Q7kMpTN7"), ("expire", -1), ("user", "Admin")], model=_UE_V2)
_ue("delete_token", {"token": "Q7kMpTN7"}, "del-token", [("token", "Q7kMpTN7")], model=_UE_V2)
_ue("get_tokens", {}, "get-tokens", model=_UE_V2)
_ue_upload("upload_zen_master_key", "upload-zen-master-key", "LS0t", b"---", "zen-master.key", model=_UE_V2)
_ue("set_zen_master", {"enabled": True, "user": "zixi", "host": "zen.example.com", "tunnel_port": 2088}, "set-zen-master-config",
    [("enable-zen-master", 1), ("user-name", "zixi"), ("host", "zen.example.com"), ("tunnel-port", 2088)], model=_UE_V2)

# ── Input (AIO) ──
_ue("select_input_source", {"source": "mixer"}, "select-input-source", [("input-source", 4)], model=_UE_V2)
_ue("set_video_mixer", {"format": "hdmi", "hdmi_on_top": True, "type": "side-by-side", "location": "one-third"},
    "set-video-mixer-config", [("input-device", 2), ("is-hdmi-top", 1), ("type", 1), ("location", 2)], model=_UE_V2)
_ue("set_4k60_input", {"enabled": True}, "enable-4k60-input", [("enable-4k60-input", 1)], model=_UE_V2)

# ── Encoding ──
_UE_VIDEO = {"stream": "sub", "follow_input": False, "width": 1280, "height": 720, "frame_rate": "30", "profile": 2, "kbps": 1024,
             "gop": 1, "full_range": True}
_UE_VIDEO_Q = [("stream", 1), ("is-auto", 0), ("cx", 1280), ("cy", 720), ("duration", 333333), ("kbps", 1024), ("gop", 1),
               ("fourcc", 0), ("profile", 2), ("cbrstat", 60), ("fullrange", 1), ("codec", 0), ("is-vbr", 0), ("min-vbr-qp", 0),
               ("max-vbr-qp", 51), ("is-time-code-sei", 0), ("is-closed-caption-sei", 0), ("ar-convert-mode", 2)]
_ue("set_video", _UE_VIDEO, "set-video", _UE_VIDEO_Q)
_ue2("set_video", {**_UE_VIDEO, "stream": "main", "codec": "hevc", "frame_rate": "follow", "timecode_sei": "source", "rotation": "90"},
     "set-video", [("stream", 0), ("is-auto", 0), ("cx", 1280), ("cy", 720), ("duration", 0), ("kbps", 1024), ("gop", 1), ("fourcc", 0),
                   ("profile", 2), ("cbrstat", 60), ("fullrange", 1), ("codec", 1), ("is-vbr", 0), ("min-vbr-qp", 0), ("max-vbr-qp", 51),
                   ("is-time-code-sei", 2), ("is-closed-caption-sei", 0), ("ar-convert-mode", 2), ("rotation", 90)])
_ue("set_audio_bitrate", {"kbps": "128"}, "set-audio", [("kbps", 128)])
_ue("set_audio_stream", {"stream": 2, "channels": "2", "kbps": "192", "ch1": 0, "ch2": 1}, "set-audio",
    [("stream", 1), ("channels", 2), ("kbps", 192), ("ch0", 0), ("ch1", 1), ("ch2", 0), ("ch3", 0), ("ch4", 0), ("ch5", 0),
     ("ch6", 0), ("ch7", 0), ("use-lfe", 0)], model=_UE_V2)
_ue("set_audio_stream_count", {"count": 2}, "set-audio-stream-count", [("audio-stream-count", 2)], model=_UE_V2)
_ue("select_audio_channels", {"mask": 3}, "select-audio-channels", [("channel-mask", 3)], model=_UE_V2)
_ue("set_deinterlace", {"enabled": True}, "enable-deinterlace", [("enable-deinterlace", 1)])
_ue("set_sub_stream_enabled", {"enabled": False}, "set-enable-stream1", [("enable", 0)], model=_UE_V2)
_ue("make_crop_thumbnail", {"stream": "sub"}, "set-in-cropping", [("stream-idx", 1)], model=_UE_V2)
_ue("set_crop", {"stream": "main", "enabled": True, "x": 160, "y": 90, "width": 1600, "height": 900}, "set-crop",
    [("stream-idx", 0), ("is-use", 1), ("x-offset", 160), ("y-offset", 90), ("act-w", 1600), ("act-h", 900)], model=_UE_V2)
_ue("set_3d_output", {"enabled": True}, "set-3d-output-config", [("enable", 1), ("mode", 0)], model="hdmi-plus")

# ── Streaming servers ──
_UE_RTMP = {"type": "youtube", "name": "YouTube", "url": "rtmp://a.rtmp.youtube.com/live2", "key": "abcd-1234"}
_UE_RTMP_Q = [("type", 2), ("name", "YouTube"), ("stream-index", 0), ("url", "rtmp://a.rtmp.youtube.com/live2"), ("key", "abcd-1234"),
              ("is-auth", 0), ("user", ""), ("passwd", ""), ("token", ""), ("event-data", ""), ("net-mode", 1)]
_ue("add_rtmp_server", _UE_RTMP, "add-server", _UE_RTMP_Q)
_ue2("add_rtmp_server", {**_UE_RTMP, "audio": 2}, "add-server", _UE_RTMP_Q + [("audio", 1)])
_ue("set_rtmp_server", {**_UE_RTMP, "id": 3}, "set-server", [("id", 3)] + _UE_RTMP_Q)
_ue2("set_rtmp_server", {**_UE_RTMP, "id": 3, "type": "wowza"}, "set-server",
     [("id", 3), ("type", 4)] + _UE_RTMP_Q[1:] + [("audio", 0)])
_UE_RTSP = {"name": "RTSP", "max_connections": 4, "main_name": "main", "sub_name": "sub", "sub_enabled": True}
_UE_RTSP_Q = [("type", 100), ("name", "RTSP"), ("port", 554), ("max-connection", 4), ("is-auth", 0), ("user", ""), ("passwd", ""),
              ("is-main", 1), ("main-stream-name", "main"), ("is-sub", 1), ("sub-stream-name", "sub"), ("is-audio", 1)]
_ue("add_rtsp_server", _UE_RTSP, "add-server", _UE_RTSP_Q)
_ue2("add_rtsp_server", _UE_RTSP, "add-server", _UE_RTSP_Q + [("audio", 0)])
_ue("set_rtsp_server", {**_UE_RTSP, "id": 1}, "set-server", [("id", 1)] + _UE_RTSP_Q)
_ue2("set_rtsp_server", {**_UE_RTSP, "id": 1}, "set-server", [("id", 1)] + _UE_RTSP_Q + [("audio", 0)])
_UE_SRT = {"name": "SRT out", "address": "10.10.1.20", "port": 9000, "encryption": "aes-128", "passphrase": "secret1234"}
_UE_SRT_Q = [("type", 120), ("name", "SRT out"), ("stream-index", 0), ("url", "10.10.1.20"), ("port", 9000), ("latency", 120),
             ("bandwidth", 25), ("aes", 16), ("ase-word", "secret1234"), ("stream-id", ""), ("mtu", 1496), ("conn-timeout", 3000),
             ("retry-duration", 10000), ("net-mode", 1)]
_UE_SRT_Q2 = [("audio-streams", 1), ("token", ""), ("event-data", ""), ("is-custom-pid", 0), ("pmt-pid", 4096), ("pcr-pid", 256),
              ("video-pid", 256), ("audio-pid", 257)]
_ue("add_srt_caller", _UE_SRT, "add-server", _UE_SRT_Q)
_ue2("add_srt_caller", _UE_SRT, "add-server", _UE_SRT_Q + _UE_SRT_Q2)
_ue("set_srt_caller", {**_UE_SRT, "id": 2}, "set-server", [("id", 2)] + _UE_SRT_Q)
_ue2("set_srt_caller", {**_UE_SRT, "id": 2, "type": "wowza"}, "set-server", [("id", 2), ("type", 122)] + _UE_SRT_Q[1:] + _UE_SRT_Q2)
_UE_SRTL = {"name": "Listener", "max_connections": 2, "latency": 200}
_UE_SRTL_Q = [("type", 121), ("name", "Listener"), ("stream-index", 0), ("port", 8000), ("max-connection", 2), ("latency", 200),
              ("bandwidth", 25), ("aes", 0), ("ase-word", ""), ("mtu", 1496)]
_UE_PID_Q = [("is-custom-pid", 0), ("pmt-pid", 4096), ("pcr-pid", 256), ("video-pid", 256), ("audio-pid", 257)]
_ue("add_srt_listener", _UE_SRTL, "add-server", _UE_SRTL_Q)
_ue2("add_srt_listener", _UE_SRTL, "add-server", _UE_SRTL_Q + [("audio", 0)] + _UE_PID_Q)
_ue("set_srt_listener", {**_UE_SRTL, "id": 4}, "set-server", [("id", 4)] + _UE_SRTL_Q)
_ue2("set_srt_listener", {**_UE_SRTL, "id": 4}, "set-server", [("id", 4)] + _UE_SRTL_Q + [("audio", 0)] + _UE_PID_Q)
_UE_NDI = {"name": "NDI HX", "source_name": "Stage", "transport": "multicast", "mcast_address": "239.255.0.0",
           "mcast_mask": "255.255.0.0", "mcast_ttl": 1}
_UE_NDI_Q = [("type", 130), ("name", "NDI HX"), ("source-name", "Stage"), ("group-name", "public"), ("enable-discovery", 0),
             ("discovery-server", ""), ("transport-mode", 1), ("mcast-addr", "239.255.0.0"), ("mcast-mask", "255.255.0.0"),
             ("mcast-ttl", 1), ("enable-fail-over", 0), ("fail-over-ndi-name", ""), ("fail-over-ip-addr", ""), ("enable-web-control", 0)]
_ue("add_ndi_hx_server", _UE_NDI, "add-server", _UE_NDI_Q)
_ue2("add_ndi_hx_server", _UE_NDI, "add-server", _UE_NDI_Q + [("audio", 0)])
_ue("set_ndi_hx_server", {**_UE_NDI, "id": 0}, "set-server", [("id", 0)] + _UE_NDI_Q)
_ue2("set_ndi_hx_server", {**_UE_NDI, "id": 0}, "set-server", [("id", 0)] + _UE_NDI_Q + [("audio", 0)])
_UE_HLSS = {"name": "HLS", "main_name": "live", "sub_name": "live-sub"}
_UE_HLSS_Q = [("type", 131), ("name", "HLS"), ("is-main", 1), ("main-stream-name", "live"), ("is-sub", 0), ("sub-stream-name", "live-sub")]
_ue("add_hls_server_streams", _UE_HLSS, "add-server", _UE_HLSS_Q)
_ue("set_hls_server_streams", {**_UE_HLSS, "id": 5}, "set-server", [("id", 5)] + _UE_HLSS_Q)
_ue("add_hls_server", {"name": "HLS", "stream_name": "live"}, "add-server",
    [("type", 131), ("name", "HLS"), ("stream-index", 0), ("stream-name", "live"), ("audio", 0)], model=_UE_V2)
_ue("set_hls_server", {"id": 5, "name": "HLS", "stream": "sub", "stream_name": "live", "audio": 3}, "set-server",
    [("id", 5), ("type", 131), ("name", "HLS"), ("stream-index", 1), ("stream-name", "live"), ("audio", 2)], model=_UE_V2)
_UE_TS = {"type": "rtp", "name": "TS", "address": "239.1.1.1", "port": 5004}
_UE_TS_Q = [("type", 133), ("name", "TS"), ("stream-index", 0), ("url", "239.1.1.1"), ("port", 5004), ("net-mode", 1)]
_ue("add_ts_server", _UE_TS, "add-server", _UE_TS_Q)
_ue2("add_ts_server", {**_UE_TS, "custom_pid": True, "pmt_pid": 4096}, "add-server",
     _UE_TS_Q + [("audio", 0), ("is-custom-pid", 1), ("pmt-pid", 4096), ("pcr-pid", 256), ("video-pid", 256), ("audio-pid", 257)])
_ue("set_ts_server", {**_UE_TS, "id": 6, "type": "udp"}, "set-server", [("id", 6), ("type", 132)] + _UE_TS_Q[1:])
_ue2("set_ts_server", {**_UE_TS, "id": 6}, "set-server", [("id", 6)] + _UE_TS_Q + [("audio", 0)] + _UE_PID_Q)
_ue("add_tvu_server", {"name": "TVU"}, "add-server", [("type", 140), ("name", "TVU"), ("port", 6539), ("stream-index", 0), ("audio", 0)],
    model=_UE_V2)
_ue("set_tvu_server", {"id": 7, "name": "TVU", "port": 6540}, "set-server",
    [("id", 7), ("type", 140), ("name", "TVU"), ("port", 6540), ("stream-index", 0), ("audio", 0)], model=_UE_V2)
_UE_ZIXI = {"name": "Zixi", "links": "10.10.35.34,2088,0,8192,0;10.10.35.34,2088,1,8192,1", "stream_id": "cam1", "latency": 1000,
            "fec_overhead": 20}
_UE_ZIXI_Q = [("type", 144), ("name", "Zixi"), ("stream-index", 0), ("links", "10.10.35.34,2088,0,8192,0;10.10.35.34,2088,1,8192,1"),
              ("bond-links", 0), ("stream-id", "cam1"), ("passwd", ""), ("latency", 1000), ("fec-overhead", 20), ("aes", 0), ("ase-word", ""),
              ("ignore-tls-cert-error", 0), ("audio-streams", 1)]
_ue("add_zixi_server", _UE_ZIXI, "add-server", _UE_ZIXI_Q, model=_UE_V2)
_ue("set_zixi_server", {**_UE_ZIXI, "id": 8}, "set-server", [("id", 8)] + _UE_ZIXI_Q, model=_UE_V2)
_UE_RIST = {"name": "RIST", "profile": "main", "address": "10.10.1.30", "port": 6000, "latency": 1000, "bandwidth": 10,
            "encryption": "aes-256", "passphrase": "pw", "audio_streams": 3}
_UE_RIST_Q = [("type", 150), ("name", "RIST"), ("stream-index", 0), ("profile", 1), ("url", "10.10.1.30"), ("port", 6000), ("latency", 1000),
              ("bandwidth", 10), ("aes", 32), ("ase-word", "pw"), ("stream-id", ""), ("mtu", 1496), ("net-mode", 1), ("audio-streams", 3)]
_ue("add_rist_caller", _UE_RIST, "add-server", _UE_RIST_Q + _UE_PID_Q, model=_UE_V2)
_ue("set_rist_caller", {**_UE_RIST, "id": 9}, "set-server", [("id", 9)] + _UE_RIST_Q + _UE_PID_Q, model=_UE_V2)
_UE_RISTL = {"name": "RIST L", "max_connections": 3, "latency": 500, "bandwidth": 10}
_UE_RISTL_Q = [("type", 151), ("name", "RIST L"), ("stream-index", 0), ("port", 6000), ("max-connection", 3), ("latency", 500),
               ("bandwidth", 10), ("aes", 0), ("ase-word", ""), ("mtu", 1496), ("audio-streams", 1)]
_ue("add_rist_listener", _UE_RISTL, "add-server", _UE_RISTL_Q + _UE_PID_Q, model=_UE_V2)
_ue("set_rist_listener", {**_UE_RISTL, "id": 10}, "set-server", [("id", 10)] + _UE_RISTL_Q + _UE_PID_Q, model=_UE_V2)
_ue("delete_server", {"id": 1}, "del-server", [("id", 1)])
_ue("enable_server", {"id": 1, "enabled": True}, "enable-server", [("id", 1), ("is-use", 1)],
    http_reply={"status": 200, "body": '{"result":-45}'}, expect_result={"error": {"error": "device_error", "code": "-45"}})
_UE_TEST_Q = [("type", 0), ("name", "Test"), ("stream-index", 0), ("url", "rtmp://10.0.0.9/live"), ("key", "k"), ("is-auth", 0),
              ("user", ""), ("passwd", ""), ("token", ""), ("event-data", ""), ("net-mode", 1)]
_ue("test_rtmp_server", {"type": "rtmp", "name": "Test", "url": "rtmp://10.0.0.9/live", "key": "k"}, "start-test-server", _UE_TEST_Q,
    http_reply={"status": 200, "body": '{"result":1}'}, expect_result={"error": {"error": "device_error", "code": "1"}})
_ue2("test_rtmp_server", {"type": "rtmp", "name": "Test", "url": "rtmp://10.0.0.9/live", "key": "k"}, "start-test-server",
     _UE_TEST_Q + [("audio", 0)])
_UE_TSRT_Q = [("type", 120), ("name", "Test"), ("stream-index", 1), ("url", "10.0.0.9"), ("port", 9000), ("latency", 120),
              ("bandwidth", 25), ("aes", 0), ("ase-word", ""), ("stream-id", "s1"), ("net-mode", 2)]
_ue("test_srt_server", {"name": "Test", "stream": "sub", "address": "10.0.0.9", "port": 9000, "stream_id": "s1", "net_mode": "wifi"},
    "start-test-server", _UE_TSRT_Q)
_ue2("test_srt_server", {"name": "Test", "stream": "sub", "address": "10.0.0.9", "port": 9000, "stream_id": "s1", "net_mode": "wifi"},
     "start-test-server", _UE_TSRT_Q + [("audio-streams", 1)])
_ue("stop_server_test", {}, "stop-test-server")
_ue("clear_server_test", {}, "clear-test-server")
_ue("get_ndi_sources", {}, "get-ndi-sources")
_ue("set_ndi_find", {"groups": "public,studio", "discovery": True, "discovery_server": "10.0.0.5"}, "set-ndi-find-config",
    [("group-name", "public,studio"), ("enable-discovery", 1), ("discovery-server", "10.0.0.5")], model=_UE_V2)
_ue("set_ndi_hx3_enabled", {"enabled": True}, "enable-ndi-hx3", [("enable-ndi-hx3", 1)], model=_UE_V2)

# ── Recording, NAS, storage ──
_ue("get_rec_channels", {}, "get-rec-channels", model=_UE_V2)
_UE_REC = {"storage": "sd", "mode": "loop", "folder": "Show/Day1", "prefix": "CAM", "format": "mov", "segment_minutes": "30", "audio": 1}
_UE_REC_Q = [("type", 1), ("stream-index", 0), ("mode", 2), ("dir-name", "Show/Day1"), ("file-prefix", 0), ("prefix-name", "CAM"),
             ("file-suffix", 1), ("time-unit", 30), ("audio", 0)]
_ue("add_rec_channel", _UE_REC, "add-rec-channel", _UE_REC_Q, model=_UE_V2)
_ue("set_rec_channel", {**_UE_REC, "id": 1}, "set-rec-channel", [("id", 1)] + _UE_REC_Q, model=_UE_V2)
_ue("delete_rec_channel", {"id": 1}, "del-rec-channel", [("id", 1)], model=_UE_V2)
_ue("enable_rec_channel", {"id": 1, "enabled": True}, "enable-rec-channel", [("id", 1), ("is-use", 1)], model=_UE_V2)
_ue("clear_rec_channel_status", {"id": 1}, "clear-rec-status", [("id", 1)], model=_UE_V2)
_ue("set_auto_record", {"enabled": True, "trigger": "usb"}, "set-auto-rec", [("is-auto", 1), ("trigger-mode", 1)], model=_UE_V2)
_UE_NAS = {"protocol": "cifs", "address": "10.10.14.193", "mount": "/share/rec", "user": "rec", "password": "pw"}
_UE_NAS_Q = [("proto", 1), ("addr", "10.10.14.193"), ("mount", "/share/rec"), ("user", "rec"), ("passwd", "pw")]
_ue("add_nas", _UE_NAS, "add-nas", _UE_NAS_Q, model=_UE_V2)
_ue("set_nas", {**_UE_NAS, "id": 0}, "set-nas", [("id", 0)] + _UE_NAS_Q, model=_UE_V2)
_ue("delete_nas", {"id": 0}, "del-nas", [("id", 0)], model=_UE_V2)
_ue("enable_nas", {"id": 0, "enabled": True}, "enable-nas", [("id", 0), ("is-use", 1)], model=_UE_V2)
_ue("get_media_files", {"storage": "sd", "count": 3}, "get-media-files", [("disk-type", 1), ("start", 0), ("count", 3)], model=_UE_V2)
V.append({"spec": UE, "command": "delete_media_files", "model": _UE_V2, "input": {"storage": "sd", "files": ["VID_6_14.mp4"]},
          "expect_request": {"method": "POST", "target": "/usapi?method=del-media-files",
                             "body": '{"disk-type":1,"media-files":["VID_6_14.mp4"]}'}})
_ue("start_disk_format", {"storage": "usb"}, "start-disk-format", [("disk-type", 0)], model=_UE_V2)
_ue("clear_disk_format", {}, "clear-disk-format", model=_UE_V2)
_ue("start_disk_test", {"storage": "sd"}, "start-disk-test", [("disk-type", 1)], model=_UE_V2)
_ue("stop_disk_test", {}, "stop-disk-test", model=_UE_V2)
_ue("clear_disk_test", {}, "clear-disk-test", model=_UE_V2)
for _c, _m in [("format_usb_legacy", "start-format-usb"), ("format_sd_legacy", "start-format-sd"),
               ("clear_format_usb_legacy", "clear-format-usb"), ("clear_format_sd_legacy", "clear-format-sd"),
               ("test_usb_legacy", "start-test-usb"), ("stop_test_usb_legacy", "stop-test-usb"),
               ("clear_test_usb_legacy", "clear-test-usb")]:
    _ue(_c, {}, _m, model=_UE_V2)

# ── Uploading files ──
_UE_FTP = {"protocol": "sftp", "url": "ftp.example.com", "port": 22, "folder": "rec", "user": "u", "password": "p"}
_UE_FTP_Q = [("type", 0), ("net-mode", 1), ("proto", 1), ("url", "ftp.example.com"), ("port", 22), ("dir-name", "rec"), ("encryption", 0),
             ("login-type", 1), ("user-name", "u"), ("passwd", "p"), ("mode", 1)]
_ue("add_ftp_upload_server", _UE_FTP, "send-file-add-server", _UE_FTP_Q, model=_UE_V2)
_ue("set_ftp_upload_server", {**_UE_FTP, "id": 0}, "send-file-set-server", [("id", 0)] + _UE_FTP_Q, model=_UE_V2)
_ue("test_ftp_upload_server", _UE_FTP, "send-file-start-test-server", _UE_FTP_Q, model=_UE_V2)
_UE_CLOUD = {"type": "dropbox", "account_id": "dbid:AA", "user": "Ops", "access_token": "at", "refresh_token": "rt", "folder": "/rec"}
_UE_CLOUD_Q = [("type", 2), ("net-mode", 1), ("account-id", "dbid:AA"), ("photo-path", ""), ("user-name", "Ops"), ("access-token", "at"),
               ("refresh-token", "rt"), ("dir-name", "/rec"), ("parents", ""), ("privacy", "")]
_ue("add_cloud_upload_server", _UE_CLOUD, "send-file-add-server", _UE_CLOUD_Q, model=_UE_V2)
_ue("set_cloud_upload_server", {**_UE_CLOUD, "id": 1}, "send-file-set-server", [("id", 1)] + _UE_CLOUD_Q, model=_UE_V2)
_ue("delete_upload_server", {"id": 1}, "send-file-del-server", [("id", 1)], model=_UE_V2)
_ue("enable_upload_server", {"id": 0, "enabled": True}, "send-file-enable-server", [("id", 0), ("is-use", 1)], model=_UE_V2)
_ue("stop_upload_server_test", {}, "send-file-stop-test-server", model=_UE_V2)
_ue("clear_upload_server_test", {}, "clear-test-send-file", model=_UE_V2)
for _c, _m in [("add_upload_files", "send-file-add"), ("delete_upload_files", "send-file-del")]:
    V.append({"spec": UE, "command": _c, "model": _UE_V2, "input": {"storage": "sd", "files": ["VID_6_14.mp4", "VID_6_15.mp4"]},
              "expect_request": {"method": "POST", "target": f"/usapi?method={_m}",
                                 "body": '{"disk-type":1,"media-files":["VID_6_14.mp4","VID_6_15.mp4"]}'}})
_ue("clear_upload_files", {}, "send-file-clear", model=_UE_V2)
_ue("get_upload_status", {"storage": "sd", "count": 10}, "send-file-get-status", [("disk-type", 1), ("start", 0), ("count", 10)],
    model=_UE_V2)
_ue("set_auto_upload", {"enabled": True}, "send-file-set-is-auto", [("is-auto", 1)], model=_UE_V2)
_ue("start_upload", {}, "send-file-start", model=_UE_V2)
_ue("stop_upload", {}, "send-file-stop", model=_UE_V2)

# ── Overlays ──
_ue("get_overlays", {}, "get-surfaces", model=_UE_V2)
_ue("get_overlay_images", {}, "get-images", model=_UE_V2)
_ue_upload("add_overlay_image", "add-image", "iVBORw==", b"\x89PNG", "overlay.png", model=_UE_V2)
_ue("delete_overlay_image", {"id": 1}, "del-image", [("id", 1)], model=_UE_V2)
_UE_TEXT = {"stream": "main", "position": "bottom-center", "margin": 20, "font": "lato", "font_size": 48, "font_weight": "bold",
            "color": "#ffffff", "width": 800, "height": 80, "text": "Live: Studio A"}
_UE_FONT_Q = [("x-offset", 0), ("y-offset", 0), ("margin", 20), ("font-family", 1), ("font-size", 48), ("font-style", 0),
              ("font-weight", 1), ("font-color", "#ffffff"), ("line-width", 0), ("line-color", ""), ("width", 800), ("height", 80)]
_ue("add_text_overlay", _UE_TEXT, "add-surface",
    [("type", 0), ("stream-index", 0), ("position", 6)] + _UE_FONT_Q + [("text", "Live: Studio A")], model=_UE_V2)
_ue("set_text_overlay", {**_UE_TEXT, "id": 3}, "set-surface",
    [("id", 3), ("type", 0), ("stream-index", 0), ("position", 6)] + _UE_FONT_Q + [("text", "Live: Studio A")], model=_UE_V2)
_UE_CLOCK = {**{k: v for k, v in _UE_TEXT.items() if k != "text"}, "format": "hms"}
_ue("add_clock_overlay", _UE_CLOCK, "add-surface",
    [("type", 1), ("stream-index", 0), ("position", 6)] + _UE_FONT_Q + [("text", 6)], model=_UE_V2)
_ue("set_clock_overlay", {**_UE_CLOCK, "id": 4}, "set-surface",
    [("id", 4), ("type", 1), ("stream-index", 0), ("position", 6)] + _UE_FONT_Q + [("text", 6)], model=_UE_V2)
_UE_IMG_Q = [("stream-index", 1), ("position", 0), ("x-offset", 40), ("y-offset", 30), ("margin", 0), ("index", 2), ("alpha", 200),
             ("scale", 50)]
_ue("add_image_overlay", {"stream": "sub", "position": "custom", "x": 40, "y": 30, "image": 2, "alpha": 200, "scale": 50}, "add-surface",
    [("type", 2)] + _UE_IMG_Q, model=_UE_V2)
_ue("set_image_overlay", {"id": 5, "stream": "sub", "position": "custom", "x": 40, "y": 30, "image": 2, "alpha": 200, "scale": 50},
    "set-surface", [("id", 5), ("type", 2)] + _UE_IMG_Q, model=_UE_V2)
_ue("delete_overlay", {"id": 5}, "del-surface", [("id", 5)], model=_UE_V2)
_ue("enable_overlay", {"id": 5, "enabled": False}, "enable-surface", [("id", 5), ("is-use", 0)], model=_UE_V2)
_ue("set_overlays_enabled", {"stream": "main", "enabled": True}, "set-enable-surface", [("stream-index", 0), ("enable", 1)], model=_UE_V2)

# ── Scheduler ──
_ue("get_schedules", {"channel": "record"}, "get-schedulers", [("id", -1), ("chn-type", 1), ("chn-id", -1)], model=_UE_V2)
_UE_SCHED = {"channel": "live", "channel_id": 2, "title": "Sunday", "description": "Service", "begin": "2026-11-01 10:00:00",
             "end": "2026-11-01 11:30:00", "repeat": True, "pattern": "weekly", "weekdays": 64, "ends": "count", "count": 10}
_UE_SCHED_TAIL = [("chn-type", 0), ("chn-id", 2), ("title", "Sunday"), ("desc", "Service"), ("is-full-day", 0),
                  ("time-begin", "2026-11-01 10:00:00"), ("time-end", "2026-11-01 11:30:00"), ("is-repeat", 1)]
_UE_SCHED_REP = [("repeat-interval", 1), ("repeat-is-workday", 0), ("repeat-wdays", 64), ("repeat-mday", 0), ("repeat-wdy", 0),
                 ("repeat-idx", 0), ("repeat-month", 0), ("end-event", 1), ("end-interval", 10), ("end-dateline", "")]
_ue("add_schedule", _UE_SCHED, "add-scheduler", _UE_SCHED_TAIL + [("repeat-event", 1)] + _UE_SCHED_REP, model=_UE_V2)
_ue("set_schedule", {**_UE_SCHED, "id": 7}, "set-scheduler", [("id", 7)] + _UE_SCHED_TAIL + [("repeat-event", 2)] + _UE_SCHED_REP,
    model=_UE_V2)
_ue("delete_schedule", {"id": 7, "channel": "reboot"}, "del-scheduler", [("id", 7), ("chn-type", 2), ("chn-id", -1)], model=_UE_V2)
_ue("enable_schedule", {"id": 7, "channel": "live", "enabled": True}, "enable-scheduler",
    [("id", 7), ("chn-type", 0), ("chn-id", -1), ("is-use", 1)], model=_UE_V2)
_ue_upload("upload_calendar", "upload-icalendar-file", "QkVHSU4=", b"BEGIN", "schedule.ics", model=_UE_V2)
_ue("import_calendar_file", {"channel": "record", "file_name": "schedule.ics"}, "import-scheduler",
    [("chn-type", 1), ("chn-id", -1), ("import-type", 1), ("file-name", "schedule.ics")], model=_UE_V2)
_ue("import_calendar_url", {"channel": "live", "url": "https://calendar.example.com/a.ics"}, "import-scheduler",
    [("chn-type", 0), ("chn-id", -1), ("import-type", 2), ("url", "https://calendar.example.com/a.ics")], model=_UE_V2)
_ue("clear_calendar_source", {"channel": "live"}, "import-scheduler", [("chn-type", 0), ("chn-id", -1), ("import-type", 0)], model=_UE_V2)
_ue("sync_calendar", {"channel": "live"}, "sync-scheduler", [("chn-type", 0), ("chn-id", -1)], model=_UE_V2)

# ── Users ──
_ue("logout", {}, "logout")
_ue("get_users", {}, "get-users")
# MD5 of "Admin" (the documents' demos) and of "Test".
_UE_MD5_ADMIN = "e3afed0047b08059d0fada10f400c1e5"
_UE_MD5_TEST = "0cbc6611f5540bd0809a388dc95a615b"
_ue("add_user", {"user": "Test", "password": "Test"}, "add-user", [("id", "Test"), ("pass", _UE_MD5_TEST)])
_ue2("add_user", {"user": "Test", "password": "Test", "admin": True}, "add-user", [("id", "Test"), ("pass", _UE_MD5_TEST), ("is-admin", 1)])
_ue("delete_user", {"user": "Test"}, "del-user", [("id", "Test")])
_ue("change_password", {"old_password": "Admin", "new_password": "Test"}, "ch-password",
    [("pass", _UE_MD5_ADMIN), ("new-pass", _UE_MD5_TEST)])
_ue2("change_password", {"old_password": "Admin", "new_password": "Test"}, "ch-passwd", [("pass", _UE_MD5_ADMIN), ("new-pass", _UE_MD5_TEST)])
_ue("set_user_password", {"user": "Test", "password": "Admin"}, "set-password", [("id", "Test"), ("pass", _UE_MD5_ADMIN)])
_ue2("set_user_password", {"user": "Test", "password": "Admin"}, "set-passwd", [("id", "Test"), ("pass", _UE_MD5_ADMIN)])

# ── Network ──
_ue("set_network_dhcp", {"interface": "wifi"}, "set-net", [("type", 0), ("is-dhcp", 1)])
_ue("set_network_static", {"address": "10.10.107.212", "netmask": "255.255.0.0", "gateway": "10.10.0.1", "dns": "10.0.0.3"}, "set-net",
    [("type", 1), ("is-dhcp", 0), ("ip", "10.10.107.212"), ("mask", "255.255.0.0"), ("router", "10.10.0.1"), ("dns", "10.0.0.3")])
_ue("set_usb_network", {"address": "192.168.66.1"}, "set-rndis-config", [("addr", "192.168.66.1")], model=_UE_V2)
_ue("set_network_priority", {"network": "mobile"}, "set-net-prior", [("net-prior", 0)], model=_UE_V2)
_ue("set_wifi_enabled", {"enabled": False}, "enable-station", [("enable-station", 0)], model=_UE_V2)
_ue("scan_wifi", {}, "scan-wifi-results")
_ue("connect_wifi_first", {"ssid": "magewell", "password": "1111111", "security": "wpa"}, "connect-wifi-first",
    [("name", "magewell"), ("passwd", "1111111"), ("secu", 2), ("is-auto", 1)])
_ue2("connect_wifi_first", {"ssid": "Corp", "password": "pw", "security": "wpa2-eap", "identity": "ops", "hidden": True},
     "connect-wifi-first", [("name", "Corp"), ("passwd", "pw"), ("secu", 4), ("is-auto", 1), ("is-hide", 1), ("identity", "ops")])
_ue("connect_wifi", {"ssid": "magewell"}, "connect-wifi", [("name", "magewell"), ("is-auto", 1)])
_ue("cancel_connect_wifi", {"ssid": "magewell"}, "cancel-connect-wifi", [("name", "magewell")])
_ue("disconnect_wifi", {}, "disconnect-wifi")
_ue("forget_wifi", {"ssid": "magewell"}, "forget-wifi", [("name", "magewell")])
_ue("set_wifi_auto_connect", {"ssid": "magewell", "auto": False}, "set-connect-wifi-auto", [("name", "magewell"), ("is-auto", 0)])
_ue("clear_connect_wifi", {}, "clear-connect-wifi")

# ── Firmware ──
_UE_FW_BODY, _UE_FW_TYPE = _ue_form(b"MWF\x00", "firmware.mwf")
V.append({"spec": UE, "command": "upload_firmware", "input": {"file": "TVdGAA=="},
          "expect_request": {"method": "POST", "target": "/usapi?method=upload-update-file", "body_hex": _UE_FW_BODY.hex(),
                             "content_type": _UE_FW_TYPE},
          "http_reply": {"status": 200, "body": '{"status":0,"up-to-date":true,"version":"1.1.72","size":11890776}'},
          "expect_result": {"ok": {"kind": "value", "value": {"status": 0, "up-to-date": True, "version": "1.1.72", "size": 11890776}}}})
_ue("update_firmware", {"mode": "online"}, "update", [("mode", "online")])
_ue("cancel_firmware_download", {}, "cancel-download")
_ue("check_firmware_online", {}, "online-update-check")
_ue("clear_firmware_check", {}, "clear-check-update")
_ue("clear_firmware_update", {}, "clear-upgrade")
_ue("set_auto_check_update", {"enabled": False}, "set-enable-check-update", [("is-check-update", 0)])

# ── EDID ──
_ue("get_edid_config", {}, "get-edid-config")
_ue("set_edid_config", {"smart": True, "keep_last": False, "add_audio": True, "limit_pixel_clock": True}, "set-edid-config",
    [("smart-edid", 1), ("keep-last", 0), ("add-audio", 1), ("limit-pixel-clock", 1)])
_ue_upload("import_edid", "import-edid", "AP///w==", b"\x00\xff\xff\xff", "edid.bin")
_ue("reset_edid", {}, "set-default-edid")
_ue("get_loop_through_edid", {}, "get-loop-through-edid")

# ── Control Hub ──
_ue("get_cloud_status", {}, "cloud-status", [("version", 1)])
_ue2("get_cloud_status", {}, "cloud-status", [("version", 1)], port=8070, path="/cloud-api")
_ue("register_cloud", {"slot": 1, "code": "1234", "address": "10.0.1.32"}, "cloud-reg-ex",
    [("id", 1), ("cloud-code", "1234"), ("cloud-ip-addr", "10.0.1.32"), ("cloud-http-port", 80), ("cloud-enable-https", 0),
     ("cloud-https-port", 443)], port=8070, path="/cloud-api")
_ue("unregister_cloud", {"slot": 0}, "cloud-unreg-ex", [("id", 0)], port=8070, path="/cloud-api")


# ── Telemetry: the documents' example replies ──
def _uet(name, path, body, state, **extra):
    telemetry(UE, name, inbound_http={"path": path, "body": body if isinstance(body, str) else json.dumps(body)},
              expect_state=state, **extra)


# get-info: the Ultra Encode SDI example (Ultra Encode API, get-info),
# shortened, and the AIO's product and recording ranges.
_uet("info", "/usapi?method=get-info", {
    "result": 0, "mac-addr": {"eth": "d0:c8:57:80:3a:70", "wifi": "d0:c8:57:80:3a:71", "blue": "d0:c8:57:80:3a:72"},
    "snapshot": "/tmp/sbox-snapshot/sbox-quarter.jpg",
    "product": {"sn": "A305200908002", "product-id": 773, "hardware-ver": "A", "firmware-id": 1, "firmware-ver-s": "1.3.540",
                "factory-firmware-ver-s": "1.3.275", "product-name": "Ultra Encode", "module-name": "Ultra Encode SDI",
                "manu-name": "MAGEWELL", "features": 1, "max-lock-count": 2, "live-support": 2047, "cloud-support": 1},
    "nosignal": {"max-count": 4, "max-width": 1920, "max-height": 1080, "max-size-kb": 512},
    "color-range": {"hue": {"max": 90, "min": -90, "def": 0}},
    "rec-control": {"file-ext": [{"name": "mp4", "value": 0}]}},
    {"device": {"serial": "A305200908002", "product_id": 773, "hardware_version": "A", "firmware": "1.3.540",
                "factory_firmware": "1.3.275", "product": "Ultra Encode", "model": "Ultra Encode SDI", "manufacturer": "MAGEWELL",
                "features": 1, "max_lock_count": 2, "live_support": 2047, "cloud_support": True},
     "network": {"ethernet": {"mac": "d0:c8:57:80:3a:70"}, "wifi": {"mac": "d0:c8:57:80:3a:71"},
                 "bluetooth": {"mac": "d0:c8:57:80:3a:72"}},
     "snapshot": {"input": "/tmp/sbox-snapshot/sbox-quarter.jpg"},
     "nosignal": {"max_count": 4, "max_width": 1920, "max_height": 1080, "max_size_kb": 512},
     "capabilities": {"color_range": '{"hue":{"max":90,"min":-90,"def":0}}', "recording": '{"file-ext":[{"name":"mp4","value":0}]}'}})
_uet("info-aio", "/usapi?method=get-info", {
    "result": 0, "mac-addr": {"softap": "d0:c8:57:81:02:2f"}, "main-snapshot": "/tmp/sbox-snapshot/mainstream.jpg",
    "product": {"record-support": 1, "scheduler-support": 1, "nas-support": 1, "multi-audio-support": 1},
    "rec": {"mode": [{"name": "Loop recording", "value": 1}]}, "lcd": {"page": []}},
    {"device": {"record_support": True, "scheduler_support": True, "nas_support": True, "multi_audio_support": True},
     "network": {"softap": {"mac": "d0:c8:57:81:02:2f"}}, "snapshot": {"main": "/tmp/sbox-snapshot/mainstream.jpg"},
     "capabilities": {"recording": '{"mode":[{"name":"Loop recording","value":1}]}', "lcd": '{"page":[]}'}}, model=_UE_V2)

# get-status: the Ultra Encode API example (HDMI and SDI models).
_uet("status", "/usapi?method=get-status", {
    "result": 0, "cur-status": 65616, "cur-time": "2021-01-11 10:00:00", "box-name": "Ultra Encode A304201201001",
    "sysstat": {"cpu": 12},
    "live-status": {"live": [{"result": 22, "run-ms": 5000, "cur-bps": 4000000, "avg-bps": 3900000, "net": 1, "result2": 0,
                              "cur-bps2": 0, "net2": 0, "client-id": ""}]},
    "upgrade-status": {"step": 0, "percent": 0, "result": 27, "client-id": "", "mode": "none"},
    "living-test": {"upload-bps": 0, "percent": 0, "result": 27, "net": 0, "client-id": ""},
    "check-upgrade": {"result": 0, "client-id": ""},
    "input-signal": {"status": 1, "cx": 1920, "cy": 1080, "interlaced": 0, "frame-rate": 59.94, "channel-valid": 2, "is-lpcm": 1,
                     "bits-per-sample": 16, "sample-rate": 48000},
    "wifi": {"name": "MWL1", "level": 0, "ip": "192.168.8.249", "mask": "255.255.255.0", "router": "192.168.8.1", "dns": "192.168.8.1"},
    "eth": {"ip": "10.10.107.212", "mask": "255.255.0.0", "router": "10.10.0.1", "dns": "10.0.0.3"},
    "mobile": {"ip": "", "mask": "", "router": "", "dns": ""},
    "upgrade": {"ver": "1.2.123", "date": "2012-1-1 00:00:00", "size-byte": 12004784,
                "info": [{"version": "1.2.123", "changelog": "## Develop version 1.2."}]},
    "channel-count": 2, "vumeters": [33, 32]},
    {"device": {"status_mask": 65616, "first_boot": False, "recording": False, "streaming": False, "disk_ready": True, "softap_on": True,
                "disk_testing": False, "updating": False, "stream_testing": False, "password_set": True, "occupied": False,
                "formatting_usb": False, "formatting_sd": False, "searching_wifi": False, "connecting_wifi": False,
                "loading_profile": False, "checking_firmware": False, "resetting": False, "rebooting": False,
                "upload_testing": False, "time": "2021-01-11 10:00:00", "name": "Ultra Encode A304201201001",
                "system_status": '{"cpu":12}'},
     "input": {"signal": {"status": 1, "width": 1920, "height": 1080, "interlaced": False, "frame_rate": 59.94, "audio_channels": 2,
                          "audio_lpcm": True, "audio_bits": 16, "audio_sample_rate": 48000}},
     "live": {"test": {"result": 27, "percent": 0, "upload_bps": 0, "net": 0},
              "sessions": {"0": {"result": 22, "run_ms": 5000, "bps": 4000000, "avg_bps": 3900000, "net": 1, "result2": 0, "bps2": 0,
                                 "net2": 0}}},
     "firmware": {"check": {"result": 0},
                  "available": {"version": "1.2.123", "date": "2012-1-1 00:00:00", "size_bytes": 12004784,
                                "notes": '[{"version":"1.2.123","changelog":"## Develop version 1.2."}]'},
                  "update": {"step": 0, "percent": 0, "result": 27, "mode": "none"}},
     "network": {"wifi": {"name": "MWL1", "level": 0, "ip": "192.168.8.249", "mask": "255.255.255.0", "router": "192.168.8.1",
                          "dns": "192.168.8.1"},
                 "ethernet": {"ip": "10.10.107.212", "mask": "255.255.0.0", "router": "10.10.0.1", "dns": "10.0.0.3"},
                 "mobile": {"ip": "", "mask": "", "router": "", "dns": ""}},
     "audio": {"channel_count": 2, "meters": {"0": 33, "1": 32}}},
    state_before={"live": {"sessions": {"1": {"result": 23}}}, "audio": {"meters": {"2": 5}}})

# get-status on the AIO (Ultra Encode AIO API, get-status).
_uet("status-aio", "/usapi?method=get-status", {
    "result": 0, "cur-status": 6, "input-source": 1, "input-device": 1, "cpu-temperature": 61100, "enable-ndi-hx3": 0,
    "codec": {"main-stream": {}}, "last-rec-status": 0,
    "live-status": {"live": [{"id": 0, "type": 130, "is-use": 1, "is-skd-runnung": 0, "name": "NDI HX", "run-ms": 231465213, "result": 22,
                              "main-inst-bps": 993141, "sub-inst-bps": 103444}]},
    "rec-status": {"rec": [{"id": 1, "type": 1, "is-use": 1, "is-skd-runnung": 0, "result": 2, "run-ms": 1700, "parted-num": 1,
                            "video-frame-count": 70, "audio-frame-count": 54}]},
    "format-status": {"type": 0, "percent": 20, "result": 2, "client-id": ""},
    "disk-test": {"type": 1, "read-bps": 0, "write-bps": 14833071, "percent": 3, "result": 2, "client-id": "web-session-32435088"},
    "nas": [{"id": 0, "type": 2, "result": 22}],
    "disk-info": [{"disk-status": 1, "disk-type": 1, "total-size": 61638148096, "used-size": 97058816}],
    "input-signal": {"status": 0, "hdmi": {"status": 0, "cx": 0, "cy": 0, "interlaced": 0, "interlaced-scale": 1, "frame-rate": 0.00,
                                           "channel-valid": 0, "is-lpcm": 0, "bits-per-sample": 0, "sample-rate": 0},
                     "sdi": {"status": 1, "cx": 1920, "cy": 1080, "interlaced": 1, "interlaced-scale": 1, "frame-rate": 25.0,
                             "channel-valid": 8, "is-lpcm": 1, "bits-per-sample": 24, "sample-rate": 48000}},
    "rndis": {"ip": "192.168.66.1"}},
    {"device": {"status_mask": 6, "first_boot": False, "recording": True, "streaming": True, "disk_ready": False, "softap_on": False,
                "disk_testing": False, "updating": False, "stream_testing": False, "password_set": False, "occupied": False,
                "formatting_usb": False, "formatting_sd": False, "searching_wifi": False, "connecting_wifi": False,
                "loading_profile": False, "checking_firmware": False, "resetting": False, "rebooting": False,
                "upload_testing": False, "cpu_temperature": 61.1, "last_record_status": 0},
     "input": {"source": 1, "device": 1, "signal": {"status": 0}},
     "inputs": {"hdmi": {"signal": {"status": 0, "width": 0, "height": 0, "interlaced": False, "interlaced_scale": 1, "frame_rate": 0.0,
                                    "audio_channels": 0, "audio_lpcm": False, "audio_bits": 0, "audio_sample_rate": 0}},
                "sdi": {"signal": {"status": 1, "width": 1920, "height": 1080, "interlaced": True, "interlaced_scale": 1,
                                   "frame_rate": 25.0, "audio_channels": 8, "audio_lpcm": True, "audio_bits": 24,
                                   "audio_sample_rate": 48000}}},
     "ndi": {"hx3": False}, "encoder": {"status": '{"main-stream":{}}'},
     "live": {"status": {"0": {"type": 130, "name": "NDI HX", "enabled": True, "scheduled": False, "run_ms": 231465213, "result": 22,
                               "main_bps": 993141, "sub_bps": 103444}}},
     "record": {"status": {"1": {"type": 1, "enabled": True, "scheduled": False, "result": 2, "run_ms": 1700, "parts": 1,
                                 "video_frames": 70, "audio_frames": 54}}},
     "storage": {"format": {"type": 0, "percent": 20, "result": 2},
                 "test": {"type": 1, "read_bps": 0, "write_bps": 14833071, "percent": 3, "result": 2},
                 "disks": {"1": {"status_mask": 1, "total_bytes": 61638148096, "used_bytes": 97058816}},
                 "nas_status": {"0": {"type": 2, "result": 22}}},
     "network": {"usb": {"status": '{"ip":"192.168.66.1"}'}}},
    model=_UE_V2, state_before={"live": {"status": {"4": {"result": 23}}}, "record": {"status": {"2": {"result": 27}}}})
_uet("status-failed", "/usapi?method=get-status", '{"result":-17}', {})

# get-signal-info (Ultra Encode API, and the AIO API with input-device).
_UE_SIG = {"result": 0, "signal-info-types": ["video-info", "audio-info", "hdmi-info"],
           "hdmi-info": {"mode": "hdmi", "vic": 0, "hdcp": False, "it-content": False, "3d-format": False, "timing-h-total": 1650},
           "audio-info": {"codec": "lpcm", "num-channels": 2, "sample-rate": 48000, "bit-count": 16},
           "video-info": {"codec": "uncompressed", "width": 1280, "height": 720, "scan": "progressive", "field-rate": 60.00,
                          "color-depth": 8, "color-format": "bt.709", "aspect-ratio": "16:9", "sampling": "4:4:4",
                          "quant-range": "limited", "sat-range": "limited", "frame-struct": "2d"}}
_UE_SIG_STATE = {"video": {"codec": "uncompressed", "width": 1280, "height": 720, "scan": "progressive", "field_rate": 60.0,
                           "color_depth": 8, "color_format": "bt.709", "aspect_ratio": "16:9", "sampling": "4:4:4",
                           "quant_range": "limited", "sat_range": "limited", "frame_struct": "2d"},
                 "audio": {"codec": "lpcm", "channels": 2, "sample_rate": 48000, "bits": 16},
                 "hdmi": {"mode": "hdmi", "vic": 0, "hdcp": False, "it_content": False, "format_3d": False,
                          "timing": '{"mode":"hdmi","vic":0,"hdcp":false,"it-content":false,"3d-format":false,"timing-h-total":1650}'}}
_uet("signal", "/usapi?method=get-signal-info", _UE_SIG, {"input": {"info": _UE_SIG_STATE}})
_uet("signal-hdmi", "/usapi?method=get-signal-info&input-device=2", _UE_SIG, {"inputs": {"hdmi": {"info": _UE_SIG_STATE}}}, model=_UE_V2)
_uet("signal-sdi", "/usapi?method=get-signal-info&input-device=1",
     {"result": 0, "video-info": {"width": 1920, "height": 1080, "scan": "interlaced"}, "audio-info": {"num-channels": 16}},
     {"inputs": {"sdi": {"info": {"video": {"width": 1920, "height": 1080, "scan": "interlaced"}, "audio": {"channels": 16}}}}},
     model=_UE_V2)

# get-settings: the Ultra Encode API example (HDMI and SDI models).
_uet("settings", "/usapi?method=get-settings", {
    "result": 0, "name": "Ultra Encode A304201201001", "passwd": 1, "is-check-update": 1, "audio-sync-offset": 0, "udp-mtu": 1496,
    "softap": {"is-softap": 1, "is-visible": 1, "softap-ssid": "A304201201001", "softap-passwd": "01201001"},
    "video-color": {"contrast": 100, "brightness": 0, "saturation": 100, "hue": 0},
    "volume": {"is-mic": 1, "mic-gain": 0, "is-spi": 1, "spi-gain": -6.5, "is-phone": 1, "phone-gain": 0},
    "enable-deinterlace": 1,
    "main-stream": {"is-auto": 0, "codec": 0, "cx": 1920, "cy": 1080, "duration": 0, "kbps": 20480, "gop": 1, "fourcc": 0, "profile": 2,
                    "cbrstat": 60, "fullrange": 0, "is-vbr": 0, "min-vbr-qp": 0, "max-vbr-qp": 0, "is-time-code-sei": 0,
                    "is-closed-caption-sei": 0, "ar-convert-mode": 2},
    "sub-stream": {"enable": 1, "codec": 0, "cx": 1280, "cy": 720, "duration": 333333, "kbps": 4096, "gop": 5, "fourcc": 0, "profile": 2,
                   "cbrstat": 60, "fullrange": 1, "is-vbr": 0, "min-vbr-qp": 0, "max-vbr-qp": 0, "is-time-code-sei": 0,
                   "is-closed-caption-sei": 0, "ar-convert-mode": 2},
    "audio": {"sample-rate": 48000, "channels": 2, "kbps": 128},
    "eth": {"is-dhcp": 1, "ip": "", "mask": "", "router": "", "dns": ""},
    "wifi": {"is-dhcp": 0, "ip": "192.168.8.20", "mask": "255.255.255.0", "router": "192.168.8.1", "dns": "192.168.8.1"},
    "stream-server": [{"id": 0, "type": 0, "url": "192.168.1.123:345/live", "key": "aa", "is-auth": 0, "user": "", "passwd": "",
                       "is-use": 0, "token": "", "net-mode": 1, "name": "192.168.1.123"}],
    "video-input-format": {"is-color-fmt": 0, "color-fmt": 3, "is-quant-range": 0, "quant-range": 2},
    "video-output-format": {"is-color-fmt": 1, "color-fmt": 3, "is-quant-range": 0, "quant-range": 2, "is-sat-range": 0, "sat-range": 2},
    "use-nosignal-file": 1,
    "nosignal-files": [{"id": 0, "is-use": 0, "is-edit": 0, "file-path": "/no-signal/default0.jpg", "time": 0},
                       {"id": 1, "is-use": 1, "is-edit": 0, "file-path": "/no-signal/default1.jpg", "time": 0}]},
    {"device": {"name": "Ultra Encode A304201201001", "auto_check_update": True},
     "audio": {"sync_offset_ms": 0, "encoder": {"sample_rate": 48000, "channels": 2, "kbps": 128},
               "volume": {"input_on": True, "input_gain": -6.5, "line_in_on": True, "line_in_gain": 0.0, "line_out_on": True,
                          "line_out_gain": 0.0}},
     "network": {"udp_mtu": 1496, "softap": {"enabled": True, "visible": True, "ssid": "A304201201001"},
                 "ethernet": {"config": {"dhcp": True, "ip": "", "mask": "", "router": "", "dns": ""}},
                 "wifi": {"config": {"dhcp": False, "ip": "192.168.8.20", "mask": "255.255.255.0", "router": "192.168.8.1",
                                     "dns": "192.168.8.1"}}},
     "video": {"deinterlace": True, "color": {"contrast": 100, "brightness": 0, "saturation": 100, "hue": 0},
               "input_format": {"color_auto": True, "color_format": 3, "quant_auto": True, "quant_range": 2},
               "output_format": {"color_auto": False, "color_format": 3, "quant_auto": True, "quant_range": 2, "sat_auto": True,
                                 "sat_range": 2}},
     "encoder": {"main-stream": {"follow_input": False, "codec": 0, "width": 1920, "height": 1080, "frame_interval": 0, "kbps": 20480,
                                 "gop": 1, "profile": 2, "stat_seconds": 60, "full_range": False, "vbr": False, "min_qp": 0, "max_qp": 0,
                                 "timecode_sei": 0, "caption_sei": False, "aspect": 2},
                 "sub-stream": {"enabled": True, "codec": 0, "width": 1280, "height": 720,
                                "frame_interval": 333333, "kbps": 4096, "gop": 5, "profile": 2, "stat_seconds": 60, "full_range": True,
                                "vbr": False, "min_qp": 0, "max_qp": 0, "timecode_sei": 0, "caption_sei": False, "aspect": 2}},
     "live": {"servers": {"0": {"type": 0, "service": "rtmp", "name": "192.168.1.123", "enabled": False, "url": "192.168.1.123:345/live",
                                "auth": False, "user": "", "net_mode": 1}}},
     "nosignal": {"enabled": True,
                  "images": {"0": {"selected": False, "custom": False, "path": "/no-signal/default0.jpg", "time": 0},
                             "1": {"selected": True, "custom": False, "path": "/no-signal/default1.jpg", "time": 0}}}},
    state_before={"live": {"servers": {"5": {"name": "Gone"}}}, "nosignal": {"images": {"3": {"path": "/no-signal/x.jpg"}}}})

# get-settings on the AIO (Ultra Encode AIO API, get-settings), shortened.
_uet("settings-aio", "/usapi?method=get-settings", {
    "result": 0, "living-kbps": 32768, "enable-advanced-pcr": 0, "enable-ndi-hx3": 0,
    "date-time": {"timezone": "Asia/Shanghai", "is-auto": 1, "ntp-server": "0.pool.ntp.org", "ntp-server-backup": "1.pool.ntp.org"},
    "input-source": {"source": 1, "mixer": {"input-device": 2, "is-hdmi-top": 0, "type": 0, "location": 2}},
    "video-color": {"hdmi": {"contrast": 100, "brightness": 0, "saturation": 100, "hue": 0},
                    "sdi": {"contrast": 120, "brightness": 5, "saturation": 90, "hue": -3}},
    "volume": {"is-spi": 1, "spi-gain": 0, "is-linein": 1, "linein-gain": 127, "is-lineout": 0, "lineout-gain": -600, "enable-mic-bias": 0},
    "3d-output": {"enable": 0, "mode": 1},
    "main-stream": {"is-auto": 0, "codec": 1, "cx": 1920, "cy": 1080, "duration": 0, "kbps": 8192, "gop": 60, "profile": 0, "cbrstat": 60,
                    "fullrange": 0, "is-vbr": 0, "min-vbr-qp": 0, "max-vbr-qp": 0, "is-time-code-sei": 1, "is-closed-caption-sei": 0,
                    "ar-convert-mode": 2, "rotation": 0, "mirroring": 0},
    "audio-streams": [{"sample-rate": 48000, "channels": 0, "kbps": 256, "ch0": 0, "ch1": 1},
                      {"sample-rate": 48000, "channels": 2, "kbps": 32, "ch0": 2, "ch1": 1}],
    "rndis": {"ip": "192.168.66.1", "mask": "255.255.255.0"},
    "stream-server": [{"id": 0, "type": 130, "name": "NDI HX", "is-use": 0, "source-name": "#serial-no#", "group-name": "public",
                       "enable-discovery": 0, "discovery-server": "", "transport-mode": 3, "mcast-addr": "", "mcast-mask": "",
                       "mcast-ttl": 0, "enable-fail-over": 0, "fail-over-ndi-name": "", "fail-over-ip-addr": "", "enable-web-control": 0,
                       "enable-ptz-control": 0, "main-stream": 0, "prvw-stream": 1, "audio": 2, "opt": 0, "is-media-hub": 0}],
    "video-input-format": {"hdmi": {"is-color-fmt": 0, "color-fmt": 1, "is-quant-range": 0, "quant-range": 1},
                           "sdi": {"is-color-fmt": 1, "color-fmt": 3, "is-quant-range": 1, "quant-range": 2}},
    "video-output-format": {"sdi": {"is-color-fmt": 0, "color-fmt": 3, "is-quant-range": 0, "quant-range": 2, "is-sat-range": 1,
                                    "sat-range": 3}},
    "nas": [{"id": 0, "type": 2, "proto": 0, "is-use": 1, "addr": "10.10.14.193", "mount": "/home/magewell/nfs"}],
    "send-file-cloud": [{"id": 0, "type": 0, "is-use": 1, "url": "ftp.example.com", "port": 21, "dir-name": "rec", "user-name": "u",
                         "net-mode": 1}],
    "web": {"is-http": 1, "http-port": 80, "is-https": 0, "https-port": 443, "is-cert-valid": 0, "is-cert-key-valid": 0, "theme": 0},
    "rec": {"is-auto": 0, "trigger-mode": 0},
    "living": {"ts": {"mtu": 1497}, "hls-push": {"seg-count": 3, "seg-duration": 3}},
    "lcd-control": {"no-touch": 0, "page-idx": 3, "no-flip": 0}},
    {"live": {"total_kbps": 32768, "advanced_pcr": False, "ts_mtu": 1497, "hls_push": {"segments": 3, "duration": 3},
              "servers": {"0": {"type": 130, "service": "ndi-hx", "name": "NDI HX", "enabled": False, "source_name": "#serial-no#",
                                "groups": "public", "discovery": False, "discovery_server": "", "transport": 3, "mcast_address": "",
                                "mcast_mask": "", "mcast_ttl": 0, "failover": False, "failover_name": "", "failover_address": "",
                                "web_control": False, "ptz_control": False, "audio": 2, "media_hub": False}}},
     "ndi": {"hx3": False},
     "time": {"zone": "Asia/Shanghai", "auto": True, "ntp_server": "0.pool.ntp.org", "ntp_backup": "1.pool.ntp.org"},
     "input": {"source": 1, "mixer": {"format": 2, "hdmi_on_top": False, "type": 0, "location": 2}},
     "inputs": {"hdmi": {"color": {"contrast": 100, "brightness": 0, "saturation": 100, "hue": 0},
                         "input_format": {"color_auto": True, "color_format": 1, "quant_auto": True, "quant_range": 1}},
                "sdi": {"color": {"contrast": 120, "brightness": 5, "saturation": 90, "hue": -3},
                        "input_format": {"color_auto": False, "color_format": 3, "quant_auto": False, "quant_range": 2},
                        "output_format": {"color_auto": True, "color_format": 3, "quant_auto": True, "quant_range": 2,
                                          "sat_auto": False, "sat_range": 3}}},
     "audio": {"volume": {"input_on": True, "input_gain": 0.0, "line_in_on": True, "line_in_gain": 1.27, "line_out_on": False,
                          "line_out_gain": -6.0, "mic_bias": False},
               "streams": {"1": {"sample_rate": 48000, "channels": 0, "kbps": 256,
                                 "map": '{"sample-rate":48000,"channels":0,"kbps":256,"ch0":0,"ch1":1}'},
                           "2": {"sample_rate": 48000, "channels": 2, "kbps": 32,
                                 "map": '{"sample-rate":48000,"channels":2,"kbps":32,"ch0":2,"ch1":1}'}}},
     "video": {"output_3d": False, "output_3d_mode": 1},
     "encoder": {"main-stream": {"follow_input": False, "codec": 1, "width": 1920, "height": 1080, "frame_interval": 0, "kbps": 8192,
                                 "gop": 60, "profile": 0, "stat_seconds": 60, "full_range": False, "vbr": False, "min_qp": 0, "max_qp": 0,
                                 "timecode_sei": 1, "caption_sei": False, "aspect": 2, "rotation": 0, "mirroring": False}},
     "network": {"usb": {"ip": "192.168.66.1", "mask": "255.255.255.0"}},
     "storage": {"nas": {"0": {"type": 2, "protocol": 0, "enabled": True, "address": "10.10.14.193", "mount": "/home/magewell/nfs"}}},
     "upload": {"servers": {"0": {"type": 0, "enabled": True, "url": "ftp.example.com", "port": 21, "folder": "rec", "user": "u",
                                  "net_mode": 1}}},
     "web": {"http": True, "http_port": 80, "https": False, "https_port": 443, "cert_valid": False, "key_valid": False, "theme": 0},
     "record": {"auto": False, "trigger": 0},
     "lcd": {"touch": False, "page": 3, "flip": False}},
    model=_UE_V2, state_before={"audio": {"streams": {"3": {"kbps": 128}}}, "storage": {"nas": {"2": {"address": "x"}}},
                                "upload": {"servers": {"4": {"url": "x"}}}})

# EDID (Ultra Encode API, EDID).
_UE_EDID = "AP///////wA09wEAAQAAAAEaAQOAAAB4"
_uet("edid", "/usapi?method=get-edid-config",
     {"result": 0, "smart-edid": 1, "keep-last": 0, "add-audio": 1, "limit-pixel-clock": 1, "data": _UE_EDID},
     {"edid": {"smart": True, "keep_last": False, "add_audio": True, "limit_pixel_clock": True, "data": _UE_EDID}})
_uet("edid-default", "/usapi?method=set-default-edid", {"result": 0, "data": _UE_EDID}, {"edid": {"data": _UE_EDID}},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=get-edid-config"}])
_uet("edid-loop-through", "/usapi?method=get-loop-through-edid", {"result": 0, "data": _UE_EDID}, {"edid": {"loop_through": _UE_EDID}})

# Users, Control Hub, NDI sources, Wi-Fi.
_uet("users", "/usapi?method=get-users", {"result": 0, "users": [{"id": "Admin", "type": 1}, {"id": "ops.team", "type": 2}]},
     {"users": {"Admin": {"admin": True}, "ops.team": {"admin": False}}}, state_before={"users": {"Gone": {"admin": False}}})
_UE_CLOUD_BODY = {"device_id": "A305200908004", "number": 2, "version": 1, "result": 0, "status": [
    {"cloud-code": "", "cloud-date": 0, "cloud-enable-https": 0, "cloud-http-port": 80, "cloud-https-port": 443,
     "cloud-ip-addr": "10.0.1.32", "cloud-reg-status": 101, "cloud-status": 35, "id": 0, "is-cloud-set": 1},
    {"cloud-code": "", "cloud-date": 0, "cloud-enable-https": 0, "cloud-http-port": 80, "cloud-https-port": 443,
     "cloud-ip-addr": "10.10.8.233", "cloud-reg-status": 103, "cloud-status": 35, "id": 1, "is-cloud-set": 1}]}
_UE_CLOUD_STATE = {"cloud": {"device_id": "A305200908004", "slot_count": 2, "slots": {
    "0": {"address": "10.0.1.32", "http_port": 80, "https": False, "https_port": 443, "registration": 101, "status": 35, "date": 0,
          "configured": True},
    "1": {"address": "10.10.8.233", "http_port": 80, "https": False, "https_port": 443, "registration": 103, "status": 35, "date": 0,
          "configured": True}}}}
_uet("cloud", "/usapi?method=cloud-status&version=1", _UE_CLOUD_BODY, _UE_CLOUD_STATE)
_uet("cloud-aio", "/cloud-api?method=cloud-status&version=1", _UE_CLOUD_BODY, _UE_CLOUD_STATE, model=_UE_V2)
_uet("ndi-sources", "/usapi?method=get-ndi-sources",
     {"result": 0, "ndi-sources": [{"ndi-name": "PRO CONVERT (#14 (B403190104002))", "ip-addr": "10.10.13.247:5962"}]},
     {"ndi": {"sources": {"0": {"name": "PRO CONVERT (#14 (B403190104002))", "address": "10.10.13.247:5962"}}}},
     state_before={"ndi": {"sources": {"1": {"name": "Gone"}}}})
_uet("wifi-scan", "/usapi?method=scan-wifi-results",
     {"result": 0, "user-items": [{"name": "MWL1", "freq": 5785, "level": -35, "is-auto": 1, "is-use": 1}],
      "scan-items": [{"name": "Guest", "freq": 2412, "level": -70, "is-auto": 0, "is-use": 0}]},
     {"wifi": {"saved": {"0": {"name": "MWL1", "frequency": 5785, "level": -35, "auto": True, "in_use": True}},
               "found": {"0": {"name": "Guest", "frequency": 2412, "level": -70}}}})

# The AIO and Plus lists (field names of the reply as the add methods name
# them; the documents show the replies' arrays empty).
_uet("rec-channels", "/usapi?method=get-rec-channels", {"result": 0, "rec-channels": [
    {"id": 1, "type": 1, "is-use": 1, "stream-index": 0, "mode": 1, "dir-name": "REC_Folder", "file-prefix": 0, "prefix-name": "VID",
     "file-suffix": 0, "time-unit": 60, "audio": 0}]},
     {"record": {"channels": {"1": {"storage": 1, "enabled": True, "stream_index": 0, "mode": 1, "folder": "REC_Folder", "prefix_mode": 0,
                                    "prefix": "VID", "format": 0, "segment_minutes": 60, "audio": 0}}}}, model=_UE_V2)
_uet("overlays", "/usapi?method=get-surfaces", {"result": 0, "main-surface": 1, "second-surface": 0, "surfaces": [
    {"id": 3, "type": 0, "is-use": 1, "stream-index": 0, "position": 6, "x-offset": 0, "y-offset": 0, "margin": 20, "text": "Live",
     "font-size": 48, "font-color": "#ffffff"}]},
     {"overlays": {"main_enabled": True, "sub_enabled": False,
                   "items": {"3": {"type": 0, "enabled": True, "stream_index": 0, "position": 6, "x": 0, "y": 0, "margin": 20,
                                   "text": "Live", "font_size": 48, "color": "#ffffff"}}}},
     model=_UE_V2, state_before={"overlays": {"items": {"9": {"text": "Gone"}}}})
_uet("overlay-images", "/usapi?method=get-images", {"result": 0, "images": [{"id": 1}]}, {"overlays": {"images": '[{"id":1}]'}},
     model=_UE_V2)
_uet("schedules", "/usapi?method=get-schedulers&id=-1&chn-type=2&chn-id=-1", {"result": 0, "schedulers": []},
     {"schedules": {"2": {"list": "[]"}}}, model=_UE_V2)
_uet("tokens", "/usapi?method=get-tokens",
     {"tokens": [{"token": "Q7kMpTN7YYJ7PHtQBGAS3HZTbNCiA35SesersYKJceZRxANZ55CK47Hctn5JnnGN", "expire": -1, "user": "Admin"}], "result": 0},
     {"tokens": {"0": {"user": "Admin", "expire": -1}}}, model=_UE_V2)
_uet("upload-status", "/usapi?method=send-file-get-status&disk-type=1&start=0&count=0",
     {"result": 0, "total-count-ongoing": 1, "total-count-done": 2, "start": 0, "count": 0, "status": {}, "media-files": {}},
     {"upload": {"waiting": 1, "done": 2, "status": "{}"}}, model=_UE_V2)

# Re-reads once the device accepts a change.
_UE_OK = '{"result":0}'
_UE_REREADS = [
    ("settings", "/usapi?method=set-name&name=Stage", "/usapi?method=get-settings"),
    ("status", "/usapi?method=start-live", "/usapi?method=get-status"),
    ("edid", "/usapi?method=set-edid-config&smart-edid=1&keep-last=0&add-audio=1&limit-pixel-clock=1", "/usapi?method=get-edid-config"),
    ("users", "/usapi?method=del-user&id=Test", "/usapi?method=get-users"),
    ("rec-channels", "/usapi?method=del-rec-channel&id=1", "/usapi?method=get-rec-channels"),
    ("overlays", "/usapi?method=del-surface&id=5", "/usapi?method=get-surfaces"),
    ("images", "/usapi?method=del-image&id=1", "/usapi?method=get-images"),
    ("tokens", "/usapi?method=del-token&token=x", "/usapi?method=get-tokens"),
    ("upload", "/usapi?method=send-file-start", "/usapi?method=send-file-get-status&disk-type=1&start=0&count=0"),
    ("wifi", "/usapi?method=forget-wifi&name=magewell", "/usapi?method=scan-wifi-results"),
]
for _n, _p, _t in _UE_REREADS:
    _uet(f"reread-{_n}", _p, _UE_OK, {}, model=_UE_V2, expect_then_send=[{"method": "GET", "target": _t}])
_uet("reread-schedules", "/usapi?method=del-scheduler&id=7&chn-type=2&chn-id=-1", _UE_OK, {}, model=_UE_V2,
     expect_then_send=[{"method": "GET", "target": f"/usapi?method=get-schedulers&id=-1&chn-type={t}&chn-id=-1"} for t in (0, 1, 2)])
_uet("reread-cloud", "/cloud-api?method=cloud-unreg-ex&id=0", _UE_OK, {},
     expect_then_send=[{"method": "GET", "target": "/usapi?method=cloud-status&version=1"}])
_uet("reread-cloud-aio", "/cloud-api?method=cloud-unreg-ex&id=0", _UE_OK, {}, model=_UE_V2,
     expect_then_send=[{"method": "GET", "port": 8070, "target": "/cloud-api?method=cloud-status&version=1"}])
_uet("reread-refused", "/usapi?method=set-name&name=Stage", '{"result":-28}', {})
