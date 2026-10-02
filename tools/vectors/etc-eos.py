E = "etc-eos"
# ETC Eos family: OSC 1.0 over TCP 3032, each packet preceded by its 32-bit
# big-endian length (Eos v3.3.10 User Manual, Eos OSC Setup: "OSC 1.0
# (packet-length headers)"). Addresses below are typed from the manual's OSC
# Dictionary, OSC Eos Control, OSC Third-Party Integration and OSC Get pages,
# not copied from the spec. Inbound telemetry is the unframed OSC packet, as the
# engine is fed it.


def e(command, input, *messages, **extra):
    """messages: (address, *args) tuples, each framed with its length."""
    frames = [length_prefixed(osc(m[0], *m[1:])) for m in messages]
    binary(E, command, input, frames if len(frames) > 1 else frames[0], **extra)


UID = "B0BAE0A0-3BBE-4004-888B-F61CA125D0B0"   # the UID example in OSC Third-Party Integration
DOWN, UP = ("f", 1.0), ("f", 0.0)

# Connection, OSC user, subscriptions, filters (OSC Dictionary inputs; Using OSC with Eos, OSC Ping).
e("ping", {"text": "hello"}, ("/eos/ping", ("s", "hello")),
  device_reply_hex=hexs(osc("/eos/out/ping", ("s", "hello"))),
  expect_result={"ok": {"kind": "value", "value": "hello"}})
e("reset", {}, ("/eos/reset",), expect_result={"ok": {"kind": "unverified"}})
e("set_osc_user", {"user": 2}, ("/eos/user", ("i", 2)))
e("set_osc_user_auto", {}, ("/eos/user/auto",))
e("subscribe", {}, ("/eos/subscribe", ("i", 1)))
e("subscribe_param", {"param": "red"}, ("/eos/subscribe/param/red", ("i", 1)))
e("add_filter", {"filter": "/eos/out/param/*"}, ("/eos/filter/add", ("s", "/eos/out/param/*")))
e("remove_filter", {"filter": "/eos/out/param/*"}, ("/eos/filter/remove", ("s", "/eos/out/param/*")))
e("clear_filters", {}, ("/eos/filter/clear",))

# Command line (OSC Command and Event).
e("command", {"text": "Chan 1 At 75#"}, ("/eos/cmd", ("s", "Chan 1 At 75#")))
e("new_command", {"text": "Chan 1 At 75#"}, ("/eos/newcmd", ("s", "Chan 1 At 75#")))
e("command_event", {"text": "Chan 1 At 75#"}, ("/eos/event", ("s", "Chan 1 At 75#")))
e("new_command_event", {"text": "Chan 1 At 75#"}, ("/eos/newevent", ("s", "Chan 1 At 75#")))
e("fire_show_control_event", {"input": "Event1"}, ("/eos/sc/Event1",))

# Keys and softkeys: 1.0 = down, 0.0 = up (Eos OSC Conventions, Button Edge).
e("go", {}, ("/eos/key/go_0", DOWN), ("/eos/key/go_0", UP), expect_result={"ok": {"kind": "unverified"}})
e("press_key", {"key": "select_active"}, ("/eos/key/select_active", DOWN), ("/eos/key/select_active", UP))
e("key_down", {"key": "shift"}, ("/eos/key/shift", DOWN))
e("key_up", {"key": "shift"}, ("/eos/key/shift", UP))
e("press_softkey", {"softkey": 1}, ("/eos/softkey/1", DOWN), ("/eos/softkey/1", UP))

# Cues and cue lists (OSC Cues and Cue List Banks).
e("fire_cue", {"cue_list": 2, "cue": "18.2"}, ("/eos/cue/2/18.2/fire",))
e("fire_cue_part", {"cue_list": 1, "cue": "1.5", "part": 2}, ("/eos/cue/1/1.5/2/fire",))
e("fire_cue_current_list", {"cue": "1.5"}, ("/eos/cue/1.5/fire",))
e("go_main", {}, ("/eos/cues/fire",))
e("stop_back", {}, ("/eos/cues/stop",))
e("go_cue_list", {"cue_list": 2}, ("/eos/cues/2/fire",))
e("stop_back_cue_list", {"cue_list": 2}, ("/eos/cues/2/stop",))
e("select_cue_list", {"cue_list": 2}, ("/eos/cues/2",))
e("configure_cue_list_bank", {"bank": 1, "cue_list": 2, "previous": 3, "pending": 6}, ("/eos/cuelist/1/config/2/3/6",))
e("configure_cue_list_bank_offset", {"bank": 1, "cue_list": 2, "previous": 3, "pending": 6, "offset": 10},
  ("/eos/cuelist/1/config/2/3/6/10",))
e("page_cue_list_bank", {"bank": 1, "delta": -1}, ("/eos/cuelist/1/page/-1",))
e("select_cue_list_bank_cue", {"bank": 1, "cue": "100.4"}, ("/eos/cuelist/1/select/100.4",))
e("reset_cue_list_bank", {"bank": 1}, ("/eos/cuelist/1/reset",))

# Submasters and macros.
e("set_submaster", {"submaster": 3, "level": 0.75}, ("/eos/sub/3", ("f", 0.75)))
e("set_submaster_to", {"submaster": 1, "action": "full"}, ("/eos/sub/1/full",))
e("bump_submaster", {"submaster": 1}, ("/eos/sub/1/fire", DOWN), ("/eos/sub/1/fire", UP))
e("bump_submaster_down", {"submaster": 1}, ("/eos/sub/1/fire", DOWN))
e("bump_submaster_up", {"submaster": 1}, ("/eos/sub/1/fire", UP))
e("select_submaster", {"submaster": 1}, ("/eos/sub", ("i", 1)))
e("fire_macro", {"macro": 101}, ("/eos/macro/fire", ("i", 101)))
e("select_macro", {"macro": 1}, ("/eos/macro", ("i", 1)))

