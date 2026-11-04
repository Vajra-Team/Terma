//! „Deine Programme“: von Hand installierte Programme mit Startmenü-Eintrag, aus jeder Quelle.
//! Was zum System oder zum Desktop gehört, erscheint nicht und lässt sich nicht entfernen.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::appimage;
use crate::appimage_update;
use crate::desktop::DesktopEntry;
use crate::dirs::UserDirs;
use crate::error::Result;
use crate::http::Http;
use crate::pacman::PacmanDb;
use crate::runner::Runner;
use crate::search::Source;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UpdateState {
    None,
    Ready,
    SelfUpdating,
    NotPossible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub icon: Option<PathBuf>,
    pub source: Source,
    pub update: UpdateState,
    pub desktop_file: PathBuf,
    pub package: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProgramInputs {
    pub system_entries: Vec<(PathBuf, DesktopEntry)>,
    pub user_entries: Vec<(PathBuf, DesktopEntry)>,
    pub owners: HashMap<PathBuf, String>,
    pub explicit: HashSet<String>,
    pub foreign: HashSet<String>,
    pub groups: HashMap<String, Vec<String>>,
    pub pending: HashSet<String>,
}

const PROTECTED_GROUPS: &[&str] = &["base", "base-devel", "cachyos", "plasma", "kde-system", "kf6", "qt5", "qt6", "xorg"];
const SYSTEM_APPLICATIONS: &str = "/usr/share/applications";

/// CachyOS-eigene Werkzeuge (`cachyos-…`, `cachy-update` …) und Bootloader-Werkzeuge (`limine-…`) gehören zum System
/// (Jamie, 30.10.2026: Cachy-Update und Limine-snapper-restore müssen nicht in „Deine Programme“).
const PROTECTED_PREFIXES: &[&str] = &["cachy", "limine"];

/// Ohne sie startet das System nicht mehr oder lässt sich nicht mehr verwalten. `base` ist unter Arch keine Gruppe mehr.
const PROTECTED_PACKAGES: &[&str] = &[
    "base", "base-devel", "bash", "coreutils", "dracut", "efibootmgr", "filesystem", "firewalld", "glibc", "grub", "linux-api-headers",
    "linux-firmware", "mkinitcpio", "networkmanager", "pacman", "polkit", "sddm", "sudo", "systemd", "terma", "ufw",
];

pub fn is_protected(pkg: &str, groups: &[String]) -> bool {
    PROTECTED_PREFIXES.iter().any(|p| pkg.starts_with(p))
        || PROTECTED_PACKAGES.contains(&pkg)
        || groups.iter().any(|g| PROTECTED_GROUPS.contains(&g.as_str()))
}

fn visible(entry: &DesktopEntry) -> bool {
    entry.get("NoDisplay") != Some("true") && entry.get("Hidden") != Some("true") && entry.get("Type").is_none_or(|t| t == "Application")
}

/// Systemwerkzeuge und Einstellungen (Kategorien `System`, `Settings`) gehören nicht zu „Deine Programme“.
fn system_tool(entry: &DesktopEntry) -> bool {
    entry.get("Categories").is_some_and(|c| c.split(';').any(|cat| cat == "System" || cat == "Settings"))
}

fn display_name(entry: &DesktopEntry, fallback: &Path) -> String {
    entry
        .get("Name[de]")
        .or_else(|| entry.get("Name"))
        .map(str::to_string)
        .unwrap_or_else(|| fallback.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
}

fn is_appimage_path(p: &str) -> bool {
    p.to_lowercase().ends_with(".appimage")
}

/// Baut die Liste. `resolve_icon` macht aus dem `Icon`-Wert eines Eintrags einen Dateipfad.
pub fn assemble(inputs: ProgramInputs, resolve_icon: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<Program> {
    let mut programs = Vec::new();
    let mut seen_packages = HashSet::new();
    let mut system = inputs.system_entries;
    system.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, entry) in system {
        let Some(owner) = inputs.owners.get(&path) else { continue };
        let groups = inputs.groups.get(owner).map(Vec::as_slice).unwrap_or(&[]);
        if !visible(&entry) || system_tool(&entry) || !inputs.explicit.contains(owner) || is_protected(owner, groups) || !seen_packages.insert(owner.clone()) {
            continue;
        }
        programs.push(Program {
            name: display_name(&entry, &path),
            icon: entry.get("Icon").and_then(resolve_icon),
            source: if inputs.foreign.contains(owner) { Source::Aur } else { Source::Repo { repo: String::new() } },
            update: if inputs.pending.contains(owner) { UpdateState::Ready } else { UpdateState::None },
            desktop_file: path,
            package: Some(owner.clone()),
        });
    }

    let mut seen_appimages = HashSet::new();
    for (path, entry) in inputs.user_entries {
        let program = entry.get("X-Terma-Path").map(str::to_string).or_else(|| entry.exec_program());
        let Some(program) = program.filter(|p| is_appimage_path(p)) else { continue };
        if !visible(&entry) || !seen_appimages.insert(program) {
            continue;
        }
        programs.push(Program {
            name: display_name(&entry, &path),
            icon: entry.get("Icon").and_then(resolve_icon),
            source: Source::AppImage,
            update: UpdateState::None,
            desktop_file: path,
            package: None,
        });
    }

    programs.sort_by_key(|p| p.name.to_lowercase());
    programs
}

pub(crate) fn read_entries(dir: &Path) -> Vec<(PathBuf, DesktopEntry)> {
    let Ok(read_dir) = std::fs::read_dir(dir) else { return Vec::new() };
    read_dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
        .filter_map(|p| std::fs::read_to_string(&p).ok().map(|t| (p, DesktopEntry::parse(&t))))
        .collect()
}

/// `pacman -Qo`: Zeilen `<pfad> is owned by <paket> <version>`.
pub fn parse_owners(text: &str) -> HashMap<PathBuf, String> {
    text.lines()
        .filter_map(|l| l.split_once(" is owned by "))
        .filter_map(|(path, rest)| Some((PathBuf::from(path), rest.split_whitespace().next()?.to_string())))
        .collect()
}

/// `pacman -Qg`: Zeilen `<gruppe> <paket>` → Paket → Gruppen.
pub fn parse_groups(text: &str) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (group, pkg) in text.lines().filter_map(|l| l.split_once(' ')) {
        map.entry(pkg.trim().to_string()).or_default().push(group.to_string());
    }
    map
}

pub fn gather(db: &PacmanDb, dirs: &UserDirs, runner: &dyn Runner, pending: HashSet<String>) -> Result<ProgramInputs> {
    let system_entries = read_entries(Path::new(SYSTEM_APPLICATIONS));
    let files: Vec<String> = system_entries.iter().map(|(p, _)| p.to_string_lossy().into_owned()).collect();
    let mut argv = vec!["pacman", "-Qo"];
    argv.extend(files.iter().map(String::as_str));
    let owners = if files.is_empty() { HashMap::new() } else { parse_owners(&runner.output(&argv)?.1) };
    Ok(ProgramInputs {
        system_entries,
        user_entries: read_entries(&dirs.applications),
        owners,
        explicit: db.explicit()?,
        foreign: db.foreign()?,
        groups: db.groups()?,
        pending,
    })
}

/// Fragt für jedes AppImage-Programm nach Updates. Ohne Netz bleibt der Zustand `None`.
pub fn appimage_updates(programs: &mut [Program], http: &dyn Http) {
    for program in programs.iter_mut().filter(|p| p.source == Source::AppImage) {
        let Ok(text) = std::fs::read_to_string(&program.desktop_file) else { continue };
        let entry = DesktopEntry::parse(&text);
        let Some(path) = entry.get("X-Terma-Path").map(str::to_string).or_else(|| entry.exec_program()) else { continue };
        let Ok(info) = appimage::inspect(Path::new(&path)) else { continue };
        program.update = if info.self_updating {
            UpdateState::SelfUpdating
        } else if info.update_info.is_none() {
            UpdateState::NotPossible
        } else {
            match appimage_update::check(&info, http) {
                Ok(Some(_)) => UpdateState::Ready,
                _ => UpdateState::None,
            }
        };
    }
}

/// Icons aus `archlinux-appstream-data`: `<quelle>/<größe>/<paket>_<icon>.<endung>` (meist JPEG XL).
pub const APPSTREAM_ICONS: &str = "/usr/share/swcatalog/icons";
const APPSTREAM_SIZES: &[&str] = &["128x128", "64x64"];

/// Icon eines offiziellen Pakets. Bringt ein Paket mehrere Programme mit, gewinnt das Icon mit dem
/// Paketnamen, sonst das erste nach Namen.
pub fn appstream_icon(pkg: &str, root: &Path) -> Option<PathBuf> {
    let prefix = format!("{pkg}_");
    let sources = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect::<Vec<_>>();
    APPSTREAM_SIZES.iter().find_map(|size| {
        let mut found: Vec<PathBuf> = sources
            .iter()
            .filter_map(|source| std::fs::read_dir(source.join(size)).ok())
            .flat_map(|dir| dir.flatten().map(|e| e.path()))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix)))
            .collect();
        found.sort();
        let own = found.iter().position(|p| p.file_stem().and_then(|n| n.to_str()) == Some(&format!("{pkg}_{pkg}")));
        own.map(|i| found.swap_remove(i)).or_else(|| found.into_iter().next())
    })
}

