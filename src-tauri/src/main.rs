fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs_watch::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
