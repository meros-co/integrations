# PiKVM (pikvm): one vector per command over the KVMD HTTP API, from the
# PiKVM HTTP API reference (docs.pikvm.org/api). Targets are the documented
# routes with their query parameters in the order the spec lists them,
# percent-encoded as query values; replies are the reference's example
# outputs ({"ok": true, "result": ...}). Websocket events are the reference's
# initial-event examples.
PK = "pikvm"
_PK_OK = '{"ok": true, "result": {}}'
_PK_ACK = {"ok": {"kind": "ack"}}


def _pk(command, input, method, target, body=None, **extra):
    request = {"method": method, "target": target}
    if body is not None:
        request["body"] = body
    V.append({"spec": PK, "command": command, "input": input, "expect_request": request, **extra})


def _pk_post(command, input, target, body=None):
    _pk(command, input, "POST", target, body,
        http_reply={"status": 200, "body": _PK_OK}, expect_result=_PK_ACK)


# ── System ────────────────────────────────────────────────────────────────
_pk("get_info", {}, "GET", "/api/info",
    http_reply={"status": 200, "body": '{"ok": true, "result": {"meta": {"server": {"host": "pikvm"}}}}'},
    expect_result={"ok": {"kind": "value", "value": {"meta": {"server": {"host": "pikvm"}}}}})
_pk("get_log", {"seek": 3600}, "GET", "/api/log?seek=3600")
_pk("get_metrics", {}, "GET", "/api/export/prometheus/metrics")
_pk("check_auth", {}, "GET", "/api/auth/check", http_reply={"status": 200, "body": _PK_OK}, expect_result=_PK_ACK)

# ── HID ───────────────────────────────────────────────────────────────────
_pk("get_hid", {}, "GET", "/api/hid")
_pk_post("set_keyboard_output", {"output": "usb"}, "/api/hid/set_params?keyboard_output=usb")
_pk_post("set_mouse_output", {"output": "usb_rel"}, "/api/hid/set_params?mouse_output=usb_rel")
# The reference's example: jiggler=0.
_pk_post("set_jiggler", {"enabled": False}, "/api/hid/set_params?jiggler=0")
_pk_post("set_hid_connected", {"connected": False}, "/api/hid/set_connected?connected=0")
_pk_post("reset_hid", {}, "/api/hid/reset")
_pk("get_keymaps", {}, "GET", "/api/hid/keymaps",
    http_reply={"status": 200, "body": '{"ok": true, "result": {"keymaps": {"available": ["de", "en-us"], "default": "de"}}}'},
    expect_result={"ok": {"kind": "value", "value": {"available": ["de", "en-us"], "default": "de"}}})
# The reference's example: "Einige Worte" in the de layout.
_pk("type_text", {"text": "Einige Worte", "keymap": "de"}, "POST", "/api/hid/print?limit=0&keymap=de", "Einige Worte",
    http_reply={"status": 200, "body": _PK_OK}, expect_result=_PK_ACK)
_pk("type_text_slow", {"text": "abc"}, "POST", "/api/hid/print?limit=0&keymap=en-us&slow=1&delay=0.020", "abc")
# ControlLeft,KeyL: the comma is percent-encoded in a query value.
_pk_post("send_shortcut", {"keys": "ControlLeft,KeyL"}, "/api/hid/events/send_shortcut?keys=ControlLeft%2CKeyL")
_pk_post("send_key", {"key": "Delete"}, "/api/hid/events/send_key?key=Delete")
_pk_post("key_down", {"key": "ShiftLeft"}, "/api/hid/events/send_key?key=ShiftLeft&state=1&finish=0")
_pk_post("key_up", {"key": "ShiftLeft"}, "/api/hid/events/send_key?key=ShiftLeft&state=0")
_pk_post("mouse_click", {"button": "left"}, "/api/hid/events/send_mouse_button?button=left")
_pk_post("mouse_button_down", {"button": "right"}, "/api/hid/events/send_mouse_button?button=right&state=1")
_pk_post("mouse_button_up", {"button": "right"}, "/api/hid/events/send_mouse_button?button=right&state=0")
_pk_post("mouse_move", {"x": 0, "y": 50}, "/api/hid/events/send_mouse_move?to_x=0&to_y=50")
_pk_post("mouse_move_relative", {"dx": 20, "dy": 200}, "/api/hid/events/send_mouse_relative?delta_x=20&delta_y=200")
_pk_post("mouse_wheel", {"dy": 200}, "/api/hid/events/send_mouse_wheel?delta_x=0&delta_y=200")

