//! AUR-Pakete bauen: Abhängigkeiten auflösen (offiziell oder AUR, rekursiv), dann klonen und `makepkg`
//! als normaler Benutzer. Installiert wird über den Root-Helfer, nicht hier.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::aur::{parse_srcinfo, source_domains, AurClient, AurPackage};
use crate::dirs::UserDirs;
use crate::error::{BuildFailure, Result, TermaError};
use crate::pacman::PacmanDb;
use crate::runner::Runner;

pub trait DepContext {
    fn satisfied(&self, dep: &str) -> bool;
    fn in_repos(&self, dep: &str) -> bool;
    fn aur_info(&self, names: &[String]) -> Result<Vec<AurPackage>>;
    fn aur_provider(&self, dep: &str) -> Result<Option<AurPackage>>;
}

/// Die echte Umgebung: installierte Pakete, eigene Paketdatenbank, AUR.
pub struct SystemDeps<'a> {
    pub db: &'a PacmanDb,
    pub aur: &'a AurClient,
}

impl DepContext for SystemDeps<'_> {
    fn satisfied(&self, dep: &str) -> bool {
        self.db.satisfied(dep)
    }
    fn in_repos(&self, dep: &str) -> bool {
        self.db.in_repos(dep)
    }
    fn aur_info(&self, names: &[String]) -> Result<Vec<AurPackage>> {
        self.aur.info(names)
    }
    fn aur_provider(&self, dep: &str) -> Result<Option<AurPackage>> {
        self.aur.provider(dep)
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AurBuildPlan {
    pub repo_deps: Vec<String>,
    pub build_order: Vec<AurPackage>,
    /// Download-Domains, die die Vertrauensregel gesehen hat. Fehlt ein Paket, wird nicht verglichen („Trotzdem …“).
    #[serde(default)]
    pub checked_sources: BTreeMap<String, BTreeSet<String>>,
}

pub struct Built {
    pub files: Vec<PathBuf>,
    pub source_domains: BTreeSet<String>,
}

/// `"foo>=1.2"` → `"foo"`
pub fn strip_version(dep: &str) -> &str {
    dep.split(['<', '>', '=']).next().unwrap_or(dep)
}

#[derive(PartialEq)]
enum Mark {
    Visiting,
    Done,
}

struct Resolver<'a> {
    ctx: &'a dyn DepContext,
    marks: HashMap<String, Mark>,
    plan: AurBuildPlan,
}

impl Resolver<'_> {
    /// Ist `dep` schon durch ein geplantes AUR-Paket abgedeckt (Name oder `provides`)?
    fn planned(&self, dep: &str) -> bool {
        let name = strip_version(dep);
        self.plan.build_order.iter().any(|p| p.name == name || p.provides.iter().any(|pr| strip_version(pr) == name))
    }

    fn visit(&mut self, pkg: AurPackage) -> Result<()> {
        match self.marks.get(&pkg.name) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => return Err(TermaError::DependencyCycle(pkg.name)),
            None => {}
        }
        self.marks.insert(pkg.name.clone(), Mark::Visiting);
        for dep in pkg.depends.iter().chain(&pkg.make_depends) {
            if self.ctx.satisfied(dep) || self.planned(dep) {
                continue;
            }
            let name = strip_version(dep);
            if self.ctx.in_repos(name) {
                if !self.plan.repo_deps.iter().any(|d| d == name) {
                    self.plan.repo_deps.push(name.to_string());
                }
                continue;
            }
            let found = match self.ctx.aur_info(&[name.to_string()])?.into_iter().next() {
                Some(p) => Some(p),
                None => self.ctx.aur_provider(dep)?,
            };
            match found {
                Some(dep_pkg) => self.visit(dep_pkg)?,
                None => return Err(TermaError::UnresolvableDependency(dep.clone())),
            }
        }
        self.marks.insert(pkg.name.clone(), Mark::Done);
        self.plan.build_order.push(pkg);
        Ok(())
    }
}

/// Baureihenfolge (Abhängigkeiten zuerst) und offizielle Pakete, die vorher installiert werden müssen.
pub fn resolve(targets: &[String], ctx: &dyn DepContext) -> Result<AurBuildPlan> {
    let mut resolver = Resolver { ctx, marks: HashMap::new(), plan: AurBuildPlan { repo_deps: Vec::new(), build_order: Vec::new(), checked_sources: BTreeMap::new() } };
    let found = ctx.aur_info(targets)?;
    for target in targets {
        let pkg = found.iter().find(|p| &p.name == target).cloned().ok_or_else(|| TermaError::UnresolvableDependency(target.clone()))?;
        resolver.visit(pkg)?;
    }
    Ok(resolver.plan)
}

pub fn classify_failure(log: &str) -> BuildFailure {
    if log.contains("unknown public key") {
        BuildFailure::PgpKeyUnknown
    } else if log.contains("did not pass the validity check") {
        BuildFailure::ChecksumMismatch
    } else {
        BuildFailure::Other
    }
}

