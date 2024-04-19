use anyhow::Context;
use gitbutler_core::error;
use gitbutler_core::error::Code;
use serde_json::json;
use tauri::{
    AboutMetadata, AppHandle, CustomMenuItem, Manager, Menu, MenuItem, PackageInfo, Runtime,
    Submenu, WindowMenuEvent,
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
    let app_name = &package_info.name;

    let mut menu = Menu::new();
    #[cfg(target_os = "macos")]
    {
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

    let mut file_menu = Menu::new();
    file_menu = file_menu.add_native_item(MenuItem::CloseWindow);
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
    project_menu =
        project_menu.add_item(disabled_menu_item("project/settings", "Project Settings"));
    menu = menu.add_submenu(Submenu::new("Project", project_menu));

    let mut window_menu = Menu::new();
    window_menu = window_menu.add_native_item(MenuItem::Minimize);
    #[cfg(target_os = "macos")]
    {
        window_menu = window_menu.add_native_item(MenuItem::Zoom);
        window_menu = window_menu.add_native_item(MenuItem::Separator);
    }
    window_menu = window_menu.add_native_item(MenuItem::CloseWindow);
    menu = menu.add_submenu(Submenu::new("Window", window_menu));

    menu

    //    #[allow(unused_mut)]
    //    let mut menu = Menu::os_default(&package_info.name)
    //
    //    #[cfg(any(debug_assertions, feature = "devtools"))]
    //    {
    //        // Try to find the View menu and attach the dev tools item
    //        let view_menu = menu.items.iter_mut().find(|item| match item {
    //            MenuEntry::CustomItem(_) => false,
    //            MenuEntry::Submenu(submenu) => submenu.title == "View",
    //            MenuEntry::NativeItem(_) => false,
    //        });
    //
    //        let devtools = CustomMenuItem::new("view/devtools", "Developer Tools");
    //        if let Some(MenuEntry::Submenu(view_menu)) = view_menu {
    //            view_menu.inner.items.push(devtools.into());
    //        } else {
    //            menu = menu.add_submenu(Submenu::new(
    //                "Developer",
    //                Menu::with_items([devtools.into()]),
    //            ));
    //        }
    //    }
    //
    //    menu
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
