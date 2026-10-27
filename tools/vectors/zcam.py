# Z CAM (zcam): one vector per command, written by hand from Z CAM's HTTP
# document (E2/protocol/http/http.md) and the request forms of the reference
# code it points to (api.js). Settings are /ctrl/set?<key>=<value>, values
# percent-encoded as api.js's encodeURIComponent does; a command that reads
# its setting back states the set request, which goes first.
ZC = "zcam"
OKZ = '{"code":0,"desc":"","msg":""}'


def _zc(command, input, target, **extra):
    http(ZC, command, input, "GET", target, **extra)


# Every setting key's set command: /ctrl/set?<key>=<value>.
for _key in ["focus", "af_mode", "af_area", "af_speed", "caf", "caf_sens", "live_caf", "af_adjust_with_ptz",
             "mf_mag", "mf_recording", "mf_drive", "lens_focus_pos", "lens_zoom", "lens_zoom_pos", "zoom_mode",
             "ev", "flicker", "meter_mode", "iris", "iso", "min_iso", "max_iso", "iso_ctrl", "shutter_angle_ctrl",
             "sht_operation", "eND", "ae_speed", "lock_ae_in_rec", "bl_comp", "wb", "mwb", "tint", "mwb_r", "mwb_g",
             "mwb_b", "mwb_r_offset", "mwb_g_offset", "mwb_b_offset", "wb_priority", "lock_awb_in_rec", "lut",
             "noise_reduction", "luma_level", "brightness", "contrast", "sharpness", "saturation", "hue", "movfmt",
             "movvfr", "video_system", "resolution", "project_fps", "vfr_ctrl", "record_file_format", "rotation",
             "split_duration", "rec_fps", "raw_over_hdmi", "rec_frame_indicator", "preroll", "preroll_duration",
             "record_meta", "camera_id", "reelname", "video_encoder", "bitrate_level", "compose_mode", "photo_q",
             "eis_on_off", "vid_rot", "record_mode", "video_tl_interval", "low_jello", "primary_audio",
             "audio_channel", "audio_phantom_power", "audio_level_display", "ain_gain_type", "audio_input_level",
             "audio_input_gain", "audio_in_l_gain", "audio_in_r_gain", "audio_output_gain", "audio_noise_reduction",
             "send_stream", "stream_resolution", "stream_fps", "stream_video_encoder", "stream_param_save",
             "assitool_display", "assitool_scope", "assitool_peak_onoff", "assitool_peak_color",
             "assitool_frame_line", "assitool_frame_line_color", "assitool_frame_line_opacity",
             "assitool_center_mark", "assitool_center_mark_color", "assitool_exposure", "assitool_zera_th1",
             "assitool_zera_th2", "assitool_grid_line", "assitool_safe_area", "hdmi_focus_area", "tc_source",
             "tc_count_up", "tc_hdmi_dispaly", "tc_drop_frame", "union_ae", "union_awb", "ezlink_mode",
             "ezlink_trigger", "genlock", "genlock_coarse", "genlock_fine", "ptz_limit", "pt_speedmode",
             "pt_speed_with_zoom_pos", "ptz_flip", "pt_priv_mode", "pt_pwr_pos", "ptz_common_speed",
             "ptz_common_time", "ptz_preset_mode", "ptz_speed_mode", "freeze_during_preset",
             "ptz_common_speed_unit", "tally_on", "color_bar_enable", "led", "desqueeze", "auto_off", "auto_standby",
             "visca_id", "visca_baudrate", "visca_enable", "https_on", "https_auth", "https_cert_source",
             "usb_device_role", "ir", "ir_id", "sdi", "3g_sdi_mode", "hdmi_format", "hdmi_osd", "osd_layout", "wifi",
             "wifi_channel"]:
    _zc("set_" + _key.lower(), {"value": "On"}, f"/ctrl/set?{_key}=On")
