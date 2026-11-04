mod support;

use std::sync::Arc;

use terma_core::aur::AurClient;
use terma_core::dirs::UserDirs;
use terma_core::file_flow::{detect_kind, name_from_filename, FileKind};
use terma_core::http::FakeHttp;
use terma_core::ops::Terma;
use terma_core::runner::FakeRunner;
use terma_core::search::Source;

const BASE: &str = "https://aur.archlinux.org";

fn terma(runner: FakeRunner, http: FakeHttp) -> (Terma, UserDirs, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(runner.on(&["fakeroot"], 0, "").on(&["pacman", "-Ss"], 1, "").on(&["pacman", "-Qmq"], 1, ""));
    let http = Arc::new(http);
    (Terma::with(runner, AurClient::new(BASE, http.clone()), http, dirs.clone()), dirs, tmp)
}

#[test]
fn name_from_filename_cases() {
    assert_eq!(name_from_filename("T3-Code-0.1.1-x86_64.AppImage"), "T3 Code");
    assert_eq!(name_from_filename("KeePassXC-2.7.12-x86_64.AppImage"), "KeePassXC");
    assert_eq!(name_from_filename("discord-0.0.112.deb"), "discord");
    assert_eq!(name_from_filename("Sidra.AppImage"), "Sidra");
    assert_eq!(name_from_filename("app_1.2_amd64.deb"), "app");
    assert_eq!(name_from_filename("tool-v2.0-linux-x64.tar.gz"), "tool");
    assert_eq!(name_from_filename("7zip-24.08.tar.xz"), "7zip");
}

#[test]
fn detect_kind_by_extension_and_magic() {
    let tmp = tempfile::tempdir().unwrap();
    let real = support::make_appimage(tmp.path(), "App.AppImage", "app.desktop", "[Desktop Entry]\nName=App\n", None, None, &[]);
    let fake = tmp.path().join("Text.AppImage");
    std::fs::write(&fake, "kein ELF").unwrap();

    assert_eq!(detect_kind(&real), FileKind::AppImage);
    assert_eq!(detect_kind(&fake), FileKind::Unknown);
    assert_eq!(detect_kind(&tmp.path().join("a.deb")), FileKind::Deb);
    assert_eq!(detect_kind(&tmp.path().join("a.rpm")), FileKind::Rpm);
    assert_eq!(detect_kind(&tmp.path().join("a.tar.zst")), FileKind::Tarball);
    assert_eq!(detect_kind(&tmp.path().join("a.zip")), FileKind::Unknown);
}

#[test]
fn t3_file_matches_aur_bin() {
    let http = FakeHttp::new().on(&format!("{BASE}/rpc/v5/search/t3code?by=name"), include_str!("fixtures/aur/search-t3.json"));
    let (terma, dirs, tmp) = terma(FakeRunner::new(), http);
    let desktop = "[Desktop Entry]\nName=T3 Code (Beta)\nExec=AppRun %U\nIcon=t3code\nX-AppImage-Version=0.1.1\n";
    let file = support::make_appimage(
        tmp.path(),
        "T3-Code-0.1.1-x86_64.AppImage",
        "t3code.desktop",
        desktop,
        Some(support::PNG_1PX),
        None,
        &[("resources/app-update.yml", b"provider: github\n")],
    );

    let inspection = terma.inspect_file(&file).unwrap();
    assert!(inspection.self_updating, "T3 Code bringt einen eigenen Updater mit");

    assert_eq!(inspection.kind, FileKind::AppImage);
    assert_eq!(inspection.app_name, "T3 Code (Beta)");
    assert_eq!(inspection.app_version.as_deref(), Some("0.1.1"));
    assert_eq!(inspection.source_match.unwrap().name, "t3code-bin");
    let icon = inspection.icon.unwrap();
    assert!(icon.starts_with(dirs.cache.join("preview")) && icon.is_file());
}

#[test]
fn keepassxc_file_matches_official() {
    let runner = FakeRunner::new()
        .on(&["pacman", "-Ss", "keepassxc"], 0, "extra/keepassxc 2.7.12-1\n    Cross-platform community-driven port of Keepass\n")
        .on(&["pacman", "-Si"], 0, "Name            : keepassxc\nDownload Size   : 8.00 MiB\n")
        .on(&["pacman", "-Fl"], 0, "keepassxc usr/share/applications/org.keepassxc.KeePassXC.desktop\n");
    let (terma, _dirs, tmp) = terma(runner, FakeHttp::new());
    let file = support::make_appimage(tmp.path(), "KeePassXC-2.7.12-x86_64.AppImage", "org.keepassxc.KeePassXC.desktop", "[Desktop Entry]\nName=KeePassXC\n", None, None, &[]);

    let inspection = terma.inspect_file(&file).unwrap();
    assert!(!inspection.self_updating);
    let m = inspection.source_match.unwrap();

    assert_eq!(m.name, "keepassxc");
    assert!(matches!(m.source, Source::Repo { .. }));
}

#[test]
fn deb_without_match() {
    let (terma, _dirs, tmp) = terma(FakeRunner::new(), FakeHttp::new());
    let file = tmp.path().join("app_1.2_amd64.deb");
    std::fs::write(&file, b"!<arch>").unwrap();

    let inspection = terma.inspect_file(&file).unwrap();

    assert_eq!(inspection.kind, FileKind::Deb);
    assert_eq!(inspection.app_name, "app");
    assert_eq!(inspection.source_match, None);
}
