# YouTube Live (youtube-live): one vector per command, and telemetry vectors
# for the polled resources. Targets are the YouTube Data API v3 methods as
# Google's reference gives them (/youtube/v3/liveBroadcasts, /transition,
# /bind, /cuepoint, /liveStreams, /videos, /liveChat/messages,
# /liveChat/bans), with query values percent-encoded by hand (a comma is
# %2C). Reply bodies follow the reference's resource representations; ids are
# made up.
YT = "youtube-live"
_YTS = {"access_token": "ya29.x", "broadcast_id": "abcDEF12345"}
_YB = "/youtube/v3/liveBroadcasts"
_PART = "part=id%2Csnippet%2CcontentDetails%2Cstatus"


def _yt(command, input, method, target, body=None, body_hex=None, content_type=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    if body_hex is not None:
        request["body_hex"] = body_hex
    if content_type is not None:
        request["content_type"] = content_type
    V.append({"spec": YT, "command": command, "input": input, "settings": _YTS,
              "expect_request": request, **extra})


_BROADCAST = {
    "kind": "youtube#liveBroadcast", "etag": "e1", "id": "abcDEF12345",
    "snippet": {"publishedAt": "2026-10-01T10:00:00Z", "channelId": "UCchannel", "title": "Sunday Service",
                "description": "", "scheduledStartTime": "2026-10-04T16:00:00Z",
                "actualStartTime": "2026-10-04T16:00:41Z", "liveChatId": "Cg0KC2FiY0RFRjEyMzQ1",
                "isDefaultBroadcast": False},
    "status": {"lifeCycleStatus": "live", "privacyStatus": "public", "recordingStatus": "recording",
               "madeForKids": False, "selfDeclaredMadeForKids": False},
    "contentDetails": {"boundStreamId": "UCchannel1700000000000", "boundStreamLastUpdateTimeMs": "2026-10-01T10:00:00Z",
                       "monitorStream": {"enableMonitorStream": True, "broadcastStreamDelayMs": 0},
                       "enableEmbed": True, "enableDvr": True, "recordFromStart": True,
                       "enableClosedCaptions": False, "projection": "rectangular", "enableLowLatency": False,
                       "latencyPreference": "normal", "enableAutoStart": False, "enableAutoStop": True}}
_BROADCAST_STATE = {
    "title": "Sunday Service", "life_cycle_status": "live", "privacy_status": "public",
    "recording_status": "recording", "bound_stream_id": "UCchannel1700000000000",
    "scheduled_start_time": "2026-10-04T16:00:00Z", "actual_start_time": "2026-10-04T16:00:41Z",
    "live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1", "monitor_stream": True, "stream_delay_ms": 0,
    "auto_start": False, "auto_stop": True, "dvr": True, "latency_preference": "normal"}

# Broadcasts.
_yt("list_broadcasts", {}, "GET",
    _YB + "?" + _PART + "&broadcastStatus=active&broadcastType=all&maxResults=25",
    http_reply={"status": 200, "body": json.dumps({"kind": "youtube#liveBroadcastListResponse",
                                                   "items": [_BROADCAST]})},
    expect_result={"ok": {"kind": "value", "value": [_BROADCAST]}})
_yt("get_broadcast", {"broadcast_id": "abcDEF12345"}, "GET", _YB + "?" + _PART + "&id=abcDEF12345")
_yt("start_testing", {"broadcast_id": "abcDEF12345"}, "POST",
    _YB + "/transition?broadcastStatus=testing&id=abcDEF12345&" + _PART,
    # The stream is not receiving data: an ordinary failure, not a refusal.
    http_reply={"status": 403, "body": json.dumps({"error": {"code": 403, "message": "Stream is inactive",
                                                             "errors": [{"reason": "errorStreamInactive"}]}})},
    expect_result={"error": {"error": "device_error", "code": "403"}})
_yt("go_live", {"broadcast_id": "abcDEF12345"}, "POST",
    _YB + "/transition?broadcastStatus=live&id=abcDEF12345&" + _PART,
    http_reply={"status": 200, "body": json.dumps(_BROADCAST)},
    expect_result={"ok": {"kind": "value", "value": _BROADCAST}})
# A plain access token (no refresh token) refused: 401 is terminal, reported as auth.
_yt("complete_broadcast", {"broadcast_id": "abcDEF12345"}, "POST",
    _YB + "/transition?broadcastStatus=complete&id=abcDEF12345&" + _PART,
    http_reply={"status": 401, "body": '{"error":{"code":401,"message":"Invalid Credentials"}}'},
    expect_result={"error": {"error": "auth"}})
_yt("bind_stream", {"broadcast_id": "abcDEF12345", "stream_id": "UCchannel1700000000000"}, "POST",
    _YB + "/bind?id=abcDEF12345&" + _PART + "&streamId=UCchannel1700000000000")
_yt("unbind_stream", {"broadcast_id": "abcDEF12345"}, "POST", _YB + "/bind?id=abcDEF12345&" + _PART)
_yt("insert_ad_break", {"broadcast_id": "abcDEF12345", "duration_secs": 60}, "POST",
    _YB + "/cuepoint?id=abcDEF12345", body='{"cueType":"cueTypeAd","durationSecs":60}')
_NEW = {"snippet": {"title": "Evening Service", "scheduledStartTime": "2026-10-04T23:00:00Z"},
        "status": {"privacyStatus": "unlisted"}}
_yt("insert_broadcast", {"broadcast": _NEW}, "POST", _YB + "?" + _PART,
    body=json.dumps(_NEW, separators=(",", ":")))
_UPD = {"id": "abcDEF12345", "snippet": {"title": "Sunday Service", "scheduledStartTime": "2026-10-04T16:00:00Z"},
        "contentDetails": {"monitorStream": {"enableMonitorStream": True, "broadcastStreamDelayMs": 0}},
        "status": {"privacyStatus": "public"}}
_yt("update_broadcast", {"broadcast": _UPD}, "PUT", _YB + "?" + _PART,
    body=json.dumps(_UPD, separators=(",", ":")))
_yt("delete_broadcast", {"broadcast_id": "abcDEF12345"}, "DELETE", _YB + "?id=abcDEF12345",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})

