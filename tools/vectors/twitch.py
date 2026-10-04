# Twitch (twitch): one vector per command, and telemetry vectors for the
# polled resources. Targets are the Helix endpoints as Twitch's API reference
# gives them (https://api.twitch.tv/helix/...), with query values
# percent-encoded by hand. Reply bodies follow the reference's examples; ids
# are made up.
TW = "twitch"
_TWS = {"client_id": "hof5gwx0su6owfnys0nyan9c87zr6t", "access_token": "2gbdx6oar67tqtcmt49t3wpcgycthx",
        "broadcaster_id": "141981764"}
_B = "broadcaster_id=141981764"
_BM = _B + "&moderator_id=141981764"


def _tw(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": TW, "command": command, "input": input, "settings": _TWS,
              "expect_request": request, **extra})


_USER = {"id": "141981764", "login": "twitchdev", "display_name": "TwitchDev", "type": "",
         "broadcaster_type": "partner", "description": "", "profile_image_url": "", "offline_image_url": "",
         "view_count": 0, "created_at": "2016-12-14T20:32:28Z"}

# Users.
_tw("get_user", {}, "GET", "/helix/users",
    http_reply={"status": 200, "body": json.dumps({"data": [_USER]})},
    expect_result={"ok": {"kind": "value", "value": [_USER]}})
_tw("get_users_by_login", {"login": "twitchdev"}, "GET", "/helix/users?login=twitchdev")

# Channel.
_tw("get_channel_information", {"broadcaster_id": "141981764"}, "GET", "/helix/channels?" + _B)
_tw("set_title", {"title": 'Sunday "Service"'}, "PATCH", "/helix/channels?" + _B,
    body='{"title":"Sunday \\"Service\\""}',
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_tw("set_category", {"game_id": "509658"}, "PATCH", "/helix/channels?" + _B, body='{"game_id":"509658"}')
_tw("set_language", {"language": "en"}, "PATCH", "/helix/channels?" + _B, body='{"broadcaster_language":"en"}')
_tw("set_tags", {"tags": ["English", "Church"]}, "PATCH", "/helix/channels?" + _B,
    body='{"tags":["English","Church"]}')
_tw("set_branded_content", {"enabled": True}, "PATCH", "/helix/channels?" + _B,
    body='{"is_branded_content":true}')
# Not a Partner: an ordinary failure, not a refusal.
_tw("set_stream_delay", {"delay": 30}, "PATCH", "/helix/channels?" + _B, body='{"delay":30}',
    http_reply={"status": 400, "body": json.dumps({"error": "Bad Request", "status": 400,
                                                   "message": "Delay is only available to partners"})},
    expect_result={"error": {"error": "device_error", "code": "400"}})
_CHANGES = {"title": "Sunday Service", "game_id": "509658", "broadcaster_language": "en"}
_tw("modify_channel_information", {"changes": _CHANGES}, "PATCH", "/helix/channels?" + _B,
    body=json.dumps(_CHANGES, separators=(",", ":")))

# Stream.
_KEY = [{"stream_key": "live_44322889_a34ub37c8ajv98a0"}]
_tw("get_stream_key", {}, "GET", "/helix/streams/key?" + _B,
    http_reply={"status": 200, "body": json.dumps({"data": _KEY})},
    expect_result={"ok": {"kind": "value", "value": _KEY}})
_tw("get_stream", {}, "GET", "/helix/streams?user_id=141981764&type=all")
_tw("get_follower_count", {}, "GET", "/helix/channels/followers?" + _B + "&first=1",
    http_reply={"status": 200, "body": json.dumps({"total": 8, "data": [], "pagination": {}})},
    expect_result={"ok": {"kind": "value", "value": 8}})
_tw("search_categories", {"query": "Just Chatting"}, "GET", "/helix/search/categories?query=Just%20Chatting&first=20")
_tw("create_stream_marker", {"description": "Sermon starts"}, "POST", "/helix/streams/markers",
    body='{"user_id":"141981764","description":"Sermon starts"}')
_tw("get_stream_markers", {"first": 5}, "GET", "/helix/streams/markers?user_id=141981764&first=5")
_tw("create_clip", {}, "POST", "/helix/clips?" + _B,
    http_reply={"status": 202, "body": json.dumps({"data": [{"id": "FiveWordsForClipSlug",
                                                            "edit_url": "https://clips.twitch.tv/FiveWordsForClipSlug/edit"}]})},
    expect_result={"ok": {"kind": "value", "value": [{"id": "FiveWordsForClipSlug",
                                                      "edit_url": "https://clips.twitch.tv/FiveWordsForClipSlug/edit"}]}})

# Ads.
_tw("start_commercial", {"length": 60}, "POST", "/helix/channels/commercial",
    body='{"broadcaster_id":"141981764","length":60}')
_tw("get_ad_schedule", {}, "GET", "/helix/channels/ads?" + _B)
_tw("snooze_next_ad", {}, "POST", "/helix/channels/ads/schedule/snooze?" + _B)

# Raids.
_tw("start_raid", {"to_broadcaster_id": "12826"}, "POST",
    "/helix/raids?from_broadcaster_id=141981764&to_broadcaster_id=12826")
_tw("cancel_raid", {}, "DELETE", "/helix/raids?" + _B,
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})

