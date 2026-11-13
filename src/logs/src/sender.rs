//! A player's side: follows the logs on this PC and turns what's new into messages for the host
//! (record/areas/logs/01-collecting.md). It follows each file by its content, not its name, so a
//! mod that rotates `x.log` to `x.1.log` and starts a new `x.log` costs the host a rename, not a
//! second copy. (read in CP77 Log Collector 1.3.1, src/Sender.cs)

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::SourceCount;
use crate::ident::{fingerprint, fingerprint_of, fnv, read_full};
use crate::scan::{Found, Scanner};
use crate::wire::{BLOCK, HostHas, Message, Move, Outbox, deflate};

/// A file is recognized by up to this many of its first bytes, with its fingerprint.
const PREFIX_MAX: usize = 512;
/// At a connection's first scan, a new file that hasn't changed for this long sends only what it
/// gains from then on.
const HISTORY_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// A new file sends at most this much of its end.
const HISTORY_BYTES: u64 = 32 << 20;
/// How far past the 32 MB cut the history looks for a line start.
const LINE_SEARCH: usize = 64 * 1024;
const FULL_SCAN: Duration = Duration::from_secs(10);
const LOOK: Duration = Duration::from_secs(1);
/// The core calls `tick` every 250ms with no way to wait for the network, so a tick sends at most
/// 1 MB of each file and 2 MB in all, which keeps a call short and the backlog of one big file from
/// holding up the others.
const FILE_BUDGET: u64 = 1 << 20;
const TICK_BUDGET: u64 = 2 << 20;

/// Where this PC's logs are. The core fills it in from the settings and the host's `Welcome`.
#[derive(Clone, Debug, Default)]
pub struct SenderConfig {
    pub game_folder: PathBuf,
    /// The game's `%LOCALAPPDATA%`: the player's own on Windows, the one in the game's Proton
    /// prefix on Linux. `None` leaves out crash reports and the game's own logs.
    pub local_app_data: Option<PathBuf>,
    /// The app's own log file.
    pub app_log: Option<PathBuf>,
    /// The host's extra paths, relative to the game folder, such as `bin/x64/StreetVendors/*.csv`.
    pub extra_paths: Vec<String>,
}

/// How sending goes, for the tester's status bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Files with more on disk than was sent: `total` counts every file that's had a backlog since
    /// the last time there was none, and `done` the ones among them that caught up.
    CatchingUp { done: u32, total: u32 },
    UpToDate,
}

/// The fingerprint of bytes `from..to` of a file.
#[derive(Clone, Copy, Debug)]
struct Print {
    from: u64,
    to: u64,
    hash: u64,
}

/// A file this PC follows.
#[derive(Clone, Debug)]
struct Followed {
    /// The logical path, which is also where the host keeps the copy.
    path: String,
    full: PathBuf,
    prefix_len: usize,
    prefix_hash: u64,
    /// The first byte the host has.
    start: u64,
    /// The host has everything before this.
    sent: u64,
    print: Option<Print>,
    last_size: Option<u64>,
    last_modified: Option<SystemTime>,
    /// The host has a copy, so a move or copy of the file is the host's to make.
    opened: bool,
    /// Found at `path` in the latest scan.
    attached: bool,
    /// The prefix grew and the host hasn't heard.
    ident_due: bool,
}

/// A player's side of logs. The core calls it from one thread, and it never touches the network.
pub struct Sender {
    config: SenderConfig,
    scanner: Scanner,
    followed: Vec<Followed>,
    /// The generation of the latest `Welcome`, or `None` before the first.
    generation: Option<u64>,
    first_scan: bool,
    force_full: bool,
    last_full: Option<Instant>,
    last_look: Option<Instant>,
    /// A file had more to send than a tick's budget.
    more: bool,
    /// The file the next tick starts sending from.
    cursor: usize,
    counts: Vec<SourceCount>,
    /// The counts the host last heard, or `None` when it hasn't since the latest `Welcome`.
    reported: Option<Vec<SourceCount>>,
    backlog: BTreeSet<String>,
    progress: Progress,
    sent_bytes: u64,
    outbox: Outbox,
}

