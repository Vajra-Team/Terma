mod support;

use std::path::Path;

use terma_core::appimage;
use terma_core::appimage_update::{self, RemoteAppImage, UpdateSource};
use terma_core::dirs::UserDirs;
use terma_core::error::TermaError;
use terma_core::http::FakeHttp;
use terma_core::integrate::integrate_appimage;
use terma_core::runner::FakeRunner;

const RELEASE: &str = include_str!("fixtures/update/gh-release.json");
const ZSYNC: &str = include_str!("fixtures/update/app.zsync-header.txt");
const KP_INFO: &str = "gh-releases-zsync|keepassxreboot|keepassxc|latest|KeePassXC-*-x86_64.AppImage.zsync";
const APP_INFO: &str = "zsync|https://x.org/a/App.AppImage.zsync";

fn app_fixture(dir: &Path, version: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let desktop = format!("[Desktop Entry]\nName=App\nExec=AppRun %U\nX-AppImage-Version={version}\n");
    support::make_appimage(dir, "App.AppImage", "app.desktop", &desktop, None, Some(APP_INFO), &[])
}

fn zsync_with_sha(sha1: &str) -> String {
    format!("zsync: 0.6.2\nFilename: App.AppImage\nURL: App.AppImage\nSHA-1: {sha1}\n\n\u{1}\u{2}binär")
}

#[test]
fn parses_gh_releases_zsync() {
    assert_eq!(
        appimage_update::parse_update_info(KP_INFO),
        Some(UpdateSource::GhReleasesZsync {
            owner: "keepassxreboot".into(),
            repo: "keepassxc".into(),
            tag: "latest".into(),
            pattern: "KeePassXC-*-x86_64.AppImage.zsync".into(),
        })
    );
}

#[test]
fn parses_plain_zsync() {
    assert_eq!(appimage_update::parse_update_info(APP_INFO), Some(UpdateSource::Zsync { url: "https://x.org/a/App.AppImage.zsync".into() }));
}

#[test]
fn rejects_garbage() {
    for s in ["", "foo", "gh-releases-zsync|a|b", "zsync|", "bintray-zsync|a|b|c|d"] {
        assert_eq!(appimage_update::parse_update_info(s), None, "{s}");
    }
}

#[test]
fn gh_asset_matching() {
    assert_eq!(
        appimage_update::release_api_url("keepassxreboot", "keepassxc", "latest"),
        "https://api.github.com/repos/keepassxreboot/keepassxc/releases/latest"
    );
    assert_eq!(appimage_update::release_api_url("a", "b", "2.7.0"), "https://api.github.com/repos/a/b/releases/tags/2.7.0");
    // Zwei Assets passen (2.8.0 und der Neubau 2.8.0-1): das zuletzt aktualisierte gewinnt.
    assert_eq!(
        appimage_update::pick_asset(RELEASE, "KeePassXC-*-x86_64.AppImage.zsync").unwrap().as_deref(),
        Some("https://github.com/keepassxreboot/keepassxc/releases/download/2.8.0/KeePassXC-2.8.0-1-x86_64.AppImage.zsync")
    );
    assert_eq!(appimage_update::pick_asset(RELEASE, "Nichts-*.zsync").unwrap(), None);
}

#[test]
fn zsync_header_relative_url() {
    let header = appimage_update::parse_zsync_header(
        ZSYNC,
        "https://github.com/keepassxreboot/keepassxc/releases/download/2.8.0/KeePassXC-2.8.0-x86_64.AppImage.zsync",
    )
    .unwrap();
    assert_eq!(header.filename, "KeePassXC-2.8.0-x86_64.AppImage");
    assert_eq!(header.url, "https://github.com/keepassxreboot/keepassxc/releases/download/2.8.0/KeePassXC-2.8.0-x86_64.AppImage");
    assert_eq!(header.sha1, "e7e0d7e489d0fdcd599caf2b9f3b105193cb1683");

    let short = appimage_update::parse_zsync_header("URL: T3.AppImage\nSHA-1: ab\n\n", "https://x.org/a/T3.AppImage.zsync").unwrap();
    assert_eq!(short.url, "https://x.org/a/T3.AppImage");
    let absolute = appimage_update::parse_zsync_header("URL: https://cdn.org/T3.AppImage\nSHA-1: ab\n\n", "https://x.org/a/T3.AppImage.zsync").unwrap();
    assert_eq!(absolute.url, "https://cdn.org/T3.AppImage");
}

