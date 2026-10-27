# ONVIF PTZ (onvif-ptz): one vector per command, written by hand from the
# ONVIF Core, PTZ, Imaging and Media service specifications: each request is
# a SOAP 1.2 envelope POSTed as application/soap+xml to the service's
# address, its body the operation's element in the service's namespace with
# the tt schema's types. Settings give the profile and video source tokens.
OV = "onvif-ptz"
OV_SET = {"profile_token": "Profile_1", "video_source_token": "VideoSource_1"}
OV_NS = {"tds": "http://www.onvif.org/ver10/device/wsdl", "trt": "http://www.onvif.org/ver10/media/wsdl",
         "tptz": "http://www.onvif.org/ver20/ptz/wsdl", "timg": "http://www.onvif.org/ver20/imaging/wsdl"}
OV_PATH = {"tds": "/onvif/device_service", "trt": "/onvif/media_service", "tptz": "/onvif/ptz_service",
           "timg": "/onvif/imaging_service"}
SOAP = "application/soap+xml; charset=utf-8"


def _ov_env(prefix, body):
    return ('<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" '
            f'xmlns:tt="http://www.onvif.org/ver10/schema" xmlns:{prefix}="{OV_NS[prefix]}"><s:Body>{body}</s:Body>'
            '</s:Envelope>')


def _ov(command, input, prefix, body, **extra):
    V.append({"spec": OV, "command": command, "input": input, "settings": dict(OV_SET),
              "expect_request": {"method": "POST", "target": OV_PATH[prefix], "content_type": SOAP,
                                 "body": _ov_env(prefix, body)}, **extra})


P = "<tptz:ProfileToken>Profile_1</tptz:ProfileToken>"
S = "<timg:VideoSourceToken>VideoSource_1</timg:VideoSourceToken>"

# Device management (Core Specification)
_ov("get_device_information", {}, "tds", "<tds:GetDeviceInformation/>")
_ov("get_services", {}, "tds", "<tds:GetServices><tds:IncludeCapability>false</tds:IncludeCapability></tds:GetServices>")
_ov("get_system_date_and_time", {}, "tds", "<tds:GetSystemDateAndTime/>")
_ov("get_hostname", {}, "tds", "<tds:GetHostname/>")
_ov("set_hostname", {"name": "cam-1"}, "tds", "<tds:SetHostname><tds:Name>cam-1</tds:Name></tds:SetHostname>")
_ov("reboot", {}, "tds", "<tds:SystemReboot/>", http_reply={"status": 200, "body": "<x/>"},
    expect_result={"ok": {"kind": "ack"}})
_ov("factory_default", {"kind": "Soft"}, "tds",
    "<tds:SetSystemFactoryDefault><tds:FactoryDefault>Soft</tds:FactoryDefault></tds:SetSystemFactoryDefault>")
_ov("get_relay_outputs", {}, "tds", "<tds:GetRelayOutputs/>")
_ov("set_relay_output", {"relay": "RelayOutput_1", "state": "active"}, "tds",
    "<tds:SetRelayOutputState><tds:RelayOutputToken>RelayOutput_1</tds:RelayOutputToken>"
    "<tds:LogicalState>active</tds:LogicalState></tds:SetRelayOutputState>")
# Media
_ov("get_profiles", {}, "trt", "<trt:GetProfiles/>")
_ov("get_video_sources", {}, "trt", "<trt:GetVideoSources/>")
_ov("get_stream_uri", {}, "trt",
    "<trt:GetStreamUri><trt:StreamSetup><tt:Stream>RTP-Unicast</tt:Stream><tt:Transport><tt:Protocol>RTSP</tt:Protocol>"
    "</tt:Transport></trt:StreamSetup><trt:ProfileToken>Profile_1</trt:ProfileToken></trt:GetStreamUri>")
