# CasparCG Server AMCP (casparcg): one vector per command, from the AMCP
# Protocol page's syntax and examples. Every command ends in CRLF; names are
# quoted with \" and \\ escaped (Special sequences). Set commands of the
# mixer read the value back with the property's get form, so their vectors
# answer both: 202 for the set, 201 and the value for the read.
CG = "casparcg"
OK = {"expect_result": {"ok": {"kind": "ack"}}}


def cg(command, input, wire, **extra):
    if isinstance(wire, list):
        text(CG, command, input, [w + "\r\n" for w in wire], **extra)
    else:
        text(CG, command, input, wire + "\r\n", **extra)


def answered(text_):
    return {"expect_result": {"ok": {"kind": "value", "value": text_}}}


# ── Basic commands ──
cg("load_background", {"channel": 1, "layer": 1, "clip": "MY_FILE", "loop": True, "transition": "PUSH", "duration": 20,
                       "tween": "easeinsine", "seek": 200, "auto": True},
   'LOADBG 1-1 "MY_FILE" LOOP PUSH 20 easeinsine RIGHT SEEK 200 AUTO', device_reply="202 LOADBG OK\r\n", **OK)
cg("load_background", {"channel": 1, "layer": 0, "clip": "Show Titles/Lower \"A\""},
   'LOADBG 1-0 "Show Titles/Lower \\"A\\"" CUT 0 linear RIGHT SEEK 0', file="load_background-quoted",
   device_reply="404 LOADBG ERROR\r\n", expect_result={"error": {"error": "device_error", "code": "404"}})
cg("load_background_with_parameters", {"channel": 1, "layer": 1, "clip": "MY_FILE", "parameters": "SLIDE 10 LEFT"},
   'LOADBG 1-1 "MY_FILE" SLIDE 10 LEFT')
cg("load", {"channel": 1, "layer": 1, "clip": "MY_FILE"}, 'LOAD 1-1 "MY_FILE" SEEK 0')
cg("load_with_parameters", {"channel": 1, "layer": 1, "clip": "MOVIE", "parameters": "SEEK 100 LOOP"},
   'LOAD 1-1 "MOVIE" SEEK 100 LOOP')
cg("play", {"channel": 1, "layer": 1}, "PLAY 1-1", device_reply="202 PLAY OK\r\n", **OK)
cg("play_clip", {"channel": 1, "layer": 1, "clip": "MY_FILE", "transition": "SLIDE", "duration": 10, "direction": "LEFT"},
   'PLAY 1-1 "MY_FILE" SLIDE 10 linear LEFT SEEK 0')
cg("play_clip_with_parameters", {"channel": 1, "layer": 10, "clip": "MY_FILE", "parameters": "STING wipe_mask 0 wipe_overlay"},
   'PLAY 1-10 "MY_FILE" STING wipe_mask 0 wipe_overlay')
cg("pause", {"channel": 1, "layer": 1}, "PAUSE 1-1")
cg("resume", {"channel": 1, "layer": 1}, "RESUME 1-1")
cg("stop", {"channel": 1, "layer": 1}, "STOP 1-1")
cg("clear_layer", {"channel": 1, "layer": 3}, "CLEAR 1-3")
cg("clear_channel", {"channel": 1}, "CLEAR 1")
cg("call", {"channel": 1, "layer": 2, "parameters": "SEEK 25"}, "CALL 1-2 SEEK 25")
cg("set_loop", {"channel": 1, "layer": 1, "loop": True}, "CALL 1-1 LOOP 1",
   device_reply="201 CALL OK\r\n1\r\n", **answered("1"))
cg("get_loop", {"channel": 1, "layer": 1}, "CALL 1-1 LOOP")
cg("seek", {"channel": 1, "layer": 1, "frame": 200}, "CALL 1-1 SEEK 200")
cg("seek_relative", {"channel": 1, "layer": 1, "from": "END", "frames": -500}, "CALL 1-1 SEEK END -500")
cg("set_start", {"channel": 1, "layer": 1, "frame": 100}, "CALL 1-1 START 100")
cg("set_length", {"channel": 1, "layer": 1, "frames": 100}, "CALL 1-1 LENGTH 100")
cg("swap_layers", {"channel": 1, "layer": 1, "other_channel": 1, "other_layer": 2, "transforms": True},
   "SWAP 1-1 1-2 TRANSFORMS")
