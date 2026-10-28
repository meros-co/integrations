# LynTec RPC (lyntec-rpc): one vector per command, from the RPC instruction
# bulletin's TCP/IP protocol (Appendix B, pp.49-50): GET /p2.rpc?IPBnnn=1 or
# =0 for breakers, IPZnnn for zones, IPEnnn for events, each followed by a
# status read; GET /p2.rpc answers breakers=..., zones=... and events=...
# (the bulletin's example status). A control is followed by a status read, sent
# once the control is answered; the vector states the control.
LY = "lyntec-rpc"
_LY_STATUS = "breakers=10110111111111111111 zones=111111111011 events=1000000"

http(LY, "get_status", {}, "GET", "/p2.rpc", http_reply={"status": 200, "body": _LY_STATUS},
     expect_result={"ok": {"kind": "value", "value": _LY_STATUS}})
V.append({"spec": LY, "command": "set_breaker", "input": {"breaker": 2, "power": True},
          "expect_request": [{"method": "GET", "target": "/p2.rpc?IPB002=1"}]})
V.append({"spec": LY, "command": "set_zone", "input": {"zone": 2, "power": False},
          "expect_request": [{"method": "GET", "target": "/p2.rpc?IPZ002=0"}]})
V.append({"spec": LY, "command": "set_event", "input": {"event": 1, "power": True},
          "expect_request": [{"method": "GET", "target": "/p2.rpc?IPE001=1"}]})
telemetry(LY, "status", inbound_http={"path": "/p2.rpc", "body": "breakers=10234 zones=120 events=1000000"},
          expect_state={"breakers": {"1": {"status": "on"}, "2": {"status": "off"}, "3": {"status": "tripped"},
                                     "4": {"status": "faulty"}, "5": {"status": "empty"}},
                        "zones": {"1": {"status": "on"}, "2": {"status": "processing"}, "3": {"status": "off"}},
                        "events": {"1": {"status": "on"}, "2": {"status": "off"}, "3": {"status": "off"},
                                   "4": {"status": "off"}, "5": {"status": "off"}, "6": {"status": "off"},
                                   "7": {"status": "off"}}})
