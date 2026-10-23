# Magewell Director Mini, One and Plus (magewell-director): GET
# /V1.0/<group>/<method>?<parameters> (Director API, sections 1.1-1.3). One
# vector per command: the command list comes from the spec, and the expected
# request is computed here independently: example values written as the API
# document shows them (true/false, decimals to the spec's precision, enum
# values through the document's numbering), percent-encoded per RFC 3986.
# Telemetry vectors use the document's own example replies.
def _director_vectors() -> None:
    import re
    from urllib.parse import quote

    sid = "magewell-director"
    doc = yaml.safe_load((ROOT / "specs" / f"{sid}.yaml").read_text(encoding="utf-8"))
    maps = doc["maps"]
    examples = {"int": 3, "float": 0.5, "bool": True}
    strings = {"cameras": "[0,1,2,3]", "ids": "23,25", "id": "1718789860579", "scene_id": "3f12205d-a92c-4575-bd09-e947fab51bc3",
               "time": "10:00", "tags": "tag2,tag3", "source": "4d056314a79745aaadebbcc9295e8a7c"}
    for name, command in doc["commands"].items():
        params = command.get("params") or {}
        values = {}
        for pname, p in params.items():
            if p["type"] == "enum":
                values[pname] = p["values"][-1]
            elif p["type"] == "string":
                values[pname] = strings.get(pname, "Scene 1" if "pattern" not in p else "abc123")
                if "pattern" in p and not re.search(p["pattern"], values[pname]):
                    values[pname] = "1718789860579"
            elif p["type"] == "int":
                lo, hi = p.get("min", 0), p.get("max", 100)
                values[pname] = min(max(3, lo), hi)
            elif p["type"] == "float":
                lo, hi = p.get("min", 0.0), p.get("max", 1.0)
                values[pname] = min(max(0.75, lo), hi)
            else:
                values[pname] = examples[p["type"]]
        send = command["send"]
        query = []
        for key, template in (send.get("query") or {}).items():
            m = re.fullmatch(r"\{(\w+)(?::(.+))?\}", template)
            if not m:
                query.append((key, template))
                continue
            v, directive = values[m.group(1)], m.group(2)
            if directive and directive.startswith("map."):
                text = maps[directive[4:]][v if not isinstance(v, bool) else str(v).lower()]
            elif directive and directive.startswith("."):
                text = f"{v:{directive}}"
            elif isinstance(v, bool):
                text = "true" if v else "false"
            else:
                text = str(v)
            query.append((key, text))
        target = send["path"]
        if query:
            target += "?" + "&".join(f"{k}={quote(t, safe='')}" for k, t in query)
        V.append({"spec": sid, "command": name, "input": values,
                  "expect_request": {"method": "GET", "target": target}})


_director_vectors()

MD = "magewell-director"
# A command answered 403: refused, not a credential (the API has none).
V.append({"spec": MD, "command": "scene_next", "input": {}, "file": "scene_next-refused",
          "expect_request": {"method": "GET", "target": "/V1.0/scene/next"},
          "http_reply": {"status": 403, "body": ""},
          "expect_result": {"error": {"error": "device_error", "code": "403"}}})
telemetry(MD, "scenes", inbound_http={"path": "/V1.0/scene/getSceneList", "body": json.dumps({
    "sceneList": [
        {"isPGM": False, "isPreview": True, "name": "PICTURE", "uuid": "17170511950740",
         "privateGfx": {"gfxSceneType": 4, "name": "GFX", "uuid": "f98f1790-8f38-4467-99fe-bd9e02c7c6bc"}},
        {"isPGM": True, "isPreview": False, "name": "HDMI 1", "uuid": "17170511950741", "privateGfx": {"gfxSceneType": 0}}],
    "totalCount": 2, "message": "SUCCESS", "status": 0})},
    state_before={"scenes": {"1": {"name": "Gone"}}},
    expect_state={"scenes": {
        "17170511950740": {"name": "PICTURE", "index": 1, "program": False, "preview": True,
                           "private_gfx": {"id": "f98f1790-8f38-4467-99fe-bd9e02c7c6bc", "name": "GFX", "type": "custom"}},
        "17170511950741": {"name": "HDMI 1", "index": 2, "program": True, "preview": False, "private_gfx": {"type": "general"}}},
        "program": {"scene_id": "17170511950741", "scene_name": "HDMI 1"},
        "preview": {"scene_id": "17170511950740", "scene_name": "PICTURE"}})
