mod app;
mod backend;
mod detail;
#[cfg(feature = "dev-script")]
mod dev;
mod navigator;
mod state;
mod theme;
mod transfer;
mod ui;

use gpui::{
    App, Bounds, Menu, MenuItem, QuitMode, TitlebarOptions, WindowBounds, WindowOptions, point,
    prelude::*, px, size,
};

use crate::app::AppView;
use crate::theme::Theme;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    backend::init();

    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            cx.set_global(Theme::for_appearance(cx.window_appearance(), cx));
            ui::text_input::bind_keys(cx);
            app::bind_keys(cx);
            cx.on_action(|_: &app::Quit, cx| cx.quit());
            cx.set_menus([Menu::new("GitButler").items([
                MenuItem::action("Add Project…", app::AddProject),
                MenuItem::action("Reload", app::Refresh),
                MenuItem::separator(),
                MenuItem::action("Quit GitButler", app::Quit),
            ])]);

            let bounds = Bounds::centered(None, size(px(1360.), px(860.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some("GitButler".into()),
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(14.), px(15.))),
                        }),
                        window_min_size: Some(size(px(900.), px(500.))),
                        ..Default::default()
                    },
                    |window, cx| cx.new(|cx| AppView::new(window, cx)),
                )
                .expect("the main window opens");
            #[cfg(feature = "dev-script")]
            dev::run_from_env(window.into(), cx);
            #[cfg(not(feature = "dev-script"))]
            let _ = window;
            cx.activate(true);
        });
}
