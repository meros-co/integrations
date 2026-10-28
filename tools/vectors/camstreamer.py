# CamStreamer apps (camstreamer): one vector per command, and telemetry
# vectors for the polled resources. Targets follow CamStreamer's ACAP API
# page (camstreamer.com/api); reply bodies its examples.
CMS = "camstreamer"


def _cms(model, command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": CMS, "model": model, "command": command, "input": input, "expect_request": request, **extra})


_CMSSTREAM = {"streamId": "api_01", "platform": "rtmp", "enabled": True, "active": True, "title": "RTMP(S) Universal",
              "trigger": {"type": "manual"},
              "video": {"input": {"type": "CRS"}, "output": {"type": "video", "url": "rtmp://a.rtmp.youtube.com/live2/key", "parameters": ""}},
              "audio": {"source": "none"}, "status": {"led": True}}

# CamStreamer App 5.x and 6.x.
_cms("app-6", "list_streams", {}, "GET", "/local/camstreamer/stream_list.cgi?action=get")
_cms("app-6", "set_stream_list", {"config": {"streamList": [_CMSSTREAM]}}, "POST", "/local/camstreamer/stream_list.cgi?action=set",
     body=json.dumps({"streamList": [_CMSSTREAM]}, separators=(",", ":")))
_cms("app-6", "clear_streams", {}, "POST", "/local/camstreamer/stream_list.cgi?action=set", body='{"streamList":[]}')
_cms("app-6", "get_stream", {"stream": "api_01"}, "GET", "/local/camstreamer/stream/stream_list.cgi?action=get&stream_id=api_01")
_cms("app-6", "set_stream", {"stream": "api_01", "config": _CMSSTREAM}, "POST",
     "/local/camstreamer/stream/stream_list.cgi?action=set&stream_id=api_01", body=json.dumps(_CMSSTREAM, separators=(",", ":")))
_cms("app-6", "set_stream_enabled", {"stream": "1057", "enabled": True}, "GET", "/local/camstreamer/set_stream_enabled.cgi?stream_id=1057&enabled=1")
_cms("app-6", "get_stream_status", {"stream": "117930"}, "GET", "/local/camstreamer/get_streamstat.cgi?stream_id=117930",
     http_reply={"status": 200, "body": json.dumps({"status": 200, "message": "OK", "data": {"is_streaming": 1}})},
     expect_result={"ok": {"kind": "value", "value": {"status": 200, "message": "OK", "data": {"is_streaming": 1}}}})
_cms("app-6", "get_hls_signature", {}, "GET", "/axis-cgi/param.cgi?action=list&group=camstreamer.signature")
# A refused credential is terminal.
_cms("app-6", "set_stream_enabled", {"stream": "1057", "enabled": False}, "GET",
     "/local/camstreamer/set_stream_enabled.cgi?stream_id=1057&enabled=0",
     http_reply={"status": 401, "body": ""}, expect_result={"error": {"error": "auth"}}, file="set_stream_enabled-refused")

# CamStreamer App 4.x.
_CMS4 = {"3577": {"enabled": "1", "active": "1", "audioSource": "default", "mediaServerUrl": "hls://", "trigger": "none"}}
_cms("app-4", "list_streams_v4", {}, "GET", "/local/camstreamer/stream/list.cgi?action=get")
_cms("app-4", "set_stream_list_v4", {"config": _CMS4}, "POST", "/local/camstreamer/stream/list.cgi?action=set",
     body=json.dumps(_CMS4, separators=(",", ":")))
_cms("app-4", "get_stream_v4", {"stream": "3577"}, "GET", "/local/camstreamer/stream/get.cgi?stream_id=3577")
_cms("app-4", "set_stream_param_v4", {"stream": "1057", "param": "mediaServerUrl",
                                      "value": "rtmp://a.rtmp.youtube.com/live2/rfp-test-2926.q9yk-3wv0-4zzg-78ks"},
     "GET", "/local/camstreamer/stream/set.cgi?stream_id=1057&mediaServerUrl=rtmp%3A%2F%2Fa.rtmp.youtube.com%2Flive2%2Frfp-test-2926.q9yk-3wv0-4zzg-78ks")
_cms("app-4", "set_stream_enabled_v4", {"stream": "1057", "enabled": False}, "GET", "/local/camstreamer/stream/set.cgi?stream_id=1057&enabled=0")
_cms("app-4", "set_stream_active_v4", {"stream": "1057", "active": True}, "GET", "/local/camstreamer/stream/set.cgi?stream_id=1057&active=1")
_cms("app-4", "remove_stream_v4", {"stream": "1057"}, "GET", "/local/camstreamer/stream/remove.cgi?stream_id=1057")
_cms("app-4", "get_params_v4", {"names": "signature,LiveLED"}, "GET", "/local/camstreamer/param/get.cgi?list=signature%2CLiveLED")
_cms("app-4", "set_param_v4", {"name": "LiveLED", "value": "1"}, "GET", "/local/camstreamer/param/set.cgi?LiveLED=1")
_cms("app-4", "set_stream_enabled", {"stream": "1057", "enabled": True}, "GET",
     "/local/camstreamer/set_stream_enabled.cgi?stream_id=1057&enabled=1", file="set_stream_enabled-app-4")
_cms("app-4", "get_stream_status", {"stream": "1057"}, "GET", "/local/camstreamer/get_streamstat.cgi?stream_id=1057",
     file="get_stream_status-app-4")

# CamOverlay App.
_cms("camoverlay", "get_camoverlay_params", {}, "GET", "/axis-cgi/param.cgi?action=list&group=CamOverlay")
_cms("camoverlay", "set_overlays_enabled", {"enabled": False}, "GET", "/axis-cgi/param.cgi?action=update&CamOverlay.Enabled=0")
_cms("camoverlay", "get_services", {}, "GET", "/local/camoverlay/api/services.cgi?action=get")
_COVS = {"services": [{"id": 1, "enabled": 1, "automationType": "manual", "cameraList": [0], "schedule": "", "name": "infoticker"}]}
_cms("camoverlay", "set_services", {"config": _COVS}, "POST", "/local/camoverlay/api/services.cgi?action=set",
     body=json.dumps(_COVS, separators=(",", ":")))
_cms("camoverlay", "set_service_enabled", {"service": 5, "enabled": True}, "GET", "/local/camoverlay/api/enabled.cgi?id_5=1")
_cms("camoverlay", "set_service_enabled_by_name", {"name": "myservice", "enabled": False}, "GET",
     "/local/camoverlay/api/enabled.cgi?name_myservice=0")
_cms("camoverlay", "set_custom_text", {"service": 2, "field": "field1", "text": "Score: 3 & 1"}, "GET",
     "/local/camoverlay/api/customGraphics.cgi?action=update_text&service_id=2&field1=Score%3A%203%20%26%201")
_cms("camoverlay", "set_custom_text_color", {"service": 2, "field": "field1", "text": "Live", "color": "255000000"}, "GET",
     "/local/camoverlay/api/customGraphics.cgi?action=update_text&service_id=2&field1=Live&field1_color=255000000")
_cms("camoverlay", "set_custom_text_variant", {"service": 2, "field": "field1", "index": 0}, "GET",
     "/local/camoverlay/api/customGraphics.cgi?action=update_text&service_id=2&field1_index=0")
_cms("camoverlay", "set_custom_image_from_camera", {"service": 2, "path": "/var/spool/storage/SD_DISK/bg.png"}, "GET",
     "/local/camoverlay/api/customGraphics.cgi?action=update_image&service_id=2&image=%2Fvar%2Fspool%2Fstorage%2FSD_DISK%2Fbg.png")
_cms("camoverlay", "set_custom_image_position", {"service": 2, "coord": "top_left", "x": 10, "y": 20}, "GET",
     "/local/camoverlay/api/customGraphics.cgi?action=update_image&service_id=2&coord_system=top_left&pos_x=10&pos_y=20")
V.append({"spec": CMS, "model": "camoverlay", "command": "upload_custom_image", "input": {"service": 2, "image": "iVBORw0KGgo="},
          "expect_request": {"method": "POST", "target": "/local/camoverlay/api/customGraphics.cgi?action=update_image&service_id=2",
                             "body_hex": "89504e470d0a1a0a", "content_type": "application/octet-stream"}})
_cms("camoverlay", "set_infoticker_text", {"service": 1, "text": "Doors open at 7"}, "GET",
     "/local/camoverlay/api/infoticker.cgi?service_id=1&text=Doors%20open%20at%207")

# CamSwitcher App.
_CSWA = "/local/camswitcher/react/local/camswitcher/api"
_CSWP = "cdb04b0cb-0c7b-4292-aa0f-7920eb73333e"
_cms("camswitcher", "get_output_info", {}, "GET", "/local/camswitcher/output_info.cgi")
_cms("camswitcher", "list_playlists", {}, "GET", _CSWA + "/playlists.cgi?action=get")
_cms("camswitcher", "switch_playlist", {"playlist": _CSWP}, "GET", _CSWA + "/playlist_switch.cgi?playlist_name=" + _CSWP)
_cms("camswitcher", "list_queue", {}, "GET", _CSWA + "/playlist_queue_list.cgi")
_cms("camswitcher", "queue_playlist", {"playlist": _CSWP}, "GET", _CSWA + "/playlist_queue_push.cgi?playlist_name=" + _CSWP)
_cms("camswitcher", "play_next", {}, "GET", _CSWA + "/playlist_queue_play_next.cgi")
_cms("camswitcher", "clear_queue", {}, "GET", _CSWA + "/playlist_queue_clear.cgi")
_cms("camswitcher", "list_clips", {}, "GET", _CSWA + "/clips.cgi?action=get")
_cms("camswitcher", "get_master_audio", {}, "GET", "/axis-cgi/param.cgi?action=list&group=camswitcher.masterAudio")
_cms("camswitcher", "set_master_audio", {"mode": "silence"}, "GET",
     "/axis-cgi/param.cgi?action=update&camswitcher.masterAudio=%7B%22type%22%3A%22SILENCE%22%7D")
_cms("camswitcher", "set_master_audio", {"mode": "source"}, "GET",
     "/axis-cgi/param.cgi?action=update&camswitcher.masterAudio=%7B%22type%22%3A%22NONE%22%7D", file="set_master_audio-source")
_cms("camswitcher", "set_master_audio_clip", {"clip": "sd2b03f67-31c7-40b2-99c1-5079dd0dab65"}, "GET",
     "/axis-cgi/param.cgi?action=update&camswitcher.masterAudio="
     "%7B%22type%22%3A%22CLIP%22%2C%22clip_name%22%3A%22sd2b03f67-31c7-40b2-99c1-5079dd0dab65%22%7D")
_cms("camswitcher", "set_master_audio_stream", {"stream": _CSWP}, "GET",
     "/axis-cgi/param.cgi?action=update&camswitcher.masterAudio="
     "%7B%22type%22%3A%22STREAM%22%2C%22stream_name%22%3A%22" + _CSWP + "%22%7D")

# CamScripter App.
_cms("camscripter", "list_packages", {}, "GET", "/local/camscripter/package/list.cgi")
_cms("camscripter", "remove_package", {"package": "clock"}, "GET", "/local/camscripter/package/remove.cgi?package_name=clock")
_cms("camscripter", "get_package_settings", {"package": "clock"}, "GET",
     "/local/camscripter/package/settings.cgi?package_name=clock&action=get")
_cms("camscripter", "set_package_settings", {"package": "clock", "config": {"format": "24h"}}, "POST",
     "/local/camscripter/package/settings.cgi?package_name=clock&action=set", body='{"format":"24h"}')
_cms("camscripter", "set_package_configurations", {"config": {"clock": {"enabled": True}}}, "GET",
     "/axis-cgi/param.cgi?action=update&camscripter.PackageConfigurations=%7B%22clock%22%3A%7B%22enabled%22%3Atrue%7D%7D")
_cms("camscripter", "list_network_cameras", {}, "GET", "/local/camscripter/network_camera_list.cgi")
# Install: multipart/form-data (RFC 7578), one part named file with the
# package's name; the boundary is the core's, derived from the parts (SPEC.md
# §4).
def _cms_form(name, data, filename, content_type):
    import hashlib
    digest = hashlib.sha256(name.encode() + b"\0" + data + b"\0").hexdigest()
    boundary = "----form-" + digest[:16]
    out = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n"
           f"Content-Type: {content_type}\r\n\r\n").encode() + data + b"\r\n" + f"--{boundary}--\r\n".encode()
    return out, f"multipart/form-data; boundary={boundary}"