# Channels.
e("select_channel", {"channel": 1}, ("/eos/chan", ("i", 1)))
e("set_channel_intensity", {"channel": 1, "level": 50.0}, ("/eos/chan/1", ("f", 50.0)))
e("set_channel_dmx", {"channel": 1, "dmx": 255}, ("/eos/chan/1/dmx", ("i", 255)))
e("set_channel_to", {"channel": 1, "action": "+%"}, ("/eos/chan/1/+%",))
e("channel_remainder_dim", {"channel": 1}, ("/eos/chan/1/remdim",))
e("set_channel_param", {"channel": 1, "param": "pan", "level": 90.0}, ("/eos/chan/1/param/pan", ("f", 90.0)))
e("set_channel_param_dmx", {"channel": 1, "param": "tilt", "dmx": 128}, ("/eos/chan/1/param/tilt/dmx", ("i", 128)))
e("set_channel_param_to", {"channel": 1, "param": "pan", "action": "home"}, ("/eos/chan/1/param/pan/home",))
e("set_channel_color_hs", {"channel": 1, "hue": 330.0, "saturation": 75.0}, ("/eos/chan/1/color/hs", ("f", 330.0), ("f", 75.0)))
e("set_color_hs", {"hue": 330.0, "saturation": 75.0}, ("/eos/color/hs", ("f", 330.0), ("f", 75.0)))
e("set_channel_color_hsxy", {"channel": 1, "x": 0.82, "y": 0.31}, ("/eos/chan/1/color/hsxy", ("f", 0.82), ("f", 0.31)))
e("set_color_hsxy", {"x": 0.82, "y": 0.31}, ("/eos/color/hsxy", ("f", 0.82), ("f", 0.31)))
e("set_channel_color_rgb", {"channel": 1, "red": 1.0, "green": 0.25, "blue": 0.63},
  ("/eos/chan/1/color/rgb", ("f", 1.0), ("f", 0.25), ("f", 0.63)))
e("set_color_rgb", {"red": 1.0, "green": 0.25, "blue": 0.63}, ("/eos/color/rgb", ("f", 1.0), ("f", 0.25), ("f", 0.63)))
e("set_channel_color_xy", {"channel": 1, "x": 0.464, "y": 0.254}, ("/eos/chan/1/color/xy", ("f", 0.464), ("f", 0.254)))
e("set_color_xy", {"x": 0.464, "y": 0.254}, ("/eos/color/xy", ("f", 0.464), ("f", 0.254)))
e("set_channel_color_xyz", {"channel": 1, "x": 0.851, "y": 0.466, "z": 0.516},
  ("/eos/chan/1/color/xyz", ("f", 0.851), ("f", 0.466), ("f", 0.516)))
e("set_color_xyz", {"x": 0.851, "y": 0.466, "z": 0.516}, ("/eos/color/xyz", ("f", 0.851), ("f", 0.466), ("f", 0.516)))
e("set_channel_pan_tilt_xy", {"channel": 1, "x": 0.5, "y": 0.5}, ("/eos/chan/1/pantilt/xy", ("f", 0.5), ("f", 0.5)))
e("set_channel_xyz", {"channel": 1, "x": 1.0, "y": 1.0, "z": 1.0}, ("/eos/chan/1/xyz", ("f", 1.0), ("f", 1.0), ("f", 1.0)))

# Groups.
e("select_group", {"group": 1}, ("/eos/group", ("i", 1)))
e("set_group_intensity", {"group": 1, "level": 50.0}, ("/eos/group/1", ("f", 50.0)))
e("set_group_dmx", {"group": 1, "dmx": 127}, ("/eos/group/1/dmx", ("i", 127)))
e("set_group_to", {"group": 1, "action": "out"}, ("/eos/group/1/out",))
e("group_remainder_dim", {"group": 1}, ("/eos/group/1/remdim",))
e("set_group_param", {"group": 1, "param": "pan", "level": 50.0}, ("/eos/group/1/param/pan", ("f", 50.0)))
e("set_group_param_dmx", {"group": 1, "param": "tilt", "dmx": 128}, ("/eos/group/1/param/tilt/dmx", ("i", 128)))
e("set_group_param_to", {"group": 1, "param": "pan", "action": "-%"}, ("/eos/group/1/param/pan/-%",))

# The current selection.
e("set_selection_intensity", {"level": 75.0}, ("/eos/at", ("f", 75.0)))
e("set_selection_dmx", {"dmx": 128}, ("/eos/at/dmx", ("i", 128)))
e("set_selection_to", {"action": "remdim"}, ("/eos/at/remdim",))
e("set_param", {"param": "pan/tilt", "level": 50.0}, ("/eos/param/pan/tilt", ("f", 50.0)))
e("set_param_dmx", {"param": "pan", "dmx": 128}, ("/eos/param/pan/dmx", ("i", 128)))
e("set_param_to", {"param": "tilt", "action": "min"}, ("/eos/param/tilt/min",))
e("set_pan_tilt_xy", {"x": 0.5, "y": 0.5}, ("/eos/pantilt/xy", ("f", 0.5), ("f", 0.5)))
e("set_xyz", {"x": 1.0, "y": -2.5, "z": 3.0}, ("/eos/xyz", ("f", 1.0), ("f", -2.5), ("f", 3.0)))

# Addresses (OSC Address, OSC DMX).
e("select_address", {"address": 513}, ("/eos/addr", ("i", 513)))
e("set_address_level", {"address": 513, "level": 100.0}, ("/eos/addr/513", ("f", 100.0)))
e("set_address_dmx", {"address": 513, "dmx": 255}, ("/eos/addr/513/dmx", ("i", 255)))

