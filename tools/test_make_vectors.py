"""Unit tests for make_vectors.py: python -m unittest discover -s tools -p "test_*.py"."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import make_vectors  # noqa: E402


def cmd(command, file=None, **input):
    v = {"spec": "x", "command": command, "input": input, "expect_wire": "w\r\n"}
    if file:
        v["file"] = file
    return v


class VectorFiles(unittest.TestCase):
    def test_two_vectors_on_one_path_are_refused(self):
        with self.assertRaises(make_vectors.DuplicateVector) as e:
            make_vectors.vector_files([cmd("tie", input=1), cmd("tie", input=2)])
        self.assertIn("vectors/x/tie.yaml is written by two vectors", str(e.exception))

    def test_paths_differing_only_in_case_are_refused(self):
        with self.assertRaises(make_vectors.DuplicateVector):
            make_vectors.vector_files([cmd("tie"), cmd("tie", file="Tie")])

    def test_a_telemetry_vector_and_a_file_name_can_collide(self):
        with self.assertRaises(make_vectors.DuplicateVector):
            make_vectors.vector_files([{"spec": "x", "telemetry": "a", "inbound": "a"},
                                       cmd("a", file="telemetry-a")])

    def test_distinct_file_names_are_all_written_without_file(self):
        files = make_vectors.vector_files([cmd("tie"), cmd("tie", file="tie-error"),
                                           {"spec": "x", "telemetry": "tie", "inbound": "t"}])
        self.assertEqual([rel for rel, _ in files], ["x/tie.yaml", "x/tie-error.yaml", "x/telemetry-tie.yaml"])
        self.assertNotIn("file", files[1][1])

    def test_the_generated_vectors_have_no_duplicate_paths(self):
        make_vectors.vector_files(make_vectors.V)

    def test_a_duplicate_stops_main_before_anything_is_written(self):
        saved = list(make_vectors.V)
        root = make_vectors.ROOT
        try:
            make_vectors.ROOT = Path("/nonexistent-root-never-written")
            make_vectors.V[:] = [cmd("tie"), cmd("tie")]
            with self.assertRaises(SystemExit) as e:
                make_vectors.main()
            self.assertIn("nothing written", str(e.exception))
        finally:
            make_vectors.V[:] = saved
            make_vectors.ROOT = root


if __name__ == "__main__":
    unittest.main()
