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

// The frame's columns (experiments/areas/interface/frames/WindowHost.dc.html), on the 4px grid.
const FOLLOW_COL: Pixels = px(72.);
const SHARER_COL: Pixels = px(104.);
const STATUS_COL: Pixels = px(248.);
const CHANGED_COL: Pixels = px(84.);

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
                        .px(INSET)
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
                        .px(INSET)
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
        // A running session needs no word: the stop square says it's running. Only a session
        // that isn't running as it should says so.
        let state: Option<(String, u32)> = match &s.link {
            Link::Reconnecting => Some(("Reconnecting".into(), WARNING)),
            Link::Connecting => Some(("Connecting".into(), MUTED)),
            Link::WaitingForHost => Some((format!("Waiting for {}'s session", s.host_name), MUTED)),
            Link::Connected { .. } => None,
        };
        let stop_tip = match s.role {
            Role::Host => "Stop hosting".to_string(),
            Role::Tester => format!("Leave {}'s session", s.host_name),
        };
        // Just the code: testers set the relay once, so the code is all a returning group needs.
        let invite = code(&s.code);
        let tip = "Copy the code".to_string();
        let copied = self.copied_invite;
        TitleBar::new()
            .h(TITLE_BAR)
            .bg(color(PANEL))
            .child(
                // The mark, the code, and the stop square sit about 8px apart, glyph to glyph: the
                // code's 4px padding and the stop button's 4.5px around its glyph make up the gaps,
                // so the row adds none of its own (measured in the window, 2026-11-13).
                h_flex()
                    .child(mark(px(14.)))
                    .child({
                        // The host's code copies the invite when clicked, so it needs no button of
                        // its own; a tester's is only a label, since they have nobody to send it to.
                        let chip = div()
                            .id("code")
                            .px(px(4.))
                            .rounded(RADIUS)
                            .font_family(MONO_FONT)
                            .text_size(TEXT_SIZE)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(color(if copied { ACCENT } else { STRONG }))
                            // "Copied" is a character shorter than a code; a space makes up the width, so the
                            // stop square stays put.
                            .child(if copied { "Copied ".to_string() } else { code(&s.code) });
                        if s.role == Role::Host {
                            chip.occlude()
                                .cursor_pointer()
                                .hover(|c| c.bg(color(RAISED)))
                                .active(|c| c.bg(color(CONTROL)))
                                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(invite.clone()));
                                    view.flash_copied(cx);
                                }))
                                .into_any_element()
                        } else {
                            chip.into_any_element()
                        }
                    })
                    .child(
                        h_flex()
                            // Pulled in 2px to make up for the space the button keeps around its glyph.
                            .ml(px(-2.))
                            .child(
                                stop_button("stop")
                                    .tooltip(move |window, cx| Tooltip::new(stop_tip.clone()).build(window, cx))
                                    .on_click(cx.listener(|view, _, _, _| {
                                        let role = view.snapshot.as_ref().and_then(|s| match &s.screen {
                                            Screen::Session(session) => Some(session.role),
                                            _ => None,
                                        });
                                        view.send(if role == Some(Role::Tester) { Command::Leave } else { Command::Stop });
                                    })),
                            ),
                    )
                    .when_some(state, |r, (words, hex)| r.child(div().text_color(color(hex)).child(words))),
            )
            .child(
                h_flex()
                    .pr(px(8.))
                    .gap(px(4.))
                    .when_some(update.map(str::to_string), |b, next| {
                        b.child(
                            div().mr(px(8.)).child(
                                accent_pill("apply-update", "Restart to update")
                                    .tooltip(move |window, cx| Tooltip::new(format!("Installs version {next} and starts again")).build(window, cx))
                                    .on_click(cx.listener(|view, _, _, _| view.send(Command::ApplyUpdate))),
                            ),
                        )
                    })
                    .child(
                        icon_button("settings", IconName::SlidersHorizontal, TEXT)
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
            let open = selected == Some(p.id);
            // The player's tile, colored as their tiles are in the mods table, and filling while
            // their mods are on their way.
            let player_tile = match p.state {
                PlayerState::Fine => tile(cx, &p.name, DONE, ON_ACCENT),
                PlayerState::Fetching { done, total } => progress_tile(cx, &p.name, done, total),
                PlayerState::NeedsRestart | PlayerState::NeedsReload | PlayerState::Held => tile(cx, &p.name, WARNING, ON_ACCENT),
                PlayerState::Reconnecting => tile(cx, &p.name, CONTROL, MUTED),
            };
            let row = h_flex()
                .id(("player", p.id as usize))
                .h(ROW)
                .px(INSET)
                .gap(px(8.))
                .when(open, |r| r.bg(color(RAISED)))
                .when(can_open && !open, |r| r.cursor_pointer().hover(|r| r.bg(color(HOVER))))
                .when(can_open && open, |r| r.cursor_pointer())
                .child(player_tile)
                .child(div().flex_1().min_w_0().truncate().text_color(color(if open { STRONG } else { TEXT })).child(p.name.clone()))
                .when_some(state, |r, state| r.child(div().text_size(SMALL_SIZE).text_color(color(hex)).child(state)))
                .when(can_open, |r| {
                    // A click opens this player's details under the row, or closes them when open.
                    r.on_click(cx.listener(move |view, _, _, cx| {
                        view.selected = if view.selected == Some(id) { None } else { Some(id) };
                        cx.notify();
                    }))
                })
                .into_any_element();
            let details = open.then(|| self.player_details(s, p, host_version, cx));
            (row, details)
        });
        let rows: Vec<AnyElement> = rows.flat_map(|(row, details)| std::iter::once(row).chain(details)).collect();

        v_flex()
            .w(PLAYERS_WIDTH)
            .flex_shrink_0()
            .bg(color(PANEL))
            .border_r_1()
            .border_color(color(LINE))
            .child(
                h_flex()
                    .h(SECTION)
                    .px(INSET)
                    .child(label("Players").flex_1())
                    .child(mono(cx, s.players.len().to_string()).text_color(color(MUTED))),
            )
            .children(rows)
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
                            .gap(px(8.))
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

        // Under the open row, on its raised background and indented to its name (past the 20px
        // tile and the 8px gap), so the details read as part of that row.
        v_flex()
            .pl(INSET + px(28.))
            .pr(INSET)
            .pt(px(4.))
            .pb(px(12.))
            .gap(px(12.))
            .bg(color(RAISED))
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
            .child(div().w(FOLLOW_COL).pl(INSET).child("Follow"))
            .child(div().flex_1().px(px(8.)).child("Mod"))
            .child(div().w(SHARER_COL).px(px(8.)).child("Shared by"))
            .child(div().w(STATUS_COL).px(px(8.)).child("Status"))
            .child(div().w(CHANGED_COL).pr(INSET).text_right().child("Changed"));

        let asking = self.asking.clone();
        let rows: Vec<AnyElement> = s.shared.iter().enumerate().map(|(i, m)| self.shared_row(s, m, i, asking.as_ref(), cx)).collect();
        let question = match &asking {
            Some(a @ (Asking::Follow(key) | Asking::Unfollow(key))) => {
                s.shared.iter().position(|m| &m.key == key).map(|i| self.question(&s.shared[i], a, i, cx))
            }
            None => None,
        };

        let my_mods = self.my_mods(s, cx);
        let empty = rows.is_empty().then(|| {
            let words = match s.role {
                Role::Host => "Nothing's shared yet. Tick a mod under Your mods to send it to everyone who follows it.",
                Role::Tester => "Nothing's shared yet. When someone shares a mod, it shows here for you to follow.",
            };
            empty_note(words)
        });

        // The shared list takes the top and scrolls on its own; "Your mods" is a drawer pinned to the
        // bottom, so a mod moving into the shared list grows the top and never shifts the row the
        // player just clicked (the user, 2026-11-13).
        v_flex()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .id("shared")
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_y_scroll()
                    .child(h_flex().h(SECTION).px(INSET).child(label("Shared in this session")))
                    // With nothing shared, the columns have nothing to name.
                    .when(empty.is_none(), |d| d.child(header))
                    .children(rows)
                    .children(empty)
                    .when_some(question, |d, q| d.child(q)),
            )
            .child(my_mods)
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
                        .gap(px(4.))
                        .children(m.followers.iter().map(|f| {
                            match f.delivery {
                                Delivery::UpToDate => tile(cx, &f.name, DONE, ON_ACCENT),
                                Delivery::Fetching { done, total } => progress_tile(cx, &f.name, done, total),
                                Delivery::NeedsRestart | Delivery::NeedsReload | Delivery::Held => tile(cx, &f.name, WARNING, ON_ACCENT),
                                Delivery::Paused => tile(cx, &f.name, CONTROL, MUTED),
                            }
                        }))
                        .tooltip(move |window, cx| {
                            let followers = followers.clone();
                            Tooltip::element(move |_, _| {
                                v_flex().gap(px(4.)).text_size(SMALL_SIZE).children(followers.iter().map(|f| {
                                    let (words, hex) = match f.delivery {
                                        Delivery::UpToDate => ("Up to date".to_string(), TEXT),
                                        Delivery::Fetching { done, total } => (format!("Fetching {done} of {total} files"), PROGRESS),
                                        Delivery::NeedsRestart => ("Has it, needs a game restart".to_string(), WARNING),
                                        Delivery::NeedsReload => ("Has it, needs a CET reload".to_string(), WARNING),
                                        Delivery::Held => ("Has it, waiting for a program to let go of the old files".to_string(), WARNING),
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
                ModStatus::Fetching { done, total } => (format!("Fetching {done} of {total} files"), PROGRESS),
                ModStatus::NeedsRestart => ("Restart the game to load it".to_string(), WARNING),
                ModStatus::NeedsReload => ("Reload all mods in CET to load it".to_string(), WARNING),
                ModStatus::Held => ("Waiting for a program to let go of the old files".to_string(), WARNING),
                ModStatus::NotOnYourPc => ("Not on your PC".to_string(), MUTED),
                ModStatus::AddedFolder => ("Added a folder, tick to take it".to_string(), WARNING),
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
            .child(div().w(FOLLOW_COL).pl(INSET).when_some(follow_cell, |d, c| d.child(c)))
            .child(div().flex_1().min_w_0().px(px(8.)).truncate().text_color(color(if m.mine || asked_here { STRONG } else { TEXT })).child(m.name.clone()))
            .child(div().w(SHARER_COL).px(px(8.)).text_color(color(MUTED)).truncate().child(if m.mine { "You".to_string() } else { m.sharer.clone() }))
            .child(div().w(STATUS_COL).px(px(8.)).child(status))
            .child(mono(cx, m.changed.as_ref().map(hhmm).unwrap_or_default()).w(CHANGED_COL).pr(INSET).text_right().text_color(color(if m.mine { TEXT } else { MUTED })))
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
            Asking::Unfollow(_) => (format!("Stop following {}?", m.name), "Delete your copy as well?".into(), "Keep my copy", "Delete it"),
        };
        let is_follow = matches!(asking, Asking::Follow(_));
        let (key_yes, key_no) = (key.clone(), key);
        let close = cx.listener(|view, _, _, cx| {
            view.asking = None;
            cx.notify();
        });
        v_flex()
            .absolute()
            .top(top)
            .left(px(40.))
            .w(px(320.))
            .p(INSET)
            .gap(px(12.))
            .bg(color(RAISED))
            .border_1()
            .border_color(color(CONTROL))
            .rounded(RADIUS)
            .shadow_md()
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(color(STRONG)).child(title))
                    .child(div().text_size(SMALL_SIZE).line_height(px(20.)).text_color(color(MUTED)).child(body)),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    // Following: Follow, or Cancel. Unfollowing: keep the copy, delete it, or Cancel,
                    // which leaves the mod followed (the user, 2026-11-13).
                    .child(button("answer-yes", yes, if is_follow { Kind::Primary } else { Kind::Secondary }).on_click(cx.listener(move |view, _, _, cx| {
                        let key = key_yes.clone();
                        view.send(if is_follow { Command::Follow { key, on: true } } else { Command::Unfollow { key, delete_copy: false } });
                        view.asking = None;
                        cx.notify();
                    })))
                    .when(!is_follow, |r| {
                        r.child(button("answer-no", no, Kind::Danger).on_click(cx.listener(move |view, _, _, cx| {
                            view.send(Command::Unfollow { key: key_no.clone(), delete_copy: true });
                            view.asking = None;
                            cx.notify();
                        })))
                    })
                    // Backing out is always the plain button on the right, in both questions.
                    .child(div().flex_1())
                    .child(plain_button("answer-cancel", "Cancel", None).on_click(close)),
            )
            .into_any_element()
    }

    fn my_mods(&self, s: &Session, cx: &mut Context<Self>) -> AnyElement {
        let open = self.my_mods_open;
        let toggle = h_flex()
            .id("my-mods")
            .flex_shrink_0()
            .h(SECTION)
            .px(INSET)
            .gap(px(8.))
            .bg(color(PANEL))
            .border_t_1()
            .when(open, |r| r.border_b_1())
            .border_color(color(LINE))
            .text_size(SMALL_SIZE)
            .cursor_pointer()
            .hover(|r| r.bg(color(HOVER)))
            // Up opens the drawer upward and down closes it; Lucide draws the two as mirror images in
            // one spot, so the chevron doesn't move between them.
            .child(icon(if open { IconName::ChevronDown } else { IconName::ChevronUp }, TEXT))
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
                        div().w(FOLLOW_COL).pl(INSET).child(
                            checkbox(("share", i), if m.shared { Check::On } else { Check::Off }, false)
                                .on_click(cx.listener(move |view, _, _, _| view.send(Command::Share { key: key.clone(), on }))),
                        ),
                    )
                    .child(div().flex_1().min_w_0().px(px(8.)).truncate().child(m.name.clone()))
                    .child(
                        h_flex()
                            .id(("places", i))
                            .w(px(264.))
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
                    .child(mono(cx, bytes(m.bytes)).w(CHANGED_COL).pr(INSET).text_right().text_color(color(MUTED)))
            }).collect::<Vec<_>>()
        });

        let empty = (open && s.my_mods.is_empty()).then(|| empty_note("No mods in your game's mod folders yet."));
        // Closed, the drawer is just its bar; open, it takes the panel's bottom half and scrolls on its own.
        v_flex()
            .flex_shrink_0()
            .when(open, |v| v.h(relative(0.5)))
            .child(toggle)
            .when(open, |v| {
                v.child(
                    div()
                        .id("my-mods-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .when_some(rows, |d, rows| d.children(rows))
                        .children(empty),
                )
            })
            .into_any_element()
    }

    fn status_bar(&self, s: &Session, version: &str, cx: &mut Context<Self>) -> AnyElement {
        let latency = match s.link {
            Link::Connected { latency_ms } => format!("{} {}ms", s.relay, latency_ms),
            _ => s.relay.clone(),
        };
        let logs: AnyElement = match &s.logs {
            LogsLine::Host { folder } => {
                let (copy, open) = (folder.clone(), folder.clone());
                // The bar names the logs folder the host chose; the session's own folder inside it
                // is named after the date and the code the title bar already shows, and the buttons
                // act on it.
                let shown = folder.parent().unwrap_or(folder).display().to_string();
                let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let (copy_tip, open_tip) = (format!("Copy the path of this session's folder, {name}"), format!("Open this session's folder, {name}"));
                // A long path gives way, so the relay and the version always show.
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .child(div().flex_shrink_0().child("Logs go to"))
                    .child(mono(cx, shown).min_w_0().truncate())
                    .child(
                        icon_button("copy-path", IconName::Copy, MUTED)
                            .size(px(20.))
                            .tooltip(move |window, cx| Tooltip::new(copy_tip.clone()).build(window, cx))
                            .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.display().to_string()))),
                    )
                    .child(
                        icon_button("open-folder", IconName::Folder, MUTED)
                            .size(px(20.))
                            .tooltip(move |window, cx| Tooltip::new(open_tip.clone()).build(window, cx))
                            .on_click(move |_, _, cx| cx.open_with_system(&open)),
                    )
                    .into_any_element()
            }
            LogsLine::Tester { host, sending } => {
                let (words, hex) = match sending {
                    Sending::Idle => (String::new(), MUTED),
                    Sending::CatchingUp { done, total } => (format!("Catching up, {done} of {total} files"), PROGRESS),
                    Sending::UpToDate => ("Up to date".to_string(), MUTED),
                    Sending::Paused => ("Paused while reconnecting".to_string(), WARNING),
                };
                h_flex().flex_1().min_w_0().gap(px(12.)).child(format!("Your logs go to {host}")).child(div().text_color(color(hex)).child(words)).into_any_element()
            }
        };
        h_flex()
            .h(STATUS_BAR)
            .flex_shrink_0()
            .px(INSET)
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

/// What a list says when it has no rows, in muted text where its first row would be.
fn empty_note(words: &'static str) -> AnyElement {
    div().px(INSET).py(px(8.)).text_color(color(MUTED)).child(words).into_any_element()
}

/// A player row's one line of state, and its color; `None` when there's nothing to say.
fn player_state(s: &Session, p: &Player, host_version: Option<&str>) -> (Option<String>, u32) {
    let off_version = host_version.is_some_and(|hv| hv != p.game_version);
    // The host sees every player's state; a tester sees their own, which their row would otherwise
    // hide behind "You".
    let sees = s.role == Role::Host || p.is_me;
    match &p.state {
        PlayerState::Reconnecting => (Some("Reconnecting".into()), WARNING),
        PlayerState::NeedsRestart if sees && off_version => (Some(format!("Game {}, needs a restart", p.game_version)), WARNING),
        PlayerState::NeedsRestart if sees => (Some("Needs a restart".into()), WARNING),
        PlayerState::NeedsReload if sees => (Some("Needs a CET reload".into()), WARNING),
        PlayerState::Held if sees => (Some("Waiting for a program".into()), WARNING),
        PlayerState::Fetching { done, total } if sees => (Some(format!("Fetching {done} of {total}")), PROGRESS),
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
