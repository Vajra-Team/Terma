#!/usr/bin/env bash
# Baut das Paket zum Veröffentlichen aus dem Commit HEAD in einem frischen Arch-Container und installiert es dort zur Probe.
# Nie auf einem eigenen Rechner bauen: CachyOS setzt in makepkg -march=native, so ein Paket stürzt auf anderen CPUs ab.
# Ergebnis liegt in release/.
set -euo pipefail
cd "$(dirname "$0")/.."
ziel="$PWD/release"
rm -rf "$ziel"
mkdir -p "$ziel"
sudo docker build -q -t terma-arch -f dev/Dockerfile.arch dev >/dev/null
git archive HEAD | sudo docker run -i --rm --user root -v "$ziel:/out" -e "OWNER=$(id -u):$(id -g)" terma-arch bash -euc '
  pacman -Syu --noconfirm --needed >/dev/null
  mkdir /home/builder/src
  tar -x -C /home/builder/src
  chown -R builder: /home/builder/src
  su builder -c "cd ~/src/packaging && makepkg -f -d --noprogressbar" >&2
  cp /home/builder/src/packaging/terma-*.pkg.tar.zst /out/
  chown -R "$OWNER" /out
  pacman -U --noconfirm /out/terma-*.pkg.tar.zst >/dev/null
  if ldd /usr/bin/terma /usr/lib/terma/terma-helper /usr/lib/terma/terma-helper-aur | grep "not found"; then exit 1; fi
  test -f /usr/share/licenses/terma/LICENSE
  pacman -Q terma
'
cd "$ziel"
sha256sum terma-*.pkg.tar.zst
