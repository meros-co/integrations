# Blustream Multicast ACM controllers (blustream-acm): each model's help text
# ("<model> - API" download) gives the commands; a carriage return ends each
# (ACM manual p.44); Blustream documents no answers, so every command is
# unverified. Inputs and outputs are written with three digits, as the help
# texts write iii and ooo. Telemetry lines are the manual's STATUS, OUT xxx
# STATUS and IN xxx STATUS samples (p.45-46).
ACM = "blustream-acm"
ACM_U = {"ok": {"kind": "unverified"}}
A500 = {"model": "acm500"}
A1000 = {"model": "acm1000"}


def acm(command, input, line, **extra):
    wire = [w + "\r" for w in line] if isinstance(line, list) else line + "\r"
    text(ACM, command, input, wire, **extra)


# System
acm("get_status", {}, "STATUS", expect_result=ACM_U)
acm("get_version", {}, "VERSION", **A500)
acm("set_ir", {"enabled": False}, "IR OFF")
acm("set_preview_image", {"enabled": True}, "PREVIEW IMAGE ON")
acm("set_serial_baud", {"baud": "9600"}, "RSB 4")
acm("reboot", {}, "REBOOT")
acm("reboot", {}, "ACM REBOOT", file="reboot-acm1000", **A1000)
acm("factory_reset", {}, "RESET")
acm("reset_network", {}, "RESET NB")
acm("factory_reset_all", {}, ["RESET ALL", "Yes"])

# Routing
acm("route", {"output": 3, "input": 12}, "OUT 003 FR 012", expect_result=ACM_U)
acm("route", {"output": 0, "input": 1}, "OUT 000 FR 001", file="route-all", **A1000)
acm("route_signal", {"output": 5, "signal": "usb", "input": 2}, "OUT 005 UFR 002")
acm("set_signal_follow", {"output": 5, "signal": "audio"}, "OUT 005 AFR AUTO")

# Outputs
acm("set_output_enabled", {"output": 7, "enabled": False}, "OUT 007 OFF")
acm("set_output_id", {"output": 7, "id": 120}, "OUT 007 ID 120")
acm("set_fast_switching", {"output": 1, "enabled": True}, "OUT 001 FAST ON")
acm("set_hdr", {"output": 2, "enabled": False}, "OUT 002 HDR OFF")
acm("set_output_cec", {"output": 1, "enabled": True}, "OUT 001 CEC ON")
acm("set_output_id_osd", {"output": 0, "shown": True}, "OUT 000 OSD ON")
acm("show_output_id_osd_for", {"output": 4, "seconds": 90}, "OUT 004 OSD ON 90")
acm("set_output_led_flash", {"output": 4, "flashing": False}, "OUT 004 FLS OFF")
acm("flash_output_led_for", {"output": 4, "seconds": 30}, "OUT 004 FLS ON 30")
acm("delete_output", {"output": 9}, "OUT 009 DEL")
acm("set_output_resolution", {"output": 1, "code": 13}, "OUT 001 RES 13")
acm("set_output_rotation", {"output": 1, "rotation": "270"}, "OUT 001 ROTATE 3")
acm("set_output_stretch", {"output": 1, "enabled": True}, "OUT 001 STRETCH ON")
acm("set_output_https", {"output": 1, "enabled": False}, "OUT 001 HTTPS OFF")
acm("set_output_audio", {"output": 2, "path": "AUDHD"}, "OUT 002 AUDHD")
acm("set_output_lan2_mode", {"output": 2, "mode": 3}, "OUT 002 LAN2M3")
acm("set_output_arc", {"output": 2, "arc": "OPT"}, "OUT 002 ARC OPT")
acm("send_output_cec", {"output": 6, "command": "POWERON"}, "OUT 006 CEC POWERON")
acm("send_output_ir", {"output": 6, "command": "TV Power"}, "OUT 006 IR TV Power")
acm("set_output_name", {"output": 6, "name": "Lobby TV"}, "OUT 006 NAME Lobby TV")
acm("set_output_mode", {"output": 6, "mode": "VW"}, "OUT 006 MODE VW")
acm("set_output_tx_mode", {"output": 6}, "OUT 006 TXMODE", **A500)
acm("set_output_debug", {"output": 6, "enabled": False}, "OUT 006 DBG OFF")
acm("set_output_buttons", {"output": 6, "enabled": False}, "OUT 006 BTN OFF")
acm("set_output_ir", {"output": 6, "enabled": True}, "OUT 006 IR ON")
acm("set_output_ir_voltage", {"output": 6, "volts": "12V"}, "OUT 006 IRVAL 12V", **A500)
acm("set_output_mute", {"output": 6, "muted": True}, "OUT 006 MUTE ON")
acm("set_output_pause", {"output": 6, "paused": True}, "OUT 006 PAUSE ON")
acm("set_output_auto_on", {"output": 6, "enabled": True}, "OUT 006 AUTOON ENABLED")
acm("set_output_auto_on", {"output": 6, "enabled": False}, "OUT 006 AUTOON OFF", file="set_output_auto_on-acm500", **A500)
acm("set_output_led_timeout", {"output": 6, "timeout": 3}, "OUT 006 LED 3")
acm("set_output_serial_guest", {"output": 6, "enabled": True, "baud": "9600", "format": "8n1"},
    "OUT 006 SG ON BR 5 BIT 8n1")
