# Barco Event Master (barco-eventmaster): one vector per command, over the
# JSON-RPC API. Every request is an HTTP POST to / on port 9999 whose body is
# a JSON-RPC 2.0 request with "id":"1234", as in every example in Barco's
# documents. Bodies are written here from the documents' own examples
# (R5919184/00 section 2.2; "JSON RPC with Event Master processors" Doc 8.2;
# "JSON RPC for Encore3 Processors v10.0") with json.dumps, not from the spec's
# templates; serial numbers, which the spec sends with two decimals, are
# written out by hand. Replies use the envelope of the documents' listContent
# and getFrameSettings examples: {"jsonrpc":"2.0","id":"1234","result":
# {"success":0,"response":...}}.
BM = "barco-eventmaster"


def _rpc(method, params):
    return json.dumps({"params": params, "method": method, "id": "1234", "jsonrpc": "2.0"},
                      separators=(",", ":"))


def _raw(method, params_text):
    """For params holding a two-decimal serial number, written by hand."""
    return '{"params":' + params_text + ',"method":"' + method + '","id":"1234","jsonrpc":"2.0"}'


def _bm(command, input, body, **extra):
    V.append({"spec": BM, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": "/", "body": body}, **extra})


def _ok(response=None):
    return {"status": 200, "body": json.dumps({"jsonrpc": "2.0", "id": "1234",
                                               "result": {"success": 0, "response": response}})}


def _fail(success=1):
    return {"status": 200, "body": json.dumps({"jsonrpc": "2.0", "id": "1234",
                                               "result": {"success": success, "response": None}})}


ACK = {"ok": {"kind": "ack"}}
DEVICE_ERROR = {"error": {"error": "device_error"}}
PW = {"super_operator_password": "123"}

# ── System ───────────────────────────────────────────────────────────────
FRAME = {"System": {"id": 0, "Name": "System1", "FrameCollection": {"id": 0, "Frame": {
    "id": "00:0c:29:0e:86:d4", "Name": "E2", "Contact": "", "Version": "4.2.30738", "OSVersion": "NA",
    "FrameType": 0, "FrameTypeName": "E2"}}}}
_bm("get_frame_settings", {}, _rpc("getFrameSettings", {}), http_reply=_ok(FRAME),
    expect_result={"ok": {"kind": "value", "value": FRAME}})
_bm("get_power_status", {}, _rpc("powerStatus", {}), http_reply=_fail(2), expect_result=DEVICE_ERROR)
_bm("reset_frame", {"reset": 0}, _rpc("resetFrameSettings", {"reset": 0}), http_reply=_ok(), expect_result=ACK)
_bm("reset_frame_encore3", {"reset": 0}, _rpc("resetFrameSettings", {"reset": 0}))
# Encore3 v10.0 example: Factory and Keep, System Native Rate.
_bm("factory_reset_keep", {"keep": 4}, _rpc("resetFrameSettings", {"reset": 2, "saveOptions": 4}))

# ── Transitions ──────────────────────────────────────────────────────────
_bm("all_trans", {}, _rpc("allTrans", {}), http_reply=_ok(), expect_result=ACK)
# A JSON-RPC error member with a null result fails the command too.
_bm("all_trans_time", {"frames": 40}, _rpc("allTrans", {"transTime": 40}),
    http_reply={"status": 200, "body": '{"jsonrpc":"2.0","id":"1234","result":null,"error":1}'},
    expect_result=DEVICE_ERROR)
_bm("all_trans_as_operator", {"operator": 1}, _rpc("allTrans", {"operatorId": 1}))
_bm("all_trans_as_super_operator", {}, _rpc("allTrans", {"password": "123"}), settings=PW)
_bm("cut", {}, _rpc("cut", {}), http_reply=_ok(), expect_result=ACK)
_bm("cut_as_operator", {"operator": 1}, _rpc("cut", {"operatorId": 1}), http_reply=_fail(), expect_result=DEVICE_ERROR)
_bm("cut_as_super_operator", {}, _rpc("cut", {"password": "123"}), settings=PW)

# ── Presets ──────────────────────────────────────────────────────────────
PRESETS = [{"id": 0, "Name": "Preset3.00", "LockMode": 0, "presetSno": 3.0},
           {"id": 1, "Name": "Preset4.00", "LockMode": 0, "presetSno": 4.0}]
_bm("list_presets", {"screen": 0}, _rpc("listPresets", {"ScreenDest": 0, "AuxDest": -1}), http_reply=_ok(PRESETS),
    expect_result={"ok": {"kind": "value", "value": PRESETS}})
_bm("list_destinations_for_preset", {"preset": 0}, _rpc("listDestinationsForPreset", {"id": 0}))
_bm("save_preset", {"name": "NewPreset"}, _rpc("savePreset", {"presetName": "NewPreset"}),
    http_reply=_ok(), expect_result=ACK)
_bm("save_preset_numbered", {"name": "NewSubPreset", "serial": 1.01, "from_program": True},
    _raw("savePreset", '{"presetName":"NewSubPreset","serialNo":1.01,"saveFromProgram":1}'))
_bm("save_preset_as_operator", {"name": "NewPreset", "operator": 2},
    _rpc("savePreset", {"presetName": "NewPreset", "operatorId": 2}))
_bm("save_preset_as_super_operator", {"name": "NewPreset"},
    _rpc("savePreset", {"presetName": "NewPreset", "password": "p@$$w0rD"}), settings={"super_operator_password": "p@$$w0rD"})
_bm("save_preset_custom",
    {"params": {"presetName": "NewPreset-S1-A1", "ScreenDestination": {"id": 0}, "AuxDestination": {"id": 0}}},
    _rpc("savePreset", {"presetName": "NewPreset-S1-A1", "ScreenDestination": {"id": 0}, "AuxDestination": {"id": 0}}))
_bm("rename_preset", {"preset": 0, "new_name": "NewPresetName"},
    _rpc("renamePreset", {"id": 0, "newPresetName": "NewPresetName"}))
_bm("rename_preset_by_serial", {"serial": 1.0, "new_name": "NewPresetName"},
    _raw("renamePreset", '{"presetSno":1.00,"newPresetName":"NewPresetName"}'))
_bm("rename_preset_by_name", {"name": "NewPreset", "new_name": "NewPresetName"},
    _rpc("renamePreset", {"presetName": "NewPreset", "newPresetName": "NewPresetName"}))
_bm("delete_preset", {"preset": 1}, _rpc("deletePreset", {"id": 1}))
_bm("delete_preset_by_serial", {"serial": 1.0}, _raw("deletePreset", '{"presetSno":1.00}'))
_bm("delete_preset_by_name", {"name": "Preset 5.00"}, _rpc("deletePreset", {"presetName": "Preset 5.00"}))
_bm("delete_preset_as_operator", {"preset": 5, "operator": 2}, _rpc("deletePreset", {"id": 5, "operatorId": 2}))
_bm("delete_preset_as_super_operator", {"preset": 6}, _rpc("deletePreset", {"id": 6, "password": "123"}), settings=PW)
# "Recall in preview with id 0."
_bm("activate_preset", {"preset": 0}, _rpc("activatePreset", {"id": 0, "type": 0}),
    http_reply=_ok(), expect_result=ACK)
# "Recall preset serial number 1.30 to program." (Encore3 v10.0)
_bm("activate_preset_by_serial", {"serial": 1.3, "program": True}, _raw("activatePreset", '{"presetSno":1.30,"type":1}'))
_bm("activate_preset_by_name", {"name": "abc"}, _rpc("activatePreset", {"presetName": "abc"}),
    http_reply=_fail(), expect_result=DEVICE_ERROR)
_bm("activate_preset_as_operator", {"preset": 5, "operator": 2}, _rpc("activatePreset", {"id": 5, "operatorId": 2}))
_bm("activate_preset_as_super_operator", {"preset": 6}, _rpc("activatePreset", {"id": 6, "password": "123"}), settings=PW)
# No preset recalled yet: an error.
_bm("recall_next_preset", {}, _rpc("recallNextPreset", {}), http_reply=_fail(), expect_result=DEVICE_ERROR)

# ── Destinations and content ─────────────────────────────────────────────
_bm("list_destinations", {}, _rpc("listDestinations", {"type": 0}))
CONTENT = {"id": 0, "Name": "ScreenDest1", "IsActive": 1}
_bm("list_content", {"screen": 0}, _rpc("listContent", {"id": 0}), http_reply=_ok(CONTENT),
    expect_result={"ok": {"kind": "value", "value": CONTENT}})
# "Lists Screen 2's BG sources and contents of the 3rd Output's layers."
_bm("list_output_content", {"screen": 1, "output": 2}, _rpc("listContent", {"id": 1, "DestOutMap": 2}))
CHANGE = {"id": 0, "BGLyr": [{"id": 0, "LastBGSourceIndex": -1, "BGShowMatte": 1,
                              "BGColor": [{"id": 0, "Red": 0, "Green": 0, "Blue": 0}]},
                             {"id": 1, "LastBGSourceIndex": -1, "BGShowMatte": 1,
                              "BGColor": [{"id": 0, "Red": 0, "Green": 1023, "Blue": 0}]}]}
_bm("change_content", {"params": CHANGE}, _rpc("changeContent", CHANGE))
_bm("change_layer_source",
    {"screen": 1, "layer": 4, "source": 7, "h_pos": 1540, "v_pos": 680, "h_size": 1600, "v_size": 900},
    _rpc("changeContent", {"id": 1, "Layers": [{"id": 4, "LastSrcIdx": 7, "Window": {
        "HPos": 1540, "VPos": 680, "HSize": 1600, "VSize": 900}, "PvwMode": 1, "PgmMode": 0}]}),
    http_reply=_ok(), expect_result=ACK)
# "Places Source ID 7 into Layer 3.2A on Destination 2's PVW at the given window size and position."
_bm("change_output_layer_source",
    {"screen": 1, "output": 1, "layer": 4, "source": 7, "h_pos": 1540, "v_pos": 680, "h_size": 1600, "v_size": 900},
    _rpc("changeContent", {"id": 1, "DestOutMap": 1, "Layers": [{"id": 4, "LastSrcIdx": 7, "Window": {
        "HPos": 1540, "VPos": 680, "HSize": 1600, "VSize": 900}, "PvwMode": 1, "PgmMode": 0}]}))
_bm("set_layer_mask", {"screen": 1, "layer": 4, "left": 12.5, "right": 12.5, "top": 0.0, "bottom": 0.0},
    _raw("changeContent", '{"id":1,"Layers":[{"id":4,"Mask":{"Left":12.50,"Right":12.50,"Top":0.00,"Bottom":0.00},'
                          '"PvwMode":1,"PgmMode":0}]}'))
_bm("set_screen_test_pattern", {"screen": 0, "pattern": 5}, _rpc("changeContent", {"id": 0, "TestPattern": 5}))
# "Turns on the 32x32 Grid Test Pattern for Output 3 of Screen Destination 2."
_bm("set_output_test_pattern", {"screen": 1, "output": 2, "pattern": 5},
    _rpc("changeContent", {"id": 1, "DestOutMap": 2, "TestPattern": 5}))
AUX = {"id": 0, "Name": "AuxDest1", "PvwLastSrcIndex": 0, "PgmLastSrcIndex": 0}
_bm("list_aux_content", {"aux": 0}, _rpc("listAuxContent", {"id": 0}), http_reply=_ok(AUX),
    expect_result={"ok": {"kind": "value", "value": AUX}})
_bm("change_aux_content", {"aux": 0, "preview_source": 6, "program_source": 1},
    _rpc("changeAuxContent", {"id": 0, "PvwLastSrcIndex": 6, "PgmLastSrcIndex": 1}), http_reply=_ok(), expect_result=ACK)
# "Places Source ID 6 into preview and Source ID 1 on program for Aux Destination 1."
_bm("change_aux_content_named", {"aux": 0, "name": "AuxDest1", "preview_source": 6, "program_source": 1},
    _rpc("changeAuxContent", {"id": 0, "Name": "AuxDest1", "PvwLastSrcIndex": 6, "PgmLastSrcIndex": 1}))
# "Turns on the 100% Color Bars Test Pattern in Aux Destination 1."
_bm("set_aux_test_pattern", {"aux": 0, "pattern": 3}, _rpc("changeAuxContent", {"id": 0, "TestPattern": 3}))
_bm("freeze_input", {"source": 0}, _rpc("freezeDestSource", {"type": 0, "id": 0, "screengroup": 0, "mode": 1}),
    http_reply=_ok(), expect_result=ACK)
_bm("freeze_background", {"source": 2, "frozen": False},
    _rpc("freezeDestSource", {"type": 1, "id": 2, "screengroup": 0, "mode": 0}))
_bm("freeze_screen", {"screen": 1}, _rpc("freezeDestSource", {"type": 2, "id": 1, "screengroup": 0, "mode": 1}))
_bm("freeze_aux", {"aux": 3, "frozen": False}, _rpc("freezeDestSource", {"type": 3, "id": 3, "screengroup": 0, "mode": 0}))
_bm("arm_destinations", {"armed": True, "screens": [{"id": 0}, {"id": 2}], "auxes": [{"id": 0}, {"id": 1}]},
    _rpc("armUnarmDestination", {"arm": 1, "ScreenDestination": [{"id": 0}, {"id": 2}],
                                 "AuxDestination": [{"id": 0}, {"id": 1}]}))
_bm("activate_dest_group", {"group": 0}, _rpc("activateDestGroup", {"id": 0}))
_bm("activate_dest_group_by_serial", {"serial": 1.0}, _raw("activateDestGroup", '{"destGrpSno":1.00}'))
_bm("activate_dest_group_by_name", {"name": "abc"}, _rpc("activateDestGroup", {"destGrpName": "abc"}))

# ── Layers ───────────────────────────────────────────────────────────────
_bm("fill_hv", {"screen": 0, "layer": 1}, _rpc("fillHV", {"screenId": 0, "Layers": [{"id": 1}]}))
# "Output Layer 2.2A in Screen 1 will fill the screen's entire width and height."
_bm("fill_hv_output", {"screen": 0, "output": 1, "layer": 2},
    _rpc("fillHV", {"screenId": 0, "DestOutMap": 1, "Layers": [{"id": 2}]}))
_bm("fill_hv_custom", {"params": {"screenId": 0, "Layers": [{"id": 0}, {"id": 1}]}},
    _rpc("fillHV", {"screenId": 0, "Layers": [{"id": 0}, {"id": 1}]}))
_bm("clear_layer", {"screen": 0, "layer": 0}, _rpc("clearLayers", {"screenId": 0, "Layers": [{"id": 0}]}))
_bm("clear_output_layer", {"screen": 1, "output": 2, "layer": 6},
    _rpc("clearLayers", {"screenId": 1, "DestOutMap": 2, "Layers": [{"id": 6}]}))
# "Clears Output Layers 4.3A & 4.3B from Screen 2."
_bm("clear_layers_custom", {"params": {"screenId": 1, "DestOutMap": 2, "Layers": [{"id": 6}, {"id": 7}]}},
    _rpc("clearLayers", {"screenId": 1, "DestOutMap": 2, "Layers": [{"id": 6}, {"id": 7}]}))

# ── Sources, inputs and outputs ──────────────────────────────────────────
SOURCES = [{"id": 0, "Name": "InSource1", "HSize": 3840, "VSize": 1080, "SrcType": 0, "InputCfgIndex": -1,
            "StillIndex": 0, "DestIndex": -1, "UserKeyIndex": -1, "Mode3D": 0, "Freeze": 1, "Capacity": 2,
            "InputCfgVideoStatus": 4}]
_bm("list_sources", {}, _rpc("listSources", {}), http_reply=_ok(SOURCES),
    expect_result={"ok": {"kind": "value", "value": SOURCES}})
_bm("list_sources_by_type", {"type": 1}, _rpc("listSources", {"type": 1}))
_bm("list_inputs", {}, _rpc("listInputs", {}))
_bm("get_input", {"input": 1}, _rpc("listInputs", {"inputId": 1}))
_bm("list_outputs", {}, _rpc("listOutputs", {}))
_bm("get_output", {"output": 1}, _rpc("listOutputs", {"outputCfgId": 1}))
_bm("list_source_main_backup", {}, _rpc("listSourceMainBackup", {}))
_bm("list_source_main_backup_by_kind", {"kind": 1}, _rpc("listSourceMainBackup", {"inputType": 1}))
_bm("list_source_main_backup_for_input", {"input": 8}, _rpc("listSourceMainBackup", {"inputType": 8}))
BACKUP = {"inputId": 8, "Backup1": {"SrcType": 1, "SourceId": 1}, "Backup2": {"SrcType": 0, "SourceId": 0},
          "Backup3": {"SrcType": 1, "SourceId": 0}, "BackUpState": 1}
_bm("activate_source_main_backup", {"params": BACKUP}, _rpc("activateSourceMainBackup", BACKUP))
_bm("reset_source_main_backup", {"input": 22}, _rpc("resetSourceMainBackup", {"id": 22}))
_bm("control_3d_input", {"input": 1, "sequential": False},
    _rpc("3dControl", {"id": 1, "type": 0, "syncSource": 1, "syncInvert": 0}))
_bm("control_3d_input_reset", {"input": 1}, _rpc("3dControl", {"id": 1}))
_bm("control_3d_output", {"output": 0, "sequential": True}, _rpc("3dControlOutput", {"outputId": 0, "3Dtype": 1}))
_bm("control_3d_output_reset", {"output": 0}, _rpc("3dControlOutput", {"outputId": 0}))

# ── Stills ───────────────────────────────────────────────────────────────
_bm("list_stills", {}, _rpc("listStill", {}))
# "This creates a still from input source id 1 as StillStore6."
_bm("take_still", {"source": 1, "file": 5}, _rpc("takeStill", {"type": 0, "id": 1, "file": 5}),
    http_reply=_ok(), expect_result=ACK)
_bm("take_still_from_background", {"source": 1, "file": 5}, _rpc("takeStill", {"type": 1, "id": 1, "file": 5}))
_bm("delete_still", {"still": 0}, _rpc("deleteStill", {"id": 0}))

# ── Cues ─────────────────────────────────────────────────────────────────
_bm("list_cues", {}, _rpc("listCues", {}))
_bm("activate_cue", {"cue": 1}, _rpc("activateCue", {"id": 1, "type": 0}), http_reply=_ok(), expect_result=ACK)
_bm("activate_cue_by_name", {"name": "Cue1", "action": 1}, _rpc("activateCue", {"cueName": "Cue1", "type": 1}))
_bm("activate_cue_by_serial", {"serial": 1.0, "action": 2}, _raw("activateCue", '{"cueSerialNo":1.00,"type":2}'))
# "Pause – type 1"
_bm("cue_transport", {"action": 1}, _rpc("activateCue", {"type": 1}))

# ── User keys ────────────────────────────────────────────────────────────
_bm("list_user_keys", {}, _rpc("listUserKeys", {}))
_bm("recall_user_key", {"key": 0, "screen": 1, "layer": 2},
    _rpc("recallUserKey", {"id": 0, "ScreenDestination": [1], "Layer": [2]}))
_bm("recall_user_key_by_name", {"name": "abc", "screen": 0, "layer": 0},
    _rpc("recallUserKey", {"userkeyName": "abc", "ScreenDestination": [0], "Layer": [0]}))
# "Applies UserKey ID 0 to Layers 1, 3 & 5 on Screens 1, 2 & 3."
_bm("recall_user_key_custom", {"params": {"id": 0, "ScreenDestination": [0, 1, 2], "Layer": [0, 2, 4]}},
    _rpc("recallUserKey", {"id": 0, "ScreenDestination": [0, 1, 2], "Layer": [0, 2, 4]}))

# ── Super destinations ───────────────────────────────────────────────────
_bm("list_super_dest_content", {"super_screen": 0}, _rpc("listSuperDestContent", {"id": 0}))
_bm("list_super_aux_content", {"super_aux": 0}, _rpc("listSuperAuxContent", {"id": 0}))
_bm("set_super_layer_window", {"super_screen": 0, "layer": 0, "h_pos": 0, "v_pos": 0, "h_size": 700, "v_size": 300},
    _rpc("changeSuperDestContent", {"id": 0, "GlobalLayers": [{"id": 0, "Window": {
        "HPos": 0, "VPos": 0, "HSize": 700, "VSize": 300}}]}))
SUPER = {"id": 0, "GlobalLayers": [{"id": 0, "Window": {"HPos": 0, "VPos": 0, "HSize": 700, "VSize": 300}}]}
_bm("change_super_dest_content", {"params": SUPER}, _rpc("changeSuperDestContent", SUPER))
SUPER_AUX = {"id": 0, "Destinations": [{"id": 0, "Name": "AuxDest1", "PvwLastSrcIndex": 0, "PgmLastSrcIndex": 0}]}
_bm("change_super_aux_content", {"params": SUPER_AUX}, _rpc("changeSuperAuxContent", SUPER_AUX))

# ── Operators ────────────────────────────────────────────────────────────
_bm("list_operators", {}, _rpc("listOperators", {}))
_bm("enable_operator", {"operator": 2}, _rpc("configureOperator", {"operatorId": 2, "enable": 1}))
_bm("rename_operator", {"operator": 2, "name": "operator3"}, _rpc("configureOperator", {"operatorId": 2, "name": "operator3"}))
_bm("set_operator_preset_range", {"operator": 2, "start": 89, "end": 95},
    _rpc("configureOperator", {"operatorId": 2, "startRange": 89, "endRange": 95}))
OPERATOR = {"operatorId": 1, "add": {"destType": 1, "destIndex": 0}, "remove": {"destType": 1, "destIndex": 1}}
_bm("configure_operator", {"params": OPERATOR}, _rpc("configureOperator", OPERATOR))

# ── Multiviewer ──────────────────────────────────────────────────────────
_bm("change_mvr_layout", {"unit": 0, "layout": 1}, _rpc("mvrLayoutChange", {"frameUnitId": 0, "mvrLayoutId": 1}))
# "On unit ID 0, change the second MVR's layout to number 4"
_bm("change_mvr_layout_encore3", {"unit": 0, "mvr": 1, "layout": 3},
    _rpc("mvrLayoutChange", {"frameUnitId": 0, "mvrId": 1, "mvrLayoutId": 3}))
_bm("list_mvr_presets", {}, _rpc("listMvrPreset", {"id": -1}))
_bm("activate_mvr_preset", {"preset": 1}, _rpc("activateMvrPreset", {"id": 1}))

# ── Notifications ────────────────────────────────────────────────────────
_bm("subscribe", {"hostname": "192.168.247.131", "port": 3000, "notifications": ["ScreenDestChanged", "AUXDestChanged"]},
    _rpc("subscribe", {"hostname": "192.168.247.131", "port": "3000",
                       "notification": ["ScreenDestChanged", "AUXDestChanged"]}),
    http_reply=_ok({"method": "subscribe"}), expect_result={"ok": {"kind": "value", "value": {"method": "subscribe"}}})
_bm("unsubscribe", {"hostname": "192.168.247.131", "port": 3000, "notifications": ["ScreenDestChanged", "AUXDestChanged"]},
    _rpc("unsubscribe", {"hostname": "192.168.247.131", "port": "3000",
                         "notification": ["ScreenDestChanged", "AUXDestChanged"]}))

# ── Telemetry ────────────────────────────────────────────────────────────
# getFrameSettings, the Encore3 document's example (abridged to two cards).
telemetry(BM, "frame", inbound_http={"path": "/", "request": {"params": {}, "method": "getFrameSettings", "id": "1234", "jsonrpc": "2.0"}, "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": {"System": {"id": 0, "Name": "System1", "FrameCollection": {"id": 0, "Frame": {
        "id": "74:fe:48:7f:22:30", "Name": "ENCORE3", "Contact": "", "Version": "10.0.0.b7e01ed8852.3640",
        "OSVersion": "0.0.23", "FrameType": 8, "FrameTypeName": "Encore3",
        "Enet": {"DhcpMode": 0, "DhcpModeName": "Static", "IP": "192.168.0.200", "MacAddress": "74:fe:48:7f:22:30"},
        "SysCard": {"SlotState": 2, "CardTypeID": 80, "CardTypeLabel": "MBoard", "CardID": 0, "CardStatusID": 2,
                    "CardStatusLabel": "Ready", "OverTemp": 0, "FanWarn": 0},
        "Slot": [{"Card": {"CardStatusID": 2, "CardStatusLabel": "Ready", "CardTypeID": 70,
                           "CardTypeLabel": "High Speed Link", "CardID": "Port1:4", "OverTemp": 0, "FanWarn": 0}},
                 {"Card": {"CardStatusID": 2, "CardStatusLabel": "Ready", "CardTypeID": 4,
                           "CardTypeLabel": "HDMI 2.0 Input", "CardID": "Card1", "OverTemp": 0, "FanWarn": 0}}]}}}}}})},
    state_before={"cards": {"Card9": {"status": "Ready", "type": "SDI Input"}}},
    expect_state={"frame": {"name": "ENCORE3", "version": "10.0.0.b7e01ed8852.3640", "os_version": "0.0.23",
                            "type": 8, "type_name": "Encore3", "ip": "192.168.0.200", "mac": "74:fe:48:7f:22:30",
                            "over_temp": False, "fan_warning": False},
                  "cards": {"Port1:4": {"status": "Ready", "type": "High Speed Link"},
                            "Card1": {"status": "Ready", "type": "HDMI 2.0 Input"}}})