_ov("get_snapshot_uri", {}, "trt", "<trt:GetSnapshotUri><trt:ProfileToken>Profile_1</trt:ProfileToken></trt:GetSnapshotUri>")
# PTZ
_ov("absolute_move", {"pan": 0.5, "tilt": -0.25, "zoom": 0}, "tptz",
    f'<tptz:AbsoluteMove>{P}<tptz:Position><tt:PanTilt x="0.5000" y="-0.2500"/><tt:Zoom x="0.0000"/></tptz:Position>'
    "</tptz:AbsoluteMove>")
_ov("absolute_move_at_speed", {"pan": -1, "tilt": 1, "zoom": 1, "pan_tilt_speed": 0.5, "zoom_speed": 1}, "tptz",
    f'<tptz:AbsoluteMove>{P}<tptz:Position><tt:PanTilt x="-1.0000" y="1.0000"/><tt:Zoom x="1.0000"/></tptz:Position>'
    '<tptz:Speed><tt:PanTilt x="0.5000" y="0.5000"/><tt:Zoom x="1.0000"/></tptz:Speed></tptz:AbsoluteMove>')
_ov("absolute_pan_tilt", {"pan": 0.1, "tilt": 0.2}, "tptz",
    f'<tptz:AbsoluteMove>{P}<tptz:Position><tt:PanTilt x="0.1000" y="0.2000"/></tptz:Position></tptz:AbsoluteMove>')
_ov("absolute_zoom", {"zoom": 0.75}, "tptz",
    f'<tptz:AbsoluteMove>{P}<tptz:Position><tt:Zoom x="0.7500"/></tptz:Position></tptz:AbsoluteMove>')
_ov("relative_move", {"pan": 0.1, "tilt": 0, "zoom": -0.1}, "tptz",
    f'<tptz:RelativeMove>{P}<tptz:Translation><tt:PanTilt x="0.1000" y="0.0000"/><tt:Zoom x="-0.1000"/>'
    "</tptz:Translation></tptz:RelativeMove>")
_ov("continuous_move", {"pan": 0.5, "tilt": 0, "zoom": 0}, "tptz",
    f'<tptz:ContinuousMove>{P}<tptz:Velocity><tt:PanTilt x="0.5000" y="0.0000"/><tt:Zoom x="0.0000"/>'
    "</tptz:Velocity></tptz:ContinuousMove>")
_ov("continuous_move_for", {"pan": 0, "tilt": -0.5, "zoom": 0, "seconds": 3}, "tptz",
    f'<tptz:ContinuousMove>{P}<tptz:Velocity><tt:PanTilt x="0.0000" y="-0.5000"/><tt:Zoom x="0.0000"/>'
    "</tptz:Velocity><tptz:Timeout>PT3S</tptz:Timeout></tptz:ContinuousMove>")
_ov("stop", {}, "tptz", f"<tptz:Stop>{P}<tptz:PanTilt>true</tptz:PanTilt><tptz:Zoom>true</tptz:Zoom></tptz:Stop>")
_ov("stop_pan_tilt", {}, "tptz", f"<tptz:Stop>{P}<tptz:PanTilt>true</tptz:PanTilt><tptz:Zoom>false</tptz:Zoom></tptz:Stop>")
_ov("stop_zoom", {}, "tptz", f"<tptz:Stop>{P}<tptz:PanTilt>false</tptz:PanTilt><tptz:Zoom>true</tptz:Zoom></tptz:Stop>")
_ov("get_status", {}, "tptz", f"<tptz:GetStatus>{P}</tptz:GetStatus>")
_ov("goto_preset", {"preset": "2"}, "tptz", f"<tptz:GotoPreset>{P}<tptz:PresetToken>2</tptz:PresetToken></tptz:GotoPreset>")
_ov("goto_preset", {"preset": "9"}, "tptz", f"<tptz:GotoPreset>{P}<tptz:PresetToken>9</tptz:PresetToken></tptz:GotoPreset>",
    http_reply={"status": 500, "body": "<s:Fault/>"}, expect_result={"error": {"error": "device_error", "code": "500"}},
    file="goto_preset-fault")
