mod cmd_log;
mod config;
mod gh;
mod git;
mod theme;
mod ui;
mod watcher;

use gpui_kit::assets::AllAssets;
use gpui_kit::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use std::path::PathBuf;

fn main() {
    let initial_repo = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .and_then(|p| git::resolve_repo_root(&p).ok());

    gpui_kit::application()
        .with_assets(AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::apply_github_dark_theme(cx);
            ui::register_keybindings(cx);

            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };

            let repo_arg = initial_repo.clone();
            gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("Git Out");
                cx.new(|cx| ui::GitOutApp::new(repo_arg.clone(), window, cx))
            })
            .expect("failed to open Git Out window");
        });
}
