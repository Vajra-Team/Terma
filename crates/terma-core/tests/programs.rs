use std::collections::HashSet;
use std::path::{Path, PathBuf};

use terma_core::desktop::DesktopEntry;
use terma_core::dirs::UserDirs;
use terma_core::programs::{assemble, is_protected, parse_groups, parse_owners, resolve_icon, appstream_icon, Program, ProgramInputs, UpdateState};
use terma_core::search::Source;

const QO: &str = include_str!("fixtures/programs/Qo.txt");
const QG: &str = include_str!("fixtures/programs/Qg.txt");

fn system(file: &str, text: &str) -> (PathBuf, DesktopEntry) {
    (PathBuf::from("/usr/share/applications").join(file), DesktopEntry::parse(text))
}

/// Ausschnitt aus einem echten PC: Besitzer und Gruppen echt, ausdrücklich installiert wie dort.
fn inputs() -> ProgramInputs {
    ProgramInputs {
        system_entries: vec![
            system("discord.desktop", "[Desktop Entry]\nName=Discord\nExec=/usr/bin/discord\nIcon=discord\nType=Application\n"),
            system("org.kde.dolphin.desktop", "[Desktop Entry]\nName=Dolphin\nExec=dolphin %u\nType=Application\n"),
            system("cachyos-hello.desktop", "[Desktop Entry]\nName=CachyOS Hello\nExec=cachyos-hello\nType=Application\n"),
            system("steam.desktop", "[Desktop Entry]\nName=Steam\nName[de]=Steam (Laufzeit)\nExec=steam %U\nType=Application\n"),
        ],
        user_entries: vec![
            (
                PathBuf::from("/home/user/.local/share/applications/sidra.desktop"),
                DesktopEntry::parse("[Desktop Entry]\nName=Sidra\nExec=/home/user/Applications/Sidra.AppImage --no-sandbox %U\nIcon=sidra\n"),
            ),
            (
                PathBuf::from("/home/user/.local/share/applications/com.t3tools.T3Code.desktop"),
                DesktopEntry::parse(include_str!("fixtures/desktop/t3-hidden.desktop")),
            ),
        ],
        owners: parse_owners(QO),
        explicit: ["discord", "dolphin", "cachyos-hello", "steam"].iter().map(|s| s.to_string()).collect(),
        foreign: HashSet::new(),
        groups: parse_groups(QG),
        pending: HashSet::new(),
    }
}

fn names(programs: &[Program]) -> Vec<&str> {
    programs.iter().map(|p| p.name.as_str()).collect()
}

fn no_icons(_: &str) -> Option<PathBuf> {
    None
}

#[test]
fn parse_owners_and_groups_from_real_output() {
    let owners = parse_owners(QO);
    assert_eq!(owners.get(Path::new("/usr/share/applications/discord.desktop")).map(String::as_str), Some("discord"));
    assert!(parse_groups(QG)["dolphin"].contains(&"kde-system".to_string()));
}

#[test]
fn discord_listed_dolphin_not() {
    let programs = assemble(inputs(), &no_icons);

    assert_eq!(names(&programs), vec!["Discord", "Sidra", "Steam (Laufzeit)"]);
    let discord = &programs[0];
    assert!(matches!(discord.source, Source::Repo { .. }));
    assert_eq!(discord.package.as_deref(), Some("discord"));
    assert_eq!(discord.desktop_file, Path::new("/usr/share/applications/discord.desktop"));
    assert_eq!(discord.update, UpdateState::None);
}

#[test]
fn foreign_is_aur() {
    let mut i = inputs();
    i.foreign.insert("steam".into());
    let programs = assemble(i, &no_icons);
    assert_eq!(programs.iter().find(|p| p.package.as_deref() == Some("steam")).unwrap().source, Source::Aur);
}

#[test]
fn hidden_entries_skipped() {
    let mut i = inputs();
    i.system_entries.push(system("discord-hidden.desktop", "[Desktop Entry]\nName=Versteckt\nExec=x\nHidden=true\n"));
    i.owners.insert(PathBuf::from("/usr/share/applications/discord-hidden.desktop"), "steam".into());
    let programs = assemble(i, &no_icons);
    assert!(!names(&programs).contains(&"Versteckt"));
    assert!(!names(&programs).iter().any(|n| n.starts_with("T3")));
}

#[test]
fn user_appimage_entries_listed() {
    let programs = assemble(inputs(), &|name: &str| Some(PathBuf::from(format!("/icons/{name}.png"))));
    let sidra = programs.iter().find(|p| p.name == "Sidra").unwrap();
    assert_eq!(sidra.source, Source::AppImage);
    assert_eq!(sidra.package, None);
    assert_eq!(sidra.icon.as_deref(), Some(Path::new("/icons/sidra.png")));
}

