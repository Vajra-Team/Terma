mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use terma_core::appimage;
use terma_core::desktop::DesktopEntry;
use terma_core::dirs::UserDirs;
use terma_core::integrate::{integrate_appimage, remove_appimage, retire_entries_for_file};
use terma_core::runner::FakeRunner;

const T3_DESKTOP: &str = "[Desktop Entry]\nName=T3 Code\nExec=AppRun --no-sandbox %U\nIcon=t3code\nType=Application\n";

fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    list.sort();
    list
}

fn read_entry(path: &Path) -> DesktopEntry {
    DesktopEntry::parse(&std::fs::read_to_string(path).unwrap())
}

fn t3_fixture(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    support::make_appimage(dir, "T3-Code-0.1.1-x86_64.AppImage", "t3code.desktop", T3_DESKTOP, Some(support::PNG_1PX), None, &[])
}

#[test]
fn integrate_replaces_hidden_t3_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let source = t3_fixture(&tmp.path().join("mnt"));
    std::fs::write(
        dirs.applications.join("com.t3tools.T3Code.desktop"),
        format!("[Desktop Entry]\nType=Application\nName=T3 Code (Beta)\nExec={} %U\nNoDisplay=true\nMimeType=x-scheme-handler/t3code;\n", source.display()),
    )
    .unwrap();
    let runner = FakeRunner::new().on(&["update-desktop-database"], 0, "");

    let app = integrate_appimage(&appimage::inspect(&source).unwrap(), &dirs, &runner).unwrap();

    let target = dirs.appimages.join("T3-Code.AppImage");
    assert_eq!(app.appimage, target);
    assert!(!source.exists());
    assert!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o111 != 0);

    assert_eq!(entries(&dirs.applications), vec![dirs.applications.join("t3code.desktop")]);
    assert_eq!(app.desktop_file, dirs.applications.join("t3code.desktop"));
    let entry = read_entry(&app.desktop_file);
    let target_str = target.to_string_lossy().into_owned();
    assert_eq!(entry.get("NoDisplay"), None);
    assert_eq!(entry.get("MimeType"), Some("x-scheme-handler/t3code;"));
    assert_eq!(entry.get("X-Terma-Source"), Some("appimage"));
    assert_eq!(entry.get("X-Terma-Path"), Some(target_str.as_str()));
    assert_eq!(entry.get("TryExec"), Some(target_str.as_str()));
    assert_eq!(entry.get("Exec"), Some(format!("{target_str} --no-sandbox %U").as_str()));
    let icon = dirs.data.join("icons/t3code.png");
    assert_eq!(entry.get("Icon"), Some(icon.to_string_lossy().as_ref()));
    assert_eq!(std::fs::read(&icon).unwrap(), support::PNG_1PX);
    assert_eq!(app.icon, Some(icon));

    let backups: Vec<PathBuf> = entries(&dirs.data.join("backup")).iter().flat_map(|d| entries(d)).collect();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].ends_with("com.t3tools.T3Code.desktop"));

    let applications = dirs.applications.to_string_lossy().into_owned();
    assert!(runner.calls().iter().any(|c| c.argv == ["update-desktop-database", applications.as_str()]));
}

#[test]
fn integrate_keeps_exec_args() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let source = support::make_appimage(
        tmp.path(),
        "Sidra.AppImage",
        "sidra.desktop",
        "[Desktop Entry]\nName=Sidra\nExec=AppRun --no-sandbox %U\nIcon=sidra\n",
        None,
        None,
        &[],
    );

    let app = integrate_appimage(&appimage::inspect(&source).unwrap(), &dirs, &FakeRunner::new()).unwrap();

    let entry = read_entry(&app.desktop_file);
    assert_eq!(entry.get("Exec"), Some(format!("{} --no-sandbox %U", app.appimage.display()).as_str()));
    assert_eq!(app.icon, None);
}

#[test]
fn integrate_path_with_spaces() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(&tmp.path().join("Meine Daten"));
    let source = t3_fixture(&tmp.path().join("Meine Apps"));
    let source = {
        let renamed = source.with_file_name("Grüße T3.AppImage");
        std::fs::rename(&source, &renamed).unwrap();
        renamed
    };

    let app = integrate_appimage(&appimage::inspect(&source).unwrap(), &dirs, &FakeRunner::new()).unwrap();

    let entry = read_entry(&app.desktop_file);
    assert_eq!(entry.get("Exec"), Some(format!("\"{}\" --no-sandbox %U", app.appimage.display()).as_str()));
    assert_eq!(entry.exec_program().as_deref(), Some(app.appimage.to_string_lossy().as_ref()));
}

