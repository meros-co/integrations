# Mitti OSC control (imimot-mitti): one vector per command. Addresses are
# those of Mitti's "OSC/UDP Controls" help page (/mitti/<command> and
# /mitti/<cue>/<command>, /mitti/play, /mitti/togglePlay with an int) and,
# where the page gives no name, of the MIT-licensed Bitfocus Companion module,
# which sends values as OSC strings and the cue index as an int. Mitti never
# answers, so every command is unverified. Feedback vectors use the help
# page's read-only messages and the value-carrying forms the Companion module
# reads.
MT = "imimot-mitti"
_MU = {"expect_result": {"ok": {"kind": "unverified"}}}


def mt(command, input, address, *args, **extra):
    binary(MT, command, input, osc(address, *args), **extra)


mt("play", {}, "/mitti/play", **_MU)
mt("set_playing", {"playing": True}, "/mitti/togglePlay", ("i", 1))
mt("set_playing", {"playing": False}, "/mitti/togglePlay", ("i", 0), file="set_playing-pause")
mt("stop", {}, "/mitti/stop")
mt("panic", {}, "/mitti/panic")
mt("rewind", {}, "/mitti/rewind")
mt("previous_cue", {}, "/mitti/jumpToPrevCue")
mt("next_cue", {}, "/mitti/jumpToNextCue")
mt("jump_to_selected_cue", {}, "/mitti/jumpToSelectedCue")
mt("select_previous_cue", {}, "/mitti/selectPrevCue")
mt("select_next_cue", {}, "/mitti/selectNextCue")
mt("goto_before_end", {"seconds": "30"}, "/mitti/goto30")
mt("goto_before_end", {"seconds": "10"}, "/mitti/goto10", file="goto_before_end-10")
mt("play_selected_cue", {}, "/mitti/playSelectedCue")
mt("play_selected_cue", {"force_cut": True}, "/mitti/playSelectedCueForceCut", file="play_selected_cue-force-cut")
mt("play_cue", {"cue": "12"}, "/mitti/playCueWithCueID", ("s", "12"))
mt("play_cue", {"cue": "A7F3", "force_cut": True}, "/mitti/playCueWithCueIDForceCut", ("s", "A7F3"),
   file="play_cue-force-cut")
mt("play_cue_named", {"name": "Walk In"}, "/mitti/playCueWithName", ("s", "Walk In"))
mt("play_cue_named", {"name": "Walk In", "force_cut": True}, "/mitti/playCueWithNameForceCut", ("s", "Walk In"),
   file="play_cue_named-force-cut")
mt("play_cue_at_index", {"index": 3}, "/mitti/playCueAtIndex", ("i", 3))
mt("play_cue_at_index", {"index": 999, "force_cut": True}, "/mitti/playCueAtIndexForceCut", ("i", 999),
   file="play_cue_at_index-force-cut")
mt("jump_to_cue_named", {"name": "Finale"}, "/mitti/jumpToCueWithName", ("s", "Finale"))
mt("set_fullscreen", {"enabled": True}, "/mitti/fullscreenOn")
mt("set_fullscreen", {"enabled": False}, "/mitti/fullscreenOff", file="set_fullscreen-off")
mt("toggle_fullscreen", {}, "/mitti/toggleFullscreen")
mt("set_playlist_loop", {"enabled": True}, "/mitti/loopOn")
mt("set_playlist_loop", {"enabled": False}, "/mitti/loopOff", file="set_playlist_loop-off")
mt("toggle_playlist_loop", {}, "/mitti/toggleLoop")
mt("set_transition_on_play", {"enabled": True}, "/mitti/transitionOnPlayOn")
mt("set_transition_on_play", {"enabled": False}, "/mitti/transitionOnPlayOff", file="set_transition_on_play-off")
mt("toggle_transition_on_play", {}, "/mitti/toggleTransitionOnPlay")
mt("resend_feedback", {}, "/mitti/resendOSCFeedback")
mt("auto_fade", {}, "/mitti/autoFade")
mt("set_main_fader", {"level": 0.5}, "/mitti/mainFader", ("s", "0.50"))
mt("set_playhead", {"position": 0.25}, "/mitti/playhead", ("s", "0.250"))
mt("scrub_playhead", {"amount": "+01:00"}, "/mitti/playheadScrub", ("s", "+01:00"))
mt("locate_playhead", {"timecode": "00:01:30:12"}, "/mitti/locateCurrentCue", ("s", "00:01:30:12"))
mt("set_video_outputs", {"enabled": True}, "/mitti/videoOutputsOn")
mt("set_video_outputs", {"enabled": False}, "/mitti/videoOutputsOff", file="set_video_outputs-off")
mt("toggle_video_outputs", {}, "/mitti/toggleVideoOutputs")
mt("set_audio_outputs", {"enabled": True}, "/mitti/unmuteAudio")
mt("set_audio_outputs", {"enabled": False}, "/mitti/muteAudio", file="set_audio_outputs-off")
mt("toggle_audio_outputs", {}, "/mitti/toggleAudio")
mt("set_in_from_playhead", {}, "/mitti/setInFromPlayhead")
mt("set_out_from_playhead", {}, "/mitti/setOutFromPlayhead")
mt("ping", {}, "/mitti/ping")
mt("jump_to_cue", {"cue": "5"}, "/mitti/5/jump")
mt("select_cue", {"cue": "next"}, "/mitti/next/select")
mt("set_cue_option", {"cue": "3", "option": "loop", "enabled": True}, "/mitti/3/loopOn")
mt("set_cue_option", {"cue": "all", "option": "pauseAtEnd", "enabled": False}, "/mitti/all/pauseAtEndOff",
   file="set_cue_option-all-off")
