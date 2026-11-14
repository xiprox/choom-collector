//! The settings the app keeps between runs, in `settings.toml` in its data folder
//! (record/areas/architecture/02-settings-and-the-game.md).

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::relay;
use crate::state::{Role, SettingsAnswers};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Settings {
    /// `None` until the first run's setup finishes.
    pub role: Option<SavedRole>,
    /// The relay's address, without `https://` or a trailing `/`, such as `relay.example.net`.
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
    /// Patterns of files in this player's shared mods that are never sent, on top of the defaults.
    pub never_share: Vec<String>,
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

/// `%APPDATA%\ChoomCollector` on Windows, `~/.config/choom-collector` on Linux.
pub fn default_data_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(if cfg!(windows) { "ChoomCollector" } else { "choom-collector" })
}

/// Moves the data folder the app had as Cyberpunk Co-op Tools to its own name, once, so settings,
/// the session code, and what this PC shares and follows carry over the rename. Does nothing when
/// the new folder exists already or there's no old one.
pub fn move_old_data_dir() {
    if let Some(base) = dirs::config_dir() {
        move_old_data_dir_in(&base);
    }
}

/// [`move_old_data_dir`] within `base`, the system's folder for settings; true when it moved one.
fn move_old_data_dir_in(base: &Path) -> bool {
    let (old, new) = if cfg!(windows) { ("CyberpunkCoopTools", "ChoomCollector") } else { ("cyberpunk-coop-tools", "choom-collector") };
    let (old, new) = (base.join(old), base.join(new));
    if new.exists() || !old.is_dir() {
        return false;
    }
    match std::fs::rename(&old, &new) {
        Ok(()) => true,
        // The app's log isn't open yet, so this goes to the console of a dev build only.
        Err(e) => {
            eprintln!("couldn't move {} to {}: {e}", old.display(), new.display());
            false
        }
    }
}

/// Where the host's logs go unless setup says otherwise.
pub fn default_logs_folder() -> PathBuf {
    let base = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
    base.join("Choom Collector Logs")
}

impl Settings {
    fn file(data_dir: &Path) -> PathBuf {
        data_dir.join("settings.toml")
    }

    /// Takes the Settings page's answers; true when the relay or the code changed, so the session
    /// must start again with them. A blank relay or a code that doesn't read as one keeps the old.
    pub fn take_answers(&mut self, answers: SettingsAnswers) -> bool {
        let code = relay::parse_code(&answers.code).unwrap_or_else(|| self.code.clone());
        let relay = relay::tidy_address(&answers.relay);
        let restart = (!relay.is_empty() && relay != self.relay) || code != self.code;
        if restart {
            // The relay's proof of hosting belongs to the old code.
            self.resume.clear();
        }
        if !relay.is_empty() {
            self.relay = relay;
        }
        self.code = code;
        self.logs_folder = Some(answers.logs_folder);
        let lines = |list: Vec<String>| list.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        self.extra_paths = lines(answers.extra_paths);
        self.never_share = lines(answers.never_share);
        restart
    }

    /// Reads the settings, or starts fresh when there are none yet; an install ID is always set.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let file = Self::file(data_dir);
        let mut settings: Settings = match std::fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("{} isn't readable as settings", file.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => return Err(e).with_context(|| format!("couldn't read {}", file.display())),
        };
        // A file can hold the address as it was typed, `https://relay.example.net/`.
        settings.relay = relay::tidy_address(&settings.relay);
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

    #[test]
    fn the_old_data_folder_moves_to_the_new_name_once() {
        let base = std::env::temp_dir().join(format!("coop-rename-{}", rand::random::<u32>()));
        let (old, new) = if cfg!(windows) { ("CyberpunkCoopTools", "ChoomCollector") } else { ("cyberpunk-coop-tools", "choom-collector") };
        std::fs::create_dir_all(base.join(old)).unwrap();
        std::fs::write(base.join(old).join("settings.toml"), "code = \"K7RM2Q\"").unwrap();

        assert!(move_old_data_dir_in(&base));
        assert_eq!(std::fs::read_to_string(base.join(new).join("settings.toml")).unwrap(), "code = \"K7RM2Q\"");
        assert!(!base.join(old).exists());

        // A second start, or an old folder that turns up again, leaves the new one alone.
        std::fs::create_dir_all(base.join(old)).unwrap();
        assert!(!move_old_data_dir_in(&base));
        assert!(base.join(new).join("settings.toml").exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    fn answers(relay: &str, code: &str) -> SettingsAnswers {
        SettingsAnswers {
            relay: relay.into(),
            code: code.into(),
            logs_folder: PathBuf::from("logs"),
            extra_paths: vec![" bin/x64/a.csv ".into(), "".into()],
            never_share: vec!["*.psd".into()],
        }
    }

    #[test]
    fn a_new_relay_or_code_restarts_the_session_and_drops_the_resume_token() {
        let mut s = Settings { relay: "relay.example.net".into(), code: "K7RM2Q".into(), resume: "token".into(), ..Default::default() };

        // Only the lists and the folder changed: the session carries on.
        assert!(!s.take_answers(answers("relay.example.net", "k7r-m2q")));
        assert_eq!(s.resume, "token");
        assert_eq!(s.extra_paths, vec!["bin/x64/a.csv"]);
        assert_eq!(s.never_share, vec!["*.psd"]);
        assert_eq!(s.logs_folder, Some(PathBuf::from("logs")));

        // The same relay as Wrangler prints it is the same relay.
        assert!(!s.take_answers(answers("https://relay.example.net/", "K7RM2Q")));
        assert_eq!((s.relay.as_str(), s.resume.as_str()), ("relay.example.net", "token"));

        // A code that doesn't read as one keeps the old code, and a blank relay the old relay.
        assert!(!s.take_answers(answers("", "nope")));
        assert_eq!((s.relay.as_str(), s.code.as_str()), ("relay.example.net", "K7RM2Q"));

        assert!(s.take_answers(answers("relay.example.net", "AB2-CD3")));
        assert_eq!(s.code, "AB2CD3");
        assert!(s.resume.is_empty());

        s.resume = "token".into();
        assert!(s.take_answers(answers("other.example.net", "AB2CD3")));
        assert!(s.resume.is_empty());
    }
}
