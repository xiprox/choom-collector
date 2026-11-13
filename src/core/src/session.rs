//! The real session: the first run's setup, then hosting or joining by itself on every start,
//! the players, and reconnecting (record/areas/network/01-relay-and-sessions.md). It runs on a
//! tokio runtime of its own, takes the window's commands, and sends it a snapshot whenever the
//! state changes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{Local, TimeZone as _};
use coop_proto::{Doing, GameInfo, Hello, PROTOCOL, PlayerInfo, ToHost, ToTester, Welcome, decode, encode};
use tokio::time::{sleep, timeout};

use crate::game;
use crate::relay::{self, HostEvent, HostLink, TesterEvent, TesterId, TesterLink};
use crate::settings::{SavedRole, Settings, default_logs_folder};
use crate::state::*;

/// How long a dropped tester shows as reconnecting before the list lets them go.
const TESTER_GRACE: Duration = Duration::from_secs(60);
/// The waits between tries to reach the relay, the last repeating.
const RETRIES: [Duration; 4] = [Duration::from_millis(500), Duration::from_millis(1500), Duration::from_secs(4), Duration::from_secs(8)];
const PING_EVERY: Duration = Duration::from_secs(10);

pub fn start(data_dir: PathBuf, app_version: String, name: Option<String>) -> (async_channel::Sender<Command>, async_channel::Receiver<Snapshot>) {
    let (command_tx, command_rx) = async_channel::unbounded::<Command>();
    let (snapshot_tx, snapshot_rx) = async_channel::unbounded::<Snapshot>();
    std::thread::Builder::new()
        .name("core".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("the core's runtime should start");
            runtime.block_on(async move {
                let mut core = Core::new(data_dir, app_version, command_rx, snapshot_tx).await;
                if let Some(name) = name {
                    core.me.name = name;
                }
                core.run().await;
            });
        })
        .expect("the core's thread should start");
    (command_tx, snapshot_rx)
}

/// What this PC is, found once at start.
struct Me {
    name: String,
    game: Option<GameInstall>,
    system: String,
}

struct Core {
    data_dir: PathBuf,
    app_version: String,
    settings: Settings,
    me: Me,
    commands: async_channel::Receiver<Command>,
    snapshots: async_channel::Sender<Snapshot>,
    /// The setup screen's state while it shows; the first run's, or after Stop or Leave.
    setup: Option<Setup>,
}

/// Why a session's loop returned.
enum Ended {
    /// The player stopped or left; show setup.
    ByPlayer,
    /// The window is gone.
    AppClosing,
}

impl Core {
    async fn new(data_dir: PathBuf, app_version: String, commands: async_channel::Receiver<Command>, snapshots: async_channel::Sender<Snapshot>) -> Self {
        let settings = Settings::load(&data_dir).unwrap_or_else(|e| {
            eprintln!("{e:#}");
            Settings::default()
        });
        let saved_game = settings.game_folder.clone();
        let me = tokio::task::spawn_blocking(move || Me {
            name: game::player_name(),
            game: saved_game.filter(|f| game::is_game_folder(f)).map(|f| game::describe(&f)).or_else(game::find),
            system: game::system(),
        })
        .await
        .expect("finding the game shouldn't panic");
        Self { data_dir, app_version, settings, me, commands, snapshots, setup: None }
    }

    fn show(&self, screen: Screen) -> bool {
        self.snapshots.try_send(Snapshot { app_version: self.app_version.clone(), screen }).is_ok()
    }

    async fn run(&mut self) {
        loop {
            let ended = match (self.settings.role, &self.setup) {
                (Some(SavedRole::Host), None) => self.host().await,
                (Some(SavedRole::Tester), None) => self.join().await,
                _ => match self.run_setup().await {
                    SetupEnd::Done => {
                        self.setup = None;
                        continue;
                    }
                    SetupEnd::AppClosing => Ended::AppClosing,
                },
            };
            match ended {
                Ended::AppClosing => return,
                // Stop and Leave show setup, with the answers so far, until the player starts again.
                Ended::ByPlayer => self.setup = Some(self.setup_screen(None)),
            }
        }
    }

    fn setup_screen(&self, error: Option<String>) -> Setup {
        let role = self.settings.role.map(Role::from).unwrap_or(Role::Tester);
        let code = relay::parse_code(&self.settings.code);
        Setup {
            role,
            invite: if role == Role::Tester { code.as_deref().map(display_code).unwrap_or_default() } else { String::new() },
            relay: self.settings.relay.clone(),
            host_code: if role == Role::Host { code.unwrap_or_else(relay::new_code) } else { relay::new_code() },
            logs_folder: self.settings.logs_folder.clone().unwrap_or_else(default_logs_folder),
            game: self.me.game.clone(),
            steam_name: self.me.name.clone(),
            error,
        }
    }

