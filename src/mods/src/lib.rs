//! Finding a player's mods, sharing them, and keeping each follower's copy the same as the
//! sharer's (record/areas/mods/01-sharing.md).
//!
//! Every PC runs a [`Player`], and the host's PC also runs the [`Hub`] that every file goes
//! through. Neither touches the network: the core passes their messages along as opaque bytes, and
//! calls both from one thread.

mod backups;
mod game;
mod hashes;
mod hub;
pub mod layout;
mod player;
mod store;
pub mod wire;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

pub use backups::Backups;
pub use coop_proto::Doing;
pub use hashes::Hash;
pub use hub::Hub;
pub use layout::Place;
pub use player::Player;

/// What a [`Player`] needs to know about its PC.
#[derive(Clone, Debug)]
pub struct PlayerConfig {
    pub game_folder: PathBuf,
    /// The app's data folder, which holds `mods.toml` and `mod-backups/`.
    pub data_dir: PathBuf,
    pub install_id: String,
    pub name: String,
    /// The sharer's own patterns for files to leave out, on top of
    /// [`layout::DEFAULT_EXCLUDES`], such as `*.psd` or `build/`. Followers go by them too.
    pub never_share: Vec<String>,
}

/// Tells the time, which tests replace to move it on by hand.
pub type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Tells whether the game is running on this PC, which tests replace.
pub type GameCheck = Arc<dyn Fn() -> bool + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(Instant::now)
}

/// Whether Cyberpunk 2077 runs on this PC, by its process (record/areas/mods/01-sharing.md#held-files-and-game-restarts).
pub fn game_running() -> bool {
    game::running()
}

/// Who a message from the [`Hub`] goes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The player with this install ID, which may be the host's own [`Player`].
    Player(String),
    /// Every player in the session, the host's own [`Player`] included.
    Everyone,
}

/// A mod in the player's own game folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MyMod {
    /// The name in lower case, which makes same-named folders one mod.
    pub key: String,
    pub name: String,
    pub places: Vec<Place>,
    pub bytes: u64,
    pub shared: bool,
    /// A file named after the mod that sits loose in a mod place, such as
    /// `archive/pc/mod/EquipmentEx.archive`, and so isn't part of it.
    pub loose_file: Option<String>,
}

/// A row of the mods someone in the session shares, the player's own included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMod {
    /// The mod's name in lower case, with `|` and the sharer's PC ID added when two PCs share
    /// parts of one mod and it takes two rows.
    pub key: String,
    pub name: String,
    pub sharer: String,
    pub mine: bool,
    pub follow: Follow,
    /// Whether any of its files run code inside the game: a `.dll`, `.asi`, or `.exe`.
    pub has_dll: bool,
    pub status: ModStatus,
    /// When its files last changed, in seconds since 1970, or 0 when unknown.
    pub changed: i64,
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

/// The player's state for a shared mod.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModStatus {
    /// Followed, and the copy here matches the sharer's.
    UpToDate,
    Fetching { done: u32, total: u32 },
    NeedsRestart,
    /// Followed, and the new files are here, but a program other than the game holds the old ones,
    /// so they land once it lets go.
    Held,
    /// Followed, and the copy here is still being hashed to compare it.
    Checking,
    /// Not followed, and none of its folders are on this PC.
    NotOnYourPc,
    /// Not followed, and this PC has its own copy, the same as the sharer's or not.
    OnYourPc { same: bool },
    /// Following, and the sharer added a folder the player hasn't agreed to yet.
    AddedFolder,
    /// The link to the host is down, this PC's or the sharer's.
    Paused,
    /// The player follows these folders from another PC, whose sharer was `by`, and ticking the
    /// row switches to this sharer's.
    FollowedElsewhere { by: String },
    /// Followed, and nobody shares it now, so the copy stays as it is.
    NotShared,
    /// Not followed, and the player offered their own, which goes out once this sharer stops.
    OfferedToo,
    /// A mod the player shares.
    Sharing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Follower {
    pub name: String,
    pub delivery: Delivery,
}

/// How a mod stands on a follower's PC, as its sharer sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delivery {
    UpToDate,
    Fetching { done: u32, total: u32 },
    /// The follower has the change, and the game they're running loaded the old files.
    NeedsRestart,
    /// The follower has the change, waiting for a program other than the game to let go of the old
    /// files.
    Held,
    /// The follower's link to the host is down.
    Paused,
}

impl Delivery {
    /// One state for several folders: paused or fetching if any is, then held, then a restart if any
    /// needs one.
    pub(crate) fn combine<'a>(all: impl IntoIterator<Item = &'a Delivery>) -> Option<Delivery> {
        let (mut any, mut paused, mut restart, mut held, mut fetching) = (false, false, false, false, None::<(u32, u32)>);
        for d in all {
            any = true;
            match d {
                Delivery::Paused => paused = true,
                Delivery::NeedsRestart => restart = true,
                Delivery::Held => held = true,
                Delivery::Fetching { done, total } => {
                    let (d0, t0) = fetching.unwrap_or((0, 0));
                    fetching = Some((d0 + done, t0 + total));
                }
                Delivery::UpToDate => {}
            }
        }
        Some(match (any, paused, fetching, restart) {
            (false, ..) => return None,
            (_, true, ..) => Delivery::Paused,
            (_, _, Some((done, total)), _) => Delivery::Fetching { done, total },
            _ if held => Delivery::Held,
            (_, _, _, true) => Delivery::NeedsRestart,
            _ => Delivery::UpToDate,
        })
    }
}
