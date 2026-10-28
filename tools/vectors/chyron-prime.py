# Chyron PRIME Intelligent Interface (chyron-prime): one vector per command.
# Lines are written by hand from Chyron's PRIME 5.1 P Commands User Guide and
# Automation User Guide, most of them the guides' own examples, each ended
# with CR LF. PRIME answers * on success, an eight-digit error code
# otherwise, and *<command>\<result>\\ for a query.
CH = "chyron-prime"
_OK = {"device_reply": "*\r\n", "expect_result": {"ok": {"kind": "ack"}}}


def _ch(command, input, wire, **extra):
    text(CH, command, input, wire + "\r\n", **extra)


_ch("load", {"channel": 1, "scene": "1000"}, "P\\LOAD\\1\\1000\\\\", **_OK)                    # guide example
_ch("load_with_data", {"channel": 1, "scene": "1000", "data": "Text 1\\Sample Text"},
    "P\\LOAD\\1\\1000\\Text 1\\Sample Text\\\\")                                                  # guide example
_ch("load_on_layer", {"channel": 1, "layer": 5, "scene": "1000"}, "P\\LOAD\\1:5\\1000\\\\",
    device_reply="00004190\r\n", expect_result={"error": {"error": "device_error"}})
_ch("load_on_layer_with_data", {"channel": 1, "layer": 5, "scene": "1000", "data": "Text 1\\Sample Text"},
    "P\\LOAD\\1:5\\1000\\Text 1\\Sample Text\\\\")
_ch("play", {"channel": 1, "scene": "*"}, "P\\PLAY\\1\\*\\\\", **_OK)                            # guide example
_ch("play_with_data", {"channel": 1, "scene": "1000", "data": "Text 1\\Sample Text"},
    "P\\PLAY\\1\\1000\\Text 1\\Sample Text\\\\")
_ch("play_on_layer", {"channel": 1, "layer": 5, "scene": "1000"}, "P\\PLAY\\1:5\\1000\\\\")
_ch("play_on_layer_with_data", {"channel": 1, "layer": 5, "scene": "1000", "data": "Text 1\\Sample Text"},
    "P\\PLAY\\1:5\\1000\\Text 1\\Sample Text\\\\")
_ch("play_layer", {"channel": 1, "layer": "<5"}, "P\\PLAY_LAYER\\1:<5\\\\")                     # guide example
_ch("play_actions", {"buffer": "Program", "channel": 1, "scene": "1000", "actions": "Action 1"},
    "P\\PLAY_ACTION:Program\\1:*\\1000\\Action 1\\\\")
_ch("play_all", {"channel": "*"}, "P\\PLAY_ALL\\*:*\\\\")
_ch("play_all_scenes", {"channel": "1", "layer": "<5", "scenes": "1000\\1002\\1003"},
    "P\\PLAY_ALL\\1:<5\\1000\\1002\\1003\\\\")
_ch("transfer", {"channel": 1, "layer": "5", "scene": "1000"}, "P\\TRANSFER\\1:5\\1000\\\\")       # guide example
_ch("transfer_layer", {"channel": 1, "layer": "5"}, "P\\TRANSFER_LAYER\\1:5\\\\")                # guide example
_ch("update", {"buffer": "Program", "channel": 1, "layer": "<5", "scene": "*", "data": "Name1\\Value1\\Name2\\Value2"},
    "P\\UPDATE:Program\\1:<5\\*\\Name1\\Value1\\Name2\\Value2\\\\", **_OK)                      # guide example
_ch("clear", {"buffer": "Preview", "channel": 1, "scene": "*"}, "P\\CLEAR:Preview\\1:*\\*\\\\")
_ch("clear_layers", {"channel": 1, "layer": "5"}, "P\\CLEAR:*\\1:5\\\\")
_ch("clear_all", {"channel": "*"}, "P\\CLEAR_ALL:*\\*:*\\\\")
_ch("clear_all_scenes", {"channel": "1", "scenes": "1000\\1002\\1003"}, "P\\CLEAR_ALL:*\\1:*\\1000\\1002\\1003\\\\")
_ch("execute_object_commands", {"channel": 1, "scene": "1000", "commands": "Data1.Home\\Data1.MoveNext"},
    "P\\COMMAND:*\\1:*\\1000\\Data1.Home\\Data1.MoveNext\\\\")
_ch("set_scene_parameters", {"channel": 1, "scene": "1000", "data": "Parameter 1\\Sample Text"},
    "P\\SCENE_PARAMETER:*\\1:*\\1000\\Parameter 1\\Sample Text\\\\")
