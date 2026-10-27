# Modulo Player (modulopi-player): JSON-RPC 2.0 over the websocket on 8080,
# as the manual's websocket pages write the requests
# ({"jsonrpc":"2.0","method":"doaction.playlist","params":{"name":"Main",
# "action":"goto","cue":1},"id":1}), each with an id naming what it does.
MP = "modulopi-player"
UU = "{45c8566e-435d-4abb-ad3a-2800cb1ce1c5}"
U2 = "{b9ff8f5b-5a15-4422-9e25-b8eeed9f8f9a}"


def _mreq(method, params, id_):
    p = "" if params is None else ',"params":' + params
    return '{"jsonrpc":"2.0","method":"%s"%s,"id":%s}' % (method, p, json.dumps(id_))


def _mp(command, input, method, params, id_, result="true", kind="ack", file=None):
    reply = '{"jsonrpc":"2.0","result":%s,"id":%s}' % (result, json.dumps(id_))
    expect = {"ok": {"kind": "ack"}} if kind == "ack" else {"ok": {"kind": "value", "value": json.loads(result)}}
    v = {"spec": MP, "command": command, "input": input, "expect_wire": _mreq(method, params, id_), "device_reply": reply,
         "expect_result": expect}
    if file:
        v["file"] = file
    V.append(v)


def _val(command, input, method, params, id_, result='{"ok":true}'):
    _mp(command, input, method, params, id_, result, "value")


OK = '{"ok":true}'
_val("call", {"method": "set.playlist.fader", "params": {"playlistname": "Main", "grandmaster": 0, "time": 3000}},
     "set.playlist.fader", '{"playlistname":"Main","grandmaster":0,"time":3000}', "call", '{"grandMaster":0,"timeMs":3000}')
_val("get_methods", {"prefix": "mixer."}, "get.list.methods", '{"prefix":"mixer."}', "methods", '[{"method":"mixer.take"}]')
_val("get_object", {"uuid": UU}, "get.object", '{"uuid":"%s","depth":0}' % UU, "object " + UU, '{"uuid":"%s","className":"Cue"}' % UU)
_val("get_tree", {}, "get.tree", '{"depth":1}', "tree", '{"root":{"name":"MyShow"}}')
_val("set_properties", {"uuid": UU, "properties": {"name": "Intro"}}, "set.property",
     '{"uuid":"%s","properties":{"name":"Intro"}}' % UU, "set " + UU, '{"uuid":"%s","changed":["name"]}' % UU)
STATUS = ('{"version":"6.3.110","apiVersion":2,"showName":"MyShow.xml","documentState":2,"frameCounter":100,"displayFrequency":60,'
          '"maxOutputs":8,"playlistCount":2,"master":true,"mediaToCache":0,"mediaCaching":0,"backgroundSave":false,"clientConnected":1}')
_val("get_player_status", {}, "get.status.player", None, "status", STATUS)
_mp("show_action", {"action": "save"}, "doaction.show", '{"action":"save"}', "show save")
_mp("load_show", {"name": "MyShow.xml"}, "doaction.show", '{"action":"load","name":"MyShow.xml"}', "load show")
_val("list_shows", {}, "get.list.shows", '{"filter":""}', "shows", '{"count":1,"shows":[{"name":"MyShow.xml"}]}')
_val("new_show", {"name": "New.xml"}, "new.show", '{"name":"New.xml","overwrite":false}', "new show", '{"name":"New.xml","ready":true}')
_val("save_show_as", {"name": "Copy.xml", "overwrite": True}, "saveas.show", '{"name":"Copy.xml","overwrite":true}', "save as",
     '{"name":"Copy.xml","replaced":true}')
_val("save_show_increment", {}, "saveincrement.show", '{"overwrite":false}', "save increment", '{"name":"MyShow_2.xml"}')
_val("backup_show", {}, "backup.show", None, "backup", '{"backup":"MyShow_2026-10-27.zip","show":"MyShow.xml"}')
_val("list_backups", {"filter": "MyShow"}, "get.list.backups", '{"filter":"MyShow"}', "backups", '{"count":0,"backups":[]}')
_val("restore_show", {"backup": "MyShow_2026-10-27.zip"}, "restore.show", '{"backup":"MyShow_2026-10-27.zip","force":false}', "restore",
     '{"ready":true}')
