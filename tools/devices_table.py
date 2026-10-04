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
    "line-udp": "Line/UDP",
    "osc-udp": "OSC/UDP",
    "osc-tcp": "OSC/TCP",
    "http": "HTTP",
}

# Native modules choose their own transport, so it is described here.
NATIVE = {
    "aes70": "OCP.1 over TCP (the device's port)",
    "allenheath-ahm": "MIDI over TCP 51325",
    "allenheath-cq": "MIDI over TCP 51325",
    "allenheath-dlive": "MIDI over TCP 51325 (MixRack) or 51328 (Surface)",
    "allenheath-qu": "MIDI over TCP 51325",
    "allenheath-sq": "MIDI over TCP 51325",
    "analogway-alta4k": "AWJ over TCP 10606",
    "analogway-livepremier": "AWJ over TCP 10606",
    "analogway-midra4k": "AWJ over TCP 10606",
    "biamp-tesira": "Tesira Text Protocol over Telnet, TCP 23",
    "blackmagic-atem": "Proprietary UDP 9910",
    "bss-london": "London Direct Inject over TCP 1023",
    "emberplus": "Ember+ (S101) over TCP",
    "focusrite-rednet": "OCP.1 over TCP (the device's port)",
    "generic-http": "HTTP or HTTPS",
    "generic-osc": "OSC over UDP or TCP",
    "generic-tcp-udp": "Text or bytes over TCP or UDP",
    "labgruppen-lake": "DLM over UDP 6016 (or 6015, answers on 6004)",
    "http-snapshot": "HTTP JPEG snapshots",
    "magewell-proconvert": "HTTP/1.1 with a session cookie (80, or 443 over HTTPS)",
    "novastar-central-control": "Binary frames over TCP 5200",
    "novastar-h": "Signed JSON over HTTP 8000",
    "obs-studio": "obs-websocket 5 over WebSocket 4455",
    "obsidian-onyx": "Telnet over TCP 2323",
    "omt": "OMT metadata frames over TCP (the source's port, 6400 first)",
    "omt-discovery": "OMT metadata frames over TCP 6399",
    "osc-listener": "OSC over UDP or TCP from any sender; receives",
    "pjlink": "PJLink over TCP 4352",
    "probel-swp08": "SW-P-08 binary frames over TCP (2008 by default)",
    "qsys": "QRC JSON-RPC over TCP 1710",
    "sennheiser-digital-6000": "SSC over UDP 45",
    "sennheiser-ew-dx": "HTTPS + SSE (SSCv2)",
    "sennheiser-ew-g3-g4": "MCP over UDP 53212",
    "sennheiser-spectera": "HTTPS + SSE (SSCv2)",
    "shure-wireless": "Command strings over TCP 2202",
    "softouch-easyworship": "JSON lines over TCP (the port EasyWorship advertises over Bonjour)",
    "sony-camera": "PTP-IP over TCP 15740, or through SSH",
    "tsl-umd-display": "TSL UMD over UDP, or TCP for V5.0; sends",
    "tsl-umd-listener": "TSL UMD over UDP, or TCP for V3.1 and V5.0; receives",
    "visca": "VISCA over IP (UDP 52381), raw VISCA over TCP or UDP",
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
            transport = f"{kind} {t['port']}"
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
