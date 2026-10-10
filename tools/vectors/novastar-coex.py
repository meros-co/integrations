# NovaStar COEX (novastar-coex): one vector per command over the HTTP API on
# port 8001. Most commands get a generated vector: example values put into the
# spec's path and body with an independent renderer written here (JSON strings
# with json.dumps, fixed decimals with Python's format, booleans as true/false
# or 1/0), so the core's template engine is checked against it. The requests
# the documents give an example for are written by hand below from those
# examples (COEX OpenAPI pages; COEX Series Interface API User Manual 2023), and
# replies use the documented {"code", "data", "message"} envelope.
import re as _nc_re

NC = "novastar-coex"
_nc_doc = yaml.safe_load((ROOT / "specs" / f"{NC}.yaml").read_text(encoding="utf-8"))
_NC_EX = {"screen": "{8C6F6B1D-0001}", "cabinets": [93138183199495], "canvases": [1, 2], "input": 1,
          "tables": [{"screenId": "{8C6F6B1D-0001}", "gammaTable": [0, 1, 2]}],
          "gamuts": [{"screenId": "{8C6F6B1D-0001}", "colorGamutInfo": {"colorTemperature": 6500}}],
          "data": [{"screenId": "{8C6F6B1D-0001}"}], "layout": {"screenID": "{8C6F6B1D-0001}", "canvases": []},
          "strategies": ["s1"], "name": "Show", "timezone": "Europe/London", "backup_file": "QUJD"}


def _nc_value(name, p):
    if name in _NC_EX and not (p.get("type") == "enum" and name == "name"):
        return _NC_EX[name]
    t = p["type"]
    if t == "bool":
        return p.get("default", True)
    if t == "int":
        return p.get("default", p.get("max", p.get("min", 1)))
    if t == "float":
        return p.get("max", 1.0)
    if t == "enum":
        return p["values"][0]
    if t == "string":
        return "Show"
    if t == "json":
        return [1]
    raise ValueError(t)


def _nc_render(template, values):
    def sub(m):
        name, directive = m.group(1), m.group(2) or ""
        v = values[name]
        if directive == ":json":
            return json.dumps(v)
        if directive == ":bool01":
            return "1" if v else "0"
        if directive.startswith(":.") and directive.endswith("f"):
            return f"{v:{directive[1:]}}"
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, (list, dict)):
            return json.dumps(v, separators=(",", ":"))
        return str(v)
    return _nc_re.sub(r"\{([a-z_]+)(:[^{}]+)?\}", sub, template)


_NC_HAND = {"set_brightness", "set_display_mode", "recall_preset", "set_cabinet_brightness_nits", "set_edid",
            "set_cabinet_color_temperature", "set_cabinet_gamma", "set_cabinet_gamut", "legacy_recall_preset",
            "set_working_mode", "legacy_set_test_pattern", "set_input_color_space", "get_snmp", "get_display_params",
            "set_display_mode_all"}
for _name, _cmd in _nc_doc["commands"].items():
    if _name in _NC_HAND:
        continue
    _params = _cmd.get("params") or {}
    _values = {k: _nc_value(k, p) for k, p in _params.items()}
    _send = _cmd["send"]
    _target = _send["path"]
    for _k, _v in _values.items():
        _target = _target.replace("{" + _k + "}", _nc_re.sub(r"[^A-Za-z0-9._~-]", lambda m: "%%%02X" % ord(m.group()), str(_v)))
    if "raw_query" in _send:
        _target += "?" + _send["raw_query"]
    _req = {"method": _send["method"], "target": _target}
    if "body" in _send:
        _req["body"] = _nc_render(_send["body"], _values)
    V.append({"spec": NC, "command": _name, "input": _values, "expect_request": _req})


def _nc(command, input, method, target, body=None, **extra):
    req = {"method": method, "target": target}
    if body is not None:
        req["body"] = body
    V.append({"spec": NC, "command": command, "input": input, "expect_request": req, **extra})


_NC_OK = {"status": 200, "body": '{"code":0,"data":null,"message":"Success"}'}
_NC_ACK = {"ok": {"kind": "ack"}}
_S = "{8C6F6B1D-0001}"
# OpenAPI "Set Screen Brightness" example: brightness 0 for one screen.
_nc("set_brightness", {"screen": _S, "brightness": 0}, "PUT", "/api/v1/screen/brightness",
    '{"screenIdList":["' + _S + '"],"brightness":0.0000}', http_reply=_NC_OK, expect_result=_NC_ACK)
