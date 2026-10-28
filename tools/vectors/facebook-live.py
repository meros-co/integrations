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
                "ingest_streams": {"10155832421506570": {
                    "is_master": True, "video_bitrate": 4005000, "audio_bitrate": 128000.5, "video_framerate": 30.0,
                    "video_gop_size": 2000, "video_width": 1920, "video_height": 1080}}}
_CREATED = {"id": _LV, "stream_url": "rtmp://live-api-s.facebook.com:80/rtmp/FB-10155832421501570-0-AbzX",
            "secure_stream_url": "rtmps://live-api-s.facebook.com:443/rtmp/FB-10155832421501570-0-AbzX"}

# Live videos.
_fb("list_live_videos", {}, "GET", _V + "/1093482904102/live_videos?" + _F + "&source=owner&limit=10",
    http_reply={"status": 200, "body": json.dumps({"data": [_VIDEO], "paging": {}})},
    expect_result={"ok": {"kind": "value", "value": [_VIDEO]}})
_fb("list_live_videos_after", {"after": "QVFIUmlTN2JX"}, "GET",
    _V + "/1093482904102/live_videos?" + _F + "&source=owner&limit=10&after=QVFIUmlTN2JX")
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
    _V + "/" + _LV + "/comments?fields=" + _q("id,message,from,created_time,is_hidden,comment_count", safe="")
    + "&order=reverse_chronological&filter=stream&live_filter=no_filter&summary=total_count&limit=25")
_fb("list_comments_after", {"live_video_id": _LV, "after": "QVFIUmlTN2JX", "limit": 50}, "GET",
    _V + "/" + _LV + "/comments?fields=" + _q("id,message,from,created_time,is_hidden,comment_count", safe="")
    + "&order=reverse_chronological&filter=stream&live_filter=no_filter&limit=50&after=QVFIUmlTN2JX")
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
                                  "10155832421501001": {"title": "Last week", "status": "VOD", "live_views": 0}}})
telemetry(FB, "live-video", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV + "?" + _F, "body": json.dumps(_VIDEO)},
    expect_state={"live_videos": {_LV: _VIDEO_STATE}})
telemetry(FB, "ingest-streams", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV + "?fields=" + _q("ingest_streams{id,stream_id,is_master,stream_health}", safe=""),
    "body": json.dumps({"ingest_streams": _INGEST + [{"id": "10155832421506571", "is_master": False,
                                                      "stream_health": {"video_bitrate": 0}}], "id": _LV})},
    expect_state={"live_videos": {_LV: {"ingest_streams": {
        "10155832421506570": _VIDEO_STATE["ingest_streams"]["10155832421506570"],
        "10155832421506571": {"is_master": False, "video_bitrate": 0}}}}})

# Comment moderation (Comment and Object Comments references).
_CM = "10155832421501570_1234567890123456"
_fb("comment_on_live_video", {"live_video_id": _LV, "message": "Welcome, everyone!"}, "POST",
    _V + "/" + _LV + "/comments", body='{"message":"Welcome, everyone!"}',
    http_reply={"status": 200, "body": json.dumps({"id": _LV + "_9999"})},
    expect_result={"ok": {"kind": "value", "value": _LV + "_9999"}})
_fb("reply_to_comment", {"comment_id": _CM, "message": "Thanks for watching!"}, "POST",
    _V + "/" + _CM + "/comments", body='{"message":"Thanks for watching!"}')
_fb("hide_comment", {"comment_id": _CM}, "POST", _V + "/" + _CM, body='{"is_hidden":true}',
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})
_fb("delete_comment", {"comment_id": _CM}, "DELETE", _V + "/" + _CM,
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})

# Scheduling (Scheduling guide: Reschedule a broadcast).
_fb("reschedule_live_video", {"live_video_id": _LV, "start_time": 1791240000}, "POST", _V + "/" + _LV,
    body='{"event_params":{"start_time":1791240000}}',
    http_reply={"status": 200, "body": json.dumps({"id": _LV})},
    expect_result={"ok": {"kind": "value", "value": {"id": _LV}}})
_fb("cancel_scheduled_live_video", {"live_video_id": _LV}, "POST", _V + "/" + _LV,
    body='{"status":"SCHEDULED_CANCELED"}')

