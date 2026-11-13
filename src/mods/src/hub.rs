//! The host's part in sharing. Players can't reach each other, so everything goes through here:
//! the list of who shares what, with one sharer per folder, and the files, each of which comes up
//! from its sharer once, into the cache, and goes on to everyone who wants it, even while it's
//! still arriving (record/areas/mods/01-sharing.md).

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::hashes::{Hash, hex};
use crate::layout::{folder_of, pc_id};
use crate::wire::{Assembly, BLOCK, Block, FileEntry, FollowerOf, FromHub, Listed, Request, Share, ToHub, pack};
use crate::{Clock, Delivery, Target, system_clock};

/// How long a player whose link dropped keeps their folders. `TESTER_GRACE` in
/// `src/core/src/session.rs` keeps a reconnecting tester's place in the players list as long, so
/// change both together.
const AWAY_GRACE: Duration = Duration::from_secs(60);
/// How often the hub looks for sharers that stopped sending what it asked for.
const ASK_CHECK: Duration = Duration::from_secs(5);
/// A sharer that sent nothing this long is asked again.
const ASK_REPEAT: Duration = Duration::from_secs(20);
const SWEEP_EVERY: Duration = Duration::from_secs(60);
/// A file stays in the cache this long after its last use, in case a sharer's link drops mid-file.
const KEEP: Duration = Duration::from_secs(600);
/// How much file data may be on its way to one player, or from one, before an `Ack` comes back.
/// The core never makes the hub wait, so without this a large archive would pile up in memory on
/// its way to the relay.
pub(crate) const WINDOW: u64 = 2 << 20;

struct Offered {
    path: String,
    changed: i64,
    files: Vec<FileEntry>,
}

struct HubPlayer {
    name: String,
    pc: String,
    /// The order players joined in, which decides who gets a folder two of them offer.
    seq: u64,
    /// When their link dropped, while the hub keeps their place.
    away: Option<Instant>,
    /// By path in lower case.
    offered: BTreeMap<String, Offered>,
    never_share: Vec<String>,
    /// Files on their way to this player.
    queue: VecDeque<Request>,
    /// The files this player was last asked to send.
    asked: HashSet<Hash>,
    asked_at: Instant,
    last_upload: Instant,
    sent: u64,
    acked: u64,
    received: u64,
    /// Their delivery for each folder they follow, by path in lower case.
    deliveries: BTreeMap<String, Delivery>,
    followers_sent: Option<Vec<(String, Vec<FollowerOf>)>>,
    needs_list: bool,
    assembly: Assembly,
}

struct HubFile {
    hash: Hash,
    size: u64,
    owner: String,
}

/// A file in the cache, named by its hash.
struct Blob {
    size: u64,
    have: u64,
    file: PathBuf,
    sha: Sha256,
    done: bool,
    used: Instant,
}

pub struct Hub {
    cache: PathBuf,
    clock: Clock,
    /// By install ID.
    players: BTreeMap<String, HubPlayer>,
    next_seq: u64,
    /// Who shares each folder, by path in lower case.
    owners: BTreeMap<String, String>,
    /// Every shared file, by path, as its sharer listed it.
    files: HashMap<String, HubFile>,
    blobs: HashMap<Hash, Blob>,
    list: Vec<Listed>,
    list_sent: Vec<u8>,
    list_dirty: bool,
    ask_dirty: bool,
    next_ask: Instant,
    next_sweep: Instant,
    outbox: Vec<(Target, FromHub)>,
    next_piece: u32,
}