_val("list_import", {"name": "Other.xml"}, "get.list.import", '{"name":"Other.xml"}', "import list", '{"count":0}')
_val("import_show_all", {"name": "Other.xml"}, "import.show",
     '{"name":"Other.xml","outputs":"all","playlists":"all","devices":"all","tasks":"all","digimaps":"all","uipanels":"all"}',
     "import", '{"count":3}')
_val("get_computer", {}, "get.object.computer", None, "computer", '{"settings":{"audioMaster":1}}')
_val("set_computer_properties", {"properties": {"audioMaster": 0.8}}, "set.property.computer", '{"properties":{"audioMaster":0.8}}',
     "set computer", '{"changed":["audioMaster"]}')
PL = [{"uuid": U2, "name": "Main", "index": 2, "play": True, "playState": True, "currentCueUuid": UU, "currentcuename": "Intro",
       "preloadCueUuid": "", "timecode": 12000, "grandMasterFader": 1.0, "audioMaster": 0.5, "cueCount": 10,
       "stepByStepEnable": False, "synchroToTimecode": False, "timecodeLocked": False, "uiColor": "#ff0000"}]
_val("list_playlists", {"level": "cue"}, "get.list.playlists", '{"level":"cue"}', "playlists cue", json.dumps(PL))
_val("get_playlist", {"playlist": "Main"}, "get.object.playlist", '{"name":"Main"}', "playlist", json.dumps(PL[0]))
_mp("playlist_next", {"playlist": "Main"}, "doaction.playlist", '{"name":"Main","action":"next"}', "next")
_mp("playlist_previous", {"playlist": "Main"}, "doaction.playlist", '{"name":"Main","action":"prev"}', "prev")
_mp("playlist_goto", {"playlist": "Main", "cue": 1}, "doaction.playlist", '{"name":"Main","action":"goto","cue":1}', "goto")
_mp("playlist_preload", {"playlist": "Main", "cue": 5}, "doaction.playlist", '{"name":"Main","action":"preload","cue":5}', "preload")
_mp("playlist_play", {"playlist": "Main"}, "doaction.playlist", '{"name":"Main","action":"play"}', "play")
_mp("playlist_pause", {"playlist": "Main"}, "doaction.playlist", '{"name":"Main","action":"pause"}', "pause")
_mp("playlist_grand_master", {"playlist": "Main", "percent": 0, "duration_ms": 3000}, "doaction.playlist",
    '{"name":"Main","action":"grandMasterFader","value":0,"duration":3000}', "grand master")
_mp("playlist_audio_master", {"playlist": "Main", "percent": 50}, "doaction.playlist",
    '{"name":"Main","action":"audioMaster","value":50,"duration":0}', "audio master")
_val("get_playlist_settings", {"playlist": "Main"}, "get.playlist.settings", '{"playlistname":"Main"}', "playlist settings",
     '{"autoStart":false}')
_val("set_playlist_settings", {"playlist": "Main", "settings": {"autoStart": True}}, "set.playlist.settings",
     '{"playlistname":"Main","settings":{"autoStart":true}}', "set playlist settings", '{"changed":["autoStart"]}')
_mp("rename_playlist", {"playlist": "Main", "name": "Stage"}, "set.property.playlist",
    '{"name":"Main","property":"name","value":"Stage"}', "rename playlist")
_val("new_playlist", {"name": "Lobby"}, "new.playlist", '{"name":"Lobby"}', "new playlist", '{"uuid":"%s"}' % U2)
_mp("delete_playlist", {"playlist": "Lobby"}, "delete.playlist", '{"name":"Lobby"}', "delete playlist")
_val("list_cues", {"playlist": "Main"}, "get.list.cues", '{"playlistname":"Main","offset":0,"limit":200}', "cues",
     '{"total":1,"cues":[{"name":"Intro"}]}')
