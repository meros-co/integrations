# BrightSign UDP (brightsign-udp): one datagram per command, no line ending,
# as the UDP Event page writes a variable assignment (name:value, several
# separated by !!) and the sample control script's commands (PLAY file...).
BU = "brightsign-udp"
U = {"ok": {"kind": "unverified"}}


def _bu(command, input, wire, model=None):
    v = {"spec": BU, "command": command, "input": input, "expect_wire": wire, "expect_result": U}
    if model:
        v["model"] = model
    V.append(v)


_bu("send_message", {"message": "Reboot_Me"}, "Reboot_Me")
_bu("set_variable", {"name": "Score", "value": "3"}, "Score:3")
_bu("set_variables", {"assignments": "Score:3!!Team:Home"}, "Score:3!!Team:Home")
C = "control-script"
_bu("play", {"file": "intro.mp4"}, "PLAY intro.mp4", C)
_bu("play_clear", {"file": "intro.mp4"}, "PLAYCL intro.mp4", C)
_bu("loop", {"file": "loop.mp4"}, "LOOP loop.mp4", C)
_bu("loop_seamless", {"file": "loop.mp4"}, "LOOPS loop.mp4", C)
_bu("stop", {}, "STOP", C)
_bu("stop_clear", {}, "STOPCL", C)
_bu("pause", {}, "PAUSE", C)
_bu("resume", {}, "RESUME", C)
_bu("search", {"file": "intro.mp4"}, "SEARCH intro.mp4", C)
_bu("set_volume", {"percent": 80}, "VOLUME 80", C)
_bu("reboot", {}, "REBOOT", C)
_bu("fast_forward", {}, "FF", C)
_bu("rewind", {}, "RW", C)
_bu("get_status", {}, "STATUS", C)
_bu("get_version", {}, "VERSION", C)

# The sample control script's responses, broadcast to UDP 5001 with a line
# feed, as SendResponse writes them (v3.01E).
telemetry(BU, "status-playing", model=C, inbound_hex=hexs(b"Playing: INTRO.MP4\n"),
          expect_state={"playback": {"status": "Playing: INTRO.MP4", "state": "playing", "file": "INTRO.MP4"}})
telemetry(BU, "status-seamless", model=C, inbound_hex=hexs(b"BS3 Seamless Looping: LOOP.MP4\n"),
          expect_state={"playback": {"status": "Seamless Looping: LOOP.MP4", "state": "seamless_looping",
                                     "file": "LOOP.MP4"}})
telemetry(BU, "status-stopped", model=C, inbound_hex=hexs(b"Stopped\n"),
          expect_state={"playback": {"status": "Stopped", "state": "stopped", "file": ""}})
telemetry(BU, "status-attract", model=C, inbound_hex=hexs(b"Playing Attract\n"),
          expect_state={"playback": {"status": "Playing Attract", "state": "playing_attract", "file": ""}})
telemetry(BU, "code-play", model=C, inbound_hex=hexs(b"PLAY\n"),
          expect_state={"last_response": "PLAY"}, expect_then_send=["STATUS"])
telemetry(BU, "code-ended", model=C, inbound_hex=hexs(b"BS3 ENDP\n"),
          expect_state={"last_response": "ENDP", "last_event": "ENDP"}, expect_then_send=["STATUS"])
telemetry(BU, "code-ready", model=C, inbound_hex=hexs(b"READY\n"), expect_state={"last_response": "READY"})
telemetry(BU, "version", model=C, inbound_hex=hexs(b"XD1034 : 8.5.47\n"),
          expect_state={"player": {"model": "XD1034", "firmware": "8.5.47"}})
telemetry(BU, "script-version", model=C, inbound_hex=hexs(b"Control Script Version: v3.01E\n"),
          expect_state={"script": {"version": "v3.01E"}})