    async fn run_setup(&mut self) -> SetupEnd {
        let mut setup = self.setup.take().unwrap_or_else(|| self.setup_screen(None));
        loop {
            if !self.show(Screen::Setup(setup.clone())) {
                return SetupEnd::AppClosing;
            }
            let Ok(command) = self.commands.recv().await else { return SetupEnd::AppClosing };
            match command {
                Command::NewCode => setup.host_code = relay::new_code(),
                Command::SetGameFolder(folder) => {
                    let found = tokio::task::spawn_blocking(move || game::is_game_folder(&folder).then(|| game::describe(&folder))).await.ok().flatten();
                    match found {
                        Some(g) => {
                            self.me.game = Some(g.clone());
                            setup.game = Some(g);
                            setup.error = None;
                        }
                        None => setup.error = Some("That folder has no bin/x64/Cyberpunk2077.exe in it".into()),
                    }
                }
                Command::SetLogsFolder(folder) => setup.logs_folder = folder,
                Command::FinishSetup(answers) => match self.apply_setup(&setup, answers) {
                    Ok(()) => return SetupEnd::Done,
                    Err(problem) => setup.error = Some(problem),
                },
                _ => {}
            }
        }
    }

    /// Checks the answers and saves them; the problem in words when they don't add up.
    fn apply_setup(&mut self, setup: &Setup, answers: SetupAnswers) -> Result<(), String> {
        let (invite_relay, invite_code) = match answers.invite.rsplit_once('/') {
            Some((relay, code)) => (relay.trim().to_string(), code.trim().to_string()),
            None => (String::new(), answers.invite.trim().to_string()),
        };
        let relay = if answers.relay.trim().is_empty() { invite_relay } else { answers.relay.trim().to_string() };
        if relay.is_empty() {
            return Err("Enter the relay's address, such as relay.example.net".into());
        }
        let code = match answers.role {
            Role::Host => setup.host_code.clone(),
            Role::Tester => relay::parse_code(&invite_code).ok_or("A code is 6 letters and digits, such as K7R-M2Q")?,
        };
        if self.settings.code != code || self.settings.relay != relay {
            self.settings.resume.clear();
        }
        self.settings.role = Some(answers.role.into());
        self.settings.relay = relay;
        self.settings.code = code;
        self.settings.logs_folder = Some(answers.logs_folder);
        self.settings.game_folder = answers.game_folder.or_else(|| self.me.game.as_ref().map(|g| g.folder.clone()));
        self.settings.save(&self.data_dir).map_err(|e| format!("Couldn't save the settings: {e:#}"))?;
        Ok(())
    }

    fn hello(&self) -> Hello {
        Hello {
            protocol: PROTOCOL,
            app_version: self.app_version.clone(),
            install_id: self.settings.install_id.clone(),
            name: self.me.name.clone(),
            game: self.game_info(),
            system: self.me.system.clone(),
        }
    }

    fn game_info(&self) -> GameInfo {
        match &self.me.game {
            Some(g) => GameInfo { version: g.version.clone().unwrap_or_default(), store: game::store(&g.folder).into() },
            None => GameInfo::default(),
        }
    }

    // ---------- hosting ----------

