Q = "evertz-quartz"
# Quartz routing switcher remote control protocol (Evertz Application Note 65,
# RCP-T01, revision 1.4): upper-case ASCII messages starting with "." and
# ending with CR, numbers in decimal from 1. Expected lines are written from the
# note's syntax lines and examples (p.6-20).
U = {"ok": {"kind": "unverified"}}
ACK = {"ok": {"kind": "ack"}}

# Connection (p.15)
text(Q, "ping", {}, ".#01\r", device_reply=".A\r", expect_result=ACK)
text(Q, "get_protocol_version", {}, ".#00\r", device_reply=".A1.17,00\r",
     expect_result={"ok": {"kind": "value", "value": "1.17,00"}})

# Routing (p.6-9). A route has no reply of its own: the .U update that follows
# is pushed state, never a reply (p.6, p.19).
text(Q, "route", {"levels": "VABC", "destination": 1, "source": 2}, ".SVABC1,2\r", expect_result=U)  # note example
text(Q, "route_block", {"levels": "VA", "first_destination": 1, "last_destination": 5,
                        "first_source": 10, "last_source": 14}, ".MVA1-5,10-14\r",
     device_reply=".A\r", expect_result=ACK)
# A route update pushed while the interrogate waits is not its reply (p.20
# example ".IV1" answered ".AV001,001").
text(Q, "interrogate", {"level": "V", "destination": 1}, ".IV1\r",
     device_reply=".UV002,007\r.AV001,001\r", expect_result={"ok": {"kind": "value", "value": "001"}})
text(Q, "list_routes", {"level": "V", "destination": 5}, ".LV5,-\r",
     device_reply=".AV005,002V006,009V007,010\r",
     expect_result={"ok": {"kind": "value", "value": "V005,002V006,009V007,010"}})             # note example

# System destination locks and salvos (p.8)
text(Q, "lock_destination", {"destination": 7}, ".BL7\r", device_reply=".BA007,255\r",
     expect_result={"ok": {"kind": "value", "value": "255"}})
text(Q, "unlock_destination", {"destination": 7}, ".BU7\r", device_reply=".BA007,0\r",
     expect_result={"ok": {"kind": "value", "value": "0"}})
text(Q, "get_lock", {"destination": 12}, ".BI12\r", device_reply=".BA012,3\r",
     expect_result={"ok": {"kind": "value", "value": "3"}})
text(Q, "fire_salvo", {"salvo": 4}, ".F4\r", expect_result=U)

# Names (p.10)
text(Q, "get_destination_name", {"destination": 2}, ".RD2\r", device_reply=".RAD002,MON 2   \r",
     expect_result={"ok": {"kind": "value", "value": "MON 2"}})
text(Q, "get_source_name", {"source": 14}, ".RS14\r", device_reply=".RAS014,CAM 1\r",
     expect_result={"ok": {"kind": "value", "value": "CAM 1"}})
text(Q, "get_level_name", {"level": "B"}, ".RLB\r", device_reply=".RALB,AES 1\r",
     expect_result={"ok": {"kind": "value", "value": "AES 1"}})

# Queue (dynamic salvo) commands (p.13-14); ".QC0" is the failure answer to
# a salvo selection.
text(Q, "salvo_select", {"salvo": 11}, ".QC11\r", device_reply=".QC0\r",
     expect_result={"error": {"error": "device_error"}})
text(Q, "salvo_deselect", {}, ".QC0\r", device_reply=".A\r", expect_result=ACK)
text(Q, "salvo_clear", {"salvo": 10}, ".QR10\r", device_reply=".A\r", expect_result=ACK)
text(Q, "salvo_add", {"levels": "VA", "destination": 4, "source": 9}, ".QSVA4,9\r",
     device_reply=".A\r", expect_result=ACK)
text(Q, "salvo_fire", {"salvo": 10}, ".QF10\r", device_reply=".A\r", expect_result=ACK)       # note example
text(Q, "salvo_fire_at", {"salvo": 10, "timecode": "10:30:00:12"}, ".QF10T1:10:30:00:12\r",
     device_reply=".A\r", expect_result=ACK)
text(Q, "salvo_destroy", {"salvo": 10}, ".QD10\r", device_reply=".A\r", expect_result=ACK)
text(Q, "salvo_count", {"salvo": 10}, ".QL10\r", device_reply=".QL10,24\r",
     expect_result={"ok": {"kind": "value", "value": "24"}})

# Telemetry: route updates pushed for every route made, by anyone (p.17, p.19),
# and the replies to interrogate, list, lock, name and queue commands.
telemetry(Q, "update", inbound=".UV003,001\r",
          expect_state={"levels": {"V": {"destinations": {"3": {"source": 1}}}}})
telemetry(Q, "update-levels", inbound=".UVAC020,012\r",
          expect_state={"levels": {"V": {"destinations": {"20": {"source": 12}}},
                                   "A": {"destinations": {"20": {"source": 12}}},
                                   "C": {"destinations": {"20": {"source": 12}}}}})
telemetry(Q, "update-magnum-level", inbound=".UZ1,40\r",
          expect_state={"levels": {"Z": {"destinations": {"1": {"source": 40}}}}})
telemetry(Q, "update-tieline", inbound=".UV001,8194\r",
          expect_state={"levels": {"V": {"destinations": {"1": {"source": 8194}}}}})
telemetry(Q, "interrogate-reply", inbound=".AB005,017\r",
          expect_state={"levels": {"B": {"destinations": {"5": {"source": 17}}}}})
telemetry(Q, "list-reply", inbound=".AV005,002V006,009V007,010\r",
          expect_state={"levels": {"V": {"destinations": {"5": {"source": 2}, "6": {"source": 9},
                                                          "7": {"source": 10}}}}})
telemetry(Q, "acknowledge", inbound=".A\r", expect_state={})
telemetry(Q, "error", inbound=".E\r", expect_state={})
telemetry(Q, "lock", inbound=".BA007,255\r",
          expect_state={"destinations": {"7": {"lock_status": 255, "locked": True}}})
telemetry(Q, "lock-protected", inbound=".BA002,4\r",
          expect_state={"destinations": {"2": {"lock_status": 4, "locked": True}}})
telemetry(Q, "unlock", inbound=".BA007,0\r",
          expect_state={"destinations": {"7": {"lock_status": 0, "locked": False}}})
telemetry(Q, "destination-name", inbound=".RAD002,MON 2   \r",
          expect_state={"destinations": {"2": {"name": "MON 2"}}})
telemetry(Q, "source-name", inbound=".RAS014,CAM 1\r", expect_state={"sources": {"14": {"name": "CAM 1"}}})
telemetry(Q, "level-name", inbound=".RALB,AES 1\r", expect_state={"levels": {"B": {"name": "AES 1"}}})
telemetry(Q, "salvo-count", inbound=".QL10,24\r", expect_state={"salvos": {"10": {"items": 24}}})
