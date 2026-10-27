# ioversal VERTEX (ioversal-vertex): VERTEX script commands, one per line
# ended by CR, on the script server's TCP 50009, as the Remote Script API page
# writes them (Playback1.Play, PME1.Playback1.MixLevel.FadeValue 1,5,
# System2.ResetVideoInputs, return Playback1.TimeCode.Value). Only return
# commands are answered.
VX = "ioversal-vertex"
U = {"ok": {"kind": "unverified"}}


def _vx(command, input, wire):
    text(VX, command, input, wire + "\r", expect_result=U)


_vx("run_script", {"command": "Script1"}, "Script1")
TC = '{"TimeSpan":"00:00:00","Days":0,"Hours":0,"Minutes":0,"Seconds":0,"Frames":0,"FPS":25.0,"TotalFrames":0}'
text(VX, "query", {"expression": "Playback1.TimeCode.Value"}, "return Playback1.TimeCode.Value\r",
     device_reply=TC + "\r", expect_result={"ok": {"kind": "value", "value": TC}})
text(VX, "list_items", {"type": "Playback"}, "return ListItems.Playback\r", device_reply='["Playback1","Playback2"]\r',
     expect_result={"ok": {"kind": "value", "value": '["Playback1","Playback2"]'}})
for cmd, method in [("playback_play", "Play"), ("playback_pause", "Pause"), ("playback_stop", "Stop"),
                    ("playback_toggle_play", "TogglePlay"), ("playback_next_cue", "GotoNextCue"),
                    ("playback_previous_cue", "GotoPrevCue"), ("playback_first_cue", "GotoFirstCue"),
                    ("playback_last_cue", "GotoLastCue"), ("playback_next_frame", "GotoNextFrame"),
                    ("playback_previous_frame", "GotoPreviousFrame"), ("playback_ignore_next_cue", "IgnoreNextCue"),
                    ("playback_enable_all_cues", "EnableAllCues")]:
    _vx(cmd, {"playback": "Playback1"}, "Playback1." + method)
_vx("playback_goto_cue", {"playback": "Playback3", "cue": 1}, "Playback3.GotoCue 1")
_vx("playback_goto_cue_play", {"playback": "PME1.Playback1", "cue": 4}, "PME1.Playback1.GotoCuePlay 4")
_vx("playback_goto_frame", {"playback": "Playback1", "frame": 250}, "Playback1.GotoFrame 250")
_vx("playback_fade_to_cue", {"playback": "Playback1", "cue": 2, "preload": 1.0, "fade": 2.5}, "Playback1.FadeToCue 2,1.000,2.500")
_vx("set_value", {"path": "ClipContainer1.Opacity", "value": "1"}, "ClipContainer1.Opacity.Value = 1")
_vx("fade_value", {"path": "PME1.Playback1.MixLevel", "value": 1.0, "seconds": 5.0}, "PME1.Playback1.MixLevel.FadeValue 1.0000,5.000")
_vx("set_prog_value", {"path": "Playback1.ClipContainer1.Opacity", "value": "0"}, "Playback1.ClipContainer1.Opacity.ProgValue = 0")
_vx("fade_prog_value", {"path": "Playback1.ClipContainer1.Opacity", "value": 1.0, "seconds": 2.0},
    "Playback1.ClipContainer1.Opacity.FadeProgValue 1.0000,2.000")
for cmd, method in [("system_enter_fullscreen", "EnterFullScreen"), ("system_leave_fullscreen", "LeaveFullScreen"),
                    ("system_reset_video_inputs", "ResetVideoInputs"), ("system_force_resync", "ForceResync"),
                    ("system_windows_restart", "WindowsRestart"), ("system_windows_shutdown", "WindowsShutdown"),
                    ("system_start_vertex", "StartVertex"), ("system_stop_vertex", "StopVertex")]:
    _vx(cmd, {"system": "System2"}, "System2." + method)
_vx("project_save", {}, "ProjectSave")
_vx("project_save_backup", {}, "ProjectSaveBackup")
_vx("cancel_scripts", {}, "Cancel")
_vx("popup_notification", {"message": "Doors open"}, "PopupNotification Doors open")
_vx("control_view_goto_page", {"page": "Page2"}, "ControlView1.GotoPage Page2,0.00")
_vx("control_click", {"control": "ControlView1.Controls.ClickButton1"}, "ControlView1.Controls.ClickButton1.Click")

telemetry(VX, "playback-list", inbound='["Playback1","Playback2"]\r', request="return ListItems.Playback",
          expect_state={"playback_list": {"0": "Playback1", "1": "Playback2"}},
          expect_then_send=["return Playback1.TimeCode.Value\r", "return Playback2.TimeCode.Value\r"])
telemetry(VX, "timecode", request="return Playback1.TimeCode.Value",
          inbound='{"TimeSpan":"00:01:20","Days":0,"Hours":0,"Minutes":1,"Seconds":20,"Frames":12,"FPS":25.0,"TotalFrames":2012}\r',
          expect_state={"playbacks": {"Playback1": {"timecode": "00:01:20", "fps": 25.0, "frames": 2012}}})
