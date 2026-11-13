//! The host's side: writes what each player sends into the player's folder in the session folder,
//! one copy per source file (record/areas/logs/01-collecting.md#the-hosts-folder).
//! (read in CP77 Log Collector 1.3.1, src/Collector.cs)

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, Utc};

use crate::host_paths::{clean_logical, local, one_line, player_folder, shorten};
use crate::ident::fingerprint_of;
use crate::scan::SESSION_MARKER;
use crate::wire::{BLOCK, Batch, HostFile, HostHas, Message, Move, inflate};
use crate::{SourceCount, SourceKind};

const STATUS_EVERY: Duration = Duration::from_secs(5);
/// A changed `_manifest.tsv` waits at most this long to be saved.
const SAVE_EVERY: Duration = Duration::from_secs(2);
const MANIFEST: &str = "_manifest.tsv";
const PEER_FILE: &str = "_peer.txt";

/// What a player's PC told the host about itself, for `_peer.txt`.
#[derive(Clone, Debug, Default)]
pub struct PeerInfo {
    pub game_folder: String,
    pub game_version: String,
    pub system: String,
}

/// One copy as the host has it: which part of which source file it holds.
#[derive(Clone, Debug)]
struct Entry {
    prefix_len: u32,
    prefix_hash: u64,
    /// The copy holds the source's bytes `start..end`.
    start: u64,
    end: u64,
    /// In UTC, as the manifest shows it.
    source_modified: String,
    source_path: String,
}

struct Player {
    name: String,
    install_id: String,
    folder: String,
    dir: PathBuf,
    peer: PeerInfo,
    table: BTreeMap<String, Entry>,
    /// Batches of any other generation were made before the player's latest `Welcome`.
    generation: u64,
    connected: bool,
    since: DateTime<Local>,
    last_seen: DateTime<Local>,
    last_data: Option<DateTime<Local>>,
    received: u64,
    sources: Vec<SourceCount>,
    dirty: bool,
    last_save: Option<Instant>,
}

/// The host's side of logs, writing into one session folder. The core calls it from one thread.
pub struct Collector {
    folder: PathBuf,
    /// By install ID.
    players: BTreeMap<String, Player>,
    last_status: Option<Instant>,
    last_generation: u64,
}

impl Collector {
    /// Collects into `session_folder`, `<logs folder>/<YYYY-MM-DD> <code>`, picking up the
    /// players' copies when the host ran this session before.
    pub fn new(session_folder: PathBuf) -> Self {
        let folder = std::path::absolute(&session_folder).unwrap_or(session_folder);
        let _ = fs::create_dir_all(&folder);
        let collector = Collector { folder, players: BTreeMap::new(), last_status: None, last_generation: 0 };
        // `_collector.log` is also what makes every player's scan skip this folder.
        collector.log(None, &format!("Collecting into {}", collector.folder.display()));
        collector.write_status(false);
        collector
    }

    /// A player joined or came back. Returns what the host has of their logs, for `Welcome.logs`.
    pub fn welcome(&mut self, install_id: &str, name: &str, peer: PeerInfo) -> Vec<u8> {
        let generation = self.next_generation();
        if !self.players.contains_key(install_id) {
            let folder = self.folder_for(install_id, name);
            let now = Local::now();
            let player = Player {
                name: name.to_string(),
                install_id: install_id.to_string(),
                dir: self.folder.join(&folder),
                folder,
                peer: PeerInfo::default(),
                table: BTreeMap::new(),
                generation: 0,
                connected: false,
                since: now,
                last_seen: now,
                last_data: None,
                received: 0,
                sources: Vec::new(),
                dirty: false,
                last_save: None,
            };
            self.players.insert(install_id.to_string(), player);
        }
        let player = self.players.get_mut(install_id).expect("the player was just added");
        let now = Local::now();
        player.name = name.to_string();
        player.peer = peer;
        player.connected = true;
        player.since = now;
        player.last_seen = now;
        player.generation = generation;
        let _ = fs::create_dir_all(&player.dir);
        if player.dirty {
            if let Err(e) = save_manifest(player) {
                let line = format!("Couldn't save {MANIFEST}: {e}");
                log_to(&self.folder, Some(player), &line);
            }
        }
        let files = load_manifest(player);
        write_peer_file(player);
        let line = format!("Connected; {} of their files are here already", files.len());
        log_to(&self.folder, Some(player), &line);
        HostHas { generation, files }.encode()
    }

