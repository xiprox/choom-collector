//! The real session: the first run's setup, then hosting or joining by itself on every start,
//! the players, and reconnecting (record/areas/network/01-relay-and-sessions.md). It runs on a
//! tokio runtime of its own, takes the window's commands, and sends it a snapshot whenever the
//! state changes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Local, TimeZone as _};
use coop_logs::{Collector, PeerInfo, Progress, Sender, SenderConfig, SourceCount};
use coop_mods::{Hub, Player as ModPlayer, PlayerConfig, Target};
use coop_proto::{Doing, GameInfo, Hello, PROTOCOL, PlayerInfo, ToHost, ToTester, Welcome, decode, encode};
use tokio::time::{sleep, timeout};

use crate::game;
use crate::relay::{self, HostEvent, HostLink, TesterEvent, TesterId, TesterLink};
use crate::settings::{SavedRole, Settings, default_logs_folder};
use crate::state::*;

/// How long a dropped tester shows as reconnecting before the list lets them go. `AWAY_GRACE` in
/// coop-mods' hub, how long a dropped sharer keeps their folders, is the same on purpose.
const TESTER_GRACE: Duration = Duration::from_secs(60);
/// The waits between tries to reach the relay, the last repeating.
const RETRIES: [Duration; 4] = [Duration::from_millis(500), Duration::from_millis(1500), Duration::from_secs(4), Duration::from_secs(8)];
const PING_EVERY: Duration = Duration::from_secs(10);
/// How often the logs' `Sender` and `Collector` get a turn. A `Sender` looks at its files once a
/// second and scans every 10s by itself, so most turns do nothing; a backlog sends up to 2 MB a turn.
const LOGS_EVERY: Duration = Duration::from_millis(250);

pub fn start(
    data_dir: PathBuf,
    app_log: PathBuf,
    app_version: String,
    name: Option<String>,
    local_app_data: Option<PathBuf>,
) -> (async_channel::Sender<Command>, async_channel::Receiver<Snapshot>) {
    let (command_tx, command_rx) = async_channel::unbounded::<Command>();
    let (snapshot_tx, snapshot_rx) = async_channel::unbounded::<Snapshot>();
    std::thread::Builder::new()
        .name("core".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("the core's runtime should start");
            runtime.block_on(async move {
                let mut core = Core::new(data_dir, app_log, app_version, command_rx, snapshot_tx).await;
                if let Some(name) = name {
                    core.me.name = name;
                }
                core.local_app_data = local_app_data;
                tokio::spawn(updates::watch(core.update.clone(), core.last.clone(), core.snapshots.clone()));
                core.run().await;
            });
        })
        .expect("the core's thread should start");
    (command_tx, snapshot_rx)
}

mod updates {
    //! Velopack's update check against the project's GitHub releases, a few seconds after start
    //! and every 6 hours after, downloading what it finds so the player only has to restart
    //! (record/decisions/004-releases-with-release-please-and-velopack.md).

    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use velopack::sources::GithubSource;
    use velopack::{UpdateCheck, UpdateInfo, UpdateManager};

    use crate::state::Snapshot;

    const RELEASES: &str = "https://github.com/xiprox/cyberpunk-coop-tools";

    pub struct Ready {
        pub version: String,
        manager: UpdateManager,
        info: UpdateInfo,
    }

    impl Ready {
        /// Installs the update and starts the new version; the process ends here when it works.
        pub fn apply(&self) {
            if let Err(e) = self.manager.apply_updates_and_restart(&self.info) {
                log::warn!("couldn't install the update to {}: {e}", self.version);
            }
        }
    }

    pub type Slot = Arc<Mutex<Option<Ready>>>;

