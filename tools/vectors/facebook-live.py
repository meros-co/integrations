# Facebook Live (facebook-live): one vector per command, and telemetry
# vectors for the polled live videos. Targets are the Graph API paths as
# Meta's Live Video API guides and Graph API reference give them, on the
# default version v26.0, with query values percent-encoded by urllib (RFC
# 3986 unreserved characters kept). Reply bodies follow the guides' examples;
# ids are made up.
from urllib.parse import quote as _q

FB = "facebook-live"
_FBS = {"token": "EAAGpagetoken", "page_id": "1093482904102"}
_V = "/v26.0"
_LV = "10155832421501570"
_FIELDS = ("id,title,description,status,live_views,broadcast_start_time,seconds_left,permalink_url,"
           "ingest_streams{id,is_master,stream_health}")
_F = "fields=" + _q(_FIELDS, safe="")


def _fb(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": FB, "command": command, "input": input, "settings": _FBS,
              "expect_request": request, **extra})


_HEALTH = {"video_bitrate": 4005000, "video_framerate": 30, "video_gop_size": 2000,
           "video_height": 1080, "video_width": 1920, "audio_bitrate": 128000.5}
_INGEST = [{"id": "10155832421506570", "is_master": True, "stream_health": _HEALTH}]
_VIDEO = {"id": _LV, "title": "Sunday Service", "description": "Live from the main hall",
          "status": "LIVE", "live_views": 214, "broadcast_start_time": "2026-10-04T16:00:12+0000",
          "seconds_left": 27588, "permalink_url": "/1093482904102/videos/10155832421501570/",
          "ingest_streams": _INGEST}
_VIDEO_STATE = {"title": "Sunday Service", "description": "Live from the main hall", "status": "LIVE",
                "live_views": 214, "broadcast_start_time": "2026-10-04T16:00:12+0000", "seconds_left": 27588,
                "permalink_url": "/1093482904102/videos/10155832421501570/",
                "ingest_streams": json.dumps(_INGEST, separators=(",", ":"))}
_CREATED = {"id": _LV, "stream_url": "rtmp://live-api-s.facebook.com:80/rtmp/FB-10155832421501570-0-AbzX",
            "secure_stream_url": "rtmps://live-api-s.facebook.com:443/rtmp/FB-10155832421501570-0-AbzX"}

# Live videos.
_fb("list_live_videos", {}, "GET", _V + "/1093482904102/live_videos?" + _F + "&source=owner&limit=10",
    http_reply={"status": 200, "body": json.dumps({"data": [_VIDEO], "paging": {}})},
    expect_result={"ok": {"kind": "value", "value": [_VIDEO]}})
_fb("list_live_videos_by_status", {"status": "SCHEDULED_UNPUBLISHED", "limit": 5}, "GET",
    _V + "/1093482904102/live_videos?" + _F + "&source=owner&broadcast_status="
    + _q('["SCHEDULED_UNPUBLISHED"]', safe="") + "&limit=5")
_fb("get_live_video", {"live_video_id": _LV}, "GET", _V + "/" + _LV + "?" + _F)
_fb("get_stream_urls", {"live_video_id": _LV}, "GET",
    _V + "/" + _LV + "?fields=" + _q("stream_url,secure_stream_url,dash_ingest_url,"
                                     "ingest_streams{id,is_master,stream_url,secure_stream_url}", safe=""))
_fb("get_ingest_streams", {"live_video_id": _LV}, "GET",
    _V + "/" + _LV + "?fields=" + _q("ingest_streams{id,stream_id,is_master,stream_health}", safe=""),
    http_reply={"status": 200, "body": json.dumps({"ingest_streams": _INGEST, "id": _LV})},
    expect_result={"ok": {"kind": "value", "value": _INGEST}})
_fb("get_errors", {"live_video_id": _LV}, "GET", _V + "/" + _LV + "?fields=errors")
_fb("create_live_video", {"title": "Sunday Service", "description": 'Live from the "main" hall'}, "POST",
    _V + "/1093482904102/live_videos",
    body='{"title":"Sunday Service","description":"Live from the \\"main\\" hall","status":"UNPUBLISHED"}',
    http_reply={"status": 200, "body": json.dumps(_CREATED)},
    expect_result={"ok": {"kind": "value", "value": _CREATED}})
