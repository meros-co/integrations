# Boinx mimoLive (boinx-mimolive): one vector per command over the HTTP API.
# Targets are the endpoints of mimoLive's user manual (Remote Control &
# Automation > HTTP API > Endpoints) under /api/v1, with the built-in outputs,
# type catalogs, comments and Zoom source assignment from Boinx's API
# reference (github.com/boinx/mimoLive-API-Reference). Bodies hold only the
# changed fields: plain objects for documents, layers, variants, sources and
# filters, as the manual's examples show; the JSON:API {"data": {...}} form
# for output destinations and layer sets, as the manual shows for those.
# JSON strings are written here with json.dumps (compact separators).
ML = "boinx-mimolive"

_ML_DOC = "458706932"
_ML_LAYER = "BA868701-8131-49CB-8EDD-8C7E6E7CD60B"
_ML_VAR = "6E1B6C1F-1B0C-4B29-9F49-4C1A2C0E9F11"
_ML_SRC = _ML_DOC + "-0B1E7C9A-3F5D-4C8B-A1E2-7D6F5A4B3C2D"
_ML_FLT = "2C3D4E5F-6A7B-4C8D-9E0F-1A2B3C4D5E6F"
_ML_OUT = "4FF003EA-D071-43FD-845F-DCD4A25DFA00"
_ML_SET = "9A8B7C6D-5E4F-4A3B-8C2D-1E0F9A8B7C6D"
_ML_D = "/api/v1/documents/" + _ML_DOC
_ML_L = _ML_D + "/layers/" + _ML_LAYER
_ML_V = _ML_L + "/variants/" + _ML_VAR
_ML_S = _ML_D + "/sources/" + _ML_SRC
_ML_O = _ML_D + "/output-destinations/" + _ML_OUT
_ML_LS = _ML_D + "/layer-sets/" + _ML_SET

_ML_OK = {"http_reply": {"status": 200, "body": "{}"}, "expect_result": {"ok": {"kind": "ack"}}}
_ML_GONE = {"http_reply": {"status": 204}, "expect_result": {"ok": {"kind": "ack"}}}
_ML_MADE = {"http_reply": {"status": 201, "body": "{}"}, "expect_result": {"ok": {"kind": "ack"}}}


