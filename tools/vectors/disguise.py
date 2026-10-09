# disguise Designer (disguise): one vector per command. Paths and request
# bodies are those of disguise's Swagger documents session.swagger.json and
# service.swagger.json (basePath /api/session and /api/service), with
# Locators given by name; replies use the documented envelope {"status":
# {"code", "message", "details"}, "result"}. The Live Update subscription and
# valuesChanged message follow the Developer Portal's "Live Update API" page.
DG = "disguise"


def _dg(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": DG, "command": command, "input": input, "expect_request": request, **extra})


_ENV_OK = json.dumps({"status": {"code": 0, "message": "", "details": []}})
_OK = {"http_reply": {"status": 200, "body": _ENV_OK}, "expect_result": {"ok": {"kind": "ack"}}}
# A partial failure answers 200 with a non-zero status.code (Error Handling).
_PARTIAL = {"http_reply": {"status": 200, "body": json.dumps(
    {"status": {"code": 4000, "message": "invalid playmode", "details": [{"message": "LoopSection"}]}})},
    "expect_result": {"error": {"error": "device_error"}}}
_T = '{"transports":[{"name":"default"}]}'
_S = "/api/session/transport/"


# Play modes.
_dg("play", {}, "POST", _S + "play", _T, **_OK)
_dg("play_section", {"transport": "Main Stage"}, "POST", _S + "playsection",
    '{"transports":[{"name":"Main Stage"}]}')
_dg("loop_section", {}, "POST", _S + "playloopsection", _T)
_dg("stop", {}, "POST", _S + "stop", _T, **_OK)
_dg("return_to_start", {}, "POST", _S + "returntostart", _T)

# Jumps.
_dg("next_section", {}, "POST", _S + "gotonextsection",
    '{"transports":[{"transport":{"name":"default"},"playmode":"NotSet"}]}', **_OK)
_dg("previous_section", {"playmode": "Play"}, "POST", _S + "gotoprevsection",
    '{"transports":[{"transport":{"name":"default"},"playmode":"Play"}]}')
_dg("next_track", {"playmode": "Stop"}, "POST", _S + "gotonexttrack",
    '{"transports":[{"transport":{"name":"default"},"playmode":"Stop"}]}')
_dg("previous_track", {}, "POST", _S + "gotoprevtrack",
    '{"transports":[{"transport":{"name":"default"},"playmode":"NotSet"}]}')
_dg("go_to_section", {"section": 2, "playmode": "PlaySection"}, "POST", _S + "gotosection",
    '{"transports":[{"transport":{"name":"default"},"section":"2","playmode":"PlaySection"}]}', **_OK)
# The Swagger document's own example: transport "default", track "track 1", PlaySection.
_dg("go_to_track", {"track": "track 1", "playmode": "PlaySection"}, "POST", _S + "gototrack",
    '{"transports":[{"transport":{"name":"default"},"track":{"name":"track 1"},"playmode":"PlaySection"}]}',
    **_PARTIAL)
_dg("go_to_note", {"note": "Act 2 \"Storm\""}, "POST", _S + "gotonote",
    '{"transports":[{"transport":{"name":"default"},"note":"Act 2 \\"Storm\\"","playmode":"NotSet"}]}')
_dg("go_to_tag", {"value": "1.2.5", "allow_global": True, "playmode": "Play"}, "POST", _S + "gototag",
    '{"transports":[{"transport":{"name":"default"},"type":"CUE","value":"1.2.5","allowGlobalJump":true,'
    '"playmode":"Play"}]}', **_OK)
_dg("go_to_time", {"time": 12.5}, "POST", _S + "gototime",
    '{"transports":[{"transport":{"name":"default"},"time":12.500,"playmode":"NotSet"}]}')
_dg("go_to_frame", {"frame": 300}, "POST", _S + "gotoframe",
    '{"transports":[{"transport":{"name":"default"},"frame":"300","playmode":"NotSet"}]}')
_dg("go_to_timecode", {"timecode": "01:00:10:12"}, "POST", _S + "gototimecode",
    '{"transports":[{"transport":{"name":"default"},"timecode":"01:00:10:12","ignoreTags":false,'
    '"playmode":"NotSet"}]}')

# Settings.
_dg("set_engaged", {"engaged": False}, "POST", _S + "engaged",
    '{"transports":[{"transport":{"name":"default"},"engaged":false}]}', **_OK)
_dg("set_volume", {"volume": 0.8}, "POST", _S + "volume",
    '{"transports":[{"transport":{"name":"default"},"volume":0.800}]}')
_dg("set_brightness", {"brightness": 1.0}, "POST", _S + "brightness",
    '{"transports":[{"transport":{"name":"default"},"brightness":1.000}]}')

# Queries.
ACTIVE = [{"uid": "2276480868532234653", "name": "default", "engaged": True, "volume": 1.0, "brightness": 1.0,
           "playmode": "Play", "currentTrack": {"uid": "123", "name": "track 1"}, "receivingTimecode": False}]
_dg("get_active_transport", {}, "GET", _S + "activetransport",
    http_reply={"status": 200, "body": json.dumps({"status": {"code": 0, "message": ""}, "result": ACTIVE})},
    expect_result={"ok": {"kind": "value", "value": ACTIVE}})
TRANSPORTS = {"status": {"code": 0, "message": ""},
              "transports": [{"uid": "2276480868532234653", "name": "default", "engaged": True, "volume": 0.5,
                              "brightness": 1.0, "playmode": "Stop",
                              "currentTrack": {"uid": "123", "name": "track 1"}, "receivingTimecode": False,
                              "setList": {"uid": "77", "name": "Show", "tracks": []}}],
              "multitransports": []}
_dg("get_transports", {}, "GET", _S + "transports",
    http_reply={"status": 200, "body": json.dumps(TRANSPORTS)},
    expect_result={"ok": {"kind": "value", "value": TRANSPORTS}})
_dg("get_tracks", {}, "GET", _S + "tracks")
_dg("get_setlists", {}, "GET", _S + "setlists")
_dg("get_annotations", {"track": "track 1"}, "GET", _S + "annotations?name=track%201")

# Status.
_dg("get_health", {}, "GET", "/api/session/status/health")
_dg("get_notifications", {}, "GET", "/api/session/status/notifications")
_dg("get_project", {}, "GET", "/api/session/status/project",
    http_reply={"status": 200, "body": json.dumps({"status": {"code": 0}, "result": {
        "projectPath": "projects/show/show.d3", "version": {"major": 30, "minor": 8, "hotfix": 3,
                                                            "revision": 191234, "releaseType": "Pro"}}})},
    expect_result={"ok": {"kind": "value", "value": {
        "projectPath": "projects/show/show.d3", "version": {"major": 30, "minor": 8, "hotfix": 3,
                                                            "revision": 191234, "releaseType": "Pro"}}}})
_dg("get_session", {}, "GET", "/api/session/status/session")
_dg("set_gui_mode", {"machine": "actor-1", "mode": "AlwaysOff"}, "POST", "/api/session/status/setguimode",
    '{"machine":{"name":"actor-1"},"mode":"AlwaysOff"}', **_OK)

# Failover.
_dg("get_failover_settings", {}, "GET", "/api/session/failover/settings")
_dg("failover_machine", {"machine": "actor-1"}, "POST", "/api/session/failover/failovermachine",
    '{"machine":{"name":"actor-1"}}', **_OK)
_dg("restore_machine", {"machine": "actor-1"}, "POST", "/api/session/failover/restoremachine",
    '{"machine":{"name":"actor-1"}}')

# Service API.
_dg("detect_systems", {}, "GET", "/api/service/system/detectsystems")
_dg("get_os_info", {}, "GET", "/api/service/system/osinfo")
_dg("get_network_adapters", {}, "GET", "/api/service/system/networkadapters")
_dg("get_gpu_outputs", {}, "GET", "/api/service/system/gpuoutputs")
_dg("get_projects", {}, "GET", "/api/service/system/projects")
_dg("start_project", {"path": "C:/Users/d3/Documents/Renderstream Projects/show/show.d3"}, "POST",
    "/api/service/project/startlocalproject",
    '{"projectPath":"C:/Users/d3/Documents/Renderstream Projects/show/show.d3","soloMode":false,'
    '"allowUpgrade":false}', **_OK)
_dg("quit_project", {}, "POST", "/api/service/project/quitlocalproject", **_OK)
_dg("restart_project", {}, "POST", "/api/service/project/restartlocalproject")
_dg("force_quit_project", {}, "POST", "/api/service/project/forcequitlocalproject",
    http_reply={"status": 500, "body": ""}, expect_result={"error": {"error": "device_error", "code": "500"}})

# ── Telemetry ──
LIVE_SUB = ('{"subscribe":{"object":"transportmanager:default","properties":["{\'playing\': '
            'object.player.playing, \'time\': object.player.tRender, \'mode\': str(object.player.playMode.state), '
            '\'track\': str(object.player.track.description), \'engaged\': object.engaged, \'volume\': '
            'object.volume, \'brightness\': object.brightness}"],"configuration":{"updateFrequencyMs":250}}}')
telemetry(DG, "transports", inbound_http={"path": "/api/session/transport/transports",
                                          "body": json.dumps(TRANSPORTS)},
          state_before={"transports": {"99": {"name": "removed", "engaged": False}}},
          expect_state={"transports": {"2276480868532234653": {
              "name": "default", "engaged": True, "volume": 0.5, "brightness": 1.0, "playmode": "Stop",
              "current_track": "track 1", "receiving_timecode": False, "setlist": "Show"}}})
telemetry(DG, "health", inbound_http={"path": "/api/session/status/health", "body": json.dumps({
    "status": {"code": 0}, "result": [{"machine": {"uid": "1", "name": "director", "hostname": "VX4-01"},
                                       "status": {"averageFPS": 59.94, "videoDroppedFrames": 0,
                                                  "videoMissedFrames": 2, "states": []}}]})},
          state_before={"machines": {"understudy": {"hostname": "VX4-02", "average_fps": 60.0}}},
          expect_state={"machines": {"director": {"hostname": "VX4-01", "average_fps": 59.94,
                                                  "video_dropped_frames": 0.0, "video_missed_frames": 2.0}}})
telemetry(DG, "project", inbound_http={"path": "/api/session/status/project", "body": json.dumps({
    "status": {"code": 0}, "result": {"projectPath": "projects/show/show.d3",
                                      "version": {"major": 30, "minor": 8, "hotfix": 3, "revision": 191234}}})},
          expect_state={"project": {"path": "projects/show/show.d3", "version": "30.8.3.191234"}})
telemetry(DG, "live-transport", expect_connect_ws=[LIVE_SUB],
          inbound_ws=json.dumps({"valuesChanged": [{"id": 1, "value": {
              "playing": True, "time": 83.25, "mode": "PlaySection", "track": "track 1", "engaged": True,
              "volume": 1.0, "brightness": 0.75}, "changeTimestamp": 257.1, "messageTimestamp": 265.59}]}),
          expect_state={"live": {"playing": True, "time": 83.25, "play_mode": "PlaySection", "track": "track 1",
                                 "engaged": True, "volume": 1.0, "brightness": 0.75}})
telemetry(DG, "live-error", inbound_ws='{"error":"Unable to subscribe to transportmanager:default"}',
          expect_state={"live": {"error": "Unable to subscribe to transportmanager:default"}})

# ── Transports, status and failover: the rest of what they read ──
_dg("apply_default_routing", {}, "POST", "/api/session/failover/applydefaultrouting", **_OK)
UNDERSTUDIES = {"understudy-1": {"targets": [{"uid": "4411", "name": "actor-1"}, {"uid": "4412", "name": "actor-2"}]}}
_dg("get_understudy_targets", {}, "GET", "/api/session/failover/understudytargets",
    http_reply={"status": 200, "body": json.dumps({"status": {"code": 0, "message": ""},
                                                   "understudies": UNDERSTUDIES})},
    expect_result={"ok": {"kind": "value", "value": UNDERSTUDIES}})


def _envelope(**fields):
    return json.dumps({"status": {"code": 0, "message": "", "details": []}, **fields})


def _compact(value):
    return json.dumps(value, separators=(",", ":"))


telemetry(DG, "transports-current-track-annotations", inbound_http={
    "path": "/api/session/transport/transports", "body": json.dumps(TRANSPORTS)},
    expect_then_send=[{"method": "GET", "target": "/api/session/transport/annotations?uid=123"}],
    expect_state={"transports": {"2276480868532234653": {
        "name": "default", "engaged": True, "volume": 0.5, "brightness": 1.0, "playmode": "Stop",
        "current_track": "track 1", "receiving_timecode": False, "setlist": "Show"}}})
telemetry(DG, "multitransports", inbound_http={"path": "/api/session/transport/transports", "body": _envelope(
    transports=[], multitransports=[{"uid": "5001", "name": "All screens", "engaged": True,
                                     "transports": ["2276480868532234653", "2276480868532234654"]}])},
    state_before={"multitransports": {"5000": {"name": "old", "engaged": False}}},
    expect_state={"multitransports": {"5001": {
        "name": "All screens", "engaged": True,
        "transports": '["2276480868532234653","2276480868532234654"]'}}})
telemetry(DG, "active-transport", inbound_http={"path": "/api/session/transport/activetransport",
                                                "body": _envelope(result=ACTIVE)},
          state_before={"active_transport": {"99": {"name": "other"}}},
          expect_state={"active_transport": {"2276480868532234653": {"name": "default"}}})
telemetry(DG, "tracks", inbound_http={"path": "/api/session/transport/tracks", "body": _envelope(result=[
    {"uid": "123", "name": "track 1", "length": 300.0, "crossfade": "Off"},
    {"uid": "124", "name": "track 2", "length": 95.5, "crossfade": "Fade"}])},
    state_before={"tracks": {"9": {"name": "deleted"}}},
    expect_state={"tracks": {"123": {"name": "track 1", "length": 300.0, "crossfade": "Off"},
                             "124": {"name": "track 2", "length": 95.5, "crossfade": "Fade"}}})
telemetry(DG, "setlists", inbound_http={"path": "/api/session/transport/setlists", "body": _envelope(result=[
    {"uid": "77", "name": "Show", "tracks": [{"uid": "123", "name": "track 1", "length": 300.0, "crossfade": "Off"},
                                            {"uid": "124", "name": "track 2", "length": 95.5,
                                             "crossfade": "Fade"}]}])},
    state_before={"setlists": {"77": {"name": "Show", "tracks": {"2": {"uid": "125", "name": "cut"}}}}},
    expect_state={"setlists": {"77": {"name": "Show", "tracks": {"0": {"uid": "123", "name": "track 1"},
                                                                 "1": {"uid": "124", "name": "track 2"}}}}})
ANNOTATIONS = {"notes": [{"time": 10.0, "text": "Act 1"}],
               "tags": [{"time": 10.0, "type": "CUE", "value": "1.2.5"}],
               "sections": [{"time": 0.0, "index": "0"}, {"time": 10.0, "index": "1"}]}
telemetry(DG, "annotations", inbound_http={
    "path": "/api/session/transport/annotations?uid=123", "body": _envelope(result={
        "uid": "123", "name": "track 1", "annotations": ANNOTATIONS})},
    expect_state={"annotations": {"123": {k: _compact(v) for k, v in ANNOTATIONS.items()}}})
telemetry(DG, "transport-command-reread", inbound_http={"path": "/api/session/transport/gototrack",
                                                        "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": "/api/session/transport/transports"}],
          expect_state={})
