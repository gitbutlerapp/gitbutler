//! An experimental native GitButler client built on GPUI. Only macOS builds it for now;
//! elsewhere GPUI needs system libraries the workspace's CI doesn't install.

#[cfg(target_os = "macos")]
mod app;
#[cfg(target_os = "macos")]
mod backend;
#[cfg(target_os = "macos")]
mod detail;
#[cfg(all(target_os = "macos", feature = "dev-script"))]
mod dev;
mod edits;
#[cfg(target_os = "macos")]
mod navigator;
#[cfg(target_os = "macos")]
mod palette;
#[cfg(target_os = "macos")]
mod state;
#[cfg(target_os = "macos")]
mod status_bar;
#[cfg(target_os = "macos")]
mod theme;
#[cfg(target_os = "macos")]
mod transfer;
#[cfg(target_os = "macos")]
mod ui;

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The GPUI client only builds on macOS for now.");
}

#[cfg(target_os = "macos")]
fn main() {
    #[cfg(feature = "dev-script")]
    dev::mark_launch();
    use gpui::{
        App, Bounds, Menu, MenuItem, QuitMode, SystemMenuType, TitlebarOptions, WindowBounds,
        WindowOptions, point, prelude::*, px, size,
    };

    use crate::app::AppView;
    use crate::theme::Theme;

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
            cx.text_system()
                .add_fonts(ui::fonts())
                .expect("the bundled fonts load");
            cx.set_global(Theme::for_appearance(cx.window_appearance()));
            ui::text_input::bind_keys(cx);
            app::bind_keys(cx);
            edits::bind_keys(cx);
            detail::bind_keys(cx);
            palette::bind_keys(cx);
            cx.on_action(|_: &app::Quit, cx| cx.quit());
            cx.on_action(|_: &app::Hide, cx| cx.hide());
            cx.on_action(|_: &app::HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &app::ShowAll, cx| cx.unhide_other_apps());
            // The standard macOS menus. AppKit fills a menu named "Window" with the window
            // list and tiling, and "Services" with what other apps offer.
            cx.set_menus([
                Menu::new("GitButler").items([
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Hide GitButler", app::Hide),
                    MenuItem::action("Hide Others", app::HideOthers),
                    MenuItem::action("Show All", app::ShowAll),
                    MenuItem::separator(),
                    MenuItem::action("Quit GitButler", app::Quit),
                ]),
                Menu::new("File").items([
                    MenuItem::action("Add Project…", app::AddProject),
                    MenuItem::action("Reload", app::Refresh),
                ]),
                Menu::new("Window").items([
                    MenuItem::action("Minimize", app::Minimize),
                    MenuItem::action("Zoom", app::Zoom),
                ]),
            ]);

            let (display_id, bounds) = match state::saved_window_place(cx) {
                Some((display, bounds)) => (Some(display), bounds),
                None => (None, Bounds::centered(None, size(px(1360.), px(860.)), cx)),
            };
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        display_id,
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
