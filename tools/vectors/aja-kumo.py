KUMO = "aja-kumo"
# AJA KUMO routers: GET /config?action=get|set&paramid=...&value=... (KUMO REST
# Automation 4.7.1: kumo_control.h, main.cpp, python_examples; AJA REST API
# chapters 1-2). Query values are percent-encoded, keeping only RFC 3986
# unreserved characters. Reply bodies follow AJA's documented JSON reply
# {paramid, name, value, value_name}; the paramid numbers in them are
# placeholders, AJA publishes none for KUMO.

_KSET = "/config?action=set&paramid="
_KGET = "/config?action=get&paramid="


def _kreply(name, value, value_name=None):
    vn = value if value_name is None else value_name
    return ('{"paramid":"1","name":"%s","value":"%s","value_name":"%s"}' % (name, value, vn))


# Generic access (REST API chapters 1-2; config_control.h).
http(KUMO, "get_parameter", {"paramid": "eParamID_KumoProductID"}, "GET", _KGET + "eParamID_KumoProductID",
     http_reply={"status": 200, "body": _kreply("eParamID_KumoProductID", "3", "KUMO 32x32 Matrix")},
     expect_result={"ok": {"kind": "value", "value": {
         "paramid": "1", "name": "eParamID_KumoProductID", "value": "3", "value_name": "KUMO 32x32 Matrix"}}})
http(KUMO, "set_parameter", {"paramid": "eParamID_SuppressPSAlarm", "value": "1"}, "GET",
     _KSET + "eParamID_SuppressPSAlarm&value=1",
     http_reply={"status": 404, "body": "<html><head><title>404 Not Found</title></head></html>"},
     expect_result={"error": {"error": "device_error", "code": "404"}})
http(KUMO, "get_descriptor", {"paramid": "eParamID_XPT_Destination1_Status"}, "GET",
     "/descriptors?paramid=eParamID_XPT_Destination1_Status")
http(KUMO, "get_all_descriptors", {}, "GET", "/desc.json")
http(KUMO, "save_preset", {"preset": 20}, "GET", _KSET + "eParamID_RegisterSave&value=20")
http(KUMO, "recall_preset", {"preset": 3}, "GET", _KSET + "eParamID_RegisterRecall&value=3",
     http_reply={"status": 200, "body": _kreply("eParamID_RegisterRecall", "3")},
     expect_result={"ok": {"kind": "ack"}})
# config_control main.cpp: eParamID_LED_Identify "1", then "0".
http(KUMO, "identify_on", {}, "GET", _KSET + "eParamID_LED_Identify&value=1")
http(KUMO, "identify_off", {}, "GET", _KSET + "eParamID_LED_Identify&value=0")
http(KUMO, "get_product_id", {}, "GET", _KGET + "eParamID_KumoProductID",
     http_reply={"status": 200, "body": _kreply("eParamID_KumoProductID", "2", "KUMO 16x4 Matrix")},
     expect_result={"ok": {"kind": "value", "value": "KUMO 16x4 Matrix"}})
# python_examples: action=connect returns {"connectionid": ...}.
http(KUMO, "open_event_connection", {}, "GET", "/config?action=connect",
     http_reply={"status": 200, "body": '{"connectionid":"20"}'},
     expect_result={"ok": {"kind": "value", "value": "20"}})
http(KUMO, "wait_for_events", {"connection": 20}, "GET",
     "/config?action=wait_for_config_events&configid=0&connectionid=20",
     http_reply={"status": 200, "body":
                 '[{"param_id":"eParamID_XPT_Destination1_Status","int_value":5,"str_value":""}]'},
     expect_result={"ok": {"kind": "value", "value": [
         {"param_id": "eParamID_XPT_Destination1_Status", "int_value": 5, "str_value": ""}]}})

# Crosspoints (kumo_control.h SetCrosspoint/GetCrosspoint; main.cpp).
# OneToOneDiagonal_CL.py: HTTP 403 for an invalid or locked switch.
http(KUMO, "set_route", {"destination": 1, "source": 15}, "GET",
     _KSET + "eParamID_XPT_Destination1_Status&value=15",
     http_reply={"status": 403, "body": "Forbidden"},
     expect_result={"error": {"error": "device_error", "code": "403"}})
