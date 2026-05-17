use anyhow::Result;
use but_api_macros::but_api;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// Configuration for a terminal application.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOption {
    /// Unique identifier for the terminal.
    pub identifier: String,
    /// Human-readable name for display in the UI.
    pub display_name: String,
    /// Platform this terminal is available on (macos, windows, or linux).
    pub platform: String,
}

/// Terminal options ordered by preference (first is recommended default).
const ALL_TERMINALS: &[(&str, &str, &str)] = &[
    // macOS
    ("terminal", "Terminal", "macos"),
    ("iterm2", "iTerm2", "macos"),
    ("ghostty", "Ghostty", "macos"),
    ("warp", "Warp", "macos"),
    ("alacritty-mac", "Alacritty", "macos"),
    ("wezterm-mac", "WezTerm", "macos"),
    ("hyper", "Hyper", "macos"),
    ("kitty", "Kitty", "macos"),
    // Windows — wt first (detected via which), falls back to always-present powershell/cmd
    ("wt", "Windows Terminal", "windows"),
    ("powershell", "PowerShell", "windows"),
    ("cmd", "Command Prompt", "windows"),
    // Linux
    ("gnome-terminal", "GNOME Terminal", "linux"),
    ("konsole", "Konsole", "linux"),
    ("xfce4-terminal", "XFCE Terminal", "linux"),
    ("alacritty", "Alacritty", "linux"),
    ("ghostty", "Ghostty", "linux"),
    ("warp", "Warp", "linux"),
    ("hyper", "Hyper", "linux"),
    ("wezterm", "WezTerm", "linux"),
    ("kitty", "Kitty", "linux"),
    ("cosmic-term", "COSMIC Terminal", "linux"),
    ("ptyxis", "Ptyxis", "linux"),
];

/// Resolves a terminal identifier to the binary name used to detect/launch it.
/// This mapping is shared between terminal detection and the open-in-terminal logic.
pub fn terminal_binary(identifier: &str) -> &str {
    match identifier {
        "warp" => "warp-terminal",
        other => other,
    }
}

/// Returns all available terminal options for the given platform.
#[but_api]
#[instrument(err(Debug))]
pub fn get_terminal_options_for_platform(platform: String) -> Result<Vec<TerminalOption>> {
    Ok(ALL_TERMINALS
        .iter()
        .filter(|(_, _, p)| *p == platform)
        .map(|(id, name, p)| TerminalOption {
            identifier: id.to_string(),
            display_name: name.to_string(),
            platform: p.to_string(),
        })
        .collect())
}

/// Returns the recommended terminal for the platform by probing what is installed.
/// On macOS, Terminal.app is always present so it is returned immediately.
/// On Linux and Windows the ordered list is walked and the first installed one wins.
#[but_api]
#[instrument(err(Debug))]
pub fn get_recommended_terminal_for_platform(platform: String) -> Result<Option<TerminalOption>> {
    // macOS is special-cased because Terminal.app is always available on every Mac and is the system default
    if platform == "macos" {
        return Ok(Some(TerminalOption {
            identifier: "terminal".to_string(),
            display_name: "Terminal".to_string(),
            platform: "macos".to_string(),
        }));
    }

    Ok(get_terminal_options_for_platform(platform)?
        .into_iter()
        .find(|t| which::which(terminal_binary(&t.identifier)).is_ok()))
}
