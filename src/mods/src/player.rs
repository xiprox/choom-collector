//! One PC's part in sharing: it finds the PC's own mods, lists the ones it shares to the hub and
//! sends the files the hub asks for, and keeps each folder it follows the same as its sharer's
//! (record/areas/mods/01-sharing.md).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::backups::Backups;
use crate::hashes::{EMPTY, Hash, Hashes, Stamp};
use crate::hub::WINDOW;
use crate::layout::{
    Excluder, TMP_SUFFIX, delete_folder, find_folders, find_loose, folder_of, has_code, local, loose_belongs, name_of, pc_id, remove_empty_folders, remove_tmp_files, usable, walk,
};
use crate::store::{Saved, SavedFollow};
use crate::wire::{Assembly, BLOCK, Block, FileEntry, FollowerOf, FromHub, Listed, Request, Share, SharedFolder, ToHub, pack};
use crate::{Clock, Delivery, Doing, Follow, Follower, GameCheck, ModStatus, MyMod, Place, PlayerConfig, SharedMod, game, system_clock};

/// A sharer looks at its shared folders this often, so a saved file is on its way within 2s.
const SHARE_SCAN_EVERY: Duration = Duration::from_secs(2);
/// A follower brings its followed folders in line this often, which undoes an edit within a
/// second or two.
const SYNC_EVERY: Duration = Duration::from_secs(1);
/// How often the mods the player doesn't follow are compared with their own copies, for the window.
const COMPARE_EVERY: Duration = Duration::from_secs(5);
const MY_MODS_EVERY: Duration = Duration::from_secs(10);
const GAME_CHECK_EVERY: Duration = Duration::from_secs(2);
/// A follower that received nothing this long asks for its files again.
const WANT_REPEAT: Duration = Duration::from_secs(20);
/// Hashing reads at most this much a tick, so a large archive never holds up the session.
const HASH_BUDGET: u64 = 32 << 20;

/// A file this PC shares, as it was when hashed.
struct Offered {
    full: PathBuf,
    size: u64,
    hash: Hash,
}

/// A file on its way here, written beside the file it replaces.
struct Part {
    hash: Hash,
    size: u64,
    have: u64,
    tmp: PathBuf,
    sha: Sha256,
    done: bool,
    /// What it replaces is in the backups.
    backed_up: bool,
}

/// A followed folder's delivery here.
#[derive(Default)]
struct Following {
    /// The files needed since the folder was last up to date, for "fetching n of m".
    round: BTreeSet<String>,
    /// `None` until its files have been hashed and compared once.
    delivery: Option<Delivery>,
}

/// How a shared folder this PC doesn't follow compares with its own copy.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Compare {
    Missing,
    Same,
    Differs,
}

/// One row of the shared mods: the folders of one name from one PC.
struct Row {
    key: String,
    name: String,
    lower: String,
    pc: String,
    sharer: String,
    folders: Vec<RowFolder>,
}

struct RowFolder {
    path: String,
    /// Its place in the hub's list, when it's listed.
    listed: Option<usize>,
    followed: bool,
}

impl SavedFollow {
    fn matches(&self, l: &Listed) -> bool {
        same(&self.folder, &l.path) && self.sharer == l.pc
    }
}

pub struct Player {
    config: PlayerConfig,
    clock: Clock,
    game_check: GameCheck,
    my_pc: String,
    /// The defaults and the player's own patterns, for what they share.
    excluder: Excluder,
    /// Each sharer's patterns, for what this PC follows.
    excluders: HashMap<Vec<String>, Excluder>,
    shares: BTreeSet<String>,
    follows: Vec<SavedFollow>,
    backups: Backups,
    hashes: Hashes,
    connected: bool,

    list: Vec<Listed>,
    /// The list came on the current link, so the follower can act on it.
    list_fresh: bool,
    /// Each folder's followers, for the folders this PC shares, by path in lower case.
    followers: HashMap<String, Vec<FollowerOf>>,
    /// When each listed folder last changed, kept for a followed folder nobody lists any more.
    known_changed: HashMap<String, i64>,

    my_mods: Vec<MyMod>,
    offered: HashMap<String, Offered>,
    shared_folders: Vec<SharedFolder>,
    /// Each shared folder's last list of files, by its hash, and when it changed.
    folder_sigs: HashMap<String, (Hash, i64)>,
    /// A first scan whose files are all hashed has run, so a `Share` won't make the hub drop a
    /// folder whose hashes are still being worked out.
    share_ready: bool,
    clashes: HashSet<String>,
    share_sent: Option<Share>,
    uploads: VecDeque<Request>,
    up_sent: u64,
    up_acked: u64,

    parts: HashMap<String, Part>,
    /// Every file a followed folder needs, by path, with its hash and size; `Data` for anything
    /// else is dropped.
    wanted: HashMap<String, (Hash, u64)>,
    wants: Vec<Request>,
    want_sent: Option<(BTreeSet<(String, Hash)>, Instant)>,
    /// Files whose last byte arrived since the last `Want`, which the hub has dropped from its
    /// queue too.
    finished: HashSet<(String, Hash)>,
    ask_again: bool,
    last_data: Instant,
    received: u64,
    /// By folder path in lower case.
    following: HashMap<String, Following>,
    compares: HashMap<String, Compare>,
    deliveries_sent: Option<Vec<(String, Delivery)>>,
    /// Followed folders that changed while the game ran, by path in lower case.
    stale_in_game: HashSet<String>,
    game_up: bool,
    kept_for_delete: HashSet<PathBuf>,
    /// Folders an unfollow couldn't delete all of, by the key it was given, to try again.
    pending_delete: HashMap<String, (Vec<String>, HashSet<PathBuf>)>,