# ── ATX ───────────────────────────────────────────────────────────────────
_pk("get_atx", {}, "GET", "/api/atx")
_pk_post("atx_power", {"action": "on"}, "/api/atx/power?action=on&wait=0")
_pk_post("atx_click", {"button": "power"}, "/api/atx/click?button=power&wait=0")

# ── Mass storage drive ────────────────────────────────────────────────────
_pk("get_msd", {}, "GET", "/api/msd")
V.append({"spec": PK, "command": "msd_upload", "input": {"image": "test.iso", "data": "AAECAw=="},
          "expect_request": {"method": "POST", "target": "/api/msd/write?image=test.iso", "body_hex": "00010203",
                             "content_type": "application/octet-stream"}})
_pk_post("msd_upload_remote", {"url": "http://example.com/test.iso", "image": "test.iso"},
         "/api/msd/write_remote?url=http%3A%2F%2Fexample.com%2Ftest.iso&image=test.iso&timeout=10")
_pk_post("msd_set_image", {"image": "test.iso"}, "/api/msd/set_params?image=test.iso")
_pk_post("msd_set_media", {"cdrom": True}, "/api/msd/set_params?cdrom=1")
_pk_post("msd_set_writable", {"rw": True}, "/api/msd/set_params?rw=1")
_pk_post("msd_set_connected", {"connected": True}, "/api/msd/set_connected?connected=1")
_pk_post("msd_remove", {"image": "test.iso"}, "/api/msd/remove?image=test.iso")
_pk_post("msd_reset", {}, "/api/msd/reset")

# ── GPIO ──────────────────────────────────────────────────────────────────
_pk("get_gpio", {}, "GET", "/api/gpio")
# The reference's examples: led1 off without waiting; a 2 s pulse.
_pk_post("gpio_switch", {"channel": "led1", "state": False}, "/api/gpio/switch?channel=led1&state=0&wait=0")
_pk_post("gpio_pulse", {"channel": "led1", "delay": 2.0}, "/api/gpio/pulse?channel=led1&delay=2.000&wait=0")

# ── Streamer ──────────────────────────────────────────────────────────────
_pk("get_streamer", {}, "GET", "/api/streamer")
_pk("get_ocr_state", {}, "GET", "/api/streamer/ocr")
# The reference's OCR example: 1000 by 150 at the top left, English.
_pk("ocr_screen", {"langs": "eng", "left": 0, "top": 0, "right": 1000, "bottom": 150}, "GET",
    "/api/streamer/snapshot?ocr=1&ocr_langs=eng&ocr_left=0&ocr_top=0&ocr_right=1000&ocr_bottom=150",
    http_reply={"status": 200, "body": "Login:"}, expect_result={"ok": {"kind": "value", "value": "Login:"}})
_pk("delete_snapshot", {}, "DELETE", "/api/streamer/snapshot", http_reply={"status": 200, "body": _PK_OK},
    expect_result=_PK_ACK)

