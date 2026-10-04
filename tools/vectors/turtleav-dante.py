# Turtle AV Dante family on TCP 8000: lower-case "set"/"get" commands, one per
# line (CR LF assumed), replies "Label: value" (Mineola 2x2 manual §7, Phoenix
# 8x8 §7, Downtown §9, 30W amplifier §9, Bluetooth wall plate §7, 150W
# amplifier §8). Wire forms follow the documents' examples: "set master vol
# 50", "set input 1 gain 10", "set input 1 mute on", "set output 5 vol 50",
# "set output 5 delay 50", "set preset recall 1", "set bt vol 10", "s input 1",
# "s master vol 50".
TD = "turtleav-dante"
PH = {"model": "phoenix-8x8"}
DT = {"model": "downtown"}
A30 = {"model": "amp-30w"}


def td(command, input, line, **extra):
    text(TD, command, input, line + "\r\n", **extra)


td("set_master_volume", {"volume": 50}, "set master vol 50",
   device_reply="Master volume: 50\r\n", expect_result={"ok": {"kind": "ack"}})
td("step_master_volume", {"direction": "+", "step": 5}, "set master vol+5")
td("set_master_mute", {"state": "on"}, "set master mute on")
td("get_master_volume", {}, "get master vol", device_reply="Master volume: 42\r\n",
   expect_result={"ok": {"kind": "value", "value": "42"}})
td("set_input_gain", {"input": 1, "gain": 10.0}, "set input 1 gain 10.0")
td("set_input_mute", {"input": 1, "state": "on"}, "set input 1 mute on")
td("set_input_phantom", {"input": 3, "state": "on"}, "set input 3 phantom power on")
td("set_input_sensitivity", {"input": 3, "level": 1}, "set input 3 sensitivity 1")
td("set_output_volume", {"output": 5, "volume": 50}, "set output 5 vol 50")
td("set_output_mute", {"output": 0, "state": "off"}, "set output 0 mute off")
td("set_output_delay", {"output": 5, "ms": 50}, "set output 5 delay 50")
td("set_output_level", {"output": 3, "level": 1}, "set output 3 gain 1")
td("recall_preset", {"preset": 1}, "set preset recall 1", device_reply="Recall preset 1\r\n",
   expect_result={"ok": {"kind": "ack"}})
td("save_preset", {"preset": 1}, "set preset save 1")
td("set_power", {"state": "on"}, "set power on")
td("set_power_phoenix", {"state": "off"}, "power off", **PH)
td("reboot", {}, "set reboot")
td("reboot_phoenix", {}, "reboot", **PH)
td("set_audio_source", {"source": 2}, "set audio source 2", **DT)
td("set_output_video", {"mode": 3}, "set output 1 video 3", **DT)
td("set_drc", {"mode": 3}, "set drc 3", **DT)
td("set_speaker_mode", {"mode": 14}, "set speaker mode 14", **DT)
td("set_output_route", {"output": 5, "source": 3}, "set output 5 from 3", device_reply="Error: invalid\r\n",
   expect_result={"error": {"error": "device_error"}}, **A30)

telemetry(TD, "master", inbound="Master volume: 50\r\n", expect_state={"master": {"volume": 50}})
telemetry(TD, "master-mute", inbound="Master mute: on\r\n", expect_state={"master": {"mute": True}})
telemetry(TD, "input-gain", inbound="XLR IN1 gain: 10dB\r\n", expect_state={"inputs": {"1": {"gain": 10.0}}})
telemetry(TD, "input-mute", inbound="LINE IN3 mute: off\r\n", expect_state={"inputs": {"3": {"mute": False}}})
telemetry(TD, "output-volume", inbound="XLR OUT5 volume: 50\r\n", expect_state={"outputs": {"5": {"volume": 50}}})
telemetry(TD, "output-delay", inbound="XLR OUT5 delay: 50ms\r\n", expect_state={"outputs": {"5": {"delay_ms": 50}}})
telemetry(TD, "preset", inbound="Recall preset 1\r\n", expect_state={"preset": {"recalled": 1}})

TB = "turtleav-bt-wallplate"


def tb(command, input, line, **extra):
    text(TB, command, input, line + "\r\n", **extra)


tb("set_bt_volume", {"volume": 10}, "set bt vol 10", device_reply="Bluetooth volume: 10\r\n",
   expect_result={"ok": {"kind": "ack"}})
tb("set_bt_mute", {"state": "on"}, "set bt mute on")
tb("set_bt_pairing", {"state": "on"}, "set bt discoverable/pairing on")
tb("clear_bt_pairing", {"device": 0}, "set bt paired clear 0")
tb("bt_transport", {"action": "play/pause"}, "set bt play/pause")
tb("set_input_gain", {"input": 1, "gain": 10.0}, "set input 1 gain 10.0")
tb("set_input_mute", {"input": 1, "state": "on"}, "set input 1 mute on")
tb("get_bt_connection", {}, "get bt connection", device_reply="Connected device1: Connected\r\n",
   expect_result={"ok": {"kind": "value", "value": "Connected device1: Connected"}})
tb("reboot", {}, "set reboot")
telemetry(TB, "volume", inbound="Bluetooth volume: 50\r\n", expect_state={"bt": {"volume": 50}})
telemetry(TB, "input", inbound="Bluetooth input left gain: 10dB\r\n", expect_state={"inputs": {"left": {"gain": 10.0}}})
telemetry(TB, "connection", inbound="Connected device2: Disconnected\r\n",
          expect_state={"bt": {"devices": {"2": {"connected": False}}}})

TP = "turtleav-amp150"


def tp(command, input, line, **extra):
    text(TP, command, input, line + "\r\n", **extra)


tp("set_master_volume", {"volume": 50}, "s master vol 50")
tp("set_master_mute", {"state": "off"}, "s master mute off")
tp("select_input", {"input": 1}, "s input 1", device_reply="Set input: Line\r\n", expect_result={"ok": {"kind": "ack"}})
tp("set_input_volume", {"input": 1, "volume": 50}, "s input 1 vol 50")
tp("set_output_volume", {"output": 1, "volume": 60}, "s output 1 vol 60")
tp("set_output_mute", {"output": 0, "state": "on"}, "s output 0 mute on")
tp("recall_preset", {"preset": 2}, "s preset recall 2")
tp("save_preset", {"preset": 2}, "s preset save 2")
tp("set_power", {"state": "on"}, "s power on")
tp("reboot", {}, "s reboot")
telemetry(TP, "input", inbound="Input: Dante\r\n", expect_state={"input": "Dante"})
telemetry(TP, "master", inbound="Master volume: 35\r\n", expect_state={"master": {"volume": 35}})
