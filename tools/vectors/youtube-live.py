# YouTube Live (youtube-live): one vector per command, and telemetry vectors
# for the polled resources. Targets are the YouTube Data API v3 methods as
# Google's reference gives them (/youtube/v3/liveBroadcasts, /transition,
# /bind, /cuepoint, /liveStreams, /videos, /liveChat/messages,
# /liveChat/bans), with query values percent-encoded by hand (a comma is
# %2C). Reply bodies follow the reference's resource representations; ids are
# made up.
YT = "youtube-live"
_YTS = {"token": "ya29.x", "broadcast_id": "abcDEF12345"}
_YB = "/youtube/v3/liveBroadcasts"
_PART = "part=id%2Csnippet%2CcontentDetails%2Cstatus"


def _yt(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
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
# The access token has expired: 401 is terminal, reported as auth.
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
    "body": json.dumps({"kind": "youtube#liveStreamListResponse", "items": [{
        "kind": "youtube#liveStream", "id": "UCchannel1700000000000",
        "snippet": {"title": "Main encoder", "isDefaultStream": False},
        "cdn": {"ingestionType": "rtmp", "resolution": "1080p", "frameRate": "30fps",
                "ingestionInfo": {"streamName": "abcd-efgh-ijkl-mnop",
                                  "ingestionAddress": "rtmp://a.rtmp.youtube.com/live2"}},
        "status": {"streamStatus": "active", "healthStatus": {
            "status": "ok", "lastUpdateTimeSeconds": "1791216000", "configurationIssues": _ISSUES}}}]})},
    expect_state={"streams": {"UCchannel1700000000000": {
        "title": "Main encoder", "stream_status": "active", "health": "ok", "health_updated": 1791216000,
        "configuration_issues": json.dumps(_ISSUES, separators=(",", ":")),
        "ingestion_type": "rtmp", "resolution": "1080p", "frame_rate": "30fps"}}})
telemetry(YT, "viewers", settings=_YTS, inbound_http={
    "path": "/youtube/v3/videos?part=liveStreamingDetails%2Cstatistics&id=abcDEF12345",
    "body": json.dumps({"kind": "youtube#videoListResponse", "items": [{
        "kind": "youtube#video", "id": "abcDEF12345",
        "liveStreamingDetails": {"actualStartTime": "2026-10-04T16:00:41Z", "concurrentViewers": "187",
                                 "activeLiveChatId": "Cg0KC2FiY0RFRjEyMzQ1"},
        "statistics": {"viewCount": "412", "likeCount": "36", "commentCount": "0"}}]})},
    expect_state={"broadcasts": {"abcDEF12345": {
        "concurrent_viewers": 187, "active_live_chat_id": "Cg0KC2FiY0RFRjEyMzQ1",
        "view_count": 412, "like_count": 36, "comment_count": 0}}})
