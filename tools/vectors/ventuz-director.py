# Ventuz Director (ventuz-director): Director Remoting over its websocket, each
# request {"Command": ..., "RequestID": <int>, parameters...} as the Director
# Remoting chapter writes it, answered with {"RequestID", "Code", "Result",
# "Warnings", "Errors"}; Code 0 is Ok.
VD = "ventuz-director"
ACK = {"ok": {"kind": "ack"}}


def _reply(rid, result="null", code=0):
    return '{"RequestID":%d,"Status":"Succeeded","Code":%d,"Result":%s,"Warnings":[],"Errors":[]}' % (rid, code, result)


def _vd(command, input, rid, cmd, extra="", result="null", expect=None, code=0):
    wire = '{"Command":"%s","RequestID":%d%s}' % (cmd, rid, extra)
    text(VD, command, input, wire, device_reply=_reply(rid, result, code), expect_result=expect or ACK)


def _val(command, input, rid, cmd, extra, result, value):
    _vd(command, input, rid, cmd, extra, result, {"ok": {"kind": "value", "value": value}})


CH = '[{"ChannelIndex":0,"ChannelName":"Main","Code":0,"TakeIndex":1,"TakeCount":1}]'
CHV = [{"ChannelIndex": 0, "ChannelName": "Main", "Code": 0, "TakeIndex": 1, "TakeCount": 1}]


def _whole(rid, result="null", code=0):
    return {"RequestID": rid, "Status": "Succeeded", "Code": code, "Result": json.loads(result), "Warnings": [], "Errors": []}


STATUS = ('{"Version":"7.2.0","LifetimeMs":123456,"User":"op","Machine":"DIR-1","TopologyName":"Studio","TopologyState":"Ok",'
          '"ShowId":"6f1c2a3b-0000-4000-8000-000000000001","ShowName":"News","ShowUri":"C:/Shows/News.vzs",'
          '"ProjectId":"6f1c2a3b-0000-4000-8000-000000000002","ProjectName":"News","ProjectUri":"C:/Projects/News.vzp",'
          '"RemotePlaylistId":"00000000-0000-0000-0000-000000000000","ChannelCount":2}')
_val("get_status", {}, 1, "status.get", "", STATUS, json.loads(STATUS))
_vd("log_write", {"level": "Warning", "message": 'Say "hi"', "popup": True}, 2, "log.write",
    ',"Level":"Warning","Message":"Say \\"hi\\"","PopUp":true')
_vd("window_fullscreen", {}, 10, "window.fullscreen")
_vd("window_reverse", {}, 11, "window.reverse")
_val("window_get_layout", {}, 12, "window.getlayout", "", '{"Index":1,"Name":"Two screens"}', {"Index": 1, "Name": "Two screens"})
_vd("window_set_layout_index", {"index": 1}, 13, "window.setlayout", ',"Index":1')
_vd("window_set_layout_name", {"name": "Two screens"}, 14, "window.setlayout", ',"Name":"Two screens"')
_vd("window_can_set_layout_index", {"index": 2}, 15, "window.can.setlayout", ',"Index":2')
_vd("window_can_set_layout_name", {"name": "Single"}, 16, "window.can.setlayout", ',"Name":"Single"')
_val("topology_get", {}, 20, "topology.get", "", '{"Name":"Studio","SourceType":"LocalFile"}',
     {"Name": "Studio", "SourceType": "LocalFile"})
_vd("topology_set", {"name": "Test Topology"}, 21, "topology.set", ',"Name":"Test Topology","SaveTopology":false')
_vd("topology_set_timeout", {"name": "Test Topology", "save": True, "timeout_ms": 30000}, 22, "topology.set",
    ',"Name":"Test Topology","SaveTopology":true,"TimeOut":30000')
_val("topology_status", {}, 23, "topology.status", "", '{"Name":"Studio","NumberOfPorts":4,"State":"Ok"}',
     {"Name": "Studio", "NumberOfPorts": 4, "State": "Ok"})
_val("show_get", {}, 30, "show.get", "", '{"Id":"a","ProjectId":"b","PublishContextId":"c","Uri":"d","Description":"e"}',
     {"Id": "a", "ProjectId": "b", "PublishContextId": "c", "Uri": "d", "Description": "e"})
_vd("show_open", {"uri": "C:\\Shows\\News.vzs"}, 31, "show.open", ',"Uri":"C:\\\\Shows\\\\News.vzs"')
_vd("show_open_timeout", {"uri": "ventuz://shows/News", "timeout_ms": 20000}, 32, "show.open",
    ',"Uri":"ventuz://shows/News","TimeOut":20000')