# Live polls (Video Poll and Live Video Polls references).
_POLLF = "fields=" + _q("id,question,status,show_results,poll_options{id,text,order,total_votes,is_correct}", safe="")
_PL = "2650153651956578"
_fb("list_polls", {"live_video_id": _LV}, "GET", _V + "/" + _LV + "/polls?" + _POLLF)
_fb("get_poll", {"poll_id": _PL}, "GET", _V + "/" + _PL + "?" + _POLLF)
_fb("create_poll", {"live_video_id": _LV, "question": "Which hymn next?", "options": ["Abide with Me", "Jerusalem"]},
    "POST", _V + "/" + _LV + "/polls",
    body='{"question":"Which hymn next?","options":["Abide with Me","Jerusalem"],"show_results":true}',
    http_reply={"status": 200, "body": json.dumps({"id": _PL, "option_ids": ["111", "112"]})},
    expect_result={"ok": {"kind": "value", "value": {"id": _PL, "option_ids": ["111", "112"]}}})
_fb("create_quiz_poll", {"live_video_id": _LV, "question": "Which book comes first?",
                         "options": ["Genesis", "Exodus"], "correct_option": 1, "show_results": False},
    "POST", _V + "/" + _LV + "/polls",
    body='{"question":"Which book comes first?","options":["Genesis","Exodus"],"correct_option":1,"show_results":false}')
_fb("show_poll_voting", {"poll_id": _PL}, "POST", _V + "/" + _PL, body='{"action":"SHOW_VOTING","show_results":true}',
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})
_fb("show_poll_results", {"poll_id": _PL}, "POST", _V + "/" + _PL, body='{"action":"SHOW_RESULTS"}')
_fb("close_poll", {"poll_id": _PL}, "POST", _V + "/" + _PL, body='{"action":"CLOSE"}')
_fb("attach_poll_to_video", {"poll_id": _PL}, "POST", _V + "/" + _PL, body='{"action":"ATTACH_TO_VIDEO"}')
_fb("delete_poll", {"poll_id": _PL}, "POST", _V + "/" + _PL, body='{"action":"DELETE_POLL"}')
_OPTIONS = {"data": [{"id": "111", "text": "Abide with Me", "order": 1, "total_votes": 14, "is_correct": False},
                     {"id": "112", "text": "Jerusalem", "order": 2, "total_votes": 9, "is_correct": False}]}
_POLL_ST = {"question": "Which hymn next?", "status": "voting_open", "show_results": True,
            "options": {"111": {"text": "Abide with Me", "order": 1, "total_votes": 14, "is_correct": False},
                        "112": {"text": "Jerusalem", "order": 2, "total_votes": 9, "is_correct": False}}}
telemetry(FB, "polls", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV + "/polls?" + _POLLF,
    "body": json.dumps({"data": [{"id": _PL, "question": "Which hymn next?", "status": "voting_open",
                                  "show_results": True, "poll_options": _OPTIONS}]})},
    state_before={"polls": {_PL: {"options": {"110": {"text": "removed"}}}}},
    expect_state={"polls": {_PL: {"live_video_id": _LV, **_POLL_ST}}})
telemetry(FB, "poll", settings=_FBS, inbound_http={
    "path": _V + "/" + _PL + "?" + _POLLF,
    "body": json.dumps({"id": _PL, "question": "Which hymn next?", "status": "closed", "show_results": True,
                        "poll_options": _OPTIONS})},
    state_before={"polls": {_PL: {"live_video_id": _LV, "status": "voting_open"}}},
    expect_state={"polls": {_PL: {"live_video_id": _LV, **_POLL_ST, "status": "closed"}}})
telemetry(FB, "poll-created", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV + "/polls", "body": json.dumps({"id": _PL, "option_ids": ["111", "112"]})},
    expect_state={}, expect_then_send=[{"method": "GET", "target": _V + "/" + _LV + "/polls?" + _POLLF}])
telemetry(FB, "poll-closed", settings=_FBS, inbound_http={
    "path": _V + "/" + _PL, "body": '{"success":true}', "request": {"action": "CLOSE"}},
    expect_state={}, expect_then_send=[{"method": "GET", "target": _V + "/" + _PL + "?" + _POLLF}])