# Overrides (OSC Override).
e("override_selection", {"level": 50}, ("/eos/at/override", ("i", 50)))
e("override_channel", {"channel": 1, "level": 50}, ("/eos/chan/1/override", ("i", 50)))
e("override_channel_param", {"channel": 1, "param": "intens", "level": 50},
  ("/eos/chan/1/param/intens/override", ("i", 50)))
e("override_channel_param_dmx", {"channel": 1, "param": "intens", "dmx": 252},
  ("/eos/chan/1/param/intens/dmx/override", ("i", 252)))
e("override_group", {"group": 1, "level": 50}, ("/eos/group/1/override", ("i", 50)))
e("override_group_param", {"group": 1, "param": "intens", "level": 50}, ("/eos/group/1/param/intens/override", ("i", 50)))
e("release_overrides", {}, ("/eos/release",))
e("release_selection_overrides", {}, ("/eos/at/release",))
e("release_param_overrides", {"param": "pan"}, ("/eos/param/pan/release",))
e("release_channel_overrides", {"channel": 4}, ("/eos/chan/4/release",))
e("release_channel_param_overrides", {"channel": 4, "param": "intens"}, ("/eos/chan/4/param/intens/release",))
e("release_group_overrides", {"group": 2}, ("/eos/group/2/release",))
e("release_group_param_overrides", {"group": 2, "param": "intens"}, ("/eos/group/2/param/intens/release",))

# Wheels and switches (OSC Wheel, OSC Switch, OSC Active Data).
e("set_wheel_mode", {"fine": True}, ("/eos/wheel", ("i", 1)))
e("wheel_level", {"ticks": -1.0}, ("/eos/wheel/level", ("f", -1.0)))
e("wheel_param", {"param": "pan/tilt", "ticks": 1.0}, ("/eos/wheel/pan/tilt", ("f", 1.0)))
e("wheel_param_fine", {"param": "pan", "ticks": 1.0}, ("/eos/wheel/fine/pan", ("f", 1.0)))
e("wheel_param_coarse", {"param": "pan", "ticks": 1.0}, ("/eos/wheel/coarse/pan", ("f", 1.0)))
e("set_switch_mode", {"fine": True}, ("/eos/switch", ("i", 1)))
e("switch_level", {"ticks": 4.0}, ("/eos/switch/level", ("f", 4.0)))
e("switch_param", {"param": "pan/tilt", "ticks": 0.0}, ("/eos/switch/pan/tilt", ("f", 0.0)))
e("switch_param_fine", {"param": "pan", "ticks": 1.0}, ("/eos/switch/fine/pan", ("f", 1.0)))
e("switch_param_coarse", {"param": "pan", "ticks": 1.0}, ("/eos/switch/coarse/pan", ("f", 1.0)))
e("active_wheel", {"wheel": 1, "ticks": 2.0}, ("/eos/active/wheel/1", ("f", 2.0)))
e("active_wheel_fine", {"wheel": 1, "ticks": 2.0}, ("/eos/active/wheel/fine/1", ("f", 2.0)))
e("active_wheel_coarse", {"wheel": 1, "ticks": 2.0}, ("/eos/active/wheel/coarse/1", ("f", 2.0)))
e("active_switch", {"wheel": 1, "ticks": 0.25}, ("/eos/active/switch/1", ("f", 0.25)))
e("active_switch_fine", {"wheel": 1, "ticks": 2.0}, ("/eos/active/switch/fine/1", ("f", 2.0)))
e("active_switch_coarse", {"wheel": 1, "ticks": -2.0}, ("/eos/active/switch/coarse/1", ("f", -2.0)))

# Palettes, presets and other targets (OSC Palettes, OSC Preset, OSC Snapshot, OSC Magic Sheet...).
e("fire_palette", {"palette_type": "bp", "palette": 12}, ("/eos/bp/12/fire",))
e("select_palette", {"palette_type": "ip", "palette": 1}, ("/eos/ip", ("i", 1)))
e("fire_preset", {"preset": 1}, ("/eos/preset/1/fire",))
e("select_preset", {"preset": 1}, ("/eos/preset", ("i", 1)))
e("fire_snapshot", {"snapshot": 1}, ("/eos/snap/1/fire",))
e("select_snapshot", {"snapshot": 1}, ("/eos/snap", ("i", 1)))
e("fire_magic_sheet", {"magic_sheet": 1}, ("/eos/ms/1/fire",))
e("open_magic_sheet", {"magic_sheet": 1}, ("/eos/ms", ("i", 1)))
e("open_magic_sheet_view", {"magic_sheet": 1, "view": 2}, ("/eos/ms/1", ("i", 2)))
e("select_effect", {"effect": 901}, ("/eos/fx", ("i", 901)))
e("select_curve", {"curve": 901}, ("/eos/curve", ("i", 901)))
e("select_pixel_map", {"pixel_map": 1}, ("/eos/pixmap", ("i", 1)))

# OSC fader banks.
e("configure_fader_bank", {"bank": 1, "faders": 10}, ("/eos/fader/1/config/10",))
e("configure_fader_bank_page", {"bank": 1, "page": 2, "faders": 10}, ("/eos/fader/1/config/2/10",))
e("page_fader_bank", {"bank": 1, "delta": -1}, ("/eos/fader/1/page/-1",))
e("reset_fader_bank", {"bank": 1}, ("/eos/fader/1/reset",))
e("set_fader", {"bank": 1, "fader": 2, "level": 0.75}, ("/eos/fader/1/2", ("f", 0.75)))
e("set_fader_to", {"bank": 0, "fader": 1, "action": "unload"}, ("/eos/fader/0/1/unload",))

