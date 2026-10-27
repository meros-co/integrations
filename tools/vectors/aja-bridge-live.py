B = "aja-bridge-live"
# AJA BRIDGE LIVE REST API (BRIDGE LIVE v1.16 REST API Documentation,
# 2024-09-04): JSON under /api with HTTP Basic credentials. Requests, bodies
# and replies follow the document's examples, shortened where they repeat.

_BL_AUTH = {"password": "password"}

_BL_PIPELINE = ('{"name":"Pipeline 3","inputs":[{"id":0,"source":{"address":{"url":"rtp://0.0.0.0:5003:lo"},'
                '"streamType":"MPEG_TS","complianceWith":"AUTO"},"video":{"format":"J2K","width":1920,'
                '"height":1080,"sampling":"4:2:2","depth":10,"scanRate":"24","scan":"PROGRESSIVE"}}]}')

# All pipelines.
http(B, "list_pipelines", {}, "GET", "/api/pipelines", settings=_BL_AUTH,
     http_reply={"status": 200, "body": '[{"version":"0.16","id":0,"name":"Pipeline 1","runningState":"INACTIVE"}]'},
     expect_result={"ok": {"kind": "value", "value": [
         {"version": "0.16", "id": 0, "name": "Pipeline 1", "runningState": "INACTIVE"}]}})
http(B, "list_pipelines", {}, "GET", "/api/pipelines", file="list_pipelines.refused",
     http_reply={"status": 401, "body": ""},
     expect_result={"error": {"error": "auth"}})
V.append({"spec": B, "command": "replace_pipelines", "settings": _BL_AUTH,
          "input": {"pipelines": [{"id": 1, "name": "Pipeline 2", "inputs": [
              {"id": 0, "source": {"address": {"url": "rtp://0.0.0.0:5002:lo"}, "streamType": "MPEG_TS",
                                   "complianceWith": "AUTO"}}]}]},
          "expect_request": {"method": "POST", "target": "/api/pipelines", "content_type": "application/json",
                             "body": '[{"id":1,"name":"Pipeline 2","inputs":[{"id":0,"source":{"address":'
                                     '{"url":"rtp://0.0.0.0:5002:lo"},"streamType":"MPEG_TS",'
                                     '"complianceWith":"AUTO"}}]}]'}})

# One pipeline.
import json as _bl_json
V.append({"spec": B, "command": "create_pipeline", "settings": _BL_AUTH,
          "input": {"pipeline": _bl_json.loads(_BL_PIPELINE)},
          "expect_request": {"method": "POST", "target": "/api/pipeline", "content_type": "application/json",
                             "body": _BL_PIPELINE},
          "http_reply": {"status": 201, "body": ""},
          "expect_result": {"ok": {"kind": "ack"}}})
# The document's error example 2: a missing property.
V.append({"spec": B, "command": "create_pipeline", "settings": _BL_AUTH, "file": "create_pipeline.invalid",
          "input": {"pipeline": {"source": {}}},
          "expect_request": {"method": "POST", "target": "/api/pipeline", "content_type": "application/json",
                             "body": '{"source":{}}'},
          "http_reply": {"status": 400, "body": '{"status":400,"error":"Missing required property \'address\'. '
                                                '(in: \'{}.inputs.0.source\')"}'},
          "expect_result": {"error": {"error": "device_error", "code": "400"}}})
http(B, "get_pipeline", {"id": 0}, "GET", "/api/pipeline/0", settings=_BL_AUTH,
     http_reply={"status": 200, "body": '{"version":"0.16","id":0,"name":"Pipeline 1","runningState":"ACTIVE"}'},
     expect_result={"ok": {"kind": "value", "value": {
         "version": "0.16", "id": 0, "name": "Pipeline 1", "runningState": "ACTIVE"}}})
V.append({"spec": B, "command": "modify_pipeline", "settings": _BL_AUTH,
          "input": {"id": 1, "pipeline": {"name": "Pipeline 2 Changed"}},
          "expect_request": {"method": "PUT", "target": "/api/pipeline/1", "content_type": "application/json",
                             "body": '{"name":"Pipeline 2 Changed"}'}})
http(B, "start_pipeline", {"id": 1}, "PUT", "/api/pipeline/1/start", settings=_BL_AUTH,
     http_reply={"status": 503, "body": "Failed to start the pipeline, try again later."},
     expect_result={"error": {"error": "device_error", "code": "503"}})
