# DHD Control API on cores (dhd-control-api) and on AES67 modules
# (dhd-aes67): one vector per command and one telemetry vector per update
# rule. Messages follow DHD's "WebSocket Usage" and "RPC
# Method" pages: {"msgID", "method", "path", "payload"} for get, set and
# subscribe, {"method":"rpc","payload":{"method","params"}} for RPCs, and
# updates rooted at the tree's root. The set commands' paths come from the
# spec (itself generated from DHD's OpenAPI document); the expected message
# and the update payloads are built here independently.
DCA = "dhd-control-api"


def _dhd_tree_vectors(DCA) -> None:
    import re

    doc = yaml.safe_load((ROOT / "specs" / f"{DCA}.yaml").read_text(encoding="utf-8"))
    ids = {"mixer": 0, "fader": 3, "group": 1, "cleanfeed": 2, "mutegroup": 4, "bus": 5, "band": 2,
           "filter": 1, "combilogic": 3, "flexprocessing": 1, "pot": 2, "output": 7, "selector": 1,
           "label": 9, "logic": 12, "accessgroup": 0, "stream": 2}
    samples = {"bool": True, "int": 7, "float": -6.5, "string": "Mic 1"}
    for name, c in doc["commands"].items():
        m = re.match(r"Set (/\S+) \((boolean|integer|number|string)\)", c["summary"])
        if not m:
            continue
        path = m.group(1)
        params = c["params"]
        values = {k: ids[k] for k in params if k != "value"}
        values["value"] = samples[params["value"]["type"]]
        concrete = re.sub(r"\{(\w+)\}", lambda g: str(values[g.group(1)]), path)
        v = values["value"]
        payload = ("true" if v else "false") if isinstance(v, bool) else (
            f"{v:.3f}" if isinstance(v, float) else json.dumps(v))
        wire = f'{{"msgID":"set {concrete}","method":"set","path":"{concrete}","payload":{payload}}}'
        reply = {"msgID": f"set {concrete}", "method": "set", "path": concrete,
                 "payload": v, "success": True}
        text(DCA, name, values, wire, device_reply=json.dumps(reply),
             expect_result={"ok": {"kind": "value", "value": v}})

    # Telemetry: each update rule, from a payload holding every value it reads.
    state_types = doc["state"]
    kinds = {"bool": True, "int": 7, "float": -6.5, "string": "Mic 1"}

    def declared(dotted):
        for pattern, d in state_types.items():
            ps, ds = pattern.split("."), dotted.split(".")
            if len(ps) == len(ds) and all(p in ("*", s) for p, s in zip(ps, ds)):
                return d
        raise KeyError(dotted)

    def put(tree, parts, value):
        for p in parts[:-1]:
            tree = tree.setdefault(p, {})
        tree[parts[-1]] = value

    for i, rule in enumerate(doc["telemetry"]["updates"]):
        if rule.get("json_match") != {"$.method": "^update$"}:
            continue
        each = rule.get("json_each")
        payload, expect = {}, {}
        base = []
        if each:
            # A plugin under params is named (eq1); any other member is an ID.
            parts = each[len("$.payload."):].split(".")
            base = [("eq1" if j and parts[j - 1] == "params" else "1") if p == "*" else p
                    for j, p in enumerate(parts)]
        for jname, rel in rule["json"].items():
            relparts = rel[2:].split(".") if rel.startswith("$.") else []
            if not each:
                relparts = rel[len("$.payload."):].split(".")
            full = base + relparts
            dotted = ".".join(full)
            d = declared(dotted)
            if "JSON text" in d["description"]:
                put(payload, full, [1, 2])
                put(expect, full, "[1,2]")
            else:
                v = kinds[d["type"]]
                put(payload, full, v)
                put(expect, full, v)
        telemetry(DCA, f"update-{i}", inbound_ws=json.dumps({"method": "update", "payload": payload}),
                  expect_state=expect)


_dhd_tree_vectors(DCA)
_dhd_tree_vectors("dhd-aes67")

# Reads, subscriptions and RPCs, their messages as DHD's pages give them.
text(DCA, "get", {"path": "/audio/mixers/0/automix/1/hold"},
     '{"msgID":"get /audio/mixers/0/automix/1/hold","method":"get","path":"/audio/mixers/0/automix/1/hold"}',
     device_reply='{"msgID":"get /audio/mixers/0/automix/1/hold","method":"get","path":"/audio/mixers/0/automix/1/hold","payload":260,"success":true}',
     expect_result={"ok": {"kind": "value", "value": 260}})
