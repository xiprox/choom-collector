//! Where mods live in the game folder, which of their files stay out of sharing, and the checks
//! every path from another PC passes before anything is written.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};


/// The game's mod places (record/areas/product/02-v1-scope.md#finding-mods). `Place` in
/// `src/core/src/state.rs` lists the same seven for the window, so change both together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Place {
    Cet,
    Red4ext,
    Redscript,
    TweakXl,
    InputLoader,
    Archive,
    Redmod,
}

impl Place {
    pub const ALL: [Place; 7] = [
        Place::Cet,
        Place::Red4ext,
        Place::Redscript,
        Place::TweakXl,
        Place::InputLoader,
        Place::Archive,
        Place::Redmod,
    ];

    /// The folder, relative to the game folder.
    pub fn folder(self) -> &'static str {
        match self {
            Place::Cet => "bin/x64/plugins/cyber_engine_tweaks/mods",
            Place::Red4ext => "red4ext/plugins",
            Place::Redscript => "r6/scripts",
            Place::TweakXl => "r6/tweaks",
            Place::InputLoader => "r6/input",
            Place::Archive => "archive/pc/mod",
            Place::Redmod => "mods",
        }
    }

    /// The name a player knows the kind of mod by.
    pub fn label(self) -> &'static str {
        match self {
            Place::Cet => "CET mod",
            Place::Red4ext => "RED4ext plugin",
            Place::Redscript => "redscript",
            Place::TweakXl => "TweakXL",
            Place::InputLoader => "Input Loader",
            Place::Archive => "archive",
            Place::Redmod => "REDmod",
        }
    }

    /// The place a mod folder such as `r6/scripts/EquipmentEx` sits in.
    pub fn of(folder: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| folder.len() > p.folder().len() && folder.starts_with(p.folder()) && folder.as_bytes()[p.folder().len()] == b'/')
    }
}

/// Files the game or a mod writes while it runs, which are never sent and never deleted from a
/// follower's copy, and the app's own files on their way. A pattern ending in `/` is a folder at
/// any depth.
pub const DEFAULT_EXCLUDES: &str = "*.log *.log.* logs/ db.sqlite3 *.sqlite3-journal *.tmp *.bak .git/ __folder_managed_by_vortex *.vortex_backup install-id.txt name.txt *.coop-tmp";

/// A file on its way to a follower is written beside the file it replaces, under this suffix,
/// which [`DEFAULT_EXCLUDES`] leaves out.
pub const TMP_SUFFIX: &str = ".coop-tmp";

/// Tells whether a path inside a mod folder is left out by a list of patterns.
#[derive(Clone, Debug)]
pub struct Excluder {
    files: Vec<Vec<char>>,
    dirs: Vec<Vec<char>>,
}

impl Excluder {
    /// Each item can hold several patterns, split by spaces, commas, semicolons, or line breaks.
    pub fn new<'a>(patterns: impl IntoIterator<Item = &'a str>) -> Self {
        let mut ex = Excluder { files: Vec::new(), dirs: Vec::new() };
        for item in patterns {
            for p in item.split([' ', ';', ',', '\t', '\r', '\n']).filter(|p| !p.is_empty()) {
                let glob: Vec<char> = p.trim_end_matches('/').to_lowercase().chars().collect();
                if glob.is_empty() {
                    continue;
                }
                if p.ends_with('/') { ex.dirs.push(glob) } else { ex.files.push(glob) }
            }
        }
        ex
    }

    /// The defaults, plus a sharer's own patterns.
    pub fn with_defaults(own: &[String]) -> Self {
        Self::new(std::iter::once(DEFAULT_EXCLUDES).chain(own.iter().map(String::as_str)))
    }

    /// `rel` is a path inside a mod folder, with a folder's ending in `/`, such as `logs/` or
    /// `scripts/init.lua`.
    pub fn excludes(&self, rel: &str) -> bool {
        let is_dir = rel.ends_with('/');
        let segs: Vec<&str> = rel.trim_end_matches('/').split('/').collect();
        for (i, seg) in segs.iter().enumerate() {
            let last = i + 1 == segs.len();
            let name: Vec<char> = seg.to_lowercase().chars().collect();
            if (!last || is_dir) && self.dirs.iter().any(|g| glob(g, &name)) {
                return true;
            }
            if last && !is_dir && self.files.iter().any(|g| glob(g, &name)) {
                return true;
            }
        }
        false
    }
}

