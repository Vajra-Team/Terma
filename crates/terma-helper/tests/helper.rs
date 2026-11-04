use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use terma_helper::{adding_argv, lock_present, only_adds, pacman_argvs, parse_aur, parse_manage, protected_in, rules_file, valid_pkg_name, Action};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn install_argv() {
    let action = parse_manage(&s(&["install", "discord"]), "user").unwrap();
    assert_eq!(action, Action::Install(s(&["discord"])));
    assert_eq!(pacman_argvs(&action), vec![s(&["pacman", "-Syu", "--noconfirm", "--needed", "--", "discord"])]);
}

#[test]
fn aur_deps_argv() {
    let tmp = tempfile::tempdir().unwrap();
    let action = parse_aur(&s(&["install-deps", "nodejs", "npm"]), tmp.path()).unwrap();
    // Systemupdate getrennt: --asdeps gilt sonst für jedes Paket der Transaktion, auch für alle Updates.
    assert_eq!(
        pacman_argvs(&action),
        vec![s(&["pacman", "-Syu", "--noconfirm"]), s(&["pacman", "-S", "--noconfirm", "--needed", "--asdeps", "--", "nodejs", "npm"])]
    );
}

#[test]
fn adding_argv_without_system_update() {
    let tmp = tempfile::tempdir().unwrap();
    let install = parse_manage(&s(&["install", "kmines"]), "user").unwrap();
    assert_eq!(adding_argv(&install), Some(s(&["pacman", "-S", "--noconfirm", "--needed", "--", "kmines"])));
    let deps = parse_aur(&s(&["add-deps", "nodejs"]), tmp.path()).unwrap();
    assert_eq!(deps, Action::AurAddDeps(s(&["nodejs"])));
    assert_eq!(adding_argv(&deps), Some(s(&["pacman", "-S", "--noconfirm", "--needed", "--asdeps", "--", "nodejs"])));
    assert_eq!(pacman_argvs(&deps), pacman_argvs(&Action::AurInstallDeps(s(&["nodejs"]))), "ändert es Installiertes, läuft das Systemupdate mit");
    assert_eq!(adding_argv(&Action::Upgrade), None);
    assert_eq!(adding_argv(&Action::AurInstallDeps(s(&["nodejs"]))), None, "Alle aktualisieren bleibt ein Systemupdate");
}

#[test]
fn only_adds_when_nothing_installed_changes() {
    assert!(only_adds("kmines\nlibkmahjongg\n", "fish\nmesa\n"));
    assert!(only_adds("", "fish\n"), "schon aktuell installiert: nichts zu tun");
    assert!(!only_adds("kmines\nmesa\n", "fish\nmesa\n"));
}

fn pkg_file(home: &std::path::Path, name: &str) -> PathBuf {
    let dir = home.join(".cache/terma/pkg");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(name);
    std::fs::write(&file, b"pkg").unwrap();
    file.canonicalize().unwrap()
}

#[test]
fn aur_files_argv() {
    let tmp = tempfile::tempdir().unwrap();
    let file = pkg_file(tmp.path(), "a-1-1-x86_64.pkg.tar.zst");
    let f = file.to_string_lossy().into_owned();

    let with = parse_aur(&s(&["install-files", "--upgrade", &f]), tmp.path()).unwrap();
    assert_eq!(with, Action::AurInstallFiles { files: vec![file.clone()], upgrade: true });
    assert_eq!(pacman_argvs(&with), vec![s(&["pacman", "-Syu", "--noconfirm"]), s(&["pacman", "-U", "--noconfirm", "--", &f])]);

    let without = parse_aur(&s(&["install-files", &f]), tmp.path()).unwrap();
    assert_eq!(pacman_argvs(&without), vec![s(&["pacman", "-U", "--noconfirm", "--", &f])]);
}

#[test]
fn remove_argv() {
    let action = parse_manage(&s(&["remove", "discord"]), "user").unwrap();
    assert_eq!(pacman_argvs(&action), vec![s(&["pacman", "-Rnsc", "--noconfirm", "--", "discord"])], "abhängige Pakete (z. B. Sprachpakete) gehen mit");
}

#[test]
fn upgrade_argv() {
    let action = parse_manage(&s(&["upgrade"]), "user").unwrap();
    assert_eq!(pacman_argvs(&action), vec![s(&["pacman", "-Syu", "--noconfirm"])]);
    assert!(parse_manage(&s(&["upgrade", "discord"]), "user").is_err());
}