_ov("goto_preset_at_speed", {"preset": "2", "pan_tilt_speed": 0.2, "zoom_speed": 0.2}, "tptz",
    f'<tptz:GotoPreset>{P}<tptz:PresetToken>2</tptz:PresetToken><tptz:Speed><tt:PanTilt x="0.2000" y="0.2000"/>'
    '<tt:Zoom x="0.2000"/></tptz:Speed></tptz:GotoPreset>')
_ov("save_preset", {"name": "Lectern"}, "tptz", f"<tptz:SetPreset>{P}<tptz:PresetName>Lectern</tptz:PresetName></tptz:SetPreset>")
_ov("update_preset", {"preset": "2", "name": "Choir"}, "tptz",
    f"<tptz:SetPreset>{P}<tptz:PresetName>Choir</tptz:PresetName><tptz:PresetToken>2</tptz:PresetToken></tptz:SetPreset>")
_ov("remove_preset", {"preset": "2"}, "tptz", f"<tptz:RemovePreset>{P}<tptz:PresetToken>2</tptz:PresetToken></tptz:RemovePreset>")
_ov("get_presets", {}, "tptz", f"<tptz:GetPresets>{P}</tptz:GetPresets>")
_ov("goto_home", {}, "tptz", f"<tptz:GotoHomePosition>{P}</tptz:GotoHomePosition>")
_ov("set_home", {}, "tptz", f"<tptz:SetHomePosition>{P}</tptz:SetHomePosition>")
_ov("send_auxiliary_command", {"command": "tt:Wiper|On"}, "tptz",
    f"<tptz:SendAuxiliaryCommand>{P}<tptz:AuxiliaryData>tt:Wiper|On</tptz:AuxiliaryData></tptz:SendAuxiliaryCommand>")
_ov("get_nodes", {}, "tptz", "<tptz:GetNodes/>")
_ov("get_configurations", {}, "tptz", "<tptz:GetConfigurations/>")
_ov("get_preset_tours", {}, "tptz", f"<tptz:GetPresetTours>{P}</tptz:GetPresetTours>")
_ov("operate_preset_tour", {"tour": "Tour_1", "operation": "Start"}, "tptz",
    f"<tptz:OperatePresetTour>{P}<tptz:PresetTourToken>Tour_1</tptz:PresetTourToken><tptz:Operation>Start</tptz:Operation>"
    "</tptz:OperatePresetTour>")
_ov("remove_preset_tour", {"tour": "Tour_1"}, "tptz",
    f"<tptz:RemovePresetTour>{P}<tptz:PresetTourToken>Tour_1</tptz:PresetTourToken></tptz:RemovePresetTour>")


# Imaging
def _img_set(command, input, inner):
    _ov(command, input, "timg", f"<timg:SetImagingSettings>{S}<timg:ImagingSettings>{inner}</timg:ImagingSettings>"
                                "</timg:SetImagingSettings>")


_ov("get_imaging_settings", {}, "timg", f"<timg:GetImagingSettings>{S}</timg:GetImagingSettings>")
_ov("get_imaging_options", {}, "timg", f"<timg:GetOptions>{S}</timg:GetOptions>")
_img_set("set_brightness", {"level": 50}, "<tt:Brightness>50.00</tt:Brightness>")
_img_set("set_color_saturation", {"level": 60}, "<tt:ColorSaturation>60.00</tt:ColorSaturation>")
_img_set("set_contrast", {"level": 45.5}, "<tt:Contrast>45.50</tt:Contrast>")
_img_set("set_sharpness", {"level": 3}, "<tt:Sharpness>3.00</tt:Sharpness>")
_img_set("set_ir_cut_filter", {"mode": "AUTO"}, "<tt:IrCutFilter>AUTO</tt:IrCutFilter>")
_img_set("set_backlight_compensation", {"mode": "ON"}, "<tt:BacklightCompensation><tt:Mode>ON</tt:Mode></tt:BacklightCompensation>")
_img_set("set_backlight_compensation_level", {"level": 10},
         "<tt:BacklightCompensation><tt:Mode>ON</tt:Mode><tt:Level>10.00</tt:Level></tt:BacklightCompensation>")