# A device-busy answer (error code 5) fails the command.
_nc("set_display_mode", {"screen": _S, "mode": 1}, "PUT", "/api/v1/screen/output/displaymode",
    '{"value":1,"screenIdList":["' + _S + '"]}',
    http_reply={"status": 200, "body": '{"code":5,"data":null,"message":"Busying"}'},
    expect_result={"error": {"error": "device_error"}})
_nc("set_display_mode_all", {"mode": 0}, "PUT", "/api/v1/screen/output/displaymode", '{"value":0,"screenIdList":[]}')
_nc("recall_preset", {"screen": _S, "preset": 2}, "POST", "/api/v1/preset/current/update",
    '{"sequenceNumber":2,"screenID":"' + _S + '"}', http_reply=_NC_OK, expect_result=_NC_ACK)
# 2023 manual examples.
_nc("set_cabinet_brightness_nits", {"cabinets": [93138183199495], "ratio": 1.0, "nits": 1000}, "PUT",
    "/api/v1/device/cabinet/brightness", '{"idList":[93138183199495],"ratio":1.0000,"nit":1000}')
_nc("set_cabinet_gamma", {"cabinets": [93138183199495], "channel": 3, "gamma": 2.8}, "PUT", "/api/v1/device/cabinet/gamma",
    '{"idList":[93138183199495],"type":3,"value":2.80}')
_nc("set_cabinet_color_temperature", {"cabinets": [93138183199495], "kelvin": 6500}, "PUT",
    "/api/v1/device/cabinet/colortemperature", '{"idList":[93138183199495],"value":6500}')
_nc("set_cabinet_gamut", {"name": "DCI-P3"}, "PUT", "/api/v1/device/correctionop/cabinets/gamut", '{"name":"DCI-P3"}')
_nc("set_edid", {"input": 1, "width": 3840, "height": 2160, "rate": 60}, "PUT", "/api/v1/device/input/1/edid",
    '{"para":{"resolution":{"width":3840,"height":2160},"refreshRate":60.00,"isCustom":false}}')
_nc("set_input_color_space", {"input": 1, "space": "0"}, "PUT", "/api/v1/device/input/1/colorspace", '{"colorSpace":0}')
_nc("legacy_recall_preset", {"preset": 1}, "PUT", "/api/v1/device/currentpreset", '{"sequenceNumber":1}')
_nc("set_working_mode", {"mode": 2}, "PUT", "/api/v1/device/hw/mode", '{"mode":2}')
# Pure white (the manual's example).
_nc("legacy_set_test_pattern", {"mode": 0, "grid_width": 1}, "PUT", "/api/v1/device/screen/controller/pattern/test",
    '{"mode":0,"parameters":{"red":4095,"green":4095,"blue":4095,"gray":4095,"gridWidth":1,"moveSpeed":50,'
    '"gradientStretch":8,"state":0}}')
_nc("get_snmp", {}, "GET", "/api/v1/device/snmpstate",
    http_reply={"status": 200, "body": '{"code":0,"data":{"state":true},"message":"Success"}'},
    expect_result={"ok": {"kind": "value", "value": True}})
_nc("get_display_params", {}, "GET", "/api/v1/screen/displayparams")

# Telemetry: the polled reads.
telemetry(NC, "display-params", inbound_http={"path": "/api/v1/screen/displayparams", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"list": [
        {"screenId": "{A}", "brightness": 0.8, "colorTemperature": 6500, "gamma": 2.8}]}})},
    expect_state={"screens": {"{A}": {"brightness": 0.8, "color_temperature": 6500, "gamma": 2.8}}})
telemetry(NC, "display-state", inbound_http={"path": "/api/v1/screen/output/display/state", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"mappingState": [{"canvasID": 1, "enable": False}],
                                                "displayState": [{"canvasID": 1, "displayMode": 2}]}})},
    state_before={"canvases": {"2": {"display_mode": 0, "mapping": True}}},
    expect_state={"canvases": {"1": {"display_mode": 2, "mapping": False}}})
telemetry(NC, "monitor", inbound_http={"path": "/api/v1/device/monitor/info?isNeedCabinetInfo=1", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"name": "MX40 Pro", "runtime": 3600, "backupStatus": 109,
                                                "mainBoardTemperature": {"value": 41.5, "status": 0}}})},
    expect_state={"device": {"runtime": 3600, "backup_status": 109, "mainboard_temperature": 41.5,
                             "mainboard_temperature_status": 0}})
