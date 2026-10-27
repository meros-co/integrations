BL = "blustream-matrix"
# Blustream matrices: each model's help text ("<model> - API" download) gives
# the commands; CR LF ends each (CMX88CS-V2 manual p.26); Blustream documents
# no answers, so every command is unverified. Expected lines are written from
# the help texts' syntax lines (OUTxx FR yy on HDBaseT models, OUT xx FR yy on
# CMX, MX and Pro models; two-digit numbers, 00 for all).
BL_U = {"ok": {"kind": "unverified"}}
BL_CMX = {"model": "cmx88cs"}
BL_V2 = {"model": "cmx88cs-v2"}
BL_HMX = {"model": "hmx88-18g"}
BL_PRO = {"model": "pro16"}
BL_VW = {"model": "mx44vw"}


def bl(command, input, line, **extra):
    text(BL, command, input, line + "\r\n", **extra)


bl("route", {"output": 1, "input": 4}, "OUT01 FR 04", expect_result=BL_U)
bl("route", {"output": 0, "input": 2}, "OUT 00 FR 02", file="route-spaced", **BL_CMX)
bl("set_output_enabled", {"output": 3, "enabled": False}, "OUT03 OFF")
bl("set_output_enabled", {"output": 2, "enabled": True}, "OUT 02 ON", file="set_output_enabled-spaced", **BL_CMX)
bl("set_input_enabled", {"input": 2, "enabled": False}, "IN02 OFF", model="c44cs")
bl("set_input_enabled", {"input": 1, "enabled": True}, "IN 01 ON", file="set_input_enabled-v2", **BL_V2)
bl("set_output_mode", {"mode": "VW22"}, "OUT MODE VW22")
bl("route_video_wall", {"input": 3}, "OUT VW FR 03")
bl("set_video_wall_bezel", {"output": 1, "side": "VCL", "pixels": 20}, "OUT 01 VCL 20")
bl("set_input_signal", {"input": 2, "signal": "VGA"}, "IN 02 FR VGA")
bl("set_multiview_audio", {"input": 0}, "MV AUD 00")
bl("set_switching_mode", {"output": 0, "mode": "AUTO"}, "OUT 00 SWITCH AUTO")
bl("set_auto_switch", {"enabled": True}, "AUTO SWITCH ON")
bl("set_input_switch_mode", {"input": 1, "mode": 5}, "IN01 SWMODE 05")
bl("save_preset", {"preset": 3}, "PRESET 03 SAVE")
bl("recall_preset", {"preset": 1}, "PRESET 01 APPLY", expect_result=BL_U)
bl("clear_preset", {"preset": 0}, "PRESET 00 CLR")
bl("set_preset", {"preset": 2, "inputs": "01,02,03,04,05,06,07,08"}, "PRESET 02 SET 01,02,03,04,05,06,07,08", **BL_CMX)
bl("set_power", {"powered": True}, "PON")
bl("set_power", {"powered": False}, "POFF", file="set_power-off")
bl("set_front_panel_keys", {"enabled": False}, "KEY OFF")
bl("set_ir", {"enabled": True}, "IR ON")
bl("set_ir_voltage", {"volts": "12V"}, "IR 12V")
bl("set_beep", {"enabled": False}, "BEEP OFF")
bl("set_lcd", {"always_on": True}, "LCD ON")
bl("set_power_led", {"state": "PON", "timeout": "30"}, "LED PON 30")
bl("set_debug", {"enabled": False}, "DBG OFF")
bl("set_diagnostics_log", {"enabled": True}, "DIAGPRINT ON")
bl("set_system_log", {"enabled": False}, "SYSTEMLOG OFF")
bl("reboot", {}, "REBOOT")
text(BL, "factory_reset", {}, ["RESET\r\n", "Yes\r\n"])
text(BL, "factory_reset_all", {}, ["RESET ALL\r\n", "Yes\r\n"])
bl("set_serial_baud", {"baud": "115200"}, "RS232BAUD 7")
bl("set_serial_baud", {"baud": "9600"}, "RSB 4", file="set_serial_baud-rsb", **BL_CMX)
bl("set_scaling", {"output": 0, "enabled": False}, "OUT 00 SCALING OFF")
bl("set_video_mode", {"output": 1, "mode": 2}, "OUT 01 SCALING 02")
bl("set_color_space_conversion", {"side": "OUT", "enabled": True}, "OUT CSC ON")
bl("set_input_uhd_edid", {"input": 5, "enabled": True}, "IN 05 UHD ON")
bl("set_long_reach", {"output": 2, "enabled": True}, "OUT 02 LR ON")
bl("set_long_reach", {"output": 2, "enabled": True}, "OUT02 LRM ON", file="set_long_reach-hmx", **BL_HMX)
bl("set_input_long_reach", {"input": 1, "enabled": False}, "IN 01 LR OFF")
bl("set_edid_source", {"output": 5, "source": "hdbt"}, "OUT05 ET")
bl("set_edid_source", {"output": 5, "source": "hdmi"}, "OUT 05 EH", file="set_edid_source-pro", **BL_PRO)
bl("set_input_edid_source", {"input": 1, "source": "hdmi"}, "IN01 EH")
bl("set_input_edid", {"input": 2, "edid": 15}, "EDID 02 DF 15")
bl("copy_edid", {"input": 0, "output": 3}, "EDID 00 CP 03")
bl("save_edid", {"output": 1, "slot": 34}, "EDID SAVE 01 TO 34")
bl("load_usb_edid", {"slot": 75}, "EDIDSAVE 75")
bl("set_mute", {"output": 9, "muted": True}, "MUTE ON TX 09")
bl("set_mute", {"output": 1, "muted": False}, "MUTE Off TX 01", file="set_mute-hmx", **BL_HMX)
bl("set_audio_output", {"output": 0, "enabled": False}, "AUDOUT 00 OFF")
bl("set_audio_output", {"output": 3, "enabled": True}, "AUDOUT 03 On", file="set_audio_output-hmx", **BL_HMX)
bl("set_volume", {"output": 1, "level": 68}, "VOL68 TX 01")
bl("set_volume", {"output": 1, "level": 50}, "VOL 50 TX 01", file="set_volume-hmx", **BL_HMX)
bl("volume_up", {"output": 2}, "VOL+ TX 02")
bl("volume_down", {"output": 0}, "VOL - TX 00", **BL_HMX)
bl("get_volume", {"output": 4}, "OUT04 VOLSTA")
bl("set_audio_source", {"output": 1, "source": 25}, "AUDIO01 FR 25")
bl("set_audio_downmix", {"output": 9, "enabled": True}, "AUDIO09 DMIX ON")
bl("set_audio_delay", {"output": 1, "milliseconds": 120}, "AUDIO01 Delay 120")
bl("set_embedded_audio", {"output": 2, "source": 9}, "OUT02 EMBED 09")
bl("set_input_audio_mode", {"input": 3, "mode": "DANTE"}, "AUD RX 03 DANTE", **BL_PRO)
bl("set_arc", {"output": 1, "mode": 2}, "OUT 01 ARC 02")
bl("set_poc", {"output": 0, "enabled": True}, "POCOUT00 ON")
bl("set_poc", {"output": 4, "enabled": False}, "POC TX 04 OFF", file="set_poc-pro", **BL_PRO)
bl("set_input_poc", {"input": 1, "enabled": True}, "POCIN01 ON")
bl("set_guest_mode", {"output": 2, "enabled": True}, "OUT 02 GUEST ON")
bl("set_input_guest_mode", {"input": 2, "enabled": False}, "IN 02 GUEST OFF")
bl("set_ir_control", {"output": 1, "enabled": True}, "IROUT01 ON")
bl("set_ir_simple_control", {"output": 1, "enabled": False}, "IROUT01 SIMPLE OFF")
bl("set_input_ir_control", {"input": 0, "enabled": True}, "IRIN00 ON")
bl("set_input_ir_simple_control", {"input": 2, "enabled": True}, "IRIN02 SIMPLE ON")
bl("send_ir", {"output": 1, "code": "0000 006C 0000 0022"}, "OUT 01 IRHEXSEND 0000 006C 0000 0022")
bl("send_input_ir", {"input": 1, "code": "0000 006C"}, "IN 01 IRHEXSEND 0000 006C")
bl("set_ir_follow_video", {"enabled": True}, "IRFV ON")
bl("route_ir", {"destination": 1, "source": 0}, "MXIR 01 FR 00")
bl("route_ir_to_receiver", {"receiver": 0, "source": 9}, "MXIRRX 00 FR 09")
bl("route_ir_to_transmitter", {"transmitter": 1, "source": 3}, "MXIRTX 01 FR 03")
bl("set_global_ir", {"set": "GI", "action": "+", "port": 17}, "MXIR GI +17", **BL_PRO)
bl("route_serial", {"receiver": 10}, "MXRS 09 TO 10")
bl("route_serial_to_receiver", {"receiver": 1, "source": 9}, "MXRS232OUT 01 FR 09")
bl("route_serial_to_transmitter", {"transmitter": 2, "source": 1}, "MXRS232IN 02 FR 01")
bl("set_serial_control", {"output": 1, "enabled": True}, "RS232OUT01 ON")
bl("set_input_serial_control", {"input": 1, "enabled": False}, "RS232IN01 OFF")
bl("send_serial", {"output": 1, "wait": 2, "format": "a", "baud": "9600", "command": "PWR ON"},
   "OUT 01 RS232SEND 02:a:3:PWR ON")
