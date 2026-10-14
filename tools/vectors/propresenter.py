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
    "/v1/look/current", "/v1/playlist/active", "/v1/presentation/focused",
    "/v1/presentation/active", "/v1/announcement/active")]

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
# The update stream (POST /v1/status/updates): each part {"url", "data"} is
# read as the reply to that GET would be.
telemetry(PPT, "stream-countdown",
          inbound_sse={"data": json.dumps({"url": "/v1/timer/video_countdown", "data": "00:01:30"})},
          expect_state={"video_countdown": "00:01:30"})
telemetry(PPT, "stream-slide",
          inbound_sse={"data": json.dumps({"url": "/v1/status/slide", "data": {
              "current": {"text": "Amazing grace", "notes": ""}, "next": {"text": "How sweet the sound", "notes": ""}}})},
          expect_state={"slide": {"current": {"text": "Amazing grace", "notes": ""},
                                  "next": {"text": "How sweet the sound", "notes": ""}}})
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

# The active presentation's groups and slides (presentationGetActive), the
# list replaced; slides kept as JSON text per group.
_WHITE = {"red": 1, "green": 1, "blue": 1, "alpha": 1}
_SLIDE = {"enabled": True, "notes": "Slow", "text": "Amazing grace", "label": "", "color": _WHITE,
          "size": {"width": 1920, "height": 1080}}
_ppt("presentation-active", "/v1/presentation/active", {"presentation": {
    "id": _id("pr1", "Amazing Grace", 0),
    "groups": [{"name": "Verse 1", "color": _WHITE, "slides": [_SLIDE]},
               {"name": "Chorus", "color": _WHITE, "slides": []}],
    "has_timeline": False, "has_timeline_items": False, "presentation_path": "/Library/Amazing Grace.pro",
    "destination": "presentation"}},
    {"presentation": {"groups": {
        "0": {"name": "Verse 1", "color": json.dumps(_WHITE, separators=(",", ":")),
              "slides": json.dumps([_SLIDE], separators=(",", ":"))},
        "1": {"name": "Chorus", "color": json.dumps(_WHITE, separators=(",", ":")), "slides": "[]"}},
        "has_timeline": False, "has_timeline_items": False, "path": "/Library/Amazing Grace.pro"}},
    state_before={"presentation": {"groups": {"0": {"name": "Old"}, "1": {"name": "Old"},
                                              "2": {"name": "Old"}}}})
_ppt("announcement-active", "/v1/announcement/active", {"announcement": {
    "id": _id("an1", "Welcome Loop", 0),
    "groups": [{"name": "Loop", "color": _WHITE, "slides": []}],
    "has_timeline": True, "has_timeline_items": True, "presentation_path": "/Library/Welcome.pro",
    "destination": "announcements"}},
    {"announcement": {"groups": {"0": {"name": "Loop", "color": json.dumps(_WHITE, separators=(",", ":")),
                                       "slides": "[]"}},
                      "has_timeline": True, "has_timeline_items": True, "path": "/Library/Welcome.pro"}})
# Each playlist (not a folder) has its items read whenever the playlists are.
_ppt("playlists-read-items", "/v1/playlists", [
    {"id": _id("f1", "2026", 0), "type": "group", "playlists": [
        {"id": _id("f2", "October", 0), "type": "group", "playlists": [
            {"id": _id("p1", "Sunday", 0), "type": "playlist"}]}]},
    {"id": _id("p2", "Loose", 1), "type": "playlist"}],
    {"playlists": {
        "f1": {"name": "2026", "index": 0, "type": "group"},
        "p2": {"name": "Loose", "index": 1, "type": "playlist"},
        "f2": {"name": "October", "index": 0, "type": "group", "parent": "f1"},
        "p1": {"name": "Sunday", "index": 0, "type": "playlist", "parent": "f2"}}},
    expect_then_send=[{"method": "GET", "target": "/v1/playlist/p2"},
                      {"method": "GET", "target": "/v1/playlist/p1"}])
_ppt("media-playlists-read-items", "/v1/media/playlists", [
    {"id": _id("mf", "Media", 0), "type": "group", "children": [
        {"id": _id("mp", "Backgrounds", 0), "type": "playlist"}]}],
    {"media_playlists": {"mf": {"name": "Media", "index": 0, "type": "group"},
                         "mp": {"name": "Backgrounds", "index": 0, "type": "playlist",
                                "parent": "mf"}}},
    expect_then_send=[{"method": "GET", "target": "/v1/media/playlist/mp"}])
# A playlist's items (playlistGet), by position, the list replaced.
_ppt("playlist-items", "/v1/playlist/p1", {"id": _id("p1", "Sunday", 0), "items": [
    {"id": _id("i1", "Welcome", 0), "type": "header", "is_hidden": False, "is_pco": False,
     "header_color": _WHITE},
    {"id": _id("i2", "Amazing Grace", 1), "type": "presentation", "is_hidden": False, "is_pco": True,
     "presentation_info": {"presentation_uuid": "pr1", "arrangement_name": "Default"},
     "target_uuid": "pr1"},
    {"id": _id("i3", "Walk-in", 2), "type": "media", "is_hidden": True, "is_pco": False, "duration": 95,
     "target_uuid": "m1"}]},
    {"playlist_items": {"p1": {
        "0": {"uuid": "i1", "name": "Welcome", "type": "header", "hidden": False, "pco": False,
              "header_color": json.dumps(_WHITE, separators=(",", ":"))},
        "1": {"uuid": "i2", "name": "Amazing Grace", "type": "presentation", "hidden": False, "pco": True,
              "presentation_uuid": "pr1", "arrangement": "Default", "target_uuid": "pr1"},
        "2": {"uuid": "i3", "name": "Walk-in", "type": "media", "hidden": True, "pco": False,
              "duration": 95, "target_uuid": "m1"}}}},
    state_before={"playlist_items": {"p1": {"3": {"uuid": "gone"}}}})
_ppt("playlist-items-empty", "/v1/playlist/p1", {"id": _id("p1", "Sunday", 0), "items": []},
     {"playlist_items": {}}, state_before={"playlist_items": {"p1": {"0": {"uuid": "gone"}}}})
_ppt("media-playlist-items", "/v1/media/playlist/mp", {"id": _id("mp", "Backgrounds", 0), "items": [
    {"id": _id("m1", "Clouds", 0), "type": "video", "artist": "", "duration": 30}]},
    {"media_playlist_items": {"mp": {"0": {"uuid": "m1", "name": "Clouds", "type": "video", "artist": "",
                                           "duration": 30}}}})
_ppt("audio-playlist-items", "/v1/audio/playlist/ap", {"id": _id("ap", "Pre-service", 0), "items": [
    {"id": _id("a1", "Song", 0), "type": "audio", "artist": "Band", "duration": 240}]},
    {"audio_playlist_items": {"ap": {"0": {"uuid": "a1", "name": "Song", "type": "audio", "artist": "Band",
                                           "duration": 240}}}})

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
