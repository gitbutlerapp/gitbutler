use std::{fs, path::Path};
use tauri::InvokeError;
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};

// return a list of files in directory recursively
fn list_files(path: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                files.append(&mut list_files(&path));
            } else {
                files.push(path.to_str().unwrap().to_string());
            }
        }
    }
    files.sort();
    files
}

// returns a list of files in directory recursively
#[tauri::command]
fn read_dir(path: &str) -> Result<Vec<String>, InvokeError> {
    let path = Path::new(path);
    if path.is_dir() {
        let files = list_files(path);
        return Ok(files);
    } else {
        return Err("Path is not a directory".into());
    }
}

// reads file contents and returns it
#[tauri::command]
fn read_file(file_path: &str) -> Result<String, InvokeError> {
    let contents = fs::read_to_string(file_path);
    if contents.is_ok() {
        return Ok(contents.unwrap());
    } else {
        return Err(contents.err().unwrap().to_string().into());
    }
}

fn main() {
    let colors = ColoredLevelConfig {
        error: Color::Red,
        warn: Color::Yellow,
        debug: Color::Blue,
        info: Color::BrightGreen,
        trace: Color::Cyan,
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_fs_watch::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Debug)
                .with_colors(colors)
                .targets([LogTarget::LogDir, LogTarget::Stdout, LogTarget::Webview])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![read_file, read_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