#[test]
fn same_name_other_app_gets_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let taken = dirs.appimages.join("T3-Code.AppImage");
    std::fs::write(&taken, "anderes Programm").unwrap();
    std::fs::write(dirs.applications.join("other.desktop"), format!("[Desktop Entry]\nName=Other\nExec={}\n", taken.display())).unwrap();
    let source = t3_fixture(&tmp.path().join("mnt"));

    let app = integrate_appimage(&appimage::inspect(&source).unwrap(), &dirs, &FakeRunner::new()).unwrap();

    assert_eq!(app.appimage, dirs.appimages.join("T3-Code-2.AppImage"));
    assert_eq!(std::fs::read_to_string(&taken).unwrap(), "anderes Programm");
    assert!(dirs.applications.join("other.desktop").exists());
}

#[test]
fn retire_entries_for_file_moves_matching() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let file = tmp.path().join("T3-Code-0.1.1-x86_64.AppImage");
    std::fs::write(dirs.applications.join("a.desktop"), format!("[Desktop Entry]\nName=A\nExec={} %U\n", file.display())).unwrap();
    std::fs::write(dirs.applications.join("b.desktop"), "[Desktop Entry]\nName=B\nExec=/usr/bin/b\n").unwrap();

    let moved = retire_entries_for_file(&file, &dirs, &FakeRunner::new()).unwrap();

    assert_eq!(moved.len(), 1);
    assert!(moved[0].ends_with("a.desktop") && moved[0].exists());
    assert_eq!(entries(&dirs.applications), vec![dirs.applications.join("b.desktop")]);
}

/// Ein AppImage ohne eigenes Icon behält seinen `Icon=`-Eintrag. Mit `..` zeigt der auf beliebige Dateien.
#[test]
fn remove_appimage_keeps_files_outside_the_icon_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let wichtig = tmp.path().join("wichtig.png");
    std::fs::write(&wichtig, "x").unwrap();
    std::fs::create_dir_all(dirs.data.join("icons")).unwrap();
    std::fs::create_dir_all(&dirs.applications).unwrap();
    let desktop = dirs.applications.join("boese.desktop");
    let tiefe = dirs.data.join("icons").strip_prefix(tmp.path()).unwrap().components().count();
    let icon = format!("{}/{}wichtig.png", dirs.data.join("icons").display(), "../".repeat(tiefe));
    std::fs::write(&desktop, format!("[Desktop Entry]\nName=Boese\nExec=/nicht/da\nIcon={icon}\n")).unwrap();

    remove_appimage(&desktop, &dirs, &FakeRunner::new()).unwrap();

    assert!(wichtig.exists());
}

#[test]
fn remove_appimage_deletes_three_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let source = t3_fixture(&tmp.path().join("mnt"));
    let app = integrate_appimage(&appimage::inspect(&source).unwrap(), &dirs, &FakeRunner::new()).unwrap();

    remove_appimage(&app.desktop_file, &dirs, &FakeRunner::new()).unwrap();

    assert!(!app.appimage.exists());
    assert!(!app.desktop_file.exists());
    assert!(!app.icon.unwrap().exists());
}

#[test]
fn remove_appimage_keeps_foreign_icons_and_non_appimages() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let icon = tmp.path().join("icon.png");
    let tool = tmp.path().join("tool");
    std::fs::write(&icon, support::PNG_1PX).unwrap();
    std::fs::write(&tool, "#!/bin/sh\n").unwrap();
    let entry = dirs.applications.join("tool.desktop");
    std::fs::write(&entry, format!("[Desktop Entry]\nName=Tool\nExec={}\nIcon={}\n", tool.display(), icon.display())).unwrap();

    remove_appimage(&entry, &dirs, &FakeRunner::new()).unwrap();

    assert!(!entry.exists());
    assert!(icon.exists(), "Icons außerhalb von data/icons bleiben");
    assert!(tool.exists(), "Nur Dateien mit Endung .AppImage werden gelöscht");
}
