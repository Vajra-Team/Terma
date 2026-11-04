//! Root-Helfer für Terma. Läuft über `pkexec` und nimmt nur feste Befehle an:
//! `terma-helper` (Aktion eu.vajrateam.terma.manage) installiert aus den offiziellen Quellen,
//! entfernt, aktualisiert und schaltet „Ohne Passwort installieren“.
//! `terma-helper-aur` (Aktion eu.vajrateam.terma.aur) installiert Abhängigkeiten und von Terma gebaute Pakete.
//! Installieren aktualisiert das System nur mit, wenn das neue Paket sonst Installiertes ändern würde.
//!
//! Exit-Codes: 0 ok · 3 Paketdatenbank gesperrt · 4 geschütztes Paket · 5 ungültiger Aufruf · sonst der von pacman.

use std::collections::HashMap;
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use terma_core::programs::{is_protected, parse_groups};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Install(Vec<String>),
    Remove(Vec<String>),
    Upgrade,
    Passwordless { on: bool, user: String },
    AurInstallDeps(Vec<String>),
    AurAddDeps(Vec<String>),
    AurInstallFiles { files: Vec<PathBuf>, upgrade: bool },
}

pub const EXIT_DB_LOCKED: i32 = 3;
pub const EXIT_PROTECTED: i32 = 4;
pub const EXIT_INVALID: i32 = 5;

const DB_LOCK: &str = "/var/lib/pacman/db.lck";
const RULES_FILE: &str = "/etc/polkit-1/rules.d/49-terma.rules";

/// Paketnamen wie bei Arch: klein, kein führender Bindestrich, keine Shell-Zeichen.
pub fn valid_pkg_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else { return false };
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '@' | '_' | '+');
    allowed(first) && chars.all(|c| allowed(c) || matches!(c, '.' | '-'))
}

fn valid_user_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-'))
}

fn packages(args: &[String]) -> Result<Vec<String>, String> {
    if args.is_empty() {
        return Err("kein Paket angegeben".into());
    }
    match args.iter().find(|a| !valid_pkg_name(a)) {
        Some(bad) => Err(format!("ungültiger Paketname: {bad}")),
        None => Ok(args.to_vec()),
    }
}

pub fn parse_manage(args: &[String], user: &str) -> Result<Action, String> {
    match args.split_first() {
        Some((cmd, rest)) if cmd == "install" => Ok(Action::Install(packages(rest)?)),
        Some((cmd, rest)) if cmd == "remove" => Ok(Action::Remove(packages(rest)?)),
        Some((cmd, rest)) if cmd == "upgrade" && rest.is_empty() => Ok(Action::Upgrade),
        Some((cmd, [state])) if cmd == "passwordless" && (state == "on" || state == "off") => {
            if !valid_user_name(user) {
                return Err(format!("ungültiger Benutzername: {user}"));
            }
            Ok(Action::Passwordless { on: state == "on", user: user.to_string() })
        }
        _ => Err("unbekannter Befehl".into()),
    }
}

/// Paketdateien müssen im Paket-Cache des Aufrufers liegen (`~/.cache/terma/pkg`), auch nach dem Auflösen von Symlinks.
pub fn parse_aur(args: &[String], user_home: &Path) -> Result<Action, String> {
    match args.split_first() {
        Some((cmd, rest)) if cmd == "install-deps" => Ok(Action::AurInstallDeps(packages(rest)?)),
        Some((cmd, rest)) if cmd == "add-deps" => Ok(Action::AurAddDeps(packages(rest)?)),
        Some((cmd, rest)) if cmd == "install-files" => {
            let (upgrade, files) = match rest.split_first() {
                Some((flag, files)) if flag == "--upgrade" => (true, files),
                _ => (false, rest),
            };
            if files.is_empty() {
                return Err("keine Paketdatei angegeben".into());
            }
            let cache = user_home.join(".cache/terma/pkg").canonicalize().map_err(|_| "kein Paket-Cache".to_string())?;
            let files = files
                .iter()
                .map(|f| {
                    let path = Path::new(f).canonicalize().map_err(|_| format!("Datei fehlt: {f}"))?;
                    let ok = path.starts_with(&cache) && path.is_file() && path.to_string_lossy().ends_with(".pkg.tar.zst");
                    if ok {
                        Ok(path)
                    } else {
                        Err(format!("Datei nicht erlaubt: {f}"))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Action::AurInstallFiles { files, upgrade })
        }
        _ => Err("unbekannter Befehl".into()),
    }
}

fn argv(parts: &[&str], rest: impl IntoIterator<Item = String>) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).chain(rest).collect()
}

