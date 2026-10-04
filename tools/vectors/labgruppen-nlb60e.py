# Lab.gruppen NLB 60E third-party protocol (TCP 65010, CR+LF). Wire formats from the NLB 60E Third Party
# Integration Protocol Description rev. 2B: "<Context>.<Parameter> = <value>" and "<Context>.<Parameter> ?"
# with spaces around the operator as in 4.1, a set answered as the get (4), errors as "ERROR! ..." (5.1).
NLB = "labgruppen-nlb60e"


def _nlb(command, input, wire, reply, result):
    text(NLB, command, input, wire + "\r\n", device_reply=reply + "\r\n", expect_result=result)


def _v(value):
    return {"ok": {"kind": "value", "value": value}}


_nlb("set_subnet_power", {"powered": True}, "Subnet.Power = 1", "3", _v("3"))
_nlb("get_subnet_power", {}, "Subnet.Power ?", "2", _v("2"))
# 4.1's own example.
_nlb("set_subnet_mute", {"muted": True}, "Subnet.Mute = 1", "1", _v("1"))
_nlb("get_subnet_mute", {}, "Subnet.Mute ?", "0", _v("0"))
_nlb("get_subnet_status_ok", {}, "Subnet.StatusOk ?", "1", _v("1"))
# 5.2.4's example: subnet 12, 10 devices, two failing.
_nlb("get_subnet_status", {}, "Subnet.Status ?", "12 10 0 0 0 0 0 1 1 B01.Z01.2 B01.Z01.5",
     _v("12 10 0 0 0 0 0 1 1 B01.Z01.2 B01.Z01.5"))
# 5.2.5's example.
_nlb("get_serial_at_position", {"position": 5}, "Subnet.SerialAtPos5 ?", "12345678", _v("12345678"))
# 4.1's VDN example.
_nlb("set_power", {"vdn": "B01-Z.2-1", "powered": True}, "B01-Z.2-1.Power = 1", "1", _v("1"))
_nlb("get_power", {"vdn": "BLDG1ZONE1"}, "BLDG1ZONE1.Power ?", "0", _v("0"))
# 5.3.2's example.
_nlb("set_mute", {"vdn": "B01.Z01.2", "channel": "B", "muted": True}, "B01.Z01.2.MuteB = 1", "1", _v("1"))
_nlb("get_mute", {"vdn": "B1@ZONE2-1", "channel": "D"}, "B1@ZONE2-1.MuteD ?", "0", _v("0"))
# 5.3.3's example.
_nlb("get_mute_status", {"vdn": "B01.Z01.2"}, "B01.Z01.2.MuteStatus ?", "1100", _v("1100"))
# 5.3.4's example: a four-channel amplifier, channel A clipping, channel C with a temperature warning.
_STATUS = "0 0 0 0 1 0 0 0 0 0 0 0 -44 0 0 0 0 0 0 0 0 -27 0 0 0 0 0 0 0 1 -26 0 0 0 0 0 0 0 0"
_nlb("get_status", {"vdn": "B01.Z01.2"}, "B01.Z01.2.Status ?", _STATUS, _v(_STATUS))
# 5.4.1's examples.
_nlb("set_vdn", {"position": 1, "vdn": "B1-zone2-1", "serial": "123456789"}, "Subnet.VDN1 = B1-zone2-1 123456789",
     "B1-ZONE2-1 123456789", _v("B1-ZONE2-1 123456789"))
_nlb("clear_vdn", {"position": 1}, "Subnet.VDN1 =", "**", _v("**"))
_nlb("get_vdn", {"position": 60}, "Subnet.VDN60 ?", "ERROR! VDN does not exist",
     {"error": {"error": "device_error"}})

telemetry(NLB, "subnet-status", expect_connect_wire=["Subnet.Status ?\r\n"],
          inbound="12 10 0 0 0 0 0 1 1 B01.Z01.2 B01.Z01.5\r\n",
          expect_state={"subnet": {"number": 12, "devices": 10, "mute": False, "closed_loop": False,
                                   "gpi1": False, "gpi2": False, "gpi3": False, "power": 1, "faults": True,
                                   "faulty_devices": "B01.Z01.2 B01.Z01.5"}})
telemetry(NLB, "subnet-status-no-faults", inbound="3 4 1 1 0 1 0 3 0\r\n",
          expect_state={"subnet": {"number": 3, "devices": 4, "mute": True, "closed_loop": True,
                                   "gpi1": False, "gpi2": True, "gpi3": False, "power": 3, "faults": False,
                                   "faulty_devices": ""}})
telemetry(NLB, "bare-value-is-not-state", inbound="1\r\n", expect_state={})

# Replies read with the message they answer (request): the bridge answers in
# order with the value only (section 4); values as section 5 gives them.
telemetry(NLB, "subnet-power", inbound="3\r\n", request="Subnet.Power = 1",
          expect_state={"subnet": {"power": 3}})
telemetry(NLB, "subnet-mute", inbound="1\r\n", request="Subnet.Mute ?", expect_state={"subnet": {"mute": True}})
telemetry(NLB, "subnet-status-ok", inbound="0\r\n", request="Subnet.StatusOk ?",
          expect_state={"subnet": {"status_ok": False}})
telemetry(NLB, "amp-power", inbound="1\r\n", request="b01.z01.Power ?", expect_state={})
telemetry(NLB, "amp-power-named", inbound="0\r\n", request="amp-3.Power = 0",
          expect_state={"amps": {"AMP-3": {"power": False}}})
# The Subnet context in any case is the subnet, never an amplifier.
telemetry(NLB, "subnet-is-not-an-amp", inbound="1\r\n", request="SUBNET.Power ?",
          expect_state={"subnet": {"power": 1}})
telemetry(NLB, "amp-channel-mute", inbound="1\r\n", request="AMP1.MuteC = 1",
          expect_state={"amps": {"AMP1": {"channels": {"C": {"mute": True}}}}})
telemetry(NLB, "amp-mute-status", inbound="0110\r\n", request="AMP1.MuteStatus ?",
          expect_state={"amps": {"AMP1": {"channels": {"A": {"mute": False}, "B": {"mute": True},
                                                        "C": {"mute": True}, "D": {"mute": False}}}}})
_NLBF = ["vpl", "cpl", "vhf_fault", "dc_fault", "load_shorted", "temperature_fault", "high_impedance",
         "temperature_warning"]
telemetry(NLB, "amp-status", inbound="1 0 1 -29 1 0 0 0 0 0 0 0 -100 0 0 0 0 0 0 1 0\r\n", request="AMP1.Status ?",
          expect_state={"amps": {"AMP1": {
              "fault": True, "power_state": 0, "pal": True, "channels": {
                  "A": {"attenuation": -29, **{f: i == 0 for i, f in enumerate(_NLBF)}},
                  "B": {"attenuation": -100, **{f: i == 6 for i, f in enumerate(_NLBF)}}}}}})
telemetry(NLB, "vdn-slot", inbound="AMP1 123456789\r\n", request="Subnet.VDN7 ?",
          expect_state={"vdn": {"7": {"name": "AMP1", "serial": "123456789"}}},
          expect_then_send=["AMP1.Status ?\r\n", "AMP1.Power ?\r\n", "AMP1.MuteStatus ?\r\n"])
telemetry(NLB, "vdn-slot-empty", inbound="*\r\n", request="Subnet.VDN7 ?",
          state_before={"vdn": {"7": {"name": "AMP1", "serial": "1"}}}, expect_state={"vdn": {}})