cg("swap_channels", {"channel": 1, "other_channel": 2}, "SWAP 1 2")
cg("add_consumer", {"channel": 1, "consumer": "DECKLINK 1"}, "ADD 1 DECKLINK 1")
cg("add_consumer_at_index", {"channel": 1, "index": 700, "consumer": "FILE filename.mov SEPARATE_KEY"},
   "ADD 1-700 FILE filename.mov SEPARATE_KEY")
cg("remove_consumer", {"channel": 1, "consumer": "SCREEN"}, "REMOVE 1 SCREEN")
cg("remove_consumer_at_index", {"channel": 1, "index": 300}, "REMOVE 1-300")
cg("print", {"channel": 1}, "PRINT 1")
cg("set_log_level", {"level": "trace"}, "LOG LEVEL trace")
cg("set_log_category", {"category": "calltrace", "enabled": True}, "LOG CATEGORY calltrace 1")
cg("set_video_mode", {"channel": 1, "mode": "PAL"}, "SET 1 MODE PAL")
cg("set_channel_layout", {"channel": 1, "layout": "smpte"}, "SET 1 CHANNEL_LAYOUT smpte")
cg("lock_channel", {"channel": 1, "phrase": "secret"}, 'LOCK 1 ACQUIRE "secret"')
cg("unlock_channel", {"channel": 1}, "LOCK 1 RELEASE")
cg("clear_channel_lock", {"channel": 1}, "LOCK 1 CLEAR")
cg("ping", {}, "PING", device_reply="PONG\r\n", **OK)

# ── Data ──
cg("data_store", {"name": "Folder1/my_data", "data": "Some useful data\nline two"},
   'DATA STORE "Folder1/my_data" "Some useful data\\nline two"')
cg("data_retrieve", {"name": "my_data"}, 'DATA RETRIEVE "my_data"',
   device_reply="201 DATA RETRIEVE OK\r\nSome useful data\r\n", **answered("Some useful data"))
cg("data_list", {}, "DATA LIST")
cg("data_list_folder", {"folder": "Folder1"}, 'DATA LIST "Folder1"')
cg("data_remove", {"name": "Folder1/my_data"}, 'DATA REMOVE "Folder1/my_data"')

# ── Templates ──
cg("cg_add", {"channel": 1, "layer": 20, "cg_layer": 1, "template": "lower_third", "play_on_load": True,
              "data": '{"f0":"Name"}'},
   'CG 1-20 ADD 1 "lower_third" 1 "{\\"f0\\":\\"Name\\"}"')
cg("cg_add_without_data", {"channel": 1, "cg_layer": 10, "template": "svtnews/info", "play_on_load": True},
   'CG 1-9999 ADD 10 "svtnews/info" 1')
cg("cg_play", {"channel": 1, "cg_layer": 0}, "CG 1-9999 PLAY 0")
cg("cg_stop", {"channel": 1, "cg_layer": 0}, "CG 1-9999 STOP 0")
cg("cg_next", {"channel": 1, "cg_layer": 0}, "CG 1-9999 NEXT 0")
cg("cg_remove", {"channel": 1, "cg_layer": 0}, "CG 1-9999 REMOVE 0")
cg("cg_clear", {"channel": 1}, "CG 1-9999 CLEAR")
cg("cg_update", {"channel": 1, "layer": 20, "cg_layer": 1, "data": "my_data"}, 'CG 1-20 UPDATE 1 "my_data"')
cg("cg_invoke", {"channel": 1, "cg_layer": 1, "method": "outro"}, 'CG 1-9999 INVOKE 1 "outro"')
cg("cg_info", {"channel": 1, "cg_layer": 1}, "CG 1-9999 INFO 1")
cg("cg_info_host", {"channel": 1}, "CG 1-9999 INFO")

# ── Mixer: sets read back with their get form ──
def mixer(command, input, set_, get, value, **extra):
    cg(command, input, [set_, get], device_reply=f"202 MIXER OK\r\n201 MIXER OK\r\n{value}\r\n", **extra)


