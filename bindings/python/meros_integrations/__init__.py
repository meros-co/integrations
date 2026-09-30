"""One implementation of third-party device control and telemetry.

The native layer returns JSON shaped by the core itself; this module only turns
it into dicts, raises IntegrationsError for failures, and offers asyncio
wrappers. It holds no protocol logic, retries or timeouts.

    from meros_integrations import Core

    core = Core()
    device = core.open({"device": "behringer-x32", "model": "x32", "host": "10.0.0.20"})
    name = core.execute(device, "get_channel_name", {"channel": 1})
    # {"kind": "value", "value": "Kick"}
"""
from __future__ import annotations

import asyncio
import json
from typing import Any, Optional

from ._native import NativeCore

__all__ = ["Core", "IntegrationsError"]


class IntegrationsError(Exception):
    """An open, validation or command failure, carrying the core's error object."""

    def __init__(self, detail: dict[str, Any]):
        super().__init__(detail.get("message", detail.get("error")))
        #: e.g. "device_error", "invalid_params", "not_connected"
        self.code: str = detail.get("error", "")
        self.detail = detail


class Core:
    """Every open device, one runtime, one event queue. Thread-safe."""

    def __init__(self, bind_address: Optional[str] = None) -> None:
        """bind_address: the local network interface for device traffic;
        every interface when None."""
        options = json.dumps({"bind_address": bind_address}) if bind_address else None
        self._native = NativeCore(options)

    def catalog(self) -> dict[str, Any]:
        return json.loads(self._native.catalog())

    def open(self, request: dict[str, Any]) -> int:
        """Start a session; returns the device id. Connects in the background."""
        result = json.loads(self._native.open(json.dumps(request)))
        if "error" in result:
            raise IntegrationsError(result["error"])
        return result["device"]

    def execute(self, device: int, command: str, params: Optional[dict[str, Any]] = None) -> dict[str, Any]:
        """Run a command and wait for its outcome: {"kind": "ack" | "value" | "unverified", ...}."""
        result = json.loads(self._native.execute(device, command, json.dumps(params or {})))
        if "error" in result:
            raise IntegrationsError(result["error"])
        return result["ok"]

    async def execute_async(self, device: int, command: str, params: Optional[dict[str, Any]] = None) -> dict[str, Any]:
        return await asyncio.to_thread(self.execute, device, command, params)

    def snapshot(self, device: int) -> Optional[dict[str, Any]]:
        """Last known state and connection status; None if the device is not open."""
        return json.loads(self._native.snapshot(device))

    def poll_events(self, max: int = 256) -> list[dict[str, Any]]:
        return json.loads(self._native.poll_events(max))

    def wait_events(self, max: int = 256, timeout_ms: int = 25_000) -> list[dict[str, Any]]:
        """Wait for events; [] on timeout or after interrupt_events()."""
        return json.loads(self._native.wait_events(max, timeout_ms))

    async def wait_events_async(self, max: int = 256, timeout_ms: int = 25_000) -> list[dict[str, Any]]:
        return await asyncio.to_thread(self.wait_events, max, timeout_ms)

    def interrupt_events(self) -> None:
        self._native.interrupt_events()

    def close(self, device: int) -> None:
        self._native.close(device)
