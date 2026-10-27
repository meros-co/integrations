TD = "tascam-da6400"
# TASCAM DA-6400 TELNET protocol (TEAC, Ver. 1.00): the machine ID "0", a two-character command and its
# data, then LF and CR. Numbers of takes travel as tens, ones, thousands, hundreds ("2301" is 123).
# Transport and preset commands are not acknowledged; sense commands are answered by a return command,
# whose own line ends LF CR too.


def _td(command, input, wire, **extra):
    text(TD, command, input, wire, **extra)


def _tdv(command, input, wire, reply, value, **extra):
    text(TD, command, input, wire, device_reply=reply + "\n\r",
         expect_result={"ok": {"kind": "value", "value": value}}, **extra)


_UNVERIFIED = {"ok": {"kind": "unverified"}}

# Transport (pp.8-11).
_td("stop", {}, "010\n\r", expect_result=_UNVERIFIED)
_td("play", {}, "012\n\r")
_td("record", {}, "01300\n\r")
_td("record_pause", {}, "01301\n\r")
_td("take_split", {}, "01302\n\r")
_td("pause", {}, "01401\n\r")
_td("search", {"direction": "reverse_high"}, "01611\n\r")
_td("skip", {"to": "previous_mark"}, "01A21\n\r")
_td("locate_take", {"take": 123}, "0232301\n\r")
_td("locate_take_then", {"take": 7, "then": "pause"}, "023070014\n\r")
_td("locate_time", {"take": 12, "hours": 0, "minutes": 1, "seconds": 30, "frames": 5},
    "02C120000013005\n\r")
_td("locate_time_then", {"take": 999, "hours": 1, "minutes": 2, "seconds": 3, "frames": 4, "then": "stop"},
    "02C99090102030410\n\r")
_td("set_clock", {"year": 2016, "month": 2, "day": 23, "hours": 12, "minutes": 34},
    ["0271602231234\n\r", "027FF\n\r"])
_tdv("get_clock", {}, "027FF\n\r", "0A7160223123456", "160223123456")
_td("set_repeat", {"mode": "vamping"}, ["03721\n\r", "037FF\n\r"])
_tdv("get_repeat", {}, "037FF\n\r", "0B701", "01")
_td("set_remote_local", {"mode": "remote"}, ["04C00\n\r", "04CFF\n\r"])
_tdv("get_remote_local", {}, "04CFF\n\r", "0CC01", "01")
_td("set_play_mode", {"mode": "one_take"}, ["04D01\n\r", "04E\n\r"])
_tdv("get_play_mode", {}, "04E\n\r", "0CE00", "00")
_tdv("get_status", {}, "050\n\r", "0D081", "81")
_tdv("get_take_number", {}, "055\n\r", "0D5002301", "002301")
_tdv("get_take_info", {}, "057\n\r", "0D7230100053010", "230100053010")
_tdv("get_time", {"kind": "remaining"}, "05811\n\r", "0D81100120304", "00120304")
_tdv("get_take_title", {"take": 5}, "0590500\n\r", "0D90500Intro", "Intro")
_tdv("get_totals", {}, "05D\n\r", "0DD120001020304", "120001020304")
_tdv("get_keyboard_type", {}, "05F\n\r", "0DF01", "01")
_tdv("get_error", {}, "078\n\r", "0F80101", "0101")
_tdv("get_caution", {}, "079\n\r", "0F90601", "0601")
_tdv("get_information", {}, "00F\n\r", "08F0130", "0130")
# A refused sense is answered ILLEGAL (F2).
text(TD, "get_mark_time", {"mark": 3}, "07F03580300\n\r", device_reply="0F2\n\r",
     expect_result={"error": {"error": "device_error"}}, file="get_mark_time-illegal")

# Vendor commands: projects (pp.31-36).
_td("project_skip", {"direction": "previous"}, "07F021A01\n\r")
_td("select_project", {"project": 54}, "07F022300005400\n\r")
text(TD, "create_project", {}, "07F024000\n\r", device_reply="0FF02C00000\n\r",
     expect_result={"ok": {"kind": "ack"}})