# Streams.
_SPART = "part=id%2Csnippet%2Ccdn%2Cstatus"
_yt("list_streams", {}, "GET", "/youtube/v3/liveStreams?" + _SPART + "&mine=true&maxResults=25")
_yt("get_stream", {"stream_id": "UCchannel1700000000000"}, "GET",
    "/youtube/v3/liveStreams?" + _SPART + "&id=UCchannel1700000000000")
_STREAM = {"snippet": {"title": "Main encoder"},
           "cdn": {"ingestionType": "rtmp", "resolution": "1080p", "frameRate": "30fps"}}
_yt("insert_stream", {"stream": _STREAM}, "POST", "/youtube/v3/liveStreams?" + _SPART,
    body=json.dumps(_STREAM, separators=(",", ":")))
_yt("update_stream", {"stream": {"id": "UCchannel1700000000000", **_STREAM}}, "PUT",
    "/youtube/v3/liveStreams?" + _SPART,
    body=json.dumps({"id": "UCchannel1700000000000", **_STREAM}, separators=(",", ":")))
_yt("delete_stream", {"stream_id": "UCchannel1700000000000"}, "DELETE",
    "/youtube/v3/liveStreams?id=UCchannel1700000000000")

# Viewers.
_yt("get_live_details", {"video_id": "abcDEF12345"}, "GET",
    "/youtube/v3/videos?part=liveStreamingDetails%2Cstatistics&id=abcDEF12345")

# Live chat.
_yt("list_chat_messages", {"live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1"}, "GET",
    "/youtube/v3/liveChat/messages?liveChatId=Cg0KC2FiY0RFRjEyMzQ1&part=id%2Csnippet%2CauthorDetails")
_yt("list_chat_messages_after", {"live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1", "page_token": "GO7GxdXV3_8CIJ_p"}, "GET",
    "/youtube/v3/liveChat/messages?liveChatId=Cg0KC2FiY0RFRjEyMzQ1&part=id%2Csnippet%2CauthorDetails"
    "&pageToken=GO7GxdXV3_8CIJ_p")
_yt("send_chat_message", {"live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1", "text": 'Welcome, "everyone"!'}, "POST",
    "/youtube/v3/liveChat/messages?part=snippet",
    body='{"snippet":{"liveChatId":"Cg0KC2FiY0RFRjEyMzQ1","type":"textMessageEvent",'
         '"textMessageDetails":{"messageText":"Welcome, \\"everyone\\"!"}}}')
