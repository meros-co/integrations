# Streamlabs Desktop (streamlabs-desktop): JSON-RPC 2.0 requests
# {"jsonrpc","id","method","params":{"resource","args"}} as the API reference
# shows them (docs/index.html), written compact, with a string id naming what
# the request acts on; events {"result":{"_type":"EVENT","resourceId","data"}}.
SL = "streamlabs-desktop"


def _rpc(id_, method, resource, args, compact=False):
    """A request as the reference writes it; args is the JSON text of the
    argument list."""
    params = '{"resource":%s,"args":%s%s}' % (json.dumps(resource), args, ',"compactMode":true' if compact else "")
    return '{"jsonrpc":"2.0","id":%s,"method":%s,"params":%s}' % (json.dumps(id_), json.dumps(method), params)


def _sl(command, input, wire, reply_id=None, result="null", expect=None):
    rid = reply_id
    reply = '{"jsonrpc":"2.0","id":%s,"result":%s}' % (json.dumps(rid), result)
    text(SL, command, input, wire, device_reply=reply, expect_result=expect or {"ok": {"kind": "ack"}})


SC, IT, SRC = "scene_02da7db7", "b18ee2b0-6ff7", "color_source_61221191"
ITEM = 'SceneItem["%s","%s","%s"]' % (SC, IT, SRC)
_I = {"scene_id": SC, "item_id": IT, "source_id": SRC}


def _item(command, id_prefix, method, args, extra=None):
    rid = f"{id_prefix} {SC} {IT}"
    _sl(command, {**_I, **(extra or {})}, _rpc(rid, method, ITEM, args), rid)


# call: any method, returning its result.
_sl("call", {"resource": "ScenesService", "method": "getScenes", "args": []},
    _rpc("call", "getScenes", "ScenesService", "[]"), "call", result='[{"id":"3efd436e5546","name":"My super scene"}]',
    expect={"ok": {"kind": "value", "value": [{"id": "3efd436e5546", "name": "My super scene"}]}})
_sl("make_scene_active", {"scene_id": SC}, _rpc(f"activate {SC}", "makeSceneActive", "ScenesService", f'["{SC}"]'),
    f"activate {SC}", result="true")
_sl("create_scene", {"name": 'Say "hi"'}, _rpc("create scene", "createScene", "ScenesService", json.dumps(['Say "hi"']), True), "create scene")
_sl("remove_scene", {"scene_id": SC}, _rpc(f"remove scene {SC}", "removeScene", "ScenesService", f'["{SC}"]'), f"remove scene {SC}")
_sl("rename_scene", {"scene_id": SC, "name": "Intro"}, _rpc(f"rename scene {SC}", "setName", f'Scene["{SC}"]', '["Intro"]'), f"rename scene {SC}")
_sl("clear_scene", {"scene_id": SC}, _rpc(f"clear scene {SC}", "clear", f'Scene["{SC}"]', "[]"), f"clear scene {SC}")
_sl("add_source_to_scene", {"scene_id": SC, "source_id": SRC},
    _rpc(f"add source {SC} {SRC}", "addSource", f'Scene["{SC}"]', f'["{SRC}"]', True), f"add source {SC} {SRC}")
_sl("add_file_to_scene", {"scene_id": SC, "path": "C:\\Media\\logo.png"},
    _rpc(f"add file {SC}", "addFile", f'Scene["{SC}"]', json.dumps(["C:\\Media\\logo.png"]), True), f"add file {SC}")
_sl("create_source_in_scene", {"scene_id": SC, "name": "Red", "type": "color_source", "source_settings": {"color": 4278190335}},
    _rpc(f"create source in {SC}", "createAndAddSource", f'Scene["{SC}"]', '["Red","color_source",{"color":4278190335}]', True),
    f"create source in {SC}")
_sl("create_folder", {"scene_id": SC, "name": "Lower thirds"},
    _rpc(f"create folder {SC}", "createFolder", f'Scene["{SC}"]', '["Lower thirds"]', True), f"create folder {SC}")
_sl("remove_folder", {"scene_id": SC, "folder_id": "f1"}, _rpc(f"remove folder {SC} f1", "removeFolder", f'Scene["{SC}"]', '["f1"]'), f"remove folder {SC} f1")
_sl("rename_folder", {"scene_id": SC, "folder_id": "f1", "name": "Graphics"},
    _rpc(f"rename folder {SC} f1", "setName", f'SceneItemFolder["{SC}","f1"]', '["Graphics"]'), f"rename folder {SC} f1")
