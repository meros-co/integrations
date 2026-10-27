# RED RCP2 (red-rcp2): one vector per command. RED's own RCP2 documents are
# behind a login, so the expected messages are written by hand from the
# message forms of the public integration the spec cites (rcp_set with id and
# value, rcp_get, rcp_subscribe with on_off, rcp_get_parameters), each one
# JSON text message on the WebSocket.
RD = "red-rcp2"


def _rd(command, input, message, **extra):
    text(RD, command, input, message, **extra)


def _set(id_, value):
    return '{"type":"rcp_set","id":"' + id_ + '","value":' + value + "}"


_rd("set_parameter", {"id": "AUDIO_HEADPHONE_MUTE", "value": 1}, _set("AUDIO_HEADPHONE_MUTE", "1"))
_rd("set_parameter_text", {"id": "SLATE_SCENE", "value": "12A"}, _set("SLATE_SCENE", '"12A"'))
_rd("get_parameter", {"id": "ISO"}, '{"type":"rcp_get","id":"ISO"}',
    device_reply='{"type":"rcp_cur_int","id":"ISO","cur":{"val":800},"display":{"str":"800"}}',
    expect_result={"ok": {"kind": "value", "value": {"type": "rcp_cur_int", "id": "ISO", "cur": {"val": 800},
                                                      "display": {"str": "800"}}}})
_rd("subscribe_parameter", {"id": "ND"}, '{"type":"rcp_subscribe","id":"ND","on_off":true}')
_rd("unsubscribe_parameter", {"id": "ND"}, '{"type":"rcp_subscribe","id":"ND","on_off":false}')
_rd("get_parameters", {}, '{"type":"rcp_get_parameters"}')
_rd("get_camera_info", {}, '{"type":"rcp_get","id":"CAMERA_INFO"}')
_rd("start_recording", {}, _set("RECORD_STATE", "1"))
_rd("stop_recording", {}, _set("RECORD_STATE", "0"))
_rd("toggle_recording", {}, _set("RECORD_STATE", "2"))
_rd("set_iso", {"iso": 800}, _set("ISO", "800"))
_rd("set_color_temperature", {"kelvin": 5600}, _set("COLOR_TEMPERATURE", "5600"))
_rd("set_exposure_adjust", {"millistops": -1500}, _set("EXPOSURE_ADJUST", "-1500"))
_rd("set_sensor_frame_rate", {"millifps": 23976}, _set("SENSOR_FRAME_RATE", "23976"))
_rd("set_record_format", {"format": "4K 16:9"}, _set("RECORD_FORMAT", "18"))
_rd("set_camera_id", {"camera_id": "A001"}, _set("CAMERA_ID", '"A001"'))
_rd("set_reel_number", {"reel": 12}, _set("REEL_NUMBER", "12"))
_rd("set_camera_position", {"letter": 1}, _set("CAMERA_POSITION", "1"))
_rd("set_lut_sdi_1", {"enabled": True}, _set("ENABLE_CAMERA_LUT_SDI_1", "1"))
_rd("set_lut_sdi_2", {"enabled": False}, _set("ENABLE_CAMERA_LUT_SDI_2", "0"), model="v-raptor")
_rd("set_magnify", {"output": "SDI_1", "enabled": True}, _set("MAGNIFY_ENABLE_SDI_1", "1"))
_rd("set_external_tally_state", {"state": 1}, _set("EXTERNAL_TALLY_STATE", "1"))
_rd("set_external_tally_color", {"tally": 1, "color": 12517376}, _set("EXTERNAL_TALLY_1_COLOR", "12517376"))
_rd("set_external_tally_opacity", {"opacity": "100"}, _set("EXTERNAL_TALLY_OPACITY", "3"))
_rd("set_external_tally_style", {"style": "bracket"}, _set("EXTERNAL_TALLY_STYLE", "2"))
_rd("set_external_tally_thickness", {"thickness": "medium"}, _set("EXTERNAL_TALLY_THICKNESS", "1"))
_rd("set_tally_led", {"enabled": True}, _set("TALLY_LED_ENABLE", "1"))
_rd("restart", {}, _set("SHUTDOWN", "1"))
_rd("shutdown", {}, _set("SHUTDOWN", "2"))

telemetry(RD, "cur-int", inbound_ws='{"type":"rcp_cur_int","id":"ISO","cur":{"val":1600},"display":{"str":"1600"}}',
          expect_state={"parameters": {"ISO": {"value": 1600, "display": "1600"}}})
telemetry(RD, "cur-str", inbound_ws='{"type":"rcp_cur_str","id":"TIMECODE","display":{"str":"01:00:00:00"}}',
          expect_state={"parameters": {"TIMECODE": {"display": "01:00:00:00"}}})
telemetry(RD, "record-state", inbound_ws='{"type":"rcp_cur_int","id":"RECORD_STATE","cur":{"val":1},"display":{"str":"Recording"}}',
          expect_state={"parameters": {"RECORD_STATE": {"value": 1, "display": "Recording"}}, "recording": "recording"})
telemetry(RD, "camera-info", inbound_ws=(
    '{"type":"rcp_cur_cam_info","name":"A-CAM","serial_number":"130-1234","camera_type":{"str":"KOMODO 6K","num":50},'
    '"version":{"str":"1.7.5"}}'),
    expect_state={"camera": {"name": "A-CAM", "serial": "130-1234", "type": "KOMODO 6K", "type_number": 50,
                             "firmware": "1.7.5"}})
telemetry(RD, "parameters", inbound_ws='{"type":"rcp_cur_parameters","parameters":["RCP_PARAM_ISO","RCP_PARAM_ND"]}',
          expect_state={"supported_parameters": '["RCP_PARAM_ISO","RCP_PARAM_ND"]'})
telemetry(RD, "session", inbound_ws='{"type":"rcp_session","status":"open","data":12345}', expect_state={},
          expect_then_send=['{"type":"rcp_session","data":12345}'])