_vd("show_close", {"save": True}, 33, "show.close", ',"SaveShow":true,"ClearRenderer":false')
_val("show_cue_uri", {"uri": "ventuz://templates/lower3", "channel": 0}, 40, "show.cue",
     ',"Uri":"ventuz://templates/lower3","ChannelIndex":0,"IgnoreChannelRules":false', CH, CHV)
_val("show_cue_template_data", {"template_data": '{"@":"ventuz://templates/lower3","Name":"Ann"}', "channel": 1, "ignore_rules": True},
     41, "show.cue", ',"TemplateData":"{\\"@\\":\\"ventuz://templates/lower3\\",\\"Name\\":\\"Ann\\"}","ChannelIndex":1,"IgnoreChannelRules":true',
     CH, CHV)
_val("show_cue_template", {"name": "Lower Third", "channel": 0}, 42, "show.cue",
     ',"TemplateDisplayName":"Lower Third","ChannelIndex":0,"IgnoreChannelRules":false', CH, CHV)
_val("show_cue_page", {"name": "Headline 1", "channel": 0}, 43, "show.cue",
     ',"PageDisplayName":"Headline 1","ChannelIndex":0,"IgnoreChannelRules":false', CH, CHV)
_val("show_can_cue_uri", {"uri": "ventuz://templates/lower3", "channel": 0}, 44, "show.can.cue",
     ',"Uri":"ventuz://templates/lower3","ChannelIndex":0,"IgnoreChannelRules":false', CH, _whole(44, CH))
_val("show_can_cue_template_data", {"template_data": "{}", "channel": 0}, 45, "show.can.cue",
     ',"TemplateData":"{}","ChannelIndex":0,"IgnoreChannelRules":false', CH, _whole(45, CH))
_val("show_can_cue_template", {"name": "Lower Third", "channel": 0}, 46, "show.can.cue",
     ',"TemplateDisplayName":"Lower Third","ChannelIndex":0,"IgnoreChannelRules":false', CH, _whole(46, CH))
_val("show_can_cue_page", {"name": "Headline 1", "channel": 0}, 47, "show.can.cue",
     ',"PageDisplayName":"Headline 1","ChannelIndex":0,"IgnoreChannelRules":false', CH, _whole(47, CH))
_val("show_can_cue_channel", {"channel": 1}, 48, "show.can.cuechannel", ',"ChannelIndex":1', CH, _whole(48, CH))
for name, rid, cmd in [("show_take", 50, "show.take"), ("show_take_recue", 52, "show.takerecue"),
                       ("show_recue_on_air", 53, "show.recueonair"), ("show_take_out", 55, "show.takeout"),
                       ("show_take_out_recue", 57, "show.takeoutrecue"), ("show_clear", 58, "show.clear")]:
    _val(name, {"channel": 0}, rid, cmd, ',"ChannelIndex":0', CH, CHV)
for name, rid, cmd in [("show_can_take", 51, "show.can.take"), ("show_can_recue_on_air", 54, "show.can.recueonair"),
                       ("show_can_take_out", 56, "show.can.takeout"), ("show_can_clear", 59, "show.can.clear")]:
    _val(name, {"channel": 0}, rid, cmd, ',"ChannelIndex":0', CH, _whole(rid, CH))
_val("show_get_templates", {}, 60, "show.gettemplates", "", '{"TemplateData":"{}"}', {"TemplateData": "{}"})
_val("show_get_pages", {}, 61, "show.getpages", "", '{"TemplateData":"{}"}', {"TemplateData": "{}"})
_vd("show_create_page", {"template_data": '{"Name":"Ann"}', "name": "Ann", "level": "ventuz://templates/lower3",
                         "keywords": "guest,news", "channel_ids": "0,1"}, 62, "show.createpage",
    ',"TemplateData":"{\\"Name\\":\\"Ann\\"}","Name":"Ann","Level":"ventuz://templates/lower3","Description":"",'
    '"Keywords":"guest,news","Category":"","ChannelIds":"0,1"')
_val("show_get_channels", {}, 63, "show.getchannels", "", '[{"ChannelIndex":0,"Name":"Main"}]', [{"ChannelIndex": 0, "Name": "Main"}])
_val("show_get_project_data", {}, 64, "show.getprojectdata", "", "{}", {})
_vd("show_trigger_project_event", {"path": "Scores.Reset"}, 65, "show.setprojectdata", ',"DataPath":"Scores.Reset"')
_vd("show_set_project_bool", {"path": "Clock.Visible", "value": True}, 66, "show.setprojectdata",
    ',"DataPath":"Clock.Visible","ValueToSet":"true"')
