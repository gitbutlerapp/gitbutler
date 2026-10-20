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

/// What the desktop app's binary gives the client when it starts it instead of its web one.
#[cfg(target_os = "macos")]
#[derive(Clone)]
pub struct Embedded {
    /// The desktop app's update feed for this build and version, which answers with a newer
    /// version or with nothing; `None` when the build doesn't look for updates.
    pub update_feed: Option<String>,
    /// How often to ask the feed.
    pub check_every: std::time::Duration,
}

#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
pub fn run(embedded: Option<Embedded>) {
    #[cfg(feature = "dev-script")]
    dev::mark_launch();
    use gpui::{
        App, Bounds, Menu, MenuItem, QuitMode, SystemMenuType, TitlebarOptions, WindowBounds,
        WindowOptions, point, prelude::*, px, size,
    };

    use crate::app::AppView;

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
            match embedded {
                Some(embedded) => cx.set_global(embedded),
                // The desktop app's bundle gives the Dock its icon.
                None => set_dock_icon(),
            }
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
