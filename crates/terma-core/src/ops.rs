//! Fassade für die App: suchen, planen, installieren, entfernen, Einstellungen.
//! Alles mit Root-Rechten läuft über die beiden Helfer und `pkexec`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::appimage;
use crate::appimage_update;
use crate::aur::{source_domains, AurClient, AurPackage, AurRecord, AurStateStore};
use crate::aur_build::{self, AurBuildPlan, SystemDeps};
use crate::dirs::UserDirs;
use crate::error::{BuildFailure, Result, TermaError};
use crate::file_flow::{detect_kind, name_from_filename, FileInspection, FileKind};
use crate::flatpak::{self, FlatpakDuplicate};
use crate::http::{Http, UreqHttp};
use crate::integrate::{self, IntegratedApp};
use crate::pacman::{PacmanDb, PendingUpdate};
use crate::programs::{self, Program, ProgramInputs, UpdateState};
use crate::runner::{LineTail, Runner, SystemRunner};
use crate::search::{normalize, query_terms, rank, without_parentheses, Candidate, SearchResult, Source};
use crate::trust::{self, TrustReason};

pub use crate::settings::Settings;

pub const HELPER: &str = "/usr/lib/terma/terma-helper";
pub const HELPER_AUR: &str = "/usr/lib/terma/terma-helper-aur";
const AUR_BASE: &str = "https://aur.archlinux.org";
/// So alt darf die eigene Paketdatenbank für die Suche sein.
const SEARCH_DB_MAX_AGE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Step {
    Refreshing,
    UpdatingSystem,
    Building { package: String },
    Installing { package: String },
    SettingUpAppImage { name: String },
    Removing { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Progress {
    Step(Step),
    Line(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemUpdate {
    pub package_count: usize,
    pub download_bytes: u64,
    pub includes_kernel: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageWarning {
    pub package: String,
    pub reasons: Vec<TrustReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub candidate: Candidate,
    pub system_update: Option<SystemUpdate>,
    pub aur: Option<AurBuildPlan>,
    pub warnings: Vec<PackageWarning>,
    pub from_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallOutcome {
    pub restart_recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overview {
    pub pending_count: usize,
    pub programs: Vec<Program>,
    pub flatpak_duplicates: Vec<FlatpakDuplicate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkippedUpdate {
    pub package: String,
    pub reasons: Vec<TrustReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FailedUpdate {
    pub package: String,
    pub kind: BuildFailure,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UpdateReport {
    pub skipped: Vec<SkippedUpdate>,
    pub failed: Vec<FailedUpdate>,
    pub appimages_updated: Vec<String>,
    pub appimages_failed: Vec<String>,
    /// Das AUR antwortete nicht: Über dessen Pakete lässt sich nichts sagen.
    pub aur_unreachable: bool,
    pub restart_recommended: bool,
}

pub struct Terma {
    pub(crate) runner: Arc<dyn Runner>,
    pub(crate) db: PacmanDb,
    pub(crate) aur: AurClient,
    pub(crate) http: Arc<dyn Http>,
    pub(crate) dirs: UserDirs,
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Systemupdate aus einer Vorschau: nur Pakete, die schon installiert sind, zählen als Update.
fn summarize(preview: &[PendingUpdate], installed: &HashMap<String, String>, kernels: &HashSet<String>) -> Option<SystemUpdate> {
    let upgrades: Vec<&PendingUpdate> = preview.iter().filter(|p| installed.contains_key(&p.name)).collect();
    if upgrades.is_empty() {
        return None;
    }
    Some(SystemUpdate {
        package_count: upgrades.len(),
        download_bytes: preview.iter().map(|p| p.download_size).sum(),
        includes_kernel: upgrades.iter().any(|p| kernels.contains(&p.name)),
    })
}

impl Terma {
    pub fn new(dirs: UserDirs) -> Terma {
        let http: Arc<dyn Http> = Arc::new(UreqHttp::new());
        Terma::with(Arc::new(SystemRunner), AurClient::new(AUR_BASE, http.clone()), http, dirs)
    }

    pub fn with(runner: Arc<dyn Runner>, aur: AurClient, http: Arc<dyn Http>, dirs: UserDirs) -> Terma {
        Terma { db: PacmanDb::new(&dirs, runner.clone()), runner, aur, http, dirs }
    }

    pub fn dirs(&self) -> &UserDirs {
        &self.dirs
    }

    pub fn refresh(&self) -> Result<()> {
        self.db.sync()
    }

    fn ensure_fresh(&self) -> Result<()> {
        let fresh = std::fs::metadata(self.dirs.cache.join("pacman-db/sync"))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < SEARCH_DB_MAX_AGE);
        if fresh {
            Ok(())
        } else {
            self.refresh()
        }
    }

    pub fn search(&self, q: &str) -> Result<SearchResult> {
        let terms = query_terms(q);
        if terms.is_empty() {
            return Ok(SearchResult { recommended: None, others: Vec::new(), aur_unavailable: false });
        }
        self.ensure_fresh()?;
        let repo = self.db.search(&terms)?;
        let foreign = self.db.foreign().unwrap_or_default();
        let mut result = rank(q, repo, self.search_aur(&terms), &foreign);
        for c in result.recommended.iter_mut().chain(result.others.iter_mut()) {
            if matches!(c.source, Source::Repo { .. }) {
                c.icon = programs::appstream_icon(&c.name, Path::new(programs::APPSTREAM_ICONS));
            }
        }
        Ok(result)
    }

    /// Fehlt das AUR bei allen Begriffen, gilt es als nicht erreichbar.
    fn search_aur(&self, terms: &[String]) -> Result<Vec<AurPackage>> {
        let mut all: Vec<AurPackage> = Vec::new();
        let mut reached = false;
        for term in terms {
            if let Ok(found) = self.aur.search(term) {
                reached = true;
                for p in found {
                    if !all.iter().any(|a| a.name == p.name) {
                        all.push(p);
                    }
                }
            }
        }
        if reached {
            Ok(all)
        } else {
            Err(TermaError::AurUnavailable)
        }
    }

    pub fn plan_install(&self, c: &Candidate, from_file: Option<&Path>) -> Result<InstallPlan> {
        self.refresh()?;
        let installed = self.db.installed()?;
        let kernels = self.db.kernel_packages();
        let (aur, warnings, preview_targets) = match &c.source {
            Source::Repo { .. } => (None, Vec::new(), vec![c.name.clone()]),
            Source::Aur => {
                let mut plan = aur_build::resolve(&[c.name.clone()], &SystemDeps { db: &self.db, aur: &self.aur })?;
                let now = unix_now();
                let mut warnings = Vec::new();
                let mut checked = BTreeMap::new();
                for p in &plan.build_order {
                    let src = self.aur.srcinfo(&p.package_base)?;
                    checked.insert(p.name.clone(), source_domains(&src));
                    let reasons = trust::evaluate(p, &src, now);
                    if !reasons.is_empty() {
                        warnings.push(PackageWarning { package: p.name.clone(), reasons });
                    }
                }
                plan.checked_sources = checked;
                let deps = plan.repo_deps.clone();
                (Some(plan), warnings, deps)
            }
            Source::AppImage => return Err(TermaError::UnsupportedFile),
        };
        // Kommen nur neue Pakete dazu, bleibt das System, wie es ist. Sonst läuft das Systemupdate mit.
        let added = self.db.add_preview(&preview_targets)?;
        let preview = if added.iter().any(|p| installed.contains_key(&p.name)) { self.db.install_preview(&preview_targets)? } else { added };
        Ok(InstallPlan {
            candidate: c.clone(),
            system_update: summarize(&preview, &installed, &kernels),
            aur,
            warnings,
            from_file: from_file.map(Path::to_path_buf),
        })
    }

    pub fn install(&self, plan: &InstallPlan, progress: &mut dyn FnMut(Progress)) -> Result<InstallOutcome> {
        match &plan.aur {
            None => {
                progress(Progress::Step(Step::Installing { package: plan.candidate.name.clone() }));
                self.helper(HELPER, &["install", &plan.candidate.name], progress)?;
            }
            Some(aur) => self.install_aur(aur, plan.system_update.is_some(), None, progress)?,
        }
        if let Some(file) = &plan.from_file {
            integrate::retire_entries_for_file(file, &self.dirs, self.runner.as_ref())?;
        }
        Ok(InstallOutcome { restart_recommended: plan.system_update.as_ref().is_some_and(|s| s.includes_kernel) })
    }

    /// Repo-Abhängigkeiten zuerst, dann jedes Paket bauen und sofort installieren, damit spätere Pakete
    /// es beim Bauen finden. Mit `update_system` läuft das Systemupdate im ersten Aufruf mit.
    /// Mit `failed` hält ein gescheiterter Bau den Rest nicht auf und wird dort vermerkt.
    pub(crate) fn install_aur(
        &self,
        aur: &AurBuildPlan,
        update_system: bool,
        mut failed: Option<&mut Vec<FailedUpdate>>,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        let mut upgraded = !update_system;
        if !aur.repo_deps.is_empty() {
            progress(Progress::Step(if update_system { Step::UpdatingSystem } else { Step::Installing { package: aur.repo_deps.join(", ") } }));
            let mut args = vec![if update_system { "install-deps" } else { "add-deps" }];
            args.extend(aur.repo_deps.iter().map(String::as_str));
            self.helper(HELPER_AUR, &args, progress)?;
            upgraded = true;
        }
        let mut store = AurStateStore::load(&self.dirs);
        for pkg in &aur.build_order {
            progress(Progress::Step(Step::Building { package: pkg.name.clone() }));
            let checked = aur.checked_sources.get(&pkg.name);
            let built = match (aur_build::build_checked(pkg, checked, &self.dirs, self.runner.as_ref(), &mut |l| progress(Progress::Line(l.to_string()))), failed.as_deref_mut()) {
                (Err(TermaError::BuildFailed { package, kind, .. }), Some(failed)) => {
                    failed.push(FailedUpdate { package, kind });
                    continue;
                }
                (built, _) => built?,
            };
            progress(Progress::Step(Step::Installing { package: pkg.name.clone() }));
            let files: Vec<String> = built.files.iter().map(|f| f.to_string_lossy().into_owned()).collect();
            let mut args = vec!["install-files"];
            if !upgraded {
                args.push("--upgrade");
                upgraded = true;
            }
            args.extend(files.iter().map(String::as_str));
            self.helper(HELPER_AUR, &args, progress)?;
            store.put(&pkg.name, AurRecord { maintainer: pkg.maintainer.clone(), source_domains: built.source_domains });
            store.save()?;
        }
        if !upgraded {
            progress(Progress::Step(Step::UpdatingSystem));
            self.helper(HELPER, &["upgrade"], progress)?;
        }
        Ok(())
    }

    pub fn setup_appimage(&self, path: &Path) -> Result<IntegratedApp> {
        integrate::integrate_appimage(&appimage::inspect(path)?, &self.dirs, self.runner.as_ref())
    }

    pub fn remove(&self, p: &Program, progress: &mut dyn FnMut(Progress)) -> Result<()> {
        progress(Progress::Step(Step::Removing { name: p.name.clone() }));
        if p.source == Source::AppImage {
            return integrate::remove_appimage(&p.desktop_file, &self.dirs, self.runner.as_ref());
        }
        let package = p.package.as_deref().ok_or_else(|| TermaError::Io(format!("{} gehört zu keinem Paket", p.name)))?;
        self.helper(HELPER, &["remove", package], progress)
    }

    /// Was beim Entfernen zusätzlich mitgeht: Pakete, die vom Programm abhängen (z. B. Sprachpakete).
    /// Probelauf ohne Root, wie ihn der Helfer vor dem Entfernen macht.
    pub fn remove_preview(&self, p: &Program) -> Result<Vec<String>> {
        let Some(package) = p.package.as_deref().filter(|_| p.source != Source::AppImage) else { return Ok(Vec::new()) };
        let (code, out) = self.runner.output(&["pacman", "-Rsc", "--print", "--print-format", "%n", "--", package])?;
        if code != 0 {
            return Err(TermaError::Command { code, tail: out });
        }
        Ok(out.split_whitespace().filter(|n| *n != package).map(str::to_string).collect())
    }

    pub fn remove_flatpak(&self, d: &FlatpakDuplicate) -> Result<()> {
        flatpak::uninstall(self.runner.as_ref(), d)
    }

    /// Beim ersten Start: Terma wird Standardprogramm für AppImages, damit der Doppelklick hier landet.
    /// Erkennbar am Fehlen der Einstellungsdatei, die danach angelegt wird.
    pub fn first_start(&self) -> Result<()> {
        if self.dirs.config.join("settings.json").exists() {
            return Ok(());
        }
        let argv = ["xdg-mime", "default", "terma.desktop", "application/vnd.appimage", "application/x-iso9660-appimage"];
        self.runner.run(&argv, &[], None, &mut |_| {})?;
        Settings::default().save(&self.dirs)
    }

    pub fn settings(&self) -> Settings {
        Settings::load(&self.dirs)
    }

    pub fn set_passwordless(&self, on: bool) -> Result<()> {
        self.helper(HELPER, &["passwordless", if on { "on" } else { "off" }], &mut |_| {})?;
        Settings { passwordless: on }.save(&self.dirs)
    }

    /// Ruft einen Helfer über `pkexec` auf und übersetzt die Exit-Codes.
    pub(crate) fn helper(&self, helper: &str, args: &[&str], progress: &mut dyn FnMut(Progress)) -> Result<()> {
        let mut argv = vec!["pkexec", helper];
        argv.extend_from_slice(args);
        let mut tail = LineTail::default();
        let mut protected = None;
        let code = self.runner.run(&argv, &[], None, &mut |line| {
            if let Some(pkg) = line.strip_prefix("PROTECTED ") {
                protected = Some(pkg.trim().to_string());
            }
            tail.push(line);
            progress(Progress::Line(line.to_string()));
        })?;
        match code {
            0 => Ok(()),
            126 | 127 => Err(TermaError::Cancelled),
            3 => Err(TermaError::DbLocked),
            4 => Err(TermaError::Protected { package: protected.unwrap_or_default() }),
            _ => Err(TermaError::Command { code, tail: tail.text() }),
        }
    }
}

impl Terma {
    /// Startseite: synchronisiert zuerst, dann Updates, Programme und doppelte Flatpaks.
    pub fn overview(&self) -> Result<Overview> {
        self.refresh()?;
        let repo_pending: Vec<String> = self.db.pending_updates()?.into_iter().map(|p| p.name).collect();
        let aur_pending: Vec<String> = self.aur_updates()?.unwrap_or_default().into_iter().map(|p| p.name).collect();
        let pending = repo_pending.iter().chain(&aur_pending).cloned().collect();
        let inputs = programs::gather(&self.db, &self.dirs, self.runner.as_ref(), pending)?;
        let mut list = programs::assemble(inputs, &|icon| programs::resolve_icon(icon, &self.dirs));
        programs::appimage_updates(&mut list, self.http.as_ref());
        let appimage_pending = list.iter().filter(|p| p.source == Source::AppImage && p.update == UpdateState::Ready).count();
        let flatpak_duplicates = flatpak::duplicates(&list, &flatpak::list_apps(self.runner.as_ref()));
        Ok(Overview { pending_count: repo_pending.len() + aur_pending.len() + appimage_pending, programs: list, flatpak_duplicates })
    }

    /// Installierte AUR-Pakete, für die das AUR eine neuere Version hat. `None`: Das AUR antwortet nicht.
    fn aur_updates(&self) -> Result<Option<Vec<AurPackage>>> {
        // Terma selbst kommt von der eigenen Website. Ein gleichnamiges AUR-Paket ist nie sein Update.
        let foreign: Vec<String> = self.db.foreign()?.into_iter().filter(|name| name != "terma").collect();
        if foreign.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let installed = self.db.installed()?;
        let Ok(remote) = self.aur.info(&foreign) else { return Ok(None) };
        Ok(Some(
            remote
                .into_iter()
                .filter(|p| installed.get(&p.name).is_some_and(|local| self.db.vercmp(local, &p.version).is_lt()))
                .collect(),
        ))
    }

    /// Vollständiges Systemupdate samt AUR und AppImages. AUR-Pakete werden vorher nachgeprüft;
    /// was auffällt, wird übersprungen und im Bericht erklärt.
    pub fn update_all(&self, progress: &mut dyn FnMut(Progress)) -> Result<UpdateReport> {
        progress(Progress::Step(Step::Refreshing));
        self.refresh()?;
        let kernels = self.db.kernel_packages();
        let restart_recommended = self.db.pending_updates()?.iter().any(|p| kernels.contains(&p.name));

        let mut store = AurStateStore::load(&self.dirs);
        let now = unix_now();
        let mut skipped = Vec::new();
        let mut failed = Vec::new();
        let mut to_build = Vec::new();
        let mut checked = BTreeMap::new();
        let updates = self.aur_updates()?;
        let mut aur_unreachable = updates.is_none();
        for pkg in updates.unwrap_or_default() {
            let Ok(src) = self.aur.srcinfo(&pkg.package_base) else {
                aur_unreachable = true;
                continue;
            };
            let mut reasons = match store.get(&pkg.name) {
                Some(old) => trust::recheck(old, &pkg, &src),
                // Noch nie geprüft, etwa ein Fremdpaket, dessen Name im AUR neu belegt wurde: die volle Regel.
                None => trust::evaluate(&pkg, &src, now),
            };
            if reasons.is_empty() {
                let plan = aur_build::resolve(std::slice::from_ref(&pkg.name), &SystemDeps { db: &self.db, aur: &self.aur })?;
                for dep in plan.build_order.iter().filter(|d| d.name != pkg.name && store.get(&d.name).is_none()) {
                    match self.aur.srcinfo(&dep.package_base) {
                        Ok(s) if trust::evaluate(dep, &s, now).is_empty() => {
                            checked.insert(dep.name.clone(), source_domains(&s));
                        }
                        _ => reasons.push(TrustReason::RiskyDependency { package: dep.name.clone() }),
                    }
                }
            }
            if reasons.is_empty() {
                if store.get(&pkg.name).is_none() {
                    store.put(&pkg.name, AurRecord { maintainer: pkg.maintainer.clone(), source_domains: source_domains(&src) });
                    store.save()?;
                }
                checked.insert(pkg.name.clone(), source_domains(&src));
                to_build.push(pkg.name);
            } else {
                skipped.push(SkippedUpdate { package: pkg.name, reasons });
            }
        }

        if to_build.is_empty() {
            progress(Progress::Step(Step::UpdatingSystem));
            self.helper(HELPER, &["upgrade"], progress)?;
        } else {
            let mut plan = aur_build::resolve(&to_build, &SystemDeps { db: &self.db, aur: &self.aur })?;
            plan.checked_sources = checked;
            self.install_aur(&plan, true, Some(&mut failed), progress)?;
        }

        let (appimages_updated, appimages_failed) = self.update_appimages(progress);
        Ok(UpdateReport { skipped, failed, appimages_updated, appimages_failed, aur_unreachable, restart_recommended })
    }

    /// „Trotzdem aktualisieren“: baut die genannten AUR-Pakete ohne Nachprüfung und merkt sich den neuen Stand.
    pub fn force_update_aur(&self, names: &[String], progress: &mut dyn FnMut(Progress)) -> Result<()> {
        let plan = aur_build::resolve(names, &SystemDeps { db: &self.db, aur: &self.aur })?;
        self.install_aur(&plan, true, None, progress)
    }

    /// Aktualisiert eingebundene AppImages. Fehler bei einzelnen Dateien halten den Rest nicht auf.
    /// Gibt die aktualisierten und die gescheiterten Namen zurück.
    fn update_appimages(&self, progress: &mut dyn FnMut(Progress)) -> (Vec<String>, Vec<String>) {
        let inputs = ProgramInputs { user_entries: programs::read_entries(&self.dirs.applications), ..Default::default() };
        let mut updated = Vec::new();
        let mut failed = Vec::new();
        for program in programs::assemble(inputs, &|_| None) {
            let Ok(text) = std::fs::read_to_string(&program.desktop_file) else { continue };
            let entry = crate::desktop::DesktopEntry::parse(&text);
            let Some(path) = entry.get("X-Terma-Path").map(str::to_string).or_else(|| entry.exec_program()) else { continue };
            let Ok(info) = appimage::inspect(Path::new(&path)) else { continue };
            let remote = match appimage_update::check(&info, self.http.as_ref()) {
                Ok(Some(remote)) => remote,
                Ok(None) => continue,
                Err(_) => {
                    failed.push(program.name);
                    continue;
                }
            };
            progress(Progress::Step(Step::SettingUpAppImage { name: program.name.clone() }));
            if appimage_update::apply(&remote, &self.dirs, self.http.as_ref(), self.runner.as_ref()).is_ok() {
                updated.push(program.name);
            } else {
                failed.push(program.name);
            }
        }
        (updated, failed)
    }

    /// Liest die Datei, ohne sie auszuführen, und sucht das Programm in den Quellen.
    /// Ein Treffer zählt nur, wenn der Name normalisiert genau passt.
    pub fn inspect_file(&self, path: &Path) -> Result<FileInspection> {
        let kind = detect_kind(path);
        let (app_name, app_version, icon, self_updating) = if kind == FileKind::AppImage {
            let info = appimage::inspect(path)?;
            let icon = match &info.icon {
                Some(data) => {
                    let dir = self.dirs.cache.join("preview");
                    std::fs::create_dir_all(&dir)?;
                    let file = dir.join(format!("{}.{}", info.desktop_file_name.trim_end_matches(".desktop"), data.ext));
                    std::fs::write(&file, &data.bytes)?;
                    Some(file)
                }
                None => None,
            };
            (info.name, info.version, icon, info.self_updating)
        } else {
            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            (name_from_filename(&file_name), None, None, false)
        };
        let wanted = normalize(&app_name);
        let source_match = self.search(&without_parentheses(&app_name)).ok().and_then(|r| {
            r.recommended.into_iter().chain(r.others).find(|c| normalize(&c.name) == wanted)
        });
        Ok(FileInspection { path: path.to_path_buf(), kind, app_name, app_version, icon, source_match, self_updating })
    }
}
