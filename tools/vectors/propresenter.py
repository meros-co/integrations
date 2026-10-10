# ProPresenter (propresenter): telemetry vectors for the state its status and
# list endpoints keep current, and the re-reads after commands. Reply bodies
# follow the response schemas and examples of Renewed Vision's OpenAPI
# document (openapi.propresenter.com); the commands' own vectors are written
# in make_vectors.py from the spec.
PPT = "propresenter"


def _ppt(name, path, body, expect_state, **extra):
    inbound = {"path": path, "body": body if isinstance(body, str) else json.dumps(body)}
    if "request" in extra:
        inbound["request"] = extra.pop("request")
    telemetry(PPT, name, inbound_http=inbound, expect_state=expect_state, **extra)


def _id(uuid, name, index):
    return {"uuid": uuid, "name": name, "index": index}


_LIVE = [{"method": "GET", "target": t} for t in (
    "/v1/status/slide", "/v1/presentation/slide_index", "/v1/status/layers",
    "/v1/look/current", "/v1/playlist/active", "/v1/presentation/focused")]

_ppt("version", "/version", {"name": "ProPresenter", "platform": "mac", "os_version": "14.4.1",
                             "host_description": "Main", "api_version": "v1"},
     {"device": {"name": "ProPresenter", "platform": "mac", "os_version": "14.4.1",
                 "host": "Main", "api_version": "v1"}})

# Presentations, announcements, timelines.
_ppt("announcement-slide-index", "/v1/announcement/slide_index",
     {"announcement_index": {"index": 3, "presentation_id": _id("an1", "Welcome Loop", 0)}},
     {"announcement": {"slide_index": 3, "name": "Welcome Loop", "uuid": "an1"}})
_ppt("focused-presentation", "/v1/presentation/focused", _id("p9", "Amazing Grace", 4),
     {"focused_presentation": {"uuid": "p9", "name": "Amazing Grace", "index": 4}})
_ppt("timeline-presentation", "/v1/presentation/active/timeline",
     {"is_running": True, "current_time": 12.5},
     {"timeline": {"presentation": {"running": True, "time": 12.5}}})
_ppt("timeline-announcement", "/v1/announcement/active/timeline",
     {"is_running": False, "current_time": 0},
     {"timeline": {"announcement": {"running": False, "time": 0.0}}})

# The live look; its screens kept as JSON text.
_ppt("look-current", "/v1/look/current",
     {"id": _id("l1", "Worship", 1), "screens": [{"slide": True}]},
     {"look": {"uuid": "l1", "name": "Worship", "index": 1, "screens": '[{"slide":true}]'}})

# Playlists: active per destination, focused, media and audio.
_ppt("playlist-active", "/v1/playlist/active",
     {"presentation": {"playlist": _id("pl1", "Sunday", 0), "item": _id("it3", "Amazing Grace", 2)},
      "announcements": {"playlist": _id("pl2", "Loop", 1), "item": _id("it7", "Welcome", 0)}},
     {"playlist": {"active": {
         "presentation": {"uuid": "pl1", "name": "Sunday", "item_uuid": "it3",
                          "item_name": "Amazing Grace", "item_index": 2},
         "announcements": {"uuid": "pl2", "name": "Loop", "item_uuid": "it7",
                           "item_name": "Welcome", "item_index": 0}}}})
_ppt("playlist-focused", "/v1/playlist/focused",
     {"playlist": _id("pl1", "Sunday", 0), "item": _id("it4", "Offering", 3)},
     {"playlist": {"focused": {"uuid": "pl1", "name": "Sunday", "item_uuid": "it4",
                               "item_name": "Offering", "item_index": 3}}})
_ppt("media-playlist-active", "/v1/media/playlist/active",
     {"playlist": _id("m1", "Backgrounds", 0), "item": _id("mi2", "Clouds", 1)},
     {"media_playlist": {"active": {"uuid": "m1", "name": "Backgrounds", "item_uuid": "mi2",
                                    "item_name": "Clouds", "item_index": 1}}})
_ppt("audio-playlist-focused", "/v1/audio/playlist/focused", _id("a1", "Walk-in", 0),
     {"audio_playlist": {"focused": {"uuid": "a1", "name": "Walk-in", "index": 0}}})

# Transport.
_ppt("transport-audio-current", "/v1/transport/audio/current",
     {"is_playing": True, "uuid": "s1", "name": "Prelude", "artist": "Band", "audio_only": True,
      "duration": 245.3},
     {"transport": {"audio": {"playing": True, "uuid": "s1", "name": "Prelude", "artist": "Band",
                              "audio_only": True, "duration": 245.3}}})
_ppt("transport-presentation-time", "/v1/transport/presentation/time", "61.25",
     {"transport": {"presentation": {"time": 61.25}}})
_ppt("transport-announcement-auto-advance", "/v1/transport/announcement/auto_advance", "true",
     {"transport": {"announcement": {"auto_advance": True}}})