pub fn pacman_argvs(a: &Action) -> Vec<Vec<String>> {
    match a {
        Action::Install(p) => vec![argv(&["pacman", "-Syu", "--noconfirm", "--needed", "--"], p.clone())],
        // -c: Pakete, die nur am Programm hängen (z. B. firefox-i18n-de), gehen mit. Der Probelauf vorher
        // prüft genau diese Liste auf geschützte Pakete.
        Action::Remove(p) => vec![argv(&["pacman", "-Rnsc", "--noconfirm", "--"], p.clone())],
        Action::Upgrade => vec![argv(&["pacman", "-Syu", "--noconfirm"], [])],
        Action::Passwordless { .. } => vec![],
        // Zwei Schritte: --asdeps gilt für jedes Paket der Transaktion. Zusammen mit -u würde jedes
        // aktualisierte Programm als Abhängigkeit markiert (31.10.2026: 714 Pakete auf einem echten PC).
        Action::AurInstallDeps(p) | Action::AurAddDeps(p) => vec![
            argv(&["pacman", "-Syu", "--noconfirm"], []),
            argv(&["pacman", "-S", "--noconfirm", "--needed", "--asdeps", "--"], p.clone()),
        ],
        Action::AurInstallFiles { files, upgrade } => {
            let install = argv(&["pacman", "-U", "--noconfirm", "--"], files.iter().map(|f| f.to_string_lossy().into_owned()));
            if *upgrade {
                vec![argv(&["pacman", "-Syu", "--noconfirm"], []), install]
            } else {
                vec![install]
            }
        }
    }
}

/// Der Befehl ohne Systemupdate, falls die Aktion das zulässt (siehe `only_adds`).
pub fn adding_argv(a: &Action) -> Option<Vec<String>> {
    match a {
        Action::Install(p) => Some(argv(&["pacman", "-S", "--noconfirm", "--needed", "--"], p.clone())),
        Action::AurAddDeps(p) => Some(argv(&["pacman", "-S", "--noconfirm", "--needed", "--asdeps", "--"], p.clone())),
        _ => None,
    }
}

/// Teilupdates sind unter Arch gefährlich, neue Pakete allein nicht: Bleibt alles Installierte, wie es ist,
/// geht es ohne Systemupdate. `to_install` und `installed` sind Paketnamen, einer pro Zeile.
pub fn only_adds(to_install: &str, installed: &str) -> bool {
    let installed: Vec<&str> = installed.lines().collect();
    !to_install.lines().any(|p| installed.contains(&p))
}

pub fn rules_file(user: &str) -> String {
    format!(
        "// Von Terma angelegt: „Ohne Passwort installieren“\n\
polkit.addRule(function(action, subject) {{\n\
\x20   if (action.id == \"eu.vajrateam.terma.manage\" &&\n\
\x20       subject.user == \"{user}\" && subject.local && subject.active) {{\n\
\x20       return polkit.Result.YES;\n\
\x20   }}\n\
}});\n"
    )
}

pub fn lock_present(lock: &Path) -> bool {
    lock.exists()
}

/// Das erste Paket, das zum System gehört und deshalb nicht entfernt werden darf.
pub fn protected_in(to_remove: &[String], groups: &HashMap<String, Vec<String>>) -> Option<String> {
    to_remove.iter().find(|p| is_protected(p, groups.get(*p).map(Vec::as_slice).unwrap_or(&[]))).cloned()
}

/// Name und Home des Benutzers, der `pkexec` aufgerufen hat.
fn caller() -> Option<(String, PathBuf)> {
    let uid: libc::uid_t = std::env::var("PKEXEC_UID").ok()?.parse().ok()?;
    // SAFETY: getpwuid liefert einen Zeiger auf statischen Speicher; der Helfer ist einthreadig.
    let pw = unsafe { libc::getpwuid(uid) };
    if pw.is_null() {
        return None;
    }
    let (name, dir) = unsafe { (CStr::from_ptr((*pw).pw_name), CStr::from_ptr((*pw).pw_dir)) };
    Some((name.to_string_lossy().into_owned(), PathBuf::from(dir.to_string_lossy().into_owned())))
}

fn fail(code: i32, message: &str) -> i32 {
    eprintln!("{message}");
    code
}