telemetry(DG, "health-states", inbound_http={"path": "/api/session/status/health", "body": _envelope(result=[
    {"machine": {"uid": "2", "name": "understudy", "hostname": "VX4-02"},
     "runningAsMachine": {"uid": "1", "name": "actor-1", "hostname": "VX4-01"},
     "status": {"averageFPS": 60.0, "videoDroppedFrames": 0, "videoMissedFrames": 0, "states": [
         {"name": "Genlock", "detail": "No reference", "category": "genlock", "severity": "warning"}]}}])},
    expect_state={"machines": {"understudy": {
        "hostname": "VX4-02", "running_as": "actor-1", "average_fps": 60.0, "video_dropped_frames": 0.0,
        "video_missed_frames": 0.0, "states": {"0": {"name": "Genlock", "detail": "No reference",
                                                     "category": "genlock", "severity": "warning"}}}}})
telemetry(DG, "notifications", inbound_http={"path": "/api/session/status/notifications", "body": _envelope(result=[
    {"machine": {"uid": "1", "name": "director", "hostname": "VX4-01"},
     "notifications": [{"summary": "Missing media", "detail": "objects/videofile/intro.mov"}]}])},
    state_before={"notifications": {"actor-1": {"0": {"summary": "gone", "detail": ""}}}},
    expect_state={"notifications": {"director": {"0": {"summary": "Missing media",
                                                       "detail": "objects/videofile/intro.mov"}}}})
