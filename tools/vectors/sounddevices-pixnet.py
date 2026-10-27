# Sound Devices / Video Devices PIXNET: every command a POST of {} under /sounddevices/, as the Bitfocus Companion
# module sends it (inferred; the official PIXNET Web API document was not retrievable).
PX = "sounddevices-pixnet"
_PX_OK = {"http_reply": {"status": 200, "body": ""}, "expect_result": {"ok": {"kind": "ack"}}}


def _px(command, input, target, **extra):
    V.append({"spec": PX, "command": command, "input": input,
              "expect_request": {"method": "POST", "target": target, "body": "{}"}, **extra})


_px("play", {}, "/sounddevices/settransport/play", **_PX_OK)
_px("stop", {}, "/sounddevices/settransport/stop", **_PX_OK)
_px("record", {}, "/sounddevices/settransport/rec",
    http_reply={"status": 500, "body": ""}, expect_result={"error": {"error": "device_error", "code": "500"}})
_px("fast_forward", {"speed": "2"}, "/sounddevices/invoke/RemoteApi/fastForwardPlay(PlaybackSpeed)/1/10,PlayX2")
_px("fast_reverse", {"speed": "16"}, "/sounddevices/invoke/RemoteApi/fastReversePlay(PlaybackSpeed)/1/10,PlayX16")
_px("false_take", {}, "/sounddevices/invoke/RemoteApi/falseTake()")
_px("jam_received_tc", {}, "/sounddevices/invoke/RemoteApi/jamReceivedTc()")
_px("jam_time_of_day", {}, "/sounddevices/invoke/RemoteApi/jamTimeOfDay()")
_px("key_press", {"key": "menu"}, "/sounddevices/invoke/RemoteApi/simulateKey(int,KeyEventType)/2/5,0x01000051/10,KeyPressAndRelease")
_px("key_press", {"key": "record", "event": "release"},
    "/sounddevices/invoke/RemoteApi/simulateKey(int,KeyEventType)/2/5,0x01000084/10,KeyRelease", file="key_press-release")
_px("close_message_box", {"button": "OK"}, "/sounddevices/invoke/RemoteApi/closeMessageBox(QString)/1/10,OK")
_px("set_dialog_dismiss", {"seconds": 2, "action": "accept"},
    "/sounddevices/invoke/RemoteApi/setAutoDismiss(int,DialogControl)/2/5,2/10,Accept")
_px("format_all_drives", {"label": "PIX"}, "/sounddevices/invoke/RemoteApi/formatAllDrives(QString,QString)/2/10,PIX/10,EXFAT")
_px("create_sound_report", {}, "/sounddevices/invoke/RemoteApi/createSoundReportCurrentReel()")
_px("set_drive_mode", {"drive": 3, "mode": "ethernet_transfer"},
    "/sounddevices/setsetting/RecordToDrive3=Ethernet%20File%20Transfer", model="pix-270i")
