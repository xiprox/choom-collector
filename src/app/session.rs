//! The window during a session: players on the left, mods on the right, the session in the title
//! bar, and where logs go in the status bar (record/areas/interface/decisions.md).

use coop_core::state::*;
// gpui has a Role of its own, for accessibility.
use coop_core::state::Role;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::*;
use crate::ui::*;
use crate::view::{AppView, Asking};

const FOLLOW_COL: Pixels = px(52.);
const SHARER_COL: Pixels = px(88.);
const STATUS_COL: Pixels = px(230.);
const CHANGED_COL: Pixels = px(64.);

/// "2.31, Steam", or as much of it as the player's PC could tell.
fn game_line(p: &Player) -> String {
    let parts: Vec<&str> = [p.game_version.as_str(), p.store.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
    if parts.is_empty() { "Version unknown".into() } else { parts.join(", ") }
}

impl AppView {
    pub fn render_session(&mut self, s: &Session, version: &str, update: Option<&str>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let host_version = s.host().map(|h| h.game_version.clone());
        let note = match (s.role, s.me(), &host_version) {
            (Role::Tester, Some(me), Some(hv)) if &me.game_version != hv => Some(format!(
                "Your game is {} and {}'s is {}, so mods built for {} may not load",
                me.game_version, s.host_name, hv, hv
            )),
            _ => None,
        };

        v_flex()
            .size_full()
            .child(self.title_bar(s, update, cx))
            .when_some(s.problem.clone(), |v, problem| {
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
                        .child(icon(IconName::CircleAlert, TROUBLE))
                        .child(div().truncate().child(problem)),
                )
            })
            .when_some(note, |v, note| {
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
                        .text_color(color(WARNING))
                        .child(icon(IconName::TriangleAlert, WARNING))
                        .child(note),
                )
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .child(self.players_panel(s, host_version.as_deref(), cx))
                    .child(self.mods_panel(s, window, cx)),
            )
            .child(self.status_bar(s, version, cx))
            .into_any_element()
    }

    fn title_bar(&self, s: &Session, update: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let state: (String, u32) = match (&s.link, s.role) {
            (Link::Reconnecting, _) => ("Reconnecting".into(), WARNING),
            (Link::Connecting, _) => ("Connecting".into(), MUTED),
            (Link::WaitingForHost, _) => (format!("Waiting for {}'s session", s.host_name), MUTED),
            (Link::Connected { .. }, Role::Host) => ("Hosting".into(), MUTED),
            (Link::Connected { .. }, Role::Tester) => (format!("Hosted by {}", s.host_name), MUTED),
        };
        let invite = format!("{}/{}", s.relay, code(&s.code));
        TitleBar::new()
            .h(TITLE_BAR)
            .bg(color(PANEL))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(mark(px(14.)))
                    .child(mono(cx, code(&s.code)).text_size(TEXT_SIZE).font_weight(FontWeight::MEDIUM).text_color(color(STRONG)))
                    .child(div().text_color(color(state.1)).child(state.0)),
            )
            .child(
                h_flex()
                    .pr(px(8.))
                    .gap(px(2.))
                    .when_some(update.map(str::to_string), |b, next| {
                        b.child(
                            div().mr(px(6.)).child(
                                accent_button("apply-update", "Restart to update")
                                    .h(px(24.))
                                    .tooltip(move |window, cx| Tooltip::new(format!("Installs version {next} and starts again")).build(window, cx))
                                    .on_click(cx.listener(|view, _, _, _| view.send(Command::ApplyUpdate))),
                            ),
                        )
                    })
                    .when(s.role == Role::Host, |b| {
                        b.child(
                            plain_button("copy-invite", "Copy invite", Some(IconName::Copy))
                                .tooltip(|window, cx| Tooltip::new("Copy the invite to send to testers").build(window, cx))
                                .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(invite.clone()))),
                        )
                        .child(plain_button("stop", "Stop", None).on_click(cx.listener(|view, _, _, _| view.send(Command::Stop))))
                    })
                    .when(s.role == Role::Tester, |b| {
                        b.child(plain_button("leave", "Leave", None).on_click(cx.listener(|view, _, _, _| view.send(Command::Leave))))
                    })
                    .child(
                        icon_button("settings", IconName::SlidersHorizontal, MUTED)
                            .tooltip(|window, cx| Tooltip::new("Settings").build(window, cx))
                            .on_click(cx.listener(|view, _, window, cx| view.open_settings(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn players_panel(&self, s: &Session, host_version: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.selected;
        let rows = s.players.iter().map(|p| {
            let (state, hex) = player_state(s, p, host_version);
            let id = p.id;
            let can_open = s.role == Role::Host || p.is_me;
            h_flex()
                .id(("player", p.id as usize))
                .h(ROW)
                .px(px(12.))
                .gap(px(8.))
                .when(selected == Some(p.id), |r| r.bg(color(RAISED)))
                .when(can_open && selected != Some(p.id), |r| r.cursor_pointer().hover(|r| r.bg(color(0x1f2329))))
                .child(div().flex_1().min_w_0().truncate().text_color(color(if selected == Some(p.id) { STRONG } else { TEXT })).child(p.name.clone()))
                .when_some(state, |r, state| r.child(div().text_size(SMALL_SIZE).text_color(color(hex)).child(state)))
                .when(can_open, |r| {
                    r.on_click(cx.listener(move |view, _, _, cx| {
                        view.selected = Some(id);
                        view.picked = true;
                        cx.notify();
                    }))
                })
        });
        let rows: Vec<_> = rows.collect();
        let details = selected.and_then(|id| s.players.iter().find(|p| p.id == id)).map(|p| self.player_details(s, p, host_version, cx));

        v_flex()
            .w(PLAYERS_WIDTH)
            .flex_shrink_0()
            .bg(color(PANEL))
            .border_r_1()
            .border_color(color(LINE))
            .child(
                h_flex()
                    .h(SECTION)
                    .px(px(12.))
                    .child(label("Players").flex_1())
                    .child(mono(cx, s.players.len().to_string()).text_color(color(MUTED))),
            )
            .children(rows)
            .when_some(details, |v, d| v.child(div().mt(px(8.)).h(px(1.)).bg(color(LINE))).child(d))
            .into_any_element()
    }

    fn player_details(&self, s: &Session, p: &Player, host_version: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let mono_family = cx.theme_mono();
        let row = |key: &'static str, value: AnyElement| {
            h_flex().text_size(SMALL_SIZE).child(div().w(px(72.)).text_color(color(MUTED)).child(key)).child(value)
        };
        let text = |t: String, hex: u32| div().text_color(color(hex)).child(t).into_any_element();
        let off_version = host_version.is_some_and(|hv| hv != p.game_version);
        let joined = format!("{}{}", hhmm(&p.joined), if p.resumed { ", resumed" } else { "" });
        let (bytes_key, whose_pc) = if p.is_me && s.role == Role::Tester { ("Sent", "this PC".to_string()) } else { ("Received", format!("{}'s PC", p.name)) };

        let sources = p.sources.as_ref().map(|sources| {
            v_flex()
                .gap(px(4.))
                .text_size(SMALL_SIZE)
                .child(label(format!("Logs picked up on {whose_pc}")))
                .children(sources.iter().map(|src| {
                    let missing = src.count == 0 && matches!(src.kind, SourceKind::Extra { .. });
                    let hex = if missing { WARNING } else { TEXT };
                    let name: AnyElement = match &src.kind {
                        SourceKind::GameFolder => text("Game folder logs".into(), TEXT),
                        SourceKind::CrashReports => text("Crash reports".into(), TEXT),
                        SourceKind::GameOwn => text("The game's own logs and settings".into(), TEXT),
                        SourceKind::AppLog => text("This app's log".into(), TEXT),
                        SourceKind::Extra { pattern } => h_flex()
                            .gap(px(6.))
                            .min_w_0()
                            .child(div().font_family(mono_family.clone()).text_color(color(hex)).truncate().child(pattern.clone()))
                            .when(s.role == Role::Tester, |r| r.child(div().text_color(color(MUTED)).child(format!("from {}", s.host_name))))
                            .into_any_element(),
                    };
                    h_flex()
                        .child(div().flex_1().min_w_0().child(name))
                        .child(div().w(px(40.)).text_right().font_family(mono_family.clone()).text_color(color(hex)).child(src.count.to_string()))
                }))
        });

        v_flex()
            .p(px(12.))
            .gap(px(12.))
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(color(STRONG)).child(p.name.clone()))
                    .when(p.is_me, |r| r.child(div().text_color(color(MUTED)).child("you"))),
            )
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(row("Game", text(game_line(p), if off_version { WARNING } else { TEXT })))
                    .child(row("System", text(p.system.clone(), TEXT)))
                    .child(row("Joined", text(joined, TEXT)))
                    .child(row(
                        bytes_key,
                        if p.bytes == 0 { text("Nothing yet".into(), MUTED) } else { div().font_family(mono_family.clone()).child(bytes(p.bytes)).into_any_element() },
                    )),
            )
            .when_some(sources, |v, s| v.child(s))
            .into_any_element()
    }

    fn mods_panel(&mut self, s: &Session, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let header = h_flex()
            .h(HEADER_ROW)
            .flex_shrink_0()
            .text_size(SMALL_SIZE)
            .text_color(color(MUTED))
            .border_b_1()
            .border_color(color(LINE))
            .child(div().w(FOLLOW_COL).pl(px(12.)).child("Follow"))
            .child(div().flex_1().px(px(8.)).child("Mod"))
            .child(div().w(SHARER_COL).px(px(8.)).child("Shared by"))
            .child(div().w(STATUS_COL).px(px(8.)).child("Status"))
            .child(div().w(CHANGED_COL).pr(px(12.)).text_right().child("Changed"));

        let asking = self.asking.clone();
        let rows: Vec<AnyElement> = s.shared.iter().enumerate().map(|(i, m)| self.shared_row(s, m, i, asking.as_ref(), cx)).collect();
        let question = match &asking {
            Some(a @ (Asking::Follow(key) | Asking::Unfollow(key))) => {
                s.shared.iter().position(|m| &m.key == key).map(|i| self.question(&s.shared[i], a, i, cx))
            }
            None => None,
        };

        let my_mods = self.my_mods(s, cx);

        div()
            .id("mods")
            .flex_1()
            .min_w_0()
            .relative()
            .overflow_y_scroll()
            .child(h_flex().h(SECTION).px(px(12.)).child(label("Shared in this session")))
            .child(header)
            .children(rows)
            .child(my_mods)
            .when_some(question, |d, q| d.child(q))
            .into_any_element()
    }

    fn shared_row(&self, _: &Session, m: &SharedMod, i: usize, asking: Option<&Asking>, cx: &mut Context<Self>) -> AnyElement {
        let key = m.key.clone();
        let asked_here = asking.is_some_and(|a| matches!(a, Asking::Follow(k) | Asking::Unfollow(k) if k == &m.key));
        let follow_cell = (!m.mine).then(|| {
            let check = match m.follow {
                Follow::No => Check::Off,
                Follow::Yes => Check::On,
                Follow::AddedFolder => Check::Partly,
            };
            let follow = m.follow;
            let key = key.clone();
            checkbox(("follow", i), check, asked_here).on_click(cx.listener(move |view, _, _, cx| {
                match follow {
                    Follow::No => view.asking = Some(Asking::Follow(key.clone())),
                    Follow::Yes => view.asking = Some(Asking::Unfollow(key.clone())),
                    Follow::AddedFolder => view.send(Command::TakeAddedFolder { key: key.clone() }),
                }
                cx.notify();
            }))
        });

        let status: AnyElement = if m.mine {
            let up = m.followers.iter().filter(|f| f.delivery == Delivery::UpToDate).count();
            let all = m.followers.len();
            let words = if all > 0 && up == all { format!("All {all} up to date") } else { format!("{up} of {all} up to date") };
            let followers = m.followers.clone();
            h_flex()
                .gap(px(8.))
                .child(
                    h_flex()
                        .id(("tiles", i))
                        .gap(px(2.))
                        .children(m.followers.iter().map(|f| {
                            let (bg, fg) = match f.delivery {
                                Delivery::UpToDate => (DONE, ON_ACCENT),
                                Delivery::Fetching { .. } => (ACCENT, ON_ACCENT),
                                Delivery::NeedsRestart => (WARNING, ON_ACCENT),
                                Delivery::Paused => (CONTROL, MUTED),
                            };
                            tile(cx, &f.name, bg, fg)
                        }))
                        .tooltip(move |window, cx| {
                            let followers = followers.clone();
                            Tooltip::element(move |_, _| {
                                v_flex().gap(px(4.)).text_size(SMALL_SIZE).children(followers.iter().map(|f| {
                                    let (words, hex) = match f.delivery {
                                        Delivery::UpToDate => ("Up to date".to_string(), TEXT),
                                        Delivery::Fetching { done, total } => (format!("Fetching {done} of {total} files"), ACCENT),
                                        Delivery::NeedsRestart => ("Has it, needs a game restart".to_string(), WARNING),
                                        Delivery::Paused => ("Paused while reconnecting".to_string(), MUTED),
                                    };
                                    h_flex().child(div().w(px(56.)).child(f.name.clone())).child(div().text_color(color(hex)).child(words))
                                }))
                            })
                            .build(window, cx)
                        }),
                )
                .child(div().text_color(color(if up == all { MUTED } else { TEXT })).child(words))
                .into_any_element()
        } else {
            let (words, hex) = match &m.status {
                ModStatus::UpToDate => ("Up to date".to_string(), MUTED),
                ModStatus::Fetching { done, total } => (format!("Fetching {done} of {total} files"), ACCENT),
                ModStatus::NeedsRestart => ("Restart the game to load it".to_string(), ACCENT),
                ModStatus::NotOnYourPc => ("Not on your PC".to_string(), MUTED),
                ModStatus::AddedFolder => ("Added a folder, tick to take it".to_string(), ACCENT),
                ModStatus::Paused => ("Paused while reconnecting".to_string(), MUTED),
                ModStatus::Checking => ("Checking your copy".to_string(), MUTED),
                ModStatus::OnYourPc { same: true } => ("Yours is the same".to_string(), MUTED),
                ModStatus::OnYourPc { same: false } => ("Yours differs".to_string(), TEXT),
                ModStatus::FollowedElsewhere { by } => (format!("Following {by}'s"), MUTED),
                ModStatus::NotShared => ("No longer shared".to_string(), MUTED),
                ModStatus::OfferedToo => ("Yours goes out when they stop".to_string(), MUTED),
                ModStatus::Sharing => (String::new(), MUTED),
            };
            div().text_color(color(hex)).child(words).into_any_element()
        };

        h_flex()
            .id(("shared", i))
            .h(ROW)
            .when(i > 0, |r| r.border_t_1().border_color(color(LINE_SOFT)))
            .when(asked_here, |r| r.bg(color(RAISED)).border_1().border_color(color(ACCENT)))
            .child(div().w(FOLLOW_COL).pl(px(12.)).when_some(follow_cell, |d, c| d.child(c)))
            .child(div().flex_1().min_w_0().px(px(8.)).truncate().text_color(color(if m.mine || asked_here { STRONG } else { TEXT })).child(m.name.clone()))
            .child(div().w(SHARER_COL).px(px(8.)).text_color(color(MUTED)).truncate().child(if m.mine { "You".to_string() } else { m.sharer.clone() }))
            .child(div().w(STATUS_COL).px(px(8.)).child(status))
            .child(mono(cx, m.changed.as_ref().map(hhmm).unwrap_or_default()).w(CHANGED_COL).pr(px(12.)).text_right().text_color(color(if m.mine { TEXT } else { MUTED })))
            .into_any_element()
    }

    /// The box under a shared mod's row that asks before following or unfollowing it.
    fn question(&self, m: &SharedMod, asking: &Asking, i: usize, cx: &mut Context<Self>) -> AnyElement {
        // The rows start under the section label and the column headers, and are all one height.
        let top = SECTION + HEADER_ROW + ROW * (i as f32 + 1.) + px(4.);
        let key = m.key.clone();
        let (title, body, yes, no): (String, String, &'static str, &'static str) = match asking {
            Asking::Follow(_) => (
                format!("Follow {} from {}?", m.name, m.sharer),
                if m.has_dll {
                    "It has a DLL, which runs inside your game. Follow only people you trust.".into()
                } else {
                    "Mods run inside your game. Follow only people you trust.".into()
                },
                "Follow",
                "Cancel",
            ),
            Asking::Unfollow(_) => (
                format!("Stop following {}?", m.name),
                "Your copy can stay as it is, or go, after a copy goes to the backups.".into(),
                "Keep my copy",
                "Delete it",
            ),
        };
        let is_follow = matches!(asking, Asking::Follow(_));
        let (key_yes, key_no) = (key.clone(), key);
        v_flex()
            .absolute()
            .top(top)
            .left(px(40.))
            .w(px(320.))
            .p(px(12.))
            .gap(px(10.))
            .bg(color(RAISED))
            .border_1()
            .border_color(color(CONTROL))
            .rounded(RADIUS)
            .shadow_md()
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(color(STRONG)).child(title))
                    .child(div().text_size(SMALL_SIZE).line_height(px(18.)).text_color(color(MUTED)).child(body)),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(accent_button("answer-yes", yes).h(px(24.)).px(px(10.)).on_click(cx.listener(move |view, _, _, cx| {
                        let key = key_yes.clone();
                        view.send(if is_follow { Command::Follow { key, on: true } } else { Command::Unfollow { key, delete_copy: false } });
                        view.asking = None;
                        cx.notify();
                    })))
                    .child(outline_button("answer-no", no).map(|b| if is_follow { b } else { b.text_color(color(TROUBLE)) }).on_click(cx.listener(move |view, _, _, cx| {
                        if !is_follow {
                            view.send(Command::Unfollow { key: key_no.clone(), delete_copy: true });
                        }
                        view.asking = None;
                        cx.notify();
                    }))),
            )
            .into_any_element()
    }

    fn my_mods(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let open = self.my_mods_open;
        let toggle = h_flex()
            .id("my-mods")
            .mt(px(16.))
            .h(SECTION)
            .px(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_b_1()
            .border_color(color(LINE))
            .text_size(SMALL_SIZE)
            .cursor_pointer()
            .hover(|r| r.bg(color(PANEL)))
            .child(icon(if open { IconName::ChevronDown } else { IconName::ChevronRight }, TEXT))
            .child(div().font_weight(FontWeight::MEDIUM).child("Your mods"))
            .child(mono(cx, s.my_mods.len().to_string()).text_color(color(MUTED)))
            .when(!open, |r| r.child(div().text_color(color(MUTED)).child("Share one with the others")))
            .on_click(cx.listener(|view, _, _, cx| {
                view.my_mods_open = !view.my_mods_open;
                cx.notify();
            }));

        let rows = open.then(|| {
            s.my_mods.iter().enumerate().map(|(i, m)| {
                let key = m.key.clone();
                let on = !m.shared;
                let places = m.places.iter().map(|p| p.label()).collect::<Vec<_>>().join(", ");
                let loose = m.loose_file.clone();
                h_flex()
                    .id(("my-mod", i))
                    .h(ROW)
                    .when(i > 0, |r| r.border_t_1().border_color(color(LINE_SOFT)))
                    .child(
                        div().w(FOLLOW_COL).pl(px(12.)).child(
                            checkbox(("share", i), if m.shared { Check::On } else { Check::Off }, false)
                                .on_click(cx.listener(move |view, _, _, _| view.send(Command::Share { key: key.clone(), on }))),
                        ),
                    )
                    .child(div().flex_1().min_w_0().px(px(8.)).truncate().child(m.name.clone()))
                    .child(
                        h_flex()
                            .id(("places", i))
                            .w(px(262.))
                            .px(px(8.))
                            .gap(px(4.))
                            .text_color(color(if loose.is_some() { WARNING } else { MUTED }))
                            .when(loose.is_some(), |r| r.child(icon(IconName::TriangleAlert, WARNING)))
                            .child(div().truncate().child(if loose.is_some() { format!("{places}, and a loose file") } else { places }))
                            .when_some(loose, |r, file| {
                                r.tooltip(move |window, cx| {
                                    Tooltip::new(format!("{file} sits loose, so sharing leaves it out. Move it into a folder named after the mod there and it joins the mod."))
                                        .build(window, cx)
                                })
                            }),
                    )
                    .child(mono(cx, bytes(m.bytes)).w(CHANGED_COL).pr(px(12.)).text_right().text_color(color(MUTED)))
            }).collect::<Vec<_>>()
        });

        v_flex().child(toggle).when_some(rows, |v, rows| v.children(rows)).into_any_element()
    }

    fn status_bar(&self, s: &Session, version: &str, cx: &mut Context<Self>) -> AnyElement {
        let latency = match s.link {
            Link::Connected { latency_ms } => format!("{} {}ms", s.relay, latency_ms),
            _ => s.relay.clone(),
        };
        let logs: AnyElement = match &s.logs {
            LogsLine::Host { folder } => {
                let (copy, open) = (folder.clone(), folder.clone());
                // A long path gives way, so the relay and the version always show.
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.))
                    .child(div().flex_shrink_0().child("Logs go to"))
                    .child(mono(cx, folder.display().to_string()).min_w_0().truncate().text_color(color(TEXT)))
                    .child(
                        icon_button("copy-path", IconName::Copy, TEXT)
                            .size(px(20.))
                            .tooltip(|window, cx| Tooltip::new("Copy the path").build(window, cx))
                            .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.display().to_string()))),
                    )
                    .child(
                        icon_button("open-folder", IconName::Folder, TEXT)
                            .size(px(20.))
                            .tooltip(|window, cx| Tooltip::new("Open the folder").build(window, cx))
                            .on_click(move |_, _, cx| cx.open_with_system(&open)),
                    )
                    .into_any_element()
            }
            LogsLine::Tester { host, sending } => {
                let (words, hex) = match sending {
                    Sending::Idle => (String::new(), MUTED),
                    Sending::CatchingUp { done, total } => (format!("Catching up, {done} of {total} files"), ACCENT),
                    Sending::UpToDate => ("Up to date".to_string(), MUTED),
                    Sending::Paused => ("Paused while reconnecting".to_string(), WARNING),
                };
                h_flex().flex_1().min_w_0().gap(px(12.)).child(format!("Your logs go to {host}")).child(div().text_color(color(hex)).child(words)).into_any_element()
            }
        };
        h_flex()
            .h(STATUS_BAR)
            .flex_shrink_0()
            .px(px(12.))
            .gap(px(12.))
            .bg(color(PANEL))
            .border_t_1()
            .border_color(color(LINE))
            .text_size(SMALL_SIZE)
            .text_color(color(MUTED))
            .child(logs)
            .child(div().flex_shrink_0().child(latency))
            .child(mono(cx, version.to_string()).flex_shrink_0())
            .into_any_element()
    }
}

