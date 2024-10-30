use std::{env, fs};

use anyhow::Context;
use gitbutler_error::error::{self, Code};
use serde_json::json;
#[cfg(target_os = "macos")]
use tauri::menu::AboutMetadata;
use tauri::Emitter;
use tauri::{
    menu::{Menu, MenuEvent, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder},
    AppHandle, Manager, Runtime, WebviewWindow,
};
use tracing::instrument;

use crate::error::Error;

#[tauri::command(async)]
#[instrument(skip(handle), err(Debug))]
pub fn menu_item_set_enabled(handle: AppHandle, id: &str, enabled: bool) -> Result<(), Error> {
    let window = handle
        .get_window("main")
        .expect("main window always present");

    let menu_item = window
        .menu()
        .context("menu not found")?
        .get(id)
        .with_context(|| error::Context::new(format!("menu item not found: {}", id)))?;

    menu_item
        .as_menuitem()
        .context(Code::Unknown)?
        .set_enabled(enabled)
        .context(Code::Unknown)?;

    Ok(())
}

#[tauri::command()]
pub fn get_editor_link_scheme() -> &'static str {
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

pub fn build<R: Runtime>(handle: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    let check_for_updates =
        MenuItemBuilder::with_id("global/update", "Check for updates…").build(handle)?;

    #[cfg(target_os = "macos")]
    let app_name = handle
        .config()
        .product_name
        .clone()
        .context("App name not defined.")?;

    #[cfg(target_os = "macos")]
    let mac_menu = &SubmenuBuilder::new(handle, app_name)
        .about(Some(AboutMetadata::default()))
        .separator()
        .text("global/settings", "Settings")
        .item(&check_for_updates)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_menu = &SubmenuBuilder::new(handle, "File")
        .items(&[
            &MenuItemBuilder::with_id("file/add-local-repo", "Add Local Repository…")
                .accelerator("CmdOrCtrl+O")
                .build(handle)?,
            &MenuItemBuilder::with_id("file/clone-repo", "Clone Repository…")
                .accelerator("CmdOrCtrl+Shift+O")
                .build(handle)?,
            &PredefinedMenuItem::separator(handle)?,
        ])
        .build()?;

    #[cfg(target_os = "macos")]
    file_menu.append(&PredefinedMenuItem::close_window(handle, None)?)?;

    #[cfg(not(target_os = "macos"))]
    file_menu.append_items(&[&PredefinedMenuItem::quit(handle, None)?, &check_for_updates])?;

    #[cfg(not(target_os = "linux"))]
    let edit_menu = &Submenu::new(handle, "Edit", true)?;

    #[cfg(target_os = "macos")]
    {
        edit_menu.append_items(&[
            &PredefinedMenuItem::undo(handle, None)?,
            &PredefinedMenuItem::redo(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
        ])?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        edit_menu.append_items(&[
            &PredefinedMenuItem::cut(handle, None)?,
            &PredefinedMenuItem::copy(handle, None)?,
            &PredefinedMenuItem::paste(handle, None)?,
        ])?;
    }

    #[cfg(target_os = "macos")]
    edit_menu.append(&PredefinedMenuItem::select_all(handle, None)?)?;

    let view_menu = &Submenu::new(handle, "View", true)?;

    #[cfg(target_os = "macos")]
    view_menu.append(&PredefinedMenuItem::fullscreen(handle, None)?)?;
    view_menu.append_items(&[
        &MenuItemBuilder::with_id("view/switch-theme", "Switch Theme")
            .accelerator("CmdOrCtrl+T")
            .build(handle)?,
        &PredefinedMenuItem::separator(handle)?,
        &MenuItemBuilder::with_id("view/zoom-in", "Zoom In")
            .accelerator("CmdOrCtrl+=")
            .build(handle)?,
        &MenuItemBuilder::with_id("view/zoom-out", "Zoom Out")
            .accelerator("CmdOrCtrl+-")
            .build(handle)?,
        &MenuItemBuilder::with_id("view/zoom-reset", "Reset Zoom")
            .accelerator("CmdOrCtrl+0")
            .build(handle)?,
        &PredefinedMenuItem::separator(handle)?,
    ])?;

    #[cfg(any(debug_assertions, feature = "devtools"))]
    view_menu.append_items(&[
        &MenuItemBuilder::with_id("view/devtools", "Developer Tools").build(handle)?,
        &MenuItemBuilder::with_id("view/reload", "Reload View")
            .accelerator("CmdOrCtrl+R")
            .build(handle)?,
    ])?;

    let project_menu = &SubmenuBuilder::new(handle, "Project")
        .item(
            &MenuItemBuilder::with_id("project/history", "Project History")
                .accelerator("CmdOrCtrl+Shift+H")
                .build(handle)?,
        )
        .text("project/open-in-vscode", "Open in VS Code")
        .separator()
        .text("project/settings", "Project Settings")
        .build()?;

    #[cfg(target_os = "macos")]
    let window_menu = &SubmenuBuilder::new(handle, "Window")
        .items(&[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::close_window(handle, None)?,
        ])
        .build()?;

    let help_menu = &SubmenuBuilder::new(handle, "Help")
        .text("help/documentation", "Documentation")
        .text("help/github", "Source Code")
        .text("help/release-notes", "Release Notes")
        .separator()
        .text("help/share-debug-info", "Share Debug Info…")
        .text("help/report-issue", "Report an Issue…")
        .separator()
        .text("help/discord", "Discord")
        .text("help/youtube", "YouTube")
        .text("help/x", "X")
        .separator()
        .item(
            &MenuItemBuilder::with_id(
                "help/version",
                format!("Version {}", handle.package_info().version),
            )
            .enabled(false)
            .build(handle)?,
        )
        .build()?;

    Menu::with_items(
        handle,
        &[
            #[cfg(target_os = "macos")]
            mac_menu,
            file_menu,
            #[cfg(not(target_os = "linux"))]
            edit_menu,
            view_menu,
            project_menu,
            #[cfg(target_os = "macos")]
            window_menu,
            help_menu,
        ],
    )
}

pub fn handle_event(webview: &WebviewWindow, event: &MenuEvent) {
    if event.id() == "file/add-local-repo" {
        emit(webview, "menu://file/add-local-repo/clicked");
        return;
    }

    if event.id() == "file/clone-repo" {
        emit(webview, "menu://file/clone-repo/clicked");
        return;
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        if event.id() == "view/devtools" {
            webview.open_devtools();
            return;
        }
    }

    if event.id() == "view/switch-theme" {
        emit(webview, "menu://view/switch-theme/clicked");
        return;
    }

    if event.id() == "view/reload" {
        emit(webview, "menu://view/reload/clicked");
        return;
    }

    if event.id() == "view/zoom-in" {
        emit(webview, "menu://view/zoom-in/clicked");
        return;
    }

    if event.id() == "view/zoom-out" {
        emit(webview, "menu://view/zoom-out/clicked");
        return;
    }

    if event.id() == "view/zoom-reset" {
        emit(webview, "menu://view/zoom-reset/clicked");
        return;
    }

    if event.id() == "help/share-debug-info" {
        emit(webview, "menu://help/share-debug-info/clicked");
        return;
    }

    if event.id() == "project/history" {
        emit(webview, "menu://project/history/clicked");
        return;
    }

    if event.id() == "project/open-in-vscode" {
        emit(webview, "menu://project/open-in-vscode/clicked");
        return;
    }

    if event.id() == "project/settings" {
        emit(webview, "menu://project/settings/clicked");
        return;
    }

    if event.id() == "global/settings" {
        emit(webview, "menu://global/settings/clicked");
        return;
    }

    if event.id() == "global/update" {
        emit(webview, "menu://global/update/clicked");
        return;
    }

    'open_link: {
        let result = match event.id().0.as_str() {
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
            tracing::error!(error = ?err, "failed to open url for {}", event.id().0);
        }

        return;
    }

    tracing::error!("unhandled 'help' menu event: {}", event.id().0);
}

fn emit<R: Runtime>(window: &tauri::WebviewWindow<R>, event: &str) {
    if let Err(err) = window.emit(event, json!({})) {
        tracing::error!(error = ?err, "failed to emit event");
    }
}
