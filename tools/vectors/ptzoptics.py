# PTZOptics (ptzoptics): one vector per command. Targets are written from the
# PTZOptics HTTP API document (API version 1.0, 2026-09-29) -- each endpoint's
# curl example where it has one, with the parameter changed where noted -- and,
# for the _g2 commands, from the 2021-10-15 HTTP-CGI Control Sheet's URL
# patterns. Arguments are positional and sent verbatim (raw_query).
P = "ptzoptics"
OK = '{"Response":{"Result":"Success"}}'
ACK = {"ok": {"kind": "ack"}}
PTZ = "/cgi-bin/ptzctrl.cgi?"
PRM = "/cgi-bin/param.cgi?"

# Movement (p.119-138; 2021 sheet p.1)
http(P, "move", {"direction": "rightup", "pan_speed": 15, "tilt_speed": 13}, "GET", PTZ + "ptzcmd&rightup&15&13",
     http_reply={"status": 200, "body": OK}, expect_result=ACK)
http(P, "move", {"direction": "left", "pan_speed": 12}, "GET", PTZ + "ptzcmd&left&12&10")
http(P, "stop_move", {}, "GET", PTZ + "ptzcmd&ptzstop&1&1")
http(P, "zoom_in", {"speed": 5}, "GET", PTZ + "ptzcmd&zoomin&5")
http(P, "zoom_in", {"speed": 0}, "GET", PTZ + "ptzcmd&zoomin&0")
http(P, "zoom_out", {"speed": 7}, "GET", PTZ + "ptzcmd&zoomout&7")
http(P, "stop_zoom", {}, "GET", PTZ + "ptzcmd&zoomstop&0")
http(P, "zoom_to", {"speed": 5, "position": "3000"}, "GET", PTZ + "ptzcmd&zoomto&5&3000")
http(P, "focus_in", {"speed": 5}, "GET", PTZ + "ptzcmd&focusin&5")
http(P, "focus_out", {}, "GET", PTZ + "ptzcmd&focusout&5")
http(P, "stop_focus", {}, "GET", PTZ + "ptzcmd&focusstop&0")
http(P, "home", {}, "GET", PTZ + "ptzcmd&home")
http(P, "pan_tilt_reset", {}, "GET", PRM + "pan_tiltdrive_reset")
http(P, "position_absolute", {"pan_speed": 15, "tilt_speed": 10, "pan": "0200", "tilt": "0200"}, "GET",
     PTZ + "ptzcmd&abs&15&10&0200&0200")
http(P, "position_relative", {"pan_speed": 24, "tilt_speed": 20, "pan": "F670", "tilt": "FE51"}, "GET",
     PTZ + "ptzcmd&rel&24&20&F670&FE51")
http(P, "recall_preset", {"preset": 3}, "GET", PTZ + "ptzcmd&poscall&3",
     http_reply={"status": 404}, expect_result={"error": {"error": "device_error", "code": "404"}})
http(P, "save_preset", {"preset": 1}, "GET", PTZ + "ptzcmd&posset&1")
http(P, "recall_preset_extended", {"preset": 254}, "GET", PTZ + "ptzcmd&poscall&254")
http(P, "save_preset_extended", {"preset": 100}, "GET", PTZ + "ptzcmd&posset&100")
http(P, "recall_preset_with_speed", {"preset": 1, "pan_speed": 10, "tilt_speed": 10, "zoom_speed": 5}, "POST",
     PTZ + "post_preset&poscallwithspeed=1&panspeed=10&tiltspeed=10&zoomspeed=5")
http(P, "recall_preset_with_speed_extended", {"preset": 120, "pan_speed": 24, "tilt_speed": 20, "zoom_speed": 7},
     "POST", PTZ + "post_preset&poscallwithspeed=120&panspeed=24&tiltspeed=20&zoomspeed=7")
http(P, "snap_focus", {}, "GET", PRM + "set_focus&one_shot_focus")

