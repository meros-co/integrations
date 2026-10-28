MS = "mixingstation"
# Mixing Station OSC API (docs: APIs, OSC): /con/v/<data path> plain values,
# /con/n/<data path> normalized, /hi/v to subscribe.
binary(MS, "get_value", {"path": "ch.0.mix.lvl"}, osc("/con/v/ch.0.mix.lvl"),
       device_reply_hex=hexs(osc("/con/v/ch.0.mix.lvl", ("f", -5.0))), expect_result={"ok": {"kind": "value", "value": -5.0}})
binary(MS, "get_value_normalized", {"path": "ch.0.mix.lvl"}, osc("/con/n/ch.0.mix.lvl"))
binary(MS, "set_value", {"path": "ch.0.mix.lvl", "value": -5.0}, osc("/con/v/ch.0.mix.lvl", ("f", -5.0)))   # docs example
binary(MS, "set_value_normalized", {"path": "ch.0.mix.lvl", "value": 0.5}, osc("/con/n/ch.0.mix.lvl", ("f", 0.5)))  # docs example
binary(MS, "set_value_int", {"path": "ch.0.cfg.color", "value": 3}, osc("/con/v/ch.0.cfg.color", ("i", 3)))
binary(MS, "set_value_bool", {"path": "ch.0.mix.on", "value": True}, osc("/con/v/ch.0.mix.on", ("T", None)))
binary(MS, "set_value_text", {"path": "ch.0.cfg.name", "value": "Vox"}, osc("/con/v/ch.0.cfg.name", ("s", "Vox")))
binary(MS, "subscribe", {}, osc("/hi/v"))
binary(MS, "subscribe_normalized", {}, osc("/hi/n"))
telemetry(MS, "value", inbound_hex=hexs(osc("/con/v/ch.0.mix.lvl", ("f", -5.0))),
          expect_state={"values": {"ch.0.mix.lvl": -5.0}})
telemetry(MS, "normalized", inbound_hex=hexs(osc("/con/n/ch.1.mix.lvl", ("f", 0.5))),
          expect_state={"normalized": {"ch.1.mix.lvl": 0.5}})
telemetry(MS, "integer", inbound_hex=hexs(osc("/con/v/ch.0.cfg.color", ("i", 3))), expect_state={"integers": {"ch.0.cfg.color": 3}})
telemetry(MS, "switch-on", inbound_hex=hexs(osc("/con/v/ch.0.mix.on", ("T", None))), expect_state={"switches": {"ch.0.mix.on": True}})
telemetry(MS, "switch-off", inbound_hex=hexs(osc("/con/v/ch.0.mix.on", ("F", None))), expect_state={"switches": {"ch.0.mix.on": False}})
telemetry(MS, "text", inbound_hex=hexs(osc("/con/v/ch.0.cfg.name", ("s", "Vox"))), expect_state={"texts": {"ch.0.cfg.name": "Vox"}})
