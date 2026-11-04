use std::path::Path;
use std::sync::Arc;

use terma_core::dirs::UserDirs;
use terma_core::pacman::{self, PacmanDb};
use terma_core::runner::{FakeRunner, SystemRunner};

const SS: &str = include_str!("fixtures/pacman/Ss-discord.txt");
const SI: &str = include_str!("fixtures/pacman/Si-discord.txt");
const FL: &str = include_str!("fixtures/pacman/Fl-discord.txt");
const SUP: &str = include_str!("fixtures/pacman/Sup-preview.txt");

#[test]
fn parse_ss_reads_repo_name_version_installed() {
    let pkgs = pacman::parse_ss(SS);
    assert_eq!(pkgs[0].repo, "extra");
    assert_eq!(pkgs[0].name, "discord");
    assert_eq!(pkgs[0].version, "1:1.0.167-1");
    assert_eq!(pkgs[0].description, "All-in-one voice and text chat for gamers");
    assert_eq!(pkgs[0].installed_version, None);

    let with_version = pacman::parse_ss("extra/discord 1:1.0.167-1 [installed: 1:1.0.156-1]\n    Chat\n");
    assert_eq!(with_version[0].installed_version.as_deref(), Some("1:1.0.156-1"));

    let current = pacman::parse_ss("extra/dolphin 25.08.2-1 (kde-applications kde-system) [installed]\n    Files\n");
    assert_eq!(current[0].name, "dolphin");
    assert_eq!(current[0].version, "25.08.2-1");
    assert_eq!(current[0].installed_version.as_deref(), Some("25.08.2-1"));
}

#[test]
fn parse_size_units() {
    assert_eq!(pacman::parse_size("98.50 MiB"), Some(103_284_736));
    assert_eq!(pacman::parse_size("1.20 GiB"), Some(1_288_490_188));
    assert_eq!(pacman::parse_size("512.00 KiB"), Some(524_288));
    assert_eq!(pacman::parse_size("0.00 B"), Some(0));
    assert_eq!(pacman::parse_size("viel"), None);
}

#[test]
fn parse_si_reads_download_size_and_url() {
    let pkgs = pacman::parse_si(SI);
    assert_eq!(pkgs[0].download_size, 2_170_552);
    assert_eq!(pkgs[0].url.as_deref(), Some("https://discord.com"));
    assert_eq!(pacman::parse_si("Name            : ohne\nURL             : None\n")[0].url, None);
}

#[test]
fn parse_fl_detects_desktop_entry() {
    let map = pacman::parse_fl_desktop(FL);
    assert_eq!(map.get("discord"), Some(&true));
    assert_eq!(map.get("lib32-glibc"), Some(&false));
}

#[test]
fn parse_print_format_reads_preview() {
    let preview = pacman::parse_print_format(SUP);
    let discord = preview.iter().find(|p| p.name == "discord").expect("discord in der Vorschau");
    assert_eq!(discord.new_version, "1:1.0.167-1");
    assert!(discord.download_size > 0);
    assert_eq!(preview.len(), SUP.lines().filter(|l| !l.trim().is_empty()).count());
}

#[test]
fn sync_runs_fakeroot_with_private_dbpath() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(FakeRunner::new().on(&["fakeroot"], 0, ""));
    let db = PacmanDb::new(&dirs, runner.clone());

    db.sync().unwrap();

    let dbpath = dirs.cache.join("pacman-db").to_string_lossy().into_owned();
    let calls: Vec<Vec<String>> = runner.calls().into_iter().map(|c| c.argv).collect();
    let expected = |op: &str| -> Vec<String> {
        ["fakeroot", "--", "pacman", op, "--dbpath", &dbpath, "--logfile", "/dev/null", "--disable-sandbox"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    };
    assert_eq!(calls, vec![expected("-Sy"), expected("-Fy")]);
    assert_eq!(
        std::fs::read_link(dirs.cache.join("pacman-db/local")).unwrap(),
        Path::new("/var/lib/pacman/local")
    );
}

#[test]
fn sync_tolerates_missing_files_db() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(
        FakeRunner::new()
            .on(&["fakeroot", "--", "pacman", "-Sy"], 0, "")
            .on(&["fakeroot", "--", "pacman", "-Fy"], 1, "")
            .on(&["pacman", "-Ss"], 0, SS)
            .on(&["pacman", "-Si"], 0, SI)
            .on(&["pacman", "-Fl"], 1, ""),
    );
    let db = PacmanDb::new(&dirs, runner);

    db.sync().unwrap();
    let found = db.search(&["discord".to_string()]).unwrap();

    let discord = found.iter().find(|p| p.name == "discord").unwrap();
    assert_eq!(discord.has_desktop_entry, None);
    assert_eq!(discord.download_size, 2_170_552);
}

#[test]
#[ignore = "braucht echtes pacman und Netz (im Arch-Container)"]
fn real_sync_finds_discord() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let db = PacmanDb::new(&dirs, Arc::new(SystemRunner));

    db.sync().unwrap();
    let found = db.search(&["discord".to_string()]).unwrap();

    let discord = found.iter().find(|p| p.name == "discord").expect("discord gefunden");
    assert_eq!(discord.repo, "extra");
    assert_eq!(discord.has_desktop_entry, Some(true));
    assert!(discord.download_size > 0);
}

#[test]
fn kernel_packages_from_module_owners() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs = UserDirs::under(tmp.path());
    let runner = Arc::new(FakeRunner::new().on(&["pacman", "-Qqo", "/usr/lib/modules"], 0, "linux-cachyos\nlinux-cachyos-nvidia-open\n"));
    let db = PacmanDb::new(&dirs, runner);

    let kernels = db.kernel_packages();

    assert!(kernels.contains("linux-cachyos") && kernels.contains("linux-cachyos-nvidia-open"));
    assert_eq!(kernels.len(), 2);
}
