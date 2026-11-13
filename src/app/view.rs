//! The window's one view: it keeps the newest snapshot from the core, and the few things only the
//! window knows, such as which player is selected.

use coop_core::state::{Command, PlayerId, Role, Screen, Snapshot};
use gpui_kit::component::input::InputState;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::settings::SettingsForm;
use crate::theme::*;

/// A question the window is asking about one shared mod.
#[derive(Clone, PartialEq, Eq)]
pub enum Asking {
    Follow(String),
    Unfollow(String),
}

pub struct AppView {
    commands: async_channel::Sender<Command>,
    pub snapshot: Option<Snapshot>,
    pub selected: Option<PlayerId>,
    /// The player clicked a row, so the selection stays where they put it.
    pub picked: bool,
    pub my_mods_open: bool,
    pub asking: Option<Asking>,
    pub setup_role: Option<Role>,
    pub invite: Entity<InputState>,
    pub relay: Entity<InputState>,
    setup_filled: bool,
    pub settings_open: bool,
    pub settings: SettingsForm,
    /// The invite was just copied, so its button shows a check for a moment.
    pub copied_invite: bool,
    _snapshots: Task<()>,
}

impl AppView {
    pub fn new(
        commands: async_channel::Sender<Command>,
        snapshots: async_channel::Receiver<Snapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let invite = cx.new(|cx| InputState::new(window, cx).placeholder("K7R-M2Q, or relay.example.net/K7R-M2Q"));
        let relay = cx.new(|cx| InputState::new(window, cx).placeholder("relay.example.net"));
        let settings = SettingsForm::new(window, cx);
        let task = cx.spawn_in(window, async move |this, cx| {
            while let Ok(snapshot) = snapshots.recv().await {
                let alive = this.update_in(cx, |this, window, cx| this.receive(snapshot, window, cx));
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            commands,
            snapshot: None,
            selected: None,
            picked: false,
            my_mods_open: false,
            asking: None,
            setup_role: None,
            invite,
            relay,
            setup_filled: false,
            settings_open: false,
            settings,
            copied_invite: false,
            _snapshots: task,
        }
    }

    pub fn commands(&self) -> async_channel::Sender<Command> {
        self.commands.clone()
    }

    /// Shows the copy button's check for 1.5s.
    pub fn flash_copied(&mut self, cx: &mut Context<Self>) {
        self.copied_invite = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            let _ = this.update(cx, |view, cx| {
                view.copied_invite = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn send(&self, command: Command) {
        // The core only stops with the app, so a failed send has nobody left to tell.
        let _ = self.commands.try_send(command);
    }

    fn receive(&mut self, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        match &snapshot.screen {
            Screen::Setup(setup) => {
                self.settings_open = false;
                if !self.setup_filled {
                    self.setup_filled = true;
                    self.setup_role = Some(setup.role);
                    let (invite, relay) = (setup.invite.clone(), setup.relay.clone());
                    self.invite.update(cx, |s, cx| s.set_value(invite, window, cx));
                    self.relay.update(cx, |s, cx| s.set_value(relay, window, cx));
                }
            }
            Screen::Session(session) => {
                self.setup_filled = false;
                let still_there = self.selected.is_some_and(|id| session.players.iter().any(|p| p.id == id));
                // A host alone in the session sees their own details until the first tester joins,
                // unless they picked their own row themselves.
                let host_waiting = session.role == Role::Host && !self.picked && session.me().map(|p| p.id) == self.selected;
                if !still_there || host_waiting {
                    // The host starts on the first tester, whose logs it receives; a tester on
                    // themselves, whose logs are the only ones they see.
                    self.selected = match session.role {
                        Role::Host => session.players.iter().find(|p| !p.is_me).or(session.me()).map(|p| p.id),
                        Role::Tester => session.me().map(|p| p.id),
                    };
                }
            }
        }
        self.snapshot = Some(snapshot);
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.snapshot.clone();
        v_flex()
            .size_full()
            .bg(color(BASE))
            .font_family(FONT)
            .text_color(color(TEXT))
            .text_size(TEXT_SIZE)
            .line_height(LINE_HEIGHT)
            .map(|v| match &snapshot {
                None => v,
                Some(Snapshot { screen: Screen::Setup(setup), .. }) => v.child(self.render_setup(setup, window, cx)),
                Some(s @ Snapshot { settings: Some(settings), .. }) if self.settings_open => v.child(self.render_settings(s, settings, cx)),
                Some(Snapshot { screen: Screen::Session(session), app_version, update, .. }) => {
                    v.child(self.render_session(session, app_version, update.as_deref(), window, cx))
                }
            })
    }
}
