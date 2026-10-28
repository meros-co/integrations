# Holographics (holographics): one vector per command, over the REST API on
# port 3000. Routes are those of the vendor SDK's EXAMPLES.md (trailing
# slashes kept), bodies the call's data as compact JSON, values from the
# Swagger page's examples (widget WSQIcdZzNOyxIymR, entry VjcxSjSQPCOFuBli).
HG = "holographics"
_W, _E, _P = "WSQIcdZzNOyxIymR", "VjcxSjSQPCOFuBli", "imHHRyynPPVwCANw"


def _hg(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": HG, "command": command, "input": input, "expect_request": request, **extra})


_hg("get_status", {}, "GET", "/api/status/", http_reply={"status": 200, "body": '{"ip":"192.168.2.7","version":"1.2.0-alpha.1"}'},
    expect_result={"ok": {"kind": "value", "value": {"ip": "192.168.2.7", "version": "1.2.0-alpha.1"}}})
_hg("get_state", {}, "GET", "/api/state/")
_hg("replace_state", {"data": {"style": {"activeTheme": "default.styl"}}}, "PUT", "/api/state/", '{"style":{"activeTheme":"default.styl"}}')
_hg("patch_state", {"data": {"style": {"activeTheme": "3D.styl"}}}, "PATCH", "/api/state/", '{"style":{"activeTheme":"3D.styl"}}',
    http_reply={"status": 200, "body": "{}"}, expect_result={"ok": {"kind": "ack"}})
_hg("reset_state", {}, "DELETE", "/api/state/")
_hg("set_theme", {"theme": "3D.styl"}, "PATCH", "/api/state/", '{"style":{"activeTheme":"3D.styl"}}')
_hg("set_style", {"key": "canvas_bg", "value": "rgba(255, 255, 255, 0)"}, "PATCH", "/api/state/",
    '{"style":{"canvas_bg":"rgba(255, 255, 255, 0)"}}')
_hg("list_themes", {}, "GET", "/api/themes/")
_hg("get_theme", {"theme": "default.styl"}, "GET", "/api/themes/default.styl")
_hg("save_theme", {"theme": "my.styl", "contents": "// Theme\nbody\n  color red"}, "PUT", "/api/themes/my.styl",
    '{"contents":"// Theme\\nbody\\n  color red"}')
_hg("delete_theme", {"theme": "my.styl"}, "DELETE", "/api/themes/my.styl",
    http_reply={"status": 404, "body": '{"name":"NotFoundError","code":404,"message":"not found","className":"warning"}'},
    expect_result={"error": {"error": "device_error", "code": "404"}})
_hg("validate_theme", {"stylus": "// Validate me"}, "POST", "/api/themes/validate", '{"stylus":"// Validate me"}')
_hg("reset_themes", {}, "GET", "/api/themes/reset")
_hg("list_animations", {}, "GET", "/api/animations/")
_hg("list_media_files", {}, "GET", "/api/mediafiles/")
_hg("list_fonts", {}, "GET", "/api/fonts/")
_hg("get_log", {"level": "HIGH", "type": "ERROR", "limit": 50}, "GET", "/api/log/?level=HIGH&type=ERROR&limit=50")
_hg("list_palettes", {}, "GET", "/api/palettes/")
_hg("get_palette", {"id": _P}, "GET", f"/api/palettes/{_P}")
_hg("create_palette", {"name": "Hello world", "colors": ["#8dccad", "#988864"]}, "POST", "/api/palettes/",
    '{"name":"Hello world","colors":["#8dccad","#988864"]}')
_hg("update_palette", {"id": _P, "data": {"name": "Hello world", "colors": ["#fea6a2"]}}, "PUT", f"/api/palettes/{_P}",
    '{"name":"Hello world","colors":["#fea6a2"]}')
_hg("patch_palette", {"id": _P, "data": {"name": "Brand"}}, "PATCH", f"/api/palettes/{_P}", '{"name":"Brand"}')
_hg("delete_palette", {"id": _P}, "DELETE", f"/api/palettes/{_P}")
_hg("list_widgets", {}, "GET", "/api/widgets/")
_hg("get_widget", {"id": _W}, "GET", f"/api/widgets/{_W}")
_hg("create_widget", {"data": {"type": "Clock"}}, "POST", "/api/widgets/", '{"type":"Clock"}')
_hg("update_widget", {"id": _W, "data": {"type": "BroadcastMessage", "name": "Hello world"}}, "PUT", f"/api/widgets/{_W}",
    '{"type":"BroadcastMessage","name":"Hello world"}')
_hg("patch_widget", {"id": _W, "data": {"name": "Hello world"}}, "PATCH", f"/api/widgets/{_W}", '{"name":"Hello world"}')
_hg("delete_widget", {"id": _W}, "DELETE", f"/api/widgets/{_W}")
_hg("toggle_widget", {"id": _W, "visible": True}, "PUT", f"/api/widgets/{_W}/toggle", '{"visibility":true}',
    http_reply={"status": 200, "body": '{"type":"Clock","props":{"visibility":"true"},"id":"WSQIcdZzNOyxIymR"}'},
    expect_result={"ok": {"kind": "ack"}})
_hg("list_entries", {"widget": "PgxwQYfeEDjruMyB"}, "GET", "/api/entries/", '{"widgetId":"PgxwQYfeEDjruMyB"}')
_hg("get_entry", {"id": _E}, "GET", f"/api/entries/{_E}")
_hg("create_entry", {"widget": "PgxwQYfeEDjruMyB", "props": {"message": "Test 123"}, "visible": True}, "POST", "/api/entries/",
    '{"widgetId":"PgxwQYfeEDjruMyB","props":{"message":"Test 123"},"visibility":true}')
_hg("update_entry", {"id": _E, "data": {"widgetId": "PgxwQYfeEDjruMyB", "props": {"message": "Hi"}, "visibility": False}},
    "PUT", f"/api/entries/{_E}", '{"widgetId":"PgxwQYfeEDjruMyB","props":{"message":"Hi"},"visibility":false}')
_hg("patch_entry", {"id": _E, "data": {"props": {"message": "Hello"}}}, "PATCH", f"/api/entries/{_E}", '{"props":{"message":"Hello"}}')
_hg("set_entry_visibility", {"id": _E, "visible": True}, "PATCH", f"/api/entries/{_E}", '{"visibility":true}')
_hg("delete_entry", {"id": _E}, "DELETE", f"/api/entries/{_E}")
_hg("list_widget_modules", {}, "GET", "/api/widgetmodules/")
_hg("reset_widget_modules", {}, "GET", "/api/widgetmodules/reset")
_hg("get_settings", {}, "GET", "/api/settings/")
_hg("patch_settings", {"data": {"updates": {"channel": "beta"}}}, "PATCH", "/api/settings/", '{"updates":{"channel":"beta"}}')
_hg("set_logging_level", {"level": "debug"}, "PATCH", "/api/settings/", '{"logging":{"level":"debug"}}')
_hg("set_update_channel", {"channel": "latest"}, "PATCH", "/api/settings/", '{"updates":{"channel":"latest"}}')
_hg("check_update", {}, "GET", "/api/update/")
_hg("download_update", {}, "GET", "/api/update/download")
_hg("cancel_update_download", {}, "GET", "/api/update/cancelDownload")
_hg("install_update", {}, "GET", "/api/update/install")
_hg("get_license", {}, "GET", "/api/license/")
_hg("validate_license", {}, "GET", "/api/license/validate")

# Telemetry: the getState example of the Swagger page, read and pushed.
_HG_STATE = {
    "style": {"activeTheme": "default.styl", "primary_color": "rgb(69, 151, 251)", "secondary_color": "rgb(69, 151, 251)",
              "background_a": "rgb(38, 38, 38)", "background_b": "rgb(79, 79, 79)", "text_a": "rgb(219, 219, 219)",
              "text_b": "rgb(0, 0, 0)", "global_padding": "8", "widget_padding": "2", "font_size": "3",
              "canvas_bg": "rgba(255, 255, 255, 0)"},
    "widgets": [{"type": "BroadcastMessage", "name": "", "style": {}, "props": {}, "id": "PgxwQYfeEDjruMyB"},
                {"type": "Clock", "name": "Clock", "props": {"timeformat": "12H", "visibility": "false"}, "id": "aGcERhDaPQEfpawZ"}],
    "entries": [{"widget_id": "PgxwQYfeEDjruMyB", "props": {"message": "Test 123"}, "visibility": True, "id": "VjcxSjSQPCOFuBli"}]}
_HG_EXPECT = {
    "style": {"active_theme": "default.styl", "primary_color": "rgb(69, 151, 251)", "secondary_color": "rgb(69, 151, 251)",
              "background_a": "rgb(38, 38, 38)", "background_b": "rgb(79, 79, 79)", "text_a": "rgb(219, 219, 219)",
              "text_b": "rgb(0, 0, 0)", "global_padding": "8", "widget_padding": "2", "font_size": "3",
              "canvas_bg": "rgba(255, 255, 255, 0)"},
    "widgets": {"PgxwQYfeEDjruMyB": {"type": "BroadcastMessage", "name": ""},
                "aGcERhDaPQEfpawZ": {"type": "Clock", "name": "Clock", "visible": False}},
    "entries": {"VjcxSjSQPCOFuBli": {"widget": "PgxwQYfeEDjruMyB", "visible": True, "props": '{"message":"Test 123"}'}}}
telemetry(HG, "state", inbound_http={"path": "/api/state/", "body": json.dumps(_HG_STATE)},
          state_before={"widgets": {"gone": {"type": "Image"}}}, expect_state=_HG_EXPECT)
telemetry(HG, "state-changed", inbound_ws="42" + json.dumps(["stateChanged", _HG_STATE], separators=(",", ":")),
          expect_state=_HG_EXPECT)
telemetry(HG, "status", inbound_http={"path": "/api/status/", "body": '{"ip":"192.168.2.7","version":"1.2.0-alpha.1"}'},
          expect_state={"system": {"ip": "192.168.2.7", "version": "1.2.0-alpha.1"}})