# Timers: settings replace the last list; the video countdown.
_ppt("timer-settings", "/v1/timers", [
    {"id": _id("t1", "Countdown", 0), "allows_overrun": True, "countdown": {"duration": 300}},
    {"id": _id("t2", "Service", 1), "allows_overrun": False,
     "count_down_to_time": {"time_of_day": 37800, "period": "am"}},
    {"id": _id("t3", "Elapsed", 2), "allows_overrun": False,
     "elapsed": {"start_time": 0, "end_time": 3600}}],
    {"timer_settings": {
        "t1": {"name": "Countdown", "index": 0, "allows_overrun": True, "countdown_duration": 300},
        "t2": {"name": "Service", "index": 1, "allows_overrun": False, "time_of_day": 37800,
               "period": "am"},
        "t3": {"name": "Elapsed", "index": 2, "allows_overrun": False, "elapsed_start": 0,
               "elapsed_end": 3600}}},
    state_before={"timer_settings": {"t0": {"name": "Deleted", "index": 0}}})
_ppt("video-countdown", "/v1/timer/video_countdown", '"00:01:30"',
     {"video_countdown": "00:01:30"})

# Props and messages: which are showing; a deleted one leaves.
_ppt("props", "/v1/props", [
    {"id": _id("pr1", "Logo", 0), "is_active": True, "auto_clear_enabled": True,
     "auto_clear_duration": 10.0, "transition": None},
    {"id": _id("pr2", "Lower Third", 1), "is_active": False, "auto_clear_enabled": False,
     "auto_clear_duration": 0}],
    {"props": {"pr1": {"name": "Logo", "index": 0, "active": True, "auto_clear_enabled": True,
                       "auto_clear_duration": 10.0},
               "pr2": {"name": "Lower Third", "index": 1, "active": False,
                       "auto_clear_enabled": False, "auto_clear_duration": 0.0}}},
    state_before={"props": {"gone": {"name": "Old", "active": False}}})
_ppt("messages", "/v1/messages", [
    {"id": _id("ms1", "Nursery", 0), "message": "Parent of {Number}", "tokens": [],
     "theme": _id("th1", "Default", 0), "visible_on_network": True, "is_active": True}],
    {"messages": {"ms1": {"name": "Nursery", "index": 0, "text": "Parent of {Number}",
                          "active": True, "visible_on_network": True, "theme": "Default"}}})

# Stage.
_ppt("stage-layout-map", "/v1/stage/layout_map", [
    {"screen": _id("sc1", "Confidence", 0), "layout": _id("ly2", "Lyrics", 1)}],
    {"stage": {"layout_map": {"sc1": {"screen_name": "Confidence", "layout_uuid": "ly2",
                                      "layout_name": "Lyrics"}}}},
    state_before={"stage": {"layout_map": {"old": {"screen_name": "Removed"}}}})
_ppt("stage-screens", "/v1/stage/screens", [_id("sc1", "Confidence", 0)],
     {"stage": {"screens": {"sc1": {"name": "Confidence", "index": 0}}}})
_ppt("stage-layouts", "/v1/stage/layouts", [{"id": _id("ly1", "Clock", 0)}, {"id": _id("ly2", "Lyrics", 1)}],
     {"stage": {"layouts": {"ly1": {"name": "Clock", "index": 0}, "ly2": {"name": "Lyrics", "index": 1}}}})
_ppt("stage-screens-enabled", "/v1/status/stage_screens", "false", {"stage_screens": False})

# Capture settings: the RTMP key is never kept.
_ppt("capture-settings", "/v1/capture/settings", {
    "source": "Audience", "audio_routing": [[1, 2]],
    "disk": {"file_location": "/Captures", "encoding": "h264",
             "resolution": {"width": 1920, "height": 1080}, "frame_rate": 29.97},
    "rtmp": {"url": "rtmp://live.example.net/app", "key": "secret-key", "encoding": "h264",
             "save_local": True, "file_location": "/Captures/rtmp"},
    "resi": {"event_name": "Sunday", "event_description": "", "destination_group": "",
             "encoding": "h264"}},
    {"capture": {"settings": {
        "source": "Audience",
        "disk": {"file_location": "/Captures", "encoding": "h264", "width": 1920, "height": 1080,
                 "frame_rate": 29.97},
        "rtmp": {"url": "rtmp://live.example.net/app", "encoding": "h264", "save_local": True,
                 "file_location": "/Captures/rtmp"},
        "resi": {"event_name": "Sunday", "encoding": "h264"}}}})
_ppt("screens", "/v1/status/screens", [
    {"id": _id("s1", "Main", 0), "size": {"width": 1920, "height": 1080}, "screen_type": "audience"}],
    {"screens": {"s1": {"name": "Main", "index": 0, "type": "audience", "width": 1920, "height": 1080}}})

# Configured lists.
_ppt("looks", "/v1/looks", [{"id": _id("l2", "Announcements", 0), "screens": []}],
     {"looks": {"l2": {"name": "Announcements", "index": 0}}},
     state_before={"looks": {"l9": {"name": "Deleted", "index": 3}}})
_ppt("macros", "/v1/macros", [{"id": _id("mc1", "Service Start", 0), "image_type": "",
                               "actions": []}],
     {"macros": {"mc1": {"name": "Service Start", "index": 0}}})
