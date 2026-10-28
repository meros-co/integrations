MX = "madrix"
# MADRIX 5: OSC to UDP 9001 (MADRIX 5 Help, Remote Control > OSC, the address
# space table) and the HTTP Remote on port 80 (MADRIX 5.8a HTTP Remote
# Control documentation: /RemoteCommands/?<Function>[=<value>]). Addresses,
# argument types and request targets are typed here from those tables.


def mx(command, input, address, *args, **extra):
    binary(MX, command, input, osc(address, *args), **extra)


def mxh(command, input, query, **extra):
    V.append({"spec": MX, "command": command, "input": input,
              "expect_request": {"method": "GET", "target": f"/RemoteCommands/?{query}", "port": 80}, **extra})


MXI = lambda v: ("i", v)  # noqa: E731
MXF = lambda v: ("f", v)  # noqa: E731

# Audio
mx("set_audio_input_level", {"level": 80}, "/Audio/Input/Level", MXI(80), expect_result={"ok": {"kind": "unverified"}})
mx("set_audio_input_mute", {"muted": True}, "/Audio/Input/Mute", MXI(1))
mx("set_audio_output_level", {"level": 100}, "/Audio/Output/Level", MXI(100))
mx("set_audio_output_mute", {"muted": False}, "/Audio/Output/Mute", MXI(0))
mx("set_audio_input_agc", {"enabled": True}, "/Audio/Input/Agc", MXI(1))
# Decks
mx("set_deck_pause", {"deck": "a", "paused": True}, "/DeckA/Pause", MXI(1))
mx("set_deck_speed_master", {"deck": "b", "speed": -2.5}, "/DeckB/SpeedMaster", MXF(-2.5))
mx("set_deck_submaster", {"deck": "a", "level": 255}, "/DeckA/Submaster", MXI(255))
mx("set_deck_filter", {"deck": "b", "filter": 102}, "/DeckB/Filter", MXI(102))
mx("set_deck_filter_color_mode", {"deck": "a", "mode": 3}, "/DeckA/FilterColor/Mode", MXI(3))
mx("set_deck_filter_color_global", {"deck": "a", "id": 4}, "/DeckA/FilterColor/GlobalId", MXI(4))
mx("set_deck_filter_color_channel", {"deck": "b", "channel": "white", "value": 10}, "/DeckB/FilterColor/White", MXI(10))
mx("set_deck_filter_color_rgb", {"deck": "a", "red": 255, "green": 128, "blue": 0}, "/DeckA/FilterColor/Rgb", MXI(255), MXI(128), MXI(0))
mx("select_deck_storage", {"deck": "a", "index": 0}, "/DeckA/Storages/Index", MXI(0))
mx("select_deck_place", {"deck": "b", "index": 11}, "/DeckB/Places/Index", MXI(11))
mx("set_place_speed_pitch", {"deck": "a", "pitch": 1.5}, "/DeckA/StoragePlace/SpeedPitch", MXF(1.5))
mx("set_place_submaster", {"deck": "a", "level": 200}, "/DeckA/StoragePlace/Submaster", MXI(200))
mx("set_place_filter", {"deck": "b", "filter": 0}, "/DeckB/StoragePlace/Filter", MXI(0))
mx("set_place_chaser_state", {"deck": "a", "state": "playing"}, "/DeckA/StoragePlace/Chaser/PlaybackState", MXI(2))
mx("set_place_chaser_speed_pitch", {"deck": "a", "pitch": -10.0}, "/DeckA/StoragePlace/Chaser/SpeedPitch", MXF(-10.0))
mx("select_place_chaser_step", {"deck": "b", "step": 3}, "/DeckB/StoragePlace/Chaser/Steps/Index", MXI(3))
mx("set_layer_submaster", {"deck": "a", "layer": 0, "level": 128}, "/DeckA/StoragePlace/Layers/0/Submaster", MXI(128))
mx("set_layer_opacity", {"deck": "b", "layer": 2, "level": 64}, "/DeckB/StoragePlace/Layers/2/Opacity", MXI(64))
mx("set_layer_blind", {"deck": "a", "layer": 1, "blind": True}, "/DeckA/StoragePlace/Layers/1/Blind", MXI(1))
mx("set_layer_solo", {"deck": "a", "layer": 1, "solo": False}, "/DeckA/StoragePlace/Layers/1/Solo", MXI(0))
mx("set_layer_filter", {"deck": "a", "layer": 0, "filter": 5}, "/DeckA/StoragePlace/Layers/0/Filter", MXI(5))
mx("set_layer_mix_mode", {"deck": "a", "layer": 0, "mode": 21}, "/DeckA/StoragePlace/Layers/0/MixMode", MXI(21))
mx("set_layer_chaser_state", {"deck": "b", "layer": 3, "state": "paused"}, "/DeckB/StoragePlace/Layers/3/Chaser/PlaybackState", MXI(1))
mx("set_layer_chaser_speed_pitch", {"deck": "b", "layer": 3, "pitch": 2.0}, "/DeckB/StoragePlace/Layers/3/Chaser/SpeedPitch", MXF(2.0))
mx("select_layer_chaser_step", {"deck": "b", "layer": 3, "step": 0}, "/DeckB/StoragePlace/Layers/3/Chaser/Steps/Index", MXI(0))
# Output; the help's example "/Output/Master", 0 to 255.
mx("set_freeze", {"frozen": True}, "/Output/Freeze", MXI(1))
mx("set_master", {"level": 160}, "/Output/Master", MXI(160))
mx("set_blackout", {"enabled": True}, "/Output/Blackout", MXI(1))
mx("set_output_filter", {"filter": 1}, "/Output/Filter", MXI(1))
mx("set_output_filter_color_mode", {"mode": 0}, "/Output/FilterColor/Mode", MXI(0))
mx("set_output_filter_color_global", {"id": 1}, "/Output/FilterColor/GlobalId", MXI(1))
mx("set_output_filter_color_channel", {"channel": "red", "value": 50}, "/Output/FilterColor/Red", MXI(50))
mx("set_output_filter_color_rgb", {"red": 1, "green": 2, "blue": 3}, "/Output/FilterColor/Rgb", MXI(1), MXI(2), MXI(3))
mx("set_strobe_color_mode", {"mode": "black_and_color"}, "/Output/StrobeColor/Mode", MXI(2))
mx("set_strobe_color_global", {"id": 7}, "/Output/StrobeColor/GlobalId", MXI(7))
mx("set_strobe_color_channel", {"channel": "blue", "value": 150}, "/Output/StrobeColor/Blue", MXI(150))
mx("set_strobe_color_rgb", {"red": 255, "green": 255, "blue": 255}, "/Output/StrobeColor/Rgb", MXI(255), MXI(255), MXI(255))
mx("set_strobe", {"strobe": 9}, "/Output/Strobe", MXI(9))
# Crossfader
mx("set_fade_time", {"seconds": 2.5}, "/Fade/Time", MXF(2.5))
mx("set_fade_type", {"type": "slide_z_cross"}, "/Fade/Type", MXI(14))
mx("set_crossfader", {"value": 255}, "/Fade/Value", MXI(255))
mx("crossfade", {"to": "deck_a"}, "/Fade/ToLeft")
mx("crossfade", {"to": "auto"}, "/Fade/Automatic", file="crossfade-auto")
# Color Kit
mx("set_global_color_channel", {"id": 1, "channel": "alpha", "value": 200}, "/ColorKit/GlobalColors/1/Alpha", MXI(200))
mx("set_global_color_rgb", {"id": 2, "red": 75, "green": 0, "blue": 130}, "/ColorKit/GlobalColors/2/Rgb", MXI(75), MXI(0), MXI(130))
# Group Control
mx("set_group_defaults", {}, "/GroupControl/Defaults")
mx("set_group_value", {"group": 0, "value": 150}, "/GroupControl/Groups/0/Value", MXI(150))
mx("set_group_flash", {"group": 4, "flash": True}, "/GroupControl/Groups/4/Flash", MXI(1))
mx("set_group_chaser_state", {"state": "stopped"}, "/GroupControl/Chaser/PlaybackState", MXI(0))
mx("set_group_chaser_speed_pitch", {"pitch": 0.5}, "/GroupControl/Chaser/SpeedPitch", MXF(0.5))
mx("select_group_chaser_step", {"step": 1}, "/GroupControl/Chaser/Steps/Index", MXI(1))
mx("set_group_fade_time", {"seconds": 12.0}, "/GroupControl/FadeTime", MXF(12.0))
mx("recall_group_preset", {"index": 1}, "/GroupControl/Presets/Index", MXI(1))
# DMX Fader Tool
mx("set_dmx_fader_state", {"fader": 23, "enabled": True}, "/DmxFaderTool/Faders/23/State", MXI(1))
mx("set_dmx_fader_value", {"fader": 0, "value": 150}, "/DmxFaderTool/Faders/0/Value", MXI(150))
# Cue lists, timelines, setups, triggers, view
mx("select_cue_list", {"index": 1}, "/CueLists/Index", MXI(1))
mx("set_cue_list_state", {"state": "playing"}, "/CueLists/PlaybackState", MXI(2))
mx("set_cue_list_mode", {"mode": "shuffle"}, "/CueLists/PlaybackMode", MXI(1))
mx("set_cue_list_timecode_source", {"source": "smpte"}, "/CueLists/TimeCodeSource", MXI(4))
mx("select_cue", {"index": 3}, "/CueLists/Cues/Index", MXI(3))
mx("select_timeline", {"index": 0}, "/Timelines/Index", MXI(0))
mx("set_timeline_state", {"state": "paused"}, "/Timelines/PlaybackState", MXI(1))
mx("select_audio_segment", {"index": 2}, "/Timelines/AudioSegments/Index", MXI(2))
mx("select_cue_segment", {"index": 5}, "/Timelines/CueSegments/Index", MXI(5))
mx("select_marker", {"id": 3}, "/Timelines/Markers/Id", MXI(3))
mx("load_setup_register", {"id": 1}, "/SetupRegisters/Id", MXI(1))
mx("set_trigger_port", {"port": 63, "position": 0.5, "value": 1.0, "segments": 0},
   "/Trigger/Ports/63/PositionAndValue", MXF(0.5), MXF(1.0), MXI(0))