    async fn host(&mut self) -> Ended {
        let started = Local::now();
        let folder = self
            .settings
            .logs_folder
            .clone()
            .unwrap_or_else(default_logs_folder)
            .join(format!("{} {}", started.format("%Y-%m-%d"), display_code(&self.settings.code)));
        let mut host = Hosting {
            players: BTreeMap::new(),
            by_tester: BTreeMap::new(),
            next_id: 2,
            link: Link::Connecting,
            latency: None,
            problem: None,
            folder,
        };
        host.players.insert(
            1,
            Entry {
                info: PlayerInfo {
                    id: 1,
                    name: self.me.name.clone(),
                    is_host: true,
                    game: self.game_info(),
                    system: self.me.system.clone(),
                    joined: started.timestamp(),
                    resumed: false,
                    doing: Doing::Nothing,
                    connected: true,
                },
                install_id: self.settings.install_id.clone(),
                tester: None,
                left: None,
            },
        );

        let mut tries = 0usize;
        loop {
            if !self.show_hosting(&host) {
                return Ended::AppClosing;
            }
            let resume = (!self.settings.resume.is_empty()).then(|| self.settings.resume.clone());
            let opened = tokio::select! {
                opened = HostLink::open(&self.settings.relay, &self.settings.code, resume.as_deref()) => opened,
                ended = self.wait_for_stop() => return ended,
            };
            let mut link = match opened {
                Ok(link) => link,
                Err(e) => {
                    let message = format!("{e:#}");
                    // Another host holding the code won't change by trying again soon.
                    let held = message.contains("another host");
                    host.problem = Some(if held { "Another host is using this code. Stop, then pick New code.".into() } else { format!("Can't reach the relay: {message}") });
                    host.link = Link::Reconnecting;
                    if !self.show_hosting(&host) {
                        return Ended::AppClosing;
                    }
                    let wait = if held { Duration::from_secs(30) } else { RETRIES[tries.min(RETRIES.len() - 1)] };
                    tries += 1;
                    tokio::select! {
                        _ = sleep(wait) => continue,
                        ended = self.wait_for_stop() => return ended,
                    }
                }
            };
            tries = 0;
            if link.resume != self.settings.resume {
                self.settings.resume = link.resume.clone();
                if let Err(e) = self.settings.save(&self.data_dir) {
                    eprintln!("{e:#}");
                }
            }
            host.link = Link::Connected { latency_ms: 0 };
            host.problem = None;
            let _ = link.ping().await;

            let mut ping = tokio::time::interval(PING_EVERY);
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                host.latency = link.latency_ms();
                host.link = Link::Connected { latency_ms: host.latency.unwrap_or(0) };
                if !self.show_hosting(&host) {
                    return Ended::AppClosing;
                }
                tokio::select! {
                    event = link.next() => match event {
                        HostEvent::TesterJoined(_) => {}
                        HostEvent::Message { from, data } => {
                            if let Some(message) = decode::<ToHost>(&data) {
                                let replies = host.receive(from, message, &self.me.name, &self.settings.extra_paths);
                                for (to, reply) in replies {
                                    let sent = match to {
                                        Some(t) => link.send_to(&t, &encode(&reply)).await,
                                        None => link.send_to_everyone(&encode(&reply)).await,
                                    };
                                    if sent.is_err() { break; }
                                }
                            }
                        }
                        HostEvent::TesterLeft(tester) => {
                            if host.left(tester) {
                                let _ = link.send_to_everyone(&encode(&ToTester::Players(host.list()))).await;
                            }
                        }
                        HostEvent::Dropped => {
                            host.link = Link::Reconnecting;
                            break;
                        }
                    },
                    command = self.commands.recv() => match command {
                        Ok(Command::Stop) => {
                            link.stop().await;
                            return Ended::ByPlayer;
                        }
                        Ok(_) => {}
                        Err(_) => {
                            link.stop().await;
                            return Ended::AppClosing;
                        }
                    },
                    _ = ping.tick() => { let _ = link.ping().await; }
                    _ = tick.tick() => {
                        if host.forget_gone() {
                            let _ = link.send_to_everyone(&encode(&ToTester::Players(host.list()))).await;
                        }
                    }
                }
            }
        }
    }

    fn show_hosting(&self, host: &Hosting) -> bool {
        self.show(Screen::Session(Session {
            role: Role::Host,
            code: self.settings.code.clone(),
            relay: self.settings.relay.clone(),
            link: host.link.clone(),
            host_name: self.me.name.clone(),
            players: host.players.values().map(|e| player(&e.info, 1, e.left.is_some() || !e.info.connected)).collect(),
            shared: Vec::new(),
            my_mods: Vec::new(),
            logs: LogsLine::Host { folder: host.folder.clone() },
            problem: host.problem.clone(),
        }))
    }

    /// Waits for Stop or Leave while the session isn't connected, so the player can give up.
    async fn wait_for_stop(&self) -> Ended {
        loop {
            match self.commands.recv().await {
                Ok(Command::Stop | Command::Leave) => return Ended::ByPlayer,
                Ok(_) => {}
                Err(_) => return Ended::AppClosing,
            }
        }
    }

    // ---------- joining ----------

    async fn join(&mut self) -> Ended {
        let mut joined = Joining { me: None, host_name: String::new(), players: Vec::new(), link: Link::Connecting, problem: None };
        let mut tries = 0usize;
        loop {
            if !self.show_joining(&joined) {
                return Ended::AppClosing;
            }
            let opened = tokio::select! {
                opened = TesterLink::open(&self.settings.relay, &self.settings.code) => opened,
                ended = self.wait_for_stop() => return ended,
            };
            let mut link = match opened {
                Ok(link) => link,
                Err(e) => {
                    let message = format!("{e:#}");
                    let (state, problem) = if message.contains("unknown-code") {
                        (Link::WaitingForHost, None)
                    } else if message.contains("session-full") {
                        (Link::Reconnecting, Some("The session is full: it has 8 testers already".to_string()))
                    } else {
                        (Link::Reconnecting, Some(format!("Can't reach the relay: {message}")))
                    };
                    joined.link = state;
                    joined.problem = problem;
                    if !self.show_joining(&joined) {
                        return Ended::AppClosing;
                    }
                    let wait = RETRIES[tries.min(RETRIES.len() - 1)];
                    tries += 1;
                    tokio::select! {
                        _ = sleep(wait) => continue,
                        ended = self.wait_for_stop() => return ended,
                    }
                }
            };
            tries = 0;
            if link.send(&encode(&ToHost::Hello(self.hello()))).await.is_err() {
                continue;
            }

            // The host answers a Hello at once; a host that doesn't is treated as a drop.
            match timeout(Duration::from_secs(10), link.next()).await {
                Ok(TesterEvent::Message(data)) => match decode::<ToTester>(&data) {
                    Some(ToTester::Welcome(Welcome { host_name, player_id, .. })) => {
                        joined.host_name = host_name;
                        joined.me = Some(player_id);
                        joined.link = Link::Connected { latency_ms: 0 };
                        joined.problem = None;
                    }
                    Some(ToTester::Refused { reason }) => {
                        joined.problem = Some(reason);
                        joined.link = Link::Reconnecting;
                        if !self.show_joining(&joined) {
                            return Ended::AppClosing;
                        }
                        // A refusal stands until one of the two apps changes, so stop asking.
                        return self.wait_for_stop().await;
                    }
                    _ => continue,
                },
                _ => {
                    link.leave().await;
                    continue;
                }
            }
            let _ = link.ping().await;

            let mut ping = tokio::time::interval(PING_EVERY);
            loop {
                joined.link = Link::Connected { latency_ms: link.latency_ms().unwrap_or(0) };
                if !self.show_joining(&joined) {
                    return Ended::AppClosing;
                }
                tokio::select! {
                    event = link.next() => match event {
                        TesterEvent::Message(data) => match decode::<ToTester>(&data) {
                            Some(ToTester::Players(players)) => joined.players = players,
                            Some(ToTester::Welcome(w)) => joined.me = Some(w.player_id),
                            _ => {}
                        },
                        TesterEvent::Ended(_) => {
                            joined.link = Link::WaitingForHost;
                            joined.players.clear();
                            break;
                        }
                        TesterEvent::Dropped => {
                            joined.link = Link::Reconnecting;
                            break;
                        }
                    },
                    command = self.commands.recv() => match command {
                        Ok(Command::Leave) => {
                            link.leave().await;
                            return Ended::ByPlayer;
                        }
                        Ok(_) => {}
                        Err(_) => {
                            link.leave().await;
                            return Ended::AppClosing;
                        }
                    },
                    _ = ping.tick() => { let _ = link.ping().await; }
                }
            }
        }
    }

    fn show_joining(&self, joined: &Joining) -> bool {
        let me = joined.me.unwrap_or(0);
        let mut players: Vec<Player> = joined.players.iter().map(|p| player(p, me, !p.connected)).collect();
        if players.is_empty() {
            // Until the host's list arrives, the window still shows who this PC is.
            players.push(player(
                &PlayerInfo {
                    id: 0,
                    name: self.me.name.clone(),
                    is_host: false,
                    game: self.game_info(),
                    system: self.me.system.clone(),
                    joined: Local::now().timestamp(),
                    resumed: false,
                    doing: Doing::Nothing,
                    connected: true,
                },
                0,
                false,
            ));
        }
        self.show(Screen::Session(Session {
            role: Role::Tester,
            code: self.settings.code.clone(),
            relay: self.settings.relay.clone(),
            link: joined.link.clone(),
            host_name: if joined.host_name.is_empty() { "the host".into() } else { joined.host_name.clone() },
            players,
            shared: Vec::new(),
            my_mods: Vec::new(),
            logs: LogsLine::Tester { host: if joined.host_name.is_empty() { "the host".into() } else { joined.host_name.clone() }, sending: Sending::Idle },
            problem: joined.problem.clone(),
        }))
    }
}