# getFrameSettings, the E2 example: no SysCard flags, one empty slot.
telemetry(BM, "frame-e2", inbound_http={"path": "/", "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": {"System": {"id": 0, "Name": "System1", "FrameCollection": {"id": 0, "Frame": {
        "id": "00:0c:29:0e:86:d4", "Name": "E2", "Contact": "", "Version": "4.2.30738", "OSVersion": "NA",
        "FrameType": 0, "FrameTypeName": "E2",
        "Enet": {"DhcpMode": 0, "DhcpModeName": "Static", "IP": "10.98.0.165", "StaticIP": "192.168.000.175",
                 "MacAddress": "00:0c:29:0e:86:d4"},
        "SysCard": {"SlotState": 2, "CardStatusID": 2, "CardStatusLabel": "Ready", "CardTypeID": 80,
                    "CardTypeLabel": "System", "CardID": 0},
        "Slot": [{"Card": {"CardStatusID": 0, "CardStatusLabel": "Not Installed", "CardTypeID": 255,
                           "CardTypeLabel": "Unknown", "CardID": "Undefined"}}]}}}}}})},
    expect_state={"frame": {"name": "E2", "version": "4.2.30738", "os_version": "NA", "type": 0, "type_name": "E2",
                            "ip": "10.98.0.165", "mac": "00:0c:29:0e:86:d4"},
                  "cards": {"Undefined": {"status": "Not Installed", "type": "Unknown"}}})
