---
name: deploy
description: Use when Terma veröffentlicht werden soll, also bei `/deploy`, „Release", „neue Version raus", „Paket auf die Website", „zum Download bereitstellen" oder wenn die Paketdatei auf terma.vajra-team.eu fehlt oder veraltet ist.
---

# Terma veröffentlichen

Ein Release besteht aus einem Tag in `Vajra-Team/Terma`, einem Paket neben der Website und dem passenden Befehl auf der Seite. Fertig ist es erst, wenn der Befehl von der Seite in einem frischen Arch-Container funktioniert hat.

| Was | Wo |
|---|---|
| Paket bauen und hochladen | `.github/workflows/paket.yml`, läuft auf GitHub aus einem Tag |
| Seite | `../terma-website`, Repo `Vajra-Team/Terma_Website` |
| Adresse des Pakets | `https://terma.vajra-team.eu/<PACKAGE>`, `PACKAGE` steht in `index.html` |

## 1. Stand prüfen

Stoppe und melde es Jamie, wenn etwas davon nicht stimmt:

```bash
git status --short                       # Geänderte Dateien fehlen sonst im Paket. Unversionierte Dateien einzeln nennen.
git branch --show-current                # main
git fetch origin && git status -sb       # Stand wie origin/main
gh secret list -R Vajra-Team/Terma            # FTP_HOST, FTP_USER, FTP_PASSWORD
gh workflow view paket.yml -R Vajra-Team/Terma   # Kennt GitHub erst, wenn die Datei auf main liegt.
git -C ../terma-website status -sb       # Sauber und aktuell. Dort arbeitet evtl. ein anderer Agent.
grep -n 'const COMMAND' ../terma-website/index.html   # Muss erst laden, dann die Datei installieren. `pacman -U <URL>` scheitert.
```

Dann die Tests: `dev/test.sh`, `npm test`, `npx svelte-check`.

## 2. Version festlegen

Schlage die neue Version mit Begründung aus `git log <letzter Tag>..HEAD` vor (beim ersten Tag aus dem ganzen Log) und warte auf Jamies Antwort. Ändert sich nur die Verpackung, steigt allein `pkgrel`. Liegt schon ein Paket unter der Adresse, die Datei nie unter gleichem Namen ersetzen.

Die Version steht hier, alle Stellen gemeinsam ändern:

- `Cargo.toml`, `Cargo.lock` (terma-app, terma-core, terma-helper)
- `package.json`, `package-lock.json` (zwei Stellen)
- `src-tauri/tauri.conf.json`, `packaging/PKGBUILD`, `README.md`

Prüfen: `git grep -nF '<alte Version>' -- . ':!Cargo.lock' ':!package-lock.json' ':!docs' ':!crates/*/tests/fixtures'`. Fremde Pakete in den Lockfiles mit gleicher Nummer bleiben unverändert.

Hat sich `Cargo.lock` geändert, `bash dev/licenses.sh` ausführen. Bricht es wegen einer neuen Lizenz ab, entscheidet Jamie.

Änderungen benennen und einzeln mit `git add <Datei>` committen.

## 3. Veröffentlichen

Zeige Jamie Version, Commit und die Schritte a bis c und warte auf das Ja. Das Paket geht zuerst online, damit die Seite nie auf eine fehlende Datei zeigt.

**a) Tag und Paket**

```bash
git push origin main
git tag v<version> && git push origin v<version>
gh workflow run paket.yml -R Vajra-Team/Terma --ref v<version>
gh run list -R Vajra-Team/Terma --workflow=paket.yml --branch v<version> --limit 1   # Liefert es nichts, kurz warten und wiederholen.
gh run watch <id> -R Vajra-Team/Terma --exit-status
```

Der Workflow prüft am Ende selbst die sha256 des Downloads. Die Summe aus dem Log für den Bericht notieren.

**b) Seite, nur wenn sich der Dateiname geändert hat**

In `../terma-website` genau diese Stellen ändern: `PACKAGE` in `index.html` und `term.installed` in `lang/de.json` und `lang/en.json`. Committen und pushen. Dann in `.github/workflows/deploy.yml` nachsehen: Steht dort ein `push`-Auslöser, läuft der Deploy von selbst, sonst `gh workflow run deploy.yml -R Vajra-Team/Terma_Website`. Lauf mit `gh run watch --exit-status` abwarten.

**c) Probe von außen**

Lies den Befehl aus `const COMMAND` in `index.html`, setze `PACKAGE` ein und führe ihn wortgleich in einem unberührten Arch aus:

```bash
sudo docker run --pull always --rm --user root archlinux:base-devel bash -euc 'pacman -Syu --noconfirm >/dev/null; yes "" | bash -euc "<Befehl>"; pacman -Q terma; test -f /usr/share/licenses/terma/LICENSE'
curl -fsS https://terma.vajra-team.eu/ | grep -c '<PACKAGE>'
```

## 4. Bericht

Nenne Version, Commit, Tag, Lauf-Adresse, sha256 und die Ausgabe von `pacman -Q terma`. Das echte Fenster auf Jamies PC bleibt offen, bis Jamie es installiert hat.

## Grenzen

| Vorhaben | Richtig |
|---|---|
| Lokal oder auf einem CachyOS-PC bauen | Nur der Workflow. CachyOS baut mit `-march=native`, das Paket stürzt auf anderen CPUs ab. |
| Paket ins Website-Repo legen | Bleibt draußen. Der Upload kennt nur diese eine Datei. |
| `FTP-Deploy-Action` oder `dangerous-clean-slate` für das Paket | Nur `curl`. Eine fremde State-Datei kann die Seite löschen. |
| Website-Launch mit erledigen (`robots.txt`, `push`-Auslöser, Texte) | Eigener Auftrag, nur die Stellen aus 3b ändern. |
| `sudo pacman -U <URL>` | Scheitert am fehlenden `.sig`. Der Befehl auf der Seite muss erst laden und dann die Datei installieren. |
| `git add -A` | Unversionierte Dateien wie `TODO.md` kämen mit. Einzeln hinzufügen. |