telemetry(FB, "poll-deleted", settings=_FBS, inbound_http={
    "path": _V + "/" + _PL, "body": '{"success":true}', "request": {"action": "DELETE_POLL"}},
    state_before={"polls": {_PL: _POLL_ST, "2650153651956579": {"question": "kept"}}},
    expect_state={"polls": {"2650153651956579": {"question": "kept"}}})
# An update of the live video itself is not a poll action.
telemetry(FB, "live-video-updated", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV, "body": json.dumps({"id": _LV}), "request": {"title": "Sunday Service"}},
    expect_state={})

# Crossposting (Crossposting guide).
_XF = "fields=" + _q("crosspost_shared_pages,crossposted_broadcasts{id,status,from{id,name}}", safe="")
_fb("list_crosspost_pages", {}, "GET",
    _V + "/1093482904102/crosspost_whitelisted_pages?fields=" + _q("id,name,allows_live_crossposts", safe="")
    + "&limit=100")
_fb("get_crossposting", {"live_video_id": _LV}, "GET", _V + "/" + _LV + "?" + _XF)
_fb("set_crossposting", {"live_video_id": _LV, "page_id": "106589754846067"}, "POST", _V + "/" + _LV + "?" + _XF,
    body='{"crossposting_actions":[{"page_id":"106589754846067","action":"enable_crossposting_and_create_post"}]}')
_XA = [{"page_id": "107738621396466", "action": "enable_crossposting"},
       {"page_id": "106343288214714", "action": "disable_crossposting"}]
_fb("set_crossposting_actions", {"live_video_id": _LV, "actions": _XA}, "POST", _V + "/" + _LV + "?" + _XF,
    body='{"crossposting_actions":' + json.dumps(_XA, separators=(",", ":")) + "}")
_fb("request_crossposting", {"partner_page_id": "106589754846067"}, "POST", _V + "/1093482904102",
    body='{"begin_crossposting_handshake":[{"partner_page_id":"106589754846067","allow_live":true}]}',
    http_reply={"status": 200, "body": '{"success":true}'}, expect_result={"ok": {"kind": "ack"}})
_fb("accept_crossposting", {"partner_page_id": "106589754846067", "allow_live": False}, "POST", _V + "/1093482904102",
    body='{"accept_crossposting_handshake":[{"partner_page_id":"106589754846067","allow_live":false}]}')
telemetry(FB, "crosspost-pages", settings=_FBS, inbound_http={
    "path": _V + "/1093482904102/crosspost_whitelisted_pages?fields=" + _q("id,name,allows_live_crossposts", safe="")
            + "&limit=100",
    "body": json.dumps({"data": [{"id": "107738621396466", "name": "Crossposting Page C", "allows_live_crossposts": False},
                                 {"id": "106589754846067", "name": "Crossposting Page B", "allows_live_crossposts": True}],
                        "paging": {"cursors": {"before": "a", "after": "b"}}})},
    state_before={"crosspost_pages": {"1": {"name": "Ended partnership"}}},
    expect_state={"crosspost_pages": {"107738621396466": {"name": "Crossposting Page C", "allows_live_crossposts": False},
                                      "106589754846067": {"name": "Crossposting Page B", "allows_live_crossposts": True}}})
telemetry(FB, "crossposting", settings=_FBS, inbound_http={
    "path": _V + "/" + _LV + "?" + _XF,
    "body": json.dumps({"crosspost_shared_pages": {"data": [{"name": "Crossposting Page C", "id": "107738621396466"}]},
                        "crossposted_broadcasts": {"data": [{"id": "10155832421509999", "status": "UNPUBLISHED",
                                                             "from": {"id": "106589754846067",
                                                                      "name": "Crossposting Page B"}}]},
                        "id": _LV})},
    state_before={"live_videos": {_LV: {"title": "Sunday Service",
                                        "crosspost_shared_pages": {"1": {"name": "No longer shared"}}}}},
    expect_state={"live_videos": {_LV: {
        "title": "Sunday Service",
        "crosspost_shared_pages": {"107738621396466": {"name": "Crossposting Page C"}},
        "crossposted_broadcasts": {"10155832421509999": {"page_id": "106589754846067",
                                                         "page_name": "Crossposting Page B",
                                                         "status": "UNPUBLISHED"}}}}})