fn pacman_output(args: &[&str]) -> Option<String> {
    let out = Command::new("pacman").args(args).env("LC_ALL", "C").stderr(Stdio::inherit()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Kernel erkennt man an ihrem Abbild. `/usr/lib/modules` allein besitzen auch Modul-Pakete wie die von VirtualBox.
fn kernel_packages() -> Vec<String> {
    let Ok(dirs) = std::fs::read_dir("/usr/lib/modules") else { return Vec::new() };
    dirs.flatten()
        .filter_map(|d| pacman_output(&["-Qqo", "--", &d.path().join("vmlinuz").to_string_lossy()]))
        .map(|name| name.trim().to_string())
        .collect()
}

fn execute(action: &Action) -> i32 {
    // Ausschalten ändert nur eine Datei und soll nicht daran scheitern, dass gerade ein Update läuft.
    if !matches!(action, Action::Passwordless { .. }) && lock_present(Path::new(DB_LOCK)) {
        return fail(EXIT_DB_LOCKED, "DB_LOCKED");
    }
    if let Action::Remove(pkgs) = action {
        // Probelauf ohne `-n`: pacman erlaubt `--nosave` nicht zusammen mit `--print`.
        let mut args = vec!["-Rsc", "--print", "--print-format", "%n", "--"];
        args.extend(pkgs.iter().map(String::as_str));
        let Some(removed) = pacman_output(&args) else { return fail(1, "pacman konnte die Entfernung nicht planen") };
        let removed: Vec<String> = removed.split_whitespace().map(str::to_string).collect();
        let Some(groups) = pacman_output(&["-Qg"]) else { return fail(1, "pacman konnte die Gruppen nicht lesen") };
        if let Some(pkg) = protected_in(&removed, &parse_groups(&groups)) {
            return fail(EXIT_PROTECTED, &format!("PROTECTED {pkg}"));
        }
        if let Some(pkg) = removed.iter().find(|p| kernel_packages().contains(*p)) {
            return fail(EXIT_PROTECTED, &format!("PROTECTED {pkg}"));
        }
    }
    if let Action::Passwordless { on, user } = action {
        let result = if *on {
            std::fs::write(RULES_FILE, rules_file(user))
                .and_then(|_| std::fs::set_permissions(RULES_FILE, std::os::unix::fs::PermissionsExt::from_mode(0o644)))
        } else {
            match std::fs::remove_file(RULES_FILE) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        };
        return match result {
            Ok(()) => 0,
            Err(e) => fail(1, &format!("Polkit-Regel: {e}")),
        };
    }
    let mut argvs = pacman_argvs(action);
    if let (Action::Install(pkgs) | Action::AurAddDeps(pkgs), Some(adding)) = (action, adding_argv(action)) {
        // Frische Listen: Veraltete zeigen auf Dateien, die die Mirrors nicht mehr haben. Geprüft wird nach
        // dem Laden; scheitert die Prüfung, bleibt es beim Systemupdate, und pacman nennt den Fehler.
        match run_pacman(&argv(&["pacman", "-Sy", "--noconfirm"], [])) {
            0 => {}
            code => return code,
        }
        let mut args = vec!["-Sp", "--needed", "--print-format", "%n", "--"];
        args.extend(pkgs.iter().map(String::as_str));
        if let (Some(to_install), Some(installed)) = (pacman_output(&args), pacman_output(&["-Qq"])) {
            if only_adds(&to_install, &installed) {
                argvs = vec![adding];
            }
        }
    }
    for argv in argvs {
        match run_pacman(&argv) {
            0 => {}
            code => return code,
        }
    }
    0
}

fn run_pacman(argv: &[String]) -> i32 {
    let status = Command::new(&argv[0]).args(&argv[1..]).env("LC_ALL", "C").stdin(Stdio::null()).status();
    match status.map(|s| s.code().unwrap_or(1)) {
        Ok(code) => code,
        Err(e) => fail(1, &format!("pacman: {e}")),
    }
}

fn is_root() -> bool {
    // SAFETY: geteuid hat keine Vorbedingungen.
    unsafe { libc::geteuid() == 0 }
}

pub fn run_manage(args: Vec<String>) -> i32 {
    if !is_root() {
        return fail(EXIT_INVALID, "terma-helper muss über pkexec laufen");
    }
    let user = caller().map(|(name, _)| name).unwrap_or_default();
    match parse_manage(&args, &user) {
        Ok(action) => execute(&action),
        Err(e) => fail(EXIT_INVALID, &e),
    }
}

pub fn run_aur(args: Vec<String>) -> i32 {
    if !is_root() {
        return fail(EXIT_INVALID, "terma-helper-aur muss über pkexec laufen");
    }
    let Some((_, home)) = caller() else { return fail(EXIT_INVALID, "Aufrufer unbekannt (PKEXEC_UID fehlt)") };
    match parse_aur(&args, &home) {
        Ok(action) => execute(&action),
        Err(e) => fail(EXIT_INVALID, &e),
    }
}
