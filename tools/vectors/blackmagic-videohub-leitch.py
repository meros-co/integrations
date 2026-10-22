VL = "blackmagic-videohub-leitch"
# Blackmagic Videohub RS-422 router control, Leitch Serial Pass-Through
# Protocol (Videohub Developer Information, May 2018, p.8-10): "@ " and the
# command, ended by CR; replies ended by CR LF; a ">" prompt with no line
# ending after each command, which is its reply. Destinations and sources are
# hexadecimal from 0 on the wire and from 1 here; the level is always 0.
VL_OK = {"ok": {"kind": "ack"}}


def vl(command, input, wire, **extra):
    text(VL, command, input, wire, **extra)


# Immediate commands (p.10); the document's own example "@ X:0/6,2" for
# destination port 7 and source port 3.
vl("route", {"destination": 7, "source": 3}, ["@ X:0/6,2\r", "@ X?06\r"],
   device_reply="S:06,2\r\n>S:06,2\r\n>", expect_result=VL_OK)
vl("route", {"destination": 288, "source": 16}, ["@ X:0/11f,f\r", "@ X?011f\r"],
   device_reply=">S:011f,f\r\n>", expect_result=VL_OK, file="route-288")
vl("route_many", {"routes": "6,2/7,2"}, ["@ X:0/6,2/7,2\r", "@ S?0\r"],
   device_reply=">S:06,2\r\nS:07,2\r\n>", expect_result=VL_OK)
vl("get_route", {"destination": 12}, "@ X?0b\r", device_reply="S:0b,0\r\n>", expect_result=VL_OK)
vl("get_all_routes", {}, "@ S?0\r", device_reply="S:00,0\r\nS:01,4\r\n>", expect_result=VL_OK)

# Global commands (p.10)
vl("enable_status_reporting", {}, "@ ?\r", device_reply=">", expect_result=VL_OK)
vl("disable_status_reporting", {}, "@ !\r")
vl("reset_routing", {}, ["@ Z:\r", "@ S?0\r"], device_reply=">S:00,0\r\n>", expect_result=VL_OK)

# Salvo commands (p.10)
vl("queue_route", {"destination": 2, "source": 40}, ["@ P:0/1,27\r", "@ P?01\r"],
   device_reply=">V:01,27\r\n>", expect_result=VL_OK)
vl("queue_routes", {"routes": "0,1/1,1"}, ["@ P:0/0,1/1,1\r", "@ V?0\r"],
   device_reply=">V:00,1\r\nV:01,1\r\n>", expect_result=VL_OK)
vl("get_salvo_route", {"destination": 1}, "@ P?00\r", device_reply="V:00,1\r\n>", expect_result=VL_OK)
vl("get_salvo", {}, "@ V?0\r")
vl("clear_salvo", {}, "@ B:C\r", device_reply=">", expect_result=VL_OK)
vl("reset_salvo", {}, "@ B:R\r")
vl("execute_salvo", {}, "@ B:E\r", device_reply="S:00,1\r\n>", expect_result=VL_OK)

# Telemetry: one vector per rule.
# On connecting, a bare CR, which the router answers with its prompt (p.9).
telemetry(VL, "route", inbound="S:06,2\r\n", expect_state={"outputs": {"7": {"input": 3}}},
          expect_connect_wire=["\r"])
telemetry(VL, "route-hex", inbound="S:011f,ff\r\n", expect_state={"outputs": {"288": {"input": 256}}})
telemetry(VL, "salvo", inbound="V:00,1\r\n", expect_state={"salvo": {"1": {"input": 2}}})
telemetry(VL, "salvo-cleared", request="@ B:C", inbound=">",
          state_before={"salvo": {"1": {"input": 2}}}, expect_state={})
telemetry(VL, "status-reporting-on", request="@ ?", inbound=">", expect_state={"status_reporting": True})
telemetry(VL, "status-reporting-off", request="@ !", inbound=">", expect_state={"status_reporting": False})