const ICON_SIZES: &[&str] = &["scalable", "512x512", "256x256", "128x128", "64x64", "48x48"];
/// Breeze legt App-Icons als `apps/<größe>/<name>.svg` ab (KDE-Standard, z. B. Konsole, KCalc).
const BREEZE_SIZES: &[&str] = &["48", "64", "32"];

pub fn resolve_icon(icon: &str, dirs: &UserDirs) -> Option<PathBuf> {
    let direct = Path::new(icon);
    if direct.is_absolute() {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    let roots = [dirs.home.join(".local/share/icons"), PathBuf::from("/usr/share/icons")];
    let hicolor = roots.iter().flat_map(|root| {
        ICON_SIZES.iter().flat_map(move |size| ["svg", "png"].map(|ext| root.join("hicolor").join(size).join("apps").join(format!("{icon}.{ext}"))))
    });
    let breeze = roots
        .iter()
        .flat_map(|root| BREEZE_SIZES.iter().map(move |size| root.join("breeze/apps").join(size).join(format!("{icon}.svg"))));
    let pixmaps = ["svg", "png"].map(|ext| PathBuf::from("/usr/share/pixmaps").join(format!("{icon}.{ext}")));
    hicolor.chain(breeze).chain(pixmaps).find(|p| p.is_file())
}
