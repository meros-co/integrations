# Pixotope (pixotope): one vector per command, over Pixotope Gateway. Each
# request is a POST to /gateway/24.1.0/publish whose body is the Data Hub
# message {"Topic": ..., "Message": ...}, written here as compact JSON from
# Pixotope's message schema and the Engine and Store call pages (their own
# examples: SpotLight_1.LightComponent0 Intensity 100, post_process_handler
# Settings.ColorGamma, Levels/TestMap0, State.General.CompositingColorSpace).
PX = "pixotope"
_PX_T = "/gateway/24.1.0/publish"


def _px(command, input, topic, message, timeout=None, **extra):
    body = {"Topic": topic, "Message": message}
    if timeout is not None:
        body["Timeout"] = timeout
    V.append({"spec": PX, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": _PX_T, "body": json.dumps(body, separators=(",", ":"))},
              **extra})


def _eng(method, params):
    return {"Type": "Call", "Target": "~LOCAL~-Engine", "Method": method}, {"Params": params}


_px("get_state_value", {"target": "Store", "name": "State.General.CompositingColorSpace"},
    {"Type": "Get", "Target": "Store", "Name": "State.General.CompositingColorSpace"}, {},
    http_reply={"status": 200, "body": '{"Value":"Video"}'}, expect_result={"ok": {"kind": "value", "value": '{"Value":"Video"}'}})
_px("set_state_value", {"target": "Store", "name": "State.General.CompositingColorSpace", "value": "Video"},
    {"Type": "Set", "Target": "Store", "Name": "State.General.CompositingColorSpace"}, {"Value": "Video"},
    http_reply={"status": 200, "body": "OK"}, expect_result={"ok": {"kind": "ack"}})
_px("reset_state", {"target": "RenderMachine1-VideoIO"}, {"Type": "Reset", "Target": "RenderMachine1-VideoIO"}, {})
_px("call", {"target": "TJCGWS040-Engine", "method": "CallFunction",
             "params": {"ObjectSearch": "level_blueprint", "FunctionName": "Test"}, "timeout": 2000},
    {"Type": "Call", "Target": "TJCGWS040-Engine", "Method": "CallFunction"},
    {"Params": {"ObjectSearch": "level_blueprint", "FunctionName": "Test"}}, timeout=2000)
_px("set_property", {"object": "SpotLight_1.LightComponent0", "property": "Intensity", "value": 100},
    *_eng("SetProperty", {"ObjectSearch": "SpotLight_1.LightComponent0", "PropertyPath": "Intensity", "Value": 100, "ApplyClamp": False}))
_px("get_property", {"object": "SpotLight_1.LightComponent0", "property": "Intensity"},
    *_eng("GetProperty", {"ObjectSearch": "SpotLight_1.LightComponent0", "PropertyPath": "Intensity", "RecursionDepth": 1}))
_px("reset_property", {"object": "post_process_handler", "property": "Settings.ColorGamma"},
    *_eng("ResetProperty", {"ObjectSearch": "post_process_handler", "PropertyPath": "Settings.ColorGamma"}))
_px("call_function", {"object": "level_blueprint", "function": "Test", "arguments": ["1"]},
    *_eng("CallFunction", {"ObjectSearch": "level_blueprint", "FunctionName": "Test", "FunctionArguments": ["1"]}))
_px("set_visibility", {"object": "DirectionalLight_1", "visible": False},
    *_eng("SetVisibility", {"ObjectSearch": "DirectionalLight_1", "Visibility": False}))
_px("stop_sequences", {"sequences": ["/Game/Seq/Intro"]}, *_eng("StopSequences", {"SequenceAssetPaths": ["/Game/Seq/Intro"]}))
_px("continue_sequences", {"labels": ["Lower Third"]}, *_eng("ContinueSequences", {"SequenceLabels": ["Lower Third"]}))
_px("get_sequence_assets", {}, *_eng("GetAvailableSequenceAssets", {}))
_px("open_level", {"level": "Levels/TestMap0"}, *_eng("OpenLevel", {"LevelPath": "Levels/TestMap0"}))
_px("set_sublevel_visibility", {"sublevel": "/Game/Maps/Set_A", "visible": True},
    *_eng("SetSublevelVisibility", {"AssetPath": "/Game/Maps/Set_A", "Visibility": True}))
_px("load_sublevel", {"sublevel": "/Game/Maps/Set_A"}, *_eng("LoadSublevel", {"AssetPath": "/Game/Maps/Set_A"}))
_px("unload_sublevel", {"sublevel": "/Game/Maps/Set_A"}, *_eng("UnloadSublevel", {"AssetPath": "/Game/Maps/Set_A"}))
_px("motion_take_in", {"asset": "/Game/MD/LowerThird", "channel": "Channel 1", "controller_ids": ["Name"],
                       "controller_values": ["Jane Doe"]},
    *_eng("AvalancheTakeIn", {"AssetPath": "/Game/MD/LowerThird", "AvalancheChannel": "Channel 1",
                              "ControllerIds": ["Name"], "ControllerValues": ["Jane Doe"]}))
_px("motion_take_out", {"asset": "/Game/MD/LowerThird", "channel": "Channel 1"},
    *_eng("AvalancheTakeOut", {"AssetPath": "/Game/MD/LowerThird", "AvalancheChannel": "Channel 1"}))
