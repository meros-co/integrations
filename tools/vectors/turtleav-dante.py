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

# Full control: every setting each model's API writes, and its read-back.
td("set_master_mute_downtown", {"muted": True}, "set master mute 1", **DT)
td("set_master_member", {"members": "11"}, "set master member <11>")
td("clear_preset", {"preset": 1}, "set preset clear 1")
td("set_preset_name", {"preset": 1, "name": "MeetingRoom 1"}, "set preset 1 name MeetingRoom 1", **DT)
td("set_standby_mode", {"mode": 2}, "set standby 2")
td("set_auto_standby", {"minutes": 10}, "set auto stb 10")
td("step_input_gain", {"input": 1, "direction": "+", "step": 5.0}, "set input 1 gain+5.0")
td("set_input_stereo", {"pair": 1, "state": "on"}, "set input 1 stereo on", **A30)
td("set_input_eq_preset", {"input": 1, "preset": 2}, "set input 1 eq preset 2")
td("set_input_eq_band_enabled", {"input": 1, "band": 0, "state": "on"}, "set input 1 eq 0 on")
td("set_input_eq_stereo", {"pair": 1, "state": "on"}, "set input 1 eq stereo on")
td("set_input_eq_band", {"input": 1, "band": 1, "type": 1, "frequency": 200, "gain": -12.5, "q": 0.7},
   "set input 1 eq 1 typ 1 frq 200 val -12.5 q 0.70")
td("clear_input_eq", {"input": 1}, "set input 1 eq clear")
td("step_output_volume", {"output": 5, "direction": "-", "step": 5}, "set output 5 vol-5")
td("set_output_stereo", {"pair": 1, "state": "on"}, "set output 1 stereo on", **A30)
td("set_output_eq_preset", {"output": 1, "preset": 2}, "set output 1 eq preset 2")
td("set_output_eq_band_enabled", {"output": 1, "band": 3, "state": "off"}, "set output 1 eq 3 off")
td("set_output_eq_stereo", {"pair": 1, "state": "on"}, "set output 1 eq stereo on")
td("set_output_eq_band", {"output": 1, "band": 2, "type": 4, "frequency": 1000, "gain": 3.0, "q": 1.4},
   "set output 1 eq 2 typ 4 frq 1000 val 3.0 q 1.40")
td("clear_output_eq", {"output": 1}, "set output 1 eq clear")
td("copy_eq", {"from_kind": "input", "from": 1, "to_kind": "output", "to": 2}, "set input 1 eq copy to output 2")
td("copy_output_eq", {"from": 1, "to": 2}, "set output 1 eq copy to 2", **A30)
td("remove_output_route", {"output": 5, "source": 1}, "set output 5 remove from 1", **A30)
td("set_mixer", {"mixer": 2, "levels": "-14 off -14 off"}, "set mixer 2 from <-14 off -14 off>", **A30)
td("set_output_ducking", {"output": 2, "source": 1, "level": -9, "threshold": -12, "attack": 100, "hold": 1000,
                          "release": 100}, "set output 2 duck on 1 lv -9 th -12 at 100 ht 1000 rt 100", **A30)
td("set_output_ducking_off", {"output": 1}, "set output 1 duck off", **A30)
td("set_amp_mode", {"mode": 2}, "set amp mode 2", **A30)
td("set_front_key", {"state": "off"}, "set key off", **A30)
td("set_lcd", {"mode": "15"}, "set lcd 15", **A30)
td("set_id_led", {"mode": "on"}, "set idled on", **A30)
td("set_trigger", {"mode": "on 1"}, "set trigger on 1", **A30)
td("set_baud_rate", {"baud": "115200"}, "set rsb 115200", **A30)
td("set_output_audio", {"mode": 2}, "set output 1 audio 2", **DT)
td("set_output_hdcp", {"mode": 1}, "set output 1 hdcp 1", **DT)
td("set_output_name", {"name": "Sony TV"}, "set output 1 name Sony TV", **DT)
td("set_input_name", {"name": "MacBook"}, "set input 1 name MacBook", **DT)
td("set_input_edid", {"edid": 1}, "set input 1 edid 1", **DT)
_edid = " ".join(["00", "FF", "FF", "FF", "FF", "FF", "FF", "00"] + ["00"] * 120)
td("set_user_edid", {"slot": 1, "data": _edid}, f"set user edid 1 <{_edid}>", **DT)
td("get_edid_data", {"source": 1}, "get edid data 1", device_reply="HDMI input 1 EDID data\r\n",
   expect_result={"ok": {"kind": "value", "value": "HDMI input 1 EDID data"}}, **DT)
