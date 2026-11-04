use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use terma_core::aur::AurPackage;
use terma_core::aur_build::{build, classify_failure, resolve, strip_version, DepContext};
use terma_core::dirs::UserDirs;
use terma_core::error::{BuildFailure, Result, TermaError};
use terma_core::runner::{FakeRunner, Runner, SystemRunner};

fn aur(name: &str, depends: &[&str], make_depends: &[&str]) -> AurPackage {
    AurPackage {
        name: name.into(),
        package_base: name.into(),
        version: "1-1".into(),
        description: String::new(),
        url: None,
        num_votes: 100,
        popularity: 1.0,
        maintainer: Some("x".into()),
        first_submitted: 0,
        last_modified: 0,
        depends: depends.iter().map(|s| s.to_string()).collect(),
        make_depends: make_depends.iter().map(|s| s.to_string()).collect(),
        provides: vec![],
    }
}

#[derive(Default)]
struct FakeCtx {
    satisfied: HashSet<String>,
    repos: HashSet<String>,
    aur: HashMap<String, AurPackage>,
    providers: HashMap<String, String>,
}

impl DepContext for FakeCtx {
    fn satisfied(&self, dep: &str) -> bool {
        self.satisfied.contains(dep)
    }
    fn in_repos(&self, dep: &str) -> bool {
        self.repos.contains(dep)
    }
    fn aur_info(&self, names: &[String]) -> Result<Vec<AurPackage>> {
        Ok(names.iter().filter_map(|n| self.aur.get(n).cloned()).collect())
    }
    fn aur_provider(&self, dep: &str) -> Result<Option<AurPackage>> {
        Ok(self.providers.get(dep).and_then(|n| self.aur.get(n).cloned()))
    }
}