_td("set_new_project_name", {"name": "Concert"}, ["07F024001Concert\n\r", "07F024001FF\n\r"])
_tdv("get_new_project_name", {}, "07F024001FF\n\r", "0FF02C001Concert", "Concert")
text(TD, "rebuild_project", {}, "07F0242\n\r", device_reply="0FF02C200\n\r",
     expect_result={"ok": {"kind": "ack"}})
text(TD, "delete_project", {"project": 3}, "07F024400000300\n\r", device_reply="0FF02C400\n\r",
     expect_result={"ok": {"kind": "ack"}})
_tdv("get_project_number", {}, "07F0255\n\r", "0FF02D500005400", "00005400")
_tdv("get_project_name", {"project": 2}, "07F025A0200\n\r", "0FF02DA0200Live Show", "Live Show")
_tdv("get_project_count", {}, "07F025E\n\r", "0FF02DE1200", "1200")

# Marks (pp.36-40).
_td("add_mark", {}, "07F0310\n\r")
_td("locate_mark", {"mark": 98}, "07F03239800\n\r")
_td("set_mark_time", {"mark": 2, "hours": 0, "minutes": 10, "seconds": 0, "frames": 12},
    ["07F0324020000100012\n\r", "07F03580200\n\r"])
_td("set_mark_name", {"mark": 2, "name": "Chorus"}, ["07F03290200Chorus\n\r", "07F03590200\n\r"])
_td("delete_mark", {"mark": 4}, "07F03440400\n\r")
_td("delete_nearest_mark", {}, "07F0344\n\r")
_tdv("get_mark_number", {}, "07F0355\n\r", "0FF03D50300", "0300")
_tdv("get_mark_time", {"mark": 3}, "07F03580300\n\r", "0FF03D8030000100012", "00100012")
_tdv("get_mark_name", {"mark": 3}, "07F03590300\n\r", "0FF03D90300Verse", "Verse")
_tdv("get_mark_count", {}, "07F035D\n\r", "0FF03DD1200", "1200")

# Timecode and sync (pp.41-46).
_td("set_chase", {"enabled": True}, ["07F060001\n\r", "07F0600FF\n\r"])
_tdv("get_chase", {}, "07F0600FF\n\r", "0FF068000", "00")
_td("set_tc_start", {"hours": 1, "minutes": 0, "seconds": 0, "frames": 0}, ["07F061001000000\n\r", "07F0610FF\n\r"])
_tdv("get_tc_start", {}, "07F0610FF\n\r", "0FF069001000000", "01000000")
_td("set_tc_user_bits", {"user_bits": "12AB34CD"}, ["07F061112AB34CD\n\r", "07F0611FF\n\r"])
_tdv("get_tc_user_bits", {}, "07F0611FF\n\r", "0FF069112AB34CD", "12AB34CD")
_td("restart_timecode", {}, "07F0612\n\r")
_td("set_tc_generator_mode", {"mode": "rec_run"}, ["07F062005\n\r", "07F0620FF\n\r"])
_tdv("get_tc_generator_mode", {}, "07F0620FF\n\r", "0FF06A003", "03")
_td("set_tc_frame_type", {"rate": "29.97df"}, ["07F062129DF\n\r", "07F0621FF\n\r"])
_tdv("get_tc_frame_type", {}, "07F0621FF\n\r", "0FF06A130DF", "30DF")
_td("set_tc_output", {"source": "playback"}, ["07F063001\n\r", "07F0630FF\n\r"])
_tdv("get_tc_output", {}, "07F0630FF\n\r", "0FF06B000", "00")
_td("set_clock_master", {"source": "slot2"}, ["07F064004\n\r", "07F0640FF\n\r"])
_tdv("get_clock_master", {}, "07F0640FF\n\r", "0FF06C002", "02")
_td("set_word_mode", {"mode": "thru"}, ["07F064811\n\r", "07F0648FF\n\r"])
_tdv("get_word_mode", {}, "07F0648FF\n\r", "0FF06C801", "01")