# Focus (p.65-76)
http(P, "focus_lock", {"action": "lock"}, "GET", PRM + "ptzcmd&lock_mfocus")
http(P, "focus_mode", {"mode": "3"}, "GET", PTZ + "post_image_value&focusmode&3")
http(P, "af_zone", {"zone": 3}, "GET", PTZ + "post_image_value&focusregion&3")
http(P, "af_sensitivity", {"sensitivity": 1}, "GET", PTZ + "post_image_value&focussense&1")
http(P, "focus_limit", {"enabled": True, "near": 3, "far": 8}, "GET",
     PRM + "post_zoomfocus&type=focusrange&enable=1&near=3&far=8")
http(P, "time_of_flight", {"state": 2}, "GET", PTZ + "post_image_value&tof&2")

# Exposure (p.37-64)
http(P, "exposure_mode", {"mode": "3"}, "GET", PTZ + "post_image_value&aemode&3",
     http_reply={"status": 200, "body": OK}, expect_result=ACK)
http(P, "exposure_compensation", {"state": 2}, "GET", PTZ + "post_image_value&expcomp_mode&2")
http(P, "exposure_compensation_level", {"level": 3}, "GET", PTZ + "post_image_value&expcomp_level&3")
http(P, "backlight", {"state": 2}, "GET", PTZ + "post_image_value&backlight&2")
http(P, "anti_flicker", {"mode": 2}, "GET", PTZ + "post_image_value&antiflicker&2")
http(P, "gain_limit", {"level": 3}, "GET", PTZ + "post_image_value&gainLimit&3")
http(P, "gain", {"level": 5}, "GET", PTZ + "post_image_value&gain&5")
http(P, "iris", {"value": 11}, "GET", PTZ + "post_image_value&iris&11")
http(P, "iris_se", {"value": "16"}, "GET", PTZ + "post_image_value&iris&16", model="move-se")
http(P, "shutter", {"value": 2}, "GET", PTZ + "post_image_value&shutter&2")
http(P, "meter_region", {"region": 1}, "GET", PTZ + "post_image_value&meter&1")
http(P, "dynamic_range", {"level": 5}, "GET", PTZ + "post_image_value&drc&5")

# Colour (p.18-36)
http(P, "white_balance_mode", {"mode": "5"}, "GET", PTZ + "post_image_value&wbmode&5")
http(P, "white_balance_one_push", {}, "GET", PTZ + "post_image_value&onepush&0")
http(P, "color_temperature", {"index": 31}, "GET", PTZ + "post_image_value&colortemp&31")
http(P, "red_gain", {"level": 200}, "GET", PTZ + "post_image_value&rgain&200")
http(P, "blue_gain", {"level": 200}, "GET", PTZ + "post_image_value&bgain&200")
http(P, "red_gain_tuning", {"level": 11}, "GET", PTZ + "post_image_value&rgaintuning&11")
http(P, "blue_gain_tuning", {"level": 11}, "GET", PTZ + "post_image_value&bgaintuning&11")

# Image (p.77-88)
http(P, "contrast", {"level": 6}, "GET", PTZ + "post_image_value&contrast&6")
http(P, "hue", {"level": 14}, "GET", PTZ + "post_image_value&hue&14")
http(P, "luminance", {"level": 7}, "GET", PTZ + "post_image_value&luminance&7")
http(P, "saturation", {"level": 7}, "GET", PTZ + "post_image_value&saturation&7")
http(P, "sharpness", {"level": 8}, "GET", PTZ + "post_image_value&sharpness&8")
http(P, "sharpness_se", {"level": 5}, "GET", PTZ + "post_image_value&sharpness&5", model="move-se")

