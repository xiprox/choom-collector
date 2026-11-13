//! Finding and following logs on a player's PC, and writing them into the host's folder
//! (record/areas/logs/01-collecting.md).
//!
//! Every player's PC runs a [`Sender`], the host's too, and the host runs a [`Collector`]. Neither
//! touches the network: the core carries the `Sender`'s messages to the host in `ToHost::Logs`,
//! and what the `Collector` already has of a player's logs back to them in `Welcome.logs`.

mod collector;
mod host_paths;
mod ident;
mod scan;
mod sender;
mod wire;

use serde::{Deserialize, Serialize};

pub use collector::{Collector, PeerInfo};
pub use sender::{Progress, Sender, SenderConfig};

/// Where on a player's PC a log was picked up.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SourceKind {
    GameFolder,
    CrashReports,
    /// `CD Projekt Red/Cyberpunk 2077` in the game's `%LOCALAPPDATA%`.
    GameOwn,
    AppLog,
    /// One of the host's extra paths, as the host wrote it.
    Extra(String),
}

/// How many files one source picked up in the player's latest full scan.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SourceCount {
    pub kind: SourceKind,
    pub count: u32,
}
