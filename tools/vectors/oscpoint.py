# OSCPoint (oscpoint): one vector per command, and one per telemetry rule.
# Addresses and argument types are those of OSCPoint's ACTIONS.md and
# FEEDBACKS.md: slide numbers and milliseconds as int, the media percent as
# float, names (sections, folders, files) as a blob of UTF-8 text (2.1 and
# later). Feedback names and JSON come as an ASCII string and the same text as
# a UTF-8 blob; the presentation and file JSON are PRESENTATION.md's and
# FILES.md's examples. OSCPoint never answers, so every command is unverified.
OP = "oscpoint"
_U = {"expect_result": {"ok": {"kind": "unverified"}}}


def op(command, input, address, *args, **extra):
    binary(OP, command, input, osc(address, *args), **extra)


op("next", {}, "/oscpoint/next", **_U)
op("previous", {}, "/oscpoint/previous")
op("go_to_slide", {"slide": 5}, "/oscpoint/goto/slide", ("i", 5))
op("go_to_first_slide", {}, "/oscpoint/goto/slide/first")
op("go_to_last_slide", {}, "/oscpoint/goto/slide/last")
op("go_to_section", {"section": "Financials"}, "/oscpoint/goto/section", ("b", "Financials"))
op("hide_slide", {"slide": 3}, "/oscpoint/slide/hide", ("i", 3))
op("unhide_slide", {"slide": 3}, "/oscpoint/slide/unhide", ("i", 3))
op("start_slideshow", {}, "/oscpoint/slideshow/start")
op("start_slideshow_at", {"slide": 5}, "/oscpoint/slideshow/start", ("i", 5))
op("start_slideshow_current", {}, "/oscpoint/slideshow/start/current")
op("start_slideshow_section", {"section": "Résumé"}, "/oscpoint/slideshow/start/section", ("b", "Résumé"))
op("end_slideshow", {}, "/oscpoint/slideshow/end")
op("set_black", {"black": True}, "/oscpoint/slideshow/black", ("i", 1))
op("toggle_black", {}, "/oscpoint/slideshow/black")
op("set_white", {"white": False}, "/oscpoint/slideshow/white", ("i", 0))
op("toggle_white", {}, "/oscpoint/slideshow/white")
op("set_laser_pointer", {"enabled": True}, "/oscpoint/slideshow/laserpointer", ("i", 1))
op("toggle_laser_pointer", {}, "/oscpoint/slideshow/laserpointer")
# Without a size, the documented default 1920x1080 is sent.
op("set_wallpaper", {}, "/oscpoint/slideshow/setwallpaper", ("i", 1920), ("i", 1080))
op("pause_slideshow", {}, "/oscpoint/slideshow/pause")
op("resume_slideshow", {}, "/oscpoint/slideshow/resume")
op("media_play", {}, "/oscpoint/media/play")
op("media_pause", {}, "/oscpoint/media/pause")
op("media_play_pause", {}, "/oscpoint/media/playpause")
op("media_stop", {}, "/oscpoint/media/stop")
op("media_go_to_from_start", {"ms": 1000}, "/oscpoint/media/goto/position/fromstart", ("i", 1000))
op("media_go_to_before_end", {"ms": 5000}, "/oscpoint/media/goto/position/beforeend", ("i", 5000))
op("media_forward", {"ms": 2500}, "/oscpoint/media/goto/position/forward", ("i", 2500))
op("media_back", {"ms": 2500}, "/oscpoint/media/goto/position/back", ("i", 2500))
op("media_go_to_percent", {"percent": 50.0}, "/oscpoint/media/goto/position/percent", ("f", 50.0))
op("media_next_bookmark", {}, "/oscpoint/media/goto/bookmark/next")
op("media_previous_bookmark", {}, "/oscpoint/media/goto/bookmark/previous")
op("set_active_folder", {"path": "Desktop\\myfolder\\oscpoint"}, "/oscpoint/files/setpath",
   ("b", "Desktop\\myfolder\\oscpoint"))
