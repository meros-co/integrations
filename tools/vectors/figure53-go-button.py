# Figure 53 Go Button 3: OSC over TCP 53100 with double-END SLIP framing (RFC
# 1055, as OSC 1.1 requires; Go Button 3 OSC Dictionary, Getting Started).
# Addresses are copied from the dictionary; every message is sent rootless, to
# the open show. Replies are /reply/<address> with one JSON string argument
# (Replies from Go Button); the engine is fed the unframed OSC packet. Booleans
# go as int 0/1 (OSC Booleans in Go Button), numbers as 32-bit floats.
GB = "figure53-go-button"
GB_OLD = {"model": "go-button-3-early"}
GB_SHOW = "34200B51-835A-4918-A137-B6511784B6CA"   # the dictionary's example show id
GB_L1 = "5B0F1A2C-0000-4000-8000-0000000000A1"
GB_C1 = "5B0F1A2C-0000-4000-8000-0000000000B1"
GB_G1 = "5B0F1A2C-0000-4000-8000-0000000000B2"
GB_C2 = "5B0F1A2C-0000-4000-8000-0000000000B3"
GB_H1 = "5B0F1A2C-0000-4000-8000-0000000000C1"
GB_P = "^/reply/(?:show(?:_id)?/[^/]+/)?"


def gb_slip(packet: bytes) -> bytes:
    body = packet.replace(b"\xdb", b"\xdb\xdd").replace(b"\xc0", b"\xdb\xdc")
    return b"\xc0" + body + b"\xc0"


def gb_f(address, *args):
    return gb_slip(osc(address, *args))


def gb(command, input, *messages, **extra):
    """messages: (address, *args) tuples, sent in order."""
    wires = [gb_f(m[0], *m[1:]) for m in messages]
    binary(GB, command, input, wires if len(wires) > 1 else wires[0], **extra)


def gb_reply(address, data, status="ok", show=True):
    body = {"address": address, "status": status, "data": data}
    if show:
        body = {"show_id": GB_SHOW, **body}
    return osc("/reply" + address, ("s", json.dumps(body)))


_U = {"expect_result": {"ok": {"kind": "unverified"}}}

# ── Application messages ──
gb("set_always_reply", {"enabled": True}, ("/alwaysReply", ("i", 1)), ("/alwaysReply",), **_U)
gb("get_always_reply", {}, ("/alwaysReply",), device_reply_hex=hexs(gb_reply("/alwaysReply", False, show=False)),
   expect_result={"ok": {"kind": "value", "value": False}})
gb("connect", {"passcode": "1234"}, ("/connect", ("s", "1234")),
   device_reply_hex=hexs(gb_reply("/connect", "ok", show=False)), expect_result={"ok": {"kind": "value", "value": "ok"}})
gb("disconnect", {}, ("/disconnect",))
gb("set_forget_me_not", {"enabled": True}, ("/forgetMeNot", ("i", 1)), ("/forgetMeNot",))
gb("get_forget_me_not", {}, ("/forgetMeNot",))
gb("set_udp_keep_alive", {"enabled": False}, ("/udpKeepAlive", ("i", 0)), ("/udpKeepAlive",))
gb("get_udp_keep_alive", {}, ("/udpKeepAlive",))
gb("set_reply_format", {"format": "#data#"}, ("/replyFormat", ("s", "#data#")), ("/replyFormat",))
gb("reset_reply_format", {}, ("/replyFormat", ("s", "")), ("/replyFormat",))
gb("get_reply_format", {}, ("/replyFormat",))
gb("get_shows", {}, ("/shows",))
gb("thump", {}, ("/thump",), device_reply_hex=hexs(gb_reply("/thump", "thump", show=False)),
   expect_result={"ok": {"kind": "value", "value": "thump"}})
gb("get_version", {}, ("/version",), device_reply_hex=hexs(gb_reply("/version", "3.3.2", show=False)),
   expect_result={"ok": {"kind": "value", "value": "3.3.2"}})

# ── Show messages ──
gb("open_show", {"show": GB_SHOW}, (f"/show/{GB_SHOW}/open",))
gb("close_show", {}, ("/close",))
for stem, word in [("cue_lists", "cueLists"), ("running_cues", "runningCues"),
                   ("running_or_paused_cues", "runningOrPausedCues")]:
    gb(f"get_{stem}", {}, (f"/{word}",))
    gb(f"get_{stem}_shallow", {}, (f"/{word}/shallow",))
    gb(f"get_{stem}_ids", {}, (f"/{word}/uniqueIDs",))
    gb(f"get_{stem}_ids_shallow", {}, (f"/{word}/uniqueIDs/shallow",))
