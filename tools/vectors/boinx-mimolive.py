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


def _ml(command, input, method, target, body=None, port=None, **extra):
    request = {"method": method, "target": target}
    if port is not None:
        request["port"] = port
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
# A layer set's layers (the manual's Data Types: layer-id, action live, off
# or force-off, variant for a live entry), kept by position. A pushed set is
# read again, since what mimoLive pushes for layer sets is not documented.
_ML_SET2 = "1F2E3D4C-5B6A-4978-8695-A4B3C2D1E0F9"
_ML_LAYER2 = "C3D4E5F6-0718-4293-A4B5-C6D7E8F90A1B"
_ML_REREAD = [{"method": "GET", "target": _ML_LS}]
_ml_entries_state = {"0": {"layer_id": _ML_LAYER, "action": "live", "variant": _ML_VAR},
                     "1": {"layer_id": "YYYYYYYY-YYYY-YYYY-YYYY-YYYYYYYYYYYY", "action": "force-off"}}


def _ml_set(attributes, set_id=_ML_SET):
    return {"type": "layer-sets", "id": set_id, "attributes": attributes,
            "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}}}}


telemetry(ML, "pushed-layer-set", inbound_ws=json.dumps({
    "event": "added", "type": "layer-sets", "id": _ML_SET,
    "data": _ml_set({"name": "Intro Scene", "active": True, "recall-on-show-start": True,
                     "recall-on-show-end": False, "layers": _ml_entries})}),
    expect_then_send=_ML_REREAD,
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "active": True, "document": _ML_DOC,
                                           "recall_on_show_start": True, "recall_on_show_end": False,
                                           "layers": _ml_entries_state}}})
# A push without layers leaves the list as it was, and still has it read.
telemetry(ML, "pushed-layer-set-without-layers", inbound_ws=json.dumps({
    "event": "changed", "type": "layer-sets", "id": _ML_SET,
    "data": _ml_set({"name": "Intro Scene", "active": False})}),
    state_before={"layer-sets": {_ML_SET: {"name": "Intro Scene", "active": True, "layers": _ml_entries_state}}},
    expect_then_send=_ML_REREAD,
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "active": False, "document": _ML_DOC,
                                           "layers": _ml_entries_state}}})
# The poll's sideloaded sets: one with layers, one answered without them.
telemetry(ML, "layer-sets-included", inbound_http={
    "path": "/api/v1/documents?include=layer-sets",
    "body": json.dumps({"data": [], "included": [
        _ml_set({"name": "Intro Scene", "active": True, "layers": _ml_entries}),
        _ml_set({"name": "Wide Shot", "active": False}, _ML_SET2)]})},
    state_before={"layer-sets": {_ML_SET2: {"layers": {"0": {"layer_id": _ML_LAYER2, "action": "off"}}}}},
    expect_state={"layer-sets": {
        _ML_SET: {"name": "Intro Scene", "active": True, "document": _ML_DOC, "layers": _ml_entries_state},
        _ML_SET2: {"name": "Wide Shot", "active": False, "document": _ML_DOC,
                   "layers": {"0": {"layer_id": _ML_LAYER2, "action": "off"}}}}})
# list_layer_sets with a shorter list: the entries it no longer has go, and
# the variant of an entry that turned off with them.
telemetry(ML, "layer-set-list-shrinks", inbound_http={
    "path": _ML_D + "/layer-sets",
    "body": json.dumps({"data": [_ml_set({"name": "Intro Scene", "layers": [
        {"layer-id": _ML_LAYER2, "action": "off"}]})]})},
    state_before={"layer-sets": {_ML_SET: {"name": "Intro Scene", "layers": {
        **_ml_entries_state, "2": {"layer_id": _ML_LAYER2, "action": "live", "variant": "edit-variant"}}}}},
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "document": _ML_DOC,
                                           "layers": {"0": {"layer_id": _ML_LAYER2, "action": "off"}}}}})
