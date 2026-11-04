//! Datei-Fenster: Was ist das für eine Datei, wie heißt das Programm, gibt es das auch in einer Quelle?

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::search::Candidate;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileKind {
    AppImage,
    Deb,
    Rpm,
    Tarball,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileInspection {
    pub path: PathBuf,
    pub kind: FileKind,
    pub app_name: String,
    pub app_version: Option<String>,
    pub icon: Option<PathBuf>,
    pub source_match: Option<Candidate>,
    /// AppImage mit eigenem Updater: dann empfiehlt Terma das AppImage statt der Quelle.
    pub self_updating: bool,
}

const TARBALLS: &[&str] = &[".tar.gz", ".tgz", ".tar.xz", ".tar.zst", ".tar.bz2"];
const EXTENSIONS: &[&str] = &[".appimage", ".deb", ".rpm", ".tar.gz", ".tgz", ".tar.xz", ".tar.zst", ".tar.bz2", ".zip"];
const ARCH_TOKENS: &[&str] = &["x86_64", "x86", "amd64", "x64", "aarch64", "arm64", "linux", "i686"];

fn is_elf(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && &magic == b"\x7fELF"
}

pub fn detect_kind(path: &Path) -> FileKind {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.ends_with(".appimage") {
        if is_elf(path) {
            FileKind::AppImage
        } else {
            FileKind::Unknown
        }
    } else if name.ends_with(".deb") {
        FileKind::Deb
    } else if name.ends_with(".rpm") {
        FileKind::Rpm
    } else if TARBALLS.iter().any(|ext| name.ends_with(ext)) {
        FileKind::Tarball
    } else {
        FileKind::Unknown
    }
}

/// `"T3-Code-0.1.1-x86_64.AppImage"` → `"T3 Code"`: abgeschnitten ab dem ersten Glied, das eine Version
/// oder eine Architektur ist.
pub fn name_from_filename(file_name: &str) -> String {
    let lower = file_name.to_lowercase();
    let stem = EXTENSIONS.iter().find(|ext| lower.ends_with(*ext)).map(|ext| &file_name[..file_name.len() - ext.len()]).unwrap_or(file_name);
    let tokens: Vec<&str> = stem.split(['-', '_']).filter(|t| !t.is_empty()).collect();
    let is_cut = |t: &str| {
        let lower = t.to_lowercase();
        t.starts_with(|c: char| c.is_ascii_digit())
            || (lower.starts_with('v') && lower[1..].starts_with(|c: char| c.is_ascii_digit()))
            || ARCH_TOKENS.contains(&lower.as_str())
    };
    let name: Vec<&str> = tokens.iter().skip(1).take_while(|t| !is_cut(t)).copied().collect();
    match tokens.first() {
        Some(first) => std::iter::once(*first).chain(name).collect::<Vec<_>>().join(" "),
        None => stem.to_string(),
    }
}
