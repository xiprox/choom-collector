//! A `Sender` against a `Collector` in one process, with temporary folders standing in for a
//! player's PC and the host's session folder, and a clock the tests move on by hand.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use coop_logs::{Collector, PeerInfo, Progress, Sender, SenderConfig, SourceCount, SourceKind};
use tempfile::TempDir;

/// The player's install ID; their name is mira, so their folder on the host is `mira`.
const MIRA: &str = "4f1c9a7e2b6d8035";

struct Rig {
    root: TempDir,
    game: PathBuf,
    local_app_data: PathBuf,
    app_log: PathBuf,
    session: PathBuf,
    extra: Vec<String>,
    clock: Instant,
    sender: Sender,
    collector: Option<Collector>,
}

impl Rig {
    fn new(extra: &[&str]) -> Rig {
        Rig::build(extra, false)
    }

    /// With `session_in_game`, the host's logs folder sits inside the game folder.
    fn build(extra: &[&str], session_in_game: bool) -> Rig {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("Cyberpunk 2077");
        let local_app_data = root.path().join("AppData").join("Local");
        let app_log = root.path().join("CyberpunkCoopTools").join("app.log");
        let logs = if session_in_game { game.join("Co-op Logs") } else { root.path().join("Co-op Logs") };
        let session = logs.join("2026-11-13 K7R-M2Q");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&session).unwrap();
        let extra: Vec<String> = extra.iter().map(|s| s.to_string()).collect();
        let sender = Sender::new(config(&game, &local_app_data, &app_log, &extra));
        let collector = Some(Collector::new(session.clone()));
        Rig { root, game, local_app_data, app_log, session, extra, clock: Instant::now(), sender, collector }
    }

    fn host(&mut self) -> &mut Collector {
        self.collector.as_mut().expect("the host is running")
    }

    /// Stops the host and starts it again on the same session folder.
    fn restart_host(&mut self) {
        self.collector = None;
        self.collector = Some(Collector::new(self.session.clone()));
    }

    fn restart_player(&mut self) {
        self.sender = Sender::new(config(&self.game, &self.local_app_data, &self.app_log, &self.extra));
    }

    /// The player connects, or the host welcomes them again.
    fn join(&mut self) {
        let welcome = self.welcome();
        self.sender.resume(&welcome);
    }

    fn welcome(&mut self) -> Vec<u8> {
        let peer = PeerInfo { game_folder: "C:/Games/Cyberpunk 2077".into(), game_version: "2.31".into(), system: "Windows 11".into() };
        self.host().welcome(MIRA, "mira", peer)
    }

    /// One tick, a second after the last, and its messages, which go nowhere yet.
    fn step(&mut self) -> Vec<Vec<u8>> {
        self.step_after(Duration::from_secs(1))
    }

    fn step_after(&mut self, wait: Duration) -> Vec<Vec<u8>> {
        self.clock += wait;
        let messages = self.sender.tick_at(self.clock);
        for m in &messages {
            assert!(m.len() < 300 * 1024, "a message of {} bytes", m.len());
        }
        messages
    }

    fn deliver(&mut self, messages: &[Vec<u8>]) {
        let clock = self.clock;
        let host = self.host();
        for m in messages {
            assert!(host.on_message(MIRA, m).is_empty());
        }
        host.tick_at(clock);
    }

    /// A tick whose messages reach the host.
    fn run(&mut self) {
        let messages = self.step();
        self.deliver(&messages);
    }

    /// A tick after the 10s that make a full scan due.
    fn rescan(&mut self) {
        let messages = self.step_after(Duration::from_secs(11));
        self.deliver(&messages);
    }

    /// Ticks until the player has sent everything.
    fn settle(&mut self) {
        for _ in 0..100 {
            self.run();
            if self.sender.progress() == Progress::UpToDate {
                return;
            }
        }
        panic!("the player never caught up: {:?}", self.sender.progress());
    }

    fn mira(&self) -> PathBuf {
        self.session.join("mira")
    }

    fn copy(&self, logical: &str) -> Vec<u8> {
        let path = logical.split('/').fold(self.mira(), |p, part| p.join(part));
        fs::read(&path).unwrap_or_else(|e| panic!("the host has no {logical}: {e}"))
    }

    fn has(&self, logical: &str) -> bool {
        logical.split('/').fold(self.mira(), |p, part| p.join(part)).exists()
    }

    fn events(&self) -> String {
        fs::read_to_string(self.mira().join("_events.log")).unwrap_or_default()
    }

    fn manifest(&self) -> String {
        fs::read_to_string(self.mira().join("_manifest.tsv")).unwrap()
    }
}

