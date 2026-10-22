# Sony BRAVIA Professional Displays (sony-bravia): one vector per command over
# the REST API and IRCC-IP. Every REST request is an HTTP POST to
# /sony/<service> whose body is a JSON-RPC request with method, id, params and
# version, as in the API reference's examples; id 1 is used throughout. Bodies
# are written with json.dumps from the reference's own shapes, not from the
# spec's templates. Replies use the reference's examples.
SB = "sony-bravia"


def _sb_body(method, params=None, version="1.0"):
    return json.dumps({"method": method, "id": 1, "params": params if params is not None else [], "version": version},
                      separators=(",", ":"))


def _sb(command, input, service, method, params=None, version="1.0", model=None, **extra):
    v = {"spec": SB, "command": command, "input": input,
         "expect_request": {"method": "POST", "target": f"/sony/{service}", "body": _sb_body(method, params, version)},
         **extra}
    if model:
        v["model"] = model
    V.append(v)


def _sb_ok(result):
    return {"status": 200, "body": json.dumps({"result": result, "id": 1})}


SB_ACK = {"ok": {"kind": "ack"}}

# ── system ──
_sb("set_power", {"power_on": False}, "system", "setPowerStatus", [{"status": False}])
_sb("get_power", {}, "system", "getPowerStatus", http_reply=_sb_ok([{"status": "standby"}]),
    expect_result={"ok": {"kind": "value", "value": "standby"}})
_sb("reboot", {}, "system", "requestReboot", http_reply=_sb_ok([]), expect_result=SB_ACK)
_sb("set_power_saving", {"mode": "pictureOff"}, "system", "setPowerSavingMode", [{"mode": "pictureOff"}])
_sb("get_power_saving", {}, "system", "getPowerSavingMode", http_reply=_sb_ok([{"mode": "high"}]),
    expect_result={"ok": {"kind": "value", "value": "high"}})
_sb("set_led_indicator", {"mode": "Demo", "status": "true"}, "system", "setLEDIndicatorStatus",
    [{"mode": "Demo", "status": "true"}], "1.1")
_sb("get_led_indicator", {}, "system", "getLEDIndicatorStatus")
_sb("set_wake_on_lan", {"enabled": False}, "system", "setWolMode", [{"enabled": False}])
_sb("get_wake_on_lan", {}, "system", "getWolMode")
_sb("get_system_information", {}, "system", "getSystemInformation", version="1.7")
_sb("get_interface_information", {}, "system", "getInterfaceInformation")
_sb("get_network_settings", {"interface": "eth0"}, "system", "getNetworkSettings", [{"netif": "eth0"}])
_sb("get_remote_access", {}, "system", "getRemoteDeviceSettings", [{"target": "accessPermission"}],
    http_reply=_sb_ok([[{"currentValue": "on", "target": "accessPermission"}]]),
    expect_result={"ok": {"kind": "value", "value": "on"}})
_sb("get_time", {}, "system", "getCurrentTime", version="1.1")
_sb("get_supported_functions", {}, "system", "getSystemSupportedFunction")
_sb("get_remote_codes", {}, "system", "getRemoteControllerInfo")
_sb("get_supported_apis", {}, "guide", "getSupportedApiInfo", [{"services": []}])
# An error answer fails the command.
_sb("get_scene", {}, "videoScreen", "getSceneSetting", model="bz",
    http_reply={"status": 200, "body": json.dumps({"error": [7, "Illegal State"], "id": 1})},
    expect_result={"error": {"error": "device_error"}})
# ── audio ──
_sb("set_volume", {"volume": 18, "target": "speaker"}, "audio", "setAudioVolume", [{"target": "speaker", "volume": "18"}])
_sb("set_volume_with_ui", {"volume": 5, "target": "speaker", "ui": "on"}, "audio", "setAudioVolume",
    [{"target": "speaker", "volume": "5", "ui": "on"}], "1.2")
