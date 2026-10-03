# Millumin OSC API (millumin): one vector per command. Addresses and argument
# types are those of the Millumin V5 "OSC documentation" wiki page (and the
# V3 page for the legacy names), without the optional /millumin prefix; an int
# is an index and a string a name. Example values are the page's own where it
# gives them (launchColumn 11 and "test", opacity 0.75, xy 12,21, rotation 90,
# scale 0.5, media time 10, normalizedTime 0.5, goToTime "01:02:03.400").
# Millumin never answers, so every command is unverified. OSC bytes come
# from the encoder in make_vectors.py; floats are 32-bit.
MI = "millumin"
_U = {"expect_result": {"ok": {"kind": "unverified"}}}


def mi(command, input, address, *args, **extra):
    binary(MI, command, input, osc(address, *args), **extra)


mi("launch_column", {"column": 11}, "/action/launchColumn", ("i", 11), **_U)
mi("launch_column_named", {"column": "test"}, "/action/launchColumn", ("s", "test"))
mi("launch_or_stop_column", {"column": 3}, "/action/launchOrStopColumn", ("i", 3))
mi("launch_or_stop_column_named", {"column": "Intro"}, "/action/launchOrStopColumn", ("s", "Intro"))
mi("stop_column", {}, "/action/stopColumn", **_U)
mi("next_column", {}, "/action/launchNextColumn")
mi("previous_column", {}, "/action/launchPreviousColumn")
mi("previous_column_legacy", {}, "/action/launchPrevColumn", model="millumin-3")
mi("select_board", {"board": 2}, "/action/selectBoard", ("i", 2))
mi("select_board_named", {"board": "Act 2"}, "/action/selectBoard", ("s", "Act 2"))
mi("play", {}, "/action/play", **_U)
mi("pause", {}, "/action/pause")
mi("play_or_pause", {}, "/action/playOrPause")
mi("go_to_time", {"seconds": -10.0}, "/action/goToTime", ("f", -10.0))
mi("go_to_timecode", {"time": "01:02:03.400"}, "/action/goToTime", ("s", "01:02:03.400"))
mi("go_to_timeline_segment", {"segment": "Chorus"}, "/action/goToTimelineSegment", ("s", "Chorus"))
mi("play_all", {}, "/action/playAll", model="millumin-3")
mi("pause_all", {}, "/action/pauseAll", model="millumin-3")
mi("play_timeline", {}, "/action/playTimeline", model="millumin-3")
mi("pause_timeline", {}, "/action/pauseTimeline", model="millumin-3")
mi("play_or_pause_timeline", {}, "/action/playOrPauseTimeline", model="millumin-3")
mi("select_layer", {"layer": 1}, "/action/selectLayer", ("i", 1))
mi("select_layer_named", {"layer": "Background"}, "/action/selectLayer", ("s", "Background"))
mi("select_light", {"light": 2}, "/action/selectLight", ("i", 2))
mi("select_light_named", {"light": "Wash"}, "/action/selectLight", ("s", "Wash"))
mi("set_master_video", {"level": 0.5}, "/masterVideo", ("f", 0.5))
mi("set_master_audio", {"level": 0.75}, "/masterAudio", ("f", 0.75))
mi("set_master_dmx", {"level": 1.0}, "/masterDMX", ("f", 1.0))
mi("brush", {"x": 0.25, "y": 0.5}, "/action/brush", ("f", 0.25), ("f", 0.5), ("f", 1.0))
mi("enter_fullscreen", {}, "/action/enterFullscreen")
mi("exit_fullscreen", {}, "/action/exitFullscreen")
mi("show_test_card", {}, "/action/displayTestCard")
mi("hide_test_card", {}, "/action/hideTestCard")
mi("enable_workspace", {}, "/action/enableWorkspace")
mi("disable_workspace", {}, "/action/disableWorkspace")
mi("open_project", {"path": "/Users/show/Main.millumin"}, "/action/openProject", ("s", "/Users/show/Main.millumin"))
mi("save_project", {}, "/action/saveProject")
mi("save_project_as", {"path": "/Users/show/Backup.millumin"}, "/action/saveProject",
   ("s", "/Users/show/Backup.millumin"))
mi("quit", {}, "/action/quit")
mi("ping", {}, "/ping")
mi("set_layer_opacity", {"layer": "test", "opacity": 0.75}, "/layer:test/opacity", ("f", 0.75), **_U)
mi("set_layer_position", {"layer": "test", "x": 12.0, "y": 21.0}, "/layer:test/position/xy", ("f", 12.0), ("f", 21.0))
mi("set_layer_x", {"layer": "test", "x": 12.0}, "/layer:test/position/x", ("f", 12.0))
mi("set_layer_y", {"layer": "test", "y": 21.0}, "/layer:test/position/y", ("f", 21.0))
mi("set_layer_rotation", {"layer": "test", "degrees": 90.0}, "/layer:test/rotation", ("f", 90.0))
mi("set_layer_scale", {"layer": "test", "scale": 0.5}, "/layer:test/scale", ("f", 0.5))
mi("set_layer_effect", {"layer": "test", "effect": 1, "param": "red", "value": 0.5}, "/layer:test/effect1/red",
   ("f", 0.5))