telemetry(DG, "session", inbound_http={"path": "/api/session/status/session", "body": _envelope(result={
    "isRunningSolo": False, "isDirectorDedicated": True,
    "director": {"uid": "11", "name": "director", "hostname": "VX4-01", "type": "Vx4", "guiMode": "AlwaysOn",
                 "guiVisible": True},
    "actors": [{"uid": "12", "name": "actor-1", "hostname": "VX4-02", "type": "Vx4", "guiMode": "OffWhenActor",
                "guiVisible": False}],
    "understudies": [{"uid": "13", "name": "understudy", "hostname": "VX4-03", "type": "Vx4",
                      "guiMode": "AlwaysOff", "guiVisible": False}]})},
    state_before={"session": {"machines": {"old-actor": {"role": "actor"}}}},
    expect_state={"session": {"solo": False, "director_dedicated": True, "director": "director", "machines": {
        "director": {"role": "director", "uid": "11", "hostname": "VX4-01", "type": "Vx4", "gui_mode": "AlwaysOn",
                     "gui_visible": True},
        "actor-1": {"role": "actor", "uid": "12", "hostname": "VX4-02", "type": "Vx4", "gui_mode": "OffWhenActor",
                    "gui_visible": False},
        "understudy": {"role": "understudy", "uid": "13", "hostname": "VX4-03", "type": "Vx4",
                       "gui_mode": "AlwaysOff", "gui_visible": False}}}})
