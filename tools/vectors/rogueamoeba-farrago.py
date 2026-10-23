# Farrago OSC (rogueamoeba-farrago): one vector per command. Addresses are
# those of Farrago's "OSC Support" manual page (tile by UUID, set and grid
# cell, or name; set endpoints; /transport, /master, /inspector, /list and
# /global). Argument types follow the MIT-licensed Bitfocus Companion module:
# an int 1 for actions, an int 1 or 0 for switches, 0 or 1 for A/B, layout and
# playback mode; volumes, colors, positions and durations are floats. Farrago
# never answers, so every command is unverified.
FG = "rogueamoeba-farrago"
_FU = {"expect_result": {"ok": {"kind": "unverified"}}}
_UUID = "DEF9215F-BFB3-40A2-B383-21ABD276FAC6"


def fg(command, input, address, *args, **extra):
    binary(FG, command, input, osc(address, *args), **extra)


fg("play_tile", {"tile": f"tile/{_UUID}"}, f"/tile/{_UUID}/play", ("i", 1), **_FU)
fg("play_tile", {"tile": "set/Sample_Sound_Effects/tile/Dog_Bark"}, "/set/Sample_Sound_Effects/tile/Dog_Bark/play",
   ("i", 1), file="play_tile-by-name")
fg("back_tile", {"tile": "set/0/tile/0/0"}, "/set/0/tile/0/0/back", ("i", 1))
fg("select_tile", {"tile": "set/selected/tile/0/0"}, "/set/selected/tile/0/0/select", ("i", 1))
fg("fade_out_tile", {"tile": "set/1/tile/2/3"}, "/set/1/tile/2/3/fadeOut", ("i", 1))
fg("toggle_tile_ab", {"tile": "set/0/tile/0/0", "side": "b"}, "/set/0/tile/0/0/toggleAB", ("i", 1))
fg("set_tile_allow_pausing", {"tile": "set/0/tile/0/0", "enabled": True}, "/set/0/tile/0/0/allowPausing", ("i", 1))
fg("set_tile_hold_to_play", {"tile": "set/0/tile/0/0", "enabled": False}, "/set/0/tile/0/0/holdToPlay", ("i", 0))
fg("set_tile_loop", {"tile": "set/0/tile/1/0", "enabled": True}, "/set/0/tile/1/0/loop", ("i", 1))
fg("set_tile_mute", {"tile": "set/0/tile/1/0", "muted": True}, "/set/0/tile/1/0/mute", ("i", 1))
fg("set_tile_solo", {"tile": "set/0/tile/1/0", "enabled": False}, "/set/0/tile/1/0/solo", ("i", 0))
fg("set_tile_color", {"tile": "set/0/tile/0/0", "color": 0.625}, "/set/0/tile/0/0/color", ("f", 0.625))
fg("set_tile_position", {"tile": "set/0/tile/0/0", "position": 0.5}, "/set/0/tile/0/0/currentPosition", ("f", 0.5))
fg("set_tile_volume", {"tile": "set/0/tile/0/0", "volume": 0.75}, "/set/0/tile/0/0/volume", ("f", 0.75))
fg("select_set", {"set": "Sample_Sound_Effects"}, "/set/Sample_Sound_Effects/select", ("i", 1))
fg("select_set", {"set": "0"}, "/set/0/select", ("i", 1), file="select_set-position")
fg("set_set_layout", {"set": "selected", "layout": "list"}, "/set/selected/layout", ("i", 1))
fg("set_set_playback_mode", {"set": "selected", "mode": "queued"}, "/set/selected/playbackMode", ("i", 0))
fg("set_set_position", {"set": "2", "position": 0}, "/set/2/position", ("i", 0))
fg("reset_set", {"set": "selected"}, "/set/selected/reset", ("i", 1))
fg("set_set_crossfade_duration", {"set": "selected", "seconds": 2.5}, "/set/selected/crossfadeDuration", ("f", 2.5))
fg("play_pause_all", {}, "/transport/playPauseAll", ("i", 1))
fg("stop_all", {}, "/transport/stopAll", ("i", 1))
fg("previous", {}, "/transport/previous", ("i", 1))
fg("next", {}, "/transport/next", ("i", 1))
fg("master_volume_up", {}, "/master/volumeUp", ("i", 1))
fg("master_volume_down", {}, "/master/volumeDown", ("i", 1))
fg("set_master_mute", {"muted": True}, "/master/mute", ("i", 1))
fg("set_master_mute", {"muted": False}, "/master/mute", ("i", 0), file="set_master_mute-unmute")
fg("set_master_volume", {"volume": 0.5}, "/master/volume", ("f", 0.5))
fg("fade_all", {}, "/master/fadeAll", ("i", 1))
fg("set_master_ab", {"side": "a"}, "/master/toggleAB", ("i", 0))
fg("set_inspector_volume", {"volume": 0.25}, "/inspector/volume", ("f", 0.25))
fg("inspector_volume_up", {}, "/inspector/volumeUp", ("i", 1))
fg("inspector_volume_down", {}, "/inspector/volumeDown", ("i", 1))
fg("set_inspector_ab", {"side": "b"}, "/inspector/toggleAB", ("i", 1))
fg("set_inspector_mute", {"muted": False}, "/inspector/mute", ("i", 0))
fg("set_inspector_loop", {"enabled": True}, "/inspector/loop", ("i", 1))
fg("set_inspector_solo", {"enabled": True}, "/inspector/solo", ("i", 1))
fg("set_inspector_hold_to_play", {"enabled": True}, "/inspector/holdToPlay", ("i", 1))
fg("set_inspector_pausable", {"enabled": False}, "/inspector/pausable", ("i", 0))
fg("set_inspector_color", {"color": 0.25}, "/inspector/color", ("f", 0.25))
fg("reset_list", {}, "/list/reset", ("i", 1))
fg("set_list_crossfade_duration", {"seconds": 1.0}, "/list/crossfadeDuration", ("f", 1.0))
fg("bring_forward", {}, "/global/bringForward", ("i", 1))
fg("ping", {}, "/ping")

