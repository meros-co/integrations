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
    # Handle 1689 was deleted on the console: the list replaces the handles.
    state_before={"handles": {"1689": {"legend": "", "active": False, "type": "fixtureHandle"}}},
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


# ── Full control: recording, colour, masked clears, timers, timelines, levels ──
# Property sets are a POST of the value as text (introduction, Set Property
# Value). Types the documentation gives no wire form for are written as the
# spec's quirks describe them (hsi_hue/hsi_saturation/hsi_intensity, mask
# names, hh:mm:ss:ff times, event type names, an empty timeStamp).
def av_set(command, input, target, body, **extra):
    V.append({"spec": AV, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": target, "body": body}, **extra})


G = "/titan/set/2/"
av("store_cue", {"group": "PlaybackWindow", "index": 4},
   S + "Playbacks/StoreCue?group=PlaybackWindow&index=4&updateOnly=false",
   http_reply={"status": 200, "body": "12"}, expect_result={"ok": {"kind": "ack"}})
av("set_record_type", {"record_type": "CueList"}, S + "Playbacks/SetRecordType?recordType=CueList")
av_set("set_palette_number", {"user_number": 12}, G + "Palette/CurrentPaletteNumber", "12.000")
av_set("set_palette_name", {"name": "Deep Blue"}, G + "Palette/CurrentPaletteName", "Deep Blue")
av_set("set_palette_shared", {"shared": False}, G + "Palette/StorePaletteMode", "false")
av("store_palette", {}, S + "Palette/StoreCurrentPalette")
av("store_palette_replace", {}, S + "Palette/StoreCurrentPaletteReplace")
av("delete_current_palette", {}, S + "Palette/DeleteCurrentPalette")
av("store_group", {"user_number": 3.5}, S + "Group/StoreGroup?userNumber=3.500")
av("store_group_replace", {"group_id": 2041}, S + "Group/StoreGroupReplace?groupId=2041")
av("delete_group", {"titan_id": 2041}, S + "Group/DeleteGroup?titanId=2041")
av("set_selected_colour_hsi", {"hue": 240.0, "saturation": 1.0, "intensity": 0.5},
   S + "Programmer/Editor/Fixtures/SetColourControlValues?hsi_hue=240.00&hsi_saturation=1.000"
       "&hsi_intensity=0.500&programmer=true&createRestorePoint=true")
av("clear_programmer_masked", {"mask": "Colour,Position", "fade_time": 2.0},
   S + "Programmer/Editor/Clear?mask=Colour,Position&presets=false&clearAllProgrammers=false&fadeTime=2.00")
av("clear_selected_fixtures_masked", {"mask": "Intensity", "clear_selection": True},
   S + "Programmer/Editor/ClearSelectedFixtures?mask=Intensity&presets=false&fadeTime=0.00&clearSelection=true")
av_set("set_clear_selected_only", {"selected_only": True}, G + "Programmer/Editor/Clear/Selected", "true")
av("clear_effects", {"effect_mask": "Colour", "selected": True},
   S + "Programmer/Editor/ClearEffects?effectMask=Colour&selected=true")
av("set_timer_source", {"timer": "TimecodeTwo", "source": "Smpte"}, S + "Timecode/TimecodeTwo/SetSource?source=Smpte")
av("timer_play", {"timer": "TimecodeOne"}, S + "Timecode/TimecodeOne/Play")
av("timer_pause", {"timer": "TimecodeOne"}, S + "Timecode/TimecodeOne/Pause")
av("timer_reset", {"timer": "TimecodeFour"}, S + "Timecode/TimecodeFour/Reset")
av("set_timer_start_time", {"timer": "TimecodeThree", "time": "01:00:00:00"},
   S + "Timecode/TimecodeThree/SetStartTime?time=01:00:00:00")
av("set_timecode_source_by_id", {"timecode": 1, "source": "Internal"},
   S + "Timecode/SetTimecodeSource?timecodeId=1&source=Internal")
av("set_timecode_port", {"timecode": 1, "port": 2}, S + "Timecode/SetTimecodePort?timecodeId=1&port=2")
av("timeline_play_from_time", {**UN, "time": "00:01:30:12"},
   S + "Timelines/PlayTimelineFromTime?handle_userNumber=6&time=00:01:30:12")
av("timeline_play_from_time_by_id", {"timeline_id": 1895, "time": "00:00:10:00"},
   S + "Timelines/PlayTimelineFromTimeById?timelineId=1895&time=00:00:10:00")
av("timeline_play_from_cursor", {"by": "titanId", "handle": "1895"},
   S + "Timelines/PlayTimelineFromCursor?handle_titanId=1895")
av("timeline_play_reset", UN, S + "Timelines/PlayResetTimeline?handle_userNumber=6")
for name, member in [("timeline_play_by_id", "PlayTimelineById"), ("timeline_pause_by_id", "PauseTimelineById"),
                     ("timeline_stop_by_id", "StopTimelineById"), ("timeline_reset_by_id", "ResetTimelineById")]:
    av(name, {"timeline_id": 1895}, S + "Timelines/%s?timelineId=1895" % member)
av("set_timeline_level", {"by": "location", "handle": "playback_2_1", "level": 0.75},
   S + "Timelines/SetTimelineHandleFaderLevel?handle_location=playback_2_1&value_level=0.750")
av("set_timeline_fader_level", {"group": "PlaybackWindow", "index": 2, "level": 1.0},
   S + "Timelines/SetTimelineFaderLevel?group=PlaybackWindow&index=2&value_level=1.000&accuracy=1.000")
av("set_master_level", {"group": "playback", "index": 1, "level": 0.5},
   S + "Masters/SetMasterLevel?group=playback&index=1&value_level=0.500&accuracy=1.000&initialise=false")
av("set_master_level_delta", {"group": "playback", "index": 1, "delta": 0.5},
   S + "Masters/SetMasterLevel?group=playback&index=1&value_leveldelta=0.500&accuracy=1.000&initialise=false")
av("set_group_fader_level", {"group": "Groups", "index": 0, "level": 0.25, "accuracy": 0.5},
   S + "Group/SetGroupFaderLevel?group=Groups&index=0&value_level=0.250&accuracy=0.500")
av("set_group_level", {"by": "titanId", "handle": "2041", "level": 1.0},
   S + "Group/SetGroupHandleFaderLevel?handle_titanId=2041&value_level=1.000&accuracy=1.000")
av("handle_key_action", {"key_class": "PlaybackWindow", "event_type": "Press", "key_group": "PlaybackWindow",
                         "key_index": 3, "key_name": "Go 1"},
   S + "Handles/HandleKeyAction?keyClass=PlaybackWindow&eventType=Press&keyGroup=PlaybackWindow&keyIndex=3"
       "&keyName=Go%201&timeStamp=")
av("handle_fader_action", {"event_type": "Move", "handle_group": "PlaybackWindow", "handle_index": 3, "level": 0.5},
   S + "Handles/HandleFaderAction?eventType=Move&handleGroup=PlaybackWindow&handleIndex=3&timeStamp=&level_level=0.500")

# Telemetry of the polled properties (each reply a JSON value).
for name, path, body, state in [
        ("grand-master-output", "Masters/GrandMasterOutputLevel", "0.8", {"masters": {"grand_master_output": 0.8}}),
        ("live-cue", "CueLists/LiveCueNumber", "3", {"cue_lists": {"live_cue_number": "3"}}),
        ("next-cue", "CueLists/NextCueNumber", "4.5", {"cue_lists": {"next_cue_number": "4.5"}}),
        ("command-line", "Command/CommandLineText", '"Record Cue"', {"command": {"line": "Record Cue"}}),
        ("show-name", "Show/ShowName", '"V10 Demo Show"', {"show": {"name": "V10 Demo Show"}}),
        ("desk-mode", "System/DeskMode", '"Run"', {"system": {"desk_mode": "Run"}}),
        ("dmx-output", "Titan/DmxOutputEnabled", "true", {"titan": {"dmx_output_enabled": True}}),
        ("timecode-enabled", "Timecode/Enabled", "false", {"timecode": {"enabled": False}}),
        ("world", "Handles/CurrentWorldName", '"World 1"', {"handles_world": {"name": "World 1"}}),
        ("palette-number", "Palette/CurrentPaletteNumber", "12", {"palette": {"current_number": 12.0}}),
        ("palette-name", "Palette/CurrentPaletteName", '"Deep Blue"', {"palette": {"current_name": "Deep Blue"}}),
        ("palette-shared", "Palette/StorePaletteMode", "true", {"palette": {"store_shared": True}}),
        ("clear-selected", "Programmer/Editor/Clear/Selected", "false",
         {"programmer": {"clear_selected_only": False}}),
        ("timer-live-time", "Timecode/TimecodeOne/LiveTime", '"00:01:02:03"',
         {"timecode": {"timers": {"TimecodeOne": {"live_time": "00:01:02:03"}}}}),
        ("timer-paused", "Timecode/TimecodeTwo/Paused", "true",
         {"timecode": {"timers": {"TimecodeTwo": {"paused": True}}}}),
        ("timer-source", "Timecode/TimecodeThree/Source", '"Smpte"',
         {"timecode": {"timers": {"TimecodeThree": {"source": "Smpte"}}}}),
        ("timer-start-time", "Timecode/TimecodeFour/StartTime", '"10:00:00:00"',
         {"timecode": {"timers": {"TimecodeFour": {"start_time": "10:00:00:00"}}}})]:
    telemetry(AV, name, inbound_http={"path": "/titan/get/2/" + path, "body": body}, expect_state=state)