text(DCA, "subscribe", {"path": "/audio/levels"},
     '{"msgID":"subscribe /audio/levels","method":"subscribe","path":"/audio/levels"}',
     device_reply='{"msgID":"subscribe /audio/levels","method":"subscribe","path":"/audio/levels","success":true}',
     expect_result={"ok": {"kind": "ack"}})
text(DCA, "unsubscribe", {"path": "/audio/levels"},
     '{"msgID":"unsubscribe /audio/levels","method":"unsubscribe","path":"/audio/levels"}',
     device_reply='{"msgID":"unsubscribe /audio/levels","method":"unsubscribe","path":"/audio/levels","success":false,"error":{"code":1203,"message":"path not found"}}',
     expect_result={"error": {"error": "device_error"}})


def _rpc(command, input, method, params, msgid=None, result=None, value=None):
    msgid = msgid or f"rpc {method}"
    wire = '{"msgID":"%s","method":"rpc","payload":{"method":"%s","params":%s}}' % (msgid, method, params)
    reply = {"msgID": msgid, "method": "rpc", "payload": {"method": method, "params": {}, "result": result if result is not None else {}},
             "success": True}
    expect = {"ok": {"kind": "value", "value": value}} if value is not None else {"ok": {"kind": "ack"}}
    text(DCA, command, input, wire, device_reply=json.dumps(reply), expect_result=expect)


_SNAPS = [{"factory": True, "id": "default", "name": "default", "readgroups": 0, "writegroups": 0},
          {"factory": False, "id": "d2b49cd3-5dea-4f9a-9fe1-382ea7b66fa0", "name": "test", "readgroups": 0,
           "writegroups": 1}]
_rpc("get_channel_snapshots", {}, "getsnapshotlist", '{"type":1}', msgid="snapshots 1", result=_SNAPS, value=_SNAPS)
_rpc("get_mixer_snapshots", {"mixer": 1}, "getsnapshotlist", '{"type":2,"mixer":1}', msgid="snapshots 2 1",
     result=_SNAPS, value=_SNAPS)
_rpc("get_processing_snapshots", {}, "getsnapshotlist", '{"type":3}', msgid="snapshots 3", result=[], value=[])
_U = "d2b49cd3-5dea-4f9a-9fe1-382ea7b66fa0"
_rpc("load_channel_snapshot", {"fader": 1, "id": _U}, "loadsnapshot", '{"mixer":0,"fader":1,"id":"%s"}' % _U)
_rpc("load_mixer_snapshot", {"id": "default"}, "loadsnapshot", '{"type":2,"mixer":0,"id":"default"}')
_rpc("load_processing_snapshot", {"flexprocessing": 1, "id": _U}, "loadsnapshot", '{"fp":1,"id":"%s"}' % _U)
_rpc("save_channel_snapshot", {"fader": 1, "name": "snapname", "writegroups": 1}, "savesnapshot",
     '{"type":1,"mixer":0,"fader":1,"id":"","name":"snapname","writegroups":1,"readgroups":0}')
_rpc("save_mixer_snapshot", {"mixer": 0, "id": "default", "name": "Morning", "writegroups": 1, "readgroups": 0},
     "savesnapshot", '{"type":2,"mixer":0,"id":"default","name":"Morning","writegroups":1,"readgroups":0}')
_rpc("save_processing_snapshot", {"flexprocessing": 2, "id": _U, "name": "Voice", "writegroups": 137},
     "savesnapshot", '{"type":3,"fp":2,"id":"%s","name":"Voice","writegroups":137,"readgroups":0}' % _U)
_rpc("modify_snapshot", {"id": _U, "name": "snapname", "readgroups": 1, "writegroups": 1}, "modifysnapshot",
     '{"id":"%s","name":"snapname","readgroups":1,"writegroups":1}' % _U)
_rpc("remove_snapshot", {"id": _U}, "removesnapshot", '{"id":"%s"}' % _U)
_rpc("get_snapshot", {"id": _U}, "getsnapshot", '{"id":"%s","mixer":0}' % _U,
     result={"snapshotdata": {"a": 1}}, value={"a": 1})
_rpc("set_snapshot", {"snapshotdata": {"a": 1}}, "setsnapshot", '{"snapshotdata":{"a":1}}')
_rpc("fade_to_value", {"fader": 2, "time": 1.5, "value": 0.0}, "fadetovalue",
     '{"mixer":0,"fader":2,"time":1.50,"value":0.00}')