_sl("ungroup_folder", {"scene_id": SC, "folder_id": "f1"},
    _rpc(f"ungroup {SC} f1", "ungroup", f'SceneItemFolder["{SC}","f1"]', "[]"), f"ungroup {SC} f1")
_item("set_item_visibility", "visibility", "setVisibility", "[false]", {"visible": False})
_item("set_item_locked", "locked", "setSettings", '[{"locked":true}]', {"locked": True})
_item("set_item_stream_visible", "stream visible", "setSettings", '[{"streamVisible":false}]', {"visible": False})
_item("set_item_recording_visible", "recording visible", "setSettings", '[{"recordingVisible":true}]', {"visible": True})
_item("set_item_position", "position", "setTransform", '[{"position":{"x":0.00,"y":25.50}}]', {"x": 0.0, "y": 25.5})
_item("set_item_scale", "scale", "setTransform", '[{"scale":{"x":1.0000,"y":0.5000}}]', {"x": 1.0, "y": 0.5})
_item("set_item_rotation", "rotation", "setTransform", '[{"rotation":90.00}]', {"degrees": 90.0})
_item("set_item_crop", "crop", "setTransform", '[{"crop":{"top":10,"bottom":0,"left":5,"right":5}}]',
      {"top": 10, "bottom": 0, "left": 5, "right": 5})
_item("rotate_item", "rotate", "rotate", "[-45.00]", {"degrees": -45.0})
_item("center_item", "center", "centerOnScreen", "[]")
_item("fit_item", "fit", "fitToScreen", "[]")
_item("stretch_item", "stretch", "stretchToScreen", "[]")
_item("flip_item_x", "flip x", "flipX", "[]")
_item("flip_item_y", "flip y", "flipY", "[]")
_item("reset_item_transform", "reset transform", "resetTransform", "[]")
_item("place_item_after", "place after", "placeAfter", '["n2"]', {"node_id": "n2"})
_item("place_item_before", "place before", "placeBefore", '["n2"]', {"node_id": "n2"})
_item("set_item_parent", "parent", "setParent", '["f1"]', {"folder_id": "f1"})
_item("remove_item", "remove item", "remove", "[]")
_sl("create_source", {"name": "Browser", "type": "browser_source", "source_settings": {}},
    _rpc("create source", "createSource", "SourcesService", '["Browser","browser_source",{}]', True), "create source")
_sl("add_file_source", {"path": "/Users/a/clip.mp4"}, _rpc("add file source", "addFile", "SourcesService", '["/Users/a/clip.mp4"]', True), "add file source")
_sl("remove_source", {"source_id": SRC}, _rpc(f"remove source {SRC}", "removeSource", "SourcesService", f'["{SRC}"]'), f"remove source {SRC}")
_sl("rename_source", {"source_id": SRC, "name": "Background"}, _rpc(f"rename source {SRC}", "setName", f'Source["{SRC}"]', '["Background"]'), f"rename source {SRC}")
_sl("update_source_settings", {"source_id": SRC, "source_settings": {"url": "https://example.com"}},
    _rpc(f"update settings {SRC}", "updateSettings", f'Source["{SRC}"]', '[{"url":"https://example.com"}]'), f"update settings {SRC}")
_sl("refresh_source", {"source_id": SRC}, _rpc(f"refresh {SRC}", "refresh", f'Source["{SRC}"]', "[]"), f"refresh {SRC}")
_sl("duplicate_source", {"source_id": SRC}, _rpc(f"duplicate {SRC}", "duplicate", f'Source["{SRC}"]', "[]", True), f"duplicate {SRC}")
MIC = "wasapi_input_capture_d497319b"
_sl("set_source_muted", {"source_id": MIC, "muted": True}, _rpc(f"mute {MIC}", "setMuted", f'AudioSource["{MIC}"]', "[true]"), f"mute {MIC}")
_sl("set_audio_deflection", {"source_id": MIC, "deflection": 0.75},
    _rpc(f"deflection {MIC}", "setDeflection", f'AudioSource["{MIC}"]', "[0.7500]"), f"deflection {MIC}")
for _c, _id, _m, _r in [("toggle_streaming", "toggle streaming", "toggleStreaming", "StreamingService"),
                        ("toggle_recording", "toggle recording", "toggleRecording", "StreamingService"),
                        ("start_replay_buffer", "start replay buffer", "startReplayBuffer", "StreamingService"),
                        ("stop_replay_buffer", "stop replay buffer", "stopReplayBuffer", "StreamingService"),
                        ("save_replay", "save replay", "saveReplay", "StreamingService"),
                        ("enable_studio_mode", "enable studio mode", "enableStudioMode", "TransitionsService"),
                        ("disable_studio_mode", "disable studio mode", "disableStudioMode", "TransitionsService"),
                        ("execute_studio_mode_transition", "studio transition", "executeStudioModeTransition", "TransitionsService"),
                        ("mark_all_notifications_read", "notifications read", "markAllAsRead", "NotificationsService")]:
    _sl(_c, {}, _rpc(_id, _m, _r, "[]"), _id)