impl Hub {
    /// Empties `cache_dir`, the data folder's `mod-cache/`, which the hub owns from here on and
    /// empties again when it's dropped.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self::with_clock(cache_dir, system_clock())
    }

    pub fn with_clock(cache_dir: PathBuf, clock: Clock) -> Self {
        let _ = fs::remove_dir_all(&cache_dir);
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            log::warn!("Mods: couldn't make the cache folder {}: {e}", cache_dir.display());
        }
        let now = clock();
        Hub {
            cache: cache_dir,
            clock,
            players: BTreeMap::new(),
            next_seq: 0,
            owners: BTreeMap::new(),
            files: HashMap::new(),
            blobs: HashMap::new(),
            list: Vec::new(),
            list_sent: crate::wire::encode(&FromHub::List(Vec::new())),
            list_dirty: false,
            ask_dirty: false,
            next_ask: now,
            next_sweep: now + SWEEP_EVERY,
            outbox: Vec::new(),
            next_piece: 0,
        }
    }

    /// A player's link opened, the host's own `Player` included: the first time, or again after
    /// a drop. They get the list on the next call that returns messages.
    pub fn joined(&mut self, install_id: &str, name: &str) {
        let now = (self.clock)();
        let p = self.player(install_id, now);
        let renamed = p.name != name;
        p.name = name.to_string();
        let back = p.away.take().is_some();
        let list_dirty = (back || renamed) && !p.offered.is_empty();
        p.sent = 0;
        p.acked = 0;
        p.received = 0;
        p.queue.clear();
        p.asked.clear();
        p.needs_list = true;
        p.followers_sent = None;
        p.assembly.reset();
        self.list_dirty |= list_dirty;
        self.ask_dirty = true;
    }

    /// A player's link dropped. Their folders stay theirs for a while, marked away, in case
    /// they're back.
    pub fn left(&mut self, install_id: &str) {
        let now = (self.clock)();
        let Some(p) = self.players.get_mut(install_id) else { return };
        p.away = Some(now);
        p.queue.clear();
        p.asked.clear();
        p.assembly.reset();
        self.list_dirty = true;
        self.ask_dirty = true;
    }

    pub fn on_message(&mut self, from: &str, message: &[u8]) -> Vec<(Target, Vec<u8>)> {
        let now = (self.clock)();
        let message = self.player(from, now).assembly.take::<ToHub>(message);
        match message {
            Some(ToHub::Share(share)) => self.on_share(from, share),
            Some(ToHub::Want(wants)) => self.on_want(from, wants),
            Some(ToHub::Deliveries(d)) => self.on_deliveries(from, d),
            Some(ToHub::Data(block)) => self.on_upload(from, block, now),
            Some(ToHub::Ack(n)) => {
                let p = self.players.get_mut(from).expect("the sender was just added");
                p.acked = n.min(p.sent);
            }
            Some(ToHub::Piece(_)) | None => {}
        }
        self.flush(now)
    }

    pub fn tick(&mut self) -> Vec<(Target, Vec<u8>)> {
        let now = (self.clock)();
        let before = self.players.len();
        self.players.retain(|id, p| {
            let keep = p.away.is_none_or(|t| now.duration_since(t) < AWAY_GRACE);
            if !keep {
                log::info!("Mods: {} ({id}) left, and their folders go to whoever offered them next", p.name);
            }
            keep
        });
        if self.players.len() != before {
            self.list_dirty = true;
            self.ask_dirty = true;
        }
        if now >= self.next_sweep {
            self.sweep(now);
            self.next_sweep = now + SWEEP_EVERY;
        }
        self.flush(now)
    }

    fn player(&mut self, id: &str, now: Instant) -> &mut HubPlayer {
        let seq = &mut self.next_seq;
        self.players.entry(id.to_string()).or_insert_with(|| {
            *seq += 1;
            HubPlayer {
                name: String::new(),
                pc: pc_id(id),
                seq: *seq,
                away: None,
                offered: BTreeMap::new(),
                never_share: Vec::new(),
                queue: VecDeque::new(),
                asked: HashSet::new(),
                asked_at: now,
                last_upload: now,
                sent: 0,
                acked: 0,
                received: 0,
                deliveries: BTreeMap::new(),
                followers_sent: None,
                needs_list: true,
                assembly: Assembly::default(),
            }
        })
    }

    /// A player says what they'd share. Any folder or file that could reach outside a mod folder
    /// goes.
    fn on_share(&mut self, from: &str, share: Share) {
        // The patterns are another PC's, so their length is capped.
        let mut chars = 0;
        let never_share: Vec<String> = share.never_share.into_iter().take_while(|p| {
            chars += p.len();
            chars <= 4000
        }).collect();
        let mut offered = BTreeMap::new();
        for folder in share.folders.into_iter().take(10_000) {
            if folder_of(&folder.path).as_deref() != Some(folder.path.as_str()) {
                log::warn!("Mods: ignored {:?} from {from}, which isn't a folder in a mod place", folder.path);
                continue;
            }
            let prefix = format!("{}/", folder.path);
            let files = folder.files.into_iter().filter(|f| f.path.starts_with(&prefix) && folder_of(&f.path).as_deref() == Some(folder.path.as_str())).collect();
            offered.insert(folder.path.to_lowercase(), Offered { path: folder.path, changed: folder.changed, files });
        }
        let p = self.players.get_mut(from).expect("the sender was just added");
        p.offered = offered;
        p.never_share = never_share;
        self.list_dirty = true;
    }

    /// A player says which files they still need, replacing what they asked for before.
    fn on_want(&mut self, from: &str, wants: Vec<Request>) {
        let files = &self.files;
        let queue = wants
            .into_iter()
            .filter_map(|mut r| {
                // Only files shared right now, as they are now.
                let f = files.get(&r.path).filter(|f| f.hash == r.hash)?;
                r.offset = r.offset.min(f.size);
                Some(r)
            })
            .collect();
        self.players.get_mut(from).expect("the sender was just added").queue = queue;
        self.ask_dirty = true;
    }

    fn on_deliveries(&mut self, from: &str, deliveries: Vec<(String, Delivery)>) {
        let p = self.players.get_mut(from).expect("the sender was just added");
        p.deliveries = deliveries.into_iter().take(10_000).filter(|(path, _)| folder_of(path).as_deref() == Some(path.as_str())).map(|(path, d)| (path.to_lowercase(), d)).collect();
    }

    /// A block from a sharer. Only what they share themselves, as it is now, gets in.
    fn on_upload(&mut self, from: &str, b: Block, now: Instant) {
        let p = self.players.get_mut(from).expect("the sender was just added");
        p.received += b.data.len() as u64;
        self.outbox.push((Target::Player(from.to_string()), FromHub::Ack(p.received)));
        let len = b.data.len() as u64;
        if folder_of(&b.path).is_none() || b.offset.checked_add(len).is_none_or(|end| end > b.size) {
            return;
        }
        let Some(f) = self.files.get(&b.path) else { return };
        if f.hash != b.hash || f.size != b.size || f.owner != from {
            return;
        }
        p.last_upload = now;
        let blob = match self.blobs.entry(b.hash) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let file = self.cache.join(hex(&b.hash));
                if let Err(err) = fs::write(&file, []) {
                    log::warn!("Mods: couldn't write to the cache: {err}");
                    return;
                }
                e.insert(Blob { size: b.size, have: 0, file, sha: Sha256::new(), done: false, used: now })
            }
        };
        blob.used = now;
        if blob.done {
            return;
        }
        if b.offset > blob.have {
            // A gap: ask again from where this copy is.
            p.asked.remove(&b.hash);
            self.ask_dirty = true;
            return;
        }
        let skip = (blob.have - b.offset) as usize;
        if skip >= b.data.len() && !b.data.is_empty() {
            return;
        }
        let fresh = &b.data[skip..];
        if let Err(err) = crate::player::append(&blob.file, fresh) {
            log::warn!("Mods: couldn't write to the cache: {err}");
            let _ = fs::remove_file(&blob.file);
            self.blobs.remove(&b.hash);
            p.asked.remove(&b.hash);
            self.ask_dirty = true;
            return;
        }
        blob.sha.update(fresh);
        blob.have += fresh.len() as u64;
        if blob.have < blob.size {
            return;
        }
        let got: Hash = blob.sha.clone().finalize().into();
        if got == b.hash {
            blob.done = true;
            for p in self.players.values_mut() {
                p.asked.remove(&b.hash);
            }
        } else {
            log::info!("Mods: {} changed on {from}'s PC while it was being sent, so the hub gets it again", b.path);
            let _ = fs::remove_file(&blob.file);
            self.blobs.remove(&b.hash);
            p.asked.remove(&b.hash);
            self.ask_dirty = true;
        }
    }

    fn flush(&mut self, now: Instant) -> Vec<(Target, Vec<u8>)> {
        if self.list_dirty {
            self.rebuild();
        }
        for (id, p) in self.players.iter_mut().filter(|(_, p)| p.needs_list && p.away.is_none()) {
            p.needs_list = false;
            self.outbox.push((Target::Player(id.clone()), FromHub::List(self.list.clone())));
        }
        if self.ask_dirty || now >= self.next_ask {
            self.ask(now);
            self.next_ask = now + ASK_CHECK;
        }
        self.pump(now);
        self.tell_sharers();
        let mut out = Vec::new();
        for (target, message) in self.outbox.drain(..) {
            for bytes in pack(&message, &mut self.next_piece) {
                out.push((target.clone(), bytes));
            }
        }
        out
    }

    /// Works out who shares which folder, and tells everyone when the list changes.
    fn rebuild(&mut self) {
        self.list_dirty = false;
        let before = self.owners.clone();
        let players = &self.players;
        self.owners.retain(|path, id| players.get(id).is_some_and(|p| p.offered.contains_key(path)));
        let mut by_seq: Vec<(&String, &HubPlayer)> = players.iter().collect();
        by_seq.sort_by_key(|(_, p)| p.seq);
        for (id, p) in by_seq {
            for path in p.offered.keys() {
                self.owners.entry(path.clone()).or_insert_with(|| id.clone());
            }
        }
        for (path, id) in &self.owners {
            let name = &players[id].name;
            match before.get(path) {
                None => log::info!("Mods: {name} shares {path}"),
                Some(old) if old != id => log::info!("Mods: {name} shares {path}, taking it over"),
                _ => {}
            }
        }
        let mut files = HashMap::new();
        let mut list = Vec::new();
        for (path, id) in &self.owners {
            let p = &players[id];
            let off = &p.offered[path];
            for f in &off.files {
                files.insert(f.path.clone(), HubFile { hash: f.hash, size: f.size, owner: id.clone() });
            }
            list.push(Listed {
                path: off.path.clone(),
                sharer: p.name.clone(),
                pc: p.pc.clone(),
                never_share: p.never_share.clone(),
                away: p.away.is_some(),
                changed: off.changed,
                files: off.files.clone(),
            });
        }
        for p in self.players.values_mut() {
            // The player asks again once they have the new list.
            p.queue.retain(|r| files.get(&r.path).is_some_and(|f: &HubFile| f.hash == r.hash));
        }
        self.files = files;
        let bytes = crate::wire::encode(&FromHub::List(list.clone()));
        self.list = list;
        if bytes != self.list_sent {
            self.list_sent = bytes;
            self.outbox.push((Target::Everyone, FromHub::List(self.list.clone())));
            for p in self.players.values_mut() {
                p.needs_list = false;
            }
        }
        self.ask_dirty = true;
    }

    /// Asks each sharer for what someone wants that isn't here yet. A sharer is asked again only
    /// when that changes, or when nothing came from them for a while.
    fn ask(&mut self, now: Instant) {
        self.ask_dirty = false;
        let mut need: HashMap<String, Vec<Request>> = HashMap::new();
        let mut seen = HashSet::new();
        for p in self.players.values().filter(|p| p.away.is_none()) {
            for r in &p.queue {
                let Some(f) = self.files.get(&r.path).filter(|f| f.hash == r.hash) else { continue };
                let blob = self.blobs.get(&r.hash);
                if blob.is_some_and(|b| b.done) || !seen.insert(r.hash) {
                    continue;
                }
                need.entry(f.owner.clone()).or_default().push(Request { path: r.path.clone(), hash: r.hash, offset: blob.map_or(0, |b| b.have) });
            }
        }
        for (id, p) in self.players.iter_mut().filter(|(_, p)| p.away.is_none()) {
            let list = need.remove(id).unwrap_or_default();
            let hashes: HashSet<Hash> = list.iter().map(|r| r.hash).collect();
            let stalled = !list.is_empty() && now.duration_since(p.last_upload.max(p.asked_at)) >= ASK_REPEAT;
            if !stalled && p.asked == hashes {
                continue;
            }
            p.asked = hashes;
            p.asked_at = now;
            self.outbox.push((Target::Player(id.clone()), FromHub::Pull(list)));
        }
    }

    /// Sends each waiting player the next blocks the cache holds for them, as far as their
    /// window allows.
    fn pump(&mut self, now: Instant) {
        for (id, p) in self.players.iter_mut().filter(|(_, p)| p.away.is_none()) {
            while p.sent - p.acked < WINDOW {
                let blobs = &self.blobs;
                let Some(i) = p.queue.iter().position(|r| blobs.get(&r.hash).is_some_and(|b| b.done || r.offset < b.have)) else { break };
                let r = &mut p.queue[i];
                let blob = self.blobs.get_mut(&r.hash).expect("the blob was just found");
                let avail = if blob.done { blob.size } else { blob.have };
                let n = (avail - r.offset).min(BLOCK as u64) as usize;
                let data = match crate::player::read_at(&blob.file, r.offset, n) {
                    Ok(d) if d.len() == n => d,
                    _ => {
                        log::warn!("Mods: couldn't read {} from the cache", hex(&r.hash));
                        break;
                    }
                };
                blob.used = now;
                self.outbox.push((Target::Player(id.clone()), FromHub::Data(Block { path: r.path.clone(), hash: r.hash, size: blob.size, offset: r.offset, data })));
                r.offset += n as u64;
                p.sent += n as u64;
                if r.offset >= blob.size {
                    p.queue.remove(i);
                }
            }
        }
    }

    /// Tells each sharer how their folders stand on their followers' PCs, when that changes.
    fn tell_sharers(&mut self) {
        let mut per_sharer: BTreeMap<String, Vec<(String, Vec<FollowerOf>)>> = BTreeMap::new();
        for (path, owner) in &self.owners {
            let followers: Vec<FollowerOf> = self
                .players
                .iter()
                .filter(|(id, _)| *id != owner)
                .filter_map(|(_, p)| {
                    let d = p.deliveries.get(path)?;
                    Some(FollowerOf { pc: p.pc.clone(), name: p.name.clone(), delivery: if p.away.is_some() { Delivery::Paused } else { d.clone() } })
                })
                .collect();
            if !followers.is_empty() {
                per_sharer.entry(owner.clone()).or_default().push((self.players[owner].offered[path].path.clone(), followers));
            }
        }
        for (id, p) in self.players.iter_mut().filter(|(_, p)| p.away.is_none()) {
            let list = per_sharer.remove(id).unwrap_or_default();
            if p.followers_sent.as_ref() == Some(&list) || (p.followers_sent.is_none() && list.is_empty()) {
                p.followers_sent = Some(list);
                continue;
            }
            p.followers_sent = Some(list.clone());
            self.outbox.push((Target::Player(id.clone()), FromHub::Followers(list)));
        }
    }

    /// The cache holds only what's on its way, and a little longer.
    fn sweep(&mut self, now: Instant) {
        let wanted: HashSet<Hash> = self.players.values().flat_map(|p| p.queue.iter().map(|r| r.hash)).collect();
        self.blobs.retain(|hash, b| {
            let keep = wanted.contains(hash) || now.duration_since(b.used) < KEEP;
            if !keep {
                let _ = fs::remove_file(&b.file);
            }
            keep
        });
    }
}

impl Drop for Hub {
    /// The cache empties when the host stops.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.cache);
    }
}
