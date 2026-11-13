//! Finding Cyberpunk 2077 on this PC, its version and store, the player's Steam name, and the
//! system, the way CP77 Log Collector does (read in CP77 Log Collector, src/Settings.cs, GameFinder).

use std::path::{Path, PathBuf};

use crate::state::GameInstall;

/// Proton names the game's prefix after it, on Linux.
#[cfg(not(windows))]
const STEAM_APP_ID: &str = "1091500";

pub fn is_game_folder(dir: &Path) -> bool {
    dir.join("bin").join("x64").join("Cyberpunk2077.exe").is_file()
}

/// The game's folder, from the first place that has the game: the app's own folder and the four
/// above it (an app dropped into the game folder), every Steam library, GOG's and Epic's records on
/// Windows, and Heroic's and Lutris's folders on Linux.
pub fn find() -> Option<GameInstall> {
    candidates().into_iter().find(|c| is_game_folder(c)).map(|folder| describe(&folder))
}

/// What a game folder holds: its version, read from the game's program, and its store.
pub fn describe(folder: &Path) -> GameInstall {
    GameInstall { folder: folder.to_path_buf(), version: version(folder) }
}

/// The store the game came from, told from where it's installed.
pub fn store(folder: &Path) -> &'static str {
    let path = folder.to_string_lossy().to_lowercase();
    if path.contains("steamapps") {
        "Steam"
    } else if path.contains("gog") || path.contains("heroic") {
        "GOG"
    } else if path.contains("epic") {
        "Epic"
    } else {
        ""
    }
}

/// The game program's product version, such as `2.31`, which Proton's copy on Linux has too, since
/// it's the same Windows program.
pub fn version(folder: &Path) -> Option<String> {
    use pelite::pe64::{Pe as _, PeFile};
    let exe = folder.join("bin").join("x64").join("Cyberpunk2077.exe");
    let map = pelite::FileMap::open(&exe).ok()?;
    let file = PeFile::from_bytes(&map).ok()?;
    let info = file.resources().ok()?.version_info().ok()?;
    let lang = *info.translation().first()?;
    info.value(lang, "ProductVersion").map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn candidates() -> Vec<PathBuf> {
    let mut list = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        list.extend(exe.ancestors().skip(1).take(5).map(Path::to_path_buf));
    }
    for library in steam_libraries() {
        list.push(library.join("steamapps").join("common").join("Cyberpunk 2077"));
    }
    #[cfg(windows)]
    list.extend(windows::gog_and_epic());
    #[cfg(not(windows))]
    if let Some(home) = dirs::home_dir() {
        list.push(home.join("Games").join("Heroic").join("Cyberpunk 2077"));
        list.push(home.join("Games").join("cyberpunk-2077").join("drive_c").join("GOG Games").join("Cyberpunk 2077"));
    }
    list
}

/// Steam's own folders: from the registry on Windows, and on Linux native Steam, its `~/.steam`
/// links, Flatpak, and Snap.
pub fn steam_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        windows::steam_root().into_iter().collect()
    }
    #[cfg(not(windows))]
    {
        let Some(home) = dirs::home_dir() else { return Vec::new() };
        let mut roots: Vec<PathBuf> = [
            ".steam/steam",
            ".steam/root",
            ".local/share/Steam",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            ".var/app/com.valvesoftware.Steam/data/Steam",
            "snap/steam/common/.local/share/Steam",
        ]
        .iter()
        .filter_map(|rel| std::fs::canonicalize(home.join(rel)).ok())
        .collect();
        roots.dedup();
        roots
    }
}

/// Steam's folders plus every library `steamapps/libraryfolders.vdf` lists.
pub fn steam_libraries() -> Vec<PathBuf> {
    let mut libraries = Vec::new();
    for root in steam_roots() {
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
            libraries.extend(vdf_values(&vdf, "path").map(|p| PathBuf::from(p.replace("\\\\", "\\"))));
        }
        libraries.push(root);
    }
    libraries.dedup();
    libraries
}

/// Every value of `key` in a Valve data file: `"key"  "value"`, at any depth.
fn vdf_values<'a>(text: &'a str, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let needle = format!("\"{key}\"");
    text.lines().filter_map(move |line| {
        let rest = line.trim_start().strip_prefix(needle.as_str())?;
        let value = rest.trim().strip_prefix('"')?;
        value.rfind('"').map(|end| &value[..end])
    })
}

/// The Steam name of whoever signed in to Steam last on this PC: the `PersonaName` of the account
/// marked `MostRecent` in `config/loginusers.vdf`, or the first account there.
pub fn steam_name() -> Option<String> {
    for root in steam_roots() {
        let Ok(text) = std::fs::read_to_string(root.join("config").join("loginusers.vdf")) else { continue };
        let mut first = None;
        // Each account is a block opened by its 17-digit Steam ID.
        for block in text.split('{').skip(2) {
            let block = block.split('}').next().unwrap_or_default();
            let Some(name) = vdf_values(block, "PersonaName").next().map(str::trim).filter(|n| !n.is_empty()) else { continue };
            if vdf_values(block, "MostRecent").next() == Some("1") {
                return Some(name.to_string());
            }
            first.get_or_insert(name.to_string());
        }
        if first.is_some() {
            return first;
        }
    }
    None
}