_px("motion_get_assets", {}, *_eng("AvalancheGetAvailableAssets", {"IncludeThumbnails": False}))
_px("motion_get_channels", {}, *_eng("AvalancheGetAllChannelDetails", {}))
_px("get_version", {}, *_eng("GetVersion", {}))
_px("execute_console_command", {"command": "stat fps"}, *_eng("ExecuteConsoleCommand", {"Command": "stat fps"}))
_px("is_ready", {}, *_eng("IsReady", {}))
_px("change_show", {"show": "Election"}, {"Type": "Call", "Target": "Store", "Method": "ChangeShow"},
    {"Params": {"ShowName": "Election"}})
_px("get_all_states", {}, {"Type": "Call", "Target": "Store", "Method": "GetAllStates"}, {"Params": {}})
# A float is written with three decimals (1.500), which json.dumps does not do.
V.append({"spec": PX, "command": "play_sequences", "input": {"sequences": ["/Game/Seq/Intro"], "speed": 1.5},
          "expect_request": {"method": "POST", "target": _PX_T, "body":
              '{"Topic":{"Type":"Call","Target":"~LOCAL~-Engine","Method":"PlaySequences"},'
              '"Message":{"Params":{"SequenceAssetPaths":["/Game/Seq/Intro"],"LoopCount":1,"PlaybackSpeed":1.500}}}'}})
# Another machine's Engine.
V.append({"spec": PX, "command": "is_ready", "input": {}, "settings": {"machine": "RenderMachine1"}, "file": "is_ready-machine",
          "expect_request": {"method": "POST", "target": _PX_T, "body":
              '{"Topic":{"Type":"Call","Target":"RenderMachine1-Engine","Method":"IsReady"},"Message":{"Params":{}}}'}})

# The Data Hub (ZeroMQ): broadcast Updates and Logs, as two frames, the topic
# and the message, compact JSON as the "How to control a Pixotope service"
# example sends them. On connecting, the read of State the Store answers.
_PX_GET = ['{"Type":"Get","Target":"Store","RespondTo":"ExternalControl","Name":"State"}', '{}']
_PX_CALLS = [['{"Type":"Call","Target":"~LOCAL~-Engine","RespondTo":"ExternalControl","Method":"%s"}' % m, '{"Params":{}}']
             for m in ("GetAvailableSequenceAssets", "AvalancheGetAvailableAssets", "AvalancheGetAllChannelDetails",
                       "GetVersion", "IsReady")]
telemetry(PX, "update-number", expect_connect_zmq=[_PX_GET] + _PX_CALLS,
          inbound_zmq=['{"Type":"Update","Target":"BROADCAST","Source":"Store","Name":"State.General.FrameRate"}',
                       '{"Value":50}'],
          expect_state={"services": {"Store": {"values": {"State.General.FrameRate": "50"}}}})
telemetry(PX, "update-text",
          inbound_zmq=['{"Type":"Update","Target":"BROADCAST","Source":"Store","Name":"State.General.CompositingColorSpace"}',
                       '{"Value":"Video"}'],
          expect_state={"services": {"Store": {"values": {"State.General.CompositingColorSpace": "Video"}}}})
telemetry(PX, "update-object-to-client",
          inbound_zmq=['{"Type":"Update","Target":"ExternalControl","Source":"RenderMachine1-Engine","Name":"State"}',
                       '{"Value":{"Level":"Levels/TestMap0"}}'],
          expect_state={"services": {"RenderMachine1-Engine": {"values": {"State": '{"Level":"Levels/TestMap0"}'}}}})
telemetry(PX, "log-with-code",
          inbound_zmq=['{"Type":"Log","Target":"BROADCAST","Source":"RenderMachine1-VideoIO"}',
                       '{"Message":"Input signal lost","Code":"NO_SIGNAL"}'],
          expect_state={"services": {"RenderMachine1-VideoIO": {"log": {"message": "Input signal lost", "code": "NO_SIGNAL"}}}})
telemetry(PX, "log-without-code",
          inbound_zmq=['{"Type":"Log","Target":"BROADCAST","Source":"Store"}', '{"Message":"Show saved"}'],
          expect_state={"services": {"Store": {"log": {"message": "Show saved", "code": ""}}}})

# The Engine's answers to the Calls published on connecting.
telemetry(PX, "call-result-sequences",
          inbound_zmq=['{"Type":"CallResult","Target":"ExternalControl","ExecutedOn":"RenderMachine1-Engine","Method":"GetAvailableSequenceAssets"}',
                       '{"Result":{"Sequences":["/Game/Seq/Intro","/Game/Seq/Outro"]}}'],
          expect_state={"services": {"RenderMachine1-Engine": {"calls": {
              "GetAvailableSequenceAssets": '{"Sequences":["/Game/Seq/Intro","/Game/Seq/Outro"]}'}}}})
telemetry(PX, "call-result-failure",
          inbound_zmq=['{"Type":"CallResult","Target":"ExternalControl","ExecutedOn":"RenderMachine1-Engine","Method":"IsReady"}',
                       '{"Failure":"Unknown method name."}'],
          expect_state={})