telemetry(DG, "gui-mode-reread", inbound_http={"path": "/api/session/status/setguimode", "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": "/api/session/status/session"}], expect_state={})
telemetry(DG, "failover-settings", inbound_http={"path": "/api/session/failover/settings", "body": _envelope(
    timeout=2.5, normalPreset="normal", currentPreset="failed over")},
    expect_state={"failover": {"timeout": 2.5, "normal_preset": "normal", "current_preset": "failed over"}})
telemetry(DG, "understudy-targets", inbound_http={"path": "/api/session/failover/understudytargets",
                                                  "body": _envelope(understudies=UNDERSTUDIES)},
          expect_state={"failover": {"understudy_targets": _compact(UNDERSTUDIES)}})
telemetry(DG, "failover-reread", inbound_http={"path": "/api/session/failover/failovermachine", "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": "/api/session/status/health"},
                            {"method": "GET", "target": "/api/session/status/session"},
                            {"method": "GET", "target": "/api/session/failover/settings"}], expect_state={})

# ── Sequencing: indirections ──
INDIRECTIONS = [{"uid": "301", "name": "main content", "resourceType": "VideoClip",
                 "currentResource": {"uid": "401", "name": "intro.mov"}}]
_dg("get_indirections", {}, "GET", "/api/session/sequencing/indirections",
    http_reply={"status": 200, "body": _envelope(result=INDIRECTIONS)},
    expect_result={"ok": {"kind": "value", "value": INDIRECTIONS}})