mixer("set_keyer", {"channel": 1, "layer": 0, "keyer": True}, "MIXER 1-0 KEYER 1", "MIXER 1-0 KEYER", "1",
      **answered("1"))
mixer("set_blend_mode", {"channel": 1, "layer": 1, "mode": "Overlay"}, "MIXER 1-1 BLEND Overlay", "MIXER 1-1 BLEND", "OVERLAY")
mixer("set_opacity", {"channel": 1, "layer": 0, "opacity": 0.5, "duration": 25, "tween": "easeinsine"},
      "MIXER 1-0 OPACITY 0.5000 25 easeinsine", "MIXER 1-0 OPACITY", "0.5", **answered("0.5"))
mixer("set_brightness", {"channel": 1, "layer": 0, "brightness": 0.5, "duration": 25, "tween": "easeinsine"},
      "MIXER 1-0 BRIGHTNESS 0.5000 25 easeinsine", "MIXER 1-0 BRIGHTNESS", "0.5")
mixer("set_saturation", {"channel": 1, "layer": 0, "saturation": 0.5}, "MIXER 1-0 SATURATION 0.5000 0 linear",
      "MIXER 1-0 SATURATION", "0.5")
mixer("set_contrast", {"channel": 1, "layer": 0, "contrast": 0.5}, "MIXER 1-0 CONTRAST 0.5000 0 linear",
      "MIXER 1-0 CONTRAST", "0.5")
mixer("set_levels", {"channel": 1, "layer": 10, "min_input": 0.0627, "max_input": 0.922, "gamma": 1.0, "min_output": 0.0,
                     "max_output": 1.0, "duration": 25, "tween": "easeinsine"},
      "MIXER 1-10 LEVELS 0.0627 0.9220 1.0000 0.0000 1.0000 25 easeinsine", "MIXER 1-10 LEVELS", "0.0627 0.922 1 0 1")
mixer("set_fill", {"channel": 1, "layer": 0, "x": 0.25, "y": 0.25, "x_scale": 0.5, "y_scale": 0.5, "duration": 25,
                   "tween": "easeinsine"},
      "MIXER 1-0 FILL 0.2500 0.2500 0.5000 0.5000 25 easeinsine", "MIXER 1-0 FILL", "0.25 0.25 0.5 0.5")
# The COMMIT example's deferred fill.
mixer("set_fill", {"channel": 1, "layer": 1, "x": 0.0, "y": 0.0, "x_scale": 0.5, "y_scale": 0.5, "duration": 25, "defer": True},
      "MIXER 1-1 FILL 0.0000 0.0000 0.5000 0.5000 25 linear DEFER", "MIXER 1-1 FILL", "0 0 0.5 0.5",
      file="set_fill-deferred")
mixer("set_clip", {"channel": 1, "layer": 0, "x": 0.25, "y": 0.25, "width": 0.5, "height": 0.5},
      "MIXER 1-0 CLIP 0.2500 0.2500 0.5000 0.5000 0 linear", "MIXER 1-0 CLIP", "0.25 0.25 0.5 0.5")
mixer("set_anchor", {"channel": 1, "layer": 10, "x": 0.5, "y": 0.6, "duration": 25, "tween": "easeinsine"},
      "MIXER 1-10 ANCHOR 0.5000 0.6000 25 easeinsine", "MIXER 1-10 ANCHOR", "0.5 0.6")
mixer("set_crop", {"channel": 1, "layer": 0, "left": 0.25, "top": 0.25, "right": 0.75, "bottom": 0.75},
      "MIXER 1-0 CROP 0.2500 0.2500 0.7500 0.7500 0 linear", "MIXER 1-0 CROP", "0.25 0.25 0.75 0.75")
mixer("set_rotation", {"channel": 1, "layer": 0, "degrees": 45.0, "duration": 25, "tween": "easeinsine"},
      "MIXER 1-0 ROTATION 45.0000 25 easeinsine", "MIXER 1-0 ROTATION", "45")
mixer("set_perspective", {"channel": 1, "layer": 10, "top_left_x": 0.4, "top_left_y": 0.4, "top_right_x": 0.6,
                          "top_right_y": 0.4, "bottom_right_x": 1.0, "bottom_right_y": 1.0, "bottom_left_x": 0.0,
                          "bottom_left_y": 1.0, "duration": 25, "tween": "easeinsine"},
      "MIXER 1-10 PERSPECTIVE 0.4000 0.4000 0.6000 0.4000 1.0000 1.0000 0.0000 1.0000 25 easeinsine",
      "MIXER 1-10 PERSPECTIVE", "0.4 0.4 0.6 0.4 1 1 0 1")
