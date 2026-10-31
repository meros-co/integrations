RV4 = "roland-vr400uhd"
# Roland VR-400UHD (Remote Control Guide edition 01 p.2-3): "set,category,ID,
# sub-ID,value", "get,category,ID,sub-ID" and "ver", each ended LF; answered
# "ack,..." or "err,<transmitted command>". Wire values are 0-based.
RV4_OK = {"ok": {"kind": "ack"}}


def rv4(command, input, wire, **extra):
    text(RV4, command, input, wire, **extra)


rv4("select_program_scene", {"scene": 1}, "set,97,46,0,0\n", device_reply="ack,97,46,0,0\n", expect_result=RV4_OK)
rv4("select_preset_scene", {"scene": 64}, "set,97,46,1,63\n", device_reply="err,set,97,46,1,63\n",
    expect_result={"error": {"error": "device_error"}})
rv4("cut", {}, "set,98,43,0,1\n", device_reply="ack,98,43,0,1\n", expect_result=RV4_OK)
rv4("auto", {}, "set,98,42,0,1\n")
rv4("set_output_fade", {"program": 2, "enabled": True}, "set,97,25,1,1\n")
rv4("set_dsk", {"enabled": True}, "set,97,79,0,1\n")
rv4("set_logo", {"enabled": False}, "set,97,63,0,0\n")
rv4("set_transition_type", {"type": 1}, "set,97,48,0,1\n")
rv4("set_wipe_pattern", {"pattern": 4}, "set,97,49,0,3\n")
rv4("set_transition_time", {"time": 20}, "set,97,18,0,20\n")
rv4("set_auto_transition", {"enabled": True}, "set,97,19,0,1\n")
rv4("set_mic_level", {"channel": 1, "level": 127}, "set,97,139,0,127\n")
rv4("set_mic_solo", {"channel": 2, "enabled": True}, "set,97,138,1,1\n")
rv4("set_mic_mute", {"channel": 6, "enabled": True}, "set,97,137,5,1\n")
rv4("set_mic_aux_send", {"channel": 3, "level": 64}, "set,97,144,2,64\n")
rv4("set_mic_pan", {"channel": 4, "pan": -50}, "set,97,140,3,-50\n")
rv4("set_mic_reverb_send", {"channel": 5, "level": 10}, "set,97,141,4,10\n")
rv4("set_mic_usb_send", {"channel": 1, "enabled": False}, "set,97,143,0,0\n")
rv4("set_mic_aux_post", {"channel": 1, "post": True}, "set,97,145,0,1\n")
rv4("set_line_level", {"channel": 3, "level": 100}, "set,97,173,2,100\n")
rv4("set_line_solo", {"channel": 1, "enabled": True}, "set,97,172,0,1\n")
rv4("set_line_mute", {"channel": 4, "enabled": True}, "set,97,171,3,1\n")
rv4("set_line_aux_send", {"channel": 2, "level": 0}, "set,97,178,1,0\n")
rv4("set_line_pan", {"channel": 2, "pan": 50}, "set,97,174,1,50\n")
rv4("set_line_reverb_send", {"channel": 1, "level": 30}, "set,97,175,0,30\n")
rv4("set_line_usb_send", {"channel": 4, "enabled": True}, "set,97,177,3,1\n")
rv4("set_line_aux_post", {"channel": 3, "post": False}, "set,97,179,2,0\n")
rv4("set_output_level", {"output": 1, "level": 100}, "set,97,182,0,100\n")
rv4("set_output_solo", {"output": 2, "enabled": True}, "set,97,181,1,1\n")
rv4("set_output_mute", {"output": 4, "enabled": True}, "set,97,180,3,1\n")
rv4("set_auto_mixing", {"enabled": True}, "set,97,265,0,1\n")
rv4("set_audio_follow_video", {"mode": 2}, "set,97,217,0,2\n")
rv4("set_reverb", {"level": 127}, "set,97,216,0,127\n")
rv4("set_parameter", {"category": 97, "id": 46, "sub_id": 0, "value": 9}, "set,97,46,0,9\n")
rv4("get_parameter", {"category": 97, "id": 46, "sub_id": 0}, "get,97,46,0\n",
    device_reply="ack,97,46,0\nset,97,46,0,9\n", expect_result=RV4_OK)
