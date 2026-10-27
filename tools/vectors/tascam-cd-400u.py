TC = "tascam-cd-400u"
# TASCAM CD-400U / CD-400UDAB RS-232C/TELNET protocol (TEAC, Ver. 1.21): on the network the machine ID "0", a
# two-character command and its data, ended by CR LF; on RS-232C the same preceded by LF and ended by CR. Track
# numbers and minutes travel as tens, ones, thousands, hundreds ("2301" is 123).


def _tc(command, input, wire, **extra):
    text(TC, command, input, wire, **extra)


def _tcv(command, input, wire, reply, value, **extra):
    text(TC, command, input, wire, device_reply=reply + "\r\n",
         expect_result={"ok": {"kind": "value", "value": value}}, **extra)


_tc("stop", {}, "010\r\n", expect_result={"ok": {"kind": "unverified"}})
_tc("play", {}, "012\r\n")
_tc("ready", {}, "01401\r\n")
_tc("search", {"direction": "reverse_high"}, "01611\r\n")
_tc("eject", {}, "018\r\n")
_tc("skip", {"to": "previous"}, "01A01\r\n")
_tc("locate_track", {"track": 123}, "0232301\r\n")
_tc("locate_track", {"track": 12}, "0231200\r\n", file="locate_track-twelve")
_tc("select_preset", {"preset": 12}, "0231200\r\n")
_tc("select_preset", {"preset": 3}, "0230300\r\n", file="select_preset-three")
_tc("set_resume", {"enabled": True}, ["03401\r\n", "034FF\r\n"])
_tcv("get_resume", {}, "034FF\r\n", "0B401", "01")
_tc("set_repeat", {"enabled": False}, ["03700\r\n", "037FF\r\n"])
_tcv("get_repeat", {}, "037FF\r\n", "0B700", "00")
_tc("set_incremental_play", {"enabled": True}, ["03A01\r\n", "03AFF\r\n"])
_tcv("get_incremental_play", {}, "03AFF\r\n", "0BA01", "01")
_tc("clear", {}, "04A\r\n")
_tc("set_remote_local", {"mode": "serial_only"}, ["04C10\r\n", "04CFF\r\n"])
_tcv("get_remote_local", {}, "04CFF\r\n", "0CC11", "11")
_tc("set_play_mode", {"mode": "random"}, ["04D06\r\n", "04E\r\n"])
_tcv("get_play_mode", {}, "04E\r\n", "0CE01", "01")
_tcv("get_status", {}, "050\r\n", "0D011", "11")
_tcv("get_track_number", {}, "055\r\n", "0D52301", "2301")
_tcv("get_media_status", {}, "056\r\n", "0D60110", "0110")
_tcv("get_track_info", {}, "057\r\n", "0D7120004003000", "120004003000")
_tcv("get_time", {"kind": "remaining"}, "05801\r\n", "0D80103000500", "03000500")
_tcv("get_totals", {}, "05D\r\n", "0DD140005002000", "140005002000")
_tcv("get_error", {}, "078\r\n", "0F80201", "0201")
_tcv("get_caution", {}, "079\r\n", "0F90601", "0601")
_tcv("get_information", {}, "00F\r\n", "08F0123", "0123")
_tc("select_device", {"device": "am"}, ["07F0131\r\n", "07F01FF\r\n"])
_tc("select_device_dab", {"device": "dab"}, ["07F0130\r\n", "07F01FF\r\n"], model="cd-400udab")
_tcv("get_device", {}, "07F01FF\r\n", "0FF0111", "11")
_tc("set_play_area", {"area": "folder_skip"}, ["07F074F0F\r\n", "07F074FFF\r\n"])
_tcv("get_play_area", {}, "07F074FFF\r\n", "0FF07CF01", "01")
_tc("enter", {}, "07F704901\r\n")
_tc("back", {"action": "hold"}, "07F704A20\r\n")
# The same commands on the RS-232C port: LF before, CR after.
text(TC, "play", {}, "\n012\r", settings={"link": "serial"}, file="play-serial")
text(TC, "get_status", {}, "\n050\r", settings={"link": "serial"}, device_reply="\n0D010\r",
     expect_result={"ok": {"kind": "value", "value": "10"}}, file="get_status-serial")

