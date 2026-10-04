# Vimeo Live (vimeo-live): one vector per command, and telemetry vectors for
# the polled resources. Targets are the live-event endpoints as Vimeo's API
# reference gives them (https://api.vimeo.com/...), using the /me aliases it
# lists. Reply bodies follow the reference's response schemas; ids are made up.
VM = "vimeo-live"
_VMS = {"token": "0123456789abcdef0123456789abcdef", "live_event_id": "12345"}
_VME = "/me/live_events/12345"


def _vm(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": VM, "command": command, "input": input, "settings": _VMS,
              "expect_request": request, **extra})


_E = {"live_event_id": "12345"}

# Events.
_vm("list_live_events", {}, "GET", "/me/live_events?type=all&sort=date&direction=desc&per_page=25&page=1")
_vm("get_live_event", _E, "GET", _VME)
_vm("get_stream_key", _E, "GET", _VME + "?fields=stream_key",
    http_reply={"status": 200, "body": json.dumps({"stream_key": "ab9c8def-7a65-4321-b098-c7dd65e43f21"})},
    expect_result={"ok": {"kind": "value", "value": "ab9c8def-7a65-4321-b098-c7dd65e43f21"}})
_vm("get_backup_stream_key", _E, "GET", _VME + "?fields=backup_stream_key")
_vm("get_rtmp_link", _E, "GET", _VME + "?fields=rtmp_link",
    http_reply={"status": 200, "body": json.dumps({"rtmp_link": "rtmp://rtmp.cloud.vimeo.com/live"})},
    expect_result={"ok": {"kind": "value", "value": "rtmp://rtmp.cloud.vimeo.com/live"}})
_vm("get_rtmps_link", _E, "GET", _VME + "?fields=rtmps_link")
_vm("get_srt_link", _E, "GET", _VME + "?fields=srt_link")
_vm("get_srt_passphrase", _E, "GET", _VME + "?fields=srt_passphrase")
_vm("create_live_event", {"title": "Sunday Service"}, "POST", "/me/live_events",
    body='{"title":"Sunday Service","stream_privacy":{"view":"anybody"},"latency":"standard"}',
    http_reply={"status": 200, "body": json.dumps({"uri": "/live_events/67890", "title": "Sunday Service"})},
    expect_result={"ok": {"kind": "value", "value": "/live_events/67890"}})
_vm("create_scheduled_live_event", {"title": "Sunday Service", "start_time": "2026-10-11T09:30:00Z",
                                    "privacy": "unlisted", "latency": "low"},
    "POST", "/me/live_events",
    body='{"title":"Sunday Service","schedule":{"start_time":"2026-10-11T09:30:00Z"},"stream_privacy":{"view":"unlisted"},"latency":"low"}')
_EV = {"title": "Midweek", "schedule": {"start_time": "2026-10-14T19:00:00Z", "rrule": "FREQ=WEEKLY"}, "dvr": True}
_vm("create_live_event_custom", {"event": _EV}, "POST", "/me/live_events", body=json.dumps(_EV, separators=(",", ":")))
_CHG = {"stream_title": "Week 12", "chat_enabled": False}
_vm("update_live_event", {"live_event_id": "12345", "changes": _CHG}, "PATCH", _VME,
    body=json.dumps(_CHG, separators=(",", ":")))
_vm("set_title", {"live_event_id": "12345", "title": "Sunday \"Service\""}, "PATCH", _VME,
    body='{"title":"Sunday \\"Service\\""}',
    http_reply={"status": 200, "body": json.dumps({"uri": "/live_events/12345", "title": "Sunday \"Service\""})},
    expect_result={"ok": {"kind": "ack"}})
_vm("set_stream_title", {"live_event_id": "12345", "title": "Week 12"}, "PATCH", _VME,
    body='{"automatically_title_stream":false,"stream_title":"Week 12"}')
_vm("set_stream_description", {"live_event_id": "12345", "description": "Live from the main hall"}, "PATCH", _VME,
    body='{"stream_description":"Live from the main hall"}')
_vm("set_privacy", {"live_event_id": "12345", "privacy": "unlisted"}, "PATCH", _VME,
    body='{"stream_privacy":{"view":"unlisted"}}')
