// A release build opens no console window on Windows, and a dev build keeps one for its output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod session;
mod settings;
mod setup;
mod theme;
mod ui;
mod view;

use std::path::PathBuf;
use std::process::ExitCode;

use gpui_kit::component::{Theme, ThemeMode, TitleBar};
use gpui_kit::*;

const USAGE: &str = "\
Usage: cyberpunk-coop-tools [options]

  --data-dir <folder>   keep settings, the app's log, and backups in this folder
  --demo <screen>       show a made-up session: host, tester, setup-join, or setup-host
  --name <name>         show up as this name, for test runs of several copies on one PC
  --local-app-data <folder>
                        read this folder as the game's %LOCALAPPDATA%, for test runs
  --help                show this
";

struct Args {
    data_dir: Option<PathBuf>,
    demo: Option<coop_core::DemoScreen>,
    name: Option<String>,
    local_app_data: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { data_dir: None, demo: None, name: None, local_app_data: None };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--data-dir" => args.data_dir = Some(it.next().ok_or("--data-dir needs a folder")?.into()),
            "--demo" => args.demo = Some(it.next().ok_or("--demo needs a screen")?.parse()?),
            "--name" => args.name = Some(it.next().ok_or("--name needs a name")?),
            "--local-app-data" => args.local_app_data = Some(it.next().ok_or("--local-app-data needs a folder")?.into()),
            "--help" | "-h" => return Err(String::new()),
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    Ok(args)
}

fn main() -> ExitCode {
    // Velopack's install, update and uninstall hooks run here and may exit the process,
    // so this comes before anything else.
    velopack::VelopackApp::build().run();

    let args = match parse_args() {
        Ok(args) => args,
        Err(problem) => {
            if !problem.is_empty() {
                eprintln!("{problem}\n");
            }
            eprint!("{USAGE}");
            return if problem.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(2) };
        }
    };
    let (commands, snapshots) = coop_core::start(coop_core::Options {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: args.data_dir,
        demo: args.demo,
        name: args.name,
        local_app_data: args.local_app_data,
    });

    gpui_kit::application().with_assets(assets::Assets).run(move |cx: &mut App| {
        gpui_kit::init(cx);
        assets::load_fonts(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        theme::apply(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1120.), px(700.)), cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(760.), px(480.))),
                titlebar: Some(TitlebarOptions {
                    // The title the taskbar shows, and tools/capture-window.ps1 looks for.
                    title: Some("Cyberpunk Co-op Tools".into()),
                    ..TitleBar::title_bar_options()
                }),
                ..TitleBar::window_options()
            },
            cx,
            |window, cx| cx.new(|cx| view::AppView::new(commands, snapshots, window, cx)),
        )
        .expect("the main window should open");
    });
    ExitCode::SUCCESS
}
