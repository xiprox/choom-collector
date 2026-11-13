//! The messages between a [`crate::Player`] and the [`crate::Hub`], encoded with postcard, which
//! the core carries as opaque bytes in `ToHost::Mods` and `ToTester::Mods`
//! (record/areas/mods/01-sharing.md#messages).

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::Delivery;
use crate::hashes::Hash;

/// A message over this size goes in [`Piece`]s, so no frame through the relay is larger.
pub const MAX_MESSAGE: usize = 300 * 1024;

/// The file data in one [`Block`], and the size of each [`Piece`].
pub const BLOCK: usize = 256 * 1024;

/// A split message larger than this many pieces, 64 MiB, is dropped rather than held in memory.
const MAX_PIECES: u32 = 256;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Game-relative, with `/`, such as `r6/scripts/EquipmentEx/module.reds`.
    pub path: String,
    pub size: u64,
    pub hash: Hash,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SharedFolder {
    /// Such as `r6/scripts/EquipmentEx`.
    pub path: String,
    /// When its list of files last changed, in seconds since 1970.
    pub changed: i64,
    pub files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Share {
    /// The sharer's own patterns for files to leave out, which followers go by too.
    pub never_share: Vec<String>,
    pub folders: Vec<SharedFolder>,
}

/// A file asked for, in the version its hash names, and how many bytes of it the asker has.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub path: String,
    pub hash: Hash,
    pub offset: u64,
}

/// Part of a file on its way, in both directions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub path: String,
    pub hash: Hash,
    /// The whole file's size.
    pub size: u64,
    pub offset: u64,
    pub data: Vec<u8>,
}

/// A shared folder, as the hub lists it to every player.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub path: String,
    pub sharer: String,
    /// The sharer's PC ID (`layout::pc_id`).
    pub pc: String,
    pub never_share: Vec<String>,
    /// The sharer's link is down, and the hub keeps their folders for a while.
    pub away: bool,
    pub changed: i64,
    pub files: Vec<FileEntry>,
}

/// A follower of one of the sharer's folders, and how the folder stands on their PC.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FollowerOf {
    pub pc: String,
    pub name: String,
    pub delivery: Delivery,
}

/// One part of a message over [`MAX_MESSAGE`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub id: u32,
    pub index: u32,
    pub count: u32,
    pub bytes: Vec<u8>,
}

/// What a player sends the hub.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ToHub {
    /// What this PC shares, replacing the last `Share`.
    Share(Share),
    /// Every file this PC's followed folders lack, replacing the last `Want`.
    Want(Vec<Request>),
    /// This PC's delivery for each folder it follows, by the folder's path.
    Deliveries(Vec<(String, Delivery)>),
    /// Part of a file the hub pulled.
    Data(Block),
    /// How many bytes of `Data` this PC has taken in since its link opened.
    Ack(u64),
    Piece(Piece),
}

/// What the hub sends a player, or every player.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FromHub {
    /// Every shared folder in the session.
    List(Vec<Listed>),
    /// The files the hub needs from this sharer, replacing the last `Pull`.
    Pull(Vec<Request>),
    /// Part of a file this PC wants.
    Data(Block),
    /// How many bytes of `Data` the hub has taken in from this PC since its link opened.
    Ack(u64),
    /// For each folder this PC shares, by path, its followers.
    Followers(Vec<(String, Vec<FollowerOf>)>),
    Piece(Piece),
}

/// A message that can travel in pieces.
pub trait Message: Serialize + DeserializeOwned {
    fn piece(piece: Piece) -> Self;
    fn into_piece(self) -> Result<Piece, Self>;
}

impl Message for ToHub {
    fn piece(piece: Piece) -> Self {
        ToHub::Piece(piece)
    }
    fn into_piece(self) -> Result<Piece, Self> {
        match self {
            ToHub::Piece(p) => Ok(p),
            other => Err(other),
        }
    }
}

impl Message for FromHub {
    fn piece(piece: Piece) -> Self {
        FromHub::Piece(piece)
    }
    fn into_piece(self) -> Result<Piece, Self> {
        match self {
            FromHub::Piece(p) => Ok(p),
            other => Err(other),
        }
    }
}