_val("get_cue_property", {"playlist": "Main", "cue": "Intro", "property": "triggerMode"}, "get.property.cue",
     '{"playlistname":"Main","name":"Intro","property":"triggerMode"}', "cue property", "0")
_val("set_cue_settings", {"playlist": "Main", "cue": 2, "settings": {"trigger": "wait", "triggerTime": 1500}}, "set.cue.settings",
     '{"playlistname":"Main","cueindex":2,"settings":{"trigger":"wait","triggerTime":1500}}', "cue settings", '{"changed":["trigger"]}')
_val("set_cue_trigger", {"playlist": "Main", "cue": 3, "trigger": "follow"}, "set.cue.settings",
     '{"playlistname":"Main","cueindex":3,"settings":{"trigger":"follow","triggerTime":0}}', "cue trigger", '{"changed":["trigger"]}')
_val("rename_cue", {"playlist": "Main", "cue": 1, "name": "Opening"}, "set.cue.settings",
     '{"playlistname":"Main","cueindex":1,"property":"name","value":"Opening"}', "rename cue", '{"changed":["name"]}')
_val("add_cues", {"playlist": "Main", "after": 0, "count": 2}, "add.cues", '{"playlistname":"Main","aftercueindex":0,"count":2}',
     "add cues", '{"cues":[]}')
_val("delete_cue", {"playlist": "Main", "cue": 4}, "delete.cues", '{"playlistname":"Main","cueindex":4}', "delete cue", '{"deleted":[]}')
_val("duplicate_cue", {"cue": UU}, "duplicate.cue", '{"cueuuid":"%s","count":1}' % UU, "duplicate cue", '{"copies":[]}')
_val("move_cue", {"playlist": "Main", "cue": 4, "to": 1}, "set.cue.index", '{"playlistname":"Main","cueindex":4,"newindex":1}',
     "move cue", '{"index":1}')
LSEL = '"playlistname":"Main","cueindex":1,"layerindex":2'
_val("get_layer", {"playlist": "Main", "cue": 1, "layer": 2}, "get.object.layer", "{%s}" % LSEL, "layer", '{"index":2}')
_val("set_layer_media", {"playlist": "Main", "cue": 1, "layer": 2, "media": "hello.mov"}, "set.layer.media",
     '{%s,"media":"hello.mov","slot":"media"}' % LSEL, "layer media", '{"index":2}')
_val("clear_layer_media", {"playlist": "Main", "cue": 1, "layer": 2, "slot": "mask"}, "clear.layer.media",
     '{%s,"slot":"mask"}' % LSEL, "clear layer", '{"index":2}')
_val("set_layer_settings", {"playlist": "Main", "cue": 1, "layer": 2, "settings": {"opacity": 0.5}}, "set.layer.settings",
     '{%s,"settings":{"opacity":0.5}}' % LSEL, "layer settings", '{"changed":["opacity"]}')
_val("list_fx_types", {}, "get.list.fx.types", None, "fx types", '{"types":["HueRotate"]}')
_val("add_fx", {"playlist": "Main", "cue": 1, "layer": 2, "type": "HueRotate"}, "add.fx", '{%s,"type":"HueRotate"}' % LSEL,
     "add fx", '{"uuid":"%s"}' % UU)
_val("set_fx_settings", {"fx": UU, "settings": {"bypass": True}}, "set.fx.settings", '{"fxuuid":"%s","settings":{"bypass":true}}' % UU,
     "fx settings", '{"bypass":true}')
_val("delete_fx", {"fx": UU}, "delete.fx", '{"fxuuid":"%s"}' % UU, "delete fx", '{"deleted":"%s"}' % UU)
TASKS = [{"index": 1, "keyword": "go", "name": "Open", "objectType": "task", "taskEnable": True, "triggerEnable": False,
          "triggered": False, "uuid": UU}]
_val("list_tasks", {}, "get.list.tasks", None, "tasks", json.dumps(TASKS))
_val("run_task", {"task": "Open"}, "run.task", '{"name":"Open"}', "run task", '{"launched":true}')
_val("set_task_enabled", {"task": "Open", "enabled": False}, "set.task", '{"name":"Open","taskEnable":false}', "set task",
     '{"changed":["taskEnable"]}')
