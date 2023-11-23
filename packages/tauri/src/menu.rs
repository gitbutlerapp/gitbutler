use serde_json::json;
use tauri::{
    AppHandle, CustomMenuItem, Manager, Menu, MenuEntry, PackageInfo, Runtime, Submenu,
    WindowMenuEvent,
};
use tracing::instrument;

use crate::error::{Code, Error};

#[tauri::command(async)]
#[instrument(skip(handle))]
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
        .ok_or_else(|| Error::UserError {
            message: format!("menu item not found: {}", menu_item_id),
            code: Code::Menu,
        })?;
    menu_item.set_enabled(enabled).map_err(|error| {
        tracing::error!(error = ?error, "failed to set menu item enabled state");
        Error::Unknown
    })?;
    Ok(())
}

pub fn build(package_info: &PackageInfo) -> Menu {
    Menu::os_default(&package_info.name).add_submenu(Submenu::new(
        "Project",
        Menu::with_items([disabled_menu_item("project/settings", "Project Settings")]),
    ))
}

fn disabled_menu_item(id: &str, title: &str) -> MenuEntry {
    let mut item = CustomMenuItem::new(id, title);
    item.enabled = false;
    item.into()
}

pub fn handle_event<R: Runtime>(event: &WindowMenuEvent<R>) {
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
