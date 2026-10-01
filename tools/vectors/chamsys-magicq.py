CQ = "chamsys-magicq"
# ChamSys MagicQ (MagicQ Manual 1.9.9.x, 2026-09-23) and QuickQ (QuickQ Manual
# 12.0): OSC 1.0 over UDP, one message per datagram, never acknowledged.
# Addresses are typed from the OSC Addresses table (MagicQ pp. 533-535,
# QuickQ p. 73); /rpc strings are typed from the ChamSys Remote Protocol
# Commands tables (MagicQ pp. 471-474), using the manual's own examples where
# it has them ("4,50I", "8,2,50J", "1A2A1S2G3,4I", "testosc /pb/1/go",
# "testosc /pb/1,100"). Not taken from the spec.


def cq(command, input, address, *args, **extra):
    binary(CQ, command, input, osc(address, *args), **extra)


def cqrpc(command, input, text, **extra):
    cq(command, input, "/rpc", ("s", text), **extra)


# ── Playbacks (MagicQ and QuickQ) ─────────────────────────────────────────
# testosc /pb/1,100 sets Playback 1 to 100%.
cq("pb_level", {"playback": 1, "level": 100}, "/pb/1", ("i", 100),
   expect_result={"ok": {"kind": "unverified"}})
# testosc /pb/1/go
cq("pb_go", {"playback": 1}, "/pb/1/go", expect_result={"ok": {"kind": "unverified"}})
cq("pb_flash", {"playback": 3, "enabled": True}, "/pb/3/flash", ("i", 1))
cq("pb_pause", {"playback": 10}, "/pb/10/pause")
cq("pb_release", {"playback": 2}, "/pb/2/release")
# <cue> := number with optional decimal, e.g. 2.5
cq("pb_jump_cue", {"playback": 8, "cue": "2.5"}, "/pb/8/2.5")

# ── Console-wide ──────────────────────────────────────────────────────────
# /dbo: "0 turns on, non-zero turns off".
cq("set_blackout", {"blackout": True}, "/dbo", ("i", 0))
# /swap: 0 = add, non-zero = swap.
cq("set_swap_mode", {"swap": True}, "/swap", ("i", 1))

# ── Execute Window ────────────────────────────────────────────────────────
# /exec/<page>/<item>: no args activate, 0 release, 1 activate, float level.
cq("exec_activate_page", {"page": "1", "item": "4x3"}, "/exec/1/4x3")
cq("exec_set_page", {"page": "2", "item": "5", "active": False}, "/exec/2/5", ("i", 0))
cq("exec_level_page", {"page": "1", "item": "12", "level": 0.5}, "/exec/1/12", ("f", 0.5))
cq("exec_activate", {"item": "7"}, "/exec/7")
cq("exec_set", {"item": "7", "active": True}, "/exec/7", ("i", 1))
cq("exec_level", {"item": "3x2", "level": 0.25}, "/exec/3x2", ("f", 0.25))

# ── 10Scene ───────────────────────────────────────────────────────────────
cq("tenscene_activate_zone", {"item": 4, "zone": 2}, "/10Scene/4/2")
cq("tenscene_set_zone", {"item": 4, "zone": 2, "active": False}, "/10Scene/4/2", ("i", 0))
cq("tenscene_toggle_zone", {"item": 10, "zone": 20}, "/10Scene/10/20/toggle")
cq("tenscene_activate", {"item": 1}, "/10Scene/1")
cq("tenscene_set", {"item": 1, "active": True}, "/10Scene/1", ("i", 1))
cq("tenscene_toggle", {"item": 6}, "/10Scene/6/toggle")
# QuickQ writes the address in lower case.
cq("quickq_tenscene_activate_zone", {"item": 3, "zone": 1}, "/10scene/3/1")
cq("quickq_tenscene_set_zone", {"item": 3, "zone": 1, "active": False}, "/10scene/3/1", ("i", 0))
cq("quickq_tenscene_activate", {"item": 9}, "/10scene/9")
cq("quickq_tenscene_set", {"item": 9, "active": True}, "/10scene/9", ("i", 1))

# ── Feedback ──────────────────────────────────────────────────────────────
cq("feedback_off", {}, "/feedback/off")
cq("feedback_playbacks", {}, "/feedback/pb")
cq("feedback_executes", {}, "/feedback/exec")
cq("feedback_all", {}, "/feedback/pb+exec")

# ── /rpc: Remote Protocol commands as an OSC string ───────────────────────
cqrpc("rpc", {"commands": "1A2A1S2G3,4I"}, "1A2A1S2G3,4I")
# <playback number> A/R/T/U/G/S/B/F
cqrpc("rpc_activate", {"playback": 1}, "1A")
cqrpc("rpc_release", {"playback": 1}, "1R")
cqrpc("rpc_test", {"playback": 2}, "2T")
cqrpc("rpc_untest", {"playback": 2}, "2U")
cqrpc("rpc_go", {"playback": 2}, "2G")
cqrpc("rpc_stop", {"playback": 1}, "1S")
cqrpc("rpc_fast_back", {"playback": 5}, "5B")
cqrpc("rpc_fast_forward", {"playback": 202}, "202F")
cqrpc("rpc_level", {"playback": 4, "level": 50}, "4,50L")
# "To jump to Cue id 2.5 on playback 8 you would use: 8,2,50J"
cqrpc("rpc_jump_cue", {"playback": 8, "cue": 2, "cue_dec": 50}, "8,2,50J")
cqrpc("rpc_page", {"page": 3}, "3P")
# "to set dimmer channel 4 to 50% you would use: 4,50I"
cqrpc("rpc_channel_intensity", {"channel": 4, "level": 50}, "4,50I")
cqrpc("rpc_tenscene_button", {"button": 5}, "5X")
cqrpc("rpc_tenscene_button_state", {"button": 5, "state": "4"}, "5,4X")
cqrpc("rpc_tenscene_zone_button_state", {"zone": 2, "button": 5, "state": "10"}, "2,5,10X")