mixer("set_mipmap", {"channel": 1, "layer": 10, "mipmap": True}, "MIXER 1-10 MIPMAP 1", "MIXER 1-10 MIPMAP", "1")
mixer("set_volume", {"channel": 1, "layer": 0, "volume": 0.0, "duration": 25}, "MIXER 1-0 VOLUME 0.0000 25 linear",
      "MIXER 1-0 VOLUME", "0")
mixer("set_master_volume", {"channel": 1, "volume": 0.5}, "MIXER 1 MASTERVOLUME 0.5000", "MIXER 1 MASTERVOLUME", "0.5")
mixer("set_straight_alpha_output", {"channel": 1, "straight": True}, "MIXER 1 STRAIGHT_ALPHA_OUTPUT 1",
      "MIXER 1 STRAIGHT_ALPHA_OUTPUT", "1")
mixer("set_chroma_key", {"channel": 1, "layer": 1, "target_hue": 120.0, "hue_width": 0.1, "min_saturation": 0.0,
                         "min_brightness": 0.0, "softness": 0.1, "spill_suppress": 0.1, "spill_suppress_saturation": 0.7},
      "MIXER 1-1 CHROMA 1 120.0000 0.1000 0.0000 0.0000 0.1000 0.1000 0.7000 0 0 linear", "MIXER 1-1 CHROMA",
      "1 120 0.1 0 0 0.1 0.1 0.7 0")
mixer("disable_chroma_key", {"channel": 1, "layer": 1}, "MIXER 1-1 CHROMA 0", "MIXER 1-1 CHROMA", "0 0 0 0 0 0 0 0 0")
for prop, kw, scope in [("keyer", "KEYER", "1-0"), ("blend_mode", "BLEND", "1-1"), ("opacity", "OPACITY", "1-0"),
                        ("brightness", "BRIGHTNESS", "1-0"), ("saturation", "SATURATION", "1-0"),
                        ("contrast", "CONTRAST", "1-0"), ("levels", "LEVELS", "1-10"), ("fill", "FILL", "1-0"),
                        ("clip", "CLIP", "1-0"), ("anchor", "ANCHOR", "1-10"), ("crop", "CROP", "1-0"),
                        ("rotation", "ROTATION", "1-0"), ("perspective", "PERSPECTIVE", "1-10"),
                        ("mipmap", "MIPMAP", "1-10"), ("volume", "VOLUME", "1-0"), ("chroma_key", "CHROMA", "1-1")]:
    ch, layer = scope.split("-")
    cg(f"get_{prop}", {"channel": int(ch), "layer": int(layer)}, f"MIXER {scope} {kw}")
cg("get_opacity", {"channel": 1, "layer": 0}, "MIXER 1-0 OPACITY", device_reply="201 MIXER OK\r\n0.5\r\n",
   file="get_opacity-reply", **answered("0.5"))
cg("get_master_volume", {"channel": 1}, "MIXER 1 MASTERVOLUME")
cg("get_straight_alpha_output", {"channel": 1}, "MIXER 1 STRAIGHT_ALPHA_OUTPUT")
cg("set_invert", {"channel": 1, "layer": 10, "invert": True}, "MIXER 1-10 INVERT 1")
cg("set_grid", {"channel": 1, "resolution": 2}, "MIXER 1 GRID 2 0 linear")
cg("mixer_commit", {"channel": 1}, "MIXER 1 COMMIT")
cg("mixer_clear_layer", {"channel": 1, "layer": 1}, "MIXER 1-1 CLEAR")
cg("mixer_clear_channel", {"channel": 1}, "MIXER 1 CLEAR")
cg("channel_grid", {}, "CHANNEL_GRID")