op("list_files", {}, "/oscpoint/files/list")
op("open_file", {"filename": "test.pptx"}, "/oscpoint/files/open", ("b", "test.pptx"))
op("activate_presentation", {"filename": "test.pptx"}, "/oscpoint/presentations/activate", ("b", "test.pptx"))
# The unsaved-changes choice picks the address: abort is plain close.
op("close_presentation", {"filename": "test.pptx"}, "/oscpoint/files/close", ("b", "test.pptx"))
op("close_presentation", {"filename": "test.pptx", "mode": "force"}, "/oscpoint/files/close/force",
   ("b", "test.pptx"), file="close_presentation.force")
op("close_presentation", {"filename": "test.pptx", "mode": "save"}, "/oscpoint/files/close/save",
   ("b", "test.pptx"), file="close_presentation.save")
op("close_active_presentation", {}, "/oscpoint/files/close")
op("close_active_presentation", {"mode": "force"}, "/oscpoint/files/close/force",
   file="close_active_presentation.force")
op("close_active_presentation", {"mode": "save"}, "/oscpoint/files/close/save",
   file="close_active_presentation.save")
op("set_actions_enabled", {"enabled": False}, "/oscpoint/actions/disable")
op("set_actions_enabled", {"enabled": True}, "/oscpoint/actions/enable", file="set_actions_enabled.enable")
op("set_feedbacks_enabled", {"enabled": True}, "/oscpoint/feedbacks/enable")
op("set_feedbacks_enabled", {"enabled": False}, "/oscpoint/feedbacks/disable", file="set_feedbacks_enabled.disable")
op("refresh_feedbacks", {}, "/oscpoint/feedbacks/refresh")


def opt(name, address, *args, **fields):
    telemetry(OP, name, inbound_hex=hexs(osc(address, *args)), **fields)


# PRESENTATION.md's example, two of its three sections.
_PRES = ('{"name":"test_presentation.pptx","path":"C:\\\\Users\\\\[USERNAME]\\\\Desktop\\\\oscpoint",'
         '"slideCount":48,"saved":true,"active":true,"slideshow":true,"sections":['
         '{"id":"{0FD0682D-A8FE-4955-AB04-F07552824B5D}","name":"Default Section","firstSlide":1,"lastSlide":5,"slideCount":5},'
         '{"id":"{ED2008A6-89BF-4DF5-B9D9-493C7E049507}","name":"Financials","firstSlide":6,"lastSlide":48,"slideCount":43}]}')
_PRES_STATE = {
    "name": "test_presentation.pptx", "path": "C:\\Users\\[USERNAME]\\Desktop\\oscpoint", "slide_count": 48,
    "saved": True, "active": True, "in_slideshow": True,
    "sections": {
        "1": {"id": "{0FD0682D-A8FE-4955-AB04-F07552824B5D}", "name": "Default Section", "first_slide": 1,
              "last_slide": 5, "slide_count": 5},
        "2": {"id": "{ED2008A6-89BF-4DF5-B9D9-493C7E049507}", "name": "Financials", "first_slide": 6,
              "last_slide": 48, "slide_count": 43},
    },
}
opt("presentation", "/oscpoint/v2/presentation", ("s", _PRES), expect_state={"presentation": _PRES_STATE})
# The same object with a UTF-8 blob beside the ASCII string: the blob's names win,
# and a section list that shrank leaves nothing behind.
_PRES_U8 = ('{"name":"Präsentation.pptx","path":"C:\\\\Users\\\\Zoë","slideCount":3,"saved":false,"active":true,'
            '"slideshow":false,"sections":[{"id":"{A}","name":"Übersicht","firstSlide":1,"lastSlide":3,"slideCount":3}]}')