/// `*` and `?` against one name. Patterns can come from another PC, so this backtracks only to
/// the last `*`, which keeps any pattern linear in the name's length times its own.
fn glob(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, n));
            p += 1;
        } else if let Some((sp, sn)) = star {
            p = sp + 1;
            n = sn + 1;
            star = Some((sp, sn + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// The mod folder a game-relative path from another PC belongs to, such as `r6/scripts/EquipmentEx`
/// for `r6/scripts/EquipmentEx/module.reds`, with the place spelled as [`Place::folder`] spells
/// it. `None` when the path could reach anything outside the mod places: `..`, a drive letter, a
/// leading `/`, or a character Windows doesn't allow in a name.
pub fn folder_of(rel: &str) -> Option<String> {
    if rel.is_empty() || rel.chars().count() > 400 || rel.contains('\\') || rel.contains(':') {
        return None;
    }
    let segs: Vec<&str> = rel.split('/').collect();
    let bad = |s: &str| {
        s.trim().is_empty() || s == "." || s == ".." || s.ends_with('.') || s.ends_with(' ') || s.chars().any(|c| (c as u32) < 32 || "<>\"|?*".contains(c))
    };
    if segs.iter().any(|s| bad(s)) {
        return None;
    }
    for place in Place::ALL {
        let n = place.folder().split('/').count();
        if segs.len() > n && segs[..n].join("/").eq_ignore_ascii_case(place.folder()) {
            return Some(format!("{}/{}", place.folder(), segs[n]));
        }
    }
    None
}

/// A game-relative path on this PC. On Linux, a folder or file that exists with other capitals
/// is used as it is, as the game, through Proton, doesn't care about capitals.
pub fn local(game: &Path, rel: &str) -> PathBuf {
    let mut cur = game.to_path_buf();
    for seg in rel.split('/') {
        let mut next = cur.join(seg);
        if !cfg!(windows) && !next.exists() {
            let lower = seg.to_lowercase();
            if let Ok(entries) = fs::read_dir(&cur) {
                for e in entries.flatten() {
                    if e.file_name().to_str().is_some_and(|n| n.to_lowercase() == lower) {
                        next = e.path();
                        break;
                    }
                }
            }
        }
        cur = next;
    }
    cur
}

/// Mods are shared from and written into a game folder only when it's there.
pub fn usable(game: &Path) -> bool {
    game.is_absolute() && game.is_dir()
}

/// A file in a mod folder on this PC.
#[derive(Clone, Debug)]
pub struct LocalFile {
    /// Game-relative, with `/`, such as `r6/scripts/EquipmentEx/module.reds`.
    pub path: String,
    pub full: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// Every file under `dir`, through links and junctions, named `prefix/<path inside dir>`.
pub fn walk(dir: &Path, prefix: &str, excluder: &Excluder) -> Vec<LocalFile> {
    let mut out = Vec::new();
    walk_into(dir, prefix, "", excluder, &mut out, 0);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn walk_into(dir: &Path, prefix: &str, rel: &str, excluder: &Excluder, out: &mut Vec<LocalFile>, depth: u32) {
    // A link back up the tree would loop forever.
    if depth > 24 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
        let Some(meta) = followed_metadata(&e) else { continue };
        if meta.is_dir() {
            let r = format!("{rel}{name}/");
            if !excluder.excludes(&r) {
                walk_into(&e.path(), prefix, &r, excluder, out, depth + 1);
            }
        } else if meta.is_file() {
            let r = format!("{rel}{name}");
            if !excluder.excludes(&r) {
                out.push(LocalFile { path: format!("{prefix}/{r}"), full: e.path(), size: meta.len(), modified: meta.modified().ok() });
            }
        }
    }
}

/// A directory entry's metadata, through a link. On Windows, `DirEntry::metadata` comes from the
/// directory listing without opening the file, which keeps a walk of thousands of files fast.
fn followed_metadata(e: &fs::DirEntry) -> Option<fs::Metadata> {
    let link = e.file_type().ok()?.is_symlink();
    if link { fs::metadata(e.path()).ok() } else { e.metadata().ok() }
}

/// A top-level folder in one of the mod places on this PC.
#[derive(Clone, Debug)]
pub struct LocalFolder {
    pub place: Place,
    pub name: String,
    /// Game-relative, with the place spelled as [`Place::folder`] spells it.
    pub path: String,
    pub full: PathBuf,
}

/// The top-level folders in every mod place, in place order and then by name, capitals aside.
pub fn find_folders(game: &Path, excluder: &Excluder) -> Vec<LocalFolder> {
    let mut list = Vec::new();
    for place in Place::ALL {
        let Ok(entries) = fs::read_dir(local(game, place.folder())) else { continue };
        let mut here: Vec<LocalFolder> = entries
            .flatten()
            .filter(|e| followed_metadata(e).is_some_and(|m| m.is_dir()))
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_string();
                let path = format!("{}/{name}", place.folder());
                (!excluder.excludes(&format!("{name}/")) && folder_of(&path).as_deref() == Some(path.as_str())).then(|| LocalFolder { place, name, path, full: e.path() })
            })
            .collect();
        here.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.name.cmp(&b.name)));
        list.extend(here);
    }
    list
}