_dg("get_indirection_resources", {"indirection": "main content"}, "GET",
    "/api/session/sequencing/indirectionresources?name=main%20content")
_dg("change_indirection", {"indirection": "main content", "resource": "act 2.mov"}, "POST",
    "/api/session/sequencing/changeindirections",
    '{"changes":[{"indirection":{"name":"main content"},"resource":{"name":"act 2.mov"}}]}', **_OK)
_dg("change_indirections", {"changes": [{"indirection": {"uid": "301"}, "resource": {"uid": "402"}},
                                        {"indirection": {"name": "logo"}, "resource": {"name": "logo b"}}]},
    "POST", "/api/session/sequencing/changeindirections",
    '{"changes":[{"indirection":{"uid":"301"},"resource":{"uid":"402"}},'
    '{"indirection":{"name":"logo"},"resource":{"name":"logo b"}}]}', **_OK)
telemetry(DG, "indirections", inbound_http={"path": "/api/session/sequencing/indirections",
                                            "body": _envelope(result=INDIRECTIONS)},
          state_before={"indirections": {"300": {"name": "removed"}}},
          expect_then_send=[{"method": "GET", "target": "/api/session/sequencing/indirectionresources?uid=301"}],
          expect_state={"indirections": {"301": {"name": "main content", "resource_type": "VideoClip",
                                                 "resource_uid": "401", "resource": "intro.mov"}}})