/// The name a player shows up as: their Steam name, or else the system's user name.
pub fn player_name() -> String {
    steam_name()
        .or_else(|| std::env::var("USERNAME").ok())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "Player".into())
}

/// The system as players know it, such as `Windows 11` or `SteamOS`.
pub fn system() -> String {
    #[cfg(windows)]
    {
        windows::system()
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|text| text.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
            .unwrap_or_else(|| "Linux".into())
    }
}

/// Where the game's `%LOCALAPPDATA%` is: the user's own on Windows, and on Linux the one inside the
/// Windows folder tree Proton keeps for the game.
pub fn local_app_data(game_folder: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let _ = game_folder;
        dirs::data_local_dir()
    }
    #[cfg(not(windows))]
    {
        let mut prefixes = Vec::new();
        // <library>/steamapps/common/Cyberpunk 2077 has its prefix in <library>/steamapps/compatdata/1091500/pfx.
        if let Some(steamapps) = game_folder.parent().and_then(Path::parent) {
            prefixes.push(steamapps.join("compatdata").join(STEAM_APP_ID).join("pfx"));
        }
        for library in steam_libraries() {
            prefixes.push(library.join("steamapps").join("compatdata").join(STEAM_APP_ID).join("pfx"));
        }
        if let Some(home) = dirs::home_dir() {
            prefixes.push(home.join("Games/Heroic/Prefixes/default/Cyberpunk 2077"));
            prefixes.push(home.join("Games/Heroic/Prefixes/Cyberpunk 2077"));
        }
        for prefix in prefixes {
            let users = prefix.join("drive_c").join("users");
            // Proton's Windows user is steamuser; other Wine setups use the Linux user's name.
            let mut names = vec!["steamuser".to_string()];
            names.extend(std::env::var("USER").ok());
            if let Ok(entries) = std::fs::read_dir(&users) {
                names.extend(entries.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n != "Public"));
            }
            for name in names {
                let local = users.join(name).join("AppData").join("Local");
                if local.is_dir() {
                    return Some(local);
                }
            }
        }
        None
    }
}

#[cfg(windows)]
mod windows {
    use std::path::PathBuf;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    pub fn steam_root() -> Option<PathBuf> {
        let user: Option<String> = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").and_then(|k| k.get_value("SteamPath")).ok();
        let machine = || -> Option<String> {
            RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam").and_then(|k| k.get_value("InstallPath")).ok()
        };
        user.or_else(machine).map(|p| PathBuf::from(p.replace('/', "\\")))
    }

    /// GOG's install record, and the Epic launcher's manifests that mention the game.
    pub fn gog_and_epic() -> Vec<PathBuf> {
        let mut list = Vec::new();
        for key in [r"SOFTWARE\WOW6432Node\GOG.com\Games\1423049311", r"SOFTWARE\GOG.com\Games\1423049311"] {
            if let Ok(path) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(key).and_then(|k| k.get_value::<String, _>("path")) {
                list.push(PathBuf::from(path));
            }
        }
        let manifests = PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into())).join(r"Epic\EpicGamesLauncher\Data\Manifests");
        if let Ok(entries) = std::fs::read_dir(manifests) {
            for entry in entries.flatten() {
                let Ok(json) = std::fs::read_to_string(entry.path()) else { continue };
                if !json.contains("Cyberpunk") {
                    continue;
                }
                if let Some(location) = serde_json::from_str::<serde_json::Value>(&json).ok().and_then(|v| v["InstallLocation"].as_str().map(PathBuf::from)) {
                    list.push(location);
                }
            }
        }
        list
    }

    /// Windows 11 reports itself as Windows 10 in `ProductName`, so the build number tells them apart.
    pub fn system() -> String {
        let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        let build: Option<u32> = key.as_ref().ok().and_then(|k| k.get_value::<String, _>("CurrentBuildNumber").ok()).and_then(|b| b.parse().ok());
        match build {
            Some(b) if b >= 22000 => "Windows 11".into(),
            Some(_) => "Windows 10".into(),
            None => "Windows".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_paths_come_out_of_the_vdf() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n";
        let paths: Vec<_> = vdf_values(vdf, "path").collect();
        assert_eq!(paths, ["C:\\\\Program Files (x86)\\\\Steam", "D:\\\\SteamLibrary"]);
    }

    /// Prints what this PC has, for a look by eye: `cargo test -p coop-core what_this_pc_has -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads this PC's game and Steam"]
    fn what_this_pc_has() {
        let game = find();
        println!("game: {:?}", game.as_ref().map(|g| (&g.folder, &g.version)));
        println!("store: {:?}", game.as_ref().map(|g| store(&g.folder)));
        println!("local app data: {:?}", game.as_ref().and_then(|g| local_app_data(&g.folder)));
        println!("steam name found: {}", steam_name().is_some());
        println!("system: {}", system());
    }

    #[test]
    fn the_store_comes_from_the_folder() {
        assert_eq!(store(Path::new(r"D:\SteamLibrary\steamapps\common\Cyberpunk 2077")), "Steam");
        assert_eq!(store(Path::new(r"C:\GOG Games\Cyberpunk 2077")), "GOG");
    }
}
