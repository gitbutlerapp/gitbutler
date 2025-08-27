//! This module is concerned with writing out the config for claude code.

use anyhow::Result;
use but_action::cli::get_cli_path;
use serde_json::json;

/// Formats the claude code config
pub fn fmt_claude_settings() -> Result<String> {
    let cli_cmd = format!(
        "GITBUTLER_IN_GUI=1 \"{}\"",
        get_cli_path()?.to_string_lossy()
    );
    let pre_cmd = format!("{cli_cmd} claude pre-tool");
    let post_cmd = format!("{cli_cmd} claude post-tool");
    let stop_cmd = format!("{cli_cmd} claude stop");

    // We could just do string formatting, but this ensures that we've at least
    // got valid JSON & does appropriate string escaping.
    let config = json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Edit|MultiEdit|Write",
                "hooks": [{
                    "type": "command",
                    "command": pre_cmd
                }]
            }],
            "PostToolUse": [{
                "matcher": "Edit|MultiEdit|Write",
                "hooks": [{
                    "type": "command",
                    "command": post_cmd
                }]
            }],
            "Stop": [{
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": stop_cmd
                }]
            }]
        }
    });

    Ok(serde_json::to_string(&config)?)
}

pub fn fmt_claude_mcp() -> Result<String> {
    let config = json!({
        "mcpServers": {
            "but-security": {
                "type": "stdio",
                // I don't really know why, but we _don't_ want this to be a string
                "command": get_cli_path()?.to_string_lossy(),
                "args": ["claude", "pp"],
                "env": {}
            }
        }
    });

    Ok(serde_json::to_string(&config)?)
}