    share_due: Instant,
    sync_due: Instant,
    compare_due: Instant,
    my_mods_due: Instant,
    game_due: Instant,
    share_now: bool,
    sync_now: bool,
    tmp_swept: bool,
    assembly: Assembly,
    next_piece: u32,
}

impl Player {
    /// A new player waits for [`Player::connected`] before it sends anything; the host's own
    /// player is connected to the hub from the start.
    pub fn new(config: PlayerConfig) -> Self {
        Self::with(config, system_clock(), Arc::new(game::running))
    }

    pub fn with(config: PlayerConfig, clock: Clock, game_check: GameCheck) -> Self {
        let saved = Saved::load(&config.data_dir);
        let now = clock();
        let mut player = Player {
            my_pc: pc_id(&config.install_id),
            excluder: Excluder::with_defaults(&config.never_share),
            excluders: HashMap::new(),
            shares: saved.shared.into_iter().map(|s| s.to_lowercase()).collect(),
            follows: saved.follows,
            backups: Backups::new(&config.data_dir),
            hashes: Hashes::default(),
            connected: false,
            list: Vec::new(),
            list_fresh: false,
            followers: HashMap::new(),
            known_changed: HashMap::new(),
            my_mods: Vec::new(),
            offered: HashMap::new(),
            shared_folders: Vec::new(),
            folder_sigs: HashMap::new(),
            share_ready: false,
            clashes: HashSet::new(),
            share_sent: None,
            uploads: VecDeque::new(),
            up_sent: 0,
            up_acked: 0,
            parts: HashMap::new(),
            wanted: HashMap::new(),
            wants: Vec::new(),
            want_sent: None,
            finished: HashSet::new(),
            ask_again: false,
            last_data: now,
            received: 0,
            following: HashMap::new(),
            compares: HashMap::new(),
            deliveries_sent: None,
            stale_in_game: HashSet::new(),
            game_up: false,
            kept_for_delete: HashSet::new(),
            pending_delete: HashMap::new(),
            share_due: now,
            sync_due: now,
            compare_due: now,
            my_mods_due: now,
            game_due: now,
            share_now: false,
            sync_now: false,
            tmp_swept: false,
            assembly: Assembly::default(),
            next_piece: 0,
            config,
            clock,
            game_check,
        };
        player.scan_my_mods(now);
        player
    }

    /// Scans as due and returns messages for the hub. The core calls it every 250ms.
    pub fn tick(&mut self) -> Vec<Vec<u8>> {
        let now = (self.clock)();
        let game = self.config.game_folder.clone();
        if now >= self.game_due {
            self.game_due = now + GAME_CHECK_EVERY;
            self.game_up = (self.game_check)();
            if !self.game_up {
                self.stale_in_game.clear();
            }
        }
        if !self.tmp_swept && usable(&game) {
            self.tmp_swept = true;
            for f in &self.follows {
                remove_tmp_files(&local(&game, &f.folder), 0);
            }
        }
        if now >= self.my_mods_due {
            self.scan_my_mods(now);
        }
        let share_waiting = (self.share_now || now >= self.share_due) && self.scan_shares(now);
        let can_sync = self.connected && self.list_fresh;
        let sync_waiting = can_sync && (self.sync_now || now >= self.sync_due) && self.sync(now);
        self.hashes.work(HASH_BUDGET);
        // Small files are hashed within the tick, so a second look sends their news now rather
        // than on the next scan.
        if share_waiting {
            self.scan_shares(now);
        }
        if sync_waiting {
            self.sync(now);
        }
        let mut out = Vec::new();
        if self.connected {
            self.send_news(now, &mut out);
            self.pump(&mut out);
        }
        self.pack(out)
    }

    /// A message from the hub; the replies go back to the hub.
    pub fn on_message(&mut self, message: &[u8]) -> Vec<Vec<u8>> {
        let Some(message) = self.assembly.take::<FromHub>(message) else { return Vec::new() };
        let now = (self.clock)();
        let mut out = Vec::new();
        match message {
            FromHub::List(list) => self.take_list(list),
            FromHub::Pull(asked) => {
                let offered = &self.offered;
                self.uploads = asked
                    .into_iter()
                    .filter_map(|mut r| {
                        // Only files shared right now, as they are now.
                        let f = offered.get(&r.path).filter(|f| f.hash == r.hash)?;
                        r.offset = r.offset.min(f.size);
                        Some(r)
                    })
                    .collect();
                self.pump(&mut out);
            }
            FromHub::Data(block) => self.on_data(block, now, &mut out),
            FromHub::Ack(n) => {
                self.up_acked = n.min(self.up_sent);
                self.pump(&mut out);
            }
            FromHub::Followers(f) => self.followers = f.into_iter().map(|(path, v)| (path.to_lowercase(), v)).collect(),
            FromHub::Piece(_) => {}
        }
        self.pack(out)
    }