mx("set_view", {"view": 5}, "/Main/View", MXI(5))
mx("feedback_all", {}, "/Main/FeedbackAll")
mx("set_artnet_timecode_stream", {"stream": 0}, "/ArtNet/Input/TimeCode/StreamId", MXI(0))
# Up, Down and Offset
mx("step", {"target": "cue", "direction": "up"}, "/CueLists/Cues/Index/Up")
mx("deck_step", {"deck": "b", "target": "place", "direction": "down"}, "/DeckB/Places/Index/Down")
mx("layer_step", {"deck": "a", "layer": 0, "target": "mix_mode", "direction": "up"}, "/DeckA/StoragePlace/Layers/0/MixMode/Up")
mx("offset", {"target": "master", "amount": -10}, "/Output/Master/Offset", MXI(-10))
mx("deck_offset", {"deck": "a", "target": "place_chaser_speed_pitch", "amount": 1}, "/DeckA/StoragePlace/Chaser/SpeedPitch/Offset", MXI(1))
mx("layer_offset", {"deck": "b", "layer": 1, "target": "opacity", "amount": 5}, "/DeckB/StoragePlace/Layers/1/Opacity/Offset", MXI(5))
mx("global_color_offset", {"id": 3, "channel": "green", "amount": -1}, "/ColorKit/GlobalColors/3/Green/Offset", MXI(-1))
mx("group_value_offset", {"group": 2, "amount": 20}, "/GroupControl/Groups/2/Value/Offset", MXI(20))
mx("dmx_fader_offset", {"fader": 1, "amount": -20}, "/DmxFaderTool/Faders/1/Value/Offset", MXI(-20))