#[test]
fn same_sha1_means_no_update() {
    let tmp = tempfile::tempdir().unwrap();
    let path = app_fixture(tmp.path(), "1.0");
    let sha = appimage_update::sha1_file(&path).unwrap();
    let http = FakeHttp::new().on("https://x.org/a/App.AppImage.zsync", zsync_with_sha(&sha));

    assert_eq!(appimage_update::check(&appimage::inspect(&path).unwrap(), &http).unwrap(), None);
}

#[test]
fn different_sha1_means_update() {
    let tmp = tempfile::tempdir().unwrap();
    let path = app_fixture(tmp.path(), "1.0");
    let http = FakeHttp::new().on("https://x.org/a/App.AppImage.zsync", zsync_with_sha("0123abcd"));

    assert_eq!(
        appimage_update::check(&appimage::inspect(&path).unwrap(), &http).unwrap(),
        Some(RemoteAppImage { url: "https://x.org/a/App.AppImage".into(), sha1: "0123abcd".into() })
    );
}

#[test]
fn gh_release_check_follows_api() {
    let tmp = tempfile::tempdir().unwrap();
    let desktop = "[Desktop Entry]\nName=KeePassXC\nExec=keepassxc %f\n";
    let path = support::make_appimage(tmp.path(), "KeePassXC.AppImage", "org.keepassxc.KeePassXC.desktop", desktop, None, Some(KP_INFO), &[]);
    let zsync_url = "https://github.com/keepassxreboot/keepassxc/releases/download/2.8.0/KeePassXC-2.8.0-1-x86_64.AppImage.zsync";
    let http = FakeHttp::new()
        .on("https://api.github.com/repos/keepassxreboot/keepassxc/releases/latest", RELEASE)
        .on(zsync_url, ZSYNC);

    let remote = appimage_update::check(&appimage::inspect(&path).unwrap(), &http).unwrap().unwrap();

    assert_eq!(remote.sha1, "e7e0d7e489d0fdcd599caf2b9f3b105193cb1683");
    assert_eq!(remote.url, "https://github.com/keepassxreboot/keepassxc/releases/download/2.8.0/KeePassXC-2.8.0-x86_64.AppImage");
}

#[test]
fn apply_replaces_in_place() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let installed = integrate_appimage(&appimage::inspect(&app_fixture(&tmp.path().join("v1"), "1.0")).unwrap(), &dirs, &FakeRunner::new()).unwrap();
    let v2 = app_fixture(&tmp.path().join("v2"), "2.0");
    let remote = RemoteAppImage { url: "https://x.org/a/App.AppImage".into(), sha1: appimage_update::sha1_file(&v2).unwrap() };
    let http = FakeHttp::new().on("https://x.org/a/App.AppImage", std::fs::read(&v2).unwrap());

    let updated = appimage_update::apply(&remote, &dirs, &http, &FakeRunner::new()).unwrap();

    assert_eq!(updated.appimage, installed.appimage);
    assert_eq!(appimage::inspect(&updated.appimage).unwrap().version.as_deref(), Some("2.0"));
    assert_eq!(std::fs::read_dir(&dirs.applications).unwrap().count(), 1);
}

#[test]
fn apply_rejects_wrong_checksum() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let installed = integrate_appimage(&appimage::inspect(&app_fixture(&tmp.path().join("v1"), "1.0")).unwrap(), &dirs, &FakeRunner::new()).unwrap();
    let v2 = app_fixture(&tmp.path().join("v2"), "2.0");
    let remote = RemoteAppImage { url: "https://x.org/a/App.AppImage".into(), sha1: "falsch".into() };
    let http = FakeHttp::new().on("https://x.org/a/App.AppImage", std::fs::read(&v2).unwrap());

    assert!(matches!(appimage_update::apply(&remote, &dirs, &http, &FakeRunner::new()), Err(TermaError::Network(_))));
    assert_eq!(appimage::inspect(&installed.appimage).unwrap().version.as_deref(), Some("1.0"));
}
