#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")/.."
cargo build --release --all-features --lib
cargo build --release --all-features --bin xca
printf '%s\n' "Release library and CLI built without cross-target debug-file collisions."