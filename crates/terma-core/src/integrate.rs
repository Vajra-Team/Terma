//! AppImages einrichten: Datei nach `~/Applications`, Startmenü-Eintrag und Icon anlegen,
//! alte Einträge desselben Programms ins Backup verschieben statt sie zu verdoppeln.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::appimage::AppImageInfo;
use crate::desktop::DesktopEntry;
use crate::dirs::UserDirs;
use crate::error::Result;
use crate::runner::Runner;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegratedApp {
    pub name: String,
    pub appimage: PathBuf,
    pub desktop_file: PathBuf,
    pub icon: Option<PathBuf>,
}

/// Errno für „anderes Dateisystem“ beim Umbenennen.
const EXDEV: i32 = 18;

pub fn integrate_appimage(info: &AppImageInfo, dirs: &UserDirs, runner: &dyn Runner) -> Result<IntegratedApp> {
    std::fs::create_dir_all(&dirs.appimages)?;
    std::fs::create_dir_all(&dirs.applications)?;
    let target = target_path(info, dirs);

    let retired = retire(dirs, |file_name, entry| {
        file_name == info.desktop_file_name || entry.exec_program().is_some_and(|p| Path::new(&p) == info.path || Path::new(&p) == target)
    })?;
    let mut mime: Vec<String> = info.desktop.mime_types();
    for (_, old) in &retired {
        for m in old.mime_types() {
            if !mime.contains(&m) {
                mime.push(m);
            }
        }
    }

    if info.path != target {
        move_file(&info.path, &target)?;
    }
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))?;

    let id = info.desktop_file_name.trim_end_matches(".desktop");
    let icon = match &info.icon {
        Some(data) => {
            let dir = dirs.data.join("icons");
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{id}.{}", data.ext));
            std::fs::write(&path, &data.bytes)?;
            Some(path)
        }
        None => None,
    };

    let target_str = target.to_string_lossy().into_owned();
    let mut entry = info.desktop.clone();
    entry.set_exec_program(&target);
    entry.remove("NoDisplay");
    entry.set("TryExec", &target_str);
    if let Some(icon) = &icon {
        entry.set("Icon", &icon.to_string_lossy());
    }
    if !mime.is_empty() {
        entry.set("MimeType", &format!("{};", mime.join(";")));
    }
    entry.set("X-Terma-Source", "appimage");
    entry.set("X-Terma-Path", &target_str);
    let desktop_file = dirs.applications.join(&info.desktop_file_name);
    std::fs::write(&desktop_file, entry.to_string())?;
    refresh_menu(dirs, runner);

    Ok(IntegratedApp { name: info.name.clone(), appimage: target, desktop_file, icon })
}

/// Verschiebt alle Startmenü-Einträge, die `file` starten, ins Backup.
pub fn retire_entries_for_file(file: &Path, dirs: &UserDirs, runner: &dyn Runner) -> Result<Vec<PathBuf>> {
    let retired = retire(dirs, |_, entry| entry.exec_program().is_some_and(|p| Path::new(&p) == file))?;
    if !retired.is_empty() {
        refresh_menu(dirs, runner);
    }
    Ok(retired.into_iter().map(|(path, _)| path).collect())
}

/// Entfernt Eintrag, AppImage und Icon. Gelöscht werden nur Dateien mit Endung `.AppImage`
/// und Icons aus Termas eigenem Icon-Ordner.
pub fn remove_appimage(desktop_file: &Path, dirs: &UserDirs, runner: &dyn Runner) -> Result<()> {
    let entry = DesktopEntry::parse(&std::fs::read_to_string(desktop_file)?);
    let program = entry.get("X-Terma-Path").map(str::to_string).or_else(|| entry.exec_program());
    if let Some(program) = program.map(PathBuf::from) {
        let is_appimage = program.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("appimage"));
        if is_appimage && program.is_file() {
            std::fs::remove_file(&program)?;
        }
    }
    if let Some(icon) = entry.get("Icon").map(PathBuf::from) {
        let inside = icon.starts_with(dirs.data.join("icons")) && !icon.components().any(|c| c == std::path::Component::ParentDir);
        if inside && icon.is_file() {
            std::fs::remove_file(&icon)?;
        }
    }
    std::fs::remove_file(desktop_file)?;
    refresh_menu(dirs, runner);
    Ok(())
}

/// `T3 Code` → `~/Applications/T3-Code.AppImage`. Gehört der Name schon einem anderen Programm, kommt `-2`, `-3` … dazu.
fn target_path(info: &AppImageInfo, dirs: &UserDirs) -> PathBuf {
    let base = file_stem_for(&info.name);
    let owned_by_us = |path: &Path| {
        let ours = dirs.applications.join(&info.desktop_file_name);
        info.path == path
            || std::fs::read_to_string(ours)
                .map(|t| DesktopEntry::parse(&t).get("X-Terma-Path").is_some_and(|p| Path::new(p) == path))
                .unwrap_or(false)
    };
    (1..)
        .map(|n| if n == 1 { format!("{base}.AppImage") } else { format!("{base}-{n}.AppImage") })
        .map(|name| dirs.appimages.join(name))
        .find(|path| !path.exists() || owned_by_us(path))
        .expect("es gibt immer einen freien Namen")
}

fn file_stem_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    let mut stem = String::new();
    for part in cleaned.split('-').filter(|p| !p.is_empty()) {
        if !stem.is_empty() {
            stem.push('-');
        }
        stem.push_str(part);
    }
    if stem.is_empty() {
        "Programm".to_string()
    } else {
        stem
    }
}

/// Verschiebt passende Einträge nach `data/backup/<unix-zeit>/` und gibt Zielpfad und Inhalt zurück.
fn retire(dirs: &UserDirs, matches: impl Fn(&str, &DesktopEntry) -> bool) -> Result<Vec<(PathBuf, DesktopEntry)>> {
    let mut retired = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(&dirs.applications) else { return Ok(retired) };
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let backup = dirs.data.join("backup").join(stamp.to_string());
    for path in read_dir.flatten().map(|e| e.path()) {
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
        if !file_name.ends_with(".desktop") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let entry = DesktopEntry::parse(&text);
        if matches(&file_name, &entry) {
            std::fs::create_dir_all(&backup)?;
            let dest = backup.join(&file_name);
            move_file(&path, &dest)?;
            retired.push((dest, entry));
        }
    }
    Ok(retired)
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    match std::fs::rename(from, to) {
        Err(e) if e.raw_os_error() == Some(EXDEV) => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)?;
            Ok(())
        }
        other => Ok(other?),
    }
}

/// Aktualisiert den MIME-Cache für Link-Handler. Fehlt das Werkzeug, merkt KDE die Änderung trotzdem.
fn refresh_menu(dirs: &UserDirs, runner: &dyn Runner) {
    let applications = dirs.applications.to_string_lossy();
    let _ = runner.run(&["update-desktop-database", &applications], &[], None, &mut |_| {});
}