_img_set("set_wide_dynamic_range", {"mode": "OFF"}, "<tt:WideDynamicRange><tt:Mode>OFF</tt:Mode></tt:WideDynamicRange>")
_img_set("set_wide_dynamic_range_level", {"level": 50},
         "<tt:WideDynamicRange><tt:Mode>ON</tt:Mode><tt:Level>50.00</tt:Level></tt:WideDynamicRange>")
_img_set("set_exposure_mode", {"mode": "AUTO"}, "<tt:Exposure><tt:Mode>AUTO</tt:Mode></tt:Exposure>")
_img_set("set_exposure_time", {"microseconds": 10000},
         "<tt:Exposure><tt:Mode>MANUAL</tt:Mode><tt:ExposureTime>10000.00</tt:ExposureTime></tt:Exposure>")
_img_set("set_gain", {"db": 6}, "<tt:Exposure><tt:Mode>MANUAL</tt:Mode><tt:Gain>6.00</tt:Gain></tt:Exposure>")
_img_set("set_iris", {"db": -3}, "<tt:Exposure><tt:Mode>MANUAL</tt:Mode><tt:Iris>-3.00</tt:Iris></tt:Exposure>")
_img_set("set_focus_mode", {"mode": "MANUAL"}, "<tt:Focus><tt:AutoFocusMode>MANUAL</tt:AutoFocusMode></tt:Focus>")
_img_set("set_white_balance_mode", {"mode": "AUTO"}, "<tt:WhiteBalance><tt:Mode>AUTO</tt:Mode></tt:WhiteBalance>")
_img_set("set_white_balance_gains", {"cr": 120, "cb": 90},
         "<tt:WhiteBalance><tt:Mode>MANUAL</tt:Mode><tt:CrGain>120.00</tt:CrGain><tt:CbGain>90.00</tt:CbGain></tt:WhiteBalance>")
_ov("focus_move_absolute", {"position": 0.5}, "timg",
    f"<timg:Move>{S}<timg:Focus><tt:Absolute><tt:Position>0.5000</tt:Position></tt:Absolute></timg:Focus></timg:Move>")
_ov("focus_move_relative", {"distance": -0.1}, "timg",
    f"<timg:Move>{S}<timg:Focus><tt:Relative><tt:Distance>-0.1000</tt:Distance></tt:Relative></timg:Focus></timg:Move>")
_ov("focus_move_continuous", {"speed": 1}, "timg",
    f"<timg:Move>{S}<timg:Focus><tt:Continuous><tt:Speed>1.0000</tt:Speed></tt:Continuous></timg:Focus></timg:Move>")
_ov("focus_stop", {}, "timg", f"<timg:Stop>{S}</timg:Stop>")
_ov("get_focus_status", {}, "timg", f"<timg:GetStatus>{S}</timg:GetStatus>")
_ov("get_imaging_presets", {}, "timg", f"<timg:GetPresets>{S}</timg:GetPresets>")
_ov("set_imaging_preset", {"preset": "Preset_2"}, "timg",
    f"<timg:SetCurrentPreset>{S}<timg:PresetToken>Preset_2</timg:PresetToken></timg:SetCurrentPreset>")
_ov("get_current_imaging_preset", {}, "timg", f"<timg:GetCurrentPreset>{S}</timg:GetCurrentPreset>")


# Telemetry: replies shaped as the specifications' schemas give them.
def _ov_reply(prefix, body):
    return ('<?xml version="1.0" encoding="UTF-8"?><env:Envelope xmlns:env="http://www.w3.org/2003/05/soap-envelope" '
            f'xmlns:tt="http://www.onvif.org/ver10/schema" xmlns:{prefix}="{OV_NS[prefix]}"><env:Body>{body}</env:Body>'
            '</env:Envelope>')


