# Analog Way Picturall (User Manual V3.5 Appendix D; Mark II TCP/IP quick start
# guide v1.0): command lines + LF on TCP 11000, no acknowledgement. Expected
# lines are written from the documents' examples ("cue 3", "set stack2 select
# cue_stack=1,major=0,minor=0", "set stack2 control command=1", "set source1
# selection slot=3,collection=2", "set layer1 composition time=1.5
# intensity=0.0", "fullscreen layer1 4", "ctrl_status source1", "poweroff").
PA = "analogway-picturall"
U = {"ok": {"kind": "unverified"}}
# Manual D.b, D.f.d, D.f.e, then the object list (D.f.c) and the status of
# every object (D.f.a).
CONNECT = ["wait_startup\n", "loglevel none\n", "receiving all\n", "enum_objects\n", "ctrl_status\n"]

text(PA, "run_cue", {"cue": 3}, "cue 3\n", expect_result=U)
text(PA, "playback_select", {"playback": 2, "cue_stack": 1, "major": 3},
     "set stack2 select cue_stack=1,major=3,minor=0\n", expect_result=U)
text(PA, "playback_go", {"playback": 2}, "set stack2 control command=1\n", expect_result=U)
text(PA, "layer_media", {"layer": 1, "slot": 3, "collection": 2}, "set source1 selection slot=3,collection=2\n",
     expect_result=U)
text(PA, "layer_play_mode", {"layer": 4, "mode": 5}, "set source4 control play_state_req=5\n", expect_result=U)
text(PA, "layer_seek", {"layer": 1, "position": 0.5}, "set source1 control seek=0.5000\n", expect_result=U)
text(PA, "layer_intensity", {"layer": 14, "intensity": 0.8}, "set layer14 composition intensity=0.800\n",
     expect_result=U)
text(PA, "layer_intensity_fade", {"layer": 1, "intensity": 0.0, "time": 1.5},
     "set layer1 composition time=1.50 intensity=0.000\n", expect_result=U)
text(PA, "layer_position", {"layer": 14, "x": 0.3, "y": 0.2}, "set layer14 composition x=0.3000,y=0.2000\n",
     expect_result=U)
text(PA, "layer_fullscreen", {"layer": 1, "display": 4}, "fullscreen layer1 4\n", expect_result=U)
text(PA, "layer_status", {"layer": 1}, "ctrl_status source1\n", expect_result=U)
text(PA, "status_all", {}, "ctrl_status\n", expect_result=U)
text(PA, "list_objects", {}, "enum_objects\n", expect_result=U)
text(PA, "poweroff", {}, "poweroff\n", expect_result=U)
text(PA, "send_raw", {"command": "set layer1 composition time=1.5 intensity=1.0,x=1.0"},
     "set layer1 composition time=1.5 intensity=1.0,x=1.0\n", expect_result=U)

# Any object's parameters, written as the set format of D.f.f; Lua commands as
# E.a's lua "..." with inner quotes escaped, calling the E.b and F.c functions.
text(PA, "set_parameter", {"object": "layer14", "control": "composition", "param": "x", "value": '"0.3"'},
     'set layer14 composition x="0.3"\n', expect_result=U)
text(PA, "set_parameters", {"object": "layer14", "control": "composition", "values": 'x="0.3",y="0.2",intensity="0.8"'},
     'set layer14 composition x="0.3",y="0.2",intensity="0.8"\n', expect_result=U)
text(PA, "set_parameter_fade", {"object": "layer1", "control": "composition", "param": "intensity", "value": "0.0",
                                "time": 1.5},
     "set layer1 composition time=1.50 intensity=0.0\n", expect_result=U)
text(PA, "set_parameters_fade", {"object": "layer1", "control": "composition", "values": "intensity=1.0,x=1.0",
                                 "time": 1.5},
     "set layer1 composition time=1.50 intensity=1.0,x=1.0\n", expect_result=U)
text(PA, "object_status", {"object": "layer30"}, "ctrl_status layer30\n", expect_result=U)
text(PA, "object_info", {"object": "source1"}, "ctrl_info source1\n", expect_result=U)
text(PA, "set_array_value", {"object": "layer1", "control": "crop", "param": "values", "index": 2, "value": 0.25},
     'lua "set_pda_value_by_index(\\"layer1\\", \\"crop\\", \\"values\\", 2, 0.250000)"\n', expect_result=U)
text(PA, "layer_play", {"layer": 2}, "set source2 control play_state_req=0\n", expect_result=U)
text(PA, "layer_pause", {"layer": 2}, "set source2 control play_state_req=5\n", expect_result=U)
text(PA, "layer_stop", {"layer": 2}, "set source2 control play_state_req=6\n", expect_result=U)
text(PA, "layer_media_end_action", {"layer": 1, "action": 3}, "set source1 control media_end_action=3\n",
     expect_result=U)
text(PA, "layer_sync_source", {"layer": 3, "source": 1}, "set source3 sync source=1\n", expect_result=U)
text(PA, "layer_fps", {"layer": 1, "fps": 25.0}, "set source1 time fps=25.000\n", expect_result=U)
text(PA, "layer_relative_fps", {"layer": 1, "speed": 0.5}, "set source1 time relative_fps=0.500\n", expect_result=U)
text(PA, "layer_fps_mode", {"layer": 1, "mode": 1}, "set source1 time fps_mode=1\n", expect_result=U)
text(PA, "layer_frame_blending", {"layer": 1, "mode": 1}, "set source1 frame_blending mode=1\n", expect_result=U)
text(PA, "layer_position_fade", {"layer": 1, "x": 1.0, "y": 0.5, "time": 2.0},
     "set layer1 composition time=2.00 x=1.0000,y=0.5000\n", expect_result=U)