acm("set_output_serial_guest_sdvoe", {"output": 6, "enabled": True, "baud": "9600", "format": "8n1"},
    "OUT 006 SG ON BR 2 BIT 8n1", **A500)
acm("start_output_serial_guest", {"output": 6}, "OUT 006 GUEST")
acm("close_serial_guest", {}, "CLOSEACMGUEST")
acm("set_output_hdcp", {"output": 6, "hdcp": "V22"}, "OUT 006 HDCP V22")
acm("set_output_system_size", {"output": 6, "size": "L"}, "OUT 006 SSM L")
acm("get_output_status", {"output": 1}, "OUT 001 STATUS")
acm("get_all_output_status", {}, "OUT STATUS")
acm("reboot_output", {"output": 1}, "OUT 001 RB")
acm("reset_output", {"output": 1}, "OUT 001 RESET")
acm("get_output_details", {"output": 1}, "OUT 001 ASTPARAM", **A1000)
acm("set_output_hotplug", {"output": 1, "enabled": True}, "OUT 001 HOTPLUG ON", **A1000)
acm("set_output_fallback", {"output": 1, "input": 4}, "OUT 001 FALLBACK 004", **A1000)
acm("set_output_aspect", {"output": 1, "aspect": "MAINTAIN"}, "OUT 001 ASPECT MAINTAIN", **A500)
acm("set_output_display_mode", {"output": 1, "mode": "genlock"}, "OUT 001 DISPLAYMODE DIS 1", **A500)

# Multiview and PIP (ACM500)
acm("set_output_multiview_preset", {"output": 2, "preset": 5}, "OUT 002 MVP 05", **A500)
acm("set_output_multiview_layout", {"output": 2, "layout": 12}, "OUT 002 MVL 12", **A500)
acm("set_output_multiview_layouts", {"output": 2, "layouts": "1,2,30"}, "OUT 002 MVLA 1,2,30", **A500)
acm("set_output_multiview_presets", {"output": 2, "presets": "ALL"}, "OUT 002 MVPA ALL", **A500)
acm("route_multiview_window", {"output": 2, "window": 3, "input": 7}, "OUT 002 W 03 FR 007", **A500)
acm("create_multiview_preset", {"preset": 4, "name": "Quad"}, "MVP 04 CREATE Quad", **A500)
acm("rename_multiview_preset", {"preset": 4, "name": "Quad A"}, "MVP 04 Name Quad A", **A500)
acm("set_multiview_preset_layout", {"preset": 4, "layout": 2}, "MVP 04 L 02", **A500)
acm("route_multiview_preset_window", {"preset": 4, "window": 1, "input": 0}, "MVP 04 W 01 FR 000", **A500)
acm("delete_multiview_preset", {"preset": 4}, "MVP 04 DEL", **A500)
acm("create_pip_layout", {"layout": 26, "name": "PIP1"}, "PIP 26 CREATE PIP1", **A500)
acm("delete_pip_layout", {"layout": 26}, "PIP 26 DEL", **A500)
acm("set_pip_option", {"layout": 26, "option": 2}, "PIP 26 OP 02", **A500)
acm("set_pip_window_active", {"layout": 26, "window": 2, "active": False}, "PIP 26 W 02 INACTIVE", **A500)
acm("set_pip_mode", {"layout": 26, "mode": "OVERLAY"}, "PIP 26 MODE OVERLAY", **A500)
acm("set_pip_position", {"layout": 26, "position": "RB"}, "PIP 26 POSITION RB", **A500)
acm("rename_pip_layout", {"layout": 26, "name": "Corner"}, "PIP 26 Name Corner", **A500)

