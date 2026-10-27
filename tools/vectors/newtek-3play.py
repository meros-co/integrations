# NewTek / Vizrt 3Play (newtek-3play): the shared HTTP interface of the
# Vizrt Automation, Integration & Control guide (section 8.3, p.162-165):
# GET /v1/shortcut?name=&value=, the macro shortcuts of p.159, triggers and
# DataLink, the XML command format posted as text/xml, and the dictionaries
# and change notifications of p.167-172.
P3 = "newtek-3play"


def _p3(command, input, target):
    http(P3, command, input, "GET", target)


_p3("shortcut", {"name": "record_toggle"}, "/v1/shortcut?name=record_toggle")
_p3("shortcut_with_value", {"name": "a_play_speed", "value": "50"}, "/v1/shortcut?name=a_play_speed&value=50")
_p3("play_macro", {"name": "Replay Last"}, "/v1/shortcut?name=play_macro_byname&value=Replay%20Last")
_p3("play_macro_by_id", {"id": "{3F2A}"}, "/v1/shortcut?name=play_macro_byid&value=%7B3F2A%7D")
_p3("stop_macro", {"name": "Replay Last"}, "/v1/shortcut?name=stop_macro_byname&value=Replay%20Last")
_p3("stop_macro_by_id", {"id": "{3F2A}"}, "/v1/shortcut?name=stop_macro_byid&value=%7B3F2A%7D")
_p3("record_macro", {"name": "Wipe"}, "/v1/shortcut?name=record_macro_byname&value=Wipe")
_p3("stop_recording_macro", {}, "/v1/shortcut?name=stop_record_macro")
_p3("trigger", {"name": "Goal"}, "/v1/trigger?name=Goal")
_p3("set_datalink", {"key": "Score", "value": "2-1"}, "/v1/datalink?key=Score&value=2-1")
V.append({"spec": P3, "command": "post_shortcut_xml",
          "input": {"xml": '<shortcut name="play_macro_byname" value="Replay Last"/>'},
          "expect_request": {"method": "POST", "target": "/v1/shortcut",
                             "body": '<shortcut name="play_macro_byname" value="Replay Last"/>'}})
telemetry(P3, "notification", inbound_ws="shortcut_states",
          expect_then_send=[{"method": "GET", "target": "/v1/dictionary?key=shortcut_states"}], expect_state={})
telemetry(P3, "notification-other", inbound_ws="ddr_timecode", expect_then_send=[], expect_state={})
telemetry(P3, "shortcut-states", inbound_http={
    "path": "/v1/dictionary?key=shortcut_states",
    "body": '<shortcut_states><shortcut_state name="record_toggle" value="1" type="bool" sender=""/></shortcut_states>'},
    expect_state={"shortcuts": {"record_toggle": "1"}})
telemetry(P3, "tally", inbound_http={
    "path": "/v1/dictionary?key=tally",
    "body": '<tally><column name="input1" index="0" on_pgm="true" on_prev="false"/></tally>'},
    expect_state={"tally": {"input1": {"program": True, "preview": False}}})
telemetry(P3, "macros", inbound_http={
    "path": "/v1/dictionary?key=macros_list",
    "body": '<macros><systemfolder><macro name="Replay Last" id="{3F2A}"/></systemfolder></macros>'},
    state_before={"macros": {"Old": {"name": "Old"}}},
    expect_state={"macros": {"Replay Last": {"name": "Replay Last", "id": "{3F2A}"}}})
_P3B = '<buffers><main><buffer selection="BFR1"/></main></buffers>'
telemetry(P3, "dictionary-other", inbound_http={"path": "/v1/dictionary?key=buffer", "body": _P3B},
          expect_state={"dictionaries": {"buffer": _P3B}})
telemetry(P3, "datalink", inbound_http={
    "path": "/v1/datalink",
    "body": "<datalink_values><data><key>Score</key><value>2-1</value></data></datalink_values>"},
    expect_state={"datalink": {"Score": "2-1"}})
telemetry(P3, "live", inbound_http={"path": "/v1/live", "body": "FALSE"},
          expect_state={"session": {"live": False}})
telemetry(P3, "product", inbound_http={
    "path": "/version",
    "body": '<product_information><product_model>3P1</product_model><product_name>3Play 3P1</product_name>'
            '<product_version>4-1</product_version><session_x_resolution>1920</session_x_resolution>'
            '<session_fielded>false</session_fielded><session_color_format>CCIR709</session_color_format>'
            '</product_information>'},
    expect_state={"product": {"model": "3P1", "name": "3Play 3P1", "version": "4-1"},
                  "session": {"width": 1920, "fielded": False, "color_format": "CCIR709"}})
