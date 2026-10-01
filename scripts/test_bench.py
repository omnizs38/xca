import importlib.util
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("xca_bench", Path(__file__).with_name("bench.py"))
bench = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(bench)


class BenchTests(unittest.TestCase):
    def test_rejects_path_traversal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(RuntimeError):
                bench.safe_destination(root, "../escape")

    def test_percentile_interpolates(self):
        self.assertEqual(bench.percentile([1.0, 2.0, 3.0, 4.0], 0.25), 1.75)
        self.assertEqual(bench.percentile([1.0, 2.0, 3.0, 4.0], 0.75), 3.25)

    def test_synthetic_generation_is_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = {path.name: bench.sha256(path) for path in bench.generate_synthetic(root)}
            second = {path.name: bench.sha256(path) for path in bench.generate_synthetic(root)}
            self.assertEqual(first, second)
            self.assertIn("near-duplicates.bin", first)
            self.assertIn("random.bin", first)


if __name__ == "__main__":
    unittest.main()