opt("presentation-utf8", "/oscpoint/v2/presentation",
    ("s", _PRES_U8.replace("ä", "?").replace("ë", "?").replace("Ü", "?")), ("b", _PRES_U8),
    state_before={"presentation": {"sections": {"2": {"name": "Old"}}}},
    expect_state={"presentation": {"name": "Präsentation.pptx", "path": "C:\\Users\\Zoë", "slide_count": 3,
                                   "saved": False, "active": True, "in_slideshow": False,
                                   "sections": {"1": {"id": "{A}", "name": "Übersicht", "first_slide": 1,
                                                      "last_slide": 3, "slide_count": 3}}}})
# The blank object sent while no presentation is open.
opt("presentation-none", "/oscpoint/v2/presentation",
    ("s", '{"name":"","path":"","slideCount":0,"saved":false,"active":false,"slideshow":false,"sections":null}'),
    state_before={"presentation": {"sections": {"1": {"name": "Old"}}}},
    expect_state={"presentation": {"name": "", "path": "", "slide_count": 0, "saved": False, "active": False,
                                   "in_slideshow": False}})
opt("presentations", "/oscpoint/v2/presentations", ("s", "[" + _PRES + ',{"name":"b.pptx","path":"C:\\\\x",'
    '"slideCount":2,"saved":false,"active":false,"slideshow":false,"sections":null}]'),
    state_before={"presentations": {"3": {"name": "closed.pptx"}}},
    expect_state={"presentations": {
        "1": _PRES_STATE,
        "2": {"name": "b.pptx", "path": "C:\\x", "slide_count": 2, "saved": False, "active": False,
              "in_slideshow": False}}})
opt("presentations-utf8", "/oscpoint/v2/presentations",
    ("s", '[{"name":"?.pptx","path":"C:\\\\x","slideCount":1,"saved":true,"active":true,"slideshow":false,'
          '"sections":[{"id":"{B}","name":"?","firstSlide":1,"lastSlide":1,"slideCount":1}]}]'),
    ("b", '[{"name":"日本.pptx","path":"C:\\\\x","slideCount":1,"saved":true,"active":true,"slideshow":false,'
          '"sections":[{"id":"{B}","name":"開始","firstSlide":1,"lastSlide":1,"slideCount":1}]}]'),
    expect_state={"presentations": {"1": {"name": "日本.pptx", "path": "C:\\x", "slide_count": 1, "saved": True,
                                          "active": True, "in_slideshow": False,
                                          "sections": {"1": {"id": "{B}", "name": "開始", "first_slide": 1,
                                                             "last_slide": 1, "slide_count": 1}}}}})
opt("files-enabled", "/oscpoint/v2/files/enabled", ("T", None),
    expect_state={"files": {"access_enabled": True}})
opt("files-enabled-int", "/oscpoint/v2/files/enabled", ("i", 0),
    expect_state={"files": {"access_enabled": False}})
opt("active-folder", "/oscpoint/v2/files/activefolder", ("s", "Desktop\\oscpoint"),
    expect_state={"files": {"active_folder": "Desktop\\oscpoint"}})
opt("active-folder-utf8", "/oscpoint/v2/files/activefolder", ("s", "Desktop\\Pr?sentationen"),
    ("b", "Desktop\\Präsentationen"), expect_state={"files": {"active_folder": "Desktop\\Präsentationen"}})
opt("active-folder-full-path", "/oscpoint/v2/files/activefolder/fullpath", ("s", "C:\\Users\\nick\\Desktop\\oscpoint"),
    expect_state={"files": {"active_folder_full_path": "C:\\Users\\nick\\Desktop\\oscpoint"}})
opt("active-folder-full-path-utf8", "/oscpoint/v2/files/activefolder/fullpath", ("s", "C:\\Users\\Zo?"),
    ("b", "C:\\Users\\Zoë"), expect_state={"files": {"active_folder_full_path": "C:\\Users\\Zoë"}})