td("set_input_hdcp", {"enabled": True}, "set input 1 hdcp 1", **DT)
td("set_pattern", {"resolution": 1, "pattern": 1}, "set pattern 1 1", **DT)
td("set_master_include", {"output": 0}, "set master include 0", **DT)
td("set_master_remove", {"output": 2}, "set master remove 2", **DT)
td("set_upmixer", {"enabled": True}, "set upmixer 1", **DT)
td("set_virtualizer", {"enabled": False}, "set virtualizer 0", **DT)
td("set_audio_out_source", {"out": 1, "source": 2}, "set audio out 1 source 2", **DT)
td("set_speaker_bass", {"out": 1, "speaker": 1, "enabled": True}, "set audio out 1 speaker 1 bass 1", **DT)
td("set_speaker_delay", {"out": 1, "speaker": 1, "ms": 20}, "set audio out 1 speaker 1 delay 20", **DT)
td("set_speaker_gain", {"out": 1, "speaker": 3, "gain": -6.5}, "set audio out 1 speaker 3 gain -6.5", **DT)
td("set_speaker_mute", {"out": 2, "speaker": 1, "muted": True}, "set audio out 2 speaker 1 mute 1", **DT)
td("reset_speakers", {"out": 1}, "set audio out 1 speaker reset", **DT)
td("set_crossover", {"out": 1, "hz": 100}, "set audio out 1 crossover 100", **DT)
td("set_downmix_include", {"out": 1, "downmix": 1, "channels": "10111010"},
   "set audio out 1 downmix 1 include <10111010>", **DT)
td("set_downmix_mono", {"out": 1, "downmix": 1, "enabled": True}, "set audio out 1 downmix 1 mono 1", **DT)
td("set_speaker_eq_band_enabled", {"out": 1, "speaker": 1, "band": 1, "state": "on"},
   "set audio out 1 speaker 1 eq 1 on", **DT)
td("set_speaker_eq_band", {"out": 1, "speaker": 1, "band": 1, "type": 1, "frequency": 200, "gain": -15.0, "q": 0.02},
   "set audio out 1 speaker 1 eq 1 typ 1 frq 200 val -15.0 q 0.02", **DT)
td("clear_speaker_eq", {"out": 1, "speaker": 1}, "set audio out 1 speaker 1 eq clear", **DT)
td("copy_speaker_eq", {"from_out": 1, "from_speaker": 1, "to_out": 2, "to_speaker": 2},
   "set audio out 1 speaker 1 eq copy to 2 2", **DT)
td("set_front_lock", {"locked": False}, "set front button 0", **DT)
td("set_ir", {"enabled": True}, "set ir 1", **DT)
td("set_baud", {"rate": 6}, "set baud 6", **DT)
td("set_auto_event_report", {"enabled": True}, "set auto event report 1", **DT)
td("set_network_ip_mode", {"network": "pri", "mode": 0}, "set pri ip mode 0")
td("set_network_ip_address", {"network": "sec", "address": "192.168.1.100"}, "set sec ip addr 192.168.1.100")
td("set_network_subnet", {"network": "pri", "mask": "255.255.255.0"}, "set pri subnet 255.255.255.0")
td("set_network_gateway", {"network": "pri", "gateway": "192.168.1.1"}, "set pri gateway 192.168.1.1")
td("set_ip_mode", {"mode": 1}, "set ip mode 1", **A30)
td("set_ip_address", {"address": "192.168.1.100"}, "set ip addr 192.168.1.100", **A30)
td("set_subnet", {"mask": "255.255.255.0"}, "set subnet 255.255.255.0", **A30)
td("set_gateway", {"gateway": "192.168.1.1"}, "set gateway 192.168.1.1", **A30)
td("set_tcp_port", {"port": 8000}, "set tcp/ip port 8000")
td("set_telnet_port", {"port": 23}, "set telnet port 23")
td("set_hostname", {"hostname": "Bridge-1"}, "set net hostname Bridge-1", **PH)
td("net_reboot", {}, "set net reboot")


def tdt(name, inbound, state, **extra):
    telemetry(TD, name, inbound=inbound + "\r\n", expect_state=state, **extra)