# HTTP Remote, from the documentation's samples where it gives one.
MX_OK = {"status": 200, "body": ""}
mxh("set_storage_place", {"storage": 20, "place": 123}, "SetStorage=S20P123", http_reply=MX_OK,
    expect_result={"ok": {"kind": "ack"}})
mxh("set_storage_place_without_fade", {"storage": 2, "place": 29}, "SetStorageWithoutFade=S2P29")
mxh("set_deck_storage_place", {"deck": "a", "storage": 3, "place": 5}, "SetStorageDeckA=S3P5")
mxh("set_deck_storage_place_without_fade", {"deck": "b", "storage": 4, "place": 24}, "SetStorageWithoutFadeDeckB=S4P24")
mxh("set_current_layer", {"deck": "a", "layer": 2}, "SetLayerDeckA=2")
mxh("set_ticker_text", {"text": "Hello"}, "SetTextTicker=Hello")
mxh("set_deck_ticker_text", {"deck": "b", "layer": 2, "text": "Hi World"}, "SetTextTickerLayerDeckB=2_Hi%20World")
mxh("set_bpm", {"bpm": 900}, "SetBpm=900")
mxh("recall_group_preset_with_fade", {"preset": 3}, "SetGroupPresetWithFade=3")
mxh("cue_list_go", {}, "SetCuelistGo")
mxh("cue_list_back", {}, "SetCuelistBack")
mxh("cue_list_go_to", {"cue": 3}, "SetCuelistGoTo=3")
mxh("cue_list_add_cue", {}, "SetCuelistAddCue")
mxh("cue_list_add_occupied_places", {}, "SetCuelistAddOccupiedStoragePlaces")
mxh("cue_list_delete_cue", {"cue": 4}, "SetCuelistDeleteCue=4")
mxh("cue_list_delete_all", {}, "SetCuelistDeleteAll")
mxh("set_cue_follow", {"cue": 1, "follow": 4}, "SetCuelistFollowCue=1_4")
mxh("set_cue_storage", {"cue": 1, "storage": 12}, "SetCuelistStorage=1_12")
mxh("set_cue_place", {"cue": 1, "place": 2}, "SetCuelistStoragePlace=1_2")
mxh("set_cue_fade_type", {"cue": 1, "type": "white_fade"}, "SetCuelistFadeType=1_1")
mxh("set_cue_fade_time", {"cue": 1, "seconds": 2.34}, "SetCuelistFadeTime=1_2.340")
mxh("set_cue_duration", {"cue": 1, "duration": "2m1s"}, "SetCuelistDuration=1_2m1s")
mxh("set_cue_timecode", {"cue": 1, "timecode": "3h10m9s"}, "SetCuelistTimeCode=1_3h10m9s")
mxh("set_cue_date", {"cue": 1, "date": "Monday"}, "SetCuelistDate=1_Monday")
mxh("set_cue_group_preset", {"cue": 1, "preset": 2}, "SetCuelistGroupPreset=1_2")
mxh("set_global_color", {"id": 1, "channels": "R150G0A200W50"}, "SetGlobalColor=1_R150G0A200W50")
mxh("get_cue_list_name", {"index": 0}, "GetCuelistName=0", http_reply={"status": 200, "body": "Main Show"},
    expect_result={"ok": {"kind": "value", "value": "Main Show"}})
