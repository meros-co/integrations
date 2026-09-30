"""Build the extension with cargo and place it in the package, for local
development and tests without maturin. Release wheels are built with maturin.

    python build_dev.py
"""
import shutil
import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parent.parent

subprocess.run(["cargo", "build", "-q", "-p", "meros-integrations-py"], cwd=root, check=True,
               env={**__import__("os").environ, "PYO3_PYTHON": sys.executable})

built = {"win32": "_native.dll", "darwin": "lib_native.dylib"}.get(sys.platform, "lib_native.so")
target = here / "meros_integrations" / ("_native.pyd" if sys.platform == "win32" else "_native.abi3.so")
shutil.copyfile(root / "target" / "debug" / built, target)
print(f"built {target}")