# Recording (pp.47-55).
_ARMED = "1" * 8 + "0" * 56
_td("set_record_function", {"channel": 64, "enabled": False}, ["07F080064000\n\r", "07F0800FF\n\r"])
_td("set_record_function_all", {"enabled": True}, ["07F080000001\n\r", "07F0800FF\n\r"])
_tdv("get_record_function", {}, "07F0800FF\n\r", "0FF08800100" + _ARMED, _ARMED)
_td("set_input_monitor", {"channel": 3, "enabled": True}, ["07F080103001\n\r", "07F0801FF\n\r"])
_td("set_input_monitor_all", {"enabled": False}, ["07F080100000\n\r", "07F0801FF\n\r"])
_tdv("get_input_monitor", {}, "07F0801FF\n\r", "0FF08810100" + _ARMED, _ARMED)
_td("set_bit_length", {"bits": "24"}, ["07F082224\n\r", "07F0822FF\n\r"])
_tdv("get_bit_length", {}, "07F0822FF\n\r", "0FF08A216", "16")
_td("set_max_file_size", {"size": "2gb"}, ["07F0823004820\n\r", "07F0823FF\n\r"])
_td("set_max_file_minutes", {"minutes": 240}, ["07F0823014002\n\r", "07F0823FF\n\r"])
_tdv("get_max_file_size", {}, "07F0823FF\n\r", "0FF08A3004006", "004006")
_td("set_pause_mode", {"mode": "no_split"}, ["07F082401\n\r", "07F0824FF\n\r"])
_tdv("get_pause_mode", {}, "07F0824FF\n\r", "0FF08A400", "00")
_td("set_user_word", {"word": "STAGE"}, ["07F082ASTAGE\n\r", "07F085A\n\r"])
_tdv("get_user_word", {}, "07F085A\n\r", "0FF08DASTAGE", "STAGE")
_td("set_interval_marker_time", {"minutes": 123}, ["07F08322301\n\r", "07F0832FF\n\r"])
_tdv("get_interval_marker_time", {}, "07F0832FF\n\r", "0FF08B21500", "1500")
_td("set_audio_over_marker", {"enabled": True}, ["07F084101\n\r", "07F0841FF\n\r"])
_tdv("get_audio_over_marker", {}, "07F0841FF\n\r", "0FF08C101", "01")
_td("set_time_interval_marker", {"enabled": False}, ["07F084200\n\r", "07F0842FF\n\r"])
_tdv("get_time_interval_marker", {}, "07F0842FF\n\r", "0FF08C200", "00")
_td("set_sync_unlock_marker", {"enabled": True}, ["07F084301\n\r", "07F0843FF\n\r"])
_tdv("get_sync_unlock_marker", {}, "07F0843FF\n\r", "0FF08C301", "01")
_td("set_sample_rate", {"rate": "96000"}, ["07F0850960000\n\r", "07F0850FF\n\r"])
_tdv("get_sample_rate", {}, "07F0850FF\n\r", "0FF08D0480000", "480000")
_td("set_file_name_mode", {"mode": "user_word"}, ["07F086011\n\r", "07F0860FF\n\r"])
_tdv("get_file_name_mode", {}, "07F0860FF\n\r", "0FF08E000", "00")

# Media, external control, routing, meters, system (pp.56-64).
_tdv("get_media_remaining", {"media": "usb"}, "07F100101\n\r", "0FF10810100001205", "00001205")
_tdv("get_current_media_remaining", {}, "07F1001\n\r", "0FF10810016000000", "0016000000")
text(TD, "format_media", {"full": True}, "07F10440100\n\r", device_reply="0FF10C400\n\r",
     expect_result={"ok": {"kind": "ack"}})