fn config(game: &Path, local_app_data: &Path, app_log: &Path, extra: &[String]) -> SenderConfig {
    SenderConfig {
        game_folder: game.to_path_buf(),
        local_app_data: Some(local_app_data.to_path_buf()),
        app_log: Some(app_log.to_path_buf()),
        extra_paths: extra.to_vec(),
    }
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn append(path: &Path, bytes: &[u8]) {
    OpenOptions::new().append(true).open(path).unwrap().write_all(bytes).unwrap();
}

/// `count` numbered lines, each starting with `tag`, so files with different tags start with
/// different bytes.
fn lines(tag: &str, count: usize) -> Vec<u8> {
    (0..count).map(|i| format!("{tag} line {i:06}: the quick brown fox jumps over the lazy dog\n")).collect::<String>().into_bytes()
}

fn age(path: &Path, days: u64) {
    let file = OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(days * 24 * 60 * 60)).unwrap();
}

/// Every file under `dir`.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn count(kind: SourceKind, count: u32) -> SourceCount {
    SourceCount { kind, count }
}

#[test]
fn a_growing_log_reaches_the_hosts_copy() {
    let mut rig = Rig::new(&[]);
    let log = rig.game.join("r6").join("logs").join("redscript_rCURRENT.log");
    write(&log, b"[info] Compiling scripts\n");
    rig.join();
    rig.run();
    assert_eq!(rig.copy("game/r6/logs/redscript_rCURRENT.log"), fs::read(&log).unwrap());

    append(&log, b"[info] Compiled 1204 scripts\n");
    rig.run();
    let source = fs::read(&log).unwrap();
    assert_eq!(rig.copy("game/r6/logs/redscript_rCURRENT.log"), source);
    assert_eq!(rig.sender.sent_bytes(), source.len() as u64);
    assert_eq!(rig.host().received_bytes(MIRA), source.len() as u64);
    assert!(rig.manifest().lines().any(|l| l.starts_with("game/r6/logs/redscript_rCURRENT.log\t")));
    assert!(rig.events().contains("New file game/r6/logs/redscript_rCURRENT.log"));
    assert!(fs::read_to_string(rig.session.join("_status.txt")).unwrap().contains("mira"));

    // A message that isn't one of ours changes nothing.
    assert!(rig.host().on_message(MIRA, &[1, 2, 3]).is_empty());
    assert_eq!(rig.copy("game/r6/logs/redscript_rCURRENT.log"), source);
}

#[test]
fn a_rotation_renames_the_hosts_copies_instead_of_sending_them_again() {
    let mut rig = Rig::new(&[]);
    let dir = rig.game.join("red4ext").join("logs");
    let log = dir.join("mod_settings.log");
    let first = lines("first run", 200);
    write(&log, &first);
    rig.join();
    rig.settle();

    let before = rig.sender.sent_bytes();
    fs::rename(&log, dir.join("mod_settings.1.log")).unwrap();
    let second = lines("second run", 50);
    write(&log, &second);
    rig.run();
    assert_eq!(rig.copy("game/red4ext/logs/mod_settings.1.log"), first);
    assert_eq!(rig.copy("game/red4ext/logs/mod_settings.log"), second);
    assert_eq!(rig.sender.sent_bytes() - before, second.len() as u64);

    let before = rig.sender.sent_bytes();
    fs::rename(dir.join("mod_settings.1.log"), dir.join("mod_settings.2.log")).unwrap();
    fs::rename(&log, dir.join("mod_settings.1.log")).unwrap();
    let third = lines("third run", 30);
    write(&log, &third);
    rig.run();
    assert_eq!(rig.copy("game/red4ext/logs/mod_settings.2.log"), first);
    assert_eq!(rig.copy("game/red4ext/logs/mod_settings.1.log"), second);
    assert_eq!(rig.copy("game/red4ext/logs/mod_settings.log"), third);
    assert_eq!(rig.sender.sent_bytes() - before, third.len() as u64);
    assert!(!rig.mira().join("_history").exists());
    assert!(rig.events().contains("Rotation: game/red4ext/logs/mod_settings.log -> game/red4ext/logs/mod_settings.1.log"));
}