mi("select_layer_by_name", {"layer": "test"}, "/layer:test/selected", ("i", 1))
mi("set_light_intensity", {"light": "Wash", "intensity": 0.5}, "/light:Wash/intensity", ("f", 0.5))
mi("set_property", {"target": "index:99", "property": "mapping/topLeft/x", "value": 10.0},
   "/index:99/mapping/topLeft/x", ("f", 10.0))
mi("adjust_property", {"target": "selectedLayer", "property": "position/x", "amount": 10.0},
   "/selectedLayer/position/x/+", ("f", 10.0))
mi("query_property", {"target": "layer:x", "property": "position"}, "/layer:x/position/?")
mi("start_media", {"layer": "boop1"}, "/layer:boop1/startMedia")
mi("start_media_column", {"layer": "boop1", "column": 5}, "/layer:boop1/startMedia", ("i", 5))
mi("start_media_named", {"layer": "boop1", "media": "Movie.mov"}, "/layer:boop1/startMedia", ("s", "Movie.mov"))
mi("pause_media", {"layer": "boop1"}, "/layer:boop1/pauseMedia")
mi("start_or_pause_media", {"layer": "boop1"}, "/layer:boop1/startOrPauseMedia")
mi("stop_media", {"layer": "boop1"}, "/layer:boop1/stopMedia")
mi("set_media_time", {"layer": "boop1", "seconds": 10.0}, "/layer:boop1/media/time", ("f", 10.0))
mi("set_media_normalized_time", {"layer": "boop1", "position": 0.5}, "/layer:boop1/media/normalizedTime", ("f", 0.5))
mi("set_media_speed", {"layer": "boop1", "speed": 0.5}, "/layer:boop1/media/speed", ("f", 0.5))
mi("set_media_text", {"layer": "Title", "text": "Welcome"}, "/layer:Title/media/text", ("s", "Welcome"))
mi("import_media", {"path": "/Users/show/media/clip.mov"}, "/selectedLayer/import", ("s", "/Users/show/media/clip.mov"))
mi("import_media_column", {"path": "/Users/show/media/clip.mov", "column": 2}, "/selectedLayer/import",
   ("s", "/Users/show/media/clip.mov"), ("i", 2))
mi("clear_layer_media", {}, "/selectedLayer/import", ("s", ""))
mi("clear_column_media", {"column": 4}, "/selectedLayer/import", ("s", ""), ("i", 4))
mi("import_and_start", {"path": "/Users/show/media/clip.mov", "column": 1}, "/selectedLayer/importAndStart",
   ("s", "/Users/show/media/clip.mov"), ("i", 1))

# ── Telemetry: feedback as the Feedback section gives it ──
telemetry(MI, "launched-column", inbound_hex=hexs(osc("/millumin/board/launchedColumn", ("i", 3), ("s", "Intro"))),
          expect_state={"board": {"column": 3, "column_name": "Intro", "column_playing": True}})
telemetry(MI, "stopped-column", inbound_hex=hexs(osc("/millumin/board/stoppedColumn", ("i", 3), ("s", "Intro"))),
          expect_state={"board": {"column": 3, "column_name": "Intro", "column_playing": False}})
telemetry(MI, "media-started", inbound_hex=hexs(osc("/millumin/layer:Background/mediaStarted",
                                                    ("i", 2), ("s", "Movie.mov"), ("f", 12.5))),
          expect_state={"layers": {"Background": {"media_index": 2, "media": "Movie.mov", "duration_s": 12.5,
                                                  "media_state": "started"}}})
telemetry(MI, "media-stopped", inbound_hex=hexs(osc("/millumin/layer:Background/mediaStopped",
                                                    ("i", 2), ("s", "Movie.mov"))),
          expect_state={"layers": {"Background": {"media_index": 2, "media": "Movie.mov", "media_state": "stopped"}}})
telemetry(MI, "media-time", inbound_hex=hexs(osc("/millumin/layer:Background/media/time", ("f", 4.5), ("f", 12.5))),
          expect_state={"layers": {"Background": {"time_s": 4.5, "duration_s": 12.5}}})
telemetry(MI, "opacity", inbound_hex=hexs(osc("/millumin/layer:Background/opacity", ("f", 0.75))),
          expect_state={"layers": {"Background": {"opacity": 0.75}}})
telemetry(MI, "light", inbound_hex=hexs(osc("/millumin/light:Wash/intensity", ("f", 0.5))),
          expect_state={"lights": {"Wash": {"intensity": 0.5}}})
