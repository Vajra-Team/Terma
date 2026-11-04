mod support;

use terma_core::appimage;
use terma_core::error::TermaError;

const T3_DESKTOP: &str = "[Desktop Entry]\nName=T3 Code\nExec=AppRun --no-sandbox %U\nIcon=t3code\nType=Application\nX-AppImage-Version=0.1.1\n";
const UPD: &str = "gh-releases-zsync|pingdotgg|t3code|latest|T3-Code-*-x86_64.AppImage.zsync";

#[test]
fn inspect_reads_name_icon_updinfo() {
    let tmp = tempfile::tempdir().unwrap();
    let path = support::make_appimage(tmp.path(), "T3-Code-0.1.1-x86_64.AppImage", "t3code.desktop", T3_DESKTOP, Some(support::PNG_1PX), Some(UPD), &[]);

    let info = appimage::inspect(&path).unwrap();

    assert_eq!(info.path, path);
    assert_eq!(info.name, "T3 Code");
    assert_eq!(info.desktop_file_name, "t3code.desktop");
    assert_eq!(info.desktop.get("Exec"), Some("AppRun --no-sandbox %U"));
    assert_eq!(info.version.as_deref(), Some("0.1.1"));
    assert_eq!(info.update_info.as_deref(), Some(UPD));
    let icon = info.icon.expect("Icon gefunden");
    assert_eq!(icon.ext, "png");
    assert_eq!(icon.bytes, support::PNG_1PX);
    assert!(!info.self_updating);
}

#[test]
fn inspect_detects_self_updating() {
    let tmp = tempfile::tempdir().unwrap();
    let path = support::make_appimage(
        tmp.path(),
        "App.AppImage",
        "app.desktop",
        "[Desktop Entry]\nName=App\nExec=AppRun\n",
        None,
        None,
        &[("resources/app-update.yml", b"provider: github\n")],
    );

    let info = appimage::inspect(&path).unwrap();

    assert!(info.self_updating);
    assert_eq!(info.update_info, None);
    assert!(info.icon.is_none());
}

#[test]
fn inspect_rejects_non_appimage() {
    let tmp = tempfile::tempdir().unwrap();
    let text = tmp.path().join("notiz.AppImage");
    std::fs::write(&text, "nur Text").unwrap();
    let elf_only = tmp.path().join("leer.AppImage");
    std::fs::write(&elf_only, support::make_elf(None)).unwrap();

    assert!(matches!(appimage::inspect(&text), Err(TermaError::NotAnAppImage)));
    assert!(matches!(appimage::inspect(&elf_only), Err(TermaError::NotAnAppImage)));
}

#[test]
fn inspect_follows_symlinked_desktop_and_icon() {
    let tmp = tempfile::tempdir().unwrap();
    let path = support::make_linuxdeploy_appimage(tmp.path(), "Example-1.0-x86_64.AppImage");

    let info = appimage::inspect(&path).unwrap();

    assert_eq!(info.name, "Example");
    assert_eq!(info.desktop_file_name, "org.example.App.desktop");
    assert_eq!(info.icon.expect("Icon über zwei Symlinks").ext, "svg");
}
