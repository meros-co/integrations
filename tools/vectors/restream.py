# Restream (restream): one vector per command, and telemetry vectors for the
# polled resources. Targets are the endpoints as Restream's developer
# reference gives them (https://api.restream.io/v2/...), and the legacy
# channel endpoints as Bitfocus's Restream module sends them. Reply bodies
# follow the reference's examples; ids are made up.
RS = "restream"
_RSS = {"client_id": "a1b2c3d4", "access_token": "2a407cfcfdf8920242fd6dc7e5f83c86374ef925"}
_RSE = "2527849f-f961-4b1d-8ae0-8eae4f068327"
_RSD = "8f14e45f-ceea-467a-9575-9a1f5a5b1a51"
_RSB = "f47ac10b-58cc-4372-a567-0e02b2c3d479"
_RST = "c3d4e5f6-a7b8-9012-cdef-345678901234"
_RSQ = "d4e5f6a7-b8c9-0123-def4-567890123456"
_RSP = "31a1e161-67aa-4a9e-a95b-3cb37ab6ea89"


def _rs(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": RS, "command": command, "input": input, "settings": _RSS,
              "expect_request": request, **extra})


# Account and reference data.
_rs("get_profile", {}, "GET", "/v2/user/profile",
    http_reply={"status": 200, "body": json.dumps({"id": 104, "username": "church", "email": "av@example.org"})},
    expect_result={"ok": {"kind": "value", "value": {"id": 104, "username": "church", "email": "av@example.org"}}})
_rs("get_selected_ingest", {}, "GET", "/v2/user/ingest",
    http_reply={"status": 200, "body": json.dumps({"ingestId": 8})},
    expect_result={"ok": {"kind": "value", "value": 8}})
_KEY = {"streamKey": "re_123_abc", "srtUrl": None}
_rs("get_stream_key", {}, "GET", "/v2/user/streamKey",
    http_reply={"status": 200, "body": json.dumps(_KEY)},
    expect_result={"ok": {"kind": "value", "value": _KEY}})
_rs("get_chat_url", {}, "GET", "/v2/user/webchat/url")
_rs("list_platforms", {}, "GET", "/v2/platform/all")
_rs("list_ingest_servers", {}, "GET", "/v2/server/all")

# Channels.
_rs("list_channels", {}, "GET", "/v2/user/channels")
_rs("get_channel", {"channel_id": 123456}, "GET", "/v2/user/channels/123456")
_CH = {"platformId": 78, "streamUrl": "srt://example.com:9000"}
_rs("add_channel", {"channel": _CH}, "POST", "/v2/user/channels", body=json.dumps(_CH, separators=(",", ":")))
_rs("add_rtmp_channel", {"stream_url": "rtmp://example.com/live", "stream_key": "abc123", "display_name": "Overflow room"},
    "POST", "/v2/user/channels",
    body='{"platformId":29,"streamUrl":"rtmp://example.com/live","streamKey":"abc123","displayName":"Overflow room"}',
    http_reply={"status": 201, "body": json.dumps({"id": 123456, "platformId": 29, "channelUrl": "https://example.com/live",
                                                  "displayName": "Overflow room"})},
    expect_result={"ok": {"kind": "value", "value": {"id": 123456, "platformId": 29,
                                                     "channelUrl": "https://example.com/live",
                                                     "displayName": "Overflow room"}}})
