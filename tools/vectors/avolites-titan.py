AV = "avolites-titan"
# Avolites Titan WebAPI (API documentation V19.2): HTTP GET on port 4430.
# Targets are typed from each member page's HTTP form and examples, such as
# "/titan/script/2/Playbacks/FirePlaybackAtLevel?handle_userNumber=6&level_level=1&alwaysRefire=true",
# with numbers written as the spec renders them (levels with three decimals,
# times with two). Not taken from the spec.

S = "/titan/script/2/"


def av(command, input, target, **extra):
    http(AV, command, input, "GET", target, **extra)


UN = {"handle": "6"}

# ── Playbacks ────────────────────────────────────────────────────────────
av("fire_playback", {**UN, "level": 1.0, "always_refire": True},
   S + "Playbacks/FirePlaybackAtLevel?handle_userNumber=6&level_level=1.000&alwaysRefire=true",
   http_reply={"status": 200, "body": ""}, expect_result={"ok": {"kind": "ack"}})
av("fire_playback_delta", {"by": "titanId", "handle": "1895", "delta": 0.5},
   S + "Playbacks/FirePlaybackAtLevel?handle_titanId=1895&level_leveldelta=0.500&alwaysRefire=false")
av("set_playback_level", {"by": "location", "handle": "playback_2_1", "level": 1.0},
   S + "Playbacks/SetPlaybackLevel?srcHandle_location=playback_2_1&level_level=1.000")
av("adjust_playback_level", {**UN, "delta": -0.25},
   S + "Playbacks/SetPlaybackLevel?srcHandle_userNumber=6&level_leveldelta=-0.250")
av("kill_playback", UN, S + "Playbacks/KillPlayback?handle_userNumber=6")
av("kill_all_playbacks", {}, S + "Playbacks/KillAllPlaybacks")
av("release_playback", {**UN, "fade_time": 5.0, "use_master_release_time": True},
   S + "Playbacks/ReleasePlayback?handle_userNumber=6&fadeTime=5.00&useMasterReleaseTime=true")
av("release_all_playbacks", {"fade_time": 5.0, "use_master_release_time": True},
   S + "Playbacks/ReleaseAllPlaybacks?fadeTime=5.00&useMasterReleaseTime=true")
av("release_all_playbacks_by_priority", {"fade_time": 5.0, "use_master_release_time": True, "all_playbacks": True},
   S + "Playbacks/ReleaseAllPlaybacksByPriority?fadeTime=5.00&useMasterReleaseTime=true&allPlaybacks=true")
for name, member in [("flash_playback", "FlashPlayback"), ("clear_flash_playback", "ClearFlashPlayback"),
                     ("flash_timed_playback", "FlashTimedPlayback"),
                     ("clear_flash_timed_playback", "ClearFlashTimedPlayback"),
                     ("swop_playback", "SwopPlayback"), ("clear_swop_playback", "ClearSwopPlayback"),
                     ("toggle_latch_playback", "ToggleLatchPlayback"),
                     ("toggle_blind_playback", "ToggleBlindPlayback")]:
    av(name, UN, S + f"Playbacks/{member}?handle_userNumber=6")
av("clear_all_blind_playbacks", {}, S + "Playbacks/ClearAllBlindPlaybacks")
av("preload_playback", {**UN, "time": 5.0}, S + "Playbacks/Preload?handle_userNumber=6&time=5.00")
av("set_manual_crossfade_level", {**UN, "level": 1.0},
   S + "Playbacks/SetManualCrossfadeLevel?handle_userNumber=6&level_level=1.000")
av("tap_tempo", UN, S + "Playbacks/TapTempo?handle_userNumber=6&panelTimeStamp=")
av("set_effect_speed_multiplier", {**UN, "multiplier": 2.0},
   S + "Playbacks/SetEffectSpeedMultiplier?handle_userNumber=6&level=2.000")
av("set_cue_legend", {**UN, "cue": 1.5, "legend": "Act 1 open"},
   S + "Playbacks/SetCueLegend?handle_userNumber=6&cueNumber=1.500&newLegend=Act%201%20open")

