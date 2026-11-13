//! The settings the app keeps between runs, in `settings.toml` in its data folder
//! (record/areas/architecture/02-settings-and-the-game.md).

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::state::Role;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Settings {
    /// `None` until the first run's setup finishes.
    pub role: Option<SavedRole>,
    /// The relay's address as the player entered it, such as `relay.example.net`.
    pub relay: String,
    /// The code this PC hosts, or the one it joins.
    pub code: String,
    /// The relay's proof that this PC hosts `code`, kept for taking the session back after a drop.
    pub resume: String,
    pub logs_folder: Option<PathBuf>,
    pub game_folder: Option<PathBuf>,
    /// Picked once per install, so the host knows a returning tester as the same player.
    pub install_id: String,
    /// The host's extra paths, relative to the game folder, that every tester's PC picks up.
    pub extra_paths: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SavedRole {
    Host,
    Tester,
}

impl From<SavedRole> for Role {
    fn from(r: SavedRole) -> Self {
        match r {
            SavedRole::Host => Role::Host,
            SavedRole::Tester => Role::Tester,
        }
    }
}

impl From<Role> for SavedRole {
    fn from(r: Role) -> Self {
        match r {
            Role::Host => SavedRole::Host,
            Role::Tester => SavedRole::Tester,
        }
    }
}

/// `%APPDATA%\CyberpunkCoopTools` on Windows, `~/.config/cyberpunk-coop-tools` on Linux.
pub fn default_data_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(if cfg!(windows) { "CyberpunkCoopTools" } else { "cyberpunk-coop-tools" })
}

/// Where the host's logs go unless setup says otherwise.
pub fn default_logs_folder() -> PathBuf {
    let base = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
    base.join("Cyberpunk Co-op Tools Logs")
}

impl Settings {
    fn file(data_dir: &Path) -> PathBuf {
        data_dir.join("settings.toml")
    }

    /// Reads the settings, or starts fresh when there are none yet; an install ID is always set.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let file = Self::file(data_dir);
        let mut settings: Settings = match std::fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("{} isn't readable as settings", file.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => return Err(e).with_context(|| format!("couldn't read {}", file.display())),
        };
        if settings.install_id.is_empty() {
            settings.install_id = (0..16).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
            settings.save(data_dir)?;
        }
        Ok(settings)
    }

    /// Writes the settings beside a temporary copy and renames it over them, so a crash mid-write
    /// leaves the old settings rather than half of the new ones.
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(data_dir).with_context(|| format!("couldn't create {}", data_dir.display()))?;
        let file = Self::file(data_dir);
        let temporary = file.with_extension("toml.tmp");
        std::fs::write(&temporary, toml::to_string_pretty(self)?).with_context(|| format!("couldn't write {}", temporary.display()))?;
        std::fs::rename(&temporary, &file).with_context(|| format!("couldn't replace {}", file.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_keep_their_install_id() {
        let dir = std::env::temp_dir().join(format!("coop-settings-{}", rand::random::<u32>()));
        let first = Settings::load(&dir).unwrap();
        assert_eq!(first.install_id.len(), 32);
        assert!(first.role.is_none());

        let mut changed = first.clone();
        changed.role = Some(SavedRole::Host);
        changed.code = "K7RM2Q".into();
        changed.save(&dir).unwrap();

        let again = Settings::load(&dir).unwrap();
        assert_eq!(again.install_id, first.install_id);
        assert_eq!(again.role, Some(SavedRole::Host));
        assert_eq!(again.code, "K7RM2Q");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
