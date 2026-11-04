//! Paketquellen über `pacman`. Abfragen laufen gegen eine eigene Datenbank im Cache, die ohne Root
//! frisch synchronisiert wird (wie `checkupdates`). Das System selbst bleibt unberührt.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::dirs::UserDirs;
use crate::error::{Result, TermaError};
use crate::runner::{LineTail, Runner};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub download_size: u64,
    pub installed_version: Option<String>,
    pub has_desktop_entry: Option<bool>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub name: String,
    pub new_version: String,
    pub download_size: u64,
}

pub struct PacmanDb {
    dbpath: PathBuf,
    runner: Arc<dyn Runner>,
}

const SYSTEM_LOCAL_DB: &str = "/var/lib/pacman/local";

impl PacmanDb {
    pub fn new(dirs: &UserDirs, runner: Arc<dyn Runner>) -> PacmanDb {
        PacmanDb { dbpath: dirs.cache.join("pacman-db"), runner }
    }

    fn dbpath(&self) -> String {
        self.dbpath.to_string_lossy().into_owned()
    }

    /// Lädt die Paketlisten frisch in die eigene Datenbank. Die Dateilisten (`-Fy`) sind optional.
    pub fn sync(&self) -> Result<()> {
        std::fs::create_dir_all(&self.dbpath)?;
        let local = self.dbpath.join("local");
        if std::fs::symlink_metadata(&local).is_err() {
            std::os::unix::fs::symlink(SYSTEM_LOCAL_DB, &local)?;
        }
        let db = self.dbpath();
        let mut tail = LineTail::default();
        // Ohne Sandbox: unter fakeroot kann pacman 7 nicht zum Download-Benutzer `alpm` wechseln
        // (wie `checkupdates`). Es läuft ohnehin nur als normaler Benutzer.
        let code = self.runner.run(
            &["fakeroot", "--", "pacman", "-Sy", "--dbpath", &db, "--logfile", "/dev/null", "--disable-sandbox"],
            &[],
            None,
            &mut |l| tail.push(l),
        )?;
        if code != 0 {
            return Err(TermaError::Command { code, tail: tail.text() });
        }
        self.runner.run(&["fakeroot", "--", "pacman", "-Fy", "--dbpath", &db, "--logfile", "/dev/null", "--disable-sandbox"], &[], None, &mut |_| {})?;
        Ok(())
    }

    fn query(&self, args: &[&str]) -> Result<(i32, String)> {
        let db = self.dbpath();
        let mut argv = vec!["pacman", "--dbpath", db.as_str()];
        argv.extend_from_slice(args);
        Ok(self.runner.output(&argv)?)
    }

    fn system(&self, args: &[&str]) -> Result<(i32, String)> {
        let mut argv = vec!["pacman"];
        argv.extend_from_slice(args);
        Ok(self.runner.output(&argv)?)
    }

    /// Sucht jeden Begriff einzeln (ODER-Verknüpfung) und ergänzt Größe und Desktop-Erkennung.
    pub fn search(&self, terms: &[String]) -> Result<Vec<RepoPackage>> {
        let mut found: Vec<RepoPackage> = Vec::new();
        for term in terms {
            let (code, out) = self.query(&["-Ss", term])?;
            if code != 0 {
                continue;
            }
            for pkg in parse_ss(&out) {
                if !found.iter().any(|p| p.name == pkg.name) {
                    found.push(pkg);
                }
            }
        }
        self.fill_details(&mut found)?;
        Ok(found)
    }

    pub fn info(&self, names: &[String]) -> Result<Vec<RepoPackage>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let mut args = vec!["-Si"];
        args.extend(names.iter().map(String::as_str));
        let (_, out) = self.query(&args)?;
        let installed = self.installed()?;
        let mut pkgs = parse_si(&out);
        for p in &mut pkgs {
            p.installed_version = installed.get(&p.name).cloned();
        }
        self.fill_details(&mut pkgs)?;
        Ok(pkgs)
    }

    fn fill_details(&self, pkgs: &mut [RepoPackage]) -> Result<()> {
        if pkgs.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        let mut si = vec!["-Si"];
        si.extend(&names);
        // Bei mehreren Quellen zählt die erste.
        let mut details: HashMap<String, RepoPackage> = HashMap::new();
        for p in parse_si(&self.query(&si)?.1) {
            details.entry(p.name.clone()).or_insert(p);
        }
        let mut fl = vec!["-Fl"];
        fl.extend(&names);
        let desktop = parse_fl_desktop(&self.query(&fl)?.1);
        for p in pkgs.iter_mut() {
            if let Some(d) = details.get(&p.name) {
                p.download_size = d.download_size;
                p.url = d.url.clone();
            }
            p.has_desktop_entry = desktop.get(&p.name).copied();
        }
        Ok(())
    }

    /// Was ein vollständiges Systemupdate laden würde.
    pub fn pending_updates(&self) -> Result<Vec<PendingUpdate>> {
        self.preview(&["-Sup"], &[])
    }

    /// Was `pacman -Syu <names>` laden würde: Updates, Ziele und neue Abhängigkeiten.
    pub fn install_preview(&self, names: &[String]) -> Result<Vec<PendingUpdate>> {
        self.preview(&["-Sup"], names)
    }

    /// Was `pacman -S --needed <names>` laden würde, also ohne Systemupdate.
    pub fn add_preview(&self, names: &[String]) -> Result<Vec<PendingUpdate>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        self.preview(&["-Sp", "--needed"], names)
    }

    fn preview(&self, operation: &[&'static str], names: &[String]) -> Result<Vec<PendingUpdate>> {
        let mut args = operation.to_vec();
        args.extend(["--print-format", "%n %v %s"]);
        args.extend(names.iter().map(String::as_str));
        let (code, out) = self.query(&args)?;
        if code != 0 {
            return Err(TermaError::Command { code, tail: out });
        }
        Ok(parse_print_format(&out))
    }

    pub fn installed(&self) -> Result<HashMap<String, String>> {
        let (_, out) = self.system(&["-Q"])?;
        Ok(out
            .lines()
            .filter_map(|l| l.split_once(' '))
            .map(|(n, v)| (n.to_string(), v.trim().to_string()))
            .collect())
    }

    pub fn foreign(&self) -> Result<HashSet<String>> {
        Ok(words(&self.system(&["-Qmq"])?.1))
    }

    pub fn explicit(&self) -> Result<HashSet<String>> {
        Ok(words(&self.system(&["-Qeq"])?.1))
    }

    /// Paket → Gruppen, aus `pacman -Qg` (Zeilen „gruppe paket“).
    pub fn groups(&self) -> Result<HashMap<String, Vec<String>>> {
        let (_, out) = self.system(&["-Qg"])?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (group, pkg) in out.lines().filter_map(|l| l.split_once(' ')) {
            map.entry(pkg.trim().to_string()).or_default().push(group.to_string());
        }
        Ok(map)
    }

    pub fn in_repos(&self, dep: &str) -> bool {
        matches!(self.query(&["-Sp", "--print-format", "%n", dep]), Ok((0, _)))
    }

    pub fn satisfied(&self, dep: &str) -> bool {
        matches!(self.system(&["-T", dep]), Ok((0, _)))
    }

    pub fn vercmp(&self, a: &str, b: &str) -> Ordering {
        match self.runner.output(&["vercmp", a, b]) {
            Ok((0, out)) => out.trim().parse::<i32>().map(|n| n.cmp(&0)).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        }
    }

    /// Pakete, nach deren Update ein Neustart nötig ist: alles, was Dateien unter `/usr/lib/modules`
    /// besitzt (Kernel, Kernel-Header, Kernel-Module wie der NVIDIA-Treiber).
    pub fn kernel_packages(&self) -> HashSet<String> {
        self.system(&["-Qqo", "/usr/lib/modules"]).map(|(_, out)| words(&out)).unwrap_or_default()
    }
}

