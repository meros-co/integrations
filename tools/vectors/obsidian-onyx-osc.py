OX = "obsidian-onyx-osc"
# Obsidian ONYX OSC MAPPING V1.20 Revision 5: OSC 1.0 over UDP, never
# acknowledged. Addresses are typed from the mapping's Execute Address tables
# (buttons up/down 0 or 1, faders float 0 to 255) and its playback page
# examples ("/Mx/playback/page1/0/go 1", "/Mx/playback/page5/9/release 1").
# Not taken from the spec.


def ox(command, input, *messages, **extra):
    wires = [osc(address, *args) for address, *args in messages]
    binary(OX, command, input, wires if len(wires) > 1 else wires[0], **extra)


def tap(address):
    return (address, ("i", 1)), (address, ("i", 0))


# ── Main playbacks: 1-10 at 42x1..42x5, 11-20 at 46x1..46x5 ───────────────
ox("playback_a", {"playback": 1, "pressed": True}, ("/Mx/button/4201", ("i", 1)),
   expect_result={"ok": {"kind": "unverified"}})
ox("playback_a_press", {"playback": 10}, *tap("/Mx/button/4291"))
ox("playback_b", {"playback": 2, "pressed": False}, ("/Mx/button/4212", ("i", 0)))
ox("playback_b_press", {"playback": 3}, *tap("/Mx/button/4222"))
ox("playback_c", {"playback": 4, "pressed": True}, ("/Mx/button/4234", ("i", 1)))
ox("playback_c_press", {"playback": 5}, *tap("/Mx/button/4244"))
ox("playback_d", {"playback": 6, "pressed": True}, ("/Mx/button/4255", ("i", 1)))
ox("playback_d_press", {"playback": 7}, *tap("/Mx/button/4265"))
ox("playback_level", {"playback": 1, "level": 255.0}, ("/Mx/fader/4203", ("f", 255.0)))
ox("upper_playback_a", {"playback": 11, "pressed": True}, ("/Mx/button/4601", ("i", 1)))
ox("upper_playback_a_press", {"playback": 20}, *tap("/Mx/button/4691"))
ox("upper_playback_b", {"playback": 12, "pressed": True}, ("/Mx/button/4612", ("i", 1)))
ox("upper_playback_b_press", {"playback": 13}, *tap("/Mx/button/4622"))
ox("upper_playback_c", {"playback": 14, "pressed": False}, ("/Mx/button/4634", ("i", 0)))
ox("upper_playback_c_press", {"playback": 15}, *tap("/Mx/button/4644"))
ox("upper_playback_d", {"playback": 16, "pressed": True}, ("/Mx/button/4655", ("i", 1)))
ox("upper_playback_d_press", {"playback": 17}, *tap("/Mx/button/4665"))
ox("upper_playback_level", {"playback": 20, "level": 127.5}, ("/Mx/fader/4693", ("f", 127.5)))

# Playback pages: page 1-100, button index 0-99, the action in the address.
ox("playback_page_action", {"page": 1, "button": 1, "action": "go"}, ("/Mx/playback/page1/0/go", ("i", 1)))

# ── Masters ──────────────────────────────────────────────────────────────
ox("grand_master_level", {"level": 255.0}, ("/Mx/fader/2202", ("f", 255.0)))
ox("flash_master_level", {"level": 0.0}, ("/Mx/fader/2212", ("f", 0.0)))
ox("group_master_a_level", {"level": 64.0}, ("/Mx/fader/2222", ("f", 64.0)))
ox("group_master_b_level", {"level": 200.0}, ("/Mx/fader/2232", ("f", 200.0)))

# ── Buttons ──────────────────────────────────────────────────────────────
ox("button", {"id": "5513", "pressed": True}, ("/Mx/button/5513", ("i", 1)))
ox("button_press", {"id": "56A1"}, *tap("/Mx/button/56A1"))
for name, num in [("select", "5502"), ("release", "5503"), ("beat", "5504"), ("snap", "5511"),
                  ("pause_back", "5512"), ("go", "5513"), ("view", "4121"), ("bank_page_up", "4412"),
                  ("bank_page_down", "4413"), ("fader_swap", "4600"), ("fade", "4321"), ("delay", "4322"),
                  ("snapshot", "4331"), ("bank", "4332"), ("macro", "2001"), ("preview", "2002"),
                  ("menu", "2003"), ("edit", "5101"), ("undo", "5102"), ("clear", "5103"),
                  ("copy", "5104"), ("move", "5106"), ("delete", "5107"), ("record", "5401"),
                  ("update", "5402"), ("load", "5411"), ("group", "5412"), ("cue", "5413"),
                  ("minus", "5210"), ("plus", "5211"), ("dot", "5212"), ("enter", "5213"),
                  ("slash", "5214"), ("backspace", "5215"), ("at", "5216"), ("full", "5301"),
                  ("through", "5302"), ("f1", "5601"), ("f2", "56A1"), ("f3", "5602"), ("f4", "56A2"),
                  ("f5", "5603"), ("f6", "56A3"), ("f7", "2101"), ("f8", "21A1"), ("f9", "2102"),
                  ("f10", "21A2"), ("f11", "2103"), ("f12", "21A3"), ("last", "6401"), ("next", "6402"),
                  ("swap_programmer", "6411"), ("highlight", "6001"), ("cv", "6003"), ("link", "6108"),
                  ("trackfunc_pan_tilt", "7001"), ("mode", "7004"), ("navigate_up", "7301"),
                  ("navigate_left", "7302"), ("navigate_down", "7303"), ("navigate_right", "7304")]:
    ox(f"press_{name}", {}, *tap(f"/Mx/button/{num}"))