# ── Thumbnails and queries ──
cg("thumbnail_list", {}, "THUMBNAIL LIST")
cg("thumbnail_list_folder", {"folder": "foo"}, 'THUMBNAIL LIST "foo"')
cg("thumbnail_retrieve", {"file": "foo/bar"}, 'THUMBNAIL RETRIEVE "foo/bar"',
   device_reply="201 THUMBNAIL RETRIEVE OK\r\niVBORw0KGgo=\r\n", **answered("iVBORw0KGgo="))
cg("thumbnail_generate", {"file": "AMB"}, 'THUMBNAIL GENERATE "AMB"')
cg("thumbnail_generate_all", {}, "THUMBNAIL GENERATE_ALL")
cg("media_info", {"file": "AMB"}, 'CINF "AMB"')
cg("list_media", {}, "CLS", device_reply='200 CLS OK\r\n"AMB" MOVIE 6445960 20170413102935 268 1/25\r\n\r\n',
   **answered('"AMB" MOVIE 6445960 20170413102935 268 1/25'))
cg("list_media_folder", {"folder": "TEST"}, 'CLS "TEST"')
cg("list_fonts", {}, "FLS")
cg("list_templates", {}, "TLS")
cg("list_templates_folder", {"folder": "svtnews"}, 'TLS "svtnews"')
cg("version", {}, "VERSION", device_reply="201 VERSION OK\r\n2.1.0.f207a33 STABLE\r\n", **answered("2.1.0.f207a33 STABLE"))
cg("component_version", {"component": "CEF"}, "VERSION CEF")
cg("info", {}, "INFO")
cg("info_channel", {"channel": 1}, "INFO 1")
cg("info_layer", {"channel": 1, "layer": 10}, "INFO 1-10")
cg("info_template", {"template": "lower_third"}, 'INFO TEMPLATE "lower_third"')
for command, words in [("info_config", "CONFIG"), ("info_paths", "PATHS"), ("info_system", "SYSTEM"),
                       ("info_server", "SERVER"), ("info_queues", "QUEUES"), ("info_threads", "THREADS")]:
    cg(command, {}, f"INFO {words}")
cg("info_channel_delay", {"channel": 1}, "INFO 1 DELAY")
cg("info_layer_delay", {"channel": 1, "layer": 10}, "INFO 1-10 DELAY")
cg("open_diagnostics", {}, "DIAG")
cg("gl_info", {}, "GL INFO")
cg("gl_gc", {}, "GL GC")
cg("bye", {}, "BYE")
cg("kill", {}, "KILL")
cg("restart", {}, "RESTART")
cg("help", {}, "HELP")
cg("help_command", {"command": "PLAY"}, "HELP PLAY")
cg("help_producers", {}, "HELP PRODUCER")
cg("help_producer", {"producer": "FFmpeg Producer"}, "HELP PRODUCER FFmpeg Producer")
cg("help_consumers", {}, "HELP CONSUMER")
cg("help_consumer", {"consumer": "Decklink Consumer"}, "HELP CONSUMER Decklink Consumer")

# ── Telemetry: replies, with the request each answers ──
telemetry(CG, "info", inbound="200 INFO OK\n1 720p5000 PLAYING\n2 PAL PLAYING", request="INFO",
          expect_state={"channels": {"1": {"format": "720p5000", "status": "PLAYING"},
                                     "2": {"format": "PAL", "status": "PLAYING"}}})
telemetry(CG, "media", inbound='200 CLS OK\n"AMB" MOVIE 6445960 20170413102935 268 1/25\n'
                               '"TEST/GO1080P25" MOVIE 1249808 20170413102935 250 1/25', request="CLS",
          state_before={"media": {"GONE": {"type": "STILL"}}},
          expect_state={"media": {"AMB": {"type": "MOVIE", "size": 6445960, "modified": "20170413102935",
                                           "frames": 268, "frame_rate": "1/25"},
                                   "TEST/GO1080P25": {"type": "MOVIE", "size": 1249808, "modified": "20170413102935",
                                                      "frames": 250, "frame_rate": "1/25"}}})
telemetry(CG, "media-folder-not-the-list", inbound='200 CLS OK\n"TEST/X" STILL 10 20170413102935 0 0/1',
          request='CLS "TEST"', state_before={"media": {"AMB": {"type": "MOVIE"}}},
          expect_state={"media": {"AMB": {"type": "MOVIE"}}})