#[test]
fn a_copy_under_a_new_name_is_copied_on_the_host() {
    let mut rig = Rig::new(&[]);
    let dir = rig.game.join("bin").join("x64").join("plugins").join("cyber_engine_tweaks");
    let log = dir.join("cyber_engine_tweaks.log");
    write(&log, &lines("cet", 300));
    rig.join();
    rig.settle();

    let before = rig.sender.sent_bytes();
    let copy = dir.join("cyber_engine_tweaks.backup.log");
    fs::copy(&log, &copy).unwrap();
    rig.rescan();
    assert_eq!(rig.copy("game/bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.backup.log"), fs::read(&copy).unwrap());
    assert_eq!(rig.sender.sent_bytes(), before);
    assert!(rig.events().contains("Copied game/bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log"));

    append(&copy, b"[info] only in the copy\n");
    rig.run();
    assert_eq!(rig.copy("game/bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.backup.log"), fs::read(&copy).unwrap());
    assert_eq!(rig.sender.sent_bytes() - before, b"[info] only in the copy\n".len() as u64);
}

#[test]
fn a_shrink_or_a_rewrite_moves_the_old_copy_to_history() {
    let mut rig = Rig::new(&[]);
    let log = rig.game.join("r6").join("logs").join("x.log");
    let long = lines("before", 100);
    write(&log, &long);
    rig.join();
    rig.settle();

    // Shorter, with other first bytes.
    let short = lines("after", 10);
    write(&log, &short);
    rig.run();
    assert_eq!(rig.copy("game/r6/logs/x.log"), short);
    let kept = files_under(&rig.mira().join("_history"));
    assert_eq!(kept.len(), 1);
    assert!(kept[0].ends_with(Path::new("game").join("r6").join("logs").join("x.log")));
    assert_eq!(fs::read(&kept[0]).unwrap(), long);

    // Longer, with the same first 600 bytes and another middle, which only the fingerprint tells.
    let header = vec![b'#'; 599].into_iter().chain(*b"\n").collect::<Vec<u8>>();
    let one: Vec<u8> = header.iter().copied().chain(lines("one", 40)).collect();
    let two: Vec<u8> = header.iter().copied().chain(lines("two", 60)).collect();
    write(&log, &one);
    rig.run();
    assert_eq!(rig.copy("game/r6/logs/x.log"), one);
    write(&log, &two);
    rig.run();
    assert_eq!(rig.copy("game/r6/logs/x.log"), two);
    let kept: Vec<Vec<u8>> = files_under(&rig.mira().join("_history")).iter().map(|p| fs::read(p).unwrap()).collect();
    assert!(kept.contains(&one));
}

#[test]
fn history_at_the_start_leaves_out_old_logs_and_sends_the_last_32_mb() {
    let mut rig = Rig::new(&[]);
    let logs = rig.game.join("r6").join("logs");
    let old = logs.join("old.log");
    write(&old, &lines("yesterday", 10));
    age(&old, 2);
    let big = logs.join("big.log");
    write(&big, &lines("big", 600_000));
    let source = fs::read(&big).unwrap();
    assert!(source.len() > 33 << 20);

    rig.join();
    rig.settle();
    assert!(!rig.has("game/r6/logs/old.log"));
    let copy = rig.copy("game/r6/logs/big.log");
    assert!(copy.len() <= 32 << 20 && copy.len() > (32 << 20) - 100);
    assert!(source.ends_with(&copy));
    assert_eq!(source[source.len() - copy.len() - 1], b'\n');

    let old_size = fs::metadata(&old).unwrap().len();
    append(&old, b"today\n");
    rig.rescan();
    assert_eq!(rig.copy("game/r6/logs/old.log"), b"today\n");
    let line = rig.manifest().lines().find(|l| l.starts_with("game/r6/logs/old.log\t")).unwrap().to_string();
    assert_eq!(line.split('\t').nth(3), Some(old_size.to_string().as_str()));
}

#[test]
fn a_restart_of_both_sides_sends_nothing_twice() {
    let mut rig = Rig::new(&[]);
    let red4ext = rig.game.join("red4ext").join("logs").join("red4ext.log");
    let scripts = rig.game.join("r6").join("logs").join("redscript.log");
    write(&red4ext, &lines("boot", 100));
    let old_scripts = lines("scripts", 50);
    write(&scripts, &old_scripts);
    rig.join();
    rig.settle();

    // While both apps are closed, one log grows and the other rotates.
    rig.collector = None;
    append(&red4ext, b"[info] after the restart\n");
    fs::rename(&scripts, scripts.with_file_name("redscript.1.log")).unwrap();
    let new_scripts = lines("scripts again", 5);
    write(&scripts, &new_scripts);

    rig.restart_host();
    rig.restart_player();
    rig.join();
    rig.settle();
    assert_eq!(rig.copy("game/red4ext/logs/red4ext.log"), fs::read(&red4ext).unwrap());
    assert_eq!(rig.copy("game/r6/logs/redscript.1.log"), old_scripts);
    assert_eq!(rig.copy("game/r6/logs/redscript.log"), new_scripts);
    assert_eq!(rig.sender.sent_bytes(), (b"[info] after the restart\n".len() + new_scripts.len()) as u64);
    assert!(!rig.mira().join("_history").exists());
}

