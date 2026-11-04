mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terma_core::appimage;
use terma_core::aur::{AurClient, AurStateStore};
use terma_core::dirs::UserDirs;
use terma_core::error::TermaError;
use terma_core::http::FakeHttp;
use terma_core::integrate::integrate_appimage;
use terma_core::ops::{PackageWarning, Progress, Settings, SystemUpdate, Terma, HELPER, HELPER_AUR};
use terma_core::programs::{Program, UpdateState};
use terma_core::runner::FakeRunner;
use terma_core::search::{Candidate, Source};
use terma_core::trust::TrustReason;

const BASE: &str = "https://aur.archlinux.org";
const INFO_T3: &str = include_str!("fixtures/aur/info-t3code-bin.json");
const SRCINFO_T3: &str = include_str!("fixtures/aur/srcinfo-t3code-bin.txt");

struct Setup {
    terma: Terma,
    runner: Arc<FakeRunner>,
    dirs: UserDirs,
    _tmp: tempfile::TempDir,
}

/// Grundregeln: Sync klappt, nichts installiert, kein Kernel, leere Vorschau.
fn base_rules(runner: FakeRunner) -> FakeRunner {
    runner
        .on(&["fakeroot"], 0, "")
        .on(&["pacman", "-Qqo", "/usr/lib/modules"], 0, "linux-cachyos\n")
        .on(&["pacman", "-Q"], 0, "")
        .on(&["pacman", "-Sup"], 0, "")
        .on(&["pacman", "-Sp"], 0, "")
}

fn setup(runner: FakeRunner, http: FakeHttp) -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(base_rules(runner));
    let http = Arc::new(http);
    let terma = Terma::with(runner.clone(), AurClient::new(BASE, http.clone()), http, dirs.clone());
    Setup { terma, runner, dirs, _tmp: tmp }
}

fn candidate(name: &str, source: Source) -> Candidate {
    Candidate {
        name: name.into(),
        display_name: name.into(),
        description: String::new(),
        source,
        version: "1-1".into(),
        download_size: Some(1),
        installed: false,
        url: None,
        icon: None,
    }
}

fn repo(name: &str) -> Candidate {
    candidate(name, Source::Repo { repo: "extra".into() })
}

fn pkexec_calls(runner: &FakeRunner) -> Vec<Vec<String>> {
    runner.calls().into_iter().map(|c| c.argv).filter(|a| a[0] == "pkexec").collect()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn no_progress(_: Progress) {}

#[test]
fn install_repo_calls_helper() {
    let s = setup(FakeRunner::new().on(&["pkexec"], 0, "").on(&["pacman", "-Sp"], 0, "discord 1:1.0.167-1 2170552\n"), FakeHttp::new());

    let plan = s.terma.plan_install(&repo("discord"), None).unwrap();
    assert_eq!(plan.system_update, None);
    let outcome = s.terma.install(&plan, &mut no_progress).unwrap();

    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER, "install", "discord"])]);
    assert!(!outcome.restart_recommended);
}

fn pending_setup() -> Setup {
    setup(
        FakeRunner::new()
            .on(&["pkexec"], 0, "")
            .on(&["pacman", "-Q"], 0, "linux-cachyos 6.0-1\nmesa 1-1\n")
            .on(&["pacman", "-Sp"], 0, "discord 1 100\nmesa 2 10\n")
            .on(&["pacman", "-Sup"], 0, "discord 1 100\nlinux-cachyos 2 1000\nmesa 2 10\n"),
        FakeHttp::new(),
    )
}

/// Offene Updates allein sind kein Grund: Das neue Paket ändert nichts Installiertes.
#[test]
fn plan_repo_only_new_packages_skips_system_update() {
    let s = setup(
        FakeRunner::new()
            .on(&["pacman", "-Q"], 0, "linux-cachyos 6.0-1\nmesa 1-1\n")
            .on(&["pacman", "-Sp"], 0, "kmines 1 100\nlibkmahjongg 1 10\n")
            .on(&["pacman", "-Sup"], 0, "kmines 1 100\nlibkmahjongg 1 10\nlinux-cachyos 2 1000\n"),
        FakeHttp::new(),
    );
    let plan = s.terma.plan_install(&repo("kmines"), None).unwrap();
    assert_eq!(plan.system_update, None);
}