    /// The link to the hub went down or came back. A new link starts from nothing: the hub hears
    /// everything again, and the player waits for the hub's list before it touches a followed
    /// folder.
    pub fn connected(&mut self, connected: bool) {
        self.connected = connected;
        self.list_fresh = false;
        self.uploads.clear();
        self.up_sent = 0;
        self.up_acked = 0;
        self.received = 0;
        self.share_sent = None;
        self.want_sent = None;
        self.finished.clear();
        self.deliveries_sent = None;
        self.ask_again = false;
        self.assembly.reset();
        if connected {
            self.followers.clear();
        }
    }

    pub fn my_mods(&self) -> Vec<MyMod> {
        self.my_mods.iter().map(|m| MyMod { shared: self.shares.contains(&m.key), ..m.clone() }).collect()
    }

    pub fn shared(&self) -> Vec<SharedMod> {
        self.rows()
            .into_iter()
            .map(|row| {
                let mine = row.pc == self.my_pc;
                let listed: Vec<&Listed> = row.folders.iter().filter_map(|f| f.listed.map(|i| &self.list[i])).collect();
                let followed = row.folders.iter().filter(|f| f.followed).count();
                let follow = match followed {
                    _ if mine => Follow::No,
                    0 => Follow::No,
                    n if n == row.folders.len() => Follow::Yes,
                    _ => Follow::AddedFolder,
                };
                let status = self.status_of(&row, mine, followed, &listed);
                SharedMod {
                    has_dll: listed.iter().any(|l| l.files.iter().any(|f| has_code(&f.path))),
                    changed: row.folders.iter().filter_map(|f| self.known_changed.get(&f.path.to_lowercase())).max().copied().unwrap_or(0),
                    followers: if mine { self.followers_of(&row) } else { Vec::new() },
                    key: row.key,
                    name: row.name,
                    sharer: row.sharer,
                    mine,
                    follow,
                    status,
                }
            })
            .collect()
    }

    /// Shares every folder with this mod's name, in any place, the ones made later included.
    /// Sharing a mod stops following it, since a folder can't be a copy and the original at once.
    pub fn share(&mut self, key: &str, on: bool) {
        let key = key.to_lowercase();
        if on {
            self.shares.insert(key.clone());
            let dropped: Vec<String> = self.follows.iter().filter(|f| name_of(&f.folder).to_lowercase() == key).map(|f| f.folder.clone()).collect();
            self.follows.retain(|f| !dropped.contains(&f.folder));
            self.drop_parts_under(&dropped);
            log::info!("Mods: sharing {key}");
        } else {
            self.shares.remove(&key);
            log::info!("Mods: no longer sharing {key}");
        }
        self.save();
        self.share_now = true;
        self.sync_now = true;
    }

    /// Follows the row's folders from its sharer, the ones not followed yet. Following a mod stops
    /// sharing this PC's own copy.
    pub fn follow(&mut self, key: &str) {
        let Some(row) = self.find_row(key) else { return };
        if row.pc == self.my_pc {
            return;
        }
        for f in row.folders.iter().filter(|f| !f.followed && f.listed.is_some()) {
            self.follows.retain(|x| !same(&x.folder, &f.path));
            self.follows.push(SavedFollow { folder: f.path.clone(), sharer: row.pc.clone(), by: row.sharer.clone() });
            log::info!("Mods: following {} from {}", f.path, row.sharer);
        }
        if self.shares.remove(&row.lower) {
            self.share_now = true;
        }
        self.save();
        self.sync_now = true;
    }

    /// Takes the folders a sharer added to a mod the player follows.
    pub fn take_added_folder(&mut self, key: &str) {
        self.follow(key);
    }

    /// Stops following the row's folders. With `delete_copy`, each followed folder on this PC
    /// goes too, after its files go to the backups. When some files can't be deleted, such as
    /// those a running game holds on Windows, the error says how many, and calling this again
    /// tries again.
    pub fn unfollow(&mut self, key: &str, delete_copy: bool) -> Result<(), String> {
        let key = key.to_lowercase();
        let row = self.find_row(&key);
        let name = row.as_ref().map_or_else(|| key.split('|').next().unwrap_or_default().to_string(), |r| r.name.clone());
        let followed: Vec<String> = row.map(|r| r.folders.into_iter().filter(|f| f.followed).map(|f| f.path).collect()).unwrap_or_default();
        if !followed.is_empty() {
            self.follows.retain(|f| !followed.iter().any(|p| same(p, &f.folder)));
            self.drop_parts_under(&followed);
            self.save();
            self.sync_now = true;
            log::info!("Mods: no longer following {name}");
        }
        if !delete_copy {
            self.pending_delete.remove(&key);
            return Ok(());
        }
        let (mut folders, mut kept) = if !followed.is_empty() {
            (followed, HashSet::new())
        } else {
            match self.pending_delete.remove(&key) {
                Some(pending) => pending,
                None => return Ok(()),
            }
        };
        let game = self.config.game_folder.clone();
        if !usable(&game) {
            return Ok(());
        }
        folders.retain(|p| !self.shares.contains(&name_of(p).to_lowercase()));
        let left: usize = folders.iter().map(|p| delete_folder(&game, p, &mut self.backups, &mut kept)).sum();
        self.backups.trim();
        self.my_mods_due = (self.clock)();
        if left == 0 {
            log::info!("Mods: deleted {} from the game folder", folders.join(", "));
            return Ok(());
        }
        self.pending_delete.insert(key, (folders, kept));
        let (count, them) = if left == 1 { ("1 file".to_string(), "it") } else { (format!("{left} files"), "them") };
        log::info!("Mods: couldn't delete {count} of {name}");
        Err(format!("{count} of {name} couldn't be deleted, most likely because the game has {them} open. Close the game and try again."))
    }