# OSC direct select banks.
e("configure_direct_select_bank", {"bank": 1, "target_type": "bp", "buttons": 10}, ("/eos/ds/1/bp/10",))
e("configure_direct_select_bank_flexi", {"bank": 1, "target_type": "macro", "buttons": 10}, ("/eos/ds/1/macro/flexi/10",))
e("configure_direct_select_bank_page", {"bank": 1, "target_type": "chan", "page": 3, "buttons": 10}, ("/eos/ds/1/chan/3/10",))
e("configure_direct_select_bank_flexi_page", {"bank": 1, "target_type": "chan", "page": 3, "buttons": 10},
  ("/eos/ds/1/chan/flexi/3/10",))
e("page_direct_select_bank", {"bank": 1, "delta": 10}, ("/eos/ds/1/page/10",))
e("press_direct_select", {"bank": 1, "button": 1}, ("/eos/ds/1/1", DOWN), ("/eos/ds/1/1", UP))

# OSC Set.
e("set_label", {"target_type": "curve", "number": 901, "label": "New Label"}, ("/eos/set/curve/901/label", ("s", "New Label")))
e("set_cue_label", {"cue_list": 1, "cue": "1", "text": "New Label"}, ("/eos/set/cue/1/1/label", ("s", "New Label")))
e("set_cue_part_label", {"cue_list": 1, "cue": "1", "part": 2, "text": "New Label"}, ("/eos/set/cue/1/1/2/label", ("s", "New Label")))
e("set_cue_notes", {"cue_list": 1, "cue": "1", "text": "New Note"}, ("/eos/set/cue/1/1/notes", ("s", "New Note")))
e("set_cue_part_notes", {"cue_list": 1, "cue": "1", "part": 2, "text": "New Note"}, ("/eos/set/cue/1/1/2/notes", ("s", "New Note")))
e("set_cue_scene", {"cue_list": 1, "cue": "1", "text": "New Scene"}, ("/eos/set/cue/1/1/scene", ("s", "New Scene")))
e("set_cue_part_scene", {"cue_list": 1, "cue": "1", "part": 2, "text": "New Scene"}, ("/eos/set/cue/1/1/2/scene", ("s", "New Scene")))
e("set_event_label", {"event_list": 1, "event": 2, "label": "New Label"}, ("/eos/set/event/1/2/label", ("s", "New Label")))
e("set_group_channels", {"group": 1, "channels": "1 > 9 21 31"}, ("/eos/set/group/1/chans", ("s", "1 > 9 21 31")))
e("set_patch_field", {"channel": 1, "field": "text10", "text": "Text 10 Entry"}, ("/eos/set/patch/1/text10", ("s", "Text 10 Entry")))
e("set_patch_part_field", {"channel": 1, "part": 2, "field": "label", "text": "Part"}, ("/eos/set/patch/1/2/label", ("s", "Part")))

# OSC Get: queries answered on /eos/out/get/...
e("get_version", {}, ("/eos/get/version",),
  device_reply_hex=hexs(osc("/eos/out/get/version", ("s", "3.3.0.273"), ("s", "3.3.0.102"), ("i", 0))),
  expect_result={"ok": {"kind": "value", "value": "3.3.0.273"}})
e("get_fixture_library_version", {}, ("/eos/get/version",),
  device_reply_hex=hexs(osc("/eos/out/get/version", ("s", "3.3.0.273"), ("s", "3.3.0.102"), ("i", 0))),
  expect_result={"ok": {"kind": "value", "value": "3.3.0.102"}})
e("get_show_path", {}, ("/eos/get/show/path",))
e("get_selected_channels", {}, ("/eos/get/chans",),
  device_reply_hex=hexs(osc("/eos/out/get/chans", ("s", "2, 6-7"))),
  expect_result={"ok": {"kind": "value", "value": "2, 6-7"}})
e("get_count", {"target_type": "macro"}, ("/eos/get/macro/count",),
  device_reply_hex=hexs(osc("/eos/out/get/macro/count", ("i", 10))),
  expect_result={"ok": {"kind": "value", "value": 10}})
e("get_cue_count", {"cue_list": 1}, ("/eos/get/cue/1/count",))
e("get_cue_count_no_parts", {"cue_list": 1}, ("/eos/get/cue/1/noparts/count",))
e("get_cue_part_count", {"cue_list": 1, "cue": "1"}, ("/eos/get/cue/1/1/count",))
e("get_event_count", {"event_list": 1}, ("/eos/get/event/1/count",))
e("get_fpe_point_count", {"fpe_set": 1}, ("/eos/get/fpe/1/count",),
  device_reply_hex=hexs(osc("/eos/out/get/fpe/1/count", ("i", 0), ("i", 4))),
  expect_result={"ok": {"kind": "value", "value": 4}})
e("get_target", {"target_type": "bp", "number": 2}, ("/eos/get/bp/2",))
e("get_target_by_index", {"target_type": "macro", "index": 0}, ("/eos/get/macro/index/0",))
e("get_target_by_uid", {"target_type": "group", "uid": UID}, (f"/eos/get/group/uid/{UID}",))
e("get_group_displayed", {"group": 2}, ("/eos/get/group/displayed/2",))
e("get_cue", {"cue_list": 1, "cue": "1"}, ("/eos/get/cue/1/1",))
e("get_cue_part", {"cue_list": 1, "cue": "1", "part": 1}, ("/eos/get/cue/1/1/1",))
e("get_cue_by_index", {"cue_list": 1, "index": 0}, ("/eos/get/cue/1/index/0",))
e("get_cue_by_uid", {"uid": UID}, (f"/eos/get/cue/uid/{UID}",))
e("get_event", {"event_list": 1, "event": 1}, ("/eos/get/event/1/1",))
e("get_event_by_index", {"event_list": 1, "index": 0}, ("/eos/get/event/1/index/0",))
e("get_patch", {"channel": 1}, ("/eos/get/patch/1",))
e("get_patch_part", {"channel": 1, "part": 1}, ("/eos/get/patch/1/1",))
e("get_params", {"channel": 1}, ("/eos/get/params/1",))
e("get_emitters", {"channel": 1}, ("/eos/get/emitters/1",))
e("get_fpe_set", {"index": 0}, ("/eos/get/fpe/index/0",))
e("get_fpe_point", {"fpe_set": 1, "point": 1}, ("/eos/get/fpe/1/1",))
e("get_setup", {}, ("/eos/get/setup",))
e("get_processors", {}, ("/eos/get/processors",))
e("get_session", {}, ("/eos/get/session",))
e("get_user_list", {}, ("/eos/get/userlist",))
e("get_augment3d_server", {}, ("/eos/get/3dserver",))
e("get_csv", {}, ("/eos/get/csv",))