_rs("delete_channel", {"channel_id": 123456}, "DELETE", "/v2/user/channels/123456",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_rs("set_channel_enabled", {"channel_id": 123456, "enabled": False}, "PATCH", "/v2/user/channel/123456",
    body='{"active":false}', http_reply={"status": 200, "body": "{}"}, expect_result={"ok": {"kind": "ack"}})
_rs("get_channel_meta", {"channel_id": 123456}, "GET", "/v2/user/channel-meta/123456")
_rs("set_channel_title", {"channel_id": 123456, "title": "Sunday \"Service\""}, "PATCH", "/v2/user/channel-meta/123456",
    body='{"title":"Sunday \\"Service\\""}')
_rs("set_channel_meta", {"channel_id": 123456, "title": "Sunday Service", "description": "Live from the main hall"},
    "PATCH", "/v2/user/channel-meta/123456",
    body='{"title":"Sunday Service","description":"Live from the main hall"}')

# Events.
_rs("list_upcoming_events", {}, "GET", "/v2/user/events/upcoming")
_rs("list_in_progress_events", {}, "GET", "/v2/user/events/in-progress")
_rs("list_events_history", {"page": 2, "limit": 5}, "GET", "/v2/user/events/history?page=2&limit=5")
_rs("get_event", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE)
_rs("create_event", {"title": "Sunday Service", "description": "Episode 12"}, "POST", "/v2/user/events/new",
    body='{"streamType":"encoder","title":"Sunday Service","description":"Episode 12"}',
    http_reply={"status": 201, "body": json.dumps({"id": _RSE})},
    expect_result={"ok": {"kind": "value", "value": _RSE}})
# A title Restream refuses: an ordinary failure.
_rs("create_scheduled_event", {"title": "Sunday Service", "stream_type": "studio", "scheduled_for": "2026-10-11T09:30:00Z"},
    "POST", "/v2/user/events/new",
    body='{"streamType":"studio","title":"Sunday Service","description":"","scheduledFor":"2026-10-11T09:30:00Z"}',
    http_reply={"status": 400, "body": json.dumps({"error": {"statusCode": 400, "status": 400, "code": 400,
                                                            "message": "EventTitleTooLong",
                                                            "name": "event_title_too_long"}})},
    expect_result={"error": {"error": "device_error", "code": "400"}})
_EV = {"streamType": "file", "fileId": "a1b2c3d4-e5f6-7890-abcd-ef1234567890", "loopsCount": 3, "title": "Replay"}
_rs("create_event_custom", {"event": _EV}, "POST", "/v2/user/events/new", body=json.dumps(_EV, separators=(",", ":")))
_rs("add_event_destination", {"event_id": _RSE, "channel_id": 12345}, "POST",
    "/v2/user/events/" + _RSE + "/destinations", body='{"channelId":12345,"streamingOrientation":"horizontal"}',
    http_reply={"status": 201, "body": json.dumps({"id": _RSD})},
    expect_result={"ok": {"kind": "value", "value": _RSD}})
_DST = {"channelId": 12345, "title": "Sunday Service", "privacyStatus": "unlisted", "latencyPreference": "low"}
_rs("add_event_destination_custom", {"event_id": _RSE, "destination": _DST}, "POST",
    "/v2/user/events/" + _RSE + "/destinations", body=json.dumps(_DST, separators=(",", ":")))
_rs("delete_event_destination", {"event_id": _RSE, "destination_id": _RSD}, "DELETE",
    "/v2/user/events/" + _RSE + "/destinations/" + _RSD,
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_rs("get_event_stream_key", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE + "/streamKey")
_rs("get_event_srt_stream_key", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE + "/srt/streamKey")
_rs("list_event_recordings", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE + "/recordings")
_rs("get_recording_download_url", {"event_id": _RSE, "file_name": "video-2026-02-26-22-01-19.mp4"}, "POST",
    "/v2/user/events/" + _RSE + "/recordings/download-url", body='{"fileName":"video-2026-02-26-22-01-19.mp4"}',
    http_reply={"status": 200, "body": json.dumps({"downloadUrl": "https://example.com/recording-download"})},
    expect_result={"ok": {"kind": "value", "value": "https://example.com/recording-download"}})
_rs("list_event_transcriptions", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE + "/recordings/transcriptions")
_rs("get_event_chat_history", {"event_id": _RSE, "page_size": 50}, "GET",
    "/v2/user/events/" + _RSE + "/chat/history?pageSize=50")
_rs("get_chat_history_download_url", {"event_id": _RSE}, "POST", "/v2/user/events/" + _RSE + "/chat/history/download-url")
_rs("get_event_viewer_analytics", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE + "/analytics/viewers")
_rs("get_event_chat_analytics", {"event_id": _RSE}, "GET", "/v2/user/events/" + _RSE + "/analytics/messages")

# Studio.
_rs("list_brands", {}, "GET", "/v2/user/studio/brands")
_rs("list_fonts", {}, "GET", "/v2/user/studio/fonts")
_rs("list_countdown_music", {}, "GET", "/v2/user/studio/audio/countdown-music")
_rs("list_audio_backgrounds", {}, "GET", "/v2/user/studio/audio-backgrounds")
_rs("list_tickers", {}, "GET", "/v2/user/studio/tickers")
_rs("create_ticker", {"text": "Welcome!", "brand_id": _RSB}, "POST", "/v2/user/studio/tickers",
    body='{"text":"Welcome!","brandId":"' + _RSB + '"}',
    http_reply={"status": 201, "body": json.dumps({"id": _RST, "text": "Welcome!", "brandId": _RSB})},
    expect_result={"ok": {"kind": "value", "value": _RST}})
_rs("update_ticker", {"ticker_id": _RST, "text": "Next: the sermon"}, "PATCH", "/v2/user/studio/tickers/" + _RST,
    body='{"text":"Next: the sermon"}')
_rs("delete_ticker", {"ticker_id": _RST}, "DELETE", "/v2/user/studio/tickers/" + _RST)
_rs("reorder_tickers", {"ids": [_RST, _RSQ]}, "PATCH", "/v2/user/studio/tickers/order",
    body='{"ids":["' + _RST + '","' + _RSQ + '"]}',
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_rs("list_captions", {}, "GET", "/v2/user/studio/captions")
_rs("create_caption", {"text": "Jane Doe", "secondary_text": "Pastor", "brand_id": _RSB}, "POST",
    "/v2/user/studio/captions", body='{"text":"Jane Doe","secondaryText":"Pastor","brandId":"' + _RSB + '"}')
_rs("update_caption", {"caption_id": _RST, "text": "John Doe"}, "PATCH", "/v2/user/studio/captions/" + _RST,
    body='{"text":"John Doe","secondaryText":""}')
_rs("delete_caption", {"caption_id": _RST}, "DELETE", "/v2/user/studio/captions/" + _RST)
_rs("list_qr_codes", {}, "GET", "/v2/user/studio/qr-codes")
_rs("create_qr_code", {"title": "Give", "link": "https://example.org/give", "brand_id": _RSB}, "POST",
    "/v2/user/studio/qr-codes",
    body='{"brandId":"' + _RSB + '","title":"Give","link":"https://example.org/give","shouldShowTitle":true}')
_rs("update_qr_code", {"qr_code_id": _RSQ, "title": "Give", "link": "https://example.org/give", "show_title": False},
    "PATCH", "/v2/user/studio/qr-codes/" + _RSQ,
    body='{"title":"Give","link":"https://example.org/give","shouldShowTitle":false}')
_rs("delete_qr_code", {"qr_code_id": _RSQ}, "DELETE", "/v2/user/studio/qr-codes/" + _RSQ)
_rs("reorder_qr_codes", {"ids": [_RSQ]}, "PATCH", "/v2/user/studio/qr-codes/order", body='{"ids":["' + _RSQ + '"]}')

# Storage and clips.
_rs("list_storage_files", {}, "GET", "/v2/user/storage/files")
_rs("list_clip_projects", {}, "GET", "/v2/user/clips/projects?limit=20")
_rs("get_clip_project", {"project_id": _RSP}, "GET", "/v2/user/clips/projects/" + _RSP)
_PRJ = {"sourceType": "Event", "eventId": _RSE, "title": "Sunday highlights"}
# A refused token with no refresh token: terminal, reported as auth.
_rs("create_clip_project", {"project": _PRJ}, "POST", "/v2/user/clips/projects",
    body=json.dumps(_PRJ, separators=(",", ":")),
    http_reply={"status": 401, "body": json.dumps({"error": {"statusCode": 401, "status": 401, "code": 401,
                                                            "message": "Invalid token: access token is invalid",
                                                            "name": "invalid_token"}})},
    expect_result={"error": {"error": "auth"}})


# Telemetry.
def _rs_event(status, started=None, finished=None, record_only=False):
    return {"id": _RSE, "status": status, "title": "Sunday Service", "description": "Episode 12",
            "coverUrl": None, "isRecordOnly": record_only, "scheduledFor": 1791710400, "startedAt": started,
            "finishedAt": finished,
            "destinations": [{"id": _RSD, "channelId": 1, "externalUrl": None, "streamingPlatformId": 5}]}


telemetry(RS, "profile", settings=_RSS, inbound_http={
    "path": "/v2/user/profile", "body": json.dumps({"id": 104, "username": "church", "email": "av@example.org"})},
    expect_state={"user": {"id": 104, "username": "church"}})
telemetry(RS, "selected-ingest", settings=_RSS, inbound_http={
    "path": "/v2/user/ingest", "body": json.dumps({"ingestId": 8})},
    expect_state={"ingest": {"selected_id": 8}})
telemetry(RS, "in-progress", settings=_RSS, inbound_http={
    "path": "/v2/user/events/in-progress", "body": json.dumps([_rs_event("in-progress", started=1791710460)])},
    expect_state={"stream": {"live": True, "event_id": _RSE},
                  "events": {_RSE: {"status": "in-progress", "title": "Sunday Service", "description": "Episode 12",
                                    "scheduled_for": 1791710400, "started_at": 1791710460, "record_only": False}}})
telemetry(RS, "none-in-progress", settings=_RSS, inbound_http={
    "path": "/v2/user/events/in-progress", "body": "[]"},
    expect_state={"stream": {"live": False, "event_id": ""}})
telemetry(RS, "upcoming", settings=_RSS, inbound_http={
    "path": "/v2/user/events/upcoming", "body": json.dumps([_rs_event("upcoming")])},
    expect_state={"events": {_RSE: {"status": "upcoming", "title": "Sunday Service", "description": "Episode 12",
                                    "scheduled_for": 1791710400, "record_only": False}}})
telemetry(RS, "history", settings=_RSS, inbound_http={
    "path": "/v2/user/events/history?page=1&limit=10",
    "body": json.dumps({"items": [_rs_event("finished", started=1791710460, finished=1791717660)],
                        "pagination": {"pages_total": 1, "page": 1, "limit": 10}})},
    expect_state={"events": {_RSE: {"status": "finished", "title": "Sunday Service", "description": "Episode 12",
                                    "scheduled_for": 1791710400, "started_at": 1791710460,
                                    "finished_at": 1791717660, "record_only": False}}})
telemetry(RS, "channels", settings=_RSS, inbound_http={
    "path": "/v2/user/channels",
    "body": json.dumps({"channels": [
        {"id": 123456, "platformId": 29, "channelUrl": "https://example.com/live", "displayName": "My Custom RTMP"},
        {"id": 123457, "platformId": 73, "channelUrl": "https://instagram.com/xxx", "displayName": "My Instagram"}]})},
    expect_state={"channels": {
        "123456": {"platform_id": 29, "display_name": "My Custom RTMP", "url": "https://example.com/live"},
        "123457": {"platform_id": 73, "display_name": "My Instagram", "url": "https://instagram.com/xxx"}}})
telemetry(RS, "channel-enabled-legacy", settings=_RSS, inbound_http={
    "path": "/v2/user/channel/all",
    "body": json.dumps([{"id": 123456, "streamingPlatformId": 29, "displayName": "My Custom RTMP", "enabled": True},
                        {"id": 123457, "streamingPlatformId": 73, "displayName": "My Instagram", "enabled": False}])},
    expect_state={"channels": {"123456": {"enabled": True}, "123457": {"enabled": False}}})

# Streaming Updates, pushed on the websocket (Private API, Streaming Updates);
# the messages follow the reference's IUpdates types, values made up.
_RSU = "f6b1c2d3e4"
telemetry(RS, "update-incoming", settings=_RSS, inbound_ws=json.dumps({
    "action": "updateIncoming", "userId": 1, "eventId": _RSE, "createdAt": 1791230000, "suid": _RSU,
    "streaming": {"fps": 29.97, "keyframeInterval": 2, "lossRate": 0, "bitrate": {"total": 6128000, "audio": 128000,
                  "video": 6000000}, "codec": {"audio": "aac", "video": "h264"}, "profileAndLevel": "High@4.1",
                  "height": 1080, "width": 1920}}),
    expect_state={"incoming": {_RSU: {"event_id": _RSE, "started_at": 1791230000, "fps": 29.97,
                                      "keyframe_interval": 2.0, "loss_rate": 0.0, "bitrate": 6128000,
                                      "video_bitrate": 6000000, "audio_bitrate": 128000, "video_codec": "h264",
                                      "audio_codec": "aac", "profile": "High@4.1", "width": 1920, "height": 1080}}})
telemetry(RS, "delete-incoming", settings=_RSS, inbound_ws=json.dumps({
    "action": "deleteIncoming", "userId": 1, "eventId": _RSE, "createdAt": 1791230000, "suid": _RSU}),
    state_before={"incoming": {_RSU: {"fps": 29.97}}}, expect_state={"incoming": {}})
telemetry(RS, "update-outgoing", settings=_RSS, inbound_ws=json.dumps({
    "action": "updateOutgoing", "userId": 1, "eventId": _RSE, "platformId": 5, "channelId": 123456,
    "createdAt": 1791230005, "channelIdentifier": "x", "eventIdentifier": "y",
    "streaming": {"status": "CONNECTED", "bitrate": 6100000, "bufferedBytes": 0}}),
    expect_state={"channels": {"123456": {"outgoing": {"status": "CONNECTED", "bitrate": 6100000,
                                                       "buffered_bytes": 0, "started_at": 1791230005,
                                                       "event_id": _RSE}}}})
telemetry(RS, "delete-outgoing", settings=_RSS, inbound_ws=json.dumps({
    "action": "deleteOutgoing", "userId": 1, "eventId": _RSE, "platformId": 5, "channelId": 123456,
    "createdAt": 1791230005}),
    state_before={"channels": {"123456": {"display_name": "YouTube", "outgoing": {"status": "CONNECTED"}}}},
    expect_state={"channels": {"123456": {"display_name": "YouTube"}}})
telemetry(RS, "update-statuses", settings=_RSS, inbound_ws=json.dumps({
    "action": "updateStatuses", "userId": 1, "eventId": _RSE, "platformId": 5, "channelId": 123456,
    "createdAt": 1791230005, "updatedAt": 1791230060, "channelIdentifier": "x", "eventIdentifier": "y",
    "channelViews": None, "followers": 1520, "gameTitle": None, "online": True, "streamViews": 88,
    "title": "Sunday Service", "viewers": 42}),
    expect_state={"channels": {"123456": {"platform": {"online": True, "viewers": 42, "stream_views": 88,
                                                       "followers": 1520, "title": "Sunday Service",
                                                       "updated_at": 1791230060}}}})