pub fn encode<T: Message>(message: &T) -> Vec<u8> {
    postcard::to_allocvec(message).expect("a message should always encode")
}

pub fn decode<T: Message>(bytes: &[u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

/// Encodes a message, in pieces when it's over [`MAX_MESSAGE`]. `next_id` tells one split
/// message's pieces from the next one's.
pub fn pack<T: Message>(message: &T, next_id: &mut u32) -> Vec<Vec<u8>> {
    let bytes = encode(message);
    if bytes.len() <= MAX_MESSAGE {
        return vec![bytes];
    }
    *next_id = next_id.wrapping_add(1);
    let count = bytes.len().div_ceil(BLOCK) as u32;
    bytes
        .chunks(BLOCK)
        .enumerate()
        .map(|(i, c)| encode(&T::piece(Piece { id: *next_id, index: i as u32, count, bytes: c.to_vec() })))
        .collect()
}

/// Joins a split message's pieces, which arrive one after another, since each side sends all of
/// one message's pieces together and the relay keeps their order.
#[derive(Default)]
pub struct Assembly {
    id: u32,
    next: u32,
    bytes: Vec<u8>,
}

impl Assembly {
    /// The message, once all of it has arrived; `None` for a piece of one still arriving, or for
    /// anything that doesn't decode.
    pub fn take<T: Message>(&mut self, raw: &[u8]) -> Option<T> {
        let piece = match decode::<T>(raw)?.into_piece() {
            Err(whole) => return Some(whole),
            Ok(p) => p,
        };
        if piece.index == 0 {
            self.id = piece.id;
            self.next = 0;
            self.bytes.clear();
        }
        if piece.id != self.id || piece.index != self.next || piece.count > MAX_PIECES {
            self.next = u32::MAX;
            self.bytes = Vec::new();
            return None;
        }
        self.bytes.extend_from_slice(&piece.bytes);
        self.next += 1;
        if self.next < piece.count {
            return None;
        }
        self.next = u32::MAX;
        let whole = decode::<T>(&std::mem::take(&mut self.bytes))?;
        // Pieces are never split again, so a piece inside one is garbage.
        whole.into_piece().err()
    }

    pub fn reset(&mut self) {
        *self = Assembly::default();
        self.next = u32::MAX;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_message_travels_in_pieces_and_comes_back_whole() {
        let files: Vec<FileEntry> = (0..5000).map(|i| FileEntry { path: format!("archive/pc/mod/Big/textures/file_{i:05}_with_a_long_name.xbm"), size: i, hash: [i as u8; 32] }).collect();
        let list = FromHub::List(vec![Listed { path: "archive/pc/mod/Big".into(), sharer: "xip".into(), pc: "0011".into(), never_share: vec![], away: false, changed: 1, files }]);
        let mut id = 0;
        let parts = pack(&list, &mut id);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| p.len() <= MAX_MESSAGE));
        let mut asm = Assembly::default();
        let mut got = None;
        for p in &parts {
            assert!(got.is_none());
            got = asm.take::<FromHub>(p);
        }
        assert_eq!(got, Some(list));
    }

    #[test]
    fn a_full_block_fits_in_one_message() {
        let block = ToHub::Data(Block { path: format!("archive/pc/mod/{}", "x".repeat(380)), hash: [7; 32], size: 1 << 30, offset: 1 << 29, data: vec![1; BLOCK] });
        let mut id = 0;
        assert_eq!(pack(&block, &mut id).len(), 1);
    }

    #[test]
    fn a_piece_out_of_order_drops_the_message() {
        let mut asm = Assembly::default();
        let p = |index| encode(&ToHub::Piece(Piece { id: 1, index, count: 3, bytes: vec![0; 4] }));
        assert_eq!(asm.take::<ToHub>(&p(0)), None);
        assert_eq!(asm.take::<ToHub>(&p(2)), None);
        assert_eq!(asm.take::<ToHub>(&p(1)), None);
        assert_eq!(asm.take::<ToHub>(&encode(&ToHub::Ack(5))), Some(ToHub::Ack(5)));
    }
}