mxh("get_timeline_name", {"index": 0}, "GetTimelinesName=0")
mxh("get_cue_description", {"cue": 1}, "GetCuelistDescription=1")
mxh("get_storage_place_description", {"storage": 1, "place": 1}, "GetStoragePlaceDescription=S1P1")
mxh("get_storage_place_filled", {"storage": 2, "place": 5}, "GetStoragePlaceFullState=S2P5")
mxh("get_group_name", {"id": 1}, "GetGroupDisplayName=1")
mxh("get_group_preset_description", {"preset": 1}, "GetGroupPresetDescription=1")
mxh("get_global_color_description", {"id": 1}, "GetGlobalColorDescription=1")
mxh("get_setup_register_description", {"id": 1}, "GetSetupRegisterDescription=1")
mxh("get_value", {"function": "GetSoundLevel=0"}, "GetSoundLevel=0", http_reply={"status": 200, "body": "0.42"},
    expect_result={"ok": {"kind": "value", "value": "0.42"}})


# OSC feedback: each address with the argument it takes.
def mx_fb(name, address, args, state, **extra):
    telemetry(MX, name, inbound_hex=hexs(osc(address, *args)), expect_state=state, **extra)


mx_fb("audio-input-level", "/Audio/Input/Level", [MXI(55)], {"audio": {"input": {"level": 55}}})
mx_fb("audio-input-mute", "/Audio/Input/Mute", [MXI(1)], {"audio": {"input": {"muted": True}}})
mx_fb("audio-output-level", "/Audio/Output/Level", [MXI(100)], {"audio": {"output": {"level": 100}}})
mx_fb("audio-output-mute", "/Audio/Output/Mute", [MXI(0)], {"audio": {"output": {"muted": False}}})
mx_fb("audio-input-agc", "/Audio/Input/Agc", [MXI(1)], {"audio": {"input": {"agc": True}}})
for name, sub, args, path, value in [
    ("pause", "Pause", [MXI(1)], ["paused"], True),
    ("speed-master", "SpeedMaster", [MXF(2.0)], ["speed_master"], 2.0),
    ("submaster", "Submaster", [MXI(200)], ["submaster"], 200),
    ("filter", "Filter", [MXI(7)], ["filter"], 7),
    ("filter-color-mode", "FilterColor/Mode", [MXI(1)], ["filter_color", "mode"], 1),
    ("filter-color-global", "FilterColor/GlobalId", [MXI(0)], ["filter_color", "global_id"], 0),
    ("filter-color-red", "FilterColor/Red", [MXI(1)], ["filter_color", "red"], 1),
    ("filter-color-green", "FilterColor/Green", [MXI(2)], ["filter_color", "green"], 2),
    ("filter-color-blue", "FilterColor/Blue", [MXI(3)], ["filter_color", "blue"], 3),
    ("filter-color-white", "FilterColor/White", [MXI(4)], ["filter_color", "white"], 4),
    ("storage-index", "Storages/Index", [MXI(0)], ["storage_index"], 0),
    ("place-index", "Places/Index", [MXI(4)], ["place_index"], 4),
    ("place-speed-pitch", "StoragePlace/SpeedPitch", [MXF(-1.5)], ["place", "speed_pitch"], -1.5),
    ("place-submaster", "StoragePlace/Submaster", [MXI(255)], ["place", "submaster"], 255),
    ("place-filter", "StoragePlace/Filter", [MXI(0)], ["place", "filter"], 0),
    ("place-chaser-state", "StoragePlace/Chaser/PlaybackState", [MXI(2)], ["place", "chaser", "state"], "playing"),
    ("place-chaser-speed-pitch", "StoragePlace/Chaser/SpeedPitch", [MXF(1.0)], ["place", "chaser", "speed_pitch"], 1.0),
    ("place-chaser-step", "StoragePlace/Chaser/Steps/Index", [MXI(3)], ["place", "chaser", "step"], 3),
]:
    nested = value
    for key in reversed(path):
        nested = {key: nested}
    mx_fb(f"deck-{name}", f"/DeckB/{sub}", args, {"decks": {"DeckB": nested}})