mt("toggle_cue_option", {"cue": "current", "option": "audio"}, "/mitti/current/toggleAudio")
mt("toggle_cue_option", {"cue": "2", "option": "videoFx"}, "/mitti/2/toggleVideoFx", file="toggle_cue_option-video-fx")
mt("set_cue_goto_target", {"cue": "4", "target": "1"}, "/mitti/4/setGotoToCueID", ("s", "1"))
mt("set_cue_scale", {"cue": "1", "percent": 150}, "/mitti/1/scaleAsPercent", ("s", "150"))
mt("set_cue_scale_horizontal", {"cue": "1", "percent": 80}, "/mitti/1/scaleHAsPercent", ("s", "80"))
mt("set_cue_scale_vertical", {"cue": "1", "percent": 120}, "/mitti/1/scaleVAsPercent", ("s", "120"))
mt("set_cue_position_x", {"cue": "selected", "pixels": -240}, "/mitti/selected/posXAsPixels", ("s", "-240"))
mt("set_cue_position_y", {"cue": "selected", "pixels": 60}, "/mitti/selected/posYAsPixels", ("s", "60"))
mt("set_cue_crop", {"cue": "1", "edge": "left", "pixels": 100}, "/mitti/1/cropLeftAsPixels", ("s", "100"))
mt("set_cue_crop", {"cue": "1", "edge": "bottom", "pixels": 20}, "/mitti/1/cropBottomAsPixels", ("s", "20"),
   file="set_cue_crop-bottom")
mt("set_cue_rotation", {"cue": "1", "degrees": -90}, "/mitti/1/rotateAsDegrees", ("s", "-90"))
mt("set_cue_hue", {"cue": "1", "degrees": 45}, "/mitti/1/hueAsDegrees", ("s", "45"))
mt("set_cue_color", {"cue": "1", "property": "saturation", "percent": -50}, "/mitti/1/saturationAsPercent", ("s", "-50"))
mt("set_cue_color", {"cue": "1", "property": "contrast", "percent": 20}, "/mitti/1/contrastAsPercent", ("s", "20"),
   file="set_cue_color-contrast")
mt("set_cue_opacity", {"cue": "current", "percent": 75}, "/mitti/current/opacityAsPercent", ("s", "75"))
mt("set_cue_volume", {"cue": "current", "db": -3.5}, "/mitti/current/volumeAsDecibels", ("s", "-3.5"))
mt("set_cue_playback_speed", {"cue": "2", "percent": 50}, "/mitti/2/playbackSpeed", ("s", "50"))

# ── Telemetry ──
telemetry(MT, "playing", expect_connect_wire_hex=[hexs(osc("/mitti/resendOSCFeedback"))],
          inbound_hex=hexs(osc("/mitti/togglePlay", ("i", 1))), expect_state={"playing": True})
telemetry(MT, "paused", inbound_hex=hexs(osc("/mitti/togglePlay", ("f", 0.0))), expect_state={"playing": False})
telemetry(MT, "time", inbound_hex=hexs(osc("/mitti/time", ("s", "00:12:03:10"))), expect_state={"time": "00:12:03:10"})
telemetry(MT, "cue-time-left", inbound_hex=hexs(osc("/mitti/cueTimeLeft", ("s", "-00:02:10:05"))),
          expect_state={"current_cue": {"time_left": "-00:02:10:05"}})
telemetry(MT, "cue-time-elapsed", inbound_hex=hexs(osc("/mitti/cueTimeElapsed", ("s", "00:00:41:20"))),
          expect_state={"current_cue": {"time_elapsed": "00:00:41:20"}})
telemetry(MT, "current-cue-trt", inbound_hex=hexs(osc("/mitti/currentCueTRT", ("s", "00:02:52:00"))),
          expect_state={"current_cue": {"trt": "00:02:52:00"}})
telemetry(MT, "current-cue-name", inbound_hex=hexs(osc("/mitti/currentCueName", ("s", "Opener"))),
          expect_state={"current_cue": {"name": "Opener"}})
telemetry(MT, "current-cue-id", inbound_hex=hexs(osc("/mitti/currentCueID", ("s", "1"))),
          expect_state={"current_cue": {"id": "1"}})