_yt("delete_chat_message", {"message_id": "LCC.abc123"}, "DELETE", "/youtube/v3/liveChat/messages?id=LCC.abc123",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_yt("ban_chat_user", {"live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1", "channel_id": "UCviewer"}, "POST",
    "/youtube/v3/liveChat/bans?part=snippet",
    body='{"snippet":{"liveChatId":"Cg0KC2FiY0RFRjEyMzQ1","type":"permanent",'
         '"bannedUserDetails":{"channelId":"UCviewer"}}}')
_yt("ban_chat_user_temporarily", {"live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1", "channel_id": "UCviewer"}, "POST",
    "/youtube/v3/liveChat/bans?part=snippet",
    body='{"snippet":{"liveChatId":"Cg0KC2FiY0RFRjEyMzQ1","type":"temporary",'
         '"bannedUserDetails":{"channelId":"UCviewer"},"banDurationSeconds":300}}')
_yt("unban_chat_user", {"ban_id": "ban.abc123"}, "DELETE", "/youtube/v3/liveChat/bans?id=ban.abc123")

# Telemetry.
telemetry(YT, "broadcasts", settings=_YTS, inbound_http={
    "path": _YB + "?" + _PART + "&id=abcDEF12345",
    "body": json.dumps({"kind": "youtube#liveBroadcastListResponse", "items": [_BROADCAST]})},
    expect_state={"broadcasts": {"abcDEF12345": _BROADCAST_STATE}})
telemetry(YT, "transition", settings=_YTS, inbound_http={
    "path": _YB + "/transition?broadcastStatus=live&id=abcDEF12345&" + _PART, "body": json.dumps(_BROADCAST)},
    expect_state={"broadcasts": {"abcDEF12345": _BROADCAST_STATE}})
_ISSUES = [{"type": "bitrateLow", "severity": "warning", "reason": "Check video settings",
            "description": "The stream's current bitrate is lower than the recommended bitrate."}]
telemetry(YT, "streams", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveStreams?" + _SPART + "&mine=true&maxResults=50",
    "body": json.dumps({"kind": "youtube#liveStreamListResponse",
                        "pageInfo": {"totalResults": 1, "resultsPerPage": 50}, "items": [{
        "kind": "youtube#liveStream", "id": "UCchannel1700000000000",
        "snippet": {"title": "Main encoder", "isDefaultStream": False},
        "cdn": {"ingestionType": "rtmp", "resolution": "1080p", "frameRate": "30fps",
                "ingestionInfo": {"streamName": "abcd-efgh-ijkl-mnop",
                                  "ingestionAddress": "rtmp://a.rtmp.youtube.com/live2"}},
        "status": {"streamStatus": "active", "healthStatus": {
            "status": "ok", "lastUpdateTimeSeconds": "1791216000", "configurationIssues": _ISSUES}}}]})},
    # The whole list (its total fits the page): a deleted stream leaves.
    state_before={"streams": {"UCdeleted": {"title": "Old encoder", "stream_status": "inactive"}}},
    expect_state={"streams": {"UCchannel1700000000000": {
        "title": "Main encoder", "stream_status": "active", "health": "ok", "health_updated": 1791216000,
        "configuration_issues": json.dumps(_ISSUES, separators=(",", ":")),
        "ingestion_type": "rtmp", "resolution": "1080p", "frame_rate": "30fps"}}})
# get_stream (a lookup by id) lists one stream: it adds, nothing leaves.
telemetry(YT, "stream-by-id", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveStreams?" + _SPART + "&id=UCchannel1700000000000",
    "body": json.dumps({"kind": "youtube#liveStreamListResponse",
                        "pageInfo": {"totalResults": 1, "resultsPerPage": 5}, "items": [{
                            "kind": "youtube#liveStream", "id": "UCchannel1700000000000",
                            "snippet": {"title": "Main encoder"}, "status": {"streamStatus": "ready"}}]})},
    state_before={"streams": {"UCother": {"title": "Backup encoder"}}},
    expect_state={"streams": {"UCother": {"title": "Backup encoder"},
                              "UCchannel1700000000000": {"title": "Main encoder", "stream_status": "ready"}}})