#[test]
fn one_program_per_package() {
    let mut i = inputs();
    i.system_entries.push(system("discord-ptb.desktop", "[Desktop Entry]\nName=Discord Zweitfenster\nExec=discord\n"));
    i.owners.insert(PathBuf::from("/usr/share/applications/discord-ptb.desktop"), "discord".into());
    let programs = assemble(i, &no_icons);
    assert_eq!(programs.iter().filter(|p| p.package.as_deref() == Some("discord")).count(), 1);
}

#[test]
fn pending_marks_ready() {
    let mut i = inputs();
    i.pending.insert("discord".into());
    assert_eq!(assemble(i, &no_icons)[0].update, UpdateState::Ready);
}

#[test]
fn protected_rules() {
    assert!(is_protected("dolphin", &["kde-system".to_string()]));
    assert!(is_protected("cachyos-settings", &[]));
    assert!(is_protected("terma", &[]));
    assert!(is_protected("cachy-update", &[]), "CachyOS-eigene Werkzeuge gehören zum System");
    assert!(is_protected("limine-snapper-sync", &[]), "Bootloader-Werkzeuge gehören zum System");
    assert!(!is_protected("discord", &[]));
}

#[test]
fn resolve_icon_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let user_icon = dirs.home.join(".local/share/icons/hicolor/256x256/apps/sidra.png");
    std::fs::create_dir_all(user_icon.parent().unwrap()).unwrap();
    std::fs::write(&user_icon, b"png").unwrap();

    assert_eq!(resolve_icon("sidra", &dirs), Some(user_icon.clone()));
    assert_eq!(resolve_icon(&user_icon.to_string_lossy(), &dirs), Some(user_icon));
    assert_eq!(resolve_icon("gibt-es-sicher-nicht-terma", &dirs), None);
}

#[test]
fn appstream_icon_prefers_own_name_and_large_size() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for file in [
        "archlinux-arch-extra/64x64/firefox_firefox.jxl",
        "archlinux-arch-extra/128x128/firefox_firefox.jxl",
        "archlinux-arch-extra/128x128/firefox-developer-edition_firefox-developer-edition.jxl",
        "archlinux-arch-extra/128x128/kdegames_org.kde.kpat.jxl",
        "archlinux-arch-extra/128x128/kdegames_kdegames.jxl",
        "archlinux-arch-extra/64x64/gimp_gimp.jxl",
    ] {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"jxl").unwrap();
    }

    assert_eq!(appstream_icon("firefox", root), Some(root.join("archlinux-arch-extra/128x128/firefox_firefox.jxl")));
    assert_eq!(appstream_icon("gimp", root), Some(root.join("archlinux-arch-extra/64x64/gimp_gimp.jxl")));
    assert_eq!(appstream_icon("kdegames", root), Some(root.join("archlinux-arch-extra/128x128/kdegames_kdegames.jxl")));
    assert_eq!(appstream_icon("firefox-developer", root), None);
    assert_eq!(appstream_icon("t3code-bin", root), None);
    assert_eq!(appstream_icon("firefox", &root.join("fehlt")), None);
}

#[test]
fn system_tools_and_settings_hidden() {
    let mut i = inputs();
    for (file, pkg, categories) in [
        ("org.kde.konsole.desktop", "konsole", "Qt;KDE;System;TerminalEmulator;"),
        ("nvidia-settings.desktop", "nvidia-settings", "Settings;HardwareSettings;"),
        ("vim.desktop", "vim", "Utility;TextEditor;"),
    ] {
        i.system_entries.push(system(file, &format!("[Desktop Entry]\nName={pkg}\nExec={pkg}\nCategories={categories}\n")));
        i.owners.insert(PathBuf::from("/usr/share/applications").join(file), pkg.into());
        i.explicit.insert(pkg.into());
    }

    let programs = assemble(i, &no_icons);

    assert!(names(&programs).contains(&"vim"));
    assert!(!names(&programs).contains(&"konsole"));
    assert!(!names(&programs).contains(&"nvidia-settings"));
}

#[test]
fn resolve_icon_breeze_theme() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let breeze = dirs.home.join(".local/share/icons/breeze/apps/48/accessories-calculator.svg");
    std::fs::create_dir_all(breeze.parent().unwrap()).unwrap();
    std::fs::write(&breeze, b"<svg/>").unwrap();

    assert_eq!(resolve_icon("accessories-calculator", &dirs), Some(breeze));
}
