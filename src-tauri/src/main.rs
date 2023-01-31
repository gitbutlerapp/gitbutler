use std::fs::read_to_string;

use tauri::InvokeError;

#[tauri::command]
fn read_file(file_path: &str) -> Result<String, InvokeError> {
    let contents = read_to_string(file_path);
    if contents.is_ok() {
        return Ok(contents.unwrap());
    } else {
        return Err(contents.err().unwrap().to_string().into());
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs_watch::init())
        .invoke_handler(tauri::generate_handler![read_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