_CMSZIP, _CMSZIPT = _cms_form("file", b"PK\x03\x04", "clock.zip", "application/zip")
V.append({"spec": CMS, "model": "camscripter", "command": "install_package",
          "input": {"package": "UEsDBA==", "filename": "clock.zip"},
          "expect_request": {"method": "POST", "target": "/local/camscripter/package/install.cgi", "body_hex": _CMSZIP.hex(),
                             "content_type": _CMSZIPT}})

# NDI HX Plugin.
_cms("ndi-hx", "get_ndi_settings", {}, "GET", "/local/ndihxplugin/settings.cgi?action=get")
_NDI = {"ndiName": "P1375", "ndiGroups": "", "channel": 2, "width": 1920, "height": 1080, "fps": 50, "gop": 101, "audioEnabled": False}
_cms("ndi-hx", "set_ndi_settings", {"config": _NDI}, "POST", "/local/ndihxplugin/settings.cgi?action=set",
     body=json.dumps(_NDI, separators=(",", ":")))

# Telemetry.
telemetry(CMS, "streams-6", model="app-6",
          inbound_http={"path": "/local/camstreamer/stream_list.cgi?action=get", "body": json.dumps({"streamList": [_CMSSTREAM]})},
          state_before={"streams": {"old": {"title": "Gone"}}},
          expect_state={"streams": {"api_01": {"title": "RTMP(S) Universal", "platform": "rtmp", "enabled": True, "active": True,
                                               "trigger": "manual"}}},
          expect_then_send=[{"method": "GET", "target": "/local/camstreamer/get_streamstat.cgi?stream_id=api_01"}])
