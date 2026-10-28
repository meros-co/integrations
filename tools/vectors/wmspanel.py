# WMSPanel (wmspanel): one vector per command, and telemetry vectors for the
# polled resources. The command list comes from the spec (written from the
# WMSPanel API reference); the expected request is computed here
# independently: example values put into the path, the query in the spec's
# order with client_id and api_key first, and the JSON body written by
# json.dumps. Reply bodies follow the reference's response examples.
WM = "wmspanel"
_WMS = {"client_id": "cid1000", "api_key": "key9999", "server_id": "538bcbd3f5aef0bed900000d"}
_WMSRV = "538bcbd3f5aef0bed900000d"
_WMQ = "client_id=cid1000&api_key=key9999"


def _wm_vectors() -> None:
    from urllib.parse import quote

    doc = yaml.safe_load((ROOT / "specs" / f"{WM}.yaml").read_text(encoding="utf-8"))
    examples = {"server_id": _WMSRV, "data_slice": "53e03ca678a6fc8599000001", "server": _WMSRV, "from": "2014-01-01",
                "to": "2014-12-31", "stream": 957567282, "license": "LIC-1234", "data_slices": ["5a5f0856fe7cd34f1f000006"],
                "streams": [-325053721, 1439361443]}
    for name, command in doc["commands"].items():
        params = command.get("params") or {}
        values = {}
        for pname, p in params.items():
            if pname in examples and not (pname == "stream" and p["type"] == "string"):
                values[pname] = examples[pname]
            elif p["type"] == "enum":
                values[pname] = p.get("default", p["values"][-1])
            elif p["type"] == "json":
                values[pname] = {"description": "Main", "paused": False}
            elif p["type"] == "int":
                values[pname] = p.get("default", 5)
            elif p["type"] == "bool":
                values[pname] = True
            elif pname == "stream":
                values[pname] = "live"
            elif pname in ("from", "to") and "T" in p.get("pattern", ""):
                values[pname] = "2021-06-20T17:45"
            else:
                values[pname] = "5458718978a6fcf15a000017"
        if "from" in values and "T" in params["from"].get("pattern", ""):
            values["from"], values["to"] = "2021-06-20T17:45", "2021-06-21T18:15"
        send = command["send"]
        target = send["path"]
        for pname, v in values.items():
            target = target.replace("{" + pname + "}", quote(str(v), safe=""))
        parts = []
        for k, v in send["query"].items():
            if v.startswith("{settings."):
                v = _WMS[v[len("{settings."):-1]]
            elif v.startswith("{"):
                raw = values[v[1:-1]]
                v = ("true" if raw else "false") if isinstance(raw, bool) else str(raw)
            parts.append(f"{k}={quote(v, safe='')}")
        target += "?" + "&".join(parts)
        request = {"method": send["method"], "target": target}
        if "body" in send:
            if send["body"] == "{body}":
                request["body"] = json.dumps(values["body"], separators=(",", ":"))
            elif "streams" in values:
                request["body"] = json.dumps({"streams": values["streams"]}, separators=(",", ":"))
            else:
                request["body"] = json.dumps({"data_slices": values["data_slices"]}, separators=(",", ":"))
        V.append({"spec": WM, "command": name, "input": values, "settings": _WMS, "expect_request": request})


_wm_vectors()
# A reply.
V.append({"spec": WM, "command": "pause_transcoder", "input": {"transcoder_id": "570df4d9fe7cd3b15f000001"}, "settings": _WMS,
          "expect_request": {"method": "GET", "target": f"/v1/transcoder/570df4d9fe7cd3b15f000001/pause?{_WMQ}"},
          "http_reply": {"status": 200, "body": '{"status":"Ok"}'},
          "expect_result": {"ok": {"kind": "value", "value": {"status": "Ok"}}}, "file": "pause_transcoder-reply"})

# Telemetry.
telemetry(WM, "servers", settings=_WMS, inbound_http={"path": f"/v1/server?{_WMQ}", "body": json.dumps({
    "status": "Ok", "servers": [
        {"id": "05e17aea6f7687aa0341aedb", "name": "WIN-PE1MBBSAUHE", "kind": "Nimble",
         "ip": ["169.168.48.213"], "status": "online", "tags": ["MyTag1"]},
        {"id": "3bb85dc9691e0bd8e9a98087", "name": "centos64-64", "kind": "Wowza", "status": "pending",
         "ip": ["192.168.254.105"]}]})},
    expect_state={"servers": {
        "05e17aea6f7687aa0341aedb": {"name": "WIN-PE1MBBSAUHE", "kind": "Nimble", "status": "online", "ip": '["169.168.48.213"]'},
        "3bb85dc9691e0bd8e9a98087": {"name": "centos64-64", "kind": "Wowza", "status": "pending", "ip": '["192.168.254.105"]'}}})
