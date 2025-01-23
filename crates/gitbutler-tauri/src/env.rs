use std::collections::BTreeMap;

#[cfg(debug_assertions)]
#[tauri::command(async)]
pub fn env_vars() -> BTreeMap<String, String> {
    std::env::vars().collect()
}
