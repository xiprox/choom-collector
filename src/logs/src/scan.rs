//! Finding the logs on a player's PC (record/areas/logs/01-collecting.md#whats-picked-up).
//! Every file gets a logical path that starts with the source it came from, such as
//! `game/red4ext/logs/red4ext.log`, which is also where the host keeps its copy.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{SourceCount, SourceKind};

/// A file a scan found.
#[derive(Clone, Debug)]
pub struct Found {
    pub logical: String,
    pub full: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    /// Picked up by one of the host's extra paths, which the 24-hour rule leaves alone.
    pub extra: bool,
}

/// What a scan returns: every file by its logical path, and how many each source picked up.
pub struct Scan {
    pub files: BTreeMap<String, Found>,
    pub counts: Vec<SourceCount>,
}

#[derive(Clone, Copy)]
enum Filter {
    Logs,
    CrashReports,
    LogsAndJson,
    LogsAndText,
}

enum Look {
    /// Every wanted file in the folder and below it.
    Walk(Filter),
    /// The files in the folder whose names match, and none below it.
    Pattern(String),
    /// This file, or if it's a folder, its logs and `.txt` at any depth.
    Path,
    /// An extra path that would reach outside the game folder.
    Refused,
}

struct Source {
    kind: SourceKind,
    /// The logical path of the source's root, such as `game` or `extra/bin/x64/StreetVendors`.
    label: String,
    root: PathBuf,
    look: Look,
}

/// A walk stops this deep, so a folder that links back to itself can't hold it forever.
const MAX_DEPTH: usize = 40;

/// Every session folder of a host's holds this file, and a walk skips any folder that does, so a
/// host whose logs folder sits inside the game folder doesn't send its own copies to itself.
pub const SESSION_MARKER: &str = "_collector.log";

pub struct Scanner {
    sources: Vec<Source>,
}

impl Scanner {
    pub fn new(game: &Path, local_app_data: Option<&Path>, app_log: Option<&Path>, extra_paths: &[String]) -> Self {
        let mut sources = Vec::new();
        let has_game = !game.as_os_str().is_empty();
        if has_game {
            sources.push(Source { kind: SourceKind::GameFolder, label: "game".into(), root: game.to_path_buf(), look: Look::Walk(Filter::Logs) });
        }
        if let Some(local) = local_app_data {
            sources.push(Source {
                kind: SourceKind::CrashReports,
                label: "crash-reports".into(),
                root: local.join("REDEngine").join("ReportQueue"),
                look: Look::Walk(Filter::CrashReports),
            });
            sources.push(Source {
                kind: SourceKind::GameOwn,
                label: "localappdata".into(),
                root: local.join("CD Projekt Red").join("Cyberpunk 2077"),
                look: Look::Walk(Filter::LogsAndJson),
            });
        }
        if let Some(log) = app_log {
            sources.push(Source { kind: SourceKind::AppLog, label: "app".into(), root: log.to_path_buf(), look: Look::Path });
        }
        for pattern in extra_paths.iter().filter(|p| !p.trim().is_empty()) {
            sources.push(extra(game, has_game, pattern));
        }
        Scanner { sources }
    }

    pub fn scan(&self) -> Scan {
        let mut scan = Scan { files: BTreeMap::new(), counts: Vec::new() };
        let mut seen = HashSet::new();
        for source in &self.sources {
            let mut found = Vec::new();
            match &source.look {
                Look::Walk(filter) => walk(&source.root, &source.label, *filter, 0, &mut found),
                Look::Pattern(pattern) => matching(&source.root, &source.label, pattern, &mut found),
                Look::Path => {
                    if source.root.is_file() {
                        // The app's log keeps its name under `app/`; an extra file keeps its path.
                        let logical = match source.kind {
                            SourceKind::AppLog => format!("{}/{}", source.label, name_of(&source.root)),
                            _ => source.label.clone(),
                        };
                        found.push((source.root.clone(), logical));
                    } else if source.root.is_dir() {
                        walk(&source.root, &source.label, Filter::LogsAndText, 0, &mut found);
                    }
                }
                Look::Refused => {}
            }
            let extra = matches!(source.kind, SourceKind::Extra(_));
            let mut count = 0;
            for (full, logical) in found {
                if name_of(&full).to_lowercase().ends_with(".dmp") {
                    continue;
                }
                // A listing can report a stale size for a file another program is still writing,
                // and asking for the file itself gets the real one. (read in CP77 Log Collector
                // 1.3.1, src/Scanner.cs)
                let Ok(meta) = fs::metadata(&full) else { continue };
                if !meta.is_file() {
                    continue;
                }
                count += 1;
                if seen.insert(full.clone()) {
                    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    scan.files.entry(logical.clone()).or_insert(Found { logical, full, size: meta.len(), modified, extra });
                }
            }
            scan.counts.push(SourceCount { kind: source.kind.clone(), count });
        }
        scan
    }
}