telemetry(WM, "server", settings=_WMS, inbound_http={"path": f"/v1/server/{_WMSRV}?{_WMQ}", "body": json.dumps({
    "status": "Ok", "server": {"id": _WMSRV, "name": "edge-centos", "kind": "Nimble", "status": "offline", "ip": ["192.168.36.1"]}})},
    expect_state={"servers": {_WMSRV: {"name": "edge-centos", "kind": "Nimble", "status": "offline", "ip": '["192.168.36.1"]'}}})
telemetry(WM, "live-streams", settings=_WMS, inbound_http={"path": f"/v1/server/{_WMSRV}/live/streams?{_WMQ}", "body": json.dumps({
    "status": "Ok", "streams": [
        {"id": "582bf11682c8c4f1946031bf", "application": "cartoons", "stream": "1567", "status": "online", "description": "",
         "protocol": "RTMP", "video_codec": "avc1.64001e", "audio_codec": "mp4a.40.2", "bandwidth": 247804,
         "resolution": "720x304", "publish_time": 1578899697, "tags": ["Cartoons"]},
        {"id": "57ad54f74a0f92fdeea49e96", "application": "nimble_live", "stream": "sportbox", "status": "offline",
         "description": "First stream", "protocol": "MPEGTS", "video_codec": "", "audio_codec": "", "bandwidth": 0,
         "resolution": "", "publisher_ip": "127.0.0.1", "publish_time": 1578899689}]})},
    expect_state={"live_streams": {
        "582bf11682c8c4f1946031bf": {"application": "cartoons", "stream": "1567", "status": "online", "description": "",
                                     "protocol": "RTMP", "video_codec": "avc1.64001e", "audio_codec": "mp4a.40.2",
                                     "bandwidth": 247804, "resolution": "720x304", "publish_time": 1578899697},
        "57ad54f74a0f92fdeea49e96": {"application": "nimble_live", "stream": "sportbox", "status": "offline",
                                     "description": "First stream", "protocol": "MPEGTS", "video_codec": "", "audio_codec": "",
                                     "bandwidth": 0, "resolution": "", "publisher_ip": "127.0.0.1",
                                     "publish_time": 1578899689}}})
telemetry(WM, "live-stream", settings=_WMS, inbound_http={
    "path": f"/v1/server/{_WMSRV}/live/streams/582bf11682c8c4f1946031bf?{_WMQ}", "body": json.dumps({
        "status": "Ok", "stream": {"id": "582bf11682c8c4f1946031bf", "application": "cartoons", "stream": "1567",
                                   "status": "offline", "protocol": "RTMP", "bandwidth": 0}})},
    expect_state={"live_streams": {"582bf11682c8c4f1946031bf": {"application": "cartoons", "stream": "1567", "status": "offline",
                                                                "protocol": "RTMP", "bandwidth": 0}}})
_WMRULE = {"id": "5458718978a6fcf15a000017", "src_app": "nimble-app", "src_strm": "stream1", "dest_addr": "ingest.mybeststreams.com",
           "dest_port": 1935, "dest_app": "live", "dest_strm": "stream2?key=mhhYOApVGGKKqXefE", "auth_schema": "NIMBLE",
           "dest_login": "wmspanel", "dest_password": "password", "description": "Ingestion", "paused": False, "ssl": False}
_WMRULESTATE = {"source": "nimble-app/stream1", "destination": "ingest.mybeststreams.com:1935/live/stream2?key=mhhYOApVGGKKqXefE",
                "auth_schema": "NIMBLE", "ssl": False, "paused": False, "description": "Ingestion"}
telemetry(WM, "rtmp-republish", settings=_WMS, inbound_http={"path": f"/v1/server/{_WMSRV}/rtmp/republish?{_WMQ}",
                                                            "body": json.dumps({"status": "Ok", "rules": [_WMRULE]})},
          expect_state={"rtmp_republish": {"5458718978a6fcf15a000017": _WMRULESTATE}})
telemetry(WM, "rtmp-republish-rule", settings=_WMS, inbound_http={
    "path": f"/v1/server/{_WMSRV}/rtmp/republish/5458718978a6fcf15a000017?{_WMQ}",
    "body": json.dumps({"status": "Ok", "rule": [dict(_WMRULE, paused=True)]})},
    expect_state={"rtmp_republish": {"5458718978a6fcf15a000017": dict(_WMRULESTATE, paused=True)}})
