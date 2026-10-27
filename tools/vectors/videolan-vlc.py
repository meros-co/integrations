# VLC web interface (videolan-vlc): one vector per command, from the VLC HTTP
# requests wiki page's list (status?command=...&input=|id=|val=, parameters
# URL-encoded), on the .json pages; rate and audiodelay as the Bitfocus
# Companion module sends them. A command reading the playlist back sends
# that read after the first request is answered; the vector states the first.
VL = "videolan-vlc"
S = "/requests/status.json?command="

http(VL, "play_now", {"input": "rtsp://cam/stream 1"}, "GET", S + "in_play&input=rtsp%3A%2F%2Fcam%2Fstream%201",
     http_reply={"status": 200, "body": "{}"}, expect_result={"ok": {"kind": "ack"}})
http(VL, "enqueue", {"input": "file:///C:/media/clip.mp4"}, "GET", S + "in_enqueue&input=file%3A%2F%2F%2FC%3A%2Fmedia%2Fclip.mp4")
http(VL, "play", {}, "GET", S + "pl_play")
http(VL, "play_item", {"id": 4}, "GET", S + "pl_play&id=4")
http(VL, "toggle_pause", {}, "GET", S + "pl_pause")
http(VL, "toggle_pause_item", {"id": 4}, "GET", S + "pl_pause&id=4")
http(VL, "stop", {}, "GET", S + "pl_stop")
http(VL, "next", {}, "GET", S + "pl_next")
http(VL, "previous", {}, "GET", S + "pl_previous")
V.append({"spec": VL, "command": "delete_item", "input": {"id": 4}, "expect_request": [
    {"method": "GET", "target": S + "pl_delete&id=4"}]})
V.append({"spec": VL, "command": "empty_playlist", "input": {}, "expect_request": [
    {"method": "GET", "target": S + "pl_empty"}]})
V.append({"spec": VL, "command": "sort_playlist", "input": {"mode": "1", "reverse": True}, "expect_request": [
    {"method": "GET", "target": S + "pl_sort&id=1&val=1"}]})
http(VL, "toggle_random", {}, "GET", S + "pl_random")
http(VL, "toggle_loop", {}, "GET", S + "pl_loop")
http(VL, "toggle_repeat", {}, "GET", S + "pl_repeat")
http(VL, "toggle_service_discovery", {"module": "sap"}, "GET", S + "pl_sd&val=sap")
http(VL, "toggle_fullscreen", {}, "GET", S + "fullscreen")
http(VL, "set_volume", {"volume": "50%"}, "GET", S + "volume&val=50%25")
http(VL, "set_volume", {"volume": "+10"}, "GET", S + "volume&val=%2B10", file="set_volume-relative")
http(VL, "seek", {"position": "+1H:2M"}, "GET", S + "seek&val=%2B1H%3A2M")
http(VL, "seek", {"position": "-10%"}, "GET", S + "seek&val=-10%25", file="seek-percent")
http(VL, "set_rate", {"rate": 1.5}, "GET", S + "rate&val=1.500")
http(VL, "set_audio_delay", {"delay": -0.25}, "GET", S + "audiodelay&val=-0.250")
http(VL, "get_status", {}, "GET", "/requests/status.json", http_reply={"status": 200, "body": '{"state":"playing"}'},
     expect_result={"ok": {"kind": "value", "value": {"state": "playing"}}})
http(VL, "get_playlist", {}, "GET", "/requests/playlist.json")
http(VL, "browse", {"dir": "file:///C:/media"}, "GET", "/requests/browse.json?dir=file%3A%2F%2F%2FC%3A%2Fmedia")
http(VL, "get_vlm", {}, "GET", "/requests/vlm.xml")
http(VL, "vlm_command", {"command": "new channel1 broadcast enabled"}, "GET",
     "/requests/vlm_cmd.xml?command=new%20channel1%20broadcast%20enabled")

telemetry(VL, "status", inbound_http={"path": "/requests/status.json", "body": json.dumps({
    "version": "3.0.21 Vetinari", "state": "playing", "time": 12, "length": 400, "position": 0.03,
    "volume": 256, "rate": 1, "audiodelay": 0, "random": False, "loop": True, "repeat": False,
    "fullscreen": False, "currentplid": 4,
    "information": {"category": {"meta": {"title": "Opening", "filename": "opening.mp4"}}}})},
    expect_state={"version": "3.0.21 Vetinari",
                  "playback": {"state": "playing", "time": 12, "length": 400, "position": 0.03, "volume": 256,
                               "rate": 1.0, "audio_delay": 0.0, "random": False, "loop": True, "repeat": False,
                               "fullscreen": False, "current_item": 4, "title": "Opening",
                               "filename": "opening.mp4"}})
telemetry(VL, "playlist", inbound_http={"path": "/requests/playlist.json", "body": json.dumps({
    "type": "node", "name": "", "id": "1", "children": [
        {"type": "node", "name": "Playlist", "id": "2", "children": [
            {"type": "leaf", "name": "opening.mp4", "id": "4", "uri": "file:///C:/media/opening.mp4", "duration": 400},
            {"type": "leaf", "name": "Camera", "id": "5", "uri": "rtsp://cam/stream", "duration": -1}]},
        {"type": "node", "name": "Media Library", "id": "3", "children": []}]})},
    state_before={"playlist": {"9": {"name": "Removed"}}},
    expect_state={"playlist": {"4": {"name": "opening.mp4", "uri": "file:///C:/media/opening.mp4", "duration": 400,
                                     "position": 0},
                               "5": {"name": "Camera", "uri": "rtsp://cam/stream", "duration": -1, "position": 1}}})
