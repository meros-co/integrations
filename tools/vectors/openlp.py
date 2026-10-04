# OpenLP (openlp): one vector per command over the HTTP API v2, from the
# OpenLP wiki's Documentation/HTTP-API page: routes under /api/v2, the
# arguments each endpoint lists sent as a JSON body (compact, written here with
# json.dumps), 204 for an action and 200 with JSON for a query. The websocket
# state from Documentation/WebSockets.
OL = "openlp"
_OL_DONE = {"http_reply": {"status": 204}, "expect_result": {"ok": {"kind": "ack"}}}


def _ol(command, input, method, route, body=None, **extra):
    request = {"method": method, "target": "/api/v2" + route}
    if body is not None:
        request["body"] = json.dumps(body, separators=(",", ":"))
    V.append({"spec": OL, "command": command, "input": input, "expect_request": request, **extra})


# ── Controller ───────────────────────────────────────────────────────────
_ol("get_live_items", {}, "GET", "/controller/live-items",
    http_reply={"status": 200, "body": "{}"}, expect_result={"ok": {"kind": "value", "value": {}}})
_ol("get_live_item", {}, "GET", "/controller/live-item")
# Slide 3 for the operator is OpenLP's index 2.
_ol("show_slide", {"slide": 3}, "POST", "/controller/show", {"id": 2}, **_OL_DONE)
_ol("next_slide", {}, "POST", "/controller/progress", {"action": "next"}, **_OL_DONE)
_ol("previous_slide", {}, "POST", "/controller/progress", {"action": "previous"})
_ol("get_theme_level", {}, "GET", "/controller/theme-level",
    http_reply={"status": 200, "body": '"global"'}, expect_result={"ok": {"kind": "value", "value": "global"}})
_ol("set_theme_level", {"level": "service"}, "POST", "/controller/theme-level", {"level": "service"})
_ol("list_themes", {}, "GET", "/controller/themes")
_ol("get_theme", {"name": "Blue Sky"}, "GET", "/controller/themes/Blue%20Sky")
_ol("get_live_theme", {}, "GET", "/controller/live-theme")
_ol("get_theme_name", {}, "GET", "/controller/theme")
_ol("set_theme", {"name": "Blue Sky"}, "POST", "/controller/theme", {"theme": "Blue Sky"},
    http_reply={"status": 501}, expect_result={"error": {"error": "device_error", "code": "501"}})
_ol("clear_live", {}, "POST", "/controller/clear/live", **_OL_DONE)
_ol("clear_preview", {}, "POST", "/controller/clear/preview")

# ── Core ─────────────────────────────────────────────────────────────────
_ol("set_display", {"mode": "desktop"}, "POST", "/core/display", {"display": "desktop"}, **_OL_DONE)
_ol("show_display", {}, "POST", "/core/display", {"display": "show"})
_ol("blank_display", {}, "POST", "/core/display", {"display": "blank"})
_ol("blank_to_theme", {}, "POST", "/core/display", {"display": "theme"})
_ol("show_desktop", {}, "POST", "/core/display", {"display": "desktop"})
_ol("hide_display", {}, "POST", "/core/display", {"display": "hide"})
_ol("list_plugins", {}, "GET", "/core/plugins")
_ol("get_system", {}, "GET", "/core/system")
_ol("login", {"username": "openlp", "password": "p\"w"}, "POST", "/core/login",
    {"username": "openlp", "password": "p\"w"},
    http_reply={"status": 200, "body": '{"token":"abc123"}'},
    expect_result={"ok": {"kind": "value", "value": "abc123"}})
_ol("get_live_image", {}, "GET", "/core/live-image",
    http_reply={"status": 200, "body": '{"binary_image":"data:image/png;base64,iVBORw0KGgo="}'},
    expect_result={"ok": {"kind": "value", "value": "data:image/png;base64,iVBORw0KGgo="}})

# ── Plugins ──────────────────────────────────────────────────────────────
_ol("search_plugin", {"plugin": "songs", "text": "amazing grace"}, "GET", "/plugins/songs/search?text=amazing%20grace")
_ol("add_plugin_item", {"plugin": "songs", "id": "42"}, "POST", "/plugins/songs/add", {"id": "42"}, **_OL_DONE)
_ol("go_live_plugin_item", {"plugin": "bibles", "id": "John 3:16"}, "POST", "/plugins/bibles/live", {"id": "John 3:16"})
_ol("get_search_options", {"plugin": "bibles"}, "GET", "/plugins/bibles/search-options")
_ol("set_search_option", {"plugin": "bibles", "option": "primary bible", "value": "KJV"}, "POST",
    "/plugins/bibles/search-options", {"option": "primary bible", "value": "KJV"})