telemetry(CG, "templates", inbound="200 TLS OK\nlower_third\nsvtnews/info", request="TLS",
          expect_state={"templates": {"lower_third": {"listed": True}, "svtnews/info": {"listed": True}}})
telemetry(CG, "templates-quoted", inbound='200 TLS OK\n"LOWER_THIRD" 5523 20130301124409', request="TLS",
          expect_state={"templates": {"LOWER_THIRD": {"listed": True}}})
telemetry(CG, "fonts", inbound="200 FLS OK\nArial Arial.ttf", request="FLS", expect_state={"fonts": {"0": "Arial Arial.ttf"}})
telemetry(CG, "datasets", inbound="200 DATA LIST OK\nmy_data\nFolder1/my_data", request="DATA LIST",
          expect_state={"datasets": {"0": "my_data", "1": "Folder1/my_data"}})
telemetry(CG, "thumbnails", inbound='200 THUMBNAIL LIST OK\n"AMB" 20130301T124409 1149\n"foo/bar" 20130523T234001 244',
          request="THUMBNAIL LIST",
          expect_state={"thumbnails": {"AMB": {"modified": "20130301T124409", "size": 1149},
                                       "foo/bar": {"modified": "20130523T234001", "size": 244}}})
telemetry(CG, "version", inbound="201 VERSION OK\n2.1.0.f207a33 STABLE", request="VERSION",
          expect_state={"server": {"version": "2.1.0.f207a33 STABLE"}})
telemetry(CG, "component-version", inbound="201 VERSION OK\n11.8.800.94", request="VERSION FLASH",
          expect_state={"server": {"components": {"flash": "11.8.800.94"}}})
telemetry(CG, "loop", inbound="201 CALL OK\n1", request="CALL 1-1 LOOP 1",
          expect_state={"channels": {"1": {"layers": {"1": {"loop": True}}}}})
telemetry(CG, "clear-layer", inbound="202 CLEAR OK", request="CLEAR 1-3",
          state_before={"channels": {"1": {"layers": {"3": {"paused": False}, "4": {"paused": True}}}}},
          expect_state={"channels": {"1": {"layers": {"4": {"paused": True}}}}})
telemetry(CG, "clear-channel", inbound="202 CLEAR OK", request="CLEAR 1",
          state_before={"channels": {"1": {"format": "PAL", "layers": {"3": {"paused": False}}}}},
          expect_state={"channels": {"1": {"format": "PAL"}}})
telemetry(CG, "stop", inbound="202 STOP OK", request="STOP 1-3",
          state_before={"channels": {"1": {"layers": {"3": {"paused": False, "file": {"name": "AMB"}}}}}},
          expect_state={"channels": {"1": {"layers": {"3": {"paused": False}}}}})
telemetry(CG, "mixer-clear-layer", inbound="202 MIXER OK", request="MIXER 1-3 CLEAR",
          state_before={"channels": {"1": {"layers": {"3": {"paused": False, "mixer": {"opacity": 0.5}}}}}},
          expect_state={"channels": {"1": {"layers": {"3": {"paused": False}}}}})
telemetry(CG, "mixer-clear-channel", inbound="202 MIXER OK", request="MIXER 1 CLEAR",
          state_before={"channels": {"1": {"format": "PAL", "mixer": {"master_volume": 0.5}}}},
          expect_state={"channels": {"1": {"format": "PAL"}}})


def mixer_state(name, request, value, layer_state=None, channel_state=None):
    st = {"channels": {"1": {}}}
    if layer_state is not None:
        st["channels"]["1"]["layers"] = {"10": {"mixer": layer_state}}
    if channel_state is not None:
        st["channels"]["1"]["mixer"] = channel_state
    telemetry(CG, name, inbound=f"201 MIXER OK\n{value}", request=request, expect_state=st)


mixer_state("mixer-keyer", "MIXER 1-10 KEYER", "1", {"keyer": True})
mixer_state("mixer-blend", "MIXER 1-10 BLEND", "SCREEN", {"blend": "SCREEN"})
mixer_state("mixer-opacity", "MIXER 1-10 OPACITY", "0.5", {"opacity": 0.5})
mixer_state("mixer-brightness", "MIXER 1-10 BRIGHTNESS", "0.5", {"brightness": 0.5})
mixer_state("mixer-saturation", "MIXER 1-10 SATURATION", "0.5", {"saturation": 0.5})
mixer_state("mixer-contrast", "MIXER 1-10 CONTRAST", "0.5", {"contrast": 0.5})
mixer_state("mixer-levels", "MIXER 1-10 LEVELS", "0 1 0.5 0 1",
            {"levels": {"min_input": 0.0, "max_input": 1.0, "gamma": 0.5, "min_output": 0.0, "max_output": 1.0}})
