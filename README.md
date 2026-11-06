# Terma

Install programs on Arch and CachyOS without a terminal. Name a program or open a file, and Terma
finds the source and sets everything up: natively from the package repositories, from the AUR, or
as an AppImage. Never Flatpak, never Snap.

Website: <https://terma.vajra-team.eu>

Made for Arch-based systems with KDE Plasma. The window follows the system language, German or English.

## Install

```bash
curl -fLO https://terma.vajra-team.eu/terma-0.1.0-1-x86_64.pkg.tar.zst
sudo pacman -Syu
sudo pacman -U ./terma-0.1.0-1-x86_64.pkg.tar.zst
```

The package is not signed. `pacman -U` with a URL refuses unsigned packages, so the download comes first.
Update the system before installing: on an outdated system pacman asks the mirrors for dependency versions they
no longer have, and the install fails with 404 errors.

## How it decides

- Repositories first, then the AUR. If neither has the program, a downloaded AppImage is set up as an AppImage.
- Anyone can upload to the AUR and nobody reviews it first, so Terma checks AUR packages and their new AUR
  dependencies at install and at update. It warns about packages younger than 180 days, with fewer than 20
  votes, without a maintainer, imitating a known name, or with a changed maintainer or source. The rule is in
  `crates/terma-core/src/trust.rs`. These are warning signs, not a review: a package's build script runs as
  your user and can do anything you can.
- Installing only updates the system when it has to. If the install would change a package you already
  have, Terma does a full update instead of a partial one.

## What runs as root

Two small helpers in `crates/terma-helper`, started through polkit, never the window itself. They accept
a fixed set of actions and refuse to remove system packages such as the kernel. The AUR helper only takes
package files from Terma's build folder in your home. That folder belongs to you, so this guards against
mistakes, not against malware already running as your user. Installing from the AUR asks for your password
even with the optional "without password" setting turned on, and the answer stays valid for a few minutes,
so one update doesn't ask once per package.

Terma has no telemetry and no account. It talks to your pacman mirrors, the AUR, the GitHub releases or
zsync address of AppImages you installed and, while building an AUR package, the addresses that package
downloads from and a key server if it needs a signing key.

## Reporting security problems

Please use [private vulnerability reporting](https://github.com/Vajra-Team/Terma/security/advisories/new)
instead of a public issue.

## Build

You need `rust`, `nodejs`, `npm`, `webkit2gtk-4.1` and `base-devel`.

```bash
cd packaging && makepkg -si   # builds from this working tree, then installs
```

| Task | Command |
|---|---|
| Rust tests in an Arch container | `dev/test.sh` |
| Tests that need root, in the container | see `dev/test.sh` and the `container_` tests in `crates/terma-helper/tests` |
| Frontend tests and types | `npm test` and `npx svelte-check` |
| Window with sample data | `npx vite`, then open it in a browser |
| Package for publishing | `bash dev/package.sh` |

## License

The code is under the [MIT License](LICENSE). Bundled fonts are under the SIL Open Font License, see
`src/styles/vajra/fonts`. Third-party code in the binary is listed in
[`packaging/THIRD-PARTY-LICENSES.txt`](packaging/THIRD-PARTY-LICENSES.txt).

The name Terma, the logo and the vajra marks (`packaging/hicolor`, `src-tauri/icons`) are not covered by the
MIT License. Please use your own name and logo for a fork.