# Mode (p.89-118)
http(P, "noise_reduction_2d", {"level": 6}, "GET", PTZ + "post_image_value&noise2d&6", model="move-se")
http(P, "noise_reduction_3d", {"level": 9}, "GET", PTZ + "post_image_value&noise3d&9")
http(P, "auto_inversion", {"state": "2"}, "GET", PTZ + "post_image_value&inversionmode&2")
http(P, "auto_tracking", {"state": "on"}, "GET", PRM + "set_overlay&autotracking&on")
http(P, "bw_mode", {"enabled": True}, "GET", PTZ + "post_image_value&bwmode&1")
http(P, "bounding_boxes", {"mode": 4}, "GET", PTZ + "post_image_value&trackbox&4")
http(P, "display_info", {"state": 2}, "GET", PTZ + "post_image_value&displayinfo&2")
http(P, "image_orientation", {"orientation": 2}, "GET", PTZ + "post_image_value&imageorientation&2")
http(P, "motion_sync", {"state": 2}, "GET", PTZ + "post_image_value&motionsync&2")
http(P, "patrol_mode", {"state": 2}, "GET", PTZ + "post_image_value&presetinspection&2")
http(P, "preset_freeze", {"state": 2}, "GET", PTZ + "post_image_value&presetfreeze&2")
http(P, "tally_mode", {"state": 2}, "GET", PTZ + "post_image_value&tallymode&2")
http(P, "tracking_overlay", {"enabled": True}, "GET", PTZ + "post_image_value&trackingoverlay&1", model="move-se")
http(P, "tracking_start_location", {"location": "1"}, "GET", PTZ + "post_image_value&trackpreset&1")
http(P, "tracking_start_location", {"location": "home"}, "GET", PTZ + "post_image_value&trackpreset&home")
http(P, "zoom_mode", {"mode": 2}, "GET", PTZ + "post_image_value&digitalzoom&2")

# OSD (p.159-162; 2021 sheet p.1-2)
http(P, "osd_menu", {}, "GET", PRM + "navigate_mode&OSD")
http(P, "osd_ptz_mode", {}, "GET", PRM + "navigate_mode&PTZ")
http(P, "osd_confirm", {}, "GET", PRM + "navigate_mode&CONFIRM")
http(P, "osd_back", {}, "GET", PRM + "navigate_mode&OSD_BACK")
http(P, "osd_navigate", {"direction": "up"}, "GET", PTZ + "ptzcmd&osd=up")

# Audio (p.4-17)
http(P, "audio_encoding", {"mode": 1}, "POST", PRM + "post_media_audio&audio_switch=1")
http(P, "audio_adts", {"enabled": True}, "POST", PRM + "post_media_audio&adts_flag=1")
http(P, "audio_bitrate", {"kbps": "96"}, "POST", PRM + "post_media_audio&streamrate=96")
http(P, "audio_sample_rate", {"khz": "44.1"}, "POST", PRM + "post_media_audio&samplerate=44.1")
http(P, "audio_input_volume", {"volume": 30}, "POST", PRM + "post_media_audio&volume_value=30")
http(P, "audio_line_out_volume", {"volume": 30}, "POST", PRM + "post_media_audio&lineout_volume=30")
http(P, "usb_audio", {"enabled": True}, "POST", PRM + "post_media_audio&usbaudio_switch=1")

# Video (p.249-277): the examples' key=value form
http(P, "video_template", {"template": "ultra"}, "POST", PRM + "post_media_video&ndi_mode=ultra")
http(P, "video_outputs", {"outputs": "USB_HDMI-SDI"}, "POST", PRM + "post_media_video&video_ability=USB_HDMI-SDI")
http(P, "video_advanced", {"mode": "hybrid"}, "POST", PRM + "post_media_video&advanced=hybrid")
http(P, "encoding_profile", {"profile": "highprofile"}, "POST", PRM + "post_media_video&profile=highprofile")
http(P, "hdmi_sdi_output", {"output": "1"}, "POST", PRM + "post_media_video&hdmi_sdi=1")
http(P, "refresh_rate", {"rate": "60"}, "POST", PRM + "post_media_video&vinorm=60")
http(P, "studio_pro_hdmi_format", {"format": "1080P60"}, "POST", PRM + "post_media_video&hdmivideoformat=1080P60",
     model="studio-pro")
http(P, "stream_protocol", {"stream": 1, "protocol": "H265"}, "POST", PRM + "post_media_video&protocol_1=H265")
http(P, "stream_resolution", {"stream": 2, "resolution": "PIC_HD720"}, "POST", PRM + "post_media_video&size_2=PIC_HD720")
http(P, "stream_bitrate", {"stream": 1, "kbps": 6000}, "POST", PRM + "post_media_video&bps_1=6000")
http(P, "stream_frame_rate", {"stream": 2, "fps": 15}, "POST", PRM + "post_media_video&fps_2=15")
http(P, "stream_keyframe_interval", {"stream": 1, "frames": 60}, "POST", PRM + "post_media_video&gop_1=60")
http(P, "stream_rate_control", {"stream": 1, "mode": "VBR"}, "POST", PRM + "post_media_video&rcmode_1=VBR")
http(P, "stream_qfactor", {"stream": 2, "qfactor": 70}, "POST", PRM + "post_media_video&qfactor_2=70")

