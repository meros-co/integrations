# OpenSong API (opensong): one vector per command, from the request forms on
# opensong.org's API page (/resource/action/identifier/param:value, GET for
# reads, POST for changes), each placeholder percent-encoded as a path
# segment; and the status reply example on the same page.
OS = "opensong"


def osv(command, input, method, target, **extra):
    http(OS, command, input, method, target, **extra)


osv("get_status", {}, "GET", "/presentation/status",
    http_reply={"status": 200, "body": '<response resource="presentation" action="status"/>'},
    expect_result={"ok": {"kind": "value", "value": '<response resource="presentation" action="status"/>'}})
osv("get_slides", {}, "GET", "/presentation/slide/list")
osv("get_slide", {"item": 12}, "GET", "/presentation/slide/12")
osv("get_songs", {}, "GET", "/song/list")
osv("get_songs_in_folder", {"folder": "Hymns"}, "GET", "/song/list/folder:Hymns")
osv("get_song", {"song": "Amazing Grace"}, "GET", "/song/detail/Amazing%20Grace")
osv("get_song_in_folder", {"song": "Amazing Grace", "folder": "Hymns"}, "GET", "/song/detail/Amazing%20Grace/folder:Hymns")
osv("get_song_folders", {}, "GET", "/song/folders")
osv("get_sets", {}, "GET", "/set/list")
osv("get_set_slides", {"set": "Sunday AM"}, "GET", "/set/slide/Sunday%20AM")
osv("get_set_slide", {"set": "Sunday AM", "index": 3}, "GET", "/set/slide/Sunday%20AM/slide:3")
osv("load_song", {"song": "Amazing Grace"}, "POST", "/song/load/Amazing%20Grace",
    http_reply={"status": 200, "body": "<response/>"}, expect_result={"ok": {"kind": "ack"}})
osv("load_song_from_folder", {"song": "Amazing Grace", "folder": "Hymns"}, "POST", "/song/load/Amazing%20Grace/folder:Hymns")
osv("present_song", {"song": "Amazing Grace"}, "POST", "/song/present/Amazing%20Grace/slide:1/display:2")
osv("present_song_from_folder", {"song": "Amazing Grace", "folder": "Hymns", "slide": 2, "display": "1"}, "POST",
    "/song/present/Amazing%20Grace/folder:Hymns/slide:2/display:1")
osv("load_set", {"set": "Sunday AM"}, "POST", "/set/load/Sunday%20AM")
osv("present_set", {"set": "Sunday AM", "slide": 4}, "POST", "/set/present/Sunday%20AM/slide:4/display:2")
B = "/presentation/slide/song"
osv("insert_song", {"song": "Amazing Grace"}, "POST", B + "/song:Amazing%20Grace")
osv("insert_song_from_folder", {"folder": "Hymns", "song": "Amazing Grace"}, "POST", B + "/folder:Hymns/song:Amazing%20Grace")
osv("insert_song_after", {"song": "Amazing Grace", "after": 0}, "POST", B + "/song:Amazing%20Grace/after:0")
osv("insert_song_from_folder_after", {"folder": "Hymns", "song": "Amazing Grace", "after": 5}, "POST",
    B + "/folder:Hymns/song:Amazing%20Grace/after:5")
osv("insert_song_in_order", {"song": "Amazing Grace", "order": "V1,C,V2"}, "POST", B + "/song:Amazing%20Grace/order:V1%2CC%2CV2")
osv("insert_song_from_folder_in_order", {"folder": "Hymns", "song": "Amazing Grace", "order": "V1,C"}, "POST",
    B + "/folder:Hymns/song:Amazing%20Grace/order:V1%2CC")
osv("insert_song_after_in_order", {"song": "Amazing Grace", "after": 2, "order": "V1"}, "POST",
    B + "/song:Amazing%20Grace/after:2/order:V1")
osv("insert_song_from_folder_after_in_order", {"folder": "Hymns", "song": "Amazing Grace", "after": 2, "order": "C"}, "POST",
    B + "/folder:Hymns/song:Amazing%20Grace/after:2/order:C")
osv("go_to_slide", {"item": 12}, "POST", "/presentation/slide/12")
for v, route in [("next_slide", "slide/next"), ("previous_slide", "slide/previous"), ("first_slide", "slide/first"),
                 ("last_slide", "slide/last"), ("next_section", "section/next"), ("previous_section", "section/previous"),
                 ("go_to_chorus", "song/current/chorus"), ("go_to_bridge", "song/current/bridge"),
                 ("go_to_prechorus", "song/current/prechorus"), ("go_to_tag", "song/current/tag"),
                 ("screen_normal", "screen/normal"), ("clear_alert", "screen/alert"), ("close_presentation", "close")]:
    osv(v, {}, "POST", f"/presentation/{route}")
osv("go_to_song", {"number": 3}, "POST", "/presentation/song/3")
osv("go_to_verse", {"verse": 2}, "POST", "/presentation/song/current/verse/verse:2")
for mode in ["black", "white", "hide", "logo", "freeze"]:
    osv(f"screen_{mode}", {}, "POST", f"/presentation/screen/{mode}")
    osv(f"toggle_screen_{mode}", {}, "POST", f"/presentation/screen/toggle_{mode}")
osv("show_alert", {"message": "Car HX12 lights on"}, "POST", "/presentation/screen/alert/message:Car%20HX12%20lights%20on")

telemetry(OS, "status-running", inbound_http={"path": "/presentation/status", "body": (
    '<?xml version="1.0" encoding="UTF-8"?>\n<response resource="presentation" action="status">\n'
    ' <presentation running="1">\n  <screen mode="N">normal</screen>\n'
    '  <slide itemnumber="1">\n   <name>Testslide</name>\n   <title>Untitled</title>\n  </slide>\n'
    ' </presentation>\n</response>')},
    expect_state={"presentation": {"running": True, "screen": {"mode": "N", "name": "normal"},
                                   "slide": {"item": 1, "name": "Testslide", "title": "Untitled"}}})
telemetry(OS, "status-stopped", inbound_http={"path": "/presentation/status", "body": (
    '<?xml version="1.0" encoding="UTF-8"?>\n<response resource="presentation" action="status">\n'
    ' <presentation running="0"/>\n</response>')},
    state_before={"presentation": {"running": True, "screen": {"mode": "N", "name": "normal"},
                                   "slide": {"item": 1, "name": "Testslide", "title": "Untitled"}}},
    expect_state={"presentation": {"running": False}})
# Downloads saved to the host file named (never written by a vector).
osv("save_slide_preview", {"item": 12, "to": {"file": "s12.jpg"}}, "GET", "/presentation/slide/12/preview")
osv("save_slide_image", {"item": 12, "to": {"file": "s12.jpg"}}, "GET", "/presentation/slide/12/image")

# The slide list, each slide shaped as the status's slide element.
telemetry(OS, "slide-list", inbound_http={"path": "/presentation/slide/list", "body":
    '<?xml version="1.0" encoding="UTF-8"?><response resource="presentation" action="slide" identifier="list">'
    '<slides><slide itemnumber="4"><name>V1</name><title>Amazing Grace</title></slide>'
    '<slide itemnumber="7"><name>C</name><title>Amazing Grace</title></slide></slides></response>'},
    state_before={"presentation": {"slides": {"2": {"name": "Old"}}}},
    expect_state={"presentation": {"slides": {"4": {"name": "V1", "title": "Amazing Grace"},
                                              "7": {"name": "C", "title": "Amazing Grace"}}}})