_rpc("set_channel_label", {"sourceid": 1, "label": "labeltext"}, "setchannellabel",
     '{"sourceid":1,"label":"labeltext"}', result={"label": "labeltext", "mixer": 0, "sourceid": 1},
     value={"label": "labeltext", "mixer": 0, "sourceid": 1})
_rpc("register_ext_list", {"list": 1}, "registerextlist", '{"list":1}', msgid="rpc registerextlist 1")
_rpc("unregister_ext_list", {"list": 1}, "unregisterextlist", '{"list":1}', msgid="rpc unregisterextlist 1")
_E = [{"label": "Name1", "background": True, "color": "FF90FF", "pagination": False}]
_rpc("set_ext_list", {"list": 1, "entries": _E}, "setextlist",
     '{"list":1,"entries":%s}' % json.dumps(_E, separators=(",", ":")), msgid="rpc setextlist 1")
_rpc("set_ext_list_page", {"list": 1, "select": 3, "entries": _E}, "setextlist",
     '{"list":1,"select":3,"entries":%s}' % json.dumps(_E, separators=(",", ":")), msgid="rpc setextlist 1")

# Snapshot lists into state, replacing what was there; a save reads them again.
telemetry(DCA, "snapshot-list-mixer", inbound_ws=json.dumps({
    "msgID": "snapshots 2 0", "method": "rpc",
    "payload": {"method": "getsnapshotlist", "params": {"type": 2}, "result": _SNAPS}, "success": True}),
    state_before={"snapshots": {"mixers": {"0": {"old": {"name": "gone", "factory": False, "readgroups": 0,
                                                         "writegroups": 1}}}}},
    expect_state={"snapshots": {"mixers": {"0": {
        "default": {"name": "default", "factory": True, "readgroups": 0, "writegroups": 0},
        _U: {"name": "test", "factory": False, "readgroups": 0, "writegroups": 1}}}}})
telemetry(DCA, "snapshot-list-channel", inbound_ws=json.dumps({
    "msgID": "snapshots 1", "method": "rpc",
    "payload": {"method": "getsnapshotlist", "params": {"type": 1}, "result": _SNAPS[1:]}, "success": True}),
    expect_state={"snapshots": {"channel": {_U: {"name": "test", "factory": False, "readgroups": 0,
                                                 "writegroups": 1}}}})
telemetry(DCA, "snapshot-list-processing", inbound_ws=json.dumps({
    "msgID": "snapshots 3", "method": "rpc",
    "payload": {"method": "getsnapshotlist", "params": {"type": 3}, "result": _SNAPS[1:]}, "success": True}),
    expect_state={"snapshots": {"processing": {_U: {"name": "test", "factory": False, "readgroups": 0,
                                                    "writegroups": 1}}}})
telemetry(DCA, "snapshot-saved", inbound_ws=json.dumps({
    "msgID": "rpc savesnapshot", "method": "rpc",
    "payload": {"method": "savesnapshot", "params": {}, "result": {}}, "success": True}),
    expect_state={},
    expect_then_send=[
        '{"msgID":"snapshots 1","method":"rpc","payload":{"method":"getsnapshotlist","params":{"type":1}}}',
        '{"msgID":"snapshots 2 0","method":"rpc","payload":{"method":"getsnapshotlist","params":{"type":2,"mixer":0}}}',
        '{"msgID":"snapshots 3","method":"rpc","payload":{"method":"getsnapshotlist","params":{"type":3}}}'])
telemetry(DCA, "extlist-event", inbound_ws=json.dumps({
    "method": "event", "payload": {"params": {"entry": 0, "list": 1, "on": 1}, "type": "extlistnotified"}}),
    expect_state={"extlists": {"1": {"entries": {"0": {"pressed": True}}}}})

# The AES67 module's reads and subscriptions are the cores' messages.
for _cmd, _path in [("get", "/general/ptp/domain"), ("subscribe", "/general/ptp/state"),
                    ("unsubscribe", "/general/ptp/state")]:
    text("dhd-aes67", _cmd, {"path": _path},
         '{"msgID":"%s %s","method":"%s","path":"%s"}' % (_cmd, _path, _cmd, _path),
         device_reply=json.dumps({"msgID": f"{_cmd} {_path}", "method": _cmd, "path": _path,
                                  "payload": 0, "success": True}),
         expect_result={"ok": {"kind": "value", "value": 0}} if _cmd == "get" else {"ok": {"kind": "ack"}})

# Every connection authenticates with a token (a test value).
for _v in V:
    if _v["spec"] in (DCA, "dhd-aes67"):
        _v.setdefault("settings", {"token": "t0k"})