# The document's own examples.
_zc("set_movfmt", {"value": "4KP30"}, "/ctrl/set?movfmt=4KP30", file="set_movfmt-example")
_zc("set_lut", {"value": "Customized 2"}, "/ctrl/set?lut=Customized%202", file="set_lut-customized-2")
_zc("set_mwb_r", {"value": "3000"}, "/ctrl/set?mwb_r=3000", file="set_mwb_r-example")
_zc("set_iso", {"value": "800"}, "/ctrl/set?iso=800", file="set_iso-refused",
    http_reply={"status": 200, "body": '{"code":1,"desc":"","msg":""}'},
    expect_result={"error": {"error": "device_error"}})
_zc("get_setting", {"key": "contrast"}, "/ctrl/get?k=contrast")
_zc("set_setting", {"key": "send_stream", "value": "Stream0"}, "/ctrl/set?send_stream=Stream0")
_zc("get_settings_catalog", {"catalog": "exposure"}, "/ctrl/getbatch?catalog=exposure")
_zc("clear_settings", {}, "/ctrl/set?action=clear")
_zc("get_info", {}, "/info")
_zc("get_status", {}, "/camera_status")
_zc("get_commit_info", {}, "/commit_info")
_zc("get_nick_name", {}, "/ctrl/nick_name")
_zc("set_nick_name", {"name": "Cam A"}, "/ctrl/nick_name?action=set&name=Cam%20A")
_zc("check_session", {}, "/ctrl/session")
_zc("occupy_session", {}, "/ctrl/session?action=occupy")
_zc("quit_session", {}, "/ctrl/session?action=quit")
_zc("set_date_time", {"date": "2026-10-27", "time": "14:05:00"}, "/datetime?date=2026-10-27&time=14:05:00")
_zc("get_date_time", {}, "/datetime/get")
_zc("set_timezone", {"timezone": "UTC+8"}, "/datetime?timezone=UTC%2B8")
_zc("start_ntp", {"server": "192.168.1.5", "interval": 60}, "/ctrl/sntp?action=start&ip_addr=192.168.1.5&port=123&interval=60")
_zc("stop_ntp", {}, "/ctrl/sntp?action=stop")
_zc("get_ntp", {}, "/ctrl/sntp?action=get")
_zc("set_ntp_interval", {"interval": 30}, "/ctrl/sntp?action=set_interval&interval=30")
_zc("get_temperature", {}, "/ctrl/temperature")
_zc("shutdown", {}, "/ctrl/shutdown")
_zc("reboot", {}, "/ctrl/reboot")
_zc("get_mode", {}, "/ctrl/mode?action=query", http_reply={"status": 200, "body": '{"code":0,"desc":"","msg":"rec"}'},
    expect_result={"ok": {"kind": "value", "value": "rec"}})
_zc("to_record_mode", {}, "/ctrl/mode?action=to_rec")
_zc("to_playback_mode", {}, "/ctrl/mode?action=to_pb")
_zc("to_standby", {}, "/ctrl/mode?action=to_standby")
_zc("exit_standby", {}, "/ctrl/mode?action=exit_standby")
_zc("start_recording", {}, "/ctrl/rec?action=start")
_zc("stop_recording", {}, "/ctrl/rec?action=stop")
_zc("get_recording", {}, "/ctrl/rec?action=query")
_zc("get_record_remaining", {}, "/ctrl/rec?action=remain")
_zc("get_repair_status", {}, "/ctrl/rec?action=query_repairing")
_zc("capture_still", {}, "/ctrl/still?action=cap", http_reply={"status": 200, "body": OKZ},
    expect_result={"ok": {"kind": "ack"}})
