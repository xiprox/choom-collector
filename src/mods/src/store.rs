//! What a player shares and follows, kept in `mods.toml` in the data folder so it survives the
//! app's restarts.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::folder_of;

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Saved {
    /// The keys of the mods this PC shares, each a mod's name in lower case.
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub follows: Vec<SavedFollow>,
}

/// A folder this PC follows, from the PC the player agreed to take it from.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SavedFollow {
    /// Such as `r6/scripts/EquipmentEx`.
    pub folder: String,
    /// The sharer's PC ID (`layout::pc_id`).
    pub sharer: String,
    /// The sharer's name when the player agreed, for the window.
    pub by: String,
}

fn file(data_dir: &Path) -> PathBuf {
    data_dir.join("mods.toml")
}

impl Saved {
    /// What's saved, without any folder that fails the path checks; nothing when there's no file
    /// or it can't be read.
    pub fn load(data_dir: &Path) -> Saved {
        let Ok(text) = fs::read_to_string(file(data_dir)) else { return Saved::default() };
        let mut saved: Saved = toml::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Mods: couldn't read mods.toml, so nothing is shared or followed: {e}");
            Saved::default()
        });
        saved.follows.retain(|f| folder_of(&f.folder).as_deref() == Some(f.folder.as_str()));
        saved
    }

    /// Writes beside the file and renames over it, so a crash mid-write leaves the last copy.
    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(data_dir)?;
        let tmp = data_dir.join("mods.toml.tmp");
        fs::write(&tmp, toml::to_string(self).expect("the shares and follows should always encode"))?;
        fs::rename(tmp, file(data_dir))
    }
}