    /// The player's own line in the players list: fetching across every follow, or a restart
    /// needed, or nothing.
    pub fn doing(&self) -> Doing {
        let all: Vec<&Delivery> = self.followed_listed().filter_map(|l| self.following.get(&l.path.to_lowercase())?.delivery.as_ref()).collect();
        match Delivery::combine(all) {
            Some(Delivery::Fetching { done, total }) => Doing::Fetching { done, total },
            Some(Delivery::NeedsRestart) => Doing::NeedsRestart,
            Some(Delivery::Held) => Doing::Held,
            _ => Doing::Nothing,
        }
    }

    fn followed_listed(&self) -> impl Iterator<Item = &Listed> {
        self.list.iter().filter(|l| l.pc != self.my_pc && self.follows.iter().any(|f| f.matches(l)))
    }

    fn save(&self) {
        let saved = Saved { shared: self.shares.iter().cloned().collect(), follows: self.follows.clone() };
        if let Err(e) = saved.save(&self.config.data_dir) {
            log::warn!("Mods: couldn't save what's shared and followed: {e}");
        }
    }

    fn pack(&mut self, out: Vec<ToHub>) -> Vec<Vec<u8>> {
        out.iter().flat_map(|m| pack(m, &mut self.next_piece)).collect()
    }

    fn excluder_for(&mut self, patterns: &[String]) -> Excluder {
        self.excluders.entry(patterns.to_vec()).or_insert_with(|| Excluder::with_defaults(patterns)).clone()
    }

    fn scan_my_mods(&mut self, now: Instant) {
        self.my_mods_due = now + MY_MODS_EVERY;
        let game = &self.config.game_folder;
        if !usable(game) {
            self.my_mods.clear();
            return;
        }
        let loose = find_loose(game, &self.excluder);
        let mut mods: BTreeMap<String, MyMod> = BTreeMap::new();
        for f in find_folders(game, &self.excluder) {
            let key = f.name.to_lowercase();
            let bytes: u64 = walk(&f.full, &f.path, &self.excluder).iter().map(|x| x.size).sum();
            let m = mods.entry(key.clone()).or_insert_with(|| MyMod { key, name: f.name.clone(), places: Vec::new(), bytes: 0, shared: false, loose_file: None });
            if !m.places.contains(&f.place) {
                m.places.push(f.place);
            }
            m.bytes += bytes;
        }
        for m in mods.values_mut() {
            m.loose_file = loose.iter().find(|l| loose_belongs(l, &m.name)).cloned();
        }
        self.my_mods = mods.into_values().collect();
    }

    /// Lists the shared folders and their files. Returns whether some files wait for their hashes.
    fn scan_shares(&mut self, now: Instant) -> bool {
        self.share_now = false;
        self.share_due = now + SHARE_SCAN_EVERY;
        let game = self.config.game_folder.clone();
        if !usable(&game) {
            self.shared_folders.clear();
            self.offered.clear();
            self.share_ready = true;
            return false;
        }
        let mut waiting = false;
        let mut complete = true;
        let mut seen_folders = HashSet::new();
        let mut offered = HashMap::new();
        let mut folders = Vec::new();
        for folder in find_folders(&game, &self.excluder) {
            if !self.shares.contains(&folder.name.to_lowercase()) || !seen_folders.insert(folder.path.to_lowercase()) {
                continue;
            }
            let lower = folder.path.to_lowercase();
            let mut files = Vec::new();
            let mut here = Vec::new();
            let mut seen = HashSet::new();
            let mut missing = false;
            let mut newest = None::<SystemTime>;
            for f in walk(&folder.full, &folder.path, &self.excluder) {
                // Two names that differ only in capitals would be one file on a follower's Windows PC.
                if !seen.insert(f.path.to_lowercase()) {
                    if self.clashes.insert(f.path.clone()) {
                        log::info!("Mods: left out {}, as another file has the same name in other capitals", f.path);
                    }
                    continue;
                }
                let entry = match self.hashes.get(&f.full, Stamp { size: f.size, modified: f.modified }, true) {
                    Some(hash) => Some((f.size, hash)),
                    None => {
                        waiting = true;
                        // A file being saved keeps its last state rather than looking deleted.
                        let old = self.offered.get(&f.path).map(|o| (o.size, o.hash));
                        missing |= old.is_none();
                        old
                    }
                };
                let Some((size, hash)) = entry else { continue };
                newest = newest.max(f.modified);
                here.push((f.path.clone(), Offered { full: f.full, size, hash }));
                files.push(FileEntry { path: f.path, size, hash });
            }
            if missing {
                // A new file isn't hashed yet: the folder goes out as it last went, or not yet.
                complete = false;
                if let Some(last) = self.shared_folders.iter().find(|s| s.path.to_lowercase() == lower) {
                    for f in &last.files {
                        if let Some(o) = self.offered.get(&f.path) {
                            offered.insert(f.path.clone(), Offered { full: o.full.clone(), size: o.size, hash: o.hash });
                        }
                    }
                    folders.push(last.clone());
                }
                continue;
            }
            let sig = signature(&files);
            let changed = match self.folder_sigs.get(&lower) {
                Some((s, c)) if *s == sig => *c,
                Some(_) => unix_now(),
                None => newest.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or_else(unix_now, |d| d.as_secs() as i64),
            };
            self.folder_sigs.insert(lower, (sig, changed));
            offered.extend(here);
            folders.push(SharedFolder { path: folder.path, changed, files });
        }
        self.offered = offered;
        self.shared_folders = folders;
        if complete {
            self.share_ready = true;
        }
        waiting
    }