telemetry(MD, "gfx", inbound_http={"path": "/V1.0/gfx/getGFXList", "body": json.dumps({
    "gfxList": [{"gfxSceneType": 1, "name": "Scoreboard", "uuid": "71465631-a682-49af-918e-e3713b6cbb18"},
                {"gfxSceneType": 9, "name": "Lower Third", "uuid": "e7d00fe7-188f-4fda-aa68-04f82a6fc844"}],
    "totalCount": 2, "message": "SUCCESS", "status": 0})},
    expect_state={"gfx": {"71465631-a682-49af-918e-e3713b6cbb18": {"name": "Scoreboard", "index": 1, "type": "scoreboard"},
                          "e7d00fe7-188f-4fda-aa68-04f82a6fc844": {"name": "Lower Third", "index": 2, "type": "pag"}}})
telemetry(MD, "bgm", inbound_http={"path": "/V1.0/bgm/getPlayList", "body": json.dumps({
    "bgmList": [{"singer": "Magewell", "artworkPath": "/storage/emulated/0/Director/Files/audio/a.png", "duration": 240,
                 "fileMD5": "19b3b7793e8b6837fafaef300955ba83", "filePath": "/storage/emulated/0/Director/Files/audio/a.mp3",
                 "notExist": False, "title": "Rain"}], "message": "SUCCESS", "status": 0})},
    expect_state={"bgm": {"tracks": {"1": {"title": "Rain", "file_path": "/storage/emulated/0/Director/Files/audio/a.mp3",
                                           "md5": "19b3b7793e8b6837fafaef300955ba83", "duration": 240, "artist": "Magewell",
                                           "artwork_path": "/storage/emulated/0/Director/Files/audio/a.png", "missing": False}}}})
telemetry(MD, "shows", inbound_http={"path": "/V1.0/show/getShowList", "body": json.dumps({
    "showList": [{"frameHeight": 1080, "frameRate": 60000, "frameWidth": 1920, "inCurrent": True, "name": "Default Show12",
                  "orientation": 0, "showId": 1717051195073},
                 {"frameHeight": 1920, "frameRate": 60000, "frameWidth": 1080, "inCurrent": False, "name": "Default Show (9:16)",
                  "orientation": 1, "showId": 1717051195074}], "totalCount": 2, "message": "SUCCESS", "status": 0})},
    expect_state={"shows": {"1717051195073": {"name": "Default Show12", "index": 0, "current": True, "orientation": "horizontal",
                                              "frame_rate_x1000": 60000, "width": 1920, "height": 1080},
                            "1717051195074": {"name": "Default Show (9:16)", "index": 1, "current": False, "orientation": "vertical",
                                              "frame_rate_x1000": 60000, "width": 1080, "height": 1920}},
                  "show": {"current_id": 1717051195073, "current_name": "Default Show12"}})
telemetry(MD, "streams", inbound_http={"path": "/V1.0/stream/getStreamServersList", "body": json.dumps({
    "serverList": [{"id": 1717051195496, "isConfigured": False, "name": "YouTube", "type": 1},
                   {"id": 1717051195500, "isConfigured": True, "name": "RTMP Server", "type": 4}],
    "totalCount": 2, "message": "SUCCESS", "status": 0})},
    expect_state={"streams": {"1717051195496": {"name": "YouTube", "index": 1, "configured": False, "type": "youtube"},
                              "1717051195500": {"name": "RTMP Server", "index": 2, "configured": True, "type": "rtmp"}}})
telemetry(MD, "shortcuts", inbound_http={"path": "/V1.0/shortcuts/getConfig", "body": json.dumps({
    "list": [{"functions": [{"id": 311, "name": "PTZ move left"}, {"id": 29, "name": "Wait for 500 ms"}],
              "id": 1725522037805, "name": "Move Left"}], "totalCount": 1, "message": "SUCCESS", "status": 0})},
    expect_state={"shortcuts": {"1725522037805": {"name": "Move Left", "functions": {
        "1": {"id": 311, "name": "PTZ move left"}, "2": {"id": 29, "name": "Wait for 500 ms"}}}}})
