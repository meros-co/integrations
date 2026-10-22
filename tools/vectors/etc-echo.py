EE = "etc-echo"
# ETC Echo UDP strings: the Echo Integration Interface installation guide
# (7186M2111 Rev A, page 8) and the EchoTouch user manual (7186M1283-3.4.0,
# Supported UDP String Commands). Commands start with E$, parameters are
# separated by ", " as in the guide's example "E$pst act: 2, 12, 3.5", and
# each string ends with CR, the default End of Message. Response strings
# (E>...) are not in ETC's documents; those below follow the community
# integration cited in the spec. Not taken from the spec.


def ee(command, input, wire, **extra):
    text(EE, command, input, wire + "\r", **extra)


# ── Set commands (both devices) ──────────────────────────────────────────
ee("preset_activate", {"space": 2, "preset": 12, "time": 3.5}, "E$pst act: 2, 12, 3.5",
   expect_result={"ok": {"kind": "unverified"}})
ee("preset_activate_default_time", {"space": 1, "preset": 64}, "E$pst act: 1, 64, 99.0")
ee("space_off", {"space": 16, "time": 0}, "E$off: 16, 0.0")
ee("space_off_default_time", {"space": 3}, "E$off: 3, 99.0")
ee("sequence_activate", {"space": 1, "sequence": 4}, "E$seq act: 1, 4")
ee("sequence_deactivate", {"space": 1, "sequence": 4}, "E$seq dact: 1, 4")
ee("zone_set", {"space": 1, "zone": 16, "level": 255, "time": 25.4}, "E$zone int: 1, 16, 255, 25.4")
ee("zone_set_default_time", {"space": 1, "zone": 2, "level": 0}, "E$zone int: 1, 2, 0, 99.0")

# ── EchoTouch only ───────────────────────────────────────────────────────
ee("channel_set", {"channel": 2, "level": 127, "time": 3.5}, "E$chan int: 2, 127, 3.5", model="echotouch")
ee("channel_set_default_time", {"channel": 80, "level": 255}, "E$chan int: 80, 255, 99.0", model="echotouch")
ee("space_raise", {"space": 1, "amount": 25}, "E$ras: 1, 25", model="echotouch")
ee("space_lower", {"space": 1, "amount": 25}, "E$low: 1, 25", model="echotouch")
ee("space_lock", {"space": 5}, "E$lok: 5", model="echotouch")
ee("space_unlock", {"space": 5}, "E$ulok: 5", model="echotouch")
ee("space_lock_get", {"space": 5}, "E$lok get: 5", model="echotouch")
ee("channel_get", {}, "E$chan int get: ", model="echotouch")

# ── Get commands ─────────────────────────────────────────────────────────
ee("preset_get", {"space": 2}, "E$pst get: 2",
   device_reply_hex=hexs(b"E>pst act: 2, 12\r"),
   expect_result={"ok": {"kind": "value", "value": "12"}})
ee("space_off_get", {"space": 2}, "E$off get: 2",
   device_reply_hex=hexs(b"E>space off: 2, 1\r"),
   expect_result={"ok": {"kind": "value", "value": "1"}})
ee("sequence_get", {"space": 2}, "E$seq get: 2",
   device_reply_hex=hexs(b"E>seq act: 2, 3\r"),
   expect_result={"ok": {"kind": "value", "value": "3"}})
ee("zone_get", {"space": 1}, "E$zone int get: 1")
ee("sync_get", {"space": 0}, "E$sync get: 0")
ee("help", {}, "E$help")

# ── Telemetry: subscriber updates and get replies have the same form ─────
telemetry(EE, "preset", inbound_hex=hexs(b"E>pst act: 2, 12\r"),
          expect_state={"spaces": {"2": {"preset": 12}}})
telemetry(EE, "preset-none", inbound_hex=hexs(b"E>pst act: 1, 0\r"),
          expect_state={"spaces": {"1": {"preset": 0}}})
telemetry(EE, "space-off", inbound_hex=hexs(b"E>space off: 3, 1\r"),
          expect_state={"spaces": {"3": {"off": True}}})
telemetry(EE, "space-on", inbound_hex=hexs(b"E>space off: 3, 0\r"),
          expect_state={"spaces": {"3": {"off": False}}})
telemetry(EE, "sequence", inbound_hex=hexs(b"E>seq act: 1, 4\r"),
          expect_state={"spaces": {"1": {"sequence": 4}}})
telemetry(EE, "zone", inbound_hex=hexs(b"E>zone int: 1, 2, 128\r"),
          expect_state={"spaces": {"1": {"zones": {"2": {"level": 128}}}}})
# A space's zone levels in one datagram, EOM between them...
telemetry(EE, "zones-batched", inbound_hex=hexs(b"E>zone int: 1, 1, 255\rE>zone int: 1, 2, 0\rE>zone int: 1, 3, 64\r"),
          expect_state={"spaces": {"1": {"zones": {"1": {"level": 255}, "2": {"level": 0}, "3": {"level": 64}}}}})
# ...or none, as a non-subscriber's get reply may come (EEB-47106).
telemetry(EE, "zones-unterminated", inbound_hex=hexs(b"E>zone int: 4, 1, 10E>zone int: 4, 2, 20"),
          expect_state={"spaces": {"4": {"zones": {"1": {"level": 10}, "2": {"level": 20}}}}})
# A sync mixing kinds in one datagram.
telemetry(EE, "sync", inbound_hex=hexs(b"E>pst act: 1, 3\rE>space off: 1, 0\rE>seq act: 1, 0\rE>zone int: 1, 1, 200\r"),
          expect_state={"spaces": {"1": {"preset": 3, "off": False, "sequence": 0, "zones": {"1": {"level": 200}}}}})
telemetry(EE, "lock", inbound_hex=hexs(b"E>lok: 5, 1\r"), model="echotouch",
          expect_state={"spaces": {"5": {"locked": True}}})
telemetry(EE, "channel", inbound_hex=hexs(b"E>chan int: 2, 127\r"), model="echotouch",
          expect_state={"channels": {"2": {"level": 127}}})