impl Sender {
    pub fn new(config: SenderConfig) -> Self {
        Sender {
            scanner: scanner_for(&config),
            config,
            followed: Vec::new(),
            generation: None,
            first_scan: true,
            force_full: false,
            last_full: None,
            last_look: None,
            more: false,
            cursor: 0,
            counts: Vec::new(),
            reported: None,
            backlog: BTreeSet::new(),
            progress: Progress::UpToDate,
            sent_bytes: 0,
            outbox: Outbox::default(),
        }
    }

    /// Takes the host's extra paths, which the next tick scans.
    pub fn set_extra_paths(&mut self, paths: Vec<String>) {
        self.config.extra_paths = paths;
        self.scanner = scanner_for(&self.config);
        self.force_full = true;
    }

    /// Goes on from what the host has, `Welcome.logs`: call it with every `Welcome`, before the next
    /// tick. Every file the host has a copy of goes on from the copy's end, and what this PC sent
    /// that the host doesn't have is sent again, once.
    pub fn resume(&mut self, host_has: &[u8]) {
        let has = HostHas::decode(host_has).unwrap_or_default();
        // A file the host never got a byte of keeps its place, so what it gained meanwhile still
        // goes out; the files the host has come from its list instead.
        let carried: Vec<Followed> = self
            .followed
            .drain(..)
            .filter(|f| !f.opened)
            .map(|f| Followed { attached: false, ident_due: false, ..f })
            .collect();
        self.followed = has
            .files
            .into_iter()
            .map(|h| Followed {
                path: h.path,
                full: PathBuf::new(),
                prefix_len: h.prefix_len as usize,
                prefix_hash: h.prefix_hash,
                start: h.start,
                sent: h.end,
                print: h.fingerprint.map(|hash| Print { from: h.start, to: h.end, hash }),
                last_size: None,
                last_modified: None,
                opened: true,
                attached: false,
                ident_due: false,
            })
            .collect();
        self.followed.extend(carried);
        self.generation = Some(has.generation);
        self.first_scan = true;
        self.reported = None;
        self.backlog.clear();
        self.progress = Progress::UpToDate;
        self.more = false;
        self.cursor = 0;
        self.outbox.clear();
    }

    /// Scans as due and returns the messages for the host, each one for `ToHost::Logs`.
    pub fn tick(&mut self) -> Vec<Vec<u8>> {
        self.tick_at(Instant::now())
    }

    /// `tick` at a given moment, so a test can move the clock on without waiting.
    #[doc(hidden)]
    pub fn tick_at(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let due = |last: Option<Instant>, every: Duration| last.is_none_or(|t| now.saturating_duration_since(t) >= every);
        // Before the host's first `Welcome` there's nobody to send to, but the counts still show
        // the player which logs this PC picks up.
        let Some(generation) = self.generation else {
            if self.force_full || due(self.last_full, FULL_SCAN) {
                self.force_full = false;
                self.full_scan(now);
            }
            return Vec::new();
        };
        let full_due = self.first_scan || self.force_full || due(self.last_full, FULL_SCAN);
        if !full_due && !self.more && !due(self.last_look, LOOK) {
            return Vec::new();
        }
        self.last_look = Some(now);
        self.force_full = false;

        let mut full = full_due;
        let looked = if full { None } else { self.look() };
        let mut files = match looked {
            Some(files) => files,
            None => {
                full = true;
                self.full_scan(now)
            }
        };
        if !self.reconcile(&files, full, generation) {
            files = self.full_scan(now);
            full = true;
            self.reconcile(&files, true, generation);
        }
        self.first_scan = false;
        if full && self.reported.as_ref() != Some(&self.counts) {
            self.reported = Some(self.counts.clone());
            self.outbox.push(generation, Message::Sources(self.counts.clone()));
        }
        self.send_pending(&files, generation);
        self.update_progress(&files);
        self.outbox.take(generation)
    }

    /// The host sends a `Sender` nothing besides `Welcome.logs`, so this ignores what it gets.
    pub fn on_message(&mut self, _message: &[u8]) {}

    /// How many files each source picked up in the latest full scan.
    pub fn sources(&self) -> Vec<SourceCount> {
        self.counts.clone()
    }

