use anyhow::Context;
use gitbutler_core::error;
use gitbutler_core::error::Code;
use serde_json::json;
use tauri::{
    AppHandle, CustomMenuItem, Manager, Menu, MenuEntry, PackageInfo, Runtime, Submenu,
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
        .with_context(|| {
            error::Context::new(Code::Menu, format!("menu item not found: {}", menu_item_id))
        })?;
    menu_item.set_enabled(enabled).context(Code::Unknown)?;
    Ok(())
}

pub fn build(package_info: &PackageInfo) -> Menu {
    #[allow(unused_mut)]
    let mut menu = Menu::os_default(&package_info.name).add_submenu(Submenu::new(
        "Project",
        Menu::with_items([disabled_menu_item("project/settings", "Project Settings")]),
    ));

    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        // Try to find the View menu and attach the dev tools item
        let view_menu = menu.items.iter_mut().find(|item| match item {
            MenuEntry::CustomItem(_) => false,
            MenuEntry::Submenu(submenu) => submenu.title == "View",
            MenuEntry::NativeItem(_) => false,
        });

        let devtools = CustomMenuItem::new("view/devtools", "Developer Tools");
        if let Some(MenuEntry::Submenu(view_menu)) = view_menu {
            view_menu.inner.items.push(devtools.into());
        } else {
            menu = menu.add_submenu(Submenu::new(
                "Developer",
                Menu::with_items([devtools.into()]),
            ));
        }
    }

    menu
}

fn disabled_menu_item(id: &str, title: &str) -> MenuEntry {
    let mut item = CustomMenuItem::new(id, title);
    item.enabled = false;
    item.into()
}

pub fn handle_event<R: Runtime>(event: &WindowMenuEvent<R>) {
    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        if event.menu_item_id() == "view/devtools" {
            event.window().open_devtools();
        }
    }
    emit(
        event.window(),
        format!("menu://{}/clicked", event.menu_item_id()).as_str(),
    );
}

fn emit<R: Runtime>(window: &tauri::Window<R>, event: &str) {
    if let Err(err) = window.emit(event, json!({})) {
        tracing::error!(error = ?err, "failed to emit event");
    }
}