# FILES.md's example list.
_FILES = ('[{"name":"test.pptx","fullName":"C:\\\\Users\\\\[USERNAME]\\\\Desktop\\\\oscpoint\\\\test_presentation.pptx",'
          '"extension":".pptx","length":265299837,"lastAccessTime":"23/04/2024 13:05:22",'
          '"lastWriteTime":"23/04/2024 09:45:52","creationTime":"23/04/2024 09:45:51","isReadOnly":false}]')
opt("files", "/oscpoint/v2/files", ("s", _FILES),
    state_before={"files": {"list": {"2": {"name": "gone.pptx"}}}},
    expect_state={"files": {"list": {"1": {
        "name": "test.pptx", "full_name": "C:\\Users\\[USERNAME]\\Desktop\\oscpoint\\test_presentation.pptx",
        "extension": ".pptx", "size": 265299837, "last_access_time": "23/04/2024 13:05:22",
        "last_write_time": "23/04/2024 09:45:52", "creation_time": "23/04/2024 09:45:51", "read_only": False}}}})
opt("files-utf8", "/oscpoint/v2/files",
    ("s", '[{"name":"?.pptx","fullName":"C:\\\\d\\\\?.pptx","extension":".pptx","length":1,'
          '"lastAccessTime":"a","lastWriteTime":"w","creationTime":"c","isReadOnly":true}]'),
    ("b", '[{"name":"ß.pptx","fullName":"C:\\\\d\\\\ß.pptx","extension":".pptx","length":1,'
          '"lastAccessTime":"a","lastWriteTime":"w","creationTime":"c","isReadOnly":true}]'),
    expect_state={"files": {"list": {"1": {"name": "ß.pptx", "full_name": "C:\\d\\ß.pptx", "extension": ".pptx",
                                           "size": 1, "last_access_time": "a", "last_write_time": "w",
                                           "creation_time": "c", "read_only": True}}}})
# Events (EVENTS.md): each replaces the last.
_OLD = {"last_event": {"type": "slideshow_next_slide", "slide": 2, "click_index": 0, "click_count": 1}}
opt("event-add-in-startup", "/oscpoint/v2/event", ("s", "add_in_startup"), ("s", "2.3.0.0"),
    state_before=_OLD,
    expect_state={"add_in": {"version": "2.3.0.0"}, "last_event": {"type": "add_in_startup", "version": "2.3.0.0"}})
opt("event-presentation-open", "/oscpoint/v2/event", ("s", "presentation_open"), ("s", "Caf?.pptx"),
    ("b", "Café.pptx"), state_before=_OLD,
    expect_state={"last_event": {"type": "presentation_open", "filename": "Café.pptx"}})
opt("event-window-activate-ascii", "/oscpoint/v2/event", ("s", "window_activate"), ("s", "test.pptx"),
    expect_state={"last_event": {"type": "window_activate", "filename": "test.pptx"}})
opt("event-slideshow-begin", "/oscpoint/v2/event", ("s", "slideshow_begin"), ("s", "test.pptx"),
    ("b", "test.pptx"), ("i", 5),
    expect_state={"last_event": {"type": "slideshow_begin", "filename": "test.pptx", "slide": 5}})
opt("event-slideshow-end", "/oscpoint/v2/event", ("s", "slideshow_end"), ("s", "Zo?.pptx"), ("b", "Zoë.pptx"),
    expect_state={"last_event": {"type": "slideshow_end", "filename": "Zoë.pptx"}})
opt("event-next-build", "/oscpoint/v2/event", ("s", "slideshow_next_build"), ("i", 4), ("i", 1), ("i", 3),
    expect_state={"last_event": {"type": "slideshow_next_build", "slide": 4, "click_index": 1, "click_count": 3}})
opt("presentation-name", "/oscpoint/presentation/name", ("s", "test.pptx"),
    expect_state={"presentation": {"name": "test.pptx"}})