rv4("get_version", {}, "ver\n")

telemetry(RV4, "program-scene", inbound="set,97,46,0,9\n",
          expect_state={"program": {"scene": 10}, "parameters": {"97-46-0": 9}})
telemetry(RV4, "preset-scene", inbound="ack,97,46,1,0\n",
          expect_state={"preset": {"scene": 1}, "parameters": {"97-46-1": 0}})
telemetry(RV4, "parameter", inbound="ack,99,1,0,5\n",
          expect_state={"parameters": {"99-1-0": 5}})
telemetry(RV4, "version", inbound="ver,VR-400UHD,1.20\n",
          expect_state={"device": {"model": "VR-400UHD", "version": "1.20"}})

# Every parameter of the command list (p.3), read with get and answered as a
# set line, or acknowledged with the value set; 0-based wire numbers.
def _rv4(name, line, state):
    category, pid, sub, value = line.split(",")[1:]
    telemetry(RV4, name, inbound=line + "\n",
              expect_state={**state, "parameters": {f"{category}-{pid}-{sub}": int(value)}})


_rv4("output-fade-pgm2", "set,97,25,1,1", {"programs": {"2": {"output_fade": True}}})
_rv4("dsk", "set,97,79,0,1", {"dsk": True})
_rv4("logo", "ack,97,63,0,0", {"logo": False})
_rv4("transition-type", "set,97,48,0,1", {"transition": {"type": 1}})
_rv4("wipe-pattern", "set,97,49,0,3", {"transition": {"wipe_pattern": 4}})
_rv4("transition-time", "set,97,18,0,20", {"transition": {"time": 20}})
_rv4("auto-transition", "set,97,19,0,1", {"transition": {"auto": True}})
_rv4("mic-mute", "set,97,137,5,1", {"mics": {"6": {"mute": True}}})
_rv4("mic-solo", "set,97,138,0,0", {"mics": {"1": {"solo": False}}})
_rv4("mic-level", "set,97,139,2,100", {"mics": {"3": {"level": 100}}})
_rv4("mic-pan", "ack,97,140,3,-50", {"mics": {"4": {"pan": -50}}})
_rv4("mic-reverb", "set,97,141,1,64", {"mics": {"2": {"reverb_send": 64}}})
_rv4("mic-usb", "set,97,143,4,1", {"mics": {"5": {"usb_send": True}}})
_rv4("mic-aux", "set,97,144,0,127", {"mics": {"1": {"aux_send": 127}}})
_rv4("mic-aux-post", "set,97,145,0,1", {"mics": {"1": {"aux_post": True}}})
_rv4("line-mute", "set,97,171,3,1", {"lines": {"4": {"mute": True}}})
_rv4("line-solo", "set,97,172,2,1", {"lines": {"3": {"solo": True}}})
_rv4("line-level", "set,97,173,0,90", {"lines": {"1": {"level": 90}}})
_rv4("line-pan", "set,97,174,1,25", {"lines": {"2": {"pan": 25}}})
_rv4("line-reverb", "set,97,175,0,10", {"lines": {"1": {"reverb_send": 10}}})
_rv4("line-usb", "set,97,177,2,0", {"lines": {"3": {"usb_send": False}}})
_rv4("line-aux", "set,97,178,3,40", {"lines": {"4": {"aux_send": 40}}})
_rv4("line-aux-post", "set,97,179,1,0", {"lines": {"2": {"aux_post": False}}})
_rv4("output-mute", "set,97,180,0,1", {"outputs": {"1": {"mute": True}}})
_rv4("output-solo", "set,97,181,1,0", {"outputs": {"2": {"solo": False}}})
_rv4("output-level", "set,97,182,3,110", {"outputs": {"4": {"level": 110}}})
_rv4("auto-mixing", "set,97,265,0,1", {"audio": {"auto_mixing": True}})
_rv4("follow-video", "set,97,217,0,2", {"audio": {"follow_video": 2}})
_rv4("reverb", "set,97,216,0,70", {"audio": {"reverb": 70}})