def _ov_tel(name, prefix, body, state, **extra):
    telemetry(OV, name, settings=dict(OV_SET), inbound_http={"path": OV_PATH[prefix], "body": _ov_reply(prefix, body)},
              expect_state=state, **extra)


_ov_tel("device-information", "tds",
        "<tds:GetDeviceInformationResponse><tds:Manufacturer>Acme</tds:Manufacturer><tds:Model>PTZ-1</tds:Model>"
        "<tds:FirmwareVersion>2.1</tds:FirmwareVersion><tds:SerialNumber>SN1</tds:SerialNumber>"
        "<tds:HardwareId>HW1</tds:HardwareId></tds:GetDeviceInformationResponse>",
        {"device": {"manufacturer": "Acme", "model": "PTZ-1", "firmware": "2.1", "serial": "SN1", "hardware_id": "HW1"}})
_ov_tel("hostname", "tds",
        "<tds:GetHostnameResponse><tds:HostnameInformation><tt:FromDHCP>false</tt:FromDHCP><tt:Name>cam-1</tt:Name>"
        "</tds:HostnameInformation></tds:GetHostnameResponse>",
        {"hostname": {"name": "cam-1", "from_dhcp": False}})
_ov_tel("services", "tds",
        "<tds:GetServicesResponse><tds:Service><tds:Namespace>http://www.onvif.org/ver20/ptz/wsdl</tds:Namespace>"
        "<tds:XAddr>http://10.0.0.5/onvif/ptz_service</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>60</tt:Minor>"
        "</tds:Version></tds:Service></tds:GetServicesResponse>",
        {"services": {"http://www.onvif.org/ver20/ptz/wsdl": {"address": "http://10.0.0.5/onvif/ptz_service"}}})
_ov_tel("relays", "tds",
        '<tds:GetRelayOutputsResponse><tds:RelayOutputs token="RelayOutput_1"><tt:Properties><tt:Mode>Bistable</tt:Mode>'
        "<tt:DelayTime>PT0S</tt:DelayTime><tt:IdleState>open</tt:IdleState></tt:Properties></tds:RelayOutputs>"
        "</tds:GetRelayOutputsResponse>",
        {"relays": {"RelayOutput_1": {"mode": "Bistable", "delay_time": "PT0S", "idle_state": "open"}}})
_ov_tel("profiles", "trt",
        '<trt:GetProfilesResponse><trt:Profiles token="Profile_1" fixed="true"><tt:Name>mainStream</tt:Name>'
        '<tt:VideoSourceConfiguration token="VSC_1"><tt:Name>vsc</tt:Name><tt:UseCount>2</tt:UseCount>'
        "<tt:SourceToken>VideoSource_1</tt:SourceToken></tt:VideoSourceConfiguration>"
        '<tt:VideoEncoderConfiguration token="VEC_1"><tt:Name>enc</tt:Name><tt:Encoding>H264</tt:Encoding>'
        "<tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution></tt:VideoEncoderConfiguration>"
        '<tt:PTZConfiguration token="PTZ_1"><tt:Name>ptz</tt:Name></tt:PTZConfiguration></trt:Profiles>'
        "</trt:GetProfilesResponse>",
        {"profiles": {"Profile_1": {"name": "mainStream", "video_source": "VideoSource_1", "encoding": "H264",
                                    "width": 1920, "height": 1080, "ptz_configuration": "PTZ_1"}}})
_ov_tel("video-sources", "trt",
        '<trt:GetVideoSourcesResponse><trt:VideoSources token="VideoSource_1"><tt:Framerate>30</tt:Framerate>'
        "<tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution></trt:VideoSources>"
        "</trt:GetVideoSourcesResponse>",
        {"video_sources": {"VideoSource_1": {"frame_rate": 30.0, "width": 1920, "height": 1080}}})