# Telemetry: the implicit outputs and get replies (OSC Dictionary, OSC Outputs).
T, F_ = ("T", None), ("F", None)   # OSC True/False tags carry no data
# On connecting: the subscription, then the first OSC Get query of the poll
# (the rest follow one at a time, each as the previous one is answered).
telemetry(E, "subscribe-on-connect", expect_connect_wire_hex=[hexs(length_prefixed(osc("/eos/subscribe", ("i", 1)))),
                                                               hexs(length_prefixed(osc("/eos/get/version")))],
          inbound_hex=hexs(osc("/eos/out/show/name", ("s", "New Show Name"))),
          expect_state={"show": {"name": "New Show Name"}})
telemetry(E, "version", inbound_hex=hexs(osc("/eos/out/get/version", ("s", "3.3.0.273"), ("s", "3.3.0.102"), ("i", 0))),
          expect_state={"version": {"eos": "3.3.0.273", "fixture_library": "3.3.0.102", "gel_swatch_mode": 0}})
telemetry(E, "show-path", inbound_hex=hexs(osc("/eos/out/get/show/path", ("s", "C:\\Users\\user\\Documents\\ETC\\Eos\\ShowArchive\\showfile.esf3d"))),
          expect_state={"show": {"path": "C:\\Users\\user\\Documents\\ETC\\Eos\\ShowArchive\\showfile.esf3d"}})
telemetry(E, "show-saved", inbound_hex=hexs(osc("/eos/out/event/show/saved", ("s", "C:\\Shows\\filename.esf3d"))),
          expect_state={"show": {"last_file_event": "saved", "last_file": "C:\\Shows\\filename.esf3d"}})
telemetry(E, "osc-user", inbound_hex=hexs(osc("/eos/out/user", ("i", 2))), expect_state={"osc_user": 2})
telemetry(E, "command-line", inbound_hex=hexs(osc("/eos/out/cmd", ("s", "LIVE: Cue 1 : Chan 1 #"), ("i", 0))),
          expect_state={"command_line": {"text": "LIVE: Cue 1 : Chan 1 #", "error": False}})
telemetry(E, "user-command-line", inbound_hex=hexs(osc("/eos/out/user/1/cmd", ("s", "LIVE: Chan 1 Error"), ("i", 1))),
          expect_state={"users": {"1": {"command_line": "LIVE: Chan 1 Error", "command_line_error": True}}})
telemetry(E, "softkey", inbound_hex=hexs(osc("/eos/out/softkey/1", ("s", "Address"))),
          expect_state={"softkeys": {"1": {"label": "Address"}}})
telemetry(E, "blind", inbound_hex=hexs(osc("/eos/out/event/state", ("i", 0))), expect_state={"console": {"live": False}})
telemetry(E, "locked", inbound_hex=hexs(osc("/eos/out/event/locked", ("i", 1))), expect_state={"console": {"locked": True}})
telemetry(E, "wheel", inbound_hex=hexs(osc("/eos/out/wheel", ("f", 1.0))), expect_state={"osc_wheel": {"value": 1.0}})
telemetry(E, "switch", inbound_hex=hexs(osc("/eos/out/switch", ("f", 0.0))), expect_state={"osc_switch": {"value": 0.0}})
telemetry(E, "active-cue-percent", inbound_hex=hexs(osc("/eos/out/active/cue", ("f", 0.5))),
          expect_state={"active_cue": {"percent": 0.5}})
telemetry(E, "active-cue", inbound_hex=hexs(osc("/eos/out/active/cue/5/1.5", ("f", 0.75))),
          expect_state={"active_cue": {"list": 5, "number": "1.5", "percent": 0.75}})
telemetry(E, "active-cue-text", inbound_hex=hexs(osc("/eos/out/active/cue/text", ("s", "1/1 Label 5.0"))),
          expect_state={"active_cue": {"text": "1/1 Label 5.0"}})
telemetry(E, "pending-cue", inbound_hex=hexs(osc("/eos/out/pending/cue/1/2")),
          expect_state={"pending_cue": {"list": 1, "number": "2", "part": 0}})
telemetry(E, "pending-cue-part", inbound_hex=hexs(osc("/eos/out/pending/cue/1/2.5/3")),
          expect_state={"pending_cue": {"list": 1, "number": "2.5", "part": 3}})
telemetry(E, "pending-cue-text", inbound_hex=hexs(osc("/eos/out/pending/cue/text", ("s", "1/2 Label 5.0"))),
          expect_state={"pending_cue": {"text": "1/2 Label 5.0"}})
telemetry(E, "previous-cue", inbound_hex=hexs(osc("/eos/out/previous/cue/1/1")),
          expect_state={"previous_cue": {"list": 1, "number": "1", "part": 0}})
telemetry(E, "previous-cue-text", inbound_hex=hexs(osc("/eos/out/previous/cue/text", ("s", "1/1 Label 5.0"))),
          expect_state={"previous_cue": {"text": "1/1 Label 5.0"}})