telemetry(DG, "indirection-resources", inbound_http={
    "path": "/api/session/sequencing/indirectionresources?uid=301",
    "body": _envelope(result=[{"uid": "401", "name": "intro.mov"}, {"uid": "402", "name": "act 2.mov"}])},
    state_before={"indirection_resources": {"301": {"400": "old.mov"}}},
    expect_state={"indirection_resources": {"301": {"401": "intro.mov", "402": "act 2.mov"}}})
telemetry(DG, "indirection-change-reread", inbound_http={"path": "/api/session/sequencing/changeindirections",
                                                         "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": "/api/session/sequencing/indirections"}], expect_state={})

# ── RenderStream ──
_RS = "/api/session/renderstream/"
RS_LAYERS = [{"uid": "601", "name": "Unreal scene"}]
_dg("get_renderstream_layers", {}, "GET", _RS + "layers",
    http_reply={"status": 200, "body": _envelope(result=RS_LAYERS)},
    expect_result={"ok": {"kind": "value", "value": RS_LAYERS}})
_dg("get_renderstream_layer_status", {"layer": "Unreal scene"}, "GET", _RS + "layerstatus?name=Unreal%20scene")
_dg("get_renderstream_layer_config", {"layer": "Unreal scene"}, "GET", _RS + "layerconfig?name=Unreal%20scene")
_dg("get_renderstream_pools", {}, "GET", _RS + "pools")
_dg("get_renderstream_assigners", {}, "GET", _RS + "assigners")
for _cmd, _path in [("start_renderstream_layer", "startlayers"), ("stop_renderstream_layer", "stoplayers"),
                    ("restart_renderstream_layer", "restartlayers"), ("sync_renderstream_layer", "synclayers")]:
    _dg(_cmd, {"layer": "Unreal scene"}, "POST", _RS + _path, '{"layers":[{"name":"Unreal scene"}]}', **_OK)
_dg("renderstream_failover_machine", {"machine": "rx-03"}, "POST", _RS + "failover",
    '{"machine":{"name":"rx-03"}}', **_OK)
_dg("renderstream_failover_pool", {"layer": "Unreal scene"}, "POST", _RS + "failoverpool",
    '{"layer":{"name":"Unreal scene"}}', **_OK)
telemetry(DG, "renderstream-layers", inbound_http={"path": _RS + "layers", "body": _envelope(result=RS_LAYERS)},
          state_before={"renderstream": {"layers": {"600": {"name": "removed"}}}},
          expect_then_send=[{"method": "GET", "target": _RS + "layerstatus?uid=601"},
                            {"method": "GET", "target": _RS + "layerconfig?uid=601"}],
          expect_state={"renderstream": {"layers": {"601": {"name": "Unreal scene"}}}})
telemetry(DG, "renderstream-layer-status", inbound_http={"path": _RS + "layerstatus?uid=601", "body": _envelope(
    result={"reference": {"tNow": 1234.5},
            "workload": {"uid": "701", "name": "Unreal scene workload", "instances": [
                {"machineUid": "801", "machineName": "rx-01", "state": "Running", "healthMessage": "OK",
                 "healthDetails": ""}]},
            "streams": [{"uid": "901", "name": "Unreal scene/rx-01", "sourceMachine": "rx-01",
                         "receiverMachine": "vx4-01",
                         "status": {"subscriptionWanted": True, "subscribeSuccessful": True, "tLastDropped": 1200.0,
                                    "tLastError": 0.0, "lastErrorMessage": ""},
                         "statusString": "Receiving"}],
            "assetErrors": []})},
    state_before={"renderstream": {"status": {"601": {"instances": {"1": {"machine": "rx-02"}}}}}},
    expect_state={"renderstream": {"status": {"601": {
        "workload": "Unreal scene workload", "workload_uid": "701", "t_now": 1234.5, "asset_errors": "[]",
        "instances": {"0": {"machine": "rx-01", "machine_uid": "801", "state": "Running", "health": "OK",
                            "health_details": ""}},
        "streams": {"0": {"uid": "901", "name": "Unreal scene/rx-01", "source": "rx-01", "receiver": "vx4-01",
                          "subscription_wanted": True, "subscribed": True, "t_last_dropped": 1200.0,
                          "t_last_error": 0.0, "last_error": "", "status": "Receiving"}}}}}})