tdt("master-member", "Set master member: 11", {"master": {"member": "11"}})
tdt("output-volume-step", "Increase XLR OUT5 volume: 51", {"outputs": {"5": {"volume": 51}}})
tdt("input-phantom", "XLR IN3 phantom power: on", {"inputs": {"3": {"phantom": True}}})
tdt("input-sensitivity", "XLR IN3 sensitivity: +24dBu", {"inputs": {"3": {"sensitivity": 1}}})
tdt("output-level", "LINE OUT3 sensitivity: -18dBV", {"outputs": {"3": {"level": 5}}})
tdt("input-eq-preset", "XLR IN1 PEQ: Custom1", {"inputs": {"1": {"eq": {"preset": 2}}}})
tdt("input-eq", "XLR IN1 EQ all: on", {"inputs": {"1": {"eq": {"enabled": True}}}})
tdt("output-eq-band", "LINE OUT1 EQ 4: off", {"outputs": {"1": {"eq": {"bands": {"4": {"enabled": False}}}}}})
tdt("input-eq-band", "XLR IN2 EQ 1: on", {"inputs": {"2": {"eq": {"bands": {"1": {"enabled": True}}}}}})
tdt("output-eq-preset", "Speaker Out CH1 PEQ: Flat", {"outputs": {"5": {"eq": {"preset": 1}}}})
tdt("output-eq", "XLR OUT1 EQ: off", {"outputs": {"1": {"eq": {"enabled": False}}}})
tdt("input-eq-stereo", "XLR IN1/2 EQ stereo mode: on",
    {"inputs": {"1": {"eq": {"stereo": True}}, "2": {"eq": {"stereo": True}}}})
tdt("output-eq-stereo", "LINE OUT3/4 EQ stereo mode: off",
    {"outputs": {"3": {"eq": {"stereo": False}}, "4": {"eq": {"stereo": False}}}})
tdt("amp-output-eq-stereo", "Line Out CH1/2 EQ stereo mode: on",
    {"outputs": {"3": {"eq": {"stereo": True}}, "4": {"eq": {"stereo": True}}}})
tdt("amp-output-stereo", "Speaker Out 1/2 stereo mode: on",
    {"outputs": {"5": {"stereo": True}, "6": {"stereo": True}}})
tdt("amp-input-stereo", "Mic/Line In CH1/2 stereo mode: off",
    {"inputs": {"3": {"stereo": False}, "4": {"stereo": False}}})
tdt("amp-input-gain", "Mic/Line In CH2 gain: -3.5dB", {"inputs": {"4": {"gain": -3.5}}})
tdt("amp-input-mute", "Dante In CH1 mute: on", {"inputs": {"1": {"mute": True}}})
tdt("amp-input-phantom", "Mic/Line In CH1 phantom power: on", {"inputs": {"3": {"phantom": True}}})
tdt("amp-output-volume", "Speaker Out CH1 volume: 50", {"outputs": {"5": {"volume": 50}}})
tdt("amp-output-mute", "Line Out CH2 mute: off", {"outputs": {"4": {"mute": False}}})
tdt("amp-output-delay", "Dante Out CH2 delay: 20ms", {"outputs": {"2": {"delay_ms": 20}}})
tdt("eq-band-setting", "XLR IN1 EQ : Type: 2, Frequency: 1000Hz, Value: 12dB, Q: 1.4",
    {"inputs": {"1": {"eq": {"bands": {"3": {"type": 2, "frequency": 1000.0, "gain": 12.0, "q": 1.4}}}}}},
    request="set input 1 eq 3 typ 2 frq 1000 val 12.0 q 1.40")
tdt("amp-eq-band-setting", "Dante Out CH1 EQ 1: Type: 3, Frequency: 200Hz, Value: -18dB, Q: 0.02",
    {"outputs": {"1": {"eq": {"bands": {"1": {"type": 3, "frequency": 200.0, "gain": -18.0, "q": 0.02}}}}}},
    request="set output 1 eq 1 typ 3 frq 200 val -18.0 q 0.02")
tdt("amp-from", "Speaker Out CH1 from: Dante In CH1", {"outputs": {"5": {"from": "Dante In CH1"}}})
tdt("amp-ducking-off", "Dante Out CH1 ducking: off", {"outputs": {"1": {"ducking": {"enabled": False}}}})
tdt("amp-ducking", "Dante Out CH2 ducking: Mic/Line In CH1, duck level: -9dB, threshold: -12dB, attack time: 100ms, "
    "hold time: 1000ms, release time: 100ms",
    {"outputs": {"2": {"ducking": {"enabled": True, "source": 1, "level": -9, "threshold": -12, "attack_ms": 100,
                                   "hold_ms": 1000, "release_ms": 100}}}})
