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
tb("set_bt_name", {"name": "ABC"}, "set bt name <ABC>", device_reply="Set Bluetooth device name to ABC\r\n",
   expect_result={"ok": {"kind": "ack"}})
tb("step_bt_volume", {"direction": "+", "step": 5}, "set bt vol+5")
tb("set_bt_window_time", {"seconds": 60}, "set bt window time 60")
tb("set_bt_window_switch", {"state": "on"}, "set bt window switch on")
tb("set_bt_audio_bridging", {"mode": 1}, "set bt audio bridging 1")
tb("get_bt_device", {}, "get bt device", device_reply="Connected device1: HUAWEI P30 Pro\r\n",
   expect_result={"ok": {"kind": "value", "value": "Connected device1: HUAWEI P30 Pro"}})
tb("set_input_stereo", {"state": "on"}, "set input 1 stereo on")
tb("step_input_gain", {"input": 1, "direction": "-", "step": 5.0}, "set input 1 gain-5.0")
tb("set_eq_band_enabled", {"input": 1, "band": 0, "state": "on"}, "set 1 eq 0 on")
tb("set_eq_stereo", {"state": "on"}, "set 1 eq stereo on")
tb("set_eq_band", {"input": 1, "band": 1, "type": 1, "frequency": 200, "gain": -15.0, "q": 0.02},
   "set 1 eq 1 typ 1 frq 200 val -15.0 q 0.02")
tb("clear_eq", {"input": 1}, "set 1 eq clear")
tb("copy_eq", {"from": 1, "to": 2}, "set 1 eq copy to 2")
tb("set_ip_mode", {"mode": 0}, "set ip mode 0")
tb("set_ip_address", {"address": "192.168.1.100"}, "set ip addr 192.168.1.100")
tb("set_subnet", {"mask": "255.255.255.0"}, "set subnet 255.255.255.0")
tb("set_gateway", {"gateway": "192.168.1.1"}, "set gateway 192.168.1.1")
tb("set_tcp_port", {"port": 8000}, "set tcp/ipport 8000")
tb("set_telnet_port", {"port": 23}, "set telnet port 23")
tb("set_hostname", {"hostname": "WallPlate-1"}, "set net hostname WallPlate-1")
tb("net_reboot", {}, "set net reboot")
telemetry(TB, "mute", inbound="Bluetooth mute: on\r\n", expect_state={"bt": {"mute": True}})
telemetry(TB, "name", request="get bt name", inbound="Bluetooth Adapter-123456\r\n",
          expect_state={"bt": {"name": "Bluetooth Adapter-123456"}})
telemetry(TB, "name-set", inbound="Set Bluetooth device name to ABC\r\n", expect_state={"bt": {"name": "ABC"}})
telemetry(TB, "pairing", inbound="Bluetooth discoverable/pairing on (60s)\r\n", expect_state={"bt": {"pairing": True}})
telemetry(TB, "window-time", inbound="Bluetooth window time: 60s\r\n", expect_state={"bt": {"window_s": 60}})
telemetry(TB, "window-switch", inbound="Bluetooth window switch: on\r\n", expect_state={"bt": {"window_switch": True}})
telemetry(TB, "bridging", inbound="Bluetooth audio bridging: Call bridging\r\n", expect_state={"bt": {"bridging": 1}})
telemetry(TB, "format", inbound="Bluetooth format: SBC\r\n", expect_state={"bt": {"format": "SBC"}})
telemetry(TB, "artist", inbound="Artist: Someone\r\n", expect_state={"bt": {"artist": "Someone"}})
telemetry(TB, "album", inbound="Album: Something\r\n", expect_state={"bt": {"album": "Something"}})
telemetry(TB, "track", inbound="Track: A Song\r\n", expect_state={"bt": {"track": "A Song"}})
telemetry(TB, "stereo", inbound="Bluetooth input left/right stereo mode: on\r\n",
          expect_state={"stereo": {"gain_linked": True}})