# Inputs
acm("set_input_id", {"input": 2, "id": 40}, "IN 002 ID 040")
acm("delete_input", {"input": 2}, "IN 002 DEL")
acm("reboot_input", {"input": 2}, "IN 002 RB")
acm("reset_input", {"input": 2}, "IN 002 RESET")
acm("set_input_audio_source", {"input": 2, "source": "ANA"}, "IN 002 AUD ANA")
acm("set_input_https", {"input": 2, "enabled": True}, "IN 002 HTTPS ON")
acm("set_input_audio", {"input": 2, "path": "AUDAD"}, "IN 002 AUDAD")
acm("set_input_lan2_mode", {"input": 2, "mode": 0}, "IN 002 LAN2M0")
acm("set_wallplate_switch_mode", {"input": 0, "mode": "auto_hdmi"}, "IN 000 SWMODE 02")
acm("set_wallplate_switch_mode", {"input": 3, "mode": "hdmi"}, "IN 003 SWMODE 05", file="set_wallplate_switch_mode-acm1000",
    **A1000)
acm("set_wallplate_button_only", {"input": 3}, "IN 003 SWMODE 06", **A1000)
acm("set_wallplate_input", {"input": 3, "source": "usbc"}, "IN 003 TGMODE 1")
acm("set_wallplate_input", {"input": 3, "source": "hdmi"}, "IN TGMODE 0", file="set_wallplate_input-acm1000", **A1000)
acm("set_input_name", {"input": 3, "name": "Laptop"}, "IN 003 NAME Laptop")
acm("set_input_cec", {"input": 3, "enabled": False}, "IN 003 CEC OFF")
acm("send_input_cec", {"input": 3, "command": "PLAY"}, "IN 003 CEC PLAY")
acm("send_input_ir", {"input": 3, "command": "Menu"}, "IN 003 IR Menu")
acm("set_input_ir", {"input": 3, "enabled": False}, "IN 003 IR OFF", **A500)
acm("set_input_ir_voltage", {"input": 3, "volts": "5V"}, "IN 003 IRVAL 5V", **A500)
acm("set_input_led_flash", {"input": 3, "flashing": True}, "IN 003 FLS ON")
acm("flash_input_led_for", {"input": 3, "seconds": 90}, "IN 003 FLS ON 90")
acm("set_input_rx_mode", {"input": 3}, "IN 003 RXMODE", **A500)
acm("set_input_stream_debug", {"input": 3, "enabled": True}, "IN 003 STREAMDBG ON", **A500)
acm("set_input_led_timeout", {"input": 3, "timeout": 0}, "IN 003 LED 0")
acm("set_input_serial_guest", {"input": 3, "enabled": False, "baud": "115200", "format": "7o2"},
    "IN 003 SG OFF BR 9 BIT 7o2")
acm("set_input_serial_guest_sdvoe", {"input": 3, "enabled": False, "baud": "115200", "format": "8n1"},
    "IN 003 SG OFF BR 6 BIT 8n1", **A500)
acm("start_input_serial_guest", {"input": 3}, "IN 003 GUEST")
acm("set_input_hdcp", {"input": 3, "hdcp": "BYP"}, "IN 003 HDCP BYP")
acm("set_input_system_size", {"input": 3, "size": "N"}, "IN 003 SSM N")
acm("set_input_arc_output", {"input": 3, "output": 8}, "IN 003 EFR 008")
acm("set_input_arc", {"input": 3, "enabled": True}, "IN 003 ARC ON")
acm("get_input_status", {"input": 0}, "IN 000 STATUS")
acm("get_input_details", {"input": 3}, "IN 003 ASTPARAM", **A1000)
acm("set_input_hotplug", {"input": 3, "enabled": False}, "IN 003 HOTPLUG OFF", **A1000)

# Secondary stream (ACM500)
acm("get_stream_status", {"input": 0}, "IN 000 SS STATUS", **A500)
acm("get_main_stream_url", {"input": 1}, "IN 001 SS MSURL", **A500)
acm("get_sub_stream_url", {"input": 1}, "IN 001 SS SSURL", **A500)
acm("get_snapshot_url", {"input": 1}, "IN 001 SS SNAPURL", **A500)
acm("set_main_stream", {"input": 1, "codec": "h265", "resolution": "manual", "width": 1920, "height": 1080,
                        "bitrate": "8192"}, "IN 001 ss MAINENCATTR E 1 R manual H 1920 V 1080 B 4", **A500)