for stem, word, toggle in [("dim", "dim", "toggleDim"), ("full_screen", "fullScreen", "toggleFullScreen"),
                           ("main_volume_visible", "mainVolumeVisible", "toggleMainVolumeVisible"),
                           ("mute", "mute", "toggleMute")]:
    gb(f"set_{stem}", {"enabled": True}, (f"/{word}", ("i", 1)), (f"/{word}",))
    gb(f"get_{stem}", {}, (f"/{word}",))
    gb(f"toggle_{stem}", {}, (f"/{toggle}",), (f"/{word}",))
gb("get_mute", {}, ("/mute",), device_reply_hex=hexs(gb_reply("/mute", True)),
   expect_result={"ok": {"kind": "value", "value": True}}, file="get_mute-reply")
gb("set_dim_over", {"enabled": True, "seconds": 2.5}, ("/dim", ("i", 1), ("f", 2.5)), ("/dim",))
gb("go", {}, ("/go",), **_U)
gb("oops", {}, ("/oops",), ("/cue/playhead/uniqueID",))
gb("panic", {}, ("/panic",))
gb("panic_in_time", {"seconds": 3.0}, ("/panicInTime", ("f", 3.0)))
gb("pause", {}, ("/pause",))
gb("resume", {}, ("/resume",))
gb("stop", {}, ("/stop",))
gb("hard_stop", {}, ("/hardStop",))
gb("reset", {}, ("/reset",), ("/cue/playhead/uniqueID",))
gb("set_playhead", {"cue": "10"}, ("/playhead/10",), ("/cue/playhead/uniqueID",))
gb("playhead_next", {}, ("/playhead/next",), ("/cue/playhead/uniqueID",))
gb("playhead_previous", {}, ("/playhead/previous",), ("/cue/playhead/uniqueID",))
gb("get_playhead_id", {}, ("/cue/playhead/uniqueID",))
gb("set_timer", {"seconds": 90.0}, ("/timer", ("f", 90.0)), ("/timer",), **GB_OLD)
gb("get_timer", {}, ("/timer",), **GB_OLD)
gb("set_timer_elapsed", {"seconds": 90.0}, ("/timer/elapsed", ("f", 90.0)), ("/timer/elapsed",))
gb("get_timer_elapsed", {}, ("/timer/elapsed",))
gb("set_timer_duration", {"seconds": 600.0}, ("/timer/duration", ("f", 600.0)), ("/timer/duration",))
gb("get_timer_duration", {}, ("/timer/duration",))
gb("start_timer", {}, ("/timer/start",))
gb("stop_timer", {}, ("/timer/stop",))
gb("toggle_timer", {}, ("/timer/toggleRunning",))
gb("reset_timer", {}, ("/timer/reset",), ("/timer/elapsed",), ("/timer/duration",))
gb("reset_timer_early", {}, ("/timer/reset",), ("/timer",), **GB_OLD)
gb("reset_timer_duration", {}, ("/timer/reset/duration",), ("/timer/duration",))
gb("reset_timer_elapsed", {}, ("/timer/reset/elapsed",), ("/timer/elapsed",))
gb("set_volume", {"db": -12.0}, ("/volume", ("f", -12.0)), ("/volume",), ("/volumePercent",))
gb("get_volume", {}, ("/volume",))
gb("set_volume_percent", {"level": 0.5}, ("/volumePercent", ("f", 0.5)), ("/volume",), ("/volumePercent",))
gb("get_volume_percent", {}, ("/volumePercent",), device_reply_hex=hexs(gb_reply("/volumePercent", 0.5)),
   expect_result={"ok": {"kind": "value", "value": 0.5}})
gb("volume_step_up", {}, ("/volumeStepUp",), ("/volume",), ("/volumePercent",))
gb("volume_step_down", {}, ("/volumeStepDown",), ("/volume",), ("/volumePercent",))