fn words(text: &str) -> HashSet<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// `pacman -Ss`: Kopfzeile `repo/name version [(gruppen)] [[installed[: version]]]`, darunter eingerückt die Beschreibung.
pub fn parse_ss(text: &str) -> Vec<RepoPackage> {
    let mut pkgs: Vec<RepoPackage> = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            if let Some(last) = pkgs.last_mut() {
                if last.description.is_empty() {
                    last.description = line.trim().to_string();
                }
            }
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(repo_name), Some(version)) = (parts.next(), parts.next()) else { continue };
        let Some((repo, name)) = repo_name.split_once('/') else { continue };
        let installed_version = if let Some(start) = line.find("[installed: ") {
            line[start + 12..].split(']').next().map(str::to_string)
        } else if line.contains("[installed]") {
            Some(version.to_string())
        } else {
            None
        };
        pkgs.push(RepoPackage {
            name: name.to_string(),
            version: version.to_string(),
            description: String::new(),
            repo: repo.to_string(),
            download_size: 0,
            installed_version,
            has_desktop_entry: None,
            url: None,
        });
    }
    pkgs
}

/// `pacman -Si`: Blöcke mit `Schlüssel : Wert`, getrennt durch Leerzeilen.
pub fn parse_si(text: &str) -> Vec<RepoPackage> {
    let mut pkgs = Vec::new();
    for block in text.split("\n\n") {
        let mut fields: HashMap<&str, &str> = HashMap::new();
        for line in block.lines().filter(|l| !l.starts_with(char::is_whitespace)) {
            if let Some((k, v)) = line.split_once(':') {
                fields.entry(k.trim()).or_insert(v.trim());
            }
        }
        let Some(name) = fields.get("Name") else { continue };
        pkgs.push(RepoPackage {
            name: name.to_string(),
            version: fields.get("Version").unwrap_or(&"").to_string(),
            description: fields.get("Description").unwrap_or(&"").to_string(),
            repo: fields.get("Repository").unwrap_or(&"").to_string(),
            download_size: fields.get("Download Size").and_then(|s| parse_size(s)).unwrap_or(0),
            installed_version: None,
            has_desktop_entry: None,
            url: fields.get("URL").filter(|u| **u != "None").map(|u| u.to_string()),
        });
    }
    pkgs
}

/// `98.50 MiB` → Bytes (abgerundet).
pub fn parse_size(text: &str) -> Option<u64> {
    let (num, unit) = text.trim().split_once(' ')?;
    let factor: f64 = match unit.trim() {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let value: f64 = num.parse().ok()?;
    Some((value * factor).floor() as u64)
}

/// `pacman -Fl`: Zeilen `paket pfad`. Wahr, wenn das Paket eine `.desktop`-Datei ins Startmenü legt.
pub fn parse_fl_desktop(text: &str) -> HashMap<String, bool> {
    let mut map: HashMap<String, bool> = HashMap::new();
    for (pkg, path) in text.lines().filter_map(|l| l.split_once(' ')) {
        let is_entry = path.starts_with("usr/share/applications/") && path.ends_with(".desktop");
        *map.entry(pkg.to_string()).or_insert(false) |= is_entry;
    }
    map
}

/// `--print-format '%n %v %s'`: Zeilen `name version bytes`.
pub fn parse_print_format(text: &str) -> Vec<PendingUpdate> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let (name, version, size) = (parts.next()?, parts.next()?, parts.next()?);
            Some(PendingUpdate { name: name.to_string(), new_version: version.to_string(), download_size: size.parse().ok()? })
        })
        .collect()
}