_zc("get_playback", {}, "/ctrl/pb?action=query")
_zc("get_timecode", {}, "/ctrl/tc?action=query")
_zc("reset_timecode", {}, "/ctrl/tc?action=reset")
_zc("timecode_to_time_of_day", {}, "/ctrl/tc?action=current_time")
_zc("set_timecode", {"timecode": "01:00:00:00"}, "/ctrl/tc?action=set&tc=01%3A00%3A00%3A00")
_zc("zoom_in", {"speed": 8}, "/ctrl/lens?action=zoomin&fspeed=8")
_zc("zoom_out", {}, "/ctrl/lens?action=zoomout&fspeed=50")
_zc("zoom_stop", {}, "/ctrl/lens?action=zoomstop")
_zc("get_zoom_status", {}, "/ctrl/lens?action=z_status")
_zc("focus_near", {"speed": 3}, "/ctrl/lens?action=focusnear&fspeed=3")
_zc("focus_far", {"speed": 3}, "/ctrl/lens?action=focusfar&fspeed=3")
_zc("focus_stop", {}, "/ctrl/lens?action=focusstop")
_zc("get_focus_status", {}, "/ctrl/lens?action=f_status")
_zc("autofocus", {}, "/ctrl/af")
_zc("set_af_roi", {"x": 0, "y": 0, "width": 100, "height": 100}, "/ctrl/af?action=update_roi&x=0&y=0&w=100&h=100")
_zc("set_af_roi_center", {"x": 500, "y": 500}, "/ctrl/af?action=update_roi_center&x=500&y=500")
_zc("get_af_roi", {}, "/ctrl/af?action=query")
_zc("trace_face", {"face": 2}, "/ctrl/af_face/trace_target?id=2")
_zc("enable_magnify", {}, "/ctrl/mag?action=enable")
_zc("disable_magnify", {}, "/ctrl/mag?action=disable")
_zc("get_magnify", {}, "/ctrl/mag?action=query")
_zc("one_push_white_balance", {}, "/ctrl/wb?action=one_push")
_zc("pan_tilt_move", {"direction": "leftup", "speed": 63}, "/ctrl/pt?action=leftup&fspeed=63")
_zc("pan_tilt_drive", {"pan_speed": -20, "tilt_speed": 10}, "/ctrl/pt?action=pt&pan_speed=-20&tilt_speed=10")
_zc("pan_tilt_stop", {}, "/ctrl/pt?action=stop")
_zc("pan_tilt_stop_all", {}, "/ctrl/pt?action=stop_all")
_zc("pan_tilt_home", {}, "/ctrl/pt?action=home")
_zc("pan_tilt_reset", {}, "/ctrl/pt?action=reset")
_zc("set_pan_tilt_limit", {"direction": "left", "pan": -1200, "tilt": 0},
    "/ctrl/pt?action=limit&direct=left&pan_pos=-1200&tilt_pos=0")