tdt("amp-mixer", "Mixer 2: <-14 off -14 off>", {"mixers": {"2": {"levels": "-14 off -14 off"}}})
tdt("amp-mode", "AMP power mode: PoE Class 0", {"amp_mode": "PoE Class 0"})
tdt("amp-key", "Key: off", {"front_key": False})
tdt("amp-lcd", "LCD light on 15s", {"lcd": "on 15s"})
tdt("amp-id-led", "ID LED light always on", {"id_led": "always on"})
tdt("amp-trigger", "Trigger on with high level", {"trigger": "high"})
tdt("amp-trigger-off", "Trigger off", {"trigger": "off"})
tdt("amp-baud", "Baud rate: 115200", {"baud": 115200})
tdt("power", "Power: off", {"power": False})
tdt("standby", "Standby mode: sleep", {"standby_mode": "sleep"})
tdt("auto-standby", "Auto standby time: 10mins", {"auto_standby_min": 10})
tdt("preset-downtown", "Preset 2: recall", {"preset": {"recalled": 2}})
tdt("preset-name", "Preset 1 name: MeetingRoom 1", {"presets": {"1": {"name": "MeetingRoom 1"}}})
tdt("front-button", "Button: locked", {"front_locked": True})
tdt("ir", "IR: on", {"ir": True})
tdt("baudrate", "Baudrate: 115200", {"baud": 115200})
tdt("temperature", "65C", {"temperature_c": 65}, request="get temp")
tdt("uptime", "000:00:13:04", {"uptime": "000:00:13:04"}, request="get uptime")
tdt("auto-event", "Auto event report: on", {"auto_event_report": True})
tdt("hdmi-name", "HDMI output 1 name: Sony TV", {"hdmi": {"output": {"name": "Sony TV"}}})
tdt("hdmi-connected", "HDMI input 1: connected", {"hdmi": {"input": {"connected": True}}})
tdt("hdmi-edid", "HDMI input 1 EDID: auto", {"hdmi": {"input": {"edid": "auto"}}})
tdt("hdmi-input-hdcp", "HDMI input 1 HDCP: on", {"hdmi": {"input": {"hdcp": True}}})
tdt("hdmi-output-hdcp", "HDMI output1 HDCP: auto (followsink)", {"hdmi": {"output": {"hdcp": "auto (followsink)"}}})
tdt("hdmi-video", "HDMI output1 video: bypass", {"hdmi": {"output": {"video": "bypass"}}})
tdt("hdmi-audio", "HDMI output 1 audio: bypass", {"hdmi": {"output": {"audio": "bypass"}}})
tdt("pattern", "Pattern: 8K30Hz color bar", {"pattern": "8K30Hz color bar"})
tdt("audio-source", "Audio source: HDMI input 1", {"audio_source": "HDMI input 1"})
tdt("master-include", "Master include: all outputs", {"master": {"include": "all outputs"}})
tdt("drc", "DRC: on", {"drc": "on"})
tdt("upmixer", "Upmixer: on", {"upmixer": True})
tdt("virtualizer", "Virtualizer: off", {"virtualizer": False})
tdt("speaker-mode", "Speaker mode 1: 2.0CH", {"speaker_mode": 1, "speaker_layout": "2.0CH"})
tdt("audio-out-source", "Line out source: active input", {"audio_out": {"Line": {"source": "active input"}}})
tdt("crossover", "Dante out crossover: 100Hz", {"audio_out": {"Dante": {"crossover_hz": 100}}})
tdt("downmix-include", "Line out downmix L: 10111010",
    {"audio_out": {"Line": {"downmix": {"L": {"include": "10111010"}}}}})
tdt("downmix-mono", "Line out downmixL mono: on", {"audio_out": {"Line": {"downmix": {"L": {"mono": True}}}}})
tdt("speaker-bass", "Line out speaker L bass: on", {"audio_out": {"Line": {"speakers": {"L": {"bass": True}}}}})
tdt("speaker-delay", "Line out speaker Ltm/DL delay: 20ms",
    {"audio_out": {"Line": {"speakers": {"Ltm/DL": {"delay_ms": 20}}}}})