# ── Service ──────────────────────────────────────────────────────────────
_ol("list_service_items", {}, "GET", "/service/items")
_ol("show_service_item", {"id": "6c1f2a"}, "POST", "/service/show", {"id": "6c1f2a"}, **_OL_DONE)
_ol("show_service_item_at", {"position": 2}, "POST", "/service/show", {"id": 2},
    http_reply={"status": 400}, expect_result={"error": {"error": "device_error", "code": "400"}})
_ol("next_service_item", {}, "POST", "/service/progress", {"action": "next"})
_ol("previous_service_item", {}, "POST", "/service/progress", {"action": "previous"})
_ol("new_service", {}, "GET", "/service/new", **_OL_DONE)

# ── Telemetry ────────────────────────────────────────────────────────────
telemetry(OL, "websocket-state", inbound_ws=json.dumps({"results": {
    "counter": 12, "service": 3, "slide": 1, "item": "6c1f2a", "twelve": True, "blank": False, "theme": True,
    "display": False, "version": 3, "isSecure": False, "chordNotation": "english"}}),
    expect_state={"live": {"counter": 12, "service_counter": 3, "slide": 1, "item_id": "6c1f2a", "blank": False,
                           "theme": True, "desktop": False, "twelve_hour": True, "auth_enabled": False,
                           "chord_notation": "english"}},
    # The websocket only counts changes: the live item and service are read.
    expect_then_send=[{"method": "GET", "target": "/api/v2/controller/live-item"},
                      {"method": "GET", "target": "/api/v2/service/items"}])
telemetry(OL, "nothing-live", inbound_http={"path": "/api/v2/controller/live-item", "body": "{}"},
          state_before={"live_item": {"title": "Amazing Grace", "slide_text": "Amazing grace"}, "theme": {"name": "x"}},
          expect_state={"theme": {"name": "x"}})
telemetry(OL, "live-item", inbound_http={"path": "/api/v2/controller/live-item", "body": json.dumps({
    "audit": ["Amazing Grace", "John Newton", "Public Domain", ""], "name": "songs", "notes": "",
    "theme": None, "title": "Amazing Grace", "type": "ServiceItemType.Text",
    "slides": [{"text": "Amazing grace! how sweet the sound", "tag": "V1", "selected": True,
                "html": "Amazing grace!", "chords": "", "footer": "", "title": "Amazing Grace"}]})},
    expect_state={"live_item": {"title": "Amazing Grace", "plugin": "songs", "type": "ServiceItemType.Text",
                                "notes": "", "slide_text": "Amazing grace! how sweet the sound", "slide_tag": "V1"}})
telemetry(OL, "service-items", inbound_http={"path": "/api/v2/service/items", "body": json.dumps([
    {"ccli_number": "", "id": "6c1f2a", "is_valid": True, "notes": "", "plugin": "songs", "selected": True,
     "title": "Amazing Grace"},
    {"ccli_number": "", "id": "9d0e1b", "is_valid": False, "notes": "Play from 0:30", "plugin": "media",
     "selected": False, "title": "Intro video"}])},
    expect_state={"service": {"items": {
        "6c1f2a": {"title": "Amazing Grace", "plugin": "songs", "live": True, "valid": True, "notes": ""},
        "9d0e1b": {"title": "Intro video", "plugin": "media", "live": False, "valid": False,
                   "notes": "Play from 0:30"}}}})
telemetry(OL, "theme-name", inbound_http={"path": "/api/v2/controller/theme", "body": '"Blue Sky"'},
          expect_state={"theme": {"name": "Blue Sky"}})
telemetry(OL, "system", inbound_http={"path": "/api/v2/core/system", "body": json.dumps(
    {"login_required": True, "websocket_port": 4317, "api_version": 2, "api_revision": 4})},
    expect_state={"system": {"login_required": True, "websocket_port": 4317, "api_version": 2, "api_revision": 4}})
