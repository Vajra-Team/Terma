#!/usr/bin/env bash
# Führt die Rust-Tests im Arch-Container aus. Argumente gehen an `cargo test --workspace`.
set -euo pipefail
cd "$(dirname "$0")/.."
sudo docker build -q -t terma-arch -f dev/Dockerfile.arch dev >/dev/null
exec sudo docker run --rm -v "$PWD:/src" -w /src -e CARGO_HOME=/src/target/cargo-home terma-arch cargo test --workspace "$@"
