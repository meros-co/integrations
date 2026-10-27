# Modulo Pi Spydog (modulopi-spydog): JSON-RPC 2.0 on the websocket on 8081,
# the methods of the manual's Spydog page.
SD = "modulopi-spydog"
text(SD, "computer_action", {"action": "rebootComputer"},
     '{"jsonrpc":"2.0","method":"doaction.computer","params":{"action":"rebootComputer"},"id":"computer rebootComputer"}',
     device_reply='{"jsonrpc":"2.0","result":true,"id":"computer rebootComputer"}', expect_result={"ok": {"kind": "ack"}})
V.append({"spec": SD, "command": "computer_action", "file": "computer_action.false", "input": {"action": "startModuloPlayer"},
          "expect_wire": '{"jsonrpc":"2.0","method":"doaction.computer","params":{"action":"startModuloPlayer"},"id":"computer startModuloPlayer"}',
          "device_reply": '{"jsonrpc":"2.0","result":false,"id":"computer startModuloPlayer"}',
          "expect_result": {"error": {"error": "device_error"}}})
STATIC = {"CPU": "AMD Ryzen 9 7950X", "GpuBrand": "NVIDIA", "GpuDriver": "552.44", "GpuName": "RTX A5000", "ModuloPlayer": "6.3.110",
          "OS": "Windows 10 IoT", "processorCount": 32, "totalMemory": 64}
DYN = {"clusterId": 0, "color": "#ff8800", "cpuTemperature": 52, "cpuUse": 12.5, "detacastTemperature": "48=50", "fps": 60, "fpsOk": True,
       "master": True, "maxAutocalibOutputs": 4, "maxOutputs": 8, "memoryUse": 30.5, "motherboardTemperature": 41,
       "serverIp": "10.0.0.10", "serverName": "MP-01", "serverTime": "2026-10-27 12:00:00", "status": 2, "upTime": "2 days"}
text(SD, "get_static_info", {}, '{"jsonrpc":"2.0","method":"get.computer.static.info","id":"static"}',
     device_reply=json.dumps({"jsonrpc": "2.0", "result": STATIC, "id": "static"}), expect_result={"ok": {"kind": "value", "value": STATIC}})
text(SD, "get_dynamic_info", {}, '{"jsonrpc":"2.0","method":"get.computer.dynamic.info","id":"dynamic"}',
     device_reply=json.dumps({"jsonrpc": "2.0", "result": DYN, "id": "dynamic"}), expect_result={"ok": {"kind": "value", "value": DYN}})
telemetry(SD, "dynamic", inbound_ws=json.dumps({"jsonrpc": "2.0", "result": DYN, "id": "poll dynamic"}), expect_state={
    "server": {"status": 2, "name": "MP-01", "ip": "10.0.0.10", "time": "2026-10-27 12:00:00", "uptime": "2 days", "master": True,
               "cluster_id": 0, "color": "#ff8800"},
    "load": {"cpu": 12.5, "memory": 30.5, "fps": 60.0, "fps_ok": True},
    "temperatures": {"cpu": 52.0, "motherboard": 41.0, "deltacast": "48=50"},
    "licence": {"max_outputs": 8, "max_autocalib_outputs": 4}})
telemetry(SD, "static", inbound_ws=json.dumps({"jsonrpc": "2.0", "result": STATIC, "id": "poll static"}), expect_state={"hardware": {
    "cpu": "AMD Ryzen 9 7950X", "gpu": "RTX A5000", "gpu_brand": "NVIDIA", "gpu_driver": "552.44", "os": "Windows 10 IoT",
    "processors": 32, "memory_gb": 64.0, "player_version": "6.3.110"}})