# A GET of one set.
telemetry(ML, "layer-set-reply", inbound_http={
    "path": _ML_LS,
    "body": json.dumps({"data": _ml_set({"name": "Intro Scene", "active": False, "recall-on-show-start": False,
                                         "recall-on-show-end": True, "layers": [
                                             {"layer-id": _ML_LAYER2, "action": "live",
                                              "variant": "edit-variant"}]})})},
    expect_state={"layer-sets": {_ML_SET: {
        "name": "Intro Scene", "active": False, "document": _ML_DOC, "recall_on_show_start": False,
        "recall_on_show_end": True,
        "layers": {"0": {"layer_id": _ML_LAYER2, "action": "live", "variant": "edit-variant"}}}}})
# An empty layers array clears the list.
telemetry(ML, "layer-set-empty-layers", inbound_http={
    "path": _ML_LS, "body": json.dumps({"data": _ml_set({"name": "Intro Scene", "layers": []})})},
    state_before={"layer-sets": {_ML_SET: {"name": "Intro Scene", "layers": _ml_entries_state}}},
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "document": _ML_DOC}}})
# set_layer_set_layers' PATCH: the list written reads back as the same state
# from the answer's object, and the set is read again.
telemetry(ML, "layer-set-layers-written", inbound_http={
    "path": _ML_LS, "request": {"data": {"attributes": {"layers": _ml_entries}}},
    "body": json.dumps({"data": _ml_set({"name": "Intro Scene", "layers": _ml_entries})})},
    state_before={"layer-sets": {_ML_SET: {"name": "Intro Scene", "layers": {
        "0": {"layer_id": _ML_LAYER2, "action": "off"}}}}},
    expect_then_send=_ML_REREAD,
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "document": _ML_DOC,
                                           "layers": _ml_entries_state}}})
# Only layer-sets objects: a layers object, even one with a layers
# attribute, keeps to its own generic state and touches no list.
telemetry(ML, "layer-object-not-a-layer-set", inbound_http={
    "path": "/api/v1/documents?include=layers",
    "body": json.dumps({"data": [], "included": [{"type": "layers", "id": _ML_LAYER, "attributes": {
        "name": "Lower Third", "live-state": "off", "layers": [{"layer-id": _ML_LAYER2, "action": "live"}]},
        "relationships": {"document": {"data": {"type": "documents", "id": _ML_DOC}}}}]})},
    state_before={"layer-sets": {_ML_SET: {"layers": _ml_entries_state}}},
    expect_state={"layer-sets": {_ML_SET: {"layers": _ml_entries_state}},
                  "layers": {_ML_LAYER: {"name": "Lower Third", "live_state": "off", "document": _ML_DOC}}})
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

# ── Full control: the document, show metadata, sources, output destinations,
# layer sets, types, comments and Zoom (the manual's Endpoints and Data
# Types; Boinx's reference for the metadata fields, the single type reads,
# /settings, the comment parameters and webinartoken).
_ml("set_document_name", {**D_, "name": "Sunday Show"}, "PUT", _ML_D, {"name": "Sunday Show"})
_ml("set_show_title", {**D_, "title": 'The "Sunday" Show'}, "PUT", _ML_D, {"metadata": {"title": 'The "Sunday" Show'}})
_ml("set_show_author", {**D_, "author": "Jane Doe"}, "PUT", _ML_D, {"metadata": {"author": "Jane Doe"}})
_ml("set_show_comments", {**D_, "comments": "Second camera on the left"}, "PUT", _ML_D,
    {"metadata": {"comments": "Second camera on the left"}})
_ml("set_show_name", {**D_, "show": "Sunday"}, "PUT", _ML_D, {"metadata": {"show": "Sunday"}})
_ml("set_show_planned_duration", {**D_, "seconds": 3600}, "PUT", _ML_D, {"metadata": {"duration": 3600}})
_ml("set_document_resolution", {**D_, "width": 1920, "height": 1080}, "PUT", _ML_D,
    {"metadata": {"width": 1920, "height": 1080}})
