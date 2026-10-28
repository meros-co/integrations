#!/usr/bin/env python3
"""Rebuild the table of integrations in DEVICES.md from specs/*.yaml.

Vendor groups come from crates/core/Cargo.toml. Run it after adding a spec
or changing a spec's models or commands:

    python tools/devices_table.py
"""
import sys
import tomllib
from collections import Counter
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
HEADER = "| Spec | Devices | Vendor group | Transport | Native | Models | Commands | Verification |"

TRANSPORT = {
    "line-tcp": "Line/TCP",
    "line-serial": "Line/serial",
    "line-udp": "Line/UDP",
    "osc-udp": "OSC/UDP",
    "osc-tcp": "OSC/TCP",
    "http": "HTTP",
    "ws": "WebSocket",
}

# Native modules choose their own transport, so it is described here.
NATIVE = {
    "aes70": "OCP.1 over TCP, UDP, WebSocket, TLS or DTLS (the device's port)",
    "allenheath-ahm": "MIDI over TCP 51325",
    "allenheath-cq": "MIDI over TCP 51325",
    "allenheath-dlive": "MIDI over TCP 51325 (MixRack) or 51328 (Surface)",
    "allenheath-qu": "MIDI over TCP 51325",
    "allenheath-sq": "MIDI over TCP 51325",
    "analogway-alta4k": "AWJ over TCP 10606",
    "analogway-livepremier": "AWJ over TCP 10606",
    "analogway-midra4k": "AWJ over TCP 10606",
    "apc-rack-pdu": "SNMP v1, v2c or v3 over UDP 161 (PowerNet-MIB); traps and informs",
    "apc-ups": "SNMP v1, v2c or v3 over UDP 161 (PowerNet-MIB); traps and informs",
    "artnet": "Art-Net 4 over UDP 6454 (broadcast poll, unicast DMX); sends and receives",
    "barco-projection-protocol": "Barco Projection Protocol, binary over TCP 43680 or RS-232",
    "biamp-tesira": "Tesira Text Protocol over Telnet, TCP 23",
    "blackmagic-atem": "Proprietary UDP 9910",
    "bss-london": "London Direct Inject over TCP 1023",
    "cyberpower-pdu": "SNMP v1, v2c or v3 over UDP 161 (CPS-MIB); traps and informs",
    "cyberpower-ups": "SNMP v1, v2c or v3 over UDP 161 (CPS-MIB); traps and informs",
    "datavideo-switcher": "Binary command and real-time ports over TCP 5001-5009, RS-232, the SE-2200's serial blocks; DVIP UDP 5002 management",
    "dbaudio": "OCP.1 over TCP 30013 or 50014 by model",
    "dhd-ecp": "ECP 16-byte blocks over TCP 2008",
    "emberplus": "Ember+ (S101) over TCP",
    "focusrite-rednet": "OCP.1 over TCP (the device's port)",
    "generic-http": "HTTP or HTTPS",
    "generic-midi": "MIDI 1.0 over RTP-MIDI (UDP 5004/5005) or a MIDI port of the host",
    "generic-osc": "OSC over UDP or TCP",
    "generic-ping": "ICMP echo or a TCP connection; Wake-on-LAN over UDP",
    "generic-snmp": "SNMP v1, v2c or v3 over UDP 161; traps and informs",
    "generic-tcp-udp": "Text or bytes over TCP, UDP or a serial port",
    "generic-websocket": "Text, bytes or JSON over WebSocket (ws or wss)",
    "christie-pandorasbox": "Pandoras Box Automation binary messages over TCP 6211",
    "epson-projector": "ESC/VP21 over ESC/VP.net (TCP 3629) or a serial port",
    "labgruppen-lake": "DLM over UDP 6016 (or 6015, answers on 6004)",
    "http-listener": "HTTP/1.1 requests from any sender (webhooks); receives",
    "http-snapshot": "HTTP JPEG snapshots",
    "grassvalley-amp": "AMP as hexadecimal text over TCP 3811, or binary on an RS-422 port",
    "meros-camera": "HTTP 8790 (JSON, NDJSON event stream, binary preview)",
    "midi-show-control": "MSC SysEx over RTP-MIDI (UDP 5004/5005) or a MIDI port of the host",
    "nec-display": "External Control binary messages over TCP 7142, or an RS-232C port",
    "nec-projector": "NEC binary commands over TCP 7142 or a serial port",
    "novastar-central-control": "Binary frames over TCP 5200",
    "novastar-h": "Signed JSON over HTTP 8000",
    "obs-studio": "obs-websocket 5 over WebSocket 4455",
    "obsidian-onyx": "Telnet over TCP 2323",
    "omt": "OMT metadata frames over TCP (the source's port, 6400 first)",
    "omt-discovery": "OMT metadata frames over TCP 6399",
    "osc-listener": "OSC over UDP or TCP from any sender; receives",
    "panasonic-projector": "NTCONTROL over TCP 1024, or RS-232C",
    "philips-sicp": "SICP binary messages over TCP 5000, or an RS-232 port",
    "pjlink": "PJLink over TCP 4352",
    "posistagenet": "PosiStageNet over UDP multicast 236.10.10.10:56565; receives and sends",
    "probel-swp02": "SW-P-02 binary messages on a serial port, or over TCP",
    "probel-swp08": "SW-P-08 binary frames over TCP (2008 by default) or a serial port",
    "qsys": "QRC JSON-RPC over TCP 1710",
    "sacn": "sACN (E1.31) over UDP 5568, multicast 239.255.x.x and unicast; sends and receives",
    "samsung-mdc": "MDC binary frames over TCP 1515 (TLS with a PIN in secured mode) or RS-232C",
    "sennheiser-digital-6000": "SSC over UDP 45",
    "sennheiser-ew-dx": "HTTPS + SSE (SSCv2); UDP (SSCv1 legacy mode)",
    "sennheiser-ew-g3-g4": "MCP over UDP 53212",
    "sennheiser-spectera": "HTTPS + SSE (SSCv2)",
    "shure-wireless": "Command strings over TCP 2202",
    "softouch-easyworship": "JSON lines over TCP (the port EasyWorship advertises over Bonjour)",
    "sony-9pin": "9-pin binary messages on an RS-422 port of the host, or a serial-to-Ethernet converter",
    "sony-bravia-serial": "Binary frames on an RS-232C port, or a serial-to-Ethernet converter",
    "sony-camera": "PTP-IP over TCP 15740, or through SSH",
    "tally-arbiter": "Socket.IO 4 over a WebSocket on 4455",
    "tsl-umd-display": "TSL UMD over UDP, or TCP for V5.0; sends",
    "tsl-umd-listener": "TSL UMD over UDP, or TCP for V3.1 and V5.0; receives",
    "ups-mib": "SNMP v1, v2c or v3 over UDP 161 (UPS-MIB, RFC 1628); traps and informs",
    "vdcp": "VDCP binary messages on an RS-422 port of the host, or over TCP",
    "visca": "VISCA over IP (UDP 52381), raw VISCA over TCP or UDP, or VISCA on a serial port",
    "vmix": "TCP API 8099",
}