# Remote programming commands: <number>, <params> H.
cqrpc("rpc_select_head", {"head": 1}, "01,1H")
cqrpc("rpc_select_head_range", {"start": 1, "end": 12}, "01,1,12H")
cqrpc("rpc_deselect_head", {"head": 3}, "02,3H")
cqrpc("rpc_deselect_head_range", {"start": 3, "end": 6}, "02,3,6H")
cqrpc("rpc_deselect_all", {}, "03H")
cqrpc("rpc_select_group", {"group": 200}, "04,200H")
cqrpc("rpc_set_intensity", {"level": 75}, "05,75H")
cqrpc("rpc_set_intensity_timed", {"level": 0, "time": 5}, "05,0,5H")
# Attribute numbers: Pan (4), Tilt (5), Cyan (16), Gobo1 (8), Zoom (13).
cqrpc("rpc_set_attribute", {"attribute": 4, "value": 128}, "06,4,128H")
cqrpc("rpc_set_attribute_timed", {"attribute": 5, "value": 64, "time": 3}, "06,5,64,3H")
cqrpc("rpc_increase_attribute", {"attribute": 16, "value": 10}, "07,16,10H")
cqrpc("rpc_increase_attribute_res", {"attribute": 4, "value": 256, "high_res": True}, "07,4,256,1H")
cqrpc("rpc_decrease_attribute", {"attribute": 8, "value": 1}, "08,8,1H")
cqrpc("rpc_decrease_attribute_res", {"attribute": 13, "value": 5, "high_res": False}, "08,13,5,0H")
cqrpc("rpc_clear_programmer", {}, "09H")
cqrpc("rpc_include_position_palette", {"palette": 1}, "10,1H")
cqrpc("rpc_include_colour_palette", {"palette": 2}, "11,2H")
cqrpc("rpc_include_beam_palette", {"palette": 1024}, "12,1024H")
cqrpc("rpc_include_cue", {"cue": 7}, "13,7H")
cqrpc("rpc_update", {}, "19H")
cqrpc("rpc_record_position_palette", {"palette": 5}, "20,5H")
cqrpc("rpc_record_colour_palette", {"palette": 6}, "21,6H")
cqrpc("rpc_record_beam_palette", {"palette": 7}, "22,7H")
cqrpc("rpc_record_cue", {"cue": 12}, "23,12H")
cqrpc("rpc_next_head", {}, "30H")
cqrpc("rpc_previous_head", {}, "31H")
cqrpc("rpc_all_heads", {}, "32H")
cqrpc("rpc_locate", {}, "40H")
cqrpc("rpc_lamp_on", {}, "41H")
cqrpc("rpc_lamp_off", {}, "42H")
cqrpc("rpc_reset_heads", {}, "43H")
cqrpc("rpc_remote_trigger", {"state": "2"}, "71,2H")
cqrpc("rpc_test_cue", {"cue": 10000}, "80,10000H")
cqrpc("rpc_untest_cue", {"cue": 1}, "81,1H")
cqrpc("rpc_test_cue_stack", {"stack": 3}, "82,3H")
cqrpc("rpc_test_cue_stack_level", {"stack": 3, "level": 50}, "82,3,50H")
cqrpc("rpc_test_cue_stack_level_cue", {"stack": 3, "level": 100, "cue": 4}, "82,3,100,4H")
cqrpc("rpc_untest_cue_stack", {"stack": 3}, "83,3H")
# <show file id>: four digit decimal number between 0000 and 9999.
cqrpc("rpc_save_show", {"show": 12}, "90,0012H")
cqrpc("rpc_load_show", {"show": 9999}, "91,9999H")
cqrpc("rpc_load_import", {"file": 1}, "92,0001H")
cqrpc("rpc_load_grid", {"grid": 0}, "93,0000H")
# \<94> , <shutdown type> , 81, 117 , 105, 116 H ; 3 is reboot.
cqrpc("rpc_shutdown", {"mode": "3"}, "94,3,81,117,105,116H")
cqrpc("rpc_emergency_hot_takeover", {"enabled": True}, "112,1H")

# ── Telemetry ─────────────────────────────────────────────────────────────
# /feedback/pb+exec on connecting (TouchOSC demo's Load vals button).
telemetry(CQ, "subscribe-on-connect", expect_connect_wire_hex=[hexs(osc("/feedback/pb+exec"))],
          inbound_hex=hexs(osc("/pb/1", ("f", 1.0))), expect_state={"playbacks": {"1": {"level": 1.0}}})
# Feedback shapes from the community Companion modules (ChamSys documents none).
telemetry(CQ, "playback-level", inbound_hex=hexs(osc("/pb/4", ("f", 0.5))),
          expect_state={"playbacks": {"4": {"level": 0.5}}})
telemetry(CQ, "playback-flash", inbound_hex=hexs(osc("/pb/2/flash", ("i", 1))),
          expect_state={"playbacks": {"2": {"flash": 1.0}}})
telemetry(CQ, "execute-level", inbound_hex=hexs(osc("/exec/1/5", ("f", 0.75))),
          expect_state={"executes": {"1": {"5": {"level": 0.75}}}})
