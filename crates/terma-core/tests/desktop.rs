use std::path::Path;

use terma_core::desktop::DesktopEntry;

const SIDRA: &str = include_str!("fixtures/desktop/sidra.desktop");
const T3_HIDDEN: &str = include_str!("fixtures/desktop/t3-hidden.desktop");

#[test]
fn roundtrip_keeps_actions() {
    assert_eq!(DesktopEntry::parse(SIDRA).to_string(), SIDRA);
}

#[test]
fn exec_program_and_args() {
    let mut entry = DesktopEntry::parse(SIDRA);
    assert_eq!(entry.exec_program().as_deref(), Some("/home/user/Applications/Sidra.AppImage"));

    entry.set_exec_program(Path::new("/x/Sidra.AppImage"));

    assert_eq!(entry.get("Exec"), Some("/x/Sidra.AppImage --no-sandbox %U"));
    assert!(entry.to_string().contains("[Desktop Action PlayPause]\nName=Play-Pause\nName[de]=Abspielen-Pausieren\nExec=dbus-send"));
}

#[test]
fn exec_quotes_special_paths() {
    let mut entry = DesktopEntry::parse(SIDRA);
    let path = "/mnt/daten/Meine Apps/Grüße \"x\".AppImage";

    entry.set_exec_program(Path::new(path));

    assert_eq!(entry.get("Exec"), Some(r#""/mnt/daten/Meine Apps/Grüße \\"x\\".AppImage" --no-sandbox %U"#));
    assert_eq!(entry.exec_program().as_deref(), Some(path));
}

#[test]
fn set_remove_and_mime_types() {
    let mut entry = DesktopEntry::parse(T3_HIDDEN);
    assert_eq!(entry.get("NoDisplay"), Some("true"));
    assert_eq!(entry.mime_types(), vec!["x-scheme-handler/t3code".to_string()]);

    entry.remove("NoDisplay");
    entry.set("TryExec", "/a/T3.AppImage");
    entry.set("Name", "T3 Code");

    let text = entry.to_string();
    assert!(!text.contains("NoDisplay"));
    assert_eq!(entry.get("Name"), Some("T3 Code"));
    assert!(text.ends_with("TryExec=/a/T3.AppImage\n"));

    let mut sidra = DesktopEntry::parse(SIDRA);
    sidra.set("X-Terma-Source", "appimage");
    let text = sidra.to_string();
    let added = text.find("X-Terma-Source=appimage").unwrap();
    assert!(added < text.find("[Desktop Action PlayPause]").unwrap(), "neuer Schlüssel gehört in [Desktop Entry]");
}
