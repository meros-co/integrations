"""Build the extension with cargo and place it in the package, for local
development and tests without maturin. Release wheels are built with maturin
(see README.md).

    python build_dev.py
    python build_dev.py --integrations sennheiser-ew-dx,shure-wireless
    MEROS_INTEGRATIONS=sennheiser-ew-dx,shure-wireless python build_dev.py

The list names spec ids, vendor groups (vendor-sennheiser) or `all`; without
one the extension contains every integration.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parent.parent

parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument("--integrations", default=os.environ.get("MEROS_INTEGRATIONS", ""),
                    help="comma-separated spec ids, vendor groups or 'all' (default: every integration)")
args = parser.parse_args()

integrations = [name for name in re.split(r"[\s,]+", args.integrations) if name]
if any(not re.fullmatch(r"[a-z0-9-]+", name) for name in integrations):
    sys.exit(f"integration names are spec ids, vendor groups or 'all': {args.integrations}")
# Each name is a feature of the core (crates/core/Cargo.toml); cargo refuses
# one it does not have.
features = (["--no-default-features", "--features",
             ",".join(f"meros-integrations/{name}" for name in integrations)]
            if integrations else [])

subprocess.run(["cargo", "build", "-q", "-p", "meros-integrations-py", *features], cwd=root, check=True,
               env={**os.environ, "PYO3_PYTHON": sys.executable})

built = {"win32": "_native.dll", "darwin": "lib_native.dylib"}.get(sys.platform, "lib_native.so")
target = here / "meros_integrations" / ("_native.pyd" if sys.platform == "win32" else "_native.abi3.so")
shutil.copyfile(root / "target" / "debug" / built, target)
print(f"built {target} with {', '.join(integrations) if integrations else 'every integration'}")
