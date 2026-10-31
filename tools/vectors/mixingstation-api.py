MSA = "mixingstation-api"
# Mixing Station's REST API over its WebSocket: JSON {path, method, body}
# (Mixing Station Docs, APIs), with the endpoints the MIT-licensed Companion
# module (src/ms/MixingStation.ts, commit b4018aa5) uses.


def msa(command, input, message, **extra):
    text(MSA, command, input, message, **extra)


msa("get_app_state", {}, '{"path":"/app/state","method":"GET","body":null}',
    device_reply='{"path":"/app/state","method":"GET","body":{"state":"connected","topState":"connected"},"error":null}',
    expect_result={"ok": {"kind": "value", "value": {"state": "connected", "topState": "connected"}}})
msa("get_console_information", {}, '{"path":"/console/information","method":"GET","body":null}',
    device_reply='{"path":"/console/information","method":"GET","body":{"totalChannels":40,"rtaFrequencies":[]},"error":null}',
    expect_result={"ok": {"kind": "value", "value": {"totalChannels": 40, "rtaFrequencies": []}}})
msa("get_available_mixers", {}, '{"path":"/app/mixers/available","method":"GET","body":null}')
msa("connect_mixer", {"console_id": 3, "ip": "192.168.1.20"},
    '{"path":"/app/mixers/connect","method":"POST","body":{"consoleId":3,"ip":"192.168.1.20"}}')
msa("start_offline", {"console_id": 3, "model_id": 1},
    '{"path":"/app/mixers/offline","method":"POST","body":{"consoleId":3,"modelId":1}}')
msa("get_data_paths", {}, '{"path":"/console/data/paths","method":"GET","body":null}')
msa("get_definition", {"path": "ch.0.mix.lvl"}, '{"path":"/console/data/definitions2/ch.0.mix.lvl","method":"GET","body":null}')
msa("get_value", {"path": "ch.0.mix.lvl"}, '{"path":"/console/data/get/ch.0.mix.lvl/val","method":"GET","body":null}',
    device_reply='{"path":"/console/data/get/ch.0.mix.lvl/val","method":"GET","body":{"value":-10.5,"format":"val"},"error":null}',
    expect_result={"ok": {"kind": "value", "value": -10.5}})
# A reply carrying an error fails the command.
msa("get_value", {"path": "ch.99.mix.lvl"}, '{"path":"/console/data/get/ch.99.mix.lvl/val","method":"GET","body":null}',
    device_reply='{"path":"/console/data/get/ch.99.mix.lvl/val","method":"GET","body":null,"error":"Invalid path"}',
    expect_result={"error": {"error": "device_error"}}, file="get_value-error")
msa("set_value", {"path": "ch.0.mix.lvl", "value": -5.0},
    '{"path":"/console/data/set/ch.0.mix.lvl/val","method":"POST","body":{"value":-5.000000,"fade":0}}')
msa("set_value", {"path": "ch.0.mix.lvl", "value": 0.0, "fade_ms": 2000},
    '{"path":"/console/data/set/ch.0.mix.lvl/val","method":"POST","body":{"value":0.000000,"fade":2000}}',
    file="set_value-fade")
msa("set_value_int", {"path": "ch.0.grp.dca.0", "value": 1},
    '{"path":"/console/data/set/ch.0.grp.dca.0/val","method":"POST","body":{"value":1,"fade":0}}')
msa("set_value_bool", {"path": "ch.0.mix.on", "value": False},
    '{"path":"/console/data/set/ch.0.mix.on/val","method":"POST","body":{"value":false,"fade":0}}')
msa("set_value_text", {"path": "ch.0.cfg.name", "value": "Lead \"Vox\""},
    '{"path":"/console/data/set/ch.0.cfg.name/val","method":"POST","body":{"value":"Lead \\"Vox\\"","fade":0}}')
msa("subscribe", {"path": "ch.*.mix.lvl"},
    '{"path":"/console/data/subscribe","method":"POST","body":{"path":"ch.*.mix.lvl","format":"val"}}')
msa("unsubscribe", {"path": "ch.*.mix.lvl"},
    '{"path":"/console/data/unsubscribe","method":"POST","body":{"path":"ch.*.mix.lvl","format":"val"}}')
msa("subscribe_meters", {"id": 0, "interval_ms": 50, "params": [{"type": 0, "index": 0}, {"type": 0, "index": 1}]},
    '{"path":"/console/metering2/subscribe","method":"POST","body":{"id":0,"interval":50,"binary":false,'
    '"params":[{"type":0,"index":0},{"type":0,"index":1}]}}')
msa("unsubscribe_meters", {"id": 0}, '{"path":"/console/metering/unsubscribe","method":"POST","body":{"id":0}}')

telemetry(MSA, "app-state", inbound_ws='{"path":"/app/state","method":"GET","body":{"state":"sceneRecall",'
          '"topState":"connected","msg":"Recalling","progress":0.5},"error":null}',
          expect_state={"app": {"top_state": "connected", "state": "sceneRecall", "message": "Recalling", "progress": 0.5}})
telemetry(MSA, "value", inbound_ws='{"path":"/console/data/get/ch.0.mix.lvl","method":"GET","body":{"value":-12.5,"format":"val"}}',
          expect_state={"values": {"ch.0.mix.lvl": -12.5}})
telemetry(MSA, "value-reply", inbound_ws='{"path":"/console/data/get/ch.3.mix.lvl/val","method":"GET",'
          '"body":{"value":0,"format":"val"},"error":null}',
          expect_state={"values": {"ch.3.mix.lvl": 0.0}})
telemetry(MSA, "switch", inbound_ws='{"path":"/console/data/get/ch.0.mix.on","method":"GET","body":{"value":true,"format":"val"}}',
          expect_state={"switches": {"ch.0.mix.on": True}})
telemetry(MSA, "text", inbound_ws='{"path":"/console/data/get/ch.0.cfg.name","method":"GET","body":{"value":"Kick","format":"val"}}',
          expect_state={"texts": {"ch.0.cfg.name": "Kick"}})
telemetry(MSA, "meters", inbound_ws='{"path":"/console/metering2/0","method":"POST","body":{"v":[[-20.5,-18],[-80,-3.25]]}}',
          expect_state={"meters": {"0": {"0": {"0": -20.5, "1": -18.0}, "1": {"0": -80.0, "1": -3.25}}}})
