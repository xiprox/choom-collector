//! What the window shows: a snapshot of the app's state, which the core rebuilds and sends to the
//! window whenever something changes (record/areas/architecture/01-spine.md).

use std::path::PathBuf;

use chrono::{DateTime, Local};

/// One snapshot of everything the window draws.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub app_version: String,
    pub screen: Screen,
}

#[derive(Clone, Debug)]
pub enum Screen {
    Setup(Setup),
    Session(Session),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Host,
    Tester,
}

/// The first run's answers so far, and what the app found on its own.
#[derive(Clone, Debug)]
pub struct Setup {
    pub role: Role,
    /// What a tester typed or pasted: a code, or `relay/code`.
    pub invite: String,
    pub relay: String,
    /// The code the host's PC picked, which stays the same session after session.
    pub host_code: String,
    pub logs_folder: PathBuf,
    pub game: Option<GameInstall>,
    pub steam_name: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct GameInstall {
    pub folder: PathBuf,
    pub version: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub role: Role,
    pub code: String,
    pub relay: String,
    pub link: Link,
    pub host_name: String,
    pub players: Vec<Player>,
    pub shared: Vec<SharedMod>,
    pub my_mods: Vec<MyMod>,
    pub logs: LogsLine,
}

impl Session {
    pub fn me(&self) -> Option<&Player> {
        self.players.iter().find(|p| p.is_me)
    }

    pub fn host(&self) -> Option<&Player> {
        self.players.iter().find(|p| p.is_host)
    }
}

/// This PC's link to the relay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    Connecting,
    Connected { latency_ms: u32 },
    /// A tester whose host isn't hosting yet.
    WaitingForHost,
    Reconnecting,
}

/// What the status bar says about logs: where they land for the host, how sending goes for a tester.
#[derive(Clone, Debug)]
pub enum LogsLine {
    Host { folder: PathBuf },
    Tester { host: String, sending: Sending },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sending {
    CatchingUp { done: u32, total: u32 },
    UpToDate,
    Paused,
}

pub type PlayerId = u64;

#[derive(Clone, Debug)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub is_me: bool,
    pub is_host: bool,
    pub game_version: String,
    pub store: String,
    pub system: String,
    pub joined: DateTime<Local>,
    pub resumed: bool,
    /// What the host received from this player, or, on a tester's own row, what they sent.
    pub bytes: u64,
    pub state: PlayerState,
    /// The logs picked up on this player's PC, which the host sees for everyone and a tester for
    /// themselves only.
    pub sources: Option<Vec<LogSource>>,
}

/// The one line of state a player's row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerState {
    Fine,
    /// Fetching the host's shared mods.
    Fetching { done: u32, total: u32 },
    NeedsRestart,
    Reconnecting,
}

#[derive(Clone, Debug)]
pub struct LogSource {
    pub kind: SourceKind,
    pub count: u32,
}

#[derive(Clone, Debug)]
pub enum SourceKind {
    GameFolder,
    CrashReports,
    GameOwn,
    /// An extra path the host listed, relative to the game folder.
    Extra { pattern: String },
}

/// A mod someone in the session shares, the player's own included.
#[derive(Clone, Debug)]
pub struct SharedMod {
    /// The mod's name in lower case, which is what makes same-named folders one mod.
    pub key: String,
    pub name: String,
    pub sharer: String,
    pub mine: bool,
    pub follow: Follow,
    pub has_dll: bool,
    pub status: ModStatus,
    pub changed: DateTime<Local>,
    /// Each follower's state, for a mod the player shares.
    pub followers: Vec<Follower>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    No,
    Yes,
    /// Following, and the sharer added a folder the player hasn't agreed to yet.
    AddedFolder,
}

/// This player's state for a shared mod.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModStatus {
    UpToDate,
    Fetching { done: u32, total: u32 },
    NeedsRestart,
    NotOnYourPc,
    AddedFolder,
    Paused,
    /// A mod the player shares: how many followers are up to date.
    Sharing,
}

#[derive(Clone, Debug)]
pub struct Follower {
    pub name: String,
    pub delivery: Delivery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delivery {
    UpToDate,
    Fetching { done: u32, total: u32 },
    NeedsRestart,
    Paused,
}

/// A mod in the player's own game folder.
#[derive(Clone, Debug)]
pub struct MyMod {
    pub key: String,
    pub name: String,
    pub places: Vec<Place>,
    pub bytes: u64,
    pub shared: bool,
    /// A file named after the mod that sits loose in a mod place, such as
    /// `archive/pc/mod/EquipmentEx.archive`, and so isn't part of it.
    pub loose_file: Option<String>,
}

/// The game's mod places (record/areas/product/02-v1-scope.md#finding-mods).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
}

/// What the window asks the core to do.
#[derive(Clone, Debug)]
pub enum Command {
    Share { key: String, on: bool },
    Follow { key: String, on: bool },
    Unfollow { key: String, delete_copy: bool },
    TakeAddedFolder { key: String },
    FinishSetup(SetupAnswers),
    NewCode,
    Stop,
    Leave,
}

#[derive(Clone, Debug)]
pub struct SetupAnswers {
    pub role: Role,
    pub invite: String,
    pub relay: String,
    pub logs_folder: PathBuf,
    pub game_folder: Option<PathBuf>,
}