# listDestinations, the document's example with aux names added.
telemetry(BM, "destinations", inbound_http={"path": "/", "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": {
        "ScreenDestination": [{"id": 0, "Name": "Dest1", "HSize": 3840, "VSize": 1080, "Layers": 1,
                               "DestOutMapColl": [{"id": 0, "DestOutMap": [{"id": 0, "Name": "Out1", "HPos": 0,
                                                                           "VPos": 0, "HSize": 1920, "VSize": 1080,
                                                                           "Freeze": 0}]}]}],
        "AuxDestination": [{"id": 0, "Name": "AuxDest1", "AuxStreamMode": 4}, {"id": 1, "AuxStreamMode": 4}]}}})},
    expect_state={"screens": {"0": {"name": "Dest1", "width": 3840, "height": 1080}},
                  "aux": {"0": {"name": "AuxDest1", "stream_mode": 4}, "1": {"stream_mode": 4}}})
# listAuxContent: the aux's sources; its Name is not taken (listContent has one too).
telemetry(BM, "aux-content", inbound_http={"path": "/", "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": {"id": 0, "Name": "AuxDest1", "PvwLastSrcIndex": 6, "PgmLastSrcIndex": 1}}})},
    expect_state={"aux": {"0": {"preview_source": 6, "program_source": 1}}})
