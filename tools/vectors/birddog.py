# BirdDog cameras (birddog): one vector per command, over the RESTful API on
# port 8080. Targets and bodies are written here from BirdDog's documents:
# the RESTful API 2.0 HTML documentation (P-series and A-series; each
# section's curl example and DEVICE SUPPORT table) and the API 2.1 Postman
# collections for the X-series/MAX and the MAKI Ultra (each request's example
# body and reply). Bodies are compact JSON with every value a JSON string, as
# in every example; single-key POSTs follow the 2.1 examples. Where a document
# example exists the vector uses its values, otherwise values within the
# documented range. Replies are the documents' example replies.
BD = "birddog"
_OK = {"ok": {"kind": "ack"}}


def _bd(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": BD, "command": command, "input": input, "expect_request": request, **extra})


def _val(v):
    return {"ok": {"kind": "value", "value": v}}


def _err(code=None):
    e = {"error": "device_error"}
    if code is not None:
        e["code"] = str(code)
    return {"error": e}


# Device information (2.0 BasicDeviceInfo; 2.1 Device Information)
_bd("get_about", {}, "GET", "/about",
    http_reply={"status": 200, "body": '{"FallbackIP":"192.168.100.100","FirmwareVersion":"BirdDog X1 Ultra 5.6.057",'
                '"Format":"CAM HX","HostName":"birddog-608F9","Status":"Active"}'},
    expect_result=_val({"FallbackIP": "192.168.100.100", "FirmwareVersion": "BirdDog X1 Ultra 5.6.057",
                        "Format": "CAM HX", "HostName": "birddog-608F9", "Status": "Active"}))
_bd("get_version", {}, "GET", "/version", http_reply={"status": 200, "body": "BirdDog X1"},
    expect_result=_val("BirdDog X1"))
_bd("get_hostname", {}, "GET", "/hostname", http_reply={"status": 200, "body": "x1-046CE"},
    expect_result=_val("x1-046CE"))
_bd("get_operation_mode", {}, "GET", "/operationmode", model="x5-ultra",
    http_reply={"status": 200, "body": "dual"}, expect_result=_val("dual"))
_bd("set_operation_mode", {"mode": "encode"}, "POST", "/operationmode", "encode",
    http_reply={"status": 200, "body": "encode"}, expect_result=_OK)
_bd("reboot", {}, "POST", "/reboot", http_reply={"status": 200, "body": '{"status":"success"}'}, expect_result=_OK)
_bd("restart_video", {}, "POST", "/restart")
_bd("factory_reset", {}, "POST", "/ResetSystemDefault")
_bd("reset_image", {}, "POST", "/RestImage", http_reply={"status": 200, "body": '{"Status":"success"}'},
    expect_result=_OK)

# Video output interface
_bd("get_video_output", {}, "GET", "/videooutputinterface", model="p400",
    http_reply={"status": 200, "body": "NormalMode"}, expect_result=_val("NormalMode"))
_bd("set_video_output_mode_p400", {"mode": "NormalMode"}, "POST", "/videooutputinterface", "NormalMode")
_bd("set_output_format", {"format": "1080p60"}, "POST", "/videooutputinterface", '{"OutputFormat":"1080p60"}',
    model="maki-ultra", http_reply={"status": 200, "body": '{"OutputFormat":"1080p60"}'}, expect_result=_OK)
_bd("set_output_format_x5u", {"format": "1920x1080p50"}, "POST", "/videooutputinterface",
    '{"OutputFormat":"1920x1080p50"}')
_bd("set_privacy_mode", {"state": "On"}, "POST", "/videooutputinterface", '{"PrivacyMode":"On"}')
_bd("set_video_output", {"output": "sdi"}, "POST", "/videooutputinterface", '{"videoformat":"sdi"}')
_bd("set_output_frequency", {"hz": "50"}, "POST", "/videooutputinterface", '{"Frequency":"50"}')
_bd("set_hdmi1_output_decoder", {"state": "Off"}, "POST", "/videooutputinterface", '{"HDMI-1OutputDecoder":"Off"}')

# Network (2.1 Network Settings)
_bd("get_ethernet", {}, "GET", "/EthSetup")
_bd("set_ethernet", {"body": {"DhcpTimeout": "33"}}, "POST", "/EthSetup", '{"DhcpTimeout":"33"}')
_bd("set_ip_mode", {"mode": "static"}, "POST", "/EthSetup", '{"IpMode":"static"}')
_bd("set_ip_address", {"address": "192.168.20.46"}, "POST", "/EthSetup", '{"IpAddr":"192.168.20.46"}')
_bd("set_netmask", {"address": "255.255.255.0"}, "POST", "/EthSetup", '{"NetMask":"255.255.255.0"}')
_bd("set_gateway", {"address": "192.168.20.20"}, "POST", "/EthSetup", '{"GateWayIp":"192.168.20.20"}')
_bd("set_dns_primary", {"address": "9.9.9.9"}, "POST", "/EthSetup", '{"DnsSvrIpFirst":"9.9.9.9"}')
_bd("set_dns_secondary", {"address": "1.1.1.1"}, "POST", "/EthSetup", '{"DnsSvrIpSecond":"1.1.1.1"}')
_bd("set_fallback_ip", {"address": "192.168.100.100"}, "POST", "/EthSetup", '{"DhcpFallbackIPAddr":"192.168.100.100"}')
_bd("set_fallback_netmask", {"address": "255.255.255.0"}, "POST", "/EthSetup", '{"DhcpFallbackNetMask":"255.255.255.0"}')
_bd("set_fallback_gateway", {"address": "192.168.1.1"}, "POST", "/EthSetup", '{"DhcpFallbackGateWayIp":"192.168.1.1"}')
_bd("set_dhcp_timeout", {"seconds": 33}, "POST", "/EthSetup", '{"DhcpTimeout":"33"}',
    http_reply={"status": 200, "body": '{"IpMode":"dhcp","IpAddr":"192.168.20.46","DhcpTimeout":"33"}'},
    expect_result=_OK)
_bd("get_wifi", {}, "GET", "/WifiSetup")
_bd("set_wifi_enabled", {"state": "On"}, "POST", "/WifiSetup", '{"Enable":"On"}')
_bd("set_wifi_dhcp", {"mode": "dhcp"}, "POST", "/WifiSetup", '{"DhcpEnable":"dhcp"}')
_bd("set_wifi_ip_address", {"address": "192.168.100.101"}, "POST", "/WifiSetup", '{"IpAddr":"192.168.100.101"}')
_bd("set_wifi_netmask", {"address": "255.255.255.0"}, "POST", "/WifiSetup", '{"NetMask":"255.255.255.0"}')
_bd("set_wifi_gateway", {"address": "192.168.1.1"}, "POST", "/WifiSetup", '{"GateWayIp":"192.168.1.1"}')
_bd("set_wifi_dns", {"address": "9.9.9.9"}, "POST", "/WifiSetup", '{"Dns":"9.9.9.9"}')
_bd("get_ndi_discovery_server", {}, "GET", "/NDIDisServer",
    http_reply={"status": 200, "body": '{"NDIDisServ":"NDIDisServDis","NDIDisServIP":"192.168.0.21"}'},
    expect_result=_val({"NDIDisServ": "NDIDisServDis", "NDIDisServIP": "192.168.0.21"}))
_bd("set_ndi_discovery_server", {"state": "NDIDisServEn"}, "POST", "/NDIDisServer", '{"NDIDisServ":"NDIDisServEn"}')
_bd("set_ndi_discovery_server_ip", {"address": "192.168.1.222"}, "POST", "/NDIDisServer", '{"NDIDisServIP":"192.168.1.222"}')
_bd("get_network_setup", {}, "GET", "/NetworkSetup")
_bd("set_preferred_nic", {"nic": "Ethernet"}, "POST", "/NetworkSetup", '{"PreferredNIC":"Ethernet"}')
_bd("set_dante", {"state": "On"}, "POST", "/NetworkSetup", '{"Dante":"On"}')

# Encoder (2.0 NDIENCODE; 2.1 Encoder Settings)
_bd("get_encode_setup", {}, "GET", "/encodesetup")
_bd("set_encode_setup", {"body": {"ChNum": "1", "BandwidthMode": "NDIManaged", "BandwidthSelect": "120"}},
    "POST", "/encodesetup", '{"ChNum":"1","BandwidthMode":"NDIManaged","BandwidthSelect":"120"}', model="p200-a4a5")
_bd("set_video_format", {"format": "1080p60"}, "POST", "/encodesetup", '{"VideoFormat":"1080p60"}')
_bd("set_video_compression", {"codec": "H265"}, "POST", "/encodesetup", '{"VideoCompression":"H265"}')
_bd("set_stream_protocol", {"protocol": "NDI|HX_UVC"}, "POST", "/encodesetup", '{"StreamProtocol":"NDI|HX_UVC"}')
_bd("set_stream_name", {"name": 'Cam "Left"'}, "POST", "/encodesetup", '{"StreamName":"Cam \\"Left\\""}')
_bd("set_stream_to_network", {"state": "On"}, "POST", "/encodesetup", '{"StreamToNetwork":"On"}', model="maki-ultra")
_bd("set_screensaver_mode", {"mode": "CaptureSS"}, "POST", "/encodesetup", '{"ScreenSaverMode":"CaptureSS"}')
_bd("set_ndi_group", {"state": "NDIGroupDis"}, "POST", "/encodesetup", '{"NDIGroup":"NDIGroupDis"}')
_bd("set_ndi_group_name", {"name": "BirdDog"}, "POST", "/encodesetup", '{"NDIGroupName":"BirdDog"}')
_bd("set_ndi_audio", {"source": "NDIAudioAnalog"}, "POST", "/encodesetup", '{"NDIAudio":"NDIAudioAnalog"}')
_bd("set_ndi_audio_mute", {"state": "Mute"}, "POST", "/encodesetup", '{"NDIAudio":"Mute"}')
_bd("set_video_sample_rate_p400", {"rate": "422"}, "POST", "/encodesetup", '{"VideoSampleRate":"422"}')
_bd("set_bandwidth_mode", {"mode": "Manual"}, "POST", "/encodesetup", '{"BandwidthMode":"Manual"}')
_bd("set_bandwidth", {"mbps": 120}, "POST", "/encodesetup", '{"BandwidthSelect":"120"}')
_bd("set_bandwidth_p400", {"mbps": 360}, "POST", "/encodesetup", '{"BandwidthSelect":"360"}')
_bd("set_tally_mode", {"mode": "TallyOn"}, "POST", "/encodesetup", '{"TallyMode":"TallyOn"}')

# Silicon2 and secondary protocol
_bd("get_sil2_codec", {}, "GET", "/sil2codec",
    http_reply={"status": 200, "body": '{"BitrateControl":"cbr","ModeSel":"high","GOPSize":"60","Bitrate":"31"}'},
    expect_result=_val({"BitrateControl": "cbr", "ModeSel": "high", "GOPSize": "60", "Bitrate": "31"}))
_bd("set_sil2_codec", {"body": {"GOPSize": "60"}}, "POST", "/sil2codec", '{"GOPSize":"60"}')
_bd("set_bitrate_control", {"mode": "cbr"}, "POST", "/sil2codec", '{"BitrateControl":"cbr"}')
_bd("set_encoder_mode", {"mode": "custom"}, "POST", "/sil2codec", '{"ModeSel":"custom"}')
_bd("set_encoder_mode_preset", {"mode": "h265_1080p59.94"}, "POST", "/sil2codec", '{"ModeSel":"h265_1080p59.94"}',
    model="x4-ultra")
_bd("set_encoder_mode_hx3", {"mode": "hx3_264_2160p60"}, "POST", "/sil2codec", '{"ModeSel":"hx3_264_2160p60"}')
_bd("set_gop_size", {"frames": 60}, "POST", "/sil2codec", '{"GOPSize":"60"}')
_bd("set_bitrate", {"mbps": 31}, "POST", "/sil2codec", '{"Bitrate":"31"}')
_bd("get_sil2_encode", {}, "GET", "/sil2enc")
_bd("set_sil2_encode", {"body": {"StreamingProtocol": "SRT", "SRT": {"Port": "7900", "latency": "120", "mode": "listener"}}},
    "POST", "/sil2enc", '{"StreamingProtocol":"SRT","SRT":{"Port":"7900","latency":"120","mode":"listener"}}')
_bd("get_secondary_protocol", {}, "GET", "/secondary_protocol")
_bd("set_secondary_protocol", {"protocol": "disable"}, "POST", "/secondary_protocol", '{"protocol":"disable"}')
_bd("set_srt_mode", {"mode": "listener"}, "POST", "/secondary_protocol", '{"SRT":{"connection_type":"listener"}}')
_bd("set_srt_port", {"port": 9000}, "POST", "/secondary_protocol", '{"SRT":{"port":"9000"}}')
_bd("set_srt_latency", {"ms": 222}, "POST", "/secondary_protocol", '{"SRT":{"latency_ms":"222"}}')
_bd("set_srt_encryption", {"state": "enable"}, "POST", "/secondary_protocol", '{"SRT":{"encryption":"enable"}}')
_bd("set_srt_encryption_length", {"length": "AES-256"}, "POST", "/secondary_protocol", '{"SRT":{"encryption_length":"AES-256"}}')
_bd("set_srt_passphrase", {"passphrase": "abc123def456"}, "POST", "/secondary_protocol", '{"SRT":{"passphrase":"abc123def456"}}')
_bd("set_rtsp_stream_name", {"name": "live/av0"}, "POST", "/secondary_protocol", '{"RTSP":{"stream_name":"live/av0"}}')
_bd("set_rtsp_port", {"port": 222}, "POST", "/secondary_protocol", '{"RTSP":{"port":"222"}}')
_bd("set_rtsp_authentication", {"state": "disable"}, "POST", "/secondary_protocol", '{"RTSP":{"authentication":"disable"}}')
_bd("set_rtsp_user_name", {"name": "admin"}, "POST", "/secondary_protocol", '{"RTSP":{"user_name":"admin"}}')
_bd("set_rtsp_password", {"password": "birddog"}, "POST", "/secondary_protocol", '{"RTSP":{"password":"birddog"}}')
_bd("set_rtmp_server_selection", {"server": "remote"}, "POST", "/secondary_protocol", '{"RTMP":{"server_selection":"remote"}}')
_bd("set_rtmp_server_url", {"url": "rtmp://192.168.1.100:8899/live"}, "POST", "/secondary_protocol",
    '{"RTMP":{"server_url":"rtmp://192.168.1.100:8899/live"}}')
_bd("set_rtmp_stream_key", {"key": "huyalive"}, "POST", "/secondary_protocol", '{"RTMP":{"stream_key":"huyalive"}}')
_bd("set_rtmp_authentication", {"state": "enable"}, "POST", "/secondary_protocol", '{"RTMP":{"authentication":"enable"}}')

# Audio
_bd("get_analog_audio", {}, "GET", "/analogaudiosetup",
    http_reply={"status": 200, "body": '{"AnalogAudioInGain":"80","AnalogAudioOutGain":"80","AnalogAudiooutputselect":"DecodeMain"}'},
    expect_result=_val({"AnalogAudioInGain": "80", "AnalogAudioOutGain": "80", "AnalogAudiooutputselect": "DecodeMain"}))
_bd("set_audio_in_gain", {"gain": 50}, "POST", "/analogaudiosetup", '{"AnalogAudioInGain":"50"}')
_bd("set_audio_in_gain_x4u", {"gain": 55}, "POST", "/analogaudiosetup", '{"AnalogAudioInGain":"55"}')
_bd("set_audio_in_gain_x5u", {"gain": "-6 dB"}, "POST", "/analogaudiosetup", '{"AnalogAudioInGain":"-6 dB"}')
_bd("set_audio_out_gain", {"gain": 50}, "POST", "/analogaudiosetup", '{"AnalogAudioOutGain":"50"}')
_bd("set_audio_output_select", {"source": "DecodeLoop"}, "POST", "/analogaudiosetup", '{"AnalogAudiooutputselect":"DecodeLoop"}')
_bd("set_audio_input_type", {"level": "Line"}, "POST", "/analogaudiosetup", '{"AnalogAudiooutputselect":"Line"}')

# NDI transport, genlock and failover
_bd("get_encode_transport", {}, "GET", "/encodeTransport")
_bd("set_ndi_transmit_method", {"method": "RUDP"}, "POST", "/encodeTransport", '{"Txpm":"RUDP"}',
    http_reply={"status": 200, "body": '{"Txmcttl":"1","Txnetmask":"255.255.0.0","Txnetprefix":"239.255.0.0","Txpm":"RUDP","rxpm":"RUDP"}'},
    expect_result=_OK)
_bd("set_ndi_transmit_method_p", {"method": "Multi-TCP"}, "POST", "/encodeTransport", '{"Txpm":"Multi-TCP"}')
_bd("set_ndi_multicast_prefix", {"address": "239.255.0.0"}, "POST", "/encodeTransport", '{"Txnetprefix":"239.255.0.0"}')
_bd("set_ndi_multicast_netmask", {"mask": "255.255.0.0"}, "POST", "/encodeTransport", '{"Txnetmask":"255.255.0.0"}')
_bd("set_ndi_multicast_ttl", {"ttl": 5}, "POST", "/encodeTransport", '{"Txmcttl":"5"}')
_bd("get_decode_transport", {}, "GET", "/decodeTransport")
_bd("set_ndi_receive_method", {"method": "TCP"}, "POST", "/decodeTransport", '{"rxpm":"TCP"}')
_bd("get_genlock_failover", {}, "GET", "/Genlock_Failover_Source")
_bd("set_genlock_source", {"source": "BIRDDOG-046CE (CAM)"}, "POST", "/Genlock_Failover_Source",
    '{"GenlockSource":"BIRDDOG-046CE (CAM)"}')
_bd("set_failover_source", {"source": "None"}, "POST", "/Genlock_Failover_Source", '{"FailoverSource":"None"}')

# Decoder
_bd("get_decode_setup", {}, "GET", "/decodesetup?ChNum=1",
    http_reply={"status": 200, "body": '{"ColorSpace":"RGB","NDIAudio":"NDIAudioAnalog","ScreenSaverMode":"BirdDogSS","TallyMode":"TallyOff"}'},
    expect_result=_val({"ColorSpace": "RGB", "NDIAudio": "NDIAudioAnalog", "ScreenSaverMode": "BirdDogSS", "TallyMode": "TallyOff"}))
_bd("set_decode_audio", {"state": "NDIAudioEn"}, "POST", "/decodesetup?ChNum=1", '{"NDIAudio":"NDIAudioEn"}')
_bd("set_decode_screensaver", {"mode": "BirdDogSS"}, "POST", "/decodesetup?ChNum=1", '{"ScreenSaverMode":"BirdDogSS"}')
_bd("get_decode_status", {}, "GET", "/decodestatus?ChNum=1",
    http_reply={"status": 200, "body": '{"decoder":"disabled"}'}, expect_result=_val({"decoder": "disabled"}))
_bd("get_connected_source", {"location": "decoder"}, "GET", "/connectTo?location=decoder",
    http_reply={"status": 500, "body": '{"error":"Setting cannot be changed while in Encode mode"}'},
    expect_result=_err(500))
_bd("connect_source", {"location": "decoder", "hostname": "BirdDog-12345", "stream": "CAM"}, "POST",
    "/connectTo?location=decoder", '{"sourceHostname":"BirdDog-12345","sourceStreamName":"CAM"}')
_bd("capture_screensaver", {}, "GET", "/capture?ChNum=1&status=Encode",
    http_reply={"status": 200, "body": "Capture Success.."}, expect_result=_OK)

# Exposure (2.0 EXPOSURE; 2.1 Exposure Setup)
_BD_E = "/birddogexpsetup"
_bd("get_exposure", {}, "GET", _BD_E,
    http_reply={"status": 200, "body": '{"ExpMode":"FULL-AUTO","ShutterSpeed":"8","IrisLevel":"13","ExpCompLvl":"0"}'},
    expect_result=_val({"ExpMode": "FULL-AUTO", "ShutterSpeed": "8", "IrisLevel": "13", "ExpCompLvl": "0"}))
_bd("set_exposure", {"body": {"AeResponse": "1", "BackLight": "Off", "ExpMode": "FULL-AUTO"}}, "POST", _BD_E,
    '{"AeResponse":"1","BackLight":"Off","ExpMode":"FULL-AUTO"}')
_bd("set_exposure_mode", {"mode": "FULL-AUTO"}, "POST", _BD_E, '{"ExpMode":"FULL-AUTO"}',
    http_reply={"status": 200, "body": '{"ExpMode":"FULL-AUTO","ShutterSpeed":"8","IrisLevel":"13"}'}, expect_result=_OK)
_bd("set_exposure_comp", {"state": "On"}, "POST", _BD_E, '{"ExpCompEn":"On"}')
_bd("set_exposure_comp_level", {"level": -1}, "POST", _BD_E, '{"ExpCompLvl":"-1"}')
_bd("set_exposure_comp_level_p", {"level": 14}, "POST", _BD_E, '{"ExpCompLvl":"14"}')
_bd("set_exposure_comp_x5u", {"state": "Off"}, "POST", _BD_E, '{"ExposureComp":"Off"}')
_bd("set_gain_level", {"level": 14}, "POST", _BD_E, '{"GainLevel":"14"}')
_bd("set_gain_level_x4u", {"level": 2}, "POST", _BD_E, '{"GainLevel":"2"}')
_bd("set_gain_level_p", {"level": 4}, "POST", _BD_E, '{"GainLevel":"4"}')
_bd("set_gain_x5u", {"level": 3}, "POST", _BD_E, '{"Gain":"3"}')
_bd("set_gain_limit", {"limit": 3}, "POST", _BD_E, '{"GainLimit":"3"}')
_bd("set_gain_limit_p", {"limit": 11}, "POST", _BD_E, '{"GainLimit":"11"}')
_bd("set_gain_limit_x5u", {"db": "27"}, "POST", _BD_E, '{"GainLimit":"27"}')
_bd("set_shutter_speed", {"index": 8}, "POST", _BD_E, '{"ShutterSpeed":"8"}')
_bd("set_shutter_speed_x4u", {"index": 16}, "POST", _BD_E, '{"ShutterSpeed":"16"}')
_bd("set_shutter_speed_p", {"index": 33}, "POST", _BD_E, '{"ShutterSpeed":"33"}', model="p400")
_bd("set_shutter_speed_x5u", {"speed": "1/60"}, "POST", _BD_E, '{"ShutterSpeed":"1/60"}')
_bd("set_iris_level", {"level": 13}, "POST", _BD_E, '{"IrisLevel":"13"}')
_bd("set_iris_level_x4u", {"level": "17"}, "POST", _BD_E, '{"IrisLevel":"17"}')
_bd("set_iris_level_p200", {"level": "5"}, "POST", _BD_E, '{"IrisLevel":"5"}')
_bd("set_iris_level_p400", {"level": "21"}, "POST", _BD_E, '{"IrisLevel":"21"}')
_bd("set_iris_level_x5u", {"level": "CLOSED"}, "POST", _BD_E, '{"IrisLevel":"CLOSED"}')
_bd("set_bright_level", {"level": 17}, "POST", _BD_E, '{"BrightLevel":"17"}')
_bd("set_bright_level_x5u", {"level": 31}, "POST", _BD_E, '{"BrightLevel":"31"}')
_bd("set_bright_level_p200", {"level": "24"}, "POST", _BD_E, '{"BrightLevel":"24"}')
_bd("set_bright_level_p100", {"level": 27}, "POST", _BD_E, '{"BrightLevel":"27"}')
_bd("set_bright_level_p400", {"level": "41"}, "POST", _BD_E, '{"BrightLevel":"41"}')
_bd("set_ae_response", {"level": 48}, "POST", _BD_E, '{"AeResponse":"48"}')
_bd("set_slow_shutter", {"state": "Off"}, "POST", _BD_E, '{"SlowShutterEn":"Off"}')
_bd("set_slow_shutter_x5u", {"state": "On"}, "POST", _BD_E, '{"SlowShutter":"On"}')
_bd("set_slow_shutter_limit", {"limit": 6}, "POST", _BD_E, '{"SlowShutterLimit":"6"}')
_bd("set_slow_shutter_limit_p400", {"limit": 13}, "POST", _BD_E, '{"SlowShutterLimit":"13"}')
_bd("set_shutter_control_overwrite", {"state": "On"}, "POST", _BD_E, '{"ShutterControlOverwrite":"On"}')
_bd("set_shutter_speed_overwrite", {"speed": 30}, "POST", _BD_E, '{"ShutterSpeedOverwrite":"30"}')
_bd("set_shutter_max_speed", {"index": 29}, "POST", _BD_E, '{"ShutterMaxSpeed":"29"}')
_bd("set_shutter_min_speed", {"index": 16}, "POST", _BD_E, '{"ShutterMinSpeed":"16"}')
_bd("set_gain_point", {"state": "Off"}, "POST", _BD_E, '{"GainPoint":"Off"}')
_bd("set_gain_point_position", {"position": 10}, "POST", _BD_E, '{"GainPointPosition":"10"}')
_bd("set_high_sensitivity", {"state": "Off"}, "POST", _BD_E, '{"HighSensitivity":"Off"}')
_bd("set_spotlight", {"state": "Off"}, "POST", _BD_E, '{"Spotlight":"Off"}')
_bd("set_backlight", {"state": "On"}, "POST", _BD_E, '{"BackLight":"On"}')
_bd("set_backlight_x5u", {"state": "On"}, "POST", _BD_E, '{"Backlight":"On"}')
_bd("set_meter", {"mode": 2}, "POST", _BD_E, '{"Meter":"2"}')

# Picture (2.0 PICTURESETTINGS; 2.1 Picture Setup)
_BD_P = "/birddogpicsetup"
_bd("get_picture", {}, "GET", _BD_P)
_bd("set_picture", {"body": {"Saturation": "4", "Flip": "Off", "Hue": "7"}}, "POST", _BD_P,
    '{"Saturation":"4","Flip":"Off","Hue":"7"}')
_bd("set_flip", {"state": "On"}, "POST", _BD_P, '{"Flip":"On"}', model="maki-ultra",
    http_reply={"status": 200, "body": '{"Saturation":"8","Flip":"On","Hue":"7","Mirror":"Off"}'}, expect_result=_OK)
_bd("set_mirror", {"state": "Off"}, "POST", _BD_P, '{"Mirror":"Off"}')
_bd("set_sharpness", {"level": 50}, "POST", _BD_P, '{"Sharpness":"50"}')
_bd("set_sharpness_x4u", {"level": 11}, "POST", _BD_P, '{"Sharpness":"11"}')
_bd("set_sharpness_x5u", {"level": 3}, "POST", _BD_P, '{"Sharpness":"3"}')
_bd("set_sharpness_maki", {"level": 4}, "POST", _BD_P, '{"Sharpness":"4"}')
_bd("set_sharpness_p200", {"level": -128}, "POST", _BD_P, '{"Sharpness":"-128"}')
_bd("set_sharpness_p100", {"level": 15}, "POST", _BD_P, '{"Sharpness":"15"}')
_bd("set_saturation", {"level": 99}, "POST", _BD_P, '{"Saturation":"99"}')
_bd("set_saturation_x4u", {"level": 4}, "POST", _BD_P, '{"Saturation":"4"}')
_bd("set_saturation_x5u", {"level": 10}, "POST", _BD_P, '{"Saturation":"10"}')
_bd("set_color_p100", {"level": 8}, "POST", _BD_P, '{"Color":"8"}')
_bd("set_hue", {"level": 50}, "POST", _BD_P, '{"Hue":"50"}')
_bd("set_hue_x4u", {"level": 7}, "POST", _BD_P, '{"Hue":"7"}')
_bd("set_hue_x5u", {"level": 15}, "POST", _BD_P, '{"Hue":"15"}')
_bd("set_contrast", {"level": 0}, "POST", _BD_P, '{"Contrast":"0"}')
_bd("set_contrast_x4u", {"level": 7}, "POST", _BD_P, '{"Contrast":"7"}')
_bd("set_contrast_x5u", {"level": 4}, "POST", _BD_P, '{"Contrast":"4"}')
_bd("set_contrast_p100", {"level": 1}, "POST", _BD_P, '{"Contrast":"1"}')
_bd("set_brightness", {"level": 99}, "POST", _BD_P, '{"Brightness":"99"}')
_bd("set_brightness_x4u", {"level": 7}, "POST", _BD_P, '{"Brightness":"7"}')
_bd("set_3dnr", {"level": 30}, "POST", _BD_P, '{"ThreeDNR":"30"}')
_bd("set_3dnr_x4u", {"level": 9}, "POST", _BD_P, '{"ThreeDNR":"9"}')
_bd("set_3dnr_x5u", {"level": 0}, "POST", _BD_P, '{"ThreeDNR":"0"}')
_bd("set_3dnr_p400", {"level": "Off"}, "POST", _BD_P, '{"ThreeDNR":"Off"}')
_bd("set_2dnr", {"level": 30}, "POST", _BD_P, '{"TWODNR":"30"}')
_bd("set_2dnr_maki", {"level": 6}, "POST", _BD_P, '{"TwoDNR":"6"}')
_bd("set_2dnr_p400", {"level": "2"}, "POST", _BD_P, '{"TWODNR":"2"}')
_bd("set_backlight_comp", {"state": "Off"}, "POST", _BD_P, '{"BackLightCom":"Off"}')
_bd("set_wdr", {"state": "On"}, "POST", _BD_P, '{"WDREnable":"On"}')
_bd("set_wide_dynamic_range_maki", {"level": 8}, "POST", _BD_P, '{"WideDynamicRange":"8"}')
_bd("set_wide_dynamic_range_p100", {"level": "Off"}, "POST", _BD_P, '{"WideDynamicRange":"Off"}')
_bd("set_highlight_comp", {"state": "On"}, "POST", _BD_P, '{"HighlightComp":"On"}')
_bd("set_highlight_comp_p200", {"level": "MEDIUM"}, "POST", _BD_P, '{"HighlightComp":"MEDIUM"}')
_bd("set_highlight_comp_mask", {"level": 15}, "POST", _BD_P, '{"HighlightCompMask":"15"}')
_bd("set_highlight_comp_mask_p200a2", {"level": 3}, "POST", _BD_P, '{"HighlightCompMask":"3"}')
_bd("set_gamma", {"gamma": "0.45"}, "POST", _BD_P, '{"Gamma":"0.45"}')
_bd("set_gamma_x4u", {"gamma": "PC"}, "POST", _BD_P, '{"Gamma":"PC"}')
_bd("set_gamma_maki", {"gamma": "ext"}, "POST", _BD_P, '{"Gamma":"ext"}')
_bd("set_gamma_p200", {"gamma": 1}, "POST", _BD_P, '{"Gamma":"1"}')
_bd("set_gamma_p100", {"gamma": 4}, "POST", _BD_P, '{"Gamma":"4"}')
_bd("set_deflicker", {"mode": "50Hz"}, "POST", _BD_P, '{"DeFlicker":"50Hz"}')
_bd("set_bw_mode", {"state": "Off"}, "POST", _BD_P, '{"BWMode":"Off"}')
_bd("set_stabilizer", {"state": "Off"}, "POST", _BD_P, '{"Stabilizer":"Off"}')
_bd("set_stabilizer_x5u", {"state": "On"}, "POST", _BD_P, '{"Stablizer":"On"}')
_bd("set_chroma_suppress", {"level": "OFF"}, "POST", _BD_P, '{"ChromeSuppress":"OFF"}')
_bd("set_ir_cut_filter", {"mode": "Auto"}, "POST", _BD_P, '{"IRCutFilter":"Auto"}')
_bd("set_ir_cut_filter_p400", {"mode": "Off"}, "POST", _BD_P, '{"IRCutFilter":"Off"}')
_bd("set_effect", {"effect": "BW"}, "POST", _BD_P, '{"Effect":"BW"}')
_bd("set_effect_p200a2", {"effect": "B&W"}, "POST", _BD_P, '{"Effect":"B&W"}')
_bd("set_noise_reduction", {"level": "Off"}, "POST", _BD_P, '{"NoiseReduction":"Off"}')
_bd("set_low_latency", {"state": "Off"}, "POST", _BD_P, '{"LowLatency":"Off"}')
_bd("set_nd_filter", {"level": 2}, "POST", _BD_P, '{"NDFilter":"2"}')

# White balance (2.0 WHITEBALANCE; 2.1 White Balance Setup)
_BD_W = "/birddogwbsetup"
_bd("get_white_balance", {}, "GET", _BD_W, model="x5-ultra",
    http_reply={"status": 200, "body": '{"BlueGain":"66","RedGain":"74","WbMode":"OUTDOOR"}'},
    expect_result=_val({"BlueGain": "66", "RedGain": "74", "WbMode": "OUTDOOR"}))
_bd("set_white_balance", {"body": {"RedGain": "111", "BlueGain": "111"}}, "POST", _BD_W, '{"RedGain":"111","BlueGain":"111"}')
_bd("set_wb_mode", {"mode": "ColorTemp"}, "POST", _BD_W, '{"WbMode":"ColorTemp"}')
_bd("set_wb_mode_x5u", {"mode": "USER"}, "POST", _BD_W, '{"WbMode":"USER"}')
_bd("set_wb_mode_p200", {"mode": "SVL-OUTDOOR-AUTO"}, "POST", _BD_W, '{"WbMode":"SVL-OUTDOOR-AUTO"}')
_bd("set_wb_mode_p100", {"mode": "MANUAL2"}, "POST", _BD_W, '{"WbMode":"MANUAL2"}')
_bd("set_wb_mode_p400", {"mode": "OUTDOOR AUTO"}, "POST", _BD_W, '{"WbMode":"OUTDOOR AUTO"}')
_bd("set_red_gain", {"level": 179}, "POST", _BD_W, '{"RedGain":"179"}')
_bd("set_blue_gain", {"level": 174}, "POST", _BD_W, '{"BlueGain":"174"}')
_bd("set_color_temp", {"kelvin": 5600}, "POST", _BD_W, '{"ColorTemp":"5600"}')
_bd("set_color_temp_maki", {"index": 31}, "POST", _BD_W, '{"ColorTemp":"31"}')
_bd("set_color_temp_p100", {"kelvin": 2800}, "POST", _BD_W, '{"ColorTemp":"2800"}')
_bd("wb_one_push", {}, "POST", _BD_W, '{"OnePushTrigger":"Take"}',
    http_reply={"status": 200, "body": '{"WbMode":"ONEPUSH","OnePushTrigger":"Take","RedGain":"128","BlueGain":"128","ColorTemp":"5600"}'},
    expect_result=_OK)
_bd("set_red_tuning", {"level": -10}, "POST", _BD_W, '{"RTuning":"-10"}')
_bd("set_blue_tuning", {"level": 10}, "POST", _BD_W, '{"BTuning":"10"}')
_bd("set_red_tuning_maki", {"level": 0}, "POST", _BD_W, '{"RTuning":"0"}')
_bd("set_blue_tuning_maki", {"level": 245}, "POST", _BD_W, '{"BTuning":"245"}')
_bd("set_wb_sensitivity", {"level": 1}, "POST", _BD_W, '{"WBSensitivity":"1"}')
_bd("set_wb_matrix_coefficient", {"coefficient": "RG", "value": -99}, "POST", _BD_W, '{"RG":"-99"}')
_bd("set_wb_matrix", {"state": "Off"}, "POST", _BD_W, '{"Matrix":"Off"}')
_bd("set_wb_level", {"level": 4}, "POST", _BD_W, '{"Level":"4"}')
_bd("set_wb_offset", {"level": 7}, "POST", _BD_W, '{"Offset":"7"}')
_bd("set_wb_phase", {"level": 7}, "POST", _BD_W, '{"Phase":"7"}')
_bd("set_wb_select", {"select": "FL LIGHT"}, "POST", _BD_W, '{"Select":"FL LIGHT"}')
_bd("set_wb_speed", {"speed": 3}, "POST", _BD_W, '{"Speed":"3"}')

# Colour matrix (2.0 COLOURMATRIX; 2.1 Colour Matrix Setup)
_BD_CM = "/birddogcmsetup"
_bd("get_colour_matrix", {}, "GET", _BD_CM)
_bd("set_colour_matrix", {"body": {"BlueHue": "35", "RedHue": "35"}}, "POST", _BD_CM, '{"BlueHue":"35","RedHue":"35"}')
_bd("set_colour_matrix_hue", {"colour": "Red", "value": 44}, "POST", _BD_CM, '{"RedHue":"44"}',
    http_reply={"status": 200, "body": '{"RedHue":"44","BlueHue":"50","GreenHue":"50","YellowHue":"50","MagHue":"50","CyanHue":"50"}'},
    expect_result=_OK)
_bd("set_colour_matrix_hue_64", {"colour": "Mag", "value": 64}, "POST", _BD_CM, '{"MagHue":"64"}')
_bd("set_colour_matrix_gain_64", {"colour": "Yellow", "value": 32}, "POST", _BD_CM, '{"YellowGain":"32"}')
_bd("set_colour_gain", {"level": 128}, "POST", _BD_CM, '{"ColourGain":"128"}')
_bd("set_hue_phase", {"level": 255}, "POST", _BD_CM, '{"HuePhase":"255"}')

# Advanced (2.0 ADVANCEDSETTINGS; 2.1 Advanced Setup)
_BD_AD = "/birddogadvancesetup"
_bd("get_advanced", {}, "GET", _BD_AD)
_bd("set_advanced", {"body": {"AFZone": "ALL"}}, "POST", _BD_AD, '{"AFZone":"ALL"}')
_bd("set_af_zone", {"zone": "ALL"}, "POST", _BD_AD, '{"AFZone":"ALL"}',
    http_reply={"status": 200, "body": '{"AFMode":"Auto","Scene":"Normal","AFZone":"ALL","AFSensitivity":"Middle"}'},
    expect_result=_OK)
_bd("set_af_zone_x4u", {"zone": "default"}, "POST", _BD_AD, '{"AFZone":"default"}')
_bd("set_af_zone_maki", {"zone": "All"}, "POST", _BD_AD, '{"AFZone":"All"}')
_bd("set_scene", {"scene": "Macro"}, "POST", _BD_AD, '{"Scene":"Macro"}')
_bd("set_scene_x4u", {"scene": "PC"}, "POST", _BD_AD, '{"Scene":"PC"}')
_bd("set_scene_maki", {"scene": "Clarity"}, "POST", _BD_AD, '{"Scene":"Clarity"}')
_bd("set_af_sensitivity", {"level": "Middle"}, "POST", _BD_AD, '{"AFSensitivity":"Middle"}')
_bd("set_af_mode", {"mode": "MANUAL"}, "POST", _BD_AD, '{"AFMode":"MANUAL"}')
_bd("set_teleconvert", {"state": "On"}, "POST", _BD_AD, '{"teleconvert_mode":"On"}')
_bd("set_image_ratio", {"ratio": "9:16"}, "POST", _BD_AD, '{"img_ratio":"9:16"}')
_bd("set_eptz", {"state": "Off"}, "POST", _BD_AD, '{"eptz_switch":"Off"}')
_bd("set_gamma_offset", {"offset": 16}, "POST", _BD_AD, '{"GammaOffset":"16"}')
_bd("set_high_resolution", {"state": "Off"}, "POST", _BD_AD, '{"HighResolution":"Off"}')
_bd("set_brightness_comp", {"level": "VERY DARK"}, "POST", _BD_AD, '{"BrightnessComp":"VERY DARK"}')
_bd("set_comp_level", {"level": "LOW"}, "POST", _BD_AD, '{"CompLevel":"LOW"}')
_bd("set_brightness_p200", {"level": 2}, "POST", _BD_AD, '{"Brightness":"2"}')
_bd("set_video_enhancement", {"state": "Off"}, "POST", _BD_AD, '{"VideoEnhancement":"Off"}')

# External (2.0 EXTERNALSETTINGS)
_BD_EX = "/birddogexternalsetup"
_bd("get_external", {}, "GET", _BD_EX,
    http_reply={"status": 200, "body": '{"Aux":"Off","RainWiper":"Off","V12vOut":"Off"}'},
    expect_result=_val({"Aux": "Off", "RainWiper": "Off", "V12vOut": "Off"}))
_bd("set_external", {"body": {"Aux": "Off", "RainWiper": "Off", "V12vOut": "Off"}}, "POST", _BD_EX,
    '{"Aux":"Off","RainWiper":"Off","V12vOut":"Off"}', model="a300-gen1")
_bd("set_aux", {"state": "On"}, "POST", _BD_EX, '{"Aux":"On"}')
_bd("set_v12_out", {"state": "On"}, "POST", _BD_EX, '{"V12vOut":"On"}')
_bd("set_rain_wiper", {"state": "On"}, "POST", _BD_EX, '{"RainWiper":"On"}')
_bd("set_defog", {"state": "On"}, "POST", _BD_EX, '{"DeFog":"On"}')

# Detail (2.0 DETAIL)
_BD_DT = "/birddogdetsetup"
_bd("get_detail", {}, "GET", _BD_DT)
_bd("set_detail", {"body": {"Bandwidth": "DEFAULT", "Detail": "On", "Level": "3"}}, "POST", _BD_DT,
    '{"Bandwidth":"DEFAULT","Detail":"On","Level":"3"}')
_bd("set_detail_enabled", {"state": "On"}, "POST", _BD_DT, '{"Detail":"On"}')
_bd("set_detail_level", {"level": 3}, "POST", _BD_DT, '{"Level":"3"}')
_bd("set_detail_bandwidth", {"bandwidth": "WIDE"}, "POST", _BD_DT, '{"Bandwidth":"WIDE"}')
_bd("set_detail_bw_balance", {"balance": "TYPE1"}, "POST", _BD_DT, '{"BwBalance":"TYPE1"}')
_bd("set_detail_crispening", {"level": 7}, "POST", _BD_DT, '{"Crispening":"7"}')
_bd("set_detail_highlight", {"level": 0}, "POST", _BD_DT, '{"HighLightDetail":"0"}')
_bd("set_detail_hv_balance", {"level": -2}, "POST", _BD_DT, '{"HvBalance":"-2"}')
_bd("set_detail_limit", {"level": 3}, "POST", _BD_DT, '{"Limit":"3"}')
_bd("set_detail_super_low", {"level": 0}, "POST", _BD_DT, '{"SuperLow":"0"}')

# Gamma (2.0 GAMMA)
_BD_GM = "/birddoggammasetup"
_bd("get_gamma", {}, "GET", _BD_GM)
_bd("set_gamma_settings", {"body": {"Settings": "PATTERN", "Pattern": "51", "PatternFine": "2"}}, "POST", _BD_GM,
    '{"Settings":"PATTERN","Pattern":"51","PatternFine":"2"}')
_bd("set_gamma_mode", {"mode": "STANDARD"}, "POST", _BD_GM, '{"Settings":"STANDARD"}')
_bd("set_gamma_level", {"level": 7}, "POST", _BD_GM, '{"Level":"7"}')
_bd("set_black_gamma_level", {"level": 7}, "POST", _BD_GM, '{"BlackGammaLevel":"7"}')
_bd("set_black_level", {"level": 96}, "POST", _BD_GM, '{"BlackLevel":"96"}')
_bd("set_black_level_range", {"range": "LOW"}, "POST", _BD_GM, '{"BlackLevelRange":"LOW"}')
_bd("set_gamma_effect", {"level": -3}, "POST", _BD_GM, '{"Effect":"-3"}')
_bd("set_gamma_curve_offset", {"offset": -64}, "POST", _BD_GM, '{"Offset":"-64"}')
_bd("set_gamma_pattern", {"pattern": 512}, "POST", _BD_GM, '{"Pattern":"512"}')
_bd("set_gamma_pattern_fine", {"level": 2}, "POST", _BD_GM, '{"PatternFine":"2"}')
_bd("set_visibility_enhancer", {"state": "Off"}, "POST", _BD_GM, '{"VisibilityEnhancer":"Off"}')

# PTZ setup (2.0 PTZ; 2.1 PTZ Setup)
_BD_PS = "/birddogptzsetup"
_bd("get_ptz_setup", {}, "GET", _BD_PS)
_bd("set_ptz_setup", {"body": {"PanSpeed": "8", "TiltSpeed": "8", "ZoomSpeed": "4"}}, "POST", _BD_PS,
    '{"PanSpeed":"8","TiltSpeed":"8","ZoomSpeed":"4"}', model="p100")
_bd("set_pan_speed", {"speed": 255}, "POST", _BD_PS, '{"PanSpeed":"255"}')
_bd("set_pan_speed_x4u", {"speed": 24}, "POST", _BD_PS, '{"PanSpeed":"24"}')
_bd("set_pan_speed_x5u", {"speed": 100}, "POST", _BD_PS, '{"PanSpeed":"100"}')
_bd("set_pan_speed_p", {"speed": 8}, "POST", _BD_PS, '{"PanSpeed":"8"}')
_bd("set_tilt_speed", {"speed": 20}, "POST", _BD_PS, '{"TiltSpeed":"20"}')
_bd("set_tilt_speed_x4u", {"speed": 20}, "POST", _BD_PS, '{"TiltSpeed":"20"}')
_bd("set_tilt_speed_x5u", {"speed": 0}, "POST", _BD_PS, '{"TiltSpeed":"0"}')
_bd("set_tilt_speed_p", {"speed": 18}, "POST", _BD_PS, '{"TiltSpeed":"18"}')
_bd("set_zoom_speed", {"speed": 7}, "POST", _BD_PS, '{"ZoomSpeed":"7"}')
_bd("set_zoom_speed_pf120", {"speed": 8}, "POST", _BD_PS, '{"ZoomSpeed":"8"}')
_bd("set_focus_mode", {"mode": "MANUAL"}, "POST", _BD_PS, '{"FocusMode":"MANUAL"}')
_bd("set_preset_mode", {"mode": "PTZonly"}, "POST", _BD_PS, '{"Preset":"PTZonly"}')
_bd("set_preset_mode_x4u", {"mode": "Camera"}, "POST", _BD_PS, '{"Preset":"Camera"}')
_bd("set_preset_speed", {"speed": 100}, "POST", _BD_PS, '{"PresetSpeed":"100"}',
    http_reply={"status": 200, "body": '{"FocusMode":"AUTO","PanSpeed":"24","Preset":"Camera","PresetSpeed":"14"}'},
    expect_result=_OK)
_bd("set_preset_speed_maki", {"speed": 0}, "POST", _BD_PS, '{"PresetSpeed":"0"}')
_bd("set_preset_speed_x5u", {"speed": "150"}, "POST", _BD_PS, '{"PresetSpeed":"150"}')
_bd("set_speed_control", {"mode": "superfine"}, "POST", _BD_PS, '{"SpeedControl":"superfine"}')
_bd("set_pan_tilt_slow", {"state": "On"}, "POST", _BD_PS, '{"PanTiltSlow":"On"}')
_bd("set_digital_zoom_limit", {"limit": "x16"}, "POST", _BD_PS, '{"DZoomLimit":"x16"}')

# Presets (the documents' example bodies)
_bd("recall_preset", {"preset": 1}, "POST", "/recall", '{"Preset":"Preset-1"}',
    http_reply={"status": 200, "body": '{"Preset":"Preset-1"}'}, expect_result=_OK)
_bd("save_preset", {"preset": 9}, "POST", "/save", '{"Preset":"Preset-9"}', model="p200-a2a3",
    http_reply={"status": 500, "body": ""}, expect_result=_err(500))

# PTZ movement (2.1 PTZ Movement Controls)
_BD_PC = "/birddogptzcontrol"
_bd("get_ptz_control", {}, "GET", _BD_PC,
    http_reply={"status": 200, "body": '{"FocusMode":"Auto","Menu":"Off","X-Axis":"9323","Y-Axis":"338","Z-Axis":"16384"}'},
    expect_result=_val({"FocusMode": "Auto", "Menu": "Off", "X-Axis": "9323", "Y-Axis": "338", "Z-Axis": "16384"}))
_bd("pan_tilt", {"direction": "upleft"}, "POST", _BD_PC, '{"PanTilt":"upleft"}')
_bd("zoom", {"direction": "tele"}, "POST", _BD_PC, '{"Zoom":"tele"}', model="maki-ultra")
_bd("focus", {"direction": "stop"}, "POST", _BD_PC, '{"Focus":"stop"}')
_bd("set_menu", {"state": "Off"}, "POST", _BD_PC, '{"Menu":"Off"}')
_bd("set_pan_tilt_position", {"pan": 444, "tilt": 444}, "POST", _BD_PC, '{"X-Axis":"444","Y-Axis":"444"}',
    http_reply={"status": 200, "body": '{"status":"Invalid Command"}'}, expect_result=_err())
_bd("set_pan_tilt_position_x4u", {"pan": -2448, "tilt": 1296}, "POST", _BD_PC, '{"X-Axis":"-2448","Y-Axis":"1296"}')
_bd("set_pan_tilt_position_x5u", {"pan": -170, "tilt": 90}, "POST", _BD_PC, '{"X-Axis":"-170","Y-Axis":"90"}')
_bd("set_zoom_position", {"position": 16384}, "POST", _BD_PC, '{"Z-Axis":"16384"}')
_bd("set_zoom_position_x4u", {"position": 4912}, "POST", _BD_PC, '{"Z-Axis":"4912"}')
_bd("set_zoom_position_x5u", {"position": 100}, "POST", _BD_PC, '{"Z-Axis":"100"}')

# NDI finder (2.0 NDIFINDER; 2.1 NDI Finder Settings)
_bd("get_ndi_group_names", {}, "GET", "/NDIGrpName",
    http_reply={"status": 200, "body": '"BirdDog-Co1","BirdDog-Co2"'}, expect_result=_val('"BirdDog-Co1","BirdDog-Co2"'))
_bd("set_ndi_group_names", {"groups": "Your_Group, My_Group"}, "POST", "/NDIGrpName", "Your_Group, My_Group",
    http_reply={"status": 200, "body": "Invalid API command"}, expect_result=_err())
_bd("get_ndi_off_subnet", {}, "GET", "/NDIOffSnSrc")
_bd("set_ndi_off_subnet", {"addresses": "192.168.100.100,192.168.100.101"}, "POST", "/NDIOffSnSrc",
    "192.168.100.100,192.168.100.101")
_bd("ndi_finder_reset", {}, "GET", "/reset", http_reply={"status": 200, "body": '{"Status":"success"}'}, expect_result=_OK)
_bd("ndi_finder_refresh", {}, "GET", "/refresh")
_bd("get_ndi_sources", {}, "GET", "/List",
    http_reply={"status": 200, "body": '{"BIRDDOG-4C526 (Stream1)":"192.168.100.100:5961"}'},
    expect_result=_val({"BIRDDOG-4C526 (Stream1)": "192.168.100.100:5961"}))

# Tally (2.1)
_bd("get_tally", {}, "GET", "/tally")
_bd("set_tally", {"state": "On"}, "POST", "/tally", '{"tally_state":"On"}',
    http_reply={"status": 200, "body": '{"tally_rest_state":"Off","tally_state":"On"}'}, expect_result=_OK)
_bd("set_tally_rest_state", {"state": "white"}, "POST", "/tally", '{"tally_rest_state":"white"}')


# Telemetry: the documents' example replies.
def _tel(name, path, body, state):
    telemetry(BD, name, inbound_http={"path": path, "body": body}, expect_state=state)


_tel("about", "/about",
     '{"FallbackIP":"192.168.100.100","FirmwareVersion":"BirdDog X1 Ultra 5.6.057","Format":"CAM HX","GateWay":"192.168.23.1",'
     '"HardwareVersion":"BirdDog X1 Ultra","HostName":"birddog-608F9","IPAddress":"192.168.23.75","MCUVersion":"5",'
     '"NetworkConfigMethod":"static","NetworkMask":"255.255.255.0","SerialNumber":"8027b62608f9","Status":"Active",'
     '"WifiConfigMethod":"static","WifiIPAddress":"192.168.100.101","WifiMask":"255.255.255.0","Dns":""}',
     {"device": {"firmware": "BirdDog X1 Ultra 5.6.057", "hardware": "BirdDog X1 Ultra", "hostname": "birddog-608F9",
                 "ip_address": "192.168.23.75", "netmask": "255.255.255.0", "gateway": "192.168.23.1",
                 "network_method": "static", "serial": "8027b62608f9", "status": "Active", "stream_name": "CAM HX",
                 "fallback_ip": "192.168.100.100", "mcu_version": "5", "dns": ""}})
_tel("encodesetup", "/encodesetup",
     '{"ChNum":"1","VideoFormat":"1080p59.94","VideoSampleRate":"420","StreamName":"CAM","NDIAudio":"NDIAudioAnalog",'
     '"ScreenSaverMode":"CaptureSS","BandwidthMode":"NDIManaged","BandwidthSelect":"120","TallyMode":"TallyOn",'
     '"NDIGroup":"NDIGroupDis","NDIGroupName":"BirdDog","VideoCompression":"H265","StreamProtocol":"NDI|HX_UVC",'
     '"StreamToNetwork":"On"}',
     {"encoder": {"video_format": "1080p59.94", "compression": "H265", "stream_protocol": "NDI|HX_UVC",
                  "stream_name": "CAM", "stream_to_network": True, "screensaver": "CaptureSS", "ndi_group": False,
                  "ndi_group_name": "BirdDog", "ndi_audio": "NDIAudioAnalog", "tally_mode": "TallyOn",
                  "bandwidth_mode": "NDIManaged", "bandwidth": 120, "video_sample_rate": "420"}})
_tel("exposure", "/birddogexpsetup",
     '{"ExpMode":"IRIS-PRI","ExpCompLvl":"-1","GainLevel":"2","GainLimit":"3","ExpCompEn":"1","BackLight":"Off",'
     '"IrisLevel":"17","ShutterSpeed":"4","BrightLevel":"8","SmartExposure":"Off","Meter":"0","DRC":"2"}',
     {"exposure": {"mode": "IRIS-PRI", "compensation": True, "compensation_level": -1, "gain": 2, "gain_limit": 3,
                   "shutter": "4", "iris": "17", "bright": 8, "backlight": False, "meter": 0}})
_tel("exposure-x5", "/birddogexpsetup",
     '{"ExpMode":"MANUAL","SlowShutter":"On","GainLimit":"30","HighSensitivity":"Off","Backlight":"On",'
     '"ExposureComp":"Off","Gain":"11","IrisLevel":"CLOSED","ShutterSpeed":"1/60","BrightLevel":"31"}',
     {"exposure": {"mode": "MANUAL", "slow_shutter_x5": True, "gain_limit": 30, "high_sensitivity": False,
                   "backlight_x5": True, "compensation_x5": False, "gain_x5": 11, "iris": "CLOSED",
                   "shutter": "1/60", "bright": 31}})
_tel("exposure-p", "/birddogexpsetup",
     '{"AeResponse":"1","BackLight":"Off","BrightLevel":"24","ExpCompEn":"Off","ExpCompLvl":"-123","ExpMode":"FULL-AUTO",'
     '"GainLevel":"4","GainLimit":"11","GainPoint":"Off","GainPointPosition":"10","HighSensitivity":"Off",'
     '"IrisLevel":"21","ShutterControlOverwrite":"On","ShutterMaxSpeed":"29","ShutterMinSpeed":"16",'
     '"ShutterSpeed":"16","ShutterSpeedOverwrite":"30","SlowShutterEn":"Off","SlowShutterLimit":"13","Spotlight":"Off"}',
     {"exposure": {"mode": "FULL-AUTO", "compensation": False, "compensation_level": -123, "gain": 4, "gain_limit": 11,
                   "shutter": "16", "iris": "21", "bright": 24, "backlight": False, "high_sensitivity": False,
                   "slow_shutter": False, "ae_response": 1, "gain_point": False, "gain_point_position": 10,
                   "shutter_control_overwrite": True, "shutter_speed_overwrite": 30, "shutter_max_speed": 29,
                   "shutter_min_speed": 16, "slow_shutter_limit": 13, "spotlight": False}})
_tel("picture", "/birddogpicsetup",
     '{"Flip":"Off","Mirror":"Off","Effect":"Off","WDREnable":"Off","WideDynamicRange":"0","Gamma":"default",'
     '"BackLightCom":"Off","DeFlicker":"50Hz","HighlightComp":"Off","Portrait":"Off","Brightness":"50","Color":"50",'
     '"Saturation":"50","Hue":"50","Contrast":"50","Sharpness":"50","TWODNR":"30","ThreeDNR":"30"}',
     {"picture": {"flip": False, "mirror": False, "effect": "Off", "wdr": False, "gamma": "default",
                  "backlight_comp": False, "deflicker": "50Hz", "highlight_comp": "Off", "brightness": 50, "color": 50,
                  "saturation": 50, "hue": 50, "contrast": 50, "sharpness": 50, "noise_reduction_2d": "30",
                  "noise_reduction_3d": "30", "wide_dynamic_range": "0"}})
_tel("picture-maki", "/birddogpicsetup",
     '{"Saturation":"8","Flip":"On","Hue":"7","Mirror":"Off","TwoDNR":"6","ThreeDNR":"9","Contrast":"7",'
     '"Gamma":"default","Sharpness":"4","WideDynamicRange":"0","BackLightCom":"Off","Brightness":"7",'
     '"DeFlicker":"default","BWMode":"0","WDREnable":"On","HighlightComp":"Off","Stablizer":"None"}',
     {"picture": {"saturation": 8, "flip": True, "hue": 7, "mirror": False, "noise_reduction_2d_maki": "6",
                  "noise_reduction_3d": "9", "contrast": 7, "gamma": "default", "sharpness": 4,
                  "backlight_comp": False, "brightness": 7, "deflicker": "default", "bw_mode": False, "wdr": True,
                  "highlight_comp": "Off", "wide_dynamic_range": "0"}})
_tel("white-balance", "/birddogwbsetup",
     '{"WbMode":"AUTO","OnePushTrigger":"None","RedGain":"128","BlueGain":"128","WBSensitivity":"Middle",'
     '"ColorTemp":"5600","GTuning":"50","RTuning":"50","BTuning":"50"}',
     {"white_balance": {"mode": "AUTO", "red_gain": 128, "blue_gain": 128, "color_temp": 5600, "red_tuning": 50,
                        "blue_tuning": 50, "sensitivity": "Middle"}})
_tel("ptz-setup", "/birddogptzsetup",
     '{"FocusMode":"AUTO","Freed":"Off","FreedIpAddr":"192.168.2.100","FreedPort":"5555","PanSpeed":"24",'
     '"Preset":"Camera","PresetSpeed":"14","SpeedControl":"standard","TiltSpeed":"20","ZoomSpeed":"7"}',
     {"ptz": {"focus_mode": "AUTO", "pan_speed": 24, "preset_mode": "Camera", "preset_speed": 14,
              "speed_control": "standard", "tilt_speed": 20, "zoom_speed": 7}})
_tel("ptz-control", "/birddogptzcontrol",
     '{"FocusMode":"Auto","Menu":"Off","X-Axis":"9323","Y-Axis":"338","Z-Axis":"16384"}',
     {"position": {"pan": 9323, "tilt": 338, "zoom": 16384}, "ptz": {"menu": False}})
_tel("tally", "/tally", '{"tally_rest_state":"white","tally_state":"Off"}',
     {"tally": {"on": False, "rest_state": "white"}})
_tel("encode-transport", "/encodeTransport",
     '{"Txmcttl":"1","Txnetmask":"255.255.0.0","Txnetprefix":"239.255.0.0","Txpm":"RUDP","rxpm":"RUDP"}',
     {"ndi": {"transmit_method": "RUDP", "multicast_prefix": "239.255.0.0", "multicast_netmask": "255.255.0.0",
              "multicast_ttl": 1, "receive_method": "RUDP"}})
_tel("decode-transport", "/decodeTransport", '{"rxpm":"TCP"}', {"ndi": {"receive_method": "TCP"}})
_tel("discovery-server", "/NDIDisServer", '{"NDIDisServ":"NDIDisServEn","NDIDisServIP":"192.168.1.100"}',
     {"ndi": {"discovery_server": True, "discovery_server_ip": "192.168.1.100"}})
_tel("analog-audio", "/analogaudiosetup",
     '{"AnalogAudioInGain":"80","AnalogAudioOutGain":"80","AnalogAudiooutputselect":"DecodeMain"}',
     {"audio": {"in_gain": "80", "out_gain": 80, "select": "DecodeMain"}})
_tel("decode-status", "/decodestatus?ChNum=1",
     '{"AudioChannel":"0","AudioSampleRate":"0","ConnectionStatus":"Connected","SourceIP":"None","SourceName":"None",'
     '"StreamBandwidth":"31","VideoFPS":"29.97","VideoFramerate":"29.97","VideoResolution":"2160p"}',
     {"decoder": {"audio_channels": 0, "audio_sample_rate": "0", "connection": "Connected", "source_ip": "None",
                  "source_name": "None", "bandwidth": 31, "video_frame_rate": "29.97", "video_resolution": "2160p"}})
_tel("video-output", "/videooutputinterface",
     '{"Dante":"Off","OutputFormat":"1080p60","OutputMode":"Encode","PreferredNIC":"Ethernet","PrivacyMode":"Off",'
     '"mode":"encode","videooutput":"hdmi"}',
     {"video_output": {"format": "1080p60", "privacy_mode": False, "interface": "hdmi"}})
_tel("video-output-x5", "/videooutputinterface", '{"Frequency":"50","HDMI-1OutputDecoder":"Off","OutputFormat":"1920x1080p50"}',
     {"video_output": {"frequency": "50", "hdmi1_decoder": False, "format": "1920x1080p50"}})
_tel("sil2-codec", "/sil2codec", '{"BitrateControl":"cbr","ModeSel":"high","GOPSize":"60","Bitrate":"31"}',
     {"encoder": {"bitrate_control": "cbr", "mode": "high", "gop_size": 60, "bitrate": 31}})
_tel("secondary-protocol", "/secondary_protocol",
     '{"RTMP":{"authentication":"disable","server_selection":"remote"},"RTSP":{"port":"554"},'
     '"SRT":{"connection_type":"listener","latency_ms":"120"},"protocol":"disable"}',
     {"encoder": {"secondary_protocol": "disable",
                  "secondary": {"rtmp": {"authentication": False, "server_selection": "remote"},
                                "rtsp": {"port": 554}, "srt": {"mode": "listener", "latency": 120}}}})

# Full control: every other endpoint the cameras read, from the documents'
# example replies (2.0 Example data, 2.1 example responses).
_tel("about-maki", "/about",
     '{"FallbackIP":"192.168.100.100","FirmwareVersion":"5.6.123","Format":"CAM","GateWay":"192.168.1.1",'
     '"HardwareVersion":"MAKI Ultra","HostName":"MAKI-12X-04E60","IPAddress":"192.168.1.20",'
     '"NetworkConfigMethod":"dhcp","NetworkMask":"255.255.255.0","SerialNumber":"0123456789","Status":"Active",'
     '"DNS":"9.9.9.9"}',
     {"device": {"firmware": "5.6.123", "hardware": "MAKI Ultra", "hostname": "MAKI-12X-04E60",
                 "ip_address": "192.168.1.20", "netmask": "255.255.255.0", "gateway": "192.168.1.1",
                 "network_method": "dhcp", "serial": "0123456789", "status": "Active", "stream_name": "CAM",
                 "fallback_ip": "192.168.100.100", "dns": "9.9.9.9"}})
_tel("picture-p", "/birddogpicsetup",
     '{"BackLightCom":"On","ChromeSuppress":"OFF","Color":"8","Contrast":"1","Effect":"BW","Flip":"Off","Gamma":"1",'
     '"HighlightComp":"On","HighlightCompMask":"3","Hue":"7","IRCutFilter":"Off","Mirror":"Off","NoiseReduction":"Off",'
     '"Sharpness":"122","Stabilizer":"Off","TWODNR":"2","ThreeDNR":"2","WideDynamicRange":"Off","LowLatency":"Off",'
     '"NDFilter":"2"}',
     {"picture": {"backlight_comp": True, "chroma_suppress": "OFF", "color": 8, "contrast": 1, "effect": "BW",
                  "flip": False, "gamma": "1", "highlight_comp": "On", "highlight_comp_mask": 3, "hue": 7,
                  "ir_cut_filter": "Off", "mirror": False, "noise_reduction": "Off", "sharpness": 122,
                  "stabilizer": False, "noise_reduction_2d": "2", "noise_reduction_3d": "2",
                  "wide_dynamic_range": "Off", "low_latency": False, "nd_filter": 2}})
_tel("picture-x5", "/birddogpicsetup", '{"Flip":"Off","Mirror":"On","Stablizer":"On"}',
     {"picture": {"flip": False, "mirror": True, "stabilizer_x5": True}})
_tel("white-balance-p", "/birddogwbsetup",
     '{"BG":"0","BR":"-2","BlueGain":"174","ColorTemp":"2800","GB":"0","GR":"3","Level":"4","Matrix":"Off",'
     '"Offset":"7","Phase":"7","RB":"0","RG":"1","RedGain":"179","Select":"OFF","Speed":"3","WbMode":"AUTO"}',
     {"white_balance": {"blue_gain": 174, "color_temp": 2800, "level": 4, "matrix": False, "offset": 7, "phase": 7,
                        "red_gain": 179, "select": "OFF", "speed": 3, "mode": "AUTO",
                        "matrix_coefficients": {"bg": 0, "br": -2, "gb": 0, "gr": 3, "rb": 0, "rg": 1}}})
_tel("ptz-setup-x5", "/birddogptzsetup", '{"PanSpeed":"50","TiltSpeed":"50","ZoomSpeed":"4","PanTiltSlow":"On"}',
     {"ptz": {"pan_speed": 50, "tilt_speed": 50, "zoom_speed": 4, "pan_tilt_slow": True}})
_tel("ptz-setup-maki", "/birddogptzsetup",
     '{"FocusMode":"AUTO","ZoomSpeed":"5","PresetSpeed":"100","Preset":"Camera","DZoomLimit":"x4"}',
     {"ptz": {"focus_mode": "AUTO", "zoom_speed": 5, "preset_speed": 100, "preset_mode": "Camera",
              "digital_zoom_limit": "x4"}})
_tel("colour-matrix", "/birddogcmsetup",
     '{"BlueGain":"32","BlueHue":"30","ColourGain":"128","CyanGain":"33","CyanHue":"32","GreenGain":"34",'
     '"GreenHue":"32","HuePhase":"120","MagGain":"35","MagHue":"31","RedGain":"36","RedHue":"29","YellowGain":"37",'
     '"YellowHue":"28"}',
     {"colour_matrix": {"blue": {"gain": 32, "hue": 30}, "cyan": {"gain": 33, "hue": 32},
                        "green": {"gain": 34, "hue": 32}, "magenta": {"gain": 35, "hue": 31},
                        "red": {"gain": 36, "hue": 29}, "yellow": {"gain": 37, "hue": 28},
                        "colour_gain": 128, "hue_phase": 120}})
_tel("advanced", "/birddogadvancesetup", '{"AFMode":"AUTO","Scene":"Normal","AFZone":"Center","AFSensitivity":"High"}',
     {"advanced": {"af_mode": "AUTO", "scene": "Normal", "af_zone": "Center", "af_sensitivity": "High"}})
_tel("advanced-p", "/birddogadvancesetup",
     '{"Brightness":"2","BrightnessComp":"STANDARD","CompLevel":"LOW","GammaOffset":"16","HighResolution":"Off",'
     '"VideoEnhancement":"On"}',
     {"advanced": {"brightness": 2, "brightness_comp": "STANDARD", "comp_level": "LOW", "gamma_offset": 16,
                   "high_resolution": False, "video_enhancement": True}})
_tel("advanced-maki", "/birddogadvancesetup",
     '{"Scene":"Clarity","AFMode":"MANUAL","AFZone":"All","NearLimit":"1m","AFSensitivity":"Low","SmartFocus":"Off",'
     '"teleconvert_mode":"On","img_ratio":"9:16","eptz_switch":"Off"}',
     {"advanced": {"scene": "Clarity", "af_mode": "MANUAL", "af_zone": "All", "af_sensitivity": "Low",
                   "teleconvert": True, "image_ratio": "9:16", "eptz": False}})
_tel("external", "/birddogexternalsetup", '{"Aux":"Off","RainWiper":"On","V12vOut":"Off","DeFog":"On"}',
     {"external": {"aux": False, "rain_wiper": True, "v12_out": False, "defog": True}})
_tel("detail", "/birddogdetsetup",
     '{"Bandwidth":"DEFAULT","BwBalance":"TYPE1","Crispening":"0","Detail":"On","HighLightDetail":"0","HvBalance":"-2",'
     '"Level":"3","Limit":"3","SuperLow":"0"}',
     {"detail": {"bandwidth": "DEFAULT", "bw_balance": "TYPE1", "crispening": 0, "enabled": True, "highlight": 0,
                 "hv_balance": -2, "level": 3, "limit": 3, "super_low": 0}})
_tel("gamma", "/birddoggammasetup",
     '{"BlackGammaLevel":"7","BlackLevel":"0","BlackLevelRange":"LOW","Effect":"0","Level":"7","Offset":"0",'
     '"Pattern":"51","PatternFine":"2","Settings":"STANDARD","VisibilityEnhancer":"Off"}',
     {"gamma": {"black_gamma_level": 7, "black_level": 0, "black_level_range": "LOW", "effect": 0, "level": 7,
                "offset": 0, "pattern": 51, "pattern_fine": 2, "mode": "STANDARD", "visibility_enhancer": False}})
_SIL2_PRESETS = ('"high":{"Bitrate":"10","GOPSize":"30","QuantFactorI":"30","QuantFactorP":"30"},'
                 '"low":{"Bitrate":"3","GOPSize":"60","QuantFactorI":"30","QuantFactorP":"30"}')
_tel("sil2-codec-2-0", "/sil2codec",
     '{' + ','.join(
         f'"{p}":{{"BitrateControl":"cbr","Custom":{{"Bitrate":"{b}","GOPSize":"{g}","QuantFactorI":"30",'
         f'"QuantFactorP":"31"}},"ModeSel":"{m}",{_SIL2_PRESETS}}}'
         for p, b, g, m in [("DISABLE", 3, 60, "Custom"), ("HX", 3, 60, "high"), ("RTMP", 3, 59, "Custom"),
                            ("RTSP", 4, 59, "Custom"), ("SRT", 5, 59, "low")]) + '}',
     {"encoder": {"sil2": {
         k: {"bitrate_control": "cbr", "mode": m, "custom_bitrate": b, "custom_gop_size": g, "custom_quant_i": 30,
             "custom_quant_p": 31}
         for k, b, g, m in [("disable", 3, 60, "Custom"), ("hx", 3, 60, "high"), ("rtmp", 3, 59, "Custom"),
                            ("rtsp", 4, 59, "Custom"), ("srt", 5, 59, "low")]}}})
_tel("sil2-encode", "/sil2enc",
     '{"HaiVisionPlayerSupport":"true","RTMP":{"AuthEnable":"0","ConnectionURL":"rtmp://192.168.2.197:1935/bdlive/birddogkey",'
     '"Password":"testpwd","Server":"TestUrl","ServerSelection":"local","StreamKeyLocal":"birddogkey",'
     '"StreamKeyRemote":"remotekey","UserName":"testuser"},"RTSP":{"AuthEnable":"1",'
     '"ConnectionURL":"rtsp://192.168.2.197:3489/birddog-stream","Password":"","Port":"3489",'
     '"StreamName":"birddog-stream","UserName":""},"SRT":{"ConnectionURL":"srt://192.168.2.197:7900?mode=listener&latency=120",'
     '"Encryption":"false","IPAddress":"","Port":"7900","latency":"120","mode":"caller","passphrase":"",'
     '"pbkeylen":"32","streamid":"21"},"StreamingProtocol":"SRT"}',
     {"encoder": {"sil2_stream": {
         "protocol": "SRT", "haivision_player": True,
         "rtmp": {"authentication": False, "connection_url": "rtmp://192.168.2.197:1935/bdlive/birddogkey",
                  "server": "TestUrl", "server_selection": "local", "user_name": "testuser"},
         "rtsp": {"authentication": True, "connection_url": "rtsp://192.168.2.197:3489/birddog-stream", "port": 3489,
                  "stream_name": "birddog-stream", "user_name": ""},
         "srt": {"connection_url": "srt://192.168.2.197:7900?mode=listener&latency=120", "encryption": False,
                 "ip_address": "", "port": 7900, "latency": 120, "mode": "caller", "key_length": 32,
                 "stream_id": "21"}}}})
_tel("secondary-protocol-full", "/secondary_protocol",
     '{"RTMP":{"authentication":"disable","connection_url":"rtmp://192.168.1.100:8899/live/huyalive",'
     '"server_selection":"remote","server_url":"rtmp://192.168.1.100:8899/live","stream_key":""},'
     '"RTSP":{"authentication":"enable","connection_url":"rtsp://192.168.20.46:554/live/av0","password":"birddog",'
     '"port":"554","stream_name":"live/av0","user_name":"admin"},"SRT":{"connection_type":"listener",'
     '"connection_url":"srt://192.168.20.46:9000?mode=caller&latency=120","encryption":"disable",'
     '"encryption_length":"AES-128","latency_ms":"120","passphrase":"","port":"9000"},"protocol":"SRT"}',
     {"encoder": {"secondary_protocol": "SRT", "secondary": {
         "rtmp": {"authentication": False, "connection_url": "rtmp://192.168.1.100:8899/live/huyalive",
                  "server_selection": "remote", "server_url": "rtmp://192.168.1.100:8899/live"},
         "rtsp": {"authentication": True, "connection_url": "rtsp://192.168.20.46:554/live/av0", "port": 554,
                  "stream_name": "live/av0", "user_name": "admin"},
         "srt": {"mode": "listener", "connection_url": "srt://192.168.20.46:9000?mode=caller&latency=120",
                 "encryption": False, "encryption_length": "AES-128", "latency": 120, "port": 9000}}}})
_tel("ethernet", "/EthSetup",
     '{"IpMode":"dhcp","IpAddr":"192.168.20.46","NetMask":"255.255.255.0","GateWayIp":"192.168.20.20",'
     '"DnsSvrIpFirst":"9.9.9.9","DnsSvrIpSecond":"1.1.1.1","DhcpTimeout":"30","DhcpFallbackIPAddr":"192.168.100.100",'
     '"DhcpFallbackNetMask":"255.255.255.0","DhcpFallbackGateWayIp":"192.168.1.1"}',
     {"network": {"ethernet": {"ip_mode": "dhcp", "ip_address": "192.168.20.46", "netmask": "255.255.255.0",
                               "gateway": "192.168.20.20", "dns_primary": "9.9.9.9", "dns_secondary": "1.1.1.1",
                               "dhcp_timeout": 30, "fallback_ip": "192.168.100.100",
                               "fallback_netmask": "255.255.255.0", "fallback_gateway": "192.168.1.1"}}})
_tel("wifi", "/WifiSetup",
     '{"Enable":"1","DhcpEnable":"1","EncryptMode":"None","Ssid":"","IpAddr":"192.168.100.101",'
     '"NetMask":"255.255.255.0","GateWayIp":"192.168.1.1","Dns":"9.9.9.9"}',
     {"network": {"wifi": {"enabled": True, "dhcp": True, "encryption": "None", "ssid": "",
                           "ip_address": "192.168.100.101", "netmask": "255.255.255.0", "gateway": "192.168.1.1",
                           "dns": "9.9.9.9"}}})
_tel("network-setup", "/NetworkSetup", '{"PreferredNIC":"Ethernet","Dante":"Off"}',
     {"network": {"preferred_nic": "Ethernet", "dante": False}})
_tel("genlock-failover", "/Genlock_Failover_Source", '{"GenlockSource":"BIRDDOG-046CE (CAM)","FailoverSource":"None"}',
     {"ndi": {"genlock_source": "BIRDDOG-046CE (CAM)", "failover_source": "None"}})
_tel("decode-setup", "/decodesetup?ChNum=1",
     '{"ColorSpace":"RGB","NDIAudio":"NDIAudioAnalog","ScreenSaverMode":"BirdDogSS","TallyMode":"TallyOff"}',
     {"decoder": {"color_space": "RGB", "ndi_audio": "NDIAudioAnalog", "screensaver": "BirdDogSS",
                  "tally_mode": "TallyOff"}})
_tel("connect-to", "/connectTo?location=decoder",
     '{"sourceHostname":"BirdDog-12345","sourceStreamName":"CAM","sourceIP":"192.168.100.61","sourcePort":"5961"}',
     {"connections": {"decoder": {"hostname": "BirdDog-12345", "stream_name": "CAM", "ip_address": "192.168.100.61",
                                  "port": 5961}}})
_tel("operation-mode", "/operationmode", "dual", {"device": {"operation_mode": "dual"}})
_tel("video-output-p400", "/videooutputinterface", "NormalMode", {"video_output": {"mode": "NormalMode"}})

# Re-reads: a POST with a JSON body has its endpoint read again; its GET
# reply does not.
telemetry(BD, "reread-exposure", inbound_http={
    "path": "/birddogexpsetup", "request": {"GainLimit": "11"},
    "body": '{"ExpMode":"MANUAL","GainLimit":"10"}'},
    expect_then_send=[{"method": "GET", "target": "/birddogexpsetup"}],
    expect_state={"exposure": {"mode": "MANUAL", "gain_limit": 10}})
telemetry(BD, "reread-exposure-get", inbound_http={
    "path": "/birddogexpsetup", "body": '{"ExpMode":"MANUAL"}'},
    expect_then_send=[], expect_state={"exposure": {"mode": "MANUAL"}})
telemetry(BD, "reread-secondary", inbound_http={
    "path": "/secondary_protocol", "request": {"SRT": {"latency_ms": "222"}}, "body": '{"protocol":"SRT"}'},
    expect_then_send=[{"method": "GET", "target": "/secondary_protocol"}],
    expect_state={"encoder": {"secondary_protocol": "SRT"}})
telemetry(BD, "reread-tally", inbound_http={
    "path": "/tally", "request": {"tally_state": "On"}, "body": '{"tally_rest_state":"Off","tally_state":"On"}'},
    expect_then_send=[{"method": "GET", "target": "/tally"}],
    expect_state={"tally": {"on": True, "rest_state": "Off"}})
telemetry(BD, "reread-decode-setup", inbound_http={
    "path": "/decodesetup?ChNum=1", "request": {"NDIAudio": "NDIAudioEn"}, "body": '{"NDIAudio":"NDIAudioEn"}'},
    expect_then_send=[{"method": "GET", "target": "/decodesetup?ChNum=1"}],
    expect_state={"decoder": {"ndi_audio": "NDIAudioEn"}})
telemetry(BD, "reread-connect-to", inbound_http={
    "path": "/connectTo?location=Genlock", "request": {"sourceHostname": "BirdDog-12345", "sourceStreamName": "CAM"},
    "body": '{"sourceName":"None"}'},
    expect_then_send=[{"method": "GET", "target": "/connectTo?location=Genlock"}], expect_state={})
telemetry(BD, "reread-recall", inbound_http={"path": "/recall", "request": {"Preset": "Preset-1"},
                                             "body": '{"Preset":"Preset-1"}'},
          expect_then_send=[{"method": "GET", "target": "/birddogexpsetup"},
                            {"method": "GET", "target": "/birddogpicsetup"},
                            {"method": "GET", "target": "/birddogwbsetup"},
                            {"method": "GET", "target": "/birddogptzcontrol"},
                            {"method": "GET", "target": "/birddogcmsetup"},
                            {"method": "GET", "target": "/birddogadvancesetup"}],
          expect_state={})