# ── Telemetry: grid-form feedback, as the Companion module reads it ──
T0 = "/set/0/tile/1/2"
telemetry(FG, "tile-title", expect_connect_wire_hex=[hexs(osc("/ping"))],
          inbound_hex=hexs(osc(f"{T0}/title", ("s", "Dog Bark"))),
          expect_state={"sets": {"0": {"tiles": {"1": {"2": {"title": "Dog Bark"}}}}}})
for endpoint, key, arg, value in [
        ("notes", "notes", ("s", "Cue on the knock"), "Cue on the knock"),
        ("icon", "icon", ("s", "dog"), "dog"),
        ("play", "playing", ("T", None), True),
        ("loop", "loop", ("i", 1), True),
        ("mute", "muted", ("i", 0), False),
        ("solo", "solo", ("F", None), False),
        ("holdToPlay", "hold_to_play", ("i", 1), True),
        ("allowPausing", "allow_pausing", ("f", 0.0), False),
        ("toggleAB", "ab", ("i", 1), "b"),
        ("volume", "volume", ("f", 0.75), 0.75),
        ("color", "color", ("f", 0.625), 0.625),
        ("currentPosition", "position", ("f", 0.5), 0.5),
        ("currentTime", "current_time", ("f", 12.5), 12.5),
        ("duration", "duration", ("f", 31.25), 31.25),
        ("remainingTime", "remaining_time", ("f", 18.75), 18.75),
        ("peakMeterL", "peak_left", ("f", 0.5), 0.5),
        ("peakMeterR", "peak_right", ("f", 0.25), 0.25),
        ("rmsMeterL", "rms_left", ("f", 0.125), 0.125),
        ("rmsMeterR", "rms_right", ("f", 0.0625), 0.0625)]:
    telemetry(FG, f"tile-{key.replace('_', '-')}", inbound_hex=hexs(osc(f"{T0}/{endpoint}", arg)),
              expect_state={"sets": {"0": {"tiles": {"1": {"2": {key: value}}}}}})
telemetry(FG, "selected-tile", inbound_hex=hexs(osc("/set/selected/tile/3/1/select", ("i", 1))),
          expect_state={"selected_tile": {"x": 3, "y": 1}})
telemetry(FG, "deselected-tile", inbound_hex=hexs(osc("/set/selected/tile/3/1/select", ("i", 0))), expect_state={})
telemetry(FG, "selected-set", inbound_hex=hexs(osc("/set/2/select", ("i", 1))), expect_state={"selected_set": 2})
telemetry(FG, "set-title", inbound_hex=hexs(osc("/set/2/title", ("s", "Sample Sound Effects"))),
          expect_state={"sets": {"2": {"title": "Sample Sound Effects"}}})
telemetry(FG, "set-notes", inbound_hex=hexs(osc("/set/2/notes", ("s", "Act 1"))),
          expect_state={"sets": {"2": {"notes": "Act 1"}}})
telemetry(FG, "set-layout", inbound_hex=hexs(osc("/set/2/layout", ("i", 1))), expect_state={"sets": {"2": {"layout": "list"}}})
telemetry(FG, "set-playback-mode", inbound_hex=hexs(osc("/set/2/playbackMode", ("i", 1))),
          expect_state={"sets": {"2": {"playback_mode": "continuous"}}})
telemetry(FG, "set-crossfade", inbound_hex=hexs(osc("/set/2/crossfadeDuration", ("f", 2.5))),
          expect_state={"sets": {"2": {"crossfade_duration": 2.5}}})
telemetry(FG, "master-mute", inbound_hex=hexs(osc("/master/mute", ("i", 1))), expect_state={"master": {"muted": True}})
telemetry(FG, "master-volume", inbound_hex=hexs(osc("/master/volume", ("f", 0.5))), expect_state={"master": {"volume": 0.5}})
telemetry(FG, "master-ab", inbound_hex=hexs(osc("/master/toggleAB", ("i", 0))), expect_state={"master": {"ab": "a"}})