/// An extra path from the host, which must stay inside the game folder, since the host could
/// otherwise ask every player's PC for any file it can read.
fn extra(game: &Path, has_game: bool, pattern: &str) -> Source {
    let kind = SourceKind::Extra(pattern.to_string());
    let refused = Source { kind: kind.clone(), label: String::new(), root: PathBuf::new(), look: Look::Refused };
    let cleaned = pattern.trim().trim_matches('"').replace('\\', "/");
    let parts: Vec<&str> = cleaned.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let wild = |p: &str| p.contains('*') || p.contains('?');
    let Some((last, folders)) = parts.split_last() else { return refused };
    if !has_game || cleaned.starts_with('/') || parts.iter().any(|p| *p == ".." || p.contains(':')) || folders.iter().any(|p| wild(p)) {
        return refused;
    }
    let folder = folders.iter().fold(game.to_path_buf(), |path, part| path.join(part));
    let label = |parts: &[&str]| ["extra"].iter().chain(parts).copied().collect::<Vec<_>>().join("/");
    if wild(last) {
        Source { kind, label: label(folders), root: folder, look: Look::Pattern(last.to_lowercase()) }
    } else {
        Source { kind, label: label(&parts), root: folder.join(last), look: Look::Path }
    }
}

fn walk(dir: &Path, logical: &str, filter: Filter, depth: usize, found: &mut Vec<(PathBuf, String)>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == SESSION_MARKER {
            return;
        }
        let Ok(kind) = entry.file_type() else { continue };
        // A link to a folder isn't walked into, but a link to a file is read like the file.
        if kind.is_dir() {
            folders.push(name);
        } else if kind.is_file() || (kind.is_symlink() && entry.path().is_file()) {
            files.push(name);
        }
    }
    let dir_name = name_of(dir);
    for name in files {
        if wanted(filter, &name.to_lowercase(), &dir_name) {
            found.push((dir.join(&name), format!("{logical}/{name}")));
        }
    }
    for name in folders {
        walk(&dir.join(&name), &format!("{logical}/{name}"), filter, depth + 1, found);
    }
}

fn matching(dir: &Path, logical: &str, pattern: &str, found: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if glob(pattern.as_bytes(), name.to_lowercase().as_bytes()) {
            found.push((entry.path(), format!("{logical}/{name}")));
        }
    }
}

fn wanted(filter: Filter, name: &str, dir_name: &str) -> bool {
    match filter {
        Filter::Logs => is_log(name, dir_name),
        // A crash report's text files, and not the save and screenshot that come with it.
        Filter::CrashReports => is_log(name, dir_name) || [".txt", ".json", ".xml", ".ini"].iter().any(|e| name.ends_with(e)),
        Filter::LogsAndJson => is_log(name, dir_name) || name.ends_with(".json"),
        Filter::LogsAndText => is_log(name, dir_name) || name.ends_with(".txt"),
    }
}

/// `x.log`, `x.log.3`, `x.log.txt`, and any `.txt` in a folder named `logs` or `log`, with
/// `name` in lower case.
fn is_log(name: &str, dir_name: &str) -> bool {
    if name.ends_with(".log") || is_rotated_log(name) {
        return true;
    }
    name.ends_with(".txt")
        && (name.ends_with(".log.txt") || dir_name.eq_ignore_ascii_case("logs") || dir_name.eq_ignore_ascii_case("log"))
}

fn is_rotated_log(name: &str) -> bool {
    name.rfind(".log.").is_some_and(|at| {
        let number = &name[at + 5..];
        !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
    })
}

/// `*` and `?` against a whole name, both in lower case.
fn glob(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => (0..=name.len()).any(|skip| glob(rest, &name[skip..])),
        Some((b'?', rest)) => !name.is_empty() && glob(rest, &name[1..]),
        Some((c, rest)) => name.first() == Some(c) && glob(rest, &name[1..]),
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_are_told_by_name_and_folder() {
        assert!(is_log("red4ext.log", "logs"));
        assert!(is_log("x.log.12", "mods"));
        assert!(is_log("cet.log.txt", "mods"));
        assert!(is_log("errors.txt", "Logs"));
        assert!(!is_log("readme.txt", "mods"));
        assert!(!is_log("x.log.", "mods"));
        assert!(!is_log("x.log.1a", "mods"));
    }

    #[test]
    fn patterns_match_whole_names() {
        assert!(glob(b"*.csv", b"vendors.csv"));
        assert!(!glob(b"*.csv", b"vendors.csvx"));
        assert!(glob(b"v?ndors.*", b"vendors.csv"));
        assert!(glob(b"*", b""));
        assert!(!glob(b"?", b""));
    }

    #[test]
    fn extra_paths_stay_inside_the_game_folder() {
        let game = Path::new("game");
        for refused in ["../outside/*.txt", "/etc/*", "C:/Users/*.txt", "bin/*/x.csv", "a/../../b.log"] {
            assert!(matches!(extra(game, true, refused).look, Look::Refused), "{refused}");
        }
        let pattern = extra(game, true, r"bin\x64\StreetVendors\*.csv");
        assert!(matches!(&pattern.look, Look::Pattern(p) if p == "*.csv"));
        assert_eq!(pattern.label, "extra/bin/x64/StreetVendors");
        let folder = extra(game, true, "r6/config");
        assert!(matches!(folder.look, Look::Path));
        assert_eq!(folder.label, "extra/r6/config");
    }
}
