# Softron OnTheAir Video REST API and /playback websocket: the calls Softron's
# release notes and support article name, the rest as the Bitfocus Companion
# module makes them (inferred). Item additions read the playlists back.
OTA = "softron-ontheair-video"
_OTA_OK = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}

http(OTA, "play", {}, "GET", "/playback/play", **_OTA_OK)
http(OTA, "pause", {}, "GET", "/playback/pause")
http(OTA, "resume", {}, "GET", "/playback/resume")
http(OTA, "stop", {}, "GET", "/playback/stop", http_reply={"status": 404, "body": '{"error":"no playlist"}'},
     expect_result={"error": {"error": "device_error", "code": "404"}})
http(OTA, "skip_next", {}, "GET", "/playback/skip_next")
http(OTA, "skip_next_live", {}, "GET", "/playback/skip_next?live_only=1")
http(OTA, "skip_previous", {}, "GET", "/playback/skip_previous")
http(OTA, "join_in_progress", {}, "GET", "/playback/join_in_progress")
http(OTA, "cue_trigger", {}, "GET", "/playback/cue_trigger")
http(OTA, "gpi_trigger", {"input": 3}, "GET", "/playback/gpi_trigger?input=3")
http(OTA, "play_playlist", {"playlist": "Main Channel"}, "GET", "/playlists/Main%20Channel/play")
http(OTA, "pause_playlist", {"playlist": "0"}, "GET", "/playlists/0/pause")
http(OTA, "resume_playlist", {"playlist": "0"}, "GET", "/playlists/0/resume")
http(OTA, "stop_playlist", {"playlist": "0"}, "GET", "/playlists/0/stop")
http(OTA, "skip_next_in_playlist", {"playlist": "1"}, "GET", "/playlists/1/skip_next")
http(OTA, "skip_previous_in_playlist", {"playlist": "1"}, "GET", "/playlists/1/previous")
http(OTA, "join_playlist_in_progress", {"playlist": "1"}, "GET", "/playlists/1/join_in_progress")
http(OTA, "play_item", {"playlist": "0", "item": "4"}, "GET", "/playlists/0/items/4/play")
http(OTA, "pause_item", {"playlist": "0", "item": "Promo"}, "GET", "/playlists/0/items/Promo/pause")
http(OTA, "join_item_in_progress", {"playlist": "0", "item": "2"}, "GET", "/playlists/0/items/2/join_in_progress")
http(OTA, "play_item_at_timecode", {"playlist": "0", "item": "2", "timecode": "00:00:10:00"}, "GET",
     "/playlists/0/items/2/play?position_relative_timecode=00%3A00%3A10%3A00")
http(OTA, "pause_item_at_timecode", {"playlist": "0", "item": "2", "reference": "media_timecode",
                                     "timecode": "10:00:05:12"}, "GET",
     "/playlists/0/items/2/pause?position_media_timecode=10%3A00%3A05%3A12")
http(OTA, "play_item_at_seconds", {"playlist": "0", "item": "2", "seconds": 12.5}, "GET",
     "/playlists/0/items/2/play?position_relative_seconds=12.500")
http(OTA, "pause_item_at_seconds", {"playlist": "0", "item": "2", "seconds": 90.0}, "GET",
     "/playlists/0/items/2/pause?position_relative_seconds=90.000")
http(OTA, "play_cg_project", {"project": "CG-1"}, "GET", "/playback/cg_projects/CG-1/play")
http(OTA, "pause_cg_project", {"project": "CG-1"}, "GET", "/playback/cg_projects/CG-1/pause")
http(OTA, "stop_cg_project", {"project": "0"}, "GET", "/playback/cg_projects/0/stop")
http(OTA, "play_all_cg_projects", {}, "GET", "/playback/cg_projects/play")
http(OTA, "stop_all_cg_projects", {}, "GET", "/playback/cg_projects/stop")
http(OTA, "set_clocked_items", {"enabled": True}, "GET", "/playback/clocked_items?enabled=1")
http(OTA, "set_clocked_items", {"enabled": False}, "GET", "/playback/clocked_items?enabled=0",
     file="set_clocked_items-off")
