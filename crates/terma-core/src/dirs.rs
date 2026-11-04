//! Ordner, in denen Terma arbeitet. Tests legen dieselbe Struktur in einem Temp-Ordner an.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct UserDirs {
    pub home: PathBuf,
    pub applications: PathBuf,
    pub appimages: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub config: PathBuf,
}

impl UserDirs {
    pub fn from_env() -> UserDirs {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let xdg = |var: &str, fallback: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
        };
        let data_home = xdg("XDG_DATA_HOME", ".local/share");
        UserDirs {
            applications: data_home.join("applications"),
            appimages: home.join("Applications"),
            data: data_home.join("terma"),
            // Fest unter ~/.cache: der AUR-Helfer nimmt Pakete nur aus ~/.cache/terma/pkg an.
            cache: home.join(".cache/terma"),
            config: xdg("XDG_CONFIG_HOME", ".config").join("terma"),
            home,
        }
    }

    /// Dieselbe Struktur unterhalb von `root`, mit angelegten Ordnern.
    pub fn under(root: &Path) -> UserDirs {
        let home = root.join("home");
        let dirs = UserDirs {
            applications: home.join(".local/share/applications"),
            appimages: home.join("Applications"),
            data: home.join(".local/share/terma"),
            cache: home.join(".cache/terma"),
            config: home.join(".config/terma"),
            home,
        };
        for d in [&dirs.applications, &dirs.appimages, &dirs.data, &dirs.cache, &dirs.config] {
            std::fs::create_dir_all(d).expect("Testordner anlegen");
        }
        dirs
    }
}
