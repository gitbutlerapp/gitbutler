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
#[expect(clippy::disallowed_methods, reason = "its work runs off the UI thread")]
mod backend;
mod detail;
#[cfg(feature = "dev-script")]
#[expect(
    clippy::disallowed_methods,
    reason = "a development tool, driving the app as a user"
)]
mod dev;
mod edits;
mod fps;
mod help;
mod keymap;
mod navigator;
mod open_in;
mod overlays;
mod palette;
mod pr;
mod projects;
mod rebuild;
mod settings;
mod state;
mod status_bar;
mod theme;
mod transfer;
mod ui;
mod whats_new;

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

/// Set when the client's window can't open and it starts the desktop app's binary again:
/// that launch opens the web app, and takes the variable out before its own relaunches.
pub const WEB_APP_ONCE: &str = "GITBUTLER_WEB_APP_ONCE";

/// Set when the client starts the desktop app's binary to install the update its feed offers:
/// that run opens no window, installs it with the web app's updater, and writes how much it
/// has downloaded, in percent, a line each time it changes.
pub const INSTALL_UPDATE: &str = "GITBUTLER_INSTALL_UPDATE";

/// The desktop app's binary, to start again. On Linux an AppImage's own file: this one is
/// inside it, from a mount that goes once this process ends.
fn this_app() -> std::io::Result<std::path::PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(appimage) => Ok(appimage.into()),
        None => std::env::current_exe(),
    }
}

/// Start the desktop app's binary again, as its web app once with `web_once`.
#[expect(
    clippy::disallowed_methods,
    reason = "called in `backend::run`, or when the window failed to open"
)]
fn relaunch(web_once: bool) -> std::io::Result<()> {
    let mut app = std::process::Command::new(this_app()?);
    if web_once {
        app.env(WEB_APP_ONCE, "1");
    }
    app.spawn()?;
    Ok(())
}