# Streaming (p.199-238)
http(P, "rtmp", {"stream": 1, "enabled": True}, "POST", PRM + "post_network_other_conf&rtmp1_en=1")
http(P, "rtmp_url", {"stream": 1, "url": "rtmp://streaming-server.com/live"}, "POST",
     PRM + "post_network_other_conf&rtmp1_mrl=rtmp://streaming-server.com/live")
http(P, "rtmp_key", {"stream": 1, "key": "my-stream-key"}, "POST", PRM + "post_network_other_conf&rtmp1_key=my-stream-key")
http(P, "srt", {"enabled": True}, "POST", PRM + "post_network_other_conf&srt_en=1")
http(P, "srt_mode", {"mode": "caller"}, "POST", PRM + "post_network_other_conf&srt_mode=caller")
http(P, "srt_server", {"server": "192.168.1.100"}, "POST", PRM + "post_network_other_conf&srt_server=192.168.1.100")
http(P, "srt_port", {"port": 5000}, "POST", PRM + "post_network_other_conf&srt_port=5000")
http(P, "srt_latency", {"ms": 120}, "POST", PRM + "post_network_other_conf&srt_latency=120")
http(P, "srt_bandwidth_overhead", {"percent": 25}, "POST", PRM + "post_network_other_conf&srt_bw_precent=25")
http(P, "srt_encryption", {"encryption": 1}, "POST", PRM + "post_network_other_conf&srt_passsid=1")
http(P, "srt_password", {"password": "secretpass"}, "POST", PRM + "post_network_other_conf&srt_passtr=secretpass")
http(P, "srt_stream_id", {"stream_id": "stream123"}, "POST", PRM + "post_network_other_conf&srt_streamid_str=stream123")
http(P, "rtsp_port", {"port": 8554}, "POST", PRM + "post_network_other_conf&rtsp_port=8554")
http(P, "rtsp_auth", {"enabled": True}, "POST", PRM + "post_network_other_conf&rtsp_auth_en=1")
http(P, "rtp_multicast", {"enabled": True}, "POST", PRM + "post_network_other_conf&multicast_en=1")
http(P, "rtp_multicast_address", {"address": "239.0.0.1"}, "POST", PRM + "post_network_other_conf&multi_rtp_addr=239.0.0.1")
http(P, "rtp_multicast_port", {"port": 5004}, "POST", PRM + "post_network_other_conf&multi_rtp_port=5004")
http(P, "rtp_multicast_ttl", {"ttl": 5}, "POST", PRM + "post_network_other_conf&multi_ttl=5")
http(P, "onvif", {"enabled": True}, "POST", PRM + "post_network_other_conf&onvif_en=1")
http(P, "onvif_auth", {"enabled": True}, "POST", PRM + "post_network_other_conf&onvif_auth_en=1")

# Network (p.139-158) and ports (p.163-168)
http(P, "ndi_name", {"name": "Stage Left #2"}, "GET", PRM + "post_ndi_info&channelName=Stage%20Left%20%232")
http(P, "ndi_group", {"group": "PTZGroup"}, "GET", PRM + "post_ndi_info&group=PTZGroup")
http(P, "ndi_discovery", {"enabled": True}, "GET", PRM + "post_ndi_info&discovery_en=1")
http(P, "ndi_multicast", {"enabled": True}, "GET", PRM + "post_ndi_info&multicast_en=1")
http(P, "device_name", {"name": "Conference_Room_Camera"}, "POST", PRM + "post_devinfo_conf&devname=Conference_Room_Camera",
     http_reply={"status": 200, "body": '{\n"Response": {\n"Result": "Success"\n}\n}'}, expect_result=ACK)
http(P, "device_name", {"name": "Cam_2"}, "POST", PRM + "post_devinfo_conf&devname=Cam_2",
     http_reply={"status": 200, "body": "{}"}, expect_result={"error": {"error": "device_error"}})