telemetry(CMS, "streams-6-data", model="app-6",
          inbound_http={"path": "/local/camstreamer/stream_list.cgi?action=get",
                        "body": json.dumps({"status": 200, "message": "OK", "data": {"streamList": [dict(_CMSSTREAM, enabled=False)]}})},
          expect_state={"streams": {"api_01": {"title": "RTMP(S) Universal", "platform": "rtmp", "enabled": False, "active": True,
                                               "trigger": "manual"}}},
          expect_then_send=[{"method": "GET", "target": "/local/camstreamer/get_streamstat.cgi?stream_id=api_01"}])
telemetry(CMS, "streams-4", model="app-4",
          inbound_http={"path": "/local/camstreamer/stream/list.cgi?action=get", "body": json.dumps(_CMS4)},
          expect_state={"streams": {"3577": {"enabled": True, "active": True, "trigger": "none"}}},
          expect_then_send=[{"method": "GET", "target": "/local/camstreamer/get_streamstat.cgi?stream_id=3577"}])
telemetry(CMS, "stream-status", model="app-6",
          inbound_http={"path": "/local/camstreamer/get_streamstat.cgi?stream_id=117930", "body": json.dumps(
              {"status": 200, "message": "OK", "data": {"net_stats": "110779840 1107300413", "stream_bytes_time_ms": 1600092179717,
                                                        "stream_bytes": 568335, "start_count": 1, "is_streaming": 1}})},
          expect_state={"stream_status": {"117930": {"streaming": True, "bytes": 568335, "start_count": 1}}})