mixer_state("mixer-fill", "MIXER 1-10 FILL", "0.25 0.25 0.5 0.5",
            {"fill": {"x": 0.25, "y": 0.25, "x_scale": 0.5, "y_scale": 0.5}})
mixer_state("mixer-clip", "MIXER 1-10 CLIP", "0.25 0.25 0.5 0.5",
            {"clip": {"x": 0.25, "y": 0.25, "width": 0.5, "height": 0.5}})
mixer_state("mixer-anchor", "MIXER 1-10 ANCHOR", "0.5 0.6", {"anchor": {"x": 0.5, "y": 0.6}})
mixer_state("mixer-crop", "MIXER 1-10 CROP", "0.25 0.25 0.75 0.75",
            {"crop": {"left": 0.25, "top": 0.25, "right": 0.75, "bottom": 0.75}})
mixer_state("mixer-rotation", "MIXER 1-10 ROTATION", "45", {"rotation": 45.0})
mixer_state("mixer-perspective", "MIXER 1-10 PERSPECTIVE", "0.4 0.4 0.6 0.4 1 1 0 1",
            {"perspective": {"top_left_x": 0.4, "top_left_y": 0.4, "top_right_x": 0.6, "top_right_y": 0.4,
                             "bottom_right_x": 1.0, "bottom_right_y": 1.0, "bottom_left_x": 0.0, "bottom_left_y": 1.0}})
mixer_state("mixer-mipmap", "MIXER 1-10 MIPMAP", "1", {"mipmap": True})
mixer_state("mixer-volume", "MIXER 1-10 VOLUME", "0.8", {"volume": 0.8})
mixer_state("mixer-master-volume", "MIXER 1 MASTERVOLUME", "0.5", channel_state={"master_volume": 0.5})
mixer_state("mixer-straight-alpha", "MIXER 1 STRAIGHT_ALPHA_OUTPUT", "1", channel_state={"straight_alpha_output": True})
mixer_state("mixer-chroma", "MIXER 1-10 CHROMA", "1 120 0.1 0 0 0.1 0 1 0",
            {"chroma": {"enabled": True, "target_hue": 120.0, "hue_width": 0.1, "min_saturation": 0.0,
                        "min_brightness": 0.0, "softness": 0.1, "spill_suppress": 0.0,
                        "spill_suppress_saturation": 1.0, "show_mask": False}})

# ── Telemetry: OSC on telemetry.osc's port, as the OSC Protocol and FFmpeg
# Producer pages give the addresses and example arguments ──
def oscv(name, address, args, state):
    telemetry(CG, name, inbound_hex=hexs(osc(address, *args)), expect_state=state)


def layer(**k):
    return {"channels": {"1": {"layers": {"10": k}}}}


oscv("osc-format", "/channel/1/format", [("s", "PAL")], {"channels": {"1": {"format": "PAL"}}})
oscv("osc-profiler", "/channel/1/profiler/time", [("f", 0.0390625), ("f", 0.0625)],
     {"channels": {"1": {"profiler": {"actual": 0.0390625, "expected": 0.0625}}}})
oscv("osc-output-type", "/channel/1/output/port/0/type", [("s", "screen")],
     {"channels": {"1": {"outputs": {"0": {"type": "screen"}}}}})
oscv("osc-output-frame", "/channel/1/output/port/1/frame", [("i", 200), ("i", 2000000000)],
     {"channels": {"1": {"outputs": {"1": {"frames": 200, "max_frames": 2000000000}}}}})