_ml("set_document_framerate", {**D_, "framerate": 29.97}, "PUT", _ML_D, '{"metadata":{"framerate":29.970}}')
_ml("set_document_samplerate", {**D_, "samplerate": 48000}, "PUT", _ML_D, {"metadata": {"samplerate": 48000}})
_ml("set_document_metadata", {**D_, "metadata": {"title": "Sunday", "comments": "Line one\nLine two"}}, "PUT",
    _ML_D, {"metadata": {"title": "Sunday", "comments": "Line one\nLine two"}})
_ml("get_server_settings", {}, "GET", "/api/v1/settings",
    http_reply={"status": 200, "body": '{"tracking":true}'},
    expect_result={"ok": {"kind": "value", "value": {"tracking": True}}})
_ml("add_layer_with_inputs", {**D_, "layer_type": "com.boinx.mimoLive.layer.lowerthird", "name": "Guest",
                              "index": 0, "values": {"tvIn_Title": "John Doe"}}, "POST", _ML_D + "/layers",
    {"layer-identifier": "com.boinx.mimoLive.layer.lowerthird", "index": 0, "name": "Guest",
     "input-values": {"tvIn_Title": "John Doe"}}, **_ML_MADE)
_ml("set_source_channel_map_by_name", {**DS, "left": "Ch. 3", "right": "Ch. 4"}, "PUT", _ML_S,
    {"channel-map": ["Ch. 3", "Ch. 4"]})
_ml("set_source_video_device_id", {**DS, "device_id": "none"}, "PUT", _ML_S, {"video-device-id": "none"})
_ml("set_source_audio_device_id", {**DS, "device_id": "BuiltInMicrophoneDevice"}, "PUT", _ML_S,
    {"audio-device-id": "BuiltInMicrophoneDevice"})
_ml("set_mimocall_video_codec", {**DS, "codec": "h264"}, "PATCH", _ML_S, {"video-codec": "h264"})
_ml("set_mimocall_high_quality_audio", {**DS, "enabled": True}, "PATCH", _ML_S,
    {"prefers-high-quality-audio": True})
_ml("set_mimocall_partner_sees", {**DS, "video": "program-output"}, "PATCH", _ML_S,
    {"partner-sees": {"type": "program-output"}})
_ML_SRC2 = _ML_DOC + "-7A6B5C4D-3E2F-4A1B-9C8D-7E6F5A4B3C2D"
_ml("set_mimocall_partner_sees_source", {**DS, "video_source": _ML_SRC2}, "PATCH", _ML_S,
    {"partner-sees": {"type": "source", "source-id": _ML_SRC2}})
_ml("set_mimocall_partner_sees_above_layer", {**DS, "layer_index": 3}, "PATCH", _ML_S,
    {"partner-sees": {"type": "above-layer", "layer-index": 3}})
_ml("set_mimocall_partner_hears", {**DS, "audio": "master-mix"}, "PATCH", _ML_S,
    {"partner-hears": {"type": "master-mix"}})
_ml("set_mimocall_partner_hears_custom_mix", {**DS, "mix": "Audio Mix 2"}, "PATCH", _ML_S,
    {"partner-hears": {"type": "custom-mix", "mix-name": "Audio Mix 2"}})
_ml("set_mimocall_partner_hears_solo_source", {**DS, "audio_source": _ML_SRC2}, "PATCH", _ML_S,
    {"partner-hears": {"type": "solo-source", "source-id": _ML_SRC2}})
_ml("add_source_with_attributes", {**D_, "attributes": {"source-type": "com.boinx.mimoLive.sources.deviceVideoSource",
                                                        "name": "Main Camera",
                                                        "video-device-name": "Logitech StreamCam"}},
    "POST", _ML_D + "/sources", {"source-type": "com.boinx.mimoLive.sources.deviceVideoSource",
                                 "name": "Main Camera", "video-device-name": "Logitech StreamCam"}, **_ML_MADE)
_ml("get_filter", {**DS, "filter": _ML_FLT}, "GET", _ML_S + "/filters/" + _ML_FLT)
_ml("set_streaming_public_url", {**DO, "public_url": "https://www.youtube.com/watch?v=abc"}, "PATCH", _ML_O,
    _ml_od({"settings": {"publicurl": "https://www.youtube.com/watch?v=abc"}}))