http(P, "dhcp", {"enabled": True}, "POST", PRM + "post_network_info_conf&dhcp=1")
http(P, "ip_address", {"address": "192.168.1.100"}, "POST", PRM + "post_network_info_conf&ipaddr=192.168.1.100")
http(P, "netmask", {"mask": "255.255.255.0"}, "POST", PRM + "post_network_info_conf&netmask=255.255.255.0")
http(P, "gateway", {"address": "192.168.1.1"}, "POST", PRM + "post_network_info_conf&gateway=192.168.1.1")
http(P, "dns", {"address": "8.8.8.8"}, "POST", PRM + "post_network_info_conf&fdns=8.8.8.8")
http(P, "http_port", {"port": 1025}, "POST", PRM + "post_network_other_conf&httpport=1025")
http(P, "visca_tcp_port", {"port": 1025}, "POST", PRM + "post_network_other_conf&ptzport=1025")
http(P, "visca_udp_port", {"port": 5678}, "POST", PRM + "post_network_other_conf&udpport=5678")

# System (p.239-248)
http(P, "reboot", {}, "POST", PRM + "post_reboot")
http(P, "factory_reset", {}, "GET", PRM + "set_overlay&fullreset&trigger")
http(P, "osd_reset", {}, "GET", PRM + "set_overlay&osdreset&trigger")
http(P, "network_reset", {}, "GET", PRM + "set_overlay&networkreset&trigger")
http(P, "ir_channel", {"channel": 1}, "GET", PRM + "post_ir_info=&ir_id=1")

# Queries (p.169-198), with the documented example replies
http(P, "get_device_config", {}, "GET", PRM + "get_device_conf",
     http_reply={"status": 200, "body": '{"status": 200, "data": {"devname": "ptzopticsmove4k", "devtype": "VX60AS"}}'},
     expect_result={"ok": {"kind": "value", "value": {"devname": "ptzopticsmove4k", "devtype": "VX60AS"}}})
http(P, "get_serial_number", {}, "GET", PRM + "get_serial_number",
     http_reply={"status": 200, "body": '{\n"status": 200,\n"data": "q1i09250211"\n}'},
     expect_result={"ok": {"kind": "value", "value": "q1i09250211"}})
http(P, "get_tally_status", {}, "GET", PRM + "get_tally_status",
     http_reply={"status": 200, "body": '{"status": 200, "data": {"tally": "Off", "standby": "0"}}'},
     expect_result={"ok": {"kind": "value", "value": {"tally": "Off", "standby": "0"}}})
http(P, "get_language_config", {}, "GET", PRM + "get_language_conf")
http(P, "get_ndi_config", {}, "GET", PRM + "get_ndi_info")
http(P, "get_login_status", {}, "GET", PRM + "get_login_info",
     http_reply={"status": 200, "body": '{"status": 200, "data": {"is_login": "0"}}'},
     expect_result={"ok": {"kind": "value", "value": {"is_login": "0"}}})
http(P, "get_system_config", {}, "GET", PRM + "get_system_conf")
http(P, "get_advanced_image_config", {}, "GET", PRM + "get_advance_image_conf",
     http_reply={"status": 200, "body": 'focus_mode="1"\nfocus_zone="1"\n'},
     expect_result={"ok": {"kind": "value", "value": 'focus_mode="1"\nfocus_zone="1"'}})
http(P, "get_image_config", {}, "GET", PRM + "get_image_conf")
http(P, "get_audio_config", {}, "GET", PRM + "get_media_audio")
http(P, "get_video_config", {}, "GET", PRM + "get_media_video")
http(P, "get_ir_channel", {}, "GET", PRM + "query_ir_info",
     http_reply={"status": 200, "body": 'IR_Channel="1"'}, expect_result={"ok": {"kind": "value", "value": "1"}})

# G2 cameras (2021 HTTP-CGI Control Sheet)
G = "ptz-g2"
http(P, "brightness_g2", {"level": 7}, "GET", PRM + "post_image_value&bright&7", model=G,
     http_reply={"status": 200, "body": ""}, expect_result=ACK)
