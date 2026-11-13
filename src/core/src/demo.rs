//! A made-up session for working on the window without a relay. It shows, all at once, the
//! states the frames in experiments/areas/interface/frames/ show, and answers the window's
//! commands the way a real session would, roughly.

use std::path::PathBuf;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use chrono::{Duration as Ago, Local};

use crate::state::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoScreen {
    Host,
    Tester,
    SetupJoin,
    SetupHost,
}

impl FromStr for DemoScreen {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "host" => Ok(Self::Host),
            "tester" => Ok(Self::Tester),
            "setup-join" => Ok(Self::SetupJoin),
            "setup-host" => Ok(Self::SetupHost),
            _ => Err(format!("unknown demo screen {s:?}: host, tester, setup-join, or setup-host")),
        }
    }
}

/// Runs the made-up session on a thread of its own, the way the real core runs its session.
pub fn start(screen: DemoScreen, app_version: String) -> (async_channel::Sender<Command>, async_channel::Receiver<Snapshot>) {
    let (command_tx, command_rx) = async_channel::unbounded::<Command>();
    let (snapshot_tx, snapshot_rx) = async_channel::unbounded::<Snapshot>();
    thread::Builder::new()
        .name("demo-session".into())
        .spawn(move || {
            let mut demo = Demo::new(screen, app_version);
            if snapshot_tx.send_blocking(demo.snapshot()).is_err() {
                return;
            }
            loop {
                // A tick a second moves the fetching along, as a real transfer would.
                match command_rx.recv_blocking_timeout(Duration::from_secs(1)) {
                    Ok(Some(command)) => demo.apply(command),
                    Ok(None) => demo.tick(),
                    Err(()) => return,
                }
                if snapshot_tx.send_blocking(demo.snapshot()).is_err() {
                    return;
                }
            }
        })
        .expect("the demo thread should start");
    (command_tx, snapshot_rx)
}

trait RecvTimeout<T> {
    /// `Ok(Some)` for a message, `Ok(None)` when the time ran out, `Err` once the sender is gone.
    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<Option<T>, ()>;
}