_ch("set_project_parameters", {"data": "Parameter 1\\Sample Text\\Parameter 2\\More Text"},
    "P\\PROJECT_PARAMETER\\Parameter 1\\Sample Text\\Parameter 2\\More Text\\\\")                # guide example
_ch("change_project", {"project": "I/Prime/Projects/Samples"}, "P\\CHANGE_PROJECT\\I/Prime/Projects/Samples\\\\")
_ch("set_channel_project", {"channel": 1, "project": "Project A"}, "P\\SET_PROJECT\\1\\Project A\\\\")  # guide example
_ch("reset_channel_project", {"channel": 1}, "P\\SET_PROJECT\\1\\\\")                            # guide example
_ch("set_application_parameters", {"data": "Parameter 1\\Sample Text"},
    "P\\APPLICATION_PARAMETER\\Parameter 1\\Sample Text\\\\")
_ch("run_sequence", {"channel": 1, "scene": "1000"}, "P\\SEQUENCE\\1\\1000\\\\")                 # guide example
_ch("clear_clips", {"player": 1}, "P\\CLEAR_CLIPS\\1\\\\")
_ch("load_clip", {"player": 1, "clip": "Opener"}, "P\\LOAD_CLIP\\1\\Opener\\\\")
_ch("play_clip", {"player": 1, "clip": "Opener"}, "P\\PLAY_CLIP\\1\\Opener\\\\", **_OK)
_ch("pause_clip", {"player": 1, "clip": "Opener"}, "P\\PAUSE_CLIP\\1\\Opener\\\\")
_ch("resume_clip", {"player": 1, "clip": "Opener"}, "P\\RESUME_CLIP\\1\\Opener\\\\")
_ch("stop_clip", {"player": 2, "clip": "Opener"}, "P\\STOP_CLIP\\2\\Opener\\\\")
_ch("get_project_list", {}, "P\\PROJECT_LIST\\\\", device_reply="*P\\PROJECT_LIST\\News\\Sports\\\\\r\n",
    expect_result={"ok": {"kind": "value", "value": "News\\Sports"}})                            # guide example
_ch("get_scene_list", {}, "P\\SCENE_LIST\\\\",
    device_reply="*P\\SCENE_LIST\\LowerThird\\HeadShot\\Scoreboard\\\\\r\n",
    expect_result={"ok": {"kind": "value", "value": "LowerThird\\HeadShot\\Scoreboard"}})
_ch("get_channel_scene_list", {"channel": 2}, "P\\SCENE_LIST\\2\\\\")
_ch("get_action_list", {"scene": "LowerThird"}, "P\\ACTION_LIST\\LowerThird\\\\",
    device_reply="*P\\ACTION_LIST\\LowerThird\\Default\\Effect In\\Effect Out\\Dissolve\\\\\r\n",
    expect_result={"ok": {"kind": "value", "value": "LowerThird\\Default\\Effect In\\Effect Out\\Dissolve"}})
_ch("get_channel_action_list", {"channel": 1, "scene": "LowerThird"}, "P\\ACTION_LIST\\1\\LowerThird\\\\")
_ch("get_clip_list", {}, "P\\CLIP_LIST\\\\")
_ch("list_scene_assets", {}, "P\\LIST_ASSETS\\Scenes\\\\", device_reply="*P\\LIST_ASSETS\\L3.pbx\\L3/123.pbx\\\\\r\n",
    expect_result={"ok": {"kind": "value", "value": "L3.pbx\\L3/123.pbx"}})                      # guide example
_ch("list_message_assets", {}, "P\\LIST_ASSETS\\Messages\\\\")
_ch("record_clip", {"recorder": 1, "command": "Stop"}, "P\\CLIP_RECORD\\1\\Stop\\*\\*\\\\")
_ch("record_clip_frames", {"recorder": 1, "command": "Start", "file": "Clip_Output", "encoding": "QuicktimeRle",
                           "frames": 300},
    "P\\CLIP_RECORD\\1\\Start\\Clip_Output\\QuicktimeRle\\300\\\\")                              # guide example
_ch("scene_exists", {"scene": "2000"}, "P\\SCENE_EXISTS\\2000\\\\")
_ch("scene_exists_in_project", {"project": "Samples", "scene": "2000"}, "P\\SCENE_EXISTS\\Samples\\2000\\\\",
    device_reply="00004190\r\n", expect_result={"error": {"error": "device_error"}})              # guide example