_sb("adjust_volume", {"steps": 1, "target": "all"}, "audio", "setAudioVolume", [{"target": "", "volume": "+1"}])
_sb("set_mute", {"muted": True}, "audio", "setAudioMute", [{"status": True}])
_sb("get_volume", {}, "audio", "getVolumeInformation")
_sb("set_audio_output", {"output": "speaker"}, "audio", "setSoundSettings",
    [{"settings": [{"target": "outputTerminal", "value": "speaker"}]}], "1.1", http_reply=_sb_ok([]), expect_result=SB_ACK)
_sb("set_speaker_setting", {"setting": "tvPosition", "value": "wallMount"}, "audio", "setSpeakerSettings",
    [{"settings": [{"target": "tvPosition", "value": "wallMount"}]}], model="bz")
_sb("get_speaker_settings", {}, "audio", "getSpeakerSettings", [{"target": ""}], model="bz")
# ── inputs and content ──
_sb("set_input", {"uri": "extInput:hdmi?port=2"}, "avContent", "setPlayContent", [{"uri": "extInput:hdmi?port=2"}])
_sb("set_hdmi_input", {"port": 3}, "avContent", "setPlayContent", [{"uri": "extInput:hdmi?port=3"}])
_sb("get_input", {}, "avContent", "getPlayingContentInfo")
_sb("get_inputs", {}, "avContent", "getCurrentExternalInputsStatus", version="1.1")
_sb("get_content_count", {"source": "extInput:hdmi"}, "avContent", "getContentCount", [{"source": "extInput:hdmi"}], "1.1",
    http_reply=_sb_ok([{"count": 4}]), expect_result={"ok": {"kind": "value", "value": 4}})
_sb("get_content_list", {"uri": "extInput:hdmi"}, "avContent", "getContentList",
    [{"uri": "extInput:hdmi", "stIdx": 0, "cnt": 50}], "1.5")
_sb("get_schemes", {}, "avContent", "getSchemeList")
_sb("get_sources", {}, "avContent", "getSourceList", [{"scheme": "extInput"}])
# ── apps ──
_sb("launch_app", {"uri": "localapp://webappruntime?url=http%3A%2F%2Fexample.com%2F"}, "appControl", "setActiveApp",
    [{"uri": "localapp://webappruntime?url=http%3A%2F%2Fexample.com%2F"}])
_sb("terminate_apps", {}, "appControl", "terminateApps")
_sb("get_apps", {}, "appControl", "getApplicationList")
_sb("get_app_status", {}, "appControl", "getApplicationStatusList")
_sb("get_web_app_status", {}, "appControl", "getWebAppStatus")
_sb("set_text", {"text": "hello world!!"}, "appControl", "setTextForm", ["hello world!!"])
# ── picture ──
_sb("set_picture_setting", {"setting": "color", "value": "2"}, "video", "setPictureQualitySettings",
    [{"settings": [{"target": "color", "value": "2"}]}], "1.1", model="bz-p")
_sb("get_picture_settings", {}, "video", "getPictureQualitySettings", [{"target": ""}], "1.0", model="bz")
_sb("set_scene", {"scene": "auto"}, "videoScreen", "setSceneSetting", [{"value": "auto"}], model="bz")
# ── IRCC-IP: the SOAP envelope of the 2019 overview page ──
_IRCC = ('<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" '
         's:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body>'
         '<u:X_SendIRCC xmlns:u="urn:schemas-sony-com:service:IRCC:1"><IRCCCode>%s</IRCCCode>'
         '</u:X_SendIRCC></s:Body></s:Envelope>')
V.append({"spec": SB, "command": "press_key", "input": {"key": "Down"},
          "expect_request": {"method": "POST", "target": "/sony/ircc", "body": _IRCC % "AAAAAQAAAAEAAAB1Aw=="}})
V.append({"spec": SB, "command": "send_ircc_code", "input": {"code": "AAAAAQAAAAEAAAAVAw=="},
          "expect_request": {"method": "POST", "target": "/sony/ircc", "body": _IRCC % "AAAAAQAAAAEAAAAVAw=="}})


# ── Telemetry: the reference's example replies ──
def _sb_tel(name, service, request, result, expect_state, model=None, **extra):
    v = {"inbound_http": {"path": f"/sony/{service}", "request": request,
                          "body": json.dumps({"result": result, "id": 1})}, "expect_state": expect_state, **extra}
    if model:
        v["model"] = model
    telemetry(SB, name, **v)


