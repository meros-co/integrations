# BirdDog converters and decoders (birddog-converters): one vector per
# command, over the RESTful API 2.0 on port 8080. Targets and bodies are
# written here from the API 2.0 HTML documentation (P-Series & Converters):
# each section's curl example and parameter table. Bodies are compact JSON
# with every value a JSON string, as in every example; encode and decode
# settings carry ChNum as the examples' bodies do. The connectTo body is the
# Pod User Guide's (p.9). Replies are the documents' example replies.
BC = "birddog-converters"
_OK = {"ok": {"kind": "ack"}}


def _bc(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": BC, "command": command, "input": input, "expect_request": request, **extra})


def _val(v):
    return {"ok": {"kind": "value", "value": v}}


ABOUT = ('{"FirmwareVersion":"BirdDog Firmware version","Format":"CAM 1","HostName":"birddog-device",'
         '"IPAddress":"192.168.100.100","NetworkConfigMethod":"dhcp","NetworkMask":"255.255.255.0",'
         '"SerialNumber":"0123456789","Status":"active"}')
QUAD = {"model": "4k-quad"}
K4 = {"model": "4k-hdmi"}
FOUT = {"model": "flex-4k-out"}

# Basic device information.
_bc("get_about", {}, "GET", "/about", http_reply={"status": 200, "body": ABOUT},
    expect_result=_val({"FirmwareVersion": "BirdDog Firmware version", "Format": "CAM 1", "HostName": "birddog-device",
                        "IPAddress": "192.168.100.100", "NetworkConfigMethod": "dhcp", "NetworkMask": "255.255.255.0",
                        "SerialNumber": "0123456789", "Status": "active"}))
_bc("get_hostname", {}, "GET", "/hostname", http_reply={"status": 200, "body": "birddog-0b710"},
    expect_result=_val("birddog-0b710"))
_bc("get_version", {}, "GET", "/version", http_reply={"status": 200, "body": "BirdDog P200A4_A5"},
    expect_result=_val("BirdDog P200A4_A5"))
_bc("reboot", {}, "POST", "/reboot", http_reply={"status": 200, "body": "{}"}, expect_result=_OK)
_bc("restart_video", {}, "POST", "/restart")

# Device settings.
_bc("get_operation_mode", {}, "GET", "/operationmode", http_reply={"status": 200, "body": "decode"},
    expect_result=_val("decode"))
_bc("set_operation_mode", {"mode": "decode"}, "POST", "/operationmode", body="decode")
_bc("get_video_output", {}, "GET", "/videooutputinterface", http_reply={"status": 200, "body": "hdmi"},
    expect_result=_val("hdmi"), **K4)
_bc("set_video_output", {"output": "sdi"}, "POST", "/videooutputinterface", body="sdi", **K4)
_bc("get_analog_audio", {}, "GET", "/analogaudiosetup")
_bc("set_audio_in_gain", {"gain": 50}, "POST", "/analogaudiosetup", body='{"AnalogAudioInGain":"50"}')
_bc("set_audio_out_gain", {"gain": 50}, "POST", "/analogaudiosetup", body='{"AnalogAudioOutGain":"50"}')
_bc("set_audio_output_select", {"source": "DecodeMain"}, "POST", "/analogaudiosetup",
    body='{"AnalogAudiooutputselect":"DecodeMain"}')

# NDI encode.
_bc("get_encode_setup", {"channel": 2}, "GET", "/encodesetup?ChNum=2", **QUAD)
_bc("set_stream_name", {"name": "CAM"}, "POST", "/encodesetup", body='{"ChNum":"1","StreamName":"CAM"}')
_bc("set_video_format", {"format": "1080p59.94"}, "POST", "/encodesetup", body='{"ChNum":"1","VideoFormat":"1080p59.94"}')
_bc("set_ndi_audio", {"source": "NDIAudioAnalog"}, "POST", "/encodesetup", body='{"ChNum":"1","NDIAudio":"NDIAudioAnalog"}')
_bc("set_screensaver_mode", {"mode": "CaptureSS"}, "POST", "/encodesetup", body='{"ChNum":"1","ScreenSaverMode":"CaptureSS"}')
_bc("set_bandwidth_mode", {"mode": "NDIManaged"}, "POST", "/encodesetup", body='{"ChNum":"1","BandwidthMode":"NDIManaged"}')
_bc("set_bandwidth", {"mbps": 120}, "POST", "/encodesetup", body='{"ChNum":"1","BandwidthSelect":"120"}')
_bc("set_bandwidth_4k", {"channel": 3, "mbps": 360}, "POST", "/encodesetup", body='{"ChNum":"3","BandwidthSelect":"360"}', **QUAD)
_bc("set_tally_mode", {"mode": "TallyOn"}, "POST", "/encodesetup", body='{"ChNum":"1","TallyMode":"TallyOn"}')
_bc("set_loop_tally", {"state": "LoopTallyDis"}, "POST", "/encodesetup", body='{"ChNum":"1","LoopTally":"LoopTallyDis"}')
_bc("set_video_csc", {"csc": "RGB"}, "POST", "/encodesetup", body='{"ChNum":"1","VideoCSC":"RGB"}')
_bc("set_video_sample_rate", {"sampling": "420"}, "POST", "/encodesetup", body='{"ChNum":"1","VideoSampleRate":"420"}', **K4)
_bc("set_color_bit_depth", {"depth": "10Bit"}, "POST", "/encodesetup", body='{"ChNum":"1","ColorBitDepth":"10Bit"}', **K4)
_bc("set_ndi_group", {"state": "NDIGroupDis"}, "POST", "/encodesetup", body='{"ChNum":"1","NDIGroup":"NDIGroupDis"}')
_bc("set_ndi_group_name", {"name": "BirdDog"}, "POST", "/encodesetup", body='{"ChNum":"1","NDIGroupName":"BirdDog"}')
_bc("get_encode_transport", {}, "GET", "/encodeTransport")
_bc("set_ndi_transmit_method", {"method": "Multi-TCP"}, "POST", "/encodeTransport", body='{"Txpm":"Multi-TCP"}')
_bc("set_ndi_multicast_prefix", {"prefix": "239.255.0.0"}, "POST", "/encodeTransport", body='{"Txnetprefix":"239.255.0.0"}')
_bc("set_ndi_multicast_netmask", {"mask": "255.255.0.0"}, "POST", "/encodeTransport", body='{"Txnetmask":"255.255.0.0"}')
_bc("set_ndi_multicast_ttl", {"ttl": 1}, "POST", "/encodeTransport", body='{"Txmcttl":"1"}')
_bc("capture_encode_screensaver", {}, "GET", "/capture?ChNum=1&status=Encode",
    http_reply={"status": 200, "body": "Capture Success.."}, expect_result=_OK)

# NDI decode.
_bc("get_connected_source", {}, "GET", "/connectTo?ChNum=1", http_reply={"status": 200, "body": '{"sourceName":"None"}'},
    expect_result=_val({"sourceName": "None"}))
_bc("connect_source", {"source": "BIRDDOG-0B710 (CAM)"}, "POST", "/connectTo?ChNum=1",
    body='{"sourceName":"BIRDDOG-0B710 (CAM)"}', http_reply={"status": 200, "body": '{"sourceName":"BIRDDOG-0B710 (CAM)"}'},
    expect_result=_OK)
_bc("get_decode_status", {}, "GET", "/decodestatus?ChNum=1")
_bc("get_decode_setup", {}, "GET", "/decodesetup?ChNum=1")
_bc("set_decode_audio", {"state": "NDIAudioEn"}, "POST", "/decodesetup?ChNum=1", body='{"ChNum":"1","NDIAudio":"NDIAudioEn"}')
_bc("set_decode_screensaver", {"mode": "BirdDogSS"}, "POST", "/decodesetup?ChNum=1", body='{"ChNum":"1","ScreenSaverMode":"BirdDogSS"}')
_bc("set_decode_tally_mode", {"mode": "VideoMode"}, "POST", "/decodesetup?ChNum=1", body='{"ChNum":"1","TallyMode":"VideoMode"}', **FOUT)
_bc("set_decode_color_space", {"space": "RGB"}, "POST", "/decodesetup?ChNum=1", body='{"ChNum":"1","ColorSpace":"RGB"}')
_bc("get_decode_transport", {}, "GET", "/decodeTransport")
_bc("set_ndi_receive_method", {"method": "TCP"}, "POST", "/decodeTransport", body='{"rxpm":"TCP"}')
_bc("capture_decode_screensaver", {}, "GET", "/capture?ChNum=1&status=Decode")

# NDI finder.
_bc("get_ndi_sources", {}, "GET", "/List", http_reply={"status": 200, "body": '{"None":"None"}'}, expect_result=_val({"None": "None"}))
_bc("ndi_finder_refresh", {}, "POST", "/refresh")
_bc("ndi_finder_reset", {}, "POST", "/reset")
_bc("get_ndi_discovery_server", {}, "GET", "/NDIDisServer")
_bc("set_ndi_discovery_server", {"state": "NDIDisServEn"}, "POST", "/NDIDisServer", body='{"NDIDisServ":"NDIDisServEn"}')
_bc("set_ndi_discovery_server_ip", {"address": "192.168.1.10"}, "POST", "/NDIDisServer", body='{"NDIDisServIP":"192.168.1.10"}')
_bc("get_ndi_group_names", {}, "GET", "/NDIGrpName", http_reply={"status": 200, "body": "BirdDog"}, expect_result=_val("BirdDog"))
_bc("set_ndi_group_names", {"groups": "BirdDog,Studio"}, "POST", "/NDIGrpName", body="BirdDog,Studio")
_bc("get_ndi_off_subnet", {}, "GET", "/NDIOffSnSrc")
_bc("set_ndi_off_subnet", {"address": "10.0.0.20"}, "POST", "/NDIOffSnSrc", body="10.0.0.20")


# ── Telemetry, from the document's example replies ──
def _tel(name, path, body, state):
    telemetry(BC, name, inbound_http={"path": path, "body": body}, expect_state=state)


_tel("about", "/about", ABOUT, {"device": {"firmware": "BirdDog Firmware version", "format": "CAM 1", "hostname": "birddog-device",
                                           "ip_address": "192.168.100.100", "netmask": "255.255.255.0", "network_method": "dhcp",
                                           "serial": "0123456789", "status": "active"}})
_tel("connect-to", "/connectTo?ChNum=1", '{"sourceName":"BIRDDOG-0B710 (CAM)"}', {"decoder": {"source": "BIRDDOG-0B710 (CAM)"}})
_tel("decode-status", "/decodestatus?ChNum=1",
     '{"Videoresolution":"1920x1080","VideoFramerate":"59.94p","VideoSamplerate":"4:2:2","Audiochannels":"2",'
     '"AudioSamplerate":"48000","AverageBitrate":"120"}',
     {"decoder": {"resolution": "1920x1080", "frame_rate": "59.94p", "sampling": "4:2:2", "audio_channels": 2,
                  "audio_sample_rate": 48000, "bitrate": "120"}})
_tel("decode-setup", "/decodesetup?ChNum=1",
     '{"ColorSpace":"RGB","NDIAudio":"NDIAudioEn","ScreenSaverMode":"BirdDogSS","TallyMode":"VideoMode","ChNum":"1"}',
     {"decoder": {"color_space": "RGB", "audio": "NDIAudioEn", "screensaver": "BirdDogSS", "tally_mode": "VideoMode"}})
_tel("decode-transport", "/decodeTransport", '{"rxpm":"TCP"}', {"decoder": {"receive_method": "TCP"}})
_tel("encode-setup", "/encodesetup?ChNum=1",
     '{"ChNum":"1","VideoFormat":"1080p59.94","VideoSampleRate":"420","ColorBitDepth":"8Bit","StreamName":"CAM",'
     '"NDIAudio":"NDIAudioAnalog","ScreenSaverMode":"CaptureSS","BandwidthMode":"NDIManaged","BandwidthSelect":"120",'
     '"LoopTally":"LoopTallyDis","TallyMode":"TallyOn","VideoCSC":"RGB","NDIGroup":"NDIGroupDis","NDIGroupName":"BirdDog"}',
     {"encoder": {"video_format": "1080p59.94", "sample_rate": "420", "bit_depth": "8Bit", "stream_name": "CAM",
                  "ndi_audio": "NDIAudioAnalog", "screensaver": "CaptureSS", "bandwidth_mode": "NDIManaged", "bandwidth": 120,
                  "loop_tally": False, "tally_mode": "TallyOn", "video_csc": "RGB", "ndi_group": False, "ndi_group_name": "BirdDog"}})
_tel("encode-transport", "/encodeTransport", '{"Txpm":"TCP","Txnetprefix":"239.255.0.0","Txnetmask":"255.255.0.0","Txmcttl":"1"}',
     {"encoder": {"transmit_method": "TCP", "multicast_prefix": "239.255.0.0", "multicast_netmask": "255.255.0.0", "multicast_ttl": 1}})
_tel("analog-audio", "/analogaudiosetup", '{"AnalogAudioInGain":"80","AnalogAudioOutGain":"80","AnalogAudiooutputselect":"DecodeMain"}',
     {"audio": {"in_gain": 80, "out_gain": 80, "output_select": "DecodeMain"}})
_tel("discovery-server", "/NDIDisServer", '{"NDIDisServ":"NDIDisServEn","NDIDisServIP":"192.168.1.10"}',
     {"finder": {"discovery_server": True, "discovery_server_ip": "192.168.1.10"}})
