# Ontime OSC API (ontime-osc): one vector per command, written by hand from
# the "OSC API" page (https://docs.getontime.no/api/protocols/osc/) and
# encoded with this script's own OSC encoder. Integers go as OSC int32,
# text and the true/false words as OSC strings.
OO = "ontime-osc"
binary(OO, "start", {}, osc("/ontime/start"))
binary(OO, "start_index", {"index": 2}, osc("/ontime/start/index", ("i", 2)))
binary(OO, "start_id", {"id": "32d31"}, osc("/ontime/start/id", ("s", "32d31")))
binary(OO, "start_cue", {"cue": "SF1.01"}, osc("/ontime/start/cue", ("s", "SF1.01")))
binary(OO, "start_next", {}, osc("/ontime/start/next"))
binary(OO, "start_previous", {}, osc("/ontime/start/previous"))
binary(OO, "pause", {}, osc("/ontime/pause"))
binary(OO, "load_index", {"index": 0}, osc("/ontime/load/index", ("i", 0)))
binary(OO, "load_id", {"id": "32d31"}, osc("/ontime/load/id", ("s", "32d31")))
binary(OO, "load_cue", {"cue": "A1"}, osc("/ontime/load/cue", ("s", "A1")))
binary(OO, "load_next", {}, osc("/ontime/load/next"))
binary(OO, "load_previous", {}, osc("/ontime/load/previous"))
binary(OO, "reload", {}, osc("/ontime/reload"))
binary(OO, "stop", {}, osc("/ontime/stop"))
binary(OO, "roll", {}, osc("/ontime/roll"))
binary(OO, "add_time", {"ms": 60000}, osc("/ontime/addtime/add", ("i", 60000)))
binary(OO, "remove_time", {"ms": 30000}, osc("/ontime/addtime/remove", ("i", 30000)))
binary(OO, "aux_set_duration", {"ms": 300000}, osc("/ontime/auxtimer/1/duration", ("i", 300000)))
binary(OO, "aux_set_direction", {"aux": 2, "direction": "count-up"},
       osc("/ontime/auxtimer/2/direction", ("s", "count-up")))
binary(OO, "aux_start", {"aux": 1}, osc("/ontime/auxtimer/1/start"))
binary(OO, "aux_pause", {"aux": 3}, osc("/ontime/auxtimer/3/pause"))
binary(OO, "aux_stop", {"aux": 1}, osc("/ontime/auxtimer/1/stop"))
binary(OO, "aux_add_time", {"aux": 1, "ms": -10000}, osc("/ontime/auxtimer/1/addtime", ("i", -10000)))
binary(OO, "set_secondary_message", {"text": "Next: Q&A"}, osc("/ontime/message/secondary", ("s", "Next: Q&A")))
binary(OO, "set_secondary_source", {"source": "off"}, osc("/ontime/message/timer/secondarySource", ("s", "off")))
binary(OO, "set_blackout", {"blackout": True}, osc("/ontime/message/timer/blackout", ("s", "true")))
binary(OO, "set_timer_message_text", {"text": "Wrap up"}, osc("/ontime/message/timer/text", ("s", "Wrap up")))
binary(OO, "set_timer_message_visible", {"visible": False}, osc("/ontime/message/timer/visible", ("s", "false")))
binary(OO, "set_timer_message_blink", {"blink": True}, osc("/ontime/message/timer/blink", ("s", "true")))
binary(OO, "set_event_title", {"id": "32d31", "title": "Albania"}, osc("/ontime/change/32d31/title", ("s", "Albania")))
binary(OO, "set_event_note", {"id": "32d31", "note": "Cam 2"}, osc("/ontime/change/32d31/note", ("s", "Cam 2")))
binary(OO, "set_event_cue", {"id": "32d31", "cue": "SF1.01"}, osc("/ontime/change/32d31/cue", ("s", "SF1.01")))
binary(OO, "set_event_colour", {"id": "32d31", "colour": "red"}, osc("/ontime/change/32d31/colour", ("s", "red")))
binary(OO, "set_event_skip", {"id": "32d31", "skip": False}, osc("/ontime/change/32d31/skip", ("s", "false")))
binary(OO, "set_event_end_action", {"id": "32d31", "end_action": "load-next"},
       osc("/ontime/change/32d31/endAction", ("s", "load-next")))
binary(OO, "set_event_timer_type", {"id": "32d31", "timer_type": "clock"},
       osc("/ontime/change/32d31/timerType", ("s", "clock")))
binary(OO, "set_event_time_warning", {"id": "32d31", "ms": 600000}, osc("/ontime/change/32d31/timeWarning", ("i", 600000)))
binary(OO, "set_event_time_danger", {"id": "32d31", "ms": 300000}, osc("/ontime/change/32d31/timeDanger", ("i", 300000)))
binary(OO, "set_event_duration", {"id": "32d31", "ms": 1200000}, osc("/ontime/change/32d31/duration", ("i", 1200000)))
binary(OO, "set_event_time_start", {"id": "32d31", "ms": 36000000}, osc("/ontime/change/32d31/timeStart", ("i", 36000000)))
binary(OO, "set_event_time_end", {"id": "32d31", "ms": 37200000}, osc("/ontime/change/32d31/timeEnd", ("i", 37200000)))
# "/ontime/change/<event-id>/custom:<field-name> <new-value>"
binary(OO, "set_event_custom_field", {"id": "32d31", "field": "song", "value": "Sekret"},
       osc("/ontime/change/32d31/custom:song", ("s", "Sekret")))