/// Fingerabdrücke aus `validpgpkeys`. Nur Hex-Zeichen, damit nichts als gpg-Option durchgeht.
fn valid_pgp_keys(srcinfo: &[String]) -> Vec<String> {
    srcinfo
        .iter()
        .filter_map(|l| l.trim().strip_prefix("validpgpkeys = "))
        .filter(|k| !k.is_empty() && k.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_string)
        .collect()
}

/// Baut ohne Vergleich der Quellen und gibt nur die Dateien zurück.
pub fn build(pkg: &AurPackage, dirs: &UserDirs, runner: &dyn Runner, on_line: &mut dyn FnMut(&str)) -> Result<Vec<PathBuf>> {
    build_checked(pkg, None, dirs, runner, on_line).map(|b| b.files)
}

/// Klont frisch und baut. Geprüft wurde `.SRCINFO` aus dem Netz, `makepkg` nimmt aber das PKGBUILD aus dem Klon:
/// Nennt es Domains, die die Prüfung nicht gesehen hat, wird nicht gebaut.
/// Gibt die Datei des Pakets selbst zurück, ohne Debug- und weitere Split-Pakete.
/// Jede Ausgabezeile geht an `on_line` und ins Log unter `<data>/logs/<pkgbase>-<unix>.log`.
pub fn build_checked(pkg: &AurPackage, checked: Option<&BTreeSet<String>>, dirs: &UserDirs, runner: &dyn Runner, on_line: &mut dyn FnMut(&str)) -> Result<Built> {
    let clone = dirs.cache.join("aur").join(&pkg.package_base);
    let pkgdest = dirs.cache.join("pkg");
    let logs = dirs.data.join("logs");
    if clone.exists() {
        std::fs::remove_dir_all(&clone)?;
    }
    for dir in [dirs.cache.join("aur"), pkgdest.clone(), logs.clone()] {
        std::fs::create_dir_all(dir)?;
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let log_path = logs.join(format!("{}-{stamp}.log", pkg.package_base));
    let mut log = std::fs::File::create(&log_path)?;
    let log_text = RefCell::new(String::new());
    let mut emit = |line: &str| {
        let _ = writeln!(log, "{line}");
        let mut text = log_text.borrow_mut();
        text.push_str(line);
        text.push('\n');
        on_line(line);
    };

    let url = format!("https://aur.archlinux.org/{}.git", pkg.package_base);
    let clone_str = clone.to_string_lossy().into_owned();
    let pkgdest_str = pkgdest.to_string_lossy().into_owned();
    let env = [("PKGDEST", pkgdest_str.as_str())];
    let failed = |kind| TermaError::BuildFailed { package: pkg.name.clone(), kind, log: log_path.clone() };

    if runner.run(&["git", "clone", "--depth", "1", &url, &clone_str], &[], None, &mut emit)? != 0 {
        return Err(failed(BuildFailure::Other));
    }
    let mut srcinfo = Vec::new();
    if runner.run(&["makepkg", "--printsrcinfo"], &env, Some(&clone), &mut |l| srcinfo.push(l.to_string()))? != 0 {
        return Err(failed(BuildFailure::Other));
    }
    let domains = source_domains(&parse_srcinfo(&srcinfo.join("\n")));
    if checked.is_some_and(|c| !domains.is_subset(c)) {
        return Err(failed(BuildFailure::SourcesChanged));
    }
    let makepkg = ["makepkg", "-f", "--noconfirm", "--cleanbuild", "--nocheck"];
    let mut code = runner.run(&makepkg, &env, Some(&clone), &mut emit)?;
    if code != 0 && classify_failure(&log_text.borrow()) == BuildFailure::PgpKeyUnknown {
        // Wie yay: Das PKGBUILD nennt den Schlüssel der Projektautoren, und makepkg akzeptiert nur ihn.
        // Geholt wird genau dieser Fingerabdruck, die Signaturprüfung bleibt an.
        let keys = valid_pgp_keys(&srcinfo);
        let mut gpg = vec!["gpg", "--batch", "--recv-keys"];
        gpg.extend(keys.iter().map(String::as_str));
        if !keys.is_empty() && runner.run(&gpg, &[], None, &mut emit)? == 0 {
            code = runner.run(&makepkg, &env, Some(&clone), &mut emit)?;
        }
    }
    if code != 0 {
        drop(emit);
        return Err(failed(classify_failure(&log_text.borrow())));
    }
    let mut listing = Vec::new();
    runner.run(&["makepkg", "--packagelist"], &env, Some(&clone), &mut |l| listing.push(l.to_string()))?;
    // `name-version-release-arch.pkg.tar.zst`. So fallen auch `name-debug` und andere Pakete der `pkgbase` weg.
    let files: Vec<PathBuf> = listing
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| {
            let file = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            file.ends_with(".pkg.tar.zst") && file.rsplitn(4, '-').nth(3) == Some(pkg.name.as_str())
        })
        .collect();
    if files.is_empty() {
        return Err(failed(BuildFailure::Other));
    }
    Ok(Built { files, source_domains: domains })
}