    fn take_list(&mut self, list: Vec<Listed>) {
        let mut out = Vec::new();
        for mut l in list.into_iter().take(10_000) {
            // Nothing outside the mod places, nothing outside its own folder, and none of the files
            // left out.
            if folder_of(&l.path).as_deref() != Some(l.path.as_str()) {
                continue;
            }
            let excluder = self.excluder_for(&l.never_share);
            let prefix = format!("{}/", l.path);
            let mut seen = HashSet::new();
            let folder = l.path.clone();
            l.files.retain(|f| {
                f.path.starts_with(&prefix) && folder_of(&f.path).as_deref() == Some(folder.as_str()) && !excluder.excludes(&f.path[prefix.len()..]) && seen.insert(f.path.to_lowercase())
            });
            self.known_changed.insert(l.path.to_lowercase(), l.changed);
            out.push(l);
        }
        self.list = out;
        self.list_fresh = true;
        self.sync_now = true;
    }

    /// Brings each followed folder in line with its sharer's list, and compares the others with
    /// the copies here. Returns whether some files wait for their hashes.
    fn sync(&mut self, now: Instant) -> bool {
        self.sync_now = false;
        self.sync_due = now + SYNC_EVERY;
        let game = self.config.game_folder.clone();
        if !usable(&game) {
            return false;
        }
        let compare = now >= self.compare_due;
        if compare {
            self.compare_due = now + COMPARE_EVERY;
        }
        let list = std::mem::take(&mut self.list);
        let mut wants = Vec::new();
        let mut wanted = HashMap::new();
        let mut waiting = false;
        let mut followed = HashSet::new();
        let my_pc = self.my_pc.clone();
        for l in list.iter().filter(|l| l.pc != my_pc) {
            if self.follows.iter().any(|f| f.matches(l)) {
                followed.insert(l.path.to_lowercase());
                waiting |= self.mirror(&game, l, &mut wants, &mut wanted);
            } else if compare && let Some(c) = self.compare(&game, l) {
                self.compares.insert(l.path.to_lowercase(), c);
            }
        }
        self.list = list;
        self.following.retain(|k, _| followed.contains(k));
        let gone: Vec<String> = self.parts.keys().filter(|p| !wanted.contains_key(*p)).cloned().collect();
        for path in gone {
            if let Some(part) = self.parts.remove(&path) {
                let _ = fs::remove_file(&part.tmp);
            }
        }
        self.wanted = wanted;
        self.wants = wants;
        self.backups.trim();
        waiting
    }

    /// Brings one followed folder in line with its sharer's list. Returns whether some files wait
    /// for their hashes.
    fn mirror(&mut self, game: &Path, l: &Listed, wants: &mut Vec<Request>, wanted: &mut HashMap<String, (Hash, u64)>) -> bool {
        let lower = l.path.to_lowercase();
        let dir = local(game, &l.path);
        let excluder = self.excluder_for(&l.never_share);
        let theirs: HashSet<String> = l.files.iter().map(|f| f.path.to_lowercase()).collect();
        let (mut changed, mut unknown) = (0u32, 0u32);
        let mut needed = Vec::new();
        let mut held = Vec::new();

        // Files the sharer doesn't have go, apart from the ones the patterns leave out, so an
        // edit here is undone.
        if dir.is_dir() {
            let mut deleted = false;
            for f in walk(&dir, &l.path, &excluder) {
                if theirs.contains(&f.path.to_lowercase()) {
                    continue;
                }
                if self.kept_for_delete.insert(f.full.clone()) {
                    self.backups.keep(&f.full, &f.path);
                }
                match fs::remove_file(&f.full) {
                    Ok(()) => {
                        self.kept_for_delete.remove(&f.full);
                        changed += 1;
                        deleted = true;
                    }
                    Err(_) => held.push(f.path),
                }
            }
            if deleted {
                remove_empty_folders(&dir);
            }
        }

        for f in &l.files {
            let target = local(game, &f.path);
            if let Some(stamp) = Stamp::of(&target).filter(|s| s.size == f.size) {
                match self.hashes.get(&target, stamp, true) {
                    Some(h) if h == f.hash => continue,
                    Some(_) => {}
                    None => {
                        unknown += 1;
                        continue;
                    }
                }
            }
            if f.size == 0 && f.hash == EMPTY {
                match place_empty(&target, &f.path, &mut self.backups) {
                    Ok(()) => changed += 1,
                    Err(_) => held.push(f.path.clone()),
                }
                continue;
            }
            wanted.insert(f.path.clone(), (f.hash, f.size));
            if let Some(part) = self.parts.get_mut(&f.path).filter(|p| p.hash == f.hash && p.done) {
                match install(part, &target, &f.path, &mut self.backups, &mut self.hashes) {
                    Ok(()) => {
                        self.parts.remove(&f.path);
                        wanted.remove(&f.path);
                        changed += 1;
                    }
                    Err(_) => held.push(f.path.clone()),
                }
                continue;
            }
            let offset = self.parts.get(&f.path).filter(|p| p.hash == f.hash).map_or(0, |p| p.have);
            wants.push(Request { path: f.path.clone(), hash: f.hash, offset });
            needed.push(f.path.clone());
        }

        if changed > 0 {
            log::info!("Mods: {}: {changed} file{} updated or removed", name_of(&l.path), if changed == 1 { "" } else { "s" });
            if self.game_up {
                self.stale_in_game.insert(lower.clone());
            }
        }
        let state = self.following.entry(lower.clone()).or_default();
        state.round.extend(needed.iter().cloned());
        state.round.extend(held.iter().cloned());
        let pending = (needed.len() + held.len()) as u32;
        if pending == 0 && unknown == 0 {
            state.round.clear();
        }
        let total = (state.round.len() as u32).max(pending);
        let fetching = Delivery::Fetching { done: total - pending, total };
        state.delivery = if !needed.is_empty() {
            Some(fetching)
        } else if !held.is_empty() {
            // The game holds the old file; once it lets go the new one lands, and the game loads it
            // when it starts again.
            Some(if self.game_up { Delivery::NeedsRestart } else { Delivery::Held })
        } else if unknown > 0 {
            state.delivery.clone()
        } else if self.stale_in_game.contains(&lower) {
            Some(Delivery::NeedsRestart)
        } else {
            Some(Delivery::UpToDate)
        };
        unknown > 0
    }

