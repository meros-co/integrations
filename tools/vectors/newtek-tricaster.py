# NewTek / Vizrt TriCaster (newtek-tricaster): the macro, media player and
# audio shortcuts, and the dictionaries and change notifications added to the
# vectors in make_vectors.py. Shortcuts are GET /v1/shortcut?name=&value=
# (Automation and Integration Guide p.62-64); macro shortcut names from p.59-60,
# media player ones from shortcuts_cfg.xml; dictionary shapes from p.67-71.
TC = "newtek-tricaster"


def _tc(command, input, query):
    http(TC, command, input, "GET", "/v1/shortcut?" + query)


_tc("play_macro_by_id", {"id": "a1b2c3d4-0000-4000-8000-000000000001"},
    "name=play_macro_byid&value=a1b2c3d4-0000-4000-8000-000000000001")
_tc("stop_macro", {"name": "Open Show"}, "name=stop_macro_byname&value=Open%20Show")
_tc("stop_macro_by_id", {"id": "a1b2c3d4-0000-4000-8000-000000000001"},
    "name=stop_macro_byid&value=a1b2c3d4-0000-4000-8000-000000000001")
_tc("record_macro", {"name": "Wide Shot"}, "name=record_macro_byname&value=Wide%20Shot")
_tc("stop_recording_macro", {}, "name=stop_record_macro")
_tc("continue_paused_macro", {}, "name=continue_pausedmacro")
_tc("media_play_toggle", {"player": "ddr1"}, "name=ddr1_play_toggle")
_tc("media_previous_clip", {"player": "gfx2"}, "name=gfx2_back")
_tc("media_next_clip", {"player": "ddr2"}, "name=ddr2_forward")
_tc("media_select_clip", {"player": "ddr1", "clip": 3}, "name=ddr1_select_clip&value=3")
_tc("media_select_preset", {"player": "ddr3", "preset": 2}, "name=ddr3_select_preset&value=2")
_tc("media_next_preset", {"player": "sound"}, "name=sound_next_preset")
_tc("media_previous_preset", {"player": "gfx1"}, "name=gfx1_previous_preset")
_tc("media_set_speed", {"player": "ddr1", "speed": 50}, "name=ddr1_playspeed&value=50")
_tc("media_mark_in", {"player": "ddr4"}, "name=ddr4_mark_in")
_tc("media_mark_out", {"player": "ddr4"}, "name=ddr4_mark_out")
_tc("media_clear_marks", {"player": "ddr4"}, "name=ddr4_mark_in_out_clear")
_tc("media_set_mode", {"player": "ddr2", "mode": "loop", "enabled": True}, "name=ddr2_loop_mode_toggle&value=true")
_tc("media_show_on_switcher", {"player": "gfx1", "shown": False}, "name=gfx1_show_on_switcher&value=false")
_tc("set_source_volume", {"source": "ddr1", "level": -6.5}, "name=ddr1_volume&value=-6.5")
_tc("mute_source", {"source": "input2", "muted": True}, "name=input2_mute&value=true")
_tc("refresh_file_browser", {}, "name=request_filebrowser_update")

# The change notification websocket sends a dictionary's name (p.71-72); that
# dictionary is read again.
telemetry(TC, "notification-tally", inbound_ws="tally",
          expect_then_send=[{"method": "GET", "target": "/v1/dictionary?key=tally"}], expect_state={})
telemetry(TC, "notification-macros", inbound_ws="macros_list",
          expect_then_send=[{"method": "GET", "target": "/v1/dictionary?key=macros_list"}], expect_state={})
# A name it does not send (ddr_timecode is polled) re-reads nothing.
telemetry(TC, "notification-other", inbound_ws="audiomixer", expect_then_send=[], expect_state={})
# The switcher dictionary with its T-bar and overlays (p.69).
telemetry(TC, "switcher-overlays", inbound_http={
    "path": "/v1/dictionary?key=switcher",
    "body": '<switcher_update main_source="BFR4" preview_source="gfx1" effect="C:\\Effects\\Fade.trans">'
            '<tbar position="0.250000" speed="0.500000"/><switcher_overlays>'
            '<overlay z_order_position="0" source="V2"><tbar position="1.000000" speed="0.000000"/></overlay>'
            '<overlay z_order_position="1" source="DDR1"><tbar position="0.000000" speed="0.000000"/></overlay>'
            '</switcher_overlays></switcher_update>'},
    state_before={"switcher": {"overlays": {"2": {"source": "GFX1"}}}},
    expect_state={"switcher": {"program": "BFR4", "preview": "gfx1", "effect": "C:\\Effects\\Fade.trans",
                               "tbar": {"position": 0.25, "speed": 0.5},
                               "overlays": {"0": {"source": "V2"}, "1": {"source": "DDR1"}}}})
_BUF = ('<buffers><main><buffer selection="BFR4"/></main>'
        '<me index="0"><row><buffer selection="BFR1"/></row></me></buffers>')
telemetry(TC, "buffer", inbound_http={"path": "/v1/dictionary?key=buffer", "body": _BUF},
          expect_state={"buffers": {"main": "BFR4", "xml": _BUF}})
_PL = '<ddr_playlist><ddr name="ddr1"><clip name="Intro.mov"/></ddr></ddr_playlist>'
telemetry(TC, "ddr-playlist", inbound_http={"path": "/v1/dictionary?key=ddr_playlist", "body": _PL + "\n"},
          expect_state={"ddr": {"playlist_xml": _PL}})
_TCX = '<ddr_timecode><ddr1 remaining="00:00:12.10"/></ddr_timecode>'
telemetry(TC, "ddr-timecode", inbound_http={"path": "/v1/dictionary?key=ddr_timecode", "body": _TCX},
          expect_state={"ddr": {"timecode_xml": _TCX}})
telemetry(TC, "macros", inbound_http={
    "path": "/v1/dictionary?key=macros_list",
    "body": '<macros><systemfolder><macro name="Stop All" id="{1}"/></systemfolder>'
            '<sessionfolder><macro name="Open Show v1.2" id="{2}"/></sessionfolder></macros>'},
    state_before={"macros": {"Old": {"name": "Old"}}},
    expect_state={"macros": {"Stop All": {"name": "Stop All", "id": "{1}"},
                             "Open Show v1.2": {"name": "Open Show v1.2", "id": "{2}"}}})
telemetry(TC, "product", inbound_http={
    "path": "/v1/version",
    "body": '<product_information><product_model>TC1</product_model><product_name>TriCaster TC1</product_name>'
            '<product_version>7-0</product_version><product_id>NCWL</product_id><product_serial_no></product_serial_no>'
            '<product_build_no>7-0-180920</product_build_no><machine_name>TC1</machine_name>'
            '<session_x_resolution>1920</session_x_resolution><session_y_resolution>1080</session_y_resolution>'
            '<session_fielded>true</session_fielded><session_frame_rate>29.970030</session_frame_rate>'
            '<session_aspect_ratio>1.777778</session_aspect_ratio><session_name>TC1 Session</session_name>'
            '</product_information>'},
    expect_state={"product": {"model": "TC1", "name": "TriCaster TC1", "version": "7-0", "build": "7-0-180920",
                              "machine_name": "TC1"},
                  "session": {"name": "TC1 Session", "width": 1920, "height": 1080, "frame_rate": 29.97003,
                              "fielded": True}})