# ── Cue lists and chases ─────────────────────────────────────────────────
av("cuelist_go", UN, S + "CueLists/Play?handle_userNumber=6")
av("cuelist_go_with_time", {**UN, "time": 2.5}, S + "CueLists/PlayWithTime?handle_userNumber=6&time=2.50")
av("cuelist_go_back", UN, S + "CueLists/GoBack?handle_userNumber=6")
av("cuelist_resume", {**UN, "time": 0.0}, S + "CueLists/Resume?handle_userNumber=6&time=0.00")
av("cuelist_pause", {**UN, "go_back_if_paused": True}, S + "CueLists/Pause?handle_userNumber=6&goBackIfPaused=true")
av("cuelist_flash_and_go", {**UN, "flash": True, "with_times": True},
   S + "CueLists/FlashAndGo?handle_userNumber=6&flash=true&withTimes=true")
av("cuelist_set_next_cue", {**UN, "cue": 4.0}, S + "CueLists/SetNextCue?handle_userNumber=6&stepNumber=4.000")
for name, member in [("cuelist_advance_next_step", "AdvanceNextStep"), ("cuelist_decrement_next_step", "DecrementNextStep"),
                     ("cuelist_reset_next_step", "ResetNextStep"), ("cuelist_next_step", "NextStep"),
                     ("cuelist_review", "Review"), ("cuelist_snap_back", "SnapBack"),
                     ("cuelist_cut_next_cue_to_live", "CutNextCueToLive")]:
    av(name, UN, S + f"CueLists/{member}?handle_userNumber=6")
av("pause_connected_chase", {}, S + "CueLists/PauseChase")
av("play_connected_chase", {}, S + "CueLists/PlayChase")
av("chase_go", UN, S + "Chases/Play?handle_userNumber=6")
av("chase_pause", {**UN, "go_back_if_paused": True}, S + "Chases/Pause?handle_userNumber=6&goBackIfPaused=true")
av("chase_go_back", UN, S + "Chases/GoBack?handle_userNumber=6")
av("chase_set_next_cue", {**UN, "step": 3.0}, S + "Chases/SetNextCue?handle_userNumber=6&stepNumber=3.000")
av("chase_next_step", UN, S + "Chases/NextStep?handle_userNumber=6")
av("chase_set_crossfade", {**UN, "crossfade": 0.5}, S + "Chases/SetXFade?handle_userNumber=6&crossfade=0.500")

# ── Masters ──────────────────────────────────────────────────────────────
av("set_desk_blackout", {"blackout": True}, S + "Masters/BlackOutDesk?deskBlackOutState=true")
av("set_grand_master", {"level": 75.0}, S + "Masters/SetGrandMasterFaderLevel?oldValue=&value=75.0")
for name, member in [("dead_blackout", "DeadBlackOut"), ("master_flash", "Flash"), ("master_clear_flash", "ClearFlash"),
                     ("master_latch", "Latch"), ("master_take", "Take"), ("reset_master", "ResetMaster")]:
    av(name, UN, S + f"Masters/{member}?handle_userNumber=6")
av("reset_all_masters", {}, S + "Masters/ResetAllMasters")
av("master_tap_tempo", UN, S + "Masters/TapTempo?handle_userNumber=6&panelTimeStamp=")

# ── Macros, palettes, groups ─────────────────────────────────────────────
av("recall_macro", UN, S + "UserMacros/RecallMacro?handle_userNumber=6")
av("recall_macro_by_id", {"macro_id": "Avolites.Macros.ClearAll"},
   S + "UserMacros/RecallMacroById?macroId=Avolites.Macros.ClearAll")
av("apply_palette", {**UN, "use_palette_times": True}, S + "Palette/ApplyPalette?handle_userNumber=6&usePaletteTimes=true")
av("apply_timed_palette", UN, S + "Palette/ApplyTimedPalette?handle_userNumber=6")
av("apply_quick_palette", {**UN, "use_palette_times": False},
   S + "Palette/ApplyQuickPalette?handle_userNumber=6&usePaletteTimes=false")
av("apply_palette_number", {"user_number": 12.0}, S + "Palette/ApplyTimedPaletteNumeric?userNumber=12.000")
av("apply_palette_to_playback", {**UN, "playback": "6", "use_palette_times": True},
   S + "Palette/ApplyPaletteToPlayback?handle_userNumber=6&playbackHandle_userNumber=6&usePaletteTimes=true")