http(OTA, "run_action", {"action": "Logo On"}, "GET", "/actions/Logo%20On/run")
http(OTA, "run_action_with_parameter", {"action": "Crawl", "parameter": "Breaking: news & more"}, "GET",
     "/actions/Crawl/run?parameter=Breaking%3A%20news%20%26%20more")
for command, index, body in [
        ("add_clip", None, '{"clip_type":"0","name":"Promo","url":"/Volumes/Media/promo.mov"}'),
        ("insert_clip", 3, '{"clip_type":"0","name":"","url":"/Volumes/Media/promo.mov"}'),
        ("add_comment", None, '{"clip_type":"3","name":"Break"}'),
        ("insert_comment", 0, '{"clip_type":"3","name":"Break"}'),
        ("add_live", None, '{"clip_type":"4","name":"","live_source_name":"Studio","duration":300}'),
        ("insert_live", 2, '{"clip_type":"4","name":"Live","live_source_name":"Studio","duration":300}'),
        ("add_stream", None, '{"clip_type":"5","name":"","url":"https://example.com/live.m3u8","duration":60}'),
        ("insert_stream", 1, '{"clip_type":"5","name":"","url":"https://example.com/live.m3u8","duration":60}')]:
    inp = {"playlist": "0"}
    if index is not None:
        inp["index"] = index
    if command.endswith("clip"):
        inp["path"] = "/Volumes/Media/promo.mov"
        if command == "add_clip":
            inp["name"] = "Promo"
    elif command.endswith("comment"):
        inp["name"] = "Break"
    elif command.endswith("live"):
        inp.update({"source": "Studio", "duration": 300})
        if command == "insert_live":
            inp["name"] = "Live"
    else:
        inp.update({"url": "https://example.com/live.m3u8", "duration": 60})
    target = "/playlists/0/items" + (f"/{index}" if index is not None else "")
    V.append({"spec": OTA, "command": command, "input": inp,
              "expect_request": [{"method": "POST", "target": target, "body": body}]})
http(OTA, "get_info", {}, "GET", "/info", http_reply={"status": 200, "body": '{"application_name":"OnTheAir Video"}'},
     expect_result={"ok": {"kind": "value", "value": {"application_name": "OnTheAir Video"}}})
http(OTA, "get_playing", {}, "GET", "/playback/playing")
http(OTA, "get_current_item", {}, "GET", "/playback/current_item")
http(OTA, "get_playlists", {}, "GET", "/playlists")
http(OTA, "get_playlist_items", {"playlist": "0"}, "GET", "/playlists/0/items")
http(OTA, "get_selected_items", {"playlist": "0"}, "GET", "/playlists/0/selected_items")
http(OTA, "get_cg_projects", {}, "GET", "/playback/cg_projects")

# ── Telemetry ──
telemetry(OTA, "ws-status", inbound_ws=json.dumps({"event": "playback_status_changed", "playback_status": "Playing"}),
          expect_state={"playback": {"status": "Playing"}})
telemetry(OTA, "ws-item-changed", inbound_ws=json.dumps({"event": "item_changed", "playlist_unique_id": "PL-1",
                                                         "item_unique_id": "IT-7", "item_name": "promo.mov"}),
          expect_state={"playback": {"playlist_id": "PL-1", "item_id": "IT-7"}})
telemetry(OTA, "ws-timing", inbound_ws=json.dumps({
    "event": "playback_timing_changed", "item_unique_id": "IT-7", "item_display_name": "Promo",
    "item_duration": 30.0, "item_elapsed_time": 12.5, "item_remaining_time": 17.5, "playlist_duration": 3600.0,
    "next_live_unique_id": "IT-9", "next_live_display_name": "News", "remaining_time_until_next_live": 845.5}),
    expect_state={"playback": {"item_id": "IT-7", "item_name": "Promo", "item_duration": 30.0, "item_elapsed": 12.5,
                               "item_remaining": 17.5, "playlist_duration": 3600.0,
                               "next_live": {"id": "IT-9", "name": "News", "remaining": 845.5}}})
