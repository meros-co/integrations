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
