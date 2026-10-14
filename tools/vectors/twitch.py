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

# EventSub over WebSocket: the messages as Twitch's reference shows them
# (Handling WebSocket Events, EventSub Subscription Types).
_TWSES = "AQoQILE98gtqShGmLD7AM6yJThAB"


def _tw_sub(kind, version, condition):
    return {"method": "POST", "target": "/helix/eventsub/subscriptions",
            "body": json.dumps({"type": kind, "version": version, "condition": condition,
                                "transport": {"method": "websocket", "session_id": _TWSES}},
                               separators=(",", ":"))}


_TWB = {"broadcaster_user_id": "141981764"}
telemetry(TW, "eventsub-welcome", settings=_TWS, inbound_ws=json.dumps({
    "metadata": {"message_id": "96a3f3b5-5dec-4eed-908e-e11ee657416c", "message_type": "session_welcome",
                 "message_timestamp": "2023-07-19T14:56:51.634234626Z"},
    "payload": {"session": {"id": _TWSES, "status": "connected", "connected_at": "2023-07-19T14:56:51.616329898Z",
                            "keepalive_timeout_seconds": 30, "reconnect_url": None}}}),
    expect_state={},
    expect_then_send=[_tw_sub("stream.online", "1", _TWB), _tw_sub("stream.offline", "1", _TWB),
                      _tw_sub("channel.update", "2", _TWB),
                      _tw_sub("channel.follow", "2", {**_TWB, "moderator_user_id": "141981764"})]
    + [_tw_sub(t, "1", _TWB) for t in ("channel.poll.begin", "channel.poll.progress", "channel.poll.end",
                                       "channel.prediction.begin", "channel.prediction.progress",
                                       "channel.prediction.lock", "channel.prediction.end")]
    + [_tw_sub(t, "1", {**_TWB, "moderator_user_id": "141981764"})
       for t in ("channel.shield_mode.begin", "channel.shield_mode.end")]
    + [_tw_sub(t, "1", _TWB) for t in ("channel.channel_points_custom_reward_redemption.add",
                                       "channel.channel_points_custom_reward_redemption.update",
                                       "channel.moderator.add", "channel.moderator.remove",
                                       "channel.vip.add", "channel.vip.remove")])


def _tw_note(kind, version, event):
    return json.dumps({
        "metadata": {"message_id": "befa7b53-d79d-478f-86b9-120f112b044e", "message_type": "notification",
                     "message_timestamp": "2023-07-19T10:11:12.464757833Z", "subscription_type": kind,
                     "subscription_version": version},
        "payload": {"subscription": {"id": "f1c2a387-161a-49f9-a165-0f21d7a4e1c4", "status": "enabled",
                                     "type": kind, "version": version, "cost": 0, "condition": _TWB,
                                     "transport": {"method": "websocket", "session_id": _TWSES},
                                     "created_at": "2023-07-19T10:11:12.464757833Z"},
                    "event": event}})


_TWU = {"broadcaster_user_id": "141981764", "broadcaster_user_login": "twitchdev",
        "broadcaster_user_name": "TwitchDev"}
telemetry(TW, "eventsub-online", settings=_TWS, inbound_ws=_tw_note("stream.online", "1", {
    "id": "9001", **_TWU, "type": "live", "started_at": "2020-10-11T10:11:12.123Z"}),
    expect_state={"stream": {"live": True, "id": "9001", "started_at": "2020-10-11T10:11:12.123Z"}},
    expect_then_send=[{"method": "GET", "target": "/helix/streams?user_id=141981764&type=all"}])
telemetry(TW, "eventsub-offline", settings=_TWS, inbound_ws=_tw_note("stream.offline", "1", _TWU),
          expect_state={"stream": {"live": False, "id": "", "viewer_count": 0, "started_at": ""}})
telemetry(TW, "eventsub-channel-update", settings=_TWS, inbound_ws=_tw_note("channel.update", "2", {
    **_TWU, "title": "Best Stream Ever", "language": "en", "category_id": "12453", "category_name": "Grand Theft Auto",
    "content_classification_labels": ["MatureGame"]}),
    expect_state={"channel": {"title": "Best Stream Ever", "language": "en", "category_id": "12453",
                              "category_name": "Grand Theft Auto", "content_labels": '["MatureGame"]'}})
telemetry(TW, "eventsub-follow", settings=_TWS, inbound_ws=_tw_note("channel.follow", "2", {
    "user_id": "1234", "user_login": "cool_user", "user_name": "Cool_User", **_TWU,
    "followed_at": "2020-07-15T18:16:11.17106713Z"}),
    expect_state={},
    expect_then_send=[{"method": "GET", "target": "/helix/channels/followers?broadcaster_id=141981764&first=1"}])

# Polls (Create Poll, End Poll, Get Polls).
_POLL = {"id": "ed961efd-8a3f-4cf5-a9d0-e616c590cd2a", "broadcaster_id": "141981764", "broadcaster_name": "TwitchDev",
         "broadcaster_login": "twitchdev", "title": "Heads or Tails?",
         "choices": [{"id": "4c123012-1351-4f33-84b7-43856e7a0f47", "title": "Heads", "votes": 0,
                      "channel_points_votes": 0, "bits_votes": 0},
                     {"id": "279087e3-54a7-467e-bcd0-c1393fcea4f0", "title": "Tails", "votes": 0,
                      "channel_points_votes": 0, "bits_votes": 0}],
         "bits_voting_enabled": False, "bits_per_vote": 0, "channel_points_voting_enabled": True,
         "channel_points_per_vote": 100, "status": "ACTIVE", "duration": 1800,
         "started_at": "2021-03-19T06:08:33.871278372Z"}