_val("create_task", {"name": "Close"}, "create.task", '{"name":"Close"}', "create task", '{"uuid":"%s"}' % U2)
_val("duplicate_task", {"task": "Open", "name": "Open 2"}, "duplicate.task", '{"name":"Open","newName":"Open 2"}', "duplicate task",
     '{"uuid":"%s"}' % U2)
_val("remove_task", {"task": "Close"}, "remove.task", '{"name":"Close"}', "remove task", '{"deleted":"%s"}' % U2)
MEDIA = [{"uuid": UU, "name": "hello.mov", "type": "movie", "mediaState": 2, "duration": 12.5, "width": 1920, "height": 1080}]
_val("list_media", {}, "get.list.media", None, "media", json.dumps(MEDIA))
_val("get_media", {"uuid": UU}, "get.object.media", '{"uuid":"%s"}' % UU, "media " + UU, json.dumps(MEDIA[0]))
_val("get_media_tree", {"search": "hello"}, "get.tree.media", '{"search":"hello","max":500}', "media tree", '{"total":1}')
_val("add_media", {"type": "solid", "name": "Red", "settings": {"color": "#ff0000"}}, "add.media",
     '{"type":"solid","name":"Red","settings":{"color":"#ff0000"}}', "add media", '{"uuid":"%s"}' % U2)
_val("set_media_settings", {"uuid": U2, "settings": {"text": "Welcome"}}, "set.media.settings",
     '{"uuid":"%s","settings":{"text":"Welcome"}}' % U2, "media settings", '{"changed":["text"]}')
_val("delete_media", {"uuid": U2}, "delete.media", '{"uuid":"%s"}' % U2, "delete media", "true")
_val("delete_missing_media", {}, "delete.media.missing", None, "delete missing", '{"count":0}')
_val("rescan_media", {}, "rescan.media", "{}", "rescan", '{"count":0}')
OUTS = {"outputs": [{"index": 1, "uuid": UU, "name": "output 1", "type": "standard"}], "desktop": {"width": 3840, "height": 1080}}
_val("list_outputs", {}, "get.list.outputs", None, "outputs", json.dumps(OUTS))
_val("get_output", {"output": 1}, "get.object.output", '{"index":1}', "output", json.dumps(OUTS["outputs"][0]))
_val("set_output_properties", {"output": 1, "properties": {"ndiStreamEnable": 1}}, "set.property.output",
     '{"index":1,"properties":{"ndiStreamEnable":1}}', "set output", '{"changed":["ndiStreamEnable"]}')
_val("add_output", {"settings": {}}, "add.output", '{"data":{}}', "add output", '{"created":true}')
_val("delete_output", {"output": 2}, "delete.output", '{"index":2}', "delete output", '{"outputCount":1}')
_val("list_monitoring", {}, "get.list.monitoring", None, "monitoring", '{"presets":[]}')
_val("activate_monitoring", {"preset": 1}, "set.property.monitoring", '{"index":1,"property":"active","value":true}',
     "monitoring active", '{"changed":["active"]}')
_val("set_monitoring_zone", {"preset": 1, "zone": 2, "source": "output", "source_name": "output 1"}, "set.monitoring.zone",
     '{"index":1,"zone":2,"source":"output","sourcename":"output 1"}', "monitoring zone", '{"zone":2}')
_val("list_digimaps", {}, "get.list.digimaps", None, "digimaps", "[]")
_val("digimap_action", {"digimap": "Tracker", "action": "captureina"}, "doaction.digimap", '{"name":"Tracker","action":"captureina"}',
     "digimap captureina", '{"action":"captureina","inA":12}')
MIX = ('{"connected":true,"ready":true,"workOnPreview":true,"relativeMode":false,"taskToLaunch":"","takeDuration":3000,'
       '"currentLayout":1,"transition":{"active":false,"position":0},"lastPreset":"",'
       '"destinations":[{"uuid":"%s","active":true,"programLocked":false,"takeInProgress":false,"freeze":false}],'
       '"tally":[{"uuid":"%s","program":true,"preview":false,"freeze":false}],"renders":[]}' % (U2, UU))