http(P, "saturation_g2", {"level": 0}, "GET", PRM + "post_image_value&saturation&0", model=G)
http(P, "contrast_g2", {"level": 14}, "GET", PRM + "post_image_value&contrast&14", model=G)
http(P, "sharpness_g2", {"level": 3}, "GET", PRM + "post_image_value&sharpness&3", model=G)
http(P, "hue_g2", {"level": 9}, "GET", PRM + "post_image_value&hue&9", model=G)
http(P, "flip_g2", {"enabled": True}, "GET", PRM + "post_image_value&flip&1", model=G)
http(P, "mirror_g2", {"enabled": False}, "GET", PRM + "post_image_value&mirror&0", model=G)
http(P, "image_defaults_g2", {}, "GET", PRM + "get_image_default_conf", model=G)
http(P, "exposure_mode_g2", {"mode": "shutter"}, "GET", PRM + "post_image_value&aemode&shutter", model=G)
http(P, "white_balance_mode_g2", {"mode": "onepush"}, "GET", PRM + "post_image_value&wbmode&onepush", model=G)
http(P, "white_balance_one_push_g2", {}, "GET", PRM + "post_image_value&wbmode&trigger", model=G)
http(P, "osd_navigate_g2", {"direction": "down"}, "GET", PTZ + "ptzcmd&down", model=G)
http(P, "snapshot_resolution_g2", {"size": "960x600"}, "GET",
     "/cgi-bin/snapshot.cgi?post_snapshot_conf&resolution=960x600", model=G)
http(P, "photobooth_photos_g2", {"delay": 5}, "GET", "/cgi-bin/booth.cgi?0&4&5&photo&0", model=G)
http(P, "photobooth_videos_g2", {"delay": 3, "length": 10}, "GET", "/cgi-bin/booth.cgi?0&4&3&video&10", model=G)
http(P, "get_video_config_g2", {}, "GET", PRM + "get_media_video", model=G)
http(P, "get_audio_config_g2", {}, "GET", PRM + "get_media_audio", model=G)
http(P, "get_network_config_g2", {}, "GET", PRM + "get_network_conf", model=G)
http(P, "get_device_config_g2", {}, "GET", PRM + "get_device_conf", model=G)
http(P, "get_serial_number_g2", {}, "GET", PRM + "get_serial_number", model=G)

# Telemetry: the documented example replies (Query, p.171-198).
telemetry(P, "advanced-image", inbound_http={"path": PRM + "get_advance_image_conf", "body": (
    'focus_mode="1"\nfocus_zone="1"\nfocus_sens="2"\nfocuslimit="0"\nfurthestpos="11"\nnearestpos="0"\n'
    'focus_mode="1"\nexposure_mode="11"\nexpcomp_mode="2"\nexpcomp_level="4"\nbacklight="3"\nantiflicker="2"\n'
    'bright="7"\ngain_limit="5"\nmanual_gain="2"\nmeter="0"\ndrc="3"\nshutter="5"\niris="12"\nwb_mode="0"\n'
    'rgain_tuning="8"\nbgain_tuning="13"\nrgain="56"\nbgain="43"\nsaturation="3"\nhue="8"\ntemperature="40"\n'
    'style="0"\nluminance="8"\nsharpness="9"\ncontrast="9"\nblackwhite_mode="0"\nimageOrientation="0"\nnr3d="9"\n'
    'nr2d="0"\ndefault_language="English"\nfocus_mode_status="unlock"\npresetautoscan="3"\npresetfreeze="3"\n'
    'tallymode="1"\npresetinspection="3"\ntrackbox="4"\nautotrack="0"\nzoommode="0"\ndigitalzoom="0"\n'
    'motionsync="2"\ninversionMode="2"\n')},
    expect_state={
        "focus": {"mode": 1, "zone": "center", "sensitivity": "normal", "locked": False,
                  "limit": {"enabled": False, "near": 0, "far": 11}},
        "exposure": {"mode": "aae", "compensation": True, "compensation_level": 4, "backlight": 3,
                     "anti_flicker": "60Hz", "bright": 7, "gain_limit": 5, "gain": 2, "meter_region": "average",
                     "dynamic_range": 3, "shutter": 5, "iris": 12},
        "white_balance": {"mode": "auto", "red_gain_tuning": 8, "blue_gain_tuning": 13, "red_gain": 56,
                          "blue_gain": 43, "color_temperature": 40},
        "image": {"saturation": 3, "hue": 8, "luminance": 8, "sharpness": 9, "contrast": 9, "style": 0,
                  "black_and_white": False, "orientation": "normal", "noise_reduction_3d": 9,
                  "noise_reduction_2d": 0},
        "preset": {"auto_scan": 3, "freeze": False, "patrol": False},
        "tally": {"mode": 1},
        "tracking": {"bounding_boxes": "select_target", "auto": 0},
        "zoom": {"mode_reported": 0, "mode": "optical"},
        "motion_sync": True,
        "auto_inversion": True,
    })