http(KUMO, "get_route", {"destination": 1}, "GET", _KGET + "eParamID_XPT_Destination1_Status",
     http_reply={"status": 200, "body": _kreply("eParamID_XPT_Destination1_Status", "16")},
     expect_result={"ok": {"kind": "value", "value": "16"}})
# kumo_control.h GetAllCrosspointsAsText, its own 1616 example.
http(KUMO, "get_all_routes", {}, "GET",
     "/options?action=get&configid=0&paramid=eParamID_XPT_DestinationAll_Status&alt=text-plain",
     http_reply={"status": 200, "body": "[1,2,4,4,5,6,7,8,10,10,11,12,13,15,15,16]"},
     expect_result={"ok": {"kind": "value", "value": "[1,2,4,4,5,6,7,8,10,10,11,12,13,15,15,16]"}})

# Button names: main.cpp "Hello"/"World" on destination 1; ChangeTextSources.py
# "Quad 1" on source 61.
http(KUMO, "set_destination_label", {"destination": 1, "line": 1, "text": "Hello"}, "GET",
     _KSET + "eParamID_XPT_Destination1_Line_1&value=Hello")
http(KUMO, "get_destination_label", {"destination": 1, "line": 2}, "GET",
     _KGET + "eParamID_XPT_Destination1_Line_2",
     http_reply={"status": 200, "body": _kreply("eParamID_XPT_Destination1_Line_2", "World", "")},
     expect_result={"ok": {"kind": "value", "value": "World"}})
http(KUMO, "set_source_label", {"source": 61, "line": 1, "text": "Quad 1"}, "GET",
     _KSET + "eParamID_XPT_Source61_Line_1&value=Quad%201")
http(KUMO, "get_source_label", {"source": 64, "line": 2}, "GET", _KGET + "eParamID_XPT_Source64_Line_2")

# Locks (community: Companion module, 1 lock / 0 unlock).
http(KUMO, "lock_destination", {"destination": 4}, "GET", _KSET + "eParamID_XPT_Destination4_Locked&value=1")
http(KUMO, "unlock_destination", {"destination": 4}, "GET", _KSET + "eParamID_XPT_Destination4_Locked&value=0")
http(KUMO, "get_destination_lock", {"destination": 4}, "GET", _KGET + "eParamID_XPT_Destination4_Locked",
     http_reply={"status": 200, "body": _kreply("eParamID_XPT_Destination4_Locked", "1")},
     expect_result={"ok": {"kind": "value", "value": "1"}})

# Salvos: kumo_control.h SetSalvo, as main.cpp's DemoSalvos sets salvo 1
# (sources 1-16 for destinations 1-16, 0 for the rest), as a form POST.
_kdemo = list(range(1, 17)) + [0] * 48
V.append({"spec": KUMO, "command": "set_salvo",
          "input": {"salvo": 1, "name": "Salvo 1", "valid": True,
                    "sources": ",".join(str(s) for s in _kdemo)},
          "expect_request": {"method": "POST", "target": "/options",
                             "body": 'paramName=eParamID_Salvo1&newValue={"index":"1","name":"Salvo 1",'
                                     '"valid":"true","salvodata":[' + ",".join(str(s) for s in _kdemo) + "]}"}})
_ksalvo2 = '{"index":"2","name":"Wide","valid":"true","salvodata":[' + ",".join(["3"] + ["0"] * 63) + "]}"
http(KUMO, "get_salvo", {"salvo": 2}, "GET", _KGET + "eParamID_Salvo2",
     http_reply={"status": 200, "body":
                 '{"paramid":"1","name":"eParamID_Salvo2","value":' + _ksalvo2 + '}'},
     expect_result={"ok": {"kind": "value", "value": {
         "index": "2", "name": "Wide", "valid": "true", "salvodata": [3] + [0] * 63}}})
http(KUMO, "take_salvo", {"salvo": 1}, "GET", _KSET + "eParamID_TakeSalvo&value=1")

# Button colors: {'classes': 'color_N'} as AJA's scripts send it, percent-
# encoded; button numbering from ConfigureSources.py and
# GroupChangeColorDests_CL.py.
_KCOLOR = "&value=%7B%27classes%27%3A%20%27{}%27%7D"
http(KUMO, "set_source_color_1_16", {"source": 4, "color": "color_7"}, "GET",
     _KSET + "eParamID_Button_Settings_4" + _KCOLOR.format("color_7"))