_vd("show_toggle_project_bool", {"path": "Clock.Visible"}, 67, "show.setprojectdata",
    ',"DataPath":"Clock.Visible","SpecialAction":"Toggle"')
_vd("show_set_project_value", {"path": "Lower.Color", "value": "255,0,128"}, 68, "show.setprojectdata",
    ',"DataPath":"Lower.Color","ValueToSet":"255,0,128"')
_vd("show_set_project_number_relative", {"path": "Scores.Home", "delta": 1.0}, 69, "show.setprojectdata",
    ',"DataPath":"Scores.Home","ValueToSet":"1.000000","SetRelative":true')
_vd("show_preload_templates", {}, 70, "show.preloadtemplates")
_vd("show_preload_templates_playlist", {}, 71, "show.preloadtemplates_playlist")
_vd("show_preload_templates_timeline", {}, 72, "show.preloadtemplates_timeline")
_vd("show_reload_templates", {"timeout_ms": 60000}, 73, "show.reloadtemplates", ',"TimeOut":60000')
_val("show_get_assets", {}, 74, "show.getassets", ',"IncludeThumbnail":false',
     '[{"Uri":"ventuz://images/logo.png","DisplayName":"logo","AssetType":"Image","AssetPool":"Images"}]',
     [{"Uri": "ventuz://images/logo.png", "DisplayName": "logo", "AssetType": "Image", "AssetPool": "Images"}])
PL = ('[{"ChannelIndex":0,"ChannelName":"Main","Items":[{"Name":"Open","DisplayName":"Opener","ChannelIndex":0,"Index":0,'
      '"Id":"i1","Status":"On-Air"},{"Name":"L3","DisplayName":"Lower third","ChannelIndex":0,"Index":1,"Id":"i2","Status":"Cued"}]}]')
_val("playlist_get", {}, 80, "playlist.get", "", PL, json.loads(PL))
META = '{"Id":"i2","Name":"L3","DisplayName":"Lower third","ChannelIndex":0,"Index":1,"Status":"Cued"}'
_val("playlist_get_meta_index", {"channel": 0, "index": 1}, 81, "playlist.getmeta", ',"ChannelIndex":0,"Index":1', META, json.loads(META))
_val("playlist_get_meta_name", {"channel": 0, "name": "L3"}, 82, "playlist.getmeta", ',"ChannelIndex":0,"Name":"L3"', META, json.loads(META))
_val("playlist_activate_index", {"channel": 0, "index": 1}, 83, "playlist.activate", ',"ChannelIndex":0,"Index":1', CH, CHV)
_val("playlist_activate_name", {"channel": 0, "name": "L3"}, 84, "playlist.activate", ',"ChannelIndex":0,"Name":"L3"', CH, CHV)
_val("playlist_can_activate_index", {"channel": 0, "index": 1}, 85, "playlist.can.activate", ',"ChannelIndex":0,"Index":1',
     CH, _whole(85, CH))
_val("playlist_can_activate_name", {"channel": 0, "name": "L3"}, 86, "playlist.can.activate", ',"ChannelIndex":0,"Name":"L3"',
     CH, _whole(86, CH))
_val("playlist_restart", {"channel": 0}, 87, "playlist.restart", ',"ChannelIndex":0', CH, CHV)
_val("playlist_can_restart", {"channel": 0}, 88, "playlist.can.restart", ',"ChannelIndex":0', CH, _whole(88, CH))
RP = '{"Id":"8a0e5c1e-1111-4222-8333-944455556666","Name":"Evening","PublishContexts":[{"Name":"Studio A","Id":"x"}]}'
_val("remote_playlist_get", {}, 90, "remoteplaylist.get", "", RP, json.loads(RP))
_vd("remote_playlist_open", {"playlist_id": "8a0e5c1e-1111-4222-8333-944455556666", "publish_context": "Studio A"}, 91,
    "remoteplaylist.open", ',"PlaylistId":"8a0e5c1e-1111-4222-8333-944455556666","PublishContextName":"Studio A","SpecialAction":"UseCurrentShow"')
_vd("remote_playlist_open_show", {"playlist_id": "8a0e5c1e-1111-4222-8333-944455556666", "show_uri": "C:/Shows/News.vzs"}, 92,
    "remoteplaylist.open", ',"PlaylistId":"8a0e5c1e-1111-4222-8333-944455556666","PublishContextName":"","SpecialAction":"UseShowUri","ShowUri":"C:/Shows/News.vzs"')
