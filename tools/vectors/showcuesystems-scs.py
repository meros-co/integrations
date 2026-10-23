# SCS network cue control (showcuesystems-scs): one vector per command, from
# the OSC Message Format column of SCS 11's "Cue Control Devices - Network"
# table (without the optional /scs prefix), with its type tags: ,s for cue
# labels, hotkeys and devices, ,f for dB levels. /ctrl/fadeall comes from the
# Bitfocus Companion module. SCS never answers, so every command is
# unverified.
SC = "showcuesystems-scs"
_SU = {"expect_result": {"ok": {"kind": "unverified"}}}


def sc(command, input, address, *args, **extra):
    binary(SC, command, input, osc(address, *args), **extra)


sc("go", {}, "/ctrl/go", **_SU)
sc("stop_all", {}, "/ctrl/stopall")
sc("pause_resume_all", {}, "/ctrl/pauseresumeall")
sc("go_to_top", {}, "/ctrl/gotop")
sc("go_back", {}, "/ctrl/goback")
sc("go_to_next", {}, "/ctrl/gotonext")
sc("go_to_end", {}, "/ctrl/gotoend")
sc("go_to_cue", {"cue": "Q90"}, "/ctrl/goto", ("s", "Q90"))
sc("play_cue", {"cue": "Q90"}, "/cue/go", ("s", "Q90"), **_SU)
sc("stop_cue", {"cue": "Q90"}, "/cue/stop", ("s", "Q90"))
sc("pause_resume_cue", {"cue": "Q12"}, "/cue/pauseresume", ("s", "Q12"))
sc("hotkey", {"key": "A"}, "/hkey/go", ("s", "A"))
sc("hotkey_on", {"key": "B"}, "/hkey/on", ("s", "B"))
sc("hotkey_off", {"key": "B"}, "/hkey/off", ("s", "B"))
sc("set_master_fader", {"db": -12.0}, "/fader/setmaster", ("f", -12.0))
sc("adjust_master_fader", {"db": -3.0}, "/fader/setmasterrel", ("f", -3.0))
sc("set_device_fader", {"device": "Front", "db": -8.5}, "/fader/setdevice", ("s", "Front"), ("f", -8.5))
sc("adjust_device_fader", {"device": "Front", "db": 1.5}, "/fader/setdevicerel", ("s", "Front"), ("f", 1.5))
sc("fade_all", {}, "/ctrl/fadeall")