#[test]
fn plan_repo_with_pending_updates() {
    let s = pending_setup();
    let plan = s.terma.plan_install(&repo("discord"), None).unwrap();
    assert_eq!(plan.system_update, Some(SystemUpdate { package_count: 2, download_bytes: 1110, includes_kernel: true }));
    assert!(s.runner.calls().iter().any(|c| c.argv.contains(&"-Sy".to_string())), "plan_install synchronisiert zuerst");
}

#[test]
fn install_outcome_restart() {
    let s = pending_setup();
    let plan = s.terma.plan_install(&repo("discord"), None).unwrap();
    assert!(s.terma.install(&plan, &mut no_progress).unwrap().restart_recommended);
}

fn t3_http() -> FakeHttp {
    FakeHttp::new()
        .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=t3code-bin"), INFO_T3)
        .on(&format!("{BASE}/cgit/aur.git/plain/.SRCINFO?h=t3code-bin"), SRCINFO_T3)
}

#[test]
fn plan_aur_collects_warnings() {
    let s = setup(FakeRunner::new().on(&["pacman", "-T"], 0, ""), t3_http());

    let plan = s.terma.plan_install(&candidate("t3code-bin", Source::Aur), None).unwrap();

    assert_eq!(plan.warnings, vec![PackageWarning { package: "t3code-bin".into(), reasons: vec![TrustReason::FewVotes { votes: 11 }] }]);
    let aur = plan.aur.unwrap();
    assert_eq!(aur.build_order.len(), 1);
    assert!(aur.repo_deps.is_empty());
}

const APP_INFO: &str = r#"{"version":5,"type":"multiinfo","resultcount":1,"results":[{"Name":"app-bin","PackageBase":"app-bin",
"Version":"1-1","URL":"https://example.org","NumVotes":500,"Popularity":9.0,"Maintainer":"x","FirstSubmitted":1500000000,
"LastModified":1700000000,"Depends":["nodejs"]}]}"#;

fn app_setup(nodejs_installed: bool, pkg_file: &str) -> Setup {
    app_setup_from(FakeRunner::new(), nodejs_installed, pkg_file)
}

fn app_setup_from(runner: FakeRunner, nodejs_installed: bool, pkg_file: &str) -> Setup {
    let runner = runner
        .on(&["pacman", "-T", "nodejs"], if nodejs_installed { 0 } else { 1 }, "")
        .on(&["pacman", "-Sp"], 0, "nodejs\n")
        .on(&["git", "clone"], 0, "")
        .on(&["makepkg", "--packagelist"], 0, pkg_file)
        .on(&["makepkg", "--printsrcinfo"], 0, "pkgbase = app-bin\n\tsource = https://dl.example.org/app.tar.gz\n")
        .on(&["makepkg"], 0, "")
        .on(&["pkexec"], 0, "");
    let http = FakeHttp::new()
        .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=app-bin"), APP_INFO)
        .on(&format!("{BASE}/cgit/aur.git/plain/.SRCINFO?h=app-bin"), "pkgbase = app-bin\n\tsource = https://dl.example.org/app.tar.gz\n");
    setup(runner, http)
}

#[test]
fn install_aur_sequence() {
    let s = app_setup(false, "/x/pkg/app-bin-1-1-x86_64.pkg.tar.zst\n");

    let plan = s.terma.plan_install(&candidate("app-bin", Source::Aur), None).unwrap();
    assert!(plan.warnings.is_empty());
    s.terma.install(&plan, &mut no_progress).unwrap();

    let relevant: Vec<Vec<String>> = s
        .runner
        .calls()
        .into_iter()
        .map(|c| c.argv)
        .filter(|a| ["pkexec", "git", "makepkg"].contains(&a[0].as_str()))
        .map(|a| a.into_iter().take(4).collect())
        .collect();
    assert_eq!(
        relevant,
        vec![
            strings(&["pkexec", HELPER_AUR, "add-deps", "nodejs"]),
            strings(&["git", "clone", "--depth", "1"]),
            strings(&["makepkg", "--printsrcinfo"]),
            strings(&["makepkg", "-f", "--noconfirm", "--cleanbuild"]),
            strings(&["makepkg", "--packagelist"]),
            strings(&["pkexec", HELPER_AUR, "install-files", "/x/pkg/app-bin-1-1-x86_64.pkg.tar.zst"]),
        ]
    );
    let record = AurStateStore::load(&s.dirs).get("app-bin").cloned().expect("AurRecord gespeichert");
    assert_eq!(record.maintainer.as_deref(), Some("x"));
    assert!(record.source_domains.contains("example.org"));
}