oscv("osc-audio-channels", "/channel/1/mixer/audio/nb_channels", [("i", 2)], {"channels": {"1": {"audio": {"channels": 2}}}})
oscv("osc-audio-level", "/channel/1/mixer/audio/1/dBFS", [("f", -20.0)], {"channels": {"1": {"audio": {"levels": {"1": -20.0}}}}})
oscv("osc-layer-time", "/channel/1/stage/layer/10/time", [("f", 101.25)], layer(time=101.25))
oscv("osc-layer-frame", "/channel/1/stage/layer/10/frame", [("i", 2531)], layer(frame=2531))
oscv("osc-layer-type", "/channel/1/stage/layer/10/type", [("s", "transition")], layer(type="transition"))
oscv("osc-layer-background-type", "/channel/1/stage/layer/10/background/type", [("s", "empty")], layer(background_type="empty"))
oscv("osc-layer-profiler", "/channel/1/stage/layer/10/profiler/time", [("f", 0.375), ("f", 0.5)],
     layer(profiler={"actual": 0.375, "expected": 0.5}))
oscv("osc-layer-paused", "/channel/1/stage/layer/10/paused", [("T", None)], layer(paused=True))
oscv("osc-layer-loop", "/channel/1/stage/layer/10/loop", [("i", 1)], layer(loop=True))
oscv("osc-host-path", "/channel/1/stage/layer/10/host/path", [("s", "template_file.ft")], layer(host={"path": "template_file.ft"}))
oscv("osc-host-width", "/channel/1/stage/layer/10/host/width", [("i", 1920)], layer(host={"width": 1920}))
oscv("osc-host-height", "/channel/1/stage/layer/10/host/height", [("i", 1080)], layer(host={"height": 1080}))
oscv("osc-host-fps", "/channel/1/stage/layer/10/host/fps", [("i", 50)], layer(host={"fps": 50.0}))
oscv("osc-file-name", "/channel/1/stage/layer/10/file/name", [("s", "TEST/GO1080P25")], layer(file={"name": "TEST/GO1080P25"}))
oscv("osc-file-name-foreground", "/channel/1/stage/layer/10/foreground/file/name", [("s", "AMB")], layer(file={"name": "AMB"}))
oscv("osc-file-path", "/channel/1/stage/layer/10/file/path", [("s", "c:/CasparCG-Server/media/TEST/GO1080P25.mp4")],
     layer(file={"path": "c:/CasparCG-Server/media/TEST/GO1080P25.mp4"}))
oscv("osc-file-time", "/channel/1/stage/layer/10/file/time", [("f", 12.0), ("f", 400.0)], layer(file={"elapsed": 12.0, "duration": 400.0}))
oscv("osc-file-frame", "/channel/1/stage/layer/10/file/frame", [("i", 300), ("i", 10000)], layer(file={"frame": 300, "frames": 10000}))
oscv("osc-file-fps", "/channel/1/stage/layer/10/file/fps", [("f", 25.0)], layer(file={"fps": 25.0}))
oscv("osc-file-stream-fps", "/channel/1/stage/layer/10/file/0/fps", [("f", 25.0)], layer(file={"streams": {"0": {"fps": 25.0}}}))
oscv("osc-video-width", "/channel/1/stage/layer/10/file/video/width", [("i", 1920)], layer(file={"video": {"width": 1920}}))
oscv("osc-video-height", "/channel/1/stage/layer/10/file/video/height", [("i", 1080)], layer(file={"video": {"height": 1080}}))
oscv("osc-video-field", "/channel/1/stage/layer/10/file/video/field", [("s", "progressive")], layer(file={"video": {"field": "progressive"}}))
oscv("osc-video-codec", "/channel/1/stage/layer/10/file/video/codec", [("s", "H.264 /AVC")], layer(file={"video": {"codec": "H.264 /AVC"}}))
oscv("osc-audio-sample-rate", "/channel/1/stage/layer/10/file/audio/sample-rate", [("i", 48000)], layer(file={"audio": {"sample_rate": 48000}}))
oscv("osc-file-audio-channels", "/channel/1/stage/layer/10/file/audio/channels", [("i", 2)], layer(file={"audio": {"channels": 2}}))
oscv("osc-audio-format", "/channel/1/stage/layer/10/file/audio/format", [("s", "s16")], layer(file={"audio": {"format": "s16"}}))
oscv("osc-audio-codec", "/channel/1/stage/layer/10/file/audio/codec", [("s", "AAC")], layer(file={"audio": {"codec": "AAC"}}))
