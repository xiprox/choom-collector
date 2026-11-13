//! A session in one process: a hub and two or three players, each with a game folder and a data
//! folder of its own under a temporary folder, passing their messages the way the core would, on
//! a clock the tests move by hand.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use coop_mods::wire::{self, FromHub, MAX_MESSAGE, ToHub};
use coop_mods::{Clock, Hub, Player, PlayerConfig, SharedMod, Target};

/// Each file a test writes gets a later modified time than the last, so two saves within the
/// file system's clock tick never look like one.
static MTIME: AtomicU64 = AtomicU64::new(0);

pub struct Pc {
    pub id: String,
    pub name: String,
    pub game: PathBuf,
    pub data: PathBuf,
    pub never_share: Vec<String>,
    pub player: Option<Player>,
    pub online: bool,
    pub game_running: Arc<AtomicBool>,
    inbox: VecDeque<Vec<u8>>,
    outbox: VecDeque<Vec<u8>>,
    /// Messages this PC takes in, or sends the hub, per step, to make a link slow.
    pub cap_in: usize,
    pub cap_out: usize,
    /// Bytes of file data this PC sent the hub.
    pub uploaded: u64,
}

pub struct Net {
    _dir: tempfile::TempDir,
    pub root: PathBuf,
    pub cache: PathBuf,
    start: Instant,
    ms: Arc<AtomicU64>,
    pub hub: Option<Hub>,
    pub pcs: Vec<Pc>,
    /// Messages that went in pieces, both ways.
    pub pieces: usize,
}

impl Net {
    /// The first name is the host's, whose player talks to the hub in-process.
    pub fn new(names: &[&str]) -> Net {
        Self::with_never_share(names, &[])
    }

    pub fn with_never_share(names: &[&str], never_share: &[&[&str]]) -> Net {
        let dir = tempfile::tempdir().expect("a temporary folder should be available");
        let root = dir.path().to_path_buf();
        let start = Instant::now();
        let ms = Arc::new(AtomicU64::new(0));
        let cache = root.join(names[0]).join("data").join("mod-cache");
        let hub = Hub::with_clock(cache.clone(), clock(start, &ms));
        let mut net = Net { _dir: dir, root: root.clone(), cache, start, ms, hub: Some(hub), pcs: Vec::new(), pieces: 0 };
        for (i, name) in names.iter().enumerate() {
            let game = root.join(name).join("game");
            let data = root.join(name).join("data");
            fs::create_dir_all(&game).unwrap();
            fs::create_dir_all(&data).unwrap();
            net.pcs.push(Pc {
                id: format!("install-{name}"),
                name: name.to_string(),
                game,
                data,
                never_share: never_share.get(i).map(|p| p.iter().map(|s| s.to_string()).collect()).unwrap_or_default(),
                player: None,
                online: false,
                game_running: Arc::new(AtomicBool::new(false)),
                inbox: VecDeque::new(),
                outbox: VecDeque::new(),
                cap_in: usize::MAX,
                cap_out: usize::MAX,
                uploaded: 0,
            });
        }
        for i in 0..net.pcs.len() {
            net.start_player(i);
            net.restore_link(i);
        }
        net
    }

    fn start_player(&mut self, i: usize) {
        let pc = &self.pcs[i];
        let config = PlayerConfig { game_folder: pc.game.clone(), data_dir: pc.data.clone(), install_id: pc.id.clone(), name: pc.name.clone(), never_share: pc.never_share.clone() };
        let running = pc.game_running.clone();
        let player = Player::with(config, clock(self.start, &self.ms), Arc::new(move || running.load(Ordering::SeqCst)));
        self.pcs[i].player = Some(player);
    }

    pub fn hub(&mut self) -> &mut Hub {
        self.hub.as_mut().expect("the hub runs")
    }

    pub fn p(&self, i: usize) -> &Player {
        self.pcs[i].player.as_ref().expect("the player runs")
    }

    pub fn pm(&mut self, i: usize) -> &mut Player {
        self.pcs[i].player.as_mut().expect("the player runs")
    }

    /// Moves the clock on 250ms, ticks every player and the hub, and passes messages until none
    /// are left, or the slow links have passed all they can this step.
    pub fn step(&mut self) {
        self.ms.fetch_add(250, Ordering::SeqCst);
        for pc in self.pcs.iter_mut().filter(|p| p.online) {
            let out = pc.player.as_mut().unwrap().tick();
            pc.outbox.extend(out);
        }
        let out = self.hub().tick();
        self.route(out);
        self.deliver();
    }

    /// Moves the clock on without ticking anyone.
    pub fn jump(&mut self, by: Duration) {
        self.ms.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
    }