_val("get_mixer_state", {}, "get.state", None, "mixer state", MIX)
_mp("mixer_set_options", {"take_ms": 2000}, "mixer.set.option", '{"workOnPreview":true,"relativeMode":false,"takeDuration":2000}',
    "mixer options", OK)
_mp("mixer_take", {"duration_ms": 2000}, "mixer.take", '{"duration":2000}', "take", OK)
_mp("mixer_cut", {}, "mixer.cut", "{}", "cut", OK)
_mp("mixer_transition_begin", {}, "mixer.transition.begin", None, "transition begin", OK)
_mp("mixer_transition_position", {"position": 0.5}, "mixer.transition.position", '{"position":0.500}', "transition position", OK)
_mp("mixer_transition_commit", {}, "mixer.transition.commit", None, "transition commit", OK)
_mp("mixer_transition_cancel", {}, "mixer.transition.cancel", None, "transition cancel", OK)
_mp("mixer_transition_type", {"type": "fade"}, "mixer.transition.type", '{"type":"fade"}', "transition type", '{"layers":2,"ok":true}')
_mp("mixer_pgm_to_pvw", {}, "mixer.pgmtopvw", None, "pgmtopvw", OK)
_mp("mixer_clear", {}, "mixer.clear", "{}", "clear", '{"layers":1,"ok":true}')
_mp("mixer_source_set", {"source": "Camera 1"}, "mixer.source.set", '{"source":"Camera 1","target":"preview","mixEngine":1}',
    "source set", '{"ok":true,"destinations":1}')
_mp("mixer_source_freeze", {"source": "Camera 1", "freeze": True}, "mixer.source.freeze", '{"name":"Camera 1","freeze":true}', "freeze",
    '{"ok":true,"freeze":true}')
SRC = [{"index": 1, "uuid": UU, "name": "Camera 1", "onProgram": True, "onPreview": False, "freeze": False}]
_val("list_mixer_sources", {}, "get.list.sources", None, "sources", json.dumps(SRC))
_val("list_mixer_destinations", {}, "get.list.destinations", None, "destinations", "[]")
_val("list_mixer_renders", {}, "get.list.renders", None, "renders", "[]")
_mp("mixer_destination_active", {"destination": "LED wall", "active": True}, "mixer.destination.active",
    '{"name":"LED wall","active":true}', "destination active", '{"ok":true,"active":true}')
_mp("mixer_destination_all", {"active": False}, "mixer.destination.all", '{"active":false}', "destinations all", OK)
_mp("mixer_set_property", {"kind": "layer", "uuid": UU, "property": "opacity", "value": 0.5}, "mixer.set.property",
    '{"kind":"layer","uuid":"%s","property":"opacity","value":0.5}' % UU, "mixer property", OK)
PRESETS = [{"index": 1, "uuid": U2, "name": "Wide", "locked": False}]
_val("list_mixer_presets", {}, "get.list.presets", None, "presets", json.dumps(PRESETS))
_mp("mixer_preset_recall", {"preset": "Wide", "target": "program"}, "mixer.preset.recall", '{"name":"Wide","target":"program"}',
    "preset recall", OK)
_mp("mixer_preset_save", {"preset": "Wide", "from": "program"}, "mixer.preset.save", '{"name":"Wide","from":"program"}', "preset save", OK)
_val("mixer_preset_add", {"from": "preview"}, "mixer.preset.add", '{"from":"preview"}', "preset add", '{"ok":true,"uuid":"%s"}' % U2)
_mp("mixer_preset_delete", {"preset": "Wide"}, "mixer.preset.delete", '{"name":"Wide"}', "preset delete", OK)
_mp("mixer_show_save", {}, "mixer.show.save", None, "mixer save", OK)
# A refused action: no playlist of that name.
V.append({"spec": MP, "command": "playlist_next", "file": "playlist_next.error", "input": {"playlist": "Intro"},
          "expect_wire": _mreq("doaction.playlist", '{"name":"Intro","action":"next"}', "next"),
          "device_reply": '{"jsonrpc":"2.0","error":{"code":-32602,"message":"no playlist is named Intro"},"id":"next"}',
          "expect_result": {"error": {"error": "device_error"}}})


