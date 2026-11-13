//! The app's own log, `app.log` in the data folder: everything the app does, a line at a time, so
//! it's there even when the app dies, and so a tester's app log reaches the host with their logs.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Past this size the log moves to `app.1.log`, replacing the one before, so it never grows
/// without end.
const KEEP_BYTES: u64 = 5 * 1024 * 1024;

struct FileLog {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl log::Log for FileLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info || metadata.target().starts_with("coop")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!("{} {:<5} {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), record.level(), record.args());
        let Ok(mut guard) = self.file.lock() else { return };
        if guard.as_ref().and_then(|f| f.metadata().ok()).is_some_and(|m| m.len() > KEEP_BYTES) {
            *guard = None;
            let _ = std::fs::rename(&self.path, self.path.with_file_name("app.1.log"));
        }
        if guard.is_none() {
            *guard = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

/// Starts logging to `app.log` in the data folder, and logs a panic before the app dies.
/// Returns the log's path, which the app's own logs include.
pub fn start(data_dir: &Path) -> PathBuf {
    let path = data_dir.join("app.log");
    let _ = std::fs::create_dir_all(data_dir);
    let logger = Box::leak(Box::new(FileLog { path: path.clone(), file: Mutex::new(None) }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}\n{}", std::backtrace::Backtrace::force_capture());
        previous(info);
    }));
    path
}