_ch("file_exists", {"path": "C/Prime/Projects/Samples/Scenes/2000.pbx"},
    "P\\FILE_EXISTS\\C/Prime/Projects/Samples/Scenes/2000.pbx\\\\")
_ch("get_scene_state", {"channel": 1, "scene": "1000"}, "P\\SCENE_STATE\\1\\1000\\\\",
    device_reply="*P\\SCENE_STATE\\Loaded\\Playing\\\\\r\n",
    expect_result={"ok": {"kind": "value", "value": "Loaded\\Playing"}})                          # guide example
_ch("validate_assets", {"buffer": "Preview", "channel": 1, "scene": "1000"},
    "P\\VALIDATE_ASSETS:Preview\\1:*\\1000\\\\", device_reply="*\r\n",
    expect_result={"ok": {"kind": "value", "value": ""}})
_ch("capture_output", {"buffer": "preview", "channel": 1, "file": "still.png"}, "P\\CO:1\\1\\still.png\\\\")
_ch("list_scene_controls", {"scene": "1000"}, "B\\AN\\1000\\\\")
_ch("get_object_value", {"scene": "1000", "object": "Text1"}, "B\\AV\\1000\\Text1\\\\")
_ch("query_scene_state", {"scene": "1000"}, "B\\QS\\1000\\\\")
_ch("list_scenes_in_state", {"state": "Playing"}, "B\\QL\\Playing\\\\")
_ch("get_scene_thumbnail", {"scene": "1000"}, "B\\TH\\1000\\\\")
_ch("get_scene_names", {}, "B\\SL\\\\")
_ch("get_scene_assets", {"scene": "1000"}, "B\\SA\\1000\\\\")
_ch("get_scene_xml", {"scene": "1000"}, "B\\SX\\1000\\\\")
_ch("get_message_xml", {"message": "00001112"}, "B\\MX\\00001112\\\\")
_ch("capture_program", {"folder": "C/Captures"}, "B\\CO\\C/Captures\\\\")
_ch("capture_screen", {"folder": "C/Captures"}, "B\\CS\\C/Captures\\\\")
_ch("get_system_info", {}, "B\\SI\\\\")
_ch("name_exists", {"name": "1000"}, "B\\QE\\1000\\\\")
_ch("save_message", {"message": "1001", "base": "1000", "values": "Jane Doe\\Host"}, "W\\1001\\1000\\Jane Doe\\Host\\\\")
_ch("legacy_play_actions", {"scene": "1000", "actions": "Action1\\Action2"}, "P\\AC\\1000\\Action1\\Action2\\\\")  # guide
_ch("legacy_set_scene_parameter", {"scene": "1000", "name": "Speed", "value": "2"}, "P\\SP\\1000\\Speed:2\\\\")
_ch("legacy_set_project_parameter", {"name": "Speed", "value": "2"}, "P\\PP\\Speed:2\\\\")
_ch("legacy_change_project", {"project": "I/Project/Name"}, "M\\I/Project/Name\\\\")
_ch("legacy_update", {"scene": "1000", "id": 2, "value": "ABC"}, "U\\1000\\2\\ABC\\\\")
_ch("legacy_load", {"channel": 1, "scene": "1000", "values": "ABC\\DEF"}, "V\\5\\13\\1\\1\\1000\\_\\ABC\\DEF\\\\")
_ch("legacy_load_v53", {"channel": 1, "scene": "1000"}, "V\\5\\3\\1\\1\\1000\\_\\\\")
_ch("legacy_take", {"channel": 1}, "V\\6\\1\\\\")

# Telemetry: the lists PRIME answers to the polls (the guide's examples).
telemetry(CH, "project-list", inbound="*P\\PROJECT_LIST\\News\\Sports\\\\\r\n",
          state_before={"projects": {"Old": True}},
          expect_state={"projects": {"News": True, "Sports": True}})
telemetry(CH, "scene-list", request="P\\SCENE_LIST\\\\",
          inbound="*P\\SCENE_LIST\\LowerThird\\HeadShot\\Scoreboard\\\\\r\n",
          expect_state={"scenes": {"LowerThird": True, "HeadShot": True, "Scoreboard": True}})
telemetry(CH, "clip-list", inbound="*P\\CLIP_LIST\\Opener\\Closer\\\\\r\n",
          expect_state={"clips": {"Opener": True, "Closer": True}})
telemetry(CH, "empty-clip-list", inbound="*P\\CLIP_LIST\\\\\r\n", state_before={"clips": {"Opener": True}},
          expect_state={})