telemetry(CMS, "services", model="camoverlay",
          inbound_http={"path": "/local/camoverlay/api/services.cgi?action=get", "body": json.dumps({"services": [
              {"id": 1, "enabled": 1, "automationType": "manual", "cameraList": [0], "schedule": "", "name": "infoticker",
               "customName": ""},
              {"id": 5, "enabled": 0, "automationType": "time", "cameraList": [0], "schedule": "", "name": "images",
               "customName": "Logo"}]})},
          expect_state={"services": {"1": {"type": "infoticker", "name": "", "enabled": True, "automation": "manual"},
                                     "5": {"type": "images", "name": "Logo", "enabled": False, "automation": "time"}}})
telemetry(CMS, "overlays-enabled", model="camoverlay",
          inbound_http={"path": "/axis-cgi/param.cgi?action=list&group=CamOverlay.Enabled", "body": "root.CamOverlay.Enabled=1\n"},
          expect_state={"overlays_enabled": True})
telemetry(CMS, "playlists", model="camswitcher",
          inbound_http={"path": _CSWA + "/playlists.cgi?action=get", "body": json.dumps({"status": 200, "message": "OK", "data": {
              _CSWP: {"niceName": "V5925 - B8A44F009C1F", "loop": False, "isFavourite": False, "stream_list": [], "keyboard": {},
                      "channel": "av"}}})},
          expect_state={"playlists": {_CSWP: {"name": "V5925 - B8A44F009C1F", "loop": False, "favourite": False, "channel": "av"}}})
telemetry(CMS, "master-audio", model="camswitcher",
          inbound_http={"path": "/axis-cgi/param.cgi?action=list&group=camswitcher.masterAudio",
                        "body": 'root.camswitcher.masterAudio={"type":"SILENCE"}\n'},
          expect_state={"master_audio": '{"type":"SILENCE"}'})
