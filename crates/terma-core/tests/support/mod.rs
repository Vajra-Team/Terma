//! Baut künstliche AppImages: ein kleines ELF mit `.upd_info`, dahinter ein Squashfs.
#![allow(dead_code)]

use std::io::Cursor;
use std::path::{Path, PathBuf};

use backhand::{FilesystemWriter, NodeHeader};
use object::write::Object;
use object::{Architecture, BinaryFormat, Endianness, SectionKind};

/// Ein gültiges 1×1-PNG.
pub const PNG_1PX: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// ELF wie die AppImage-Runtime: `.upd_info` ist 1024 Bytes groß und mit NUL aufgefüllt.
pub fn make_elf(upd_info: Option<&str>) -> Vec<u8> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
    let section = obj.add_section(Vec::new(), b".upd_info".to_vec(), SectionKind::ReadOnlyData);
    let mut data = upd_info.unwrap_or("").as_bytes().to_vec();
    data.resize(1024, 0);
    obj.set_section_data(section, data, 1);
    let bytes = obj.write().expect("ELF schreiben");
    let shoff = u64::from_le_bytes(bytes[0x28..0x30].try_into().unwrap());
    let shentsize = u16::from_le_bytes(bytes[0x3A..0x3C].try_into().unwrap()) as u64;
    let shnum = u16::from_le_bytes(bytes[0x3C..0x3E].try_into().unwrap()) as u64;
    assert_eq!(shoff + shentsize * shnum, bytes.len() as u64, "Sektionstabelle muss am Ende liegen");
    bytes
}

pub fn make_appimage(
    dir: &Path,
    file_name: &str,
    desktop_name: &str,
    desktop: &str,
    icon_png: Option<&[u8]>,
    upd_info: Option<&str>,
    extra_files: &[(&str, &[u8])],
) -> PathBuf {
    let header = NodeHeader::new(0o644, 0, 0, 0);
    let mut fs = FilesystemWriter::default();
    fs.push_file(Cursor::new(desktop.as_bytes().to_vec()), desktop_name, header).unwrap();
    if let Some(png) = icon_png {
        let icon_name = format!("{}.png", desktop_name.trim_end_matches(".desktop"));
        fs.push_file(Cursor::new(png.to_vec()), &icon_name, header).unwrap();
        fs.push_symlink(&icon_name, ".DirIcon", header).unwrap();
    }
    for (path, bytes) in extra_files {
        if let Some(parent) = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()) {
            fs.push_dir_all(parent, NodeHeader::new(0o755, 0, 0, 0)).unwrap();
        }
        fs.push_file(Cursor::new(bytes.to_vec()), path, header).unwrap();
    }
    let mut squashfs = Cursor::new(Vec::new());
    fs.write(&mut squashfs).unwrap();

    let mut bytes = make_elf(upd_info);
    bytes.extend_from_slice(squashfs.get_ref());
    let path = dir.join(file_name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// Aufbau wie bei linuxdeploy: `.desktop` und Icon liegen unter `usr/share`, im Wurzelverzeichnis nur Symlinks.
pub fn make_linuxdeploy_appimage(dir: &Path, file_name: &str) -> PathBuf {
    let file = NodeHeader::new(0o644, 0, 0, 0);
    let folder = NodeHeader::new(0o755, 0, 0, 0);
    let desktop = "[Desktop Entry]\nName=Example\nExec=example\nIcon=example\nType=Application\n";
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>";
    let mut fs = FilesystemWriter::default();
    fs.push_dir_all("usr/share/applications", folder).unwrap();
    fs.push_dir_all("usr/share/icons/hicolor/scalable/apps", folder).unwrap();
    fs.push_file(Cursor::new(desktop.as_bytes().to_vec()), "usr/share/applications/org.example.App.desktop", file).unwrap();
    fs.push_file(Cursor::new(svg.as_bytes().to_vec()), "usr/share/icons/hicolor/scalable/apps/example.svg", file).unwrap();
    fs.push_symlink("usr/share/applications/org.example.App.desktop", "org.example.App.desktop", file).unwrap();
    fs.push_symlink("usr/share/icons/hicolor/scalable/apps/example.svg", "example.svg", file).unwrap();
    fs.push_symlink("example.svg", ".DirIcon", file).unwrap();
    let mut squashfs = Cursor::new(Vec::new());
    fs.write(&mut squashfs).unwrap();

    let mut bytes = make_elf(None);
    bytes.extend_from_slice(squashfs.get_ref());
    let path = dir.join(file_name);
    std::fs::write(&path, bytes).unwrap();
    path
}
