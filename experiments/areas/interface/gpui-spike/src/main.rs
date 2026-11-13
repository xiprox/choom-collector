// Feasibility spike: gpui (gpui-pre snapshot) + gpui-component (via gpui-kit) + velopack.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{thread, time::Duration};

use gpui_kit::component::{
    ActiveTheme as _, StyledExt as _, Theme, ThemeMode,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    table::{Column, DataTable, TableDelegate, TableState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::*;

// ---------- fake data ----------

struct ModRow {
    checked: bool,
    name: SharedString,
    kind: SharedString,
    shared_by: SharedString,
    status: SharedString,
    folders: Vec<SharedString>,
}

fn fake_rows(n: usize) -> Vec<ModRow> {
    const NAMES: [&str; 12] = [
        "Better Netrunning", "Night City Lights", "Immersive Rippers", "Cyber Engine Tweaks",
        "ArchiveXL", "TweakXL", "Codeware", "RED4ext", "Appearance Menu Mod",
        "Virtual Car Dealer", "Braindance Protocol", "Kiroshi Optics Overhaul",
    ];
    const KINDS: [&str; 4] = ["archive", "redscript", "CET", "tweak"];
    const PEOPLE: [&str; 4] = ["V", "Judy", "Panam", "Johnny"];
    const STATUS: [&str; 4] = ["in sync", "downloading", "outdated", "missing"];
    (0..n)
        .map(|i| {
            let name = format!("{} {}", NAMES[i % NAMES.len()], i / NAMES.len() + 1);
            let kind = KINDS[i % KINDS.len()];
            ModRow {
                checked: i % 3 == 0,
                folders: vec![
                    format!("archive/pc/mod/{}", name.replace(' ', "_")).into(),
                    format!("r6/scripts/{}", name.replace(' ', "")).into(),
                    format!("bin/x64/plugins/cyber_engine_tweaks/mods/{i}").into(),
                ],
                name: name.into(),
                kind: kind.into(),
                shared_by: PEOPLE[(i * 7) % PEOPLE.len()].into(),
                status: STATUS[(i * 5) % STATUS.len()].into(),
            }
        })
        .collect()
}

// ---------- table ----------

struct ModsDelegate {
    rows: Vec<ModRow>,
}

impl TableDelegate for ModsDelegate {
    fn columns_count(&self, _: &App) -> usize {
        5
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("on", "").width(44.).resizable(false),
            1 => Column::new("name", "Name").width(280.),
            2 => Column::new("type", "Type").width(110.),
            3 => Column::new("shared_by", "Shared by").width(130.),
            _ => Column::new("status", "Status").width(130.),
        }
    }

    // Row-level tooltip with a fake folder list.
    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let row = &self.rows[row_ix];
        let name = row.name.clone();
        let folders = row.folders.clone();
        div().id(("row", row_ix)).tooltip(move |window, cx| {
            let name = name.clone();
            let folders = folders.clone();
            Tooltip::element(move |_, _| {
                v_flex()
                    .gap_0p5()
                    .child(div().font_semibold().child(name.clone()))
                    .children(folders.iter().map(|f| div().text_xs().child(f.clone())))
            })
            .build(window, cx)
        })
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let row = &self.rows[row_ix];
        match col_ix {
            0 => Checkbox::new(("chk", row_ix))
                .checked(row.checked)
                .accessibility_label(row.name.clone())
                .on_click(cx.listener(move |state, checked: &bool, _, cx| {
                    state.delegate_mut().rows[row_ix].checked = *checked;
                    cx.notify();
                }))
                .into_any_element(),
            1 => row.name.clone().into_any_element(),
            2 => row.kind.clone().into_any_element(),
            3 => row.shared_by.clone().into_any_element(),
            _ => row.status.clone().into_any_element(),
        }
    }
}

// ---------- worker thread -> UI ----------

#[derive(Clone, Copy, Default)]
struct NetStats {
    players: u32,
    bytes: u64,
    tick: u64,
}

fn spawn_worker() -> async_channel::Receiver<NetStats> {
    let (tx, rx) = async_channel::unbounded();
    thread::Builder::new()
        .name("net-worker".into())
        .spawn(move || {
            let mut s = NetStats::default();
            loop {
                thread::sleep(Duration::from_secs(1));
                s.tick += 1;
                s.bytes += 900_000 + (s.tick * 7919 % 600_000);
                s.players = 1 + (s.tick / 4 % 4) as u32;
                if tx.send_blocking(s).is_err() {
                    break; // UI is gone
                }
            }
        })
        .expect("spawn worker");
    rx
}

// ---------- main view ----------

struct SpikeApp {
    code: Entity<InputState>,
    table: Entity<TableState<ModsDelegate>>,
    session: Option<SharedString>,
    stats: NetStats,
    _subs: Vec<Subscription>,
    _stats_task: Task<()>,
}

impl SpikeApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let code = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Session code, e.g. K7R-M2Q")
                .default_value("K7R-M2Q")
        });
        let table = cx.new(|cx| {
            TableState::new(ModsDelegate { rows: fake_rows(60) }, window, cx).row_selectable(true)
        });
        let sub = cx.subscribe_in(&code, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.start(window, cx);
            }
        });

        let rx = spawn_worker();
        let stats_task = cx.spawn(async move |this, cx| {
            while let Ok(stats) = rx.recv().await {
                let alive = this.update(cx, |this, cx| {
                    this.stats = stats;
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });

        Self {
            code,
            table,
            session: None,
            stats: NetStats::default(),
            _subs: vec![sub],
            _stats_task: stats_task,
        }
    }

    fn start(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let code = self.code.read(cx).value();
        self.session = Some(code);
        cx.notify();
    }
}

impl Render for SpikeApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.table.read(cx).delegate().rows.iter().filter(|r| r.checked).count();
        let mb = self.stats.bytes as f64 / 1_000_000.0;
        let session = match &self.session {
            Some(code) => format!("Joined session {code}"),
            None => "Not in a session".to_string(),
        };

        v_flex()
            .size_full()
            .gap_3()
            .p_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(260.)).child(Input::new(&self.code)))
                    .child(
                        Button::new("start")
                            .primary()
                            .label("Start")
                            .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
                    )
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child(session)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(DataTable::new(&self.table).stripe(true).bordered(true)),
            )
            .child(
                h_flex()
                    .justify_between()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "{} players \u{b7} {:.1} MB received \u{b7} tick {}",
                        self.stats.players, mb, self.stats.tick
                    ))
                    .child(format!("{selected}/60 mods selected")),
            )
    }
}

fn main() {
    velopack::VelopackApp::build().run();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(920.), px(640.)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("gpui spike".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| SpikeApp::new(window, cx)),
            )
            .expect("failed to open window");
        });
}