/// Files lying directly in a mod place, game-relative. They can't be shared, as only folders are.
pub fn find_loose(game: &Path, excluder: &Excluder) -> Vec<String> {
    let mut list = Vec::new();
    for place in Place::ALL {
        let Ok(entries) = fs::read_dir(local(game, place.folder())) else { continue };
        let mut here: Vec<String> = entries
            .flatten()
            .filter(|e| followed_metadata(e).is_some_and(|m| m.is_file()))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|name| !excluder.excludes(name))
            .map(|name| format!("{}/{name}", place.folder()))
            .collect();
        here.sort();
        list.extend(here);
    }
    list
}

/// Whether a loose file looks like part of the mod of this name: its name starts with the mod's,
/// after whatever is put in front to set the load order, and that's a whole word, as
/// `#EquipmentEx-patch.archive` is for `EquipmentEx`.
pub fn loose_belongs(file: &str, mod_name: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file).trim_start_matches(['!', '#', '-', '_', ' ']);
    let (lower, m) = (name.to_lowercase(), mod_name.to_lowercase());
    lower.len() > m.len() && lower.starts_with(&m) && lower[m.len()..].chars().next().is_some_and(|c| !c.is_alphanumeric())
}

/// The last part of a game-relative path: the mod's name for a folder.
pub fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Stands for one PC in the list of shared folders without giving away its install ID, which is
/// how the host knows a returning tester.
pub fn pc_id(install_id: &str) -> String {
    Sha256::digest(install_id.as_bytes())[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Code that runs inside the game, as opposed to scripts and data.
pub fn has_code(path: &str) -> bool {
    let lower = path.to_lowercase();
    [".dll", ".asi", ".exe"].iter().any(|e| lower.ends_with(e))
}

/// Takes a mod folder off this PC, all of it, the files the game wrote there included. Returns how
/// many files are still there, which on Windows means the running game holds them.
pub fn delete_folder(game: &Path, folder: &str) -> usize {
    let dir = local(game, folder);
    if fs::symlink_metadata(&dir).is_err() {
        return 0;
    }
    delete_tree(&dir)
}

/// Unlike [`walk`], this stops at a link to a folder and removes only the link, so what it points
/// at, such as a modder's working copy of the mod, isn't touched.
fn delete_tree(dir: &Path) -> usize {
    let Ok(meta) = fs::symlink_metadata(dir) else { return 0 };
    if meta.file_type().is_symlink() {
        return usize::from(remove_link(dir).is_err());
    }
    let mut left = 0;
    match fs::read_dir(dir) {
        Ok(entries) => {
            for e in entries.flatten() {
                let path = e.path();
                let Ok(kind) = e.file_type() else {
                    left += 1;
                    continue;
                };
                if kind.is_dir() {
                    left += delete_tree(&path);
                } else if kind.is_symlink() && fs::metadata(&path).is_ok_and(|m| m.is_dir()) {
                    left += usize::from(remove_link(&path).is_err());
                } else if fs::remove_file(&path).is_err() {
                    left += 1;
                }
            }
        }
        Err(_) => left += 1,
    }
    if left == 0 && fs::remove_dir(dir).is_err() {
        left += 1;
    }
    left
}

/// Windows removes a link to a folder, symlink or junction, as a folder, and Linux as a file.
fn remove_link(path: &Path) -> std::io::Result<()> {
    fs::remove_dir(path).or_else(|_| fs::remove_file(path))
}

/// Deletes the `*.coop-tmp` files a run that stopped mid-file left in a folder.
pub fn remove_tmp_files(dir: &Path, depth: u32) {
    if depth > 24 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            remove_tmp_files(&e.path(), depth + 1);
        } else if kind.is_file() && e.file_name().to_string_lossy().ends_with(TMP_SUFFIX) {
            let _ = fs::remove_file(e.path());
        }
    }
}