_vm("set_latency", {"live_event_id": "12345", "latency": "fail-safe"}, "PATCH", _VME, body='{"latency":"fail-safe"}')
_vm("set_low_latency", {"live_event_id": "12345", "enabled": True}, "PATCH", _VME + "/low_latency",
    body='{"low_latency":true}',
    http_reply={"status": 200, "body": json.dumps({"lowLatency": True})},
    expect_result={"ok": {"kind": "value", "value": {"lowLatency": True}}})
_vm("set_chat_enabled", {"live_event_id": "12345", "enabled": False}, "PATCH", _VME, body='{"chat_enabled":false}')
_vm("set_dvr", {"live_event_id": "12345", "enabled": True}, "PATCH", _VME, body='{"dvr":true}')
_vm("set_auto_captions", {"live_event_id": "12345", "enabled": True, "language": "de-DE"}, "PATCH", _VME,
    body='{"auto_cc_enabled":true,"auto_cc_language":"de-DE"}')
_vm("set_schedule", {"live_event_id": "12345", "start_time": "2026-10-11T09:30:00+01:00"}, "PATCH", _VME,
    body='{"schedule":{"start_time":"2026-10-11T09:30:00+01:00"}}')
# Already activated: an ordinary failure.
_vm("activate_live_event", _E, "POST", _VME + "/activate", body="{}",
    http_reply={"status": 400, "body": json.dumps({"error": "The event has already been activated.",
                                                   "error_code": 2428})},
    expect_result={"error": {"error": "device_error", "code": "400"}})
_vm("end_live_event", _E, "POST", _VME + "/end")
_vm("delete_live_event", _E, "DELETE", _VME,
    http_reply={"status": 204, "body": ""}, expect_result={"ok": {"kind": "ack"}})
_vm("get_session_status", _E, "GET", "/live_events/12345/session_status")
_vm("get_m3u8_playback", {"live_event_id": "12345", "ttl": 30}, "GET", _VME + "/m3u8_playback?ttl=30")
# A 401 for an event the user may not see (error 3200) is not a refused token.
_vm("list_live_event_videos", _E, "GET", _VME + "/videos?sort=date&direction=desc&per_page=25",
    http_reply={"status": 401, "body": json.dumps({"error": "You can't access this event.", "error_code": 3200})},
    expect_result={"error": {"error": "device_error", "code": "401"}})

# Destinations.
_vm("list_destinations", _E, "GET", _VME + "/destinations")
_vm("add_rtmp_destination", {"live_event_id": "12345", "display_name": "Overflow", "stream_url": "rtmp://example.com/live",
                             "stream_key": "abc123"},
    "POST", _VME + "/destinations",
    body='{"service_name":"custom_rtmp","type":"custom","display_name":"Overflow","stream_url":"rtmp://example.com/live","stream_key":"abc123","is_enabled":true}',
    http_reply={"status": 200, "body": json.dumps({"id": 1234, "display_name": "Overflow", "is_enabled": True,
                                                   "service_name": "custom_rtmp"})},
    expect_result={"ok": {"kind": "value", "value": 1234}})
_DST = {"service_name": "youtube", "type": "channel", "display_name": "Church channel",
        "provider_destination_id": "UC123", "privacy": "unlisted"}
_vm("add_destination_custom", {"live_event_id": "12345", "destination": _DST}, "POST", _VME + "/destinations",
    body=json.dumps(_DST, separators=(",", ":")))
_vm("set_destination_enabled", {"destination_id": 1234, "enabled": False}, "PATCH", "/destination/1234",
    body='{"is_enabled":false}')
_vm("update_destination", {"destination_id": 1234, "changes": {"display_name": "Overflow 2"}}, "PATCH",
    "/destination/1234", body='{"display_name":"Overflow 2"}')
_vm("delete_destination", {"destination_id": 1234}, "DELETE", "/destination/1234")
# A refused token (error 8003): terminal, reported as auth.
_vm("list_available_destinations", {}, "GET", "/me/destinations",
    http_reply={"status": 401, "body": json.dumps({"error": "Something strange occurred.",
                                                   "developer_message": "The app didn't receive the user's credentials.",
                                                   "error_code": 8003})},
    expect_result={"error": {"error": "auth"}})