# ── Cue and hit messages: by number, unique ID, hit number and hit ID ──
for stem, word in [("action_elapsed", "actionElapsed"), ("percent_action_elapsed", "percentActionElapsed"),
                   ("default_name", "defaultName"), ("display_name", "displayName"), ("duration", "duration"),
                   ("go_button_text", "goButtonText"), ("is_broken", "isBroken"), ("is_loaded", "isLoaded"),
                   ("is_paused", "isPaused"), ("is_running", "isRunning"), ("pre_wait_elapsed", "preWaitElapsed"),
                   ("percent_pre_wait_elapsed", "percentPreWaitElapsed"), ("unique_id", "uniqueID"),
                   ("color", "colorName"), ("name", "name"), ("number", "number"), ("pre_wait", "preWait")]:
    gb(f"get_cue_{stem}", {"ref": "10"}, (f"/cue/10/{word}",))
gb("get_cue_name", {"kind": "cue_id", "ref": GB_C1}, (f"/cue_id/{GB_C1}/name",),
   device_reply_hex=hexs(gb_reply(f"/cue_id/{GB_C1}/name", "Preshow")),
   expect_result={"ok": {"kind": "value", "value": "Preshow"}}, file="get_cue_name-by-id")
gb("get_cue_go_button_text", {"ref": "playhead"}, ("/cue/playhead/goButtonText",), file="get_cue_go_button_text-playhead")
gb("set_cue_color", {"kind": "hit", "ref": "3", "color": "green"}, ("/hit/3/colorName", ("s", "green")), ("/hit/3/colorName",))
gb("set_cue_name", {"kind": "cue_id", "ref": GB_C1, "name": "Walk-in"},
   (f"/cue_id/{GB_C1}/name", ("s", "Walk-in")), (f"/cue_id/{GB_C1}/name",))
gb("set_cue_number", {"ref": "10", "number": "10.5"}, ("/cue/10/number", ("s", "10.5")), ("/cue/10/number",))
# The dictionary's example: /cue/10/preWait/+ 1 adds a second.
gb("set_cue_pre_wait", {"ref": "10", "seconds": 1.5}, ("/cue/10/preWait", ("f", 1.5)), ("/cue/10/preWait",))
gb("adjust_cue_pre_wait", {"ref": "10", "op": "+", "seconds": 1.0}, ("/cue/10/preWait/+", ("f", 1.0)), ("/cue/10/preWait",))
gb("adjust_cue_pre_wait", {"kind": "hit_id", "ref": GB_H1, "op": "-", "seconds": 0.5},
   (f"/hit_id/{GB_H1}/preWait/-", ("f", 0.5)), (f"/hit_id/{GB_H1}/preWait",), file="adjust_cue_pre_wait-minus")
for stem, word in [("load_cue", "load"), ("panic_cue", "panic"), ("pause_cue", "pause"),
                   ("toggle_pause_cue", "togglePause"), ("reset_cue", "reset"), ("resume_cue", "resume"),
                   ("start_cue", "start"), ("stop_cue", "stop"), ("hard_stop_cue", "hardStop")]:
    gb(stem, {"ref": "10"}, (f"/cue/10/{word}",))
gb("start_cue", {"kind": "hit", "ref": "1*"}, ("/hit/1*/start",), file="start_cue-wildcard")
gb("load_cue_at", {"ref": "10", "seconds": 30.0}, ("/cue/10/loadAt", ("f", 30.0)))
gb("panic_cue_in_time", {"kind": "cue_id", "ref": GB_C1, "seconds": 5.0}, (f"/cue_id/{GB_C1}/panicInTime", ("f", 5.0)))

# ── Telemetry ──
def gb_hex(address, *args):
    return hexs(gb_f(address, *args))


def gb_show_reads():
    reads = [gb_hex(a) for a in ["/cueLists", "/cue/playhead/uniqueID", "/dim", "/mute", "/fullScreen",
                                 "/mainVolumeVisible", "/volume", "/volumePercent", "/timer/duration"]]
    return reads


GB_CUE_READS = ["name", "number", "colorName", "displayName", "defaultName", "duration", "preWait", "goButtonText",
                "isBroken", "isLoaded", "isPaused", "isRunning"]


def gb_cue_reads(uid, props):
    return [gb_hex(f"/cue_id/{uid}/{p}") for p in props]


