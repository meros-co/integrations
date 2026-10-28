# Haivision KB Encoder/Transcoder (haivision-kb): the KB REST API of the
# KB 5.9 API Integrator's Reference, /ecs/... with the .json suffix on port
# 1080, bodies as the reference's JSON samples give them (written compact).
# The user name is left empty, so no login goes first; the session itself is
# covered by the engine's session tests. Writes are followed by a read of
# what they changed; only the write is stated.
HKB = "haivision-kb"
_HKB_CH = "5775626faa602b6d251cf1d6"
_HKB_OK = '{"messages":{"count":1,"message":[{"text":"Request processed successfully.","type":"info"}]}}'


def _hkb_j(obj):
    return json.dumps(obj, separators=(",", ":"))


def _hkb(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body if isinstance(body, str) else _hkb_j(body)
    V.append({"spec": HKB, "command": command, "input": input, "expect_request": request, **extra})


for _c, _t in [("get_version", "/ecs/version.json"), ("get_system", "/ecs/system.json"), ("get_time", "/ecs/time.json"),
               ("get_ecs_state", "/ecs.json"), ("get_license", "/ecs/license.json"),
               ("get_license_state", "/ecs/license/state.json"), ("list_channels", "/ecs/channels.json"),
               ("list_channel_configs", "/ecs/channelconfigs.json")]:
    _hkb(_c, {}, "GET", _t)
_hkb("get_version", {}, "GET", "/ecs/version.json", file="get_version-reply",
     http_reply={"status": 200, "body": '{"version":{"build":"406","major":"5","minor":"0","subminor":"0","type":"DEV"}}'},
     expect_result={"ok": {"kind": "value", "value": {"version": {"build": "406", "major": "5", "minor": "0",
                                                                  "subminor": "0", "type": "DEV"}}}})
_hkb("reset_license", {}, "DELETE", "/ecs/license.json")
_hkb("create_channel", {"label": "Channel_1"}, "POST", "/ecs/channels.json",
     {"channel": {"label": "Channel_1", "id": "", "previewonly": "0"}})
_hkb("create_channel", {"label": "Preview", "id": "21EC2020-3AEA-1069-A2DD-08002B30309D", "preview_only": True}, "POST",
     "/ecs/channels.json", {"channel": {"label": "Preview", "id": "21EC2020-3AEA-1069-A2DD-08002B30309D", "previewonly": "1"}},
     file="create_channel-preview")
_hkb("get_channel", {"channel": _HKB_CH}, "GET", f"/ecs/channels/{_HKB_CH}.json")
_hkb("save_channel_preview", {"channel": _HKB_CH, "to": {"file": "preview.jpg"}}, "GET", f"/ecs/channels/{_HKB_CH}/preview.jpg")
_hkb("delete_channel", {"channel": "Channel_1"}, "DELETE", "/ecs/channels/Channel_1.json")
_hkb("start_channel", {"channel": _HKB_CH}, "PUT", f"/ecs/channels/{_HKB_CH}.json",
     {"invoke": {"command": "StartChannel", "param": ""}})
_hkb("start_channel", {"channel": _HKB_CH, "record": True}, "PUT", f"/ecs/channels/{_HKB_CH}.json",
     {"invoke": {"command": "StartChannel", "param": "startrecord"}}, file="start_channel-record")
for _c, _w in [("prepare_stop_channel", "PrepareStop"), ("stop_channel", "StopChannel"),
               ("start_recording", "StartRecord"), ("stop_recording", "StopRecord")]:
    _hkb(_c, {"channel": _HKB_CH}, "PUT", f"/ecs/channels/{_HKB_CH}.json", {"invoke": {"command": _w, "param": ""}})
_HKB_IN = "Device_DeckLink_Baseband_2"
_hkb("set_audio_gain", {"channel": _HKB_CH, "input": _HKB_IN, "gain": -10.0}, "PUT",
     f"/ecs/channels/{_HKB_CH}/inputs/{_HKB_IN}.json", {"invoke": {"command": "setaudiogain", "param": "-10.0"}})
_hkb("increase_audio_gain", {"channel": _HKB_CH, "input": _HKB_IN}, "PUT", f"/ecs/channels/{_HKB_CH}/inputs/{_HKB_IN}.json",
     {"invoke": {"command": "audiogaininc", "param": ""}})
_hkb("decrease_audio_gain", {"channel": _HKB_CH, "input": _HKB_IN}, "PUT", f"/ecs/channels/{_HKB_CH}/inputs/{_HKB_IN}.json",
     {"invoke": {"command": "audiogaindec", "param": ""}})
_hkb("enable_output", {"channel": _HKB_CH, "output": "HLS_Sink_12"}, "PUT", f"/ecs/channels/{_HKB_CH}/outputs/HLS_Sink_12.json",
     {"invoke": {"command": "EnableOutput", "param": ""}})
_hkb("disable_output", {"channel": _HKB_CH, "output": "HLS_Sink_12"}, "PUT", f"/ecs/channels/{_HKB_CH}/outputs/HLS_Sink_12.json",
     {"invoke": {"command": "DisableOutput", "param": ""}})
_hkb("get_signals_after", {"channel": _HKB_CH, "index": -1}, "GET", f"/ecs/channels/{_HKB_CH}/signals.json?index=-1")
_hkb("get_signals_since", {"channel": _HKB_CH, "time": "02:15:55:918"}, "GET",
     f"/ecs/channels/{_HKB_CH}/signals.json?time=02%3A15%3A55%3A918")
_hkb("get_signals_recent", {"channel": _HKB_CH, "seconds": 60}, "GET", f"/ecs/channels/{_HKB_CH}/signals.json?timespan=60")
_HKB_MD = [{"label": "MetadataName1", "type": "int", "value": "12345"},
           {"label": "MetadataName4", "type": "string", "value": "This is how we inject metadata."}]
_hkb("inject_metadata", {"channel": _HKB_CH, "output": "RTMP_Sink_6", "stream": "rtmp", "data": _HKB_MD}, "PUT",
     f"/ecs/channels/{_HKB_CH}/outputs/RTMP_Sink_6/streams/rtmp.json", {"data": _HKB_MD},
     http_reply={"status": 200, "body": _HKB_OK}, expect_result={"ok": {"kind": "ack"}})
_hkb("inject_scte35", {"channel": _HKB_CH, "input": _HKB_IN, "duration": 30.0}, "PUT",
     f"/ecs/channels/{_HKB_CH}/inputs/{_HKB_IN}/adcue.json",
     {"adcue": {"type": "scte35", "cue": "", "duration": "30.000", "offset": "0.000", "repeat": "false"}})
_hkb("inject_scte35", {"channel": _HKB_CH, "input": _HKB_IN, "cue": "<SpliceInfoSection>\n</SpliceInfoSection>",
                       "duration": 0.0, "offset": 2.5, "repeat": True}, "PUT",
     f"/ecs/channels/{_HKB_CH}/inputs/{_HKB_IN}/adcue.json",
     {"adcue": {"type": "scte35", "cue": "<SpliceInfoSection>\n</SpliceInfoSection>", "duration": "0.000",
                "offset": "2.500", "repeat": "true"}}, file="inject_scte35-cue")
_hkb("inject_splice_out", {"channel": _HKB_CH, "input": _HKB_IN, "duration": 5.0}, "PUT",
     f"/ecs/channels/{_HKB_CH}/inputs/{_HKB_IN}/adcue.json",
     {"adcue": {"type": "SpliceOut", "duration": "5.000", "offset": "0.000", "repeat": "false"}},
     http_reply={"status": 500, "body": '{"messages":{"count":1,"message":[{"text":"KULA_ERROR_WRONG_STATE","type":"error"}]}}'},
     expect_result={"error": {"error": "device_error", "code": "500"}})
_hkb("get_channel_configuration", {"channel": _HKB_CH}, "GET", f"/ecs/channels/{_HKB_CH}/preset.json")
_HKB_PRESET = {"version": "5.0", "type": "multistream", "label": "Test", "kulabytepreset": {"output": []}}
_hkb("load_channel_configuration", {"channel": _HKB_CH, "configuration": _HKB_PRESET}, "PUT",
     f"/ecs/channels/{_HKB_CH}/preset.json", _HKB_PRESET)
_hkb("get_configuration_label", {"channel": _HKB_CH}, "GET", f"/ecs/channels/{_HKB_CH}/preset/label.json",
     http_reply={"status": 200, "body": '{"preset":"This is the configuration Label"}'},
     expect_result={"ok": {"kind": "value", "value": "This is the configuration Label"}})


def _hkb_t(name, path, body, expect_state, **extra):
    V.append({"spec": HKB, "telemetry": name,
              "inbound_http": {"path": path, "body": body if isinstance(body, str) else _hkb_j(body)},
              "expect_state": expect_state, **extra})


_hkb_t("ecs", "/ecs.json", {"encoder": {
    "CurrentCpuUsage": "32.78", "FreeDiskSpaceGB": "403.792", "UsedDiskSpacePercent": "0", "app": "kulabyte", "build": "406",
    "major": "5", "minor": "0", "subminor": "0", "type": "DEV", "label": "localhost.localdomain", "temperature": "29.8",
    "network": {"in": "0.032", "out": "0.037", "total": "0.069"}, "uptime": {"hours": 12},
    "gpu": {"hwencoder": {"h264": False, "hevc": False}},
    "channels": {"channel": [{"event_id": "", "event_label": "", "id": _HKB_CH, "label": "TestChannel2",
                              "outoforder": False, "port": 2094, "startcounter": 1}], "count": 1},
    "license": {"channels": 4, "expired": False, "mac": "D4:AE:52:B9:3E:E9", "registered_channels": 1, "start_year": 2016,
                "valid": True, "type": "key"},
    "time": {"localtime": {"hour": 22, "minute": 4}, "utctime": {"hour": 3}}}},
    {"encoder": {"cpu": 32.78, "disk_free_gb": 403.792, "disk_used_percent": 0.0, "app": "kulabyte", "label": "localhost.localdomain",
                 "temperature": 29.8, "uptime_hours": 12,
                 "version": {"major": "5", "minor": "0", "subminor": "0", "build": "406", "type": "DEV"},
                 "network": {"in": 0.032, "out": 0.037, "total": 0.069}, "gpu": {"h264": False, "hevc": False}},
     "channels": {_HKB_CH: {"label": "TestChannel2", "port": 2094, "out_of_order": False, "start_counter": 1,
                            "event_id": "", "event_label": ""}},
     "license": {"channels": 4, "expired": False, "mac": "D4:AE:52:B9:3E:E9", "registered_channels": 1,
                 "start": {"year": 2016}, "valid": True, "type": "key"},
     "time": {"local": {"hour": 22, "minute": 4}, "utc": {"hour": 3}}},
    expect_then_send=[{"method": "GET", "target": f"/ecs/channels/{_HKB_CH}.json"}],
    state_before={"channels": {"gone": {"label": "Old"}}})
_hkb_t("channel-state", f"/ecs/channels/{_HKB_CH}.json", {"channel": {
    "CurrentCpuUsage": "14.52", "ElapsedRunningTime": "00:00:07", "PresetLabel": "TestChannel2", "PresetState": "fully_loaded",
    "RecordState": "idle", "Threads": "110", "id": _HKB_CH, "label": "TestChannel2", "state": "running", "type": "multistream",
    "encoders": {"count": "1", "encoder": [{"Implementation": "Software", "VideoCodec": "H264", "VideoEncoderBitrate": "1372000",
                                            "VideoEncoderFps": "29.970", "VideoFrameHeight": "540", "VideoFrameWidth": "960",
                                            "comp": "AVC_ENC", "id": "AVC_ENC_6", "label": "AVC_ENC_6", "state": "enabled",
                                            "type": "videoencoder"}]},
    "inputs": {"active": "Device_DeckLink_Baseband_2", "count": "1", "input": [
        {"AudioGainValue": "0.000", "AudioLevel": "-25.6;-23.3", "DetectedVideo": "1", "DetectedVideoWidth": "1920",
         "InputSignal": "good", "SupportsAudioGainControl": "1", "comp": "Device_DeckLink_Baseband",
         "id": "Device_DeckLink_Baseband_2", "label": "Device_DeckLink_Baseband_2"}]},
    "outputs": {"count": "1", "output": [{"StreamURL": "http://hls.server.address", "VideoEncoder": "AVC_ENC_6|AVC_ENC_7",
                                          "comp": "HLS_Sink", "id": "HLS_Sink_12", "label": "HLS_Sink_12",
                                          "name": "HLS output", "state": "disconnected", "type": "http"}]}}},
    {"channel_state": {_HKB_CH: {
        "cpu": 14.52, "elapsed": "00:00:07", "preset_label": "TestChannel2", "preset_state": "fully_loaded",
        "record_state": "idle", "threads": 110, "label": "TestChannel2", "state": "running", "type": "multistream",
        "active_input": "Device_DeckLink_Baseband_2",
        "encoders": {"AVC_ENC_6": {"implementation": "Software", "video_codec": "H264", "video_bitrate": 1372000,
                                   "video_fps": 29.97, "video_height": 540, "video_width": 960, "component": "AVC_ENC",
                                   "label": "AVC_ENC_6", "state": "enabled", "type": "videoencoder"}},
        "inputs": {"Device_DeckLink_Baseband_2": {"audio_gain_db": 0.0, "audio_levels": "-25.6;-23.3", "detected_video": True,
                                                  "detected_video_width": 1920, "signal": "good", "audio_gain_control": True,
                                                  "component": "Device_DeckLink_Baseband",
                                                  "label": "Device_DeckLink_Baseband_2"}},
        "outputs": {"HLS_Sink_12": {"stream_url": "http://hls.server.address", "video_encoder": "AVC_ENC_6|AVC_ENC_7",
                                    "component": "HLS_Sink", "label": "HLS_Sink_12", "name": "HLS output",
                                    "state": "disconnected", "type": "http"}}}}})
_hkb_t("channel-deleted", f"/ecs/channels/{_HKB_CH}.json",
       '{"messages":{"count":1,"message":[{"text":"Resource deleted successfully.","type":"info"}]}}',
       {"channel_state": {}, "channel_signals": {}, "channel_configuration": {}},
       state_before={"channel_state": {_HKB_CH: {"state": "running"}}})
_hkb_t("channel-configs", "/ecs/channelconfigs.json", [{
    "preset": {"version": "5.0", "type": "multistream", "label": "RTMP"}, "_id": "5c6c3e74c08586c8070219d7",
    "state": "idle", "actions": {"recording": "idle"}, "name": "RTMP", "description": "",
    "metrics": {"memory": "0", "v_memory": "0", "cpu": "0", "network": {"incoming": "0", "outgoing": "0"}}}],
    {"configs": {"5c6c3e74c08586c8070219d7": {
        "label": "RTMP", "type": "multistream", "version": "5.0", "state": "idle", "recording": "idle", "name": "RTMP",
        "description": "", "memory": 0.0, "virtual_memory": 0.0, "cpu": 0.0, "network_in": 0.0, "network_out": 0.0,
        "preset": '{"version":"5.0","type":"multistream","label":"RTMP"}'}}})
_hkb_t("system", "/ecs/system.json", {"system": {"capturesources": {"aja_detected": False, "ajacard": [], "decklink_detected": True},
                                                 "cores_count": 8, "cpu_count": 1, "gpu": {"hwencoder": {"h264": False, "hevc": True}},
                                                 "label": "localhost.localdomain", "memory": {"mem_total": "3872260 kB"},
                                                 "network_adapters": {"adapter": [{"address": "10.67.12.128",
                                                                                   "mac": "d4:ae:52:b9:3e:e9", "name": "em1"}]},
                                                 "uptime": {"hours": 12}}},
       {"system": {"aja_detected": False, "aja_cards": "[]", "decklink_detected": True, "cores": 8, "cpu_count": 1,
                   "gpu_h264": False, "gpu_hevc": True, "label": "localhost.localdomain", "memory_total": "3872260 kB",
                   "network_adapters": '[{"address":"10.67.12.128","mac":"d4:ae:52:b9:3e:e9","name":"em1"}]',
                   "uptime_hours": 12}})
_hkb_t("license", "/ecs/license.json", {"license": {"channels": 4, "demo_mode": False, "expired": False, "no_watermark": True,
                                                    "start_day": 10, "stop_year": 2016, "time_limit": 0, "time_zone": "",
                                                    "valid": True}},
       {"license": {"channels": 4, "demo_mode": False, "expired": False, "no_watermark": True, "start": {"day": 10},
                    "stop": {"year": 2016}, "time_limit": 0, "time_zone": "", "valid": True}})
_hkb_t("license-state", "/ecs/license/state.json", {"license_state": {"mac": "D4:AE:52:B9:3E:E9", "registered_channels": 2,
                                                                      "type": "key"}},
       {"license": {"mac": "D4:AE:52:B9:3E:E9", "registered_channels": 2, "type": "key"}})
_hkb_t("time", "/ecs/time.json", {"time": {"localtime": {"day": 11, "year": 2016}, "utctime": {"day": 12}}},
       {"time": {"local": {"day": 11, "year": 2016}, "utc": {"day": 12}}})
_hkb_t("channel-list", "/ecs/channels.json", {"channels": {"channel": [{"id": "57740998aa602b6d251cf1d5", "label": "TestChannel1",
                                                                        "outoforder": False, "port": 2088, "startcounter": 2}],
                                                            "count": 1}},
       {"channels": {"57740998aa602b6d251cf1d5": {"label": "TestChannel1", "out_of_order": False, "port": 2088,
                                                  "start_counter": 2}}},
       expect_then_send=[{"method": "GET", "target": "/ecs/channels/57740998aa602b6d251cf1d5.json"}])
_hkb_t("signals", f"/ecs/channels/{_HKB_CH}/signals.json?index=-1",
       '{"signals":{"channel":"TestChannel2","count":1,"signal":[{"index":"0","type":"CHANNEL_START"}]}}',
       {"channel_signals": {_HKB_CH: '{"signals":{"channel":"TestChannel2","count":1,"signal":[{"index":"0","type":"CHANNEL_START"}]}}'}})
_hkb_t("preset-label", f"/ecs/channels/{_HKB_CH}/preset/label.json", {"preset": "Test"},
       {"channel_configuration": {_HKB_CH: {"label": "Test"}}})
_hkb_t("preset", f"/ecs/channels/{_HKB_CH}/preset.json", {"label": "Test", "kulabytepreset": {"output": []}},
       {"channel_configuration": {_HKB_CH: {"configuration": '{"label":"Test","kulabytepreset":{"output":[]}}'}}})