_td("set_aux_key", {"aux": 6, "function": "mark_skip_forward"}, ["07F1110060A\n\r", "07F111006FF\n\r"])
_tdv("get_aux_key", {"aux": 6}, "07F111006FF\n\r", "0FF1190060D", "0D")
_td("set_aux_tally", {"aux": 1, "function": "eom_error"}, ["07F11110106\n\r", "07F111101FF\n\r"])
_tdv("get_aux_tally", {"aux": 1}, "07F111101FF\n\r", "0FF11910103", "03")
_td("set_input_route", {"channel": 63, "slot": 2, "source": 64}, ["07F125063006410\n\r", "07F125000FF\n\r"])
_ROUTE = "".join(f"{c % 100:02d}0{c // 100}" for c in range(1, 65))
_tdv("get_input_routing", {}, "07F125000FF\n\r", "0FF12D00100" + _ROUTE, _ROUTE)
_td("set_output_route", {"slot": 1, "channel": 63, "source": 32}, ["07F125163003200\n\r", "07F125100FF\n\r"])
_tdv("get_output_routing", {"slot": 2}, "07F125101FF\n\r", "0FF12D10110" + _ROUTE, _ROUTE)
_td("set_peak_hold", {"hold": "infinite"}, ["07F1321FE\n\r", "07F1321FF\n\r"])
_tdv("get_peak_hold", {}, "07F1321FF\n\r", "0FF13A110", "10")
_td("clear_peaks", {}, "07F1330\n\r")
_td("set_reference_level", {"level": "-18"}, ["07F142018\n\r", "07F1420FF\n\r"])
_tdv("get_reference_level", {}, "07F1420FF\n\r", "0FF14A020", "20")

# Take editing (pp.65-66).
text(TD, "rename_take", {"take": 14, "name": "Take A"}, "07F42001400Take A\n\r", device_reply="0FF428000\n\r",
     expect_result={"ok": {"kind": "ack"}})
text(TD, "erase_take", {"take": 14}, "07F42301400\n\r", device_reply="0FF42B000\n\r",
     expect_result={"ok": {"kind": "ack"}})
text(TD, "copy_take", {"take": 14, "destination": "B:\\Export"}, "07F42521400B:\\Export\n\r",
     device_reply="0FF42D200\n\r", expect_result={"ok": {"kind": "ack"}})

# Telemetry: pushes and the returns of the sense commands.
telemetry(TD, "change-status-transport", inbound="0F600\n\r", expect_state={}, expect_then_send=["050\n\r"])
telemetry(TD, "change-status-take", inbound="0F603\n\r", expect_state={}, expect_then_send=["055\n\r", "05D\n\r"])
telemetry(TD, "error-changed", inbound="0F0\n\r", expect_state={}, expect_then_send=["078\n\r"])
telemetry(TD, "caution-changed", inbound="0F1\n\r", expect_state={}, expect_then_send=["079\n\r"])
telemetry(TD, "illegal", inbound="0F2\n\r", expect_state={"last_illegal": True})
telemetry(TD, "status", inbound="0D082\n\r", expect_state={"transport": {"status": "record_ready"}})
telemetry(TD, "take-number", inbound="0D5002301\n\r", expect_state={"take": {"number": 123}},
          expect_then_send=["0592301\n\r"])
telemetry(TD, "take-info", inbound="0D7230100053010\n\r", expect_state={"take": {"length": "00:05:30:10"}})
telemetry(TD, "time-elapsed", inbound="0D81001020304\n\r", expect_state={"time": {"elapsed": "01:02:03:04"}})
telemetry(TD, "time-remaining", inbound="0D81100120304\n\r", expect_state={"time": {"remaining": "00:12:03:04"}})
telemetry(TD, "time-total-elapsed", inbound="0D81200000010\n\r", expect_state={"time": {"total_elapsed": "00:00:00:10"}})
telemetry(TD, "time-total-remaining", inbound="0D81323595900\n\r", expect_state={"time": {"total_remaining": "23:59:59:00"}})
telemetry(TD, "timecode", inbound="0D81410000000\n\r", expect_state={"time": {"timecode": "10:00:00:00"}})
telemetry(TD, "take-title", inbound="0D90500Intro\n\r", expect_state={"takes": {"5": {"title": "Intro"}}})
telemetry(TD, "take-title-hundreds", inbound="0D90501Outro\n\r", expect_state={"takes": {"105": {"title": "Outro"}}})
telemetry(TD, "totals", inbound="0DD120001020304\n\r",
          expect_state={"takes": {"total": 12, "total_time": "01:02:03:04"}})