telemetry(NC, "device", inbound_http={"path": "/api/v1/device/hw", "body": json.dumps(
    {"code": 0, "message": "Success", "data": {"name": "MX40 Pro", "sn": "ABC123", "swVersion": "V1.5.0", "mode": 2}})},
    expect_state={"device": {"name": "MX40 Pro", "serial": "ABC123", "software_version": "V1.5.0", "working_mode": 2}})


# Telemetry: the reads added for full control. Bodies use the field names of
# the COEX OpenAPI read pages inside the {"code", "data", "message"} envelope.
def _nc_ok(data):
    return json.dumps({"code": 0, "message": "Success", "data": data})


_NC_SCREEN = {
    "screenID": "{A}", "screenName": "Main", "workingMode": 1, "lowLatency": False, "layoutMode": 0,
    "screenIndex": 1, "screenGroupID": "G1", "masterFrameRate": 60, "pixToPixMode": 0,
    "canvases": [{"canvasID": 1, "outputCardId": 2, "position": {"x": 0, "y": 0},
                  "size": {"width": 1920, "height": 1080}, "maxFrameRate": 60, "zorder": 1,
                  "frequencyPhaseStatus": 0}],
    "layersInWorkingMode": [{"workingMode": 1, "layerLayoutMode": 0, "layers": [
        {"id": 1, "layerIndex": 1, "source": 3, "position": {"x": 0, "y": 0}, "zOrder": 1, "lock": False,
         "border": {"enable": True, "width": 2}, "cut": {"enable": False, "rect": {"x": 0, "y": 0, "width": 1920,
                                                                                    "height": 1080}},
         "scaler": {"width": 1920, "height": 1080}}]}]}
_NC_SCREEN_STATE = {
    "name": "Main", "working_mode": 1, "low_latency": False, "layout_mode": 0, "index": 1, "group": "G1",
    "max_frame_rate": 60.0, "pixel_to_pixel": False,
    "canvases": {"1": {"output_card": 2, "x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0,
                       "max_frame_rate": 60.0, "z_order": 1, "in_phase": True}},
    "working_modes": {"1": {"layer_layout": 0, "layers": {"1": {
        "index": 1, "source": 3, "x": 0.0, "y": 0.0, "width": 1920.0, "height": 1080.0, "z_order": 1,
        "locked": False, "border": True, "border_width": 2.0, "crop": False, "crop_x": 0.0, "crop_y": 0.0,
        "crop_width": 1920.0, "crop_height": 1080.0}}}}}
# The 10-second read without cabinets: each screen's canvases and layers are
# replaced (layer 2 and canvas 9 leave), its other values and its cabinet
# positions stay, and the screen groups are replaced.
telemetry(NC, "screens", inbound_http={"path": "/api/v1/screen?isNeedCabinetInfo=1", "body": _nc_ok(
    {"screens": [_NC_SCREEN], "screenGroups": [{"screenGroupID": "G1", "name": "Stage"}]})},
    state_before={"screens": {"{A}": {"brightness": 0.5, "canvases": {"9": {"z_order": 2}},
                                      "working_modes": {"1": {"layers": {"2": {"source": 1}}}},
                                      "cabinet_positions": {"7": {"x": 0.0}}}},
                  "screen_groups": {"OLD": {"name": "Gone"}}},
    expect_state={"screens": {"{A}": {"brightness": 0.5, "cabinet_positions": {"7": {"x": 0.0}},
                                      **_NC_SCREEN_STATE}},
                  "screen_groups": {"G1": {"name": "Stage"}}})
# The 60-second read with cabinets also replaces the cabinet positions.
_NC_SCREEN_CAB = json.loads(json.dumps(_NC_SCREEN))
_NC_SCREEN_CAB["canvases"][0]["cabinets"] = [{"cabinetID": 93138183199495, "outputID": 1,
                                              "position": {"x": 0, "y": 0},
                                              "size": {"width": 480, "height": 270}, "angle": 0}]