    pub fn progress(&self) -> Progress {
        self.progress
    }

    /// The bytes of logs sent since this `Sender` started, before compression.
    pub fn sent_bytes(&self) -> u64 {
        self.sent_bytes
    }

    fn full_scan(&mut self, now: Instant) -> BTreeMap<String, Found> {
        let scan = self.scanner.scan();
        self.counts = scan.counts;
        self.last_full = Some(now);
        scan.files
    }

    /// Fresh sizes of the files this PC follows, or `None` when one is gone or shorter than what was
    /// sent, which only a full scan can explain.
    fn look(&self) -> Option<BTreeMap<String, Found>> {
        let mut files = BTreeMap::new();
        for f in &self.followed {
            if !f.attached {
                return None;
            }
            let meta = fs::metadata(&f.full).ok()?;
            if meta.len() < f.sent {
                return None;
            }
            let found = Found { logical: f.path.clone(), full: f.full.clone(), size: meta.len(), modified: meta.modified().ok()?, extra: false };
            files.insert(f.path.clone(), found);
        }
        Some(files)
    }

    /// Matches the files on disk against the ones this PC follows, and queues the moves, copies,
    /// and deletions the host has to make. Returns false when a look finds a change only a full
    /// scan can explain.
    fn reconcile(&mut self, files: &BTreeMap<String, Found>, full: bool, generation: u64) -> bool {
        let mut detached: Vec<usize> = (0..self.followed.len()).filter(|&i| !self.followed[i].attached).collect();
        for i in 0..self.followed.len() {
            if !self.followed[i].attached {
                continue;
            }
            let Some(found) = files.get(&self.followed[i].path) else {
                if !full {
                    return false;
                }
                self.followed[i].attached = false;
                detached.push(i);
                continue;
            };
            let f = &self.followed[i];
            // Between full scans, an unchanged size and time is taken as an unchanged file. A full
            // scan reads the content, since a file renamed into place can keep both.
            if !full && f.last_size == Some(found.size) && f.last_modified == Some(found.modified) {
                continue;
            }
            // A file another program holds for a moment gets another look next time.
            let Some(prefix) = read_prefix(found) else { continue };
            if matches(f, found, &prefix) {
                accept(&mut self.followed[i], found, &prefix);
                continue;
            }
            if !full {
                return false;
            }
            self.followed[i].attached = false;
            detached.push(i);
        }
        if !full {
            self.flush_idents(generation);
            return true;
        }

        let mut claimed: HashSet<String> = self.followed.iter().filter(|f| f.attached).map(|f| f.path.clone()).collect();
        // A file still at its own path keeps it before anything is taken for a move, since two
        // crash reports can start with the same bytes.
        let mut left = Vec::new();
        for d in detached {
            let path = self.followed[d].path.clone();
            let back = if claimed.contains(&path) {
                None
            } else {
                files.get(&path).filter(|found| found.size > 0).and_then(|found| {
                    let prefix = read_prefix(found)?;
                    matches(&self.followed[d], found, &prefix).then_some((found, prefix))
                })
            };
            match back {
                Some((found, prefix)) => {
                    self.followed[d].attached = true;
                    accept(&mut self.followed[d], found, &prefix);
                    claimed.insert(path);
                }
                None => left.push(d),
            }
        }
        let mut detached = left;

        let mut moves = Vec::new();
        let mut copies = Vec::new();
        let mut added = Vec::new();
        for (key, found) in files {
            if claimed.contains(key) || found.size == 0 {
                continue;
            }
            let Some(prefix) = read_prefix(found) else { continue };
            if prefix.is_empty() {
                continue;
            }
            if let Some(at) = detached.iter().position(|&d| matches(&self.followed[d], found, &prefix)) {
                let f = &mut self.followed[detached.remove(at)];
                if f.path != *key && f.opened {
                    moves.push(Move { from: f.path.clone(), to: key.clone(), source_path: text(&found.full) });
                }
                f.path = key.clone();
                f.attached = true;
                accept(f, found, &prefix);
                continue;
            }
            let start = self.history_start(found);
            // A copy of a file the host has, such as a mod saving its log under a second name, is
            // copied on the host instead of sent again, unless the history rule leaves it out.
            if start < found.size {
                let twin = self.followed.iter().find(|s| s.attached && s.opened && s.sent > s.start && matches(s, found, &prefix));
                if let Some(twin) = twin {
                    copies.push(Move { from: twin.path.clone(), to: key.clone(), source_path: text(&found.full) });
                    added.push(Followed {
                        path: key.clone(),
                        full: found.full.clone(),
                        last_size: Some(found.size),
                        last_modified: Some(found.modified),
                        ..twin.clone()
                    });
                    continue;
                }
            }
            added.push(new_followed(found, &prefix, start));
        }

        // What's left detached is gone from this PC. The first scan after a `Welcome` drops such
        // files quietly, since they're the host's old copies of files this PC deleted while away.
        let gone: HashSet<usize> = detached.into_iter().collect();
        let followed = std::mem::take(&mut self.followed);
        for (i, f) in followed.into_iter().enumerate() {
            if !gone.contains(&i) {
                self.followed.push(f);
            } else if f.opened && !self.first_scan {
                self.outbox.push(generation, Message::Gone { path: f.path });
            }
        }
        if !moves.is_empty() {
            self.outbox.push(generation, Message::Moves(moves));
        }
        for copy in copies {
            self.outbox.push(generation, Message::Copy(copy));
        }
        self.flush_idents(generation);
        self.followed.extend(added);
        true
    }

