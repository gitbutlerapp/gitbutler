#[cfg(debug_assertions)]
#[tauri::command(async)]
pub fn env_vars() -> std::collections::BTreeMap<String, String> {
    std::env::vars().collect()
}