_ml("reset_recording_location", DO, "PATCH", _ML_O, _ml_od({"settings": {"location": None}}))
_ml("reset_recording_filename", DO, "PATCH", _ML_O, _ml_od({"settings": {"filename": None}}))
_ml("add_output_destination_at", {**D_, "output_type": "com.boinx.mimoLive.outputDestination.fileRecording",
                                  "index": 0}, "POST", _ML_D + "/output-destinations",
    {"output-destination-type": "com.boinx.mimoLive.outputDestination.fileRecording", "index": 0}, **_ML_MADE)
_ml_new_od = {"data": {"attributes": {"output-destination-type": "com.boinx.mimoLive.outputDestination.liveStreaming",
                                      "title": "My Stream",
                                      "settings": {"rtmpurl": "rtmp://fb.live/1234567", "streamingkey": "abc"}}}}
_ml("add_output_destination_with_attributes", {**D_, "attributes": _ml_new_od}, "POST",
    _ML_D + "/output-destinations", _ml_new_od, **_ML_MADE)
_ml("get_layer_set", DLS, "GET", _ML_LS)
_ml("set_layer_set_recall_on_show_start", {**DLS, "enabled": True}, "PATCH", _ML_LS,
    {"data": {"attributes": {"recall-on-show-start": True}}})
_ml("set_layer_set_recall_on_show_end", {**DLS, "enabled": False}, "PATCH", _ML_LS,
    {"data": {"attributes": {"recall-on-show-end": False}}})
_ml("add_layer_set_with_layers", {**D_, "name": "Interview Scene", "recall_on_show_start": True,
                                  "recall_on_show_end": False, "layers": _ml_entries}, "POST",
    _ML_D + "/layer-sets", {"data": {"attributes": {"name": "Interview Scene", "recall-on-show-start": True,
                                                    "recall-on-show-end": False, "layers": _ml_entries}}},
    **_ML_MADE)
_ml("get_layer_type", {"type_id": "com.boinx.mimoLive.layer.lowerthird"}, "GET",
    "/api/v1/layertypes/com.boinx.mimoLive.layer.lowerthird")
_ml("get_source_type", {"type_id": "com.boinx.mimoLive.sources.deviceVideoSource"}, "GET",
    "/api/v1/sourcetypes/com.boinx.mimoLive.sources.deviceVideoSource")
_ml("get_output_destination_type", {"type_id": "com.boinx.mimoLive.outputDestination.fileRecording"}, "GET",
    "/api/v1/outputdestinationtypes/com.boinx.mimoLive.outputDestination.fileRecording")
_ml("inject_comment_with_details", {"username": "Jane", "comment": "Hello", "platform": "twitch",
                                    "date": "2026-10-07T18:30:00Z", "favorite": True,
                                    "user_image_url": "https://example.com/jane.png"}, "POST",
    "/api/v1/comments/new?username=Jane&comment=Hello&platform=twitch&date=2026-10-07T18%3A30%3A00Z"
    "&favorite=true&userimageurl=https%3A%2F%2Fexample.com%2Fjane.png")
_ml("zoom_join_webinar", {"meeting": "123456789", "account": "My Work Account", "webinar_token": "tk-1",
                          "display_name": "mimoLive", "virtual_camera": False}, "GET",
    "/api/v1/zoom/join?meetingid=123456789&zoomaccountname=My%20Work%20Account&webinartoken=tk-1"
    "&displayname=mimoLive&virtualcamera=false")

# mlController (its Services/WebServer.swift): port 8990, every answer 200
# with JSON, no mimoLive key sent.
_ML_STATUS = {"availableMimoLiveApps": [{"name": "mimoLive (6.15)", "path": "/Applications/mimoLive.app"}],
              "localDocuments": ["/Users/me/Documents/show1.tvshow"],
              "openDocuments": [{"id": _ML_DOC, "name": "My Show", "path": "/Users/me/Documents/show1.tvshow"}],
              "running": True, "selectedMimoLive": "mimoLive (6.15)",
              "selectedMimoLivePath": "/Applications/mimoLive.app"}
