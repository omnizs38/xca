#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <corpus-file>" >&2
  exit 2
fi

for tool in cargo xz lz4 sha256sum; do
  command -v "$tool" >/dev/null || { echo "Missing required tool: $tool" >&2; exit 1; }
done

input=$(realpath "$1")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --release
xca=$(realpath target/release/xca)

printf 'Corpus: %s\n' "$input"
printf 'SHA-256: '
sha256sum "$input" | cut -d' ' -f1
printf 'Input bytes: %s\n\n' "$(wc -c < "$input")"

run() {
  local name=$1
  shift
  printf '== %s ==\n' "$name"
  /usr/bin/time -p "$@"
  printf '\n'
}

run_to_file() {
  local name=$1
  local output=$2
  shift 2
  printf '== %s ==\n' "$name"
  /usr/bin/time -p "$@" > "$output"
  printf '\n'
}

run 'XCA encode' "$xca" compress "$input" "$work/data.xca"
run 'XCA decode' "$xca" decompress "$work/data.xca" "$work/data.xca.out"
cmp "$input" "$work/data.xca.out"

run_to_file 'LZMA encode' "$work/data.lzma" xz --format=lzma -9 -c "$input"
run_to_file 'LZMA decode' "$work/data.lzma.out" xz --format=lzma -d -c "$work/data.lzma"
cmp "$input" "$work/data.lzma.out"

run_to_file 'LZMA2 encode' "$work/data.xz" xz -9 -c "$input"
run_to_file 'LZMA2 decode' "$work/data.xz.out" xz -d -c "$work/data.xz"
cmp "$input" "$work/data.xz.out"

run 'LZ4 encode' lz4 -q -f "$input" "$work/data.lz4"
run 'LZ4 decode' lz4 -q -d -f "$work/data.lz4" "$work/data.lz4.out"
cmp "$input" "$work/data.lz4.out"

printf '\n%-12s %12s\n' 'Format' 'Bytes'
printf '%-12s %12s\n' 'input' "$(wc -c < "$input")"
printf '%-12s %12s\n' 'xca' "$(wc -c < "$work/data.xca")"
printf '%-12s %12s\n' 'lzma' "$(wc -c < "$work/data.lzma")"
printf '%-12s %12s\n' 'lzma2/xz' "$(wc -c < "$work/data.xz")"
printf '%-12s %12s\n' 'lz4' "$(wc -c < "$work/data.lz4")"
