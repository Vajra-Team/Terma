//! Updates für eingebundene AppImages über die eingebettete Update-Information.
//! Geladen wird immer die ganze Datei (kein Delta), geprüft über die SHA-1 aus der `.zsync`-Datei.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::appimage::{self, AppImageInfo};
use crate::dirs::UserDirs;
use crate::error::{Result, TermaError};
use crate::http::Http;
use crate::integrate::{integrate_appimage, IntegratedApp};
use crate::runner::Runner;

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateSource {
    GhReleasesZsync { owner: String, repo: String, tag: String, pattern: String },
    Zsync { url: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZsyncHeader {
    pub filename: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteAppImage {
    pub url: String,
    pub sha1: String,
}

pub fn parse_update_info(s: &str) -> Option<UpdateSource> {
    let parts: Vec<&str> = s.trim().split('|').collect();
    match parts.as_slice() {
        ["gh-releases-zsync", owner, repo, tag, pattern] if [owner, repo, tag, pattern].iter().all(|p| !p.is_empty()) => {
            Some(UpdateSource::GhReleasesZsync {
                owner: owner.to_string(),
                repo: repo.to_string(),
                tag: tag.to_string(),
                pattern: pattern.to_string(),
            })
        }
        ["zsync", url] if url.starts_with("https://") => Some(UpdateSource::Zsync { url: url.to_string() }),
        _ => None,
    }
}

pub fn release_api_url(owner: &str, repo: &str, tag: &str) -> String {
    if tag == "latest" {
        format!("https://api.github.com/repos/{owner}/{repo}/releases/latest")
    } else {
        format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}")
    }
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    updated_at: String,
}

/// Download-Adresse des passenden Assets. Passen mehrere, gewinnt das zuletzt aktualisierte.
pub fn pick_asset(release_json: &str, pattern: &str) -> Result<Option<String>> {
    let release: Release = serde_json::from_str(release_json).map_err(|e| TermaError::Network(e.to_string()))?;
    let pattern = glob::Pattern::new(pattern).map_err(|e| TermaError::Network(e.to_string()))?;
    Ok(release
        .assets
        .into_iter()
        .filter(|a| pattern.matches(&a.name))
        .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
        .map(|a| a.browser_download_url))
}

/// Liest den Textkopf einer `.zsync`-Datei (bis zur ersten Leerzeile).
pub fn parse_zsync_header(text: &str, zsync_url: &str) -> Option<ZsyncHeader> {
    let mut filename = String::new();
    let mut url = None;
    let mut sha1 = None;
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        match line.split_once(": ") {
            Some(("Filename", v)) => filename = v.trim().to_string(),
            Some(("URL", v)) => url = Some(v.trim().to_string()),
            Some(("SHA-1", v)) => sha1 = Some(v.trim().to_lowercase()),
            _ => {}
        }
    }
    let url = url?;
    let url = if url.contains("://") {
        url
    } else {
        let base = zsync_url.rsplit_once('/')?.0;
        format!("{base}/{url}")
    };
    Some(ZsyncHeader { filename, url, sha1: sha1? })
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// `None`, wenn es keine Update-Information gibt oder die Datei schon aktuell ist.
pub fn check(app: &AppImageInfo, http: &dyn Http) -> Result<Option<RemoteAppImage>> {
    let Some(source) = app.update_info.as_deref().and_then(parse_update_info) else { return Ok(None) };
    let zsync_url = match source {
        UpdateSource::Zsync { url } => url,
        UpdateSource::GhReleasesZsync { owner, repo, tag, pattern } => {
            match pick_asset(&http.get_string(&release_api_url(&owner, &repo, &tag))?, &pattern)? {
                Some(url) => url,
                None => return Ok(None),
            }
        }
    };
    let bytes = http.get_bytes(&zsync_url)?;
    let header = parse_zsync_header(&String::from_utf8_lossy(&bytes), &zsync_url)
        .ok_or_else(|| TermaError::Network(format!("unlesbare zsync-Datei: {zsync_url}")))?;
    if header.sha1 == sha1_file(&app.path)? {
        return Ok(None);
    }
    Ok(Some(RemoteAppImage { url: header.url, sha1: header.sha1 }))
}

/// Lädt die neue Datei, prüft die Prüfsumme und richtet sie an Stelle der alten ein.
pub fn apply(remote: &RemoteAppImage, dirs: &UserDirs, http: &dyn Http, runner: &dyn Runner) -> Result<IntegratedApp> {
    let download_dir = dirs.cache.join("download");
    std::fs::create_dir_all(&download_dir)?;
    let file_name = remote.url.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("update.AppImage");
    let path = download_dir.join(file_name);
    http.download(&remote.url, &path)?;
    if sha1_file(&path)? != remote.sha1.to_lowercase() {
        let _ = std::fs::remove_file(&path);
        return Err(TermaError::Network(format!("Prüfsumme passt nicht: {}", remote.url)));
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    integrate_appimage(&appimage::inspect(&path)?, dirs, runner)
}