COL = "d3165bf4-2690-426a-80a1-f3421658e04e"
_sl("load_scene_collection", {"collection_id": COL}, _rpc(f"load collection {COL}", "load", "SceneCollectionsService", f'["{COL}"]'),
    f"load collection {COL}", result='{"_type":"SUBSCRIPTION","resourceId":"5c3cf84f797a","emitter":"PROMISE"}')
_sl("create_scene_collection", {"name": "Show"}, _rpc("create collection", "create", "SceneCollectionsService", '[{"name":"Show"}]'), "create collection")
_sl("delete_scene_collection", {"collection_id": COL}, _rpc(f"delete collection {COL}", "delete", "SceneCollectionsService", f'["{COL}"]'), f"delete collection {COL}")
_sl("rename_scene_collection", {"collection_id": COL, "name": "Show 2"},
    _rpc(f"rename collection {COL}", "rename", "SceneCollectionsService", f'["Show 2","{COL}"]'), f"rename collection {COL}")
_sl("push_notification", {"message": "Going live", "type": "WARNING", "lifetime": 5000},
    _rpc("notify", "push", "NotificationsService", '[{"message":"Going live","type":"WARNING","lifeTime":5000}]'), "notify")
_sl("set_notification_settings", {"enabled": True, "play_sound": False},
    _rpc("notification settings", "setSettings", "NotificationsService", '[{"enabled":true,"playSound":false}]'), "notification settings")
# An error reply fails the command.
text(SL, "remove_source", {"source_id": SRC}, _rpc(f"remove source {SRC}", "removeSource", "SourcesService", f'["{SRC}"]'),
     device_reply='{"jsonrpc":"2.0","id":"remove source %s","error":{"code":-32600,"message":"Source not found"}}' % SRC,
     expect_result={"error": {"error": "device_error"}}, file="remove_source-error")


def _event(resource, data):
    return json.dumps({"jsonrpc": "2.0", "result": {"_type": "EVENT", "emitter": "STREAM", "resourceId": resource, "data": data}})


telemetry(SL, "scenes", inbound_ws=json.dumps({"jsonrpc": "2.0", "id": "scenes", "result": [
    {"_type": "HELPER", "resourceId": 'Scene["scene_02da7db7"]', "id": "scene_02da7db7", "name": "Scene", "nodes": [
        {"id": "b18ee2b0-6ff7", "sceneItemId": "b18ee2b0-6ff7", "sceneId": "scene_02da7db7", "sceneNodeType": "item",
         "name": "Color", "sourceId": "color_source_61221191", "visible": True, "locked": False, "streamVisible": True,
         "recordingVisible": True, "transform": {"position": {"x": 0, "y": 25}, "scale": {"x": 1, "y": 1}, "rotation": 0,
                                                 "crop": {"top": 0, "bottom": 0, "left": 0, "right": 0}}},
        {"id": "f1", "sceneId": "scene_02da7db7", "sceneNodeType": "folder", "name": "Graphics", "childrenIds": []}]}]}),
    state_before={"scenes": {"old": {"name": "Gone"}}},
    expect_state={"scenes": {"scene_02da7db7": {"name": "Scene", "index": 0, "nodes": {
        "b18ee2b0-6ff7": {"type": "item", "name": "Color", "source_id": "color_source_61221191", "visible": True, "locked": False,
                          "stream_visible": True, "recording_visible": True, "position": {"x": 0.0, "y": 25.0},
                          "scale": {"x": 1.0, "y": 1.0}, "rotation": 0.0, "crop": {"top": 0, "bottom": 0, "left": 0, "right": 0}},
        "f1": {"type": "folder", "name": "Graphics"}}}}},
    settings={"token": "t0k"})
