WLS = "workpro-lightshark"
# LightShark OSC (User Manual v1.4, 7.2 OSC): /LS/... with one float, 1.0 a
# press and 0.0 a release. The manual's own hexadecimal table is the check:
# "Page Up 2f4c532f506167652f5570002c66000000000000", "DBO push =
# 2f4c532f44424f002c6600003f800000", "Executor Toggle Mode
# 2f4c532f4578656375746f722f312f312f3100002c66000000000000", "Sync All
# 2f4c532f53796e63000000002c66000000000000".


def wls(command, input, *messages, **extra):
    wires = [osc(*m) for m in messages]
    binary(WLS, command, input, wires if len(wires) > 1 else wires[0], **extra)


REL = ("f", 0.0)
PUSH = ("f", 1.0)

assert hexs(osc("/LS/Page/Up", REL)) == "2f4c532f506167652f5570002c66000000000000"
assert hexs(osc("/LS/DBO", PUSH)) == "2f4c532f44424f002c6600003f800000"
assert hexs(osc("/LS/Executor/1/1/1", REL)) == "2f4c532f4578656375746f722f312f312f3100002c66000000000000"

wls("press_key", {"key": "page_up"}, ("/LS/Page/Up", REL), expect_result={"ok": {"kind": "unverified"}})
wls("press_key", {"key": "color"}, ("/LS/Color", REL), file="press_key-color")
wls("press_key", {"key": "sync_executors"}, ("/LS/Sync/Executors", REL), file="press_key-sync-executors")
wls("key_down", {"key": "dbo"}, ("/LS/DBO", PUSH))
wls("key_up", {"key": "dbo"}, ("/LS/DBO", REL))
# "To press Go on Playback number 9: /LS/Go/PB/9".
wls("playback_key", {"action": "go", "playback": 9}, ("/LS/Go/PB/9", REL))
wls("playback_key_down", {"action": "flash", "playback": 1}, ("/LS/Flash/PB/1", PUSH))
wls("playback_key_up", {"action": "flash", "playback": 1}, ("/LS/Flash/PB/1", REL))
# "To Adjust Fader Level on PB 17: /LS/Level/PB/17".
wls("set_playback_level", {"playback": 17, "level": 128.0}, ("/LS/Level/PB/17", ("f", 128.0)))
wls("set_playback_chase_speed", {"playback": 2, "speed": 120.0}, ("/LS/ChaseValue/PB/2", ("f", 120.0)))
wls("set_grand_master", {"level": 255.0}, ("/LS/Level/GM", ("f", 255.0)))
wls("set_master_level", {"master": "fx_size", "level": 64.0}, ("/LS/Level/SmSize", ("f", 64.0)))
wls("reset_master_level", {"master": "chase_speed"}, ("/LS/Reset/SmChase", REL))
# "To Adjust parameters using Encoder B: /LS/Encoder/2".
wls("encoder", {"encoder": 2, "value": -1.0}, ("/LS/Encoder/2", ("f", -1.0)))
# "To Trigger Executor Position 2-2 /LS/Executor/1/2/2".
wls("executor", {"page": 1, "x": 2, "y": 2}, ("/LS/Executor/1/2/2", REL))
wls("executor_down", {"page": 1, "x": 7, "y": 1}, ("/LS/Executor/1/7/1", PUSH))
wls("executor_up", {"page": 1, "x": 7, "y": 1}, ("/LS/Executor/1/7/1", REL))
wls("executor_row", {"row": 1}, ("/LS/ExecutorLine/1", PUSH), ("/LS/ExecutorLine/1", REL))

telemetry(WLS, "grand-master", inbound_hex=hexs(osc("/LS/Level/GM", ("f", 200.0))),
          expect_state={"grand_master": 200.0})
telemetry(WLS, "master-chase", inbound_hex=hexs(osc("/LS/Level/SmChase", ("f", 128.0))),
          expect_state={"masters": {"chase_speed": 128.0}})
telemetry(WLS, "master-fx-size", inbound_hex=hexs(osc("/LS/Level/SmSize", ("f", 10.0))),
          expect_state={"masters": {"fx_size": 10.0}})
telemetry(WLS, "master-fx-speed", inbound_hex=hexs(osc("/LS/Level/SmSpeed", ("f", 20.0))),
          expect_state={"masters": {"fx_speed": 20.0}})
telemetry(WLS, "playback-level", inbound_hex=hexs(osc("/LS/Level/PB/3", ("f", 255.0))),
          expect_state={"playbacks": {"3": {"level": 255.0}}})
telemetry(WLS, "executor", inbound_hex=hexs(osc("/LS/Executor/2/8/6", ("i", 1))),
          expect_state={"executors": {"2": {"8": {"6": {"active": True}}}}})
