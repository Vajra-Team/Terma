mod support;

use std::collections::BTreeSet;
use std::sync::Arc;

use terma_core::appimage;
use terma_core::aur::{AurClient, AurRecord, AurStateStore};
use terma_core::dirs::UserDirs;
use terma_core::error::{BuildFailure, TermaError};
use terma_core::http::FakeHttp;
use terma_core::integrate::integrate_appimage;
use terma_core::ops::{FailedUpdate, Progress, SkippedUpdate, Terma, HELPER, HELPER_AUR};
use terma_core::programs::UpdateState;
use terma_core::runner::FakeRunner;
use terma_core::trust::TrustReason;

const BASE: &str = "https://aur.archlinux.org";

fn info(maintainer: &str) -> String {
    format!(
        r#"{{"version":5,"type":"multiinfo","resultcount":1,"results":[{{"Name":"foo-bin","PackageBase":"foo-bin","Version":"2-1",
"URL":"https://example.org","NumVotes":3,"Popularity":0.1,"Maintainer":"{maintainer}","FirstSubmitted":1790000000,"LastModified":1793000000}}]}}"#
    )
}

const SRCINFO: &str = "pkgbase = foo-bin\n\tsource = https://dl.example.org/foo-2.tar.gz\n";
const PKG: &str = "/x/pkg/foo-bin-2-1-x86_64.pkg.tar.zst\n";

struct Setup {
    terma: Terma,
    runner: Arc<FakeRunner>,
    dirs: UserDirs,
    tmp: tempfile::TempDir,
}

fn setup(foreign: &str, maintainer: &str, extra_http: FakeHttp) -> Setup {
    setup_from(FakeRunner::new(), foreign, maintainer, extra_http)
}

fn setup_from(runner: FakeRunner, foreign: &str, maintainer: &str, extra_http: FakeHttp) -> Setup {
    setup_info(runner, foreign, &info(maintainer), extra_http)
}

fn multiinfo(result: &str) -> String {
    format!(r#"{{"version":5,"type":"multiinfo","resultcount":1,"results":[{result}]}}"#)
}

/// Lange da und viel genutzt, die Vertrauensregel hat nichts einzuwenden.
fn established(name: &str, depends: &[&str]) -> String {
    let depends = depends.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(",");
    multiinfo(&format!(
        r#"{{"Name":"{name}","PackageBase":"{name}","Version":"2-1","URL":"https://example.org","NumVotes":500,"Popularity":5.0,"Maintainer":"alice","FirstSubmitted":1500000000,"LastModified":1793000000,"Depends":[{depends}]}}"#
    ))
}

fn setup_info(runner: FakeRunner, foreign: &str, aur_info: &str, extra_http: FakeHttp) -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(
        runner
            .on(&["fakeroot"], 0, "")
            .on(&["pacman", "-Qqo", "/usr/lib/modules"], 0, "linux-cachyos\n")
            .on(&["pacman", "-Qmq"], 0, foreign)
            .on(&["pacman", "-Q"], 0, "mesa 1-1\nfoo-bin 1-1\nterma 1-1\n")
            .on(&["pacman", "-Sup"], 0, "mesa 2-1 10\n")
            .on(&["vercmp", "1-1", "2-1"], 0, "-1\n")
            .on(&["git", "clone"], 0, "")
            .on(&["makepkg", "--packagelist"], 0, PKG)
            .on(&["makepkg", "--printsrcinfo"], 0, SRCINFO)
            .on(&["makepkg"], 0, "")
            .on(&["pkexec"], 0, "")
            .on(&["update-desktop-database"], 0, ""),
    );
    let http = Arc::new(
        extra_http
            .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=foo-bin"), aur_info)
            .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=terma"), aur_info)
            .on(&format!("{BASE}/cgit/aur.git/plain/.SRCINFO?h=foo-bin"), SRCINFO),
    );
    let terma = Terma::with(runner.clone(), AurClient::new(BASE, http.clone()), http, dirs.clone());
    Setup { terma, runner, dirs, tmp }
}

fn pkexec_calls(runner: &FakeRunner) -> Vec<Vec<String>> {
    runner.calls().into_iter().map(|c| c.argv).filter(|a| a[0] == "pkexec").collect()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn no_progress(_: Progress) {}

fn record(maintainer: &str) -> AurRecord {
    AurRecord { maintainer: Some(maintainer.into()), source_domains: BTreeSet::from(["example.org".to_string()]) }
}

fn put_record(dirs: &UserDirs, maintainer: &str) {
    let mut store = AurStateStore::load(dirs);
    store.put("foo-bin", record(maintainer));
    store.save().unwrap();
}

#[test]
fn update_all_repo_only() {
    let s = setup("", "alice", FakeHttp::new());
    let report = s.terma.update_all(&mut no_progress).unwrap();
    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER, "upgrade"])]);
    assert!(report.skipped.is_empty());
}