_ml("mlcontroller_status", {}, "GET", "/api/status", port=8990,
    http_reply={"status": 200, "body": json.dumps(_ML_STATUS)},
    expect_result={"ok": {"kind": "value", "value": _ML_STATUS}})
_ml("launch_mimolive", {}, "POST", "/api/start", port=8990,
    http_reply={"status": 200, "body": '{"status":"starting"}'}, expect_result={"ok": {"kind": "ack"}})
_ml("quit_mimolive", {}, "POST", "/api/stop", port=8990)
_ml("restart_mimolive", {}, "POST", "/api/restart", port=8990)
_ml("open_document", {"path": "/Users/me/Documents/show1.tvshow"}, "POST", "/api/open",
    '{"path":"/Users/me/Documents/show1.tvshow"}', port=8990)
_ml("select_mimolive_app", {"path": ""}, "POST", "/api/select", '{"path":""}', port=8990)
# mlController's own password set: its 401 answers that command only.
V.append({"spec": ML, "command": "launch_mimolive", "file": "launch_mimolive-401",
          "input": {}, "settings": {"token": "a" * 64},
          "expect_request": {"method": "POST", "port": 8990, "target": "/api/start"},
          "http_reply": {"status": 401, "body": "Unauthorized"},
          "expect_result": {"error": {"error": "device_error", "code": "401"}}})
V.append({"spec": ML, "command": "mlcontroller_status", "file": "mlcontroller_status-other-port",
          "input": {}, "settings": {"mlcontroller_port": 9100},
          "expect_request": {"method": "GET", "port": 9100, "target": "/api/status"}})

# ── Telemetry for full control ───────────────────────────────────────────
# The show metadata (the manual's Data Types, documents).
telemetry(ML, "document-metadata", inbound_http={
    "path": _ML_D,
    "body": json.dumps({"data": {"type": "documents", "id": _ML_DOC, "attributes": {
        "name": "Show.tvshow", "live-state": "off", "metadata": {
            "title": "Sunday", "comments": "Two cameras", "author": "Jane Doe", "show": "Weekly",
            "width": 1920, "height": 1080, "framerate": 25, "samplerate": 48000, "duration": 3600}}}})},
    expect_state={"documents": {_ML_DOC: {
        "name": "Show.tvshow", "live_state": "off", "show_title": "Sunday", "show_comments": "Two cameras",
        "show_author": "Jane Doe", "show_name": "Weekly", "width": 1920, "height": 1080, "framerate": 25.0,
        "samplerate": 48000, "planned_duration": 3600}}})
# Input and output values, kept whole as JSON text.
telemetry(ML, "pushed-layer-input-values", inbound_ws=json.dumps({
    "event": "changed", "type": "layers", "id": _ML_LAYER,
    "data": {"type": "layers", "id": _ML_LAYER, "attributes": {
        "name": "Lower Third", "live-state": "live",
        "input-values": {"tvIn_Title": "Jane Smith", "tvIn_Visible": True},
        "output-values": {"tvOut_SettingName": "Lower Third"}}}}),
    expect_state={"layers": {_ML_LAYER: {
        "name": "Lower Third", "live_state": "live",
        "input_values": '{"tvIn_Title":"Jane Smith","tvIn_Visible":true}',
        "output_values": '{"tvOut_SettingName":"Lower Third"}'}}})
