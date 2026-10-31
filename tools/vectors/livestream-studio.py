# Livestream Studio 6 (livestream-studio): the commands and messages the
# MIT-licensed Companion module (companion-module-vimeo-livestreamstudio6)
# sends and reads; a line feed ends each command, inputs, graphics stacks and
# media players are 0-based on the wire. Studio answers no command, so every
# command is unverified.
LS = "livestream-studio"
LS_U = {"ok": {"kind": "unverified"}}


def ls(command, input, line, **extra):
    text(LS, command, input, line + "\n", **extra)


ls("set_program", {"input": 3}, "SPmI:2", expect_result=LS_U)
ls("set_preview", {"input": 1}, "SPrI:0")
ls("cut", {}, "RCut")
ls("auto", {}, "RAuto")
ls("fade_in", {}, "RFIn")
ls("fade_out", {}, "RFOut")
ls("push_graphics", {"stack": 1}, "RGMOS:0")
ls("pull_graphics", {"stack": 2}, "RGMOH:1")
ls("show_graphics_preview", {"stack": 3}, "RGPvS:2")
ls("hide_graphics_preview", {"stack": 3}, "RGMPvH:2")
ls("play_media", {"input": 4}, "RMFP:3")
ls("play_media_in_out", {"input": 4}, "RMIOP:3")
ls("pause_media", {"input": 4}, "RMPause:3")
ls("set_input_volume", {"input": 2, "level": -6000}, "SIVL:1:-6000")
ls("adjust_input_volume", {"input": 2, "step": 1000}, "IVL:1:1000")
ls("set_input_gain", {"input": 2, "gain": 5000}, "SIGL:1:5000")
ls("set_input_mute", {"input": 2, "muted": True}, "IAM:1:on")
ls("set_input_audio_to_program", {"input": 2, "mode": "program"}, "IAP:1:2")
ls("set_input_headphones", {"input": 2, "enabled": False}, "IAH:1:0")
ls("set_master_volume", {"master": "stream", "level": 0}, "SSVL:0")
ls("set_master_volume", {"master": "record", "level": -10000}, "SRVL:-10000", file="set_master_volume-record")
ls("adjust_master_volume", {"master": "record", "step": -500}, "RVL:-500")
ls("set_master_mute", {"master": "stream", "muted": True}, "SM:1")
ls("set_master_headphones", {"master": "record", "enabled": True}, "RH:1")
ls("start_recording", {}, "RecStart")
ls("stop_recording", {}, "RecStop")
ls("start_streaming", {}, "StrStart")
ls("stop_streaming", {}, "StrStop")


def ls_t(name, inbound, state, **extra):
    telemetry(LS, name, inbound=inbound + "\n", expect_state=state, **extra)


ls_t("input-count", "ILCC:4", {"input_count": 4})
ls_t("input", 'ILC:0:"Camera 1":-2500:5000:0:1:2:1',
     {"inputs": {"1": {"name": "Camera 1", "volume": -2500, "gain": 5000, "muted": False, "headphones": True,
                       "audio_to_program": "program", "type": 1}}})
ls_t("input-name", 'INC:2:"Wide"', {"inputs": {"3": {"name": "Wide"}}})
ls_t("program", "PmIS:2", {"program": 3})
ls_t("preview", "PwIS:0", {"preview": 1})
ls_t("input-volume", "AVC:1:-12000", {"inputs": {"2": {"volume": -12000}}})
ls_t("input-gain", "AGC:1:2500", {"inputs": {"2": {"gain": 2500}}})
ls_t("input-mute", "AMC:1:1", {"inputs": {"2": {"muted": True}}})
ls_t("input-headphones", "ASC:1:0", {"inputs": {"2": {"headphones": False}}})
ls_t("input-audio-to-program", "AOC:1:1", {"inputs": {"2": {"audio_to_program": "always"}}})
ls_t("media-full", "MFP:3", {"inputs": {"4": {"media": "full"}}})
ls_t("media-in-out", "MIOP:3", {"inputs": {"4": {"media": "in_out"}}})
ls_t("media-paused", "MPause:3", {"inputs": {"4": {"media": "paused"}}})
ls_t("graphics-on", "GMOn:0", {"graphics": {"1": {"active": True}}})
ls_t("graphics-off", "GMOff:1", {"graphics": {"2": {"active": False}}})
ls_t("graphics-pushed", "GMOS:0:1", {"graphics": {"1": {"pushed": True}}})
ls_t("graphics-pulled", "GMOH:0:0", {"graphics": {"1": {"pushed": False}}})
ls_t("graphics-preview", "GMPvS:2:1", {"graphics": {"3": {"preview": True}}})
ls_t("graphics-preview-hidden", "GMPvH:2:0", {"graphics": {"3": {"preview": False}}})
ls_t("graphics-push-available", "GPA:1:1", {"graphics": {"2": {"push_available": True}}})
ls_t("stream-master", "SVC:-3000", {"masters": {"stream": {"volume": -3000}}})
ls_t("stream-mute", "SMC:1", {"masters": {"stream": {"muted": True}}})
ls_t("stream-headphones", "SSC:0", {"masters": {"stream": {"headphones": False}}})
ls_t("record-master", "RVC:0", {"masters": {"record": {"volume": 0}}})
ls_t("record-mute", "RMC:0", {"masters": {"record": {"muted": False}}})
ls_t("record-headphones", "RSC:1", {"masters": {"record": {"headphones": True}}})
ls_t("fade", "FOut", {"fade": "out"})
ls_t("streaming", "StrStarting", {"streaming": "starting"})
ls_t("recording", "RecStopped", {"recording": "stopped"})
