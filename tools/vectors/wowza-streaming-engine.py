# Wowza Streaming Engine (wowza-streaming-engine): one vector per command, and
# telemetry vectors for the polled resources. The command list comes from the
# spec (itself generated from Wowza's REST API reference); the expected
# request is computed here independently: example values put into the path
# with RFC 3986 encoding, the query in the spec's order, and the JSON body
# written by json.dumps. Reply bodies follow the reference's response schemas.
WZ = "wowza-streaming-engine"
_WZP = "/v2/servers/_defaultServer_/vhosts/_defaultVHost_"


def _wz_vectors() -> None:
    from urllib.parse import quote

    doc = yaml.safe_load((ROOT / "specs" / f"{WZ}.yaml").read_text(encoding="utf-8"))
    examples = {"app": "live", "instance": "_definst_", "stream": "my Stream", "entry": "YouTube 1",
                "recorder": "myStream", "template": "transrate", "encode": "720p", "user": "operator",
                "action": "convert"}
    hand = {"start_recording", "stop_recording", "set_stream_target_enabled"}
    for name, command in doc["commands"].items():
        if name in hand:
            continue
        params = command.get("params") or {}
        values = {}
        for pname, p in params.items():
            if p["type"] == "enum":
                values[pname] = p["values"][-1]
            elif p["type"] == "json":
                values[pname] = {"name": "Item 1", "enabled": True}
            elif p["type"] == "int":
                values[pname] = 50
            else:
                values[pname] = examples.get(pname, "Item 1")
        send = command["send"]
        target = send["path"].replace("{settings.server}", "_defaultServer_").replace("{settings.vhost}", "_defaultVHost_")
        for pname, v in values.items():
            target = target.replace("{" + pname + "}", quote(str(v), safe=""))
        if send.get("query"):
            target += "?" + "&".join(f"{k}={quote(str(values[v.strip('{}')]), safe='')}"
                                     for k, v in send["query"].items())
        request = {"method": send["method"], "target": target}
        if "body" in send:
            request["body"] = json.dumps(values["config"], separators=(",", ":"))
        V.append({"spec": WZ, "command": name, "input": values, "expect_request": request})


_wz_vectors()

# The hand-written commands, and replies.
http(WZ, "start_recording", {"app": "live", "stream": "myStream"}, "POST",
     _WZP + "/applications/live/instances/_definst_/streamrecorders/myStream",
     expect_request={"method": "POST", "target": _WZP + "/applications/live/instances/_definst_/streamrecorders/myStream",
                     "body": '{"recorderName":"myStream","instanceName":"_definst_","defaultRecorder":true}'},
     http_reply={"status": 201, "body": json.dumps({"success": True, "message": "Recorder Created", "data": None})},
     expect_result={"ok": {"kind": "ack"}})
http(WZ, "stop_recording", {"app": "live", "instance": "_definst_", "stream": "myStream"}, "PUT",
     _WZP + "/applications/live/instances/_definst_/streamrecorders/myStream/actions/stopRecording")
http(WZ, "set_stream_target_enabled", {"app": "live", "entry": "YouTube 1", "enabled": False}, "PUT",
     _WZP + "/applications/live/pushpublish/mapentries/YouTube%201/actions/disable")
http(WZ, "set_stream_target_enabled", {"app": "live", "entry": "YouTube 1", "enabled": True}, "PUT",
     _WZP + "/applications/live/pushpublish/mapentries/YouTube%201/actions/enable", file="set_stream_target_enabled-on")
# A refused credential is terminal.
http(WZ, "get_server_status", {}, "GET", "/v2/servers/_defaultServer_/status",
     http_reply={"status": 401, "body": ""}, expect_result={"error": {"error": "auth"}},
     file="get_server_status-refused")

# Telemetry.
telemetry(WZ, "server-status", inbound_http={"path": "/v2/servers/_defaultServer_/status", "body": json.dumps({
    "serverName": "_defaultServer_", "wseName": "Wowza Streaming Engine", "serverVersion": "4.9.0",
    "serverBuild": "4.9.0.01", "serverMode": "standalone", "licenseType": "Subscription",
    "licenseExpireDate": "2027-01-01", "currentConnections": 12, "maxConnections": -1,
    "maxIncommingStreams": -1, "timeRunning": 86400.5, "processorCores": 8, "totalMemory": 16000000000,
    "availableMemory": 9000000000, "transcoderLicensed": True, "transcoderAvailable": True,
    "transcoderLicenses": 2, "transcoderLicensesInUse": 1, "transcoderWatermark": False,
    "gpuAcceleration": "none", "dvrLicensed": True, "dvrInUse": False, "drmLicensed": False, "drmInUse": False,
    "transcoderApplications": ["live"], "dvrApplications": [], "drmApplications": [],
    "javaVersion": "11.0.2", "osName": "Linux"})},
    expect_state={"server": {
        "name": "Wowza Streaming Engine", "version": "4.9.0", "build": "4.9.0.01", "mode": "standalone",
        "license_type": "Subscription", "license_expire_date": "2027-01-01", "current_connections": 12,
        "max_connections": -1, "max_incoming_streams": -1, "time_running": 86400.5, "processor_cores": 8,
        "total_memory": 16000000000, "available_memory": 9000000000, "transcoder_licensed": True,
        "transcoder_available": True, "transcoder_licenses": 2, "transcoder_licenses_in_use": 1,
        "transcoder_watermark": False, "gpu_acceleration": "none", "dvr_licensed": True, "dvr_in_use": False,
        "drm_licensed": False, "drm_in_use": False, "transcoder_applications": '["live"]',
        "dvr_applications": "[]", "drm_applications": "[]", "java_version": "11.0.2", "os_name": "Linux"}})
