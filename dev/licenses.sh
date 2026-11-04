#!/usr/bin/env bash
# Schreibt packaging/THIRD-PARTY-LICENSES.txt neu. Bricht ab, wenn eine Abhängigkeit eine Lizenz außerhalb von dev/about.toml hat.
set -euo pipefail
cd "$(dirname "$0")/.."
sudo docker build -q -t terma-arch -f dev/Dockerfile.arch dev >/dev/null
tmp=$(mktemp)
sudo docker run --rm --user root -v terma-about:/opt/about -v "$PWD:/src:ro" -w /src -e CARGO_HOME=/tmp/cargo -e CARGO_TARGET_DIR=/tmp/target terma-arch bash -euc '
  cargo install --quiet --locked --features cli --version 0.9.3 --root /opt/about cargo-about >&2
  /opt/about/bin/cargo-about generate --locked --fail -c dev/about.toml dev/about.hbs' > "$tmp"
# Die beiden Pakete, die vite ins Fenster bündelt.
for lizenz in node_modules/svelte/LICENSE.md node_modules/@tauri-apps/api/LICENSE-MIT; do
  printf '\n%s\n%s\n\n' '--------------------------------------------------------------------------------' "$lizenz" >> "$tmp"
  cat "$lizenz" >> "$tmp"
done
mv "$tmp" packaging/THIRD-PARTY-LICENSES.txt
chmod 644 packaging/THIRD-PARTY-LICENSES.txt
