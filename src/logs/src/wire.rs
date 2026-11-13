//! The messages a `Sender` sends its host, and what the host's `Collector` puts in `Welcome.logs`
//! (record/areas/logs/01-collecting.md#the-messages).

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};

use crate::SourceCount;

/// Logs travel in blocks of at most this many raw bytes.
pub const BLOCK: usize = 256 * 1024;

/// A batch closes before its messages pass this many bytes, which keeps every `ToHost::Logs`
/// under about 300 KB, the size the core asks of each message.
const BATCH_LIMIT: usize = 280 * 1024;

/// What one `ToHost::Logs` carries: a tick's messages, or as many of them as fit.
#[derive(Serialize, Deserialize, Debug)]
pub struct Batch {
    /// The generation of the `Welcome` the `Sender` last resumed from. The `Collector` drops a batch
    /// of any other generation, since it was made before the host's latest `Welcome` and would
    /// repeat what that `Welcome` makes the `Sender` send again.
    pub generation: u64,
    pub messages: Vec<Message>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    /// A file the host has no copy of: the copy starts at the source's byte `start`.
    Open {
        path: String,
        start: u64,
        prefix_len: u32,
        prefix_hash: u64,
        source_size: u64,
        /// Seconds since 1970.
        source_modified: i64,
        source_path: String,
    },
    Data {
        path: String,
        offset: u64,
        raw_len: u32,
        zipped: bool,
        bytes: Vec<u8>,
    },
    /// One scan's renames, which the host applies as one, so `x.1.log` to `x.2.log` and `x.log` to
    /// `x.1.log` can't trip over each other.
    Moves(Vec<Move>),
    /// A file the player copied, which the host copies from its own copy.
    Copy(Move),
    /// The file grew past its old prefix.
    Ident { path: String, prefix_len: u32, prefix_hash: u64 },
    /// Deleted or replaced on the player's PC; the host keeps its copy.
    Gone { path: String },
    Sources(Vec<SourceCount>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Move {
    pub from: String,
    pub to: String,
    pub source_path: String,
}

/// What the host has of one player's logs, which `Welcome.logs` carries.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct HostHas {
    pub generation: u64,
    pub files: Vec<HostFile>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HostFile {
    pub path: String,
    pub prefix_len: u32,
    pub prefix_hash: u64,
    /// The source's bytes `start..end` are in the host's copy.
    pub start: u64,
    pub end: u64,
    /// Of the host's whole copy, so of the source's `start..end`.
    pub fingerprint: Option<u64>,
}

impl HostHas {
    pub fn encode(&self) -> Vec<u8> {
        deflate(&postcard::to_allocvec(self).expect("what the host has should always encode"))
    }

    /// `None` for bytes that aren't a `HostHas`, such as the empty `logs` of a host that hasn't
    /// collected anything.
    pub fn decode(bytes: &[u8]) -> Option<HostHas> {
        postcard::from_bytes(&inflate(bytes, 64 << 20)?).ok()
    }
}

pub fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).expect("writing to memory should never fail");
    encoder.finish().expect("writing to memory should never fail")
}

/// Inflates at most `max` bytes, and `None` for anything that isn't deflate or is longer.
pub fn inflate(bytes: &[u8], max: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    DeflateDecoder::new(bytes).take(max as u64 + 1).read_to_end(&mut out).ok()?;
    (out.len() <= max).then_some(out)
}

/// Packs messages into batches, in order, each one encoded and ready for `ToHost::Logs`.
#[derive(Default)]
pub struct Outbox {
    messages: Vec<Message>,
    size: usize,
    batches: Vec<Vec<u8>>,
}

impl Outbox {
    pub fn push(&mut self, generation: u64, message: Message) {
        let size = estimate(&message);
        if !self.messages.is_empty() && self.size + size > BATCH_LIMIT {
            self.close(generation);
        }
        self.size += size;
        self.messages.push(message);
    }

    pub fn take(&mut self, generation: u64) -> Vec<Vec<u8>> {
        self.close(generation);
        std::mem::take(&mut self.batches)
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.size = 0;
        self.batches.clear();
    }

    fn close(&mut self, generation: u64) {
        if self.messages.is_empty() {
            return;
        }
        let batch = Batch { generation, messages: std::mem::take(&mut self.messages) };
        self.batches.push(postcard::to_allocvec(&batch).expect("a batch should always encode"));
        self.size = 0;
    }
}

/// A message's encoded size or a little more: postcard writes each length as a varint of at most
/// 10 bytes, and each byte of a `Vec<u8>` as itself.
fn estimate(message: &Message) -> usize {
    const SOME: usize = 32;
    let moved = |m: &Move| m.from.len() + m.to.len() + m.source_path.len() + SOME;
    SOME + match message {
        Message::Open { path, source_path, .. } => path.len() + source_path.len() + 4 * SOME,
        Message::Data { path, bytes, .. } => path.len() + bytes.len() + SOME,
        Message::Moves(moves) => moves.iter().map(moved).sum(),
        Message::Copy(m) => moved(m),
        Message::Ident { path, .. } | Message::Gone { path } => path.len() + SOME,
        Message::Sources(counts) => counts
            .iter()
            .map(|c| SOME + if let crate::SourceKind::Extra(p) = &c.kind { p.len() } else { 0 })
            .sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_of_incompressible_bytes_fits_one_message() {
        let bytes: Vec<u8> = (0..BLOCK as u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        let message = Message::Data { path: "game/r6/logs/redscript_rCURRENT.log".into(), offset: 0, raw_len: BLOCK as u32, zipped: false, bytes };
        let mut outbox = Outbox::default();
        outbox.push(7, message);
        let batches = outbox.take(7);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].len() < 300 * 1024);
        assert!(estimate(&postcard::from_bytes::<Batch>(&batches[0]).unwrap().messages[0]) >= batches[0].len());
    }

    #[test]
    fn two_blocks_go_in_two_batches() {
        let mut outbox = Outbox::default();
        for offset in [0, BLOCK as u64] {
            outbox.push(1, Message::Data { path: "game/x.log".into(), offset, raw_len: BLOCK as u32, zipped: false, bytes: vec![b'x'; BLOCK] });
        }
        assert_eq!(outbox.take(1).len(), 2);
    }

    #[test]
    fn garbage_is_no_host_list() {
        assert!(HostHas::decode(&[]).is_none());
        assert!(HostHas::decode(&[0xff, 0x00, 0x13]).is_none());
    }
}
