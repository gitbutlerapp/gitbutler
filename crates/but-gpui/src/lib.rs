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
#[cfg(target_os = "macos")]
mod edits;
#[cfg(target_os = "macos")]
mod help;
#[cfg(target_os = "macos")]
mod navigator;
#[cfg(target_os = "macos")]
mod open_in;
#[cfg(target_os = "macos")]
mod overlays;
#[cfg(target_os = "macos")]
mod palette;
#[cfg(target_os = "macos")]
mod pr;
#[cfg(target_os = "macos")]
mod projects;
#[cfg(target_os = "macos")]
mod settings;
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

/// Draw text as GitButler's web apps do, with `-webkit-font-smoothing: antialiased`. Unless the
/// app's `AppleFontSmoothing` says otherwise, GPUI thickens glyph strokes the way macOS font
/// smoothing would, lighter text more, so the same fonts read heavier here than in but.dev.
/// A registered default lives in memory only and is never written to the app's preferences.
#[cfg(target_os = "macos")]
fn draw_text_unsmoothed() {
    use objc2_foundation::{NSDictionary, NSNumber, NSString, NSUserDefaults};

    let key = NSString::from_str("AppleFontSmoothing");
    let off = NSNumber::numberWithInt(0);
    let defaults =
        NSDictionary::<NSString, objc2::runtime::AnyObject>::from_slices(&[&*key], &[off.as_ref()]);
    // SAFETY: the dictionary maps strings to objects, as `registerDefaults:` takes.
    unsafe { NSUserDefaults::standardUserDefaults().registerDefaults(&defaults) };
}

/// Run the app until it quits: open its window on this thread, which must be the process's
/// main thread, as macOS draws windows from there.
#[cfg(target_os = "macos")]
pub fn run() {
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
    draw_text_unsmoothed();

    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            cx.text_system()
                .add_fonts(ui::fonts())
                .expect("the bundled fonts load");
            cx.set_global(Theme::for_appearance(
                theme::Appearance::System,
                cx.window_appearance(),
                theme::CodeSize::default(),
            ));
            ui::text_input::bind_keys(cx);
            app::bind_keys(cx);
            navigator::bind_keys(cx);
            detail::bind_keys(cx);
            palette::bind_keys(cx);
            settings::bind_keys(cx);
            open_in::bind_keys(cx);
            help::register(cx);
            cx.on_action(|_: &app::Hide, cx| cx.hide());
            cx.on_action(|_: &app::HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &app::ShowAll, cx| cx.unhide_other_apps());
            // The standard macOS menus. AppKit fills a menu named "Window" with the window
            // list and tiling, and "Services" with what other apps offer.
            cx.set_menus([
                Menu::new("GitButler").items([
                    MenuItem::action("About GitButler", help::About),
                    MenuItem::separator(),
                    MenuItem::action("Settings…", settings::OpenSettings),
                    MenuItem::separator(),
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
                    MenuItem::action("Clone Repository…", projects::CloneRepository),
                ]),
                Menu::new("Window").items([
                    MenuItem::action("Minimize", app::Minimize),
                    MenuItem::action("Zoom", app::Zoom),
                ]),
                Menu::new("Help").items([
                    MenuItem::action("GitButler Docs", help::OpenDocs),
                    MenuItem::action("Release Notes", help::OpenReleaseNotes),
                    MenuItem::action("Keyboard Shortcuts", app::CommandPalette),
                    MenuItem::separator(),
                    MenuItem::action("Report an Issue…", help::ReportIssue),
                    MenuItem::action("Show Settings Folder", help::ShowConfigFolder),
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
            // With the window minimized no window is active, so the menu's Quit comes here.
            // It asks the window first, as ⌘Q does.
            cx.on_action(move |_: &app::Quit, cx| {
                window
                    .update(cx, |app, window, cx| app.quit(&app::Quit, window, cx))
                    .unwrap_or_else(|_| cx.quit())
            });
            #[cfg(feature = "dev-script")]
            dev::run_from_env(window.into(), cx);
            cx.activate(true);
        });
}