# Telemetry.
telemetry(VM, "event", settings=_VMS, inbound_http={
    "path": _VME + "?fields=uri%2Ctitle%2Cstatus%2Cstream_mode%2Clatency%2Cstream_privacy%2Cchat_enabled%2Cdvr%2Cauto_cc_enabled%2Cstream_title%2Cstart_time%2Cnext_occurrence_time%2Clink",
    "body": json.dumps({"uri": "/live_events/12345", "title": "Sunday Service", "status": "started",
                        "stream_mode": "live", "latency": "low", "stream_privacy": {"view": "unlisted", "embed": "public"},
                        "chat_enabled": True, "dvr": False, "auto_cc_enabled": False, "stream_title": "Week 12",
                        "start_time": "2026-10-11T09:30:00+00:00", "next_occurrence_time": "2026-10-18T09:30:00+00:00",
                        "link": "https://vimeo.com/event/12345"})},
    expect_state={"events": {"12345": {
        "title": "Sunday Service", "status": "started", "stream_mode": "live", "latency": "low",
        "privacy": "unlisted", "chat_enabled": True, "dvr": False, "auto_cc_enabled": False,
        "stream_title": "Week 12", "start_time": "2026-10-11T09:30:00+00:00",
        "next_occurrence_time": "2026-10-18T09:30:00+00:00", "link": "https://vimeo.com/event/12345"}}})
telemetry(VM, "low-latency", settings=_VMS, inbound_http={
    "path": _VME + "/low_latency", "body": json.dumps({"lowLatency": True})},
    expect_state={"events": {"12345": {"low_latency": True}}})
telemetry(VM, "session-streaming", settings=_VMS, inbound_http={
    "path": "/live_events/12345/session_status",
    "body": json.dumps({"archive": None, "can_manage": True, "id": 1111, "status": "started", "stream_mode": "live",
                        "ingest": {"encoder_type": "rtmp", "end_time": None, "height": 1080, "width": 1920,
                                   "is_rtmp_session": True, "rtmp_link": "rtmp://rtmp.cloud.vimeo.com/live",
                                   "stream_key": "ab9c8def-7a65-4321-b098-c7dd65e43f21", "start_time": 1791710460,
                                   "status": 4, "stream_ended_reason": None},
                        "metering": {"seconds_max": 36000, "seconds_remaining": 32400}})},
    expect_state={"events": {"12345": {
        "session_status": "started", "live_video_id": 1111, "ingest_status": 4, "encoder_type": "rtmp",
        "ingest_width": 1920, "ingest_height": 1080, "ingest_start_time": 1791710460, "seconds_remaining": 32400}}})
_VMD = [{"id": 1234, "display_name": "Overflow", "is_enabled": True, "service_name": "custom_rtmp", "state": 1,
         "state_message": "Couldn't connect to rtmp://1.2.3.4/live", "stream_key": "secret",
         "stream_url": "rtmp://1.2.3.4/live", "type": "custom"}]
_VMDS = {"events": {"12345": {"destinations": {"1234": {
    "name": "Overflow", "service": "custom_rtmp", "enabled": True, "state": 1,
    "state_message": "Couldn't connect to rtmp://1.2.3.4/live"}}}}}
# The whole list replaces the event's destinations: 99, deleted, leaves.
_VMGONE = {"events": {"12345": {"title": "Sunday", "destinations": {"99": {"name": "Deleted", "enabled": False}}}}}
_VMKEPT = {"events": {"12345": {"title": "Sunday", **_VMDS["events"]["12345"]}}}
telemetry(VM, "destinations", settings=_VMS, inbound_http={
    "path": _VME + "/destinations", "body": json.dumps(_VMD)},
    state_before=_VMGONE, expect_state=_VMKEPT)
telemetry(VM, "destinations-paged", settings=_VMS, inbound_http={
    "path": _VME + "/destinations", "body": json.dumps({"total": 1, "page": 1, "per_page": 25, "data": _VMD})},
    state_before=_VMGONE, expect_state=_VMKEPT)
# More destinations than one page holds: the page adds, nothing leaves.
telemetry(VM, "destinations-page-of-several", settings=_VMS, inbound_http={
    "path": _VME + "/destinations", "body": json.dumps({"total": 30, "page": 1, "per_page": 25, "data": _VMD})},
    state_before=_VMGONE,
    expect_state={"events": {"12345": {"title": "Sunday", "destinations": {
        "99": {"name": "Deleted", "enabled": False}, **_VMDS["events"]["12345"]["destinations"]}}}})
# Adding a destination answers it alone, not a list: nothing leaves (it
# shows at the next poll).
telemetry(VM, "destination-added", settings=_VMS, inbound_http={
    "path": _VME + "/destinations", "body": json.dumps(_VMD[0])},
    state_before=_VMGONE, expect_state=_VMGONE)
