use std::{env, fs};

use anyhow::Context;
use gitbutler_core::error;
use gitbutler_core::error::Code;
use serde_json::json;

#[cfg(target_os = "macos")]
use tauri::AboutMetadata;
use tauri::{
    AppHandle, CustomMenuItem, Manager, Menu, MenuItem, PackageInfo, Runtime, Submenu,
    WindowMenuEvent,
};
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn menu_item_set_enabled(
    handle: AppHandle,
    menu_item_id: &str,
    enabled: bool,
) -> Result<(), Error> {
    let window = handle
        .get_window("main")
        .expect("main window always present");

    let menu_item = window
        .menu_handle()
        .try_get_item(menu_item_id)
        .with_context(|| error::Context::new(format!("menu item not found: {}", menu_item_id)))?;

    menu_item.set_enabled(enabled).context(Code::Unknown)?;

    Ok(())
}

#[tauri::command()]
pub fn resolve_vscode_variant() -> &'static str {
    let vscodium_installed = check_if_installed("codium");
    if vscodium_installed {
        "vscodium"
    } else {
        // Fallback to vscode, as it was the previous behavior
        "vscode"
    }
}

fn check_if_installed(executable_name: &str) -> bool {
    match env::var_os("PATH") {
        Some(env_path) => env::split_paths(&env_path).any(|mut path| {
            path.push(executable_name);
            fs::metadata(path).is_ok()
        }),
        None => false,
    }
}

pub fn build(_package_info: &PackageInfo) -> Menu {
    let mut menu = Menu::new();

    #[cfg(target_os = "macos")]
    {
        let app_name = &_package_info.name;

        menu = menu.add_submenu(Submenu::new(
            app_name,
            Menu::new()
                .add_native_item(MenuItem::About(
                    app_name.to_string(),
                    AboutMetadata::default(),
                ))
                .add_native_item(MenuItem::Separator)
                .add_item(CustomMenuItem::new("global/settings", "Settings").accelerator("Cmd+,"))
                .add_native_item(MenuItem::Separator)
                .add_native_item(MenuItem::Services)
                .add_native_item(MenuItem::Separator)
                .add_native_item(MenuItem::Hide)
                .add_native_item(MenuItem::HideOthers)
                .add_native_item(MenuItem::ShowAll)
                .add_native_item(MenuItem::Separator)
                .add_native_item(MenuItem::Quit),
        ));
    }

    let mut file_menu = Menu::new();
    #[cfg(target_os = "macos")]
    {
        // NB: macOS has the concept of having an app running but its
        // window closed, but other platforms do not
        file_menu = file_menu.add_native_item(MenuItem::CloseWindow);
    }
    #[cfg(not(target_os = "macos"))]
    {
        file_menu = file_menu.add_native_item(MenuItem::Quit);
    }

    menu = menu.add_submenu(Submenu::new("File", file_menu));

    #[cfg(not(target_os = "linux"))]
    let mut edit_menu = Menu::new();

    #[cfg(target_os = "macos")]
    {
        edit_menu = edit_menu.add_native_item(MenuItem::Undo);
        edit_menu = edit_menu.add_native_item(MenuItem::Redo);
        edit_menu = edit_menu.add_native_item(MenuItem::Separator);
    }

    #[cfg(not(target_os = "linux"))]
    {
        edit_menu = edit_menu.add_native_item(MenuItem::Cut);
        edit_menu = edit_menu.add_native_item(MenuItem::Copy);
        edit_menu = edit_menu.add_native_item(MenuItem::Paste);
    }

    #[cfg(target_os = "macos")]
    {
        edit_menu = edit_menu.add_native_item(MenuItem::SelectAll);
    }

    #[cfg(not(target_os = "linux"))]
    {
        menu = menu.add_submenu(Submenu::new("Edit", edit_menu));
    }

    let mut view_menu = Menu::new();

    #[cfg(target_os = "macos")]
    {
        view_menu = view_menu.add_native_item(MenuItem::EnterFullScreen);

        #[cfg(any(debug_assertions, feature = "devtools"))]
        {
            view_menu = view_menu.add_native_item(MenuItem::Separator);
        }
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        view_menu = view_menu.add_item(CustomMenuItem::new("view/devtools", "Developer Tools"));
    }

    menu = menu.add_submenu(Submenu::new("View", view_menu));

    let mut project_menu = Menu::new();
    project_menu = project_menu.add_item(
        CustomMenuItem::new("project/history", "Project History").accelerator("CmdOrCtrl+Shift+H"),
    );
    project_menu = project_menu.add_item(CustomMenuItem::new(
        "project/open-in-vscode",
        "Open in VS Code",
    ));

    project_menu = project_menu.add_native_item(MenuItem::Separator);
    project_menu =
        project_menu.add_item(CustomMenuItem::new("project/settings", "Project Settings"));
    menu = menu.add_submenu(Submenu::new("Project", project_menu));

    #[cfg(target_os = "macos")]
    {
        let mut window_menu = Menu::new();
        window_menu = window_menu.add_native_item(MenuItem::Minimize);

        window_menu = window_menu.add_native_item(MenuItem::Zoom);
        window_menu = window_menu.add_native_item(MenuItem::Separator);

        window_menu = window_menu.add_native_item(MenuItem::CloseWindow);
        menu = menu.add_submenu(Submenu::new("Window", window_menu));
    }

    let mut help_menu = Menu::new();
    help_menu = help_menu.add_item(CustomMenuItem::new("help/documentation", "Documentation"));
    help_menu = help_menu.add_item(CustomMenuItem::new("help/github", "Source Code"));
    help_menu = help_menu.add_item(CustomMenuItem::new("help/release-notes", "Release Notes"));
    help_menu = help_menu.add_native_item(MenuItem::Separator);
    help_menu = help_menu.add_item(CustomMenuItem::new(
        "help/share-debug-info",
        "Share Debug Info…",
    ));
    help_menu = help_menu.add_item(CustomMenuItem::new("help/report-issue", "Report an Issue…"));
    help_menu = help_menu.add_native_item(MenuItem::Separator);
    help_menu = help_menu.add_item(CustomMenuItem::new("help/discord", "Discord"));
    help_menu = help_menu.add_item(CustomMenuItem::new("help/youtube", "YouTube"));
    help_menu = help_menu.add_item(CustomMenuItem::new("help/x", "X"));
    help_menu = help_menu.add_native_item(MenuItem::Separator);
    help_menu = help_menu.add_item(disabled_menu_item(
        "help/version",
        &format!("Version {}", _package_info.version),
    ));
    menu = menu.add_submenu(Submenu::new("Help", help_menu));

    menu
}