    /// How a shared folder this PC doesn't follow compares with its own copy; `None` while its
    /// files are being hashed.
    fn compare(&mut self, game: &Path, l: &Listed) -> Option<Compare> {
        let dir = local(game, &l.path);
        if !dir.is_dir() {
            return Some(Compare::Missing);
        }
        let excluder = self.excluder_for(&l.never_share);
        let here = walk(&dir, &l.path, &excluder);
        let theirs: HashMap<String, &FileEntry> = l.files.iter().map(|f| (f.path.to_lowercase(), f)).collect();
        if here.len() != theirs.len() || here.iter().any(|f| theirs.get(&f.path.to_lowercase()).is_none_or(|t| t.size != f.size)) {
            return Some(Compare::Differs);
        }
        let mut unknown = false;
        for f in &here {
            match self.hashes.get(&f.full, Stamp { size: f.size, modified: f.modified }, false) {
                Some(h) if h == theirs[&f.path.to_lowercase()].hash => {}
                Some(_) => return Some(Compare::Differs),
                None => unknown = true,
            }
        }
        (!unknown).then_some(Compare::Same)
    }

    fn on_data(&mut self, b: Block, now: Instant, out: &mut Vec<ToHub>) {
        self.received += b.data.len() as u64;
        self.last_data = now;
        out.push(ToHub::Ack(self.received));
        // Only a file this PC asked for, so the hub can't write anywhere else.
        let Some(&(hash, size)) = self.wanted.get(&b.path) else { return };
        let len = b.data.len() as u64;
        if hash != b.hash || size != b.size || folder_of(&b.path).is_none() || b.offset.checked_add(len).is_none_or(|end| end > size) {
            return;
        }
        if self.parts.get(&b.path).is_some_and(|p| p.hash != b.hash)
            && let Some(old) = self.parts.remove(&b.path)
        {
            let _ = fs::remove_file(old.tmp);
        }
        if !self.parts.contains_key(&b.path) {
            let tmp = tmp_path(&local(&self.config.game_folder, &b.path));
            let made = tmp.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| File::create(&tmp).map(drop));
            if let Err(e) = made {
                log::warn!("Mods: couldn't write {}: {e}", b.path);
                return;
            }
            self.parts.insert(b.path.clone(), Part { hash, size, have: 0, tmp, sha: Sha256::new(), done: false, backed_up: false });
        }
        let part = self.parts.get_mut(&b.path).expect("the part was just made");
        if part.done {
            return;
        }
        if b.offset > part.have {
            // A gap, such as after this PC lost what it had: ask from where the copy is.
            self.ask_again = true;
            return;
        }
        let skip = (part.have - b.offset) as usize;
        if skip >= b.data.len() && !b.data.is_empty() {
            return;
        }
        let fresh = &b.data[skip..];
        if let Err(e) = append(&part.tmp, fresh) {
            log::warn!("Mods: couldn't write {}: {e}", b.path);
            let _ = fs::remove_file(&part.tmp);
            self.parts.remove(&b.path);
            self.ask_again = true;
            return;
        }
        part.sha.update(fresh);
        part.have += fresh.len() as u64;
        if part.have < part.size {
            return;
        }
        let got: Hash = part.sha.clone().finalize().into();
        if got == part.hash {
            part.done = true;
            self.finished.insert((b.path, hash));
            self.sync_now = true;
        } else {
            log::info!("Mods: {} changed while it was on its way, so it's fetched again", b.path);
            let _ = fs::remove_file(&part.tmp);
            self.parts.remove(&b.path);
            self.ask_again = true;
        }
    }

    /// Sends the hub what changed: the shared folders, the files wanted, and each follow's delivery.
    fn send_news(&mut self, now: Instant, out: &mut Vec<ToHub>) {
        if self.share_ready {
            let share = Share { never_share: self.config.never_share.clone(), folders: self.shared_folders.clone() };
            if self.share_sent.as_ref() != Some(&share) {
                self.share_sent = Some(share.clone());
                out.push(ToHub::Share(share));
            }
        }
        if !self.list_fresh {
            return;
        }
        let set: BTreeSet<(String, Hash)> = self.wants.iter().map(|r| (r.path.clone(), r.hash)).collect();
        let resend = match &mut self.want_sent {
            None => true,
            Some((last, at)) => {
                // The hub drops a file from its queue once it has sent all of it, so a file that
                // arrived needs no new `Want`, and another file's bytes on their way aren't sent twice.
                let added = set.iter().any(|x| !last.contains(x));
                let dropped = last.iter().any(|x| !set.contains(x) && !self.finished.contains(x));
                let stalled = !set.is_empty() && now.duration_since(self.last_data.max(*at)) >= WANT_REPEAT;
                if !(added || dropped || stalled || self.ask_again) {
                    last.retain(|x| !self.finished.contains(x));
                    self.finished.clear();
                }
                added || dropped || stalled || self.ask_again
            }
        };
        if resend {
            let parts = &self.parts;
            let wants = self.wants.iter().map(|r| Request { offset: parts.get(&r.path).filter(|p| p.hash == r.hash && !p.done).map_or(0, |p| p.have), ..r.clone() }).collect();
            out.push(ToHub::Want(wants));
            self.want_sent = Some((set, now));
            self.finished.clear();
            self.ask_again = false;
        }
        let deliveries: Vec<(String, Delivery)> = self.followed_listed().filter_map(|l| Some((l.path.clone(), self.following.get(&l.path.to_lowercase())?.delivery.clone()?))).collect();
        if self.deliveries_sent.as_ref() != Some(&deliveries) {
            self.deliveries_sent = Some(deliveries.clone());
            out.push(ToHub::Deliveries(deliveries));
        }
    }

    /// Sends the hub the next blocks it asked for, as far as the window allows.
    fn pump(&mut self, out: &mut Vec<ToHub>) {
        while self.up_sent - self.up_acked < WINDOW {
            let Some(r) = self.uploads.front_mut() else { break };
            let Some(f) = self.offered.get(&r.path).filter(|f| f.hash == r.hash) else {
                self.uploads.pop_front();
                continue;
            };
            let n = (f.size - r.offset).min(BLOCK as u64) as usize;
            match read_at(&f.full, r.offset, n) {
                Ok(data) if data.len() == n => {
                    out.push(ToHub::Data(Block { path: r.path.clone(), hash: r.hash, size: f.size, offset: r.offset, data }));
                    r.offset += n as u64;
                    self.up_sent += n as u64;
                    if r.offset >= f.size {
                        self.uploads.pop_front();
                    }
                }
                // Can't be read now, or it's shorter than it was: the hub asks again after the
                // next list.
                _ => {
                    self.uploads.pop_front();
                }
            }
        }
    }

    fn drop_parts_under(&mut self, folders: &[String]) {
        let gone: Vec<String> = self.parts.keys().filter(|p| folder_of(p).is_some_and(|f| folders.iter().any(|x| same(x, &f)))).cloned().collect();
        for path in gone {
            if let Some(part) = self.parts.remove(&path) {
                let _ = fs::remove_file(part.tmp);
            }
        }
    }

    fn find_row(&self, key: &str) -> Option<Row> {
        let key = key.to_lowercase();
        self.rows().into_iter().find(|r| r.key == key)
    }

    /// One row per mod and sharer, from the hub's list and this PC's follows. A mod whose folders
    /// two PCs share takes two rows, since a follower trusts each sharer apart.
    fn rows(&self) -> Vec<Row> {
        let mut groups: BTreeMap<(String, String), Row> = BTreeMap::new();
        for (i, l) in self.list.iter().enumerate() {
            let name = name_of(&l.path);
            let lower = name.to_lowercase();
            let row = groups.entry((lower.clone(), l.pc.clone())).or_insert_with(|| Row { key: String::new(), name: name.to_string(), lower, pc: l.pc.clone(), sharer: l.sharer.clone(), folders: Vec::new() });
            row.folders.push(RowFolder { path: l.path.clone(), listed: Some(i), followed: self.follows.iter().any(|f| f.matches(l)) });
        }
        for f in &self.follows {
            // A follow of a folder someone lists shows on that row, whoever shares it now.
            if self.list.iter().any(|l| same(&l.path, &f.folder)) {
                continue;
            }
            let name = name_of(&f.folder);
            let lower = name.to_lowercase();
            let row = groups.entry((lower.clone(), f.sharer.clone())).or_insert_with(|| Row { key: String::new(), name: name.to_string(), lower, pc: f.sharer.clone(), sharer: f.by.clone(), folders: Vec::new() });
            row.folders.push(RowFolder { path: f.folder.clone(), listed: None, followed: true });
        }
        let mut count: HashMap<String, usize> = HashMap::new();
        for (lower, _) in groups.keys() {
            *count.entry(lower.clone()).or_default() += 1;
        }
        groups
            .into_iter()
            .map(|((lower, pc), mut row)| {
                // `|` can't be in a folder's name, so a split mod's keys never clash with a mod's.
                row.key = if count[&lower] > 1 { format!("{lower}|{pc}") } else { lower };
                // The name is spelled as in the first place, as `my_mods` spells it.
                row.folders.sort_by_key(|f| (Place::of(&f.path), f.path.to_lowercase()));
                row.name = name_of(&row.folders[0].path).to_string();
                row
            })
            .collect()
    }

    fn status_of(&self, row: &Row, mine: bool, followed: usize, listed: &[&Listed]) -> ModStatus {
        if mine {
            return ModStatus::Sharing;
        }
        if !self.connected || !self.list_fresh {
            return ModStatus::Paused;
        }
        if followed == 0 {
            if let Some(f) = self.follows.iter().find(|f| row.folders.iter().any(|r| same(&r.path, &f.folder))) {
                return ModStatus::FollowedElsewhere { by: f.by.clone() };
            }
            if self.shares.contains(&row.lower) {
                return ModStatus::OfferedToo;
            }
            let states: Vec<Compare> = listed.iter().filter_map(|l| self.compares.get(&l.path.to_lowercase()).copied()).collect();
            if states.iter().all(|c| *c == Compare::Missing) {
                return ModStatus::NotOnYourPc;
            }
            return ModStatus::OnYourPc { same: states.len() == listed.len() && states.iter().all(|c| *c == Compare::Same) };
        }
        if followed < row.folders.len() {
            return ModStatus::AddedFolder;
        }
        let mine_listed: Vec<&&Listed> = listed.iter().filter(|l| self.follows.iter().any(|f| f.matches(l))).collect();
        if mine_listed.is_empty() {
            return ModStatus::NotShared;
        }
        if mine_listed.iter().any(|l| l.away) {
            return ModStatus::Paused;
        }
        let states: Vec<Option<&Delivery>> = mine_listed.iter().map(|l| self.following.get(&l.path.to_lowercase()).and_then(|s| s.delivery.as_ref())).collect();
        let fetching = Delivery::combine(states.iter().flatten().copied().filter(|d| matches!(d, Delivery::Fetching { .. })));
        if let Some(Delivery::Fetching { done, total }) = fetching {
            return ModStatus::Fetching { done, total };
        }
        if states.iter().any(Option::is_none) {
            return ModStatus::Checking;
        }
        match Delivery::combine(states.into_iter().flatten()) {
            Some(Delivery::NeedsRestart) => ModStatus::NeedsRestart,
            Some(Delivery::Held) => ModStatus::Held,
            Some(Delivery::Paused) => ModStatus::Paused,
            _ => ModStatus::UpToDate,
        }
    }

    fn followers_of(&self, row: &Row) -> Vec<Follower> {
        let mut by_pc: BTreeMap<&str, (&str, Vec<&Delivery>)> = BTreeMap::new();
        for f in &row.folders {
            for fo in self.followers.get(&f.path.to_lowercase()).into_iter().flatten() {
                by_pc.entry(fo.pc.as_str()).or_insert((fo.name.as_str(), Vec::new())).1.push(&fo.delivery);
            }
        }
        let mut list: Vec<Follower> = by_pc
            .into_values()
            .filter_map(|(name, ds)| Some(Follower { name: name.to_string(), delivery: Delivery::combine(ds)? }))
            .collect();
        list.sort_by_key(|f| f.name.to_lowercase());
        list
    }
}