telemetry(TB, "input-mute", inbound="Bluetooth input right mute: on\r\n", expect_state={"inputs": {"right": {"mute": True}}})
telemetry(TB, "eq", inbound="Bluetooth input left EQ: on\r\n", expect_state={"inputs": {"left": {"eq": {"enabled": True}}}})
telemetry(TB, "eq-band", inbound="Bluetooth input left EQ 3: off\r\n",
          expect_state={"inputs": {"left": {"eq": {"bands": {"3": {"enabled": False}}}}}})
telemetry(TB, "eq-stereo", inbound="Bluetooth input left/right EQ stereo mode: on\r\n",
          expect_state={"stereo": {"eq_linked": True}})
telemetry(TB, "eq-band-setting", inbound="Bluetooth input left EQ 1: Type: 3, Frequency: 200Hz, Value: -15dB, Q: 0.02\r\n",
          expect_state={"inputs": {"left": {"eq": {"bands": {"1": {
              "type": 3, "frequency": 200.0, "gain": -15.0, "q": 0.02}}}}}})
telemetry(TB, "ip-mode", inbound="IP mode: Static\r\n", expect_state={"network": {"ip_mode": "static"}})
telemetry(TB, "ip", inbound="IP: 192.168.0.100\r\n", expect_state={"network": {"ip": "192.168.0.100"}})
telemetry(TB, "subnet", inbound="Subnet mask: 255.255.255.0\r\n", expect_state={"network": {"subnet": "255.255.255.0"}})
telemetry(TB, "gateway", inbound="Gateway: 192.168.1.1\r\n", expect_state={"network": {"gateway": "192.168.1.1"}})
telemetry(TB, "tcp-port", inbound="TCP/IP port: 8000\r\n", expect_state={"network": {"tcp_port": 8000}})
telemetry(TB, "telnet-port", inbound="Telnet port: 23\r\n", expect_state={"network": {"telnet_port": 23}})
telemetry(TB, "mac", inbound="MAC: 6C:DF:FB:00:03:56\r\n", expect_state={"network": {"mac": "6C:DF:FB:00:03:56"}})
telemetry(TB, "hostname", inbound="Hostname: 1234\r\n", expect_state={"network": {"hostname": "1234"}})

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
tp("set_auto_standby", {"minutes": 10}, "s auto stb 10")
tp("set_lcd", {"mode": "on 15"}, "s lcd on 15")
tp("set_id_led", {"mode": "off"}, "s idled off")
tp("set_trigger", {"mode": "on 1"}, "s trigger on 1")
tp("set_baud_rate", {"baud": "115200"}, "s rsb 115200")
tp("set_fan", {"fan": 0, "state": "on"}, "s fan 0 on")
tp("step_input_volume", {"input": 1, "direction": "+"}, "s input 1 vol+")
tp("set_input_mute", {"input": 1, "state": "on"}, "s input 1 mute on",
   device_reply="Set input line mute on\r\n", expect_result={"ok": {"kind": "ack"}})
tp("set_master_member", {"members": "111"}, "s master member 111")
tp("step_master_volume", {"direction": "-"}, "s master vol-")
tp("step_output_volume", {"output": 1, "direction": "+"}, "s output 1 vol+")
tp("set_output_mix", {"output": 1, "mix": 1}, "s output 1 mix 1")
tp("set_output_delay", {"output": 1, "ms": 50}, "s output 1 delay 50")
tp("set_output_geq", {"output": 1, "band": 1, "value": 10}, "s output 1 eq 1 val 10")
tp("set_output_geq_preset", {"output": 1, "preset": 1}, "s output 1 eq preset 1")
tp("clear_output_geq", {"output": 1}, "s output 1 eq clear")
tp("clear_preset", {"preset": 1}, "s preset clear 1")
tp("set_preset_name", {"preset": 1, "name": "MeetingRoom 1"}, "s preset 1 name MeetingRoom 1")
tp("set_ip_mode", {"mode": 0}, "s ip mode 0")
tp("set_ip_address", {"address": "192.168.1.100"}, "s ip addr 192.168.1.100")
tp("set_subnet", {"mask": "255.255.255.0"}, "s subnet 255.255.255.0")
tp("set_gateway", {"gateway": "192.168.1.1"}, "s gateway 192.168.1.1")
tp("set_tcp_port", {"port": 8000}, "s tcp/ipport 8000")
tp("set_telnet_port", {"port": 23}, "s telnet port 23")
tp("net_reboot", {}, "s net reboot")
telemetry(TP, "input", inbound="Input: Dante\r\n", expect_state={"input": "Dante"})
telemetry(TP, "master", inbound="Master volume: 35\r\n", expect_state={"master": {"volume": 35}})
telemetry(TP, "master-set", inbound="Set master volume: 50\r\n", expect_state={"master": {"volume": 50}})
telemetry(TP, "master-mute", inbound="Master mute on\r\n", expect_state={"master": {"mute": True}})
telemetry(TP, "master-member", inbound="Set master member: 111\r\n", expect_state={"master": {"member": "111"}})
telemetry(TP, "input-volume", inbound="Input line volume: 50\r\n", expect_state={"inputs": {"1": {"volume": 50}}})
telemetry(TP, "input-mute", inbound="Set input line mute on\r\n", expect_state={"inputs": {"1": {"mute": True}}})
telemetry(TP, "output-volume", inbound="Increase output speaker volume: 52\r\n",
          expect_state={"outputs": {"1": {"volume": 52}}})
