// A release build opens no console window on Windows, and a dev build keeps one for its output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

struct MainView;

impl Render for MainView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x15171b))
    }
}

fn main() {
    // Velopack's install, update and uninstall hooks run here and may exit the process,
    // so this comes before anything else.
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

            let bounds = Bounds::centered(None, size(px(1120.), px(700.)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Cyberpunk Co-op Tools".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| MainView),
            )
            .expect("the main window should open");
        });
}