impl Drop for Player {
    /// A file on its way starts over when the app starts again, so the half of it written beside
    /// its target goes now.
    fn drop(&mut self) {
        for part in self.parts.values() {
            let _ = fs::remove_file(&part.tmp);
        }
    }
}

/// Puts a finished file in place by a rename, so the game never reads half a file. On Windows
/// the rename fails while the running game holds the old file, and the finished file waits beside
/// it for the next try; on Linux the game keeps reading the old file until it closes it.
fn install(part: &mut Part, target: &Path, rel: &str, backups: &mut Backups, hashes: &mut Hashes) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if !part.backed_up {
        backups.keep(target, rel);
        part.backed_up = true;
    }
    fs::rename(&part.tmp, target)?;
    if let Some(stamp) = Stamp::of(target) {
        hashes.set(target, stamp, part.hash);
    }
    Ok(())
}

fn place_empty(target: &Path, rel: &str, backups: &mut Backups) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    backups.keep(target, rel);
    let tmp = tmp_path(target);
    File::create(&tmp)?;
    fs::rename(&tmp, target)
}

fn tmp_path(target: &Path) -> PathBuf {
    let mut s = target.as_os_str().to_owned();
    s.push(TMP_SUFFIX);
    PathBuf::from(s)
}

/// Paths in mod places are compared without capitals, as Windows does, and the game on Linux
/// through Proton.
fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn signature(files: &[FileEntry]) -> Hash {
    let mut sha = Sha256::new();
    for f in files {
        sha.update(f.path.as_bytes());
        sha.update([0]);
        sha.update(f.hash);
    }
    sha.finalize().into()
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub(crate) fn append(path: &Path, data: &[u8]) -> std::io::Result<()> {
    OpenOptions::new().append(true).open(path)?.write_all(data)
}

/// Up to `n` bytes from `offset`, fewer when the file ends first.
pub(crate) fn read_at(path: &Path, offset: u64, n: usize) -> std::io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(n);
    f.take(n as u64).read_to_end(&mut buf)?;
    Ok(buf)
}