acm("set_sub_stream", {"input": 1, "codec": "h264", "width": 640, "height": 360, "bitrate": "512"},
    "IN 001 ss SUBENCATTR E 0 H 640 V 360 B 2", **A500)
acm("set_stream_enabled", {"input": 1, "stream": "PIC", "enabled": True}, "IN 001 SS PIC ON", **A500)

# EDID
acm("copy_edid", {"input": 1, "output": 4}, "EDID 001 CP 004")
acm("set_input_edid", {"input": 1, "edid": 15}, "EDID 001 DF 15")
acm("load_user_edid", {"slot": 2}, "EDID 002 DL", **A500)

# Presets, groups and macros (ACM1000)
acm("recall_preset", {"preset": 3}, "PRE 03 AP", **A1000)
acm("trigger_macro", {"macro": 12}, "MACRO 12", **A1000)
acm("add_preset_route", {"preset": 3, "output": 5, "input": 2}, "PRE 03 ADD RX 005 TX 002", **A1000)
acm("remove_preset_route", {"preset": 3, "output": 5, "input": 2}, "PRE 03 RM RX 005 TX 002", **A1000)
acm("set_preset_enabled", {"preset": 3, "enabled": False}, "PRE 03 OFF", **A1000)
acm("set_group_enabled", {"group": 1, "enabled": True}, "GRP 01 ON", **A1000)
acm("add_group_output", {"group": 1, "output": 10}, "GRP 01 AD 010", **A1000)
acm("remove_group_output", {"group": 1, "output": 10}, "GRP 01 RM 010", **A1000)
acm("route_group", {"group": 1, "input": 6}, "GRP 01 FR 006", **A1000)

# Video walls
acm("create_video_wall", {"wall": 2, "columns": 2, "rows": 2, "name": "Wall"}, "VW 02 CREATE 02X02 Wall")
acm("rename_video_wall", {"wall": 2, "name": "Main Wall"}, "VW 02 NAME Main Wall")
acm("delete_video_wall", {"wall": 2}, "VW 02 DEL")
acm("place_video_wall_output", {"wall": 2, "output": 3, "column": 1, "row": 2}, "VW 02 OUT 003 H01V02")
acm("create_video_wall_config", {"wall": 2, "config": 1, "name": "Full"}, "VW 02 C 01 CREATE Full")
acm("rename_video_wall_config", {"wall": 2, "config": 1, "name": "Split"}, "VW 02 C 01 NAME Split")
acm("apply_video_wall_config", {"wall": 2, "config": 1}, "VW 02 C 01 APPLY")
acm("delete_video_wall_config", {"wall": 2, "config": 1}, "VW 02 C 01 DEL")
acm("set_video_wall_group_screen", {"wall": 2, "config": 1, "group": "B", "column": 2, "row": 1},
    "VW 02 C 01 G B H02V01")
acm("route_video_wall_group", {"wall": 2, "config": 1, "group": "A", "input": 4}, "VW 02 C 01 G A FR 004")
acm("set_video_wall_single_screen", {"wall": 2, "config": 1, "column": 1, "row": 1}, "VW 02 C 01 S H01V01")
acm("route_video_wall_single_screen", {"wall": 2, "config": 1, "column": 1, "row": 1, "input": 2},
    "VW 02 C 01 S H01V01 FR 002")
acm("set_video_wall_screen_width", {"wall": 2, "column": 1, "row": 1, "outer": 1000, "visible": 950},
    "VW 02 H01V01 OW1000 VW950")
acm("set_video_wall_screen_height", {"wall": 2, "column": 1, "row": 1, "outer": 600, "visible": 560},
    "VW 02 H01V01 OH600 VH560")
acm("get_video_wall_status", {"wall": 2}, "VW 02 STATUS")
acm("get_all_video_wall_status", {}, "VW STATUS")

# Project
acm("scan", {}, "SCAN")
acm("get_scan_status", {}, "SCAN STATUS")
acm("reset_scan", {}, "SCAN RESET")
acm("set_scan_osd", {"shown": True}, "SCAN OSD ON")
acm("reset_assignments", {}, "ASSIGN RESET")
acm("assign_default_input", {"input": 4}, "ASSIGN DF IN 004")
acm("assign_scanned_input", {"index": 2, "input": 4, "replace": True}, "ASSIGN INDEX 02 IN 004 REPLACE")
acm("assign_default_output", {"output": 9, "replace": True}, "ASSIGN DF OUT 009 REPLACE")
acm("assign_scanned_output", {"index": 7, "output": 9}, "ASSIGN INDEX 07 OUT 009")
acm("assign_auto", {}, "ASSIGN AUTO")