telemetry(WM, "mpegts-incoming", settings=_WMS, inbound_http={"path": f"/v1/server/{_WMSRV}/mpegts/incoming?{_WMQ}", "body": json.dumps({
    "status": "Ok", "streams": [
        {"id": "5747cf450823891317000001", "status": "online", "protocol": "http", "probe_interval": 600,
         "url": "http://84.141.37.68:2233/udp/223.68.114.183:44444", "name": "NSGTRK",
         "active_url": "http://84.141.37.70:8081/hls/fb/playlist.m3u8", "description": "TV Channel #1", "bandwidth": 32699},
        {"id": "5748307e0823891558000001", "status": "pending", "protocol": "udp", "ip": "224.168.1.2", "port": 2222,
         "description": "Meeting room main camera"}]})},
    expect_state={"mpegts_incoming": {
        "5747cf450823891317000001": {"name": "NSGTRK", "status": "online", "protocol": "http",
                                     "url": "http://84.141.37.68:2233/udp/223.68.114.183:44444",
                                     "active_url": "http://84.141.37.70:8081/hls/fb/playlist.m3u8", "bandwidth": 32699,
                                     "description": "TV Channel #1"},
        "5748307e0823891558000001": {"status": "pending", "protocol": "udp", "ip": "224.168.1.2", "port": 2222,
                                     "description": "Meeting room main camera"}}})
telemetry(WM, "mpegts-outgoing", settings=_WMS, inbound_http={"path": f"/v1/server/{_WMSRV}/mpegts/outgoing?{_WMQ}", "body": json.dumps({
    "status": "Ok", "streams": [{"id": "574be1c2082389251f000001", "status": "synced", "application": "nimble_live",
                                 "stream": "nsgtv", "description": "World of Tanks", "paused": False}]})},
    expect_state={"mpegts_outgoing": {"574be1c2082389251f000001": {"application": "nimble_live", "stream": "nsgtv",
                                                                   "status": "synced", "paused": False,
                                                                   "description": "World of Tanks"}}})
telemetry(WM, "transcoders", settings=_WMS, inbound_http={"path": f"/v1/transcoder?{_WMQ}&server_id={_WMSRV}", "body": json.dumps({
    "status": "Ok", "transcoders": [{"id": "570df4d9fe7cd3b15f000001", "name": "Main transcoder", "description": "This is main transcoder",
                                     "paused": False, "server_id": _WMSRV, "out_of_process": True}]})},
    expect_state={"transcoders": {"570df4d9fe7cd3b15f000001": {"name": "Main transcoder", "description": "This is main transcoder",
                                                               "paused": False, "out_of_process": True}}})
telemetry(WM, "tasks", settings=_WMS, inbound_http={"path": f"/v1/server/{_WMSRV}/tasks?{_WMQ}", "body": json.dumps({
    "status": "Ok", "tasks": [{"id": "542d024e78a6fc4ab4000052",
                               "command": "ffmpeg -re -i test/content/mp4/sample.mp4 -c copy -f flv rtmp://127.0.0.1:1999/live/stream",
                               "alias": "Stream to 1999 port", "status": "failed", "autorestart": True, "info": "exit code 1"}]})},
    expect_state={"tasks": {"542d024e78a6fc4ab4000052": {
        "alias": "Stream to 1999 port", "status": "failed", "autorestart": True, "info": "exit code 1",
        "command": "ffmpeg -re -i test/content/mp4/sample.mp4 -c copy -f flv rtmp://127.0.0.1:1999/live/stream"}}})
telemetry(WM, "task-started", settings=_WMS, inbound_http={
    "path": f"/v1/server/{_WMSRV}/tasks/542d024e78a6fc4ab4000052/start?{_WMQ}", "body": '{"status":"Ok"}'},
    expect_state={},
    expect_then_send=[{"method": "GET", "target": f"/v1/server/{_WMSRV}/tasks?{_WMQ}"}])
telemetry(WM, "transcoder-paused", settings=_WMS, inbound_http={
    "path": f"/v1/transcoder/570df4d9fe7cd3b15f000001/pause?{_WMQ}", "body": '{"status":"Ok"}'},
    expect_state={},
    expect_then_send=[{"method": "GET", "target": f"/v1/transcoder?{_WMQ}&server_id={_WMSRV}"}])
telemetry(WM, "mpegts-incoming-paused", settings=_WMS, inbound_http={
    "path": f"/v1/server/{_WMSRV}/mpegts/incoming/5747cf450823891317000001/pause?{_WMQ}&apply_out=1", "body": '{"status":"Ok"}'},
    expect_state={},
    expect_then_send=[{"method": "GET", "target": f"/v1/server/{_WMSRV}/mpegts/incoming?{_WMQ}"}])