telemetry(WZ, "machine", inbound_http={"path": "/v2/machine/monitoring/current", "body": json.dumps({
    "serverName": "_defaultServer_", "cpuUser": 12, "cpuSystem": 3, "cpuIdle": 85, "memoryUsed": 7000000000,
    "memoryFree": 9000000000, "heapUsed": 400000000, "heapFree": 600000000, "diskUsed": 20000000000,
    "diskFree": 80000000000, "connectionCount": 14, "serverUptime": 86400})},
    expect_state={"machine": {"cpu_user": 12, "cpu_system": 3, "cpu_idle": 85, "memory_used": 7000000000,
                              "memory_free": 9000000000, "heap_used": 400000000, "heap_free": 600000000,
                              "disk_used": 20000000000, "disk_free": 80000000000, "connections": 14,
                              "uptime": 86400}})
telemetry(WZ, "vhost", inbound_http={"path": _WZP + "/monitoring/current", "body": json.dumps({
    "totalConnections": 14, "bytesIn": 1000, "bytesOut": 5000, "uptime": 3600})},
    expect_state={"vhost": {"connections": 14, "bytes_in": 1000, "bytes_out": 5000, "uptime": 3600}})
telemetry(WZ, "applications", inbound_http={"path": _WZP + "/applications", "body": json.dumps({
    "serverName": "_defaultServer_", "applications": [
        {"id": "live", "href": "/v2/servers/_defaultServer_/vhosts/_defaultVHost_/applications/live",
         "appType": "Live", "drmEnabled": False, "dvrEnabled": True, "transcoderEnabled": True,
         "streamTargetsEnabled": True}]})},
    expect_state={"applications": {"live": {"app_type": "Live", "drm_enabled": False, "dvr_enabled": True,
                                            "transcoder_enabled": True, "stream_targets_enabled": True}}},
    expect_then_send=[{"method": "GET", "target": _WZP + "/applications/live/instances"},
                      {"method": "GET", "target": _WZP + "/applications/live/pushpublish/mapentries"},
                      {"method": "GET", "target": _WZP + "/applications/live/monitoring/current"}])
telemetry(WZ, "application-stats", inbound_http={"path": _WZP + "/applications/live/monitoring/current", "body": json.dumps({
    "totalConnections": 9, "bytesIn": 100000, "bytesOut": 900000, "bytesInRate": 600000, "bytesOutRate": 5400000,
    "uptime": 1800})},
    expect_state={"applications": {"live": {"connections": 9, "bytes_in": 100000, "bytes_out": 900000,
                                            "bytes_in_rate": 600000, "bytes_out_rate": 5400000, "uptime": 1800}}})
telemetry(WZ, "instances", inbound_http={"path": _WZP + "/applications/live/instances", "body": json.dumps({
    "serverName": "_defaultServer_", "instanceList": [{
        "name": "_definst_",
        "incomingStreams": [{"name": "my Stream", "sourceIp": "rtmp://192.168.1.20:52000", "isConnected": True,
                             "isRecordingSet": True, "isPublishedToVOD": False, "isPTZEnabled": False,
                             "isStreamManagerStream": False, "applicationInstance": "_definst_"}],
        "outgoingStreams": [],
        "recorders": [{"recorderName": "my Stream", "recorderState": "Recording in progress",
                       "currentFile": "/usr/local/WowzaStreamingEngine/content/my Stream.mp4",
                       "currentSize": 1048576, "currentDuration": 60000, "recordingStartTime": "2026-10-28-10:00:00",
                       "outputPath": "/usr/local/WowzaStreamingEngine/content", "fileFormat": "MP4",
                       "segmentationType": "None", "recorderErrorString": ""}],
        "streamGroups": [{"groupName": "my Stream_all", "members": ["my Stream_720p", "my Stream_360p"],
                          "isTranscodeResult": True}]}]})},
    state_before={"applications": {"live": {"instances": {"_definst_": {"incoming_streams": {
        "old": {"connected": True}}}}}}},
    expect_state={"applications": {"live": {"instances": {"_definst_": {
        "incoming_streams": {"my Stream": {"connected": True, "source_ip": "rtmp://192.168.1.20:52000",
                                           "recording": True, "published_to_vod": False, "ptz_enabled": False,
                                           "stream_manager": False}},
        "recorders": {"my Stream": {"state": "Recording in progress",
                                    "current_file": "/usr/local/WowzaStreamingEngine/content/my Stream.mp4",
                                    "current_size": 1048576, "current_duration": 60000,
                                    "start_time": "2026-10-28-10:00:00",
                                    "output_path": "/usr/local/WowzaStreamingEngine/content",
                                    "file_format": "MP4", "segmentation_type": "None", "error": ""}},
        "stream_groups": {"my Stream_all": {"members": '["my Stream_720p","my Stream_360p"]',
                                            "transcode_result": True}}}}}}},
    expect_then_send=[{"method": "GET", "target": _WZP + "/applications/live/instances/_definst_/incomingstreams/my%20Stream/monitoring/current"}])