def _ml(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body if isinstance(body, str) else json.dumps(body, separators=(",", ":"))
    V.append({"spec": ML, "command": command, "input": input, "expect_request": request, **extra})


def _ml_od(attributes):
    return {"data": {"type": "output-destinations", "id": _ML_OUT, "attributes": attributes}}


D_ = {"document": _ML_DOC}
DL = {"document": _ML_DOC, "layer": _ML_LAYER}
DLV = {**DL, "variant": _ML_VAR}
DS = {"document": _ML_DOC, "source": _ML_SRC}
DO = {"document": _ML_DOC, "output": _ML_OUT}
DLS = {"document": _ML_DOC, "layer_set": _ML_SET}

# ── Documents and the show ───────────────────────────────────────────────
_ML_DOCS = ('{"data":[{"type":"documents","id":"' + _ML_DOC + '","attributes":{"name":"Show.tvshow",'
            '"live-state":"off"}}]}')
_ml("list_documents", {}, "GET", "/api/v1/documents",
    http_reply={"status": 200, "body": _ML_DOCS},
    expect_result={"ok": {"kind": "value", "value": [{"type": "documents", "id": _ML_DOC, "attributes": {
        "name": "Show.tvshow", "live-state": "off"}}]}})
_ml("get_document", D_, "GET", _ML_D)
_ml("start_show", D_, "POST", _ML_D + "/setLive", **_ML_OK)
_ml("stop_show", D_, "POST", _ML_D + "/setOff")
_ml("toggle_show", D_, "POST", _ML_D + "/toggleLive")
_ml("set_program_volume", {**D_, "volume": 0.8}, "PUT", _ML_D, '{"programOutputMasterVolume":0.800}')
_ml("start_output", {**D_, "output": "record"}, "POST", _ML_D + "/outputs/record/setLive")
_ml("stop_output", {**D_, "output": "stream"}, "POST", _ML_D + "/outputs/stream/setOff",
    http_reply={"status": 409, "body": '{"errors":[{"status":"409","title":"Conflict"}]}'},
    expect_result={"error": {"error": "device_error", "code": "409"}})

# ── Layers ───────────────────────────────────────────────────────────────
_ml("list_layers", D_, "GET", _ML_D + "/layers")
_ml("get_layer", DL, "GET", _ML_L)
_ml("layer_live", DL, "POST", _ML_L + "/setLive", **_ML_OK)
_ml("layer_off", DL, "POST", _ML_L + "/setOff")
_ml("layer_toggle", DL, "POST", _ML_L + "/toggleLive")
_ml("layer_next_variant", DL, "POST", _ML_L + "/cycleThroughVariants")
_ml("layer_previous_variant", DL, "POST", _ML_L + "/cycleThroughVariantsBackwards")
_ml("layer_live_first_variant", DL, "POST", _ML_L + "/setLiveFirstVariant")
_ml("layer_live_last_variant", DL, "POST", _ML_L + "/setLiveLastVariant")
_ml("set_layer_volume", {**DL, "volume": 0.5}, "PUT", _ML_L, '{"volume":0.500}')
_ml("set_layer_name", {**DL, "name": 'Lower "Third"'}, "PUT", _ML_L, {"name": 'Lower "Third"'})
_ml("set_layer_text", {**DL, "input_key": "tvGroup_Content__Subtitle", "text": "This is a Test"}, "PUT", _ML_L,
    {"input-values": {"tvGroup_Content__Subtitle": "This is a Test"}}, **_ML_OK)
_ml("set_layer_number", {**DL, "input_key": "tvGroup_Geometry__Scale", "value": 1.25}, "PUT", _ML_L,
    '{"input-values":{"tvGroup_Geometry__Scale":1.2500}}')
_ml("set_layer_flag", {**DL, "input_key": "tvGroup_Control__Visible", "value": False}, "PUT", _ML_L,
    '{"input-values":{"tvGroup_Control__Visible":false}}')
_ml("set_layer_inputs", {**DL, "values": {"tvIn_Title": "Jane Smith", "tvIn_Subtitle": "CEO"}}, "PUT", _ML_L,
    {"input-values": {"tvIn_Title": "Jane Smith", "tvIn_Subtitle": "CEO"}})
_ml("trigger_layer_signal", {**DL, "signal": "tvGroup_Control__Start_TypeSignal"}, "POST",
    _ML_L + "/signals/tvGroup_Control__Start_TypeSignal")
_ml("layer_media_control", {**DL, "input_key": "tvIn_VideoSourceAImage", "media_command": "skipahead"}, "POST",
    _ML_L + "/inputs/tvIn_VideoSourceAImage/mediacontrol/skipahead")
_ml("add_layer", {**D_, "layer_type": "com.boinx.mimoLive.layer.lowerthird", "name": "My Lower Third"}, "POST",
    _ML_D + "/layers", {"layer-identifier": "com.boinx.mimoLive.layer.lowerthird", "name": "My Lower Third"},
    **_ML_MADE)
_ml("add_layer_at", {**D_, "layer_type": "com.boinx.mimoLive.layer.lowerthird", "name": "My Lower Third",
                     "index": 4}, "POST", _ML_D + "/layers",
    {"layer-identifier": "com.boinx.mimoLive.layer.lowerthird", "index": 4, "name": "My Lower Third"})
_ml("delete_layer", DL, "DELETE", _ML_L, **_ML_GONE)

# ── Layer variants ───────────────────────────────────────────────────────
_ml("list_variants", DL, "GET", _ML_L + "/variants")
_ml("get_variant", DLV, "GET", _ML_V)
_ml("variant_live", DLV, "POST", _ML_V + "/setLive")
_ml("variant_off", DLV, "POST", _ML_V + "/setOff")
_ml("variant_toggle", DLV, "POST", _ML_V + "/toggleLive")
_ml("set_variant_name", {**DLV, "name": "New name of the Variant"}, "PUT", _ML_V, {"name": "New name of the Variant"})
_ml("set_variant_text", {**DLV, "input_key": "tvGroup_Content__Subtitle", "text": "This is a Test"}, "PUT", _ML_V,
    {"input-values": {"tvGroup_Content__Subtitle": "This is a Test"}})
_ml("set_variant_inputs", {**DLV, "values": {"tvIn_Title": "A"}}, "PUT", _ML_V, {"input-values": {"tvIn_Title": "A"}})
_ml("trigger_variant_signal", {**DLV, "signal": "tvGroup_Control__Next_TypeSignal"}, "POST",
    _ML_V + "/signals/tvGroup_Control__Next_TypeSignal")

# ── Sources and filters ──────────────────────────────────────────────────
_ml("list_sources", D_, "GET", _ML_D + "/sources")
_ml("get_source", DS, "GET", _ML_S)
_ml("set_source_gain", {**DS, "gain": 1.25}, "PUT", _ML_S, '{"gain":1.250}')
_ml("set_source_name", {**DS, "name": "Main Camera"}, "PUT", _ML_S, {"name": "Main Camera"})
_ml("set_source_inputs", {**DS, "values": {"tvGroup_Content__Subtitle": "This is a Test"}}, "PUT", _ML_S,
    {"input-values": {"tvGroup_Content__Subtitle": "This is a Test"}})
_ml("set_source_video_device", {**DS, "device_name": "Logitech StreamCam"}, "PUT", _ML_S,
    {"video-device-name": "Logitech StreamCam"})
_ml("set_source_audio_device", {**DS, "device_name": "none"}, "PUT", _ML_S, {"audio-device-name": "none"})
_ml("set_source_channel_map", {**DS, "left": 2, "right": 3}, "PUT", _ML_S, {"channel-map": [2, 3]})
_ml("source_media_control", {**DS, "media_command": "play"}, "POST", _ML_S + "/mediacontrol/play")
_ml("trigger_source_signal", {**DS, "signal": "tvGroup_Control__Reset_TypeSignal"}, "POST",
    _ML_S + "/signals/tvGroup_Control__Reset_TypeSignal")
_ml("source_action", {**DS, "action": "reconnect"}, "GET", _ML_S + "/actions/reconnect",
    http_reply={"status": 409, "body": '{"errors":[{"status":"409","title":"Operation not allowed"}]}'},
    expect_result={"error": {"error": "device_error", "code": "409"}})
_ml("add_source", {**D_, "source_type": "com.boinx.mimoLive.sources.deviceVideoSource", "name": "Main Camera"},
    "POST", _ML_D + "/sources",
    {"source-type": "com.boinx.mimoLive.sources.deviceVideoSource", "name": "Main Camera"}, **_ML_MADE)
_ml("delete_source", DS, "DELETE", _ML_S, **_ML_GONE)
_ml("list_filters", DS, "GET", _ML_S + "/filters")
_ml("set_filter_inputs", {**DS, "filter": _ML_FLT, "values": {"tvIn_Amount": 0.5}}, "PUT",
    _ML_S + "/filters/" + _ML_FLT, {"input-values": {"tvIn_Amount": 0.5}})
_ml("trigger_filter_signal", {**DS, "filter": _ML_FLT, "signal": "tvIn_Reset_TypeSignal"}, "POST",
    _ML_S + "/filters/" + _ML_FLT + "/signals/tvIn_Reset_TypeSignal")

# ── Output destinations ──────────────────────────────────────────────────
_ml("list_output_destinations", D_, "GET", _ML_D + "/output-destinations")
_ml("get_output_destination", DO, "GET", _ML_O)
_ml("output_destination_live", DO, "POST", _ML_O + "/setLive", **_ML_OK)
_ml("output_destination_off", DO, "POST", _ML_O + "/setOff")
_ml("set_output_destination_title", {**DO, "title": "Our Recording"}, "PATCH", _ML_O,
    _ml_od({"title": "Our Recording"}))
_ml("set_output_destination_starts_with_show", {**DO, "enabled": True}, "PATCH", _ML_O,
    _ml_od({"starts-with-show": True}))
_ml("set_output_destination_stops_with_show", {**DO, "enabled": False}, "PATCH", _ML_O,
    _ml_od({"stops-with-show": False}))
_ml("set_recording_location", {**DO, "location": "~/Movies"}, "PATCH", _ML_O,
    _ml_od({"settings": {"location": "~/Movies"}}))
_ml("set_recording_filename", {**DO, "filename": "%show %year-%month-%day.%extension"}, "PATCH", _ML_O,
    _ml_od({"settings": {"filename": "%show %year-%month-%day.%extension"}}))
_ml("set_streaming_target", {**DO, "rtmp_url": "rtmp://stream.example.com/live", "stream_key": "your-key"},
    "PATCH", _ML_O, _ml_od({"settings": {"rtmpurl": "rtmp://stream.example.com/live", "streamingkey": "your-key"}}))
_ml("add_output_destination", {**D_, "output_type": "com.boinx.mimoLive.outputDestination.fileRecording"}, "POST",
    _ML_D + "/output-destinations",
    {"output-destination-type": "com.boinx.mimoLive.outputDestination.fileRecording"})
_ml("delete_output_destination", DO, "DELETE", _ML_O, **_ML_GONE)

# ── Layer sets ───────────────────────────────────────────────────────────
_ml("list_layer_sets", D_, "GET", _ML_D + "/layer-sets")
_ml("recall_layer_set", DLS, "POST", _ML_LS + "/recall", **_ML_OK)
_ml("set_layer_set_name", {**DLS, "name": "Wide Shot Scene"}, "PATCH", _ML_LS,
    {"data": {"attributes": {"name": "Wide Shot Scene"}}})
_ml_entries = [{"layer-id": _ML_LAYER, "action": "live", "variant": _ML_VAR},
               {"layer-id": "YYYYYYYY-YYYY-YYYY-YYYY-YYYYYYYYYYYY", "action": "force-off"}]
_ml("set_layer_set_layers", {**DLS, "layers": _ml_entries}, "PATCH", _ML_LS,
    {"data": {"attributes": {"layers": _ml_entries}}})
_ml("add_layer_set", {**D_, "name": "Interview Scene"}, "POST", _ML_D + "/layer-sets",
    {"data": {"attributes": {"name": "Interview Scene"}}}, **_ML_MADE)
_ml("delete_layer_set", DLS, "DELETE", _ML_LS)

# ── Data stores ──────────────────────────────────────────────────────────
_ml("get_datastore", {**D_, "store": "scoreboard"}, "GET", _ML_D + "/datastores/scoreboard",
    http_reply={"status": 200, "body": '{"home":3,"away":1}'},
    expect_result={"ok": {"kind": "value", "value": {"home": 3, "away": 1}}})
_ml("put_datastore", {**D_, "store": "scoreboard", "data": {"home": 3, "away": 1}}, "PUT",
    _ML_D + "/datastores/scoreboard", {"home": 3, "away": 1})
_ml("delete_datastore", {**D_, "store": "scoreboard"}, "DELETE", _ML_D + "/datastores/scoreboard",
    http_reply={"status": 404, "body": "{}"}, expect_result={"error": {"error": "device_error", "code": "404"}})

# ── Devices, accounts, types and comments ────────────────────────────────
_ml("list_devices", {}, "GET", "/api/v1/devices")
_ml("get_device", {"device": "BuiltInMicrophoneDevice"}, "GET", "/api/v1/devices/BuiltInMicrophoneDevice")
_ml("list_accounts", {}, "GET", "/api/v1/accounts")
_ml("list_layer_types", {}, "GET", "/api/v1/layertypes")
_ml("list_source_types", {}, "GET", "/api/v1/sourcetypes")
_ml("list_output_destination_types", {}, "GET", "/api/v1/outputdestinationtypes")
_ml("inject_comment", {"username": "Jane", "comment": "Hello there", "platform": "youtube"}, "POST",
    "/api/v1/comments/new?username=Jane&comment=Hello%20there&platform=youtube")

# ── Zoom ─────────────────────────────────────────────────────────────────
_ml("zoom_join", {"meeting": "123456789", "account": "My Work Account"}, "GET",
    "/api/v1/zoom/join?meetingid=123456789&zoomaccountname=My%20Work%20Account")
_ml("zoom_join_with_passcode", {"meeting": "123456789", "account": "My Work Account", "passcode": "abc123",
                                "display_name": "mimoLive"}, "GET",
    "/api/v1/zoom/join?meetingid=123456789&zoomaccountname=My%20Work%20Account&passcode=abc123"
    "&displayname=mimoLive&virtualcamera=true")
_ml("zoom_leave", {}, "GET", "/api/v1/zoom/leave")
_ml("zoom_end", {}, "GET", "/api/v1/zoom/end")
_ml("zoom_participants", {}, "GET", "/api/v1/zoom/participants",
    http_reply={"status": 200, "body": '{"data":[]}'}, expect_result={"ok": {"kind": "value", "value": []}})
_ml("zoom_meeting_action", {"action": "muteAll"}, "GET", "/api/v1/zoom/meetingaction?command=muteAll")
_ml("zoom_participant_action", {"action": "muteAudio", "user": 16786432}, "GET",
    "/api/v1/zoom/meetingaction?command=muteAudio&userid=16786432")
_ml("zoom_assign_participant", {**DS, "user": 16786432}, "PATCH", _ML_S, {"zoom-userid": 16786432})
_ml("zoom_unassign_source", DS, "PATCH", _ML_S, {"zoom-userselectiontype": 0})
_ml("zoom_assign_active_speaker", DS, "PATCH", _ML_S, {"zoom-userselectiontype": 2})
_ml("zoom_assign_screen_share", DS, "PATCH", _ML_S, {"zoom-userselectiontype": 6})

# ── Telemetry ────────────────────────────────────────────────────────────
# The poll: the document list with layers sideloaded (the manual's data
# types; the include query from Boinx's reference).
telemetry(ML, "documents-with-layers", expect_connect_ws=['{"event":"ping"}'], inbound_http={
    "path": "/api/v1/documents?include=layers",
    "body": json.dumps({
        "data": [{"type": "documents", "id": _ML_DOC, "attributes": {
            "name": "Show.tvshow", "live-state": "live", "duration": 125, "show-start": 1759572000,
            "programOutputMasterVolume": 0.9,
            "metadata": {"title": "Sunday", "width": 1920, "height": 1080, "framerate": 29.97}}}],
        "included": [{"type": "layers", "id": _ML_LAYER, "attributes": {
            "name": "Lower Third", "live-state": "live", "volume": None, "index": 5,
            "composition-id": "com.boinx.layer.lowerThird"},
            "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}},
                              "active-variant": {"data": {"type": "variants", "id": _ML_VAR}},
                              "live-variant": {"data": {"type": "variants", "id": _ML_VAR}}}}]})},
    expect_state={
        "documents": {_ML_DOC: {"name": "Show.tvshow", "live_state": "live", "duration": 125,
                                "show_start": 1759572000, "program_volume": 0.9, "show_title": "Sunday",
                                "width": 1920, "height": 1080, "framerate": 29.97}},
        "layers": {_ML_LAYER: {"name": "Lower Third", "live_state": "live", "index": 5,
                               "composition_id": "com.boinx.layer.lowerThird", "document": _ML_DOC,
                               "active_variant": _ML_VAR, "live_variant": _ML_VAR}}})