_VSNIP = {"publishedAt": "2026-10-01T10:00:00Z", "channelId": "UCchannel", "title": "Sunday Service",
          "description": "Live from the main hall.", "tags": ["church", "live"], "categoryId": "29",
          "liveBroadcastContent": "live", "defaultLanguage": "en"}
_VSTATUS = {"uploadStatus": "uploaded", "privacyStatus": "public", "license": "youtube", "embeddable": True,
            "publicStatsViewable": True, "madeForKids": False, "selfDeclaredMadeForKids": False,
            "containsSyntheticMedia": False}
_VIDEO_STATE = {"description": "Live from the main hall.", "category_id": "29", "tags": '["church","live"]',
                "default_language": "en", "made_for_kids": False, "self_declared_made_for_kids": False,
                "embeddable": True, "public_stats_viewable": True, "license": "youtube",
                "contains_synthetic_media": False}
_VPART = "part=snippet%2Cstatus%2CliveStreamingDetails%2Cstatistics"
telemetry(YT, "viewers", settings=_YTS, inbound_http={
    "path": "/youtube/v3/videos?" + _VPART + "&id=abcDEF12345",
    "body": json.dumps({"kind": "youtube#videoListResponse", "items": [{
        "kind": "youtube#video", "id": "abcDEF12345", "snippet": _VSNIP, "status": _VSTATUS,
        "liveStreamingDetails": {"actualStartTime": "2026-10-04T16:00:41Z", "concurrentViewers": "187",
                                 "activeLiveChatId": "Cg0KC2FiY0RFRjEyMzQ1"},
        "statistics": {"viewCount": "412", "likeCount": "36", "commentCount": "0"}}]})},
    expect_state={"broadcasts": {"abcDEF12345": {
        "concurrent_viewers": 187, "active_live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1",
        "view_count": 412, "like_count": 36, "comment_count": 0, **_VIDEO_STATE}}})

# The broadcast's video.
_yt("get_video", {"video_id": "abcDEF12345"}, "GET", "/youtube/v3/videos?" + _VPART + "&id=abcDEF12345")
_VIDEO = {"id": "abcDEF12345", "snippet": {"title": "Sunday Service", "categoryId": "29",
                                           "description": "Live from the main hall.", "tags": ["church", "live"]},
          "status": {"privacyStatus": "public", "selfDeclaredMadeForKids": False, "embeddable": True,
                     "publicStatsViewable": True, "license": "youtube"}}
_yt("update_video", {"video": _VIDEO}, "PUT", "/youtube/v3/videos?part=snippet%2Cstatus",
    body=json.dumps(_VIDEO, separators=(",", ":")),
    http_reply={"status": 200, "body": json.dumps({"kind": "youtube#video", **_VIDEO})},
    expect_result={"ok": {"kind": "value", "value": {"kind": "youtube#video", **_VIDEO}}})
_VSN = {"id": "abcDEF12345", "snippet": _VIDEO["snippet"]}
_yt("update_video_snippet", {"video": _VSN}, "PUT", "/youtube/v3/videos?part=snippet",
    body=json.dumps(_VSN, separators=(",", ":")))
_VST = {"id": "abcDEF12345", "status": _VIDEO["status"]}
_yt("update_video_status", {"video": _VST}, "PUT", "/youtube/v3/videos?part=status",
    body=json.dumps(_VST, separators=(",", ":")))
# The image's bytes are the whole body, sent as they are: the first eight
# bytes of a PNG file (its signature), given as base64.
_yt("set_thumbnail", {"video_id": "abcDEF12345", "image": "iVBORw0KGgo=", "format": "image/png"}, "POST",
    "/upload/youtube/v3/thumbnails/set?videoId=abcDEF12345&uploadType=media",
    body_hex="89504e470d0a1a0a", content_type="image/png",
    http_reply={"status": 200, "body": json.dumps({"kind": "youtube#thumbnailSetResponse", "etag": "e3",
                                                   "items": [{"default": {"url": "https://i.ytimg.com/vi/x/default.jpg",
                                                                          "width": 120, "height": 90}}]})},
    expect_result={"ok": {"kind": "value", "value": [{"default": {"url": "https://i.ytimg.com/vi/x/default.jpg",
                                                                   "width": 120, "height": 90}}]}})
