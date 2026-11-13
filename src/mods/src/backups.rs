//! What following replaces or deletes, and what unfollowing deletes, in case it was someone's own
//! work (record/areas/mods/01-sharing.md#backups).

use std::fs;
use std::path::{Path, PathBuf};

/// The backups in the data folder's `mod-backups/`, one folder per second something was kept,
/// such as `2026-11-13_21-04-09/r6/scripts/EquipmentEx/module.reds`.
pub struct Backups {
    root: PathBuf,
    cap: u64,
    max_file: u64,
    kept: bool,
}

impl Backups {
    /// All the backups together, beyond which the oldest go.
    pub const CAP: u64 = 2 << 30;
    /// A file larger than this isn't kept at all.
    pub const MAX_FILE: u64 = 512 << 20;

    pub fn new(data_dir: &Path) -> Self {
        Self::with_limits(data_dir.join("mod-backups"), Self::CAP, Self::MAX_FILE)
    }

    pub fn with_limits(root: PathBuf, cap: u64, max_file: u64) -> Self {
        Backups { root, cap, max_file, kept: false }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Keeps a copy of a file about to be replaced or deleted, under its game-relative path `rel`.
    /// False when it couldn't be kept just now; a file that isn't there, or is too large to keep,
    /// counts as kept, since trying again wouldn't change anything.
    pub fn keep(&mut self, full: &Path, rel: &str) -> bool {
        let Ok(meta) = fs::metadata(full) else { return true };
        if !meta.is_file() {
            return true;
        }
        if meta.len() > self.max_file {
            log::info!("Mods: no backup of {rel}, as it's {} MB", meta.len() >> 20);
            return true;
        }
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let dest = rel.split('/').fold(self.root.join(stamp), |p, seg| p.join(seg));
        let result = (|| -> std::io::Result<()> {
            fs::create_dir_all(dest.parent().expect("a backup's path has a folder"))?;
            let _ = fs::remove_file(&dest);
            // The file is replaced by a rename or deleted right after, so a hard link keeps the old
            // bytes without copying a large archive. A link of a link would keep only the link.
            let is_link = fs::symlink_metadata(full).is_ok_and(|m| m.file_type().is_symlink());
            if is_link || fs::hard_link(full, &dest).is_err() {
                fs::copy(full, &dest)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.kept = true;
                true
            }
            Err(e) => {
                log::warn!("Mods: couldn't back up {rel}: {e}");
                false
            }
        }
    }

    /// Deletes the oldest backups once all of them come to more than the cap: walking from the
    /// newest, the folder that takes the total over goes, and every older one with it.
    pub fn trim(&mut self) {
        if !self.kept {
            return;
        }
        self.kept = false;
        let Ok(entries) = fs::read_dir(&self.root) else { return };
        let mut dirs: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
        dirs.sort();
        let mut total = 0;
        for dir in dirs.iter().rev() {
            total += size_of(dir);
            if total > self.cap
                && let Err(e) = fs::remove_dir_all(dir)
            {
                log::warn!("Mods: couldn't tidy the backups: {e}");
            }
        }
    }
}

fn size_of(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size_of(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}