bl("send_input_serial", {"input": 1, "command": "POWR1"}, "IN 01 RS232SEND 00:a:6:POWR1")
bl("send_local_serial", {"format": "h", "baud": "115200", "command": "AA 01"}, "RS232ONOUT h:0:AA 01")
bl("set_serial_delay", {"output": 0, "seconds": 3}, "RS232DLYOUT00:3")
bl("set_input_serial_delay", {"input": 2, "seconds": 10}, "RS232DLYIN02:10")
bl("set_serial_on_command", {"output": 1, "slot": 1, "baud": "9600", "command": "PWR ON"}, "RS232ONOUT01 1:a:3:PWR ON")
bl("set_serial_off_command", {"output": 1, "baud": "9600", "command": "PWR OFF"}, "RS232OFFOUT01 a:3:PWR OFF", model="c66cs")
bl("set_serial_off_command_slot", {"output": 1, "slot": 2, "baud": "9600", "command": "PWR OFF"}, "RS232OFFOUT01 2:a:3:PWR OFF")
bl("disable_serial_commands", {"output": 3, "event": "OFF"}, "RS232OFFOUT03 DISABLE")
bl("set_input_serial_on_command", {"input": 1, "slot": 3, "format": "h", "command": "0A 0B"}, "RS232ONIN01 3:h:6:0A 0B")
bl("set_input_serial_off_command", {"input": 2, "slot": 1, "command": "OFF"}, "RS232OFFIN02 1:a:6:OFF")
bl("disable_input_serial_commands", {"input": 1, "event": "ON"}, "RS232ONIN01 DISABLE")
bl("set_output_cec", {"output": 1, "enabled": True}, "OUT01 CEC ENABLE")
bl("cec_output", {"output": 0, "action": "POFF"}, "OUT00 CEC POFF")
bl("cec_output_input", {"output": 2, "tv_input": 3}, "OUT02 CEC INPUT03")
bl("set_input_cec", {"input": 4, "enabled": False}, "IN04 CEC DISABLE")
bl("cec_input", {"input": 1, "action": "PLAY"}, "IN01 CEC PLAY")
bl("cec_user_command", {"message": "F0 40 44 41"}, "CECUSERCMD <F0 40 44 41 >")
bl("set_dhcp", {"enabled": False}, "NET DHCP OFF")
bl("set_ip_address", {"address": "192.168.0.200"}, "NET IP 192.168.0.200")
bl("set_gateway", {"address": "192.168.0.1"}, "NET GW 192.168.0.1")
bl("set_subnet_mask", {"address": "255.255.255.0"}, "NET SM 255.255.255.0")
bl("network_reboot", {}, "NET RB")
bl("set_telnet_port", {"port": 23}, "NET TN 23")
bl("set_telnet", {"enabled": True}, "NET TN ON")
bl("set_port_8000", {"enabled": False}, "NET TN8000 OFF")
bl("set_tcp_port", {"value": "8000"}, "NET TCPPORT 8000")
bl("set_mdns", {"enabled": True}, "NET MDNS ON")
bl("set_mdns_name", {"name": "cmx88cs-v2"}, "NET MDNS cmx88cs-v2.local")
bl("set_dns_name", {"name": "matrix1"}, "NET DNS matrix1")
bl("get_status", {}, "STATUS")
bl("get_input_status", {}, "INSTA")
bl("get_output_status", {}, "OUTSTA")
bl("get_control_status", {}, "CTRLSTA")
bl("get_hdbt_status", {}, "HDBTSTA")
bl("get_preset_status", {}, "PRESETSTATUS")
bl("get_preset_status", {}, "PRESET STATUS", file="get_preset_status-spaced", **BL_PRO)
bl("get_audio_status", {}, "AUDSTA")
bl("get_ir_routing", {}, "MXSTA")
bl("get_firmware", {}, "FWVERSION")
bl("get_firmware", {}, "FWVER", file="get_firmware-cmx", **BL_CMX)
bl("get_output_diagnostics", {"output": 1}, "OUT 01 DIAG")
bl("get_input_diagnostics", {"input": 0}, "IN 00 DIAG")