telemetry(NC, "screens-cabinets", inbound_http={"path": "/api/v1/screen", "body": _nc_ok(
    {"screens": [_NC_SCREEN_CAB], "screenGroups": []})},
    state_before={"screens": {"{A}": {"cabinet_positions": {"7": {"x": 0.0}}}}},
    expect_state={"screens": {"{A}": {"cabinet_positions": {"93138183199495": {
        "canvas": 1, "output_port": 1, "x": 0.0, "y": 0.0, "width": 480.0, "height": 270.0, "angle": 0.0}},
        **_NC_SCREEN_STATE}}})
# Presets: each screen's list replaces the last, and the applied one is the
# current preset.
telemetry(NC, "presets", inbound_http={"path": "/api/v1/preset", "body": _nc_ok({"screenPresetList": [
    {"screenID": "{A}", "presetList": [
        {"sequenceNumber": 1, "name": "Walk-in", "state": False, "sourceData": True, "processingData": True,
         "outputData": False, "screenData": True, "effectSwitch": 0},
        {"sequenceNumber": 2, "name": "Show", "state": True, "sourceData": True, "processingData": False,
         "outputData": True, "screenData": False, "effectSwitch": 1}]}]})},
    state_before={"screens": {"{A}": {"current_preset": 9, "presets": {"9": {"name": "Old"}}}}},
    expect_state={"screens": {"{A}": {"current_preset": 2, "presets": {
        "1": {"name": "Walk-in", "applied": False, "source_data": True, "processing_data": True,
              "output_data": False, "screen_data": True, "through_black": False},
        "2": {"name": "Show", "applied": True, "source_data": True, "processing_data": False,
              "output_data": True, "screen_data": False, "through_black": True}}}}})
# None applied: the current preset leaves.
telemetry(NC, "presets-none-applied", inbound_http={"path": "/api/v1/preset", "body": _nc_ok(
    {"screenPresetList": [{"screenID": "{A}", "presetList": []}]})},
    state_before={"screens": {"{A}": {"current_preset": 2, "presets": {"2": {"name": "Show"}}}}},
    expect_state={"screens": {"{A}": {}}})
telemetry(NC, "output", inbound_http={"path": "/api/v1/screen/output", "body": _nc_ok([
    {"screenid": "{A}", "lowDelay": True, "additionalFrameDelay": 1,
     "outputBitDepth": {"bitDepth": 255, "currentBitDepth": 1}, "threeD": {"enable": False},
     "currentFrameRate": 59.94, "genlock": {"masterLayerGroupId": 2, "selectedType": 0},
     "gamutList": {"currentGamutName": "Rec.709"}}])},
    expect_state={"screens": {"{A}": {"output": {
        "low_latency": True, "additional_frame_delay": 1, "bit_depth": 255, "current_bit_depth": 1,
        "three_d": False, "frame_rate": 59.94, "genlock_group": 2, "sync_type": 0, "gamut": "Rec.709"}}}})
telemetry(NC, "screen-properties", inbound_http={"path": "/api/v1/screen/base/info", "body": _nc_ok(
    {"characteristic": 1, "multiModeInfo": [{"screenID": "{A}", "multiModeParam": {
        "currentModeId": 2, "currentCfgParam": {"cabinetName": "P2.6"},
        "modeInfo": [{"modeId": 1, "modeName": "Normal"}, {"modeId": 2, "modeName": "HDR"}]}}]})},
    state_before={"screens": {"{A}": {"multimodes": {"5": {"name": "Old"}}}}},
    expect_state={"device": {"screen_calibrated": 1},
                  "screens": {"{A}": {"multimode": 2, "cabinet_file": "P2.6",
                                      "multimodes": {"1": {"name": "Normal"}, "2": {"name": "HDR"}}}}})
telemetry(NC, "schedules", inbound_http={"path": "/api/v1/screen/schedule/all", "body": _nc_ok([
    {"screenId": "{A}", "enable": True, "brightnessMappingMode": 0,
     "brightnessStrategyList": [{"strategyId": "b1", "startTime": "08:00:00", "endTime": "20:00:00",
                                 "adjustType": 0, "brightnessRatio": 0.8, "brightnessNit": 800,
                                 "repeatEnable": True}],
     "presetStrategyList": [{"strategyId": "p1", "startTime": "19:00:00", "presetNum": 2,
                             "presetName": "Show", "repeatEnable": False}]}])},
    state_before={"screens": {"{A}": {"schedule": {"preset_strategies": {"old": {"preset": 1}}}}}},
    expect_state={"screens": {"{A}": {"schedule": {
        "enabled": True, "brightness_mapping_mode": 0,
        "brightness_strategies": {"b1": {"start": "08:00:00", "end": "20:00:00", "adjust_type": 0,
                                         "brightness": 0.8, "nits": 800.0, "repeat": True}},
        "preset_strategies": {"p1": {"start": "19:00:00", "preset": 2, "preset_name": "Show",
                                     "repeat": False}}}}}})
