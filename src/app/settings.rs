//! The Settings page, from the title bar's sliders: the relay and code, and for the host the logs
//! folder and the extra paths every player's PC picks up (record/areas/interface/decisions.md).

use std::path::PathBuf;

use coop_core::state::{Command, Role, SettingsAnswers, SettingsView, Snapshot};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::*;
use crate::ui::*;
use crate::view::AppView;

/// The page's answers while it's open; they reach the core only on Save.
pub struct SettingsForm {
    pub relay: Entity<InputState>,
    pub code: Entity<InputState>,
    pub extra_paths: Entity<TextareaState>,
    pub never_share: Entity<TextareaState>,
    pub logs_folder: PathBuf,
    pub error: Option<String>,
}

impl SettingsForm {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            relay: cx.new(|cx| InputState::new(window, cx).placeholder("relay.example.net")),
            code: cx.new(|cx| InputState::new(window, cx).placeholder("K7R-M2Q")),
            extra_paths: cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 8).placeholder("bin/x64/StreetVendors/*.csv")),
            never_share: cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 8).placeholder("*.psd")),
            logs_folder: PathBuf::new(),
            error: None,
        }
    }
}

impl AppView {
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.snapshot.as_ref().and_then(|s| s.settings.clone()) else { return };
        let form = &mut self.settings;
        form.relay.update(cx, |s, cx| s.set_value(view.relay.clone(), window, cx));
        form.code.update(cx, |s, cx| s.set_value(code(&view.code), window, cx));
        form.extra_paths.update(cx, |s, cx| s.set_value(view.extra_paths.join("\n"), window, cx));
        form.never_share.update(cx, |s, cx| s.set_value(view.never_share.join("\n"), window, cx));
        form.logs_folder = view.logs_folder.clone();
        form.error = None;
        self.settings_open = true;
        cx.notify();
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let form = &mut self.settings;
        let relay = form.relay.read(cx).value().trim().to_string();
        let typed_code = form.code.read(cx).value().to_string();
        if relay.is_empty() {
            form.error = Some("Enter the relay's address, such as relay.example.net".into());
            cx.notify();
            return;
        }
        let Some(code) = coop_core::relay::parse_code(&typed_code) else {
            form.error = Some("A code is 6 letters and digits, such as K7R-M2Q".into());
            cx.notify();
            return;
        };
        let lines = |text: SharedString| text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect::<Vec<_>>();
        let answers = SettingsAnswers {
            relay,
            code,
            logs_folder: form.logs_folder.clone(),
            extra_paths: lines(form.extra_paths.read(cx).value()),
            never_share: lines(form.never_share.read(cx).value()),
        };
        self.send(Command::SaveSettings(answers));
        self.settings_open = false;
        cx.notify();
    }

    fn pick_settings_logs_folder(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = picked.await
                && let Some(path) = paths.pop()
            {
                let _ = this.update(cx, |view, cx| {
                    view.settings.logs_folder = path;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn render_settings(&mut self, snapshot: &Snapshot, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let host = view.role == Some(Role::Host);
        let form = &self.settings;

        let field = |label_text: &'static str, control: AnyElement, helper: AnyElement| {
            v_flex().gap(px(6.)).child(label(label_text)).child(control).child(helper)
        };
        let helper = |text: String| div().text_size(SMALL_SIZE).text_color(color(MUTED)).child(text).into_any_element();

        let code_row = h_flex()
            .gap(px(8.))
            .child(div().w(px(120.)).child(Input::new(&form.code)))
            .when(host, |r| {
                r.child(outline_button("new-code", "New code").on_click(cx.listener(|view, _, window, cx| {
                    let fresh = code(&coop_core::relay::new_code());
                    view.settings.code.update(cx, |s, cx| s.set_value(fresh, window, cx));
                })))
            })
            .into_any_element();

        let logs_row = h_flex()
            .gap(px(8.))
            .child(mono(cx, form.logs_folder.display().to_string()).flex_1().min_w_0().truncate())
            .child(outline_button("change-logs", "Change").on_click(cx.listener(|view, _, _, cx| view.pick_settings_logs_folder(cx))))
            .into_any_element();

        let app_log = view.app_log.clone();
        let app_row = h_flex()
            .gap(px(8.))
            .child(mono(cx, view.app_log.display().to_string()).flex_1().min_w_0().truncate())
            .child(outline_button("show-log", "Show").on_click(move |_, _, cx| cx.reveal_path(&app_log)))
            .into_any_element();

        let version_row = h_flex()
            .gap(px(12.))
            .child(div().child(format!("Version {}", snapshot.app_version)))
            .map(|r| match &snapshot.update {
                Some(next) => r
                    .child(div().text_color(color(MUTED)).child(format!("Version {next} is downloaded")))
                    .child(accent_button("apply-update", "Restart to update").on_click(cx.listener(|view, _, _, _| view.send(Command::ApplyUpdate)))),
                None => r,
            })
            .into_any_element();

        let body = v_flex()
            .gap(px(16.))
            .child(field(
                "Relay",
                Input::new(&form.relay).into_any_element(),
                helper(if host { "Your relay. Testers take it from the invite".into() } else { "The host's relay".into() }),
            ))
            .child(field(
                if host { "Your code" } else { "Code" },
                code_row,
                helper(if host { "A new code means testers type it once more".into() } else { "The host's code".into() }),
            ))
            .when(host, |b| {
                b.child(field("Logs go to", logs_row, helper("A folder per session, a folder per player inside it".into())))
                    .child(field(
                        "Extra files to pick up",
                        Textarea::new(&form.extra_paths).into_any_element(),
                        helper("On every player's PC, relative to the game folder, one a line. * stands for any name.".into()),
                    ))
            })
            .child(field(
                "Never send from your shared mods",
                Textarea::new(&form.never_share).into_any_element(),
                helper("Files in the mods you share that stay on your PC, one pattern a line".into()),
            ))
            .child(field("App", version_row, helper("Updates download by themselves, and install when you restart".into())))
            .child(field("App log", app_row, helper("Send this along when the app itself misbehaves".into())))
            .child(div().h(px(32.)));

        // Save and Cancel sit in the title bar, where they show however far the page scrolls.
        let title_bar = TitleBar::new()
            .h(TITLE_BAR)
            .bg(color(PANEL))
            .child(div().text_color(color(MUTED)).child("Settings"))
            .child(
                h_flex()
                    .pr(px(8.))
                    .gap(px(6.))
                    .child(plain_button("cancel-settings", "Cancel", None).on_click(cx.listener(|view, _, _, cx| {
                        view.settings_open = false;
                        cx.notify();
                    })))
                    .child(accent_button("save-settings", "Save").h(px(24.)).on_click(cx.listener(|view, _, _, cx| view.save_settings(cx)))),
            );

        v_flex()
            .size_full()
            .child(title_bar)
            .when_some(form.error.clone(), |v, error| {
                v.child(
                    h_flex()
                        .h(SECTION)
                        .flex_shrink_0()
                        .px(px(12.))
                        .gap(px(8.))
                        .bg(color(PANEL))
                        .border_b_1()
                        .border_color(color(LINE))
                        .text_size(SMALL_SIZE)
                        .text_color(color(TROUBLE))
                        .child(icon(gpui_kit::assets::IconName::CircleAlert, TROUBLE))
                        .child(error),
                )
            })
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().flex().justify_center().child(v_flex().w(px(512.)).pt(px(32.)).child(body))),
            )
            .into_any_element()
    }
}