def bl_t(name, inbound, state, **extra):
    telemetry(BL, name, inbound=inbound + "\r\n", expect_state=state, **extra)


# Telemetry: the PLA88CS STATUS and CTRLSTA reports, line by line.
bl_t("model", "              PLA88CS HDBaseT Matrix", {"model": "PLA88CS HDBaseT Matrix"})
bl_t("firmware", "              FW Version: V2.3.0", {"firmware": "V2.3.0"})
bl_t("system", "ON         ON         ON         OFF        57600      ON",
     {"power": True, "ir": True, "front_panel_keys": True, "lcd_always_on": False,
      "serial_baud": 57600, "ir_follow_video": True})
bl_t("input-row", "01         Default_00 OFF        ON         HDMI       FORCE_HDMI HDMI       OFF        OFF        OFF",
     {"inputs": {"1": {"edid": "Default_00", "connected": False, "cec": True, "priority": "HDMI",
                       "switch_mode": "FORCE_HDMI", "switch_selected": "HDMI", "poc": "OFF",
                       "long_reach": "OFF", "guest_mode": "OFF"}}})
bl_t("output-row", "02         05         ON         Yes        SNK        OFF        OFF        ON         ON         HDBT       OFF",
     {"outputs": {"2": {"input": 5, "connected": True, "enabled": True, "osp": "SNK", "poc": False,
                        "long_reach": False, "scaling": True, "cec": True, "priority": "HDBT",
                        "guest_mode": False}}})