telemetry(MD, "replay-events", inbound_http={"path": "/V1.0/replay/getEventList", "body": json.dumps({
    "status": 0, "message": "SUCCESS", "eventList": [
        {"clipSuffixName": "", "customName": "Event 5b", "duration": 5000, "number": 5, "rating": 0, "replayRecordId": "55",
         "tags": ["tag 3", "tag 5"], "teamName": "", "totalSize": 7099937}]})},
    state_before={"replay": {"events": {"54": {"name": "Old"}}}},
    expect_state={"replay": {"events": {"55": {"name": "Event 5b", "number": 5, "duration": 5000, "size": 7099937, "rating": 0.0,
                                               "team": "", "clip_suffix": "", "tags": {"1": "tag 3", "2": "tag 5"}}}}})
telemetry(MD, "replay-tags", inbound_http={"path": "/V1.0/replay/getAllTags", "body": json.dumps({
    "data": ["tag 2", "tag 3"], "message": "SUCCESS", "status": 0})},
    expect_state={"replay": {"tags": {"1": "tag 2", "2": "tag 3"}}})
telemetry(MD, "ptz-devices", inbound_http={"path": "/V1.0/ptz/getDeviceList", "body": json.dumps({
    "status": 0, "message": "SUCCESS", "current": "fcd51e30551e4795879c8bdc8f5397cd",
    "deviceList": [{"id": "fcd51e30551e4795879c8bdc8f5397cd", "isSupportPtz": True, "controlType": 102,
                    "layerName": "PHONE CAMERA - Camera #1"}]})},
    expect_state={"ptz": {"current_id": "fcd51e30551e4795879c8bdc8f5397cd", "devices": {
        "fcd51e30551e4795879c8bdc8f5397cd": {"name": "PHONE CAMERA - Camera #1", "supports_ptz": True, "type": "phone_camera"}}}})
telemetry(MD, "phone-camera", inbound_http={"path": "/V1.0/ptz/getPhoneCameraConfig", "body": json.dumps({
    "status": 0, "message": "SUCCESS", "info": {
        "cameraList": [{"name": "Front-facing", "position": 2, "uniqueId": "com.apple.avfoundation.avcapturedevice.built-in_video:1"}],
        "currentDevice": "com.apple.avfoundation.avcapturedevice.built-in_video:1", "duration": 0,
        "exposureTargetBias": {"currentValue": 2, "maxValue": 2, "minValue": -2, "stepValue": 0.2, "support": True},
        "flashlight": {"enable": False, "support": True},
        "focusLens": {"continuousAuto": False, "currentValue": 0, "maxValue": 1, "minValue": 0, "support": True},
        "id": "faad1f50232f48f4bd63e848d584c1f6", "isRecording": False,
        "iso": {"continuousAuto": False, "currentValue": 34, "maxValue": 1360, "minValue": 34, "support": True},
        "lastErrorCode": 0, "micMute": False, "videoStabilization": {"enable": False, "support": True},
        "whiteBalance": {"currentValue": 3714.533, "maxValue": 60000, "minValue": 2000, "mode": 0, "support": True},
        "clientVersion": "3.1.1", "clientPlatform": "iOS", "name": "Camera #2", "resolution": 1080, "fpsx1000": 30000,
        "videoBitrate": 8192}})},
    expect_state={"phone_camera": {
        "id": "faad1f50232f48f4bd63e848d584c1f6", "name": "Camera #2", "app_version": "3.1.1", "platform": "iOS",
        "resolution": 1080, "frame_rate_x1000": 30000, "video_bitrate": 8192, "recording": False, "recording_duration": 0,
        "last_record_error": "none", "audio_capture": False, "camera": "com.apple.avfoundation.avcapturedevice.built-in_video:1",
        "cameras": {"1": {"id": "com.apple.avfoundation.avcapturedevice.built-in_video:1", "name": "Front-facing", "position": 2}},
        "white_balance": {"supported": True, "mode": "custom", "value": 3714.533, "min": 2000.0, "max": 60000.0},
        "iso": {"supported": True, "auto": False, "value": 34.0, "min": 34.0, "max": 1360.0},
        "exposure_bias": {"supported": True, "value": 2.0, "min": -2.0, "max": 2.0, "step": 0.2},
        "focus": {"supported": True, "auto": False, "value": 0.0, "min": 0.0, "max": 1.0},
        "stabilization": {"supported": True, "enabled": False}, "flashlight": {"supported": True, "enabled": False}}})