# On connecting (no passcode configured, so no /connect): /updates 1, then the
# first poll of the running or paused cues.
telemetry(GB, "show-update",
          expect_connect_wire_hex=[gb_hex("/updates", ("i", 1)), gb_hex("/runningOrPausedCues/shallow")],
          inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}")),
          expect_state={"show": {"id": GB_SHOW}}, expect_then_send_hex=gb_show_reads())
telemetry(GB, "root-group-update", inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}/cue_id/[root group of cue lists]")),
          expect_state={}, expect_then_send_hex=[gb_hex("/cueLists")])
telemetry(GB, "cue-update", inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}/cue_id/{GB_C1}")),
          expect_state={}, expect_then_send_hex=gb_cue_reads(GB_C1, GB_CUE_READS))
telemetry(GB, "playback-position", inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}/playbackPosition", ("s", GB_C1))),
          expect_state={"show": {"id": GB_SHOW, "playhead_id": GB_C1}})
telemetry(GB, "playback-position-none", inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}/playbackPosition")),
          state_before={"show": {"id": GB_SHOW, "playhead_id": GB_C1}},
          expect_state={"show": {"id": GB_SHOW}})
telemetry(GB, "volume-update", inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}/volume", ("f", -6.0))),
          expect_state={"show": {"id": GB_SHOW, "volume_db": -6.0}})
telemetry(GB, "show-disconnect", inbound_hex=hexs(osc(f"/update/show/{GB_SHOW}/disconnect")),
          state_before={"app": {"version": "3.3.2"}, "show": {"id": GB_SHOW, "mute": False},
                        "cues": {GB_C1: {"name": "Preshow"}}, "cue_lists": {GB_L1: {"index": 0}},
                        "active": {"0": GB_C1}},
          expect_state={"app": {"version": "3.3.2"}})
telemetry(GB, "connect", inbound_hex=hexs(gb_reply("/connect", "badpass", show=False)),
          expect_state={"app": {"connect": "badpass"}})
telemetry(GB, "version", inbound_hex=hexs(gb_reply("/version", "3.3.2", show=False)),
          expect_state={"app": {"version": "3.3.2"}})
telemetry(GB, "always-reply", inbound_hex=hexs(gb_reply("/alwaysReply", True, show=False)),
          expect_state={"app": {"always_reply": True}})
telemetry(GB, "udp-keep-alive", inbound_hex=hexs(gb_reply("/udpKeepAlive", 0, show=False)),
          expect_state={"app": {"forget_me_not": False}})
telemetry(GB, "reply-format", inbound_hex=hexs(gb_reply("/replyFormat", "", show=False)),
          expect_state={"app": {"reply_format": ""}})
telemetry(GB, "shows", inbound_hex=hexs(gb_reply("/shows", [
    {"uniqueID": GB_SHOW, "displayName": "Matinee", "port": 53100, "udpReplyPort": 53101, "version": "3.3.2"}], show=False)),
          state_before={"shows": {"OLD-1": {"name": "Closed"}}},
          expect_state={"shows": {GB_SHOW: {"name": "Matinee", "port": 53100, "udp_reply_port": 53101,
                                            "version": "3.3.2"}}})
# A reply to a message with the show prefix, as the dictionary's example names it.
telemetry(GB, "volume-percent-prefixed",
          inbound_hex=hexs(osc(f"/reply/show_id/{GB_SHOW}/volumePercent",
                               ("s", json.dumps({"show_id": GB_SHOW, "address": f"/show/{GB_SHOW}/volumePercent",
                                                 "status": "ok", "data": 0.25})))),
          expect_state={"show": {"volume_percent": 0.25}})
telemetry(GB, "volume", inbound_hex=hexs(gb_reply("/volume", -96.0)), expect_state={"show": {"volume_db": -96.0}})
telemetry(GB, "dim", inbound_hex=hexs(gb_reply("/dim", True)), expect_state={"show": {"dim": True}})
telemetry(GB, "mute", inbound_hex=hexs(gb_reply("/mute", False)), expect_state={"show": {"mute": False}})
telemetry(GB, "full-screen", inbound_hex=hexs(gb_reply("/fullScreen", 1)), expect_state={"show": {"full_screen": True}})
telemetry(GB, "main-volume-visible", inbound_hex=hexs(gb_reply("/mainVolumeVisible", False)),
          expect_state={"show": {"main_volume_visible": False}})
