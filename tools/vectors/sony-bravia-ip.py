# Sony BRAVIA Simple IP Control (sony-bravia-ip): 24-byte messages, "*S", the
# type, the FourCC, sixteen parameter characters and LF, written here from the
# knowledge center's command table; enquiries fill the parameters with #.
SI = "sony-bravia-ip"


def _si(kind, fourcc, param):
    msg = f"*S{kind}{fourcc}{param}"
    assert len(msg) == 23, msg
    return msg + "\n"


_SI_ACK = {"ok": {"kind": "ack"}}
_Q = "#" * 16
_Z = "0" * 16

text(SI, "set_power", {"power_on": True}, [_si("C", "POWR", "0" * 15 + "1"), _si("E", "POWR", _Q)],
     device_reply=_si("A", "POWR", _Z) + _si("A", "POWR", "0" * 15 + "1"), expect_result=_SI_ACK)
text(SI, "toggle_power", {}, [_si("C", "TPOW", _Q), _si("E", "POWR", _Q)],
     device_reply=_si("A", "TPOW", _Z).replace("TPOW", "POWR") + _si("A", "POWR", _Z), expect_result=_SI_ACK)
text(SI, "get_power", {}, _si("E", "POWR", _Q), device_reply=_si("A", "POWR", "0" * 15 + "1"),
     expect_result={"ok": {"kind": "value", "value": "1"}})
text(SI, "set_volume", {"volume": 29}, _si("C", "VOLU", "0000000000000029"), device_reply=_si("A", "VOLU", _Z),
     expect_result=_SI_ACK)
# An error answer, sixteen F, fails the command.
text(SI, "get_volume", {}, _si("E", "VOLU", _Q), device_reply=_si("A", "VOLU", "F" * 16),
     expect_result={"error": {"error": "device_error"}})
text(SI, "set_mute", {"muted": True}, _si("C", "AMUT", "0" * 15 + "1"))
text(SI, "get_mute", {}, _si("E", "AMUT", _Q), device_reply=_si("A", "AMUT", _Z),
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(SI, "set_input", {"kind": "hdmi", "number": 2}, _si("C", "INPT", "0000000100000002"))
text(SI, "get_input", {}, _si("E", "INPT", _Q))
text(SI, "set_picture_mute", {"muted": False}, _si("C", "PMUT", _Z))
text(SI, "toggle_picture_mute", {}, _si("C", "TPMU", _Q))
text(SI, "get_picture_mute", {}, _si("E", "PMUT", _Q))
text(SI, "set_scene", {"scene": "auto24pSync"}, _si("C", "SCEN", "auto24pSync#####"), model="bz")
text(SI, "get_scene", {}, _si("E", "SCEN", _Q), model="bz", device_reply=_si("A", "SCEN", "general#########"),
     expect_result={"ok": {"kind": "value", "value": "general"}})
text(SI, "press_key", {"key": "hdmi1"}, _si("C", "IRCC", "0000000000000124"))
text(SI, "get_broadcast_address", {"interface": "eth0"}, _si("E", "BADR", "eth0############"),
     device_reply=_si("A", "BADR", "192.168.0.14####"), expect_result={"ok": {"kind": "value", "value": "192.168.0.14"}})
text(SI, "get_mac_address", {"interface": "wlan0"}, _si("E", "MADR", "wlan0###########"))

telemetry(SI, "power-answer", request=_si("E", "POWR", _Q)[:-1], inbound=_si("A", "POWR", "0" * 15 + "1"),
          expect_state={"power": True})
telemetry(SI, "power-notify", inbound=_si("N", "POWR", _Z), expect_state={"power": False})
telemetry(SI, "volume-answer", request=_si("E", "VOLU", _Q)[:-1], inbound=_si("A", "VOLU", "0000000000000029"),
          expect_state={"volume": 29})
telemetry(SI, "volume-notify", inbound=_si("N", "VOLU", "0000000000000030"), expect_state={"volume": 30})
telemetry(SI, "mute-answer", request=_si("E", "AMUT", _Q)[:-1], inbound=_si("A", "AMUT", "0" * 15 + "1"),
          expect_state={"mute": True})
telemetry(SI, "mute-notify", inbound=_si("N", "AMUT", _Z), expect_state={"mute": False})
telemetry(SI, "input-answer", request=_si("E", "INPT", _Q)[:-1], inbound=_si("A", "INPT", "0000000100000003"),
          expect_state={"input": {"kind": "hdmi", "number": 3}})
telemetry(SI, "input-notify", inbound=_si("N", "INPT", "0000000400000001"),
          expect_state={"input": {"kind": "component", "number": 1}})
telemetry(SI, "picture-mute-answer", request=_si("E", "PMUT", _Q)[:-1], inbound=_si("A", "PMUT", "0" * 15 + "1"),
          expect_state={"picture_mute": True})
telemetry(SI, "picture-mute-notify", inbound=_si("N", "PMUT", _Z), expect_state={"picture_mute": False})
telemetry(SI, "scene", request=_si("E", "SCEN", _Q)[:-1], inbound=_si("A", "SCEN", "auto############"),
          expect_state={"scene": "auto"})
telemetry(SI, "broadcast", request=_si("E", "BADR", "eth0############")[:-1], inbound=_si("A", "BADR", "192.168.0.14####"),
          expect_state={"network": {"eth0": {"broadcast": "192.168.0.14"}}})
telemetry(SI, "mac", request=_si("E", "MADR", "wlan0###########")[:-1], inbound=_si("A", "MADR", "0123456789AB####"),
          expect_state={"network": {"wlan0": {"mac": "0123456789AB"}}})
# A set's answer (sixteen 0) is not read as a value.
telemetry(SI, "set-answer", request=_si("C", "POWR", _Z)[:-1], inbound=_si("A", "POWR", _Z), expect_state={})