def _req(method, params=None, version="1.0"):
    return {"method": method, "id": 1, "params": params if params is not None else [], "version": version}


_sb_tel("power", "system", _req("getPowerStatus"), [{"status": "active"}], {"power": "active"})
_sb_tel("volume", "audio", _req("getVolumeInformation"),
        [[{"volume": 25, "minVolume": 0, "mute": False, "maxVolume": 100, "target": "speaker"},
          {"volume": 10, "minVolume": 0, "mute": True, "maxVolume": 100, "target": "headphone"}]],
        {"audio": {"speaker": {"volume": 25, "mute": False, "max": 100, "min": 0},
                   "headphone": {"volume": 10, "mute": True, "max": 100, "min": 0}}})
_sb_tel("playing", "avContent", _req("getPlayingContentInfo"),
        [{"source": "extInput:hdmi", "title": "HDMI 2", "uri": "extInput:hdmi?port=2"}],
        {"input": {"uri": "extInput:hdmi?port=2", "source": "extInput:hdmi", "title": "HDMI 2"}})
_sb_tel("inputs", "avContent", _req("getCurrentExternalInputsStatus", version="1.1"),
        [[{"icon": "meta:game", "connection": False, "label": "GAME", "title": "Component1",
           "uri": "extInput:component?port=1", "status": "false"},
          {"icon": "meta:hdmi", "connection": True, "label": "", "title": "HDMI1", "uri": "extInput:hdmi?port=1",
           "status": "true"}]],
        {"inputs": {"0": {"uri": "extInput:component?port=1", "title": "Component1", "label": "GAME", "connected": False,
                          "icon": "meta:game", "signal": False},
                    "1": {"uri": "extInput:hdmi?port=1", "title": "HDMI1", "label": "", "connected": True,
                          "icon": "meta:hdmi", "signal": True}}})
_sb_tel("power-saving", "system", _req("getPowerSavingMode"), [{"mode": "high"}], {"power_saving": "high"})
_sb_tel("led", "system", _req("getLEDIndicatorStatus"), [{"mode": "Demo", "status": "true"}],
        {"led": {"mode": "Demo", "lit": True}})
_sb_tel("wol", "system", _req("getWolMode"), [{"enabled": True}], {"wake_on_lan": True})
_sb_tel("picture", "video", _req("getPictureQualitySettings", [{"target": ""}]),
        [[{"target": "color", "currentValue": "2", "isAvailable": True, "candidate": [{"max": 100, "min": 0, "step": 1}]}]],
        {"picture": {"color": {"value": "2", "available": True, "candidates": '[{"max":100,"min":0,"step":1}]'}}})
_sb_tel("scene", "videoScreen", _req("getSceneSetting"),
        [{"candidate": [{"value": "auto"}, {"value": "general"}], "currentValue": "auto"}],
        {"scene": {"value": "auto", "candidates": '[{"value":"auto"},{"value":"general"}]'}}, model="bz")
_sb_tel("speaker", "audio", _req("getSpeakerSettings", [{"target": ""}]),
        [[{"currentValue": "tableTop", "target": "tvPosition"}]], {"speaker": {"tvPosition": "tableTop"}}, model="bz")
_sb_tel("time", "system", _req("getCurrentTime", version="1.1"),
        [{"dateTime": "2018-10-03T13:03:59+0100", "timeZoneOffsetMinute": 60, "dstOffsetMinute": 60}],
        {"clock": {"time": "2018-10-03T13:03:59+0100", "timezone_offset_min": 60, "dst_offset_min": 60}})
_sb_tel("app-status", "appControl", _req("getApplicationStatusList"),
        [[{"name": "textInput", "status": "off"}, {"name": "webBrowse", "status": "on"}]],
        {"apps": {"status": {"textInput": "off", "webBrowse": "on"}}})
_sb_tel("web-app", "appControl", _req("getWebAppStatus"), [{"active": True, "url": "http://example.com/"}],
        {"webapp": {"active": True, "url": "http://example.com/"}})