av("set_palette_fade_time", {"time": 5.0}, S + "Palette/SetFadeTime?value=5.00")
av("recall_group", UN, S + "Group/RecallGroup?handle_userNumber=6")
av("recall_group_number", {"user_number": 3.0}, S + "Group/RecallGroupNumeric?userId=3.000")
av("group_flash", {**UN, "enabled": True}, S + "Group/Flash?handle_userNumber=6&enable=true")
av("group_swop", {**UN, "enabled": True}, S + "Group/Swop?handle_userNumber=6&enable=true")

# ── Programmer and fixtures ──────────────────────────────────────────────
P = S + "Programmer/Editor/"
av("clear_programmer", {"presets": True, "all_programmers": False}, P + "ClearAll?presets=true&allProgrammers=false")
av("release_programmer", {"fade_time": 5.0}, P + "Fixtures/Release?fadeTime=5.00")
av("locate_selected_fixtures", {"all_attributes": True}, P + "Fixtures/LocateSelectedFixtures?allAttributes=true")
av("set_selected_dimmer", {"level": 50.0}, P + "Fixtures/SetDimmerLevel?level=50.0")
av("set_selected_attribute", {"control": "Colour", "function": "Red", "value": 0.5, "programmer": True,
                              "create_restore_point": True},
   P + "Fixtures/SetControlValueByName?controlName=Colour&functionName=Red&value=0.500&programmer=true&createRestorePoint=true")
for name, member in [("toggle_highlight", "Fixtures/ToggleHighlight"), ("flash_selected_on", "Fixtures/FlashOn"),
                     ("flash_selected_out", "Fixtures/FlashOut"), ("clear_selected_flash", "Fixtures/ClearFlash"),
                     ("clear_selection", "Selection/Clear"), ("pattern_next", "Selection/PatternNext"),
                     ("pattern_previous", "Selection/PatternPrevious"), ("invert_pattern", "Selection/InvertPattern")]:
    av(name, {}, P + member)
av("select_fixture", UN, P + "Selection/SelectFixture?handle_userNumber=6")
av("set_blind", {"enabled": True, "set_changes_live": True, "fade_time": 5.0, "fixture_overlap": 100.0},
   S + "Programmer/SetBlind?enabled=true&setChangesLive=true&fadeTime=5.00&fixtureOverlap=100.0")
av("try_cue", {}, S + "Programmer/TryCue")
for name, member in [("flash_fixture", "FlashFixture"), ("clear_flash_fixture", "ClearFlashFixture"),
                     ("swop_fixture", "SwopFixture"), ("clear_swop_fixture", "ClearSwopFixture")]:
    av(name, UN, S + f"Fixtures/{member}?handle_userNumber=6")

# ── Timelines, timecode, DMX, pages, mode ────────────────────────────────
for name, member in [("timeline_play", "PlayTimeline"), ("timeline_pause", "PauseTimeline"),
                     ("timeline_play_pause", "PlayPauseTimeline"), ("timeline_stop", "StopTimeline"),
                     ("timeline_reset", "ResetTimeline"), ("timeline_release", "ReleaseTimeline"),
                     ("timeline_toggle_latch", "ToggleLatchTimeline")]:
    av(name, UN, S + f"Timelines/{member}?handle_userNumber=6")
av("timeline_skip", {**UN, "forwards": True}, S + "Timelines/SkipTimeline?handle_userNumber=6&forwards=true")
av("timeline_flash", {**UN, "flash": True}, S + "Timelines/FlashTimeline?handle_userNumber=6&flashState=true")
av("release_all_timelines", {}, S + "Timelines/ReleaseAllTimelines")
av("set_timecode_enabled", {"enabled": True}, S + "Timecode/SetEnabled?enabled=true")
av("timecode_play", {"timecode": 1}, S + "Timecode/Play?timecodeId=1")
av("timecode_pause", {"timecode": 1}, S + "Timecode/Pause?timecodeId=1")
av("timecode_restart", {"timecode": 1}, S + "Timecode/Restart?timecodeId=1")
av("set_dmx_frozen", {"frozen": True}, S + "Dmx/FreezeDmx?freeze=true")
av("set_group_page", {"group": "Playbacks", "page": 2}, S + "Handles/SetGroupPage?groupName=Playbacks&page=2")
av("toggle_desk_mode", {}, S + "System/ToggleMode")