/// Removes the empty folders inside a mod folder, after files in them were deleted.
pub fn remove_empty_folders(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            remove_empty_folders(&e.path());
            if fs::read_dir(e.path()).is_ok_and(|mut d| d.next().is_none()) {
                let _ = fs::remove_dir(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_names_and_folders_at_any_depth() {
        let ex = Excluder::with_defaults(&["*.psd".into(), "build/".into()]);
        assert!(ex.excludes("CyberEngineTweaks.log"));
        assert!(ex.excludes("x.log.1"));
        assert!(ex.excludes("logs/today.txt"));
        assert!(ex.excludes("deep/logs/today.txt"));
        assert!(ex.excludes("db.sqlite3"));
        assert!(ex.excludes("art/Icon.PSD"));
        assert!(ex.excludes("build/"));
        assert!(ex.excludes("src/build/out.bin"));
        assert!(!ex.excludes("init.lua"));
        assert!(!ex.excludes("logs"), "a file named logs isn't the logs folder");
        assert!(!ex.excludes("catalog.txt"));
    }

    #[test]
    fn glob_handles_stars_and_question_marks() {
        let g = |p: &str, n: &str| glob(&p.chars().collect::<Vec<_>>(), &n.chars().collect::<Vec<_>>());
        assert!(g("*.log", "a.log"));
        assert!(g("*.log.*", "a.log.1"));
        assert!(!g("*.log", "a.log.1"));
        assert!(g("a?c", "abc"));
        assert!(g("*a*b*c*", "xxaxxbxxcxx"));
        assert!(!g("*a*b*c", "xxaxxbxx"));
        assert!(g("*", ""));
        let long: String = "a".repeat(5000);
        assert!(!g(&"*a".repeat(50).chars().chain("b".chars()).collect::<String>(), &long));
    }

    #[test]
    fn paths_from_another_pc_stay_inside_a_mod_folder() {
        assert_eq!(folder_of("r6/scripts/EquipmentEx/module.reds").as_deref(), Some("r6/scripts/EquipmentEx"));
        assert_eq!(folder_of("R6/Scripts/EquipmentEx").as_deref(), Some("r6/scripts/EquipmentEx"));
        assert_eq!(folder_of("archive/pc/mod/a/b/c.archive").as_deref(), Some("archive/pc/mod/a"));
        for bad in [
            "",
            "r6/scripts",
            "r6/scripts/",
            "r6/scripts/../../bin/x64/evil.dll",
            "r6/scripts/X/../../../evil",
            "r6/scripts/./X",
            "/etc/passwd",
            "/r6/scripts/X",
            "C:/Windows/System32",
            "r6/scripts/X:stream",
            "r6\\scripts\\X",
            "r6/scripts/X /a",
            "r6/scripts/X./a",
            "r6/scripts/X/a?b",
            "r6/scripts/X/a\u{1}b",
            "bin/x64/evil.dll",
            "r6/scriptsX/a",
        ] {
            assert_eq!(folder_of(bad), None, "{bad:?} got through");
        }
        assert_eq!(folder_of(&format!("r6/scripts/{}", "a".repeat(400))), None);
    }

    #[test]
    fn loose_files_belong_to_the_mod_they_are_named_after() {
        assert!(loose_belongs("archive/pc/mod/EquipmentEx.archive", "EquipmentEx"));
        assert!(loose_belongs("archive/pc/mod/#EquipmentEx-patch.archive", "equipmentex"));
        assert!(!loose_belongs("archive/pc/mod/EquipmentExtra.archive", "EquipmentEx"));
        assert!(!loose_belongs("archive/pc/mod/EquipmentEx", "EquipmentEx"));
    }

    #[test]
    fn a_pc_id_is_16_hex_digits_and_stable() {
        let id = pc_id("a1b2");
        assert_eq!(id.len(), 16);
        assert_eq!(id, pc_id("a1b2"));
        assert_ne!(id, pc_id("a1b3"));
    }
}