telemetry(WZ, "stream-stats", inbound_http={
    "path": _WZP + "/applications/live/instances/_definst_/incomingstreams/my%20Stream/monitoring/current",
    "body": json.dumps({"name": "my Stream", "totalConnections": 7, "bytesIn": 5000, "bytesOut": 35000,
                        "bytesInRate": 500000, "bytesOutRate": 3500000, "uptime": 120})},
    expect_state={"applications": {"live": {"instances": {"_definst_": {"stream_stats": {"my Stream": {
        "connections": 7, "bytes_in": 5000, "bytes_out": 35000, "bytes_in_rate": 500000,
        "bytes_out_rate": 3500000, "uptime": 120}}}}}}})
telemetry(WZ, "recorders", inbound_http={
    "path": _WZP + "/applications/live/instances/_definst_/streamrecorders",
    "body": json.dumps({"instanceName": "_definst_", "streamrecorder": [
        {"recorderName": "myStream", "recorderState": "Waiting for stream", "currentFile": "", "currentSize": 0,
         "currentDuration": 0, "recordingStartTime": "", "outputPath": "/content", "fileFormat": "MP4",
         "segmentationType": "SegmentByDuration", "recorderErrorString": ""}]})},
    expect_state={"applications": {"live": {"instances": {"_definst_": {"recorders": {"myStream": {
        "state": "Waiting for stream", "current_file": "", "current_size": 0, "current_duration": 0,
        "start_time": "", "output_path": "/content", "file_format": "MP4",
        "segmentation_type": "SegmentByDuration", "error": ""}}}}}}})
telemetry(WZ, "stream-targets", inbound_http={"path": _WZP + "/applications/live/pushpublish/mapentries", "body": json.dumps({
    "serverName": "_defaultServer_", "mapEntries": [
        {"entryName": "YouTube 1", "enabled": True, "profile": "rtmp", "sourceStreamName": "my Stream",
         "host": "a.rtmp.youtube.com", "port": 1935, "application": "live2", "streamName": "abcd-1234",
         "password": "secret", "sessionStatus": "Active", "sessionFailureReason": ""}]})},
    expect_state={"applications": {"live": {"stream_targets": {"YouTube 1": {
        "enabled": True, "profile": "rtmp", "source_stream": "my Stream", "host": "a.rtmp.youtube.com",
        "port": 1935, "application": "live2", "stream_name": "abcd-1234", "session_status": "Active",
        "failure_reason": ""}}}}})
telemetry(WZ, "stream-target-changed", inbound_http={
    "path": _WZP + "/applications/live/pushpublish/mapentries/YouTube%201/actions/disable",
    "body": json.dumps({"success": True, "message": "", "data": None})},
    expect_state={},
    expect_then_send=[{"method": "GET", "target": _WZP + "/applications/live/pushpublish/mapentries"}])
telemetry(WZ, "recorder-changed", inbound_http={
    "path": _WZP + "/applications/live/instances/_definst_/streamrecorders/myStream/actions/stopRecording",
    "body": json.dumps({"success": True, "message": "", "data": None})},
    expect_state={},
    expect_then_send=[{"method": "GET", "target": _WZP + "/applications/live/instances/_definst_/streamrecorders"}])
telemetry(WZ, "startup-streams", inbound_http={"path": _WZP + "/startupstreams", "body": json.dumps({
    "serverName": "_defaultServer_", "Streams": [
        {"appName": "live", "instance": "_definst_", "streamName": "camera1.stream", "mediaCasterType": "rtp"}]})},
    expect_state={"startup_streams": {"live": {"_definst_": {"camera1.stream": {"media_caster_type": "rtp"}}}}})