_val("macro_get", {}, 100, "macro.get", "", '[{"Id":"m1","Name":"Reset"}]', [{"Id": "m1", "Name": "Reset"}])
_vd("macro_execute_id", {"id": "m1"}, 101, "macro.execute", ',"Id":"m1"')
_vd("macro_execute_name", {"name": "Reset"}, 102, "macro.execute", ',"Name":"Reset"')
_vd("macro_can_execute_id", {"id": "m1"}, 103, "macro.can.execute", ',"Id":"m1"')
_vd("macro_can_execute_name", {"name": "Reset"}, 104, "macro.can.execute", ',"Name":"Reset"')
for name, rid, cmd in [("timeline_play", 110, "timeline.play"), ("timeline_pause", 111, "timeline.pause"),
                       ("timeline_rewind", 112, "timeline.rewind"), ("timeline_forward", 113, "timeline.forward"),
                       ("timeline_jump_start", 114, "timeline.jumpstart"), ("timeline_jump_end", 115, "timeline.jumpend")]:
    _vd(name, {}, rid, cmd)
# A refusal: no show open (code 4) fails the command.
V.append({"spec": VD, "command": "timeline_play", "file": "timeline_play.no-show", "input": {},
          "expect_wire": '{"Command":"timeline.play","RequestID":110}',
          "device_reply": _reply(110, "null", 4).replace('"Status":"Succeeded"', '"Status":"Failed"'),
          "expect_result": {"error": {"error": "device_error", "code": "4"}}})

# Telemetry: the polled replies.
telemetry(VD, "status", inbound_ws=_reply(1000, STATUS), expect_state={
    "status": {"version": "7.2.0", "lifetime_ms": 123456, "user": "op", "machine": "DIR-1", "channel_count": 2},
    "topology": {"name": "Studio", "state": "Ok"},
    "show": {"id": "6f1c2a3b-0000-4000-8000-000000000001", "name": "News", "uri": "C:/Shows/News.vzs"},
    "project": {"id": "6f1c2a3b-0000-4000-8000-000000000002", "name": "News", "uri": "C:/Projects/News.vzp"},
    "remote_playlist": {"id": "00000000-0000-0000-0000-000000000000"}})
telemetry(VD, "topology", inbound_ws=_reply(1002, '{"Name":"Studio","SourceType":"EmbeddedInProject"}'),
          expect_state={"topology": {"name": "Studio", "source_type": "EmbeddedInProject"}})
telemetry(VD, "topology-status", inbound_ws=_reply(1003, '{"Name":"Studio","NumberOfPorts":4,"State":"Warning"}'),
          expect_state={"topology": {"name": "Studio", "number_of_ports": 4, "state": "Warning"}})
telemetry(VD, "show", inbound_ws=_reply(1004, '{"Id":"s1","ProjectId":"p1","PublishContextId":"c1","Uri":"C:/Shows/News.vzs","Description":"Evening news"}'),
          expect_state={"show": {"id": "s1", "publish_context_id": "c1", "uri": "C:/Shows/News.vzs", "description": "Evening news"},
                        "project": {"id": "p1"}})
telemetry(VD, "channels", inbound_ws=_reply(1005, '[{"ChannelIndex":0,"Name":"Main"},{"ChannelIndex":1,"Name":"Wall"}]'),
          expect_state={"channels": {"0": {"name": "Main"}, "1": {"name": "Wall"}}})
telemetry(VD, "window-layout", inbound_ws=_reply(1006, '{"Index":1,"Name":"Two screens"}'),
          expect_state={"window": {"layout_index": 1, "layout_name": "Two screens"}})
telemetry(VD, "macros", inbound_ws=_reply(1007, '[{"Id":"m.1","Name":"Reset"}]'),
          expect_state={"macros": {"m.1": {"name": "Reset"}}})
telemetry(VD, "remote-playlist", inbound_ws=_reply(1008, RP),
          expect_state={"remote_playlist": {"id": "8a0e5c1e-1111-4222-8333-944455556666", "name": "Evening"}})
telemetry(VD, "playlists", inbound_ws=_reply(1001, PL), expect_state={"playlists": {"0": {
    "channel_name": "Main", "items": {
        "0": {"name": "Open", "display_name": "Opener", "id": "i1", "status": "On-Air"},
        "1": {"name": "L3", "display_name": "Lower third", "id": "i2", "status": "Cued"}}}}})
telemetry(VD, "failed-status", inbound_ws=_reply(1000, "null", 4), expect_state={})