telemetry(NC, "input-sources", inbound_http={"path": "/api/v1/device/input/sources", "body": _nc_ok([
    {"id": 1, "groupId": 1, "type": 3, "name": "HDMI 1", "sourceStatus": 1,
     "actualResolution": {"width": 3840, "height": 2160}, "actualRefreshRate": 60, "colorSpace": "0",
     "range": 1, "scanMode": 0, "defaultEDID": {"resolution": {"width": 3840, "height": 2160},
                                                "refreshRate": 60}, "isEdidCustom": False}])},
    state_before={"inputs": {"9": {"name": "Gone"}}},
    expect_state={"inputs": {"1": {"group": 1, "type": 3, "name": "HDMI 1", "signal": True, "width": 3840,
                                   "height": 2160, "refresh_rate": 60.0, "color_space": "0", "range": 1,
                                   "interlaced": False, "edid_width": 3840, "edid_height": 2160,
                                   "edid_refresh_rate": 60.0, "edid_custom": False}}})
telemetry(NC, "input-config", inbound_http={"path": "/api/v1/device/input", "body": _nc_ok(
    {"testPattern": {"mode": 16, "parameters": {"red": 4095, "green": 0, "blue": 0, "gray": 255, "gridWidth": 8,
                                                "moveSpeed": 50, "gradientStretch": 8, "state": 1}},
     "InputPortConfig": [{"logicId": 1, "cscParameter": {"HueValue": 0, "ContrastValue": 100,
                                                         "SaturationValue": 120},
                          "isLimitToFull": False, "range": 255, "colorSpaceType": 255, "colorGamut": 2,
                          "hdrParameter": {"overrideHdrType": 255, "realHdrType": "SDR"}}]})},
    expect_state={"test_pattern": {"mode": 16, "on": True, "red": 4095, "green": 0, "blue": 0, "gray": 255,
                                   "grid_width": 8, "move_speed": 50, "gradient_stretch": 8},
                  "input_config": {"1": {"hue": 0, "contrast": 100, "saturation": 120, "limit_to_full": False,
                                         "range": 255, "color_space": 255, "color_gamut": 2, "hdr_mode": 255,
                                         "real_hdr_mode": "SDR"}}})
telemetry(NC, "cabinets", inbound_http={"path": "/api/v1/device/cabinet", "body": _nc_ok([
    {"id": 93138183199495, "brightness": 0.75, "colorTemperature": 6500, "gamma": {"r": 2.8, "g": 2.8, "b": 2.8},
     "canvasID": 1, "outputCardID": 2, "outputID": 1, "size": {"width": 480, "height": 270}}])},
    state_before={"cabinets": {"1": {"brightness": 1.0}}},
    expect_state={"cabinets": {"93138183199495": {
        "brightness": 0.75, "color_temperature": 6500, "gamma_red": 2.8, "gamma_green": 2.8, "gamma_blue": 2.8,
        "canvas": 1, "output_card": 2, "output_port": 1, "width": 480, "height": 270}}})
_NC_HEALTH = {"runtime": 3600, "totalRuntime": 7200, "backupStatus": 109,
              "fanInfos": [{"fanSpeed": 3200, "fanType": 0, "status": 0}],
              "voltageInfos": [{"voltage": 1.2, "voltageType": 0, "status": 0}],
              "temperatureInfos": [{"temperature": 45.5, "temperatureType": 1, "status": 1}],
              "controllerPortMonitorInfos": [{"controllerPortID": 1, "status": 0}],
              "cardMonitorInfo": [{"cardID": 3, "cardType": 1, "status": 0}],
              "screenSourceStatus": [{"portID": 1, "status": 1, "inputCardID": 0}],
              "outputStatus": [{"outputID": 1, "type": 0, "status": 1, "outputCardID": 2}]}
