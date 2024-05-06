// Importing required libraries and modules

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

// Command to set menu item enabled or disabled
#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub async fn menu_item_set_enabled(
    handle: AppHandle,
    menu_item_id: &str,
    enabled: bool,
) -> Result<(), Error> {
    // Retrieve the main window from the app handle
    let window = handle
        .get_window("main")
        .expect("main window always present");

    // Get the specified menu item from the menu handle
    let menu_item = window
        .menu_handle()
        .try_get_item(menu_item_id)
        .with_context(|| {
            error::Context::new(Code::Menu, format!("menu item not found: {}", menu_item_id))
        })?;

    // Set the enabled state of the menu item
    menu_item.set_enabled(enabled).context(Code::Unknown)?;

    Ok(())
}

// Function to build the main application menu
pub fn build(_package_info: &PackageInfo) -> Menu {
    // Start with an empty menu
    let mut menu = Menu::new();

    // macOS-specific application submenu
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
                .add_native_item(MenuItem::Services)
                .add_native_item(MenuItem::Separator)
                .add_native_item(MenuItem::Hide)
                .add_native_item(MenuItem::HideOthers)
                .add_native_item(MenuItem::ShowAll)
                .add_native_item(MenuItem::Separator)
                .add_native_item(MenuItem::Quit),
        ));
    }

    // File submenu
    let mut file_menu = Menu::new();
    file_menu = file_menu.add_native_item(MenuItem::CloseWindow);

    #[cfg(not(target_os = "macos"))]
    {
        file_menu = file_menu.add_native_item(MenuItem::Quit);
    }

    menu = menu.add_submenu(Submenu::new("File", file_menu));

    // Edit submenu (excluding Linux platforms)
    #[cfg(not(target_os = "linux"))]
    let mut edit_menu = Menu::new();

    #[cfg(target_os = "macos")]
    {
        // Common edit operations on macOS
        edit_menu = edit_menu.add_native_item(MenuItem::Undo);
        edit_menu = edit_menu.add_native_item(MenuItem::Redo);
        edit_menu = edit_menu.add_native_item(MenuItem::Separator);
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Common edit operations for other platforms
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
        // Add the edit submenu to the main menu
        menu = menu.add_submenu(Submenu::new("Edit", edit_menu));
    }

    // View submenu (contains developer tools for debugging or devtools feature)
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

    // Project submenu (with a disabled Project Settings item)
    let mut project_menu = Menu::new();
    project_menu =
        project_menu.add_item(disabled_menu_item("project/settings", "Project Settings"));
    menu = menu.add_submenu(Submenu::new("Project", project_menu));

    // Window submenu for common window operations
    let mut window_menu = Menu::new();
    window_menu = window_menu.add_native_item(MenuItem::Minimize);

    #[cfg(target_os = "macos")]
    {
        window_menu = window_menu.add_native_item(MenuItem::Zoom);
        window_menu = window_menu.add_native_item(MenuItem::Separator);
    }

    window_menu = window_menu.add_native_item(MenuItem::CloseWindow);
    menu = menu.add_submenu(Submenu::new("Window", window_menu));

    // Help submenu
    let mut help_menu = Menu::new();
    // Adding a few common items to the Help menu
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
    // add version info
    help_menu = help_menu.add_item(disabled_menu_item(
        "help/version",
        format!("Version {}", _package_info.version).as_str(),
    ));
    // add as a submenu
    menu = menu.add_submenu(Submenu::new("Help", help_menu));

    menu
}

// Helper function to create a disabled menu item
fn disabled_menu_item(id: &str, title: &str) -> CustomMenuItem {
    let mut item = CustomMenuItem::new(id, title);
    item.enabled = false;
    item
}

// Handle menu events and trigger corresponding actions
pub fn handle_event<R: Runtime>(event: &WindowMenuEvent<R>) {
    // DEVELOPER TOOLS MENU ITEM
    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        if event.menu_item_id() == "view/devtools" {
            event.window().open_devtools();
        }
    }

    // HELP MENU ITEMS
    if event.menu_item_id() == "help/documentation" {
        // Open the documentation URL in the default browser
        if let Err(err) = open::that("https://docs.gitbutler.com") {
            tracing::error!(error = ?err, "failed to open documentation URL");
        }
    }
    if event.menu_item_id() == "help/github" {
        // Open the GitHub repository URL in the default browser
        if let Err(err) = open::that("https://github.com/gitbutlerapp/gitbutler") {
            tracing::error!(error = ?err, "failed to open GitHub URL");
        }
    }
    if event.menu_item_id() == "help/release-notes" {
        // Open the release notes URL in the default browser
        if let Err(err) =
            open::that("https://discord.com/channels/1060193121130000425/1183737922785116161")
        {
            tracing::error!(error = ?err, "failed to open release notes URL");
        }
    }
    if event.menu_item_id() == "help/report-issue" {
        // Open the issue reporting URL in the default browser
        if let Err(err) = open::that("https://github.com/gitbutlerapp/gitbutler/issues/new") {
            tracing::error!(error = ?err, "failed to open issue reporting URL");
        }
    }
    if event.menu_item_id() == "help/discord" {
        // Open the Discord invite URL in the default browser
        if let Err(err) = open::that("https://discord.com/invite/MmFkmaJ42D") {
            tracing::error!(error = ?err, "failed to open Discord invite URL");
        }
    }
    if event.menu_item_id() == "help/youtube" {
        // Open the YouTube channel URL in the default browser
        if let Err(err) = open::that("https://www.youtube.com/@gitbutlerapp") {
            tracing::error!(error = ?err, "failed to open YouTube channel URL");
        }
    }
    if event.menu_item_id() == "help/x" {
        // Open the Twitter profile URL in the default browser
        if let Err(err) = open::that("https://x.com/gitbutler") {
            tracing::error!(error = ?err, "failed to open Twitter profile URL");
        }
    }

    // Emit event based on the menu item clicked
    emit(
        event.window(),
        format!("menu://{}/clicked", event.menu_item_id()).as_str(),
    );
}

// Emit a custom event to the given window
fn emit<R: Runtime>(window: &tauri::Window<R>, event: &str) {
    if let Err(err) = window.emit(event, json!({})) {
        tracing::error!(error = ?err, "failed to emit event");
    }
}
