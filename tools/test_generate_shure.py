"""generate_shure.py reproduces the committed Shure specs and vector modules:
python -m unittest discover -s tools -p "test_*.py"."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parent


class GeneratorMatchesCommittedFiles(unittest.TestCase):
    def test_output_is_byte_for_byte_the_committed_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "specs").mkdir()
            (out / "tools" / "vectors").mkdir(parents=True)
            subprocess.run([sys.executable, str(TOOLS / "generate_shure.py"), str(out)],
                           check=True, capture_output=True)
            written = sorted(p.relative_to(out).as_posix() for p in out.rglob("*") if p.is_file())
            self.assertTrue(written, "the generator wrote nothing")
            for rel in written:
                with self.subTest(file=rel):
                    self.assertEqual((out / rel).read_bytes(), (ROOT / rel).read_bytes(),
                                     f"{rel} differs from tools/generate_shure.py's output: "
                                     "change the generator, not the file, and run it again")


if __name__ == "__main__":
    unittest.main()