/// Install the update the desktop app's feed offers with a run of its binary, telling
/// `progress` how much it has downloaded, in percent. Waits until it's installed.
#[expect(clippy::disallowed_methods, reason = "called in `backend::run`")]
fn install_update(progress: impl Fn(u8)) -> anyhow::Result<()> {
    use std::io::BufRead as _;
    use std::process::{Command, Stdio};

    let mut install = Command::new(this_app()?);
    // `CREATE_NO_WINDOW`: a debug build is a console program, which would open a console.
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut install, 0x0800_0000);
    let mut install = install
        .env(INSTALL_UPDATE, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(stdout) = install.stdout.take() {
        for line in std::io::BufReader::new(stdout).lines() {
            if let Ok(percent) = line?.trim().parse() {
                progress(percent);
            }
        }
    }
    let output = install.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

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

/// Start the log, before anything that could write to it. The desktop app's binary calls it
/// before its own setup for the client, which logs too.
pub use backend::logs::init as init_logging;

/// Run the app until it quits: open its window on this thread, which must be the process's
/// main thread, as macOS draws windows from there. `embedded` when the desktop app's binary
/// starts it. Call [`init_logging`] first.
pub fn run(embedded: Option<Embedded>) {
    #[cfg(feature = "dev-script")]
    dev::mark_launch();
    use gpui::{
        App, Bounds, Menu, MenuItem, QuitMode, SystemMenuType, TitlebarOptions, WindowBounds,
        WindowDecorations, WindowOptions, point, prelude::*, size,
    };

    use crate::app::AppView;
    use crate::ui::fixed;

    backend::init();
    backend::metrics::start(embedded.as_ref().map(|embedded| embedded.version.clone()));
    // The desktop app writes to the same files, so say which app this is.
    tracing::info!(
        version = embedded.as_ref().map(|embedded| embedded.version.as_str()),
        channel = backend::settings::channel(),
        os = std::env::consts::OS,
        "starting the GPUI client"
    );
    #[cfg(target_os = "macos")]
    draw_text_unsmoothed();

    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            // Before the other quit handlers run, and so outside the little time GPUI gives
            // what they return, as it sends what's queued.
            cx.on_app_quit(|_| {
                backend::metrics::quit();
                async {}
            })
            .detach();
            // GPUI awaits what quit handlers return only once they've all run, the store's
            // save included, so this writes out what they logged.
            cx.on_app_quit(|_| async { backend::logs::flush() })
                .detach();
            // Without them, text falls back to the system's fonts.
            if let Err(err) = cx.text_system().add_fonts(ui::fonts()) {
                tracing::error!("couldn't load the bundled fonts: {err:#}");
            }
            #[cfg(target_os = "linux")]
            ui::prewarm_fonts(cx);
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
            help::register(cx);
            cx.on_action(|_: &app::Hide, cx| cx.hide());
            cx.on_action(|_: &app::HideOthers, cx| cx.hide_other_apps());
            cx.on_action(|_: &app::ShowAll, cx| cx.unhide_other_apps());
            // The standard macOS menus. AppKit fills a menu named "Window" with the window
            // list and tiling, and "Services" with what other apps offer. Set again whenever
            // the keys change, as the menu bar takes them when it's set.
            let set_menus = move |cx: &mut App| {
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
                        MenuItem::separator(),
                        MenuItem::action("Close Window", app::CloseWindow),
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
                        MenuItem::action("What's New", help::OpenReleaseNotes),
                        MenuItem::action("Keyboard Shortcuts", settings::OpenKeyboardShortcuts),
                        MenuItem::action("Open Keymap File", keymap::OpenKeymap),
                        MenuItem::separator(),
                        MenuItem::action("Report an Issue…", help::ReportIssue),
                        MenuItem::action("Show Settings Folder", help::ShowConfigFolder),
                        MenuItem::action("Show Logs Folder", help::ShowLogsFolder),
                    ]),
                ]);
            };

            // A script drives a window no one sees, so its run keeps out of the Dock, the menu
            // bar and the keys of the app in front. Its shots draw it all the same.
            let scripted = app::scripted();
            if scripted {
                cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
            }
            // Before the window, which opens where the saved state says.
            let store = cx.new(state::Store::new);
            keymap::watch(store.clone(), set_menus, cx);
            let default_size = size(fixed(1360.), fixed(860.));
            let (display_id, bounds) = match store.read(cx).window_place(cx) {
                // Never shown (`show` below), and should anything show it anyway, off to the
                // side, where AppKit leaves only an edge on screen.
                _ if scripted => (
                    None,
                    WindowBounds::Windowed(Bounds::new(
                        point(fixed(-10000.), fixed(-10000.)),
                        default_size,
                    )),
                ),
                Some((display, bounds)) => (Some(display), bounds),
                None => (
                    None,
                    WindowBounds::Windowed(Bounds::centered(None, default_size, cx)),
                ),
            };
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(bounds),
                    display_id,
                    titlebar: Some(TitlebarOptions {
                        title: Some("GitButler".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(fixed(14.), fixed(15.))),
                    }),
                    window_min_size: Some(size(fixed(900.), fixed(500.))),
                    focus: !scripted,
                    show: !scripted,
                    // macOS only. The headers drag the window and take its title bar's double
                    // click (`ui::drags_window`), so AppKit leaves the title bar to them, and
                    // doesn't hold each click there back to see whether a second one follows.
                    app_owns_titlebar_drag: true,
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
                    // As when the GPU's drivers can't draw it. Started by the desktop app,
                    // GitButler then opens as its web app: with the setting that opens this
                    // client on, quitting alone would keep it from opening at all.
                    if cx.has_global::<Embedded>() {
                        backend::metrics::fell_back_to_web(&err);
                        if let Err(err) = relaunch(true) {
                            tracing::error!("couldn't open the web app instead: {err:#}");
                        }
                    }
                    // Once the event loop runs: on Linux it only starts after this returns, and
                    // forgets a quit asked for before.
                    cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
                    return;
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
            if !scripted {
                cx.activate(true);
            }
        });
}