    fn flush_idents(&mut self, generation: u64) {
        for f in self.followed.iter_mut().filter(|f| f.ident_due) {
            f.ident_due = false;
            self.outbox.push(generation, Message::Ident { path: f.path.clone(), prefix_len: f.prefix_len as u32, prefix_hash: f.prefix_hash });
        }
    }

    /// Where a new file starts. At a connection's first scan, a log that sat untouched for 24 hours
    /// sends only what it gains from then on, unless an extra path picked it up, since the host
    /// named those. Any other file sends its last 32 MB, from a line start.
    fn history_start(&self, found: &Found) -> u64 {
        let old = SystemTime::now().duration_since(found.modified).is_ok_and(|age| age > HISTORY_AGE);
        if self.first_scan && !found.extra && old {
            return found.size;
        }
        if found.size <= HISTORY_BYTES {
            return 0;
        }
        let start = found.size - HISTORY_BYTES;
        let line = || -> io::Result<Option<usize>> {
            let mut file = File::open(&found.full)?;
            file.seek(SeekFrom::Start(start))?;
            let mut buf = vec![0; LINE_SEARCH];
            let n = read_full(&mut file, &mut buf)?;
            Ok(buf[..n].iter().position(|&b| b == b'\n'))
        };
        match line() {
            Ok(Some(newline)) => start + newline as u64 + 1,
            _ => start,
        }
    }

    fn send_pending(&mut self, files: &BTreeMap<String, Found>, generation: u64) {
        self.more = false;
        let count = self.followed.len();
        if count == 0 {
            return;
        }
        let mut budget = TICK_BUDGET;
        let first = self.cursor % count;
        for k in 0..count {
            let i = (first + k) % count;
            let f = &self.followed[i];
            let Some(found) = files.get(&f.path).filter(|found| f.attached && found.size > f.sent) else { continue };
            if budget == 0 {
                self.more = true;
                self.cursor = i;
                return;
            }
            // A file that went away or is held gets sorted out by the next full scan.
            if self.send_file(i, found, &mut budget, generation).is_err() {
                self.force_full = true;
            }
        }
    }