MAPPINGS = [{"channel": "Default", "mapping": {"uid": "1001", "name": "LED wall"},
             "assigner": {"uid": "1101", "name": "4 nodes"}}]
telemetry(DG, "renderstream-layer-config", inbound_http={"path": _RS + "layerconfig?uid=601", "body": _envelope(
    result={"framerateFractionDivisor": 1, "asset": {"uid": "1201", "name": "Scene.uproject"},
            "pool": {"uid": "1301", "name": "rx pool"}, "channelMappings": MAPPINGS,
            "defaultAssigner": {"uid": "1101", "name": "4 nodes"}})},
    expect_state={"renderstream": {"config": {"601": {
        "asset": "Scene.uproject", "pool": "rx pool", "default_assigner": "4 nodes", "framerate_divisor": 1,
        "channel_mappings": _compact(MAPPINGS)}}}})
POOL_MACHINES = [{"uid": "801", "name": "rx-01", "preferredSyncAdapter": "d3net",
                  "adapters": [{"name": "d3net", "ipAddress": "10.0.0.11", "subnet": "255.255.255.0"}]}]
telemetry(DG, "renderstream-pools", inbound_http={"path": _RS + "pools", "body": _envelope(result=[
    {"uid": "1301", "name": "rx pool", "machines": POOL_MACHINES, "understudies": []}])},
    expect_state={"renderstream": {"pools": {"1301": {"name": "rx pool", "machines": _compact(POOL_MACHINES),
                                                      "understudies": "[]"}}}})
telemetry(DG, "renderstream-assigners", inbound_http={"path": _RS + "assigners", "body": _envelope(result=[
    {"uid": "1101", "name": "4 nodes", "transport": {"type": "NDI", "format": "RGBA", "bitDepth": 8},
     "alpha": True, "overlapPixels": 16, "paddingPixels": 4,
     "preferredNetwork": {"ip": "10.0.1.0", "name": "media"}}])},
    expect_state={"renderstream": {"assigners": {"1101": {
        "name": "4 nodes", "transport": "NDI", "format": "RGBA", "bit_depth": 8, "alpha": True,
        "overlap_pixels": 16, "padding_pixels": 4, "network_ip": "10.0.1.0", "network": "media"}}}})
telemetry(DG, "renderstream-command-reread", inbound_http={"path": _RS + "restartlayers", "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": _RS + "layers"}], expect_state={})
telemetry(DG, "renderstream-failover-reread", inbound_http={"path": _RS + "failover", "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": _RS + "layers"},
                            {"method": "GET", "target": "/api/session/status/health"},
                            {"method": "GET", "target": _RS + "pools"}], expect_state={})

# ── Sockpuppet ──
_SP = "/api/session/sockpuppet/"
_dg("get_sockpuppet_patches", {}, "GET", _SP + "patches")
_dg("get_easing_functions", {}, "GET", _SP + "easingfunctions",
    http_reply={"status": 200, "body": json.dumps({"easingFunctions": ["linear", "easeInOutQuad"]})},
    expect_result={"ok": {"kind": "value", "value": ["linear", "easeInOutQuad"]}})
_dg("set_live_float", {"patch": "/stage/wall", "field": "brightness", "value": 0.5, "duration": 2.0,
                       "easing": "easeInOutQuad"}, "POST", _SP + "live",
    '{"patches":[{"address":"/stage/wall","changes":[{"field":"brightness","floatValue":{"value":0.500000,'
    '"duration":2.000,"easingFunction":"easeInOutQuad"}}]}]}', **_OK)
_dg("set_live_float", {"patch": "/stage/wall", "field": "x", "value": -12.25}, "POST", _SP + "live",
    '{"patches":[{"address":"/stage/wall","changes":[{"field":"x","floatValue":{"value":-12.250000,'
    '"duration":0.000,"easingFunction":""}}]}]}', file="set_live_float_now")
_dg("set_live_string", {"patch": "/stage/wall", "field": "blend", "value": "add"}, "POST", _SP + "live",
    '{"patches":[{"address":"/stage/wall","changes":[{"field":"blend","stringValue":"add"}]}]}', **_OK)