# listContent (Doc 8.2 example, abridged): IsActive only.
telemetry(BM, "screen-content", inbound_http={"path": "/", "body": json.dumps({"jsonrpc": "2.0", "result": {
    "success": 0, "response": {"id": 0, "Name": "ScreenDest1", "IsActive": 1, "BGLyr": [
        {"id": 0, "LastBGSourceIndex": -1, "BGShowMatte": 1, "BGColor": {"id": 0, "Red": 0, "Green": 0, "Blue": 0}}],
        "Transition": [{"id": 0, "TransTime": 30, "TransPos": 0, "ArmMode": 1}]}}, "id": "1234"})},
    expect_state={"screens": {"0": {"active": True}}})
# listSources, the document's example plus a still.
telemetry(BM, "sources", inbound_http={"path": "/", "request": {"params": {}, "method": "listSources", "id": "1234", "jsonrpc": "2.0"}, "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": SOURCES + [
        {"id": 1, "Name": "Still1", "HSize": 1920, "VSize": 1080, "SrcType": 1, "InputCfgIndex": -1, "StillIndex": 0,
         "DestIndex": -1, "UserKeyIndex": -1, "Mode3D": 0, "Freeze": 0, "Capacity": 1, "InputCfgVideoStatus": 1}]}})},
    state_before={"sources": {"7": {"video_status": "valid", "type": "input"}}},
    expect_state={"sources": {"0": {"video_status": "no_sync", "frozen": True, "type": "input"},
                              "1": {"video_status": "valid", "frozen": False, "type": "still"}}})
