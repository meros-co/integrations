# SPX Graphics Controller (spx-graphics): one vector per command. Targets are
# written by hand from SPX's API page (GET /api/v1, routes/routes-api-v1.js
# of SPX 1.4.1): each call's path and example query, with the apikey
# parameter SPX reads on GET (empty when SPX has no key) and in the body on
# POST. Query values are percent-encoded as RFC 3986 query components.
SPX = "spx-graphics"
_K = {"apikey": "s3cret"}


def _spx(command, input, method, target, body=None, model=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    v = {"spec": SPX, "command": command, "input": input, "expect_request": request, **extra}
    if model:
        v["model"] = model
    V.append(v)


_ACK = {"http_reply": {"status": 200, "body": '{"info":"Sent request to SPX Controller. Acknowledgement is not to be expected.","APIcmd":"ItemPlay"}'},
        "expect_result": {"ok": {"kind": "ack"}}}

# Common API.
_spx("get_version", {}, "GET", "/api/v1/version",
     http_reply={"status": 200, "body": '{"vendor":"SPX Graphics","product":"SPX Solo","version":"1.4.1"}'},
     expect_result={"ok": {"kind": "value", "value": {"vendor": "SPX Graphics", "product": "SPX Solo", "version": "1.4.1"}}})
_spx("panic", {}, "GET", "/api/v1/panic?apikey=")
_spx("feed_proxy", {"url": "https://feeds.bbci.co.uk/news/rss.xml", "format": "xml"}, "GET",
     "/api/v1/feedproxy?url=https%3A%2F%2Ffeeds.bbci.co.uk%2Fnews%2Frss.xml&format=xml&apikey=")
# The page's own example body, with the API key added as SPX reads it on POST.
_spx("feed_proxy_post", {"url": "https://api.endpoint.com/requiring/customheaders/",
                         "headers": {"key1": "my first value"}, "post_body": '{"a":1}'},
     "POST", "/api/v1/feedproxy",
     '{"url":"https://api.endpoint.com/requiring/customheaders/","headers":{"key1":"my first value"},'
     '"postBody":"{\\"a\\":1}","apikey":"s3cret"}', settings=_K)

# Controller API.
_spx("focus_previous", {}, "GET", "/api/v1/rundown/focusPrevious?apikey=")
_spx("focus_next", {}, "GET", "/api/v1/rundown/focusNext?apikey=s3cret", settings=_K, **_ACK)
_spx("focus_first", {}, "GET", "/api/v1/rundown/focusFirst?apikey=")
_spx("focus_last", {}, "GET", "/api/v1/rundown/focusLast?apikey=")
_spx("focus_item", {"item": "1234567890"}, "GET", "/api/v1/rundown/focusByID/1234567890?apikey=")
_spx("stop_all_layers", {}, "GET", "/api/v1/rundown/stopAllLayers?apikey=")
_spx("play", {}, "GET", "/api/v1/item/play?apikey=", **_ACK)
_spx("continue", {}, "GET", "/api/v1/item/continue?apikey=")
_spx("stop", {}, "GET", "/api/v1/item/stop?apikey=")
_spx("load_rundown", {"project": "MyFirstProject", "rundown": "MyFirstRundown"}, "GET",
     "/api/v1/rundown/load?file=MyFirstProject%2FMyFirstRundown&apikey=")
_spx("play_item", {"item": "1234567890"}, "GET", "/api/v1/item/play/1234567890?apikey=", model="spx-production")
_spx("continue_item", {"item": "1234567890"}, "GET", "/api/v1/item/continue/1234567890?apikey=", model="spx-production")
_spx("stop_item", {"item": "1234567890"}, "GET", "/api/v1/item/stop/1234567890?apikey=", model="spx-production")
# The page's example: playserver=OVERLAY&playchannel=1&playlayer=19&webplayout=19&function=myCustomTemplateFunction&params=Hello%20World
_spx("invoke_template_function", {"playlayer": 19, "webplayout": 19, "function": "myCustomTemplateFunction",
                                  "params": "Hello World"}, "GET",
     "/api/v1/invokeTemplateFunction?playserver=OVERLAY&playchannel=1&playlayer=19&webplayout=19"
     "&function=myCustomTemplateFunction&params=Hello%20World&apikey=", model="spx-production")
_spx("invoke_extension_function", {"function": "sendCmd", "params": "incrementNumber"}, "GET",
     "/api/v1/invokeExtensionFunction?function=sendCmd&params=incrementNumber&apikey=", model="spx-production")

# Server API, with the page's example values.
_DF = [{"field": "f0", "value": "Firstname"}, {"field": "f1", "value": "Lastname"}]
_DF_JSON = '[{"field":"f0","value":"Firstname"},{"field":"f1","value":"Lastname"}]'
_spx("direct_playout", {"layer": 20, "web_layer": 20, "template": "/vendor/pack/template.html",
                        "fields": _DF, "command": "play"}, "POST", "/api/v1/directplayout",
     '{"casparServer":"OVERLAY","casparChannel":"1","casparLayer":"20","webplayoutLayer":"20",'
     '"relativeTemplatePath":"/vendor/pack/template.html","out":"manual","DataFields":' + _DF_JSON
     + ',"command":"play","apikey":""}', model="spx-broadcast")
_spx("direct_playout_update_item", {"layer": 20, "web_layer": 20, "template": "/vendor/pack/template.html",
                                    "fields": _DF, "command": "play", "item": "myItemID",
                                    "project": "myFirstProject", "rundown": "myFirstRundown"},
     "POST", "/api/v1/directplayout",
     '{"casparServer":"OVERLAY","casparChannel":"1","casparLayer":"20","webplayoutLayer":"20",'
     '"relativeTemplatePath":"/vendor/pack/template.html","out":"manual","DataFields":' + _DF_JSON
     + ',"command":"play","updateRundownItem":{"updateUI":true,"itemID":"myItemID","persist":true,'
     '"project":"myFirstProject","rundown":"myFirstRundown"},"apikey":""}', model="spx-broadcast")
_spx("control_rundown_item", {"project": "MyProject", "rundown": "FirstRundown", "item": "1234567890",
                              "command": "play"}, "GET",
     "/api/v1/controlRundownItemByID?file=MyProject%2FFirstRundown&item=1234567890&command=play&apikey=",
     model="spx-broadcast")
_spx("get_projects", {}, "GET", "/api/v1/getprojects?apikey=", model="spx-broadcast",
     http_reply={"status": 200, "body": '["MyProject","News"]'},
     expect_result={"ok": {"kind": "value", "value": ["MyProject", "News"]}})
_spx("get_rundowns", {"project": "MyProject"}, "GET", "/api/v1/getrundowns?project=MyProject&apikey=",
     model="spx-broadcast")
_spx("get_all_rundowns", {}, "GET", "/api/v1/allrundowns?apikey=", model="spx-broadcast")
_spx("get_rundown", {}, "GET", "/api/v1/rundown/get?apikey=", model="spx-broadcast")
_spx("get_rundown_file", {"project": "MyProject", "rundown": "FirstRundown"}, "GET",
     "/api/v1/rundown/json?project=MyProject&rundown=FirstRundown&apikey=", model="spx-broadcast")
_spx("save_rundown_file", {"project": "myProjectName", "file": "newRundown.json",
                           "content": {"comment": "Playlist generated by MyApp", "templates": []}},
     "POST", "/api/v1/rundown/json",
     '{"project":"myProjectName","file":"newRundown.json","content":{"comment":"Playlist generated by MyApp",'
     '"templates":[]},"apikey":""}', model="spx-broadcast")
_spx("get_layer_state", {}, "GET", "/api/v1/getlayerstate?apikey=", model="spx-broadcast")
_spx("get_templates", {"project": "MyProject"}, "GET", "/api/v1/gettemplates?project=MyProject&apikey=",
     model="spx-broadcast")
_spx("execute_script", {"file": "win-open-calculator.bat"}, "GET",
     "/api/v1/executeScript?file=win-open-calculator.bat&apikey=", model="spx-broadcast")
_spx("get_file_list", {"folder": "excel"}, "GET", "/api/v1/getFileList?assetsfolder=excel&apikey=",
     model="spx-broadcast")
_spx("save_custom_json", {"subfolder": "todoApp", "filename": "myData.json",
                          "content": {"note": "Get these done by the end of month"}},
     "POST", "/api/v1/saveCustomJSON",
     '{"subfolder":"todoApp","filename":"myData.json","content":{"note":"Get these done by the end of month"},'
     '"apikey":""}', model="spx-broadcast")

# Telemetry. The version reply's fields are those SPX's /version handler
# writes; the rundown is SPX's rundown file shape (templates with itemID,
# onair as text, DataFields), values made up.
telemetry(SPX, "version", inbound_http={"path": "/api/v1/version", "body": json.dumps(
    {"vendor": "SPX Graphics", "product": "SPX Broadcast", "version": "1.4.1", "id": "a1b2c3", "hostname": "gfx1",
     "os": "win32", "rootFolder": "C:\\SPX", "uptime": {"seconds": 3600, "text": "1 hour"}})},
    expect_state={"system": {"product": "SPX Broadcast", "version": "1.4.1", "host_id": "a1b2c3",
                             "hostname": "gfx1", "os": "win32", "uptime_s": 3600}})
telemetry(SPX, "rundown", model="spx-broadcast",
          inbound_http={"path": "/api/v1/rundown/get?apikey=", "body": json.dumps({
              "comment": "Show", "templates": [
                  {"itemID": "1001", "onair": "true", "description": "Name strap", "playserver": "OVERLAY",
                   "playchannel": "1", "playlayer": "20", "webplayout": "20",
                   "relpath": "softpix/Template_Pack_1.3/SPX1_INFO_LEFT.html", "out": "manual",
                   "DataFields": [{"field": "f0", "ftype": "textfield", "value": "Jane Doe"},
                                  {"field": "f1", "ftype": "textfield", "value": "Host"}]},
                  {"itemID": "1002", "onair": "false", "description": "Bug", "playserver": "-",
                   "playchannel": "1", "playlayer": "5", "webplayout": "5", "relpath": "bug.html", "out": "4000",
                   "DataFields": []}]})},
          state_before={"rundown": {"items": {"0999": {"description": "Removed"}}}},
          expect_state={"rundown": {"items": {
              "1001": {"index": 1, "on_air": True, "description": "Name strap", "server": "OVERLAY", "channel": "1",
                       "layer": "20", "web_layer": "20", "template": "softpix/Template_Pack_1.3/SPX1_INFO_LEFT.html",
                       "out": "manual", "fields": {"f0": "Jane Doe", "f1": "Host"}},
              "1002": {"index": 2, "on_air": False, "description": "Bug", "server": "-", "channel": "1", "layer": "5",
                       "web_layer": "5", "template": "bug.html", "out": "4000"}}}})
# Solo's 501 answer to a commercial read leaves state alone.
telemetry(SPX, "not-in-solo", inbound_http={"path": "/api/v1/rundown/get?apikey=", "body": json.dumps(
    {"status": 501, "message": "Not Implemented", "info": "This API endpoint is not available in SPX Solo."})},
    expect_state={})
telemetry(SPX, "projects", model="spx-broadcast",
          inbound_http={"path": "/api/v1/getprojects?apikey=", "body": '["MyProject","News"]'},
          state_before={"projects": {"Old": True}},
          expect_state={"projects": {"MyProject": True, "News": True}},
          expect_then_send=[{"method": "GET", "target": "/api/v1/getrundowns?project=MyProject&apikey="},
                            {"method": "GET", "target": "/api/v1/getrundowns?project=News&apikey="}])

# Each project's rundowns, read after the project list.
telemetry(SPX, "rundowns", inbound_http={"path": "/api/v1/getrundowns?project=My%20Show&apikey=", "body": '["Opening", "News"]'},
          state_before={"rundowns": {"My Show": {"Old": True}}},
          expect_state={"rundowns": {"My Show": {"Opening": True, "News": True}}})
telemetry(SPX, "rundown-loaded", inbound_http={"path": "/api/v1/rundown/load?file=MyFirstProject%2FMyFirstRundown&apikey=",
          "body": '{"info": "Sent request to SPX Controller.", "APIcmd": "RundownLoad", "file": "MyFirstProject/MyFirstRundown", "apikey": ""}'},
          expect_state={"rundown": {"project": "MyFirstProject", "name": "MyFirstRundown"}})