    /// Checks until an update is ready, then sends the window its last snapshot again with the
    /// update in it, so the button shows even while nothing else changes.
    pub async fn watch(slot: Slot, last: Arc<Mutex<Option<Snapshot>>>, snapshots: async_channel::Sender<Snapshot>) {
        tokio::time::sleep(Duration::from_secs(5)).await;
        loop {
            let found = tokio::task::spawn_blocking(|| -> Result<Option<Ready>, velopack::Error> {
                // An app run from a build folder, not installed by Velopack, can't update, and says so here.
                let manager = UpdateManager::new(GithubSource::new(RELEASES, None, false), None, None)?;
                match manager.check_for_updates()? {
                    UpdateCheck::UpdateAvailable(info) => {
                        manager.download_updates(&info, None)?;
                        Ok(Some(Ready { version: info.TargetFullRelease.Version.clone(), manager, info: *info }))
                    }
                    _ => Ok(None),
                }
            })
            .await;
            match found {
                Ok(Ok(Some(ready))) => {
                    log::info!("downloaded the update to {}", ready.version);
                    let version = ready.version.clone();
                    *slot.lock().expect("the update slot isn't poisoned") = Some(ready);
                    // Sent under the lock `Core::show` takes too, so this can't overtake a newer snapshot.
                    let mut last = last.lock().expect("the last snapshot isn't poisoned");
                    if let Some(snapshot) = last.as_mut() {
                        snapshot.update = Some(version);
                        let _ = snapshots.try_send(snapshot.clone());
                    }
                    return;
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => log::info!("no update check: {e}"),
                Err(e) => log::warn!("the update check stopped: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    }
}

/// What this PC is, found once at start.
struct Me {
    name: String,
    game: Option<GameInstall>,
    system: String,
}

struct Core {
    data_dir: PathBuf,
    app_log: PathBuf,
    app_version: String,
    settings: Settings,
    me: Me,
    commands: async_channel::Receiver<Command>,
    snapshots: async_channel::Sender<Snapshot>,
    /// The snapshot sent last, for sending again when only the settings or the update changed.
    last: Arc<Mutex<Option<Snapshot>>>,
    /// The setup screen's state while it shows; the first run's, or after Stop or Leave.
    setup: Option<Setup>,
    update: updates::Slot,
    /// `--local-app-data`, which test runs use instead of the game's own.
    local_app_data: Option<PathBuf>,
}

/// Why a session's loop returned.
enum Ended {
    /// The player stopped or left; show setup.
    ByPlayer,
    /// Settings changed the relay or the code; start the session again with them.
    Restart,
    /// The window is gone.
    AppClosing,
}

impl Core {
    async fn new(data_dir: PathBuf, app_log: PathBuf, app_version: String, commands: async_channel::Receiver<Command>, snapshots: async_channel::Sender<Snapshot>) -> Self {
        let settings = Settings::load(&data_dir).unwrap_or_else(|e| {
            log::warn!("{e:#}");
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
        Self { data_dir, app_log, app_version, settings, me, commands, snapshots, last: Default::default(), setup: None, update: Default::default(), local_app_data: None }
    }

    fn show(&self, screen: Screen) -> bool {
        let settings = self.settings.role.map(|role| SettingsView {
            role: Some(role.into()),
            relay: self.settings.relay.clone(),
            code: self.settings.code.clone(),
            logs_folder: self.settings.logs_folder.clone().unwrap_or_else(default_logs_folder),
            extra_paths: self.settings.extra_paths.clone(),
            never_share: self.settings.never_share.clone(),
            app_log: self.app_log.clone(),
        });
        let update = self.update.lock().ok().and_then(|u| u.as_ref().map(|r| r.version.clone()));
        let snapshot = Snapshot { app_version: self.app_version.clone(), screen, update, settings };
        let mut last = self.last.lock().expect("the last snapshot isn't poisoned");
        *last = Some(snapshot.clone());
        self.snapshots.try_send(snapshot).is_ok()
    }

    /// Sends the last screen again, with the settings as they are now.
    fn show_again(&self) {
        let screen = self.last.lock().expect("the last snapshot isn't poisoned").as_ref().map(|s| s.screen.clone());
        if let Some(screen) = screen {
            self.show(screen);
        }
    }

    /// Handles the commands any screen of a session takes: true when the session must start again.
    fn common_command(&mut self, command: Command) -> bool {
        match command {
            Command::ApplyUpdate => self.apply_update(),
            Command::SaveSettings(answers) => {
                if self.save_settings(answers) {
                    return true;
                }
                self.show_again();
            }
            _ => {}
        }
        false
    }

    fn apply_update(&self) {
        let slot = self.update.lock().expect("the update slot isn't poisoned");
        if let Some(ready) = slot.as_ref() {
            ready.apply();
        }
    }

    /// Saves the Settings page's answers; true when the session must start again for them.
    fn save_settings(&mut self, answers: SettingsAnswers) -> bool {
        let logs_folder = |s: &Settings| s.logs_folder.clone().unwrap_or_else(default_logs_folder);
        let before = logs_folder(&self.settings);
        let never_share = self.settings.never_share.clone();
        let mut restart = self.settings.take_answers(answers);
        // The host's Collector writes into one folder for the whole session, and a mods Player
        // takes its patterns when it starts.
        restart |= self.settings.role == Some(SavedRole::Host) && logs_folder(&self.settings) != before;
        restart |= self.settings.never_share != never_share;
        if let Err(e) = self.settings.save(&self.data_dir) {
            log::warn!("{e:#}");
        }
        restart
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
                Ended::Restart => {}
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
                Command::ApplyUpdate => self.apply_update(),
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

    /// This PC's logs, or `None` when the game isn't found: a `Sender` with no game folder would
    /// scan whatever folder the app started in.
    fn new_sender(&self, extra_paths: Vec<String>) -> Option<Sender> {
        let game = self.me.game.as_ref()?;
        Some(Sender::new(SenderConfig {
            game_folder: game.folder.clone(),
            local_app_data: self.local_app_data.clone().or_else(|| game::local_app_data(&game.folder)),
            app_log: Some(self.app_log.clone()),
            extra_paths,
        }))
    }

    /// This PC's mods, or `None` when the game isn't found.
    fn new_mod_player(&self) -> Option<ModPlayer> {
        let game = self.me.game.as_ref()?;
        Some(ModPlayer::new(PlayerConfig {
            game_folder: game.folder.clone(),
            data_dir: self.data_dir.clone(),
            install_id: self.settings.install_id.clone(),
            name: self.me.name.clone(),
            never_share: self.settings.never_share.clone(),
        }))
    }

    /// What the host's `_peer.txt` says about this PC. The game folder stays out of it, since its
    /// path can hold the player's user name.
    fn peer_info(&self) -> PeerInfo {
        let game = self.game_info();
        PeerInfo { game_folder: String::new(), game_version: game.version, system: self.me.system.clone() }
    }

    fn game_info(&self) -> GameInfo {
        match &self.me.game {
            Some(g) => GameInfo { version: g.version.clone().unwrap_or_default(), store: game::store(&g.folder).into() },
            None => GameInfo::default(),
        }
    }

    // ---------- hosting ----------

    /// The folder this session's logs land in: the logs folder, then one per day and code.
    fn session_folder(&self, started: DateTime<Local>) -> PathBuf {
        let logs_folder = self.settings.logs_folder.clone().unwrap_or_else(default_logs_folder);
        logs_folder.join(format!("{} {}", started.format("%Y-%m-%d"), display_code(&self.settings.code)))
    }

    async fn host(&mut self) -> Ended {
        let started = Local::now();
        let folder = self.session_folder(started);
        // The Sender and Collector touch only files, so the core calls them in its own thread:
        // nothing else runs there, and the relay's sockets carry on in the runtime's workers.
        let mut host = Hosting {
            players: BTreeMap::new(),
            by_tester: BTreeMap::new(),
            next_id: 2,
            link: Link::Connecting,
            latency: None,
            problem: None,
            collector: Collector::new(folder.clone()),
            own: self.new_sender(self.settings.extra_paths.clone()),
            own_id: self.settings.install_id.clone(),
            extra_paths: self.settings.extra_paths.clone(),
            folder,
            hub: Hub::new(self.data_dir.join("mod-cache")),
            mods: self.new_mod_player(),
        };
        // The host's own logs go into the session folder like everyone else's, and its own mods
        // through the hub like everyone else's.
        let has = host.collector.welcome(&host.own_id, &self.me.name, self.peer_info());
        if let Some(own) = &mut host.own {
            own.resume(&has);
        }
        host.hub.joined(&host.own_id, &self.me.name);
        if let Some(mods) = &mut host.mods {
            mods.connected(true);
        }
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
        let mut connected_before = false;
        loop {
            if !self.show_hosting(&host) {
                return Ended::AppClosing;
            }
            let resume = (!self.settings.resume.is_empty()).then(|| self.settings.resume.clone());
            let (relay_address, code) = (self.settings.relay.clone(), self.settings.code.clone());
            let opened = tokio::select! {
                opened = HostLink::open(&relay_address, &code, resume.as_deref()) => opened,
                ended = self.wait_for_stop() => return ended,
            };
            let mut link = match opened {
                Ok(link) => link,
                Err(e) => {
                    let message = format!("{e:#}");
                    // Another host holding the code won't change by trying again soon.
                    let held = message.contains("another host");
                    host.problem = Some(if held { "Another host is using this code. Stop, then pick New code.".into() } else { relay_problem(&self.settings.relay, &message) });
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
                    log::warn!("{e:#}");
                }
            }
            host.link = Link::Connected { latency_ms: 0 };
            host.problem = None;
            let _ = link.ping().await;
            // The relay drops what testers send while the host is away, and the host's own sends
            // failed, so every tester still there starts again from what the host has.
            if connected_before {
                send_all(&mut link, host.welcome_again(&self.me.name)).await;
            }
            connected_before = true;

            let mut ping = tokio::time::interval(PING_EVERY);
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            let mut logs = tokio::time::interval(LOGS_EVERY);
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
                                send_all(&mut link, replies).await;
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
                        Ok(command) => {
                            if let Some(command) = mod_command(&mut host.mods, command)
                                && self.common_command(command)
                            {
                                link.stop().await;
                                return Ended::Restart;
                            }
                        }
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
                    _ = logs.tick() => {
                        // Settings changed the extra paths: every player's PC hears them in a new Welcome.
                        if host.extra_paths != self.settings.extra_paths {
                            for (to, welcome) in host.new_extra_paths(self.settings.extra_paths.clone(), &self.me.name) {
                                let _ = link.send_to(&to, &encode(&welcome)).await;
                            }
                        }
                        host.collect();
                        let out = host.share_mods();
                        send_all(&mut link, out).await;
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
            players: host
                .players
                .values()
                .map(|e| {
                    let mut p = player(&e.info, 1, e.left.is_some() || !e.info.connected);
                    p.bytes = host.collector.received_bytes(&e.install_id);
                    p.sources = sources(host.collector.sources(&e.install_id));
                    p
                })
                .collect(),
            shared: host.mods.as_ref().map(|m| m.shared().into_iter().map(shared_mod).collect()).unwrap_or_default(),
            my_mods: host.mods.as_ref().map(|m| m.my_mods().into_iter().map(my_mod).collect()).unwrap_or_default(),
            logs: LogsLine::Host { folder: host.folder.clone() },
            problem: host.problem.clone(),
        }))
    }

    /// Waits for Stop or Leave while the session isn't connected, so the player can give up, or
    /// fix the relay's address in Settings.
    async fn wait_for_stop(&mut self) -> Ended {
        loop {
            match self.commands.recv().await {
                Ok(Command::Stop | Command::Leave) => return Ended::ByPlayer,
                Ok(command) => {
                    if self.common_command(command) {
                        return Ended::Restart;
                    }
                }
                Err(_) => return Ended::AppClosing,
            }
        }
    }

    // ---------- joining ----------

    async fn join(&mut self) -> Ended {
        let mut joined = Joining {
            me: None,
            host_name: String::new(),
            players: Vec::new(),
            link: Link::Connecting,
            problem: None,
            sender: self.new_sender(Vec::new()),
            welcomed: false,
            mods: self.new_mod_player(),
            doing: Doing::Nothing,
        };
        let mut tries = 0usize;
        loop {
            if !self.show_joining(&joined) {
                return Ended::AppClosing;
            }
            let (relay_address, code) = (self.settings.relay.clone(), self.settings.code.clone());
            let opened = tokio::select! {
                opened = TesterLink::open(&relay_address, &code) => opened,
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
                        (Link::Reconnecting, Some(relay_problem(&self.settings.relay, &message)))
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
                    Some(ToTester::Welcome(welcome)) => {
                        joined.welcome(welcome);
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
            let mut logs = tokio::time::interval(LOGS_EVERY);
            loop {
                joined.link = Link::Connected { latency_ms: link.latency_ms().unwrap_or(0) };
                if !self.show_joining(&joined) {
                    return Ended::AppClosing;
                }
                tokio::select! {
                    event = link.next() => match event {
                        TesterEvent::Message(data) => match decode::<ToTester>(&data) {
                            Some(ToTester::Players(players)) => joined.players = players,
                            // The host sends another when its extra paths change, or its socket came back.
                            Some(ToTester::Welcome(welcome)) => joined.welcome(welcome),
                            Some(ToTester::Mods(message)) => {
                                let answers = joined.mods.as_mut().map(|m| m.on_message(&message)).unwrap_or_default();
                                for answer in answers {
                                    if link.send(&encode(&ToHost::Mods(answer))).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            _ => {}
                        },
                        TesterEvent::Ended(_) => {
                            joined.link = Link::WaitingForHost;
                            joined.players.clear();
                            joined.dropped();
                            break;
                        }
                        TesterEvent::Dropped => {
                            joined.link = Link::Reconnecting;
                            joined.dropped();
                            break;
                        }
                    },
                    command = self.commands.recv() => match command {
                        Ok(Command::Leave) => {
                            link.leave().await;
                            return Ended::ByPlayer;
                        }
                        Ok(command) => {
                            if let Some(command) = mod_command(&mut joined.mods, command)
                                && self.common_command(command)
                            {
                                link.leave().await;
                                return Ended::Restart;
                            }
                        }
                        Err(_) => {
                            link.leave().await;
                            return Ended::AppClosing;
                        }
                    },
                    _ = ping.tick() => { let _ = link.ping().await; }
                    _ = logs.tick() => {
                        // A message lost to a drop is sent again after the next Welcome, from what
                        // the host has, so a failed send needs nothing more here.
                        let mut out: Vec<ToHost> = joined.sender.as_mut().map(Sender::tick).unwrap_or_default().into_iter().map(ToHost::Logs).collect();
                        if let Some(mods) = &mut joined.mods {
                            out.extend(mods.tick().into_iter().map(ToHost::Mods));
                            // The host shows what this PC's mods are doing on its row.
                            let doing = mods.doing();
                            if doing != joined.doing {
                                joined.doing = doing.clone();
                                out.push(ToHost::Doing(doing));
                            }
                        }
                        for message in out {
                            if link.send(&encode(&message)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    fn show_joining(&self, joined: &Joining) -> bool {
        let me = joined.me.unwrap_or(0);
        let mut players: Vec<Player> = joined.players.iter().map(|p| player(p, me, !p.connected)).collect();
        let host = if joined.host_name.is_empty() { "the host".to_string() } else { joined.host_name.clone() };
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
        if let (Some(sender), Some(row)) = (&joined.sender, players.iter_mut().find(|p| p.is_me)) {
            row.bytes = sender.sent_bytes();
            row.sources = sources(sender.sources());
        }
        let sending = match &joined.sender {
            Some(sender) if joined.welcomed && matches!(joined.link, Link::Connected { .. }) => match sender.progress() {
                Progress::CatchingUp { done, total } => Sending::CatchingUp { done, total },
                Progress::UpToDate => Sending::UpToDate,
            },
            Some(_) if joined.welcomed => Sending::Paused,
            _ => Sending::Idle,
        };
        let problem = joined.problem.clone().or_else(|| {
            joined.sender.is_none().then(|| format!("The game isn't found on this PC, so no logs go to {host}. Leave, then pick its folder."))
        });
        self.show(Screen::Session(Session {
            role: Role::Tester,
            code: self.settings.code.clone(),
            relay: self.settings.relay.clone(),
            link: joined.link.clone(),
            host_name: host.clone(),
            players,
            shared: joined.mods.as_ref().map(|m| m.shared().into_iter().map(shared_mod).collect()).unwrap_or_default(),
            my_mods: joined.mods.as_ref().map(|m| m.my_mods().into_iter().map(my_mod).collect()).unwrap_or_default(),
            logs: LogsLine::Tester { host, sending },
            problem,
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
    collector: Collector,
    /// The host's own logs, or `None` when this PC has no game.
    own: Option<Sender>,
    own_id: String,
    /// The extra paths every player's PC has heard in its latest Welcome.
    extra_paths: Vec<String>,
    folder: PathBuf,
    /// Every shared file goes through the hub on the host's PC.
    hub: Hub,
    /// The host's own mods, or `None` when this PC has no game.
    mods: Option<ModPlayer>,
}

impl Hosting {
    /// The relay's ID for the player with this install ID, while their socket is open.
    fn tester_of(&self, install_id: &str) -> Option<TesterId> {
        self.players.values().find(|e| e.install_id == install_id).and_then(|e| e.tester)
    }

    /// Gives the hub and the host's own mods a turn, and returns what goes to the testers.
    fn share_mods(&mut self) -> Vec<(Option<TesterId>, ToTester)> {
        let mut from_hub = self.hub.tick();
        if let Some(mods) = &mut self.mods {
            for message in mods.tick() {
                from_hub.extend(self.hub.on_message(&self.own_id, &message));
            }
        }
        let mut out = self.route(from_hub);
        // The host's row shows what its own mods are doing, as a tester's does.
        let doing = self.mods.as_ref().map(ModPlayer::doing).unwrap_or(Doing::Nothing);
        if let Some(entry) = self.players.get_mut(&1)
            && entry.info.doing != doing
        {
            entry.info.doing = doing;
            out.push((None, ToTester::Players(self.list())));
        }
        out
    }

    /// Delivers the hub's messages: the host's own Player's in-process, and its answers back to
    /// the hub, until none are left; the testers' to send.
    fn route(&mut self, from_hub: Vec<(Target, Vec<u8>)>) -> Vec<(Option<TesterId>, ToTester)> {
        let mut queue = std::collections::VecDeque::from(from_hub);
        let mut out = Vec::new();
        while let Some((target, message)) = queue.pop_front() {
            let to_me = match &target {
                Target::Everyone => {
                    out.push((None, ToTester::Mods(message.clone())));
                    true
                }
                Target::Player(id) if *id == self.own_id => true,
                Target::Player(id) => {
                    if let Some(tester) = self.tester_of(id) {
                        out.push((Some(tester), ToTester::Mods(message.clone())));
                    }
                    false
                }
            };
            if to_me && let Some(mods) = &mut self.mods {
                for answer in mods.on_message(&message) {
                    queue.extend(self.hub.on_message(&self.own_id, &answer));
                }
            }
        }
        out
    }

    /// A Welcome again for every tester still connected, after the host's socket came back: their
    /// logs go on from what the host has, and their mods start over with the hub.
    fn welcome_again(&mut self, host_name: &str) -> Vec<(Option<TesterId>, ToTester)> {
        let mut out = Vec::new();
        for entry in self.players.values() {
            let Some(tester) = entry.tester else { continue };
            let peer = PeerInfo { game_folder: String::new(), game_version: entry.info.game.version.clone(), system: entry.info.system.clone() };
            let logs = self.collector.welcome(&entry.install_id, &entry.info.name, peer);
            self.hub.left(&entry.install_id);
            self.hub.joined(&entry.install_id, &entry.info.name);
            out.push((Some(tester), ToTester::Welcome(Welcome { host_name: host_name.into(), player_id: entry.info.id, extra_paths: self.extra_paths.clone(), logs })));
        }
        out
    }
    /// Gives the host's own logs a turn, straight into the Collector, and the Collector its own.
    fn collect(&mut self) {
        if let Some(own) = &mut self.own {
            for message in own.tick() {
                self.collector.on_message(&self.own_id, &message);
            }
        }
        self.collector.tick();
    }

    /// Takes new extra paths, and returns a Welcome with them for every connected tester. Each
    /// Welcome starts a new generation of logs, which the testers' Senders go on from.
    fn new_extra_paths(&mut self, extra_paths: Vec<String>, host_name: &str) -> Vec<(TesterId, ToTester)> {
        self.extra_paths = extra_paths;
        if let Some(own) = &mut self.own {
            own.set_extra_paths(self.extra_paths.clone());
        }
        let mut welcomes = Vec::new();
        for entry in self.players.values() {
            let Some(tester) = entry.tester else { continue };
            let peer = PeerInfo { game_folder: String::new(), game_version: entry.info.game.version.clone(), system: entry.info.system.clone() };
            let logs = self.collector.welcome(&entry.install_id, &entry.info.name, peer);
            welcomes.push((tester, ToTester::Welcome(Welcome { host_name: host_name.into(), player_id: entry.info.id, extra_paths: self.extra_paths.clone(), logs })));
        }
        welcomes
    }
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
                // The tester's PC goes on from what the host already has of its logs.
                let peer = PeerInfo { game_folder: String::new(), game_version: entry.info.game.version.clone(), system: entry.info.system.clone() };
                let logs = self.collector.welcome(&hello.install_id, &entry.info.name, peer);
                // A tester who comes back on a new socket left the hub on the old one's close, or
                // is still in it when the relay hasn't said yet.
                self.hub.left(&hello.install_id);
                self.hub.joined(&hello.install_id, &entry.info.name);
                vec![
                    (Some(from), ToTester::Welcome(Welcome { host_name: host_name.into(), player_id: id, extra_paths: extra_paths.to_vec(), logs })),
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
            ToHost::Logs(message) => {
                if let Some(entry) = self.by_tester.get(&from).and_then(|id| self.players.get(id)) {
                    // The host never answers a Sender, so there's nothing to send back.
                    self.collector.on_message(&entry.install_id, &message);
                }
                Vec::new()
            }
            ToHost::Mods(message) => {
                let Some(install_id) = self.by_tester.get(&from).and_then(|id| self.players.get(id)).map(|e| e.install_id.clone()) else {
                    return Vec::new();
                };
                let from_hub = self.hub.on_message(&install_id, &message);
                self.route(from_hub)
            }
        }
    }

    /// A tester's socket closed; true when the list changed.
    fn left(&mut self, tester: TesterId) -> bool {
        let Some(id) = self.by_tester.remove(&tester) else { return false };
        if let Some(entry) = self.players.get_mut(&id) {
            entry.tester = None;
            entry.left = Some(Instant::now());
            self.collector.left(&entry.install_id);
            self.hub.left(&entry.install_id);
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
    /// This PC's logs, or `None` when it has no game.
    sender: Option<Sender>,
    /// A Welcome came, so the Sender has somewhere to send.
    welcomed: bool,
    /// This PC's mods, or `None` when it has no game.
    mods: Option<ModPlayer>,
    /// What the host last heard this PC's mods were doing.
    doing: Doing,
}

impl Joining {
    fn welcome(&mut self, welcome: Welcome) {
        self.host_name = welcome.host_name;
        self.me = Some(welcome.player_id);
        if let Some(sender) = &mut self.sender {
            sender.set_extra_paths(welcome.extra_paths);
            sender.resume(&welcome.logs);
        }
        if let Some(mods) = &mut self.mods {
            // A second Welcome means the host started this PC over with the hub, so the Player
            // does too.
            if self.welcomed {
                mods.connected(false);
            }
            mods.connected(true);
        }
        self.welcomed = true;
        // The host knows nothing yet of what this PC's mods are doing.
        self.doing = Doing::Nothing;
    }

    /// The link to the host closed.
    fn dropped(&mut self) {
        if let Some(mods) = &mut self.mods {
            mods.connected(false);
        }
    }
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
                Doing::Held => PlayerState::Held,
            }
        },
        sources: None,
    }
}

/// Sends the host's messages, `None` meaning every tester, and stops at the first that fails,
/// since the socket is gone and the loop finds out next.
async fn send_all(link: &mut HostLink, out: Vec<(Option<TesterId>, ToTester)>) {
    for (to, message) in out {
        let sent = match to {
            Some(tester) => link.send_to(&tester, &encode(&message)).await,
            None => link.send_to_everyone(&encode(&message)).await,
        };
        if sent.is_err() {
            break;
        }
    }
}

/// Takes the window's commands for this PC's mods, and hands back any other.
fn mod_command(mods: &mut Option<ModPlayer>, command: Command) -> Option<Command> {
    let Some(mods) = mods else { return Some(command) };
    match command {
        Command::Share { key, on } => mods.share(&key, on),
        Command::Follow { key, on: true } => mods.follow(&key),
        Command::Follow { key, on: false } => {
            if let Err(e) = mods.unfollow(&key, false) {
                log::warn!("couldn't stop following {key}: {e}");
            }
        }
        Command::Unfollow { key, delete_copy } => {
            if let Err(e) = mods.unfollow(&key, delete_copy) {
                log::warn!("couldn't stop following {key}: {e}");
            }
        }
        Command::TakeAddedFolder { key } => mods.take_added_folder(&key),
        other => return Some(other),
    }
    None
}

fn place(p: coop_mods::Place) -> Place {
    match p {
        coop_mods::Place::Cet => Place::Cet,
        coop_mods::Place::Red4ext => Place::Red4ext,
        coop_mods::Place::Redscript => Place::Redscript,
        coop_mods::Place::TweakXl => Place::TweakXl,
        coop_mods::Place::InputLoader => Place::InputLoader,
        coop_mods::Place::Archive => Place::Archive,
        coop_mods::Place::Redmod => Place::Redmod,
    }
}

fn my_mod(m: coop_mods::MyMod) -> MyMod {
    MyMod { key: m.key, name: m.name, places: m.places.into_iter().map(place).collect(), bytes: m.bytes, shared: m.shared, loose_file: m.loose_file }
}

fn delivery(d: coop_mods::Delivery) -> Delivery {
    match d {
        coop_mods::Delivery::UpToDate => Delivery::UpToDate,
        coop_mods::Delivery::Fetching { done, total } => Delivery::Fetching { done, total },
        coop_mods::Delivery::NeedsRestart => Delivery::NeedsRestart,
        coop_mods::Delivery::Held => Delivery::Held,
        coop_mods::Delivery::Paused => Delivery::Paused,
    }
}

fn shared_mod(m: coop_mods::SharedMod) -> SharedMod {
    use coop_mods::ModStatus as S;
    SharedMod {
        key: m.key,
        name: m.name,
        sharer: m.sharer,
        mine: m.mine,
        follow: match m.follow {
            coop_mods::Follow::No => Follow::No,
            coop_mods::Follow::Yes => Follow::Yes,
            coop_mods::Follow::AddedFolder => Follow::AddedFolder,
        },
        has_dll: m.has_dll,
        status: match m.status {
            S::UpToDate => ModStatus::UpToDate,
            S::Fetching { done, total } => ModStatus::Fetching { done, total },
            S::NeedsRestart => ModStatus::NeedsRestart,
            S::Held => ModStatus::Held,
            S::Checking => ModStatus::Checking,
            S::NotOnYourPc => ModStatus::NotOnYourPc,
            S::OnYourPc { same } => ModStatus::OnYourPc { same },
            S::AddedFolder => ModStatus::AddedFolder,
            S::Paused => ModStatus::Paused,
            S::FollowedElsewhere { by } => ModStatus::FollowedElsewhere { by },
            S::NotShared => ModStatus::NotShared,
            S::OfferedToo => ModStatus::OfferedToo,
            S::Sharing => ModStatus::Sharing,
        },
        changed: (m.changed > 0).then(|| Local.timestamp_opt(m.changed, 0).single()).flatten(),
        followers: m.followers.into_iter().map(|f| Follower { name: f.name, delivery: delivery(f.delivery) }).collect(),
    }
}

/// Why the relay can't be reached, in words for the player, from the error under it, which the app's
/// log keeps in full.
fn relay_problem(relay: &str, error: &str) -> String {
    log::info!("can't reach the relay: {error}");
    let e = error.to_lowercase();
    let says = |needles: &[&str]| needles.iter().any(|n| e.contains(n));
    if says(&["no such host", "11001", "name or service not known", "failed to lookup", "nodename nor servname", "dns"]) {
        format!("Can't find {relay}. Check the relay's address in Settings, or your connection.")
    } else if says(&["timed out", "timeout", "10060"]) {
        format!("{relay} isn't answering. Check your connection; the app keeps trying.")
    } else if says(&["refused", "10061"]) {
        format!("{relay} refused the connection. Check the relay's address in Settings.")
    } else if says(&["certificate", "tls", "ssl"]) {
        format!("Can't connect securely to {relay}. Check the relay's address in Settings.")
    } else if says(&["limiting attempts"]) {
        "The relay is limiting attempts from this network. The app tries again in a minute.".into()
    } else if says(&["malformed"]) {
        "The relay says the code is malformed. Check the code in Settings.".into()
    } else {
        format!("Can't reach {relay}. The app keeps trying; the app's log has the details.")
    }
}

/// The window's list of where a player's logs come from, or `None` before their PC has said.
fn sources(counts: Vec<SourceCount>) -> Option<Vec<LogSource>> {
    if counts.is_empty() {
        return None;
    }
    let kind = |k: coop_logs::SourceKind| match k {
        coop_logs::SourceKind::GameFolder => SourceKind::GameFolder,
        coop_logs::SourceKind::CrashReports => SourceKind::CrashReports,
        coop_logs::SourceKind::GameOwn => SourceKind::GameOwn,
        coop_logs::SourceKind::AppLog => SourceKind::AppLog,
        coop_logs::SourceKind::Extra(pattern) => SourceKind::Extra { pattern },
    };
    Some(counts.into_iter().map(|c| LogSource { kind: kind(c.kind), count: c.count }).collect())
}

/// `K7RM2Q` reads `K7R-M2Q`.
fn display_code(code: &str) -> String {
    if code.len() == 6 { format!("{}-{}", &code[..3], &code[3..]) } else { code.to_string() }
}