_ppt("libraries", "/v1/libraries", [{"id": _id("lb1", "Default", 0)}],
     {"libraries": {"lb1": {"name": "Default", "index": 0}}})
_ppt("video-inputs", "/v1/video_inputs", [_id("vi1", "Camera 1", 0)],
     {"video_inputs": {"vi1": {"name": "Camera 1", "index": 0}}})
_ppt("clear-groups", "/v1/clear/groups", [
    {"id": _id("cg1", "Clear Words", 0), "icon": "Slide", "tint": None,
     "layers": ["presentation", "props"], "stop_timeline_announcements": False,
     "stop_timeline_presentation": True, "clear_next_presentation": False}],
    {"clear_groups": {"cg1": {"name": "Clear Words", "index": 0, "icon": "Slide",
                              "layers": '["presentation","props"]'}}})
# Folders three deep, each with the folder it is in.
_ppt("playlists", "/v1/playlists", [
    {"id": _id("f1", "2026", 0), "type": "group", "playlists": [
        {"id": _id("f2", "October", 0), "type": "group", "playlists": [
            {"id": _id("p1", "Sunday", 0), "type": "playlist"}]}]},
    {"id": _id("p2", "Loose", 1), "type": "playlist"}],
    {"playlists": {
        "f1": {"name": "2026", "index": 0, "type": "group"},
        "p2": {"name": "Loose", "index": 1, "type": "playlist"},
        "f2": {"name": "October", "index": 0, "type": "group", "parent": "f1"},
        "p1": {"name": "Sunday", "index": 0, "type": "playlist", "parent": "f2"}}},
    state_before={"playlists": {"gone": {"name": "Deleted"}}})
_ppt("media-playlists", "/v1/media/playlists", [
    {"id": _id("mf", "Media", 0), "type": "group", "children": [
        {"id": _id("mp", "Backgrounds", 0), "type": "playlist"}]}],
    {"media_playlists": {"mf": {"name": "Media", "index": 0, "type": "group"},
                         "mp": {"name": "Backgrounds", "index": 0, "type": "playlist",
                                "parent": "mf"}}})

# Re-reads. A trigger reads what is live again at once.
_ppt("reread-trigger", "/v1/presentation/active/next/trigger", "", {},
     expect_then_send=_LIVE)
_ppt("reread-clear-layer", "/v1/clear/layer/slide", "", {}, expect_then_send=_LIVE)
_ppt("reread-transport", "/v1/transport/audio/skip_forward/10", "", {}, expect_then_send=_LIVE)
_ppt("reread-timer", "/v1/timer/t1/start", "", {},
     expect_then_send=[{"method": "GET", "target": "/v1/timers/current"}])
_ppt("reread-capture", "/v1/capture/start", "", {},
     expect_then_send=[{"method": "GET", "target": "/v1/capture/status"}])
# Configuration written with a body has its list read again.
_ppt("reread-look-put", "/v1/look/l1", "", {}, request={"id": _id("l1", "Worship", 1), "screens": []},
     expect_then_send=[{"method": "GET", "target": "/v1/look/current"},
                       {"method": "GET", "target": "/v1/looks"}])
_ppt("reread-look-current-put", "/v1/look/current", "", {},
     request={"id": _id("l1", "Worship", 1), "screens": []},
     expect_then_send=[{"method": "GET", "target": "/v1/look/current"}])
_ppt("reread-timer-put", "/v1/timer/t1", "", {},
     request={"id": _id("t1", "Countdown", 0), "allows_overrun": True, "countdown": {"duration": 120}},
     expect_then_send=[{"method": "GET", "target": "/v1/timers"}])
_ppt("reread-message-put", "/v1/message/ms1", "", {},
     request={"id": _id("ms1", "Nursery", 0), "message": "Parent of {Number}"},
     expect_then_send=[{"method": "GET", "target": "/v1/messages"}])
_ppt("reread-timer-create", "/v1/timers", "", {},
     request={"id": {"name": "New"}, "allows_overrun": False, "countdown": {"duration": 60}},
     expect_then_send=[{"method": "GET", "target": "/v1/timers"},
                       {"method": "GET", "target": "/v1/timers/current"}])
_ppt("reread-playlist-put", "/v1/playlist/pl1", "", {}, request=[_id("it3", "Amazing Grace", 0)],
     expect_then_send=[{"method": "GET", "target": "/v1/playlists"}])
# A delete has no body.
_ppt("reread-macro-delete", "/v1/macro/mc1", "", {},
     expect_then_send=[{"method": "GET", "target": "/v1/macros"}])
_ppt("reread-look-delete", "/v1/look/l2", "", {},
     expect_then_send=[{"method": "GET", "target": "/v1/looks"}])
# The polled reads ask for nothing: the poll's live look, the 30 s list, a
# focused playlist.
_ppt("reread-none-look-current", "/v1/look/current",
     {"id": _id("l1", "Worship", 1), "screens": []},
     {"look": {"uuid": "l1", "name": "Worship", "index": 1, "screens": "[]"}}, expect_then_send=[])
_ppt("reread-none-looks", "/v1/looks", [], {}, expect_then_send=[])
