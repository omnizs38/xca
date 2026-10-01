#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <corpus-file>" >&2
  exit 2
fi
root=$(cd "$(dirname "$0")/.." && pwd)
exec python3 "$root/scripts/bench.py" run --suite quick --input "$1"