for name, sub, args, path, value in [
    ("submaster", "Submaster", [MXI(10)], ["submaster"], 10),
    ("opacity", "Opacity", [MXI(128)], ["opacity"], 128),
    ("blind", "Blind", [MXI(1)], ["blind"], True),
    ("solo", "Solo", [MXI(0)], ["solo"], False),
    ("filter", "Filter", [MXI(9)], ["filter"], 9),
    ("mix-mode", "MixMode", [MXI(13)], ["mix_mode"], 13),
    ("chaser-state", "Chaser/PlaybackState", [MXI(0)], ["chaser", "state"], "stopped"),
    ("chaser-speed-pitch", "Chaser/SpeedPitch", [MXF(-2.0)], ["chaser", "speed_pitch"], -2.0),
    ("chaser-step", "Chaser/Steps/Index", [MXI(1)], ["chaser", "step"], 1),
]:
    nested = value
    for key in reversed(path):
        nested = {key: nested}
    mx_fb(f"layer-{name}", f"/DeckA/StoragePlace/Layers/2/{sub}", args, {"decks": {"DeckA": {"layers": {"2": nested}}}})
for name, address, args, path, value in [
    ("freeze", "/Output/Freeze", [MXI(1)], ["frozen"], True),
    ("master", "/Output/Master", [MXI(160)], ["master"], 160),
    ("blackout", "/Output/Blackout", [MXI(0)], ["blackout"], False),
    ("filter", "/Output/Filter", [MXI(2)], ["filter"], 2),
    ("filter-color-mode", "/Output/FilterColor/Mode", [MXI(3)], ["filter_color", "mode"], 3),
    ("filter-color-global", "/Output/FilterColor/GlobalId", [MXI(5)], ["filter_color", "global_id"], 5),
    ("filter-color-red", "/Output/FilterColor/Red", [MXI(50)], ["filter_color", "red"], 50),
    ("filter-color-green", "/Output/FilterColor/Green", [MXI(100)], ["filter_color", "green"], 100),
    ("filter-color-blue", "/Output/FilterColor/Blue", [MXI(150)], ["filter_color", "blue"], 150),
    ("filter-color-white", "/Output/FilterColor/White", [MXI(200)], ["filter_color", "white"], 200),
    ("strobe-mode", "/Output/StrobeColor/Mode", [MXI(1)], ["strobe_color", "mode"], "output_and_black"),
    ("strobe-global", "/Output/StrobeColor/GlobalId", [MXI(0)], ["strobe_color", "global_id"], 0),
    ("strobe-red", "/Output/StrobeColor/Red", [MXI(1)], ["strobe_color", "red"], 1),
    ("strobe-green", "/Output/StrobeColor/Green", [MXI(2)], ["strobe_color", "green"], 2),
    ("strobe-blue", "/Output/StrobeColor/Blue", [MXI(3)], ["strobe_color", "blue"], 3),
    ("strobe-white", "/Output/StrobeColor/White", [MXI(4)], ["strobe_color", "white"], 4),
    ("strobe", "/Output/Strobe", [MXI(5)], ["strobe"], 5),
]:
    nested = value
    for key in reversed(path):
        nested = {key: nested}
    mx_fb(f"output-{name}", address, args, {"output": nested})
