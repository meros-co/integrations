DC = "denon-dn500cb"
# Denon Professional DN-500CB serial command protocol (Manual Version 1.4): "@0", the command, CR. Each command is
# answered with the ACK byte (06H) or NACK (15H); a status request with ACK, then the status, "@0" + code + CR.
_DC_SERIAL = {"serial_port": "COM3"}
_DC_OK = {"ok": {"kind": "ack"}}


def _dc(command, input, wire, **extra):
    text(DC, command, input, wire, settings=_DC_SERIAL, device_reply="\x06", expect_result=_DC_OK, **extra)


def _dcv(command, wire, status, value, **extra):
    text(DC, command, {}, wire, settings=_DC_SERIAL, device_reply="\x06" + status + "\r",
         expect_result={"ok": {"kind": "value", "value": value}}, **extra)


_dc("power_on", {}, "@0PW00\r")
_dc("power_off", {}, "@0PW01\r")
_dc("get_power", {}, "@0?PW\r")
_dc("eject", {}, "@0PCDTRYOP\r")
_dc("play", {}, "@02353\r")
text(DC, "play", {}, "@02353\r", settings=_DC_SERIAL, device_reply="\x15",
     expect_result={"error": {"error": "device_error"}}, file="play-nack")
_dc("pause", {}, "@02348\r")
_dc("stop", {}, "@02354\r")
_dc("set_mute", {"muted": True}, "@0mt00\r")
_dc("next_track", {}, "@02332\r")
_dc("previous_track", {}, "@02333\r")
_dc("locate_track", {"track": 12}, "@0Tr0012\r")
_dc("rewind", {}, "@0PCSLSR\r")
_dc("fast_forward", {}, "@0PCSLSF\r")
_dc("number_key", {"digit": 7}, "@0PCTKEY7\r")
_dc("set_time_display", {"mode": "total_remaining"}, "@0PCTMDTR\r")
_dcv("get_disc_status", "@0?CD\r", "@0CDCI", "CI")
_dcv("get_status", "@0?ST\r", "@0STDVFF", "DVFF")
_dcv("get_track", "@0?Tr\r", "@0Tr0003", "0003")
_dcv("get_track_count", "@0?Tt\r", "@0Tt0014", "0014")
_dcv("get_title", "@0?ti\r", "@0tiOpening Theme", "Opening Theme")
_dcv("get_artist", "@0?at\r", "@0at The Band", "The Band")
_dcv("get_album", "@0?al\r", "@0alLive", "Live")
_dcv("get_track_time", "@0?tl\r", "@0tl00342", "00342")
_dcv("get_elapsed", "@0?ET\r", "@0ET0000342", "0000342")
_dcv("get_remaining", "@0?RM\r", "@0RM0010000", "0010000")

telemetry(DC, "disc", settings=_DC_SERIAL, inbound="@0CDNC\r", expect_state={"disc": {"status": "no_disc"}})
telemetry(DC, "status", settings=_DC_SERIAL, inbound="@0STDVFR\r", expect_state={"transport": {"status": "rewind"}})
telemetry(DC, "track", settings=_DC_SERIAL, inbound="@0Tr0003\r", expect_state={"track": {"number": 3}},
          expect_then_send=["@0?ti\r", "@0?at\r", "@0?al\r"])
telemetry(DC, "track-unknown", settings=_DC_SERIAL, inbound="@0TrUNKN\r", state_before={"track": {"number": 3}},
          expect_state={"track": {}})
telemetry(DC, "tracks", settings=_DC_SERIAL, inbound="@0Tt0014\r", expect_state={"tracks": {"total": 14}})
telemetry(DC, "tracks-unknown", settings=_DC_SERIAL, inbound="@0TtUNKN\r", state_before={"tracks": {"total": 14}},
          expect_state={"tracks": {}})
telemetry(DC, "title", settings=_DC_SERIAL, inbound="@0tiOpening Theme\r", expect_state={"track": {"title": "Opening Theme"}})
telemetry(DC, "artist", settings=_DC_SERIAL, inbound="@0at The Band\r", expect_state={"track": {"artist": "The Band"}})
telemetry(DC, "album", settings=_DC_SERIAL, inbound="@0alLive\r", expect_state={"track": {"album": "Live"}})
telemetry(DC, "position", settings=_DC_SERIAL, inbound="@0tl00342\r", expect_state={"track": {"position": "3:42"}})
telemetry(DC, "elapsed", settings=_DC_SERIAL, inbound="@0ET0000342\r", expect_state={"time": {"elapsed": "0:03:42"}})
telemetry(DC, "remaining", settings=_DC_SERIAL, inbound="@0RM0010000\r", expect_state={"time": {"remaining": "1:00:00"}})