fn disabled_menu_item(id: &str, title: &str) -> CustomMenuItem {
    let mut item = CustomMenuItem::new(id, title);
    item.enabled = false;
    item
}

pub fn handle_event<R: Runtime>(event: &WindowMenuEvent<R>) {
    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        if event.menu_item_id() == "view/devtools" {
            event.window().open_devtools();
            return;
        }
    }

    if event.menu_item_id() == "help/share-debug-info" {
        emit(event.window(), "menu://help/share-debug-info/clicked");
        return;
    }

    if event.menu_item_id() == "project/history" {
        emit(event.window(), "menu://project/history/clicked");
        return;
    }

    if event.menu_item_id() == "project/open-in-vscode" {
        emit(event.window(), "menu://project/open-in-vscode/clicked");
        return;
    }

    if event.menu_item_id() == "project/settings" {
        emit(event.window(), "menu://project/settings/clicked");
        return;
    }

    if event.menu_item_id() == "global/settings" {
        emit(event.window(), "menu://global/settings/clicked");
        return;
    }

    'open_link: {
        let result = match event.menu_item_id() {
            "help/documentation" => open::that("https://docs.gitbutler.com"),
            "help/github" => open::that("https://github.com/gitbutlerapp/gitbutler"),
            "help/release-notes" => {
                open::that("https://discord.com/channels/1060193121130000425/1183737922785116161")
            }
            "help/report-issue" => {
                open::that("https://github.com/gitbutlerapp/gitbutler/issues/new")
            }
            "help/discord" => open::that("https://discord.com/invite/MmFkmaJ42D"),
            "help/youtube" => open::that("https://www.youtube.com/@gitbutlerapp"),
            "help/x" => open::that("https://x.com/gitbutler"),
            _ => break 'open_link,
        };

        if let Err(err) = result {
            tracing::error!(error = ?err, "failed to open url for {}", event.menu_item_id());
        }

        return;
    }

    tracing::error!("unhandled 'help' menu event: {}", event.menu_item_id());
}

fn emit<R: Runtime>(window: &tauri::Window<R>, event: &str) {
    if let Err(err) = window.emit(event, json!({})) {
        tracing::error!(error = ?err, "failed to emit event");
    }
}