ox("press_playback_bank", {"bank": 3}, *tap("/Mx/button/4423"))
ox("press_view_key", {"view": 8}, *tap("/Mx/button/1108"))
ox("press_view_key_9_16", {"view": 9}, *tap("/Mx/button/3101"))
ox("press_digit", {"digit": 0}, *tap("/Mx/button/5200"))
ox("press_pf_group", {"group": 2}, *tap("/Mx/button/5702"))
ox("press_base_channel_group", {"group": 5}, *tap("/Mx/button/6105"))
ox("press_effect_channel_group", {"group": 1}, *tap("/Mx/button/6201"))
ox("press_base_channel", {"channel": 3}, *tap("/Mx/button/6131"))
ox("press_effect_channel", {"channel": 4}, *tap("/Mx/button/6241"))
ox("bank_scroll_up", {}, *tap("/Mx/scroll/4110/up"))
ox("bank_scroll_down", {}, *tap("/Mx/scroll/4110/down"))
ox("bank_page_scroll_up", {}, *tap("/Mx/scroll/4411/up"))
ox("bank_page_scroll_down", {}, *tap("/Mx/scroll/4411/down"))
ox("pf_group_scroll_up", {}, *tap("/Mx/scroll/5706/up"))
ox("pf_group_scroll_down", {}, *tap("/Mx/scroll/5706/down"))
ox("device_space_up", {}, *tap("/Mx/configuration/deviceSpace/up"))
ox("device_space_down", {}, *tap("/Mx/configuration/deviceSpace/down"))

# ── Telemetry: update addresses ──────────────────────────────────────────
telemetry(OX, "playback-level", inbound_hex=hexs(osc("/Mx/fader/4203", ("f", 255.0))),
          expect_state={"playbacks": {"1": {"level": 255.0}}})
telemetry(OX, "upper-playback-level", inbound_hex=hexs(osc("/Mx/fader/4693", ("f", 0.0))),
          expect_state={"playbacks": {"20": {"level": 0.0}}})
telemetry(OX, "playback-name", inbound_hex=hexs(osc("/Mx/button/4214/text", ("s", "Front Wash"))),
          expect_state={"playbacks": {"2": {"name": "Front Wash"}}, "buttons": {"4214": {"text": "Front Wash"}}})
telemetry(OX, "playback-led", inbound_hex=hexs(osc("/Mx/button/4601/led", ("i", 1))),
          expect_state={"playbacks": {"11": {"a": {"led": 1}}}, "buttons": {"4601": {"led": 1}}})
telemetry(OX, "playback-blink", inbound_hex=hexs(osc("/Mx/button/4225/led/blink", ("i", 0))),
          expect_state={"playbacks": {"3": {"d": {"blink": 0}}}, "buttons": {"4225": {"blink": 0}}})
telemetry(OX, "button-led-colour", inbound_hex=hexs(osc("/Mx/button/5513/led/color", ("s", "green"))),
          expect_state={"buttons": {"5513": {"led_color": "green"}}})
telemetry(OX, "command-line", inbound_hex=hexs(osc("/Mx/commandLine/0001/text", ("s", "FREE"))),
          expect_state={"command_line": {"status": "FREE"}})
telemetry(OX, "command-line-command", inbound_hex=hexs(osc("/Mx/commandLine/0002/text", ("s", "Group 1 @"))),
          expect_state={"command_line": {"command": "Group 1 @"}})
telemetry(OX, "bank-label", inbound_hex=hexs(osc("/Mx/label/4401/text", ("s", "Bank 3"))),
          expect_state={"playback_bank": "Bank 3"})
telemetry(OX, "device-space", inbound_hex=hexs(osc("/Mx/configuration/deviceSpace", ("s", "2"))),
          expect_state={"device_space": "2"})
telemetry(OX, "belt", inbound_hex=hexs(osc("/Mx/belt/6112/channelName", ("s", "Dimmer"))),
          expect_state={"belts": {"6112": {"name": "Dimmer"}}})
telemetry(OX, "belt-value", inbound_hex=hexs(osc("/Mx/belt/6212/channelValue", ("s", "50%"))),
          expect_state={"belts": {"6212": {"value": "50%"}}})
