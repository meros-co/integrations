# Logos Proclaim (logos-proclaim): GET /appCommand/perform?appCommandName=X
# [&index=N] on port 52195, as the App Command API article gives the
# requests ("GET http://[ipaddress]:52195/appCommand/perform?appCommandName=
# nextSlide", "...=goToServiceItem&index=4"), with the article's names. The
# password is left empty, so no login goes first (a client on the Proclaim
# computer); the login itself is covered by the engine's session tests.
PR = "logos-proclaim"
NO_LOGIN = {"settings": {"password": ""}}


def perform(command, name, input=None, index=None, **extra):
    target = f"/appCommand/perform?appCommandName={name}" + (f"&index={index}" if index is not None else "")
    http(PR, command, input or {}, "GET", target, **NO_LOGIN, **extra)


for command, name in [
    ("go_on_air", "GoOnAir"), ("go_off_air", "GoOffAir"), ("next_slide", "NextSlide"),
    ("previous_slide", "PreviousSlide"), ("next_service_item", "NextServiceItem"),
    ("previous_service_item", "PreviousServiceItem"), ("start_pre_service", "StartPreService"),
    ("start_warm_up", "StartWarmUp"), ("start_service", "StartService"), ("start_post_service", "StartPostService"),
    ("next_audio_item", "NextAudioItem"), ("previous_audio_item", "PreviousPreviousAudioItem"),
    ("video_restart", "VideoRestart"), ("video_rewind", "VideoRewind"), ("video_fast_forward", "VideoFastForward"),
    ("video_play", "VideoPlay"), ("video_pause", "VideoPause"), ("show_blank_quick_screen", "ShowBlankQuickScreen"),
    ("show_logo_quick_screen", "ShowLogoQuickScreen"), ("show_no_text_quick_screen", "ShowNoTextQuickScreen"),
    ("show_floating_hearts_quick_screen", "ShowFloatingHeartsQuickScreen"),
    ("show_floating_amens_quick_screen", "ShowFloatingAmensQuickScreen"), ("show_amen_quick_screen", "ShowAmenQuickScreen"),
    ("show_hallelujah_quick_screen", "ShowHallelujahQuickScreen"),
    ("show_praise_the_lord_quick_screen", "ShowPraiseTheLordQuickScreen"),
    ("show_he_is_risen_quick_screen", "ShowHeIsRisenQuickScreen"),
    ("show_he_is_risen_indeed_quick_screen", "ShowHeIsRisenWhiteQuickScreen"),
    ("show_text", "ShowText"), ("show_last_slide", "ShowLastSlide"),
]:
    perform(command, name)
perform("next_slide", "NextSlide", http_reply={"status": 200, "body": "success"},
        expect_result={"ok": {"kind": "ack"}}, file="next_slide-reply")
perform("go_to_service_item", "GoToServiceItem", {"item": 4}, index=4)
perform("go_to_slide", "GoToSlide", {"slide": 2}, index=2)
perform("show_custom_quick_screen", "ShowCustomQuickScreen", {"screen": 1}, index=1)
perform("show_song_part", "ShowSongLyricsVerseByIndex", {"part": "Verse", "number": 1}, index=1)
perform("show_song_part", "ShowSongLyricsChorusByIndex", {"part": "Chorus", "number": 2}, index=2, file="show_song_part-chorus-2")
perform("show_song_part_unnumbered", "ShowSongLyricsBridgeByIndex", {"part": "Bridge"})
perform("app_command", "nextSlide", {"name": "nextSlide"})
perform("app_command_with_index", "goToServiceItem", {"name": "goToServiceItem", "index": 4}, index=4)
http(PR, "get_on_air_session", {}, "GET", "/onair/session", **NO_LOGIN,
     http_reply={"status": 200, "body": "8f2c1e"}, expect_result={"ok": {"kind": "value", "value": "8f2c1e"}})

telemetry(PR, "on-air", inbound_http={"path": "/onair/session", "body": "8f2c1e"},
          expect_state={"on_air": True, "on_air_session": "8f2c1e"})
telemetry(PR, "off-air", inbound_http={"path": "/onair/session", "body": ""},
          state_before={"on_air": True, "on_air_session": "8f2c1e"}, expect_state={"on_air": False})
