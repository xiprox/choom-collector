//! Whether Cyberpunk 2077 is running on this PC, and whether CET has reloaded its mods. The game
//! loads mods when it starts, so a change that lands while it runs needs a restart, except in a
//! CET folder, which "Reload all mods" in CET's overlay loads; the follower tells its sharer which
//! (record/areas/mods/01-sharing.md#held-files-and-game-restarts and #cet-reloads).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const PROGRAM: &str = "Cyberpunk2077.exe";

/// CET's log of its Lua mods, relative to the game folder.
const CET_LOG: &str = "bin/x64/plugins/cyber_engine_tweaks/scripting.log";
/// What CET logs once "Reload all mods" has loaded every mod again, after a `Mod <name> loaded!`
/// line for each (measured in five players' logs, record/areas/mods/log.md).
const RELOADED: &[u8] = b"LuaVM: Reloaded all mods!";

/// Follows CET's log for reloads. CET keeps one log across the game's launches and rotates it to
/// `scripting.1.log` once it's large, so this reads only what the log gained since the last look,
/// starting from its end.
#[derive(Default)]
pub struct CetLog {
    /// How far the log has been read; `None` until it's first seen.
    read: Option<u64>,
}

impl CetLog {
    /// Whether CET logged a reload since the last call. A log shorter than what was read was
    /// rotated, so it's read from its start. The log can hold NUL bytes, so it's searched as bytes.
    pub fn reloaded(&mut self, game: &Path) -> bool {
        let Ok(mut file) = File::open(game.join(CET_LOG)) else { return false };
        let len = file.metadata().map_or(0, |m| m.len());
        let from = match self.read {
            None => {
                self.read = Some(len);
                return false;
            }
            Some(read) if read > len => 0,
            Some(read) => read,
        };
        let mut new = Vec::new();
        if from == len || file.seek(SeekFrom::Start(from)).is_err() || file.take(len - from).read_to_end(&mut new).is_err() {
            self.read = Some(from);
            return false;
        }
        // A line CET is still writing waits for the next look.
        let Some(end) = new.iter().rposition(|&b| b == b'\n') else {
            self.read = Some(from);
            return false;
        };
        self.read = Some(from + end as u64 + 1);
        new[..end].windows(RELOADED.len()).any(|w| w == RELOADED)
    }
}

#[cfg(windows)]
pub fn running() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};

    // SAFETY: the snapshot handle is checked before use and closed once, and each entry is a
    // zeroed PROCESSENTRY32W with its size set, as Process32FirstW requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(PROGRAM) {
                found = true;
                break;
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        found
    }
}

/// Under Proton the game is a Wine process, whose first argument is the Windows path of the
/// program, and whose name the kernel cuts to 15 characters, `Cyberpunk2077.e`. Unverified: we
/// haven't read `/proc` while the game runs under Proton.
#[cfg(target_os = "linux")]
pub fn running() -> bool {
    let Ok(procs) = std::fs::read_dir("/proc") else { return false };
    procs.flatten().filter(|e| e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))).any(|e| {
        let first_arg = std::fs::read(e.path().join("cmdline")).ok().map(|c| {
            let arg = c.split(|&b| b == 0).next().unwrap_or_default();
            String::from_utf8_lossy(arg).rsplit(['/', '\\']).next().unwrap_or_default().to_string()
        });
        let comm = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default();
        let comm = comm.trim_end();
        first_arg.is_some_and(|a| a.eq_ignore_ascii_case(PROGRAM)) || (comm.len() >= 15 && PROGRAM.starts_with(comm))
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn running() -> bool {
    false
}