# ── PiKVM Switch ──────────────────────────────────────────────────────────
_pk("get_switch", {}, "GET", "/api/switch")
_pk_post("switch_previous", {}, "/api/switch/set_active_prev")
_pk_post("switch_next", {}, "/api/switch/set_active_next")
_pk_post("switch_set_active", {"port": "2,2"}, "/api/switch/set_active?port=2%2C2")
_pk_post("switch_set_beacon", {"port": "3", "state": True}, "/api/switch/set_beacon?port=3&state=1")
# The reference's example: port 2 named PORT3.
_pk_post("switch_set_port_name", {"port": 2, "name": "PORT3"}, "/api/switch/set_port_params?port=2&name=PORT3")
_pk_post("switch_set_port_edid", {"port": 2, "edid_id": "default"}, "/api/switch/set_port_params?port=2&edid_id=default")
_pk_post("switch_set_port_dummy", {"port": 2, "dummy": True}, "/api/switch/set_port_params?port=2&dummy=1")
_pk_post("switch_set_port_atx_delays", {"port": 0, "power": 0.5, "power_long": 5.5, "reset": 0.5},
         "/api/switch/set_port_params?port=0&atx_click_power_delay=0.50&atx_click_power_long_delay=5.50"
         "&atx_click_reset_delay=0.50")
# The reference's example: orange at 75 % brightness blinking every 40 ms.
_pk_post("switch_set_colors", {"beacon": "FFA500:BF:0028"}, "/api/switch/set_colors", "FFA500:BF:0028")
_pk_post("switch_reboot", {"unit": 0, "bootloader": True}, "/api/switch/reset?unit=0&bootloader=1")
_pk_post("switch_create_edid", {"name": "Gigabyte-GA-H77-DS3H", "data": "00FFFFFFFFFFFF00"},
         "/api/switch/edids/create?name=Gigabyte-GA-H77-DS3H&data=00FFFFFFFFFFFF00")
_pk_post("switch_change_edid", {"id": "ca3d7114-79ca-47fc-80b4-b80ac63fc329", "name": "Monitor", "data": "00FF"},
         "/api/switch/edids/change?id=ca3d7114-79ca-47fc-80b4-b80ac63fc329&name=Monitor&data=00FF")
_pk_post("switch_remove_edid", {"id": "ca3d7114-79ca-47fc-80b4-b80ac63fc329"},
         "/api/switch/edids/remove?id=ca3d7114-79ca-47fc-80b4-b80ac63fc329")
_pk_post("switch_atx_power", {"port": 0, "action": "reset_hard"}, "/api/switch/atx/power?port=0&action=reset_hard")
_pk_post("switch_atx_click", {"port": 0, "button": "power_long"}, "/api/switch/atx/click?port=0&button=power_long")

# ── Redfish ───────────────────────────────────────────────────────────────
_pk("redfish_get_system", {}, "GET", "/redfish/v1/Systems/0",
    http_reply={"status": 200, "body": '{"Id": "0", "PowerState": "Off"}'},
    expect_result={"ok": {"kind": "value", "value": {"Id": "0", "PowerState": "Off"}}})
# The reference's example: ForceRestart, answered 204.
_pk("redfish_reset", {"type": "ForceRestart"}, "POST", "/redfish/v1/Systems/0/Actions/ComputerSystem.Reset",
    '{"ResetType":"ForceRestart"}', http_reply={"status": 204, "body": ""}, expect_result=_PK_ACK)
_pk("redfish_insert_media", {"image": "https://releases.ubuntu.com/noble/ubuntu.iso"}, "POST",
    "/redfish/v1/Managers/BMC/VirtualMedia/MSD/Actions/VirtualMedia.InsertMedia",
    '{"Image":"https://releases.ubuntu.com/noble/ubuntu.iso","Inserted":true,"WriteProtected":true}')
_pk("redfish_eject_media", {}, "POST", "/redfish/v1/Managers/BMC/VirtualMedia/MSD/Actions/VirtualMedia.EjectMedia", "{}")

# ── Telemetry: the websocket's events ────────────────────────────────────
telemetry(PK, "ws-connect", expect_connect_ws=['{"event_type":"ping","event":{}}'],
          inbound_ws=json.dumps({"event_type": "atx_state",
                                 "event": {"enabled": True, "busy": False, "leds": {"power": False, "hdd": False}}}),
          expect_state={"atx": {"enabled": True, "busy": False, "power_led": False, "hdd_led": False}})