# GPIO
acm("set_gpio_direction", {"port": 1, "direction": "OUT"}, "GPIO 01 DIR OUT")
acm("set_gpio_output", {"port": 1, "high": True}, "GPIO 01 SET 1")
acm("read_gpio", {"port": 3}, "GPIO 03 GET")
acm("get_gpio_status", {}, "GPIO STATUS")
acm("set_gpio_vcc_voltage", {"volts": "12"}, "GPIO VCC SET VOLTS 12", **A1000)
acm("set_gpio_voltage", {"port": 2, "volts": "5"}, "GPIO 02 SET VOLTS 5", **A1000)
acm("set_gpio_current", {"port": 2, "high": False}, "GPIO 02 SET CURR 0", **A1000)

# Network
acm("set_control_dhcp", {"enabled": False}, "NET LAN2 DHCP OFF")
acm("set_ip_address", {"lan": "LAN2", "address": "10.0.0.225"}, "NET LAN2 IP 10.0.0.225")
acm("set_gateway", {"lan": "LAN2", "address": "10.0.0.1"}, "NET LAN2 GW 10.0.0.1")
acm("set_subnet_mask", {"lan": "LAN1", "address": "255.255.0.0"}, "NET LAN1 SM 255.255.0.0")
acm("network_reboot", {}, "NET RB")
acm("set_telnet", {"enabled": True}, "NET TN ON")
acm("set_telnet_port", {"port": 2323}, "NET TN 2323")
acm("set_ssh", {"enabled": False}, "NET SSH OFF")
acm("set_ssh_port", {"port": 2222}, "NET SSH 2222")
acm("set_web", {"enabled": True}, "NET WEB ON")
acm("set_https", {"enabled": True}, "NET HTTPS ON")
acm("set_dns_name", {"name": "acm.local"}, "NET DNS acm.local")


def acm_t(name, inbound, state, **extra):
    telemetry(ACM, name, inbound=inbound + "\r\n", expect_state=state, **extra)


acm_t("model", "IP Control Box ACM200 Status Info", {"model": "ACM200"})
acm_t("firmware", "FW Version: 1.14", {"firmware": "1.14"})
acm_t("system", "On      On      57600", {"power": True, "ir": True, "serial_baud": 57600})
acm_t("input-row", "001     DF009   169.254.003.001  On /On",
      {"inputs": {"1": {"edid": 9, "ip_address": "169.254.003.001", "online": True, "signal": True}}})
acm_t("output-row", "002     002     169.254.006.002  On /Off   00   VW02",
      {"outputs": {"2": {"input": 2, "ip_address": "169.254.006.002", "online": True, "connected": False,
                         "resolution": 0, "mode": "VW02"}}})
acm_t("video-lan", "01_POE  Off     169.254.002.225 169.254.002.001 255.255.000.000",
      {"network": {"video": {"dhcp": False, "ip_address": "169.254.002.225", "gateway": "169.254.002.001",
                             "subnet_mask": "255.255.000.000"}}})
acm_t("control-lan", "02_CTRL Off     010.000.000.225 010.000.000.001 255.255.000.000",
      {"network": {"control": {"dhcp": False, "ip_address": "010.000.000.225", "gateway": "010.000.000.001",
                               "subnet_mask": "255.255.000.000"}}})
acm_t("telnet-mac", "0023    34:D0:B8:20:4E:19       34:D0:B8:20:4E:1A",
      {"network": {"telnet_port": 23, "video": {"mac_address": "34:D0:B8:20:4E:19"},
                   "control": {"mac_address": "34:D0:B8:20:4E:1A"}}})
acm_t("output-detail", "001    On     Off   A7.3.0   VW      00       0       Receiver 001",
      {"outputs": {"1": {"online": True, "connected": False, "firmware": "A7.3.0", "mode": "VW", "resolution": 0,
                         "rotation": 0, "name": "Receiver 001"}}})
acm_t("input-detail", "001  On    On   A7.3.0  DF015   HDMI  On         Transmitter 001",
      {"inputs": {"1": {"online": True, "signal": True, "firmware": "A7.3.0", "edid": 15, "audio": "HDMI",
                        "multicast": True, "name": "Transmitter 001"}}})