http(KUMO, "set_source_color_17_32", {"source": 17, "color": "color_2"}, "GET",
     _KSET + "eParamID_Button_Settings_33" + _KCOLOR.format("color_2"))
# ConfigureSources.py: sources 61-64 red; 61 is button 93.
http(KUMO, "set_source_color_33_64", {"source": 61, "color": "color_1"}, "GET",
     _KSET + "eParamID_Button_Settings_93" + _KCOLOR.format("color_1"))
http(KUMO, "set_destination_color_1_16", {"destination": 1, "color": "color_4"}, "GET",
     _KSET + "eParamID_Button_Settings_17" + _KCOLOR.format("color_4"))
http(KUMO, "set_destination_color_17_32", {"destination": 32, "color": "color_5"}, "GET",
     _KSET + "eParamID_Button_Settings_64" + _KCOLOR.format("color_5"))
http(KUMO, "set_destination_color_33_64", {"destination": 64, "color": "color_9"}, "GET",
     _KSET + "eParamID_Button_Settings_128" + _KCOLOR.format("color_9"))

# Router information.
http(KUMO, "get_source_count", {}, "GET", _KGET + "eParamID_NumberOfSources",
     http_reply={"status": 200, "body": _kreply("eParamID_NumberOfSources", "32")},
     expect_result={"ok": {"kind": "value", "value": "32"}})
http(KUMO, "get_destination_count", {}, "GET", _KGET + "eParamID_NumberOfDestinations")
http(KUMO, "get_ps_alarm_suppression", {}, "GET", _KGET + "eParamID_SuppressPSAlarm")

# Telemetry.
telemetry(KUMO, "route-get", inbound_http={
    "path": _KGET + "eParamID_XPT_Destination12_Status",
    "body": _kreply("eParamID_XPT_Destination12_Status", "7")},
    expect_state={"params": {"eParamID_XPT_Destination12_Status": {"value": "7", "value_name": "7"}},
                  "outputs": {"12": {"input": 7}}})
telemetry(KUMO, "route-set-reply", inbound_http={
    "path": _KSET + "eParamID_XPT_Destination1_Status&value=14",
    "body": _kreply("eParamID_XPT_Destination1_Status", "14")},
    expect_state={"params": {"eParamID_XPT_Destination1_Status": {"value": "14", "value_name": "14"}},
                  "outputs": {"1": {"input": 14}}})
_KALL = "/options?action=get&configid=0&paramid=eParamID_XPT_DestinationAll_Status&alt=text-plain"
telemetry(KUMO, "all-routes-16", inbound_http={"path": _KALL,
                                            "body": "[1,2,4,4,5,6,7,8,10,10,11,12,13,15,15,16]"},
          expect_state={"outputs": {str(d): {"input": s} for d, s in
                                    enumerate([1, 2, 4, 4, 5, 6, 7, 8, 10, 10, 11, 12, 13, 15, 15, 16], 1)}})
telemetry(KUMO, "all-routes-4", inbound_http={"path": _KALL, "body": "[16,1,9,3]"},
          expect_state={"outputs": {"1": {"input": 16}, "2": {"input": 1}, "3": {"input": 9},
                                    "4": {"input": 3}}})
telemetry(KUMO, "all-routes-1", inbound_http={"path": _KALL, "body": "[2]"},
          expect_state={"outputs": {"1": {"input": 2}}})
telemetry(KUMO, "all-routes-2", inbound_http={"path": _KALL, "body": "[3,4]"},
          expect_state={"outputs": {"1": {"input": 3}, "2": {"input": 4}}})
telemetry(KUMO, "all-routes-8", inbound_http={"path": _KALL, "body": "[8,7,6,5,4,3,2,1]"},
          expect_state={"outputs": {str(d): {"input": 9 - d} for d in range(1, 9)}})
telemetry(KUMO, "all-routes-32", inbound_http={"path": _KALL,
                                            "body": "[" + ",".join(str(33 - d) for d in range(1, 33)) + "]"},
          expect_state={"outputs": {str(d): {"input": 33 - d} for d in range(1, 33)}})