telemetry(OTA, "ws-cg-project", inbound_ws=json.dumps({"type": "project property", "projectUniqueID": "CG-1",
                                                       "changes": {"status": "Playing", "elapsed_time": 2.0}}),
          expect_state={"cg_projects": {"CG-1": {"status": "Playing", "elapsed": 2.0}}})
telemetry(OTA, "info", inbound_http={"path": "/info", "body": json.dumps({
    "application_name": "OnTheAir Video", "application_version": "4.5.8", "mac_os_version": "15.6",
    "computer_name": "Playout 1", "available_actions": ["Logo On", "Logo Off"]})},
    state_before={"actions": {"0": "Old", "1": "Gone", "2": "Too"}},
    expect_state={"info": {"application_name": "OnTheAir Video", "application_version": "4.5.8",
                           "mac_os_version": "15.6", "computer_name": "Playout 1"},
                  "actions": {"0": "Logo On", "1": "Logo Off"}})
telemetry(OTA, "playing", inbound_http={"path": "/playback/playing", "body": json.dumps({
    "playback_status": "Paused", "playlist_index": 0, "playlist_display_name": "Main", "item_index": 3,
    "item_display_name": "Promo", "item_duration": 30.0, "item_elapsed": 10.0, "item_remaining": 20.0,
    "playlist_duration": 3600.0, "playlist_elapsed": 600.0, "playlist_remaining": 3000.0})},
    expect_state={"playback": {"status": "Paused", "playlist_index": 0, "playlist_name": "Main", "item_index": 3,
                               "item_name": "Promo", "item_duration": 30.0, "item_elapsed": 10.0,
                               "item_remaining": 20.0, "playlist_duration": 3600.0, "playlist_elapsed": 600.0,
                               "playlist_remaining": 3000.0}})
telemetry(OTA, "playlists", inbound_http={"path": "/playlists", "body": json.dumps([
    {"unique_id": "PL-1", "name": "Main"}, {"unique_id": "PL-2", "name": "Backup"}])},
    state_before={"playlists": {"PL-0": {"name": "Closed"}}},
    expect_state={"playlists": {"PL-1": {"name": "Main", "index": 0}, "PL-2": {"name": "Backup", "index": 1}}},
    expect_then_send=[{"method": "GET", "target": "/playlists/PL-1/items"},
                      {"method": "GET", "target": "/playlists/PL-2/items"}])
telemetry(OTA, "playlist-items", inbound_http={"path": "/playlists/PL-1/items", "body": json.dumps([
    {"unique_id": "IT-1", "name": "Open", "duration": 10.0}, {"unique_id": "IT-2", "name": "Promo", "duration": 30.0}])},
    state_before={"playlists": {"PL-1": {"name": "Main", "items": {"2": {"name": "Old"}}}}},
    expect_state={"playlists": {"PL-1": {"name": "Main", "items": {
        "0": {"id": "IT-1", "name": "Open", "duration": 10.0}, "1": {"id": "IT-2", "name": "Promo", "duration": 30.0}}}}})
telemetry(OTA, "cg-projects", inbound_http={"path": "/playback/cg_projects", "body": json.dumps([
    {"unique_id": "CG-1", "display_name": "Lower third", "status": "Stopped", "elapsed_time": 0.0, "duration": 8.0}])},
    expect_state={"cg_projects": {"CG-1": {"name": "Lower third", "status": "Stopped", "elapsed": 0.0, "duration": 8.0}}})
telemetry(OTA, "read-back", inbound_http={"path": "/playback/stop", "body": ""},
          expect_state={}, expect_then_send=[{"method": "GET", "target": "/playback/playing"}])