# ── Reads ────────────────────────────────────────────────────────────────
G = "/titan/get/2/"
av("get_software_version", {}, G + "System/SoftwareVersion",
   http_reply={"status": 200, "body": '"19.2"'}, expect_result={"ok": {"kind": "value", "value": "19.2"}})
av("get_show_name", {}, G + "Show/ShowName",
   http_reply={"status": 200, "body": '"V10 Demo Show"'}, expect_result={"ok": {"kind": "value", "value": "V10 Demo Show"}})
for name, member in [("get_desk_mode", "System/DeskMode"), ("get_desk_blackout", "Masters/IsDeskBlackedOut"),
                     ("get_grand_master_output", "Masters/GrandMasterOutputLevel"),
                     ("get_blind_active", "Programmer/BlindActive"),
                     ("get_highlight", "Programmer/Editor/Fixtures/Highlight"),
                     ("get_dmx_output_enabled", "Titan/DmxOutputEnabled"), ("get_timecode_enabled", "Timecode/Enabled"),
                     ("get_live_cue_number", "CueLists/LiveCueNumber"), ("get_next_cue_number", "CueLists/NextCueNumber"),
                     ("get_command_line", "Command/CommandLineText"), ("get_current_world", "Handles/CurrentWorldName"),
                     ("get_device_info", "Titan/DeviceInfo")]:
    av(name, {}, G + member)
av("get_property", {"provider": "Masters", "property": "IsDeskBlackedOut"}, G + "Masters/IsDeskBlackedOut",
   http_reply={"status": 200, "body": "true"}, expect_result={"ok": {"kind": "value", "value": True}})
av("get_handles", {}, "/titan/handles")
av("get_group_handles", {"group": "Fixtures"}, "/titan/handles/Fixtures")
av("get_page_handles", {"group": "Colours", "page": 0}, "/titan/handles/Colours/0")

# ── Telemetry ────────────────────────────────────────────────────────────
# The introduction's handle record (Active, Legend) and the community
# module's (legend, userNumber.hashCode).
telemetry(AV, "handles-documented", inbound_http={"path": "/titan/handles", "body": json.dumps([
    {"handleLocation": {"group": "Fixtures", "index": 0, "page": 1},
     "properties": [{"Key": "lockState", "Value": "Unlocked"}],
     "titanId": 1689, "type": "fixtureHandle", "Active": False, "Legend": ""},
    {"handleLocation": {"group": "Colours", "index": 0, "page": 0},
     "properties": [{"Key": "lockState", "Value": "Unlocked"}],
     "titanId": 5331, "type": "paletteHandle", "Active": True, "Legend": "Red"}])},
    expect_state={"handles": {
        "1689": {"legend": "", "active": False, "type": "fixtureHandle", "group": "Fixtures", "page": 1, "index": 0},
        "5331": {"legend": "Red", "active": True, "type": "paletteHandle", "group": "Colours", "page": 0, "index": 0}}})
telemetry(AV, "handles-lower-case", inbound_http={"path": "/titan/handles", "body": json.dumps([
    {"handleLocation": {"group": "PlaybackWindow", "index": 2, "page": 0}, "titanId": 1895,
     "type": "playbackHandle", "legend": "Intro", "active": True,
     "userNumber": {"hashCode": "6", "userNumbers": [6]}}])},
    expect_state={"handles": {"1895": {"legend": "Intro", "active": True, "type": "playbackHandle",
                                       "group": "PlaybackWindow", "page": 0, "index": 2, "user_number": "6"}}})
telemetry(AV, "software-version", inbound_http={"path": "/titan/get/2/System/SoftwareVersion", "body": '"19.2"'},
          expect_state={"system": {"software_version": "19.2"}})
telemetry(AV, "blackout", inbound_http={"path": "/titan/get/2/Masters/IsDeskBlackedOut", "body": "true"},
          expect_state={"masters": {"desk_blackout": True}})
telemetry(AV, "blind", inbound_http={"path": "/titan/get/2/Programmer/BlindActive", "body": "false"},
          expect_state={"programmer": {"blind": False}})
telemetry(AV, "highlight", inbound_http={"path": "/titan/get/2/Programmer/Editor/Fixtures/Highlight", "body": "true"},
          expect_state={"programmer": {"highlight": True}})