bl_t("output-row-4x4", "02      01           ON /OFF        No          ON",
     {"outputs": {"2": {"input": 1, "enabled": False}}})
bl_t("audio-row", "01         VideoOut01 ON         ON         ON         ON         0          68         OFF        FOLVideo",
     {"audio": {"1": {"source": "VideoOut01", "optical": True, "optical_downmix": True, "analog": True,
                      "analog_downmix": True, "delay": 0, "volume": 68, "muted": False, "embed": "FOLVideo"}}})
bl_t("network", "ON              192.168.0.200   192.168.0.1     255.255.255.0",
     {"network": {"dhcp": True, "ip_address": "192.168.0.200", "gateway": "192.168.0.1",
                  "subnet_mask": "255.255.255.0"}})
bl_t("telnet", "23              ON              ON              PLA88CS",
     {"network": {"telnet_port": 23, "port_8000": True, "mdns": True, "mdns_name": "PLA88CS"}})
bl_t("mac", "34:D0:B8:23:19:CC", {"network": {"mac_address": "34:D0:B8:23:19:CC"}})
bl_t("control-input-row", "01         OFF        ON         ON         01         01         OFF             OFF",
     {"inputs": {"1": {"serial_control": False, "ir_control": True, "ir_simple_control": True}}})
bl_t("control-output-row", "03         OFF        ON         ON         03         OFF        03         OFF        OFF        3",
     {"outputs": {"3": {"serial_control": False, "ir_control": True, "ir_simple_control": True}}})
bl_t("serial-on-command", "RS232ONOUT01  Command 1 Display On: Disable",
     {"outputs": {"1": {"serial_on_commands": {"1": "Disable"}}}})
bl_t("serial-off-command", "RS232OFFOUT02 Command Off 2: Disable",
     {"outputs": {"2": {"serial_off_commands": {"2": "Disable"}}}})
bl_t("serial-delay", "RS232DLYOUT08 Time Delay: 3 Second(s)", {"outputs": {"8": {"serial_delay": 3}}})
bl_t("input-serial-on-command", "RS232ONIN02  Command 5 User 3: Disable",
     {"inputs": {"2": {"serial_on_commands": {"5": "Disable"}}}})
bl_t("input-serial-off-command", "RS232OFFIN01 Command Off 1: Disable",
     {"inputs": {"1": {"serial_off_commands": {"1": "Disable"}}}})
bl_t("input-serial-delay", "RS232DLYIN01 Time Delay: 3 Second(s)", {"inputs": {"1": {"serial_delay": 3}}})
bl_t("success-route", "[SUCCESS]Set output 01 connect from input 03.", {"outputs": {"1": {"input": 3}}})