    /// A player's connection closed.
    pub fn left(&mut self, install_id: &str) {
        let Some(player) = self.players.get_mut(install_id) else { return };
        if !player.connected {
            return;
        }
        player.connected = false;
        player.last_seen = Local::now();
        if let Err(e) = save_manifest(player) {
            log_to(&self.folder, Some(player), &format!("Couldn't save {MANIFEST}: {e}"));
        }
        write_peer_file(player);
        log_to(&self.folder, Some(player), "Disconnected");
    }

    /// A message from a player, from `ToHost::Logs`. The host never answers one, so the replies are
    /// always empty.
    pub fn on_message(&mut self, install_id: &str, message: &[u8]) -> Vec<Vec<u8>> {
        let Some(player) = self.players.get_mut(install_id) else { return Vec::new() };
        if !player.connected {
            return Vec::new();
        }
        let Ok(batch) = postcard::from_bytes::<Batch>(message) else {
            log_to(&self.folder, Some(player), "Ignored a message it couldn't read");
            return Vec::new();
        };
        if batch.generation != player.generation {
            return Vec::new();
        }
        player.last_seen = Local::now();
        for message in batch.messages {
            let what = name_of(&message);
            if let Err(e) = apply(&self.folder, player, message) {
                log_to(&self.folder, Some(player), &format!("File problem while handling {what}: {e}"));
            }
        }
        Vec::new()
    }

    /// The bytes of logs received from the player since this `Collector` started, before
    /// compression.
    pub fn received_bytes(&self, install_id: &str) -> u64 {
        self.players.get(install_id).map_or(0, |p| p.received)
    }

    /// How many files each source on the player's PC picked up, as the player last reported them.
    pub fn sources(&self, install_id: &str) -> Vec<SourceCount> {
        self.players.get(install_id).map_or_else(Vec::new, |p| p.sources.clone())
    }

    /// Writes `_status.txt` and each connected player's `_peer.txt` every 5s, and saves each
    /// changed `_manifest.tsv` at most 2s after it changed.
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    /// `tick` at a given moment, so a test can move the clock on without waiting.
    #[doc(hidden)]
    pub fn tick_at(&mut self, now: Instant) {
        for player in self.players.values_mut() {
            if player.dirty && player.last_save.is_none_or(|t| now.saturating_duration_since(t) >= SAVE_EVERY) {
                if let Err(e) = save_manifest(player) {
                    log_to(&self.folder, Some(player), &format!("Couldn't save {MANIFEST}: {e}"));
                }
                player.last_save = Some(now);
            }
        }
        if self.last_status.is_none_or(|t| now.saturating_duration_since(t) >= STATUS_EVERY) {
            self.last_status = Some(now);
            self.write_status(false);
            for player in self.players.values().filter(|p| p.connected) {
                write_peer_file(player);
            }
        }
    }

    /// A generation no player of this session folder has held: the clock in nanoseconds, or one
    /// more than the last when the clock hasn't moved, so a restarted host never hands a player a
    /// generation its old batches carry.
    fn next_generation(&mut self) -> u64 {
        let clock = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        self.last_generation = clock.max(self.last_generation + 1);
        self.last_generation
    }

    /// The folder this install had in the session, found by the `install id:` line of its
    /// `_peer.txt`, or else a new one named after the player and unlike every other.
    fn folder_for(&self, install_id: &str, name: &str) -> String {
        let id_line = format!("install id: {install_id}");
        if let Ok(entries) = fs::read_dir(&self.folder) {
            for entry in entries.flatten() {
                let Ok(text) = fs::read_to_string(entry.path().join(PEER_FILE)) else { continue };
                let owned = !self.players.values().any(|p| p.dir == entry.path());
                if owned && text.lines().any(|line| line == id_line) {
                    return entry.file_name().to_string_lossy().into_owned();
                }
            }
        }
        let base = player_folder(name);
        let taken = |folder: &str| self.players.values().any(|p| p.folder.eq_ignore_ascii_case(folder)) || self.folder.join(folder).exists();
        let mut folder = base.clone();
        for n in 2.. {
            if !taken(&folder) {
                break;
            }
            folder = format!("{base}-{n}");
        }
        folder
    }

    fn write_status(&self, stopped: bool) {
        let mut text = format!(
            "Choom Collector, the logs of this session{}\nupdated {}\n\n",
            if stopped { ", stopped" } else { "" },
            stamp(Local::now())
        );
        text += &format!("{:<24} {:<14} {:>6} {:>12}  {}\n", "player", "state", "files", "received", "last data");
        for p in self.players.values() {
            let state = if p.connected { "connected" } else { "disconnected" };
            let last = p.last_data.map_or_else(|| "never".to_string(), stamp);
            text += &format!("{:<24} {:<14} {:>6} {:>12}  {}\n", p.folder, state, p.table.len(), size_text(p.received), last);
        }
        let _ = write_text(&self.folder.join("_status.txt"), &text);
    }