telemetry(E, "cue-fired", inbound_hex=hexs(osc("/eos/out/event/cue/1/7/fire", ("s", "Cue 7 Label"))),
          expect_state={"last_cue_event": {"list": 1, "number": "7", "action": "fire", "label": "Cue 7 Label"}})
telemetry(E, "macro-fired", inbound_hex=hexs(osc("/eos/out/event/macro/1")), expect_state={"last_macro_event": {"macro": 1}})
telemetry(E, "sub-bump", inbound_hex=hexs(osc("/eos/out/event/sub/1", ("i", 1))),
          expect_state={"submasters": {"1": {"bump": True}}})
telemetry(E, "relay", inbound_hex=hexs(osc("/eos/out/event/relay/1/1", ("i", 1))),
          expect_state={"relays": {"1": {"1": {"state": 1}}}})
telemetry(E, "active-chan", inbound_hex=hexs(osc("/eos/out/active/chan", ("s", "1-2 [100]"))),
          expect_state={"selection": {"active_channels": "1-2 [100]"}})
telemetry(E, "active-chan-two-args", inbound_hex=hexs(osc("/eos/out/active/chan", ("i", 2), ("s", "[100] ETC_Revolution_Original FixtureNote"))),
          expect_state={"selection": {"active_channels": "2", "active_channel_info": "[100] ETC_Revolution_Original FixtureNote"}})
telemetry(E, "osc-user-chans", inbound_hex=hexs(osc("/eos/out/get/chans", ("s", "2, 6-7"))),
          expect_state={"selection": {"osc_user_channels": "2, 6-7"}})
telemetry(E, "color-hs", inbound_hex=hexs(osc("/eos/out/color/hs", ("f", 296.5), ("f", 38.25))),
          expect_state={"selection": {"hue": 296.5, "saturation": 38.25}})
telemetry(E, "pantilt", inbound_hex=hexs(osc("/eos/out/pantilt", ("f", -270.0), ("f", 270.0), ("f", -135.0), ("f", 135.0),
                                             ("f", 0.0), ("f", 0.0))),
          expect_state={"selection": {"pan_min": -270.0, "pan_max": 270.0, "tilt_min": -135.0, "tilt_max": 135.0,
                                      "pan": 0.0, "tilt": 0.0}})
telemetry(E, "xyz", inbound_hex=hexs(osc("/eos/out/xyz", ("f", 1.0), ("f", 2.0), ("f", 0.5))),
          expect_state={"selection": {"x": 1.0, "y": 2.0, "z": 0.5}})
telemetry(E, "active-wheel", inbound_hex=hexs(osc("/eos/out/active/wheel/1", ("s", "Intens [0]"), ("i", 1), ("f", 0.0))),
          expect_state={"wheels": {"1": {"label": "Intens [0]", "category": 1, "value": 0.0}}})
telemetry(E, "param", inbound_hex=hexs(osc("/eos/out/param/red", ("f", 20.0), ("f", 0.0), ("f", 100.0))),
          expect_state={"params": {"red": {"value": 20.0, "min": 0.0, "max": 100.0}}})
telemetry(E, "fader-page", inbound_hex=hexs(osc("/eos/out/fader/1", ("s", "2"))),
          expect_state={"fader_banks": {"1": {"page": "2"}}})
telemetry(E, "fader-level", inbound_hex=hexs(osc("/eos/out/fader/1/1", ("f", 0.75))),
          expect_state={"fader_banks": {"1": {"faders": {"1": {"level": 0.75}}}}})
telemetry(E, "fader-level-dictionary-form", inbound_hex=hexs(osc("/eos/fader/1/1", ("f", 0.5))),
          expect_state={"fader_banks": {"1": {"faders": {"1": {"level": 0.5}}}}})
telemetry(E, "fader-label-on-level-address", inbound_hex=hexs(osc("/eos/out/fader/1/1", ("s", "S 1 Label"))),
          expect_state={})
telemetry(E, "fader-name", inbound_hex=hexs(osc("/eos/out/fader/1/1/name", ("s", "S 1 Hold P3"))),
          expect_state={"fader_banks": {"1": {"faders": {"1": {"name": "S 1 Hold P3"}}}}})
telemetry(E, "fader-range", inbound_hex=hexs(osc("/eos/out/fader/range/1/1", ("f", 0.0), ("f", 100.0))),
          expect_state={"fader_banks": {"1": {"faders": {"1": {"min": 0.0, "max": 100.0}}}}})
telemetry(E, "cue-list-bank", inbound_hex=hexs(osc("/eos/out/cuelist/1", ("s", "Cue List 1"), ("i", 10), ("i", 0))),
          expect_state={"cue_list_banks": {"1": {"label": "Cue List 1", "total_cues": 10, "follow_time": 0}}})
telemetry(E, "cue-list-bank-row", inbound_hex=hexs(osc("/eos/out/cuelist/1/3", ("s", "2 P2 Label"), ("s", "2 P2"), ("s", "Label"),
                                                       ("s", "note"), ("s", "scene 2"), F_, ("i", 5000), ("i", -1))),
          expect_state={"cue_list_banks": {"1": {"rows": {"3": {
              "osc_label": "2 P2 Label", "cue_number": "2 P2", "label": "Label", "notes": "note", "scene": "scene 2",
              "scene_end": False, "duration": 5000, "remaining": -1}}}}})
telemetry(E, "direct-select-bank", inbound_hex=hexs(osc("/eos/out/ds/1", ("s", "Beam Palettes [1]"))),
          expect_state={"direct_select_banks": {"1": {"title": "Beam Palettes [1]"}}})
telemetry(E, "direct-select-button", inbound_hex=hexs(osc("/eos/out/ds/1/1", ("s", "Beam Palette Label [1]"), ("s", "1"))),
          expect_state={"direct_select_banks": {"1": {"buttons": {"1": {"target": "Beam Palette Label [1]", "name": "1"}}}}})