# Chat.
_tw("get_chat_settings", {}, "GET", "/helix/chat/settings?" + _BM)
_tw("update_chat_settings", {"changes": {"emote_mode": False, "slow_mode": True, "slow_mode_wait_time": 10}},
    "PATCH", "/helix/chat/settings?" + _BM, body='{"emote_mode":false,"slow_mode":true,"slow_mode_wait_time":10}')
_tw("slow_mode_on", {"wait_time": 10}, "PATCH", "/helix/chat/settings?" + _BM,
    body='{"slow_mode":true,"slow_mode_wait_time":10}')
_tw("slow_mode_off", {}, "PATCH", "/helix/chat/settings?" + _BM, body='{"slow_mode":false}')
_tw("follower_mode_on", {"duration": 10}, "PATCH", "/helix/chat/settings?" + _BM,
    body='{"follower_mode":true,"follower_mode_duration":10}')
_tw("follower_mode_off", {}, "PATCH", "/helix/chat/settings?" + _BM, body='{"follower_mode":false}')
_tw("subscriber_mode", {"enabled": True}, "PATCH", "/helix/chat/settings?" + _BM, body='{"subscriber_mode":true}')
_tw("emote_mode", {"enabled": False}, "PATCH", "/helix/chat/settings?" + _BM, body='{"emote_mode":false}')
_tw("unique_chat_mode", {"enabled": True}, "PATCH", "/helix/chat/settings?" + _BM, body='{"unique_chat_mode":true}')
_tw("chat_delay_on", {"duration": "6"}, "PATCH", "/helix/chat/settings?" + _BM,
    body='{"non_moderator_chat_delay":true,"non_moderator_chat_delay_duration":6}')