telemetry(TD, "clock", inbound="0A7160223123456\n\r", expect_state={"clock": "2016-02-23T12:34:56"})
telemetry(TD, "repeat", inbound="0B721\n\r", expect_state={"repeat": "vamping"})
telemetry(TD, "remote-local", inbound="0CC00\n\r", expect_state={"remote_local": "remote"})
telemetry(TD, "play-mode", inbound="0CE01\n\r", expect_state={"play_mode": "one_take"})
telemetry(TD, "keyboard", inbound="0DF01\n\r", expect_state={"keyboard": "english"})
telemetry(TD, "version", inbound="08F0130\n\r", expect_state={"version": "1.30"})
telemetry(TD, "error", inbound="0F80901\n\r", expect_state={"error": {"code": "1-09", "text": "information_write_error"}})
telemetry(TD, "caution", inbound="0F90601\n\r", expect_state={"caution": {"code": "1-06", "text": "media_full"}})
telemetry(TD, "caution-none", inbound="0F90000\n\r", expect_state={"caution": {"code": "0-00", "text": "none"}})
telemetry(TD, "project-number", inbound="0FF02D500005400\n\r", expect_state={"project": {"number": 54}},
          expect_then_send=["07F025A5400\n\r"])
telemetry(TD, "project-changed", inbound="0FF02F600000300\n\r", expect_state={"project": {"number": 3}},
          expect_then_send=["07F025A0300\n\r"])
telemetry(TD, "project-name", inbound="0FF02DA0300Live Show\n\r", expect_state={"projects": {"3": {"name": "Live Show"}}})
telemetry(TD, "project-count", inbound="0FF02DE1200\n\r", expect_state={"projects": {"total": 12}})
telemetry(TD, "new-project-name", inbound="0FF02C001Concert\n\r", expect_state={"project": {"new_name": "Concert"}})
telemetry(TD, "create-project-done", inbound="0FF02C00011\n\r", expect_state={"operations": {"create_project": "ok"}})
telemetry(TD, "rebuild-started", inbound="0FF02C200\n\r", expect_state={"operations": {"rebuild_project": "started"}})
telemetry(TD, "delete-project-failed", inbound="0FF02C412\n\r", expect_state={"operations": {"delete_project": "failed"}})
telemetry(TD, "format-done", inbound="0FF10C40011\n\r", expect_state={"operations": {"format_media": "ok"}})
telemetry(TD, "rename-done", inbound="0FF42800011\n\r", expect_state={"operations": {"rename_take": "ok"}})
telemetry(TD, "erase-done", inbound="0FF42B011\n\r", expect_state={"operations": {"erase_take": "ok"}})
telemetry(TD, "copy-failed", inbound="0FF42D20012\n\r", expect_state={"operations": {"copy_take": "failed"}})
telemetry(TD, "mark-number", inbound="0FF03D50700\n\r", expect_state={"mark": {"number": 7}})
telemetry(TD, "mark-count", inbound="0FF03DD1200\n\r", expect_state={"marks": {"total": 12}})
telemetry(TD, "mark-time", inbound="0FF03D8980000100012\n\r", expect_state={"marks": {"98": {"time": "00:10:00:12"}}})
telemetry(TD, "mark-name", inbound="0FF03D90300Verse\n\r", expect_state={"marks": {"3": {"name": "Verse"}}})
telemetry(TD, "chase", inbound="0FF068001\n\r", expect_state={"timecode": {"chase": True}})
telemetry(TD, "tc-start", inbound="0FF069001000000\n\r", expect_state={"timecode": {"start": "01:00:00:00"}})
telemetry(TD, "tc-user-bits", inbound="0FF069112AB34CD\n\r", expect_state={"timecode": {"user_bits": "12AB34CD"}})
telemetry(TD, "tc-generator", inbound="0FF06A006\n\r", expect_state={"timecode": {"generator_mode": "tc_sync_rec"}})
telemetry(TD, "tc-frame-rate", inbound="0FF06A129DF\n\r", expect_state={"timecode": {"frame_rate": "29.97df"}})
telemetry(TD, "tc-frame-rate-ndf", inbound="0FF06A129\n\r", expect_state={"timecode": {"frame_rate": "29.97ndf"}})
telemetry(TD, "tc-output", inbound="0FF06B001\n\r", expect_state={"timecode": {"output": "playback"}})
telemetry(TD, "clock-master", inbound="0FF06C003\n\r", expect_state={"clock_master": "slot1"})
telemetry(TD, "word-mode", inbound="0FF06C800\n\r", expect_state={"word_mode": "out_terminated"})
telemetry(TD, "record-function", inbound="0FF08800100" + "10" + "0" * 62 + "\n\r",
          expect_state={"channels": {str(c): {"record": c == 1} for c in range(1, 65)}})