_CHOICES = [{"title": "Heads"}, {"title": "Tails"}]
_tw("create_poll", {"title": "Heads or Tails?", "choices": _CHOICES, "duration": 1800}, "POST", "/helix/polls",
    body='{"broadcaster_id":"141981764","title":"Heads or Tails?","choices":[{"title":"Heads"},{"title":"Tails"}],"duration":1800}',
    http_reply={"status": 200, "body": json.dumps({"data": [_POLL]})},
    expect_result={"ok": {"kind": "value", "value": [_POLL]}})
_tw("create_poll_with_channel_points", {"title": "Heads or Tails?", "choices": _CHOICES, "duration": 1800,
                                        "points_per_vote": 100}, "POST", "/helix/polls",
    body='{"broadcaster_id":"141981764","title":"Heads or Tails?","choices":[{"title":"Heads"},{"title":"Tails"}],'
         '"duration":1800,"channel_points_voting_enabled":true,"channel_points_per_vote":100}')
_tw("end_poll", {"id": "ed961efd-8a3f-4cf5-a9d0-e616c590cd2a", "status": "TERMINATED"}, "PATCH", "/helix/polls",
    body='{"broadcaster_id":"141981764","id":"ed961efd-8a3f-4cf5-a9d0-e616c590cd2a","status":"TERMINATED"}')
_tw("get_polls", {}, "GET", "/helix/polls?" + _B + "&first=1")

# Predictions.
_tw("create_prediction", {"title": "Any leaks in the stream?", "outcomes": [{"title": "Yes, give it time."},
                                                                         {"title": "Definitely not."}],
                          "prediction_window": 120}, "POST", "/helix/predictions",
    body='{"broadcaster_id":"141981764","title":"Any leaks in the stream?","outcomes":[{"title":"Yes, give it time."},'
         '{"title":"Definitely not."}],"prediction_window":120}')
_PRED = "bc637af0-7766-4525-9308-4112f4cbf178"
_tw("lock_prediction", {"id": _PRED}, "PATCH", "/helix/predictions",
    body='{"broadcaster_id":"141981764","id":"' + _PRED + '","status":"LOCKED"}')
_tw("resolve_prediction", {"id": _PRED, "winning_outcome_id": "73085848-a94d-4040-9d21-2cb7a89374b7"}, "PATCH",
    "/helix/predictions",
    body='{"broadcaster_id":"141981764","id":"' + _PRED + '","status":"RESOLVED",'
         '"winning_outcome_id":"73085848-a94d-4040-9d21-2cb7a89374b7"}')
_tw("cancel_prediction", {"id": _PRED}, "PATCH", "/helix/predictions",
    body='{"broadcaster_id":"141981764","id":"' + _PRED + '","status":"CANCELED"}')
_tw("get_predictions", {"first": 5}, "GET", "/helix/predictions?" + _B + "&first=5")

# Moderation.
_tw("ban_user", {"user_id": "9876", "reason": "no reason"}, "POST", "/helix/moderation/bans?" + _BM,
    body='{"data":{"user_id":"9876","reason":"no reason"}}')
_tw("timeout_user", {"user_id": "9876", "duration": 300, "reason": "no reason"}, "POST",
    "/helix/moderation/bans?" + _BM, body='{"data":{"user_id":"9876","duration":300,"reason":"no reason"}}')
