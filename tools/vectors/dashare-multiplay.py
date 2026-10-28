# MultiPlay 3 OSC control (dashare-multiplay): one vector per command, the
# addresses and argument types of MultiPlay 3's online help ("OSC Control":
# Cue List Navigation, Cue Actions, Stopwatch; "OSC Remote > Status"). The
# help gives no default port; the vector runner opens the device on its own.
# MultiPlay never answers, so every command is unverified.
MP = "dashare-multiplay"
_MPU = {"expect_result": {"ok": {"kind": "unverified"}}}


def mp(command, input, address, *args, **extra):
    binary(MP, command, input, osc(address, *args), **extra)


mp("select", {"position": "first"}, "/select/first", **_MPU)
mp("select", {"position": "previous"}, "/select/prev", file="select-previous")
mp("select", {"position": "page_down"}, "/select/pagedown", file="select-page-down")
mp("select_cue", {"cue": "12.5"}, "/select/12.5")
mp("go", {}, "/cue/playhead/Go")
mp("go", {"cue": "7"}, "/cue/7/Go", file="go-cue")
mp("stop", {"cue": "active"}, "/cue/active/Stop")
mp("pause", {"cue": "3"}, "/cue/3/Pause")
mp("toggle_pause", {}, "/cue/playhead/PauseToggle")
mp("resume", {"cue": "active"}, "/cue/active/Resume")
mp("restart", {"cue": "current"}, "/cue/current/Restart")
mp("fade", {"cue": "active"}, "/cue/active/Fade")
mp("jump_back", {}, "/cue/playhead/JumpBack")
mp("jump_back_by", {"seconds": 5.0}, "/cue/playhead/JumpBack", ("f", 5.0))
mp("jump_forward", {"cue": "active"}, "/cue/active/JumpFwd")
mp("jump_forward_by", {"cue": "4", "seconds": 2.5}, "/cue/4/JumpFwd", ("f", 2.5))
mp("jump_to_end", {}, "/cue/playhead/JumpEnd")
mp("jump_to_end_by", {"seconds": 10.0}, "/cue/playhead/JumpEnd", ("f", 10.0))
mp("cue_point", {"point": "Next"}, "/cue/playhead/CuePoint", ("s", "Next"))
mp("cue_point", {"cue": "2", "point": "3"}, "/cue/2/CuePoint", ("s", "3"), file="cue_point-number")
mp("set_position", {"cue": "9", "seconds": 61.5}, "/cue/9/Position", ("f", 61.5))
mp("track", {"cue": "5", "track": "Last"}, "/cue/5/Track", ("s", "Last"))
mp("set_pan", {"pan": -40}, "/cue/playhead/Pan", ("i", -40))
mp("set_pan_scale", {"pan": 0.5}, "/cue/playhead/PanS", ("f", 0.5))
mp("pan_left", {"amount": 10}, "/cue/playhead/Pan/-", ("i", 10))
mp("pan_right", {"cue": "2", "amount": 25}, "/cue/2/Pan/+", ("i", 25))
mp("revert_pan", {}, "/cue/playhead/Pan/Revert")
mp("set_speed", {"percent": 120}, "/cue/playhead/Speed", ("i", 120))
mp("set_speed_scale", {"speed": -0.25}, "/cue/playhead/SpeedS", ("f", -0.25))
mp("slow_down", {"amount": 5}, "/cue/playhead/Speed/-", ("i", 5))
mp("speed_up", {"amount": 5}, "/cue/playhead/Speed/+", ("i", 5))
mp("revert_speed", {"cue": "8"}, "/cue/8/Speed/Revert")
mp("set_volume", {"db": -12}, "/cue/playhead/Volume", ("i", -12))
mp("set_volume_scale", {"level": 0.75}, "/cue/playhead/VolumeS", ("f", 0.75))
mp("volume_down", {}, "/cue/playhead/Volume/-")
mp("volume_down_by", {"db": 6}, "/cue/playhead/Volume/-", ("i", 6))
mp("volume_up", {"cue": "1"}, "/cue/1/Volume/+")
mp("volume_up_by", {"db": 3}, "/cue/playhead/Volume/+", ("i", 3))
mp("revert_volume", {}, "/cue/playhead/Volume/Revert")
mp("stopwatch", {"action": "start"}, "/Stopwatch/Start")
mp("stopwatch", {"action": "reset"}, "/Stopwatch/Reset", file="stopwatch-reset")
mp("refresh_status", {}, "/status/refresh")

# ── Telemetry: the status messages ──
telemetry(MP, "go-enabled", settings={"feedback_port": 53001},
          expect_connect_wire_hex=[hexs(osc("/status/refresh"))],
          inbound_hex=hexs(osc("/status/go", ("T", None))), expect_state={"buttons": {"go": True}})
for addr, key, arg, value in [("/status/select/prev", "previous", ("s", "FALSE"), False),
                              ("/status/select/next", "next", ("s", "TRUE"), True),
                              ("/status/stopall", "stop_all", ("F", None), False),
                              ("/status/fadeall", "fade_all", ("i", 1), True)]:
    telemetry(MP, f"button-{key.replace('_', '-')}", settings={"feedback_port": 53001},
              inbound_hex=hexs(osc(addr, arg)), expect_state={"buttons": {key: value}})
telemetry(MP, "qdesc", settings={"feedback_port": 53001},
          inbound_hex=hexs(osc("/status/current/qdesc", ("s", "12 Storm SFX"))),
          expect_state={"current": {"qdesc": "12 Storm SFX"}})
telemetry(MP, "elapsed", settings={"feedback_port": 53001},
          inbound_hex=hexs(osc("/status/elapsed", ("s", "00:01:12.4"))), expect_state={"elapsed": "00:01:12.4"})
telemetry(MP, "remaining", settings={"feedback_port": 53001},
          inbound_hex=hexs(osc("/status/remaining", ("s", "00:02:47.6"))), expect_state={"remaining": "00:02:47.6"})