def _mt(name, msg, state, **kw):
    telemetry(MP, name, inbound_ws=msg, expect_state=state, **kw)


_mt("status", '{"jsonrpc":"2.0","result":%s,"id":"poll status"}' % STATUS, {"player": {
    "version": "6.3.110", "api_version": 2, "show": "MyShow.xml", "document_state": 2, "display_frequency": 60.0, "max_outputs": 8,
    "playlist_count": 2, "master": True, "media_to_cache": 0, "media_caching": 0, "clients": 1}})
_mt("playlists", '{"jsonrpc":"2.0","result":%s,"id":"poll playlists"}' % json.dumps(PL), {"playlists": {U2: {
    "name": "Main", "index": 2, "playing": True, "current_cue": UU, "current_cue_name": "Intro", "preload_cue": "", "timecode": 12000,
    "grand_master": 1.0, "audio_master": 0.5, "cue_count": 10, "step_by_step": False, "synchro_to_timecode": False,
    "timecode_locked": False, "color": "#ff0000"}}})
_mt("tasks", '{"jsonrpc":"2.0","result":%s,"id":"poll tasks"}' % json.dumps(TASKS), {"tasks": {UU: {
    "name": "Open", "index": 1, "keyword": "go", "enabled": True, "trigger_enabled": False, "triggered": False}}})
_mt("media", '{"jsonrpc":"2.0","result":%s,"id":"poll media"}' % json.dumps(MEDIA), {"media": {UU: {
    "name": "hello.mov", "type": "movie", "state": 2, "duration": 12.5, "width": 1920, "height": 1080}}})
_mt("outputs", '{"jsonrpc":"2.0","result":%s,"id":"poll outputs"}' % json.dumps(OUTS), {"outputs": {UU: {
    "name": "output 1", "index": 1, "type": "standard"}}})
MIXSTATE = {"mixer": {"ready": True, "work_on_preview": True, "relative_mode": False, "take_duration": 3000, "current_layout": 1,
                      "transition_active": False, "transition_position": 0.0, "last_preset": "",
                      "destinations": {U2: {"active": True, "program_locked": False, "take_in_progress": False, "freeze": False}},
                      "tally": {UU: {"program": True, "preview": False, "freeze": False}}}}
_mt("mixer-state-reply", '{"jsonrpc":"2.0","result":%s,"id":"poll mixer"}' % MIX, MIXSTATE)
_mt("mixer-state-push", '{"jsonrpc":"2.0","method":"mixer.state","params":%s}' % MIX, MIXSTATE)
_mt("presets", '{"jsonrpc":"2.0","result":%s,"id":"poll presets"}' % json.dumps(PRESETS), {"mixer": {"presets": {U2: {
    "name": "Wide", "index": 1, "locked": False}}}})
_mt("sources", '{"jsonrpc":"2.0","result":%s,"id":"poll sources"}' % json.dumps(SRC), {"mixer": {"sources": {UU: {
    "name": "Camera 1", "index": 1}}}})
_mt("show-loaded", '{"jsonrpc":"2.0","method":"event.show.loaded","params":{"name":"MyShow.xml"}}', {"player": {"show": "MyShow.xml"}},
    expect_then_send=[
        '{"jsonrpc":"2.0","method":"get.list.playlists","params":{"level":"playlist"},"id":"poll playlists"}',
        '{"jsonrpc":"2.0","method":"get.list.tasks","id":"poll tasks"}',
        '{"jsonrpc":"2.0","method":"get.list.media","id":"poll media"}',
        '{"jsonrpc":"2.0","method":"get.list.outputs","id":"poll outputs"}',
        '{"jsonrpc":"2.0","method":"get.state","id":"poll mixer"}'])
