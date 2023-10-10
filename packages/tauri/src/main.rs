use anyhow::Context;
use futures::executor::block_on;
use tauri::{generate_context, Manager};

use gitbutler::{zip, *};

fn main() {
    let tauri_context = generate_context!();

    let _guard = sentry::init(("https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032", sentry::ClientOptions {
        release: Some(tauri_context.package_info().version.to_string().into()),
        environment: Some(match tauri_context.package_info().name.as_str() {
            "GitButler" => "production",
            "GitButler Nightly" => "nightly",
            "GitButler Dev" => "development",
            _ => "unknown",
        }.into()),
        attach_stacktrace: true,
        default_integrations: true,
        ..Default::default()
    }));

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tauri::async_runtime::set(tokio::runtime::Handle::current());
            let app_title = tauri_context.package_info().name.clone();

            let quit = tauri::CustomMenuItem::new("quit".to_string(), "Quit");
            let hide =
                tauri::CustomMenuItem::new("toggle".to_string(), format!("Hide {}", app_title));
            let tray_menu = tauri::SystemTrayMenu::new().add_item(hide).add_item(quit);
            let tray = tauri::SystemTray::new().with_menu(tray_menu);

            tauri::Builder::default()
                .system_tray(tray)
                .on_system_tray_event(|app_handle, event| {
                    if let tauri::SystemTrayEvent::MenuItemClick { id, .. } = event {
                        let app_title = app_handle.package_info().name.clone();
                        let item_handle = app_handle.tray_handle().get_item(&id);
                        match id.as_str() {
                            "quit" => {
                                tracing::info!("Quitting app");
                                app_handle.exit(0);
                            }
                            "toggle" => match get_window(app_handle) {
                                Some(window) => {
                                    if window.is_visible().unwrap() {
                                        hide_window(app_handle).expect("Failed to hide window");
                                    } else {
                                        show_window(app_handle).expect("Failed to show window");
                                    }
                                }
                                None => {
                                    create_window(app_handle).expect("Failed to create window");
                                    item_handle
                                        .set_title(format!("Hide {}", app_title))
                                        .unwrap();
                                }
                            },
                            _ => {}
                        }
                    }
                })
                .on_window_event(|event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                        hide_window(&event.window().app_handle()).expect("Failed to hide window");
                        api.prevent_close();
                    }
                })
                .setup(move |tauri_app| {
                    let window =
                        create_window(&tauri_app.handle()).expect("Failed to create window");
                    #[cfg(debug_assertions)]
                    window.open_devtools();

                    let app_handle = tauri_app.handle();

                    logs::init(&app_handle);

                    tracing::info!("Starting app");

                    let analytics_cfg = if cfg!(debug_assertions) {
                        analytics::Config {
                            posthog_token: Some("phc_t7VDC9pQELnYep9IiDTxrq2HLseY5wyT7pn0EpHM7rr"),
                        }
                    } else {
                        analytics::Config {
                            posthog_token: Some("phc_yJx46mXv6kA5KTuM2eEQ6IwNTgl5YW3feKV5gi7mfGG"),
                        }
                    };
                    let analytics_client = analytics::Client::new(&app_handle, analytics_cfg);
                    tauri_app.manage(analytics_client);

                    let watchers = watcher::Watchers::try_from(&app_handle)
                        .expect("failed to initialize watchers");
                    tauri_app.manage(watchers);

                    let proxy =
                        assets::Proxy::try_from(&app_handle).expect("failed to initialize proxy");
                    tauri_app.manage(proxy);

                    let database = database::Database::try_from(&app_handle)
                        .expect("failed to initialize database");
                    app_handle.manage(database);

                    let storage = storage::Storage::try_from(&app_handle)
                        .expect("failed to initialize storage");
                    app_handle.manage(storage);

                    let zipper = zip::Controller::try_from(&app_handle)
                        .expect("failed to initialize zipc controller ");
                    tauri_app.manage(zipper);

                    let search = search::Searcher::try_from(&app_handle)
                        .expect("failed to initialize search");
                    app_handle.manage(search);

                    let projects_controller = projects::Controller::try_from(&app_handle)
                        .expect("failed to initialize projects controller");
                    app_handle.manage(projects_controller);

                    let vbranch_contoller =
                        virtual_branches::controller::Controller::try_from(&app_handle)
                            .expect("failed to initialize virtual branches controller");
                    app_handle.manage(vbranch_contoller);

                    let keys_controller = keys::Controller::try_from(&app_handle)
                        .expect("failed to initialize keys controller");
                    app_handle.manage(keys_controller);

                    let users_controller = users::Controller::try_from(&app_handle)
                        .expect("failed to initialize users controller");
                    if let Some(user) = users_controller.get_user().context("failed to get user")? {
                        sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())))
                    }
                    app_handle.manage(users_controller);

                    let app: app::App = app::App::try_from(&tauri_app.app_handle())
                        .expect("failed to initialize app");

                    block_on(app.init()).context("failed to init app")?;

                    app_handle.manage(app);

                    Ok(())
                })
                .plugin(tauri_plugin_window_state::Builder::default().build())
                .invoke_handler(tauri::generate_handler![
                    commands::list_deltas,
                    commands::list_sessions,
                    commands::list_session_files,
                    commands::search,
                    commands::git_remote_branches,
                    commands::git_remote_branches_data,
                    commands::git_head,
                    commands::git_wd_diff,
                    commands::delete_all_data,
                    commands::upsert_bookmark,
                    commands::list_bookmarks,
                    commands::fetch_from_target,
                    commands::mark_resolved,
                    commands::git_set_global_config,
                    commands::git_get_global_config,
                    zip::commands::get_logs_archive_path,
                    zip::commands::get_project_archive_path,
                    zip::commands::get_project_data_archive_path,
                    users::commands::set_user,
                    users::commands::delete_user,
                    users::commands::get_user,
                    projects::commands::add_project,
                    projects::commands::get_project,
                    projects::commands::update_project,
                    projects::commands::delete_project,
                    projects::commands::list_projects,
                    virtual_branches::commands::list_virtual_branches,
                    virtual_branches::commands::create_virtual_branch,
                    virtual_branches::commands::commit_virtual_branch,
                    virtual_branches::commands::get_base_branch_data,
                    virtual_branches::commands::set_base_branch,
                    virtual_branches::commands::update_base_branch,
                    virtual_branches::commands::merge_virtual_branch_upstream,
                    virtual_branches::commands::update_virtual_branch,
                    virtual_branches::commands::delete_virtual_branch,
                    virtual_branches::commands::apply_branch,
                    virtual_branches::commands::unapply_branch,
                    virtual_branches::commands::unapply_ownership,
                    virtual_branches::commands::push_virtual_branch,
                    virtual_branches::commands::create_virtual_branch_from_branch,
                    virtual_branches::commands::can_apply_virtual_branch,
                    virtual_branches::commands::can_apply_remote_branch,
                    virtual_branches::commands::list_remote_commit_files,
                    virtual_branches::commands::reset_virtual_branch,
                    keys::commands::get_public_key,
                ])
                .build(tauri_context)
                .expect("Failed to build tauri app")
                .run(|app_handle, event| match event {
                    tauri::RunEvent::WindowEvent {
                        event: tauri::WindowEvent::Focused(is_focused),
                        ..
                    } => {
                        if is_focused {
                            set_toggle_menu_hide(app_handle)
                                .expect("Failed to set toggle menu hide");
                        } else {
                            set_toggle_menu_show(app_handle)
                                .expect("Failed to set toggle menu show");
                        }
                    }
                    tauri::RunEvent::ExitRequested { api, .. } => {
                        hide_window(app_handle).expect("Failed to hide window");
                        api.prevent_exit();
                    }
                    _ => {}
                });
        });
}

