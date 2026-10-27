# PlaybackPro Plus (dtvideolabs-playbackpro-plus): one vector per command,
# from the four tables of the PlaybackPro Plus X user guide. Each command is
# its characters alone (the Bitfocus Companion module sends no line ending),
# GOxxx and the like with the number unpadded. Inquiries are answered with a
# line; the other commands are not answered, so they report unverified.
PB = "dtvideolabs-playbackpro-plus"
_U = {"expect_result": {"ok": {"kind": "unverified"}}}

for command, code in [
    ("preview_fast_forward", "EF"), ("preview_fast_reverse", "ER"), ("preview_go_to_in", "EI"),
    ("preview_go_to_out", "EO"), ("preview_mark_in", "ES"), ("preview_mark_out", "EE"),
    ("preview_play_pause", "EP"), ("preview_step_back", "E-"), ("preview_step_forward", "E+"),
    ("end_all", "KL"), ("program_fast_forward", "OF"), ("program_fast_reverse", "OR"),
    ("go_to_10_seconds", "10"), ("go_to_20_seconds", "20"), ("go_to_30_seconds", "30"),
    ("program_go_to_in", "OI"), ("program_go_to_out", "OO"), ("enable_loop_temp", "SL"),
    ("disable_loop_temp", "EL"), ("pause", "PA"), ("play", "PL"), ("program_play_pause", "OP"),
    ("program_step_back", "O-"), ("program_step_forward", "O+"), ("clear_preview", "CL"),
    ("decrease_end_all_fade", "K-"), ("increase_end_all_fade", "K+"), ("toggle_freeze_temp", "FT"),
    ("decrease_link_temp", "I-"), ("increase_link_temp", "I+"), ("toggle_link_temp", "IT"),
    ("decrease_loop_temp", "L-"), ("increase_loop_temp", "L+"), ("toggle_loop_temp", "LT"),
    ("next_clip", "NX"), ("previous_clip", "PR"), ("save_show", "SA"), ("decrease_fade_on_take", "F-"),
    ("increase_fade_on_take", "F+"), ("decrease_take_duration", "T-"), ("increase_take_duration", "T+"),
]:
    text(PB, command, {}, code)
text(PB, "take", {}, "TA", **_U)
text(PB, "load_preview", {"clip": 7}, "GO7", **_U)
text(PB, "load_and_take", {"clip": 12}, "GT12")
text(PB, "select_preview_by_id", {"id": 101}, "SI101")
text(PB, "take_by_id", {"id": 3}, "TI3")
text(PB, "send_command", {"command": "GO12"}, "GO12")
for command, code in [("get_clip_durations", "AD"), ("get_clip_names", "AN"), ("get_clip_numbers", "AU"),
                      ("get_clip_ids", "ID"), ("get_loop_status", "LS"), ("get_preview_duration", "VD"),
                      ("get_preview_name", "VN"), ("get_preview_number", "VU"), ("get_program_duration", "GD"),
                      ("get_program_name", "GN"), ("get_program_number", "GU"), ("get_time_elapsed", "TE"),
                      ("get_show_file", "SF")]:
    text(PB, command, {}, code)
text(PB, "get_playback_status", {}, "PS", device_reply="1\r", expect_result={"ok": {"kind": "value", "value": "1"}})
text(PB, "get_time_remaining", {}, "TR", device_reply="00:01:30:12\r\n",
     expect_result={"ok": {"kind": "value", "value": "00:01:30:12"}})

# Inquiry answers, each with the inquiry it answers.
telemetry(PB, "playing", inbound="1", request="PS", expect_state={"program": {"status": "playing", "playing": True}})
telemetry(PB, "paused", inbound="0", request="PS", expect_state={"program": {"status": "paused", "playing": False}})
telemetry(PB, "no-clip", inbound="N/A", request="PS", expect_state={"program": {"status": "none", "playing": False}})
telemetry(PB, "loop", inbound="Yes", request="LS", expect_state={"program": {"loop": True}})
for name, code, path, value in [("program-name", "GN", ("program", "name"), "Walk In Loop"),
                                ("program-number", "GU", ("program", "number"), "N/A"),
                                ("program-duration", "GD", ("program", "duration"), "00:03:00:00"),
                                ("elapsed", "TE", ("program", "elapsed"), "00:00:12:05"),
                                ("remaining", "TR", ("program", "remaining"), "00:02:47:20"),
                                ("preview-name", "VN", ("preview", "name"), "Opener"),
                                ("preview-number", "VU", ("preview", "number"), "4"),
                                ("preview-duration", "VD", ("preview", "duration"), "00:00:30:00")]:
    telemetry(PB, name, inbound=value, request=code, expect_state={path[0]: {path[1]: value}})
telemetry(PB, "show-file", inbound="Sunday Service", request="SF", expect_state={"show_file": "Sunday Service"})
telemetry(PB, "clip-names", inbound="Opener,Walk In Loop,Video. Part 2", request="AN",
          state_before={"clips": {"names": {"4": "Removed"}}},
          expect_state={"clips": {"names": {"1": "Opener", "2": "Walk In Loop", "3": "Video. Part 2"}}})
telemetry(PB, "clip-durations", inbound="00:00:30:00,00:03:00:00", request="AD",
          expect_state={"clips": {"durations": {"1": "00:00:30:00", "2": "00:03:00:00"}}})
telemetry(PB, "clip-numbers", inbound="4,N/A", request="AU", expect_state={"clips": {"numbers": {"1": "4", "2": "N/A"}}})
telemetry(PB, "clip-ids", inbound="1 Opener.mov,2 Walk In.mov", request="ID",
          expect_state={"clips": {"ids": {"1": "1 Opener.mov", "2": "2 Walk In.mov"}}})
