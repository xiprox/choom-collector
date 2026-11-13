//! Everything that turns a name from a player into a place on the host's disk
//! (record/areas/logs/01-collecting.md#the-hosts-folder). A player's PC is someone else's, so
//! nothing it sends may land outside its folder or on the host's own files.

use std::path::{Path, PathBuf};

use crate::ident::fnv;

/// A copy whose full path would be longer than this goes in `_long/`, since a program on the host
/// that hasn't opted into long paths can't open one past Windows' 260-character `MAX_PATH`.
const LONG_PATH: usize = 240;

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// One part of a path, made safe to create on Windows and Linux: `..` comes out as `_`, and `CON`
/// as `_CON`.
pub fn segment(part: &str) -> String {
    let replaced: String = part.chars().map(|c| if (c as u32) < 32 || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let mut s = replaced.trim_end_matches([' ', '.']).to_string();
    if s.is_empty() {
        s = "_".into();
    }
    let stem = s.split('.').next().unwrap_or("").trim_end_matches(' ');
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        s.insert(0, '_');
    }
    s.chars().take(120).collect()
}

/// A logical path from a player, as safe parts joined by `/`. The first part never starts with
/// `_`, so it can't land on `_history`, `_manifest.tsv`, or any other file of the host's.
pub fn clean_logical(path: &str) -> String {
    let mut parts: Vec<String> = path.split(['/', '\\']).filter(|p| !p.is_empty()).map(segment).collect();
    if parts.is_empty() {
        parts.push("unnamed".into());
    }
    if parts[0].starts_with('_') {
        parts[0].insert(0, 'x');
    }
    parts.join("/")
}

/// The folder name for a player's name: `mira`, or `peer` for a name with nothing usable in it.
pub fn player_folder(name: &str) -> String {
    let kept: String = name.trim().chars().map(|c| if c.is_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect();
    let s = segment(&kept);
    let s = if s == "_" { "peer".to_string() } else { s };
    s.chars().take(40).collect()
}

/// Where the host keeps the copy of a cleaned logical path, always inside `dir`.
pub fn local(dir: &Path, logical: &str) -> PathBuf {
    let path = logical.split('/').fold(dir.to_path_buf(), |p, part| p.join(part));
    if !too_long(&path) {
        return path;
    }
    let name = logical.rsplit('/').next().unwrap_or(logical);
    dir.join("_long").join(format!("{:016x}_{}", fnv(logical.as_bytes()), last_chars(name, 60)))
}

/// `path`, or a place in `dir/_long` for it when it's too long.
pub fn shorten(dir: &Path, path: PathBuf) -> PathBuf {
    if !too_long(&path) {
        return path;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dir.join("_long").join(format!("{:016x}_{}", fnv(path.to_string_lossy().as_bytes()), last_chars(&name, 60)))
}

/// Windows counts a path's length in UTF-16 units.
fn too_long(path: &Path) -> bool {
    path.to_string_lossy().encode_utf16().count() > LONG_PATH
}

fn last_chars(s: &str, n: usize) -> String {
    let count = s.chars().count();
    s.chars().skip(count.saturating_sub(n)).collect()
}

/// Text for one field of a tab-separated line.
pub fn one_line(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_players_path_cant_leave_its_folder() {
        assert_eq!(clean_logical("game/../../x.log"), "game/_/_/x.log");
        assert_eq!(clean_logical("_history/x.log"), "x_history/x.log");
        assert_eq!(clean_logical(r"game\r6\logs\x.log"), "game/r6/logs/x.log");
        assert_eq!(clean_logical("game/CON.log"), "game/_CON.log");
        assert_eq!(clean_logical("game/a:b*.log. "), "game/a_b_.log");
        assert_eq!(clean_logical(""), "unnamed");
    }

    #[test]
    fn player_folders_keep_what_a_name_can_hold() {
        assert_eq!(player_folder("mira"), "mira");
        assert_eq!(player_folder("V / Silverhand"), "V___Silverhand");
        assert_eq!(player_folder("  "), "peer");
        assert_eq!(player_folder(&"x".repeat(60)).len(), 40);
    }

    #[test]
    fn a_long_path_goes_to_long() {
        let dir = Path::new("/host/logs/2026-11-13 K7RM2Q/mira");
        let logical = format!("game/{}/x.log", "deep/".repeat(60).trim_end_matches('/'));
        let path = local(dir, &logical);
        assert!(path.starts_with(dir.join("_long")));
        assert!(path.to_string_lossy().ends_with("_x.log"));
        assert_eq!(local(dir, "game/x.log"), dir.join("game").join("x.log"));
    }
}