#[test]
fn what_the_host_lost_while_its_socket_was_down_is_sent_again_once() {
    let mut rig = Rig::new(&[]);
    let log = rig.game.join("r6").join("logs").join("x.log");
    write(&log, &lines("one", 20));
    rig.join();
    rig.run();

    // The relay drops what the player sends while the host is away.
    append(&log, &lines("two", 20));
    write(&rig.game.join("r6").join("logs").join("new.log"), &lines("new", 20));
    let _lost = rig.step_after(Duration::from_secs(11));
    // A batch the player made before the host's new Welcome reaches the host after it.
    append(&log, &lines("three", 20));
    let late = rig.step();
    let welcome = rig.welcome();
    rig.deliver(&late);
    rig.sender.resume(&welcome);
    rig.settle();

    assert_eq!(rig.copy("game/r6/logs/x.log"), fs::read(&log).unwrap());
    assert_eq!(rig.copy("game/r6/logs/new.log"), lines("new", 20));
    assert!(!rig.mira().join("_history").exists());
    assert!(!rig.events().contains("out of step"));
}

#[test]
fn extra_paths_reach_the_host_and_stay_inside_the_game_folder() {
    let mut rig = Rig::new(&["bin/x64/StreetVendors/*.csv", r"r6\config", "../outside/*.txt", "bin/x64/StreetVendors/*"]);
    let vendors = rig.game.join("bin").join("x64").join("StreetVendors");
    write(&vendors.join("vendors.csv"), b"id,name\n1,Wakako\n");
    age(&vendors.join("vendors.csv"), 3);
    write(&vendors.join("crash.dmp"), b"MDMP");
    write(&rig.game.join("r6").join("config").join("settings.txt"), b"fov=90\n");
    write(&rig.game.join("r6").join("config").join("inputs.json"), b"{}");
    write(&rig.root.path().join("outside").join("secret.txt"), b"not the host's");
    rig.join();
    rig.settle();

    // An extra path's file is sent even when it hasn't changed for days, since the host named it.
    assert_eq!(rig.copy("extra/bin/x64/StreetVendors/vendors.csv"), b"id,name\n1,Wakako\n");
    assert_eq!(rig.copy("extra/r6/config/settings.txt"), b"fov=90\n");
    assert!(!rig.has("extra/r6/config/inputs.json"));
    let names: Vec<String> = files_under(&rig.session).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert!(!names.contains(&"secret.txt".to_string()));
    assert!(!names.contains(&"crash.dmp".to_string()));

    let sources = rig.host().sources(MIRA);
    assert_eq!(sources, rig.sender.sources());
    assert!(sources.contains(&count(SourceKind::Extra("bin/x64/StreetVendors/*.csv".into()), 1)));
    assert!(sources.contains(&count(SourceKind::Extra("../outside/*.txt".into()), 0)));
    assert!(sources.contains(&count(SourceKind::Extra("bin/x64/StreetVendors/*".into()), 1)));

    // The host adds a path mid-session.
    write(&rig.game.join("archive").join("pc").join("mod").join("vendors.xl"), b"Vendors.Wakako: {}\n");
    rig.sender.set_extra_paths(vec!["archive/pc/mod/*.xl".into()]);
    rig.run();
    assert_eq!(rig.copy("extra/archive/pc/mod/vendors.xl"), b"Vendors.Wakako: {}\n");
    assert!(rig.host().sources(MIRA).contains(&count(SourceKind::Extra("archive/pc/mod/*.xl".into()), 1)));
}