enum SetupEnd {
    /// The answers are saved; the session they ask for starts next.
    Done,
    AppClosing,
}

struct Entry {
    info: PlayerInfo,
    install_id: String,
    /// The relay's ID for this player's socket while it's open.
    tester: Option<TesterId>,
    /// When the player's socket closed, while they're shown as reconnecting.
    left: Option<Instant>,
}

struct Hosting {
    players: BTreeMap<u64, Entry>,
    by_tester: BTreeMap<TesterId, u64>,
    next_id: u64,
    link: Link,
    latency: Option<u32>,
    problem: Option<String>,
    folder: PathBuf,
}

impl Hosting {
    fn list(&self) -> Vec<PlayerInfo> {
        self.players.values().map(|e| PlayerInfo { connected: e.left.is_none(), ..e.info.clone() }).collect()
    }

    /// Takes a tester's message and returns what to send, and to whom: `None` is everyone.
    fn receive(&mut self, from: TesterId, message: ToHost, host_name: &str, extra_paths: &[String]) -> Vec<(Option<TesterId>, ToTester)> {
        match message {
            ToHost::Hello(hello) => {
                if hello.protocol != PROTOCOL {
                    return vec![(Some(from), ToTester::Refused {
                        reason: format!("{host_name} runs another version of Cyberpunk Co-op Tools; update so you both run the same one"),
                    })];
                }
                // A player who comes back keeps their place in the list, and their ID.
                let returning = self.players.iter().find(|(_, e)| e.install_id == hello.install_id && !e.info.is_host).map(|(id, _)| *id);
                let id = returning.unwrap_or_else(|| {
                    self.next_id += 1;
                    self.next_id - 1
                });
                let entry = self.players.entry(id).or_insert_with(|| Entry {
                    info: PlayerInfo {
                        id,
                        name: hello.name.clone(),
                        is_host: false,
                        game: hello.game.clone(),
                        system: hello.system.clone(),
                        joined: Local::now().timestamp(),
                        resumed: false,
                        doing: Doing::Nothing,
                        connected: true,
                    },
                    install_id: hello.install_id.clone(),
                    tester: None,
                    left: None,
                });
                if let Some(old) = entry.tester.replace(from) {
                    self.by_tester.remove(&old);
                }
                entry.info.resumed = returning.is_some();
                entry.info.name = hello.name;
                entry.info.game = hello.game;
                entry.info.system = hello.system;
                entry.left = None;
                self.by_tester.insert(from, id);
                vec![
                    (Some(from), ToTester::Welcome(Welcome { host_name: host_name.into(), player_id: id, extra_paths: extra_paths.to_vec(), logs: Vec::new() })),
                    (None, ToTester::Players(self.list())),
                ]
            }
            ToHost::Doing(doing) => {
                let Some(id) = self.by_tester.get(&from) else { return Vec::new() };
                let entry = self.players.get_mut(id).expect("a tester's entry lives as long as its socket");
                if entry.info.doing == doing {
                    return Vec::new();
                }
                entry.info.doing = doing;
                vec![(None, ToTester::Players(self.list()))]
            }
            // Stages 5 and 6 of record/areas/product/plan.md pass these to coop-logs and coop-mods.
            ToHost::Logs(_) | ToHost::Mods(_) => Vec::new(),
        }
    }

