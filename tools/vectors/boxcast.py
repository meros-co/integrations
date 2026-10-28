# BoxCast (boxcast): one vector per command, and telemetry vectors for the
# polled resources. Targets are the endpoints as BoxCast's API reference
# gives them (https://rest.boxcast.com/...); reply bodies follow its
# examples, ids are the reference's or made up.
BX = "boxcast"
_BXS = {"client_id": "cid1000", "access_token": "YKLCJ-EJHnFXV4TThF7YNPdkyz3ia29UmCJY3vZOB"}
_BXB = "Z7a0XjcgnYpUIuKqvxGk"
_BXC = "m9c4ebfb82112"
_BXH = "h9a0Xacgn7pUIuKqvxHj"
_BXR = "Q9b1VkadIxkdsI93nkdco"
_BXU = "e0lbabxqsqfetjjc5bfs"
_BXCH = "dSUdyxXH1loPJYIlR4F0"


def _bx(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": BX, "command": command, "input": input, "settings": _BXS,
              "expect_request": request, **extra})


_BXBROADCAST = {
    "id": _BXB, "name": "Celtics vs Knicks", "channel_id": "celtics-vs-knicks", "stream_source": "boxcaster",
    "boxcaster_id": _BXC, "boxcaster_name": "Main Camera", "starts_at": "2013-04-28T23:00:00Z",
    "stops_at": "2013-04-29T02:00:00Z", "timeframe": "future", "is_private": False, "is_ticketed": True,
    "ticket_price": 5.95, "free_variant": "low", "viewer_url": "https://www.boxcast.com/show/#/celtics-vs-knicks",
    "description": "Lorem ipsum dolor sit...", "in_channel_ids": [],
    "static_playlist_url": "https://play.boxcast.com/p/Z7a0XjcgnYpUIuKqvxGk/v/all-ext.m3u8?signed=yes"}

# Account and BoxCasters.
_bx("get_account", {}, "GET", "/account")
_bx("list_boxcasters", {}, "GET", "/account/boxcasters?s=name")
_bx("search_boxcasters", {"query": "name:*Camera*", "sort": "id"}, "GET", "/account/boxcasters?q=name%3A%2ACamera%2A&s=id")
_bx("get_boxcaster", {"boxcaster_id": _BXC}, "GET", "/account/boxcasters/" + _BXC)
_bx("rename_boxcaster", {"boxcaster_id": _BXC, "name": "New Name"}, "PUT", "/account/boxcasters/" + _BXC,
    body='{"name":"New Name"}',
    http_reply={"status": 422, "body": json.dumps({"error": "unprocessable_entity",
                                                   "error_description": "The request could not be processed.",
                                                   "invalid_fields": {"name": "can't be blank"}})},
    expect_result={"error": {"error": "device_error", "code": "422"}})

# Broadcasts.
_bx("list_broadcasts", {"page": 2}, "GET", "/account/broadcasts?p=2&l=20&s=-starts_at")
_bx("search_broadcasts", {"query": "football +timeframe:current", "sort": "name"}, "GET",
    "/account/broadcasts?q=football%20%2Btimeframe%3Acurrent&p=0&l=20&s=name")
_bx("list_live_broadcasts", {}, "GET",
    "/account/broadcasts?q=timeframe%3Apreroll%20timeframe%3Acurrent&s=starts_at&l=100")
_bx("list_upcoming_broadcasts", {}, "GET", "/account/broadcasts?q=timeframe%3Afuture&s=starts_at&l=50")
_bx("get_broadcast", {"broadcast_id": _BXB}, "GET", "/account/broadcasts/" + _BXB)
_bx("schedule_broadcast", {"name": "Celtics vs Knicks", "boxcaster_id": "n9c4ebf123456",
                           "starts_at": "2013-04-28T23:00:00Z", "stops_at": "2013-04-29T02:00:00Z"},
    "POST", "/account/broadcasts",
    body='{"name":"Celtics vs Knicks","stream_source":"boxcaster","boxcaster_id":"n9c4ebf123456",'
         '"starts_at":"2013-04-28T23:00:00Z","stops_at":"2013-04-29T02:00:00Z","is_private":false}',
    http_reply={"status": 200, "body": json.dumps(_BXBROADCAST)},
    expect_result={"ok": {"kind": "value", "value": _BXB}})
