#!/usr/bin/env python3
"""Build the core with each integration on its own, so no integration depends
on code that only another one's feature compiles.

    python tools/check_features.py                 # every integration, and none
    python tools/check_features.py --groups        # also every vendor group and `all`
    python tools/check_features.py sony-camera pjlink
    python tools/check_features.py --offline       # extra arguments go to cargo

Runs `cargo check -p meros-integrations --all-targets --no-default-features
--features <one>` for each. It recompiles the core once per feature, so a full
run takes a while. Exits non-zero if any build fails.
"""
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def features():
    """The core's features, in Cargo.toml order: integrations and vendor groups."""
    manifest = (ROOT / "crates" / "core" / "Cargo.toml").read_text(encoding="utf-8")
    section = manifest.split("\n[features]\n", 1)[1].split("\n[", 1)[0]
    names = re.findall(r"^([a-z0-9-]+) = ", section, re.M)
    integrations = [n for n in names if n not in ("default", "all") and not n.startswith("vendor-")]
    groups = [n for n in names if n.startswith("vendor-")]
    return integrations, groups


def main(argv):
    groups_too = "--groups" in argv
    cargo_args = [a for a in argv if a.startswith("--") and a != "--groups"]
    chosen = [a for a in argv if not a.startswith("--")]
    integrations, groups = features()
    unknown = [c for c in chosen if c not in integrations + groups + ["all"]]
    if unknown:
        sys.exit(f"not a feature of the core: {', '.join(unknown)}")
    targets = chosen or ([""] + integrations + (groups + ["all"] if groups_too else []))

    failed = []
    started = time.monotonic()
    for feature in targets:
        cmd = ["cargo", "check", "-q", "-p", "meros-integrations", "--all-targets",
               "--no-default-features", *cargo_args]
        if feature:
            cmd += ["--features", feature]
        t0 = time.monotonic()
        result = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        took = time.monotonic() - t0
        label = feature or "(no features)"
        # A warning is a defect too: some integration's build would fail clippy.
        warned = "warning" in result.stderr
        if result.returncode != 0 or warned:
            failed.append(label)
            print(f"FAIL {label} ({took:.0f}s)\n{result.stderr}", flush=True)
        else:
            print(f"ok   {label} ({took:.0f}s)", flush=True)
    total = time.monotonic() - started
    print(f"{len(targets) - len(failed)}/{len(targets)} built on their own in {total:.0f}s")
    if failed:
        sys.exit("failed: " + ", ".join(failed))


if __name__ == "__main__":
    main(sys.argv[1:])