telemetry(P, "image", inbound_http={"path": PRM + "get_image_conf", "body": (
    'style="0"\nluminance="8"\nsharpness="9"\ncontrast="9"\nblackwhite_mode="0"\nimageOrientation="0"\n'
    'presetautoscan="3"\npresetfreeze="3"\ntallymode="1"\npresetinspection="3"\ntrackbox="4"\nautotrack="0"\n'
    'zoommode="0"\ndigitalzoom="0"\nmotionsync="2"\ninversionMode="2"\ntrackswitch="0"\ntrackmode="tracking"\n'
    'boundingbox="4"\ntrackcomposition="median"\ntracksens="median"\ntrackzoom="2"\ntrackdelay="0"\n'
    'trackdelaytime="25"\ntrackstart="39"\ntrackstop="-1"\nlosstimeout="35"\nlosspreset="22"\nbright="8"\n'
    'saturation="3"\ncontrast="9"\nsharpness="9"\nhue="8"\nldc="0"\nflip="0"\nmirror="0"\n')},
    expect_state={
        "image": {"style": 0, "luminance": 8, "sharpness": 9, "contrast": 9, "black_and_white": False,
                  "orientation": "normal", "saturation": 3, "hue": 8, "lens_distortion_correction": 0,
                  "flip": False, "mirror": False},
        "exposure": {"bright": 8},
        "preset": {"auto_scan": 3, "freeze": False, "patrol": False},
        "tally": {"mode": 1},
        "tracking": {"bounding_boxes": "select_target", "auto": 0, "switch": 0, "mode": "tracking",
                     "composition": "median", "sensitivity": "median", "zoom": 2, "delay": 0, "delay_time": 25,
                     "start": 39, "stop": -1, "loss_timeout": 35, "loss_preset": 22},
        "zoom": {"mode_reported": 0, "mode": "optical"},
        "motion_sync": True,
        "auto_inversion": True,
    })
telemetry(P, "audio", inbound_http={"path": PRM + "get_media_audio", "body": (
    'audio_switch="1"\naeformat="AAC"\nadts_flag="0"\nsamplerate="48"\nstreamrate="128"\naudioinput="linein"\n'
    'vol_left_in="15"\nvol_right_in="15"\nvolume_value="6"\nlineout_volume="20"\nlineout_switch="0"\n'
    'usbaudio_switch="1"\n')},
    expect_state={"audio": {"encoding": "on", "format": "AAC", "adts": False, "sample_rate": "48", "bitrate": 128,
                            "input": "linein", "volume_left": 15, "volume_right": 15, "input_volume": 6,
                            "line_out_volume": 20, "line_out": False, "usb": True}})