telemetry(SL, "item-updated", inbound_ws=_event("ScenesService.itemUpdated", {
    "id": "b18ee2b0-6ff7", "sceneItemId": "b18ee2b0-6ff7", "sceneId": "scene_02da7db7", "sceneNodeType": "item", "name": "Color",
    "sourceId": "color_source_61221191", "visible": False, "locked": False, "streamVisible": True, "recordingVisible": True,
    "transform": {"position": {"x": 10, "y": 20}, "scale": {"x": 0.5, "y": 0.5}, "rotation": 90,
                  "crop": {"top": 1, "bottom": 2, "left": 3, "right": 4}}}),
    expect_state={"scenes": {"scene_02da7db7": {"nodes": {"b18ee2b0-6ff7": {
        "type": "item", "name": "Color", "source_id": "color_source_61221191", "visible": False, "locked": False,
        "stream_visible": True, "recording_visible": True, "position": {"x": 10.0, "y": 20.0}, "scale": {"x": 0.5, "y": 0.5},
        "rotation": 90.0, "crop": {"top": 1, "bottom": 2, "left": 3, "right": 4}}}}}},
    settings={"token": "t0k"})
telemetry(SL, "active-scene", inbound_ws='{"jsonrpc":"2.0","id":"active scene","result":"scene_02da7db7"}',
          expect_state={"scene": {"active_id": "scene_02da7db7"}}, settings={"token": "t0k"})
telemetry(SL, "scene-switched", inbound_ws=_event("ScenesService.sceneSwitched", {"id": "5c3cf84f797a", "name": "NewScene (1)", "activeItemIds": []}),
          expect_state={"scene": {"active_id": "5c3cf84f797a", "active_name": "NewScene (1)"}}, settings={"token": "t0k"})
telemetry(SL, "item-added", inbound_ws=_event("ScenesService.itemAdded", {"id": "n9", "sceneId": "scene_02da7db7"}),
          expect_then_send=['{"jsonrpc":"2.0","id":"scenes","method":"getScenes","params":{"resource":"ScenesService","args":[]}}'],
          expect_state={}, settings={"token": "t0k"})
telemetry(SL, "source-added", inbound_ws=_event("SourcesService.sourceAdded", {"sourceId": "image_source_1", "name": "Logo"}),
          expect_then_send=['{"jsonrpc":"2.0","id":"sources","method":"getSources","params":{"resource":"SourcesService","args":[]}}',
                            '{"jsonrpc":"2.0","id":"audio","method":"getSources","params":{"resource":"AudioService","args":[]}}'],
          expect_state={}, settings={"token": "t0k"})
telemetry(SL, "collection-switched", inbound_ws=_event("SceneCollectionsService.collectionSwitched", {"id": "c2", "name": "Show"}),
          expect_then_send=[
              '{"jsonrpc":"2.0","id":"collections","method":"collections","params":{"resource":"SceneCollectionsService","args":[]}}',
              '{"jsonrpc":"2.0","id":"active collection","method":"activeCollection","params":{"resource":"SceneCollectionsService","args":[]}}',
              '{"jsonrpc":"2.0","id":"scenes","method":"getScenes","params":{"resource":"ScenesService","args":[]}}',
              '{"jsonrpc":"2.0","id":"active scene","method":"activeSceneId","params":{"resource":"ScenesService","args":[]}}',
              '{"jsonrpc":"2.0","id":"sources","method":"getSources","params":{"resource":"SourcesService","args":[]}}'],
          expect_state={}, settings={"token": "t0k"})
telemetry(SL, "sources", inbound_ws=json.dumps({"jsonrpc": "2.0", "id": "sources", "result": [
    {"_type": "HELPER", "resourceId": 'Source["wasapi_input_capture_d497319b"]', "sourceId": "wasapi_input_capture_d497319b",
     "id": "wasapi_input_capture_d497319b", "name": "Mic/Aux", "type": "wasapi_input_capture", "audio": True, "video": False,
     "muted": False, "width": 0, "height": 0, "async": False, "doNotDuplicate": True, "channel": 3}]}),
    expect_state={"sources": {"wasapi_input_capture_d497319b": {"name": "Mic/Aux", "type": "wasapi_input_capture", "audio": True,
                                                                "video": False, "muted": False, "width": 0, "height": 0,
                                                                "async": False, "channel": 3}}},
    settings={"token": "t0k"})
telemetry(SL, "source-updated", inbound_ws=_event("SourcesService.sourceUpdated", {
    "sourceId": "image_source_1", "name": "Logo 2", "type": "image_source", "audio": False, "video": True, "muted": False,
    "width": 400, "height": 200, "async": False}),
    expect_state={"sources": {"image_source_1": {"name": "Logo 2", "type": "image_source", "audio": False, "video": True,
                                                 "muted": False, "width": 400, "height": 200, "async": False}}},
    settings={"token": "t0k"})