fn names(plan: &[AurPackage]) -> Vec<&str> {
    plan.iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn strip_version_cases() {
    assert_eq!(strip_version("foo>=1.2"), "foo");
    assert_eq!(strip_version("libfoo.so=1-64"), "libfoo.so");
    assert_eq!(strip_version("bar"), "bar");
}

#[test]
fn chain_orders_deps_first() {
    let mut ctx = FakeCtx::default();
    ctx.aur.insert("a".into(), aur("a", &["b"], &[]));
    ctx.aur.insert("b".into(), aur("b", &["c>=2"], &[]));
    ctx.repos.insert("c".into());

    let plan = resolve(&["a".to_string()], &ctx).unwrap();

    assert_eq!(names(&plan.build_order), vec!["b", "a"]);
    assert_eq!(plan.repo_deps, vec!["c".to_string()]);
}

#[test]
fn satisfied_deps_skipped() {
    let mut ctx = FakeCtx::default();
    ctx.aur.insert("a".into(), aur("a", &["glibc"], &["git"]));
    ctx.satisfied.insert("glibc".into());
    ctx.repos.insert("git".into());

    let plan = resolve(&["a".to_string()], &ctx).unwrap();

    assert_eq!(names(&plan.build_order), vec!["a"]);
    assert_eq!(plan.repo_deps, vec!["git".to_string()], "Make-Abhängigkeiten zählen mit");
}

#[test]
fn provider_resolution() {
    let mut ctx = FakeCtx::default();
    ctx.aur.insert("a".into(), aur("a", &["libfoo.so=1-64"], &[]));
    ctx.aur.insert("libfoo-bin".into(), aur("libfoo-bin", &[], &[]));
    ctx.providers.insert("libfoo.so=1-64".into(), "libfoo-bin".into());

    let plan = resolve(&["a".to_string()], &ctx).unwrap();

    assert_eq!(names(&plan.build_order), vec!["libfoo-bin", "a"]);
}

#[test]
fn shared_dependency_built_once() {
    let mut ctx = FakeCtx::default();
    ctx.aur.insert("a".into(), aur("a", &["b", "c"], &[]));
    ctx.aur.insert("b".into(), aur("b", &["c"], &[]));
    ctx.aur.insert("c".into(), aur("c", &[], &[]));

    let plan = resolve(&["a".to_string()], &ctx).unwrap();

    assert_eq!(names(&plan.build_order), vec!["c", "b", "a"]);
}

#[test]
fn cycle_detected() {
    let mut ctx = FakeCtx::default();
    ctx.aur.insert("a".into(), aur("a", &["b"], &[]));
    ctx.aur.insert("b".into(), aur("b", &["a"], &[]));

    assert_eq!(resolve(&["a".to_string()], &ctx).unwrap_err(), TermaError::DependencyCycle("a".into()));
}

#[test]
fn unresolvable() {
    let mut ctx = FakeCtx::default();
    ctx.aur.insert("a".into(), aur("a", &["gibtsnicht>=1"], &[]));

    assert_eq!(resolve(&["a".to_string()], &ctx).unwrap_err(), TermaError::UnresolvableDependency("gibtsnicht>=1".into()));
    assert_eq!(resolve(&["fehlt".to_string()], &ctx).unwrap_err(), TermaError::UnresolvableDependency("fehlt".into()));
}

#[test]
fn build_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let clone = dirs.cache.join("aur/t3code-bin");
    std::fs::create_dir_all(&clone).unwrap();
    std::fs::write(clone.join("alt.txt"), "vom letzten Mal").unwrap();
    let pkg_dir = dirs.cache.join("pkg");
    let built = pkg_dir.join("t3code-bin-0.1.5-1-x86_64.pkg.tar.zst");
    let listing = format!("{}\n{}\n", built.display(), pkg_dir.join("t3code-bin-debug-0.1.5-1-x86_64.pkg.tar.zst").display());
    let runner = FakeRunner::new().on(&["git", "clone"], 0, "").on(&["makepkg", "--packagelist"], 0, &listing).on(&["makepkg"], 0, "==> Fertig");
    let mut lines = Vec::new();

    let files = build(&aur("t3code-bin", &[], &[]), &dirs, &runner, &mut |l| lines.push(l.to_string())).unwrap();

    assert_eq!(files, vec![built]);
    assert!(!clone.join("alt.txt").exists(), "alter Klon wird gelöscht");
    let calls = runner.calls();
    assert_eq!(calls[0].argv, ["git", "clone", "--depth", "1", "https://aur.archlinux.org/t3code-bin.git", &clone.to_string_lossy()]);
    assert_eq!(calls[1].argv, ["makepkg", "--printsrcinfo"]);
    assert_eq!(calls[2].argv, ["makepkg", "-f", "--noconfirm", "--cleanbuild", "--nocheck"]);
    assert_eq!(calls[3].argv, ["makepkg", "--packagelist"]);
    let pkgdest = ("PKGDEST".to_string(), pkg_dir.to_string_lossy().into_owned());
    for call in &calls[1..] {
        assert_eq!(call.cwd.as_ref(), Some(&clone));
        assert!(call.env.contains(&pkgdest));
    }
    assert!(lines.contains(&"==> Fertig".to_string()));
}

const KEY: &str = "D021FF8ECF4BE09719D61A27231C4CBC60D5CAFE";

/// makepkg scheitert am Signaturschlüssel, bis gpg ihn geholt hat.
#[derive(Default)]
struct MissingKeyRunner {
    imported: std::sync::Mutex<Vec<Vec<String>>>,
    fetchable: bool,
}

impl Runner for MissingKeyRunner {
    fn run(&self, argv: &[&str], _: &[(&str, &str)], _: Option<&std::path::Path>, on_line: &mut dyn FnMut(&str)) -> std::io::Result<i32> {
        let fetched = !self.imported.lock().unwrap().is_empty() && self.fetchable;
        match argv {
            ["gpg", ..] => {
                self.imported.lock().unwrap().push(argv.iter().map(|a| a.to_string()).collect());
                Ok(if self.fetchable { 0 } else { 2 })
            }
            ["makepkg", "--printsrcinfo"] => {
                on_line("pkgbase = foo");
                on_line(&format!("\tvalidpgpkeys = {KEY}"));
                on_line("\tvalidpgpkeys = --evil");
                Ok(0)
            }
            ["makepkg", "--packagelist"] => {
                on_line("/x/foo-1-1-any.pkg.tar.zst");
                Ok(0)
            }
            ["makepkg", ..] if !fetched => {
                on_line("foo.tar.gz ... FAILED (unknown public key 231C4CBC60D5CAFE)");
                Ok(1)
            }
            _ => Ok(0),
        }
    }