#[test]
fn update_all_with_aur() {
    let s = setup("foo-bin\n", "alice", FakeHttp::new());
    put_record(&s.dirs, "alice");

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER_AUR, "install-files", "--upgrade", PKG.trim()])]);
    assert!(report.skipped.is_empty());
}

/// Jamie, 02.11.2026: Der Bau von lib32-mpg123 scheiterte, und das ganze Systemupdate fiel aus.
#[test]
fn update_all_failed_build_keeps_the_rest_going() {
    let s = setup_from(FakeRunner::new().on(&["makepkg", "-f"], 1, "FAILED (unknown public key ABC)\n"), "foo-bin\n", "alice", FakeHttp::new());
    put_record(&s.dirs, "alice");

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(report.failed, vec![FailedUpdate { package: "foo-bin".into(), kind: BuildFailure::PgpKeyUnknown }]);
    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER, "upgrade"])], "das System wird trotzdem aktualisiert");
}

#[test]
fn force_update_aur_reports_failed_build() {
    let s = setup_from(FakeRunner::new().on(&["makepkg", "-f"], 1, ""), "foo-bin\n", "alice", FakeHttp::new());

    let err = s.terma.force_update_aur(&["foo-bin".to_string()], &mut no_progress).unwrap_err();

    assert!(matches!(err, TermaError::BuildFailed { .. }));
    assert!(pkexec_calls(&s.runner).is_empty());
}

#[test]
fn update_all_skips_takeover() {
    let s = setup("foo-bin\n", "mallory", FakeHttp::new());
    put_record(&s.dirs, "alice");

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(
        report.skipped,
        vec![SkippedUpdate {
            package: "foo-bin".into(),
            reasons: vec![TrustReason::MaintainerChanged { old: Some("alice".into()), new: Some("mallory".into()) }],
        }]
    );
    assert!(!s.runner.calls().iter().any(|c| c.argv[0] == "git"), "nicht gebaut");
    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER, "upgrade"])], "der Rest läuft weiter");
    assert_eq!(AurStateStore::load(&s.dirs).get("foo-bin"), Some(&record("alice")), "Record bleibt");
}

/// Ohne gespeicherten Stand zählt die volle Regel. Sonst könnte jemand im AUR den Namen eines Fremdpakets
/// belegen, und sein Paket würde ohne Warnung als Update installiert.
#[test]
fn update_all_without_record_applies_the_full_rule() {
    let s = setup("foo-bin\n", "alice", FakeHttp::new());

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(report.skipped.len(), 1);
    assert!(report.skipped[0].reasons.iter().any(|r| matches!(r, TrustReason::FewVotes { .. })));
    assert!(!s.runner.calls().iter().any(|c| c.argv[0] == "git"), "nicht gebaut");
    assert_eq!(AurStateStore::load(&s.dirs).get("foo-bin"), None, "kein Stand für ungeprüften Code");
}

#[test]
fn update_all_without_record_builds_an_established_package() {
    let http = FakeHttp::new();
    let s = setup_info(FakeRunner::new(), "foo-bin\n", &established("foo-bin", &[]), http);

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert!(report.skipped.is_empty());
    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER_AUR, "install-files", "--upgrade", PKG.trim()])]);
    assert_eq!(AurStateStore::load(&s.dirs).get("foo-bin"), Some(&record("alice")));
}

/// Terma wird per `pacman -U` installiert und ist damit ein Fremdpaket. Ein Paket `terma` im AUR darf nie als Update gelten.
#[test]
fn terma_itself_is_never_taken_from_the_aur() {
    let s = setup_info(FakeRunner::new(), "terma\n", &established("terma", &[]), FakeHttp::new());

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert!(report.skipped.is_empty());
    assert!(!s.runner.calls().iter().any(|c| c.argv[0] == "git"), "nicht gebaut");
    assert_eq!(s.terma.overview().unwrap().pending_count, 1, "nur mesa aus den Quellen");
}