_ov_tel("stream-uri", "trt",
        "<trt:GetStreamUriResponse><trt:MediaUri><tt:Uri>rtsp://10.0.0.5:554/stream1</tt:Uri>"
        "<tt:InvalidAfterConnect>false</tt:InvalidAfterConnect></trt:MediaUri></trt:GetStreamUriResponse>",
        {"stream_uri": "rtsp://10.0.0.5:554/stream1"})
_ov_tel("snapshot-uri", "trt",
        "<trt:GetSnapshotUriResponse><trt:MediaUri><tt:Uri>http://10.0.0.5/snap.jpg</tt:Uri></trt:MediaUri>"
        "</trt:GetSnapshotUriResponse>",
        {"snapshot_uri": "http://10.0.0.5/snap.jpg"})
_ov_tel("status", "tptz",
        '<tptz:GetStatusResponse><tptz:PTZStatus><tt:Position><tt:PanTilt x="0.25" y="-0.5" '
        'space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/><tt:Zoom x="0.1"/></tt:Position>'
        "<tt:MoveStatus><tt:PanTilt>MOVING</tt:PanTilt><tt:Zoom>IDLE</tt:Zoom></tt:MoveStatus><tt:Error>NO error</tt:Error>"
        "<tt:UtcTime>2026-10-27T12:00:00Z</tt:UtcTime></tptz:PTZStatus></tptz:GetStatusResponse>",
        {"position": {"pan": 0.25, "tilt": -0.5, "zoom": 0.1}, "move_status": {"pan_tilt": "MOVING", "zoom": "IDLE"},
         "ptz_error": "NO error", "ptz_time": "2026-10-27T12:00:00Z"})
_ov_tel("presets", "tptz",
        '<tptz:GetPresetsResponse><tptz:Preset token="1"><tt:Name>Home</tt:Name><tt:PTZPosition>'
        '<tt:PanTilt x="0" y="0"/><tt:Zoom x="0"/></tt:PTZPosition></tptz:Preset><tptz:Preset token="2">'
        '<tt:Name>Pulpit</tt:Name><tt:PTZPosition><tt:PanTilt x="0.3" y="-0.1"/><tt:Zoom x="0.6"/></tt:PTZPosition>'
        "</tptz:Preset></tptz:GetPresetsResponse>",
        {"presets": {"1": {"name": "Home", "pan": 0.0, "tilt": 0.0, "zoom": 0.0},
                     "2": {"name": "Pulpit", "pan": 0.3, "tilt": -0.1, "zoom": 0.6}}},
        state_before={"presets": {"7": {"name": "Gone"}}})
_ov_tel("nodes", "tptz",
        '<tptz:GetNodesResponse><tptz:PTZNode token="Node_1" FixedHomePosition="false"><tt:Name>PTZ</tt:Name>'
        "<tt:MaximumNumberOfPresets>255</tt:MaximumNumberOfPresets><tt:HomeSupported>true</tt:HomeSupported>"
        "</tptz:PTZNode></tptz:GetNodesResponse>",
        {"nodes": {"Node_1": {"name": "PTZ", "maximum_presets": 255, "home_supported": True}}})
_ov_tel("preset-tours", "tptz",
        '<tptz:GetPresetToursResponse><tptz:PresetTour token="Tour_1"><tt:Name>Sweep</tt:Name>'
        "<tt:Status><tt:State>Touring</tt:State></tt:Status><tt:AutoStart>false</tt:AutoStart></tptz:PresetTour>"
        "</tptz:GetPresetToursResponse>",
        {"preset_tours": {"Tour_1": {"name": "Sweep", "auto_start": False, "state": "Touring"}}})
