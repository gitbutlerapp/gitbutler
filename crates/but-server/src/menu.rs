use crate::RequestContext;
use std::{env, fs};

// This function doesn't require Tauri-specific functionality
pub fn get_editor_link_scheme(
    _ctx: &RequestContext,
    _params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let vscodium_installed = check_if_installed("codium");
    let scheme = if vscodium_installed {
        "vscodium"
    } else {
        // Fallback to vscode, as it was the previous behavior
        "vscode"
    };
    Ok(serde_json::to_value(scheme)?)
}

// Note: menu_item_set_enabled is too Tauri-specific for HTTP API
// It requires AppHandle and window management which doesn't exist in HTTP context
// This would need to be handled differently in a web-based UI

fn check_if_installed(executable_name: &str) -> bool {
    match env::var_os("PATH") {
        Some(env_path) => env::split_paths(&env_path).any(|mut path| {
            path.push(executable_name);
            fs::metadata(path).is_ok()
        }),
        None => false,
    }
}