_yt("list_video_categories", {"region_code": "GB"}, "GET", "/youtube/v3/videoCategories?part=snippet&regionCode=GB")
telemetry(YT, "video-updated", settings=_YTS, inbound_http={
    "path": "/youtube/v3/videos?part=snippet%2Cstatus",
    "body": json.dumps({"kind": "youtube#video", "etag": "e2", "id": "abcDEF12345",
                        "snippet": {**_VSNIP, "categoryId": "22", "tags": ["church"]},
                        "status": {**_VSTATUS, "selfDeclaredMadeForKids": True, "madeForKids": True}})},
    state_before={"broadcasts": {"abcDEF12345": {"title": "Sunday Service"}}},
    expect_state={"broadcasts": {"abcDEF12345": {"title": "Sunday Service", **_VIDEO_STATE, "category_id": "22",
                                                 "tags": '["church"]', "made_for_kids": True,
                                                 "self_declared_made_for_kids": True}}})

# Chat moderators.
_CHAT = "Cg0KC2FiY0RFRjEyMzQ1"
_MOD = {"kind": "youtube#liveChatModerator", "etag": "m1", "id": "Mod-ab12_cd",
        "snippet": {"moderatorDetails": {"channelId": "UCmoderator", "channelUrl": "http://www.youtube.com/channel/UCmoderator",
                                         "displayName": "Usher Team", "profileImageUrl": "https://yt3.ggpht.com/a"},
                    "liveChatId": _CHAT}}
_MOD_STATE = {"live_chat_id": _CHAT, "channel_id": "UCmoderator", "display_name": "Usher Team"}
_yt("list_chat_moderators", {"live_chat_id": _CHAT}, "GET",
    "/youtube/v3/liveChat/moderators?liveChatId=" + _CHAT + "&part=id%2Csnippet&maxResults=50")
_yt("add_chat_moderator", {"live_chat_id": _CHAT, "channel_id": "UCmoderator"}, "POST",
    "/youtube/v3/liveChat/moderators?part=snippet",
    body='{"snippet":{"liveChatId":"' + _CHAT + '","moderatorDetails":{"channelId":"UCmoderator"}}}',
    http_reply={"status": 200, "body": json.dumps(_MOD)},
    expect_result={"ok": {"kind": "value", "value": _MOD}})
_yt("remove_chat_moderator", {"moderator_id": "Mod-ab12_cd"}, "DELETE", "/youtube/v3/liveChat/moderators?id=Mod-ab12_cd",
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
telemetry(YT, "chat-moderators", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveChat/moderators?liveChatId=" + _CHAT + "&part=id%2Csnippet&maxResults=50",
    "body": json.dumps({"kind": "youtube#liveChatModeratorListResponse",
                        "pageInfo": {"totalResults": 1, "resultsPerPage": 50}, "items": [_MOD]})},
    state_before={"chat_moderators": {"Gone": {"channel_id": "UCgone"}}},
    expect_state={"chat_moderators": {"Mod-ab12_cd": _MOD_STATE}})
telemetry(YT, "chat-moderator-added", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveChat/moderators?part=snippet", "body": json.dumps(_MOD)},
    state_before={"chat_moderators": {"Other": {"channel_id": "UCother"}}},
    expect_state={"chat_moderators": {"Other": {"channel_id": "UCother"}, "Mod-ab12_cd": _MOD_STATE}})
telemetry(YT, "chat-moderator-removed", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveChat/moderators?id=Mod-ab12_cd", "body": ""},
    state_before={"chat_moderators": {"Other": {"channel_id": "UCother"}, "Mod-ab12_cd": _MOD_STATE}},
    expect_state={"chat_moderators": {"Other": {"channel_id": "UCother"}}})

# Live chat polls.
_POLLMSG = {"kind": "youtube#liveChatMessage", "etag": "p1", "id": "LCC.poll123",
            "snippet": {"type": "pollEvent", "liveChatId": _CHAT, "authorChannelId": "UCchannel",
                        "publishedAt": "2026-10-04T16:20:00Z", "hasDisplayContent": True,
                        "displayMessage": "Which song next?",
                        "pollDetails": {"metadata": {"questionText": "Which song next?",
                                                     "options": [{"optionText": "Amazing Grace", "tally": "12"},
                                                                 {"optionText": "Be Thou My Vision", "tally": "7"}],
                                                     "status": "closed"}}}}