_zc("get_pan_tilt", {}, "/ctrl/pt?action=query")
_zc("get_pan_tilt_detail", {}, "/ctrl/pt?action=query&detail=y")
_zc("recall_preset", {"preset": 3}, "/ctrl/preset?action=recall&index=3")
_zc("save_preset", {"preset": 3}, "/ctrl/preset?action=set&index=3")
_zc("delete_preset", {"preset": 3}, "/ctrl/preset?action=del&index=3")
_zc("get_preset", {"preset": 3}, "/ctrl/preset?action=get_info&index=3")
_zc("rename_preset", {"preset": 3, "name": "Pulpit"}, "/ctrl/preset?action=set_name&index=3&new_name=Pulpit")
_zc("set_preset_speed_unit", {"preset": 3, "unit": "time"}, "/ctrl/preset?action=preset_speed&index=3&preset_speed_unit=time")
_zc("set_preset_time", {"preset": 3, "time": 5}, "/ctrl/preset?action=preset_speed&index=3&preset_time=5")
_zc("set_preset_speed", {"preset": 3, "speed": 7}, "/ctrl/preset?action=preset_speed&index=3&preset_speed=7")
_zc("delete_trace", {"trace": 1}, "/ctrl/ptrace?action=del&index=1")
_zc("rename_trace", {"trace": 1, "name": "Sweep"}, "/ctrl/ptrace?action=set_name&index=1&new_name=Sweep")
_zc("start_trace_recording", {"trace": 1}, "/ctrl/ptrace?action=rec_start&index=1")
_zc("stop_trace_recording", {}, "/ctrl/ptrace?action=rec_stop")
_zc("prepare_trace", {"trace": 1}, "/ctrl/ptrace?action=play_prepare&index=1")
_zc("play_trace", {}, "/ctrl/ptrace?action=play_start")
_zc("stop_trace", {}, "/ctrl/ptrace?action=play_stop")
_zc("get_traces", {}, "/ctrl/ptrace?action=query")
_zc("get_trace", {"trace": 1}, "/ctrl/ptrace?action=get_info&index=1")
F = "/ctrl/framing/"
_zc("set_framing_detection", {"enabled": True}, F + "detect?enable=1")
_zc("set_framing_tracking", {"enabled": False}, F + "trace?enable=0")
_zc("set_framing_target", {"target": 4}, F + "trace_target?id=4")
_zc("set_framing_start_position", {"preset": 2}, F + "start_position?index=2")
_zc("set_framing_fov_type", {"type": "half"}, F + "fov?type=half")
_zc("set_framing_body_size", {"size": "50"}, F + "fov?body_size=50")
_zc("set_framing_face_height", {"height": "30"}, F + "fov?face_height=30")
_zc("set_framing_placement", {"placement": "center"}, F + "fov?placement=center")
_zc("get_framing_fov_debug", {}, F + "debug_fov")
_zc("set_framing_auto_start", {"enabled": True}, F + "auto_start?enable=1")
_zc("restart_framing", {}, F + "restart")
_zc("set_framing_auto_target", {"enabled": True}, F + "auto_target?enable=1")
_zc("set_framing_target_lost_action", {"action": "home"}, F + "target_lost?action=home")
_zc("set_framing_target_lost_timeout", {"seconds": 10}, F + "target_lost?timeout_sec=10")
_zc("get_framing_target_lost", {}, F + "target_lost?action=query")
_zc("set_framing_detect_restart", {"seconds": 5}, F + "detect_restart?time=5")
_zc("get_framing_detect_restart", {}, F + "detect_restart?action=query")
_zc("set_framing_manual_wait", {"seconds": 3}, F + "manual_target?wait_time=3")
_zc("get_framing_manual_wait", {}, F + "manual_target?action=query")
_zc("get_framing_status", {}, F + "status")
_zc("get_framing_all_status", {}, F + "all_status")
_zc("get_framing_info", {}, F + "info")
_zc("set_framing_tally", {"enabled": True}, F + "tally?enable=1")
_zc("set_framing_mask_enabled", {"enabled": False}, F + "mask_area?enable=0")
_zc("set_framing_mask_area", {"left": 100, "top": 50, "right": 900, "bottom": 950},
    F + "mask_area?left=100&top=50&right=900&bottom=950")
_zc("get_framing_mask_area", {}, F + "mask_area")
_zc("set_framing_track_active_face", {"enabled": True}, F + "track_active_face?enable=1")
_zc("get_framing_track_active_face", {}, F + "track_active_face")
_zc("set_framing_target_count", {"count": 2}, F + "target_num?num=2")
_zc("get_framing_target_count", {}, F + "target_num")
_zc("set_framing_tilt_lock", {"enabled": True}, F + "tilt_lock?enable=1")
_zc("get_framing_tilt_lock", {}, F + "tilt_lock")
_zc("ezframing_center", {"x": 500, "y": 400, "speed": 20}, "/ctrl/ezframing/center?x=500&y=400&speed=20")
_zc("ezframing_region", {"x": 500, "y": 400, "speed": 20, "ratio": 300},
    "/ctrl/ezframing/roi?x=500&y=400&speed=20&ratio=300")
_zc("get_face_database", {}, "/ctrl/facedb/query")
_zc("prepare_face", {"face": 7}, "/ctrl/facedb/prepare?face_id=7")
_zc("add_face", {"face": 7, "name": "Host", "priority": 1, "active": True},
    "/ctrl/facedb/add?which=7&name=Host&priority=1&activate=1")
_zc("update_face", {"face": 7, "name": "Guest", "priority": 2, "active": False},
    "/ctrl/facedb/update?uid=7&name=Guest&priority=2&activate=0")
