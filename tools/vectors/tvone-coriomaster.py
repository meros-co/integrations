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
