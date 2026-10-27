# CueTimer (cuetimer): one vector per command and telemetry vectors for the
# state document, written by hand from CueTimer's "TCP API" page
# (https://github.com/MortenBrekkeStensland/api_public_cuetimer/blob/HEAD/tcp.md):
# ASCII commands ended by $, parameters after #, and the page's own feedback
# example.
CT = "cuetimer"
text(CT, "fire_next", {}, "FireNext$")
text(CT, "cue_next", {}, "CueNext$")
text(CT, "cue_current", {}, "CueCurrent$")
text(CT, "pause", {}, "Pause$")
text(CT, "restart", {}, "Restart$")
text(CT, "toggle_blackout", {}, "Blackout$")
text(CT, "add_minute", {}, "AddMinute$")
text(CT, "subtract_minute", {}, "SubMinute$")
text(CT, "speed_up", {}, "AddSpeed$")
text(CT, "slow_down", {}, "SubSpeed$")
# "AddXMinutes#5$", "SubXMinutes#5$", "SetDuration#00:10:00$"
text(CT, "add_minutes", {"minutes": 5}, "AddXMinutes#5$")
text(CT, "subtract_minutes", {"minutes": 5}, "SubXMinutes#5$")
text(CT, "set_duration", {"minutes": 10}, "SetDuration#00:10:00$")
# "FireTimerWithID#1$", "CueTimerWithID#1$"
text(CT, "fire_timer", {"id": "1"}, "FireTimerWithID#1$")
text(CT, "cue_timer", {"id": "1"}, "CueTimerWithID#1$")
text(CT, "move_next_up", {}, "MoveNextUp$")
text(CT, "move_next_down", {}, "MoveNextDown$")
# "Fullscreen#on$", "Fullscreen#off$", "Fullscreen#toggle$"
text(CT, "set_fullscreen", {"state": "on"}, "Fullscreen#on$")
text(CT, "set_preview", {"state": "off"}, "Preview#off$")
text(CT, "set_presenter", {"state": "toggle"}, "Presenter#toggle$")
text(CT, "set_multitimer_preview", {"state": "on"}, "MultitimerPreview#on$")
text(CT, "set_multitimer_fullscreen", {"state": "off"}, "MultitimerFullscreen#off$")
text(CT, "toggle_ndi", {}, "NDI$")
text(CT, "toggle_multitimer_ndi", {}, "MultitimerNDI$")
text(CT, "toggle_message", {}, "Message$")
text(CT, "toggle_clock", {}, "Clock$")
text(CT, "toggle_single_timer_mode", {}, "STM$")
text(CT, "init_list", {}, "InitList$")
# "select_list#a926123d-593b-4b53-913c-febeacc06f1c$", "select_list#$"
text(CT, "select_list", {"guid": "a926123d-593b-4b53-913c-febeacc06f1c"},
     "select_list#a926123d-593b-4b53-913c-febeacc06f1c$")
text(CT, "select_active_list", {}, "select_list#$")
text(CT, "activate_next_list", {}, "ActivateNextList$")
text(CT, "activate_previous_list", {}, "ActivatePreviousList$")
text(CT, "activate_list", {"guid": "7cff78a1-46cf-4a2f-8820-a04cce550726"},
     "ActivateListByGUID#7cff78a1-46cf-4a2f-8820-a04cce550726$")

# The page's feedback example, as CueTimer sends it (compact here).
_CT_FEEDBACK = {
    "listNumber": "1", "listGUID": "a926123d-593b-4b53-913c-febeacc06f1c",
    "lists": [{"guid": "a926123d-593b-4b53-913c-febeacc06f1c", "title": "List 1", "isActive": False},
              {"guid": "7cff78a1-46cf-4a2f-8820-a04cce550726", "title": "List 2", "isActive": True},
              {"guid": "130b1132-9ea7-4669-97ec-1c3c623e5acd", "title": "List 3", "isActive": False}],
    "h": "0", "m": "31", "s": "8", "fg": "#FFFFFFFF", "bg": "#FF696969", "speed": "100", "name": "",
    "endTime": "22:12:37", "Fullscreen": False, "Preview": False, "Presenter": False,
    "MultitimerPreview": False, "MultitimerFullscreen": False, "NDI": False, "MultitimerNDI": False,
    "Message": False, "STM": False, "Clock": False, "Pause": False, "Blackout": False,
    "nextTimerName": "", "nextTimerDuration": "01:00:00", "scheduleOffset": "-00:05:01",
    "scheduleOffsetStatus": "Ahead",
    "timers": {"1": {"name": "", "duration": "00:10:00", "bg": "#000000"},
               "2": {"name": "", "duration": "01:00:00", "bg": "#FF0000"}}}
telemetry(CT, "feedback", inbound=json.dumps(_CT_FEEDBACK, separators=(",", ":")) + "$",
          state_before={"timers": {"9": {"name": "Gone"}}},
          expect_state={
              "list": {"number": 1, "guid": "a926123d-593b-4b53-913c-febeacc06f1c"},
              "lists": {"1": {"guid": "a926123d-593b-4b53-913c-febeacc06f1c", "title": "List 1", "active": False},
                        "2": {"guid": "7cff78a1-46cf-4a2f-8820-a04cce550726", "title": "List 2", "active": True},
                        "3": {"guid": "130b1132-9ea7-4669-97ec-1c3c623e5acd", "title": "List 3", "active": False}},
              "timer": {"hours": 0, "minutes": 31, "seconds": 8, "foreground": "#FFFFFFFF",
                        "background": "#FF696969", "speed": 100, "name": "", "end_time": "22:12:37"},
              "outputs": {"fullscreen": False, "preview": False, "presenter": False, "multitimer_preview": False,
                          "multitimer_fullscreen": False, "ndi": False, "multitimer_ndi": False},
              "modes": {"message": False, "single_timer": False, "clock": False, "pause": False, "blackout": False},
              "next_timer": {"name": "", "duration": "01:00:00"},
              "schedule": {"offset": "-00:05:01", "status": "Ahead"},
              "timers": {"1": {"name": "", "duration": "00:10:00", "background": "#000000"},
                         "2": {"name": "", "duration": "01:00:00", "background": "#FF0000"}}})
