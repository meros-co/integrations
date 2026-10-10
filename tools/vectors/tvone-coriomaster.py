TV1 = "tvone-coriomaster"
# tvONE CORIOmaster command line API over TCP 10001: one command per line,
# CR LF terminated; a set is "<path> = <value>", a read is "<path>", a method
# is "<path>()", and every command ends with "!Done <path>" (tvONE CORIOmaster
# Commands 411.0.1). Expected lines are typed from the reference's syntax and
# examples (Window1.Input = Slot4.In1, Preset.Take = 1, Stbds.Stbd1.Take(),
# AddEvents(HDMI) ...), not taken from the spec.
_TV_ACK = {"ok": {"kind": "ack"}}


def _tv(command, input, lines, **extra):
    # A command of several lines sends each after the previous one's "!Done",
    # so the vector sees only the first; the others are listed for the reader.
    first = lines[0] if isinstance(lines, list) else lines
    text(TV1, command, input, first + "\r\n", **extra)


_tv("window_input", {"window": 1, "input": "Slot4.In1"}, "Window1.Input = Slot4.In1",
    device_reply="Window1.Input = Slot4.In1\r\n!Done Window1.Input\r\n", expect_result=_TV_ACK)
_tv("window_canvas", {"window": 1, "canvases": "Canvas1"}, "Window1.Canvas = Canvas1")
_tv("window_alias", {"window": 1, "alias": "DVDplayer"}, "Window1.Alias = DVDplayer")
_tv("window_width", {"window": 1, "width": 1280}, "Window1.CanWidth = 1280")
_tv("window_height", {"window": 1, "height": 720}, "Window1.CanHeight = 720")
_tv("window_x", {"window": 1, "x": 689}, "Window1.CanXCentre = 689")
_tv("window_y", {"window": 2, "y": -100}, "Window2.CanYCentre = -100")
_tv("window_geometry", {"window": 3, "width": 1920, "height": 1080, "x": 0, "y": 0},
    ["StartBatch", "Window3.CanWidth = 1920", "Window3.CanHeight = 1080", "Window3.CanXCentre = 0",
     "Window3.CanYCentre = 0", "EndBatch"])
_tv("window_zorder", {"window": 1, "zorder": 1}, "Window1.Zorder = 1")
_tv("window_rotation", {"window": 1, "degrees": 90}, "Window1.RotateDeg = 90")
_tv("window_quality", {"window": 1, "quality": "2048"}, "Window1.WDPQ = 2048")
_tv("window_border_width", {"window": 1, "width": 1}, "Window1.BdrPixWidth = 1")
_tv("window_border_color", {"window": 1, "rgb": 16711680}, "Window1.BdrRGB = 16711680")
_tv("window_hflip", {"window": 1, "enabled": "Off"}, "Window1.HFlip = Off")
_tv("window_vflip", {"window": 1}, "Window1.VFlip = On")
_tv("window_fade", {"window": 1, "level": 256}, "Window1.FTB = 256")
_tv("window_animate_fade", {"window": 1}, "Window1.SCFTB = On")
_tv("window_animate_hshrink", {"window": 1, "enabled": "Off"}, "Window1.SCHShrink = Off")
_tv("window_animate_vshrink", {"window": 1, "enabled": "Off"}, "Window1.SCVShrink = Off")
_tv("window_animate_spin", {"window": 1, "spin": -3}, "Window1.SCSpin = -3")
_tv("window_keying_layer", {"window": 1, "layer": 2}, "Window1.KeyingLayer = 2")
_tv("window_luma_key", {"window": 1, "min": 0, "max": 40, "softness": 5},
    ["StartBatch", "Window1.KeyingYMin = 0", "Window1.KeyingYMax = 40", "Window1.KeyingYSoft = 5", "EndBatch"])