telemetry(GB, "timer-elapsed", inbound_hex=hexs(gb_reply("/timer/elapsed", 125.5)),
          expect_state={"show": {"timer": {"elapsed": 125.5}}})
telemetry(GB, "timer-early", inbound_hex=hexs(gb_reply("/timer", 12.0)),
          expect_state={"show": {"timer": {"elapsed": 12.0}}})
telemetry(GB, "timer-duration", inbound_hex=hexs(gb_reply("/timer/duration", 600)),
          expect_state={"show": {"timer": {"duration": 600.0}}})
telemetry(GB, "playhead", inbound_hex=hexs(gb_reply("/cue/playhead/uniqueID", GB_C1)),
          expect_state={"show": {"playhead_id": GB_C1}})
# The cue lists: the main list and the hits list, a Group in the main list.
telemetry(GB, "cue-lists", inbound_hex=hexs(gb_reply("/cueLists", [
    {"uniqueID": GB_L1, "number": "", "name": "Main Cue List", "listName": "Main Cue List", "type": "Cue List",
     "colorName": "none", "armed": True, "cues": [
         {"uniqueID": GB_C1, "number": "1", "name": "Preshow", "listName": "Preshow", "type": "Audio",
          "colorName": "none", "armed": True},
         {"uniqueID": GB_G1, "number": "2", "name": "", "listName": "Group", "type": "Group",
          "colorName": "none", "armed": False, "cues": [
              {"uniqueID": GB_C2, "number": "2.1", "name": "Thunder", "listName": "Thunder", "type": "Audio",
               "colorName": "none", "armed": True}]}]}])),
          state_before={"cues": {"OLD-1": {"name": "Deleted"}}, "cue_lists": {"OLD-2": {"index": 1}}},
          expect_state={
              "cue_lists": {GB_L1: {"index": 0}},
              "cues": {
                  GB_L1: {"number": "", "name": "Main Cue List", "list_name": "Main Cue List", "type": "Cue List",
                          "color": "none", "armed": True, "index": 0, "children": {"0": GB_C1, "1": GB_G1}},
                  GB_C1: {"number": "1", "name": "Preshow", "list_name": "Preshow", "type": "Audio",
                          "color": "none", "armed": True, "index": 0, "parent": GB_L1},
                  GB_G1: {"number": "2", "name": "", "list_name": "Group", "type": "Group", "color": "none",
                          "armed": False, "index": 1, "parent": GB_L1, "children": {"0": GB_C2}},
                  GB_C2: {"number": "2.1", "name": "Thunder", "list_name": "Thunder", "type": "Audio",
                          "color": "none", "armed": True, "index": 0, "parent": GB_G1}}})
# Each cue of the tree has its other properties read.
GB_TREE_READS = ["displayName", "defaultName", "duration", "preWait", "goButtonText",
                 "isBroken", "isLoaded", "isPaused", "isRunning"]
telemetry(GB, "cue-lists-reads", inbound_hex=hexs(gb_reply("/cueLists", [
    {"uniqueID": GB_L1, "number": "", "name": "Main", "listName": "Main", "type": "Cue List",
     "colorName": "none", "armed": True, "cues": [
         {"uniqueID": GB_H1, "number": "1", "name": "Bell", "listName": "Bell", "type": "Audio",
          "colorName": "red", "armed": True}]}])),
          expect_state={"cue_lists": {GB_L1: {"index": 0}},
                        "cues": {GB_L1: {"number": "", "name": "Main", "list_name": "Main", "type": "Cue List",
                                         "color": "none", "armed": True, "index": 0, "children": {"0": GB_H1}},
                                 GB_H1: {"number": "1", "name": "Bell", "list_name": "Bell", "type": "Audio",
                                         "color": "red", "armed": True, "index": 0, "parent": GB_L1}}},
          expect_then_send_hex=gb_cue_reads(GB_L1, GB_TREE_READS) + gb_cue_reads(GB_H1, GB_TREE_READS))
