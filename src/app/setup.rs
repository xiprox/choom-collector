//! The first run's setup: Join or Host, asked once (record/areas/interface/decisions.md).

use coop_core::state::{Command, Role, Setup, SetupAnswers};
use gpui_kit::component::input::Input;
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::*;
use crate::ui::*;
use crate::view::AppView;

impl AppView {
    pub fn render_setup(&mut self, setup: &Setup, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let role = self.setup_role.unwrap_or(setup.role);
        let invite = self.invite.read(cx).value().to_string();
        let filled_from_invite = invite.contains('/');
        let code_in_invite = invite.rsplit('/').next().unwrap_or_default().trim().to_uppercase();

        let tab = |id: &'static str, text: &'static str, this: Role| {
            div()
                .id(id)
                .flex()
                .items_center()
                .h_full()
                .px(px(14.))
                .cursor_pointer()
                .map(|t| if role == this { t.bg(color(RAISED)).text_color(color(STRONG)) } else { t.text_color(color(MUTED)) })
                .child(text)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.setup_role = Some(this);
                    cx.notify();
                }))
        };

        let game = setup.game.as_ref();
        let game_line = format!(
            "{}You'll show up as {}, your Steam name.",
            match game.and_then(|g| g.version.as_deref()) {
                Some(v) => format!("Found on its own, version {v}. "),
                None => String::new(),
            },
            setup.steam_name
        );

        let field = |label_text: &'static str, control: AnyElement, helper: AnyElement| {
            v_flex().gap(px(6.)).child(label(label_text)).child(control).child(helper)
        };
        let helper = |text: String| div().text_size(SMALL_SIZE).text_color(color(MUTED)).child(text).into_any_element();
        let change_game = outline_button("change-game", "Change").on_click(cx.listener(|view, _, _, cx| view.pick_folder(true, cx)));
        let change_logs = outline_button("change-logs", "Change").on_click(cx.listener(|view, _, _, cx| view.pick_folder(false, cx)));
        let path_row = |path: String, change: Stateful<Div>| {
            h_flex()
                .gap(px(8.))
                .child(mono(cx, path).flex_1().min_w_0().truncate())
                .child(change)
                .into_any_element()
        };

        let body = match role {
            Role::Tester => v_flex()
                .gap(px(16.))
                .child(field("Invite", Input::new(&self.invite).into_any_element(), helper("The code, or the relay and the code together".into())))
                .child(field(
                    "Relay",
                    Input::new(&self.relay).into_any_element(),
                    helper(if filled_from_invite { "Filled in from the invite".into() } else { "The host's relay, set once".into() }),
                ))
                .child(field(
                    "Game",
                    path_row(game.map(|g| g.folder.display().to_string()).unwrap_or_else(|| "Not found".into()), change_game),
                    helper(game_line),
                ))
                .child(
                    h_flex()
                        .mt(px(8.))
                        .gap(px(16.))
                        .child(accent_button("join", "Join").on_click(cx.listener(|view, _, _, cx| view.finish_setup(Role::Tester, cx))))
                        .child(helper(format!(
                            "From then on, the app joins {} by itself when it starts",
                            if code_in_invite.is_empty() { "the session".to_string() } else { code(&code_in_invite.replace('-', "")) }
                        ))),
                ),
            Role::Host => v_flex()
                .gap(px(16.))
                .child(field(
                    "Relay",
                    Input::new(&self.relay).into_any_element(),
                    div()
                        .flex()
                        .gap(px(4.))
                        .text_size(SMALL_SIZE)
                        .text_color(color(MUTED))
                        .child("Your own relay, free on Cloudflare.")
                        .child(div().id("relay-guide").text_color(color(ACCENT)).cursor_pointer().child("How to set one up"))
                        .into_any_element(),
                ))
                .child(field(
                    "Your code",
                    h_flex()
                        .gap(px(8.))
                        .child(mono(cx, code(&setup.host_code)).text_size(TEXT_SIZE).font_weight(FontWeight::MEDIUM).text_color(color(STRONG)))
                        .child(outline_button("new-code", "New code").on_click(cx.listener(|view, _, _, _| view.send(Command::NewCode))))
                        .into_any_element(),
                    helper("It stays yours, so testers type it only once".into()),
                ))
                .child(field(
                    "Logs go to",
                    path_row(setup.logs_folder.display().to_string(), change_logs),
                    helper("A folder per session, a folder per player inside it".into()),
                ))
                .child(
                    h_flex()
                        .mt(px(8.))
                        .gap(px(16.))
                        .child(accent_button("host", "Host").on_click(cx.listener(|view, _, _, cx| view.finish_setup(Role::Host, cx))))
                        .child(helper(format!("From then on, the app hosts {} by itself when it starts", code(&setup.host_code)))),
                ),
        };

        v_flex()
            .size_full()
            .child(
                TitleBar::new()
                    .h(TITLE_BAR)
                    .bg(color(PANEL))
                    .child(div().text_color(color(MUTED)).child("Setup")),
            )
            .child(
                div().flex_1().flex().justify_center().child(
                    v_flex()
                        .w(px(512.))
                        .pt(px(48.))
                        .gap(px(16.))
                        .child(
                            h_flex()
                                .gap(px(12.))
                                .pb(px(4.))
                                .child(mark(px(28.)))
                                .child(
                                    v_flex()
                                        .gap(px(2.))
                                        .child(div().text_size(TITLE_SIZE).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).text_color(color(STRONG)).child("Cyberpunk Co-op Tools"))
                                        .child(div().text_size(SMALL_SIZE).text_color(color(MUTED)).child("The host's mod builds go out to everyone, and everyone's logs go back to the host")),
                                ),
                        )
                        .child(
                            h_flex()
                                .self_start()
                                .h(px(24.))
                                .border_1()
                                .border_color(color(LINE))
                                .rounded(RADIUS)
                                .overflow_hidden()
                                .child(tab("tab-join", "Join", Role::Tester))
                                .child(div().w(px(1.)).h_full().bg(color(LINE)))
                                .child(tab("tab-host", "Host", Role::Host)),
                        )
                        .child(body)
                        .when_some(setup.error.clone(), |v, error| v.child(div().text_color(color(TROUBLE)).child(error))),
                ),
            )
            .into_any_element()
    }

    /// Asks the system for a folder, the game's or the logs', and tells the core.
    fn pick_folder(&mut self, game: bool, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
        let commands = self.commands();
        cx.spawn(async move |_, _| {
            if let Ok(Ok(Some(mut paths))) = picked.await
                && let Some(path) = paths.pop()
            {
                let _ = commands.send(if game { Command::SetGameFolder(path) } else { Command::SetLogsFolder(path) }).await;
            }
        })
        .detach();
    }

    fn finish_setup(&mut self, role: Role, cx: &mut Context<Self>) {
        let Some(coop_core::state::Screen::Setup(setup)) = self.snapshot.as_ref().map(|s| &s.screen) else { return };
        let answers = SetupAnswers {
            role,
            invite: self.invite.read(cx).value().trim().to_string(),
            relay: self.relay.read(cx).value().trim().to_string(),
            logs_folder: setup.logs_folder.clone(),
            game_folder: setup.game.as_ref().map(|g| g.folder.clone()),
        };
        self.send(Command::FinishSetup(answers));
    }
}