_fb("schedule_live_video", {"title": "Evening Service", "start_time": 1791226800}, "POST",
    _V + "/1093482904102/live_videos",
    body='{"title":"Evening Service","description":"","status":"SCHEDULED_UNPUBLISHED",'
         '"event_params":{"start_time":1791226800}}')
_CUSTOM = {"status": "UNPUBLISHED", "enable_backup_ingest": True}
_fb("create_live_video_custom", {"live_video": _CUSTOM}, "POST", _V + "/1093482904102/live_videos",
    body=json.dumps(_CUSTOM, separators=(",", ":")))
_fb("set_title", {"live_video_id": _LV, "title": "Sunday Service"}, "POST", _V + "/" + _LV,
    body='{"title":"Sunday Service"}')
_fb("set_description", {"live_video_id": _LV, "description": "Main hall"}, "POST", _V + "/" + _LV,
    body='{"description":"Main hall"}')
_fb("set_privacy", {"live_video_id": _LV, "value": "EVERYONE"}, "POST", _V + "/" + _LV,
    body='{"privacy":{"value":"EVERYONE"}}')
_fb("set_embeddable", {"live_video_id": _LV, "enabled": False}, "POST", _V + "/" + _LV,
    body='{"embeddable":false}')
_fb("set_master_ingest_stream", {"live_video_id": _LV, "ingest_stream_id": "10155832421506571"}, "POST",
    _V + "/" + _LV, body='{"master_ingest_stream_id":"10155832421506571"}')
_fb("update_live_video", {"live_video_id": _LV, "changes": {"event_params": {"start_time": 1791230400}}},
    "POST", _V + "/" + _LV, body='{"event_params":{"start_time":1791230400}}')
# The Page can't go live yet (fewer than 100 followers): an ordinary failure.
_fb("go_live", {"live_video_id": _LV}, "POST", _V + "/" + _LV, body='{"status":"LIVE_NOW"}',
    http_reply={"status": 400, "body": json.dumps({"error": {
        "message": "Your Page must have at least 100 followers to go live.", "type": "OAuthException",
        "code": 200, "error_subcode": 1363144, "fbtrace_id": "AbCdEf"}})},
    expect_result={"error": {"error": "device_error", "code": "400"}})
# An expired token: code 190 under HTTP 400 is the terminal refusal.
_fb("end_live_video", {"live_video_id": _LV}, "POST", _V + "/" + _LV, body='{"end_live_video":true}',
    http_reply={"status": 400, "body": json.dumps({"error": {
        "message": "Error validating access token: Session has expired.", "type": "OAuthException",
        "code": 190, "error_subcode": 463, "fbtrace_id": "AbCdEf"}})},
    expect_result={"error": {"error": "auth"}})
_fb("delete_live_video", {"live_video_id": _LV}, "DELETE", _V + "/" + _LV,
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})

# Engagement.
_fb("list_comments", {"live_video_id": _LV}, "GET",
    _V + "/" + _LV + "/comments?order=reverse_chronological&filter=stream&live_filter=no_filter"
    "&summary=total_count&limit=25")
_fb("get_comment_count", {"live_video_id": _LV}, "GET",
    _V + "/" + _LV + "/comments?filter=stream&summary=total_count&limit=0",
    http_reply={"status": 200, "body": json.dumps({"data": [], "summary": {"order": "chronological",
                                                                          "total_count": 57}})},
    expect_result={"ok": {"kind": "value", "value": 57}})
_fb("get_reaction_count", {"live_video_id": _LV}, "GET",
    _V + "/" + _LV + "/reactions?summary=total_count&limit=0")

# Telemetry.
telemetry(FB, "page-live-videos", settings=_FBS, inbound_http={
    "path": _V + "/1093482904102/live_videos?" + _F + "&source=owner&limit=5",
    "body": json.dumps({"data": [_VIDEO, {"id": "10155832421501001", "title": "Last week", "status": "VOD",
                                          "live_views": 0, "ingest_streams": []}], "paging": {}})},
    expect_state={"live_videos": {_LV: _VIDEO_STATE,
                                  "10155832421501001": {"title": "Last week", "status": "VOD", "live_views": 0,
                                                        "ingest_streams": "[]"}}})
telemetry(FB, "live-video", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV + "?" + _F, "body": json.dumps(_VIDEO)},
    expect_state={"live_videos": {_LV: _VIDEO_STATE}})