telemetry(E, "notify", inbound_hex=hexs(osc("/eos/out/notify/macro/list/0/3", ("i", 7), ("i", 5), ("s", "8-9"))),
          expect_state={"show_data": {"macro": {"notify_sequence": 7}}})
telemetry(E, "notify-cue", inbound_hex=hexs(osc("/eos/out/notify/cue/1/list/0/2", ("i", 12), ("i", 4))),
          expect_state={"cue_lists": {"1": {"notify_sequence": 12}}})
telemetry(E, "count", inbound_hex=hexs(osc("/eos/out/get/group/count", ("i", 10))),
          expect_state={"show_data": {"group": {"count": 10}}})
telemetry(E, "cue-count", inbound_hex=hexs(osc("/eos/out/get/cue/1/count", ("i", 10))),
          expect_state={"cue_lists": {"1": {"cue_count": 10}}})
telemetry(E, "cue-count-no-parts", inbound_hex=hexs(osc("/eos/out/get/cue/1/noparts/count", ("i", 8))),
          expect_state={"cue_lists": {"1": {"cue_count_no_parts": 8}}})
telemetry(E, "cue-list", inbound_hex=hexs(osc("/eos/out/get/cuelist/1/list/0/13", ("i", 0), ("s", UID), ("s", "Cue List Label"),
                                              ("s", "Master (U2)"), ("s", "Proportional"), F_, F_, F_, F_, T, F_, ("i", -1), F_)),
          expect_state={"cue_lists": {"1": {
              "uid": UID, "label": "Cue List Label", "playback_mode": "Master (U2)", "fader_mode": "Proportional",
              "independent": False, "htp": False, "assert": False, "block": False, "background": True,
              "solo_mode": False, "timecode_list": -1, "oos_sync": False}}})
# The OSC Dictionary's own example reply for Cue 1 Part 1 in Cue List 1.
CUE_ARGS = [("i", 1), ("s", UID), ("s", "Cue 1 Label"), ("i", 5000), ("i", 0), ("i", -1), ("i", -1), ("i", -1), ("i", -1),
            ("i", -1), ("i", -1), ("i", -1), ("i", -1), F_, ("s", "0"), ("i", 100), ("s", ""), ("s", ""), ("s", ""),
            ("s", "2/1"), ("i", -1), ("i", -1), F_, ("i", -1), F_, ("s", ""), ("i", 1), ("s", "Note"), ("s", "Scene"), T,
            ("i", 0)]
CUE_STATE = {"label": "Cue 1 Label", "up_time": 5000, "up_delay": 0, "down_time": -1, "down_delay": -1,
             "focus_time": -1, "focus_delay": -1, "color_time": -1, "color_delay": -1, "beam_time": -1,
             "beam_delay": -1, "preheat": False, "curve": "0", "rate": 100, "mark": "", "block": "", "assert": "",
             "link": "2/1", "follow_time": -1, "hang_time": -1, "all_fade": False, "loop": -1, "solo": False,
             "timecode": "", "parts": 1, "notes": "Note", "scene": "Scene", "scene_end": True, "part_index": 0}
telemetry(E, "cue", inbound_hex=hexs(osc("/eos/out/get/cue/1/1/1/list/0/31", *CUE_ARGS)),
          expect_state={"cues": {UID: {"list": 1, "number": "1", "part": 1, **CUE_STATE}}})
telemetry(E, "cue-base", inbound_hex=hexs(osc("/eos/out/get/cue/1/2.5/list/0/31", *CUE_ARGS)),
          expect_state={"cues": {UID: {"list": 1, "number": "2.5", "part": 0, **CUE_STATE}}})
telemetry(E, "cue-continuation-ignored", inbound_hex=hexs(osc("/eos/out/get/cue/1/1/1/list/14/31", ("s", "x"))),
          expect_state={})
telemetry(E, "sub", inbound_hex=hexs(osc("/eos/out/get/sub/2/list/0/13", ("i", 1), ("s", UID), ("s", "Sub 2 Label"),
                                         ("s", "Additive"), ("s", "Proportional"), T, F_, T, F_, ("s", "P3"), ("s", "0"),
                                         ("s", "Man"), ("s", "0"))),
          expect_state={"submasters": {"2": {
              "uid": UID, "label": "Sub 2 Label", "mode": "Additive", "fader_mode": "Proportional", "htp": True,
              "exclusive": False, "background": True, "restore": False, "priority": "P3", "up_time": "0",
              "dwell_time": "Man", "down_time": "0"}}})
telemetry(E, "macro", inbound_hex=hexs(osc("/eos/out/get/macro/2/list/0/4", ("i", 1), ("s", UID), ("s", "Macro Label"),
                                           ("s", "Foreground"))),
          expect_state={"macros": {"2": {"uid": UID, "label": "Macro Label", "mode": "Foreground"}}})
telemetry(E, "macro-text", inbound_hex=hexs(osc("/eos/out/get/macro/2/text/list/0/3", ("i", 1), ("s", UID),
                                                ("s", "Go_To_Cue Out Time 0"))),
          expect_state={"macros": {"2": {"text": "Go_To_Cue Out Time 0"}}})
telemetry(E, "group", inbound_hex=hexs(osc("/eos/out/get/group/2/list/0/3", ("i", 1), ("s", UID), ("s", "Group 2 Label"))),
          expect_state={"groups": {"2": {"uid": UID, "label": "Group 2 Label"}}})
telemetry(E, "group-channels-ignored", inbound_hex=hexs(osc("/eos/out/get/group/2/channels/list/0/4", ("i", 1), ("s", UID),
                                                            ("i", 1), ("s", "11-12"))),
          expect_state={})