# Zoom participant and mimoCall source attributes (the manual's mimoCall
# source properties; Boinx's reference for the Zoom ones).
telemetry(ML, "sources-zoom-and-mimocall", inbound_http={
    "path": _ML_D + "/sources",
    "body": json.dumps({"data": [
        {"type": "sources", "id": _ML_SRC, "attributes": {
            "name": "Zoom 1", "source-type": "com.boinx.mimoLive.sources.zoomparticipant",
            "is-hidden": False, "is-static": False, "zoom-userid": 16786432, "zoom-username": "John Doe",
            "zoom-userselectiontype": 1, "zoom-videoresolution": "1080p"}},
        {"type": "sources", "id": _ML_SRC2, "attributes": {
            "name": "Guest", "source-type": "com.boinx.mimoLive.sources.webRTCSource",
            "video-codec": "vp9", "prefers-high-quality-audio": True,
            "partner-sees": {"type": "above-layer", "layer-index": 3},
            "partner-hears": {"type": "custom-mix", "mix-name": "Audio Mix 2"},
            "available-actions": ["reconnect"]}}]})},
    expect_state={"sources": {
        _ML_SRC: {"name": "Zoom 1", "source_type": "com.boinx.mimoLive.sources.zoomparticipant",
                  "hidden": False, "static": False, "zoom_user_id": 16786432, "zoom_user_name": "John Doe",
                  "zoom_selection_type": 1, "zoom_video_resolution": "1080p"},
        _ML_SRC2: {"name": "Guest", "source_type": "com.boinx.mimoLive.sources.webRTCSource",
                   "video_codec": "vp9", "high_quality_audio": True, "partner_sees": "above-layer",
                   "partner_sees_layer_index": 3, "partner_hears": "custom-mix",
                   "partner_hears_mix": "Audio Mix 2", "available_actions": '["reconnect"]'}}})
# A source's filters sideloaded in a GET of the source, and a filter removed.
telemetry(ML, "source-filters-included", inbound_http={
    "path": _ML_S,
    "body": json.dumps({"data": {"type": "sources", "id": _ML_SRC, "attributes": {
        "name": "Camera 1", "filepath": "/Users/me/Pictures/logo.png"}},
        "included": [{"type": "filters", "id": _ML_FLT, "attributes": {
            "name": "Color Correction", "composition-id": "com.boinx.filter.colorcorrection",
            "input-values": {"tvIn_Amount": 0.5}},
            "relationships": {"source": {"data": {"type": "sources", "id": _ML_SRC}}}}]})},
    expect_state={"sources": {_ML_SRC: {"name": "Camera 1", "filepath": "/Users/me/Pictures/logo.png"}},
                  "filters": {_ML_FLT: {"name": "Color Correction",
                                        "composition_id": "com.boinx.filter.colorcorrection",
                                        "source": _ML_SRC, "input_values": '{"tvIn_Amount":0.5}'}}})
telemetry(ML, "pushed-filter-removal", inbound_ws=json.dumps({"event": "removed", "type": "filters", "id": _ML_FLT}),
          state_before={"filters": {_ML_FLT: {"name": "Color Correction"}, "F2": {"name": "Blur"}}},
          expect_state={"filters": {"F2": {"name": "Blur"}}})
# An output destination's settings.
telemetry(ML, "output-destination-settings", inbound_http={
    "path": _ML_D + "/output-destinations",
    "body": json.dumps({"data": [
        {"type": "output-destinations", "id": _ML_OUT, "attributes": {
            "title": "File Recording", "output-destination-type": "com.boinx.mimoLive.outputDestination.fileRecording",
            "settings": {"location": "~/Movies", "filename": "%show %year-%month-%day.%extension"}}},
        {"type": "output-destinations", "id": "S1", "attributes": {
            "title": "Stream", "settings": {"rtmpurl": "rtmp://a.rtmp.youtube.com/l***", "streamingkey": "****",
                                            "publicurl": "https://youtu.be/abc"}}}]})},
    expect_state={"output-destinations": {
        _ML_OUT: {"title": "File Recording",
                  "output_type": "com.boinx.mimoLive.outputDestination.fileRecording",
                  "location": "~/Movies", "filename": "%show %year-%month-%day.%extension"},
        "S1": {"title": "Stream", "rtmp_url": "rtmp://a.rtmp.youtube.com/l***",
               "public_url": "https://youtu.be/abc"}}})