fn get_window(handle: &tauri::AppHandle) -> Option<tauri::Window> {
    handle.get_window("main")
}

#[cfg(not(target_os = "macos"))]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    let app_title = handle.package_info().name.clone();
    let window =
        tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
            .resizable(true)
            .title(app_title)
            .disable_file_drop_handler()
            .min_inner_size(600.0, 300.0)
            .inner_size(800.0, 600.0)
            .build()?;
    tracing::info!("app window created");
    Ok(window)
}

#[cfg(target_os = "macos")]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    let window =
        tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
            .resizable(true)
            .title(handle.package_info().name.clone())
            .min_inner_size(1024.0, 600.0)
            .inner_size(1024.0, 600.0)
            .hidden_title(true)
            .disable_file_drop_handler()
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .build()?;
    tracing::info!("window created");
    Ok(window)
}

fn set_toggle_menu_hide(handle: &tauri::AppHandle) -> tauri::Result<()> {
    handle
        .tray_handle()
        .get_item("toggle")
        .set_title(format!("Hide {}", handle.package_info().name))
}

fn show_window(handle: &tauri::AppHandle) -> tauri::Result<()> {
    set_toggle_menu_hide(handle)?;

    #[cfg(target_os = "macos")]
    handle.show()?;

    if let Some(window) = get_window(handle) {
        window.set_focus()?;

        #[cfg(not(target_os = "macos"))]
        window.hide()?;
    }

    Ok(())
}

fn set_toggle_menu_show(handle: &tauri::AppHandle) -> tauri::Result<()> {
    handle
        .tray_handle()
        .get_item("toggle")
        .set_title(format!("Show {}", handle.package_info().name))
}

fn hide_window(handle: &tauri::AppHandle) -> tauri::Result<()> {
    set_toggle_menu_show(handle)?;

    #[cfg(target_os = "macos")]
    handle.hide()?;

    #[cfg(not(target_os = "macos"))]
    if let Some(window) = get_window(handle) {
        window.hide()?;
    }

    Ok(())
}