telemetry(CMS, "packages", model="camscripter",
          inbound_http={"path": "/local/camscripter/package/list.cgi", "body": json.dumps([
              {"package_name": "clockAnalog", "package_menu_name": "Analog Clock", "package_version": "1.0.1",
               "vendor": "Camstreamer s.r.o.", "required_camscripter_version": "1.2.3",
               "ui_link": "/local/camscripter/package/clockAnalog/index.html"}])},
          expect_state={"packages": {"clockAnalog": {"menu_name": "Analog Clock", "version": "1.0.1", "vendor": "Camstreamer s.r.o.",
                                                     "ui_link": "/local/camscripter/package/clockAnalog/index.html"}}})
telemetry(CMS, "ndi", model="ndi-hx",
          inbound_http={"path": "/local/ndihxplugin/settings.cgi?action=get", "body": json.dumps(_NDI)},
          expect_state={"ndi": {"name": "P1375", "groups": "", "channel": 2, "width": 1920, "height": 1080, "fps": 50.0, "gop": 101,
                                "audio_enabled": False}})

# The apps' websocket events (CamStreamer API: CSt, COv and CSw WebSocket API),
# as they happen and as the initial state wrapped in init.
telemetry(CMS, "ws-stream-state", model="app-6",
          inbound_ws='{"type":"StreamState","streamID":"117930","enabled":0,"active":1,"automationState":1,"isStreaming":0}',
          expect_state={"streams": {"117930": {"enabled": False, "active": True, "automation_state": 1}},
                        "stream_status": {"117930": {"streaming": False}}})
telemetry(CMS, "ws-stream-state-init", model="app-4",
          inbound_ws='{"type":"init","data":{"type":"StreamState","streamID":"117930","enabled":1,"active":1,"automationState":1,"isStreaming":1}}',
          expect_state={"streams": {"117930": {"enabled": True, "active": True, "automation_state": 1}},
                        "stream_status": {"117930": {"streaming": True}}})
telemetry(CMS, "ws-api-success", model="app-6",
          inbound_ws='{"type":"CS_API_SUCCESS","streamID":"117930","apiCall":"startBroadcast","message":"Broadcast started."}',
          expect_state={"stream_status": {"117930": {"last_api_call": "startBroadcast",
                                                     "last_api_message": "Broadcast started."}}})
telemetry(CMS, "ws-port-changed", model="app-6",
          inbound_ws='{"type":"init","data":{"type":"PortChanged","port":0, "value":1}}',
          expect_state={"io_ports": {"0": {"value": 1}}})
telemetry(CMS, "ws-service-start", model="camoverlay", inbound_ws='{"type":"ServiceStart","serviceId":2}',
          expect_state={"services": {"2": {"running": True}}})
telemetry(CMS, "ws-service-stop-init", model="camoverlay",
          inbound_ws='{"type":"init","data":{"type":"ServiceStop","serviceId":3}}',
          expect_state={"services": {"3": {"running": False}}})
telemetry(CMS, "ws-switcher-start", model="camswitcher",
          inbound_ws='{"type":"init","data":{"type":"SwitcherStart","default_playlist_id":"c26b737ea-1178-4f9e-851c-479cbdd6e97d"}}',
          expect_state={"switcher": {"default_playlist": "c26b737ea-1178-4f9e-851c-479cbdd6e97d"}})
telemetry(CMS, "ws-switch-video", model="camswitcher",
          inbound_ws='{"type":"StreamSwitchVideo","playlist_active_stream":2,"stream_name":"cam2","playlist_name":"main"}',
          expect_state={"switcher": {"video": {"playlist": "main", "stream": "cam2", "index": 2}}})
telemetry(CMS, "ws-switch-audio-init", model="camswitcher",
          inbound_ws='{"type":"init","data":{"playlist_active_stream":0,"stream_name":"c4f673956-99c5-4a92-8854-c369d1f7da27","playlist_name":"c4f673956-99c5-4a92-8854-c369d1f7da27","type":"StreamSwitchAudio"}}',
          expect_state={"switcher": {"audio": {"playlist": "c4f673956-99c5-4a92-8854-c369d1f7da27",
                                               "stream": "c4f673956-99c5-4a92-8854-c369d1f7da27", "index": 0}}})
telemetry(CMS, "ws-stream-available", model="camswitcher",
          inbound_ws='{"type":"init","data":{"type":"StreamAvailable","stream_name":"cam.2","state":true}}',
          expect_state={"stream_available": {"cam.2": {"available": True}}})