#[test]
fn crash_dumps_are_never_sent() {
    let mut rig = Rig::new(&[]);
    let report = rig.local_app_data.join("REDEngine").join("ReportQueue").join("crash-report-1");
    write(&report.join("report.txt"), b"Error reason: Unhandled exception\n");
    write(&report.join("metadata.json"), b"{\"version\":\"2.31\"}");
    write(&report.join("minidump.dmp"), b"MDMP");
    write(&report.join("screenshot.png"), b"\x89PNG");
    let own = rig.local_app_data.join("CD Projekt Red").join("Cyberpunk 2077");
    write(&own.join("UserSettings.json"), b"{}");
    write(&rig.app_log, b"2026-11-13 10:00:00 started\n");
    rig.join();
    rig.settle();

    assert!(rig.has("crash-reports/crash-report-1/report.txt"));
    assert!(rig.has("crash-reports/crash-report-1/metadata.json"));
    assert!(rig.has("localappdata/UserSettings.json"));
    assert!(rig.has("app/app.log"));
    let names: Vec<String> = files_under(&rig.session).iter().map(|p| p.to_string_lossy().into_owned()).collect();
    assert!(!names.iter().any(|n| n.ends_with(".dmp") || n.ends_with(".png")), "{names:?}");
    let sources = rig.sender.sources();
    assert!(sources.contains(&count(SourceKind::CrashReports, 2)));
    assert!(sources.contains(&count(SourceKind::GameOwn, 1)));
    assert!(sources.contains(&count(SourceKind::AppLog, 1)));
}

#[test]
fn the_hosts_own_session_folder_is_left_out() {
    let mut rig = Rig::build(&[], true);
    write(&rig.game.join("r6").join("logs").join("x.log"), &lines("host", 10));
    rig.join();
    rig.settle();
    rig.rescan();
    rig.rescan();
    assert!(rig.has("game/r6/logs/x.log"));
    assert!(!rig.mira().join("game").join("Co-op Logs").exists());
    assert!(rig.sender.sources().contains(&count(SourceKind::GameFolder, 1)));
}

#[test]
fn players_with_one_name_get_folders_of_their_own_for_the_whole_session() {
    let root = tempfile::tempdir().unwrap();
    let session = root.path().join("2026-11-13 K7R-M2Q");
    let mut host = Collector::new(session.clone());
    host.welcome("aaaa", "mira", PeerInfo::default());
    host.welcome("bbbb", "mira", PeerInfo::default());
    assert!(fs::read_to_string(session.join("mira").join("_peer.txt")).unwrap().contains("install id: aaaa"));
    assert!(fs::read_to_string(session.join("mira-2").join("_peer.txt")).unwrap().contains("install id: bbbb"));

    drop(host);
    let mut host = Collector::new(session.clone());
    host.welcome("bbbb", "mira renamed", PeerInfo::default());
    assert!(fs::read_to_string(session.join("mira-2").join("_peer.txt")).unwrap().contains("name: mira renamed"));
    assert!(!session.join("mira_renamed").exists());
}

#[test]
fn a_path_too_long_for_windows_goes_to_long() {
    let mut rig = Rig::new(&[]);
    // Five folders of 38 characters make the host's path pass 240 characters wherever the
    // temporary folder is.
    let deep = (0..5).fold(rig.game.clone(), |p, i| p.join(format!("a_folder_with_a_long_name_for_depth_{i}")));
    let log = deep.join("logs").join("deep.log");
    write(&log, &lines("deep", 5));
    rig.join();
    rig.settle();

    let line = rig.manifest().lines().find(|l| l.contains("deep.log")).unwrap().to_string();
    let here = line.rsplit('\t').next().unwrap().to_string();
    assert!(here.starts_with("_long/") && here.ends_with("_deep.log"), "{here}");
    assert_eq!(fs::read(rig.mira().join(&here)).unwrap(), lines("deep", 5));
}

#[test]
fn progress_counts_files_while_catching_up() {
    let mut rig = Rig::new(&[]);
    let logs = rig.game.join("r6").join("logs");
    write(&logs.join("a.log"), &lines("a", 20_000));
    write(&logs.join("b.log"), &lines("b", 60_000));
    rig.join();
    let mut seen = Vec::new();
    for _ in 0..20 {
        rig.run();
        seen.push(rig.sender.progress());
        if rig.sender.progress() == Progress::UpToDate {
            break;
        }
    }
    assert_eq!(seen.first(), Some(&Progress::CatchingUp { done: 0, total: 2 }));
    assert!(seen.contains(&Progress::CatchingUp { done: 1, total: 2 }));
    assert_eq!(seen.last(), Some(&Progress::UpToDate));
    assert_eq!(rig.copy("game/r6/logs/b.log"), lines("b", 60_000));
}

#[test]
fn sources_show_before_the_host_answers() {
    let mut rig = Rig::new(&[]);
    write(&rig.game.join("r6").join("logs").join("x.log"), b"x\n");
    assert!(rig.step().is_empty());
    assert!(rig.sender.sources().contains(&count(SourceKind::GameFolder, 1)));
}