/// A player row's one line of state, and its color; `None` when there's nothing to say.
fn player_state(s: &Session, p: &Player, host_version: Option<&str>) -> (Option<String>, u32) {
    let off_version = host_version.is_some_and(|hv| hv != p.game_version);
    match (&p.state, s.role) {
        (PlayerState::Reconnecting, _) => (Some("Reconnecting".into()), WARNING),
        (PlayerState::NeedsRestart, Role::Host) if off_version => (Some(format!("Game {}, needs a restart", p.game_version)), WARNING),
        (PlayerState::NeedsRestart, Role::Host) => (Some("Needs a restart".into()), WARNING),
        (PlayerState::Fetching { done, total }, Role::Host) => (Some(format!("Fetching {done} of {total}")), ACCENT),
        _ if p.is_host && p.is_me => (Some("You, hosting".into()), MUTED),
        _ if p.is_host => (Some("Hosting".into()), MUTED),
        _ if p.is_me => (Some("You".into()), MUTED),
        _ if off_version && s.role == Role::Host => (Some(format!("Game {}", p.game_version)), WARNING),
        _ => (None, MUTED),
    }
}

trait MonoFamily {
    fn theme_mono(&self) -> SharedString;
}

impl<T> MonoFamily for Context<'_, T> {
    fn theme_mono(&self) -> SharedString {
        use gpui_kit::component::ActiveTheme as _;
        self.theme().mono_font_family.clone()
    }
}