# The device list, whole: a device no longer listed leaves state.
telemetry(ML, "devices-list", inbound_http={
    "path": "/api/v1/devices",
    "body": json.dumps({"data": [{"type": "devices", "id": "BuiltInMicrophoneDevice", "attributes": {
        "name": "MacBook Pro Microphone", "connected": True, "video": False, "audio": True,
        "device-type": "com.boinx.devicetype.avfoundation", "tally-state": "off",
        "input-channels": ["Ch. 1"]}}]})},
    state_before={"devices": {"OldCam": {"name": "Old Camera", "connected": False}}},
    expect_state={"devices": {"BuiltInMicrophoneDevice": {
        "name": "MacBook Pro Microphone", "connected": True, "video": False, "audio": True,
        "device_type": "com.boinx.devicetype.avfoundation", "tally_state": "off",
        "input_channels": '["Ch. 1"]'}}})
telemetry(ML, "accounts-list", inbound_http={
    "path": "/api/v1/accounts",
    "body": json.dumps({"data": [{"type": "accounts", "id": "6995291F-E0C3-4307-84E0-BEA3A3EA7A8E", "attributes": {
        "name": "Jane Doe", "account-type": "Zoom", "identifier": "aBcDeFgHiJ", "email": "jane@example.com"}}]})},
    expect_state={"accounts": {"6995291F-E0C3-4307-84E0-BEA3A3EA7A8E": {
        "name": "Jane Doe", "account_type": "Zoom", "identifier": "aBcDeFgHiJ", "email": "jane@example.com"}}})
# The Zoom meeting's participants (Boinx's reference), whole; an empty list
# (no meeting) clears them.
telemetry(ML, "zoom-participants", inbound_http={
    "path": "/api/v1/zoom/participants",
    "body": json.dumps({"data": [{"id": 16786432, "name": "John Doe", "userRole": "Host", "isHost": True,
                                  "isCoHost": False, "isVideoOn": True, "isAudioOn": True, "isTalking": False,
                                  "isRaisingHand": False}]})},
    state_before={"zoom": {"participants": {"5": {"name": "Gone"}}}},
    expect_state={"zoom": {"participants": {"16786432": {
        "name": "John Doe", "role": "Host", "host": True, "co_host": False, "video_on": True, "audio_on": True,
        "talking": False, "hand_raised": False}}}})
telemetry(ML, "zoom-participants-none", inbound_http={
    "path": "/api/v1/zoom/participants", "body": '{"data":[]}'},
    state_before={"zoom": {"participants": {"16786432": {"name": "John Doe"}}}},
    expect_state={"zoom": {}})
telemetry(ML, "zoom-action-rereads-participants", inbound_http={
    "path": "/api/v1/zoom/meetingaction?command=muteAll", "body": "{}"},
    expect_then_send=[{"method": "GET", "target": "/api/v1/zoom/participants"}],
    expect_state={})
# mlController's status.
telemetry(ML, "mlcontroller-status", inbound_http={"path": "/api/status", "body": json.dumps(_ML_STATUS)},
          expect_state={"mlcontroller": {
              "running": True, "selected_app": "mimoLive (6.15)",
              "selected_app_path": "/Applications/mimoLive.app",
              "available_apps": '[{"name":"mimoLive (6.15)","path":"/Applications/mimoLive.app"}]',
              "open_documents": '[{"id":"' + _ML_DOC + '","name":"My Show","path":"/Users/me/Documents/show1.tvshow"}]',
              "local_documents": '["/Users/me/Documents/show1.tvshow"]'}})
# A recall flag PATCH has the set read again, like a layers PATCH.
telemetry(ML, "layer-set-recall-flag-written", inbound_http={
    "path": _ML_LS, "request": {"data": {"attributes": {"recall-on-show-start": True}}},
    "body": json.dumps({"data": _ml_set({"name": "Intro Scene", "recall-on-show-start": True})})},
    expect_then_send=_ML_REREAD,
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "document": _ML_DOC,
                                           "recall_on_show_start": True}}})
# A GET of the set (no request body) is not read again.
telemetry(ML, "layer-set-get-not-reread", inbound_http={
    "path": _ML_LS, "body": json.dumps({"data": _ml_set({"name": "Intro Scene", "recall-on-show-end": True})})},
    expect_then_send=[],
    expect_state={"layer-sets": {_ML_SET: {"name": "Intro Scene", "document": _ML_DOC,
                                           "recall_on_show_end": True}}})