    fn output(&self, _: &[&str]) -> std::io::Result<(i32, String)> {
        Ok((0, String::new()))
    }
}

/// `makepkg --packagelist` nennt alle Pakete der `pkgbase`. Installiert wird nur das geprüfte.
#[test]
fn split_package_installs_only_the_requested_one() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let wanted = PathBuf::from("/x/foo-1-1-x86_64.pkg.tar.zst");
    let listing = format!("/x/foo-docs-1-1-any.pkg.tar.zst\n{}\n/x/foo-extra-1-1-x86_64.pkg.tar.zst\n", wanted.display());
    let runner = FakeRunner::new().on(&["git", "clone"], 0, "").on(&["makepkg", "--packagelist"], 0, &listing).on(&["makepkg"], 0, "");

    let files = build(&aur("foo", &[], &[]), &dirs, &runner, &mut |_| {}).unwrap();

    assert_eq!(files, vec![wanted]);
}

#[test]
fn missing_signing_key_is_fetched_and_build_retried() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = MissingKeyRunner { fetchable: true, ..Default::default() };

    let files = build(&aur("foo", &[], &[]), &dirs, &runner, &mut |_| {}).unwrap();

    assert_eq!(files, vec![PathBuf::from("/x/foo-1-1-any.pkg.tar.zst")]);
    let expected: Vec<String> = ["gpg", "--batch", "--recv-keys", KEY].iter().map(|s| s.to_string()).collect();
    assert_eq!(*runner.imported.lock().unwrap(), vec![expected], "nur der Fingerabdruck aus dem PKGBUILD, kein Options-Text");
}

#[test]
fn unfetchable_signing_key_stays_a_build_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());

    let err = build(&aur("foo", &[], &[]), &dirs, &MissingKeyRunner::default(), &mut |_| {}).unwrap_err();

    assert!(matches!(err, TermaError::BuildFailed { kind: BuildFailure::PgpKeyUnknown, .. }));
}

#[test]
fn classifies_build_failures() {
    assert_eq!(classify_failure(include_str!("fixtures/build/pgp-fail.log")), BuildFailure::PgpKeyUnknown);
    assert_eq!(classify_failure(include_str!("fixtures/build/checksum-fail.log")), BuildFailure::ChecksumMismatch);
    assert_eq!(classify_failure("==> ERROR: A failure occurred in build()."), BuildFailure::Other);

    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = FakeRunner::new().on(&["git", "clone"], 0, "").on(&["makepkg", "-f"], 1, include_str!("fixtures/build/pgp-fail.log")).on(&["makepkg"], 0, "");

    let err = build(&aur("example", &[], &[]), &dirs, &runner, &mut |_| {}).unwrap_err();

    let TermaError::BuildFailed { package, kind, log } = err else { panic!("{err:?}") };
    assert_eq!(package, "example");
    assert_eq!(kind, BuildFailure::PgpKeyUnknown);
    assert!(log.starts_with(dirs.data.join("logs")));
    assert!(log.file_name().unwrap().to_string_lossy().starts_with("example-"));
    assert!(std::fs::read_to_string(&log).unwrap().contains("unknown public key"));
}

#[test]
#[ignore = "baut ein echtes AUR-Paket (im Arch-Container als builder)"]
fn real_build_small_package() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());

    let files: Vec<PathBuf> = build(&aur("pfetch", &[], &[]), &dirs, &SystemRunner, &mut |_| {}).unwrap();

    assert!(!files.is_empty());
    assert!(files.iter().all(|f| f.is_file() && f.to_string_lossy().ends_with(".pkg.tar.zst")), "{files:?}");
}