telemetry(KUMO, "all-routes-64", inbound_http={"path": _KALL,
                                            "body": "[" + ",".join(str(d) for d in range(1, 65)) + "]"},
          expect_state={"outputs": {str(d): {"input": d} for d in range(1, 65)}})
telemetry(KUMO, "all-routes-unsupported", inbound_http={"path": _KALL, "body": "{}"}, expect_state={})
telemetry(KUMO, "destination-label-1", inbound_http={
    "path": _KGET + "eParamID_XPT_Destination3_Line_1",
    "body": _kreply("eParamID_XPT_Destination3_Line_1", "FS-HDR", "")},
    expect_state={"params": {"eParamID_XPT_Destination3_Line_1": {"value": "FS-HDR", "value_name": ""}},
                  "outputs": {"3": {"label_line_1": "FS-HDR"}}})
telemetry(KUMO, "destination-label-2", inbound_http={
    "path": _KSET + "eParamID_XPT_Destination1_Line_2&value=World",
    "body": _kreply("eParamID_XPT_Destination1_Line_2", "World", "")},
    expect_state={"params": {"eParamID_XPT_Destination1_Line_2": {"value": "World", "value_name": ""}},
                  "outputs": {"1": {"label_line_2": "World"}}})
telemetry(KUMO, "source-label-1", inbound_http={
    "path": _KGET + "eParamID_XPT_Source61_Line_1",
    "body": _kreply("eParamID_XPT_Source61_Line_1", "Quad 1", "")},
    expect_state={"params": {"eParamID_XPT_Source61_Line_1": {"value": "Quad 1", "value_name": ""}},
                  "inputs": {"61": {"label_line_1": "Quad 1"}}})
telemetry(KUMO, "source-label-2", inbound_http={
    "path": _KGET + "eParamID_XPT_Source61_Line_2",
    "body": _kreply("eParamID_XPT_Source61_Line_2", "SDI1", "")},
    expect_state={"params": {"eParamID_XPT_Source61_Line_2": {"value": "SDI1", "value_name": ""}},
                  "inputs": {"61": {"label_line_2": "SDI1"}}})
telemetry(KUMO, "locked", inbound_http={
    "path": _KGET + "eParamID_XPT_Destination4_Locked",
    "body": _kreply("eParamID_XPT_Destination4_Locked", "1")},
    expect_state={"params": {"eParamID_XPT_Destination4_Locked": {"value": "1", "value_name": "1"}},
                  "outputs": {"4": {"locked": True}}})
telemetry(KUMO, "salvo", inbound_http={
    "path": _KGET + "eParamID_Salvo2",
    "body": '{"paramid":"1","name":"eParamID_Salvo2","value":' + _ksalvo2 + '}'},
    expect_state={"params": {"eParamID_Salvo2": {"value": _ksalvo2}},
                  "salvos": {"2": {"name": "Wide", "valid": True}}})
telemetry(KUMO, "product", inbound_http={
    "path": _KGET + "eParamID_KumoProductID",
    "body": _kreply("eParamID_KumoProductID", "5", "KUMO 64x64 Matrix")},
    expect_state={"params": {"eParamID_KumoProductID": {"value": "5", "value_name": "KUMO 64x64 Matrix"}},
                  "device": {"product_id": 5, "product": "KUMO 64x64 Matrix"}})
telemetry(KUMO, "source-count", inbound_http={
    "path": _KGET + "eParamID_NumberOfSources",
    "body": _kreply("eParamID_NumberOfSources", "16")},
    expect_state={"params": {"eParamID_NumberOfSources": {"value": "16", "value_name": "16"}},
                  "device": {"sources": 16}})
telemetry(KUMO, "destination-count", inbound_http={
    "path": _KGET + "eParamID_NumberOfDestinations",
    "body": _kreply("eParamID_NumberOfDestinations", "4")},
    expect_state={"params": {"eParamID_NumberOfDestinations": {"value": "4", "value_name": "4"}},
                  "device": {"destinations": 4}})
telemetry(KUMO, "not-json", inbound_http={
    "path": _KGET + "eParamID_XPT_Destination1_Status",
    "body": "<html><head><title>403 Forbidden</title></head></html>"},
    expect_state={})