_sb_tel("apps", "appControl", _req("getApplicationList"),
        [[{"title": "Screen mirroring", "uri": "com.sony.dtv.screnmirroring", "icon": "http://10.0.0.5/icon.png"}]],
        {"apps": {"list": {"0": {"title": "Screen mirroring", "uri": "com.sony.dtv.screnmirroring",
                                 "icon": "http://10.0.0.5/icon.png"}}}})
_sb_tel("system", "system", _req("getSystemInformation", version="1.7"),
        [{"generation": "5.6.0", "product": "TV", "serial": "1234567", "name": "BRAVIA", "language": "eng",
          "model": "FW-43BZ35J", "macAddr": "04:5D:4B:AD:6D:E1", "fwVersion": "PKG0.6.0.81.00.1.00.0951BBA",
          "androidOs": "12", "webAppRuntimeVersion": "4.0.0+7", "mode": "Normal"}],
        {"system": {"generation": "5.6.0", "product": "TV", "serial": "1234567", "name": "BRAVIA", "language": "eng",
                    "model": "FW-43BZ35J", "mac": "04:5D:4B:AD:6D:E1", "firmware": "PKG0.6.0.81.00.1.00.0951BBA",
                    "android": "12", "webapp_runtime": "4.0.0+7", "mode": "Normal"}})
_sb_tel("interface", "system", _req("getInterfaceInformation"),
        [{"modelName": "FW-55BZ35F", "serverName": "", "interfaceVersion": "5.0.1", "productName": "BRAVIA",
          "productCategory": "tv"}],
        {"interface": {"model_name": "FW-55BZ35F", "server_name": "", "version": "5.0.1", "product_name": "BRAVIA",
                       "product_category": "tv"}})
_sb_tel("network", "system", _req("getNetworkSettings", [{"netif": ""}]),
        [[{"hwAddr": "FF-FF-FF-FF-FF-FF", "netmask": "255.255.255.0", "ipAddrV4": "10.0.0.5", "netif": "eth0",
           "ipAddrV6": "fe80::1", "dns": ["10.0.0.1"], "gateway": "10.0.0.1"}]],
        {"network": {"eth0": {"mac": "FF-FF-FF-FF-FF-FF", "netmask": "255.255.255.0", "ipv4": "10.0.0.5",
                              "ipv6": "fe80::1", "dns": '["10.0.0.1"]', "gateway": "10.0.0.1"}}})
_sb_tel("remote-access", "system", _req("getRemoteDeviceSettings", [{"target": "accessPermission"}]),
        [[{"currentValue": "on", "target": "accessPermission"}]], {"remote_access": "on"})
_sb_tel("functions", "system", _req("getSystemSupportedFunction"),
        [[{"value": "00:00:00:00:00:00:00:E0", "option": "WOL"}]], {"functions": {"WOL": "00:00:00:00:00:00:00:E0"}})
_sb_tel("remote-codes", "system", _req("getRemoteControllerInfo"),
        [{"bundled": True, "type": "IR_REMOTE_BUNDLE_TYPE_AEP"}, [{"name": "PowerOff", "value": "AAAAAQAAAAEAAAAvAw=="}]],
        {"ircc_codes": {"PowerOff": "AAAAAQAAAAEAAAAvAw=="}})
_sb_tel("schemes", "avContent", _req("getSchemeList"), [[{"scheme": "extInput"}]],
        {"content": {"schemes": '[{"scheme":"extInput"}]'}})
_sb_tel("sources", "avContent", _req("getSourceList", [{"scheme": "extInput"}]),
        [[{"source": "extInput:hdmi"}]], {"content": {"sources": {"extInput": '[{"source":"extInput:hdmi"}]'}}})
_sb_tel("apis", "guide", _req("getSupportedApiInfo", [{"services": []}]),
        [[{"service": "system", "protocols": ["xhrpost:jsonizer"], "apis": []}]],
        {"supported_apis": '[{"service":"system","protocols":["xhrpost:jsonizer"],"apis":[]}]'})
