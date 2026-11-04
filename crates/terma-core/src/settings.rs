//! Einstellungen in `~/.config/terma/settings.json`.

use serde::{Deserialize, Serialize};

use crate::dirs::UserDirs;
use crate::error::{Result, TermaError};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// „Ohne Passwort installieren“, ab Werk aus.
    pub passwordless: bool,
}

impl Settings {
    pub fn load(dirs: &UserDirs) -> Settings {
        std::fs::read_to_string(dirs.config.join("settings.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, dirs: &UserDirs) -> Result<()> {
        std::fs::create_dir_all(&dirs.config)?;
        let text = serde_json::to_string_pretty(self).map_err(|e| TermaError::Io(e.to_string()))?;
        std::fs::write(dirs.config.join("settings.json"), text)?;
        Ok(())
    }
}
