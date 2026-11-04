//! AppImages lesen, ohne sie auszuführen: ELF-Kopf für Größe und `.upd_info`, dahinter das Squashfs.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use backhand::{FilesystemReader, InnerNode};

use crate::desktop::DesktopEntry;
use crate::error::{Result, TermaError};

#[derive(Debug, Clone, PartialEq)]
pub struct IconData {
    pub ext: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AppImageInfo {
    pub path: PathBuf,
    pub name: String,
    pub desktop_file_name: String,
    pub desktop: DesktopEntry,
    pub icon: Option<IconData>,
    pub update_info: Option<String>,
    pub self_updating: bool,
    pub version: Option<String>,
}

/// Größer ist kein AppImage-Runtime-ELF.
const MAX_ELF_SIZE: u64 = 64 * 1024 * 1024;

pub fn inspect(path: &Path) -> Result<AppImageInfo> {
    let mut file = File::open(path)?;
    let (offset, update_info) = read_elf(&mut file).ok_or(TermaError::NotAnAppImage)?;
    let fs = FilesystemReader::from_reader_with_offset(BufReader::new(file), offset).map_err(|_| TermaError::NotAnAppImage)?;

    // Bei linuxdeploy-AppImages ist die `.desktop`-Datei im Wurzelverzeichnis ein Symlink.
    let mut desktop_names: Vec<String> = fs
        .files()
        .filter(|n| matches!(n.inner, InnerNode::File(_) | InnerNode::Symlink(_)) && n.fullpath.parent() == Some(Path::new("/")))
        .filter_map(|n| n.fullpath.file_name()?.to_str().map(str::to_string))
        .filter(|name| name.ends_with(".desktop"))
        .collect();
    desktop_names.sort();
    let desktop_file_name = desktop_names.into_iter().next().ok_or(TermaError::NotAnAppImage)?;
    let text = read_file(&fs, &Path::new("/").join(&desktop_file_name)).ok_or(TermaError::NotAnAppImage)?;
    let desktop = DesktopEntry::parse(&String::from_utf8_lossy(&text));

    let icon = find_icon(&fs, &desktop);
    let self_updating = fs.files().any(|n| n.fullpath.ends_with("resources/app-update.yml"));

    Ok(AppImageInfo {
        path: path.to_path_buf(),
        name: desktop.get("Name").unwrap_or(desktop_file_name.trim_end_matches(".desktop")).to_string(),
        version: desktop.get("X-AppImage-Version").map(str::to_string),
        desktop_file_name,
        desktop,
        icon,
        update_info,
        self_updating,
    })
}

/// Liefert das Ende der ELF-Sektionstabelle (= Beginn des Squashfs) und den Inhalt von `.upd_info`.
fn read_elf(file: &mut File) -> Option<(u64, Option<String>)> {
    let len = file.metadata().ok()?.len();
    let mut head = [0u8; 64];
    file.read_exact(&mut head).ok()?;
    if &head[..4] != b"\x7fELF" {
        return None;
    }
    let header = goblin::elf::Elf::parse_header(&head).ok()?;
    let end = header.e_shoff + u64::from(header.e_shnum) * u64::from(header.e_shentsize);
    if end == 0 || end > len || end > MAX_ELF_SIZE {
        return None;
    }
    let mut elf_bytes = vec![0u8; end as usize];
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_exact(&mut elf_bytes).ok()?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).ok()?;
    if &magic != b"hsqs" {
        return None;
    }
    file.seek(SeekFrom::Start(0)).ok()?;

    let elf = goblin::elf::Elf::parse(&elf_bytes).ok()?;
    let update_info = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(".upd_info"))
        .and_then(|sh| elf_bytes.get(sh.sh_offset as usize..(sh.sh_offset + sh.sh_size) as usize))
        .map(|data| String::from_utf8_lossy(data).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string())
        .filter(|s| !s.is_empty());
    Some((end, update_info))
}

fn find_node<'a, 'b>(fs: &'a FilesystemReader<'b>, path: &Path) -> Option<&'a backhand::Node<backhand::SquashfsFileReader>> {
    fs.files().find(|n| n.fullpath == path)
}

/// Liest eine Datei und folgt dabei bis zu acht Symlinks.
fn read_file(fs: &FilesystemReader<'_>, path: &Path) -> Option<Vec<u8>> {
    resolve(fs, path).map(|(_, bytes)| bytes)
}

fn resolve(fs: &FilesystemReader<'_>, path: &Path) -> Option<(PathBuf, Vec<u8>)> {
    let mut current = path.to_path_buf();
    for _ in 0..8 {
        match &find_node(fs, &current)?.inner {
            InnerNode::File(f) => {
                let mut bytes = Vec::new();
                fs.file(f).reader().read_to_end(&mut bytes).ok()?;
                return Some((current, bytes));
            }
            InnerNode::Symlink(link) => {
                let base = current.parent().unwrap_or(Path::new("/"));
                current = normalize(&base.join(&link.link));
            }
            _ => return None,
        }
    }
    None
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for c in path.components() {
        match c {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                out.pop();
            }
            _ => {}
        }
    }
    out
}

/// `.DirIcon`, sonst `<Icon>.png` oder `<Icon>.svg` im Wurzelverzeichnis.
fn find_icon(fs: &FilesystemReader<'_>, desktop: &DesktopEntry) -> Option<IconData> {
    let mut candidates = vec![PathBuf::from("/.DirIcon")];
    if let Some(icon) = desktop.get("Icon") {
        candidates.push(Path::new("/").join(format!("{icon}.png")));
        candidates.push(Path::new("/").join(format!("{icon}.svg")));
    }
    candidates.iter().find_map(|c| resolve(fs, c)).map(|(path, bytes)| IconData { ext: icon_ext(&path, &bytes), bytes })
}

fn icon_ext(path: &Path, bytes: &[u8]) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext @ ("png" | "svg")) => ext.to_string(),
        _ if bytes.starts_with(b"\x89PNG") => "png".to_string(),
        _ if String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).contains("<svg") => "svg".to_string(),
        _ => "png".to_string(),
    }
}