telemetry(E, "palette", inbound_hex=hexs(osc("/eos/out/get/ip/2/list/0/6", ("i", 1), ("s", UID),
                                             ("s", "Intensity Palette 2 Label"), F_, F_, ("i", 0))),
          expect_state={"palettes": {"ip": {"2": {"uid": UID, "label": "Intensity Palette 2 Label", "absolute": False,
                                                  "locked": False, "tracking_channel": 0}}}})
telemetry(E, "preset", inbound_hex=hexs(osc("/eos/out/get/preset/2/list/0/6", ("i", 1), ("s", UID), ("s", "Preset 2 Label"),
                                            T, F_, ("i", 0))),
          expect_state={"presets": {"2": {"uid": UID, "label": "Preset 2 Label", "absolute": True, "locked": False,
                                          "tracking_channel": 0}}})
telemetry(E, "effect", inbound_hex=hexs(osc("/eos/out/get/fx/901/list/0/8", ("i", 0), ("s", UID), ("s", "Effect 901 Label"),
                                            ("s", "Focus"), ("s", "Immediate"), ("s", "Immediate"), ("s", "Infinite"), ("i", 25))),
          expect_state={"effects": {"901": {"uid": UID, "label": "Effect 901 Label", "type": "Focus", "entry": "Immediate",
                                            "exit": "Immediate", "duration": "Infinite", "scale": 25}}})
telemetry(E, "pixel-map", inbound_hex=hexs(osc("/eos/out/get/pixmap/2/list/0/9", ("i", 1), ("s", UID), ("s", "Pixel Map Label"),
                                               ("i", 100), ("s", "sACN"), ("i", 32), ("i", 32), ("i", 1024), ("i", 1024))),
          expect_state={"pixel_maps": {"2": {"uid": UID, "label": "Pixel Map Label", "server_channel": 100,
                                             "interface": "sACN", "width": 32, "height": 32, "pixel_count": 1024,
                                             "fixture_count": 1024}}})
telemetry(E, "curve", inbound_hex=hexs(osc("/eos/out/get/curve/2/list/0/3", ("i", 1), ("s", UID), ("s", "Curve Label"))),
          expect_state={"curves": {"2": {"uid": UID, "label": "Curve Label"}}})
telemetry(E, "snapshot", inbound_hex=hexs(osc("/eos/out/get/snap/2/list/0/3", ("i", 1), ("s", UID), ("s", "Snapshot Label"))),
          expect_state={"snapshots": {"2": {"uid": UID, "label": "Snapshot Label"}}})
telemetry(E, "magic-sheet", inbound_hex=hexs(osc("/eos/out/get/ms/2/list/0/3", ("i", 1), ("s", UID), ("s", "Magic Sheet Label"))),
          expect_state={"magic_sheets": {"2": {"uid": UID, "label": "Magic Sheet Label"}}})
# The OSC Dictionary's example reply for Chan 1 Part 1.
telemetry(E, "patch", inbound_hex=hexs(osc("/eos/out/get/patch/1/1/list/0/21", ("i", 0), ("s", UID), ("s", "Channel Label"),
                                           ("s", "ETC_Fixtures"), ("s", "Releve_Spot_Direct"), ("i", 513), ("i", 513),
                                           ("i", 100), ("s", ""), *[("s", f"Text{i}") for i in range(1, 11)],
                                           ("i", 1), ("i", 537))),
          expect_state={"patch": {"1": {"parts": {"1": {
              "uid": UID, "label": "Channel Label", "manufacturer": "ETC_Fixtures", "fixture_type": "Releve_Spot_Direct",
              "address": 513, "intensity_address": 513, "level": 100, "gel": "",
              **{f"text{i}": f"Text{i}" for i in range(1, 11)}, "part_count": 1, "end_address": 537}}}}})
telemetry(E, "patch-notes", inbound_hex=hexs(osc("/eos/out/get/patch/1/1/notes", ("i", 0), ("s", UID), ("s", "Note"))),
          expect_state={"patch": {"1": {"parts": {"1": {"notes": "Note"}}}}})
telemetry(E, "params", inbound_hex=hexs(osc("/eos/out/get/params/1", ("s", "ETC Fixtures"), ("s", "Releve Spot Direct"),
                                            ("s", "Intens"), ("f", 0.0), ("f", 0.0), ("f", 100.0))),
          expect_state={"channels": {"1": {"manufacturer": "ETC Fixtures", "fixture_type": "Releve Spot Direct"}}})
telemetry(E, "setup", inbound_hex=hexs(osc("/eos/out/get/setup/list/0/5", ("i", 5), ("i", 5), ("i", 5), ("i", 5), ("i", 5))),
          expect_state={"setup": {"up_time": 5, "down_time": 5, "focus_time": 5, "color_time": 5, "beam_time": 5}})
telemetry(E, "fpe-set", inbound_hex=hexs(osc("/eos/out/get/fpe/1", ("i", 0), ("s", "FPE Label"), ("i", 4))),
          expect_state={"fpe_sets": {"1": {"label": "FPE Label", "point_count": 4}}})
telemetry(E, "fpe-count", inbound_hex=hexs(osc("/eos/out/get/fpe/1/count", ("i", 0), ("i", 4))),
          expect_state={"fpe_sets": {"1": {"point_count": 4}}})
telemetry(E, "fpe-point", inbound_hex=hexs(osc("/eos/out/get/fpe/1/0", ("i", 0), ("i", 0), ("s", "FPE Point 1/1"), ("i", 901),
                                               ("f", -3.0), ("f", 3.0), ("f", 0.0))),
          expect_state={"fpe_sets": {"1": {"points": {"0": {"name": "FPE Point 1/1", "number": 901, "x": -3.0, "y": 3.0,
                                                            "z": 0.0}}}}})