    fn log(&self, player: Option<&Player>, text: &str) {
        log_to(&self.folder, player, text);
    }
}

impl Drop for Collector {
    /// Saves what changed and marks everyone disconnected, so the next `Collector` on this folder
    /// starts from where this one stopped.
    fn drop(&mut self) {
        let ids: Vec<String> = self.players.keys().cloned().collect();
        for id in ids {
            self.left(&id);
        }
        for player in self.players.values_mut().filter(|p| p.dirty) {
            let _ = save_manifest(player);
        }
        self.write_status(true);
        self.log(None, "Stopped collecting");
    }
}

fn apply(session: &Path, p: &mut Player, message: Message) -> io::Result<()> {
    match message {
        Message::Open { path, start, prefix_len, prefix_hash, source_size, source_modified, source_path } => {
            let path = clean_logical(&path);
            let target = local(&p.dir, &path);
            start_copy(session, p, &path, &target, start, prefix_len, prefix_hash)?;
            let entry = p.table.get_mut(&path).expect("start_copy adds the entry");
            entry.source_modified = utc_text(source_modified);
            entry.source_path = one_line(&source_path);
            let tail = if start > 0 { format!(" (its last {}, from byte {start})", size_text(source_size.saturating_sub(start))) } else { String::new() };
            log_to(session, Some(p), &format!("New file {path}{tail}"));
        }
        Message::Data { path, offset, raw_len, zipped, bytes } => on_data(session, p, &clean_logical(&path), offset, raw_len, zipped, bytes)?,
        Message::Moves(moves) => on_moves(session, p, moves)?,
        Message::Copy(copy) => on_copy(session, p, copy)?,
        Message::Ident { path, prefix_len, prefix_hash } => {
            if let Some(entry) = p.table.get_mut(&clean_logical(&path)) {
                entry.prefix_len = prefix_len;
                entry.prefix_hash = prefix_hash;
                p.dirty = true;
            }
        }
        Message::Gone { path } => {
            log_to(session, Some(p), &format!("Gone from their PC, deleted or replaced, and the copy here is kept: {}", clean_logical(&path)));
        }
        Message::Sources(counts) => {
            p.sources = counts;
            write_peer_file(p);
        }
    }
    Ok(())
}

fn name_of(message: &Message) -> &'static str {
    match message {
        Message::Open { .. } => "Open",
        Message::Data { .. } => "Data",
        Message::Moves(_) => "Moves",
        Message::Copy(_) => "Copy",
        Message::Ident { .. } => "Ident",
        Message::Gone { .. } => "Gone",
        Message::Sources(_) => "Sources",
    }
}

