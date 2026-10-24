//! An experimental native GitButler client built on GPUI, for macOS, Windows and Linux.
// Built into the desktop app, this runs as GitButler itself, so a panic here quits GitButler:
// failures are handled rather than unwrapped, and a lint is set aside with `#[expect]`, which
// fails the build once it's needless. A task let go of with `let _` is cancelled, and each
// `unsafe` block says why it's sound.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::allow_attributes,
    clippy::dbg_macro,
    clippy::let_underscore_must_use,
    clippy::undocumented_unsafe_blocks
)]
// Indexes and byte offsets go through `get`, `split_once` and the like: what's drawn skips what's
// gone, as a row a reload just took away, and the rest returns early rather than make up a value.
#![cfg_attr(not(test), deny(clippy::indexing_slicing, clippy::string_slice))]

mod app;
mod backend;
mod detail;
#[cfg(feature = "dev-script")]
mod dev;
mod edits;
mod help;
mod navigator;
mod open_in;
mod overlays;
mod palette;
mod pr;
mod projects;
mod settings;
mod state;
mod status_bar;
mod theme;
mod transfer;
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

/// What the desktop app's binary gives the client when it starts it instead of its web one.
#[derive(Clone)]
pub struct Embedded {
    /// The desktop app's version, as its updates count it.
    pub version: String,
    /// The desktop app's update feed for this build and version, which answers with a newer
    /// version or with nothing; `None` when the build doesn't look for updates.
    pub update_feed: Option<String>,
    /// How often to ask the feed.
    pub check_every: std::time::Duration,
}

impl gpui::Global for Embedded {}

/// Set when the client relaunches the desktop app to install an update: that launch opens
/// the web app, which installs it, and takes the variable out before its own relaunch.
pub const WEB_APP_ONCE: &str = "GITBUTLER_WEB_APP_ONCE";

/// Give the Dock the desktop app's icon for this build's channel, as its bundle has it. A
/// binary run on its own has no bundle to carry one, so the Dock would show a terminal's.
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use but_path::AppChannel;
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSData;

    let icns: &[u8] = match AppChannel::new() {
        AppChannel::Release => include_bytes!("../../gitbutler-tauri/icons/release/icon.icns"),
        AppChannel::Nightly => include_bytes!("../../gitbutler-tauri/icons/nightly/icon.icns"),
        AppChannel::Dev => include_bytes!("../../gitbutler-tauri/icons/dev/icon.icns"),
    };
    let icns = NSData::with_bytes(icns);
    // SAFETY: AppKit's documented `NSImage` and `NSApplication` messages, sent on the main
    // thread once the application exists, with the argument types they take.
    unsafe {
        let alloc: Allocated<AnyObject> = msg_send![class!(NSImage), alloc];
        let image: Option<Retained<AnyObject>> = msg_send![alloc, initWithData: &*icns];
        let Some(image) = image else {
            return;
        };
        let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![&*app, setApplicationIconImage: &*image];
    }
}

/// Run the app until it quits: open its window on this thread, which must be the process's
/// main thread, as macOS draws windows from there. `embedded` when the desktop app's binary
/// starts it.
pub fn run(embedded: Option<Embedded>) {
    #[cfg(feature = "dev-script")]
    dev::mark_launch();
    use gpui::{
        App, Bounds, Menu, MenuItem, QuitMode, SystemMenuType, TitlebarOptions, WindowBounds,
        WindowDecorations, WindowOptions, point, prelude::*, size,
    };

    use crate::app::AppView;
    use crate::ui::fixed;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    backend::init();
    #[cfg(target_os = "macos")]
    draw_text_unsmoothed();

    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            // Without them, text falls back to the system's fonts.
            if let Err(err) = cx.text_system().add_fonts(ui::fonts()) {
                tracing::error!("couldn't load the bundled fonts: {err:#}");
            }
            let has_update_feed = embedded.as_ref().is_some_and(|e| e.update_feed.is_some());
            match embedded {
                Some(embedded) => cx.set_global(embedded),
                // The desktop app's bundle gives the Dock its icon.
                #[cfg(target_os = "macos")]
                None => set_dock_icon(),
                #[cfg(not(target_os = "macos"))]
                None => {}
            }
            // GPUI has no alerts of its own on Linux, and its stand-in cuts long text off.
            #[cfg(target_os = "linux")]
            cx.set_prompt_builder(ui::alert::alert);
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
            let mut app_menu = vec![MenuItem::action("About GitButler", help::About)];
            if has_update_feed {
                app_menu.push(MenuItem::action("Check for Updates…", app::CheckForUpdates));
            }
            cx.set_menus([
                Menu::new("GitButler").items(app_menu.into_iter().chain([
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
                ])),
                Menu::new("File").items([
                    MenuItem::action("Add Project…", app::AddProject),
                    MenuItem::action("Clone Repository…", projects::CloneRepository),
                ]),
                Menu::new("View").items([
                    MenuItem::action("Actual Size", app::ActualSize),
                    MenuItem::action("Zoom In", app::ZoomIn),
                    MenuItem::action("Zoom Out", app::ZoomOut),
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

            // Before the window, which opens where the saved state says.
            let store = cx.new(state::Store::new);
            let (display_id, bounds) = match store.read(cx).window_place(cx) {
                Some((display, bounds)) => (Some(display), bounds),
                None => (
                    None,
                    Bounds::centered(None, size(fixed(1360.), fixed(860.)), cx),
                ),
            };
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    display_id,
                    titlebar: Some(TitlebarOptions {
                        title: Some("GitButler".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(fixed(14.), fixed(15.))),
                    }),
                    window_min_size: Some(size(fixed(900.), fixed(500.))),
                    // Linux only. The app draws the frame itself, as it has no title bar
                    // elsewhere either, and the window goes with the desktop app's launcher,
                    // which names the executable as its window class.
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: std::env::current_exe()
                        .ok()
                        .and_then(|exe| Some(exe.file_name()?.to_str()?.to_owned())),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| AppView::new(store, window, cx)),
            );
            let window = match window {
                Ok(window) => window,
                Err(err) => {
                    tracing::error!("couldn't open the window: {err:#}");
                    return cx.quit();
                }
            };
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