opt("presentation-name-utf8", "/oscpoint/presentation/name", ("s", "Caf?.pptx"), ("b", "Café.pptx"),
    expect_state={"presentation": {"name": "Café.pptx"}})
opt("slide-count", "/oscpoint/presentation/slides/count", ("i", 48),
    expect_state={"presentation": {"slide_count": 48}})
opt("visible-slide-count", "/oscpoint/presentation/slides/count/visible", ("i", 45),
    expect_state={"presentation": {"visible_slide_count": 45}})
opt("slideshow-state", "/oscpoint/slideshow/state", ("s", "running"),
    expect_state={"slideshow": {"state": "running"}})
opt("current-slide", "/oscpoint/slideshow/currentslide", ("i", 7),
    expect_state={"slideshow": {"current_slide": 7}})
opt("build-position", "/oscpoint/slideshow/builds/position", ("i", 2),
    expect_state={"slideshow": {"build_position": 2}})
opt("build-count", "/oscpoint/slideshow/builds/count", ("i", 4),
    expect_state={"slideshow": {"build_count": 4}})
opt("builds-remaining", "/oscpoint/slideshow/builds/remaining", ("i", 2),
    expect_state={"slideshow": {"builds_remaining": 2}})
opt("slides-remaining", "/oscpoint/slideshow/slidesremaining", ("i", 41),
    expect_state={"slideshow": {"slides_remaining": 41}})
opt("notes-ascii", "/oscpoint/slideshow/notes", ("s", "Welcome\rThank the sponsors"),
    expect_state={"slideshow": {"notes_ascii": "Welcome\rThank the sponsors"}})
opt("notes-utf8", "/oscpoint/slideshow/notes-utf8", ("b", "Willkommen\rDanke schön"),
    expect_state={"slideshow": {"notes": "Willkommen\rDanke schön"}})
# The Companion module's spelling of the same address.
opt("notes-utf-8", "/oscpoint/slideshow/notes-utf-8", ("b", "Merci à tous"),
    expect_state={"slideshow": {"notes": "Merci à tous"}})
opt("section-index", "/oscpoint/slideshow/section/index", ("i", 2),
    expect_state={"slideshow": {"section": {"index": 2}}})
opt("section-name", "/oscpoint/slideshow/section/name", ("s", "Financials"),
    expect_state={"slideshow": {"section": {"name": "Financials"}}})
opt("section-name-utf8", "/oscpoint/slideshow/section/name", ("s", "?bersicht"), ("b", "Übersicht"),
    expect_state={"slideshow": {"section": {"name": "Übersicht"}}})
opt("section-slides-remaining", "/oscpoint/slideshow/section/slidesremaining", ("i", 3),
    expect_state={"slideshow": {"section": {"slides_remaining": 3}}})
opt("media-state", "/oscpoint/slideshow/media/state", ("s", "playing"),
    expect_state={"slideshow": {"media": {"state": "playing"}}})
opt("media-duration", "/oscpoint/slideshow/media/duration", ("i", 90000),
    expect_state={"slideshow": {"media": {"duration_ms": 90000}}})
opt("media-duration-trimmed", "/oscpoint/slideshow/media/durationtrimmed", ("i", 60000),
    expect_state={"slideshow": {"media": {"duration_trimmed_ms": 60000}}})
opt("media-start-point", "/oscpoint/slideshow/media/startpoint", ("i", 10000),
    expect_state={"slideshow": {"media": {"start_point_ms": 10000}}})
opt("media-end-point", "/oscpoint/slideshow/media/endpoint", ("i", 70000),
    expect_state={"slideshow": {"media": {"end_point_ms": 70000}}})
opt("media-position", "/oscpoint/slideshow/media/position", ("i", 12500),
    expect_state={"slideshow": {"media": {"position_ms": 12500}}})
opt("media-remaining", "/oscpoint/slideshow/media/remaining", ("i", 47500),
    expect_state={"slideshow": {"media": {"remaining_ms": 47500}}})
