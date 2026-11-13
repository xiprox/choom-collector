//! Whether Cyberpunk 2077 is running on this PC. The game loads mods only when it starts, so a
//! change that lands while it runs needs a restart, and the follower tells its sharer so
//! (record/areas/mods/01-sharing.md#held-files-and-game-restarts).

const PROGRAM: &str = "Cyberpunk2077.exe";

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
