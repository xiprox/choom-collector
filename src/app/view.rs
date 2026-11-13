//! The window's one view: it keeps the newest snapshot from the core, and the few things only the
//! window knows, such as which player is selected.

use coop_core::state::{Command, PlayerId, Role, Screen, Snapshot};
use gpui_kit::component::input::InputState;
use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

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
    pub my_mods_open: bool,
    pub asking: Option<Asking>,
    pub setup_role: Option<Role>,
    pub invite: Entity<InputState>,
    pub relay: Entity<InputState>,
    setup_filled: bool,
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
            my_mods_open: false,
            asking: None,
            setup_role: None,
            invite,
            relay,
            setup_filled: false,
            _snapshots: task,
        }
    }

    pub fn send(&self, command: Command) {
        // The core only stops with the app, so a failed send has nobody left to tell.
        let _ = self.commands.try_send(command);
    }

    fn receive(&mut self, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        match &snapshot.screen {
            Screen::Setup(setup) => {
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
                if !still_there {
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
        let screen = self.snapshot.as_ref().map(|s| (s.screen.clone(), s.app_version.clone()));
        v_flex()
            .size_full()
            .bg(color(BASE))
            .text_color(color(TEXT))
            .text_size(TEXT_SIZE)
            .line_height(LINE_HEIGHT)
            .map(|v| match screen {
                None => v,
                Some((Screen::Setup(setup), _)) => v.child(self.render_setup(&setup, window, cx)),
                Some((Screen::Session(session), version)) => v.child(self.render_session(&session, &version, window, cx)),
            })
    }
}