_NC_HEALTH_STATE = {
    "device": {"runtime": 3600, "total_runtime": 7200, "backup_status": 109},
    "monitoring": {"fans": {"0": {"speed": 3200.0, "type": 0, "status": 0}},
                   "voltages": {"0": {"value": 1.2, "type": 0, "status": 0}},
                   "temperatures": {"0": {"value": 45.5, "type": 1, "status": 1}},
                   "ports": {"1": {"status": 0}}, "cards": {"3": {"type": 1, "status": 0}},
                   "sources": {"1": {"status": 1, "input_card": 0}},
                   "outputs": {"1": {"type": 0, "status": 1, "output_card": 2}}}}
# The 10-second read without cabinets keeps the cabinet monitoring.
telemetry(NC, "monitor-health", inbound_http={"path": "/api/v1/device/monitor/info?isNeedCabinetInfo=1",
                                              "body": _nc_ok(_NC_HEALTH)},
    state_before={"monitoring": {"fans": {"1": {"speed": 1.0}}, "cabinets": {"7": {"voltage": 5.0}}}},
    expect_state={"device": _NC_HEALTH_STATE["device"],
                  "monitoring": {**_NC_HEALTH_STATE["monitoring"], "cabinets": {"7": {"voltage": 5.0}}}})
# The 60-second read with cabinets replaces them, with their receiving cards.
telemetry(NC, "monitor-cabinets", inbound_http={"path": "/api/v1/device/monitor/info?isNeedCabinetInfo=0",
                                                "body": _nc_ok({**_NC_HEALTH, "cabinets": [
    {"CabinetID": 93138183199495, "outPutID": 1, "temperature": {"value": 38.5, "status": 0},
     "voltage": {"value": 5.1, "status": 0},
     "rvCards": [{"rvCardID": 4, "runtime": 600, "temperature": {"value": 40, "status": 0},
                  "voltage": {"value": 5, "status": 0}, "humidity": {"value": 30, "status": 0},
                  "nextCabinetLinkStatus": {"linkStatus": False, "status": 0}}]}]})},
    state_before={"monitoring": {"cabinets": {"7": {"voltage": 5.0}}}},
    expect_state={"device": _NC_HEALTH_STATE["device"], "monitoring": {
        **_NC_HEALTH_STATE["monitoring"],
        "cabinets": {"93138183199495": {"output_port": 1, "temperature": 38.5, "temperature_status": 0,
                                        "voltage": 5.1, "voltage_status": 0, "receiving_cards": {"4": {
                                            "runtime": 600, "temperature": 40.0, "temperature_status": 0,
                                            "voltage": 5.0, "voltage_status": 0, "humidity": 30.0,
                                            "humidity_status": 0, "next_link": False,
                                            "next_link_status": 0}}}}}})
telemetry(NC, "audio", inbound_http={"path": "/api/v1/device/audio", "body": _nc_ok({"enable": True, "source": 2})},
          expect_state={"audio": {"enabled": True, "source": 2}})
telemetry(NC, "backup", inbound_http={"path": "/api/v1/device/backup", "body": _nc_ok(
    {"master": "54:B5:6C:00:00:01", "backup": "54:B5:6C:00:00:02", "masterName": "MX40 A", "backupName": "MX40 B"})},
    expect_state={"backup": {"primary_mac": "54:B5:6C:00:00:01", "backup_mac": "54:B5:6C:00:00:02",
                             "primary_name": "MX40 A", "backup_name": "MX40 B"}})
telemetry(NC, "snmp", inbound_http={"path": "/api/v1/device/snmpstate", "body": _nc_ok({"state": True})},
          expect_state={"snmp": {"enabled": True}})
telemetry(NC, "multifunction-cards", inbound_http={"path": "/api/v1/device/multifunc-card/detailinfo",
                                                   "body": _nc_ok([
    {"id": 1, "name": "MFN300", "firmware": "4.6", "linkStatus": "1", "powerInfo": {"allPowerState": True},
     "lightSensorInfos": [{"peripheralIndex": 0, "status": 1, "brightness": "850"}],
     "environmentSensorInfos": [{"peripheralIndex": 1, "status": 1, "temperature": 24.5, "humidity": 40}]}])},
    state_before={"multifunction_cards": {"9": {"name": "Gone"}}},
    expect_state={"multifunction_cards": {"1": {
        "name": "MFN300", "firmware": "4.6", "connected": True, "power": True,
        "light_sensors": {"0": {"ok": True, "brightness": "850"}},
        "environment_sensors": {"1": {"ok": True, "temperature": 24.5, "humidity": 40.0}}}}})

