RX4 = "roland-xs42h"
# Roland XS-42H / VP-42H (Reference Manual v1.3, edition 03, p.28-29):
# "set,category,ID,sub-ID,value", "get,category,ID,sub-ID" and "ver", each
# ended LF; answered "ack,..." (a get then sends the value as "set,...") or
# "err,...". Scenes, outputs, inputs and layers are 0-based on the wire;
# scene 15 is the currently selected scene.
RX4_OK = {"ok": {"kind": "ack"}}
RX4_VP = {"model": "vp-42h"}


def rx4(command, input, wire, **extra):
    text(RX4, command, input, wire, **extra)


rx4("set_transition_time", {"time": 4000}, "set,10,19,0,4000\n", device_reply="ack,10,19,0,4000\n", expect_result=RX4_OK)
rx4("recall_scene", {"scene": 10}, "set,11,11,0,9\n", device_reply="err,set,11,11,0,9\n",
    expect_result={"error": {"error": "device_error"}})
rx4("select_output_input", {"output": 2, "input": 5}, "set,11,10,1,4\n")
rx4("get_scene", {}, "get,10,20,0\n", device_reply="ack,10,20,0\nset,10,20,0,3\n", expect_result=RX4_OK)
rx4("get_output_input", {"scene": 1, "output": 1}, "get,0,0,0\n")
rx4("set_mixer1_input_level", {"scene": 2, "input": 4, "level": 127}, "set,1,1,3,127\n")
rx4("set_mixer1_input_level_current", {"input": 1, "level": 0}, "set,15,1,0,0\n")
rx4("set_mixer1_level", {"scene": 10, "level": 100}, "set,9,2,0,100\n")
rx4("set_mixer1_level_current", {"level": 64}, "set,15,2,0,64\n")
rx4("set_mixer2_input_level", {"scene": 1, "input": 2, "level": 90}, "set,0,6,1,90\n")
rx4("set_mixer2_input_level_current", {"input": 3, "level": 10}, "set,15,6,2,10\n")
rx4("set_mixer2_level", {"scene": 5, "level": 127}, "set,4,7,0,127\n")
rx4("set_mixer2_level_current", {"level": 1}, "set,15,7,0,1\n")
rx4("set_layer_visible", {"scene": 1, "layer": 4, "enabled": True}, "set,0,41,3,1\n", **RX4_VP)
rx4("set_layer_visible_current", {"layer": 1, "enabled": False}, "set,15,41,0,0\n", **RX4_VP)
rx4("set_layer_input", {"scene": 3, "layer": 2, "input": 5}, "set,2,42,1,4\n", **RX4_VP)
rx4("set_layer_input_current", {"layer": 1, "input": 1}, "set,15,42,0,0\n", **RX4_VP)
rx4("set_input_level", {"scene": 1, "input": 1, "level": 100}, "set,0,1,0,100\n", **RX4_VP)
rx4("set_input_level_current", {"input": 2, "level": 50}, "set,15,1,1,50\n", **RX4_VP)
rx4("set_master_level", {"scene": 10, "level": 0}, "set,9,2,0,0\n", **RX4_VP)
rx4("set_master_level_current", {"level": 127}, "set,15,2,0,127\n", **RX4_VP)
rx4("set_parameter", {"category": 11, "id": 11, "sub_id": 0, "value": 2}, "set,11,11,0,2\n")
rx4("get_parameter", {"category": 0, "id": 0, "sub_id": 1}, "get,0,0,1\n")
rx4("get_version", {}, "ver\n", device_reply="ack,ver\nver,XS-42H,1.30\n", expect_result=RX4_OK)

telemetry(RX4, "scene", inbound="set,10,20,0,3\n",
          expect_state={"scene": 4, "parameters": {"10-20-0": 3}})
telemetry(RX4, "scene-recalled", inbound="ack,11,11,0,9\n",
          expect_state={"scene": 10, "parameters": {"11-11-0": 9}})
telemetry(RX4, "output-input", inbound="ack,11,10,1,4\n",
          expect_state={"outputs": {"2": {"input": 5}}, "parameters": {"11-10-1": 4}})
telemetry(RX4, "parameter", inbound="set,0,0,1,2\n", expect_state={"parameters": {"0-0-1": 2}})
telemetry(RX4, "version", inbound="ver,XS-42H,1.30\n",
          expect_state={"device": {"model": "XS-42H", "version": "1.30"}})