mx_fb("fade-time", "/Fade/Time", [MXF(3.0)], {"fade": {"time": 3.0}})
mx_fb("fade-type", "/Fade/Type", [MXI(2)], {"fade": {"type": "black_fade"}})
mx_fb("fade-value", "/Fade/Value", [MXI(128)], {"fade": {"value": 128}})
for ch in ("Red", "Green", "Blue", "White", "Alpha"):
    mx_fb(f"global-color-{ch.lower()}", f"/ColorKit/GlobalColors/3/{ch}", [MXI(9)], {"global_colors": {"3": {ch.lower(): 9}}})
mx_fb("group-value", "/GroupControl/Groups/1/Value", [MXI(77)], {"groups": {"1": {"value": 77}}})
mx_fb("group-flash", "/GroupControl/Groups/1/Flash", [MXI(1)], {"groups": {"1": {"flash": True}}})
mx_fb("group-chaser-state", "/GroupControl/Chaser/PlaybackState", [MXI(1)], {"group_control": {"chaser": {"state": "paused"}}})
mx_fb("group-chaser-speed-pitch", "/GroupControl/Chaser/SpeedPitch", [MXF(0.5)], {"group_control": {"chaser": {"speed_pitch": 0.5}}})
mx_fb("group-chaser-step", "/GroupControl/Chaser/Steps/Index", [MXI(2)], {"group_control": {"chaser": {"step": 2}}})
mx_fb("group-fade-time", "/GroupControl/FadeTime", [MXF(12.0)], {"group_control": {"fade_time": 12.0}})
mx_fb("dmx-fader-state", "/DmxFaderTool/Faders/5/State", [MXI(1)], {"dmx_faders": {"5": {"enabled": True}}})
mx_fb("dmx-fader-value", "/DmxFaderTool/Faders/5/Value", [MXI(99)], {"dmx_faders": {"5": {"value": 99}}})
mx_fb("cue-list-index", "/CueLists/Index", [MXI(1)], {"cue_list": {"index": 1}})
mx_fb("cue-list-state", "/CueLists/PlaybackState", [MXI(2)], {"cue_list": {"state": "playing"}})
mx_fb("cue-list-mode", "/CueLists/PlaybackMode", [MXI(0)], {"cue_list": {"mode": "loop"}})
mx_fb("cue-list-timecode", "/CueLists/TimeCodeSource", [MXI(3)], {"cue_list": {"timecode_source": "artnet"}})
mx_fb("cue-list-cue", "/CueLists/Cues/Index", [MXI(6)], {"cue_list": {"cue_index": 6}})
mx_fb("timeline-index", "/Timelines/Index", [MXI(0)], {"timeline": {"index": 0}})
mx_fb("timeline-state", "/Timelines/PlaybackState", [MXI(0)], {"timeline": {"state": "stopped"}})
mx_fb("timeline-audio-segment", "/Timelines/AudioSegments/Index", [MXI(1)], {"timeline": {"audio_segment": 1}})
mx_fb("timeline-cue-segment", "/Timelines/CueSegments/Index", [MXI(2)], {"timeline": {"cue_segment": 2}})
mx_fb("timeline-marker", "/Timelines/Markers/Id", [MXI(3)], {"timeline": {"marker": 3}})
mx_fb("setup-register", "/SetupRegisters/Id", [MXI(1)], {"setup_register": {"id": 1}})
mx_fb("view", "/Main/View", [MXI(5)], {"main": {"view": 5}})
mx_fb("artnet-stream", "/ArtNet/Input/TimeCode/StreamId", [MXI(4)], {"artnet": {"timecode_stream": 4}})