    fn send_file(&mut self, i: usize, found: &Found, budget: &mut u64, generation: u64) -> io::Result<()> {
        let f = &mut self.followed[i];
        let mut file = File::open(&f.full)?;
        let available = file.metadata()?.len();
        if available < f.sent {
            self.force_full = true;
            return Ok(());
        }
        if !f.opened {
            self.outbox.push(
                generation,
                Message::Open {
                    path: f.path.clone(),
                    start: f.start,
                    prefix_len: f.prefix_len as u32,
                    prefix_hash: f.prefix_hash,
                    source_size: available,
                    source_modified: found.modified.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
                    source_path: text(&f.full),
                },
            );
            f.opened = true;
        }
        file.seek(SeekFrom::Start(f.sent))?;
        let before = f.sent;
        let mut left = FILE_BUDGET.min(*budget);
        let mut block = vec![0; BLOCK];
        while left > 0 && f.sent < available {
            let want = (BLOCK as u64).min(left).min(available - f.sent) as usize;
            let n = read_full(&mut file, &mut block[..want])?;
            if n == 0 {
                break;
            }
            let raw = &block[..n];
            let packed = deflate(raw);
            let zipped = packed.len() < n;
            let bytes = if zipped { packed } else { raw.to_vec() };
            self.outbox.push(generation, Message::Data { path: f.path.clone(), offset: f.sent, raw_len: n as u32, zipped, bytes });
            f.sent += n as u64;
            left -= n as u64;
            *budget -= n as u64;
            self.sent_bytes += n as u64;
        }
        if f.sent > before {
            f.print = fingerprint(&mut file, f.start, f.sent).ok().map(|hash| Print { from: f.start, to: f.sent, hash });
        }
        if f.sent < available {
            self.more = true;
        }
        Ok(())
    }

    fn update_progress(&mut self, files: &BTreeMap<String, Found>) {
        let behind: Vec<&String> = self
            .followed
            .iter()
            .filter(|f| f.attached && files.get(&f.path).is_some_and(|found| found.size > f.sent))
            .map(|f| &f.path)
            .collect();
        if behind.is_empty() {
            self.backlog.clear();
            self.progress = Progress::UpToDate;
            return;
        }
        self.backlog.extend(behind.iter().map(|p| (*p).clone()));
        let total = self.backlog.len() as u32;
        self.progress = Progress::CatchingUp { done: total - behind.len() as u32, total };
    }
}

fn scanner_for(config: &SenderConfig) -> Scanner {
    Scanner::new(&config.game_folder, config.local_app_data.as_deref(), config.app_log.as_deref(), &config.extra_paths)
}

/// Is `found` the file `f` follows: the same first bytes, at least as long as what was sent, and the
/// same fingerprint for the part that was sent?
fn matches(f: &Followed, found: &Found, prefix: &[u8]) -> bool {
    if f.prefix_len == 0 && f.print.is_none() {
        return false;
    }
    if found.size < f.sent || prefix.len() < f.prefix_len || fnv(&prefix[..f.prefix_len]) != f.prefix_hash {
        return false;
    }
    f.print.is_none_or(|p| fingerprint_of(&found.full, p.from, p.to) == Some(p.hash))
}

/// Takes `found` as the file `f` follows, and grows the prefix when the file has grown past it.
fn accept(f: &mut Followed, found: &Found, prefix: &[u8]) {
    f.full = found.full.clone();
    f.last_size = Some(found.size);
    f.last_modified = Some(found.modified);
    if f.prefix_len < PREFIX_MAX && prefix.len() > f.prefix_len {
        f.prefix_len = prefix.len();
        f.prefix_hash = fnv(prefix);
        f.ident_due = f.opened;
    }
}

fn new_followed(found: &Found, prefix: &[u8], start: u64) -> Followed {
    Followed {
        path: found.logical.clone(),
        full: found.full.clone(),
        prefix_len: prefix.len(),
        prefix_hash: fnv(prefix),
        start,
        sent: start,
        // Nothing is sent yet of a file followed from its end, so it's recognized by what it holds.
        print: (start > 0).then(|| fingerprint_of(&found.full, 0, start).map(|hash| Print { from: 0, to: start, hash })).flatten(),
        last_size: Some(found.size),
        last_modified: Some(found.modified),
        opened: false,
        attached: true,
        ident_due: false,
    }
}

fn read_prefix(found: &Found) -> Option<Vec<u8>> {
    let mut file = File::open(&found.full).ok()?;
    let mut buf = vec![0; (PREFIX_MAX as u64).min(found.size) as usize];
    let n = read_full(&mut file, &mut buf).ok()?;
    buf.truncate(n);
    Some(buf)
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
