//! Settings, finding the game, the relay client, the session and its players, and passing messages
//! to logs and mods. The window talks to it through [`start`], sending [`state::Command`]s and
//! drawing the [`state::Snapshot`]s it sends back.

mod applog;
pub mod demo;
pub mod game;
pub mod relay;
mod session;
pub mod settings;
pub mod state;

use std::path::PathBuf;

pub use demo::DemoScreen;
use state::{Command, Snapshot};

pub struct Options {
    /// The program's version, which the window shows; the core's own crate has none of its own.
    pub app_version: String,
    /// Where settings, the app's log, and the host's cache live; `None` for the usual place.
    pub data_dir: Option<PathBuf>,
    /// A made-up session instead of a real one.
    pub demo: Option<DemoScreen>,
    /// The name to show up as instead of the Steam name, for test runs of several copies on one PC.
    pub name: Option<String>,
    /// The folder to read as the game's `%LOCALAPPDATA%`, for test runs that keep off the real
    /// game's crash reports and logs.
    pub local_app_data: Option<PathBuf>,
}

/// Starts the core, and returns the channel the window sends commands on and the channel it
/// receives snapshots from.
pub fn start(options: Options) -> (async_channel::Sender<Command>, async_channel::Receiver<Snapshot>) {
    match options.demo {
        Some(screen) => demo::start(screen, options.app_version),
        None => {
            let data_dir = match options.data_dir {
                Some(dir) => dir,
                None => {
                    // Only the usual folder moves; a test run's --data-dir is left as it is.
                    settings::move_old_data_dir();
                    settings::default_data_dir()
                }
            };
            let app_log = applog::start(&data_dir);
            log::info!("Choom Collector {} starting, data in {}", options.app_version, data_dir.display());
            session::start(data_dir, app_log, options.app_version, options.name, options.local_app_data)
        }
    }
}