telemetry(SL, "audio", inbound_ws=json.dumps({"jsonrpc": "2.0", "id": "audio", "result": [
    {"_type": "HELPER", "resourceId": 'AudioSource["wasapi_input_capture_d497319b"]', "sourceId": "wasapi_input_capture_d497319b",
     "name": "AuxAudioDevice1", "muted": True, "fader": {"db": -6.5, "deflection": 0.75, "mul": 0.47}, "audioMixers": 1,
     "monitoringType": 0, "forceMono": False, "syncOffset": 0, "mixerHidden": False}]}),
    expect_state={"audio": {"wasapi_input_capture_d497319b": {"name": "AuxAudioDevice1", "muted": True, "db": -6.5, "deflection": 0.75,
                                                              "mul": 0.47, "mixers": 1, "monitoring_type": 0, "force_mono": False,
                                                              "sync_offset": 0.0, "mixer_hidden": False}}},
    settings={"token": "t0k"})
telemetry(SL, "streaming", inbound_ws=json.dumps({"jsonrpc": "2.0", "id": "streaming", "result": {
    "streamingStatus": "live", "streamingStatusTime": "2026-10-23T18:00:00.000Z", "recordingStatus": "offline",
    "recordingStatusTime": "2026-10-23T17:00:00.000Z", "replayBufferStatus": "running", "replayBufferStatusTime": "2026-10-23T17:30:00.000Z"}}),
    expect_state={"streaming": {"status": "live", "status_time": "2026-10-23T18:00:00.000Z"},
                  "recording": {"status": "offline", "status_time": "2026-10-23T17:00:00.000Z"},
                  "replay_buffer": {"status": "running", "status_time": "2026-10-23T17:30:00.000Z"}},
    settings={"token": "t0k"})
telemetry(SL, "streaming-event", inbound_ws=_event("StreamingService.streamingStatusChange", "starting"),
          expect_then_send=['{"jsonrpc":"2.0","id":"streaming","method":"getModel","params":{"resource":"StreamingService","args":[]}}'],
          expect_state={"streaming": {"status": "starting"}}, settings={"token": "t0k"})
telemetry(SL, "recording-event", inbound_ws=_event("StreamingService.recordingStatusChange", "recording"),
          expect_then_send=['{"jsonrpc":"2.0","id":"streaming","method":"getModel","params":{"resource":"StreamingService","args":[]}}'],
          expect_state={"recording": {"status": "recording"}}, settings={"token": "t0k"})
telemetry(SL, "replay-buffer-event", inbound_ws=_event("StreamingService.replayBufferStatusChange", "saving"),
          expect_then_send=['{"jsonrpc":"2.0","id":"streaming","method":"getModel","params":{"resource":"StreamingService","args":[]}}'],
          expect_state={"replay_buffer": {"status": "saving"}}, settings={"token": "t0k"})
telemetry(SL, "studio-mode", inbound_ws='{"jsonrpc":"2.0","id":"transitions","result":{"studioMode":true}}',
          expect_state={"studio_mode": True}, settings={"token": "t0k"})
telemetry(SL, "studio-mode-event", inbound_ws=_event("TransitionsService.studioModeChanged", False),
          expect_state={"studio_mode": False}, settings={"token": "t0k"})
telemetry(SL, "collections", inbound_ws='{"jsonrpc":"2.0","id":"collections","result":[{"id":"d3165bf4","name":"Show"}]}',
          state_before={"collections": {"old": {"name": "Old"}}},
          expect_state={"collections": {"d3165bf4": {"name": "Show"}}}, settings={"token": "t0k"})
telemetry(SL, "active-collection", inbound_ws='{"jsonrpc":"2.0","id":"active collection","result":{"id":"d3165bf4","name":"Show"}}',
          expect_state={"collection": {"active_id": "d3165bf4", "active_name": "Show"}}, settings={"token": "t0k"})
telemetry(SL, "performance", inbound_ws=json.dumps({"jsonrpc": "2.0", "id": "performance", "result": {
    "CPU": 12.5, "frameRate": 60, "numberDroppedFrames": 3, "percentageDroppedFrames": 0.1, "bandwidth": 6000}}),
    expect_state={"performance": {"cpu": 12.5, "frame_rate": 60.0, "dropped_frames": 3, "dropped_frames_percent": 0.1, "bandwidth": 6000.0}},
    settings={"token": "t0k"})
telemetry(SL, "notification-settings", inbound_ws='{"jsonrpc":"2.0","id":"notification settings get","result":{"enabled":true,"playSound":false}}',
          expect_state={"notifications": {"enabled": True, "play_sound": False}}, settings={"token": "t0k"})

for _v in V:
    if _v["spec"] == SL:
        _v.setdefault("settings", {"token": "t0k"})