_zc("delete_face", {"face": 7}, "/ctrl/facedb/delete?uid=7")
_zc("set_face_database_enabled", {"enabled": True}, "/ctrl/facedb/set?enable=1")
_zc("get_face_database_status", {}, "/ctrl/facedb/status")
_zc("get_color_matrix", {}, "/ctrl/matrix")
_zc("set_color_matrix_enabled", {"enabled": True}, "/ctrl/matrix?enable=1")
_zc("set_color_matrix_value", {"index": 4, "value": 256}, "/ctrl/matrix?index=4&value=256")
_zc("get_color_correction", {}, "/ctrl/cc")
_zc("set_color_correction_enabled", {"enabled": False}, "/ctrl/cc?enable=0")
_zc("set_color_correction_hue", {"index": 2, "value": -5}, "/ctrl/cc?action=set_hue&index=2&value=-5")
_zc("set_color_correction_saturation", {"index": 2, "value": 10}, "/ctrl/cc?action=set_sat&index=2&value=10")
_zc("get_black_level", {}, "/ctrl/black_level")
_zc("set_black_level_enabled", {"enabled": True}, "/ctrl/black_level?enable=1")
_zc("set_black_level", {"master": 0, "red": 0, "green": 0, "blue": 0}, "/ctrl/black_level?master=0&r=0&g=0&b=0")
_zc("get_gamma", {}, "/ctrl/gamma?action=get&option=gamma")
_zc("set_gamma_rec709", {}, "/ctrl/gamma?action=set&option=gamma&base=0")
_zc("set_gamma_srgb", {}, "/ctrl/gamma?action=set&option=gamma&base=1")
_zc("set_gamma_power", {"power": 22}, "/ctrl/gamma?action=set&option=gamma&base=2&power=22")
_zc("get_knee_black_level", {}, "/ctrl/gamma?action=get&option=black_level")
_zc("set_knee_black_level", {"enabled": True, "level": 10}, "/ctrl/gamma?action=set&option=black_level&enable=1&level=10")
_zc("get_black_gamma", {}, "/ctrl/gamma?action=get&option=black_gamma")
_zc("set_black_gamma", {"enabled": True, "range": 0, "level": 0},
    "/ctrl/gamma?action=set&option=black_gamma&enable=1&range=0&level=0")
_zc("get_knee", {}, "/ctrl/gamma?action=get&option=knee")
_zc("set_knee", {"enabled": True, "point": 0, "slope": 10}, "/ctrl/gamma?action=set&option=knee&enable=1&point=0&slope=10")
_zc("get_manual_black_level", {}, "/ctrl/manual_blc?action=get")
_zc("set_manual_black_level", {"r": 256, "gr": 256, "gb": 256, "b": 256},
    "/ctrl/manual_blc?action=set&enable=1&rggb=256,256,256,256")
_zc("disable_manual_black_level", {}, "/ctrl/manual_blc?action=set&enable=0&rggb=0,0,0,0")
SS = "/ctrl/stream_setting?"
_zc("get_stream_settings", {"stream": "stream1"}, SS + "action=query&index=stream1")
_zc("set_stream_size", {"stream": "stream1", "width": 1920, "height": 1080}, SS + "index=stream1&width=1920&height=1080")
_zc("set_stream_bitrate", {"bps": 2000000}, SS + "index=stream1&bitrate=2000000")
_zc("set_stream_frame_rate", {"fps": 60}, SS + "index=stream1&fps=60")
_zc("set_stream_encoder", {"encoder": "h265"}, SS + "index=stream1&venc=h265")
_zc("set_stream_bit_depth", {"bits": "10"}, SS + "index=stream1&bitwidth=10")
_zc("set_stream_split", {"stream": "stream0", "seconds": 5}, SS + "index=stream0&split=5")
_zc("get_stream_performance", {"stream": "stream0"}, SS + "action=stream_performance&index=stream0")
_zc("get_rtmp", {}, "/ctrl/rtmp?action=query&index=0")
_zc("start_rtmp", {}, "/ctrl/rtmp?action=start")
_zc("start_rtmp_to", {"url": "rtmp://a.example/live", "key": "ab-cd"},
    "/ctrl/rtmp?action=start&url=rtmp%3A%2F%2Fa.example%2Flive&key=ab-cd")