#[test]
fn passwordless_parsing() {
    assert_eq!(parse_manage(&s(&["passwordless", "on"]), "user").unwrap(), Action::Passwordless { on: true, user: "user".into() });
    assert_eq!(parse_manage(&s(&["passwordless", "off"]), "user").unwrap(), Action::Passwordless { on: false, user: "user".into() });
    assert!(parse_manage(&s(&["passwordless", "vielleicht"]), "user").is_err());
    assert!(parse_manage(&s(&["passwordless", "on"]), "user\"; evil").is_err(), "Benutzername landet in JavaScript");
    assert!(pacman_argvs(&Action::Passwordless { on: true, user: "user".into() }).is_empty());
}

#[test]
fn rejects_bad_names() {
    for bad in ["-rf", "foo;bar", "Foo", "", "foo bar", "$(id)"] {
        assert!(!valid_pkg_name(bad), "{bad:?}");
        assert!(parse_manage(&s(&["install", bad]), "user").is_err(), "{bad:?}");
    }
    for good in ["discord", "lib32-glibc", "gtk+3", "python3.12", "@scope", "t3code-bin"] {
        assert!(valid_pkg_name(good), "{good}");
    }
    assert!(parse_manage(&s(&["install"]), "user").is_err(), "ohne Paket");
    assert!(parse_manage(&s(&["rm", "-rf", "/"]), "user").is_err());
    assert!(parse_manage(&s(&[]), "user").is_err());
}

#[test]
fn aur_files_must_be_in_users_pkg_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let inside = pkg_file(&home, "a-1-1-x86_64.pkg.tar.zst");
    let outside = tmp.path().join("x.pkg.tar.zst");
    std::fs::write(&outside, b"pkg").unwrap();
    let link = home.join(".cache/terma/pkg/link-1-1-x86_64.pkg.tar.zst");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let wrong_ext = pkg_file(&home, "a.sh");

    assert!(parse_aur(&s(&["install-files", &inside.to_string_lossy()]), &home).is_ok());
    for bad in [&outside, &link, &wrong_ext, &home.join(".cache/terma/pkg/fehlt.pkg.tar.zst")] {
        assert!(parse_aur(&s(&["install-files", &bad.to_string_lossy()]), &home).is_err(), "{}", bad.display());
    }
    assert!(parse_aur(&s(&["install-files"]), &home).is_err(), "ohne Datei");
    assert!(parse_aur(&s(&["install", "discord"]), &home).is_err(), "nur feste Befehle");
}

#[test]
fn rules_file_exact() {
    assert_eq!(
        rules_file("user"),
        "// Von Terma angelegt: „Ohne Passwort installieren“\n\
polkit.addRule(function(action, subject) {\n\
\x20   if (action.id == \"eu.vajrateam.terma.manage\" &&\n\
\x20       subject.user == \"user\" && subject.local && subject.active) {\n\
\x20       return polkit.Result.YES;\n\
\x20   }\n\
});\n"
    );
}

#[test]
fn lock_detected_exits_3() {
    let tmp = tempfile::tempdir().unwrap();
    let lock = tmp.path().join("db.lck");
    assert!(!lock_present(&lock));
    std::fs::write(&lock, "").unwrap();
    assert!(lock_present(&lock));
}

#[test]
fn remove_refuses_protected() {
    let groups: HashMap<String, Vec<String>> = HashMap::from([("dolphin".to_string(), vec!["kde-system".to_string()])]);
    assert_eq!(protected_in(&s(&["discord", "dolphin"]), &groups), Some("dolphin".to_string()));
    assert_eq!(protected_in(&s(&["cachyos-hello"]), &HashMap::new()), Some("cachyos-hello".to_string()));
    assert_eq!(protected_in(&s(&["discord"]), &groups), None);
    for wichtig in ["base", "glibc", "systemd", "pacman", "sudo", "polkit", "linux-firmware", "networkmanager", "sddm"] {
        assert_eq!(protected_in(&s(&[wichtig]), &HashMap::new()), Some(wichtig.to_string()), "{wichtig} darf nie entfernt werden");
    }
}

#[test]
fn refuses_to_run_without_root() {
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_terma-helper")).args(["install", "discord"]).output().unwrap();
    assert_eq!(out.status.code(), Some(5));
}