_bx("schedule_rtmp_broadcast", {"name": "Sunday Service", "rtmp_source_id": _BXR, "starts_at": "now",
                                "stops_at": "2026-11-01T17:00:00Z", "private": True},
    "POST", "/account/broadcasts",
    body='{"name":"Sunday Service","stream_source":"rtmp","static_rtmp_id":"Q9b1VkadIxkdsI93nkdco",'
         '"starts_at":"now","stops_at":"2026-11-01T17:00:00Z","is_private":true}')
_NEWB = {"name": "Graduation", "stream_source": "boxcaster", "boxcaster_id": _BXC, "audio_source": "embedded",
         "starts_at": "2026-06-01T14:00:00Z", "stops_at": "2026-06-01T17:00:00Z", "target_video_resolution": 1080}
_bx("schedule_broadcast_custom", {"broadcast": _NEWB}, "POST", "/account/broadcasts",
    body=json.dumps(_NEWB, separators=(",", ":")))
_CHB = {"name": "The Big Game", "description": "This is a really big deal."}
_bx("update_broadcast", {"broadcast_id": _BXB, "changes": _CHB}, "PUT", "/account/broadcasts/" + _BXB,
    body=json.dumps(_CHB, separators=(",", ":")),
    http_reply={"status": 200, "body": json.dumps(dict(_BXBROADCAST, **_CHB))},
    expect_result={"ok": {"kind": "ack"}})
_bx("rename_broadcast", {"broadcast_id": _BXB, "name": "The \"Big\" Game"}, "PUT", "/account/broadcasts/" + _BXB,
    body='{"name":"The \\"Big\\" Game"}')
_bx("set_broadcast_description", {"broadcast_id": _BXB, "description": "Line one\nLine two"}, "PUT",
    "/account/broadcasts/" + _BXB, body='{"description":"Line one\\nLine two"}')
_bx("set_broadcast_times", {"broadcast_id": _BXB, "starts_at": "2026-11-01T15:00:00Z",
                            "stops_at": "2026-11-01T17:00:00Z"},
    "PUT", "/account/broadcasts/" + _BXB, body='{"starts_at":"2026-11-01T15:00:00Z","stops_at":"2026-11-01T17:00:00Z"}')
_bx("set_broadcast_start", {"broadcast_id": _BXB, "starts_at": "now"}, "PUT", "/account/broadcasts/" + _BXB,
    body='{"starts_at":"now"}')
_bx("set_broadcast_stop", {"broadcast_id": _BXB, "stops_at": "now"}, "PUT", "/account/broadcasts/" + _BXB,
    body='{"stops_at":"now"}')
_bx("set_broadcast_private", {"broadcast_id": _BXB, "private": True}, "PUT", "/account/broadcasts/" + _BXB,
    body='{"is_private":true}')
_bx("set_broadcast_audio_source", {"broadcast_id": _BXB, "source": "analog"}, "PUT", "/account/broadcasts/" + _BXB,
    body='{"audio_source":"analog"}')
_bx("set_broadcast_boxcaster", {"broadcast_id": _BXB, "boxcaster_id": "m9c4ebf96d67a"}, "PUT",
    "/account/broadcasts/" + _BXB, body='{"boxcaster_id":"m9c4ebf96d67a"}')
_bx("set_broadcast_ticketing", {"broadcast_id": _BXB, "ticketed": True, "price": 5.95, "free_variant": "low"},
    "PUT", "/account/broadcasts/" + _BXB, body='{"is_ticketed":true,"ticket_price":5.95,"free_variant":"low"}')