telemetry(ML, "sources-included", inbound_http={
    "path": "/api/v1/documents?include=sources",
    "body": json.dumps({"data": [], "included": [{"type": "sources", "id": _ML_SRC, "attributes": {
        "name": "Camera 1", "tally-state": "program", "gain": 1.0, "video": True, "audio": False,
        "video-device-connected": True, "summary": "MacBook Air Camera",
        "source-type": "com.boinx.mimoLive.sources.deviceVideoSource"},
        "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}}}}]})},
    expect_state={"sources": {_ML_SRC: {
        "name": "Camera 1", "tally_state": "program", "gain": 1.0, "video": True, "audio": False,
        "video_connected": True, "summary": "MacBook Air Camera",
        "source_type": "com.boinx.mimoLive.sources.deviceVideoSource", "document": _ML_DOC}}})
telemetry(ML, "output-destination-reply", inbound_http={
    "path": _ML_O + "/setLive",
    "body": json.dumps({"data": {"type": "output-destinations", "id": _ML_OUT, "attributes": {
        "title": "File Recording", "type": "File Recording", "live-state": "startup", "ready-to-go-live": True,
        "starts-with-show": True, "stops-with-show": True, "summary": "~/Movies, Program Output (H.264)"},
        "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}}}}})},
    expect_state={"output-destinations": {_ML_OUT: {
        "title": "File Recording", "kind": "File Recording", "live_state": "startup", "ready": True,
        "starts_with_show": True, "stops_with_show": True, "summary": "~/Movies, Program Output (H.264)",
        "document": _ML_DOC}}})
telemetry(ML, "builtin-outputs", inbound_http={
    "path": _ML_D,
    "body": json.dumps({"data": {"type": "documents", "id": _ML_DOC, "attributes": {
        "name": "Show.tvshow", "live-state": "off",
        "outputs": [{"id": "record", "type": "record", "live-state": "live"},
                    {"id": "stream", "type": "stream", "live-state": "off"}]}}})},
    expect_state={"documents": {_ML_DOC: {"name": "Show.tvshow", "live_state": "off",
                                          "outputs": {"record": {"live_state": "live"},
                                                      "stream": {"live_state": "off"}}}}})
# Pushed on the websocket (Boinx's reference; the Companion module's handler).
telemetry(ML, "pushed-layer-change", inbound_ws=json.dumps({
    "event": "changed", "type": "layers", "id": _ML_LAYER,
    "data": {"type": "layers", "id": _ML_LAYER, "attributes": {"name": "Lower Third", "live-state": "shutdown",
                                                              "index": 2, "volume": 0.5},
             "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}},
                               "active-variant": {"data": {"type": "variants", "id": _ML_VAR}}}}}),
    expect_state={"layers": {_ML_LAYER: {"name": "Lower Third", "live_state": "shutdown", "index": 2,
                                         "volume": 0.5, "document": _ML_DOC, "active_variant": _ML_VAR}}})
telemetry(ML, "pushed-layer-set", inbound_ws=json.dumps({
    "event": "added", "type": "layer-sets", "id": _ML_SET,
    "data": {"type": "layer-sets", "id": _ML_SET, "attributes": {"name": "Intro Scene", "active": True},
             "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}}}}}),
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "active": True, "document": _ML_DOC}}})
telemetry(ML, "pushed-document-outputs", inbound_ws=json.dumps({
    "event": "changed", "type": "documents", "id": _ML_DOC,
    "data": {"type": "documents", "id": _ML_DOC, "attributes": {
        "name": "Show.tvshow", "live-state": "live",
        "outputs": [{"id": "record", "type": "record", "live-state": "live"}]}}}),
    expect_state={"documents": {_ML_DOC: {"name": "Show.tvshow", "live_state": "live",
                                          "outputs": {"record": {"live_state": "live"}}}}})
telemetry(ML, "pushed-removal", inbound_ws=json.dumps({"event": "removed", "type": "variants", "id": _ML_VAR}),
          state_before={"variants": {_ML_VAR: {"name": "Lower third", "layer": "L1"}, "V2": {"name": "Other"}}},
          expect_state={"variants": {"V2": {"name": "Other"}}})
telemetry(ML, "pong-is-not-state", inbound_ws='{"event":"pong"}', expect_state={})