telemetry(PK, "ws-hid", inbound_ws=json.dumps({"event_type": "hid_state", "event": {
    "online": True, "keyboard": {"online": True, "leds": {"caps": False, "scroll": False, "num": True}},
    "mouse": {"online": True}}}),
    expect_state={"hid": {"online": True, "keyboard": {"online": True, "caps_lock": False, "num_lock": True,
                                                         "scroll_lock": False}, "mouse": {"online": True}}})
telemetry(PK, "ws-msd", inbound_ws=json.dumps({"event_type": "msd_state", "event": {
    "enabled": True, "online": True, "busy": False,
    "storage": {"size": 234950152192, "free": 23514271744, "images": {}, "uploading": False},
    "drive": {"image": None, "connected": False, "cdrom": True}, "features": {"multi": True, "cdrom": True}}}),
    state_before={"msd": {"drive": {"image": "old.iso"}, "storage": {"images": {"old.iso": {"size": 1}}}}},
    expect_state={"msd": {"enabled": True, "online": True, "busy": False, "storage": {},
                          "drive": {"connected": False, "cdrom": True, "image": ""}}})
telemetry(PK, "ws-msd-images", inbound_ws=json.dumps({"event_type": "msd_state", "event": {
    "enabled": True, "online": True, "busy": False,
    "storage": {"images": {"ubuntu.iso": {"size": 1024, "complete": True}}},
    "drive": {"image": {"name": "ubuntu.iso", "size": 1024, "complete": True}, "connected": True, "cdrom": True,
              "rw": False}}}),
    expect_state={"msd": {"enabled": True, "online": True, "busy": False,
                          "drive": {"connected": True, "cdrom": True, "rw": False, "image": "ubuntu.iso"},
                          "storage": {"images": {"ubuntu.iso": {"size": 1024, "complete": True}}}}})
telemetry(PK, "ws-gpio", inbound_ws=json.dumps({"event_type": "gpio_state", "event": {
    "inputs": {"led1": {"online": True, "state": False}},
    "outputs": {"relay1": {"online": False, "state": False, "busy": False}}}}),
    expect_state={"gpio": {"inputs": {"led1": {"online": True, "state": False}},
                           "outputs": {"relay1": {"online": False, "state": False, "busy": False}}}})
telemetry(PK, "ws-hw", inbound_ws=json.dumps({"event_type": "info_hw_state", "event": {
    "platform": {"type": "rpi", "base": "Virtual Raspberry Pi"},
    "health": {"temp": {"cpu": 36.511, "gpu": 35.0}, "throttling": {"raw_flags": 0, "parsed_flags": {
        "undervoltage": {"now": False, "past": False}, "freq_capped": {"now": False, "past": False},
        "throttled": {"now": False, "past": False}}}}}}),
    expect_state={"hw": {"platform_type": "rpi", "platform_base": "Virtual Raspberry Pi", "cpu_temp": 36.511,
                         "undervoltage": False, "throttled": False}})
telemetry(PK, "ws-system", inbound_ws=json.dumps({"event_type": "info_system_state", "event": {
    "kvmd": {"version": "1.102"}, "streamer": {"app": "ustreamer", "version": "1.25"},
    "kernel": {"system": "Linux", "release": "5.8.10-arch1-1"}}}),
    expect_state={"system": {"kvmd_version": "1.102", "streamer_app": "ustreamer", "streamer_version": "1.25",
                             "kernel_release": "5.8.10-arch1-1"}})
telemetry(PK, "ws-streamer", inbound_ws=json.dumps({"event_type": "streamer_state", "event": {
    "limits": {"max_fps": 40}, "params": {"desired_fps": 30, "quality": 80}, "snapshot": {"saved": None},
    "streamer": None, "features": {"quality": True, "resolution": False}}}),
    expect_state={"streamer": {"online": False, "desired_fps": 30, "quality": 80}})
telemetry(PK, "http-atx", inbound_http={"path": "/api/atx", "body": json.dumps(
    {"ok": True, "result": {"busy": False, "enabled": True, "leds": {"hdd": False, "power": True}}})},
    expect_state={"atx": {"enabled": True, "busy": False, "power_led": True, "hdd_led": False}})