_tw("unban_user", {"user_id": "5432"}, "DELETE", "/helix/moderation/bans?" + _BM + "&user_id=5432",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_tw("get_banned_users", {}, "GET", "/helix/moderation/banned?" + _B + "&first=100")
_tw("warn_user", {"user_id": "9876", "reason": "stop doing that!"}, "POST", "/helix/moderation/warnings?" + _BM,
    body='{"data":{"user_id":"9876","reason":"stop doing that!"}}')
_tw("delete_chat_message", {"message_id": "abc-123-def"}, "DELETE",
    "/helix/moderation/chat?" + _BM + "&message_id=abc-123-def")
_tw("clear_chat", {}, "DELETE", "/helix/moderation/chat?" + _BM)
_tw("get_shield_mode", {}, "GET", "/helix/moderation/shield_mode?" + _BM)
_tw("set_shield_mode", {"active": True}, "PUT", "/helix/moderation/shield_mode?" + _BM, body='{"is_active":true}')
_tw("get_moderators", {}, "GET", "/helix/moderation/moderators?" + _B + "&first=100")
_tw("add_moderator", {"user_id": "44444"}, "POST", "/helix/moderation/moderators?" + _B + "&user_id=44444")
_tw("remove_moderator", {"user_id": "44444"}, "DELETE", "/helix/moderation/moderators?" + _B + "&user_id=44444")
_tw("get_vips", {}, "GET", "/helix/channels/vips?" + _B + "&first=100")
_tw("add_vip", {"user_id": "456"}, "POST", "/helix/channels/vips?" + _B + "&user_id=456")
_tw("remove_vip", {"user_id": "456"}, "DELETE", "/helix/channels/vips?" + _B + "&user_id=456")
_tw("get_blocked_terms", {}, "GET", "/helix/moderation/blocked_terms?" + _BM + "&first=100")
_tw("add_blocked_term", {"text": "A phrase I'm not fond of"}, "POST", "/helix/moderation/blocked_terms?" + _BM,
    body='{"text":"A phrase I\'m not fond of"}')
_tw("remove_blocked_term", {"id": "c9fc79b8-0f63-4ef7-9d38-efd811e74ac2"}, "DELETE",
    "/helix/moderation/blocked_terms?" + _BM + "&id=c9fc79b8-0f63-4ef7-9d38-efd811e74ac2")
_tw("send_shoutout", {"to_broadcaster_id": "626262"}, "POST",
    "/helix/chat/shoutouts?from_broadcaster_id=141981764&to_broadcaster_id=626262&moderator_id=141981764")

# Channel Points.
_REWARD = "92af127c-7326-4483-a52b-b0da0be61c01"
_tw("get_custom_rewards", {}, "GET", "/helix/channel_points/custom_rewards?" + _B + "&only_manageable_rewards=false")
_tw("create_custom_reward", {"reward": {"title": "game analysis 1v1", "cost": 50000}}, "POST",
    "/helix/channel_points/custom_rewards?" + _B, body='{"title":"game analysis 1v1","cost":50000}')
_tw("update_custom_reward", {"id": _REWARD, "changes": {"cost": 30000}}, "PATCH",
    "/helix/channel_points/custom_rewards?" + _B + "&id=" + _REWARD, body='{"cost":30000}')
_tw("set_reward_enabled", {"id": _REWARD, "enabled": False}, "PATCH",
    "/helix/channel_points/custom_rewards?" + _B + "&id=" + _REWARD, body='{"is_enabled":false}')
_tw("set_reward_paused", {"id": _REWARD, "paused": True}, "PATCH",
    "/helix/channel_points/custom_rewards?" + _B + "&id=" + _REWARD, body='{"is_paused":true}')
_tw("delete_custom_reward", {"id": _REWARD}, "DELETE", "/helix/channel_points/custom_rewards?" + _B + "&id=" + _REWARD)
_tw("get_redemptions", {"reward_id": _REWARD}, "GET",
    "/helix/channel_points/custom_rewards/redemptions?" + _B + "&reward_id=" + _REWARD + "&status=UNFULFILLED&first=50")
_tw("update_redemption_status", {"reward_id": _REWARD, "id": "17fa2df1-ad76-4804-bfa5-a40ef63efe63",
                                 "status": "CANCELED"}, "PATCH",
    "/helix/channel_points/custom_rewards/redemptions?id=17fa2df1-ad76-4804-bfa5-a40ef63efe63&" + _B
    + "&reward_id=" + _REWARD, body='{"status":"CANCELED"}')

# Schedule. Segment ids are base64 text: = is percent-encoded in a query.
_SEG = "eyJzZWdtZW50SUQiOiJlNGFjYzcyNC0zNzFmLTQwMmMtODFjYS0yM2FkYTc5NzU5ZDQiLCJpc29ZZWFyIjoyMDIxLCJpc29XZWVrIjoyNn0="
_SEGQ = _SEG.replace("=", "%3D")
_tw("get_schedule", {}, "GET", "/helix/schedule?" + _B + "&first=25")
_tw("create_schedule_segment", {"segment": {"start_time": "2021-07-01T18:00:00Z", "timezone": "America/New_York",
                                            "is_recurring": False, "duration": "60", "category_id": "509670",
                                            "title": "TwitchDev Monthly Update // July 1, 2021"}},
    "POST", "/helix/schedule/segment?" + _B,
    body='{"start_time":"2021-07-01T18:00:00Z","timezone":"America/New_York","is_recurring":false,"duration":"60",'
         '"category_id":"509670","title":"TwitchDev Monthly Update // July 1, 2021"}')
_tw("update_schedule_segment", {"id": _SEG, "changes": {"duration": "120"}}, "PATCH",
    "/helix/schedule/segment?" + _B + "&id=" + _SEGQ, body='{"duration":"120"}')
_tw("cancel_schedule_segment", {"id": _SEG}, "PATCH", "/helix/schedule/segment?" + _B + "&id=" + _SEGQ,
    body='{"is_canceled":true}')
_tw("delete_schedule_segment", {"id": _SEG}, "DELETE", "/helix/schedule/segment?" + _B + "&id=" + _SEGQ)
_tw("set_vacation", {"start_time": "2021-05-16T00:00:00Z", "end_time": "2021-05-23T00:00:00Z",
                     "timezone": "America/New_York"}, "PATCH",
    "/helix/schedule/settings?" + _B + "&is_vacation_enabled=true&vacation_start_time=2021-05-16T00%3A00%3A00Z"
    "&vacation_end_time=2021-05-23T00%3A00%3A00Z&timezone=America%2FNew_York",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_tw("end_vacation", {}, "PATCH", "/helix/schedule/settings?" + _B + "&is_vacation_enabled=false")

# Clips and videos.
_tw("get_clips", {}, "GET", "/helix/clips?" + _B + "&first=20")
_tw("get_clips_between", {"started_at": "2026-10-01T00:00:00Z", "ended_at": "2026-10-08T00:00:00Z", "first": 5},
    "GET", "/helix/clips?" + _B + "&started_at=2026-10-01T00%3A00%3A00Z&ended_at=2026-10-08T00%3A00%3A00Z&first=5")
_tw("get_clip", {"id": "AwkwardHelplessSalamanderSwiftRage"}, "GET", "/helix/clips?id=AwkwardHelplessSalamanderSwiftRage")
_tw("get_videos", {"type": "archive"}, "GET", "/helix/videos?user_id=141981764&type=archive&sort=time&first=20")
_tw("get_video", {"id": "335921245"}, "GET", "/helix/videos?id=335921245")
_tw("delete_video", {"id": "1234"}, "DELETE", "/helix/videos?id=1234",
    http_reply={"status": 200, "body": json.dumps({"data": ["1234"]})},
    expect_result={"ok": {"kind": "value", "value": ["1234"]}})

# Telemetry: polls.
_POLL_STATE = {"id": _POLL["id"], "title": "Heads or Tails?", "status": "ACTIVE", "duration": 1800,
               "started_at": "2021-03-19T06:08:33.871278372Z", "channel_points_voting": True,
               "channel_points_per_vote": 100,
               "choices": {"0": {"id": "4c123012-1351-4f33-84b7-43856e7a0f47", "title": "Heads", "votes": 0,
                                 "channel_points_votes": 0},
                           "1": {"id": "279087e3-54a7-467e-bcd0-c1393fcea4f0", "title": "Tails", "votes": 0,
                                 "channel_points_votes": 0}}}
# A new poll replaces the last one whole, its choices too.
telemetry(TW, "poll-created", settings=_TWS, inbound_http={"path": "/helix/polls", "body": json.dumps({"data": [_POLL]})},
          state_before={"poll": {"id": "old", "ended_at": "2021-03-18T06:00:00Z",
                                 "choices": {"2": {"id": "x", "title": "Edge", "votes": 4}}}},
          expect_state={"poll": _POLL_STATE})
telemetry(TW, "poll-ended", settings=_TWS, inbound_http={"path": "/helix/polls?" + _B + "&first=1", "body": json.dumps(
    {"data": [{**_POLL, "status": "TERMINATED", "ended_at": "2021-03-19T06:11:26.746889614Z"}], "pagination": {}})},
    expect_state={"poll": {**_POLL_STATE, "status": "TERMINATED", "ended_at": "2021-03-19T06:11:26.746889614Z"}})
telemetry(TW, "poll-none", settings=_TWS, inbound_http={"path": "/helix/polls?" + _B + "&first=1",
                                                         "body": json.dumps({"data": [], "pagination": {}})},
          state_before={"poll": {"id": "kept"}}, expect_state={"poll": {"id": "kept"}})
_TWP = {"id": "1243456", **_TWU, "title": "Aren't shoes just really hard socks?",
        "choices": [{"id": "123", "title": "Yeah!"}, {"id": "124", "title": "No!"}],
        "bits_voting": {"is_enabled": False, "amount_per_vote": 0},
        "channel_points_voting": {"is_enabled": True, "amount_per_vote": 10},
        "started_at": "2020-07-15T17:16:03.17106713Z", "ends_at": "2020-07-15T17:16:08.17106713Z"}
telemetry(TW, "eventsub-poll-begin", settings=_TWS, inbound_ws=_tw_note("channel.poll.begin", "1", _TWP),
          state_before={"poll": {"id": "old", "status": "COMPLETED", "ended_at": "2020-07-14T00:00:00Z"}},
          expect_state={"poll": {"id": "1243456", "title": "Aren't shoes just really hard socks?", "status": "ACTIVE",
                                 "started_at": "2020-07-15T17:16:03.17106713Z",
                                 "ends_at": "2020-07-15T17:16:08.17106713Z", "channel_points_voting": True,
                                 "channel_points_per_vote": 10,
                                 "choices": {"0": {"id": "123", "title": "Yeah!"}, "1": {"id": "124", "title": "No!"}}}})
_TWP_VOTES = [{"id": "123", "title": "Yeah!", "bits_votes": 0, "channel_points_votes": 7, "votes": 12},
              {"id": "124", "title": "No!", "bits_votes": 0, "channel_points_votes": 4, "votes": 14}]
telemetry(TW, "eventsub-poll-progress", settings=_TWS,
          inbound_ws=_tw_note("channel.poll.progress", "1", {**_TWP, "choices": _TWP_VOTES}),
          state_before={"poll": {"id": "1243456", "status": "ACTIVE"}},
          expect_state={"poll": {"id": "1243456", "status": "ACTIVE", "choices": {
              "0": {"id": "123", "title": "Yeah!", "votes": 12, "channel_points_votes": 7},
              "1": {"id": "124", "title": "No!", "votes": 14, "channel_points_votes": 4}}}})
_TWP_END = {k: v for k, v in _TWP.items() if k != "ends_at"}
telemetry(TW, "eventsub-poll-end", settings=_TWS,
          inbound_ws=_tw_note("channel.poll.end", "1", {**_TWP_END, "choices": _TWP_VOTES, "status": "completed",
                                                        "ended_at": "2020-07-15T17:16:11.17106713Z"}),
          state_before={"poll": {"id": "1243456", "status": "ACTIVE"}},
          expect_state={"poll": {"id": "1243456", "status": "COMPLETED", "ended_at": "2020-07-15T17:16:11.17106713Z",
                                 "choices": {
                                     "0": {"id": "123", "title": "Yeah!", "votes": 12, "channel_points_votes": 7},
                                     "1": {"id": "124", "title": "No!", "votes": 14, "channel_points_votes": 4}}}})

# Telemetry: predictions.
_PREDICTION = {"id": _PRED, "broadcaster_id": "141981764", "broadcaster_name": "TwitchDev",
               "broadcaster_login": "twitchdev", "title": "Will there be any leaks today?",
               "winning_outcome_id": None,
               "outcomes": [{"id": "73085848-a94d-4040-9d21-2cb7a89374b7", "title": "Yes", "users": 0,
                             "channel_points": 0, "top_predictors": None, "color": "BLUE"},
                            {"id": "906b70ba-1f12-47ea-9e95-e5f93d20e9cc", "title": "No", "users": 0,
                             "channel_points": 0, "top_predictors": None, "color": "PINK"}],
               "prediction_window": 600, "status": "ACTIVE", "created_at": "2021-04-28T16:03:06.320848689Z",
               "ended_at": None, "locked_at": None}
telemetry(TW, "prediction", settings=_TWS,
          inbound_http={"path": "/helix/predictions?" + _B + "&first=1",
                        "body": json.dumps({"data": [_PREDICTION], "pagination": {}})},
          state_before={"prediction": {"id": "old", "status": "RESOLVED", "winning_outcome_id": "w",
                                       "ended_at": "2021-04-27T00:00:00Z"}},
          expect_state={"prediction": {
              "id": _PRED, "title": "Will there be any leaks today?", "status": "ACTIVE", "prediction_window": 600,
              "created_at": "2021-04-28T16:03:06.320848689Z",
              "outcomes": {"0": {"id": "73085848-a94d-4040-9d21-2cb7a89374b7", "title": "Yes", "color": "BLUE",
                                 "users": 0, "channel_points": 0},
                           "1": {"id": "906b70ba-1f12-47ea-9e95-e5f93d20e9cc", "title": "No", "color": "PINK",
                                 "users": 0, "channel_points": 0}}}})
_TWQ = {"id": "1243456", **_TWU, "title": "Aren't shoes just really hard socks?",
        "outcomes": [{"id": "1243456", "title": "Yeah!", "color": "blue"},
                     {"id": "2243456", "title": "No!", "color": "pink"}],
        "started_at": "2020-07-15T17:16:03.17106713Z", "locks_at": "2020-07-15T17:21:03.17106713Z"}
telemetry(TW, "eventsub-prediction-begin", settings=_TWS, inbound_ws=_tw_note("channel.prediction.begin", "1", _TWQ),
          state_before={"prediction": {"id": "old", "status": "CANCELED"}},
          expect_state={"prediction": {"id": "1243456", "title": "Aren't shoes just really hard socks?",
                                       "status": "ACTIVE", "created_at": "2020-07-15T17:16:03.17106713Z",
                                       "locks_at": "2020-07-15T17:21:03.17106713Z",
                                       "outcomes": {"0": {"id": "1243456", "title": "Yeah!", "color": "BLUE"},
                                                    "1": {"id": "2243456", "title": "No!", "color": "PINK"}}}})
_TWQ_PROG = [{"id": "1243456", "title": "Yeah!", "color": "blue", "users": 10, "channel_points": 15000,
              "top_predictors": [{"user_name": "Cool_User", "user_login": "cool_user", "user_id": "1234",
                                  "channel_points_won": None, "channel_points_used": 500}]},
             {"id": "2243456", "title": "No!", "color": "pink", "users": 3, "channel_points": 5000,
              "top_predictors": []}]
_TWQ_PROG_STATE = {"0": {"id": "1243456", "title": "Yeah!", "color": "BLUE", "users": 10, "channel_points": 15000},
                   "1": {"id": "2243456", "title": "No!", "color": "PINK", "users": 3, "channel_points": 5000}}
telemetry(TW, "eventsub-prediction-progress", settings=_TWS,
          inbound_ws=_tw_note("channel.prediction.progress", "1", {**_TWQ, "outcomes": _TWQ_PROG}),
          state_before={"prediction": {"id": "1243456", "status": "ACTIVE"}},
          expect_state={"prediction": {"id": "1243456", "status": "ACTIVE", "outcomes": _TWQ_PROG_STATE}})
_TWQ_LOCK = {k: v for k, v in _TWQ.items() if k != "locks_at"}
telemetry(TW, "eventsub-prediction-lock", settings=_TWS,
          inbound_ws=_tw_note("channel.prediction.lock", "1", {**_TWQ_LOCK, "outcomes": _TWQ_PROG,
                                                              "locked_at": "2020-07-15T17:21:03.17106713Z"}),
          state_before={"prediction": {"id": "1243456", "status": "ACTIVE"}},
          expect_state={"prediction": {"id": "1243456", "status": "LOCKED",
                                       "locked_at": "2020-07-15T17:21:03.17106713Z", "outcomes": _TWQ_PROG_STATE}})
telemetry(TW, "eventsub-prediction-end", settings=_TWS,
          inbound_ws=_tw_note("channel.prediction.end", "1", {**_TWQ_LOCK, "outcomes": _TWQ_PROG,
                                                             "winning_outcome_id": "1243456", "status": "resolved",
                                                             "ended_at": "2020-07-15T17:16:11.17106713Z"}),
          state_before={"prediction": {"id": "1243456", "status": "LOCKED"}},
          expect_state={"prediction": {"id": "1243456", "status": "RESOLVED", "winning_outcome_id": "1243456",
                                       "ended_at": "2020-07-15T17:16:11.17106713Z", "outcomes": _TWQ_PROG_STATE}})

# Telemetry: Shield Mode.
telemetry(TW, "shield-mode", settings=_TWS, inbound_http={
    "path": "/helix/moderation/shield_mode?" + _BM,
    "body": json.dumps({"data": [{"is_active": True, "moderator_id": "98765", "moderator_name": "SimplySimple",
                                  "moderator_login": "simplysimple", "last_activated_at": "2022-07-26T17:16:03.123Z"}]})},
    expect_state={"moderation": {"shield_mode": True, "shield_mode_last_activated_at": "2022-07-26T17:16:03.123Z"}})
telemetry(TW, "eventsub-shield-begin", settings=_TWS, inbound_ws=_tw_note("channel.shield_mode.begin", "1", {
    **_TWU, "moderator_user_id": "98765", "moderator_user_name": "ParticularlyParticular123",
    "moderator_user_login": "particularlyparticular123", "started_at": "2022-07-26T17:00:03.17106713Z"}),
    expect_state={"moderation": {"shield_mode": True,
                                 "shield_mode_last_activated_at": "2022-07-26T17:00:03.17106713Z"}})
telemetry(TW, "eventsub-shield-end", settings=_TWS, inbound_ws=_tw_note("channel.shield_mode.end", "1", {
    **_TWU, "moderator_user_id": "98765", "moderator_user_name": "ParticularlyParticular123",
    "moderator_user_login": "particularlyparticular123", "ended_at": "2022-07-27T01:30:23.17106713Z"}),
    state_before={"moderation": {"shield_mode": True}}, expect_state={"moderation": {"shield_mode": False}})

# Telemetry: moderators and VIPs, replaced whole; changes read them again.
_MODS_GET = {"method": "GET", "target": "/helix/moderation/moderators?" + _B + "&first=100"}
_VIPS_GET = {"method": "GET", "target": "/helix/channels/vips?" + _B + "&first=100"}
telemetry(TW, "moderators", settings=_TWS, inbound_http={
    "path": "/helix/moderation/moderators?" + _B + "&first=100",
    "body": json.dumps({"data": [{"user_id": "424596340", "user_login": "quotrok", "user_name": "quotrok"}],
                        "pagination": {}})},
    state_before={"moderation": {"moderators": {"1": {"login": "gone", "name": "Gone"}}}},
    expect_state={"moderation": {"moderators": {"424596340": {"login": "quotrok", "name": "quotrok"}}}})
telemetry(TW, "moderator-added", settings=_TWS, inbound_http={
    "path": "/helix/moderation/moderators?" + _B + "&user_id=44444", "body": ""},
    expect_state={}, expect_then_send=[_MODS_GET])
telemetry(TW, "eventsub-moderator-remove", settings=_TWS, inbound_ws=_tw_note("channel.moderator.remove", "1", {
    **_TWU, "user_id": "1234", "user_login": "not_mod_user", "user_name": "Not_Mod_User"}),
    expect_state={}, expect_then_send=[_MODS_GET])
telemetry(TW, "vips", settings=_TWS, inbound_http={
    "path": "/helix/channels/vips?" + _B + "&first=100",
    "body": json.dumps({"data": [{"user_id": "11111", "user_name": "UserDisplayName", "user_login": "userloginname"}],
                        "pagination": {}})},
    expect_state={"moderation": {"vips": {"11111": {"login": "userloginname", "name": "UserDisplayName"}}}})
telemetry(TW, "vip-removed", settings=_TWS, inbound_http={
    "path": "/helix/channels/vips?" + _B + "&user_id=456", "body": ""},
    expect_state={}, expect_then_send=[_VIPS_GET])
telemetry(TW, "eventsub-vip-add", settings=_TWS, inbound_ws=_tw_note("channel.vip.add", "1", {
    **_TWU, "user_id": "1234", "user_login": "mod_user", "user_name": "Mod_User"}),
    expect_state={}, expect_then_send=[_VIPS_GET])

# Telemetry: bans and blocked terms.
telemetry(TW, "banned", settings=_TWS, inbound_http={
    "path": "/helix/moderation/banned?" + _B + "&first=100",
    "body": json.dumps({"data": [{"user_id": "423374343", "user_login": "glowillig", "user_name": "glowillig",
                                  "expires_at": "2022-03-15T02:00:28Z", "created_at": "2022-03-15T01:30:28Z",
                                  "reason": "Does not like pineapple on pizza.", "moderator_id": "141981764",
                                  "moderator_login": "twitchdev", "moderator_name": "TwitchDev"}],
                        "pagination": {}})},
    state_before={"moderation": {"banned": {"9": {"login": "unbanned"}}}},
    expect_state={"moderation": {"banned": {"423374343": {
        "login": "glowillig", "name": "glowillig", "expires_at": "2022-03-15T02:00:28Z",
        "reason": "Does not like pineapple on pizza.", "moderator_name": "TwitchDev"}}}})
telemetry(TW, "ban-made", settings=_TWS, inbound_http={
    "path": "/helix/moderation/bans?" + _BM,
    "body": json.dumps({"data": [{"broadcaster_id": "141981764", "moderator_id": "141981764", "user_id": "9876",
                                  "created_at": "2021-09-28T19:27:31Z", "end_time": "2021-09-28T19:32:31Z"}]})},
    expect_state={}, expect_then_send=[{"method": "GET", "target": "/helix/moderation/banned?" + _B + "&first=100"}])
_TERM = {"broadcaster_id": "141981764", "moderator_id": "141981764", "id": "520e4d4e-0cda-49c7-821e-e5ef4f88c2f2",
         "text": "A phrase I'm not fond of", "created_at": "2021-09-29T19:45:37Z",
         "updated_at": "2021-09-29T19:45:37Z", "expires_at": None}
telemetry(TW, "blocked-terms", settings=_TWS, inbound_http={
    "path": "/helix/moderation/blocked_terms?" + _BM + "&first=100",
    "body": json.dumps({"data": [_TERM], "pagination": {}})},
    state_before={"moderation": {"blocked_terms": {"old": {"text": "removed elsewhere"}}}},
    expect_state={"moderation": {"blocked_terms": {_TERM["id"]: {"text": "A phrase I'm not fond of"}}}})
telemetry(TW, "blocked-term-added", settings=_TWS, inbound_http={
    "path": "/helix/moderation/blocked_terms?" + _BM, "body": json.dumps({"data": [_TERM]})},
    state_before={"moderation": {"blocked_terms": {"other": {"text": "kept"}}}},
    expect_state={"moderation": {"blocked_terms": {"other": {"text": "kept"},
                                                   _TERM["id"]: {"text": "A phrase I'm not fond of"}}}})
telemetry(TW, "blocked-term-removed", settings=_TWS, inbound_http={
    "path": "/helix/moderation/blocked_terms?" + _BM + "&id=" + _TERM["id"], "body": ""},
    expect_state={},
    expect_then_send=[{"method": "GET", "target": "/helix/moderation/blocked_terms?" + _BM + "&first=100"}])

# Telemetry: rewards and redemptions.
_RW = {"broadcaster_name": "torpedo09", "broadcaster_login": "torpedo09", "broadcaster_id": "274637212",
       "id": _REWARD, "image": None, "background_color": "#00E5CB", "is_enabled": True, "cost": 50000,
       "title": "game analysis", "prompt": "", "is_user_input_required": False,
       "max_per_stream_setting": {"is_enabled": False, "max_per_stream": 0},
       "max_per_user_per_stream_setting": {"is_enabled": False, "max_per_user_per_stream": 0},
       "global_cooldown_setting": {"is_enabled": False, "global_cooldown_seconds": 0}, "is_paused": False,
       "is_in_stock": True, "default_image": {"url_1x": "https://static-cdn.jtvnw.net/custom-reward-images/default-1.png"},
       "should_redemptions_skip_request_queue": False, "redemptions_redeemed_current_stream": None,
       "cooldown_expires_at": None}
_RW_STATE = {"title": "game analysis", "cost": 50000, "prompt": "", "enabled": True, "paused": False,
             "in_stock": True, "background_color": "#00E5CB", "user_input_required": False, "skip_queue": False,
             "max_per_stream_enabled": False, "max_per_stream": 0, "max_per_user_per_stream_enabled": False,
             "max_per_user_per_stream": 0, "global_cooldown_enabled": False, "global_cooldown_seconds": 0}
telemetry(TW, "rewards", settings=_TWS, inbound_http={
    "path": "/helix/channel_points/custom_rewards?" + _B + "&only_manageable_rewards=false",
    "body": json.dumps({"data": [_RW]})},
    state_before={"channel_points": {"rewards": {"deleted": {"title": "gone"}}}},
    expect_state={"channel_points": {"rewards": {_REWARD: _RW_STATE}}})
telemetry(TW, "reward-updated", settings=_TWS, inbound_http={
    "path": "/helix/channel_points/custom_rewards?" + _B + "&id=" + _REWARD,
    "body": json.dumps({"data": [{**_RW, "is_paused": True}]})},
    state_before={"channel_points": {"rewards": {"other": {"title": "kept"}}}},
    expect_state={"channel_points": {"rewards": {"other": {"title": "kept"}, _REWARD: {**_RW_STATE, "paused": True}}}},
    expect_then_send=[{"method": "GET",
                       "target": "/helix/channel_points/custom_rewards?" + _B + "&only_manageable_rewards=false"}])
_RD = {"broadcaster_name": "torpedo09", "broadcaster_login": "torpedo09", "broadcaster_id": "274637212",
       "id": "17fa2df1-ad76-4804-bfa5-a40ef63efe63", "user_id": "274637212", "user_name": "torpedo09",
       "user_login": "torpedo09", "user_input": "", "status": "UNFULFILLED",
       "redeemed_at": "2020-07-01T18:37:32Z",
       "reward": {"id": _REWARD, "title": "game analysis", "prompt": "", "cost": 50000}}
_RD_STATE = {"reward_id": _REWARD, "reward_title": "game analysis", "user_id": "274637212",
             "user_name": "torpedo09", "user_input": "", "redeemed_at": "2020-07-01T18:37:32Z"}
telemetry(TW, "redemptions", settings=_TWS, inbound_http={
    "path": "/helix/channel_points/custom_rewards/redemptions?" + _B + "&reward_id=" + _REWARD
            + "&status=UNFULFILLED&first=50",
    "body": json.dumps({"data": [_RD], "pagination": {}})},
    expect_state={"channel_points": {"redemptions": {_RD["id"]: _RD_STATE}}})
telemetry(TW, "redemption-fulfilled", settings=_TWS, inbound_http={
    "path": "/helix/channel_points/custom_rewards/redemptions?id=" + _RD["id"] + "&" + _B + "&reward_id=" + _REWARD,
    "body": json.dumps({"data": [{**_RD, "status": "CANCELED"}]})},
    state_before={"channel_points": {"redemptions": {_RD["id"]: _RD_STATE, "other": {"user_name": "kept"}}}},
    expect_state={"channel_points": {"redemptions": {"other": {"user_name": "kept"}}}})
_RD_EVENT = {"id": "17fa2df1-ad76-4804-bfa5-a40ef63efe63", **_TWU, "user_id": "9001", "user_login": "cooler_user",
             "user_name": "Cooler_User", "user_input": "pogchamp", "status": "unfulfilled",
             "reward": {"id": _REWARD, "title": "title", "cost": 100, "prompt": "reward prompt"},
             "redeemed_at": "2020-07-15T17:16:03.17106713Z"}
telemetry(TW, "eventsub-redemption-add", settings=_TWS,
          inbound_ws=_tw_note("channel.channel_points_custom_reward_redemption.add", "1", _RD_EVENT),
          expect_state={"channel_points": {"redemptions": {_RD_EVENT["id"]: {
              "reward_id": _REWARD, "reward_title": "title", "user_id": "9001", "user_name": "Cooler_User",
              "user_input": "pogchamp", "redeemed_at": "2020-07-15T17:16:03.17106713Z"}}}})
telemetry(TW, "eventsub-redemption-update", settings=_TWS,
          inbound_ws=_tw_note("channel.channel_points_custom_reward_redemption.update", "1",
                              {**_RD_EVENT, "status": "fulfilled"}),
          state_before={"channel_points": {"redemptions": {_RD_EVENT["id"]: {"user_name": "Cooler_User"},
                                                           "other": {"user_name": "kept"}}}},
          expect_state={"channel_points": {"redemptions": {"other": {"user_name": "kept"}}}})

# Telemetry: the schedule poll, and a change read again.
_SCHED = {"segments": [{"id": _SEG, "start_time": "2021-07-01T18:00:00Z", "end_time": "2021-07-01T19:00:00Z",
                        "title": "TwitchDev Monthly Update // July 1, 2021", "canceled_until": None,
                        "category": {"id": "509670", "name": "Science & Technology"}, "is_recurring": False}],
          "broadcaster_id": "141981764", "broadcaster_name": "TwitchDev", "broadcaster_login": "twitchdev",
          "vacation": None}
telemetry(TW, "schedule", settings=_TWS, inbound_http={
    "path": "/helix/schedule?" + _B + "&first=25", "body": json.dumps({"data": _SCHED, "pagination": {}})},
    state_before={"schedule": {"vacation": True, "vacation_start_time": "x", "segments": {"gone": {"title": "x"}}}},
    expect_state={"schedule": {"vacation": False, "vacation_start_time": "", "vacation_end_time": "", "segments": {
        _SEG: {"start_time": "2021-07-01T18:00:00Z", "end_time": "2021-07-01T19:00:00Z",
               "title": "TwitchDev Monthly Update // July 1, 2021", "category_id": "509670",
               "category_name": "Science & Technology", "recurring": False}}}})
telemetry(TW, "schedule-vacation", settings=_TWS, inbound_http={
    "path": "/helix/schedule?" + _B + "&first=25",
    "body": json.dumps({"data": {**_SCHED, "segments": [], "vacation": {"start_time": "2021-05-16T00:00:00Z",
                                                                       "end_time": "2021-05-23T00:00:00Z"}},
                        "pagination": {}})},
    state_before={"schedule": {"segments": {"gone": {"title": "x"}}}},
    expect_state={"schedule": {"vacation": True, "vacation_start_time": "2021-05-16T00:00:00Z",
                               "vacation_end_time": "2021-05-23T00:00:00Z"}})
telemetry(TW, "schedule-changed", settings=_TWS, inbound_http={
    "path": "/helix/schedule/segment?" + _B + "&id=" + _SEGQ, "body": ""},
    expect_state={}, expect_then_send=[{"method": "GET", "target": "/helix/schedule?" + _B + "&first=25"}])