_ov_tel("imaging-settings", "timg",
        "<timg:GetImagingSettingsResponse><timg:ImagingSettings><tt:BacklightCompensation><tt:Mode>OFF</tt:Mode>"
        "</tt:BacklightCompensation><tt:Brightness>50</tt:Brightness><tt:ColorSaturation>55</tt:ColorSaturation>"
        "<tt:Contrast>50</tt:Contrast><tt:Exposure><tt:Mode>MANUAL</tt:Mode><tt:ExposureTime>20000</tt:ExposureTime>"
        "<tt:Gain>6</tt:Gain><tt:Iris>0</tt:Iris></tt:Exposure><tt:Focus><tt:AutoFocusMode>AUTO</tt:AutoFocusMode>"
        "</tt:Focus><tt:IrCutFilter>AUTO</tt:IrCutFilter><tt:Sharpness>4</tt:Sharpness><tt:WideDynamicRange>"
        "<tt:Mode>ON</tt:Mode><tt:Level>30</tt:Level></tt:WideDynamicRange><tt:WhiteBalance><tt:Mode>MANUAL</tt:Mode>"
        "<tt:CrGain>128</tt:CrGain><tt:CbGain>96</tt:CbGain></tt:WhiteBalance></timg:ImagingSettings>"
        "</timg:GetImagingSettingsResponse>",
        {"imaging": {"brightness": 50.0, "color_saturation": 55.0, "contrast": 50.0, "sharpness": 4.0,
                     "ir_cut_filter": "AUTO", "backlight_compensation": {"mode": "OFF"},
                     "wide_dynamic_range": {"mode": "ON", "level": 30.0},
                     "exposure": {"mode": "MANUAL", "time": 20000.0, "gain": 6.0, "iris": 0.0},
                     "focus_mode": "AUTO", "white_balance": {"mode": "MANUAL", "cr_gain": 128.0, "cb_gain": 96.0}}})
_ov_tel("imaging-options", "timg",
        "<timg:GetOptionsResponse><timg:ImagingOptions><tt:Brightness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Brightness>"
        "<tt:Exposure><tt:Mode>AUTO</tt:Mode><tt:Mode>MANUAL</tt:Mode><tt:ExposureTime><tt:Min>33</tt:Min><tt:Max>40000</tt:Max>"
        "</tt:ExposureTime><tt:Gain><tt:Min>0</tt:Min><tt:Max>36</tt:Max></tt:Gain></tt:Exposure><tt:WhiteBalance>"
        "<tt:Mode>AUTO</tt:Mode><tt:YrGain><tt:Min>0</tt:Min><tt:Max>255</tt:Max></tt:YrGain></tt:WhiteBalance>"
        "</timg:ImagingOptions></timg:GetOptionsResponse>",
        {"imaging_options": {"brightness": {"min": 0.0, "max": 100.0}, "exposure_time": {"min": 33.0, "max": 40000.0},
                             "gain": {"min": 0.0, "max": 36.0}, "cr_gain": {"min": 0.0, "max": 255.0}}})
_ov_tel("focus-status", "timg",
        "<timg:GetStatusResponse><timg:Status><tt:FocusStatus20><tt:Position>0.42</tt:Position>"
        "<tt:MoveStatus>IDLE</tt:MoveStatus></tt:FocusStatus20></timg:Status></timg:GetStatusResponse>",
        {"focus": {"position": 0.42, "move_status": "IDLE"}})
_ov_tel("imaging-presets", "timg",
        '<timg:GetPresetsResponse><timg:Preset token="Preset_1" type="Custom"><tt:Name>Default</tt:Name></timg:Preset>'
        '<timg:Preset token="Preset_2" type="Outdoor"><tt:Name>Outdoor</tt:Name></timg:Preset></timg:GetPresetsResponse>',
        {"imaging_presets": {"Preset_1": {"type": "Custom", "name": "Default"},
                             "Preset_2": {"type": "Outdoor", "name": "Outdoor"}}})
_ov_tel("current-imaging-preset", "timg",
        '<timg:GetCurrentPresetResponse><timg:Preset token="Preset_2" type="Outdoor"><tt:Name>Outdoor</tt:Name>'
        "</timg:Preset></timg:GetCurrentPresetResponse>",
        {"imaging_preset": "Preset_2"})
