# KUMA Timer (kuma-timer): one vector per command and telemetry vectors for
# the status. Bodies are written by hand from the "API Reference" page
# (https://kuma.pl-tech.co.uk/api.html): POST /api/command with
# {"action": ..., params}, answered {"ok": true}; the actions the page does
# not list follow the vendor's Companion module. Every body carries key,
# empty when no web control password is set.
KT = "kuma-timer"
_KOK = {"http_reply": {"status": 200, "body": '{"ok":true}'}, "expect_result": {"ok": {"kind": "ack"}}}


def _kt(command, input, body, **extra):
    V.append({"spec": KT, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": "/api/command", "body": body}, **extra})


_kt("start", {}, '{"action":"start","key":""}', **_KOK)
_kt("pause", {}, '{"action":"pause","key":""}')
_kt("reset", {}, '{"action":"reset","key":""}')
_kt("hide", {}, '{"action":"hide","key":""}')
_kt("count_up", {}, '{"action":"count_up","key":""}')
_kt("set_mode", {"mode": "CLOCK"}, '{"action":"set_mode","mode":"CLOCK","key":""}')
_kt("add_minute", {}, '{"action":"add_minute","key":""}')
_kt("subtract_minute", {}, '{"action":"sub_minute","key":""}')
_kt("add_seconds", {"seconds": -30}, '{"action":"add_seconds","seconds":-30,"key":""}')
_kt("cut_nudge", {"delta_seconds": 60}, '{"action":"cut_nudge","delta_seconds":60,"key":""}')
# "load_time seconds int ... (e.g. 300 = 5:00)"
_kt("load_time", {"seconds": 300}, '{"action":"load_time","seconds":300,"key":""}', **_KOK)
_kt("load_preset", {"index": 0}, '{"action":"preset","index":0,"key":""}')
_kt("glide_to_time", {"hours": 18, "minutes": 0}, '{"action":"warp_time","hours":18,"minutes":0,"seconds":0,"key":""}')
_kt("glide_for_duration", {"minutes": 5}, '{"action":"warp_duration","hours":0,"minutes":5,"seconds":0,"key":""}')
_kt("cancel_glide", {}, '{"action":"cancel_warp","key":""}')
_kt("cancel_jump", {}, '{"action":"cancel_jump","key":""}')
# "Load cue index 2 (third row)"
_kt("load_cue", {"index": 2}, '{"action":"load_cue","index":2,"key":""}')
_kt("next_cue", {}, '{"action":"next_cue","key":""}',
    http_reply={"status": 400, "body": '{"error":"unknown action: next_cue"}'},
    expect_result={"error": {"error": "device_error", "code": "400"}})
_kt("previous_cue", {}, '{"action":"prev_cue","key":""}')
_kt("recall_layout", {"slot": 2}, '{"action":"recall_layout","slot":2,"key":""}')
_kt("recall_layout_by_name", {"name": "Keynote"}, '{"action":"recall_layout","name":"Keynote","key":""}')
# The page's example: a red flashing SMS for 20 seconds.
_kt("send_message", {"text": "PLEASE WRAP UP", "duration": 20, "color": "#ff4444", "border_color": "#ff0000",
                     "size": "large", "position": "top", "flash": True},
    '{"action":"send_sms","text":"PLEASE WRAP UP","duration":20,"color":"#ff4444","border_color":"#ff0000",'
    '"size":"large","position":"top","flash":true,"scroll":false,"scroll_invert":false,"fullscreen":false,"key":""}',
    **_KOK)
_kt("cancel_message", {}, '{"action":"cancel_sms","key":""}')
_kt("set_qlab_hold", {"on": True}, '{"action":"qlab_follow_hold","on":true,"key":""}')
_kt("set_qlab_follow", {"on": False}, '{"action":"qlab_follow_enable","on":false,"key":""}')
_kt("set_qlab_triggers", {"on": True}, '{"action":"qlab_triggers_enable","on":true,"key":""}')
_kt("set_qlab_follow_cue", {"cue": "12"}, '{"action":"qlab_set_follow_cue","cue":"12","key":""}')
_kt("set_omt_output", {"on": True}, '{"action":"omt_enable","on":true,"key":""}')
_kt("set_ltc_generator", {"on": True}, '{"action":"ltc_generator_enable","on":true,"key":""}')
_kt("set_ltc_chase", {"on": False}, '{"action":"ltc_chase_enable","on":false,"key":""}')
V.append({"spec": KT, "command": "set_blackmagic_output", "input": {"on": True}, "settings": {"password": "s3cret"},
          "expect_request": {"method": "POST", "target": "/api/command",
                             "body": '{"action":"blackmagic_enable","on":true,"key":"s3cret"}'},
          "http_reply": {"status": 401, "body": '{"error":"auth_required"}'},
          "expect_result": {"error": {"error": "device_error", "code": "401"}}})

# Telemetry: the page's status example, then one with the module's fields.
telemetry(KT, "status", inbound_http={"path": "/api/status", "body": json.dumps({
    "status": "live", "timer": "04:32", "timer_seconds": 272, "overtime": False, "progress": 74.3,
    "cue_name": "Jan Kowalski", "cue_index": 2, "sms_active": False})},
    expect_state={"timer": {"status": "live", "display": "04:32", "seconds": 272, "overtime": False,
                            "progress": 74.3},
                  "cue": {"name": "Jan Kowalski", "index": 2}, "message": {"active": False}})
telemetry(KT, "status-full",
          state_before={"cues": {"5": {"name": "Gone"}}, "powerpoint": {"file": "old.pptx", "slide": 9}},
          inbound_http={"path": "/api/status", "body": json.dumps({
              "status": "paused", "timer": "-00:12", "timer_seconds": -12, "overtime": True, "progress": 0,
              "cue_name": "", "cue_index": -1, "sms_active": True, "display_mode": "TIMER", "is_countup": False,
              "app_version": "1.19.3", "cues": ["Welcome", "Keynote"], "presets": [5, 10],
              "preset_seconds": [300, 600], "qlab_triggers_enabled": True, "qlab_follow_enabled": False,
              "qlab_following": False, "qlab_follow_cue": "12", "qlab_follow_mode": "cue", "qlab_health": "idle",
              "qlab_hold": False, "omt_enabled": True, "omt_active": False, "ltc_tx_enabled": False,
              "ltc_tx_active": False, "ltc_chase_enabled": True, "ltc_chase_active": False,
              "blackmagic_active": True, "warp_active": False, "jump_active": True,
              "layout_presets": [{"slot": 1, "id": "a1", "name": "Default"}, {"slot": 2, "id": "b2", "name": "Keynote"}],
              "layout_preset_active": "b2",
              "ppt": {"file": "sample-presentation.pptx", "slide": 3, "total": 12, "builds_remaining": 1,
                      "builds_total": 2, "media": "playing", "media_remaining": 42}})},
          expect_state={
              "timer": {"status": "paused", "display": "-00:12", "seconds": -12, "overtime": True, "progress": 0.0,
                        "count_up": False, "glide_active": False, "jump_active": True},
              "display": {"mode": "TIMER"}, "cue": {"name": "", "index": -1}, "message": {"active": True},
              "app": {"version": "1.19.3"},
              "cues": {"0": {"name": "Welcome"}, "1": {"name": "Keynote"}},
              "presets": {"0": {"seconds": 300}, "1": {"seconds": 600}},
              "layouts": {"active": "b2", "presets": {"1": {"id": "a1", "name": "Default"},
                                                      "2": {"id": "b2", "name": "Keynote"}}},
              "qlab": {"triggers": True, "follow": False, "following": False, "follow_cue": "12",
                       "follow_mode": "cue", "health": "idle", "hold": False},
              "outputs": {"omt_enabled": True, "omt_active": False, "blackmagic_active": True},
              "ltc": {"generator_enabled": False, "generator_active": False, "chase_enabled": True,
                      "chase_active": False},
              "powerpoint": {"file": "sample-presentation.pptx", "slide": 3, "total": 12, "builds_remaining": 1,
                             "builds_total": 2, "media": "playing", "media_remaining": 42}})