def vendor_groups():
    features = tomllib.loads((ROOT / "crates/core/Cargo.toml").read_text(encoding="utf-8"))["features"]
    groups = {}
    for name, members in features.items():
        if name.startswith("vendor-"):
            for member in members:
                groups[member] = name
    return groups


def verification(models):
    counts = Counter((m.get("verification") or "none") for m in models)
    if len(counts) == 1:
        return f"`{next(iter(counts))}`"
    return ", ".join(f"{counts[s]} `{s}`" for s in ("field", "bench", "none") if counts[s])


def main():
    groups = vendor_groups()
    rows = []
    total = 0
    for path in sorted((ROOT / "specs").glob("*.yaml")):
        doc = yaml.safe_load(path.read_text(encoding="utf-8"))
        sid = doc["id"]
        if sid not in groups:
            sys.exit(f"{sid} is in no vendor group in crates/core/Cargo.toml")
        models = doc.get("models") or []
        commands = len(doc.get("commands") or {})
        total += commands
        native = doc.get("implementation") == "native"
        if native:
            if sid not in NATIVE:
                sys.exit(f"no transport text for native spec {sid}; add it to NATIVE in {Path(__file__).name}")
            transport = NATIVE[sid]
        else:
            t = doc["transport"]
            kind = TRANSPORT[t["type"]]
            if t["type"] == "http" and t.get("scheme") == "https":
                kind = "HTTPS"
            # A serial port has no number.
            transport = f"{kind} {t['port']}" if "port" in t else kind
            # The same protocol on a serial port of the host (transport.serial).
            if "serial" in t:
                transport += " or serial"
        rows.append(
            f"| `{sid}` | {doc['name']} | `{groups[sid]}` | {transport} | {'yes' if native else ''} "
            f"| {len(models)} | {commands} | {verification(models)} |"
        )

    table = "\n".join([HEADER, "|---|---|---|---|---|---|---|---|", *rows])
    out = ROOT / "DEVICES.md"
    text = out.read_text(encoding="utf-8")
    start = text.index(HEADER)
    end = text.find("\n\n", start)
    text = text[:start] + table + (text[end:] if end != -1 else "\n")
    out.write_text(text, encoding="utf-8", newline="\n")
    print(f"{len(rows)} integrations, {total} commands")


if __name__ == "__main__":
    main()