text(PA, "layer_fullscreen_displays", {"layer": 2, "displays": "2 5 3"}, "fullscreen layer2 2 5 3\n", expect_result=U)
text(PA, "connect_io", {"output": "gpu1", "output_id": 0, "input": "canvas1", "input_id": 1},
     'lua "connect(\\"gpu1\\", 0, \\"canvas1\\", 1)"\n', expect_result=U)
text(PA, "set_global", {"name": "enable_cef_logging", "value": "1"}, "set_global enable_cef_logging 1\n",
     expect_result=U)
text(PA, "web_ui_event", {"source": 1, "event": "Mouse ButtonLeftDown 1010 890"},
     'lua "send_ui_event(\\"source1\\", \\"Mouse ButtonLeftDown 1010 890\\")"\n', expect_result=U)
text(PA, "run_lua", {"code": 'dmesg(\\"hello\\"); dmesg(\\"world\\")'},
     'lua "dmesg(\\"hello\\"); dmesg(\\"world\\")"\n', expect_result=U)
text(PA, "run_lua_file", {"path": "my_script.lua"}, "lua_file my_script.lua\n", expect_result=U)

# ── Telemetry: the control status example of D.e.c, as one line ──
STATUS = ('MSG(100002, 176, 13, object name="source1",description=""\\ninfo media_file='
          '"/picturall/media/33_CederbergWildernessArea.jpg",play_state=5,timecode=0,media_length=40000000'
          '\\nselection slot=2,collection=0\\ncontrol media_end_action=0,play_state_req=0,seek=0\\nsync '
          'source=0\\ntime fps=30,relative_fps=1,fps_mode=0,effective_fps=25,fps_control_allowed=1'
          '\\nframe_blending mode=0\\n)\n')
telemetry(PA, "media-file", inbound=STATUS, expect_connect_wire=CONNECT,
          expect_state={
              "layers": {"1": {"media_file": "/picturall/media/33_CederbergWildernessArea.jpg",
                               "play_state": 5, "timecode_ns": 0, "media_length_ns": 40000000,
                               "slot": 2, "collection": 0, "media_end_action": 0, "play_mode": 0, "seek": 0.0,
                               "sync_source": 0, "fps": 30.0, "relative_fps": 1.0, "fps_mode": 0,
                               "effective_fps": 25.0, "frame_blending": 0}},
              "objects": {"source1": {"controls": {
                  "info": 'media_file="/picturall/media/33_CederbergWildernessArea.jpg",play_state=5,timecode=0,'
                          'media_length=40000000',
                  "selection": "slot=2,collection=0",
                  "control": "media_end_action=0,play_state_req=0,seek=0",
                  "sync": "source=0",
                  "time": "fps=30,relative_fps=1,fps_mode=0,effective_fps=25,fps_control_allowed=1",
                  "frame_blending": "mode=0"}}}})
telemetry(PA, "play-state", inbound='MSG(100002, 176, 13, object name="source3",play_state=6)\n',
          expect_state={"layers": {"3": {"play_state": 6}}})
telemetry(PA, "timecode", inbound='MSG(100002, 176, 13, object name="source2",timecode=1000)\n',
          expect_state={"layers": {"2": {"timecode_ns": 1000}}})
telemetry(PA, "media-length", inbound='MSG(100002, 176, 13, object name="source2",media_length=7)\n',
          expect_state={"layers": {"2": {"media_length_ns": 7}}})
telemetry(PA, "slot", inbound='MSG(1, 2, 13, object name="source5"\\nselection slot=9,collection=1\\n)\n',
          expect_state={"layers": {"5": {"slot": 9, "collection": 1}},
                        "objects": {"source5": {"controls": {"selection": "slot=9,collection=1"}}}})
telemetry(PA, "collection", inbound='MSG(1, 2, 13, object name="source6"\\nselection collection=4\\n)\n',
          expect_state={"layers": {"6": {"collection": 4}},
                        "objects": {"source6": {"controls": {"selection": "collection=4"}}}})
telemetry(PA, "intensity", inbound='MSG(100002, 175, 13, object name="layer14"\\ncomposition x=0.3,intensity=0.8\\n)\n',
          expect_state={"layers": {"14": {"intensity": 0.8, "x": 0.3}},
                        "objects": {"layer14": {"controls": {"composition": "x=0.3,intensity=0.8"}}}})
# The manual's set example, quoted values (D.f.f): x, y and intensity read back.
telemetry(PA, "position",
          inbound='MSG(100002, 175, 13, object name="layer2"\\ncomposition x="0.3",y="0.2",intensity="0.8"\\n)\n',
          expect_state={"layers": {"2": {"intensity": 0.8, "x": 0.3, "y": 0.2}},
                        "objects": {"layer2": {"controls": {"composition": 'x="0.3",y="0.2",intensity="0.8"'}}}})
telemetry(PA, "object-added", inbound="MSG(100002, 1, 24, layer19)\n",
          expect_state={"server": {"last_object_added": "layer19"}},
          expect_then_send=["enum_objects\n", "ctrl_status layer19\n"])
# The object list of D.e.e, cut short; a list read again replaces the last.
telemetry(PA, "object-list",
          inbound="MSG(100002, 1, 15, 101:gl_help:\\n102:canvas1:\\n175:layer1:\\n176:source1:\\n)\n",
          state_before={"objects_by_id": {"99": "gone1"}},
          expect_state={"objects_by_id": {"101": "gl_help", "102": "canvas1", "175": "layer1", "176": "source1"}})
telemetry(PA, "overflow", inbound="MSG(100002, 0, 20, )\n", expect_state={"server": {"overflow": True}})