/// Läuft nur als Root im Arch-Container: `sudo docker run --user root … cargo test -p terma-helper -- --ignored --test-threads=1`
/// (nacheinander: dieser Test setzt die pacman-Sperre, die anderen Root-Tests würden daran scheitern)
#[test]
#[ignore = "nur als Root im Arch-Container"]
fn container_install_lock_and_remove_as_root() {
    assert_eq!(unsafe { libc::geteuid() }, 0, "muss als Root laufen");
    let helper = env!("CARGO_BIN_EXE_terma-helper");

    let lock = std::path::Path::new("/var/lib/pacman/db.lck");
    std::fs::write(lock, "").unwrap();
    let locked = Command::new(helper).args(["install", "cowsay"]).output().unwrap();
    std::fs::remove_file(lock).unwrap();
    assert_eq!(locked.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&locked.stderr).contains("DB_LOCKED"));

    assert_eq!(Command::new(helper).args(["install", "cowsay"]).status().unwrap().code(), Some(0));
    assert!(Command::new("pacman").args(["-Q", "cowsay"]).status().unwrap().success());
    assert_eq!(Command::new(helper).args(["remove", "cowsay"]).status().unwrap().code(), Some(0));
    assert!(!Command::new("pacman").args(["-Q", "cowsay"]).status().unwrap().success());
}

const OLD_COWSAY: &str = "https://archive.archlinux.org/packages/c/cowsay/cowsay-3.8.3-1-any.pkg.tar.zst";

fn pacman(args: &[&str]) -> bool {
    Command::new("pacman").args(args).status().unwrap().success()
}

fn cowsay_version() -> String {
    String::from_utf8_lossy(&Command::new("pacman").args(["-Q", "cowsay"]).output().unwrap().stdout).trim().to_string()
}

/// Jamie, 31.10.2026: „Alle aktualisieren“ lief über `install-deps` und markierte alle 714 Updates als
/// Abhängigkeit. Ein ausdrücklich installiertes Paket muss nach einem Update ausdrücklich bleiben.
/// Läuft nur als Root im Arch-Container, braucht Netz (Arch-Archiv).
#[test]
#[ignore = "nur als Root im Arch-Container"]
fn container_install_deps_keeps_explicit_packages() {
    assert_eq!(unsafe { libc::geteuid() }, 0, "muss als Root laufen");
    assert!(pacman(&["-Sy"]));
    assert!(pacman(&["-U", "--noconfirm", OLD_COWSAY]));
    assert!(pacman(&["-Qqe", "cowsay"]));

    let status = Command::new(env!("CARGO_BIN_EXE_terma-helper-aur")).args(["install-deps", "sl"]).env("PKEXEC_UID", "0").status().unwrap();

    assert_eq!(status.code(), Some(0));
    assert_ne!(cowsay_version(), "cowsay 3.8.3-1", "cowsay wurde nicht aktualisiert");
    assert!(pacman(&["-Qqe", "cowsay"]), "cowsay ist nach dem Update nicht mehr ausdrücklich installiert");
    assert!(pacman(&["-Qqd", "sl"]), "sl ist nicht als Abhängigkeit installiert");
}

/// Jamie, 02.11.2026: Nicht bei jeder Installation das ganze System aktualisieren.
/// Kommen nur neue Pakete dazu, bleibt das Installierte unberührt; sonst läuft das Systemupdate mit.
#[test]
#[ignore = "nur als Root im Arch-Container"]
fn container_install_updates_system_only_when_needed() {
    assert_eq!(unsafe { libc::geteuid() }, 0, "muss als Root laufen");
    let helper = env!("CARGO_BIN_EXE_terma-helper");
    let helper_aur = env!("CARGO_BIN_EXE_terma-helper-aur");
    assert!(pacman(&["-Sy"]));
    assert!(pacman(&["-U", "--noconfirm", OLD_COWSAY]));

    assert_eq!(Command::new(helper).args(["install", "figlet"]).status().unwrap().code(), Some(0));
    assert!(pacman(&["-Qqe", "figlet"]));
    assert_eq!(cowsay_version(), "cowsay 3.8.3-1", "neues Paket darf nichts Installiertes ändern");

    assert_eq!(Command::new(helper_aur).args(["add-deps", "cmatrix"]).env("PKEXEC_UID", "0").status().unwrap().code(), Some(0));
    assert!(pacman(&["-Qqd", "cmatrix"]));
    assert_eq!(cowsay_version(), "cowsay 3.8.3-1", "neue Abhängigkeit darf nichts Installiertes ändern");

    assert_eq!(Command::new(helper).args(["install", "cowsay"]).status().unwrap().code(), Some(0));
    assert_ne!(cowsay_version(), "cowsay 3.8.3-1", "installiertes Paket als Ziel: Systemupdate läuft mit");
}