_bx("set_broadcast_channels", {"broadcast_id": _BXB, "channel_ids": [_BXCH]}, "PUT", "/account/broadcasts/" + _BXB,
    body='{"in_channel_ids":["dSUdyxXH1loPJYIlR4F0"]}')
_bx("delete_broadcast", {"broadcast_id": "x1LUy0tRVj9mXuOEWP7k"}, "DELETE", "/account/broadcasts/x1LUy0tRVj9mXuOEWP7k",
    http_reply={"status": 200, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_bx("prepare_recording_download", {"recording_id": "rec123", "audio_only": True}, "POST",
    "/account/recordings/rec123/download", body='{"audio_only":true}',
    http_reply={"status": 201, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_bx("get_recording", {"recording_id": "rec123"}, "GET", "/account/recordings/rec123")

# Channels.
_bx("list_channels", {}, "GET", "/account/channels?s=name&l=100")
_bx("search_channels", {"query": "name:*Basketball*"}, "GET", "/account/channels?q=name%3A%2ABasketball%2A&s=name")
_bx("get_channel", {"channel_id": _BXB}, "GET", "/account/channels/" + _BXB)
_bx("create_channel", {"name": "Basketball Channel", "description": ""}, "POST", "/account/channels",
    body='{"name":"Basketball Channel","description":""}',
    http_reply={"status": 200, "body": json.dumps({"id": _BXB, "name": "Basketball Channel", "is_private": False,
                                                   "is_ticketed": False, "ticket_price": 0, "free_variant": "best",
                                                   "description": ""})},
    expect_result={"ok": {"kind": "value", "value": _BXB}})
_bx("create_channel_custom", {"channel": {"name": "Season pass", "is_ticketed": True, "ticket_price": 24.95}},
    "POST", "/account/channels", body='{"name":"Season pass","is_ticketed":true,"ticket_price":24.95}')
_bx("update_channel", {"channel_id": _BXB, "changes": {"free_variant": "audio"}}, "PUT", "/account/channels/" + _BXB,
    body='{"free_variant":"audio"}')
_bx("rename_channel", {"channel_id": _BXB, "name": "Basketball Channel"}, "PUT", "/account/channels/" + _BXB,
    body='{"name":"Basketball Channel"}')
_bx("set_channel_ticketing", {"channel_id": _BXB, "ticketed": False}, "PUT", "/account/channels/" + _BXB,
    body='{"is_ticketed":false,"ticket_price":0.00,"free_variant":"none"}')
_bx("list_channel_broadcasts", {"channel_id": _BXCH}, "GET", "/account/channels/" + _BXCH + "/broadcasts?p=0&l=20")

# Highlights.
_BXHL = "/account/broadcasts/" + _BXB + "/highlights"
_bx("list_highlights", {"broadcast_id": _BXB}, "GET", _BXHL + "?l=100")
_bx("get_highlight", {"broadcast_id": _BXB, "highlight_id": _BXH}, "GET", _BXHL + "/" + _BXH)
_bx("create_highlight", {"broadcast_id": _BXB, "name": "Tip-off", "description": "", "start_seconds": 120,
                         "stop_seconds": 140},
    "POST", _BXHL, body='{"name":"Tip-off","description":"","start_seconds":120,"stop_seconds":140}',
    http_reply={"status": 200, "body": json.dumps({"id": _BXH, "name": "Tip-off", "broadcast_id": _BXB,
                                                   "start_seconds": 120, "stop_seconds": 140, "duration": 20})},
    expect_result={"ok": {"kind": "value", "value": _BXH}})
_bx("update_highlight", {"broadcast_id": _BXB, "highlight_id": _BXH, "changes": {"stop_seconds": 130}}, "PUT",
    _BXHL + "/" + _BXH, body='{"stop_seconds":130}')
_bx("rename_highlight", {"broadcast_id": _BXB, "highlight_id": _BXH, "name": "Tip-off Won By Knicks",
                         "description": "Way to start the game!"},
    "PUT", _BXHL + "/" + _BXH, body='{"name":"Tip-off Won By Knicks","description":"Way to start the game!"}')
_bx("set_highlight_range", {"broadcast_id": _BXB, "highlight_id": _BXH, "start_seconds": 120, "stop_seconds": 130},
    "PUT", _BXHL + "/" + _BXH, body='{"start_seconds":120,"stop_seconds":130}')
_bx("delete_highlight", {"broadcast_id": _BXB, "highlight_id": _BXH}, "DELETE", _BXHL + "/" + _BXH)
_bx("prepare_highlight_download", {"highlight_id": _BXH}, "POST", "/account/highlights/" + _BXH + "/download",
    body='{"audio_only":false}')
_bx("get_highlight_download", {"highlight_id": _BXH}, "GET", "/account/highlights/" + _BXH)

# RTMP sources.
_BXRTMP = {"id": _BXR, "name": "Wirecast", "description": "Production booth Wirecast computer",
           "server_url": "rtmps://rtmp.boxcast.com/live", "stream_key": "Qid9aksdhlk9eiSKNd"}
_bx("list_rtmp_sources", {}, "GET", "/account/static_rtmps?s=name&l=100")
_bx("get_rtmp_source", {"rtmp_source_id": _BXR}, "GET", "/account/static_rtmps/" + _BXR)
_bx("get_rtmp_ingest", {"rtmp_source_id": _BXR}, "GET", "/account/static_rtmps/" + _BXR,
    http_reply={"status": 200, "body": json.dumps(_BXRTMP)},
    expect_result={"ok": {"kind": "value", "value": "Qid9aksdhlk9eiSKNd"}})
_bx("create_rtmp_source", {"name": "Wirecast", "description": "Production booth Wirecast computer"}, "POST",
    "/account/static_rtmps", body='{"name":"Wirecast","description":"Production booth Wirecast computer"}',
    http_reply={"status": 200, "body": json.dumps(_BXRTMP)}, expect_result={"ok": {"kind": "value", "value": _BXR}})
_bx("rename_rtmp_source", {"rtmp_source_id": _BXR, "name": "New Name"}, "PUT", "/account/static_rtmps/" + _BXR,
    body='{"name":"New Name"}')
_bx("set_rtmp_source_description", {"rtmp_source_id": _BXR, "description": "Booth B"}, "PUT",
    "/account/static_rtmps/" + _BXR, body='{"description":"Booth B"}')
_bx("regenerate_rtmp_stream_key", {"rtmp_source_id": _BXR}, "PUT", "/account/static_rtmps/" + _BXR,
    body='{"regenerate_stream_key":true}')
_bx("delete_rtmp_source", {"rtmp_source_id": _BXR}, "DELETE", "/account/static_rtmps/" + _BXR)

# Uploads.
_bx("create_upload", {"filename": "overlay.png", "filesize": 123, "mimetype": "image/png"}, "POST",
    "/account/uploads", body='{"filename":"overlay.png","filesize":123,"mimetype":"image/png","slot":"overlay"}')
_bx("mark_upload_uploaded", {"upload_id": _BXU}, "PUT", "/account/uploads/" + _BXU, body='{"status":"uploaded"}')
_bx("get_upload", {"upload_id": _BXU}, "GET", "/account/uploads/" + _BXU)
_bx("list_uploads", {}, "GET", "/account/uploads?filter.slot=overlay&s=-created_at")

# Viewer analytics.
_bx("get_viewer_analytics", {"id": "gnz2vfaodkceal0sk3ot"}, "GET",
    "/account/viewer_analytics/gnz2vfaodkceal0sk3ot?include_viewer_timeseries=false")
_bx("get_viewer_analytics_range", {"id": "gnz2vfaodkceal0sk3ot", "since": "2020-03-11T00:00:00-04:00",
                                   "until": "2020-04-11", "timeseries": True}, "GET",
    "/account/viewer_analytics/gnz2vfaodkceal0sk3ot?since=2020-03-11T00%3A00%3A00-04%3A00&until=2020-04-11"
    "&include_viewer_timeseries=true",
    # An expired token: terminal, reported as auth.
    http_reply={"status": 401, "body": json.dumps({"error": "invalid_token",
                                                   "error_description": "The access token is expired."})},
    expect_result={"error": {"error": "auth"}})

# Telemetry.
telemetry(BX, "account", settings=_BXS, inbound_http={"path": "/account", "body": json.dumps({
    "id": "MTVUYQLN", "name": "The Ocho", "channel_id": "3nRUBPr00vqUSYYKfBtX",
    "description": "The obscure sports network.", "time_zone": "Pacific Time (US & Canada)",
    "static_playlist_url": "https://play.boxcast.com/p/MTVUYQLN/v/all-ext.m3u8?signed=yes"})},
    expect_state={"account": {"id": "MTVUYQLN", "name": "The Ocho", "channel_id": "3nRUBPr00vqUSYYKfBtX",
                              "description": "The obscure sports network.",
                              "time_zone": "Pacific Time (US & Canada)"}})
_BXCASTERS = [
    {"id": _BXC, "name": "Main Camera", "status": "broadcasting", "warning": "none",
     "last_seen_at": "2012-10-09T15:44:09Z", "next_broadcast_at": "2012-10-09T15:00:00Z",
     "next_broadcast_id": "e3u2Te__7aaAqUNkjvpj", "next_broadcast_name": "Smith Wedding 10/9/12"},
    {"id": "m9c4ebf96d67a", "name": "Mobile Camera", "status": "ready", "warning": "updating_firmware",
     "last_seen_at": "2012-10-09T15:44:17Z", "next_broadcast_at": "9999-12-31T23:59:59Z",
     "next_broadcast_id": "", "next_broadcast_name": ""}]
telemetry(BX, "boxcasters", settings=_BXS,
          inbound_http={"path": "/account/boxcasters?s=name", "body": json.dumps(_BXCASTERS)},
          state_before={"boxcasters": {"gone1": {"name": "Sold", "status": "offline"}}},
          expect_state={"boxcasters": {
              _BXC: {"name": "Main Camera", "status": "broadcasting", "warning": "none",
                     "last_seen_at": "2012-10-09T15:44:09Z", "next_broadcast_at": "2012-10-09T15:00:00Z",
                     "next_broadcast_id": "e3u2Te__7aaAqUNkjvpj", "next_broadcast_name": "Smith Wedding 10/9/12"},
              "m9c4ebf96d67a": {"name": "Mobile Camera", "status": "ready", "warning": "updating_firmware",
                                "last_seen_at": "2012-10-09T15:44:17Z",
                                "next_broadcast_at": "9999-12-31T23:59:59Z",
                                "next_broadcast_id": "", "next_broadcast_name": ""}}})
telemetry(BX, "boxcaster", settings=_BXS, inbound_http={"path": "/account/boxcasters/" + _BXC, "body": json.dumps(
    dict(_BXCASTERS[0], channel_id="dSUdyxXH1loPJYIlR4F0"))},
    expect_state={"boxcasters": {_BXC: {
        "name": "Main Camera", "status": "broadcasting", "warning": "none", "last_seen_at": "2012-10-09T15:44:09Z",
        "next_broadcast_at": "2012-10-09T15:00:00Z", "next_broadcast_id": "e3u2Te__7aaAqUNkjvpj",
        "next_broadcast_name": "Smith Wedding 10/9/12", "channel_id": "dSUdyxXH1loPJYIlR4F0"}}})
_BXLIVE = {"id": "1", "name": "Hockey Game vs St. Marks (Audio Only)", "channel_id": "hockey-game-vs-st-marks-166959",
           "stream_source": "app", "boxcaster_id": "", "boxcaster_name": "", "starts_at": "2016-12-01T12:30:00Z",
           "stops_at": "2016-12-02T14:30:00Z", "timeframe": "current", "is_private": False, "is_ticketed": False,
           "ticket_price": 0, "free_variant": "best", "viewer_url": "https://boxcast.tv/view/hockey-game-vs-st-marks-166959",
           "preview": "https://assets.boxcast.com/latest/static/audio-only.png",
           "poster": "https://assets.boxcast.com/latest/static/audio-only.png", "total_active_viewer_count": 12,
           "total_minutes_viewed": 340, "total_view_count": 40}
_BXLIVESTATE = {"name": "Hockey Game vs St. Marks (Audio Only)", "channel_id": "hockey-game-vs-st-marks-166959",
                "stream_source": "app", "boxcaster_id": "", "boxcaster_name": "",
                "starts_at": "2016-12-01T12:30:00Z", "stops_at": "2016-12-02T14:30:00Z", "timeframe": "current",
                "is_private": False, "is_ticketed": False, "ticket_price": 0.0, "free_variant": "best",
                "viewer_url": "https://boxcast.tv/view/hockey-game-vs-st-marks-166959",
                "preview": "https://assets.boxcast.com/latest/static/audio-only.png",
                "poster": "https://assets.boxcast.com/latest/static/audio-only.png",
                "total_active_viewer_count": 12, "total_minutes_viewed": 340.0, "total_view_count": 40}
telemetry(BX, "on-air", settings=_BXS, inbound_http={
    "path": "/account/broadcasts?q=timeframe%3Apreroll%20timeframe%3Acurrent&s=starts_at&l=100",
    "body": json.dumps([_BXLIVE])},
    # A broadcast that ended leaves the on-air list, not broadcasts.
    state_before={"on_air": {"old": {"name": "Earlier", "timeframe": "current"}}},
    expect_state={"on_air": {"1": {"name": "Hockey Game vs St. Marks (Audio Only)", "timeframe": "current"}},
                  "broadcasts": {"1": _BXLIVESTATE}})
telemetry(BX, "broadcast", settings=_BXS,
          inbound_http={"path": "/account/broadcasts/" + _BXB, "body": json.dumps(dict(_BXBROADCAST, recording_id="rec123"))},
          expect_state={"broadcasts": {_BXB: {
              "name": "Celtics vs Knicks", "channel_id": "celtics-vs-knicks", "stream_source": "boxcaster",
              "boxcaster_id": _BXC, "boxcaster_name": "Main Camera", "starts_at": "2013-04-28T23:00:00Z",
              "stops_at": "2013-04-29T02:00:00Z", "timeframe": "future", "is_private": False, "is_ticketed": True,
              "ticket_price": 5.95, "free_variant": "low",
              "viewer_url": "https://www.boxcast.com/show/#/celtics-vs-knicks",
              "description": "Lorem ipsum dolor sit...", "in_channel_ids": "[]", "recording_id": "rec123"}}})
telemetry(BX, "broadcast-deleted", settings=_BXS,
          inbound_http={"path": "/account/broadcasts/" + _BXB, "body": ""},
          state_before={"broadcasts": {_BXB: {"name": "Celtics vs Knicks"}}, "on_air": {_BXB: {"name": "Celtics vs Knicks"}}},
          expect_state={"broadcasts": {}, "on_air": {}})
telemetry(BX, "highlights", settings=_BXS, inbound_http={"path": _BXHL + "?l=100", "body": json.dumps([
    {"id": _BXH, "name": "Tip-off Won By Knicks", "broadcast_id": _BXB, "description": "Way to start the game!",
     "duration": 20, "start_seconds": 120, "stop_seconds": 140}])},
    expect_state={"broadcasts": {_BXB: {"highlights": {_BXH: {
        "name": "Tip-off Won By Knicks", "description": "Way to start the game!", "duration": 20.0,
        "start_seconds": 120, "stop_seconds": 140}}}}})
telemetry(BX, "recording", settings=_BXS, inbound_http={"path": "/account/recordings/rec123", "body": json.dumps(
    {"id": "rec123", "audio_download_status": "", "audio_download_url": "", "download_status": "processing:50",
     "download_url": ""})},
    expect_state={"recordings": {"rec123": {"download_status": "processing:50", "download_url": "",
                                            "audio_download_status": "", "audio_download_url": ""}}})
telemetry(BX, "highlight-download", settings=_BXS, inbound_http={"path": "/account/highlights/" + _BXH, "body": json.dumps(
    {"id": _BXH, "audio_download_status": "", "audio_download_url": "", "download_status": "ready",
     "download_url": "https://example.com/h.mp4"})},
    expect_state={"highlight_downloads": {_BXH: {"download_status": "ready", "download_url": "https://example.com/h.mp4",
                                                 "audio_download_status": "", "audio_download_url": ""}}})
telemetry(BX, "channels", settings=_BXS, inbound_http={"path": "/account/channels?s=name&l=100", "body": json.dumps([
    {"id": _BXB, "name": "Basketball Channel", "channel_id": "2016-basketball-season", "is_ticketed": True,
     "ticket_price": 24.95, "free_variant": ""}])},
    expect_state={"channels": {_BXB: {"name": "Basketball Channel", "channel_id": "2016-basketball-season",
                                      "is_ticketed": True, "ticket_price": 24.95, "free_variant": ""}}})
telemetry(BX, "rtmp-sources", settings=_BXS,
          inbound_http={"path": "/account/static_rtmps?s=name&l=100", "body": json.dumps([_BXRTMP])},
          expect_state={"rtmp_sources": {_BXR: {"name": "Wirecast", "description": "Production booth Wirecast computer",
                                                "server_url": "rtmps://rtmp.boxcast.com/live"}}})
telemetry(BX, "uploads", settings=_BXS,
          inbound_http={"path": "/account/uploads?filter.slot=overlay&s=-created_at", "body": json.dumps([
              {"id": _BXU, "filename": "overlay.png", "filesize": 123, "mimetype": "image/png",
               "created_at": "2020-12-18T00:00:00Z", "description": "", "slot": "overlay",
               "view_url": "https://uploads.boxcast.com/overlay.png", "status": "uploaded"}])},
          expect_state={"uploads": {_BXU: {"filename": "overlay.png", "filesize": 123, "mimetype": "image/png",
                                           "created_at": "2020-12-18T00:00:00Z", "slot": "overlay",
                                           "view_url": "https://uploads.boxcast.com/overlay.png",
                                           "status": "uploaded"}}})
telemetry(BX, "analytics", settings=_BXS, inbound_http={
    "path": "/account/viewer_analytics/gnz2vfaodkceal0sk3ot?include_viewer_timeseries=false",
    "body": json.dumps({"live_view_count": 0, "live_view_seconds": 0, "live_buffer_seconds": 0,
                        "recorded_view_count": 7, "recorded_view_seconds": 13, "recorded_buffer_seconds": 6,
                        "unique_viewer_count": 2, "viewed_broadcast_count": 1, "active_viewer_log": None})},
    expect_state={"analytics": {"gnz2vfaodkceal0sk3ot": {
        "live_view_count": 0, "live_view_seconds": 0, "live_buffer_seconds": 0, "recorded_view_count": 7,
        "recorded_view_seconds": 13, "recorded_buffer_seconds": 6, "unique_viewer_count": 2,
        "viewed_broadcast_count": 1}}})