    /// A tester's socket closed; true when the list changed.
    fn left(&mut self, tester: TesterId) -> bool {
        let Some(id) = self.by_tester.remove(&tester) else { return false };
        if let Some(entry) = self.players.get_mut(&id) {
            entry.tester = None;
            entry.left = Some(Instant::now());
        }
        true
    }

    /// Lets go of testers who've been gone longer than the grace; true when the list changed.
    fn forget_gone(&mut self) -> bool {
        let before = self.players.len();
        self.players.retain(|_, e| e.left.is_none_or(|t| t.elapsed() < TESTER_GRACE));
        self.players.len() != before
    }
}

struct Joining {
    me: Option<u64>,
    host_name: String,
    players: Vec<PlayerInfo>,
    link: Link,
    problem: Option<String>,
}

fn player(info: &PlayerInfo, me: u64, reconnecting: bool) -> Player {
    Player {
        id: info.id,
        name: info.name.clone(),
        is_me: info.id == me,
        is_host: info.is_host,
        game_version: info.game.version.clone(),
        store: info.game.store.clone(),
        system: info.system.clone(),
        joined: Local.timestamp_opt(info.joined, 0).single().unwrap_or_else(Local::now),
        resumed: info.resumed,
        bytes: 0,
        state: if reconnecting {
            PlayerState::Reconnecting
        } else {
            match info.doing {
                Doing::Nothing => PlayerState::Fine,
                Doing::Fetching { done, total } => PlayerState::Fetching { done, total },
                Doing::NeedsRestart => PlayerState::NeedsRestart,
            }
        },
        sources: None,
    }
}

/// `K7RM2Q` reads `K7R-M2Q`.
fn display_code(code: &str) -> String {
    if code.len() == 6 { format!("{}-{}", &code[..3], &code[3..]) } else { code.to_string() }
}