# listSources filtered by type lists only some sources: nothing is replaced.
telemetry(BM, "sources-filtered", inbound_http={
    "path": "/", "request": {"params": {"type": 1}, "method": "listSources", "id": "1234", "jsonrpc": "2.0"},
    "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {"success": 0, "response": [
        {"id": 1, "Name": "Still1", "SrcType": 1, "Freeze": 0, "InputCfgVideoStatus": 1}]}})},
    state_before={"sources": {"0": {"video_status": "no_sync", "frozen": True, "type": "input"}}},
    expect_state={"sources": {"0": {"video_status": "no_sync", "frozen": True, "type": "input"},
                              "1": {"video_status": "valid", "frozen": False, "type": "still"}}})
# listPresets holds none of the fields the source rule reads: no state.
telemetry(BM, "presets-ignored", inbound_http={"path": "/", "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": PRESETS}})}, expect_state={})
# listInputs (Encore3 v10.0 example).
telemetry(BM, "inputs", inbound_http={"path": "/", "request": {"params": {}, "method": "listInputs", "id": "1234", "jsonrpc": "2.0"}, "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": [{"id": 8, "Name": "DPInput1", "SyncStatus": "None", "VideoStatus": "No Sync",
                                "Format": "3840x2160p @59.94", "Color_Space": "RGB, Full Range",
                                "Colorimetry": "BT.709", "GammaFx": "SDR", "ColorDepth": "8"}]}})},
    state_before={"inputs": {"9": {"sync_status": "Locked"}}},
    expect_state={"inputs": {"8": {"sync_status": "None"}}})