#[test]
fn install_aur_without_repo_deps_skips_system_update() {
    let s = app_setup(true, "/x/pkg/app-bin-1-1-x86_64.pkg.tar.zst\n");
    let plan = s.terma.plan_install(&candidate("app-bin", Source::Aur), None).unwrap();
    assert_eq!(plan.system_update, None);
    s.terma.install(&plan, &mut no_progress).unwrap();
    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER_AUR, "install-files", "/x/pkg/app-bin-1-1-x86_64.pkg.tar.zst"])]);
}

#[test]
fn install_aur_dep_changing_installed_updates_system() {
    let runner = FakeRunner::new()
        .on(&["pacman", "-Q"], 0, "nodejs 1-1\n")
        .on(&["pacman", "-Sp"], 0, "nodejs 2-1 10\n")
        .on(&["pacman", "-Sup"], 0, "nodejs 2-1 10\n");
    let s = app_setup_from(runner, false, "/x/pkg/app-bin-1-1-x86_64.pkg.tar.zst\n");
    let plan = s.terma.plan_install(&candidate("app-bin", Source::Aur), None).unwrap();
    assert_eq!(plan.system_update, Some(SystemUpdate { package_count: 1, download_bytes: 10, includes_kernel: false }));
    s.terma.install(&plan, &mut no_progress).unwrap();
    assert_eq!(
        pkexec_calls(&s.runner),
        vec![
            strings(&["pkexec", HELPER_AUR, "install-deps", "nodejs"]),
            strings(&["pkexec", HELPER_AUR, "install-files", "/x/pkg/app-bin-1-1-x86_64.pkg.tar.zst"]),
        ]
    );
}

#[test]
fn install_from_file_retires_old_entries() {
    let s = setup(FakeRunner::new().on(&["pkexec"], 0, "").on(&["update-desktop-database"], 0, ""), FakeHttp::new());
    let file = PathBuf::from("/mnt/daten/T3-Code-0.1.1-x86_64.AppImage");
    let hidden = s.dirs.applications.join("com.t3tools.T3Code.desktop");
    std::fs::write(&hidden, format!("[Desktop Entry]\nName=T3\nExec={} %U\nNoDisplay=true\n", file.display())).unwrap();

    let plan = s.terma.plan_install(&repo("t3code"), Some(&file)).unwrap();
    s.terma.install(&plan, &mut no_progress).unwrap();

    assert!(!hidden.exists());
    assert!(s.dirs.data.join("backup").exists());
}

fn helper_exit(code: i32, stdout: &str) -> TermaError {
    let s = setup(FakeRunner::new().on(&["pkexec"], code, stdout), FakeHttp::new());
    let plan = s.terma.plan_install(&repo("discord"), None).unwrap();
    s.terma.install(&plan, &mut no_progress).unwrap_err()
}

#[test]
fn pkexec_126_is_cancelled() {
    assert_eq!(helper_exit(126, ""), TermaError::Cancelled);
    assert_eq!(helper_exit(127, ""), TermaError::Cancelled);
}

#[test]
fn exit_3_maps_to_db_locked() {
    assert_eq!(helper_exit(3, "DB_LOCKED"), TermaError::DbLocked);
    assert_eq!(helper_exit(4, "PROTECTED dolphin"), TermaError::Protected { package: "dolphin".into() });
    assert!(matches!(helper_exit(1, "error: failed to commit transaction"), TermaError::Command { code: 1, tail } if tail.contains("failed to commit")));
}

#[test]
fn remove_variants() {
    let s = setup(FakeRunner::new().on(&["pkexec"], 0, "").on(&["update-desktop-database"], 0, ""), FakeHttp::new());
    let mut steps = Vec::new();
    let discord = Program {
        name: "Discord".into(),
        icon: None,
        source: Source::Repo { repo: String::new() },
        update: UpdateState::None,
        desktop_file: PathBuf::from("/usr/share/applications/discord.desktop"),
        package: Some("discord".into()),
    };
    s.terma.remove(&discord, &mut |p| steps.push(p)).unwrap();
    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER, "remove", "discord"])]);
    assert!(!steps.is_empty());

    let source = support::make_appimage(s._tmp.path(), "App.AppImage", "app.desktop", "[Desktop Entry]\nName=App\nExec=AppRun\n", None, None, &[]);
    let app = integrate_appimage(&appimage::inspect(&source).unwrap(), &s.dirs, s.runner.as_ref()).unwrap();
    let program = Program { name: "App".into(), source: Source::AppImage, desktop_file: app.desktop_file.clone(), package: None, ..discord };

    s.terma.remove(&program, &mut no_progress).unwrap();

    assert_eq!(pkexec_calls(&s.runner).len(), 1, "AppImages brauchen keinen Root");
    assert!(!app.appimage.exists() && !app.desktop_file.exists());
}

