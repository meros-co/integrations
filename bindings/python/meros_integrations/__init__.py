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
from typing import Any, Iterator, NamedTuple, Optional

from ._native import NativeCore

__all__ = ["Core", "Frame", "IntegrationsError", "Stream"]


class IntegrationsError(Exception):
    """An open, validation or command failure, carrying the core's error object."""

    def __init__(self, detail: dict[str, Any]):
        super().__init__(detail.get("message", detail.get("error")))
        #: e.g. "device_error", "invalid_params", "not_connected"
        self.code: str = detail.get("error", "")
        self.detail = detail


class Frame(NamedTuple):
    """One frame of a stream."""

    #: The encoding the spec declares, such as "jpeg".
    format: str
    data: bytes
    #: Rises by one per frame the device published; a gap is frames not seen.
    sequence: int
    #: Frames replaced before they were taken, since the last frame.
    dropped: int


class Stream:
    """One watcher of a device's stream, such as a camera's "live" preview.

    Only the newest frame is kept for a slow reader; ``dropped`` counts the
    frames it replaced. The device produces frames while a stream is open.
    Iterating yields frames until the stream is closed or the device's
    session ends. Usable as a context manager, which closes it.
    """

    def __init__(self, native: NativeCore, stream_id: int) -> None:
        self._native = native
        self._id = stream_id

    def next_frame(self, timeout_ms: int = 5_000) -> Optional[Frame]:
        """The next frame within timeout_ms; None on timeout or once ended."""
        frame = self._native.wait_frame(self._id, timeout_ms)
        return Frame(*frame) if frame is not None else None

    async def next_frame_async(self, timeout_ms: int = 5_000) -> Optional[Frame]:
        return await asyncio.to_thread(self.next_frame, timeout_ms)

    @property
    def ended(self) -> bool:
        """Closed, or the device's session has ended."""
        return self._native.stream_ended(self._id)

    def close(self) -> None:
        """Stop watching. A waiting next_frame returns None. Any thread."""
        self._native.close_stream(self._id)

    def __iter__(self) -> Iterator[Frame]:
        while True:
            frame = self.next_frame(1_000)
            if frame is not None:
                yield frame
            elif self.ended:
                return

    def __enter__(self) -> "Stream":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


class Core:
    """Every open device, one runtime, one event queue. Thread-safe."""

    def __init__(
        self,
        bind_address: Optional[str] = None,
        devices: Optional[list[str]] = None,
    ) -> None:
        """bind_address: the local network interface for device traffic;
        every interface when None.

        devices: the only integrations this core works with: spec ids,
        vendor groups ("vendor-sennheiser") or "all"; every integration in
        the build when None. The catalogue then lists only them,
        opening any other raises IntegrationsError "not_selected", and
        discovery runs only their protocols. Raises RuntimeError for an id
        the build does not include."""
        options: dict[str, Any] = {}
        if bind_address:
            options["bind_address"] = bind_address
        if devices is not None:
            options["devices"] = list(devices)
        self._native = NativeCore(json.dumps(options) if options else None)

    def catalog(self) -> dict[str, Any]:
        return json.loads(self._native.catalog())

    def open(self, request: dict[str, Any]) -> int:
        """Start a session; returns the device id. Connects in the background.

        request: {"device", "model", "host", "port"?, "settings"?, "monitor"?}.
        "monitor": False opens the device for commands only: no subscription,
        connect-time read or poll, so none of its subscription slots is taken."""
        result = json.loads(self._native.open(json.dumps(request)))
        if "error" in result:
            raise IntegrationsError(result["error"])
        return result["device"]

    def discover(self, request: dict[str, Any]) -> None:
        """Listen for devices, scan for them now, or stop:
        {"action": "listen" | "scan" | "stop", "protocols": [...], "hints": [...]}.
        Found devices arrive as {"event": "discovered", ...} events."""
        result = json.loads(self._native.discover(json.dumps(request)))
        if "error" in result:
            raise IntegrationsError(result["error"])

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

    def open_stream(self, device: int, stream: str) -> Stream:
        """Watch a stream declared under the device's "streams" in the
        catalogue. Raises IntegrationsError ("unknown_stream",
        "unsupported_for_model", "closed")."""
        result = json.loads(self._native.open_stream(device, stream))
        if "error" in result:
            raise IntegrationsError(result["error"])
        return Stream(self._native, result["stream"])

    def close(self, device: int) -> None:
        self._native.close(device)

    def close_all(self, grace_ms: int = 5_000) -> None:
        """Shut down cleanly before the process exits: refuse new devices and
        commands ("closing"), give commands in flight up to grace_ms, then close
        every device so each ends what it started on the device."""
        self._native.close_all(grace_ms)