_tv("get_window", {"window": 1}, "Window1")
_tv("canvas_alias", {"canvas": 1, "alias": "Main"}, "Canvas1.Alias = Main")
_tv("canvas_windows", {"canvas": 1, "windows": "Window1,Window2"}, "Canvas1.WindowList = Window1,Window2")
_tv("canvas_layouts", {"canvas": 1, "layouts": "Layout1"}, "Canvas1.LayoutList = Layout1")
_tv("canvas_audio_mode", {"canvas": 1, "mode": "FromSource"}, "Canvas1.AudioMode = FromSource")
_tv("canvas_audio_source", {"canvas": 1, "source": "Slot1.In1"}, "Canvas1.AudioSource = Slot1.In1")
_tv("canvas_audio_follow_window", {"canvas": 1, "window": 1}, "Canvas1.AudioFollowWindow = 1")
_tv("canvas_audio_mute", {"canvas": 1}, "Canvas1.AudioMute = On")
_tv("canvas_audio_volume", {"canvas": 1, "volume": 100}, "Canvas1.AudioVolume = 100")
_tv("get_canvas", {"canvas": 1}, "Canvas1")
_tv("layout_canvas", {"layout": 1, "canvas": 1}, "Layout1.Canvas = Canvas1")
_tv("layout_outputs", {"layout": 1, "outputs": "Slot13.Out1,Slot16.Out2"}, "Layout1.OutputList = Slot13.Out1,Slot16.Out2")
_tv("layout_size", {"layout": 1, "width": 4096, "height": 4096},
    ["StartBatch", "Layout1.CanWidth4kUnit = 4096", "Layout1.CanHeight4kUnit = 4096", "EndBatch"])
_tv("layout_centre", {"layout": 1, "x": 0, "y": 0}, ["StartBatch", "Layout1.CanXCentre = 0", "Layout1.CanYCentre = 0", "EndBatch"])
_tv("storyboard_take", {"storyboard": 1}, "Stbds.Stbd1.Take()",
    device_reply="!Done Stbds.Stbd1.Take()\r\n", expect_result=_TV_ACK)
_tv("storyboard_save", {"storyboard": 1}, "Stbds.Stbd1.Save()")
_tv("storyboard_remove", {"storyboard": 1}, "Stbds.Stbd1.Remove()")
_tv("storyboard_name", {"storyboard": 1, "name": "start"}, "Stbds.Stbd1.Name = start")
_tv("preset_take", {"preset": 1}, "Preset.Take = 1", device_reply="Preset.Take = 1\r\n!Done Preset.Take\r\n",
    expect_result=_TV_ACK)
_tv("preset_save", {"preset": 3}, ["Preset.Read = 3", "Preset.SaveRead()"])
_tv("preset_name", {"preset": 3, "name": "side_by_side"}, ["Preset.Read = 3", "Preset.NameRead = side_by_side"])
_tv("preset_duration", {"preset": 3, "ms": 3000}, ["Preset.Read = 3", "Preset.DurationRead = 3000"])
_tv("preset_remove", {"preset": 3}, ["Preset.Read = 3", "Preset.RmvPresetFileRead()"])
_tv("preset_list", {}, "Preset.PresetList()")
_tv("preset_save_all", {}, "Preset.SaveAllPresets()")
_tv("preset_restore_all", {}, "Preset.RestoreAllPresets()")
_tv("preset_remove_all", {}, "Preset.RemovePresetFiles()")
_tv("get_property", {"path": "CORIOmax.Model_Name"}, "CORIOmax.Model_Name")
_tv("set_property", {"path": "Slot4.Out1.Mute", "value": "Off"}, "Slot4.Out1.Mute = Off")
# An error line fails the command (the form is assumed; see the quirks).
_tv("add_events", {"category": "HDMI"}, "AddEvents(HDMI)", device_reply="!Error -1 : Unknown category\r\n",
    expect_result={"error": {"error": "device_error"}})
_tv("remove_events", {"category": "HDMI"}, "RemoveEvents(HDMI)")
_tv("get_device_info", {}, "CORIOmax")

# Telemetry: events and echoed values (Commands 411 event and property examples).
telemetry(TV1, "window-input-event", inbound="!Event WINDOW,INPUT,Window1,Slot5.In1\r\n",
          expect_state={"windows": {"1": {"input": "Slot5.In1"}}})
telemetry(TV1, "preset-take-event", inbound="!Event PRESET,TAKE,1\r\n", expect_state={"preset": {"current": 1}})
telemetry(TV1, "storyboard-event", inbound="!Event STBD, ISCURRENT_CHANGED,Stbd1,1\r\n",
          expect_state={"storyboards": {"1": {"current": True}}})