# listStill (R5919184/00 example).
telemetry(BM, "stills", inbound_http={"path": "/", "request": {"params": {}, "method": "listStill", "id": "1234", "jsonrpc": "2.0"}, "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": [{"id": 0, "Name": "StillStore1", "LockMode": 0,
                                "HSize": {"Min": 0, "Max": 99999, "$t": 1920}, "VSize": {"Min": 0, "Max": 99999, "$t": 1080},
                                "StillState": {"Min": 0, "Max": 4, "$t": 3}, "PngState": {"Min": 0, "Max": 2, "$t": 0},
                                "FileSize": {"Min": 0, "Max": 100000, "$t": 9331.2}}]}})},
    state_before={"stills": {"5": {"state": 3}}},
    expect_state={"stills": {"0": {"state": 3, "file_size_kb": 9331.2, "width": 1920, "height": 1080}}})
# listOperators (Doc 8.2 example, first two operators).
telemetry(BM, "operators", inbound_http={"path": "/", "request": {"params": {}, "method": "listOperators", "id": "1234", "jsonrpc": "2.0"}, "body": json.dumps({"jsonrpc": "2.0", "id": "1234", "result": {
    "success": 0, "response": [
        {"id": 0, "Name": "Operator 1", "Enable": 0, "StartRange": 1, "EndRange": 1000, "InvertColor": 0, "DestCollection": []},
        {"id": 1, "Name": "Operator 2", "Enable": 1, "StartRange": 3, "EndRange": 4, "InvertColor": 0,
         "DestCollection": [{"id": 0, "DestType": 1, "DestXmlId": 0, "Name": "ScreenDest1"}]}]}})},
    state_before={"operators": {"7": {"enabled": True}}},
    expect_state={"operators": {"0": {"enabled": False, "preset_start": 1, "preset_end": 1000},
                                "1": {"enabled": True, "preset_start": 3, "preset_end": 4}}})