#[test]
fn set_passwordless_roundtrip() {
    let s = setup(FakeRunner::new().on(&["pkexec"], 0, ""), FakeHttp::new());
    assert!(!s.terma.settings().passwordless);

    s.terma.set_passwordless(true).unwrap();

    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER, "passwordless", "on"])]);
    assert!(Settings::load(&s.dirs).passwordless);

    let cancelled = Terma::with(
        Arc::new(base_rules(FakeRunner::new().on(&["pkexec"], 126, ""))),
        AurClient::new(BASE, Arc::new(FakeHttp::new())),
        Arc::new(FakeHttp::new()),
        s.dirs.clone(),
    );
    assert_eq!(cancelled.set_passwordless(false).unwrap_err(), TermaError::Cancelled);
    assert!(Settings::load(&s.dirs).passwordless, "bei Abbruch unverändert");
}

#[test]
fn setup_appimage_integrates() {
    let s = setup(FakeRunner::new().on(&["update-desktop-database"], 0, ""), FakeHttp::new());
    let source = support::make_appimage(s._tmp.path(), "App.AppImage", "app.desktop", "[Desktop Entry]\nName=App\nExec=AppRun\n", None, None, &[]);

    let app = s.terma.setup_appimage(&source).unwrap();

    assert!(app.appimage.starts_with(&s.dirs.appimages));
    assert!(Path::new(&app.desktop_file).exists());
}

#[test]
fn search_combines_sources() {
    let s = setup(
        FakeRunner::new()
            .on(&["pacman", "-Ss"], 0, "extra/discord 1:1.0.167-1\n    All-in-one voice and text chat for gamers\n")
            .on(&["pacman", "-Si"], 0, "Name            : discord\nURL             : https://discord.com\nDownload Size   : 2.07 MiB\n")
            .on(&["pacman", "-Fl"], 0, "discord usr/share/applications/discord.desktop\n")
            .on(&["pacman", "-Qmq"], 1, ""),
        FakeHttp::new().on(&format!("{BASE}/rpc/v5/search/discord?by=name"), include_str!("fixtures/aur/search-t3.json").replace("t3code", "discord")),
    );

    let result = s.terma.search("Discord").unwrap();

    let recommended = result.recommended.unwrap();
    assert_eq!(recommended.name, "discord");
    assert_eq!(recommended.url.as_deref(), Some("https://discord.com"));
    assert!(!result.aur_unavailable);
}

#[test]
fn first_start_sets_appimage_default_once() {
    let s = setup(FakeRunner::new().on(&["xdg-mime"], 0, ""), FakeHttp::new());
    let xdg_calls = |r: &FakeRunner| r.calls().into_iter().filter(|c| c.argv[0] == "xdg-mime").map(|c| c.argv).collect::<Vec<_>>();

    s.terma.first_start().unwrap();
    s.terma.first_start().unwrap();

    assert_eq!(
        xdg_calls(&s.runner),
        vec![strings(&["xdg-mime", "default", "terma.desktop", "application/vnd.appimage", "application/x-iso9660-appimage"])]
    );
    assert!(s.dirs.config.join("settings.json").exists());
}

#[test]
fn remove_preview_lists_dependents() {
    let s = setup(FakeRunner::new().on(&["pacman", "-Rsc", "--print"], 0, "firefox-i18n-de\nfirefox\n"), FakeHttp::new());
    let firefox = Program {
        name: "Firefox".into(),
        icon: None,
        source: Source::Repo { repo: String::new() },
        update: UpdateState::None,
        desktop_file: PathBuf::from("/usr/share/applications/firefox.desktop"),
        package: Some("firefox".into()),
    };

    assert_eq!(s.terma.remove_preview(&firefox).unwrap(), vec!["firefox-i18n-de".to_string()]);
    let appimage = Program { source: Source::AppImage, package: None, ..firefox };
    assert!(s.terma.remove_preview(&appimage).unwrap().is_empty());
}
