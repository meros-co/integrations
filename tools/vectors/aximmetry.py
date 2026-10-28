# Aximmetry Composer (aximmetry): one vector per command. Each request is a
# POST to / on port 21463 whose body is the action's XML, written here from
# Aximmetry's "List of Actions" request formats with the documents' own
# example values (Root\Switch Text, Root\VirtualCam_3-Cam\CAMERAS\SELECT
# CAMERA, CAM 2), attributes in the documented order.
AX = "aximmetry"
_AX_OK = {"http_reply": {"status": 200, "body": '<response Code="0" Message="OK" />'}, "expect_result": {"ok": {"kind": "ack"}}}
_PANEL = "Root\\VirtualCam_3-Cam\\CAMERAS\\SELECT CAMERA"


def _ax(command, input, body, **extra):
    V.append({"spec": AX, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": "/", "body": body}, **extra})


_ax("close_all_documents", {}, '<action type="ComposerCloseAllDocumentsAction" />', **_AX_OK)
_ax("open_and_run_document", {"path": "C:\\Shows\\Studio.xcomp", "close_all": True},
    '<action type="ComposerOpenAndRunDocumentAction" Path="C:\\Shows\\Studio.xcomp" CloseAll="true" />',
    http_reply={"status": 200, "body": '<response Code="0" Message="OK">\n <ReturnValue title="Studio" filePath="C:\\Shows\\Studio.xcomp" type="Compound" />\n</response>'},
    expect_result={"ok": {"kind": "value", "value": '<response Code="0" Message="OK">\n <ReturnValue title="Studio" filePath="C:\\Shows\\Studio.xcomp" type="Compound" />\n</response>'}})
_ax("begin_composite", {"document": "@", "text": "Move the cameras"},
    '<action type="ComposerBeginCompositeAction" Document="@" DisplayText="Move the cameras" />')
_ax("finish_composite", {"document": "@"}, '<action type="ComposerFinishCompositeAction" Document="@" />')
_ax("finish_all_composites", {}, '<action type="ComposerFinishAllCompositesAction" />')
_ax("find_modules", {"module": "Root", "pattern": "Text*", "recursive": False},
    '<action type="ComposerFindModulesAction" Document="" Module="Root" NamePattern="Text*" Recursive="false" />')
_ax("list_pins", {"module": "Root\\Switch Text"}, '<action type="ComposerEnumPinsAction" Document="" Module="Root\\Switch Text" />')
# The documents' first example: set the Selected pin of Root\Switch Text to 7.
_ax("set_pin_value", {"module": "Root\\Switch Text", "pin": "Selected", "value": "7"},
    '<action type="ComposerSetPinValueAction" Document="" Module="Root\\Switch Text" Pin="Selected" Value="7" Undoable="false" SuppressDocChange="false" />',
    **_AX_OK)
_ax("set_pin_value", {"module": "Root", "pin": "TextNo", "value": "x"},
    '<action type="ComposerSetPinValueAction" Document="" Module="Root" Pin="TextNo" Value="x" Undoable="false" SuppressDocChange="false" />',
    http_reply={"status": 200, "body": '<response Code="3" Message="Invalid value" />'},
    expect_result={"error": {"error": "device_error"}}, file="set_pin_value-error")
_ax("get_pin_value", {"module": "Root", "pin": "TextNo"},
    '<action type="ComposerGetPinValueAction" Document="" Module="Root" Pin="TextNo" />',
    http_reply={"status": 200, "body": '<response Code="0" Message="OK">\n <ReturnValue type="Integer" value="7" />\n</response>'},
    expect_result={"ok": {"kind": "value", "value": "7"}})
_ax("get_pin_live_value", {"module": "Root\\Timer", "pin": "Elapsed", "output": True},
    '<action type="ComposerGetPinLiveValueAction" Document="" Module="Root\\Timer" Pin="Elapsed" Output="true" />',
    http_reply={"status": 200, "body": '<response Code="0" Message="OK">\n <ReturnValue>12.5</ReturnValue>\n</response>'},
    expect_result={"ok": {"kind": "value", "value": "12.5"}})
_ax("list_control_boards", {"module": "Root"}, '<action type="ComposerEnumCtrBoardsAction" Document="" Module="Root" Recursive="true" />')
_ax("list_panels", {"board": "Root\\VirtualCam_3-Cam\\CAMERAS"},
    '<action type="ComposerEnumCtrBoardPanelsAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS" />')
_ax("press_button", {"panel": _PANEL, "button": "CAM 2"},
    '<action type="ComposerCtrBoardPressButtonAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\SELECT CAMERA" Button="CAM 2" Undoable="false" />',
    **_AX_OK)
_ax("set_button", {"panel": "Root\\VirtualCam_3-Cam\\CAMERAS\\STREAMING", "button": "On", "state": True},
    '<action type="ComposerCtrBoardSetButtonAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\STREAMING" Button="On" State="true" Undoable="false" />')
_ax("get_button", {"panel": "Root\\VirtualCam_3-Cam\\CAMERAS\\STREAMING", "button": "On"},
    '<action type="ComposerCtrBoardGetButtonAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\STREAMING" Button="On" />',
    http_reply={"status": 200, "body": '<response Code="0" Message="OK">\n <ReturnValue>true</ReturnValue>\n</response>'},
    expect_result={"ok": {"kind": "value", "value": "true"}})
_ax("list_buttons", {"panel": _PANEL}, '<action type="ComposerEnumCtrBoardButtonsAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\SELECT CAMERA" />')
_ax("list_panel_properties", {"panel": _PANEL},
    '<action type="ComposerEnumCtrBoardPropertiesAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\SELECT CAMERA" />')
_ax("set_panel_property", {"panel": _PANEL, "property": "Zoom", "value": "1.5", "undoable": True},
    '<action type="ComposerSetCtrBoardPropertyValueAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\SELECT CAMERA" Property="Zoom" Value="1.5" Undoable="true" />')
_ax("get_panel_property", {"panel": _PANEL, "property": "Zoom"},
    '<action type="ComposerGetCtrBoardPropertyValueAction" Document="" Module="Root\\VirtualCam_3-Cam\\CAMERAS\\SELECT CAMERA" Property="Zoom" />')
_ax("set_sequence_part_property", {"module": "Root\\Sequencer", "part": 1, "property": "Duration", "value": "2.5"},
    '<action type="ComposerSetSequencePartPropertyAction" Document="" Module="Root\\Sequencer" Part="1" Property="Duration" Value="2.5" />')
_ax("get_sequence_part_property", {"module": "Root\\Sequencer", "part": 1, "property": "Duration"},
    '<action type="ComposerGetSequencePartPropertyAction" Document="" Module="Root\\Sequencer" Part="1" Property="Duration" />')
_ax("group_actions", {"actions": '<action type="ComposerNopAction" /><action type="ComposerNopAction" />'},
    '<action type="ComposerGroupAction"><action type="ComposerNopAction" /><action type="ComposerNopAction" /></action>')
_ax("nop", {}, '<action type="ComposerNopAction" />', **_AX_OK)
_ax("active_sensing", {}, '<action type="ComposerActiveSensingAction" />',
    http_reply={"status": 200, "body": '<response Code="1" Message="Engine stopped" />'}, expect_result={"error": {"error": "device_error"}})
_ax("restart_reopening_document", {}, '<action type="ComposerRestartApplicationAction" ReopenCurrentlyRunningDocument="true" />')
_ax("restart_opening_document", {"path": "C:\\Shows\\Studio.xcomp"},
    '<action type="ComposerRestartApplicationAction" ReopenCurrentlyRunningDocument="false" OpenDocument="C:\\Shows\\Studio.xcomp" />')
