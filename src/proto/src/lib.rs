//! The messages players exchange through the relay, and their encoding: each message is one
//! binary frame, encoded with postcard (record/areas/network/01-relay-and-sessions.md).

use serde::{Deserialize, Serialize};

/// Raised whenever a message changes shape, so a host and a tester on different versions find out
/// from the first message instead of misreading each other.
pub const PROTOCOL: u32 = 1;

/// What a tester sends the host.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ToHost {
    /// A tester's first message on every connection.
    Hello(Hello),
    /// What the tester's PC is doing now, for the host's list of players.
    Doing(Doing),
}

/// What the host sends a tester, or every tester.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ToTester {
    Welcome(Welcome),
    Refused { reason: String },
    Players(Vec<PlayerInfo>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Hello {
    pub protocol: u32,
    pub app_version: String,
    /// Stays the same across the app's restarts on one PC, which is how the host knows a
    /// reconnecting tester for the same player.
    pub install_id: String,
    pub name: String,
    pub game: GameInfo,
    pub system: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct GameInfo {
    /// As the game's program names it, such as `2.31`, or empty when it couldn't be read.
    pub version: String,
    /// Steam, GOG, or Epic, or empty when it couldn't be told.
    pub store: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Welcome {
    pub host_name: String,
    /// The player's ID in this session, which the `Players` list uses.
    pub player_id: u64,
    /// Patterns relative to the game folder that every tester's PC picks up besides its logs.
    pub extra_paths: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub id: u64,
    pub name: String,
    pub is_host: bool,
    pub game: GameInfo,
    pub system: String,
    /// When the player joined, in seconds since 1970 by the host's clock.
    pub joined: i64,
    /// Whether the player came back after a drop and picked up where they left off.
    pub resumed: bool,
    pub doing: Doing,
    pub connected: bool,
}

/// The one line of state a player's row shows.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub enum Doing {
    #[default]
    Nothing,
    Fetching { done: u32, total: u32 },
    NeedsRestart,
}

pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_allocvec(message).expect("a message should always encode")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hello_survives_the_trip() {
        let hello = ToHost::Hello(Hello {
            protocol: PROTOCOL,
            app_version: "0.1.0".into(),
            install_id: "a1b2".into(),
            name: "tetsu".into(),
            game: GameInfo { version: "2.31".into(), store: "Steam".into() },
            system: "SteamOS".into(),
        });
        assert_eq!(decode::<ToHost>(&encode(&hello)), Some(hello));
    }

    #[test]
    fn garbage_decodes_to_nothing() {
        assert_eq!(decode::<ToTester>(&[0xff, 0xff, 0xff]), None);
    }
}