telemetry(MT, "previous-cue-name", inbound_hex=hexs(osc("/mitti/previousCueName", ("s", "-"))),
          expect_state={"previous_cue": {"name": "-"}})
telemetry(MT, "next-cue-name", inbound_hex=hexs(osc("/mitti/nextCueName", ("s", "Walk In"))),
          expect_state={"next_cue": {"name": "Walk In"}})
telemetry(MT, "selected-cue-name", inbound_hex=hexs(osc("/mitti/selectedCueName", ("s", "Finale"))),
          expect_state={"selected_cue": {"name": "Finale"}})
telemetry(MT, "selected-cue-id", inbound_hex=hexs(osc("/mitti/selectedCueID", ("s", "9"))),
          expect_state={"selected_cue": {"id": "9"}})
telemetry(MT, "playhead", inbound_hex=hexs(osc("/mitti/playhead", ("f", 0.5))), expect_state={"playhead": 0.5})
telemetry(MT, "main-fader", inbound_hex=hexs(osc("/mitti/mainFader", ("f", 0.25))), expect_state={"main_fader": 0.25})
telemetry(MT, "video-outputs", inbound_hex=hexs(osc("/mitti/toggleVideoOutputs", ("i", 0))),
          expect_state={"video_outputs": False})
telemetry(MT, "audio-outputs", inbound_hex=hexs(osc("/mitti/toggleAudio", ("i", 1))), expect_state={"audio_outputs": True})
telemetry(MT, "fullscreen", inbound_hex=hexs(osc("/mitti/toggleFullscreen", ("i", 1))), expect_state={"fullscreen": True})
telemetry(MT, "playlist-loop", inbound_hex=hexs(osc("/mitti/toggleLoop", ("i", 0))), expect_state={"playlist_loop": False})
telemetry(MT, "transition-on-play", inbound_hex=hexs(osc("/mitti/toggleTransitionOnPlay", ("i", 1))),
          expect_state={"transition_on_play": True})
telemetry(MT, "in-from-playhead", inbound_hex=hexs(osc("/mitti/inFromPlayheadEnabled", ("i", 1))),
          expect_state={"in_from_playhead_enabled": True})
telemetry(MT, "out-from-playhead", inbound_hex=hexs(osc("/mitti/outFromPlayheadEnabled", ("i", 0))),
          expect_state={"out_from_playhead_enabled": False})
telemetry(MT, "cue-name", inbound_hex=hexs(osc("/mitti/3/cueName", ("s", "Interval"))),
          expect_state={"cues": {"3": {"name": "Interval"}}})
telemetry(MT, "cue-deleted", inbound_hex=hexs(osc("/mitti/3/deleted", ("i", 1))),
          state_before={"cues": {"3": {"name": "Interval", "loop": True}, "4": {"name": "Finale"}}},
          expect_state={"cues": {"4": {"name": "Finale"}}})
for flag, key in [("Audio", "audio"), ("FadeIn", "fade_in"), ("FadeOut", "fade_out"), ("Goto", "goto"),
                  ("Loop", "loop"), ("PauseAtBeginning", "pause_at_beginning"), ("PauseAtEnd", "pause_at_end"),
                  ("Transition", "transition"), ("VideoFx", "video_fx")]:
    telemetry(MT, f"cue-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"/mitti/2/toggle{flag}", ("i", 1))),
              expect_state={"cues": {"2": {key: True}}})
telemetry(MT, "current-cue-loop-off", inbound_hex=hexs(osc("/mitti/current/toggleLoop", ("f", 0.0))),
          expect_state={"cues": {"current": {"loop": False}}})
for addr, key, value in [("scaleAsPercent", "scale", 150.0), ("scaleHAsPercent", "scale_horizontal", 80.0),
                         ("scaleVAsPercent", "scale_vertical", 120.0), ("posXAsPixels", "position_x", -240.0),
                         ("posYAsPixels", "position_y", 60.0), ("cropLeftAsPixels", "crop_left", 100.0),
                         ("cropRightAsPixels", "crop_right", 4.0), ("cropTopAsPixels", "crop_top", 8.0),
                         ("cropBottomAsPixels", "crop_bottom", 20.0), ("rotateAsDegrees", "rotation", -90.0),
                         ("hueAsDegrees", "hue", 45.0), ("saturationAsPercent", "saturation", -50.0),
                         ("vibranceAsPercent", "vibrance", 10.0), ("brightnessAsPercent", "brightness", 5.0),
                         ("contrastAsPercent", "contrast", 20.0), ("opacityAsPercent", "opacity", 75.0),
                         ("volumeAsDecibels", "volume_db", -3.5), ("playbackSpeed", "playback_speed", 50.0)]:
    telemetry(MT, f"cue-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"/mitti/1/{addr}", ("f", value))),
              expect_state={"cues": {"1": {key: value}}})
telemetry(MT, "cue-goto-target", inbound_hex=hexs(osc("/mitti/4/setGotoToCueID", ("s", "1"))),
          expect_state={"cues": {"4": {"goto_target": "1"}}})