http(B, "stop_pipeline", {"id": 1}, "PUT", "/api/pipeline/1/stop", settings=_BL_AUTH,
     http_reply={"status": 200, "body": ""}, expect_result={"ok": {"kind": "ack"}})
http(B, "delete_pipeline", {"id": 7}, "DELETE", "/api/pipeline/7", settings=_BL_AUTH,
     http_reply={"status": 404, "body": '{"status":404,"error":"Pipeline with id = 7 doesn\'t exist."}'},
     expect_result={"error": {"error": "device_error", "code": "404"}})
http(B, "get_pipeline_state", {"id": 0}, "GET", "/api/pipeline/0/state", settings=_BL_AUTH)

# System.
http(B, "get_version", {}, "GET", "/api/system/version", settings=_BL_AUTH)
http(B, "list_network_interfaces", {}, "GET", "/api/system/network-interfaces", settings=_BL_AUTH)
http(B, "list_sdi_devices", {}, "GET", "/api/system/sdi-devices", settings=_BL_AUTH)
http(B, "list_ndi_sources", {}, "GET", "/api/system/ndi-devices?ndiGroup=public", settings=_BL_AUTH)
http(B, "list_ndi_sources", {"groups": "cameras,studio1,10am show"}, "GET",
     "/api/system/ndi-devices?ndiGroup=cameras%2Cstudio1%2C10am%20show", settings=_BL_AUTH,
     file="list_ndi_sources.groups")
http(B, "reinstall", {}, "GET", "/api/system/reinstall", settings=_BL_AUTH,
     http_reply={"status": 200, "body": '{"state":"SUCCESS","errorMessage":""}'},
     expect_result={"ok": {"kind": "value", "value": "SUCCESS"}})
http(B, "get_reinstall_state", {}, "GET", "/api/system/reinstall-state", settings=_BL_AUTH)
http(B, "factory_reset", {}, "GET", "/api/system/factory-reset", settings=_BL_AUTH)
http(B, "get_factory_reset_state", {}, "GET", "/api/system/factory-reset-state", settings=_BL_AUTH,
     http_reply={"status": 200, "body": '{"state":"IN-PROGRESS","errorMessage":""}'},
     expect_result={"ok": {"kind": "value", "value": "IN-PROGRESS"}})
http(B, "restart_service", {}, "GET", "/api/system/restart-service", settings=_BL_AUTH,
     http_reply={"status": 200, "body": '{"message":"Main program service will be restarted in a while."}'},
     expect_result={"ok": {"kind": "value", "value": "Main program service will be restarted in a while."}})
http(B, "reboot", {}, "GET", "/api/system/reboot", settings=_BL_AUTH)
http(B, "shutdown", {}, "GET", "/api/system/shutdown", settings=_BL_AUTH,
     http_reply={"status": 200, "body": '{"message":"The machine will be shutdown in a while."}'},
     expect_result={"ok": {"kind": "value", "value": "The machine will be shutdown in a while."}})
http(B, "get_version", {}, "GET", "/api/system/version", file="get_version.http-8080",
     settings={"password": "password", "scheme": "http"})

# Telemetry: the pipeline list, then each listed pipeline's state is read.
_BL_LIST = ('[{"version":"0.16","id":0,"name":"Pipeline 1","runningState":"ACTIVE",'
            '"inputs":[{"id":0,"source":{"streamType":"MPEG_TS","address":{"url":"srt://127.0.0.1:5001:lo"}},'
            '"video":{"format":"J2K","width":1920,"height":1080,"scanRate":"50","scan":"INTERLACED"}}],'
            '"outputs":[{"streamType":"MPEG_TS","id":0,"name":"Output Stream 1",'
            '"sink":{"address":{"url":"srt://0.0.0.0:6001:lo"}},"ts":{"bitrate":11000000},'
            '"video":{"format":"H264","width":720,"height":576,"bitrate":8000000}}]},'
            '{"id":1,"name":"Pipeline 2","runningState":"INACTIVE","inputs":[],"outputs":[]}]')