_zc("stop_rtmp", {"index": 0}, "/ctrl/rtmp?action=stop&index=0")
_zc("set_rtmp_auto_restart", {"enabled": True}, "/ctrl/rtmp?action=set&autoRestart=1")
_zc("set_rtmp_destination", {"url": "rtmp://a.example/live", "key": "ab-cd"},
    "/ctrl/rtmp?action=set&url=rtmp%3A%2F%2Fa.example%2Flive&key=ab-cd")
_zc("get_srt", {}, "/ctrl/srt?action=query")
_zc("start_srt", {"url": "srt://10.0.0.2:9000"}, "/ctrl/srt?action=start&url=srt%3A%2F%2F10.0.0.2%3A9000")
_zc("stop_srt", {}, "/ctrl/srt?action=stop")
_zc("set_srt", {"mode": "caller", "passphrase": "", "key_length": "16", "latency": 120, "ttl": 64},
    "/ctrl/srt?action=set&mode=caller&passphrase=&pbkeyLen=16&latency=120&ttl=64")
_zc("set_srt_auto_restart", {"enabled": False}, "/ctrl/srt?action=set&autoRestart=0")
_zc("set_srt_url", {"url": "srt://10.0.0.2:9000"}, "/ctrl/srt?action=set&url=srt%3A%2F%2F10.0.0.2%3A9000")
_zc("get_ndi", {}, "/ctrl/ndi?action=query")
_zc("set_ndi", {"machine": "ZCAM", "stream": "CAM 1", "group": "", "ttl": 1, "multicast": False, "discovery_1": "",
                "discovery_2": "", "multicast_netmask": "255.255.0.0", "multicast_prefix": "239.255.0.0",
                "bridge_name": "", "bridge_key": "", "bridge_server": "", "bridge_port": 5990},
    "/ctrl/ndi?action=set&machine=ZCAM&stream=CAM%201&group=&ttl=1&multicastEnable=0&discoveryServer1="
    "&discoveryServer2=&multicastNetmask=255.255.0.0&multicastPrefix=239.255.0.0&bridgeName="
    "&bridgeEncryptionKey=&bridgeServer=&bridgePort=5990")
_zc("get_rtsp", {}, "/ctrl/rtsp?action=query")
_zc("set_rtsp_authentication", {"enabled": True}, "/ctrl/rtsp?action=set&authentication=1")
_zc("get_network_mode", {}, "/ctrl/network?action=query")
_zc("get_network_info", {}, "/ctrl/network?action=info")
_zc("set_network_router", {}, "/ctrl/network?action=set&mode=Router")
_zc("set_network_direct", {}, "/ctrl/network?action=set&mode=Direct")
_zc("set_network_static", {"address": "192.168.1.100", "netmask": "255.255.255.0", "gateway": "192.168.1.1"},
    "/ctrl/network?action=set&mode=Static&ipaddr=192.168.1.100&netmask=255.255.255.0&gateway=192.168.1.1")
_zc("set_network_static_dns", {"address": "192.168.1.100", "netmask": "255.255.255.0", "gateway": "192.168.1.1",
                               "dns": "8.8.8.8"},
    "/ctrl/network?action=set&mode=Static&ipaddr=192.168.1.100&netmask=255.255.255.0&gateway=192.168.1.1&dns=8.8.8.8")
_zc("get_wifi", {}, "/ctrl/wifi_ctrl?action=query")
_zc("get_usb", {}, "/ctrl/usb?action=query")
_zc("get_ip_access", {}, "/ipac")
for _l, _n in (("allowlist", "allow"), ("denylist", "deny")):
    _zc(f"enable_{_n}_list", {}, f"/ipac/{_l}/enable")
    _zc(f"disable_{_n}_list", {}, f"/ipac/{_l}/disable")
    _zc(f"add_to_{_n}_list", {"address": "10.0.0.9"}, f"/ipac/{_l}/add?ip=10.0.0.9")
    _zc(f"remove_from_{_n}_list", {"address": "10.0.0.9"}, f"/ipac/{_l}/delete?ip=10.0.0.9")