# HTTP Remote polls: the Get function's answer as the body.
def mx_poll(name, function, body, state):
    telemetry(MX, f"http-{name}", inbound_http={"path": f"/RemoteCommands/?{function}", "body": body},
              expect_state=state)


mx_poll("product", "GetProduct", "MADRIX 5", {"info": {"product": "MADRIX 5"}})
mx_poll("version", "GetVersion", "5.8a", {"info": {"version": "5.8a"}})
mx_poll("version-number", "GetVersionNumber", "5.8.1.0", {"info": {"version_number": "5.8.1.0"}})
mx_poll("host-name", "GetHostName", "LIGHT-PC", {"info": {"host_name": "LIGHT-PC"}})
mx_poll("host-label", "GetHostLabel", "Main", {"info": {"host_label": "Main"}})
mx_poll("matrix-x", "GetMatrixSizeX", "320", {"matrix": {"size_x": 320}})
mx_poll("matrix-y", "GetMatrixSizeY", "180", {"matrix": {"size_y": 180}})
mx_poll("matrix-z", "GetMatrixSizeZ", "1", {"matrix": {"size_z": 1}})
mx_poll("cue-list-count", "GetCuelistCount", "12", {"cue_list": {"count": 12}})
mx_poll("timeline-count", "GetTimelinesCount", "2", {"timeline": {"count": 2}})
mx_poll("group-count", "GetGroupCount", "8", {"group_control": {"count": 8}})
mx_poll("setup-current", "GetSetupRegisterCurrentId", "0", {"setup_register": {"current": 0}})
for deck in ("A", "B"):
    d = f"Deck{deck}"
    mx_poll(f"storage-{deck.lower()}", f"GetStorage{d}", "3", {"decks": {d: {"storage": 3}}})
    mx_poll(f"place-{deck.lower()}", f"GetStoragePlace{d}", "17", {"decks": {d: {"place": 17}}})
    mx_poll(f"layer-{deck.lower()}", f"GetLayer{d}", "1", {"decks": {d: {"current_layer": 1}}})
    mx_poll(f"layer-count-{deck.lower()}", f"GetLayerCount{d}", "4", {"decks": {d: {"layer_count": 4}}})
    mx_poll(f"layer-name-{deck.lower()}", f"GetLayerName{d}", "Wash", {"decks": {d: {"layer_name": "Wash"}}})
    mx_poll(f"effect-name-{deck.lower()}", f"GetLayerEffectName{d}", "SCE Color", {"decks": {d: {"effect_name": "SCE Color"}}})
    mx_poll(f"ticker-{deck.lower()}", f"GetLayerTickerText{d}", "Welcome", {"decks": {d: {"ticker_text": "Welcome"}}})
mx_poll("cue-current", "GetCuelistCueCurrent", "5", {"cue_list": {"current_cue": 5}})
mx_poll("timeline-duration", "GetTimelinesDuration", "00:03:20", {"timeline": {"duration": "00:03:20"}})
