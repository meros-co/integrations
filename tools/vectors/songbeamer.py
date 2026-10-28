# SongBeamer OSC remote control (songbeamer): the addresses and types of
# SongBeamer_OSC.docx: values set with an argument, read with none and
# answered on the same address; enums sent as strings, booleans as ints 0/1,
# timecodes as strings or doubles (1.0 = 24 h).
SB = "songbeamer"
_SBU = {"expect_result": {"ok": {"kind": "unverified"}}}


def sb(command, input, address, *args, **extra):
    binary(SB, command, input, osc(address, *args), **extra)


sb("set_presentation_state", {"state": "black"}, "/presentation/state", ("s", "black"), **_SBU)
sb("go_to_page", {"page": 3}, "/presentation/page", ("i", 3))
sb("next_page", {}, "/presentation/nextpage", ("i", 1))
sb("previous_page", {}, "/presentation/prevpage", ("i", 1))
sb("set_page_caption", {"caption": "Vers 1"}, "/presentation/pagecaption", ("s", "Vers 1"))
sb("set_primary_language", {"language": 2}, "/presentation/primarylanguage", ("i", 2))
sb("set_languages", {"languages": "13"}, "/presentation/languages", ("s", "13"))
sb("set_languages", {}, "/presentation/languages", ("s", ""), file="set_languages-all")
sb("set_message_text", {"text": "Parking lot B is full"}, "/presentation/message/text", ("s", "Parking lot B is full"))
sb("set_message_visible", {"visible": True}, "/presentation/message/visible", ("i", 1))
sb("set_permanent_black", {"black": False}, "/presentation/permanentblack", ("i", 0))
sb("set_stage_layout", {"layout": "StageMonitor"}, "/stage/layoutname", ("s", "StageMonitor"))
sb("set_stage_timer", {"time": "05:00"}, "/stage/timerinit", ("s", "05:00"))
sb("set_stage_timer", {"time": "+30"}, "/stage/timerinit", ("s", "+30"), file="set_stage_timer-relative")
sb("set_stage_message", {"text": "Wrap up"}, "/stage/message/text", ("s", "Wrap up"))
sb("select_playlist_item", {"index": 4}, "/playlist/itemindex", ("i", 4))
sb("next_playlist_item", {}, "/playlist/next", ("i", 1))
sb("previous_playlist_item", {}, "/playlist/previous", ("i", 1))
sb("get_playlist_item_caption", {"index": 2}, "/playlist/items/2/caption",
   device_reply_hex=hexs(osc("/playlist/items/2/caption", ("s", "Amazing Grace"))),
   expect_result={"ok": {"kind": "value", "value": "Amazing Grace"}})
sb("get_playlist_item_filename", {"index": 2}, "/playlist/items/2/filename")
sb("get_playlist_item_type", {"index": 2}, "/playlist/items/2/streamclass")
sb("get_playlist_item_color", {"index": 2}, "/playlist/items/2/color")
sb("get_page_caption", {"index": 0}, "/presentation/pages/0/caption")
sb("set_video_state", {"state": "pause"}, "/video/state", ("s", "pause"))
sb("set_video_position", {"position": "-10"}, "/video/position", ("s", "-10"))
sb("set_live_video", {"state": "play"}, "/livevideo/state", ("s", "play"))
sb("get_info", {}, "/info")
sb("get", {"address": "presentation/text"}, "/presentation/text",
   device_reply_hex=hexs(osc("/presentation/text", ("s", "Amazing grace, how sweet the sound"))),
   expect_result={"ok": {"kind": "value", "value": "Amazing grace, how sweet the sound"}})

# ── Telemetry ──
telemetry(SB, "state", expect_connect_wire_hex=[hexs(osc("/xremote")), hexs(osc("/presentation/state"))],
          inbound_hex=hexs(osc("/presentation/state", ("i", 2))), expect_state={"presentation": {"state": "page"}})
telemetry(SB, "state-string", inbound_hex=hexs(osc("/presentation/state", ("s", "logo"))),
          expect_state={"presentation": {"state": "logo"}})
telemetry(SB, "page", inbound_hex=hexs(osc("/presentation/page", ("i", 3))), expect_state={"presentation": {"page": 3}})
telemetry(SB, "page-count", inbound_hex=hexs(osc("/presentation/pagecount", ("i", 6))),
          expect_state={"presentation": {"page_count": 6}})
telemetry(SB, "page-caption", inbound_hex=hexs(osc("/presentation/pagecaption", ("s", "Refrain"))),
          expect_state={"presentation": {"page_caption": "Refrain"}})
telemetry(SB, "text", inbound_hex=hexs(osc("/presentation/text", ("s", "Line one"))),
          expect_state={"presentation": {"text": "Line one"}})
telemetry(SB, "message-visible", inbound_hex=hexs(osc("/presentation/message/visible", ("i", 1))),
          expect_state={"presentation": {"message": {"visible": True}}})
telemetry(SB, "permanent-black", inbound_hex=hexs(osc("/presentation/permanentblack", ("i", 0))),
          expect_state={"presentation": {"permanent_black": False}})
telemetry(SB, "page-n-caption", inbound_hex=hexs(osc("/presentation/pages/2/caption", ("s", "Vers 2"))),
          expect_state={"pages": {"2": {"caption": "Vers 2"}}})
telemetry(SB, "playlist-item", inbound_hex=hexs(osc("/playlist/itemindex", ("i", 5))),
          expect_state={"playlist": {"item": 5}})
telemetry(SB, "playlist-item-caption", inbound_hex=hexs(osc("/playlist/items/1/caption", ("s", "Welcome"))),
          expect_state={"playlist": {"items": {"1": {"caption": "Welcome"}}}})
telemetry(SB, "playlist-changed", inbound_hex=hexs(osc("/playlist/changed", ("i", 1))),
          state_before={"playlist": {"count": 3, "items": {"0": {"caption": "Old"}}}},
          expect_state={"playlist": {"count": 3}},
          expect_then_send_hex=[hexs(osc("/playlist/count")), hexs(osc("/playlist/filename")),
                                hexs(osc("/playlist/itemindex"))])
telemetry(SB, "video-state", inbound_hex=hexs(osc("/video/state", ("s", "play"))), expect_state={"video": {"state": "play"}})
telemetry(SB, "video-position", inbound_hex=hexs(osc("/video/position", ("d", 0.5))),
          expect_state={"video": {"position_s": 43200.0}})
telemetry(SB, "live-video", inbound_hex=hexs(osc("/livevideo/state", ("s", "stop"))), expect_state={"live_video": "stop"})
telemetry(SB, "info", inbound_hex=hexs(osc("/info", ("s", "6.13a"))), expect_state={"info": "6.13a"})