# Read-backs: a write the controller accepted has what it changed read again.
_NC_OK_BODY = '{"code":0,"data":null,"message":"Success"}'


def _nc_get(path, query=None):
    return {"method": "GET", "target": path + (f"?{query}" if query else "")}


for _name, _path, _reads in [
    ("brightness", "/api/v1/screen/brightness", [_nc_get("/api/v1/screen/displayparams")]),
    ("display-mode", "/api/v1/screen/output/displaymode", [_nc_get("/api/v1/screen/output/display/state")]),
    ("canvas-display-mode", "/api/v1/device/displaymode", [_nc_get("/api/v1/screen/output/display/state")]),
    ("gamut", "/api/v1/screen/output/gamut", [_nc_get("/api/v1/screen/output")]),
    ("legacy-bit-depth", "/api/v1/device/screen/video/bitdepth", [_nc_get("/api/v1/screen/output")]),
    ("multimode", "/api/v1/screen/output/multimode", [_nc_get("/api/v1/screen/base/info")]),
    ("layer-source", "/api/v1/screen/layer/input", [_nc_get("/api/v1/screen", "isNeedCabinetInfo=1")]),
    ("recall-preset", "/api/v1/preset/current/update", [
        _nc_get("/api/v1/preset"), _nc_get("/api/v1/screen", "isNeedCabinetInfo=1"),
        _nc_get("/api/v1/screen/displayparams"), _nc_get("/api/v1/screen/output/display/state"),
        _nc_get("/api/v1/screen/output")]),
    ("modify-preset", "/api/v1/preset/update", [_nc_get("/api/v1/preset")]),
    ("schedule", "/api/v1/screen/schedule/enable/update", [_nc_get("/api/v1/screen/schedule/all")]),
    ("move-cabinets", "/api/v1/screen/cabinets", [_nc_get("/api/v1/screen")]),
    ("edid", "/api/v1/device/input/1/edid", [_nc_get("/api/v1/device/input"),
                                             _nc_get("/api/v1/device/input/sources")]),
    ("test-pattern", "/api/v1/device/input/pattern/test", [_nc_get("/api/v1/device/input"),
                                                          _nc_get("/api/v1/device/input/sources")]),
    ("controller-name", "/api/v1/device/hw/customname", [_nc_get("/api/v1/device/hw")]),
    ("cabinet-brightness", "/api/v1/device/cabinet/brightness", [_nc_get("/api/v1/device/cabinet")]),
]:
    telemetry(NC, f"reread-{_name}", inbound_http={"path": _path, "body": _NC_OK_BODY},
              expect_then_send=_reads, expect_state={})
# Writes sharing a path with their read are told apart by the request body.
telemetry(NC, "reread-audio", inbound_http={"path": "/api/v1/device/audio", "body": _NC_OK_BODY,
                                            "request": {"enable": True, "source": 1}},
          expect_then_send=[_nc_get("/api/v1/device/audio")], expect_state={})
telemetry(NC, "reread-snmp", inbound_http={"path": "/api/v1/device/snmpstate", "body": _NC_OK_BODY,
                                           "request": {"state": False}},
          expect_then_send=[_nc_get("/api/v1/device/snmpstate")], expect_state={})
telemetry(NC, "reread-import-project", inbound_http={"path": "/api/v1/device/hw/deviceengineeringdocdata",
                                                     "body": _NC_OK_BODY, "request": {"backupFile": "QUJD"}},
          expect_then_send=[_nc_get(p) for p in [
              "/api/v1/device/hw", "/api/v1/screen", "/api/v1/preset", "/api/v1/screen/displayparams",
              "/api/v1/screen/output/display/state", "/api/v1/screen/output", "/api/v1/screen/base/info",
              "/api/v1/screen/schedule/all", "/api/v1/device/input", "/api/v1/device/input/sources",
              "/api/v1/device/cabinet"]], expect_state={})
# A read on the same path queues nothing, and a refused write is not read back.
telemetry(NC, "audio-read-no-reread", inbound_http={"path": "/api/v1/device/audio",
                                                    "body": _nc_ok({"enable": False, "source": 0})},
          expect_then_send=[], expect_state={"audio": {"enabled": False, "source": 0}})
telemetry(NC, "refused-write-no-reread", inbound_http={
    "path": "/api/v1/screen/brightness", "body": '{"code":5,"data":null,"message":"Busying"}'},
    expect_then_send=[], expect_state={})