telemetry(B, "pipelines", settings=_BL_AUTH, inbound_http={"path": "/api/pipelines", "body": _BL_LIST},
          expect_state={"pipelines": {
              "0": {"name": "Pipeline 1", "running_state": "ACTIVE", "active": True, "schema_version": "0.16",
                    "inputs": {"0": {"url": "srt://127.0.0.1:5001:lo", "stream_type": "MPEG_TS",
                                     "video_format": "J2K", "width": 1920, "height": 1080, "scan_rate": "50",
                                     "scan": "INTERLACED"}},
                    "outputs": {"0": {"name": "Output Stream 1", "url": "srt://0.0.0.0:6001:lo",
                                      "stream_type": "MPEG_TS", "video_format": "H264", "width": 720,
                                      "height": 576, "video_bitrate": 8000000, "ts_bitrate": 11000000}}},
              "1": {"name": "Pipeline 2", "running_state": "INACTIVE", "active": False}}},
          expect_then_send=[{"method": "GET", "target": "/api/pipeline/0/state"},
                            {"method": "GET", "target": "/api/pipeline/1/state"}])
# A pipeline gone from the list leaves state.
telemetry(B, "pipelines-shrink", settings=_BL_AUTH,
          state_before={"pipelines": {"0": {"name": "Pipeline 1"}, "1": {"name": "Pipeline 2"}}},
          inbound_http={"path": "/api/pipelines", "body": '[{"id":1,"name":"Pipeline 2","runningState":"INACTIVE"}]'},
          expect_state={"pipelines": {"1": {"name": "Pipeline 2", "running_state": "INACTIVE", "active": False}}},
          expect_then_send=[{"method": "GET", "target": "/api/pipeline/1/state"}])
# The document's Pipeline state example, shortened.
_BL_STATE = ('{"id":0,"status":"ERROR","operationState":"STARTED","licenseStatus":"GRANTED",'
             '"licenseExpirationDate":"1970-01-01T00:00:00Z","licenseRemainingTime":"00:00:00","name":"Pipeline 1",'
             '"uniqueId":"577b54da-5dfe-4bea-add6-b689e8cd7eea",'
             '"inputs":[{"id":0,"status":"ERROR","name":"rtp://0.0.0.0:5000:lo",'
             '"description":"JPEG2000 1920x1080 4:4:4 10-bit","elementaryStreams":[{"id":0,"pid":502,"type":"VIDEO",'
             '"state":"NEW","format":"J2K","elementaryStream":{"streamDescription":"Video Stream (pid: 502)"}}],'
             '"decodedFps":0.0,"decodingLatency":0.0,"invalidFps":0.0,'
             '"messages":[{"severity":"ERROR","message":"rtp://0.0.0.0:5000:lo: input stream #0 error."}]}],'
             '"outputs":[{"id":0,"uniqueId":"ddc2b880-d32c-4bae-afd1-b4c5b2b75d19","status":"ERROR",'
             '"name":"Output Stream 1","sink":"rtp://127.0.0.1:50001","description":"H.264 1920x1080 4:2:0 8-bit",'
             '"encodingLatency":0.0,"transcodedFps":25.0,"transcodingLatency":40.5,"droppedFps":0.0,'
             '"messages":[],"latencyAddition":0}]}')
telemetry(B, "pipeline-state", settings=_BL_AUTH, inbound_http={"path": "/api/pipeline/0/state", "body": _BL_STATE},
          expect_state={"runtime": {"0": {
              "status": "ERROR", "operation_state": "STARTED", "license_status": "GRANTED",
              "license_expiration_date": "1970-01-01T00:00:00Z", "license_remaining_time": "00:00:00",
              "unique_id": "577b54da-5dfe-4bea-add6-b689e8cd7eea",
              "inputs": {"0": {"status": "ERROR", "name": "rtp://0.0.0.0:5000:lo",
                               "description": "JPEG2000 1920x1080 4:4:4 10-bit", "decoded_fps": 0.0,
                               "decoding_latency_ms": 0.0, "invalid_fps": 0.0,
                               "messages": '[{"severity":"ERROR",'
                                           '"message":"rtp://0.0.0.0:5000:lo: input stream #0 error."}]',
                               "streams": {"0": {"type": "VIDEO", "pid": 502, "state": "NEW", "format": "J2K",
                                                 "description": "Video Stream (pid: 502)"}}}},
              "outputs": {"0": {"unique_id": "ddc2b880-d32c-4bae-afd1-b4c5b2b75d19", "status": "ERROR",
                                "name": "Output Stream 1", "sink": "rtp://127.0.0.1:50001",
                                "description": "H.264 1920x1080 4:2:0 8-bit", "encoding_latency_ms": 0.0,
                                "transcoded_fps": 25.0, "transcoding_latency_ms": 40.5, "dropped_fps": 0.0,
                                "latency_addition_ms": 0, "messages": "[]"}}}}})