telemetry(TD, "input-monitor", inbound="0FF08810100" + "0" * 63 + "1\n\r",
          expect_state={"channels": {str(c): {"input_monitor": c == 64} for c in range(1, 65)}})
telemetry(TD, "bit-length", inbound="0FF08A224\n\r", expect_state={"recording": {"bit_length": 24}})
telemetry(TD, "max-file-size", inbound="0FF08A3002410\n\r",
          expect_state={"recording": {"max_file": {"kind": "size", "value": 1024}}})
telemetry(TD, "max-file-minutes", inbound="0FF08A3014002\n\r",
          expect_state={"recording": {"max_file": {"kind": "time", "value": 240}}})
telemetry(TD, "pause-mode", inbound="0FF08A401\n\r", expect_state={"recording": {"pause_mode": "no_split"}})
telemetry(TD, "user-word", inbound="0FF08DASTAGE\n\r", expect_state={"recording": {"user_word": "STAGE"}})
telemetry(TD, "interval-minutes", inbound="0FF08B22301\n\r", expect_state={"markers": {"interval_minutes": 123}})
telemetry(TD, "audio-over-marker", inbound="0FF08C101\n\r", expect_state={"markers": {"audio_over": True}})
telemetry(TD, "time-interval-marker", inbound="0FF08C200\n\r", expect_state={"markers": {"time_interval": False}})
telemetry(TD, "sync-unlock-marker", inbound="0FF08C301\n\r", expect_state={"markers": {"sync_unlock": True}})
telemetry(TD, "sample-rate", inbound="0FF08D0880000\n\r", expect_state={"recording": {"sample_rate": 88200}})
telemetry(TD, "file-name", inbound="0FF08E001\n\r", expect_state={"recording": {"file_name": "folder"}})
telemetry(TD, "media-ssd", inbound="0FF10810016000000\n\r",
          expect_state={"media": {"ssd": {"remaining_gb": 16, "remaining_mb": 0}}})
telemetry(TD, "media-usb", inbound="0FF10810100001205\n\r",
          expect_state={"media": {"usb": {"remaining_gb": 0, "remaining_mb": 512}}})
telemetry(TD, "aux-key", inbound="0FF1190030A\n\r", expect_state={"aux": {"keys": {"3": "mark_skip_forward"}}})
telemetry(TD, "aux-tally", inbound="0FF11910504\n\r", expect_state={"aux": {"tallies": {"5": "eom"}}})
_IN = "".join(f"{c % 100:02d}{(c - 1) % 2}{c // 100}" for c in range(1, 65))
telemetry(TD, "input-routing", inbound="0FF12D00100" + _IN + "\n\r",
          expect_state={"channels": {str(c): {"input": {"source": c, "slot": 1 + (c - 1) % 2}} for c in range(1, 65)}})
telemetry(TD, "output-routing", inbound="0FF12D10110" + _ROUTE + "\n\r",
          expect_state={"output_slots": {"2": {"channels": {str(c): {"source": c} for c in range(1, 65)}}}})
telemetry(TD, "peak-hold", inbound="0FF13A1FE\n\r", expect_state={"meters": {"peak_hold": "infinite"}})
telemetry(TD, "reference-level", inbound="0FF14A018\n\r", expect_state={"reference_level": -18})