tdt("speaker-gain", "Dante out speaker Sub gain: -3.5dB", {"audio_out": {"Dante": {"speakers": {"Sub": {"gain": -3.5}}}}})
tdt("speaker-mute", "Line out speaker C mute: on", {"audio_out": {"Line": {"speakers": {"C": {"mute": True}}}}})
tdt("speaker-eq-band", "Line out speaker L EQ 1: on",
    {"audio_out": {"Line": {"speakers": {"L": {"eq": {"bands": {"1": {"enabled": True}}}}}}}})
tdt("speaker-eq-setting", "Line out speaker L EQ 1 TYP 1 FRQ 200 VAL -18 Q 0.02",
    {"audio_out": {"Line": {"speakers": {"L": {"eq": {"bands": {"1": {
        "type": 1, "frequency": 200.0, "gain": -18.0, "q": 0.02}}}}}}}})
tdt("primary-ip-mode", "Primary IP mode: DHCP", {"network": {"primary": {"ip_mode": "dhcp"}}})
tdt("primary-ip", "Primary IP: 192.168.0.100", {"network": {"primary": {"ip": "192.168.0.100"}}})
tdt("secondary-subnet", "Secondary Subnet Mask: 255.255.255.0", {"network": {"secondary": {"subnet": "255.255.255.0"}}})
tdt("secondary-gateway", "Secondary Gateway: 192.168.1.1", {"network": {"secondary": {"gateway": "192.168.1.1"}}})
tdt("primary-mac", "Primary MAC: 6C:DF:FB:0C:B3:8E", {"network": {"primary": {"mac": "6C:DF:FB:0C:B3:8E"}}})
tdt("amp-ip", "IP: 192.168.62.106", {"network": {"primary": {"ip": "192.168.62.106"}}}, model="amp-30w")
tdt("amp-ip-mode", "IP mode: Static", {"network": {"primary": {"ip_mode": "static"}}}, model="amp-30w")
tdt("amp-subnet", "Subnet Mask: 255.255.255.0", {"network": {"primary": {"subnet": "255.255.255.0"}}}, model="amp-30w")
tdt("amp-gateway", "Gateway: 192.168.1.1", {"network": {"primary": {"gateway": "192.168.1.1"}}}, model="amp-30w")
tdt("amp-mac", "MAC: 6C:DF:FB:0C:83:8E", {"network": {"primary": {"mac": "6C:DF:FB:0C:83:8E"}}}, model="amp-30w")
tdt("tcp-port", "TCP/IP port: 8000", {"network": {"tcp_port": 8000}})
tdt("telnet-port", "Telnet port: 23", {"network": {"telnet_port": 23}})
tdt("hostname", "Hostname: IP-Module-333", {"network": {"hostname": "IP-Module-333"}})
# The two common reads queue each model's own reads.
tdt("fast-reads", "Master volume: 50", {"master": {"volume": 50}}, request="get master vol", model="mineola-2x2",
    expect_then_send=[x + "\r\n" for x in [
        "get master mute", "get input 1 gain", "get input 1 mute", "get input 2 gain", "get input 2 mute",
        "get output 1 vol", "get output 1 mute", "get output 2 vol", "get output 2 mute"]])
tdt("fast-reads-downtown", "Master volume: 50", {"master": {"volume": 50}}, request="get master vol", model="downtown",
    expect_then_send=[x + "\r\n" for x in [
        "get master mute", "get audio source", "get input 1 connected", "get output 1 connected"]])
tdt("slow-reads", "Power: on", {"power": True}, request="get power", model="mineola-2x2",
    expect_then_send=[x + "\r\n" for x in [
        "get standby", "get auto stb", "get master member",
        "get input 1 sensitivity", "get input 1 phantom power", "get input 1 eq preset", "get input 1 eq",
        "get input 2 sensitivity", "get input 2 phantom power", "get input 2 eq preset", "get input 2 eq",
        "get input 1 eq stereo",
        "get output 1 gain", "get output 1 delay", "get output 1 eq preset", "get output 1 eq",
        "get output 2 gain", "get output 2 delay", "get output 2 eq preset", "get output 2 eq",
        "get output 1 eq stereo",
        "get pri ip mode", "get pri ip addr", "get pri subnet", "get pri gateway", "get pri mac addr",
        "get sec ip mode", "get sec ip addr", "get sec subnet", "get sec gateway", "get sec mac addr",
        "get tcp/ip port", "get telnet port"]])

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