_zc("generate_certificate", {}, "/cert/gen")
_zc("get_certificate", {}, "/cert/info")
_zc("delete_certificate", {}, "/cert/delete")
_zc("get_current_user", {}, "/login/me")
_zc("get_users", {}, "/login/user")
_zc("add_user", {"user": "op", "password": "p@ss", "group": "operator"},
    "/login/adduser?user=op&pswd=p%40ss&grp=operator")
_zc("delete_user", {"user": "op"}, "/login/deluser?user=op")
_zc("change_password", {"user": "op", "old_password": "a", "new_password": "b"}, "/login/pswd?user=op&old=a&new=b")
_zc("logout", {}, "/login/quit")
_zc("get_snmp", {}, "/ctrl/snmp")
_zc("set_snmp_enabled", {"enabled": True}, "/ctrl/snmp?enable=1")
_zc("set_snmp_security", {"user": "nms", "auth": "SHA", "encryption": "AES", "password": "secret1"},
    "/ctrl/snmp?user=nms&auth=SHA&enc=AES&pass=secret1")
_zc("set_snmp_system_name", {"name": "Cam A"}, "/ctrl/snmp?system=Cam%20A")
_zc("set_snmp_location", {"location": "Stage"}, "/ctrl/snmp?location=Stage")
_zc("set_snmp_contact", {"contact": "ops"}, "/ctrl/snmp?contact=ops")
_zc("get_freed", {}, "/ctrl/freed")
_zc("set_freed_enabled", {"enabled": True}, "/ctrl/freed?enable=1")
_zc("set_freed_camera_id", {"id": 3}, "/ctrl/freed?camera_id=3")
_zc("set_freed_destination", {"index": 1, "address": "10.0.0.20", "port": 6301}, "/ctrl/freed?ip1=10.0.0.20&port1=6301")
_zc("check_firmware", {}, "/ctrl/upgrade?action=fw_check")
_zc("run_firmware_update", {}, "/ctrl/upgrade?action=run")
_zc("check_ui_update", {}, "/ctrl/upgrade?action=ui_check")
_zc("get_card_present", {}, "/ctrl/card?action=present")
_zc("get_card_free", {}, "/ctrl/card?action=query_free")
_zc("get_card_total", {}, "/ctrl/card?action=query_total")
_zc("get_card_format", {}, "/ctrl/card?action=query_format")
_zc("format_card", {}, "/ctrl/card?action=format")
_zc("format_card_fat32", {}, "/ctrl/card?action=fat32")
_zc("format_card_exfat", {}, "/ctrl/card?action=exfat")
_zc("list_folders", {}, "/DCIM/")
_zc("list_files", {"folder": "100ZCAME"}, "/DCIM/100ZCAME/")
_zc("delete_file", {"folder": "110ZCAME", "file": "ZCAM0099_0000_201811121522.MOV"},
    "/DCIM/110ZCAME/ZCAM0099_0000_201811121522.MOV?act=rm")
_zc("get_file_info", {"folder": "110ZCAME", "file": "ZCAM0100_0000_201811121522.MOV"},
    "/DCIM/110ZCAME/ZCAM0100_0000_201811121522.MOV?act=info")
_zc("get_file_created", {"folder": "110ZCAME", "file": "ZCAM0101_0000_201811121539.MOV"},
    "/DCIM/110ZCAME/ZCAM0101_0000_201811121539.MOV?act=ct",
    http_reply={"status": 200, "body": '{"code":0,"desc":"","msg":1542037145}'},
    expect_result={"ok": {"kind": "value", "value": 1542037145}})

