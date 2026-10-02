"""Plays tests/bindings/script.json through the raw Python binding against
integrations-sim, the same script every other delivery runs.

    python build_dev.py && python -m unittest discover -s tests
"""
import json
import subprocess
import sys
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent.parent
sys.path.insert(0, str(HERE.parent))

from meros_integrations import Core, IntegrationsError  # noqa: E402
from meros_integrations._native import NativeCore  # noqa: E402


def strip_messages(v):
    if isinstance(v, dict):
        return {k: strip_messages(x) for k, x in v.items() if k != "message"}
    if isinstance(v, list):
        return [strip_messages(x) for x in v]
    return v


def substitute(v, variables):
    if isinstance(v, str) and v.startswith("$"):
        return variables[v[1:]]
    if isinstance(v, dict):
        return {k: substitute(x, variables) for k, x in v.items()}
    if isinstance(v, list):
        return [substitute(x, variables) for x in v]
    return v


def at(v, path):
    for key in path.split("."):
        v = v.get(key) if isinstance(v, dict) else None
    return v


class SharedScript(unittest.TestCase):
    def setUp(self):
        subprocess.run(["cargo", "build", "-q", "-p", "meros-integrations-sim"], cwd=ROOT, check=True)
        exe = ROOT / "target" / "debug" / ("integrations-sim.exe" if sys.platform == "win32" else "integrations-sim")
        self.sim = subprocess.Popen([str(exe)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.ports = json.loads(self.sim.stdout.readline())

    def tearDown(self):
        self.sim.stdin.close()
        self.sim.wait()
        self.sim.stdout.close()

    def test_the_shared_binding_script(self):
        script = json.loads((ROOT / "tests" / "bindings" / "script.json").read_text())
        native = NativeCore()
        variables = dict(self.ports)
        for index, raw in enumerate(script["steps"]):
            step = substitute(raw, variables)
            label = f"step {index}: {raw}"
            op = step["op"]
            if op == "catalog":
                self.assertEqual(at(json.loads(native.catalog()), step["path"]), step["expect"], label)
            elif op == "open":
                result = json.loads(native.open(json.dumps(step["request"])))
                if "expect" in step:
                    self.assertEqual(strip_messages(result), step["expect"], label)
                if "save" in step:
                    variables[step["save"]] = result["device"]
            elif op == "wait":
                deadline = time.monotonic() + 5
                while at(json.loads(native.snapshot(step["device"])), step["path"]) != step["equals"]:
                    self.assertLess(time.monotonic(), deadline, f"{label}: timed out")
                    time.sleep(0.02)
            elif op == "execute":
                result = json.loads(native.execute(step["device"], step["command"], json.dumps(step["params"])))
                self.assertEqual(strip_messages(result), step["expect"], label)
            elif op == "stream":
                opened = json.loads(native.open_stream(step["device"], step["stream"]))
                if "expect" in step:
                    self.assertEqual(strip_messages(opened), step["expect"], label)
                    continue
                stream_id = opened["stream"]
                last = 0
                for _ in range(step["frames"]):
                    frame = native.wait_frame(stream_id, 5000)
                    self.assertIsNotNone(frame, f"{label}: no frame")
                    fmt, data, sequence, _dropped = frame
                    self.assertEqual(fmt, step["format"], label)
                    self.assertIsInstance(data, bytes, label)
                    self.assertEqual(data[:2], bytes([0xFF, 0xD8]), label)
                    self.assertGreater(sequence, last, label)
                    last = sequence
                self.assertFalse(native.stream_ended(stream_id))
                native.close_stream(stream_id)
                self.assertIsNone(native.wait_frame(stream_id, 100))
                self.assertTrue(native.stream_ended(stream_id))
            elif op == "close":
                native.close(step["device"])
            else:
                self.fail(f"unknown op {op}")

    def test_the_idiomatic_wrapper(self):
        core = Core()
        with self.assertRaises(IntegrationsError) as caught:
            core.open({"device": "no-such-device", "model": "x", "host": "127.0.0.1"})
        self.assertEqual(caught.exception.code, "unknown_device")
        device = core.open({"device": "kramer-p3000", "model": "p3000-generic", "host": "127.0.0.1",
                            "port": self.ports["kramer"]})
        deadline = time.monotonic() + 5
        while core.snapshot(device)["connection"]["status"] != "connected":
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.02)
        self.assertEqual(core.execute(device, "get_model"), {"kind": "value", "value": "VS-88UT"})
        with self.assertRaises(IntegrationsError) as caught:
            core.execute(device, "route_video", {"input": 300, "output": 1})
        self.assertEqual(caught.exception.code, "invalid_params")
        core.close(device)

    def test_streams(self):
        core = Core()
        device = core.open({"device": "http-snapshot", "model": "generic", "host": "127.0.0.1",
                            "port": self.ports["snapshot"],
                            "settings": {"path": "/snapshot.jpg", "interval_ms": 50}})
        with self.assertRaises(IntegrationsError) as caught:
            core.open_stream(device, "nope")
        self.assertEqual(caught.exception.code, "unknown_stream")
        with core.open_stream(device, "live") as stream:
            frames = []
            for frame in stream:
                frames.append(frame)
                if len(frames) == 2:
                    break
        self.assertEqual(frames[0].format, "jpeg")
        self.assertTrue(frames[0].data.startswith(bytes([0xFF, 0xD8])))
        self.assertGreater(frames[1].sequence, frames[0].sequence)
        self.assertTrue(stream.ended)
        core.close(device)


if __name__ == "__main__":
    unittest.main()