telemetry(MD, "audio", inbound_http={"path": "/V1.0/volume/getConfig", "body": json.dumps({
    "audioList": [
        {"audioSourceId": 3, "meter": [-100.0, -100.0], "audioInfo": {
            "beInMultiScenes": 0, "boost": False, "direction": 0, "isGFX": False, "label": "", "muted": 0, "name": "PROGRAM",
            "preview": False, "scope": 0, "solo": 0, "sourceType": 0, "supportAFV": False, "type": 3, "volume": 0, "volumeRecovery": 0}},
        {"audioSourceId": 2048, "meter": [-12.5, -14.0], "audioInfo": {
            "beInMultiScenes": 1, "boost": False, "direction": 0, "isGFX": False, "label": "", "muted": 2, "name": "HDMI 2",
            "preview": False, "scope": 1, "solo": 1, "sourceType": 7, "supportAFV": True, "type": 2048, "uuid": "eco1",
            "volume": -13, "volumeRecovery": 0}}],
    "micAudioDevice": {"audioSourceId": 513, "cardNo": 0, "devNo": 0, "deviceType": 0, "direction": 1, "id": 513, "isMic": True, "sourceType": 0},
    "monitorAudioDevice": {"audioSourceId": 0, "cardNo": 0, "devNo": 0, "deviceType": 0, "direction": 0, "id": 0, "isMic": True, "sourceType": 0},
    "monitorMicSound": True, "message": "SUCCESS", "status": 0})},
    expect_state={"audio": {
        "sources": {
            "3": {"source_id": 3, "name": "PROGRAM", "label": "", "state": "on", "volume": 0.0, "solo": False, "scope": "shared",
                  "supports_afv": False, "in_several_scenes": False, "source_type": 0, "meter": {"left": -100.0, "right": -100.0}},
            "2048": {"source_id": 2048, "name": "HDMI 2", "label": "", "state": "afv", "volume": -13.0, "solo": True, "scope": "own",
                     "supports_afv": True, "in_several_scenes": True, "uuid": "eco1", "source_type": 7,
                     "meter": {"left": -12.5, "right": -14.0}}},
        "monitor_mic": True, "mic_device": {"id": 513, "type": "internal"},
        "monitor_device": {"id": 0, "type": "internal", "card": 0, "device": 0}}})
telemetry(MD, "monitor-devices", inbound_http={"path": "/V1.0/volume/getMonitorDevices", "body": json.dumps({
    "monitorAudioDeivceList": [
        {"audioSourceId": 0, "cardNo": 0, "devNo": 0, "deviceType": 0, "direction": 0, "id": 0, "isMic": True, "sourceType": 0},
        {"audioSourceId": 2, "cardName": "USB Capture HDMI+", "cardNo": 3, "devNo": 0, "deviceType": 1, "direction": 0, "id": 2,
         "isMic": True, "name": "HDMI [USB Capture HDMI+]", "sourceType": 0}], "message": "SUCCESS", "status": 0})},
    expect_state={"audio": {"monitor_devices": {
        "1": {"id": 0, "type": "internal", "card": 0, "device": 0},
        "2": {"id": 2, "type": "external", "card": 3, "device": 0, "name": "HDMI [USB Capture HDMI+]", "card_name": "USB Capture HDMI+"}}}})
telemetry(MD, "output-sources", model="director-plus", inbound_http={"path": "/V1.0/system/getOutSettingsSourceList", "body": json.dumps([
    {"mode": 1}, {"mode": 9, "name": "Phone Camera #1", "uuid": "4d056314a79745aaadebbcc9295e8a7c"}, {"mode": 8, "name": "Chrom", "uuid": "0"}])},
    expect_state={"output_sources": {"1": {"mode": "program"},
                                     "2": {"mode": "loop_custom_source", "name": "Phone Camera #1", "id": "4d056314a79745aaadebbcc9295e8a7c"},
                                     "3": {"mode": "loop_app", "name": "Chrom", "id": "0"}}})