telemetry(TP, "output-mute", inbound="Output speaker mute on\r\n", expect_state={"outputs": {"1": {"mute": True}}})
telemetry(TP, "output-mix", inbound="Output speaker mix: Stereo\r\n", expect_state={"outputs": {"1": {"mix": 1}}})
telemetry(TP, "output-delay", inbound="Set output speaker delay: 50ms\r\n",
          expect_state={"outputs": {"1": {"delay_ms": 50}}})
telemetry(TP, "output-geq", inbound="Output speaker GEQ index 1: 20dB\r\n",
          expect_state={"outputs": {"1": {"geq": {"1": 20}}}})
telemetry(TP, "output-geq-preset", inbound="Set output speaker GEQ: Flat\r\n",
          expect_state={"outputs": {"1": {"geq_preset": 1}}})
telemetry(TP, "power", inbound="Power off\r\n", expect_state={"power": False})
telemetry(TP, "auto-standby", inbound="Auto standby time: 10mins\r\n", expect_state={"auto_standby_min": 10})
telemetry(TP, "lcd", inbound="LCD light always on\r\n", expect_state={"lcd": "always on"})
telemetry(TP, "id-led", inbound="Set ID LED light on 15s\r\n", expect_state={"id_led": "on 15s"})
telemetry(TP, "trigger", inbound="Trigger on with high level\r\n", expect_state={"trigger": "high"})
telemetry(TP, "trigger-off", inbound="Set trigger off\r\n", expect_state={"trigger": "off"})
telemetry(TP, "baud", inbound="Baud rate 115200\r\n", expect_state={"baud": 115200})
telemetry(TP, "fans", inbound="All fan on\r\n", expect_state={"fans_auto": True})
telemetry(TP, "preset-name", inbound="Preset 1 name: MeetingRoom 1\r\n",
          expect_state={"presets": {"1": {"name": "MeetingRoom 1"}}})
telemetry(TP, "ip-mode", inbound="IP mode: DHCP\r\n", expect_state={"network": {"ip_mode": "dhcp"}})
telemetry(TP, "ip", inbound="IP address: 192.168.0.100\r\n", expect_state={"network": {"ip": "192.168.0.100"}})
telemetry(TP, "subnet", inbound="Subnet Mask: 255.255.255.0\r\n", expect_state={"network": {"subnet": "255.255.255.0"}})
telemetry(TP, "gateway", inbound="Gateway: 192.168.1.1\r\n", expect_state={"network": {"gateway": "192.168.1.1"}})
telemetry(TP, "ports", inbound="TCP/IP port: 8000\r\n", expect_state={"network": {"tcp_port": 8000}})
telemetry(TP, "telnet-port", inbound="Telnet port: 23\r\n", expect_state={"network": {"telnet_port": 23}})
telemetry(TP, "mac", inbound="MAC: 6C:DF:FB:0C:B3:8E\r\n", expect_state={"network": {"mac": "6C:DF:FB:0C:B3:8E"}})