telemetry(TV1, "window-values", inbound="Window1.Input = Slot4.In1\r\nWindow1.CanWidth = 1280\r\nWindow1.CanXCentre = -689\r\n",
          expect_state={"windows": {"1": {"input": "Slot4.In1", "width": 1280, "x": -689}}})
telemetry(TV1, "preset-list", inbound="Routing.Preset.PresetList[2]=side_by_side,Canvas1,3000\r\n",
          expect_state={"presets": {"2": {"name": "side_by_side", "canvas": "Canvas1", "duration": 3000}}})
telemetry(TV1, "device", inbound="CORIOmax.Model_Name = CORIOmaster\r\nCORIOmax.Model_Number = C3-540\r\n",
          expect_state={"device": {"model_name": "CORIOmaster", "model_number": "C3-540"}})
_tv("layout_alias", {"layout": 1, "alias": "Wall"}, "Layout1.Alias = Wall")
_tv("storyboard_canvas", {"storyboard": 1, "canvas": 1}, "Stbds.Stbd1.Canvas = Canvas1")

# Lists: each entry is read whole (Commands 411, Windows, Canvases, Layouts and Stbds lists).
telemetry(TV1, "windows-list", inbound="Windows.Window3 = <...>\r\n", expect_state={},
          expect_then_send=["Window3\r\n"])
telemetry(TV1, "canvases-list", inbound="Routing.Canvases.Canvas2 = <...>\r\n", expect_state={},
          expect_then_send=["Canvas2\r\n"])
telemetry(TV1, "layouts-list", inbound="Layouts.Layout1 = <...>\r\n", expect_state={},
          expect_then_send=["Layout1\r\n"])
telemetry(TV1, "storyboards-list", inbound="Stbds.Stbd12 = <...>\r\n", expect_state={},
          expect_then_send=["Stbds.Stbd12\r\n"])
telemetry(TV1, "window-properties",
          inbound="Window1.Status = FREE\r\nWindow1.RotateDeg = 90\r\nWindow1.WDPQ = 2048\r\nWindow1.BdrPixWidth = 1\r\n"
                  "Window1.BdrRGB = 16711680\r\nWindow1.HFlip = On\r\nWindow1.VFlip = Off\r\nWindow1.FTB = 0\r\n"
                  "Window1.SCFTB = Off\r\nWindow1.SCHShrink = On\r\nWindow1.SCVShrink = Off\r\nWindow1.SCSpin = -3\r\n"
                  "Window1.KeyingLayer = 2\r\nWindow1.KeyingYMin = 0\r\nWindow1.KeyingYMax = 40\r\n"
                  "Window1.KeyingYSoft = 5\r\nWindow1.Zorder = 1\r\nWindow1.Canvas = Canvas1\r\nWindow1.Alias = NULL\r\n",
          expect_state={"windows": {"1": {
              "status": "FREE", "rotation": 90, "quality": 2048, "border_width": 1, "border_color": 16711680,
              "hflip": True, "vflip": False, "fade": 0, "animate_fade": False, "animate_hshrink": True,
              "animate_vshrink": False, "animate_spin": -3, "keying_layer": 2, "luma_key_min": 0,
              "luma_key_max": 40, "luma_key_softness": 5, "zorder": 1, "canvas": "Canvas1", "alias": "NULL"}}})
telemetry(TV1, "canvas-properties",
          inbound="Canvas1.Status = FREE\r\nCanvas1.Alias = Main\r\nCanvas1.WindowList = Window1,Window2\r\n"
                  "Canvas1.LayoutList = Layout1\r\nCanvas1.StbdCurrent = NULL\r\nCanvas1.AudioFollowWindow = 0\r\n"
                  "Canvas1.AudioMute = Off\r\nCanvas1.AudioSource = NULL\r\nCanvas1.AudioMode = FromSource\r\n"
                  "Canvas1.AudioVolume = 100\r\n",
          expect_state={"canvases": {"1": {
              "status": "FREE", "alias": "Main", "windows": "Window1,Window2", "layouts": "Layout1",
              "current_storyboard": "NULL", "audio_follow_window": 0, "audio_mute": False, "audio_source": "NULL",
              "audio_mode": "FromSource", "audio_volume": 100}}})