# Three levels down: a Group in a Group in a list.
GB_C3 = "5B0F1A2C-0000-4000-8000-0000000000B4"
telemetry(GB, "cue-lists-deep", inbound_hex=hexs(gb_reply("/cueLists/shallow", [
    {"uniqueID": GB_L1, "number": "", "name": "Main", "listName": "Main", "type": "Cue List", "colorName": "none",
     "armed": True, "cues": [{"uniqueID": GB_G1, "number": "2", "name": "G", "listName": "G", "type": "Group",
                              "colorName": "none", "armed": True, "cues": [
                                  {"uniqueID": GB_C2, "number": "2.1", "name": "H", "listName": "H", "type": "Group",
                                   "colorName": "none", "armed": True, "cues": [
                                       {"uniqueID": GB_C3, "number": "2.1.1", "name": "Rain", "listName": "Rain",
                                        "type": "Audio", "colorName": "none", "armed": True}]}]}]}])),
          expect_state={"cue_lists": {GB_L1: {"index": 0}},
                        "cues": {GB_L1: {"number": "", "name": "Main", "list_name": "Main", "type": "Cue List",
                                         "color": "none", "armed": True, "index": 0, "children": {"0": GB_G1}},
                                 GB_G1: {"number": "2", "name": "G", "list_name": "G", "type": "Group", "color": "none",
                                         "armed": True, "index": 0, "parent": GB_L1, "children": {"0": GB_C2}},
                                 GB_C2: {"number": "2.1", "name": "H", "list_name": "H", "type": "Group",
                                         "color": "none", "armed": True, "index": 0, "parent": GB_G1,
                                         "children": {"0": GB_C3}},
                                 GB_C3: {"number": "2.1.1", "name": "Rain", "list_name": "Rain", "type": "Audio",
                                         "color": "none", "armed": True, "index": 0, "parent": GB_C2}}})
GB_TIME_READS = ["isRunning", "isPaused", "actionElapsed", "percentActionElapsed", "preWaitElapsed",
                 "percentPreWaitElapsed"]
telemetry(GB, "running-or-paused", inbound_hex=hexs(gb_reply("/runningOrPausedCues/shallow", [
    {"uniqueID": GB_C1, "number": "1", "name": "Preshow", "listName": "Preshow", "type": "Audio",
     "colorName": "none", "armed": True}])),
          state_before={"active": {"0": GB_C2, "1": GB_C1}},
          expect_state={"active": {"0": GB_C1}}, expect_then_send_hex=gb_cue_reads(GB_C1, GB_TIME_READS))
telemetry(GB, "running-or-paused-none", inbound_hex=hexs(gb_reply("/runningOrPausedCues", [])),
          state_before={"active": {"0": GB_C1}}, expect_state={})
telemetry(GB, "running", inbound_hex=hexs(gb_reply("/runningCues/shallow", [
    {"uniqueID": GB_C2, "number": "2.1"}, {"uniqueID": GB_H1, "number": "1"}])),
          expect_state={"running": {"0": GB_C2, "1": GB_H1}})
# A cue's or hit's properties, read by unique ID.
for word, key, data, value in [
        ("name", "name", "Preshow", "Preshow"), ("number", "number", "1", "1"), ("colorName", "color", "blue", "blue"),
        ("displayName", "display_name", "Preshow", "Preshow"), ("defaultName", "default_name", "Audio", "Audio"),
        ("duration", "duration", 182.5, 182.5), ("preWait", "pre_wait", 2, 2.0),
        ("goButtonText", "go_button_text", "GO: Preshow", "GO: Preshow"),
        ("isBroken", "broken", False, False), ("isLoaded", "loaded", True, True), ("isPaused", "paused", 0, False),
        ("isRunning", "running", 1, True), ("actionElapsed", "action_elapsed", 12.25, 12.25),
        ("percentActionElapsed", "percent_action_elapsed", 0.5, 0.5),
        ("preWaitElapsed", "pre_wait_elapsed", 1.5, 1.5),
        ("percentPreWaitElapsed", "percent_pre_wait_elapsed", 0.75, 0.75)]:
    telemetry(GB, f"cue-{key.replace('_', '-')}", inbound_hex=hexs(gb_reply(f"/cue_id/{GB_C1}/{word}", data)),
              expect_state={"cues": {GB_C1: {key: value}}})
# A hit read by its hit ID lands in the same place.
telemetry(GB, "hit-name", inbound_hex=hexs(gb_reply(f"/hit_id/{GB_H1}/name", "Bell")),
          expect_state={"cues": {GB_H1: {"name": "Bell"}}})
# An error reply changes nothing.
telemetry(GB, "error-reply", inbound_hex=hexs(gb_reply(f"/cue_id/{GB_C1}/name", "no such cue", status="error")),
          expect_state={})