/// Starts an empty copy of `path`, after moving any copy at its place into `_history`.
fn start_copy(session: &Path, p: &mut Player, path: &str, target: &Path, start: u64, prefix_len: u32, prefix_hash: u64) -> io::Result<()> {
    displace(session, p, path, target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(target)?;
    let entry = Entry { prefix_len, prefix_hash, start, end: start, source_modified: String::new(), source_path: String::new() };
    p.table.insert(path.to_string(), entry);
    p.dirty = true;
    Ok(())
}

/// Clears `path`'s place, keeping any copy there in `_history`, so nothing the player's own
/// rotation or rewrite throws away is lost here.
fn displace(session: &Path, p: &mut Player, path: &str, target: &Path) -> io::Result<()> {
    p.table.remove(path);
    if !target.exists() {
        return Ok(());
    }
    let relative = target.strip_prefix(&p.dir).unwrap_or(target).to_path_buf();
    let stamped = p.dir.join("_history").join(Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    let wanted = shorten(&p.dir, stamped.join(&relative));
    let mut kept = wanted.clone();
    for n in 2.. {
        if !kept.exists() {
            break;
        }
        kept = PathBuf::from(format!("{}.{n}", wanted.display()));
    }
    if let Some(parent) = kept.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(target, &kept)?;
    let shown = kept.strip_prefix(&p.dir).unwrap_or(&kept).to_string_lossy().replace('\\', "/");
    log_to(session, Some(p), &format!("Kept the previous copy of {} in {shown}", relative.to_string_lossy().replace('\\', "/")));
    Ok(())
}

fn on_data(session: &Path, p: &mut Player, path: &str, offset: u64, raw_len: u32, zipped: bool, bytes: Vec<u8>) -> io::Result<()> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    if raw_len as usize > BLOCK {
        return Err(bad("a data block longer than a block"));
    }
    let raw = if zipped { inflate(&bytes, raw_len as usize).ok_or_else(|| bad("a block that doesn't inflate"))? } else { bytes };
    if raw.len() != raw_len as usize {
        return Err(bad("a data block of another length than it says"));
    }
    let target = local(&p.dir, path);
    if !p.table.contains_key(path) {
        log_to(session, Some(p), &format!("Data for {path} without an Open; starting a copy at byte {offset}"));
        start_copy(session, p, path, &target, offset, 0, 0)?;
    }
    let entry = &p.table[path];
    let (start, end) = (entry.start, entry.end);
    let have = fs::metadata(&target).ok().map(|m| m.len());
    let in_step = have == Some(end - start);
    let raw_end = offset + raw.len() as u64;
    let skip = if in_step && offset == end {
        0
    } else if in_step && offset < end && raw_end > end {
        (end - offset) as usize
    } else if in_step && raw_end <= end && offset >= start {
        return Ok(());
    } else {
        let have = have.map_or_else(|| "no".to_string(), |h| h.to_string());
        let line = format!("The copy of {path} was out of step ({have} bytes here, {} expected, data at byte {offset}); keeping it in _history and starting over", end - start);
        log_to(session, Some(p), &line);
        let (prefix_len, prefix_hash) = (entry.prefix_len, entry.prefix_hash);
        start_copy(session, p, path, &target, offset, prefix_len, prefix_hash)?;
        0
    };
    OpenOptions::new().append(true).open(&target)?.write_all(&raw[skip..])?;
    let written = (raw.len() - skip) as u64;
    p.table.get_mut(path).expect("the copy has an entry").end += written;
    p.dirty = true;
    p.received += written;
    p.last_data = Some(Local::now());
    Ok(())
}

/// A rotation on the player's PC: every copy in the batch moves at once, out to `_tmp` and then to
/// its new place, so `x.1.log` to `x.2.log` and `x.log` to `x.1.log` can't trip over each other.
/// A copy pushed out of its place goes to `_history`.
fn on_moves(session: &Path, p: &mut Player, moves: Vec<Move>) -> io::Result<()> {
    let tmp = p.dir.join("_tmp");
    let mut pending = Vec::new();
    let mut described = Vec::new();
    for (i, m) in moves.into_iter().enumerate() {
        let (from, to) = (clean_logical(&m.from), clean_logical(&m.to));
        let Some(mut entry) = p.table.remove(&from) else {
            log_to(session, Some(p), &format!("Asked to rename {from}, which isn't here"));
            continue;
        };
        entry.source_path = one_line(&m.source_path);
        let source = local(&p.dir, &from);
        let parked = if source.exists() {
            fs::create_dir_all(&tmp)?;
            let parked = tmp.join(format!("{}-{i}", SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
            fs::rename(&source, &parked)?;
            Some(parked)
        } else {
            None
        };
        described.push(format!("{from} -> {to}"));
        pending.push((to, parked, entry));
    }
    for (to, parked, entry) in pending {
        let target = local(&p.dir, &to);
        displace(session, p, &to, &target)?;
        if let Some(parked) = parked {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(parked, &target)?;
        }
        p.table.insert(to, entry);
    }
    p.dirty = true;
    let _ = fs::remove_dir(&tmp);
    log_to(session, Some(p), &format!("Rotation: {}", described.join(", ")));
    Ok(())
}

fn on_copy(session: &Path, p: &mut Player, copy: Move) -> io::Result<()> {
    let (from, to) = (clean_logical(&copy.from), clean_logical(&copy.to));
    let source = local(&p.dir, &from);
    let Some(entry) = p.table.get(&from).filter(|_| source.exists()).cloned() else {
        log_to(session, Some(p), &format!("Asked to copy {from}, which isn't here"));
        return Ok(());
    };
    let target = local(&p.dir, &to);
    displace(session, p, &to, &target)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &target)?;
    p.table.insert(to.clone(), Entry { source_path: one_line(&copy.source_path), ..entry });
    p.dirty = true;
    log_to(session, Some(p), &format!("Copied {from} to {to}, as the player did"));
    Ok(())
}

/// Writes `_manifest.tsv` beside itself and copies it over, which works while another program has it
/// open for reading, where a rename over it fails on Windows.
fn save_manifest(p: &mut Player) -> io::Result<()> {
    let mut text = String::from("# path\tprefix length\tprefix hash\tfirst byte\tend byte\tsource modified (UTC)\tsource path\tfile here\n");
    for (path, e) in &p.table {
        let here = local(&p.dir, path);
        let here = here.strip_prefix(&p.dir).unwrap_or(&here).to_string_lossy().replace('\\', "/");
        text += &format!("{path}\t{}\t{:016x}\t{}\t{}\t{}\t{}\t{here}\n", e.prefix_len, e.prefix_hash, e.start, e.end, e.source_modified, e.source_path);
    }
    let file = p.dir.join(MANIFEST);
    let fresh = p.dir.join(format!("{MANIFEST}.new"));
    write_text(&fresh, &text)?;
    fs::copy(&fresh, &file)?;
    fs::remove_file(&fresh)?;
    p.dirty = false;
    Ok(())
}

/// Reads `_manifest.tsv` into the player's table and returns it as `Welcome.logs` lists it. What's
/// on disk wins: each copy's end comes from its size, since the list can be 2s behind it, and a
/// copy that isn't there is dropped.
fn load_manifest(p: &mut Player) -> Vec<HostFile> {
    p.table.clear();
    let mut files = Vec::new();
    let Ok(text) = fs::read_to_string(p.dir.join(MANIFEST)) else { return files };
    for line in text.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 7 {
            continue;
        }
        let (Ok(prefix_len), Ok(prefix_hash), Ok(start)) = (fields[1].parse(), u64::from_str_radix(fields[2], 16), fields[3].parse::<u64>()) else {
            continue;
        };
        let path = clean_logical(fields[0]);
        let copy = local(&p.dir, &path);
        let Ok(meta) = fs::metadata(&copy) else { continue };
        let size = meta.len();
        let fingerprint = if size > 0 { fingerprint_of(&copy, 0, size) } else { None };
        let entry = Entry { prefix_len, prefix_hash, start, end: start + size, source_modified: fields[5].to_string(), source_path: fields[6].to_string() };
        files.push(HostFile { path: path.clone(), prefix_len, prefix_hash, start, end: entry.end, fingerprint });
        p.table.insert(path, entry);
    }
    files
}

fn write_peer_file(p: &Player) {
    let state = if p.connected { format!("connected since {}", stamp(p.since)) } else { format!("disconnected, last seen {}", stamp(p.last_seen)) };
    let sources: Vec<String> = p
        .sources
        .iter()
        .map(|s| {
            let kind = match &s.kind {
                SourceKind::GameFolder => "the game folder".to_string(),
                SourceKind::CrashReports => "crash reports".to_string(),
                SourceKind::GameOwn => "the game's own logs".to_string(),
                SourceKind::AppLog => "the app's log".to_string(),
                SourceKind::Extra(pattern) => format!("extra path {pattern}"),
            };
            format!("{kind}: {} {}", s.count, if s.count == 1 { "file" } else { "files" })
        })
        .collect();
    let text = format!(
        "name: {}\nstate: {state}\ninstall id: {}\ngame folder: {}\ngame version: {}\nsystem: {}\nfiles here: {}, received this session: {}\npicks up: {}\n",
        p.name,
        p.install_id,
        p.peer.game_folder,
        p.peer.game_version,
        p.peer.system,
        p.table.len(),
        size_text(p.received),
        if sources.is_empty() { "not reported yet".to_string() } else { sources.join(", ") },
    );
    let _ = write_text(&p.dir.join(PEER_FILE), &text);
}

/// A line in `_collector.log`, and in the player's `_events.log` when it's about one player.
fn log_to(session: &Path, player: Option<&Player>, text: &str) {
    let now = stamp(Local::now());
    let who = player.map_or("host", |p| p.folder.as_str());
    let _ = append_line(&session.join(SESSION_MARKER), &format!("{now}  [{who}] {text}"));
    if let Some(p) = player {
        let _ = append_line(&p.dir.join("_events.log"), &format!("{now}  {text}"));
    }
}

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    OpenOptions::new().create(true).append(true).open(path)?.write_all(format!("{line}\n").as_bytes())
}

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    fs::write(path, text)
}

fn stamp(t: DateTime<Local>) -> String {
    t.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn utc_text(seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(seconds, 0).map_or_else(String::new, |t| t.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// `512 B`, `1.2 MB`, `34 MB`.
fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    let units = ["KB", "MB", "GB", "TB"];
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 { format!("{value:.1} {}", units[unit]) } else { format!("{value:.0} {}", units[unit]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_people_write_them() {
        assert_eq!(size_text(512), "512 B");
        assert_eq!(size_text(1300), "1.3 KB");
        assert_eq!(size_text(34 << 20), "34 MB");
    }
}