telemetry(TV1, "canvas-audio-mode-lower", inbound="canvas1.AudioMode = FollowWindow\r\n",
          expect_state={"canvases": {"1": {"audio_mode": "FollowWindow"}}})
telemetry(TV1, "layout-properties",
          inbound="Layout1.Status = FREE\r\nLayout1.Alias = NULL\r\nLayout1.Canvas = Canvas1\r\n"
                  "Layout1.CanWidth4kUnit = 4096\r\nLayout1.CanHeight4kUnit = 4096\r\nLayout1.CanXCentre = 0\r\n"
                  "Layout1.CanYCentre = -10\r\nLayout1.StbdActive = No\r\nLayout1.OutputList = Slot13.Out1,Slot16.Out2\r\n"
                  "Layout1.Mode = Normal\r\n",
          expect_state={"layouts": {"1": {
              "status": "FREE", "alias": "NULL", "canvas": "Canvas1", "width": 4096, "height": 4096, "x": 0, "y": -10,
              "storyboard_active": False, "outputs": "Slot13.Out1,Slot16.Out2", "mode": "Normal"}}})
telemetry(TV1, "storyboard-properties",
          inbound="Stbds.Stbd1.Name = start\r\nStbds.Stbd1.Canvas = Canvas1\r\nStbds.Stbd1.IsCurrent = No\r\n",
          expect_state={"storyboards": {"1": {"name": "start", "canvas": "Canvas1", "current": False}}})
telemetry(TV1, "canvas-mute-event", inbound="!Event CANVAS,PROPERTY_CHANGED,Canvas1,AudioMute,On\r\n",
          expect_state={"canvases": {"1": {"audio_mute": True}}})
telemetry(TV1, "canvas-mode-event", inbound="!Event CANVAS,PROPERTY_CHANGED,Canvas1,AudioMode,FromSource\r\n",
          expect_state={"canvases": {"1": {"audio_mode": "FromSource"}}})
telemetry(TV1, "canvas-follow-event", inbound="!Event CANVAS,PROPERTY_CHANGED,Canvas1,AudioFollowWindow,Window1\r\n",
          expect_state={"canvases": {"1": {"audio_follow_window": 1}}})
telemetry(TV1, "canvas-source-event", inbound="!Event CANVAS,PROPERTY_CHANGED,Canvas1,AudioSource,Slot1.In1\r\n",
          expect_state={"canvases": {"1": {"audio_source": "Slot1.In1"}}})
telemetry(TV1, "canvas-volume-event", inbound="!Event CANVAS,PROPERTY_CHANGED,Canvas1,AudioVolume,50\r\n",
          expect_state={"canvases": {"1": {"audio_volume": 50}}})
telemetry(TV1, "canvas-storyboard-event", inbound="!Event CANVAS, STBDCURRENT_CHANGED,Canvas1,Stbd1,\r\n",
          expect_state={"canvases": {"1": {"current_storyboard": "Stbd1"}}})
telemetry(TV1, "preset-complete-event", inbound="!Event PRESET, COMPLETE,1\r\n",
          expect_state={"preset": {"completed": 1}}, expect_then_send=["Windows\r\n"])
telemetry(TV1, "storyboard-event-reread", inbound="!Event STBD,ISCURRENT_CHANGED,Stbd2,0\r\n",
          expect_state={"storyboards": {"2": {"current": False}}}, expect_then_send=["Windows\r\n"])
telemetry(TV1, "preset-save-event", inbound="!Event PRESET,SAVE,3\r\n", expect_state={},
          expect_then_send=["Preset.PresetList()\r\n"])
telemetry(TV1, "preset-remove-event", inbound="!Event PRESET, REMOVE,3\r\n",
          state_before={"presets": {"3": {"name": "side_by_side", "canvas": "Canvas1", "duration": 3000},
                                    "4": {"name": "two", "canvas": "Canvas1", "duration": 1000}}},
          expect_state={"presets": {"4": {"name": "two", "canvas": "Canvas1", "duration": 1000}}})
telemetry(TV1, "preset-edit-reread", inbound="Preset.NameRead = side_by_side\r\n", expect_state={},
          expect_then_send=["Preset.PresetList()\r\n"])