telemetry(B, "start-reply", settings=_BL_AUTH, inbound_http={"path": "/api/pipeline/1/start", "body": ""},
          expect_state={},
          expect_then_send=[{"method": "GET", "target": "/api/pipeline/1/state"},
                            {"method": "GET", "target": "/api/pipelines"}])
telemetry(B, "removed", settings=_BL_AUTH,
          state_before={"runtime": {"1": {"status": "OK"}}},
          inbound_http={"path": "/api/pipeline/1", "body": ""},
          expect_state={"runtime": {}},
          expect_then_send=[{"method": "GET", "target": "/api/pipelines"},
                            {"method": "GET", "target": "/api/pipeline/1/state"}])
telemetry(B, "created", settings=_BL_AUTH, inbound_http={"path": "/api/pipeline", "body": ""},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/api/pipelines"}])
telemetry(B, "replaced", settings=_BL_AUTH, inbound_http={"path": "/api/pipelines", "body": ""},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/api/pipelines"}])
telemetry(B, "version", settings=_BL_AUTH, inbound_http={"path": "/api/system/version", "body":
    '{"transcoder":{"version":"1.0.0","stage":"pipelines/38649","date":"2024-09-04","hash":"7814d7e",'
    '"fullVersionName":"1.0.0-pipelines/38649 2024-09-04 7814d7e","deploymentMode":"host"},'
    '"j2kDec":{"version":"2.5.10"},"j2kEnc":{"version":"2.5.10"},"j2kTrc":{"version":"2.5.10"},'
    '"nvidiaDriver":"384.98","cudaDriver":"9.0","cudaRuntime":"9.0"}'},
    expect_state={"system": {"transcoder_version": "1.0.0",
                             "transcoder_full_version": "1.0.0-pipelines/38649 2024-09-04 7814d7e",
                             "transcoder_date": "2024-09-04", "deployment_mode": "host",
                             "j2k_decoder_version": "2.5.10", "j2k_encoder_version": "2.5.10",
                             "j2k_transcoder_version": "2.5.10", "nvidia_driver": "384.98",
                             "cuda_driver": "9.0", "cuda_runtime": "9.0"}})
telemetry(B, "network-interfaces", settings=_BL_AUTH, inbound_http={
    "path": "/api/system/network-interfaces",
    "body": '[{"id":"enp4s0f0","name":"Ethernet card 1 (enp4s0f0, 1 Gb/s link)","connected":true},'
            '{"id":"lo","name":"Loopback (lo)","connected":true},{"id":"ens5","name":"Ethernet card 3 (ens5)",'
            '"connected":false}]'},
    expect_state={"system": {"network_interfaces": {
        "enp4s0f0": {"name": "Ethernet card 1 (enp4s0f0, 1 Gb/s link)", "connected": True},
        "lo": {"name": "Loopback (lo)", "connected": True},
        "ens5": {"name": "Ethernet card 3 (ens5)", "connected": False}}}})
telemetry(B, "sdi-devices", settings=_BL_AUTH, inbound_http={
    "path": "/api/system/sdi-devices",
    "body": '[{"id":"aja-sdi-0","name":"AJA SDI #0","ports":["1","2","3","4"]}]'},
    expect_state={"system": {"sdi_devices": {"aja-sdi-0": {"name": "AJA SDI #0", "ports": '["1","2","3","4"]'}}}})
telemetry(B, "ndi-sources", settings=_BL_AUTH, inbound_http={
    "path": "/api/system/ndi-devices?ndiGroup=public",
    "body": '[{"name":"TRANSCODER 0 (Studio 0)"},{"name":"TRANSCODER 0 (Studio 1)"}]'},
    expect_state={"ndi": {"sources": {"0": "TRANSCODER 0 (Studio 0)", "1": "TRANSCODER 0 (Studio 1)"}}})
telemetry(B, "reinstall-state", settings=_BL_AUTH, inbound_http={
    "path": "/api/system/reinstall-state", "body": '{"state":"NOT-STARTED","errorMessage":""}'},
    expect_state={"system": {"reinstall": {"state": "NOT-STARTED", "error_message": ""}}})
telemetry(B, "factory-reset", settings=_BL_AUTH, inbound_http={
    "path": "/api/system/factory-reset", "body": '{"state":"FAILED","errorMessage":"disk error"}'},
    expect_state={"system": {"factory_reset": {"state": "FAILED", "error_message": "disk error"}}})