_dg("set_live_resource", {"patch": "/stage/wall", "field": "video", "resource": "intro.mov"}, "POST", _SP + "live",
    '{"patches":[{"address":"/stage/wall","changes":[{"field":"video","resourceValue":{"name":"intro.mov"}}]}]}',
    **_OK)
_dg("send_live_changes", {"patches": [{"address": "/stage/wall", "changes": [{"field": "blend",
                                                                              "stringValue": "add"}]}]},
    "POST", _SP + "live",
    '{"patches":[{"address":"/stage/wall","changes":[{"field":"blend","stringValue":"add"}]}]}', **_OK)
FIELDS = [
    {"name": "brightness", "displayName": "Brightness", "type": "float",
     "floatMeta": {"min": 0.0, "max": 1.0, "defaultValue": 1.0, "step": 0.01},
     "floatValue": {"value": 0.5, "duration": 2.0, "easingFunction": "linear", "startValue": 1.0,
                    "currentValue": 0.75}},
    {"name": "blend", "displayName": "Blend mode", "type": "string",
     "stringMeta": {"options": ["over", "add"]}, "stringValue": "add"},
    {"name": "video", "displayName": "Video", "type": "resource", "resourceMeta": {"type": "VideoClip"},
     "resourceValue": {"uid": "401", "name": "intro.mov"}}]
telemetry(DG, "sockpuppet-patches", inbound_http={"path": _SP + "patches", "body": _envelope(result=[
    {"address": "/stage/wall", "uid": "1501", "description": "LED wall", "fields": FIELDS}])},
    state_before={"sockpuppet": {"patches": {"1500": {"address": "/old"}}}},
    expect_state={"sockpuppet": {"patches": {"1501": {"address": "/stage/wall", "description": "LED wall", "fields": {
        "0": {"name": "brightness", "display_name": "Brightness", "type": "float", "value": 0.5,
              "current_value": 0.75, "min": 0.0, "max": 1.0, "default": 1.0, "step": 0.01},
        "1": {"name": "blend", "display_name": "Blend mode", "type": "string", "text": "add",
              "options": '["over","add"]'},
        "2": {"name": "video", "display_name": "Video", "type": "resource", "resource": "intro.mov",
              "resource_uid": "401", "resource_type": "VideoClip"}}}}}})
telemetry(DG, "easing-functions", inbound_http={"path": _SP + "easingfunctions",
                                                "body": json.dumps({"easingFunctions": ["linear", "easeIn"]})},
          expect_state={"sockpuppet": {"easing_functions": '["linear","easeIn"]'}})
telemetry(DG, "sockpuppet-live-reread", inbound_http={"path": _SP + "live", "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": _SP + "patches"}], expect_state={})

# ── Colour: CDLs ──
_dg("get_cdls", {}, "GET", "/api/session/colour/cdls")
_dg("set_cdl", {"cdl": "wall grade", "slope_r": 1.1, "offset_b": -0.02, "power_g": 0.9, "saturation": 0.8},
    "POST", "/api/session/colour/cdl",
    '{"cdl":{"name":"wall grade","slope":{"x":1.100000,"y":1.000000,"z":1.000000},'
    '"offset":{"x":0.000000,"y":0.000000,"z":-0.020000},"power":{"x":1.000000,"y":0.900000,"z":1.000000},'
    '"saturation":0.800000}}', **_OK)
telemetry(DG, "cdls", inbound_http={"path": "/api/session/colour/cdls", "body": _envelope(result=[
    {"uid": "1601", "name": "wall grade", "slope": {"x": 1.1, "y": 1.0, "z": 1.0},
     "offset": {"x": 0.0, "y": 0.0, "z": -0.02}, "power": {"x": 1.0, "y": 0.9, "z": 1.0}, "saturation": 0.8}])},
    state_before={"cdls": {"1600": {"name": "deleted"}}},
    expect_state={"cdls": {"1601": {"name": "wall grade", "slope_r": 1.1, "slope_g": 1.0, "slope_b": 1.0,
                                    "offset_r": 0.0, "offset_g": 0.0, "offset_b": -0.02, "power_r": 1.0,
                                    "power_g": 0.9, "power_b": 1.0, "saturation": 0.8}}})
telemetry(DG, "cdl-reread", inbound_http={"path": "/api/session/colour/cdl", "body": _ENV_OK},
          expect_then_send=[{"method": "GET", "target": "/api/session/colour/cdls"}], expect_state={})
