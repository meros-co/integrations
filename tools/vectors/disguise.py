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
