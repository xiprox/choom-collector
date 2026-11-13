//! Settings, finding the game, the relay client, the session and its players, and passing messages
//! to logs and mods. The window talks to it through [`start`], sending [`state::Command`]s and
//! drawing the [`state::Snapshot`]s it sends back.

pub mod demo;
pub mod state;

use std::path::PathBuf;

pub use demo::DemoScreen;
use state::{Command, Snapshot};

pub struct Options {
    /// The program's version, which the window shows; the core's own crate has none of its own.
    pub app_version: String,
    /// Where settings, the app's log, backups, and the host's cache live; `None` for the usual place.
    pub data_dir: Option<PathBuf>,
    /// A made-up session instead of a real one.
    pub demo: Option<DemoScreen>,
}

/// Starts the core, and returns the channel the window sends commands on and the channel it
/// receives snapshots from.
pub fn start(options: Options) -> (async_channel::Sender<Command>, async_channel::Receiver<Snapshot>) {
    // Until stage 4 of record/areas/product/plan.md builds the real session, every run is a demo,
    // and a run without --demo starts at the first run's setup.
    demo::start(options.demo.unwrap_or(DemoScreen::SetupJoin), options.app_version)
}