impl<T> RecvTimeout<T> for async_channel::Receiver<T> {
    fn recv_blocking_timeout(&self, timeout: Duration) -> Result<Option<T>, ()> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.try_recv() {
                Ok(v) => return Ok(Some(v)),
                Err(async_channel::TryRecvError::Closed) => return Err(()),
                Err(async_channel::TryRecvError::Empty) => {}
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

struct Demo {
    screen: Screen,
    app_version: String,
}

impl Demo {
    fn new(screen: DemoScreen, app_version: String) -> Self {
        let screen = match screen {
            DemoScreen::Host => Screen::Session(session(Role::Host)),
            DemoScreen::Tester => Screen::Session(session(Role::Tester)),
            DemoScreen::SetupJoin => Screen::Setup(setup(Role::Tester)),
            DemoScreen::SetupHost => Screen::Setup(setup(Role::Host)),
        };
        Self { screen, app_version }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            app_version: self.app_version.clone(),
            screen: self.screen.clone(),
            // The host's showcase has an update waiting, so the title bar's button shows somewhere.
            update: matches!(&self.screen, Screen::Session(s) if s.role == Role::Host).then(|| "0.2.0".to_string()),
            settings: match &self.screen {
                Screen::Session(s) => Some(SettingsView {
                    role: Some(s.role),
                    relay: s.relay.clone(),
                    code: s.code.clone(),
                    logs_folder: PathBuf::from(r"D:\Cyberpunk Logs"),
                    extra_paths: vec!["bin/x64/StreetVendors/*.csv".into()],
                    never_share: Vec::new(),
                    app_log: PathBuf::from("app.log"),
                }),
                Screen::Setup(_) => None,
            },
        }
    }

    fn tick(&mut self) {
        let Screen::Session(s) = &mut self.screen else { return };
        for player in &mut s.players {
            if let PlayerState::Fetching { done, total } = &mut player.state {
                *done = if *done >= *total { 4 } else { *done + 1 };
            }
        }
        for m in &mut s.shared {
            for f in &mut m.followers {
                if let Delivery::Fetching { done, total } = &mut f.delivery {
                    *done = if *done >= *total { 4 } else { *done + 1 };
                }
            }
            if let ModStatus::Fetching { done, total } = &mut m.status {
                if *done + 1 >= *total {
                    m.status = ModStatus::UpToDate;
                } else {
                    *done += 1;
                }
            }
        }
        if let LogsLine::Tester { sending: Sending::CatchingUp { done, total }, .. } = &mut s.logs {
            *done = (*done + 3).min(*total);
        }
    }

    fn apply(&mut self, command: Command) {
        match (&mut self.screen, command) {
            (Screen::Setup(_), Command::FinishSetup(answers)) => {
                self.screen = Screen::Session(session(answers.role));
            }
            (Screen::Setup(setup), Command::NewCode) => {
                setup.host_code = if setup.host_code == "K7RM2Q" { "B4XT9P".into() } else { "K7RM2Q".into() };
            }
            (Screen::Session(s), Command::Follow { key, on }) => {
                if let Some(m) = s.shared.iter_mut().find(|m| m.key == key) {
                    m.follow = if on { Follow::Yes } else { Follow::No };
                    m.status = if on { ModStatus::Fetching { done: 0, total: 6 } } else { ModStatus::NotOnYourPc };
                }
            }
            (Screen::Session(s), Command::Unfollow { key, .. }) => {
                if let Some(m) = s.shared.iter_mut().find(|m| m.key == key) {
                    m.follow = Follow::No;
                    m.status = ModStatus::NotOnYourPc;
                }
            }
            (Screen::Session(s), Command::TakeAddedFolder { key }) => {
                if let Some(m) = s.shared.iter_mut().find(|m| m.key == key) {
                    m.follow = Follow::Yes;
                    m.status = ModStatus::Fetching { done: 0, total: 3 };
                }
            }
            (Screen::Session(s), Command::Share { key, on }) => {
                let Some(mine) = s.my_mods.iter_mut().find(|m| m.key == key) else { return };
                mine.shared = on;
                let (name, me) = (mine.name.clone(), s.players.iter().find(|p| p.is_me).map(|p| p.name.clone()).unwrap_or_default());
                if on {
                    let followers = s.players.iter().filter(|p| !p.is_me).map(|p| Follower { name: p.name.clone(), delivery: Delivery::UpToDate }).collect();
                    s.shared.insert(0, SharedMod {
                        key,
                        name,
                        sharer: me,
                        mine: true,
                        follow: Follow::No,
                        has_dll: false,
                        status: ModStatus::Sharing,
                        changed: Local::now(),
                        followers,
                    });
                } else {
                    s.shared.retain(|m| !(m.mine && m.key == key));
                }
            }
            (Screen::Session(_), Command::Stop | Command::Leave) => {
                self.screen = Screen::Setup(setup(Role::Tester));
            }
            _ => {}
        }
    }
}

fn setup(role: Role) -> Setup {
    Setup {
        role,
        invite: if role == Role::Tester { "relay.example.net/K7R-M2Q".into() } else { String::new() },
        relay: "relay.example.net".into(),
        host_code: "K7RM2Q".into(),
        logs_folder: PathBuf::from(r"D:\Cyberpunk Logs"),
        game: Some(GameInstall {
            folder: PathBuf::from(r"D:\SteamLibrary\steamapps\common\Cyberpunk 2077"),
            version: Some(if role == Role::Tester { "2.3" } else { "2.31" }.into()),
        }),
        steam_name: if role == Role::Tester { "tetsu" } else { "xip" }.into(),
        error: None,
    }
}

fn session(role: Role) -> Session {
    let now = Local::now();
    let host = role == Role::Host;
    let sources = |game: u32, crashes: u32, own: u32, extra: u32| {
        Some(vec![
            LogSource { kind: SourceKind::GameFolder, count: game },
            LogSource { kind: SourceKind::CrashReports, count: crashes },
            LogSource { kind: SourceKind::GameOwn, count: own },
            LogSource { kind: SourceKind::Extra { pattern: "bin/x64/StreetVendors/*.csv".into() }, count: extra },
        ])
    };
    let player = |id, name: &str, me: bool, is_host: bool, version: &str, system: &str, mins: i64, bytes, state| Player {
        id,
        name: name.into(),
        is_me: me,
        is_host,
        game_version: version.into(),
        store: "Steam".into(),
        system: system.into(),
        joined: now - Ago::minutes(mins),
        resumed: !me && !is_host,
        bytes,
        state,
        sources: None,
    };
    let mut players = vec![
        player(1, "xip", host, true, "2.31", "Windows 11", 75, 0, PlayerState::Fine),
        player(2, "bossun", false, false, "2.31", "Windows 11", 71, 12_400_000, PlayerState::Fetching { done: 4, total: 12 }),
        player(3, "mira", false, false, "2.31", "Windows 10", 60, 3_100_000, PlayerState::Reconnecting),
        player(4, "tetsu", !host, false, "2.3", "SteamOS, Steam Deck", 2, 4_100_000, PlayerState::NeedsRestart),
    ];
    if host {
        players[1].sources = sources(182, 3, 3, 0);
        players[2].sources = sources(140, 0, 3, 0);
        players[3].sources = sources(86, 1, 3, 2);
        players[0].sources = sources(214, 0, 3, 0);
    } else {
        players[3].sources = sources(86, 1, 3, 2);
        for p in &mut players[1..3] {
            p.state = if p.state == PlayerState::Reconnecting { PlayerState::Reconnecting } else { PlayerState::Fine };
        }
    }

    let tile = |name: &str, delivery| Follower { name: name.into(), delivery };
    let shared = if host {
        vec![
            SharedMod {
                key: "street vendors+".into(),
                name: "Street Vendors+".into(),
                sharer: "xip".into(),
                mine: true,
                follow: Follow::No,
                has_dll: false,
                status: ModStatus::Sharing,
                changed: now - Ago::minutes(2),
                followers: vec![
                    tile("bossun", Delivery::Fetching { done: 4, total: 12 }),
                    tile("mira", Delivery::Paused),
                    tile("tetsu", Delivery::NeedsRestart),
                ],
            },
            SharedMod {
                key: "neon signs overhaul".into(),
                name: "Neon Signs Overhaul".into(),
                sharer: "xip".into(),
                mine: true,
                follow: Follow::No,
                has_dll: false,
                status: ModStatus::Sharing,
                changed: now - Ago::minutes(45),
                followers: ["bossun", "mira", "tetsu"].iter().map(|n| tile(n, Delivery::UpToDate)).collect(),
            },
            others_mod("equipmentex", "EquipmentEx", "bossun", true, Follow::Yes, ModStatus::UpToDate, now - Ago::minutes(59)),
            others_mod("better apartments", "Better Apartments", "mira", false, Follow::No, ModStatus::NotOnYourPc, now - Ago::minutes(73)),
        ]
    } else {
        vec![
            others_mod("street vendors+", "Street Vendors+", "xip", false, Follow::Yes, ModStatus::NeedsRestart, now - Ago::minutes(2)),
            others_mod("neon signs overhaul", "Neon Signs Overhaul", "xip", false, Follow::Yes, ModStatus::UpToDate, now - Ago::minutes(45)),
            others_mod("equipmentex", "EquipmentEx", "bossun", true, Follow::No, ModStatus::NotOnYourPc, now - Ago::minutes(59)),
            others_mod("better apartments", "Better Apartments", "mira", false, Follow::No, ModStatus::NotOnYourPc, now - Ago::minutes(73)),
        ]
    };

    let my_mod = |name: &str, places: &[Place], bytes, shared, loose: Option<&str>| MyMod {
        key: name.to_lowercase(),
        name: name.into(),
        places: places.to_vec(),
        bytes,
        shared,
        loose_file: loose.map(Into::into),
    };
    let my_mods = vec![
        my_mod("Street Vendors+", &[Place::Archive, Place::Redscript, Place::TweakXl], 1_200_000, host, None),
        my_mod("Neon Signs Overhaul", &[Place::Archive], 212_000_000, host, None),
        my_mod("ArchiveXL", &[Place::Red4ext, Place::Archive], 3_800_000, false, None),
        my_mod("Codeware", &[Place::Red4ext], 2_100_000, false, None),
        my_mod("Photo Mode Unlocker", &[Place::Cet], 96_000, false, None),
        my_mod("Virtual Atelier", &[Place::Redscript], 640_000, false, Some("archive/pc/mod/VirtualAtelier.archive")),
    ];

    Session {
        role,
        code: "K7RM2Q".into(),
        relay: "relay.example.net".into(),
        link: Link::Connected { latency_ms: 38 },
        host_name: "xip".into(),
        players,
        shared,
        my_mods,
        logs: if host {
            LogsLine::Host { folder: PathBuf::from(r"D:\Cyberpunk Logs\2026-11-13 K7R-M2Q") }
        } else {
            LogsLine::Tester { host: "xip".into(), sending: Sending::CatchingUp { done: 38, total: 96 } }
        },
        problem: None,
    }
}

fn others_mod(
    key: &str,
    name: &str,
    sharer: &str,
    has_dll: bool,
    follow: Follow,
    status: ModStatus,
    changed: chrono::DateTime<Local>,
) -> SharedMod {
    SharedMod {
        key: key.into(),
        name: name.into(),
        sharer: sharer.into(),
        mine: false,
        follow,
        has_dll,
        status,
        changed,
        followers: Vec::new(),
    }
}