telemetry(P, "video", inbound_http={"path": PRM + "get_media_video", "body": (
    'hdmi_sdi="1"\nvinorm="60"\nndi_mode="265P60"\nadvanced="off"\nprotocol_1="H265"\nprofile="mainprofile"\n'
    'size_1="PIC_HD1080"\nbps_1="50000"\nqfactor_1="20"\nfps_1="60"\ngop_1="20"\nrcmode_1="CBR"\n'
    'imagegrade_1="1"\nslice_en_1="0"\nslice_mode_1="1"\nslice_size1="68"\nprotocol_2="H264"\n'
    'size_2="PIC_640_360"\nbps_2="2048"\nqfactor_2="20"\nfps_2="30"\ngop_2="60"\nrcmode_2="CBR"\n'
    'imagegrade_2="1"\nslice_en_2="0"\nslice_mode_2="1"\nslice_size2="15"\nusbprotocol="H265"\n'
    'usbwidth="1920"\nusbheight="1080"\nusbframerate="60"\nvi_framerate="60"\ndial_support_4K_en="1"\n'
    'dial_support_framerate="60"\ndial_format="0"\nuhd4k="1"\n')},
    expect_state={"video": {
        "baseband_output": "hdmi", "refresh_rate": "60", "template": "265P60", "advanced": "off",
        "profile": "mainprofile", "frame_rate": 60, "uhd": True,
        "usb": {"protocol": "H265", "width": 1920, "height": 1080, "frame_rate": 60},
        "streams": {"1": {"protocol": "H265", "resolution": "PIC_HD1080", "bitrate": 50000, "qfactor": 20,
                          "frame_rate": 60, "keyframe_interval": 20, "rate_control": "CBR"},
                    "2": {"protocol": "H264", "resolution": "PIC_640_360", "bitrate": 2048, "qfactor": 20,
                          "frame_rate": 30, "keyframe_interval": 60, "rate_control": "CBR"}}}})
telemetry(P, "ir-channel", inbound_http={"path": PRM + "query_ir_info", "body": 'IR_Channel="1"'},
          expect_state={"ir_channel": 1})
telemetry(P, "advanced-image-json", inbound_http={
    "path": PRM + "get_advance_image_conf",
    "body": '{"status": 200, "data": {"wb_mode": "32", "expcomp_mode": "3", "focus_mode_status": "lock"}}'},
    expect_state={"white_balance": {"mode": "var"}, "exposure": {"compensation": False}, "focus": {"locked": True}})
telemetry(P, "device", inbound_http={"path": PRM + "get_device_conf", "body": (
    '{"status": 200, "data": {"devname": "ptzopticsmove4k", "devtype": "VX60AS", '
    '"mirrors": "https://firmware.ptzoptics.com/", "versioninfo": "SOC v0.0.85 - ARM 6.2.42SHIS", '
    '"serial_num": "q1i09250211", "device_model": "F51.HI"}}')},
    expect_state={"device": {"name": "ptzopticsmove4k", "type": "VX60AS", "firmware": "SOC v0.0.85 - ARM 6.2.42SHIS",
                             "serial": "q1i09250211", "model": "F51.HI"}})
telemetry(P, "language", inbound_http={"path": PRM + "get_language_conf", "body": (
    '{"status": 200, "data": {"default_language": "English", "focus_mode_status": "unlock", "isndi": "1", '
    '"isdante": "0", "streaming": "1", "standby": "1", "is_root_user": "0", "focusmode": "afocus"}}')},
    expect_state={"system": {"language": "English", "ndi_capable": True, "dante_capable": False, "streaming": "1",
                             "standby": "1"},
                  "focus": {"mode_name": "afocus", "locked": False}})
telemetry(P, "tally", inbound_http={"path": PRM + "get_tally_status", "body": (
    '{"status": 200, "data": {"tally": "Off", "standby": "0", "privacy": "0", '
    '"objs": [{"ID": "1", "X": 113, "Y": 9, "Width": 372, "Height": 350}], "shoulddraw": "0", "target": "0"}}')},
    expect_state={"tally": {"state": "Off", "standby": "0", "privacy": "0"}, "tracking": {"target": "0"}})
telemetry(P, "ndi", inbound_http={"path": PRM + "get_ndi_info", "body": (
    '{"status": 200, "data": {"channelName": "Chan1", "group": "Public", "discovery_en": "0", '
    '"discovery_server": "192.168.1.2", "multicast_en": "0", "multicast_ip": "239.255.0.10", '
    '"multicast_mask": "255.255.0.0", "multicast_ttl": "1", "fullndi_en": "0", "ndi": "PT5.1.1"}}')},
    expect_state={"ndi": {"name": "Chan1", "group": "Public", "discovery": False, "discovery_server": "192.168.1.2",
                          "multicast": False, "multicast_ip": "239.255.0.10", "multicast_mask": "255.255.0.0",
                          "multicast_ttl": 1, "full_ndi": False, "version": "PT5.1.1"}})