_POLL_STATE = {"message_id": "LCC.poll123", "live_chat_id": _CHAT, "question": "Which song next?",
               "status": "closed", "options": {"0": {"text": "Amazing Grace", "tally": 12},
                                               "1": {"text": "Be Thou My Vision", "tally": 7}}}
_OPTS = [{"optionText": "Amazing Grace"}, {"optionText": "Be Thou My Vision"}]
_yt("create_chat_poll", {"live_chat_id": _CHAT, "question": "Which song next?", "options": _OPTS}, "POST",
    "/youtube/v3/liveChat/messages?part=snippet",
    body='{"snippet":{"liveChatId":"' + _CHAT + '","type":"pollEvent","pollDetails":{"metadata":'
         '{"questionText":"Which song next?","options":[{"optionText":"Amazing Grace"},'
         '{"optionText":"Be Thou My Vision"}]}}}}')
_yt("close_chat_poll", {"message_id": "LCC.poll123"}, "POST",
    "/youtube/v3/liveChat/messages/transition?id=LCC.poll123&status=closed&part=id%2Csnippet",
    http_reply={"status": 200, "body": json.dumps(_POLLMSG)},
    expect_result={"ok": {"kind": "value", "value": _POLLMSG}})
telemetry(YT, "chat-poll-closed", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveChat/messages/transition?id=LCC.poll123&status=closed&part=id%2Csnippet",
    "body": json.dumps(_POLLMSG)},
    state_before={"chat_poll": {"message_id": "LCC.old", "options": {"2": {"text": "Old option"}}}},
    expect_state={"chat_poll": _POLL_STATE})
telemetry(YT, "chat-poll-in-chat", settings=_YTS, inbound_http={
    "path": "/youtube/v3/liveChat/messages?liveChatId=" + _CHAT + "&part=id%2Csnippet%2CauthorDetails",
    "body": json.dumps({"kind": "youtube#liveChatMessageListResponse", "pollingIntervalMillis": 5000, "items": [
        {"kind": "youtube#liveChatMessage", "id": "LCC.text1",
         "snippet": {"type": "textMessageEvent", "liveChatId": _CHAT,
                     "textMessageDetails": {"messageText": "Hello"}}},
        {**_POLLMSG, "snippet": {**_POLLMSG["snippet"], "pollDetails": {"metadata": {
            **_POLLMSG["snippet"]["pollDetails"]["metadata"], "status": "active"}}}}]})},
    expect_state={"chat_poll": {**_POLL_STATE, "status": "active"}})

# Super Chats.
_SC = {"kind": "youtube#superChatEvent", "etag": "s1", "id": "SC-1",
       "snippet": {"channelId": "UCchannel",
                   "supporterDetails": {"channelId": "UCfan", "channelUrl": "http://www.youtube.com/channel/UCfan",
                                        "displayName": "A Fan", "profileImageUrl": "https://yt3.ggpht.com/b"},
                   "commentText": "Great sermon!", "createdAt": "2026-10-04T16:30:00Z", "amountMicros": "5000000",
                   "currency": "USD", "displayString": "$5.00", "messageType": 2, "isSuperStickerEvent": False}}
_yt("list_super_chat_events", {}, "GET", "/youtube/v3/superChatEvents?part=snippet&maxResults=50")
_yt("list_super_chat_events_page", {"page_token": "CAoQAA"}, "GET",
    "/youtube/v3/superChatEvents?part=snippet&maxResults=50&pageToken=CAoQAA")
telemetry(YT, "super-chats", settings=_YTS, inbound_http={
    "path": "/youtube/v3/superChatEvents?part=snippet&maxResults=50",
    "body": json.dumps({"kind": "youtube#superChatEventListResponse",
                        "pageInfo": {"totalResults": 1, "resultsPerPage": 50}, "items": [_SC]})},
    state_before={"super_chats": {"SC-old": {"comment": "older than the page"}}},
    expect_state={"super_chats": {"SC-1": {
        "supporter_name": "A Fan", "supporter_channel_id": "UCfan", "comment": "Great sermon!",
        "amount_micros": 5000000, "currency": "USD", "display_string": "$5.00", "tier": 2, "sticker": False,
        "created_at": "2026-10-04T16:30:00Z"}}})