# Telemetry: the document's example answers.
telemetry(ZC, "setting-choice", inbound_http={"path": "/ctrl/get?k=video_system", "body": json.dumps(
    {"code": 0, "desc": "...", "key": "video_system", "type": 1, "ro": 0, "value": "NTSC",
     "opts": ["NTSC", "PAL", "CINEMA"]})},
    expect_state={"settings": {"video_system": {"value": "NTSC", "kind": "choice", "read_only": False,
                                                "options": '["NTSC","PAL","CINEMA"]', "supported": True}}})
telemetry(ZC, "setting-range", inbound_http={"path": "/ctrl/get?k=mwb_r_offset", "body": json.dumps(
    {"code": 0, "desc": "string", "key": "mwb_r_offset", "type": 2, "ro": 0, "value": 0, "min": -100, "max": 100,
     "step": 1})},
    expect_state={"settings": {"mwb_r_offset": {"value": "0", "kind": "range", "read_only": False, "min": -100,
                                                "max": 100, "step": 1, "supported": True}}})
telemetry(ZC, "setting-unsupported", inbound_http={"path": "/ctrl/get?k=genlock",
                                                   "body": '{"code":-1,"desc":"","key":"genlock"}'},
          expect_state={"settings": {"genlock": {"supported": False}}})
telemetry(ZC, "info", inbound_http={"path": "/info", "body": json.dumps(
    {"model": "e2", "sw": "1.0.0", "mac": "4e:4:b8:2d:78:db", "eth_ip": "192.168.9.81", "sn": "329A0010009"})},
    expect_state={"info": {"model": "e2", "firmware": "1.0.0", "mac": "4e:4:b8:2d:78:db", "ip": "192.168.9.81",
                           "serial": "329A0010009"}})
telemetry(ZC, "mode", inbound_http={"path": "/ctrl/mode?action=query", "body": '{"code":0,"desc":"","msg":"rec_ing"}'},
          expect_state={"mode": "rec_ing"})
telemetry(ZC, "remaining", inbound_http={"path": "/ctrl/rec?action=remain", "body": '{"code":0,"desc":"","msg":"125"}'},
          expect_state={"recording": {"remaining_minutes": 125}})
telemetry(ZC, "card-free", inbound_http={"path": "/ctrl/card?action=query_free", "body": '{"code":0,"desc":"","msg":"61200"}'},
          expect_state={"card": {"free": "61200"}})
telemetry(ZC, "card-total", inbound_http={"path": "/ctrl/card?action=query_total", "body": '{"code":0,"desc":"","msg":"121900"}'},
          expect_state={"card": {"total": "121900"}})
telemetry(ZC, "card-present", inbound_http={"path": "/ctrl/card?action=present", "body": '{"code":0,"desc":"","msg":"Yes"}'},
          expect_state={"card": {"present": '{"code":0,"desc":"","msg":"Yes"}'}})
telemetry(ZC, "srt", inbound_http={"path": "/ctrl/srt?action=query", "body": '{"code":0,"status":"idle"}'},
          expect_state={"srt": '{"code":0,"status":"idle"}'})
telemetry(ZC, "stream1", inbound_http={"path": "/ctrl/stream_setting?action=query&index=stream1",
                                       "body": '{"code":0,"width":1920}'},
          expect_state={"streams": {"stream1": '{"code":0,"width":1920}'}})
telemetry(ZC, "rtmp", inbound_http={"path": "/ctrl/rtmp?action=query&index=0", "body": '{"code":0}'},
          expect_state={"rtmp": {"0": '{"code":0}'}})
for _p, _k in (("/ctrl/tc?action=query", "timecode"), ("/ctrl/nick_name", "nick_name"),
               ("/ctrl/temperature", "temperature"), ("/ctrl/ndi?action=query", "ndi"),
               ("/ctrl/pt?action=query", "pan_tilt"), ("/ctrl/network?action=info", "network")):
    telemetry(ZC, _k.replace("_", "-"), inbound_http={"path": _p, "body": '{"code":0,"msg":"x"}'},
              expect_state={_k: '{"code":0,"msg":"x"}'})