telemetry(TC, "change-status", inbound="0F600\r\n", expect_state={}, expect_then_send=["050\r\n"])
telemetry(TC, "change-track", inbound="0F603\r\n", expect_state={},
          expect_then_send=["055\r\n", "057\r\n", "05D\r\n"])
telemetry(TC, "error-changed", inbound="0F0\r\n", expect_state={}, expect_then_send=["078\r\n"])
telemetry(TC, "caution-changed", inbound="0F1\r\n", expect_state={}, expect_then_send=["079\r\n"])
telemetry(TC, "illegal", inbound="0F2\r\n", expect_state={"last_illegal": True})
telemetry(TC, "power-on", inbound="0F4\r\n", expect_state={"powered_on": True})
telemetry(TC, "status", inbound="0D028\r\n", expect_state={"transport": {"status": "cue"}})
telemetry(TC, "track-number", inbound="0D52301\r\n", expect_state={"track": {"number": 123}})
telemetry(TC, "track-number-eom", inbound="0D5011200\r\n", expect_state={"track": {"number": 12, "eom": True}})
telemetry(TC, "media-status", inbound="0D60110\r\n", expect_state={"media": {"loaded": True, "type": "data_cd"}})
telemetry(TC, "media-status-empty", inbound="0D60000\r\n", expect_state={"media": {"loaded": False}})
telemetry(TC, "track-info", inbound="0D7120004003000\r\n",
          expect_state={"track": {"number": 12, "length": "0004:30"}, "tuner": {"am_khz": 4003, "fm_mhz": 40.03}})
telemetry(TC, "track-info-fm", inbound="0D7030008950000\r\n",
          expect_state={"track": {"number": 3, "length": "9508:00"}, "tuner": {"am_khz": 8950, "fm_mhz": 89.5}})
telemetry(TC, "track-info-dab", inbound="0D7050012B03000\r\n",
          expect_state={"track": {"number": 5}, "tuner": {"dab_channel": "12B", "dab_service": 3}})
telemetry(TC, "time-elapsed", inbound="0D80012000500\r\n", expect_state={"time": {"elapsed": "0012:05"}})
telemetry(TC, "time-remaining", inbound="0D80103000500\r\n", expect_state={"time": {"remaining": "0003:05"}})
telemetry(TC, "time-total-elapsed", inbound="0D80299590000\r\n", expect_state={"time": {"total_elapsed": "5999:00"}})
telemetry(TC, "time-total-remaining", inbound="0D80301000000\r\n", expect_state={"time": {"total_remaining": "0001:00"}})
telemetry(TC, "totals", inbound="0DD140005002000\r\n", expect_state={"tracks": {"total": 14, "total_time": "0005:20"}})
telemetry(TC, "resume", inbound="0B401\r\n", expect_state={"resume": True})
telemetry(TC, "repeat", inbound="0B700\r\n", expect_state={"repeat": False})
telemetry(TC, "incremental-play", inbound="0BA01\r\n", expect_state={"incremental_play": True})
telemetry(TC, "remote-local", inbound="0CC11\r\n", expect_state={"remote_local": "ir_disabled"})
telemetry(TC, "play-mode", inbound="0CE06\r\n", expect_state={"play_mode": "random"})
telemetry(TC, "version", inbound="08F0123\r\n", expect_state={"version": "1.23"})
telemetry(TC, "error", inbound="0F80201\r\n", expect_state={"error": {"code": "1-02", "text": "device_error"}})
telemetry(TC, "caution", inbound="0F91F01\r\n", expect_state={"caution": {"code": "1-1F", "text": "media_not_match"}})
telemetry(TC, "device", inbound="0FF0131\r\n", expect_state={"device": "am"}, model="cd-400u")
telemetry(TC, "device-dab", inbound="0FF0131\r\n", expect_state={"device": "fm"}, model="cd-400udab")
telemetry(TC, "play-area", inbound="0FF07CF0F\r\n", expect_state={"play_area": "folder_skip"})
