import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from bindings import xca

source = b"XCA universal integration example" * 1000
compressed = xca.compress(source)
restored = xca.decompress(compressed)
assert restored == source
print(f"XCA {xca.version()}: {len(source)} -> {len(compressed)} -> {len(restored)} bytes")