    pub fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 4.0) as usize {
            self.step();
        }
    }

    /// Steps until `done` holds, for at most `seconds`; true when it held.
    pub fn run_until(&mut self, seconds: f64, done: impl Fn(&Net) -> bool) -> bool {
        for _ in 0..(seconds * 4.0) as usize {
            if done(self) {
                return true;
            }
            self.step();
        }
        done(self)
    }

    fn route(&mut self, out: Vec<(Target, Vec<u8>)>) {
        for (target, bytes) in out {
            assert!(bytes.len() <= MAX_MESSAGE, "the hub sent a message of {} bytes", bytes.len());
            if let Some(FromHub::Piece(_)) = wire::decode::<FromHub>(&bytes) {
                self.pieces += 1;
            }
            for pc in self.pcs.iter_mut().filter(|p| p.online) {
                if target == Target::Everyone || target == Target::Player(pc.id.clone()) {
                    pc.inbox.push_back(bytes.clone());
                }
            }
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn deliver(&mut self) {
        let mut taken_in = vec![0usize; self.pcs.len()];
        let mut sent_up = vec![0usize; self.pcs.len()];
        loop {
            let mut moved = false;
            for i in 0..self.pcs.len() {
                while sent_up[i] < self.pcs[i].cap_out {
                    let Some(bytes) = self.pcs[i].outbox.pop_front() else { break };
                    assert!(bytes.len() <= MAX_MESSAGE, "{} sent a message of {} bytes", self.pcs[i].name, bytes.len());
                    match wire::decode::<ToHub>(&bytes) {
                        Some(ToHub::Data(b)) => self.pcs[i].uploaded += b.data.len() as u64,
                        Some(ToHub::Piece(_)) => self.pieces += 1,
                        _ => {}
                    }
                    sent_up[i] += 1;
                    moved = true;
                    let id = self.pcs[i].id.clone();
                    let out = self.hub().on_message(&id, &bytes);
                    self.route(out);
                }
            }
            for i in 0..self.pcs.len() {
                while taken_in[i] < self.pcs[i].cap_in {
                    let Some(bytes) = self.pcs[i].inbox.pop_front() else { break };
                    taken_in[i] += 1;
                    moved = true;
                    let out = self.pm(i).on_message(&bytes);
                    self.pcs[i].outbox.extend(out);
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// The player's link to the hub drops, and what was on its way is lost.
    pub fn drop_link(&mut self, i: usize) {
        let pc = &mut self.pcs[i];
        pc.online = false;
        pc.inbox.clear();
        pc.outbox.clear();
        let id = pc.id.clone();
        if let Some(p) = pc.player.as_mut() {
            p.connected(false);
        }
        self.hub().left(&id);
    }

    pub fn restore_link(&mut self, i: usize) {
        let (id, name) = (self.pcs[i].id.clone(), self.pcs[i].name.clone());
        self.pcs[i].online = true;
        self.hub().joined(&id, &name);
        self.pm(i).connected(true);
    }

    /// The app on this PC stops and starts again with the same folders.
    pub fn restart(&mut self, i: usize) {
        self.drop_link(i);
        self.pcs[i].player = None;
        self.start_player(i);
        self.restore_link(i);
    }

    pub fn write(&self, i: usize, rel: &str, data: impl AsRef<[u8]>) {
        let path = self.path(i, rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        let n = MTIME.fetch_add(1, Ordering::SeqCst);
        let when = SystemTime::now() + Duration::from_secs(100 + n);
        fs::File::options().write(true).open(&path).unwrap().set_modified(when).unwrap();
    }

    pub fn read(&self, i: usize, rel: &str) -> Option<Vec<u8>> {
        fs::read(self.path(i, rel)).ok()
    }

    pub fn path(&self, i: usize, rel: &str) -> PathBuf {
        rel.split('/').fold(self.pcs[i].game.clone(), |p, s| p.join(s))
    }

    /// Every file under a game-relative folder, by its path inside it.
    pub fn files(&self, i: usize, rel: &str) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        collect(&self.path(i, rel), "", &mut out);
        out
    }

    /// The row for a mod in this player's shared list.
    pub fn row(&self, i: usize, key: &str) -> Option<SharedMod> {
        self.p(i).shared().into_iter().find(|m| m.key == key)
    }
}

fn collect(dir: &Path, rel: &str, out: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        if path.is_dir() {
            collect(&path, &format!("{rel}{name}/"), out);
        } else {
            out.insert(format!("{rel}{name}"), fs::read(&path).unwrap_or_default());
        }
    }
}

fn clock(start: Instant, ms: &Arc<AtomicU64>) -> Clock {
    let ms = ms.clone();
    Arc::new(move || start + Duration::from_millis(ms.load(Ordering::SeqCst)))
}

/// Bytes that differ from one call to the next, `n` of them.
pub fn bytes(seed: u8, n: usize) -> Vec<u8> {
    (0..n).map(|i| (i as u32).wrapping_mul(31).wrapping_add(seed as u32 * 7) as u8).collect()
}

pub fn hash_map<K: std::hash::Hash + Eq, V>(pairs: impl IntoIterator<Item = (K, V)>) -> HashMap<K, V> {
    pairs.into_iter().collect()
}