/// Ein vertrautes Paket braucht plötzlich ein unbekanntes aus dem AUR: Das fällt auf, beide bleiben ungebaut.
#[test]
fn update_all_checks_new_aur_dependencies() {
    let runner = FakeRunner::new().on(&["pacman", "-T", "evil-lib"], 127, "").on(&["pacman", "-Sp", "--print-format", "%n", "evil-lib"], 1, "");
    let http = FakeHttp::new()
        .on(&format!("{BASE}/rpc/v5/info?arg%5B%5D=evil-lib"), multiinfo(r#"{"Name":"evil-lib","PackageBase":"evil-lib","Version":"1-1","NumVotes":0,"Popularity":0,"Maintainer":"mallory","FirstSubmitted":1793000000,"LastModified":1793000000}"#))
        .on(&format!("{BASE}/cgit/aur.git/plain/.SRCINFO?h=evil-lib"), "pkgbase = evil-lib\n");
    let s = setup_info(runner, "foo-bin\n", &established("foo-bin", &["evil-lib"]), http);
    put_record(&s.dirs, "alice");

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(
        report.skipped,
        vec![SkippedUpdate { package: "foo-bin".into(), reasons: vec![TrustReason::RiskyDependency { package: "evil-lib".into() }] }]
    );
    assert!(!s.runner.calls().iter().any(|c| c.argv[0] == "git"), "nichts gebaut");
}

/// Geprüft wurde `.SRCINFO` aus dem Netz, gebaut wird das PKGBUILD. Weicht es ab, wird nichts gebaut.
#[test]
fn update_all_stops_when_the_clone_has_other_sources() {
    let other = "pkgbase = foo-bin\n\tsource = https://evil.example.net/foo.tar.gz\n";
    let s = setup_from(FakeRunner::new().on(&["makepkg", "--printsrcinfo"], 0, other), "foo-bin\n", "alice", FakeHttp::new());
    put_record(&s.dirs, "alice");

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(report.failed, vec![FailedUpdate { package: "foo-bin".into(), kind: BuildFailure::SourcesChanged }]);
    assert!(!s.runner.calls().iter().any(|c| c.argv.starts_with(&strings(&["makepkg", "-f"]))), "nicht gebaut");
    assert_eq!(AurStateStore::load(&s.dirs).get("foo-bin"), Some(&record("alice")), "Stand bleibt");
}

#[test]
fn update_all_says_when_the_aur_is_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(
        FakeRunner::new()
            .on(&["fakeroot"], 0, "")
            .on(&["pacman", "-Qmq"], 0, "foo-bin\n")
            .on(&["pacman", "-Q"], 0, "foo-bin 1-1\n")
            .on(&["pacman", "-Sup"], 0, "")
            .on(&["pkexec"], 0, "")
            .on(&["update-desktop-database"], 0, ""),
    );
    let http = Arc::new(FakeHttp::new());
    let terma = Terma::with(runner, AurClient::new(BASE, http.clone()), http, dirs);

    let report = terma.update_all(&mut no_progress).unwrap();

    assert!(report.aur_unreachable);
}

#[test]
fn force_update_aur_builds_without_recheck() {
    let s = setup("foo-bin\n", "mallory", FakeHttp::new());
    put_record(&s.dirs, "alice");

    s.terma.force_update_aur(&["foo-bin".to_string()], &mut no_progress).unwrap();

    assert_eq!(pkexec_calls(&s.runner), vec![strings(&["pkexec", HELPER_AUR, "install-files", "--upgrade", PKG.trim()])]);
    assert_eq!(AurStateStore::load(&s.dirs).get("foo-bin").unwrap().maintainer.as_deref(), Some("mallory"));
}

#[test]
fn update_all_updates_appimages() {
    let zsync_url = "https://x.org/a/App.AppImage.zsync";
    let v2_dir = tempfile::tempdir().unwrap();
    let desktop = |v: &str| format!("[Desktop Entry]\nName=App\nExec=AppRun\nX-AppImage-Version={v}\n");
    let v2 = support::make_appimage(v2_dir.path(), "App.AppImage", "app.desktop", &desktop("2.0"), None, Some(&format!("zsync|{zsync_url}")), &[]);
    let sha = terma_core::appimage_update::sha1_file(&v2).unwrap();
    let http = FakeHttp::new()
        .on(zsync_url, format!("URL: App.AppImage\nSHA-1: {sha}\n\n"))
        .on("https://x.org/a/App.AppImage", std::fs::read(&v2).unwrap());
    let s = setup("", "alice", http);
    let v1 = support::make_appimage(s.tmp.path(), "App.AppImage", "app.desktop", &desktop("1.0"), None, Some(&format!("zsync|{zsync_url}")), &[]);
    let app = integrate_appimage(&appimage::inspect(&v1).unwrap(), &s.dirs, s.runner.as_ref()).unwrap();

    let report = s.terma.update_all(&mut no_progress).unwrap();

    assert_eq!(report.appimages_updated, vec!["App".to_string()]);
    assert_eq!(appimage::inspect(&app.appimage).unwrap().version.as_deref(), Some("2.0"));
}

#[test]
fn overview_counts_pending() {
    let zsync_url = "https://x.org/a/App.AppImage.zsync";
    let http = FakeHttp::new().on(zsync_url, "URL: App.AppImage\nSHA-1: 0000\n\n");
    let s = setup("foo-bin\n", "alice", http);
    let file = support::make_appimage(s.tmp.path(), "App.AppImage", "app.desktop", "[Desktop Entry]\nName=App\nExec=AppRun\n", None, Some(&format!("zsync|{zsync_url}")), &[]);
    integrate_appimage(&appimage::inspect(&file).unwrap(), &s.dirs, s.runner.as_ref()).unwrap();

    let overview = s.terma.overview().unwrap();

    assert_eq!(overview.pending_count, 3, "mesa (offiziell) + foo-bin (AUR) + App (AppImage)");
    let app = overview.programs.iter().find(|p| p.name == "App").unwrap();
    assert_eq!(app.update, UpdateState::Ready);
    assert!(s.runner.calls().iter().any(|c| c.argv.contains(&"-Sy".to_string())), "overview synchronisiert");
}