_tw("chat_delay_off", {}, "PATCH", "/helix/chat/settings?" + _BM, body='{"non_moderator_chat_delay":false}')
_tw("send_announcement", {"message": "Welcome, everyone!", "color": "purple"}, "POST",
    "/helix/chat/announcements?" + _BM, body='{"message":"Welcome, everyone!","color":"purple"}',
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
# A plain access token (no refresh token) refused: 401 is terminal, reported as auth.
_tw("send_chat_message", {"message": "Hello"}, "POST", "/helix/chat/messages",
    body='{"broadcaster_id":"141981764","sender_id":"141981764","message":"Hello"}',
    http_reply={"status": 401, "body": json.dumps({"error": "Unauthorized", "status": 401,
                                                   "message": "Invalid OAuth token"})},
    expect_result={"error": {"error": "auth"}})

# Telemetry.
telemetry(TW, "user", settings=_TWS, inbound_http={
    "path": "/helix/users", "body": json.dumps({"data": [_USER]})},
    expect_state={"user": {"login": "twitchdev", "display_name": "TwitchDev", "broadcaster_type": "partner"}})
telemetry(TW, "channel", settings=_TWS, inbound_http={
    "path": "/helix/channels?" + _B,
    "body": json.dumps({"data": [{
        "broadcaster_id": "141981764", "broadcaster_login": "twitchdev", "broadcaster_name": "TwitchDev",
        "broadcaster_language": "en", "game_id": "509670", "game_name": "Science & Technology",
        "title": "TwitchDev Monthly Update", "delay": 0, "tags": ["DevsInTheKnow"],
        "content_classification_labels": ["Gambling", "DrugsIntoxication"], "is_branded_content": False}]})},
    expect_state={"channel": {
        "title": "TwitchDev Monthly Update", "category_id": "509670", "category_name": "Science & Technology",
        "language": "en", "tags": '["DevsInTheKnow"]', "content_labels": '["Gambling","DrugsIntoxication"]',
        "branded_content": False, "delay": 0}})
telemetry(TW, "followers", settings=_TWS, inbound_http={
    "path": "/helix/channels/followers?" + _B + "&first=1",
    "body": json.dumps({"total": 8, "data": [{"user_id": "11111", "user_name": "UserDisplayName",
                                              "user_login": "userloginname", "followed_at": "2022-05-24T22:22:08Z"}],
                        "pagination": {"cursor": "eyJiIjpudWxsLCJhIjp7Ik9mZnNldCI6NX19"}})},
    expect_state={"channel": {"followers": 8}})
telemetry(TW, "stream-live", settings=_TWS, inbound_http={
    "path": "/helix/streams?user_id=141981764&type=all",
    "body": json.dumps({"data": [{
        "id": "40952121085", "user_id": "141981764", "user_login": "twitchdev", "user_name": "TwitchDev",
        "game_id": "509670", "game_name": "Science & Technology", "type": "live",
        "title": "TwitchDev Monthly Update", "tags": ["DevsInTheKnow"], "viewer_count": 78365,
        "started_at": "2026-10-04T15:00:01Z", "language": "en",
        "thumbnail_url": "https://static-cdn.jtvnw.net/previews-ttv/live_user_twitchdev-{width}x{height}.jpg",
        "tag_ids": [], "is_mature": False}], "pagination": {}})},
    expect_state={"stream": {"live": True, "id": "40952121085", "viewer_count": 78365,
                             "started_at": "2026-10-04T15:00:01Z", "title": "TwitchDev Monthly Update",
                             "category_name": "Science & Technology"}})
telemetry(TW, "stream-offline", settings=_TWS, inbound_http={
    "path": "/helix/streams?user_id=141981764&type=all", "body": json.dumps({"data": [], "pagination": {}})},
    expect_state={"stream": {"live": False, "id": "", "viewer_count": 0, "started_at": ""}})
telemetry(TW, "chat-settings", settings=_TWS, inbound_http={
    "path": "/helix/chat/settings?" + _BM,
    "body": json.dumps({"data": [{
        "broadcaster_id": "141981764", "slow_mode": True, "slow_mode_wait_time": 10, "follower_mode": True,
        "follower_mode_duration": 0, "subscriber_mode": False, "emote_mode": False, "unique_chat_mode": False,
        "non_moderator_chat_delay": True, "non_moderator_chat_delay_duration": 4,
        "moderator_id": "141981764"}]})},
    expect_state={"chat": {"slow_mode": True, "slow_mode_wait_time": 10, "follower_mode": True,
                           "follower_mode_duration": 0, "subscriber_mode": False, "emote_mode": False,
                           "unique_chat_mode": False, "non_moderator_chat_delay": True,
                           "non_moderator_chat_delay_duration": 4}})
